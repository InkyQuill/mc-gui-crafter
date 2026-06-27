use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::animation::Animation;
use crate::project::{
    AssetMetadata, AttachedRegion, AttachedRegionAnchor, AttachedRegionState,
    AttachedRegionStateOverridePatch, CodegenMode, EditScope, Element,
    ElementAttachedRegionStateOverride, ElementStateOverridePatch, ElementType, FillDirection,
    GroupStateOverride, Layer, ModTarget, NineSliceMode, Project, ProjectExportSettings,
    ProjectSessionManager, ProjectState, SemanticGroup, SemanticGroupKind, SlotRole,
    StateOverrideTarget, TextureRenderMode,
};
use crate::session_log::{SessionLogEntry, SessionLogLevel};
use crate::{templates, AppState};

const MCP_PATH: &str = "/mcp";
const SERVER_NAME: &str = "mc-gui-crafter";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct McpServerHandle {
    address: SocketAddr,
    shutdown: mpsc::Sender<()>,
}

impl McpServerHandle {
    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl Drop for McpServerHandle {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Serialize)]
pub struct McpServerStatus {
    pub address: String,
}

pub fn start_web_server(
    app_handle: AppHandle,
    preferred_port: Option<u16>,
) -> Result<McpServerHandle, String> {
    let listener = bind_mcp_listener(preferred_port)?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to configure MCP server: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("Failed to read MCP server address: {error}"))?;
    let (shutdown, shutdown_rx) = mpsc::channel();

    thread::spawn(move || loop {
        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        match listener.accept() {
            Ok((stream, _)) => {
                let app_handle = app_handle.clone();
                thread::spawn(move || {
                    let _ = handle_http_connection(stream, &app_handle);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    });

    Ok(McpServerHandle { address, shutdown })
}

pub(crate) fn bind_mcp_listener(preferred_port: Option<u16>) -> Result<TcpListener, String> {
    if let Some(port) = preferred_port {
        if let Ok(listener) = TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
            return Ok(listener);
        }
    }

    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("Failed to bind MCP server: {error}"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn mcp_status(state: tauri::State<'_, AppState>) -> Option<McpServerStatus> {
    let handle = state.mcp_handle.lock().unwrap();
    handle.as_ref().map(|handle| McpServerStatus {
        address: format!("http://{}/mcp", handle.address()),
    })
}

fn handle_http_connection(mut stream: TcpStream, app_handle: &AppHandle) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let request = read_http_request(&mut stream)?;
    let response = handle_http_request(&request, app_handle);
    stream.write_all(&response)?;
    stream.flush()
}

fn read_http_request(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut chunk = [0; 1024];
    let header_end;
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Ok(buffer);
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            header_end = index;
            break;
        }
        if buffer.len() > 32 * 1024 {
            return Ok(buffer);
        }
    }

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let content_length = header_text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let body_start = header_end + 4;
    while buffer.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }

    Ok(buffer)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn handle_http_request(raw: &[u8], app_handle: &AppHandle) -> Vec<u8> {
    let state = app_handle.state::<AppState>();
    let (status, content_type, body) = match parse_http_request(raw) {
        Ok(request) => route_http_request(request, &state),
        Err(message) => (
            400,
            "application/json",
            serde_json::to_vec(&json_rpc_error(None, -32700, message)).unwrap_or_default(),
        ),
    };

    let status_text = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        415 => "Unsupported Media Type",
        _ => "Internal Server Error",
    };
    let mut response = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(&body);
    response
}

struct HttpRequest<'a> {
    method: &'a str,
    path: &'a str,
    headers: Vec<(&'a str, &'a str)>,
    body: &'a [u8],
}

fn parse_http_request(raw: &[u8]) -> Result<HttpRequest<'_>, String> {
    let header_end = find_header_end(raw).ok_or("Malformed HTTP request".to_string())?;
    let header = std::str::from_utf8(&raw[..header_end])
        .map_err(|_| "HTTP headers must be valid UTF-8".to_string())?;
    let mut lines = header.lines();
    let request_line = lines.next().ok_or("Missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or("Missing HTTP method".to_string())?;
    let path = request_parts
        .next()
        .ok_or("Missing HTTP path".to_string())?;
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim(), value.trim()))
        })
        .collect();
    let body = &raw[header_end + 4..];

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn route_http_request(request: HttpRequest<'_>, state: &AppState) -> (u16, &'static str, Vec<u8>) {
    if request.path != MCP_PATH {
        return (404, "text/plain", b"Not Found".to_vec());
    }
    if !origin_is_allowed(&request) {
        return (400, "text/plain", b"Disallowed Origin".to_vec());
    }
    if request.method == "GET" {
        return (405, "text/plain", b"SSE is not supported".to_vec());
    }
    if request.method != "POST" {
        return (405, "text/plain", b"Method Not Allowed".to_vec());
    }
    if !accepts_json(&request) {
        return (
            406,
            "text/plain",
            b"Accept must allow application/json".to_vec(),
        );
    }
    if !content_type_is_json(&request) {
        return (
            415,
            "text/plain",
            b"Content-Type must be application/json".to_vec(),
        );
    }

    match handle_json_rpc_bytes(request.body, state) {
        RpcReply::Response(response) => {
            let body = serde_json::to_vec(&response).unwrap_or_default();
            (200, "application/json", body)
        }
        RpcReply::Notification => (202, "application/json", Vec::new()),
    }
}

fn origin_is_allowed(request: &HttpRequest<'_>) -> bool {
    let Some(origin) = header(request, "origin") else {
        return true;
    };
    let Some(host) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let host = host.split('/').next().unwrap_or(host);
    let host = host
        .strip_prefix('[')
        .and_then(|value| value.split(']').next())
        .unwrap_or_else(|| host.split(':').next().unwrap_or(host));
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

fn accepts_json(request: &HttpRequest<'_>) -> bool {
    header(request, "accept").is_none_or(|accept| {
        accept
            .split(',')
            .map(|part| part.split(';').next().unwrap_or("").trim())
            .any(|mime| matches!(mime, "*/*" | "application/*" | "application/json"))
    })
}

fn content_type_is_json(request: &HttpRequest<'_>) -> bool {
    header(request, "content-type").is_some_and(|content_type| {
        content_type
            .split(';')
            .next()
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
    })
}

fn header<'a>(request: &'a HttpRequest<'_>, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| *value)
}

enum RpcReply {
    Response(JsonRpcResponse),
    Notification,
}

fn handle_json_rpc_bytes(bytes: &[u8], state: &AppState) -> RpcReply {
    let value: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(error) => {
            return RpcReply::Response(json_rpc_error(
                None,
                -32700,
                format!("Parse error: {error}"),
            ));
        }
    };
    handle_json_rpc_value(value, state)
}

fn handle_json_rpc_value(value: serde_json::Value, state: &AppState) -> RpcReply {
    let id = value.get("id").cloned();
    let request: JsonRpcRequest = match serde_json::from_value(value) {
        Ok(request) => request,
        Err(error) => {
            return RpcReply::Response(json_rpc_error(
                id,
                -32600,
                format!("Invalid Request: {error}"),
            ));
        }
    };
    if request.jsonrpc != "2.0" {
        return RpcReply::Response(json_rpc_error(
            request.id,
            -32600,
            "Invalid Request: jsonrpc must be \"2.0\"",
        ));
    }

    if request.id.is_none() {
        if request.method == "notifications/initialized" {
            return RpcReply::Notification;
        }
        return RpcReply::Notification;
    }

    RpcReply::Response(handle_mcp_method(request, state))
}

fn handle_mcp_method(request: JsonRpcRequest, state: &AppState) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => json_rpc_result(
            request.id,
            serde_json::json!({
                "protocolVersion": "2025-03-26",
                "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
                "capabilities": { "tools": {} }
            }),
        ),
        "tools/list" => json_rpc_result(
            request.id,
            serde_json::json!({ "tools": get_tool_definitions() }),
        ),
        "tools/call" => {
            let Some(params) = request.params else {
                return json_rpc_error(request.id, -32602, "Missing params");
            };
            let Some(tool_name) = params.get("name").and_then(|value| value.as_str()) else {
                return json_rpc_error(request.id, -32602, "Missing tool name");
            };
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

            let snapshot_before = if is_mutating_tool(tool_name) {
                project_mutation_snapshot(state, &arguments)
            } else {
                None
            };

            let started = crate::session_log::timestamp_millis();
            match execute_tool(tool_name, &arguments, state) {
                Ok(content) => {
                    log_mcp_tool_success(state, tool_name, &arguments, &content, started);
                    if should_emit_project_changed(tool_name, state, &arguments, snapshot_before) {
                        emit_project_changed(state, tool_name);
                    }
                    json_rpc_result(
                        request.id,
                        serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&content).unwrap_or_else(|_| "{}".to_string())
                            }]
                        }),
                    )
                }
                Err(message) => {
                    log_mcp_tool_error(state, tool_name, &arguments, &message, started);
                    json_rpc_error(request.id, -32000, message)
                }
            }
        }
        _ => json_rpc_error(
            request.id,
            -32601,
            format!("Method not found: {}", request.method),
        ),
    }
}

fn json_rpc_result(id: Option<serde_json::Value>, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn json_rpc_error(
    id: Option<serde_json::Value>,
    code: i32,
    message: impl Into<String>,
) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.into(),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectMutationSnapshot {
    project_id: String,
    revision: u64,
    is_dirty: bool,
    project_path: Option<String>,
    active_state_id: Option<String>,
    edit_scope: EditScope,
    active_project_id: Option<String>,
}

fn project_mutation_snapshot(
    state: &AppState,
    args: &serde_json::Value,
) -> Option<ProjectMutationSnapshot> {
    let sessions = state.sessions.lock().unwrap();
    let project_id = optional_string(args, "project_id");
    let active_project_id = sessions
        .active_session()
        .ok()
        .map(|session| session.id.clone());
    let session = sessions.resolve(project_id.as_deref()).ok()?;
    Some(ProjectMutationSnapshot {
        project_id: session.id.clone(),
        revision: session.revision,
        is_dirty: session.project.is_dirty,
        project_path: session.project.project_path.clone(),
        active_state_id: session.active_state_id.clone(),
        edit_scope: session.edit_scope,
        active_project_id,
    })
}

fn should_emit_project_changed(
    tool_name: &str,
    state: &AppState,
    args: &serde_json::Value,
    snapshot_before: Option<ProjectMutationSnapshot>,
) -> bool {
    if !is_mutating_tool(tool_name) {
        return false;
    }

    let snapshot_after = project_mutation_snapshot(state, args);
    match (snapshot_before, snapshot_after) {
        (Some(before), Some(after)) => before != after,
        _ => true,
    }
}

fn emit_project_changed(state: &AppState, tool_name: &str) {
    let Some(handle) = state.app_handle.lock().unwrap().clone() else {
        return;
    };
    let _ = handle.emit(
        "project-changed",
        serde_json::json!({
            "source": "mcp",
            "tool": tool_name,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        }),
    );
}

fn is_mutating_tool(tool_name: &str) -> bool {
    !matches!(
        tool_name,
        "project_summary"
            | "session_report"
            | "project_list_sessions"
            | "project_get_active"
            | "project_export_preview"
            | "project_export"
            | "project_render"
            | "project_screenshot"
            | "state_list"
            | "element_list"
            | "group_list"
            | "attached_region_list"
            | "animation_list"
            | "asset_list"
            | "asset_get_data_url"
            | "gui_template_list"
            | "schema_discover"
    )
}

fn log_mcp_tool_success(
    state: &AppState,
    tool_name: &str,
    arguments: &serde_json::Value,
    content: &serde_json::Value,
    started: u128,
) {
    let level = if content
        .get("errors")
        .and_then(|value| value.as_array())
        .is_some_and(|errors| !errors.is_empty())
    {
        SessionLogLevel::Error
    } else if content
        .get("warnings")
        .and_then(|value| value.as_array())
        .is_some_and(|warnings| !warnings.is_empty())
    {
        SessionLogLevel::Warning
    } else {
        SessionLogLevel::Info
    };
    let _ = state.session_log.lock().unwrap().append(SessionLogEntry {
        level,
        source: "mcp".to_string(),
        category: "tool_call".to_string(),
        message: format!("{tool_name} completed"),
        details: Some(serde_json::json!({
            "tool": tool_name,
            "duration_ms": crate::session_log::timestamp_millis().saturating_sub(started),
            "arguments": compact_log_value(arguments.clone()),
            "warnings": content.get("warnings").cloned().unwrap_or(serde_json::Value::Null),
            "errors": content.get("errors").cloned().unwrap_or(serde_json::Value::Null),
        })),
    });
}

fn log_mcp_tool_error(
    state: &AppState,
    tool_name: &str,
    arguments: &serde_json::Value,
    message: &str,
    started: u128,
) {
    let _ = state.session_log.lock().unwrap().append(SessionLogEntry {
        level: SessionLogLevel::Error,
        source: "mcp".to_string(),
        category: "tool_call".to_string(),
        message: format!("{tool_name} failed"),
        details: Some(serde_json::json!({
            "tool": tool_name,
            "duration_ms": crate::session_log::timestamp_millis().saturating_sub(started),
            "arguments": compact_log_value(arguments.clone()),
            "error": message,
        })),
    });
}

fn compact_log_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) if text.starts_with("data:image/") => {
            serde_json::Value::String(format!("[data url {} chars]", text.len()))
        }
        serde_json::Value::String(text) if text.len() > 1_000 => serde_json::Value::String(
            format!("{}...", text.chars().take(1_000).collect::<String>()),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().take(25).map(compact_log_value).collect())
        }
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .into_iter()
                .take(80)
                .map(|(key, value)| (key, compact_log_value(value)))
                .collect(),
        ),
        other => other,
    }
}

fn get_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        td(
            "project_new",
            "Create a new GUI project",
            object_schema(vec![
                ("name", string_schema("Project name"), true),
                ("width", string_type_schema("integer", "GUI width"), false),
                ("height", string_type_schema("integer", "GUI height"), false),
                ("template", string_schema("Template name"), false),
                ("mod_target", string_schema(mod_target_description()), false),
            ]),
        ),
        td(
            "project_open",
            "Open an existing .mcgui project",
            props(&[("path", "string", "Path to .mcgui file", true)]),
        ),
        td("project_save", "Save a project", project_props(&[])),
        td(
            "project_save_as",
            "Save a project to a .mcgui path",
            project_props(&[("path", "string", "Path to write the .mcgui project", true)]),
        ),
        td(
            "project_resize",
            "Resize the project GUI canvas without moving or scaling elements",
            project_props(&[
                (
                    "width",
                    "integer",
                    "New GUI width; must be greater than zero",
                    true,
                ),
                (
                    "height",
                    "integer",
                    "New GUI height; must be greater than zero",
                    true,
                ),
            ]),
        ),
        td(
            "project_export_preview",
            "Preview generated export files and preflight warnings",
            export_props(),
        ),
        td(
            "project_export",
            "Export generated mod files",
            export_props(),
        ),
        td(
            "project_render",
            "Render the current project to a PNG image and return compact metadata",
            project_props(&[
                (
                    "output_path",
                    "string",
                    "Optional PNG path to write; temp file is used when omitted",
                    false,
                ),
                (
                    "include_data_url",
                    "boolean",
                    "Include data:image/png;base64 payload; defaults to false",
                    false,
                ),
                (
                    "state_id",
                    "string",
                    "Optional editable state ID to render as an effective layout",
                    false,
                ),
            ]),
        ),
        td(
            "session_report",
            "Append an AI/user feedback report to the active app session log",
            object_schema(vec![
                (
                    "summary",
                    string_schema("Short report summary for issue triage"),
                    true,
                ),
                (
                    "severity",
                    serde_json::json!({
                        "type": "string",
                        "enum": ["info", "warning", "error"],
                        "description": "Report severity"
                    }),
                    false,
                ),
                (
                    "details",
                    json_schema("Detailed report, reproduction notes, observed warnings, or suggested fix"),
                    false,
                ),
                (
                    "project_id",
                    string_schema("Optional project session ID related to this report"),
                    false,
                ),
            ]),
        ),
        td(
            "project_screenshot",
            "Deprecated alias for project_render; renders the current project to a PNG image",
            project_props(&[
                (
                    "output_path",
                    "string",
                    "Optional PNG path to write; temp file is used when omitted",
                    false,
                ),
                (
                    "include_data_url",
                    "boolean",
                    "Include data:image/png;base64 payload; defaults to false",
                    false,
                ),
                (
                    "state_id",
                    "string",
                    "Optional editable state ID to render as an effective layout",
                    false,
                ),
            ]),
        ),
        td(
            "project_summary",
            "Get a project summary",
            project_props(&[]),
        ),
        td(
            "project_export_settings_update",
            "Update project code generation/export settings",
            project_schema(vec![
                (
                    "codegen_mode",
                    string_schema(codegen_mode_description()),
                    false,
                ),
                (
                    "generate_runtime_helpers",
                    string_type_schema("boolean", "Generate runtime helper hooks"),
                    false,
                ),
                (
                    "generate_semantic_registry",
                    string_type_schema("boolean", "Generate semantic registry in modular mode"),
                    false,
                ),
            ]),
        ),
        td(
            "project_semantic_groups_update",
            "Replace project semantic group definitions",
            semantic_groups_props(),
        ),
        td(
            "project_list_sessions",
            "List open project sessions",
            props(&[]),
        ),
        td(
            "project_get_active",
            "Get active project session and project",
            props(&[]),
        ),
        td(
            "schema_discover",
            "Return accepted MCP enum values, editable fields, defaults, and alpha schema notes",
            props(&[]),
        ),
        td(
            "state_list",
            "List editable state variants",
            project_props(&[]),
        ),
        td(
            "state_add",
            "Add an editable state variant",
            project_schema(vec![
                ("id", string_schema("State ID"), true),
                ("label", string_schema("Human-readable state label"), true),
                (
                    "description",
                    string_schema("Optional state description"),
                    false,
                ),
                (
                    "initial",
                    string_type_schema("boolean", "Mark this as the initial state"),
                    false,
                ),
                (
                    "export_role",
                    string_schema("Optional export role, such as collapsed or expanded"),
                    false,
                ),
            ]),
        ),
        td(
            "state_update",
            "Update an editable state variant",
            project_schema(vec![
                ("id", string_schema("State ID"), true),
                ("label", string_schema("Human-readable state label"), false),
                (
                    "description",
                    serde_json::json!({
                        "type": ["string", "null"],
                        "description": "Optional state description; null clears it"
                    }),
                    false,
                ),
                (
                    "initial",
                    string_type_schema("boolean", "Mark this as the initial state"),
                    false,
                ),
                (
                    "export_role",
                    serde_json::json!({
                        "type": ["string", "null"],
                        "description": "Optional export role; null clears it"
                    }),
                    false,
                ),
            ]),
        ),
        td(
            "state_remove",
            "Remove an editable state variant and its overrides",
            project_props(&[("id", "string", "State ID", true)]),
        ),
        td(
            "state_set_active",
            "Set the session active state and edit scope without changing project data",
            project_schema(vec![
                (
                    "state_id",
                    serde_json::json!({
                        "type": ["string", "null"],
                        "description": "State ID to activate; null clears active state"
                    }),
                    false,
                ),
                (
                    "edit_scope",
                    serde_json::json!({
                        "type": "string",
                        "enum": ["base", "state"],
                        "description": "Editor edit scope"
                    }),
                    false,
                ),
            ]),
        ),
        td(
            "state_override_update",
            "Update alpha state overrides for an element, attached region, or group",
            project_schema(vec![
                ("state_id", string_schema("State ID"), true),
                (
                    "target_type",
                    serde_json::json!({
                        "type": "string",
                        "enum": ["element", "attached_region", "group"],
                        "description": "Override target kind"
                    }),
                    true,
                ),
                ("target_id", string_schema("Target ID"), true),
                ("fields", object_schema(Vec::new()), true),
            ]),
        ),
        td(
            "state_override_clear",
            "Clear alpha state overrides for a target or one override field",
            project_schema(vec![
                ("state_id", string_schema("State ID"), true),
                (
                    "target_type",
                    serde_json::json!({
                        "type": "string",
                        "enum": ["element", "attached_region", "group"],
                        "description": "Override target kind"
                    }),
                    true,
                ),
                ("target_id", string_schema("Target ID"), true),
                (
                    "field",
                    string_schema("Override field to clear; omit to clear target"),
                    false,
                ),
            ]),
        ),
        td(
            "project_undo",
            "Undo the last project mutation",
            project_props(&[]),
        ),
        td(
            "project_redo",
            "Redo the last undone project mutation",
            project_props(&[]),
        ),
        td(
            "element_add",
            "Add an element",
            project_props(&[(
                "element",
                "object",
                "Element object; flat element fields are also accepted",
                false,
            )]),
        ),
        td(
            "element_add_many",
            "Add multiple elements atomically",
            element_add_many_props(),
        ),
        td(
            "slot_grid_add",
            "Create a grouped grid of slot elements with semantic metadata",
            slot_grid_props(),
        ),
        td(
            "element_move",
            "Move an element",
            project_props(&[
                ("id", "string", "Element ID", true),
                ("x", "integer", "New X", true),
                ("y", "integer", "New Y", true),
            ]),
        ),
        td(
            "element_update",
            "Update element fields",
            project_props(&[
                ("id", "string", "Element ID", true),
                ("changes", "object", "Element fields to update", true),
                (
                    "state_id",
                    "string",
                    "Optional state ID; when present writes alpha state overrides instead of base fields",
                    false,
                ),
                (
                    "edit_scope",
                    "string",
                    "Set to state to write alpha state overrides; defaults to base",
                    false,
                ),
            ]),
        ),
        td(
            "element_update_many",
            "Update multiple elements atomically in one project revision",
            project_schema(vec![
                (
                    "updates",
                    serde_json::json!({
                        "type": "array",
                        "description": "Element update patches",
                        "items": {
                            "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "changes": { "type": "object" },
                                    "state_id": {
                                        "type": "string",
                                        "description": "Optional state ID for this update"
                                    },
                                    "edit_scope": {
                                        "type": "string",
                                        "enum": ["base", "state"],
                                        "description": "Set to state to write alpha state overrides"
                                    }
                                },
                            "required": ["id", "changes"]
                        }
                    }),
                    true,
                ),
                (
                    "state_id",
                    serde_json::json!({
                        "type": "string",
                        "description": "Optional batch-wide state ID for all updates that do not provide their own state_id"
                    }),
                    false,
                ),
                (
                    "edit_scope",
                    serde_json::json!({
                        "type": "string",
                        "enum": ["base", "state"],
                        "description": "Optional batch-wide edit scope for all updates that do not provide their own edit_scope"
                    }),
                    false,
                ),
            ]),
        ),
        td(
            "element_resize",
            "Resize an element",
            project_props(&[
                ("id", "string", "Element ID", true),
                ("x", "integer", "New X; current X when omitted", false),
                ("y", "integer", "New Y; current Y when omitted", false),
                ("width", "integer", "New width", true),
                ("height", "integer", "New height", true),
            ]),
        ),
        td(
            "element_reorder",
            "Move an element to a z-order index",
            project_props(&[
                ("id", "string", "Element ID", true),
                ("index", "integer", "Target index", true),
            ]),
        ),
        td(
            "element_remove",
            "Remove an element",
            project_props(&[("id", "string", "Element ID", true)]),
        ),
        td("element_list", "List elements", project_props(&[])),
        td(
            "group_create",
            "Group two or more elements",
            project_props(&[
                ("element_ids", "array", "Element IDs to group", true),
                ("group_id", "string", "Optional group ID", false),
            ]),
        ),
        td(
            "group_upsert",
            "Create or replace a project group membership without ungrouping first",
            project_props(&[
                ("group_id", "string", "Group ID", true),
                (
                    "element_ids",
                    "array",
                    "Replacement element IDs for the group",
                    true,
                ),
            ]),
        ),
        td(
            "group_ungroup",
            "Remove a group while keeping its elements",
            project_props(&[("group_id", "string", "Group ID", true)]),
        ),
        td("group_list", "List groups", project_props(&[])),
        td(
            "attached_region_add",
            "Add an attached region",
            attached_region_props(true),
        ),
        td(
            "attached_region_update",
            "Update attached region fields",
            attached_region_update_props(),
        ),
        td(
            "attached_region_remove",
            "Remove an attached region",
            project_props(&[("id", "string", "Attached region ID", true)]),
        ),
        td(
            "attached_region_list",
            "List attached regions",
            project_props(&[]),
        ),
        td(
            "attached_region_move_with_elements",
            "Move an attached region and its attached child elements",
            project_props(&[
                ("id", "string", "Attached region ID", true),
                ("x", "integer", "New X", true),
                ("y", "integer", "New Y", true),
            ]),
        ),
        td(
            "animation_create",
            "Create an animation",
            project_props(&[(
                "animation",
                "object",
                "Animation object; flat animation fields are also accepted",
                false,
            )]),
        ),
        td(
            "animation_update",
            "Update animation fields",
            project_props(&[
                ("id", "string", "Animation ID", true),
                ("changes", "object", "Animation fields to update", true),
            ]),
        ),
        td(
            "animation_remove",
            "Remove an animation",
            project_props(&[("id", "string", "Animation ID", true)]),
        ),
        td(
            "animation_bind",
            "Bind animation to element",
            project_props(&[
                ("element_id", "string", "Element ID", true),
                ("animation_id", "string", "Animation ID", true),
            ]),
        ),
        td(
            "animation_unbind",
            "Unbind an element animation",
            project_props(&[("element_id", "string", "Element ID", true)]),
        ),
        td("animation_list", "List animations", project_props(&[])),
        td(
            "asset_import",
            "Import a PNG asset from disk",
            project_props(&[
                ("file_path", "string", "PNG path", true),
                ("name", "string", "Optional project asset name", false),
            ]),
        ),
        td(
            "asset_update",
            "Replace an existing asset with PNG data URL",
            project_props(&[
                ("name", "string", "Asset name", true),
                ("data_url", "string", "data:image/png;base64,...", true),
            ]),
        ),
        td(
            "asset_metadata_update",
            "Update metadata for an existing asset",
            project_schema(vec![
                ("name", string_schema("Asset name"), true),
                ("metadata", asset_metadata_schema(), true),
            ]),
        ),
        td(
            "asset_remove",
            "Remove an asset",
            project_props(&[("name", "string", "Asset name", true)]),
        ),
        td(
            "asset_get_data_url",
            "Get an asset as data URL",
            project_props(&[("name", "string", "Asset name", true)]),
        ),
        td("asset_list", "List assets", project_props(&[])),
        td("gui_template_list", "List available templates", props(&[])),
    ]
}

fn td(name: &str, description: &str, schema: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "name": name, "description": description, "inputSchema": schema })
}

fn project_props(items: &[(&str, &str, &str, bool)]) -> serde_json::Value {
    let mut with_project = vec![(
        "project_id",
        "string",
        "Optional project session ID; active session is used when omitted",
        false,
    )];
    with_project.extend_from_slice(items);
    props(&with_project)
}

fn string_schema(description: impl Into<String>) -> serde_json::Value {
    string_type_schema("string", description)
}

fn json_schema(description: impl Into<String>) -> serde_json::Value {
    serde_json::json!({
        "description": description.into()
    })
}

fn string_type_schema(typ: &str, description: impl Into<String>) -> serde_json::Value {
    serde_json::json!({ "type": typ, "description": description.into() })
}

fn export_props() -> serde_json::Value {
    project_schema(vec![
        ("target", string_schema(mod_target_description()), true),
        ("mod_id", string_schema("Minecraft mod id"), true),
        ("package", string_schema("Java package name"), true),
        (
            "class_name",
            string_schema("Generated Screen class name"),
            true,
        ),
        (
            "output_dir",
            string_schema("Directory where export files are written"),
            true,
        ),
        (
            "codegen_mode",
            string_schema(codegen_mode_description()),
            false,
        ),
        (
            "generate_runtime_helpers",
            string_type_schema("boolean", "Generate runtime helper hooks"),
            false,
        ),
        (
            "generate_semantic_registry",
            string_type_schema("boolean", "Generate semantic registry in modular mode"),
            false,
        ),
        (
            "export_scope",
            serde_json::json!({
                "type": "string",
                "enum": ["full_mod", "textures_only"],
                "description": "Export a full mod scaffold or only generated texture assets"
            }),
            false,
        ),
        (
            "overwrite",
            string_type_schema(
                "boolean",
                "Allow overwriting planned generated files without existing-file warnings",
            ),
            false,
        ),
        (
            "state_id",
            string_schema("Optional editable state ID to export as an effective layout"),
            false,
        ),
    ])
}

fn semantic_groups_props() -> serde_json::Value {
    project_schema(vec![(
        "semantic_groups",
        serde_json::json!({
            "type": "array",
            "description": "Semantic group array",
            "items": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Semantic group ID" },
                    "kind": { "type": "string", "description": semantic_group_kind_description() },
                    "columns": { "type": "integer", "description": "Grid column count" },
                    "visible_rows": { "type": "integer", "description": "Visible row count" },
                    "total_rows": { "type": "integer", "description": "Total row count" },
                    "slot_count": { "type": "integer", "description": "Total slot count" },
                    "member_ids": {
                        "type": "array",
                        "description": "Explicit element IDs that belong to this semantic group",
                        "items": { "type": "string" }
                    },
                    "data_source": { "type": "string", "description": "Semantic data source key" },
                    "scroll_binding": { "type": "string", "description": "Scroll binding ID" },
                    "dynamic_height": { "type": "boolean", "description": "Whether this group can change height dynamically" }
                },
                "required": ["id", "kind"]
            }
        }),
        true,
    )])
}

fn asset_metadata_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "description": "Asset metadata fields such as dimensions and nine-slice guides",
        "properties": {
            "width": { "type": "integer", "description": "Optional asset width in pixels" },
            "height": { "type": "integer", "description": "Optional asset height in pixels" },
            "nine_slice": {
                "type": "object",
                "description": "Nine-slice guide distances and repeat modes",
                "properties": {
                    "left": { "type": "integer", "description": "Fixed left guide width" },
                    "right": { "type": "integer", "description": "Fixed right guide width" },
                    "top": { "type": "integer", "description": "Fixed top guide height" },
                    "bottom": { "type": "integer", "description": "Fixed bottom guide height" },
                    "edge_mode": { "type": "string", "enum": ["tile", "stretch"], "description": "How nine-slice edges repeat" },
                    "center_mode": { "type": "string", "enum": ["tile", "stretch"], "description": "How the nine-slice center repeats" }
                },
                "required": ["left", "right", "top", "bottom"]
            }
        }
    })
}

fn element_add_many_props() -> serde_json::Value {
    project_schema(vec![(
        "elements",
        serde_json::json!({
            "type": "array",
            "description": "Element objects to add atomically",
            "items": { "type": "object" }
        }),
        true,
    )])
}

fn slot_grid_props() -> serde_json::Value {
    project_schema(vec![
        (
            "id_prefix",
            serde_json::json!({ "type": "string", "description": "Prefix for generated element IDs" }),
            true,
        ),
        (
            "x",
            serde_json::json!({ "type": "integer", "description": "Grid origin X" }),
            true,
        ),
        (
            "y",
            serde_json::json!({ "type": "integer", "description": "Grid origin Y" }),
            true,
        ),
        (
            "columns",
            serde_json::json!({ "type": "integer", "description": "Grid column count" }),
            true,
        ),
        (
            "rows",
            serde_json::json!({ "type": "integer", "description": "Grid row count" }),
            true,
        ),
        (
            "slot_size",
            serde_json::json!({ "type": "integer", "description": "Slot size; defaults to 18" }),
            false,
        ),
        (
            "spacing",
            serde_json::json!({ "type": "integer", "description": "Distance between slot origins; defaults to 18" }),
            false,
        ),
        (
            "slot_role",
            serde_json::json!({ "type": "string", "description": slot_role_description() }),
            false,
        ),
        (
            "inventory_group",
            serde_json::json!({ "type": "string", "description": "Inventory group ID for generated slots" }),
            false,
        ),
        (
            "slot_index_start",
            serde_json::json!({ "type": "integer", "description": "First slot index; defaults to 0" }),
            false,
        ),
        (
            "group_id",
            serde_json::json!({ "type": "string", "description": "Optional project group ID" }),
            false,
        ),
        (
            "semantic_group_kind",
            serde_json::json!({ "type": "string", "description": semantic_group_kind_description() }),
            false,
        ),
        (
            "slot_count",
            serde_json::json!({ "type": "integer", "description": "Semantic total slot count" }),
            false,
        ),
        (
            "scroll_binding",
            serde_json::json!({ "type": "string", "description": "Scroll binding ID for generated slots and semantic metadata" }),
            false,
        ),
    ])
}

fn attached_region_props(require_region: bool) -> serde_json::Value {
    project_schema(vec![
        (
            "id",
            serde_json::json!({ "type": "string", "description": "Attached region ID" }),
            require_region,
        ),
        (
            "anchor",
            serde_json::json!({ "type": "string", "description": attached_region_anchor_description() }),
            require_region,
        ),
        (
            "x",
            serde_json::json!({ "type": "integer", "description": "Attached region X" }),
            require_region,
        ),
        (
            "y",
            serde_json::json!({ "type": "integer", "description": "Attached region Y" }),
            require_region,
        ),
        (
            "width",
            serde_json::json!({ "type": "integer", "description": "Attached region width" }),
            require_region,
        ),
        (
            "height",
            serde_json::json!({ "type": "integer", "description": "Attached region height" }),
            require_region,
        ),
        (
            "state",
            serde_json::json!({ "type": "string", "description": format!("{} Defaults to static when omitted.", attached_region_state_description()) }),
            false,
        ),
        (
            "kind",
            serde_json::json!({ "type": "string", "description": "Attached region kind" }),
            false,
        ),
        (
            "semantic_group",
            serde_json::json!({ "type": "string", "description": "Semantic group ID for this attached region" }),
            false,
        ),
        (
            "visible",
            serde_json::json!({ "type": "boolean", "description": "Whether this attached region is visible. Defaults to true when omitted." }),
            false,
        ),
    ])
}

fn attached_region_update_props() -> serde_json::Value {
    project_schema(vec![
        (
            "id",
            serde_json::json!({ "type": "string", "description": "Attached region ID" }),
            true,
        ),
        (
            "changes",
            serde_json::json!({
                "type": "object",
                "description": "Attached region fields to update; id cannot be changed.",
                "properties": {
                    "anchor": {
                        "type": "string",
                        "description": attached_region_anchor_description()
                    },
                    "x": {
                        "type": "integer",
                        "description": "Attached region X coordinate"
                    },
                    "y": {
                        "type": "integer",
                        "description": "Attached region Y coordinate"
                    },
                    "width": {
                        "type": "integer",
                        "description": "Attached region width"
                    },
                    "height": {
                        "type": "integer",
                        "description": "Attached region height"
                    },
                    "state": {
                        "type": "string",
                        "description": attached_region_state_description()
                    },
                    "kind": {
                        "type": "string",
                        "description": "Attached region kind"
                    },
                    "semantic_group": {
                        "type": "string",
                        "description": "Semantic group ID for this attached region"
                    },
                    "visible": {
                        "type": "boolean",
                        "description": "Whether this attached region is visible"
                    }
                }
            }),
            true,
        ),
    ])
}

fn props(items: &[(&str, &str, &str, bool)]) -> serde_json::Value {
    let mut required = Vec::new();
    let mut properties = serde_json::Map::new();
    for (name, typ, description, is_required) in items {
        properties.insert(
            (*name).to_string(),
            serde_json::json!({ "type": typ, "description": description }),
        );
        if *is_required {
            required.push((*name).to_string());
        }
    }
    serde_json::json!({ "type": "object", "properties": properties, "required": required })
}

fn project_schema(items: Vec<(&str, serde_json::Value, bool)>) -> serde_json::Value {
    let mut required = Vec::new();
    let mut properties = serde_json::Map::new();
    properties.insert(
        "project_id".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Optional project session ID; active session is used when omitted"
        }),
    );
    for (name, schema, is_required) in items {
        properties.insert(name.to_string(), schema);
        if is_required {
            required.push(name.to_string());
        }
    }
    serde_json::json!({ "type": "object", "properties": properties, "required": required })
}

fn object_schema(items: Vec<(&str, serde_json::Value, bool)>) -> serde_json::Value {
    let mut required = Vec::new();
    let mut properties = serde_json::Map::new();
    for (name, schema, is_required) in items {
        properties.insert(name.to_string(), schema);
        if is_required {
            required.push(name.to_string());
        }
    }
    serde_json::json!({ "type": "object", "properties": properties, "required": required })
}

fn serde_values<T: Serialize>(values: impl IntoIterator<Item = T>) -> serde_json::Value {
    serde_json::to_value(values.into_iter().collect::<Vec<_>>()).unwrap()
}

fn serde_variant_names<T: Serialize>(values: impl IntoIterator<Item = T>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| {
            serde_json::to_value(value)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

fn accepted_values_sentence<T: Serialize>(values: impl IntoIterator<Item = T>) -> String {
    format!(
        "Accepted values: {}.",
        serde_variant_names(values).join(", ")
    )
}

fn mod_target_description() -> String {
    accepted_values_sentence(ModTarget::variants())
}

fn codegen_mode_description() -> String {
    accepted_values_sentence(CodegenMode::variants())
}

fn slot_role_description() -> String {
    accepted_values_sentence(SlotRole::variants())
}

fn semantic_group_kind_description() -> String {
    accepted_values_sentence(SemanticGroupKind::variants())
}

fn attached_region_anchor_description() -> String {
    format!(
        "Attached region anchor. {}",
        accepted_values_sentence(AttachedRegionAnchor::variants())
    )
}

fn attached_region_state_description() -> String {
    format!(
        "Attached region state. {} Toggleable is metadata only in this release.",
        accepted_values_sentence(AttachedRegionState::variants())
    )
}

fn schema_discover() -> serde_json::Value {
    let export_defaults = ProjectExportSettings::default();

    serde_json::json!({
        "mod_targets": serde_values(ModTarget::variants()),
        "element_types": serde_values(ElementType::variants()),
        "slot_roles": serde_values(SlotRole::variants()),
        "semantic_group_kinds": serde_values(SemanticGroupKind::variants()),
        "attached_region_anchors": serde_values(AttachedRegionAnchor::variants()),
        "attached_region_states": serde_values(AttachedRegionState::variants()),
        "fill_directions": serde_values(FillDirection::variants()),
        "layers": serde_values(Layer::variants()),
        "texture_render_modes": serde_values(TextureRenderMode::variants()),
        "nine_slice_modes": serde_values(NineSliceMode::variants()),
        "export_settings": {
            "codegen_modes": serde_values(CodegenMode::variants()),
            "codegen_mode_default": serde_json::to_value(&export_defaults.codegen_mode).unwrap(),
            "generate_runtime_helpers_default": export_defaults.generate_runtime_helpers,
            "generate_semantic_registry_default": export_defaults.generate_semantic_registry
        },
        "editable_element_fields": [
            "x",
            "y",
            "width",
            "height",
            "size",
            "asset",
            "icon",
            "icon_uv",
            "tooltip",
            "direction",
            "content",
            "font",
            "color",
            "shadow",
            "animation",
            "visible",
            "uv",
            "render_mode",
            "nine_slice",
            "layer",
            "slot_role",
            "slot_index",
            "inventory_group",
            "scroll_binding",
            "scroll_min",
            "scroll_max",
            "visible_rows",
            "total_rows",
            "columns",
            "target_group",
            "binding",
            "dock",
            "open_width",
            "open_height",
            "attached_region"
        ],
        "state_variants": {
            "state_fields": ["id", "label", "description", "initial", "export_role"],
            "element_override_fields": ["visible", "x", "y", "width", "height", "attached_region", "layer"],
            "attached_region_override_fields": ["visible", "x", "y", "width", "height"],
            "group_override_fields": ["visible"],
            "edit_scopes": ["base", "state"],
            "tools": [
                "state_list",
                "state_add",
                "state_update",
                "state_remove",
                "state_set_active",
                "state_override_update",
                "state_override_clear"
            ]
        },
        "tools_accepting_state_id": [
            "element_update",
            "element_update_many",
            "project_render",
            "project_screenshot",
            "project_export_preview",
            "project_export"
        ],
        "asset_metadata_fields": ["width", "height", "nine_slice"],
        "serialization_defaults": schema_serialization_defaults()
    })
}

fn schema_serialization_defaults() -> serde_json::Value {
    let element = serde_json::to_value(schema_default_element()).unwrap();
    let semantic_group = serde_json::to_value(schema_default_semantic_group()).unwrap();
    let attached_region = serde_json::to_value(schema_default_attached_region()).unwrap();

    serde_json::json!({
        "layer_background_omitted_in_project_json": element.get("layer").is_none(),
        "visible_true_omitted": element.get("visible").is_none(),
        "dynamic_height_false_omitted": semantic_group.get("dynamic_height").is_none(),
        "attached_region_visible_true_omitted": attached_region.get("visible").is_none()
    })
}

fn schema_default_element() -> Element {
    Element {
        id: "schema_default_element".to_string(),
        element_type: ElementType::Slot,
        x: 0,
        y: 0,
        width: None,
        height: None,
        size: None,
        asset: None,
        icon: None,
        icon_uv: None,
        tooltip: None,
        direction: None,
        content: None,
        font: None,
        color: None,
        shadow: None,
        animation: None,
        visible: true,
        uv: None,
        render_mode: crate::project::TextureRenderMode::Plain,
        nine_slice: None,
        layer: Layer::Background,
        slot_role: None,
        slot_index: None,
        inventory_group: None,
        scroll_binding: None,
        scroll_min: None,
        scroll_max: None,
        visible_rows: None,
        total_rows: None,
        columns: None,
        target_group: None,
        binding: None,
        dock: None,
        open_width: None,
        open_height: None,
        attached_region: None,
    }
}

fn schema_default_semantic_group() -> SemanticGroup {
    SemanticGroup {
        id: "schema_default_group".to_string(),
        kind: SemanticGroupKind::FixedSlots,
        columns: None,
        visible_rows: None,
        total_rows: None,
        slot_count: None,
        member_ids: Vec::new(),
        data_source: None,
        scroll_binding: None,
        dynamic_height: false,
    }
}

fn schema_default_attached_region() -> AttachedRegion {
    AttachedRegion {
        id: "schema_default_region".to_string(),
        anchor: AttachedRegionAnchor::Right,
        x: 0,
        y: 0,
        width: 1,
        height: 1,
        state: AttachedRegionState::Static,
        kind: None,
        semantic_group: None,
        visible: true,
        state_owned: Vec::new(),
    }
}

fn session_report(state: &AppState, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let summary = required_str(args, "summary")?.trim();
    if summary.is_empty() {
        return Err("Report summary cannot be empty".to_string());
    }
    let severity = optional_string(args, "severity").unwrap_or_else(|| "info".to_string());
    let level = match severity.as_str() {
        "info" => SessionLogLevel::Info,
        "warning" => SessionLogLevel::Warning,
        "error" => SessionLogLevel::Error,
        other => return Err(format!("Unknown report severity: {other}")),
    };
    let details = args.get("details").cloned();
    let project_id = optional_string(args, "project_id");

    let log_path = {
        let mut log = state.session_log.lock().unwrap();
        log.append(SessionLogEntry {
            level,
            source: "mcp".to_string(),
            category: "feedback_report".to_string(),
            message: summary.to_string(),
            details: Some(serde_json::json!({
                "severity": severity,
                "details": details,
                "project_id": project_id,
            })),
        })?;
        log.path().to_string_lossy().to_string()
    };

    Ok(serde_json::json!({
        "status": "logged",
        "log_path": log_path,
        "next_step": "Ask the user to attach this session log when filing an issue."
    }))
}

fn execute_tool(
    name: &str,
    args: &serde_json::Value,
    state: &AppState,
) -> Result<serde_json::Value, String> {
    if name == "session_report" {
        return session_report(state, args);
    }

    let mut sessions = state.sessions.lock().unwrap();
    let project_id = optional_string(args, "project_id");
    let project_id = project_id.as_deref();

    match name {
        "project_new" => project_new(&mut sessions, args),
        "project_open" => project_open(&mut sessions, args),
        "project_save" => project_save(&mut sessions, project_id),
        "project_save_as" => project_save_as(&mut sessions, project_id, args),
        "project_resize" => project_resize(&mut sessions, project_id, args),
        "project_export_preview" => project_export_preview(&sessions, project_id, args),
        "project_export" => project_export(&sessions, project_id, args),
        "project_render" | "project_screenshot" => project_render(&sessions, project_id, args),
        "project_summary" => project_summary(&sessions, project_id),
        "project_export_settings_update" => {
            project_export_settings_update(&mut sessions, project_id, args)
        }
        "project_semantic_groups_update" => {
            project_semantic_groups_update(&mut sessions, project_id, args)
        }
        "project_list_sessions" => Ok(serde_json::json!({ "sessions": sessions.list_sessions() })),
        "project_get_active" => project_get_active(&sessions),
        "schema_discover" => Ok(schema_discover()),
        "state_list" => state_list(&sessions, project_id),
        "state_add" => state_add(&mut sessions, project_id, args),
        "state_update" => state_update(&mut sessions, project_id, args),
        "state_remove" => state_remove(&mut sessions, project_id, args),
        "state_set_active" => state_set_active(&mut sessions, project_id, args),
        "state_override_update" => state_override_update(&mut sessions, project_id, args),
        "state_override_clear" => state_override_clear(&mut sessions, project_id, args),
        "project_undo" => Ok(serde_json::to_value(sessions.undo(project_id)?).unwrap()),
        "project_redo" => Ok(serde_json::to_value(sessions.redo(project_id)?).unwrap()),
        "element_add" => element_add(&mut sessions, project_id, args),
        "element_add_many" => element_add_many(&mut sessions, project_id, args),
        "slot_grid_add" => slot_grid_add(&mut sessions, project_id, args),
        "element_move" => element_move(&mut sessions, project_id, args),
        "element_update" => element_update(&mut sessions, project_id, args),
        "element_update_many" => element_update_many(&mut sessions, project_id, args),
        "element_resize" => element_resize(&mut sessions, project_id, args),
        "element_reorder" => element_reorder(&mut sessions, project_id, args),
        "element_remove" => element_remove(&mut sessions, project_id, args),
        "element_list" => {
            let session = sessions.resolve(project_id)?;
            let elements = session
                .project
                .elements
                .iter()
                .map(element_for_mcp)
                .collect::<Vec<_>>();
            Ok(serde_json::json!({ "elements": elements }))
        }
        "group_create" => group_create(&mut sessions, project_id, args),
        "group_upsert" => group_upsert(&mut sessions, project_id, args),
        "group_ungroup" => group_ungroup(&mut sessions, project_id, args),
        "group_list" => {
            let session = sessions.resolve(project_id)?;
            Ok(serde_json::json!({ "groups": session.project.groups }))
        }
        "attached_region_add" => attached_region_add(&mut sessions, project_id, args),
        "attached_region_update" => attached_region_update(&mut sessions, project_id, args),
        "attached_region_remove" => attached_region_remove(&mut sessions, project_id, args),
        "attached_region_list" => attached_region_list(&sessions, project_id),
        "attached_region_move_with_elements" => {
            attached_region_move_with_elements(&mut sessions, project_id, args)
        }
        "animation_create" => animation_create(&mut sessions, project_id, args),
        "animation_update" => animation_update(&mut sessions, project_id, args),
        "animation_remove" => animation_remove(&mut sessions, project_id, args),
        "animation_bind" => animation_bind(&mut sessions, project_id, args),
        "animation_unbind" => animation_unbind(&mut sessions, project_id, args),
        "animation_list" => {
            let session = sessions.resolve(project_id)?;
            Ok(serde_json::json!({ "animations": session.project.animations }))
        }
        "asset_import" => asset_import(&mut sessions, project_id, args),
        "asset_update" => asset_update(&mut sessions, project_id, args),
        "asset_metadata_update" => asset_metadata_update(&mut sessions, project_id, args),
        "asset_remove" => asset_remove(&mut sessions, project_id, args),
        "asset_get_data_url" => asset_get_data_url(&sessions, project_id, args),
        "asset_list" => asset_list(&sessions, project_id),
        "gui_template_list" => Ok(serde_json::to_value(templates::list_template_info()).unwrap()),
        _ => Err(format!("Unknown tool: {name}")),
    }
}

fn project_new(
    sessions: &mut ProjectSessionManager,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let name = args
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("Untitled");
    let width = args
        .get("width")
        .and_then(|value| value.as_u64())
        .unwrap_or(176) as u32;
    let height = args
        .get("height")
        .and_then(|value| value.as_u64())
        .unwrap_or(166) as u32;
    let mod_target = match args
        .get("mod_target")
        .and_then(|value| value.as_str())
        .unwrap_or("forge")
    {
        "fabric" => ModTarget::Fabric,
        "neoforge" | "neo_forge" => ModTarget::NeoForge,
        _ => ModTarget::Forge,
    };
    let mut project = Project::new(name, width, height, mod_target);
    if let Some(template) = args.get("template").and_then(|value| value.as_str()) {
        templates::apply_template(&mut project, template)?;
    } else {
        templates::apply_generated_defaults(&mut project)?;
    }
    let project_id = sessions.create_session(project);
    project_result(sessions, &project_id)
}

fn project_open(
    sessions: &mut ProjectSessionManager,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let path = required_str(args, "path")?;
    let project = crate::format::load_from_mcgui(path)?;
    let project_id = sessions.create_session(project);
    project_result(sessions, &project_id)
}

fn project_save(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve_mut(project_id)?;
    crate::format::save_to_mcgui(&session.project)?;
    session.project.is_dirty = false;
    Ok(serde_json::json!({
        "project_id": session.id,
        "status": "saved",
        "path": session.project.project_path,
        "is_dirty": false,
    }))
}

fn project_save_as(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let path = required_str(args, "path")?.to_string();
    let session = sessions.resolve_mut(project_id)?;
    let previous_path = session.project.project_path.clone();
    session.project.project_path = Some(path.clone());
    if let Err(error) = crate::format::save_to_mcgui(&session.project) {
        session.project.project_path = previous_path;
        return Err(error);
    }
    session.project.is_dirty = false;
    Ok(serde_json::json!({
        "project_id": session.id,
        "status": "saved",
        "path": path,
        "is_dirty": false,
    }))
}

fn project_export_preview(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let (project, config, target) = export_request(sessions, project_id, args)?;
    let state_id = optional_state_id(args, "state_id")?;
    serde_json::to_value(crate::export::preview_export_for_state(
        project,
        &config,
        target,
        state_id.as_deref(),
    )?)
    .map_err(|error| error.to_string())
}

fn project_export(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let (project, config, target) = export_request(sessions, project_id, args)?;
    let state_id = optional_state_id(args, "state_id")?;
    Ok(serde_json::json!({
        "files": crate::export::export_project_for_state(project, &config, target, state_id.as_deref())?,
    }))
}

fn export_request<'a>(
    sessions: &'a ProjectSessionManager,
    project_id: Option<&str>,
    args: &'a serde_json::Value,
) -> Result<(&'a Project, crate::export::ExportConfig, &'a str), String> {
    let target = required_str(args, "target")?;
    let project = &sessions.resolve(project_id)?.project;
    let config = crate::export::ExportConfig {
        mod_id: required_str(args, "mod_id")?.to_string(),
        package: required_str(args, "package")?.to_string(),
        class_name: required_str(args, "class_name")?.to_string(),
        output_dir: required_str(args, "output_dir")?.to_string(),
        settings_override: export_settings_override(project, args)?,
        overwrite: optional_bool(args, "overwrite")?.unwrap_or(false),
        scope: crate::export::ExportScope::parse(optional_string(args, "export_scope").as_deref())?,
    };
    Ok((project, config, target))
}

fn export_settings_override(
    project: &Project,
    args: &serde_json::Value,
) -> Result<Option<crate::project::ProjectExportSettings>, String> {
    let has_override = args.get("codegen_mode").is_some()
        || args.get("generate_runtime_helpers").is_some()
        || args.get("generate_semantic_registry").is_some();
    if !has_override {
        return Ok(None);
    }

    let mut settings = project.export_settings.clone();
    apply_export_settings_args(&mut settings, args)?;
    default_semantic_registry_from_mode_when_unspecified(&mut settings, args);
    Ok(Some(settings.normalized()))
}

fn project_summary(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve(project_id)?;
    Ok(serde_json::json!({
        "project_id": session.id,
        "name": session.project.name,
        "gui_size": session.project.gui_size,
        "mod_target": session.project.mod_target,
        "element_count": session.project.elements.len(),
        "is_dirty": session.project.is_dirty,
        "path": session.project.project_path,
        "export_settings": session.project.export_settings,
        "revision": session.revision,
        "session": sessions.list_sessions().into_iter().find(|summary| summary.id == session.id),
    }))
}

fn project_resize(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let width = required_u32(args, "width")?;
    let height = required_u32(args, "height")?;
    if width == 0 || height == 0 {
        return Err("Project dimensions must be greater than zero".to_string());
    }

    let session = sessions.resolve(project_id)?;
    let old_size = session.project.gui_size.clone();
    let new_size = crate::project::Size { width, height };
    if old_size == new_size {
        return Ok(serde_json::json!({
            "project_id": session.id,
            "old_size": old_size,
            "new_size": new_size,
            "changed": false
        }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.gui_size = new_size.clone();
    let session_id = session.id.clone();
    sessions.mark_changed(project_id)?;

    Ok(serde_json::json!({
        "project_id": session_id,
        "old_size": old_size,
        "new_size": new_size,
        "changed": true
    }))
}

fn project_export_settings_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let current = sessions
        .resolve(project_id)?
        .project
        .export_settings
        .clone();
    let mut next = current.clone();
    apply_export_settings_args(&mut next, args)?;
    default_semantic_registry_from_mode_when_unspecified(&mut next, args);
    next = next.normalized();
    if next == current {
        return serde_json::to_value(next).map_err(|error| error.to_string());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.export_settings = next.clone();
    sessions.mark_changed(project_id)?;
    serde_json::to_value(next).map_err(|error| error.to_string())
}

fn apply_export_settings_args(
    settings: &mut crate::project::ProjectExportSettings,
    args: &serde_json::Value,
) -> Result<(), String> {
    if let Some(value) = args.get("codegen_mode") {
        let mode = value
            .as_str()
            .ok_or("codegen_mode must be \"simple\" or \"modular\"")?;
        settings.codegen_mode = match mode {
            "simple" => CodegenMode::Simple,
            "modular" => CodegenMode::Modular,
            other => return Err(format!("Unknown codegen_mode: {other}")),
        };
    }
    if let Some(value) = args.get("generate_runtime_helpers") {
        let value = value
            .as_bool()
            .ok_or("generate_runtime_helpers must be boolean")?;
        settings.generate_runtime_helpers = value;
    }
    if let Some(value) = args.get("generate_semantic_registry") {
        let value = value
            .as_bool()
            .ok_or("generate_semantic_registry must be boolean")?;
        settings.generate_semantic_registry = value;
    }
    Ok(())
}

fn default_semantic_registry_from_mode_when_unspecified(
    settings: &mut crate::project::ProjectExportSettings,
    args: &serde_json::Value,
) {
    if args.get("codegen_mode").is_some() && args.get("generate_semantic_registry").is_none() {
        settings.generate_semantic_registry = settings.codegen_mode == CodegenMode::Modular;
    }
}

fn project_semantic_groups_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let groups_value = args
        .get("semantic_groups")
        .ok_or("Missing semantic_groups")?
        .clone();
    let groups: Vec<SemanticGroup> = serde_json::from_value(groups_value)
        .map_err(|error| format!("Invalid semantic_groups: {error}"))?;
    let current = sessions
        .resolve(project_id)?
        .project
        .semantic_groups
        .clone();
    if groups == current {
        return serde_json::to_value(groups).map_err(|error| error.to_string());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.semantic_groups = groups.clone();
    sessions.mark_changed(project_id)?;
    serde_json::to_value(groups).map_err(|error| error.to_string())
}

fn project_get_active(sessions: &ProjectSessionManager) -> Result<serde_json::Value, String> {
    let active = sessions.active_session()?;
    Ok(serde_json::json!({
        "summary": sessions.list_sessions().into_iter().find(|summary| summary.id == active.id),
        "project": active.project,
    }))
}

fn element_add(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let element = parse_element_arg(args)?;
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.add_element(element.clone());
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(element).unwrap())
}

fn element_add_many(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let values = args
        .get("elements")
        .and_then(|value| value.as_array())
        .ok_or("Missing elements")?;
    if values.is_empty() {
        return Err("elements array cannot be empty".to_string());
    }
    let elements = values
        .iter()
        .map(parse_element_arg)
        .collect::<Result<Vec<_>, _>>()?;
    validate_new_element_ids(sessions, project_id, &elements)?;

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.elements.extend(elements.clone());
    sessions.mark_changed(project_id)?;
    let returned_elements = elements.iter().map(element_for_mcp).collect::<Vec<_>>();
    Ok(serde_json::json!({
        "created_count": elements.len(),
        "elements": returned_elements,
    }))
}

fn slot_grid_add(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id_prefix = required_str(args, "id_prefix")?;
    let x = required_i32(args, "x")?;
    let y = required_i32(args, "y")?;
    let columns = required_u32(args, "columns")?;
    let rows = required_u32(args, "rows")?;
    if columns == 0 || rows == 0 {
        return Err("columns and rows must be greater than 0".to_string());
    }
    let slot_size = optional_u32(args, "slot_size")?.unwrap_or(18);
    let spacing = optional_u32(args, "spacing")?.unwrap_or(18);
    let slot_index_start = optional_u32(args, "slot_index_start")?.unwrap_or(0);
    let slot_role = optional_slot_role(args, "slot_role")?;
    let inventory_group = optional_string(args, "inventory_group");
    let scroll_binding = optional_string(args, "scroll_binding");
    let group_id = optional_string(args, "group_id");
    let semantic_group_kind = optional_semantic_group_kind(args, "semantic_group_kind")?;
    let semantic_slot_count = optional_u32(args, "slot_count")?;

    let options = SlotGridOptions {
        id_prefix: id_prefix.to_string(),
        x,
        y,
        columns,
        rows,
        slot_size,
        spacing,
        slot_role,
        inventory_group: inventory_group.clone(),
        slot_index_start,
        scroll_binding: scroll_binding.clone(),
    };
    let elements = slot_grid_elements(&options)?;
    validate_new_element_ids(sessions, project_id, &elements)?;
    if let Some(group_id) = &group_id {
        if sessions
            .resolve(project_id)?
            .project
            .groups
            .iter()
            .any(|group| group.id == *group_id)
        {
            return Err("Group already exists".to_string());
        }
    }

    let element_ids = elements
        .iter()
        .map(|element| element.id.clone())
        .collect::<Vec<_>>();
    let group = group_id.map(|id| crate::project::Group {
        id,
        x,
        y,
        elements: element_ids,
        visible: None,
        state_owned: Vec::new(),
    });
    let semantic_group = match (semantic_group_kind, inventory_group.clone()) {
        (Some(kind), Some(inventory_group)) => Some(SemanticGroup {
            id: inventory_group.clone(),
            kind,
            columns: Some(columns),
            visible_rows: Some(rows),
            total_rows: Some(rows),
            slot_count: Some(semantic_slot_count.unwrap_or(elements.len() as u32)),
            member_ids: elements.iter().map(|element| element.id.clone()).collect(),
            data_source: Some(inventory_group),
            scroll_binding: scroll_binding.clone(),
            dynamic_height: false,
        }),
        _ => None,
    };

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.elements.extend(elements.clone());
    if let Some(group) = &group {
        session.project.groups.push(group.clone());
    }
    if let Some(semantic_group) = &semantic_group {
        session
            .project
            .semantic_groups
            .retain(|group| group.id != semantic_group.id);
        session.project.semantic_groups.push(semantic_group.clone());
    }
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({
        "created_count": elements.len(),
        "elements": elements,
        "group": group,
        "semantic_group": semantic_group,
    }))
}

struct SlotGridOptions {
    id_prefix: String,
    x: i32,
    y: i32,
    columns: u32,
    rows: u32,
    slot_size: u32,
    spacing: u32,
    slot_role: Option<crate::project::SlotRole>,
    inventory_group: Option<String>,
    slot_index_start: u32,
    scroll_binding: Option<String>,
}

fn slot_grid_elements(options: &SlotGridOptions) -> Result<Vec<Element>, String> {
    let count = options
        .columns
        .checked_mul(options.rows)
        .ok_or("slot grid dimensions are too large")?;
    let capacity = usize::try_from(count).map_err(|_| "slot grid dimensions are too large")?;
    let mut elements = Vec::new();
    elements
        .try_reserve_exact(capacity)
        .map_err(|_| "slot grid dimensions are too large")?;
    for local_index in 0..count {
        let column = local_index % options.columns;
        let row = local_index / options.columns;
        let x = slot_grid_coordinate(options.x, column, options.spacing, "x")?;
        let y = slot_grid_coordinate(options.y, row, options.spacing, "y")?;
        let slot_index = options
            .slot_index_start
            .checked_add(local_index)
            .ok_or("slot_index_start is too large")?;
        elements.push(base_slot_element(
            format!("{}_{local_index}", options.id_prefix),
            x,
            y,
            slot_index,
            options,
        ));
    }
    Ok(elements)
}

fn slot_grid_coordinate(origin: i32, index: u32, spacing: u32, axis: &str) -> Result<i32, String> {
    let overflow_error = || format!("slot grid {axis} coordinate overflow");
    let offset = u64::from(index)
        .checked_mul(u64::from(spacing))
        .ok_or_else(&overflow_error)?;
    let max_offset = (i64::from(i32::MAX) - i64::from(origin)) as u64;
    if offset > max_offset {
        return Err(overflow_error());
    }
    let coordinate = i64::from(origin) + i64::try_from(offset).map_err(|_| overflow_error())?;
    i32::try_from(coordinate).map_err(|_| overflow_error())
}

fn base_slot_element(
    id: String,
    x: i32,
    y: i32,
    slot_index: u32,
    options: &SlotGridOptions,
) -> Element {
    Element {
        id,
        element_type: crate::project::ElementType::Slot,
        x,
        y,
        width: None,
        height: None,
        size: Some(options.slot_size),
        asset: None,
        icon: None,
        icon_uv: None,
        tooltip: None,
        direction: None,
        content: None,
        font: None,
        color: None,
        shadow: None,
        animation: None,
        visible: true,
        uv: None,
        render_mode: crate::project::TextureRenderMode::Plain,
        nine_slice: None,
        layer: crate::project::Layer::Background,
        slot_role: options.slot_role.clone(),
        slot_index: Some(slot_index),
        inventory_group: options.inventory_group.clone(),
        scroll_binding: options.scroll_binding.clone(),
        scroll_min: None,
        scroll_max: None,
        visible_rows: None,
        total_rows: None,
        columns: None,
        target_group: None,
        binding: None,
        dock: None,
        open_width: None,
        open_height: None,
        attached_region: None,
    }
}

fn parse_element_arg(value: &serde_json::Value) -> Result<Element, String> {
    let payload = value.get("element").unwrap_or(value).clone();
    serde_json::from_value(payload).map_err(|error| format!("Invalid element payload: {error}"))
}

fn validate_new_element_ids(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    elements: &[Element],
) -> Result<(), String> {
    let mut ids = HashSet::new();
    for element in elements {
        if !ids.insert(element.id.as_str()) {
            return Err(format!("Duplicate element id: {}", element.id));
        }
    }
    let project = &sessions.resolve(project_id)?.project;
    for element in elements {
        if project.find_element(&element.id).is_some() {
            return Err(format!("Element already exists: {}", element.id));
        }
    }
    Ok(())
}

fn state_list(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve(project_id)?;
    Ok(serde_json::json!({
        "project_id": session.id,
        "revision": session.revision,
        "active_state_id": session.active_state_id,
        "edit_scope": session.edit_scope,
        "states": session.project.states,
    }))
}

fn state_add(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?.trim();
    let label = required_str(args, "label")?.trim();
    if id.is_empty() {
        return Err("state id cannot be empty".to_string());
    }
    if label.is_empty() {
        return Err("state label cannot be empty".to_string());
    }
    let description = nullable_string(
        args.get("description").unwrap_or(&serde_json::Value::Null),
        "description",
    )?;
    let initial = optional_bool(args, "initial")?.unwrap_or(false);
    let export_role = nullable_string(
        args.get("export_role").unwrap_or(&serde_json::Value::Null),
        "export_role",
    )?;

    sessions
        .resolve(project_id)?
        .project
        .validate_state_id_available(id)?;
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    if initial {
        for state in &mut session.project.states {
            state.initial = false;
        }
    }
    let state = ProjectState {
        id: id.to_string(),
        label: label.to_string(),
        description,
        initial,
        export_role,
    };
    session.project.states.push(state.clone());
    sessions.mark_changed(project_id)?;
    let session = sessions.resolve(project_id)?;
    Ok(serde_json::json!({
        "project_id": session.id,
        "revision": session.revision,
        "state": state,
        "states": session.project.states,
    }))
}

fn state_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    let current = sessions
        .resolve(project_id)?
        .project
        .find_state(id)
        .ok_or_else(|| format!("unknown state '{id}'"))?;
    let mut updated = current.clone();
    if let Some(label) = args.get("label") {
        let label = label.as_str().ok_or("label must be a string")?.trim();
        if label.is_empty() {
            return Err("state label cannot be empty".to_string());
        }
        updated.label = label.to_string();
    }
    if let Some(description) = args.get("description") {
        updated.description = nullable_string(description, "description")?;
    }
    if let Some(initial) = optional_bool(args, "initial")? {
        updated.initial = initial;
    }
    if let Some(export_role) = args.get("export_role") {
        updated.export_role = nullable_string(export_role, "export_role")?;
    }

    if updated == *current {
        let session = sessions.resolve(project_id)?;
        return Ok(serde_json::json!({
            "project_id": session.id,
            "revision": session.revision,
            "state": current,
            "states": session.project.states,
        }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    if updated.initial {
        for state in &mut session.project.states {
            state.initial = false;
        }
    }
    *session
        .project
        .find_state_mut(id)
        .ok_or_else(|| format!("unknown state '{id}'"))? = updated.clone();
    sessions.mark_changed(project_id)?;
    let session = sessions.resolve(project_id)?;
    Ok(serde_json::json!({
        "project_id": session.id,
        "revision": session.revision,
        "state": updated,
        "states": session.project.states,
    }))
}

fn state_remove(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    if sessions
        .resolve(project_id)?
        .project
        .find_state(id)
        .is_none()
    {
        return Err(format!("unknown state '{id}'"));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.states.retain(|state| state.id != id);
    session.project.state_overrides.remove(id);
    for group in &mut session.project.groups {
        group.state_owned.retain(|state_id| state_id != id);
    }
    for region in &mut session.project.attached_regions {
        region.state_owned.retain(|state_id| state_id != id);
    }
    if session.active_state_id.as_deref() == Some(id) {
        session.active_state_id = session.project.initial_state_id().map(str::to_owned);
        session.edit_scope = EditScope::Base;
    }
    sessions.mark_changed(project_id)?;
    let session = sessions.resolve(project_id)?;
    Ok(serde_json::json!({
        "project_id": session.id,
        "revision": session.revision,
        "removed": true,
        "states": session.project.states,
        "active_state_id": session.active_state_id,
    }))
}

fn state_set_active(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let state_id = optional_state_id(args, "state_id")?;
    if let Some(state_id) = state_id.as_deref() {
        if sessions
            .resolve(project_id)?
            .project
            .find_state(state_id)
            .is_none()
        {
            return Err(format!("unknown state '{state_id}'"));
        }
    }
    let edit_scope = parse_edit_scope(args)?;
    let session_id = {
        let session = sessions.resolve_mut(project_id)?;
        session.active_state_id = state_id;
        if session.active_state_id.is_none() {
            session.edit_scope = EditScope::Base;
        } else if let Some(edit_scope) = edit_scope {
            session.edit_scope = edit_scope;
        }
        session.id.clone()
    };
    let summary = session_summary(sessions, &session_id)?;
    Ok(serde_json::json!({
        "project_id": session_id,
        "active_state_id": summary.active_state_id,
        "edit_scope": summary.edit_scope,
        "summary": summary,
    }))
}

fn state_override_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let state_id = required_str(args, "state_id")?;
    let target_type = required_str(args, "target_type")?;
    let target_id = required_str(args, "target_id")?;
    let fields = args
        .get("fields")
        .and_then(|value| value.as_object())
        .ok_or("fields must be an object")?;

    match target_type {
        "element" => {
            update_element_state_override_for_mcp(sessions, project_id, target_id, state_id, fields)
        }
        "attached_region" => {
            let patch = parse_attached_region_state_patch(fields)?;
            let changed = {
                let mut preview = sessions.resolve(project_id)?.project.clone();
                preview.update_attached_region_state_override(state_id, target_id, patch.clone())?
            };
            if changed {
                sessions.record_history(project_id)?;
                let session = sessions.resolve_mut(project_id)?;
                session
                    .project
                    .update_attached_region_state_override(state_id, target_id, patch)?;
                sessions.mark_changed(project_id)?;
            }
            state_override_response(
                sessions,
                project_id,
                state_id,
                target_type,
                target_id,
                changed,
            )
        }
        "group" => {
            let visible = parse_group_state_patch(fields)?;
            let changed = {
                let project = &sessions.resolve(project_id)?.project;
                validate_group_state_override_target(project, state_id, target_id)?;
                let mut preview = project.clone();
                apply_group_state_override(&mut preview, state_id, target_id, visible);
                preview.state_overrides != project.state_overrides
            };
            if changed {
                sessions.record_history(project_id)?;
                let session = sessions.resolve_mut(project_id)?;
                apply_group_state_override(&mut session.project, state_id, target_id, visible);
                session.project.is_dirty = true;
                sessions.mark_changed(project_id)?;
            }
            state_override_response(
                sessions,
                project_id,
                state_id,
                target_type,
                target_id,
                changed,
            )
        }
        _ => Err(format!(
            "unknown state override target_type '{target_type}'"
        )),
    }
}

fn state_override_clear(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let state_id = required_str(args, "state_id")?;
    let target_type = required_str(args, "target_type")?;
    let target_id = required_str(args, "target_id")?;
    let field = optional_string(args, "field");
    let changed = {
        let project = &sessions.resolve(project_id)?.project;
        let mut preview = project.clone();
        apply_state_override_clear_for_mcp(
            &mut preview,
            state_id,
            target_type,
            target_id,
            field.as_deref(),
        )?;
        preview.state_overrides != project.state_overrides
    };
    if changed {
        sessions.record_history(project_id)?;
        let session = sessions.resolve_mut(project_id)?;
        apply_state_override_clear_for_mcp(
            &mut session.project,
            state_id,
            target_type,
            target_id,
            field.as_deref(),
        )?;
        sessions.mark_changed(project_id)?;
    }
    state_override_response(
        sessions,
        project_id,
        state_id,
        target_type,
        target_id,
        changed,
    )
}

#[derive(Debug)]
struct ElementPatch {
    id: String,
    changes: serde_json::Map<String, serde_json::Value>,
    state_id: Option<String>,
    edit_scope: Option<EditScope>,
}

fn parse_element_patches(args: &serde_json::Value) -> Result<Vec<ElementPatch>, String> {
    let updates = args
        .get("updates")
        .and_then(|value| value.as_array())
        .ok_or("Missing updates")?;
    if updates.is_empty() {
        return Err("updates array cannot be empty".to_string());
    }

    let mut ids = HashSet::new();
    let mut patches = Vec::with_capacity(updates.len());
    let top_level_state_id = optional_state_id(args, "state_id")?;
    for update in updates {
        let object = update.as_object().ok_or("Each update must be an object")?;
        let id = object
            .get("id")
            .and_then(|value| value.as_str())
            .ok_or("Each update requires an id")?
            .to_string();
        if !ids.insert(id.clone()) {
            return Err(format!("Duplicate element update id: {id}"));
        }
        let changes = object
            .get("changes")
            .and_then(|value| value.as_object())
            .ok_or("Each update requires object changes")?
            .clone();
        let state_id = if object.contains_key("state_id") {
            optional_state_id(update, "state_id")?
        } else {
            top_level_state_id.clone()
        };
        let edit_scope = object
            .get("edit_scope")
            .or_else(|| args.get("edit_scope"))
            .map(|value| {
                serde_json::from_value(value.clone())
                    .map_err(|error| format!("Invalid edit_scope: {error}"))
            })
            .transpose()?;
        patches.push(ElementPatch {
            id,
            changes,
            state_id,
            edit_scope,
        });
    }
    Ok(patches)
}

fn element_move(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    let x = required_i32(args, "x")?;
    let y = required_i32(args, "y")?;
    let current = sessions
        .resolve(project_id)?
        .project
        .find_element(id)
        .ok_or("Element not found")?;
    if current.x == x && current.y == y {
        return Ok(serde_json::to_value(current).unwrap());
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = session
        .project
        .find_element_mut(id)
        .ok_or("Element not found")?;
    element.x = x;
    element.y = y;
    let element = element.clone();
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(element).unwrap())
}

fn apply_element_changes(
    current: &Element,
    changes: &serde_json::Map<String, serde_json::Value>,
) -> Result<Element, String> {
    let mut value = serde_json::to_value(current).map_err(|error| error.to_string())?;
    let target = value
        .as_object_mut()
        .ok_or("Element payload must be an object")?;
    for (key, value) in changes {
        if key == "id" || key == "type" {
            continue;
        }
        target.insert(key.clone(), value.clone());
    }
    serde_json::from_value(value).map_err(|error| format!("Invalid element update: {error}"))
}

fn parse_edit_scope(args: &serde_json::Value) -> Result<Option<EditScope>, String> {
    args.get("edit_scope")
        .map(|value| {
            serde_json::from_value(value.clone())
                .map_err(|error| format!("Invalid edit_scope: {error}"))
        })
        .transpose()
}

fn effective_state_id_for_element_update(
    session_active_state_id: Option<&str>,
    state_id: Option<&str>,
    edit_scope: Option<EditScope>,
) -> Result<Option<String>, String> {
    if state_id.is_some() || edit_scope == Some(EditScope::State) {
        return state_id
            .or(session_active_state_id)
            .map(|value| Some(value.to_string()))
            .ok_or(
                "state_id is required when edit_scope is state and no active state is set"
                    .to_string(),
            );
    }
    Ok(None)
}

fn parse_element_state_patch(
    changes: &serde_json::Map<String, serde_json::Value>,
) -> Result<ElementStateOverridePatch, String> {
    let unsupported = changes
        .keys()
        .filter(|key| {
            !matches!(
                key.as_str(),
                "visible" | "x" | "y" | "width" | "height" | "attached_region" | "layer"
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(format!(
            "unsupported element state override field(s): {}",
            unsupported.join(", ")
        ));
    }

    let mut patch = ElementStateOverridePatch::default();
    for (key, value) in changes {
        match key.as_str() {
            "visible" => patch.visible = Some(nullable_bool(value, key)?),
            "x" => patch.x = Some(nullable_i32(value, key)?),
            "y" => patch.y = Some(nullable_i32(value, key)?),
            "width" => patch.width = Some(nullable_u32(value, key)?),
            "height" => patch.height = Some(nullable_u32(value, key)?),
            "attached_region" => {
                patch.attached_region = Some(attached_region_override(value, key)?)
            }
            "layer" => {
                patch.layer = Some(if value.is_null() {
                    None
                } else {
                    Some(
                        serde_json::from_value(value.clone())
                            .map_err(|error| format!("Invalid layer override: {error}"))?,
                    )
                });
            }
            _ => unreachable!(),
        }
    }
    Ok(patch)
}

fn parse_attached_region_state_patch(
    changes: &serde_json::Map<String, serde_json::Value>,
) -> Result<AttachedRegionStateOverridePatch, String> {
    let unsupported = changes
        .keys()
        .filter(|key| !matches!(key.as_str(), "visible" | "x" | "y" | "width" | "height"))
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(format!(
            "unsupported attached-region state override field(s): {}",
            unsupported.join(", ")
        ));
    }

    let mut patch = AttachedRegionStateOverridePatch::default();
    for (key, value) in changes {
        match key.as_str() {
            "visible" => patch.visible = Some(nullable_bool(value, key)?),
            "x" => patch.x = Some(nullable_i32(value, key)?),
            "y" => patch.y = Some(nullable_i32(value, key)?),
            "width" => patch.width = Some(nullable_u32(value, key)?),
            "height" => patch.height = Some(nullable_u32(value, key)?),
            _ => unreachable!(),
        }
    }
    Ok(patch)
}

fn parse_group_state_patch(
    changes: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<Option<bool>>, String> {
    let unsupported = changes
        .keys()
        .filter(|key| key.as_str() != "visible")
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(format!(
            "unsupported group state override field(s): {}",
            unsupported.join(", ")
        ));
    }
    changes
        .get("visible")
        .map(|value| nullable_bool(value, "visible"))
        .transpose()
}

fn validate_group_state_override_target(
    project: &Project,
    state_id: &str,
    group_id: &str,
) -> Result<(), String> {
    if project.find_state(state_id).is_none() {
        return Err(format!("unknown state '{state_id}'"));
    }
    if !project.groups.iter().any(|group| group.id == group_id) {
        return Err(format!("unknown group '{group_id}'"));
    }
    Ok(())
}

fn apply_group_state_override(
    project: &mut Project,
    state_id: &str,
    group_id: &str,
    visible: Option<Option<bool>>,
) {
    let override_value = project
        .state_overrides
        .entry(state_id.to_string())
        .or_default()
        .groups
        .entry(group_id.to_string())
        .or_insert_with(GroupStateOverride::default);
    if let Some(value) = visible {
        override_value.visible = value;
    }
    if override_value.visible.is_none() {
        if let Some(overrides) = project.state_overrides.get_mut(state_id) {
            overrides.groups.remove(group_id);
            if overrides.elements.is_empty()
                && overrides.groups.is_empty()
                && overrides.attached_regions.is_empty()
            {
                project.state_overrides.remove(state_id);
            }
        }
    }
}

fn apply_state_override_clear_for_mcp(
    project: &mut Project,
    state_id: &str,
    target_type: &str,
    target_id: &str,
    field: Option<&str>,
) -> Result<(), String> {
    if let Some(field) = field {
        let target = match target_type {
            "element" => StateOverrideTarget::Element(target_id.to_string()),
            "attached_region" => StateOverrideTarget::AttachedRegion(target_id.to_string()),
            "group" => StateOverrideTarget::Group(target_id.to_string()),
            _ => {
                return Err(format!(
                    "unknown state override target_type '{target_type}'"
                ))
            }
        };
        project.clear_state_override_field(state_id, target, field)?;
        return Ok(());
    }

    if project.find_state(state_id).is_none() {
        return Err(format!("unknown state '{state_id}'"));
    }
    match target_type {
        "element" => {
            if project.find_element(target_id).is_none() {
                return Err(format!("unknown element '{target_id}'"));
            }
            if let Some(overrides) = project.state_overrides.get_mut(state_id) {
                overrides.elements.remove(target_id);
            }
        }
        "attached_region" => {
            if project.find_attached_region(target_id).is_none() {
                return Err(format!("unknown attached region '{target_id}'"));
            }
            if let Some(overrides) = project.state_overrides.get_mut(state_id) {
                overrides.attached_regions.remove(target_id);
            }
        }
        "group" => {
            if !project.groups.iter().any(|group| group.id == target_id) {
                return Err(format!("unknown group '{target_id}'"));
            }
            if let Some(overrides) = project.state_overrides.get_mut(state_id) {
                overrides.groups.remove(target_id);
            }
        }
        _ => {
            return Err(format!(
                "unknown state override target_type '{target_type}'"
            ))
        }
    }
    if project
        .state_overrides
        .get(state_id)
        .is_some_and(|overrides| {
            overrides.elements.is_empty()
                && overrides.groups.is_empty()
                && overrides.attached_regions.is_empty()
        })
    {
        project.state_overrides.remove(state_id);
    }
    project.is_dirty = true;
    Ok(())
}

fn state_override_response(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    state_id: &str,
    target_type: &str,
    target_id: &str,
    changed: bool,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve(project_id)?;
    Ok(serde_json::json!({
        "project_id": session.id,
        "revision": session.revision,
        "state_id": state_id,
        "target_type": target_type,
        "target_id": target_id,
        "changed": changed,
        "state_overrides": session.project.state_overrides.get(state_id),
    }))
}

fn update_element_state_override_for_mcp(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    element_id: &str,
    state_id: &str,
    changes: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let patch = parse_element_state_patch(changes)?;
    let changed = {
        let mut preview = sessions.resolve(project_id)?.project.clone();
        preview.update_element_state_override(state_id, element_id, patch.clone())?
    };
    if !changed {
        let session = sessions.resolve(project_id)?;
        return Ok(serde_json::json!({
            "project_id": session.id,
            "revision": session.revision,
            "state_id": state_id,
            "target_type": "element",
            "target_id": element_id,
            "changed": false,
            "state_overrides": session.project.state_overrides.get(state_id),
        }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session
        .project
        .update_element_state_override(state_id, element_id, patch)?;
    sessions.mark_changed(project_id)?;
    let session = sessions.resolve(project_id)?;
    Ok(serde_json::json!({
        "project_id": session.id,
        "revision": session.revision,
        "state_id": state_id,
        "target_type": "element",
        "target_id": element_id,
        "changed": true,
        "state_overrides": session.project.state_overrides.get(state_id),
    }))
}

fn element_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    let changes = args
        .get("changes")
        .ok_or("Missing changes")?
        .as_object()
        .ok_or("Element changes must be an object")?;
    let edit_scope = parse_edit_scope(args)?;
    let explicit_state_id = optional_state_id(args, "state_id")?;
    let active_state_id = sessions
        .resolve(project_id)?
        .active_state_id
        .as_deref()
        .map(str::to_owned);
    if let Some(state_id) = effective_state_id_for_element_update(
        active_state_id.as_deref(),
        explicit_state_id.as_deref(),
        edit_scope,
    )? {
        return update_element_state_override_for_mcp(sessions, project_id, id, &state_id, changes);
    }
    let current = sessions
        .resolve(project_id)?
        .project
        .find_element(id)
        .ok_or("Element not found")?;
    let updated = apply_element_changes(current, changes)?;
    if &updated == current {
        return Ok(serde_json::to_value(current).unwrap());
    }
    let refresh_group_positions = current.x != updated.x || current.y != updated.y;
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    *session
        .project
        .find_element_mut(id)
        .ok_or("Element not found")? = updated.clone();
    if refresh_group_positions {
        refresh_group_positions_for_elements(&mut session.project, &[id.to_string()]);
    }
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(updated).unwrap())
}

fn element_update_many(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let patches = parse_element_patches(args)?;
    let state_scoped_count = patches
        .iter()
        .filter(|patch| patch_requests_state_scope(patch))
        .count();
    if state_scoped_count > 0 && state_scoped_count < patches.len() {
        return Err(
            "element_update_many cannot mix base updates and state-scoped updates".to_string(),
        );
    }
    if state_scoped_count > 0 {
        return element_update_many_state_overrides(sessions, project_id, &patches);
    }

    let (session_id, updated, changed_count, coordinate_changed_ids) = {
        let session = sessions.resolve(project_id)?;
        let mut updated = Vec::with_capacity(patches.len());
        for patch in &patches {
            let current = session
                .project
                .find_element(&patch.id)
                .ok_or_else(|| format!("Element not found: {}", patch.id))?;
            updated.push(apply_element_changes(current, &patch.changes)?);
        }
        let mut changed_count = 0usize;
        let mut coordinate_changed_ids = Vec::new();
        for element in &updated {
            if let Some(current) = session.project.find_element(&element.id) {
                if current != element {
                    changed_count += 1;
                }
                if current.x != element.x || current.y != element.y {
                    coordinate_changed_ids.push(element.id.clone());
                }
            }
        }
        (
            session.id.clone(),
            updated,
            changed_count,
            coordinate_changed_ids,
        )
    };

    if changed_count == 0 {
        return Ok(serde_json::json!({
            "project_id": session_id,
            "updated_count": 0,
            "results": updated.iter().map(element_for_mcp).collect::<Vec<_>>()
        }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    for element in &updated {
        *session
            .project
            .find_element_mut(&element.id)
            .ok_or_else(|| format!("Element not found: {}", element.id))? = element.clone();
    }
    refresh_group_positions_for_elements(&mut session.project, &coordinate_changed_ids);
    sessions.mark_changed(project_id)?;

    Ok(serde_json::json!({
        "project_id": session_id,
        "updated_count": changed_count,
        "results": updated.iter().map(element_for_mcp).collect::<Vec<_>>()
    }))
}

fn patch_requests_state_scope(patch: &ElementPatch) -> bool {
    patch.state_id.is_some() || patch.edit_scope == Some(EditScope::State)
}

fn element_update_many_state_overrides(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    patches: &[ElementPatch],
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve(project_id)?;
    let active_state_id = session.active_state_id.as_deref();
    let session_id = session.id.clone();
    let mut parsed = Vec::with_capacity(patches.len());
    for patch in patches {
        let state_id = effective_state_id_for_element_update(
            active_state_id,
            patch.state_id.as_deref(),
            patch.edit_scope,
        )?
        .ok_or("state_id or edit_scope:'state' is required for state override updates")?;
        let override_patch = parse_element_state_patch(&patch.changes)?;
        parsed.push((patch.id.clone(), state_id, override_patch));
    }

    let changed_count = {
        let mut preview = session.project.clone();
        let mut changed_count = 0usize;
        for (element_id, state_id, patch) in &parsed {
            if preview.update_element_state_override(state_id, element_id, patch.clone())? {
                changed_count += 1;
            }
        }
        changed_count
    };

    if changed_count == 0 {
        let session = sessions.resolve(project_id)?;
        return Ok(serde_json::json!({
            "project_id": session_id,
            "revision": session.revision,
            "updated_count": 0,
            "state_overrides": session.project.state_overrides,
        }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    for (element_id, state_id, patch) in &parsed {
        session
            .project
            .update_element_state_override(state_id, element_id, patch.clone())?;
    }
    sessions.mark_changed(project_id)?;
    let session = sessions.resolve(project_id)?;
    Ok(serde_json::json!({
        "project_id": session_id,
        "revision": session.revision,
        "updated_count": changed_count,
        "state_overrides": session.project.state_overrides,
    }))
}

fn element_resize(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    let width = required_u32(args, "width")?;
    let height = required_u32(args, "height")?;
    let current = sessions
        .resolve(project_id)?
        .project
        .find_element(id)
        .ok_or("Element not found")?;
    let x = args
        .get("x")
        .and_then(|value| value.as_i64())
        .unwrap_or(current.x as i64) as i32;
    let y = args
        .get("y")
        .and_then(|value| value.as_i64())
        .unwrap_or(current.y as i64) as i32;
    let mut updated = current.clone();
    updated.x = x;
    updated.y = y;
    if updated.element_type == crate::project::ElementType::Slot {
        updated.size = Some(width.max(height).max(8));
    } else {
        updated.width = Some(width.max(4));
        updated.height = Some(height.max(4));
    }
    if &updated == current {
        return Ok(serde_json::to_value(current).unwrap());
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    *session
        .project
        .find_element_mut(id)
        .ok_or("Element not found")? = updated.clone();
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(updated).unwrap())
}

fn element_reorder(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    let index = args
        .get("index")
        .and_then(|value| value.as_u64())
        .ok_or("Missing index")? as usize;
    let session = sessions.resolve(project_id)?;
    let current_index = session
        .project
        .elements
        .iter()
        .position(|element| element.id == id)
        .ok_or("Element not found")?;
    let target_index = index.min(session.project.elements.len().saturating_sub(1));
    if current_index == target_index {
        return Ok(serde_json::to_value(session_summary(sessions, &session.id)?).unwrap());
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = session.project.elements.remove(current_index);
    session.project.elements.insert(target_index, element);
    Ok(serde_json::to_value(sessions.mark_changed(project_id)?).unwrap())
}

fn element_remove(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    if sessions
        .resolve(project_id)?
        .project
        .find_element(id)
        .is_none()
    {
        return Ok(serde_json::json!({ "removed": false }));
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let removed = session.project.remove_element(id).is_some();
    if removed {
        sessions.mark_changed(project_id)?;
    }
    Ok(serde_json::json!({ "removed": removed }))
}

fn group_create(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let element_ids = string_array(args, "element_ids")?;
    let group_id = args
        .get("group_id")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("group_{}", uuid::Uuid::new_v4()));

    if sessions
        .resolve(project_id)?
        .project
        .groups
        .iter()
        .any(|group| group.id == group_id)
    {
        return Err("Group already exists".to_string());
    }
    let element_ids = validate_group_members(sessions, project_id, &element_ids)?;

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let group = session.project.group_elements(group_id, element_ids)?;
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(group).unwrap())
}

fn group_upsert(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let group_id = required_str(args, "group_id")?.to_string();
    let element_ids = string_array(args, "element_ids")?;
    let element_ids = validate_group_members(sessions, project_id, &element_ids)?;
    let project = &sessions.resolve(project_id)?.project;
    let existing = project.groups.iter().find(|group| group.id == group_id);
    let created = existing.is_none();
    let x = match existing {
        Some(group) => group.x,
        None => min_element_coordinate(project, &element_ids, true)?,
    };
    let y = match existing {
        Some(group) => group.y,
        None => min_element_coordinate(project, &element_ids, false)?,
    };
    let next = crate::project::Group {
        id: group_id.clone(),
        x,
        y,
        elements: element_ids,
        visible: existing.and_then(|group| group.visible),
        state_owned: Vec::new(),
    };
    let mut next_groups = Vec::new();
    let mut target_applied = false;
    for group in &project.groups {
        if group.id == group_id {
            next_groups.push(next.clone());
            target_applied = true;
            continue;
        }

        let mut group = group.clone();
        group
            .elements
            .retain(|element_id| !next.elements.iter().any(|id| id == element_id));
        if group.elements.len() >= 2 {
            next_groups.push(group);
        }
    }
    if !target_applied {
        next_groups.push(next.clone());
    }

    if project.groups == next_groups {
        let session = sessions.resolve(project_id)?;
        let member_count = next.elements.len();
        return Ok(serde_json::json!({
            "project_id": session.id,
            "group": next,
            "created": false,
            "updated": false,
            "member_count": member_count
        }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.groups = next_groups;
    let session_id = session.id.clone();
    let member_count = next.elements.len();
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({
        "project_id": session_id,
        "group": next,
        "created": created,
        "updated": !created,
        "member_count": member_count
    }))
}

fn validate_group_members(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    element_ids: &[String],
) -> Result<Vec<String>, String> {
    let project = &sessions.resolve(project_id)?.project;
    let mut unique_ids = Vec::new();
    for id in element_ids {
        if !unique_ids.contains(id) {
            unique_ids.push(id.clone());
        }
        if project.find_element(id).is_none() {
            return Err(format!("Element not found: {id}"));
        }
    }
    if unique_ids.len() < 2 {
        return Err("At least two elements are required to create a group".to_string());
    }
    Ok(unique_ids)
}

fn string_array(value: &serde_json::Value, key: &str) -> Result<Vec<String>, String> {
    value
        .get(key)
        .and_then(|value| value.as_array())
        .ok_or(format!("Missing {key}"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or(format!("{key} must contain only strings"))
        })
        .collect()
}

fn min_element_coordinate(
    project: &Project,
    element_ids: &[String],
    x_axis: bool,
) -> Result<i32, String> {
    element_ids
        .iter()
        .filter_map(|id| project.find_element(id))
        .map(|element| if x_axis { element.x } else { element.y })
        .min()
        .ok_or("Group must contain at least one existing element".to_string())
}

fn group_ungroup(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let group_id = required_str(args, "group_id")?;
    if !sessions
        .resolve(project_id)?
        .project
        .groups
        .iter()
        .any(|group| group.id == group_id)
    {
        return Ok(serde_json::json!({ "removed": false }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let removed = session.project.ungroup(group_id);
    if removed {
        sessions.mark_changed(project_id)?;
    }
    Ok(serde_json::json!({ "removed": removed }))
}

fn attached_region_add(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let region = parse_attached_region_arg(args)?;
    let project = &sessions.resolve(project_id)?.project;
    if project.find_attached_region(&region.id).is_some() {
        return Err(format!("Attached region already exists: {}", region.id));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.attached_regions.push(region.clone());
    sessions.mark_changed(project_id)?;
    serde_json::to_value(region).map_err(|error| error.to_string())
}

fn attached_region_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    let changes = args
        .get("changes")
        .ok_or("Missing changes")?
        .as_object()
        .ok_or("Attached region changes must be an object")?;
    let current = sessions
        .resolve(project_id)?
        .project
        .find_attached_region(id)
        .ok_or_else(|| format!("Attached region not found: {id}"))?;
    if changes
        .get("id")
        .is_some_and(|value| value.as_str() != Some(current.id.as_str()))
    {
        return Err("Attached region id cannot be changed".to_string());
    }

    let mut value = serde_json::to_value(current)
        .map_err(|error| format!("Failed to encode attached region: {error}"))?;
    let target = value
        .as_object_mut()
        .ok_or("Attached region payload must be an object")?;
    for (key, value) in changes {
        if key == "id" {
            continue;
        }
        target.insert(key.clone(), value.clone());
    }
    let updated: AttachedRegion = serde_json::from_value(value)
        .map_err(|error| format!("Invalid attached region update: {error}"))?;
    if &updated == current {
        return serde_json::to_value(current).map_err(|error| error.to_string());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    *session
        .project
        .find_attached_region_mut(id)
        .ok_or_else(|| format!("Attached region not found: {id}"))? = updated.clone();
    sessions.mark_changed(project_id)?;
    serde_json::to_value(updated).map_err(|error| error.to_string())
}

fn attached_region_remove(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    let project = &sessions.resolve(project_id)?.project;
    if project.find_attached_region(id).is_none() {
        return Ok(serde_json::json!({ "removed": false }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session
        .project
        .attached_regions
        .retain(|region| region.id != id);
    for element in &mut session.project.elements {
        if element.attached_region.as_deref() == Some(id) {
            element.attached_region = None;
        }
    }
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({ "removed": true }))
}

fn attached_region_list(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve(project_id)?;
    Ok(serde_json::json!({ "attached_regions": session.project.attached_regions }))
}

fn attached_region_move_with_elements(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?.to_string();
    let x = required_i32(args, "x")?;
    let y = required_i32(args, "y")?;
    let project = &sessions.resolve(project_id)?.project;
    let current = project
        .find_attached_region(&id)
        .ok_or_else(|| format!("Attached region not found: {id}"))?;
    if current.x == x && current.y == y {
        return serde_json::to_value(current).map_err(|error| error.to_string());
    }

    let dx = x
        .checked_sub(current.x)
        .ok_or("Attached region move overflow")?;
    let dy = y
        .checked_sub(current.y)
        .ok_or("Attached region move overflow")?;
    let moved_child_ids = project
        .elements
        .iter()
        .filter(|element| element.attached_region.as_deref() == Some(id.as_str()))
        .map(|element| {
            element
                .x
                .checked_add(dx)
                .ok_or("Attached region child move overflow")?;
            element
                .y
                .checked_add(dy)
                .ok_or("Attached region child move overflow")?;
            Ok(element.id.clone())
        })
        .collect::<Result<Vec<_>, String>>()?;

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let updated = {
        let region = session
            .project
            .find_attached_region_mut(&id)
            .ok_or_else(|| format!("Attached region not found: {id}"))?;
        region.x = x;
        region.y = y;
        region.clone()
    };
    for element in &mut session.project.elements {
        if element.attached_region.as_deref() == Some(id.as_str()) {
            element.x = element
                .x
                .checked_add(dx)
                .ok_or("Attached region child move overflow")?;
            element.y = element
                .y
                .checked_add(dy)
                .ok_or("Attached region child move overflow")?;
        }
    }
    refresh_group_positions_for_elements(&mut session.project, &moved_child_ids);
    sessions.mark_changed(project_id)?;
    serde_json::to_value(updated).map_err(|error| error.to_string())
}

fn parse_attached_region_arg(args: &serde_json::Value) -> Result<AttachedRegion, String> {
    let mut payload = args.clone();
    let object = payload
        .as_object_mut()
        .ok_or("Attached region payload must be an object")?;
    object
        .entry("state".to_string())
        .or_insert_with(|| serde_json::json!("static"));
    object
        .entry("visible".to_string())
        .or_insert(serde_json::Value::Bool(true));
    serde_json::from_value(payload)
        .map_err(|error| format!("Invalid attached region payload: {error}"))
}

fn refresh_group_positions_for_elements(project: &mut Project, moved_ids: &[String]) {
    if moved_ids.is_empty() {
        return;
    }

    let elements = &project.elements;
    for group in &mut project.groups {
        if !group
            .elements
            .iter()
            .any(|element_id| moved_ids.iter().any(|moved_id| moved_id == element_id))
        {
            continue;
        }

        let mut positions = group.elements.iter().filter_map(|element_id| {
            elements
                .iter()
                .find(|element| element.id == *element_id)
                .map(|element| (element.x, element.y))
        });
        if let Some((mut min_x, mut min_y)) = positions.next() {
            for (x, y) in positions {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
            }
            group.x = min_x;
            group.y = min_y;
        }
    }
}

fn animation_create(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let payload = args.get("animation").unwrap_or(args).clone();
    let animation: Animation = serde_json::from_value(payload)
        .map_err(|error| format!("Invalid animation payload: {error}"))?;
    if sessions
        .resolve(project_id)?
        .project
        .animations
        .iter()
        .any(|existing| existing.id == animation.id)
    {
        return Err("Animation already exists".to_string());
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.animations.push(animation.clone());
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(animation).unwrap())
}

fn animation_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    let changes = args
        .get("changes")
        .ok_or("Missing changes")?
        .as_object()
        .ok_or("Animation changes must be an object")?;
    let current = sessions
        .resolve(project_id)?
        .project
        .animations
        .iter()
        .find(|animation| animation.id == id)
        .ok_or("Animation not found")?;
    let mut value = serde_json::to_value(current).map_err(|error| error.to_string())?;
    let target = value
        .as_object_mut()
        .ok_or("Animation payload must be an object")?;
    for (key, value) in changes {
        if key == "id" || key == "type" {
            continue;
        }
        target.insert(key.clone(), value.clone());
    }
    let updated: Animation = serde_json::from_value(value)
        .map_err(|error| format!("Invalid animation update: {error}"))?;
    if &updated == current {
        return Ok(serde_json::to_value(current).unwrap());
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let animation = session
        .project
        .animations
        .iter_mut()
        .find(|animation| animation.id == id)
        .ok_or("Animation not found")?;
    *animation = updated.clone();
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(updated).unwrap())
}

fn animation_remove(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = required_str(args, "id")?;
    if !sessions
        .resolve(project_id)?
        .project
        .animations
        .iter()
        .any(|animation| animation.id == id)
    {
        return Ok(serde_json::json!({ "removed": false }));
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session
        .project
        .animations
        .retain(|animation| animation.id != id);
    for element in &mut session.project.elements {
        if element.animation.as_deref() == Some(id) {
            element.animation = None;
        }
    }
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({ "removed": true }))
}

fn animation_bind(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let element_id = required_str(args, "element_id")?;
    let animation_id = required_str(args, "animation_id")?;
    let project = &sessions.resolve(project_id)?.project;
    if !project
        .animations
        .iter()
        .any(|animation| animation.id == animation_id)
    {
        return Err("Animation not found".to_string());
    }
    let current = project
        .find_element(element_id)
        .ok_or("Element not found")?;
    if current.animation.as_deref() == Some(animation_id) {
        return Ok(serde_json::to_value(current).unwrap());
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = session
        .project
        .find_element_mut(element_id)
        .ok_or("Element not found")?;
    element.animation = Some(animation_id.to_string());
    let element = element.clone();
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(element).unwrap())
}

fn animation_unbind(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let element_id = required_str(args, "element_id")?;
    let current = sessions
        .resolve(project_id)?
        .project
        .find_element(element_id)
        .ok_or("Element not found")?;
    if current.animation.is_none() {
        return Ok(serde_json::to_value(current).unwrap());
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = session
        .project
        .find_element_mut(element_id)
        .ok_or("Element not found")?;
    element.animation = None;
    let element = element.clone();
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(element).unwrap())
}

fn asset_import(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let file_path = required_str(args, "file_path")?;
    let data = std::fs::read(file_path).map_err(|error| format!("Failed to read file: {error}"))?;
    let image = image::load_from_memory(&data)
        .map_err(|error| format!("Failed to decode image: {error}"))?;
    let asset_path = if let Some(name) = optional_string(args, "name") {
        validate_asset_name(&name)?;
        name
    } else {
        let name = std::path::Path::new(file_path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("texture");
        let asset_path = format!("textures/{name}.png");
        validate_asset_name(&asset_path)?;
        asset_path
    };
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session
        .project
        .texture_data
        .insert(asset_path.clone(), data.clone());
    if !session.project.assets.contains(&asset_path) {
        session.project.assets.push(asset_path.clone());
    }
    sessions.mark_changed(project_id)?;

    Ok(compact_asset_metadata_with_dimensions(
        &asset_path,
        &data,
        image.width(),
        image.height(),
    ))
}

fn asset_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let name = required_str(args, "name")?;
    let data_url = required_str(args, "data_url")?;
    let data = decode_png_data_url(data_url)?;
    let image =
        image::load_from_memory(&data).map_err(|error| format!("Failed to decode PNG: {error}"))?;
    if !sessions
        .resolve(project_id)?
        .project
        .assets
        .iter()
        .any(|asset| asset == name)
    {
        return Err(format!("Asset not found: {name}"));
    }
    if sessions
        .resolve(project_id)?
        .project
        .texture_data
        .get(name)
        .is_some_and(|current| current == &data)
    {
        return Ok(serde_json::json!({
            "name": name,
            "width": image.width(),
            "height": image.height(),
            "data_url": data_url,
        }));
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.texture_data.insert(name.to_string(), data);
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({
        "name": name,
        "width": image.width(),
        "height": image.height(),
        "data_url": data_url,
    }))
}

fn asset_metadata_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let name = required_str(args, "name")?;
    let metadata_value = args.get("metadata").ok_or("Missing metadata")?.clone();
    let metadata: AssetMetadata = serde_json::from_value(metadata_value)
        .map_err(|error| format!("Invalid asset metadata: {error}"))?;

    let session_id = {
        let project = &sessions.resolve(project_id)?.project;
        if !project.assets.iter().any(|asset| asset == name) {
            return Err(format!("Asset not found: {name}"));
        }
        if let Some(current) = project.asset_metadata.get(name) {
            if current == &metadata {
                return Ok(serde_json::json!({
                    "project_id": sessions.resolve(project_id)?.id,
                    "name": name,
                    "metadata": current,
                }));
            }
        } else if metadata == AssetMetadata::default() {
            return Ok(serde_json::json!({
                "project_id": sessions.resolve(project_id)?.id,
                "name": name,
                "metadata": metadata,
            }));
        }
        sessions.resolve(project_id)?.id.clone()
    };

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session
        .project
        .asset_metadata
        .insert(name.to_string(), metadata.clone());
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({
        "project_id": session_id,
        "name": name,
        "metadata": metadata,
    }))
}

fn asset_remove(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let name = required_str(args, "name")?;
    let exists = {
        let project = &sessions.resolve(project_id)?.project;
        project.assets.iter().any(|asset| asset == name)
            || project.texture_data.contains_key(name)
            || project.asset_metadata.contains_key(name)
    };
    if !exists {
        return Ok(serde_json::json!({ "removed": false }));
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let removed_texture = session.project.texture_data.remove(name).is_some();
    let removed_metadata = session.project.asset_metadata.remove(name).is_some();
    let old_len = session.project.assets.len();
    session.project.assets.retain(|asset| asset != name);
    let removed_asset = session.project.assets.len() != old_len;
    if removed_texture || removed_asset || removed_metadata {
        sessions.mark_changed(project_id)?;
    }
    Ok(serde_json::json!({ "removed": removed_texture || removed_asset || removed_metadata }))
}

fn project_render(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve(project_id)?;
    let state_id = optional_state_id(args, "state_id")?;
    let render_project = session.project.effective_for_state(state_id.as_deref())?;
    let png = crate::texture::composite_project_preview(&render_project)?;
    let path = optional_string(args, "output_path")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "mc-gui-crafter-render-{}.png",
                uuid::Uuid::new_v4()
            ))
        });
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        return Err("output_path must end with .png".to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create render directory: {error}"))?;
    }
    std::fs::write(&path, &png).map_err(|error| format!("Failed to write render PNG: {error}"))?;

    let image = image::load_from_memory(&png)
        .map_err(|error| format!("Failed to inspect render PNG: {error}"))?;
    let mut metadata = compact_asset_metadata_with_dimensions(
        path.to_string_lossy().as_ref(),
        &png,
        image.width(),
        image.height(),
    );
    metadata["project_id"] = serde_json::json!(session.id);
    if let Some(state_id) = state_id {
        metadata["state_id"] = serde_json::json!(state_id);
    }
    metadata["path"] = serde_json::json!(path.to_string_lossy().to_string());
    if optional_bool(args, "include_data_url")?.unwrap_or(false) {
        metadata["data_url"] = serde_json::json!(data_url_for_png(&png));
    }
    Ok(metadata)
}

fn asset_get_data_url(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let name = required_str(args, "name")?;
    let project = &sessions.resolve(project_id)?.project;
    let data = project
        .texture_data
        .get(name)
        .ok_or(format!("Asset not found: {name}"))?;
    Ok(serde_json::json!({
        "name": name,
        "data_url": data_url_for_png(data),
    }))
}

fn asset_list(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let project = &sessions.resolve(project_id)?.project;
    let assets = project
        .assets
        .iter()
        .map(|name| compact_asset_metadata(name, project.texture_data.get(name).map(Vec::as_slice)))
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "assets": assets }))
}

fn compact_asset_metadata(name: &str, data: Option<&[u8]>) -> serde_json::Value {
    let Some(data) = data else {
        return compact_asset_metadata_with_dimensions(name, &[], 16, 16);
    };
    let image = image::load_from_memory(data).ok();
    compact_asset_metadata_with_dimensions(
        name,
        data,
        image.as_ref().map(|image| image.width()).unwrap_or(16),
        image.as_ref().map(|image| image.height()).unwrap_or(16),
    )
}

fn compact_asset_metadata_with_dimensions(
    name: &str,
    data: &[u8],
    width: u32,
    height: u32,
) -> serde_json::Value {
    use sha2::{Digest, Sha256};

    serde_json::json!({
        "name": name,
        "width": width,
        "height": height,
        "bytes": data.len(),
        "sha256": format!("{:x}", Sha256::digest(data)),
    })
}

fn validate_asset_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Asset name cannot be empty".to_string());
    }
    if name.contains('\\') || std::path::Path::new(name).is_absolute() {
        return Err("Asset name must be a relative project path".to_string());
    }
    if !name.starts_with("textures/") || !name.ends_with(".png") {
        return Err("Asset name must start with textures/ and end with .png".to_string());
    }
    let texture_name = name
        .strip_prefix("textures/")
        .and_then(|name| name.strip_suffix(".png"))
        .unwrap_or_default();
    if texture_name.is_empty() || texture_name.ends_with('/') {
        return Err("Asset name must include a texture filename".to_string());
    }
    if name
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(
            "Asset name cannot contain empty, current, or parent path components".to_string(),
        );
    }
    Ok(())
}

fn element_for_mcp(element: &Element) -> serde_json::Value {
    let mut value = serde_json::to_value(element).unwrap();
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "layer".to_string(),
            serde_json::to_value(&element.layer).unwrap(),
        );
    }
    value
}

fn data_url_for_png(data: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(data)
    )
}

fn decode_png_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let Some(payload) = data_url.strip_prefix("data:image/png;base64,") else {
        return Err("Invalid asset data URL: expected data:image/png;base64,...".to_string());
    };
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|error| format!("Invalid PNG base64 payload: {error}"))
}

fn project_result(
    sessions: &ProjectSessionManager,
    project_id: &str,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve(Some(project_id))?;
    Ok(serde_json::json!({
        "project_id": project_id,
        "project": session.project,
        "summary": session_summary(sessions, project_id)?,
    }))
}

fn session_summary(
    sessions: &ProjectSessionManager,
    project_id: &str,
) -> Result<crate::project::ProjectSessionSummary, String> {
    sessions
        .list_sessions()
        .into_iter()
        .find(|summary| summary.id == project_id)
        .ok_or("Project session not found".to_string())
}

fn optional_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(String::from)
}

fn optional_state_id(value: &serde_json::Value, key: &str) -> Result<Option<String>, String> {
    let Some(value) = value.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| format!("{key} must be a string or null"))
}

fn optional_bool(value: &serde_json::Value, key: &str) -> Result<Option<bool>, String> {
    value
        .get(key)
        .map(|value| value.as_bool().ok_or(format!("{key} must be boolean")))
        .transpose()
}

fn nullable_string(value: &serde_json::Value, key: &str) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| format!("{key} must be a string or null"))
}

fn attached_region_override(
    value: &serde_json::Value,
    key: &str,
) -> Result<ElementAttachedRegionStateOverride, String> {
    if value.is_null() {
        return Ok(ElementAttachedRegionStateOverride::Detached);
    }
    value
        .as_str()
        .map(|value| ElementAttachedRegionStateOverride::Region(value.to_string()))
        .ok_or_else(|| format!("{key} must be a string or null"))
}

fn nullable_bool(value: &serde_json::Value, key: &str) -> Result<Option<bool>, String> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| format!("{key} must be boolean or null"))
}

fn nullable_i32(value: &serde_json::Value, key: &str) -> Result<Option<i32>, String> {
    if value.is_null() {
        return Ok(None);
    }
    if let Some(value) = value.as_i64() {
        return i32::try_from(value)
            .map(Some)
            .map_err(|_| format!("{key} is out of range"));
    }
    if let Some(value) = value.as_u64() {
        return i32::try_from(value)
            .map(Some)
            .map_err(|_| format!("{key} is out of range"));
    }
    Err(format!("{key} must be an integer or null"))
}

fn nullable_u32(value: &serde_json::Value, key: &str) -> Result<Option<u32>, String> {
    if value.is_null() {
        return Ok(None);
    }
    json_number_to_u32(value, key).map(Some)
}

fn required_str<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .ok_or(format!("Missing {key}"))
}

fn required_i32(value: &serde_json::Value, key: &str) -> Result<i32, String> {
    let value = value.get(key).ok_or(format!("Missing {key}"))?;
    if let Some(value) = value.as_i64() {
        return i32::try_from(value).map_err(|_| format!("{key} is out of range"));
    }
    if let Some(value) = value.as_u64() {
        return i32::try_from(value).map_err(|_| format!("{key} is out of range"));
    }
    Err(format!("{key} must be an integer"))
}

fn required_u32(value: &serde_json::Value, key: &str) -> Result<u32, String> {
    let value = value.get(key).ok_or(format!("Missing {key}"))?;
    json_number_to_u32(value, key)
}

fn optional_u32(value: &serde_json::Value, key: &str) -> Result<Option<u32>, String> {
    value
        .get(key)
        .map(|value| json_number_to_u32(value, key))
        .transpose()
}

fn json_number_to_u32(value: &serde_json::Value, key: &str) -> Result<u32, String> {
    if let Some(value) = value.as_i64() {
        return u32::try_from(value).map_err(|_| format!("{key} is out of range"));
    }
    if let Some(value) = value.as_u64() {
        return u32::try_from(value).map_err(|_| format!("{key} is out of range"));
    }
    Err(format!("{key} must be an integer"))
}

fn optional_slot_role(
    value: &serde_json::Value,
    key: &str,
) -> Result<Option<crate::project::SlotRole>, String> {
    value
        .get(key)
        .map(|value| {
            serde_json::from_value(value.clone()).map_err(|error| format!("Invalid {key}: {error}"))
        })
        .transpose()
}

fn optional_semantic_group_kind(
    value: &serde_json::Value,
    key: &str,
) -> Result<Option<crate::project::SemanticGroupKind>, String> {
    value
        .get(key)
        .map(|value| {
            serde_json::from_value(value.clone()).map_err(|error| format!("Invalid {key}: {error}"))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, TcpListener};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    fn test_state() -> AppState {
        let log_dir = std::env::temp_dir().join(format!(
            "mc-gui-crafter-mcp-test-log-{}",
            crate::session_log::timestamp_millis()
        ));
        AppState {
            sessions: Mutex::new(ProjectSessionManager::default()),
            mcp_handle: Mutex::new(None),
            app_handle: Mutex::new(None),
            session_log: Mutex::new(crate::session_log::SessionLogger::new(&log_dir).unwrap()),
        }
    }

    fn response_for(value: serde_json::Value, state: &AppState) -> serde_json::Value {
        match handle_json_rpc_value(value, state) {
            RpcReply::Response(response) => serde_json::to_value(response).unwrap(),
            RpcReply::Notification => panic!("expected JSON-RPC response"),
        }
    }

    fn tool_text_value(response: &serde_json::Value) -> serde_json::Value {
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(content).unwrap()
    }

    fn attached_region_tool_call(
        state: &AppState,
        tool_name: &str,
        project_id: &str,
        arguments: serde_json::Value,
    ) -> serde_json::Value {
        let mut args = arguments.as_object().cloned().unwrap_or_default();
        args.insert(
            "project_id".to_string(),
            serde_json::Value::String(project_id.to_string()),
        );
        response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": tool_name,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": args
                }
            }),
            state,
        )
    }

    fn revision_for(state: &AppState, project_id: &str) -> u64 {
        state
            .sessions
            .lock()
            .unwrap()
            .resolve(Some(project_id))
            .unwrap()
            .revision
    }

    fn mutation_snapshot_for(
        state: &AppState,
        project_id: &str,
    ) -> Option<ProjectMutationSnapshot> {
        project_mutation_snapshot(state, &serde_json::json!({ "project_id": project_id }))
    }

    #[test]
    fn new_alpha_mutation_responses_include_project_id() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Response Contract", 176, 166, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "a",
                    "type": "slot",
                    "x": 8,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "b",
                    "type": "slot",
                    "x": 26,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };

        for (tool_name, arguments) in [
            (
                "project_resize",
                serde_json::json!({ "project_id": project_id, "width": 180, "height": 166 }),
            ),
            (
                "group_upsert",
                serde_json::json!({ "project_id": project_id, "group_id": "machine", "element_ids": ["a", "b"] }),
            ),
            (
                "element_update_many",
                serde_json::json!({ "project_id": project_id, "updates": [{ "id": "a", "changes": { "x": 10 } }] }),
            ),
        ] {
            let response = response_for(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": tool_name,
                    "method": "tools/call",
                    "params": {
                        "name": tool_name,
                        "arguments": arguments
                    }
                }),
                &state,
            );
            assert!(response["error"].is_null(), "{tool_name}: {response:#}");
            let value = tool_text_value(&response);
            assert_eq!(value["project_id"], project_id, "{tool_name}");
        }
    }

    #[test]
    fn state_set_active_is_mutating_for_project_changed_detection() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("State Activation", 176, 166, ModTarget::Forge);
            project.states.push(ProjectState {
                id: "expanded".into(),
                label: "Expanded".into(),
                description: None,
                initial: true,
                export_role: None,
            });
            sessions.create_session(project)
        };
        let args = serde_json::json!({ "project_id": project_id });
        let before = project_mutation_snapshot(&state, &args);

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-set-active",
                "method": "tools/call",
                "params": {
                    "name": "state_set_active",
                    "arguments": {
                        "project_id": project_id,
                        "state_id": "expanded",
                        "edit_scope": "state"
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        assert!(is_mutating_tool("state_set_active"));
        assert!(should_emit_project_changed(
            "state_set_active",
            &state,
            &args,
            before
        ));
    }

    #[test]
    fn project_render_and_asset_list_do_not_inline_binary_payloads_by_default() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Compact", 32, 24, ModTarget::Forge);
            let asset = "textures/generated/gui_panel.png";
            project.assets.push(asset.to_string());
            project.texture_data.insert(
                asset.to_string(),
                crate::texture::generated_gui_panel(16, 16).unwrap(),
            );
            sessions.create_session(project)
        };
        let render_output_path = TempPath::new("mc-gui-crafter-render-compact-test", "png");

        for (tool_name, arguments) in [
            (
                "project_render",
                serde_json::json!({
                    "project_id": project_id,
                    "output_path": render_output_path.path_string()
                }),
            ),
            (
                "asset_list",
                serde_json::json!({
                    "project_id": project_id
                }),
            ),
        ] {
            let response = response_for(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": tool_name,
                    "method": "tools/call",
                    "params": {
                        "name": tool_name,
                        "arguments": arguments
                    }
                }),
                &state,
            );
            assert!(response["error"].is_null(), "{tool_name}: {response:#}");
            let value = tool_text_value(&response);
            assert!(
                !serde_json::to_string(&value)
                    .unwrap()
                    .contains("data:image/png;base64"),
                "{tool_name} should be compact by default"
            );
        }
    }

    #[test]
    fn no_op_batch_tools_do_not_change_revision() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Noop Batch", 176, 166, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "a",
                    "type": "slot",
                    "x": 8,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "b",
                    "type": "slot",
                    "x": 26,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            project.groups.push(crate::project::Group {
                id: "machine".to_string(),
                x: 8,
                y: 18,
                elements: vec!["a".to_string(), "b".to_string()],
                visible: None,
                state_owned: Vec::new(),
            });
            sessions.create_session(project)
        };

        let calls = [
            (
                "project_resize",
                serde_json::json!({ "project_id": project_id, "width": 176, "height": 166 }),
            ),
            (
                "group_upsert",
                serde_json::json!({ "project_id": project_id, "group_id": "machine", "element_ids": ["a", "b"] }),
            ),
            (
                "element_update_many",
                serde_json::json!({ "project_id": project_id, "updates": [{ "id": "a", "changes": { "x": 8 } }] }),
            ),
        ];

        for (tool_name, arguments) in calls {
            let before = mutation_snapshot_for(&state, &project_id);
            let response = response_for(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": tool_name,
                    "method": "tools/call",
                    "params": {
                        "name": tool_name,
                        "arguments": arguments
                    }
                }),
                &state,
            );
            assert!(response["error"].is_null(), "{tool_name}: {response:#}");
            assert_eq!(
                mutation_snapshot_for(&state, &project_id),
                before,
                "{tool_name}"
            );
        }
    }

    fn attached_region_project(state: &AppState) -> String {
        let mut project = Project::new("Attached Region Regression", 176, 166, ModTarget::Forge);
        project.attached_regions.push(AttachedRegion {
            id: "returns_pocket".to_string(),
            anchor: crate::project::AttachedRegionAnchor::Right,
            x: 100,
            y: 18,
            width: 54,
            height: 72,
            state: crate::project::AttachedRegionState::Static,
            kind: Some("returns_pocket".to_string()),
            semantic_group: Some("food_returns".to_string()),
            visible: true,
            state_owned: Vec::new(),
        });
        state.sessions.lock().unwrap().create_session(project)
    }

    struct TempPath {
        path: PathBuf,
    }

    impl TempPath {
        fn new(prefix: &str, extension: &str) -> Self {
            let file_name = if extension.is_empty() {
                format!("{prefix}-{}", uuid::Uuid::new_v4())
            } else {
                format!("{prefix}-{}.{}", uuid::Uuid::new_v4(), extension)
            };
            Self {
                path: std::env::temp_dir().join(file_name),
            }
        }

        fn from_path(path: PathBuf) -> Self {
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn path_string(&self) -> String {
            self.path.to_string_lossy().into_owned()
        }
    }

    impl Drop for TempPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[test]
    fn initialize_returns_server_info_without_initialized_notification() {
        let state = test_state();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            &state,
        );
        let initialized = handle_json_rpc_value(
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
            &state,
        );

        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["serverInfo"]["name"], SERVER_NAME);
        assert_eq!(
            response["result"]["capabilities"]["tools"],
            serde_json::json!({})
        );
        assert!(matches!(initialized, RpcReply::Notification));
    }

    #[test]
    fn tools_list_contains_live_session_tools() {
        let state = test_state();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "tools",
                "method": "tools/list"
            }),
            &state,
        );
        let names = response["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"project_list_sessions"));
        assert!(names.contains(&"project_get_active"));
        assert!(names.contains(&"project_save_as"));
        assert!(names.contains(&"project_export_preview"));
        assert!(names.contains(&"project_export"));
        assert!(names.contains(&"element_update"));
        assert!(names.contains(&"element_reorder"));
        assert!(names.contains(&"group_create"));
        assert!(names.contains(&"group_ungroup"));
        assert!(names.contains(&"asset_get_data_url"));
        assert!(names.contains(&"project_undo"));
        assert!(names.contains(&"project_redo"));
    }

    #[test]
    fn tools_list_exposes_export_settings_update() {
        let tools = get_tool_definitions();

        assert!(tools
            .iter()
            .any(|tool| tool["name"] == "project_export_settings_update"));
    }

    #[test]
    fn tools_list_exposes_project_render_as_preferred_visual_tool() {
        let tools = get_tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"project_render"));
        assert!(names.contains(&"project_screenshot"));

        let render = tools
            .iter()
            .find(|tool| tool["name"] == "project_render")
            .expect("project_render should be listed");
        assert!(render["description"].as_str().unwrap().contains("Render"));
        assert!(render["inputSchema"]["properties"]
            .as_object()
            .unwrap()
            .contains_key("include_data_url"));
    }

    #[test]
    fn tools_list_exposes_project_resize() {
        let tools = get_tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"project_resize"));
    }

    #[test]
    fn tools_list_exposes_session_report() {
        let tools = get_tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"session_report"));
    }

    #[test]
    fn session_report_writes_feedback_log_entry() {
        let state = test_state();
        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "report",
                "method": "tools/call",
                "params": {
                    "name": "session_report",
                    "arguments": {
                        "summary": "Export warning confusing",
                        "severity": "warning",
                        "details": {
                            "reproduction": "Preview warning should explain resize options.",
                            "expected": "Actionable resize guidance"
                        }
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let log_path = {
            let log = state.session_log.lock().unwrap();
            log.path().to_path_buf()
        };
        let content = std::fs::read_to_string(log_path).unwrap();
        assert!(content.contains("feedback_report"));
        assert!(content.contains("Export warning confusing"));
    }

    #[test]
    fn tools_list_exposes_group_upsert() {
        let tools = get_tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"group_upsert"));
    }

    #[test]
    fn tools_list_exposes_element_update_many() {
        let tools = get_tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"element_update_many"));
    }

    #[test]
    fn element_update_many_schema_exposes_batch_state_scope() {
        let tools = get_tool_definitions();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "element_update_many")
            .expect("element_update_many should be listed");
        let properties = &tool["inputSchema"]["properties"];

        assert_eq!(properties["state_id"]["type"], "string");
        assert_eq!(
            properties["edit_scope"]["enum"],
            serde_json::json!(["base", "state"])
        );
        assert!(properties["updates"]["items"]["properties"]
            .as_object()
            .unwrap()
            .contains_key("state_id"));
    }

    #[test]
    fn tools_list_exposes_schema_discover() {
        let tools = get_tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"schema_discover"));
    }

    #[test]
    fn tools_list_exposes_state_variant_tools() {
        let tools = get_tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        for name in [
            "state_list",
            "state_add",
            "state_update",
            "state_remove",
            "state_set_active",
            "state_override_update",
            "state_override_clear",
        ] {
            assert!(names.contains(&name), "{name} should be listed");
        }
    }

    #[test]
    fn schema_discover_lists_visual_authoring_fields() {
        let state = test_state();
        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "schema-visual-authoring",
                "method": "tools/call",
                "params": {
                    "name": "schema_discover",
                    "arguments": {}
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert!(value["texture_render_modes"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("nine_slice")));
        assert!(value["nine_slice_modes"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("tile")));
        assert!(value["editable_element_fields"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("render_mode")));
        assert!(value["editable_element_fields"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("nine_slice")));
    }

    #[test]
    fn schema_discover_lists_state_override_fields() {
        let state = test_state();
        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "schema-state-variants",
                "method": "tools/call",
                "params": {
                    "name": "schema_discover",
                    "arguments": {}
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert!(value["state_variants"]["element_override_fields"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("visible")));
        assert!(value["state_variants"]["attached_region_override_fields"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("width")));
        assert!(value["tools_accepting_state_id"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("project_render")));
        assert!(value["tools_accepting_state_id"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("project_export_preview")));
        assert!(value["tools_accepting_state_id"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("project_export")));
    }

    #[test]
    fn schema_discover_returns_agent_authoring_enums_and_defaults() {
        let state = test_state();
        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "schema",
                "method": "tools/call",
                "params": {
                    "name": "schema_discover",
                    "arguments": {}
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["mod_targets"], serde_values(ModTarget::variants()));
        assert_eq!(
            value["element_types"],
            serde_values(ElementType::variants())
        );
        assert_eq!(value["slot_roles"], serde_values(SlotRole::variants()));
        assert_eq!(
            value["semantic_group_kinds"],
            serde_values(SemanticGroupKind::variants())
        );
        assert_eq!(
            value["attached_region_anchors"],
            serde_values(AttachedRegionAnchor::variants())
        );
        assert_eq!(
            value["attached_region_states"],
            serde_values(AttachedRegionState::variants())
        );
        assert_eq!(
            value["fill_directions"],
            serde_values(FillDirection::variants())
        );
        assert_eq!(value["layers"], serde_values(Layer::variants()));
        assert_eq!(
            value["texture_render_modes"],
            serde_values(TextureRenderMode::variants())
        );
        assert_eq!(
            value["nine_slice_modes"],
            serde_values(NineSliceMode::variants())
        );
        assert_eq!(
            value["export_settings"]["codegen_modes"],
            serde_values(CodegenMode::variants())
        );
        let export_defaults = ProjectExportSettings::default();
        assert_eq!(
            value["export_settings"]["codegen_mode_default"],
            serde_json::to_value(&export_defaults.codegen_mode).unwrap()
        );
        assert_eq!(
            value["export_settings"]["generate_runtime_helpers_default"],
            export_defaults.generate_runtime_helpers
        );
        assert_eq!(
            value["export_settings"]["generate_semantic_registry_default"],
            export_defaults.generate_semantic_registry
        );
        assert_eq!(
            value["editable_element_fields"],
            serde_json::json!([
                "x",
                "y",
                "width",
                "height",
                "size",
                "asset",
                "icon",
                "icon_uv",
                "tooltip",
                "direction",
                "content",
                "font",
                "color",
                "shadow",
                "animation",
                "visible",
                "uv",
                "render_mode",
                "nine_slice",
                "layer",
                "slot_role",
                "slot_index",
                "inventory_group",
                "scroll_binding",
                "scroll_min",
                "scroll_max",
                "visible_rows",
                "total_rows",
                "columns",
                "target_group",
                "binding",
                "dock",
                "open_width",
                "open_height",
                "attached_region"
            ])
        );
        assert_eq!(
            value["asset_metadata_fields"],
            serde_json::json!(["width", "height", "nine_slice"])
        );
        assert_eq!(
            value["serialization_defaults"],
            schema_serialization_defaults()
        );
    }

    #[test]
    fn tools_list_exposes_alpha_ergonomics_tools() {
        let tools = get_tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"project_save_as"));
        assert!(names.contains(&"project_export_preview"));
        assert!(names.contains(&"project_export"));
        assert!(names.contains(&"project_export_settings_update"));
        assert!(names.contains(&"project_semantic_groups_update"));
        assert!(names.contains(&"element_add_many"));
        assert!(names.contains(&"slot_grid_add"));
    }

    #[test]
    fn tools_list_exposes_attached_region_tools() {
        let state = test_state();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "tools",
                "method": "tools/list"
            }),
            &state,
        );
        let names = response["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"attached_region_add"));
        assert!(names.contains(&"attached_region_update"));
        assert!(names.contains(&"attached_region_remove"));
        assert!(names.contains(&"attached_region_list"));
        assert!(names.contains(&"attached_region_move_with_elements"));
    }

    #[test]
    fn attached_region_schemas_describe_defaults_and_update_fields() {
        let tools = get_tool_definitions();
        let add_schema = tools
            .iter()
            .find(|tool| tool["name"] == "attached_region_add")
            .unwrap();
        let update_schema = tools
            .iter()
            .find(|tool| tool["name"] == "attached_region_update")
            .unwrap();

        assert!(
            add_schema["inputSchema"]["properties"]["state"]["description"]
                .as_str()
                .unwrap()
                .contains("Defaults to static")
        );
        assert!(
            add_schema["inputSchema"]["properties"]["visible"]["description"]
                .as_str()
                .unwrap()
                .contains("Defaults to true")
        );
        let changes = &update_schema["inputSchema"]["properties"]["changes"];
        assert_eq!(changes["type"], "object");
        assert!(changes["description"]
            .as_str()
            .unwrap()
            .contains("id cannot be changed"));
        for field in [
            "anchor",
            "x",
            "y",
            "width",
            "height",
            "state",
            "kind",
            "semantic_group",
            "visible",
        ] {
            assert!(changes["properties"].get(field).is_some(), "{field}");
        }
        assert_eq!(
            changes["properties"]["anchor"]["description"],
            attached_region_anchor_description()
        );
        assert_eq!(
            changes["properties"]["state"]["description"],
            attached_region_state_description()
        );
    }

    #[test]
    fn group_upsert_creates_and_updates_existing_group() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Group Upsert", 176, 166, ModTarget::Forge);
            for (id, x) in [("a", 8), ("b", 26), ("c", 44)] {
                project.elements.push(
                    parse_element_arg(&serde_json::json!({
                        "id": id,
                        "type": "slot",
                        "x": x,
                        "y": 18,
                        "size": 18
                    }))
                    .unwrap(),
                );
            }
            sessions.create_session(project)
        };

        let create_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "group-upsert-create",
                "method": "tools/call",
                "params": {
                    "name": "group_upsert",
                    "arguments": {
                        "project_id": project_id,
                        "group_id": "machine",
                        "element_ids": ["a", "b"]
                    }
                }
            }),
            &state,
        );
        assert!(create_response["error"].is_null(), "{create_response:#}");
        let create_value = tool_text_value(&create_response);
        assert_eq!(create_value["project_id"], project_id);
        assert_eq!(create_value["created"], true);
        assert_eq!(create_value["updated"], false);
        assert_eq!(create_value["member_count"], 2);

        let update_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "group-upsert-update",
                "method": "tools/call",
                "params": {
                    "name": "group_upsert",
                    "arguments": {
                        "project_id": project_id,
                        "group_id": "machine",
                        "element_ids": ["a", "c"]
                    }
                }
            }),
            &state,
        );
        assert!(update_response["error"].is_null(), "{update_response:#}");
        let update_value = tool_text_value(&update_response);
        assert_eq!(update_value["created"], false);
        assert_eq!(update_value["updated"], true);
        assert_eq!(update_value["member_count"], 2);

        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.groups.len(), 1);
        assert_eq!(
            session.project.groups[0].elements,
            vec!["a".to_string(), "c".to_string()]
        );
        assert_eq!(session.project.groups[0].x, 8);
        assert_eq!(session.project.groups[0].y, 18);
        assert_eq!(session.revision, 2);
    }

    #[test]
    fn group_upsert_no_op_does_not_change_revision() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Group Upsert Noop", 176, 166, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "a",
                    "type": "slot",
                    "x": 8,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "b",
                    "type": "slot",
                    "x": 26,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            project.groups.push(crate::project::Group {
                id: "machine".to_string(),
                x: 8,
                y: 18,
                elements: vec!["a".to_string(), "b".to_string()],
                visible: None,
                state_owned: Vec::new(),
            });
            sessions.create_session(project)
        };
        let before = mutation_snapshot_for(&state, &project_id);

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "group-upsert-noop",
                "method": "tools/call",
                "params": {
                    "name": "group_upsert",
                    "arguments": {
                        "project_id": project_id,
                        "group_id": "machine",
                        "element_ids": ["a", "b"]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        assert_eq!(mutation_snapshot_for(&state, &project_id), before);
    }

    #[test]
    fn group_upsert_preserves_existing_group_position_metadata() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Group Upsert Metadata", 176, 166, ModTarget::Forge);
            for (id, x) in [("a", 8), ("b", 26), ("c", 44)] {
                project.elements.push(
                    parse_element_arg(&serde_json::json!({
                        "id": id,
                        "type": "slot",
                        "x": x,
                        "y": 18,
                        "size": 18
                    }))
                    .unwrap(),
                );
            }
            project.groups.push(crate::project::Group {
                id: "machine".to_string(),
                x: 99,
                y: 77,
                elements: vec!["a".to_string(), "b".to_string()],
                visible: None,
                state_owned: Vec::new(),
            });
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "group-upsert-metadata",
                "method": "tools/call",
                "params": {
                    "name": "group_upsert",
                    "arguments": {
                        "project_id": project_id,
                        "group_id": "machine",
                        "element_ids": ["a", "c"]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["group"]["x"], 99);
        assert_eq!(value["group"]["y"], 77);
        assert_eq!(value["group"]["elements"], serde_json::json!(["a", "c"]));

        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.groups[0].x, 99);
        assert_eq!(session.project.groups[0].y, 77);
        assert_eq!(
            session.project.groups[0].elements,
            vec!["a".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn group_upsert_removes_members_from_other_groups_and_prunes_short_groups() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Group Upsert Rehome", 176, 166, ModTarget::Forge);
            for (id, x) in [("a", 8), ("b", 26), ("c", 44), ("d", 62)] {
                project.elements.push(
                    parse_element_arg(&serde_json::json!({
                        "id": id,
                        "type": "slot",
                        "x": x,
                        "y": 18,
                        "size": 18
                    }))
                    .unwrap(),
                );
            }
            project.groups.push(crate::project::Group {
                id: "g1".to_string(),
                x: 8,
                y: 18,
                elements: vec!["a".to_string(), "b".to_string()],
                visible: None,
                state_owned: Vec::new(),
            });
            project.groups.push(crate::project::Group {
                id: "g2".to_string(),
                x: 44,
                y: 18,
                elements: vec!["c".to_string(), "d".to_string()],
                visible: None,
                state_owned: Vec::new(),
            });
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "group-upsert-rehome",
                "method": "tools/call",
                "params": {
                    "name": "group_upsert",
                    "arguments": {
                        "project_id": project_id,
                        "group_id": "g1",
                        "element_ids": ["a", "c"]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.revision, 1);
        assert_eq!(session.project.groups.len(), 1);
        assert_eq!(session.project.groups[0].id, "g1");
        assert_eq!(
            session.project.groups[0].elements,
            vec!["a".to_string(), "c".to_string()]
        );
        assert!(session.project.groups.iter().all(|group| group.id != "g2"));
        assert!(session
            .project
            .groups
            .iter()
            .filter(|group| group.id != "g1")
            .all(|group| !group.elements.contains(&"c".to_string())));
    }

    #[test]
    fn semantic_groups_schema_describes_object_array_and_enums() {
        let tools = get_tool_definitions();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "project_semantic_groups_update")
            .expect("project_semantic_groups_update tool should exist");
        let semantic_groups = &tool["inputSchema"]["properties"]["semantic_groups"];
        let description = semantic_groups["items"]["properties"]["kind"]["description"]
            .as_str()
            .unwrap();

        assert_eq!(semantic_groups["type"], "array");
        assert_eq!(semantic_groups["items"]["type"], "object");
        assert!(description.contains("fixed_slots"));
        assert!(description.contains("virtual_slot_grid"));
        assert!(description.contains("player_inventory"));
    }

    #[test]
    fn semantic_groups_schema_exposes_member_ids() {
        let tools = get_tool_definitions();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "project_semantic_groups_update")
            .unwrap();
        let properties =
            &tool["inputSchema"]["properties"]["semantic_groups"]["items"]["properties"];

        assert_eq!(properties["member_ids"]["type"], "array");
        assert_eq!(properties["member_ids"]["items"]["type"], "string");
    }

    #[test]
    fn export_props_accept_codegen_override() {
        let schema = export_props();
        let properties = schema["properties"].as_object().unwrap();

        assert!(properties.contains_key("codegen_mode"));
        assert!(properties.contains_key("generate_runtime_helpers"));
        assert!(properties.contains_key("generate_semantic_registry"));
        assert!(properties.contains_key("overwrite"));
        assert_eq!(
            properties["target"]["description"],
            mod_target_description()
        );
        assert_eq!(
            properties["codegen_mode"]["description"],
            codegen_mode_description()
        );
    }

    #[test]
    fn tool_schemas_generate_mod_target_and_codegen_descriptions() {
        let tools = get_tool_definitions();
        let project_new = tools
            .iter()
            .find(|tool| tool["name"] == "project_new")
            .unwrap();
        let settings_update = tools
            .iter()
            .find(|tool| tool["name"] == "project_export_settings_update")
            .unwrap();

        assert_eq!(
            project_new["inputSchema"]["properties"]["mod_target"]["description"],
            mod_target_description()
        );
        assert_eq!(
            settings_update["inputSchema"]["properties"]["codegen_mode"]["description"],
            codegen_mode_description()
        );
    }

    #[test]
    fn bind_mcp_listener_uses_fallback_when_preferred_port_is_busy() {
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let occupied_port = occupied.local_addr().unwrap().port();

        let listener = bind_mcp_listener(Some(occupied_port)).unwrap();

        assert_ne!(listener.local_addr().unwrap().port(), occupied_port);
    }

    #[test]
    fn project_save_as_tool_sets_project_path() {
        let state = test_state();
        let path = std::env::temp_dir()
            .join(format!(
                "gui-crafter-mcp-save-as-{}.mcgui",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Save As MCP", 176, 166, ModTarget::Forge))
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "save-as",
                "method": "tools/call",
                "params": {
                    "name": "project_save_as",
                    "arguments": {
                        "project_id": project_id,
                        "path": path,
                    }
                }
            }),
            &state,
        );
        let _ = std::fs::remove_file(&path);

        assert!(response["error"].is_null());
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(value["path"], path);
        assert_eq!(value["is_dirty"], false);
    }

    #[test]
    fn project_save_as_changes_project_mutation_snapshot() {
        let state = test_state();
        let path = std::env::temp_dir()
            .join(format!(
                "gui-crafter-mcp-save-as-snapshot-{}.mcgui",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Save As Snapshot", 176, 166, ModTarget::Forge))
        };
        let before = mutation_snapshot_for(&state, &project_id);

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "save-as-snapshot",
                "method": "tools/call",
                "params": {
                    "name": "project_save_as",
                    "arguments": {
                        "project_id": project_id,
                        "path": path,
                    }
                }
            }),
            &state,
        );
        let _ = std::fs::remove_file(&path);
        let after = mutation_snapshot_for(&state, &project_id);

        assert!(response["error"].is_null());
        assert_eq!(revision_for(&state, &project_id), 0);
        assert_ne!(before, after);
    }

    #[test]
    fn project_new_empty_template_respects_requested_dimensions() {
        let state = test_state();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "new-empty",
                "method": "tools/call",
                "params": {
                    "name": "project_new",
                    "arguments": {
                        "name": "Custom Empty",
                        "template": "empty",
                        "width": 264,
                        "height": 162,
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null());
        let value = tool_text_value(&response);
        assert_eq!(value["project"]["gui_size"]["width"], 264);
        assert_eq!(value["project"]["gui_size"]["height"], 162);

        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert_eq!(active.project.gui_size.width, 264);
        assert_eq!(active.project.gui_size.height, 162);
    }

    #[test]
    fn project_resize_changes_only_gui_size_and_preserves_elements() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Resize", 176, 166, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "outside_slot",
                    "type": "slot",
                    "x": 200,
                    "y": -12,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "resize",
                "method": "tools/call",
                "params": {
                    "name": "project_resize",
                    "arguments": {
                        "project_id": project_id,
                        "width": 264,
                        "height": 162
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["project_id"], project_id);
        assert_eq!(
            value["old_size"],
            serde_json::json!({ "width": 176, "height": 166 })
        );
        assert_eq!(
            value["new_size"],
            serde_json::json!({ "width": 264, "height": 162 })
        );

        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.gui_size.width, 264);
        assert_eq!(session.project.gui_size.height, 162);
        let element = session.project.find_element("outside_slot").unwrap();
        assert_eq!(element.x, 200);
        assert_eq!(element.y, -12);
        assert_eq!(session.revision, 1);
        assert!(session.project.is_dirty);
    }

    #[test]
    fn project_resize_no_op_does_not_change_revision() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Resize Noop", 176, 166, ModTarget::Forge))
        };
        let before = mutation_snapshot_for(&state, &project_id);

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "resize-noop",
                "method": "tools/call",
                "params": {
                    "name": "project_resize",
                    "arguments": {
                        "project_id": project_id,
                        "width": 176,
                        "height": 166
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        assert_eq!(mutation_snapshot_for(&state, &project_id), before);
        assert_eq!(revision_for(&state, &project_id), 0);
    }

    #[test]
    fn project_resize_rejects_zero_dimensions_without_mutation() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Resize Bad", 176, 166, ModTarget::Forge))
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "resize-bad",
                "method": "tools/call",
                "params": {
                    "name": "project_resize",
                    "arguments": {
                        "project_id": project_id,
                        "width": 0,
                        "height": 166
                    }
                }
            }),
            &state,
        );

        assert_eq!(
            response["error"]["message"],
            "Project dimensions must be greater than zero"
        );
        assert_eq!(revision_for(&state, &project_id), 0);
    }

    #[test]
    fn project_export_preview_tool_returns_planned_files() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Export Preview MCP",
                176,
                166,
                ModTarget::Forge,
            ))
        };
        let output_dir = std::env::temp_dir()
            .join(format!("gui-crafter-mcp-export-{}", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into_owned();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "export-preview",
                "method": "tools/call",
                "params": {
                    "name": "project_export_preview",
                    "arguments": {
                        "project_id": project_id,
                        "target": "forge",
                        "mod_id": "mcp_test",
                        "package": "net.inkyquill.mcptest",
                        "class_name": "FourInputProcessor",
                        "output_dir": output_dir,
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null());
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(value["target"], "forge");
        assert!(value["files"].as_array().unwrap().iter().any(|path| {
            path.as_str()
                .unwrap()
                .ends_with("FourInputProcessorScreen.java")
        }));
    }

    #[test]
    fn project_export_tool_accepts_state_id_and_writes_effective_layout() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("State Export MCP", 176, 166, ModTarget::Forge);
            let mut panel = schema_default_element();
            panel.id = "panel".into();
            panel.element_type = ElementType::Panel;
            panel.x = 0;
            panel.y = 0;
            panel.width = Some(40);
            panel.height = Some(20);
            project.elements.push(panel);
            project.states.push(ProjectState {
                id: "expanded".into(),
                label: "Expanded".into(),
                description: None,
                initial: true,
                export_role: Some("expanded".into()),
            });
            project
                .update_element_state_override(
                    "expanded",
                    "panel",
                    ElementStateOverridePatch {
                        x: Some(Some(96)),
                        ..Default::default()
                    },
                )
                .unwrap();
            sessions.create_session(project)
        };
        let output_dir = std::env::temp_dir().join(format!(
            "gui-crafter-mcp-state-export-{}",
            uuid::Uuid::new_v4()
        ));

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-export",
                "method": "tools/call",
                "params": {
                    "name": "project_export",
                    "arguments": {
                        "project_id": project_id,
                        "target": "forge",
                        "mod_id": "mcp_test",
                        "package": "net.inkyquill.mcptest",
                        "class_name": "StateExportGui",
                        "output_dir": output_dir,
                        "state_id": "expanded"
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        let layout_path = value["files"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|path| {
                let path = path.as_str().unwrap();
                path.ends_with("stateexportgui_layout.json")
                    .then(|| PathBuf::from(path))
            })
            .unwrap();
        let layout: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&layout_path).unwrap()).unwrap();

        assert_eq!(layout["effective_state"], "expanded");
        assert_eq!(layout["elements"][0]["x"], 96);
        assert_eq!(layout["states"][0]["id"], "expanded");
        assert_eq!(
            layout["state_overrides"]["expanded"]["elements"]["panel"]["x"],
            96
        );

        let _ = std::fs::remove_dir_all(output_dir);
    }

    #[test]
    fn project_export_rejects_non_string_state_id() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Invalid Export State",
                176,
                166,
                ModTarget::Forge,
            ))
        };
        let output_dir = std::env::temp_dir().join(format!(
            "gui-crafter-mcp-invalid-state-export-{}",
            uuid::Uuid::new_v4()
        ));

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "invalid-state-export",
                "method": "tools/call",
                "params": {
                    "name": "project_export",
                    "arguments": {
                        "project_id": project_id,
                        "target": "forge",
                        "mod_id": "mcp_test",
                        "package": "net.inkyquill.mcptest",
                        "class_name": "InvalidStateGui",
                        "output_dir": output_dir,
                        "state_id": 42
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("state_id must be a string or null"));
    }

    #[test]
    fn project_export_preview_rejects_non_string_state_id() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Invalid Preview State",
                176,
                166,
                ModTarget::Forge,
            ))
        };
        let output_dir = std::env::temp_dir().join(format!(
            "gui-crafter-mcp-invalid-state-preview-{}",
            uuid::Uuid::new_v4()
        ));

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "invalid-state-preview",
                "method": "tools/call",
                "params": {
                    "name": "project_export_preview",
                    "arguments": {
                        "project_id": project_id,
                        "target": "forge",
                        "mod_id": "mcp_test",
                        "package": "net.inkyquill.mcptest",
                        "class_name": "InvalidPreviewStateGui",
                        "output_dir": output_dir,
                        "state_id": 42
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("state_id must be a string or null"));
    }

    #[test]
    fn asset_import_accepts_explicit_name_and_returns_compact_metadata() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Asset Import", 176, 166, ModTarget::Forge))
        };
        let png = crate::texture::generated_gui_panel(32, 24).unwrap();
        let path = std::env::temp_dir()
            .join(format!(
                "gui-crafter-mcp-asset-import-{}.png",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned();
        std::fs::write(&path, &png).unwrap();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "asset-import",
                "method": "tools/call",
                "params": {
                    "name": "asset_import",
                    "arguments": {
                        "project_id": project_id,
                        "file_path": path,
                        "name": "textures/generated/custom_panel.png",
                    }
                }
            }),
            &state,
        );
        let _ = std::fs::remove_file(&path);

        assert!(response["error"].is_null());
        let value = tool_text_value(&response);
        assert_eq!(value["name"], "textures/generated/custom_panel.png");
        assert_eq!(value["width"], 32);
        assert_eq!(value["height"], 24);
        assert!(value["bytes"].as_u64().unwrap() > 0);
        assert_eq!(value["sha256"].as_str().unwrap().len(), 64);
        assert!(!value.as_object().unwrap().contains_key("data_url"));

        let sessions = state.sessions.lock().unwrap();
        let project = &sessions.resolve(Some(&project_id)).unwrap().project;
        assert_eq!(
            project
                .texture_data
                .get("textures/generated/custom_panel.png"),
            Some(&png)
        );
    }

    #[test]
    fn asset_metadata_update_sets_nine_slice_metadata() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Asset Metadata", 176, 166, ModTarget::Forge);
            project
                .assets
                .push("textures/gui/panel_atlas.png".to_string());
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "asset-metadata",
                "method": "tools/call",
                "params": {
                    "name": "asset_metadata_update",
                    "arguments": {
                        "project_id": project_id,
                        "name": "textures/gui/panel_atlas.png",
                        "metadata": {
                            "nine_slice": {
                                "left": 4,
                                "right": 4,
                                "top": 4,
                                "bottom": 4,
                                "edge_mode": "tile",
                                "center_mode": "tile"
                            }
                        }
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["project_id"], project_id);
        assert_eq!(value["name"], "textures/gui/panel_atlas.png");
        assert_eq!(value["metadata"]["nine_slice"]["left"], 4);
    }

    #[test]
    fn asset_metadata_update_missing_asset_preserves_redo_snapshot() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let project_id = sessions.create_session(Project::new(
                "Asset Metadata Missing",
                176,
                166,
                ModTarget::Forge,
            ));
            sessions.record_history(Some(&project_id)).unwrap();
            sessions
                .resolve_mut(Some(&project_id))
                .unwrap()
                .project
                .assets
                .push("textures/gui/panel_atlas.png".to_string());
            sessions.mark_changed(Some(&project_id)).unwrap();
            sessions.undo(Some(&project_id)).unwrap();
            project_id
        };
        let before_revision = {
            let sessions = state.sessions.lock().unwrap();
            sessions.resolve(Some(&project_id)).unwrap().revision
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "asset-metadata-missing",
                "method": "tools/call",
                "params": {
                    "name": "asset_metadata_update",
                    "arguments": {
                        "project_id": project_id,
                        "name": "textures/gui/missing.png",
                        "metadata": {
                            "width": 16,
                            "height": 16
                        }
                    }
                }
            }),
            &state,
        );

        assert_eq!(
            response["error"]["message"],
            "Asset not found: textures/gui/missing.png"
        );
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert_eq!(session.revision, before_revision);
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn asset_metadata_update_noop_preserves_revision_and_redo() {
        let state = test_state();
        let metadata = AssetMetadata {
            width: Some(16),
            height: Some(16),
            nine_slice: None,
        };
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Asset Metadata Noop", 176, 166, ModTarget::Forge);
            project
                .assets
                .push("textures/gui/panel_atlas.png".to_string());
            let project_id = sessions.create_session(project);
            drop(sessions);

            let response = response_for(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "asset-metadata-seed",
                    "method": "tools/call",
                    "params": {
                        "name": "asset_metadata_update",
                        "arguments": {
                            "project_id": project_id,
                            "name": "textures/gui/panel_atlas.png",
                            "metadata": {
                                "width": 16,
                                "height": 16
                            }
                        }
                    }
                }),
                &state,
            );
            assert!(response["error"].is_null(), "{response:#}");

            let mut sessions = state.sessions.lock().unwrap();
            sessions.undo(Some(&project_id)).unwrap();
            sessions
                .resolve_mut(Some(&project_id))
                .unwrap()
                .project
                .asset_metadata
                .insert("textures/gui/panel_atlas.png".to_string(), metadata.clone());
            project_id
        };
        let before_revision = {
            let sessions = state.sessions.lock().unwrap();
            sessions.resolve(Some(&project_id)).unwrap().revision
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "asset-metadata-noop",
                "method": "tools/call",
                "params": {
                    "name": "asset_metadata_update",
                    "arguments": {
                        "project_id": project_id,
                        "name": "textures/gui/panel_atlas.png",
                        "metadata": {
                            "width": 16,
                            "height": 16
                        }
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["metadata"]["width"], 16);
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert_eq!(session.revision, before_revision);
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn asset_remove_removes_metadata_only_entries() {
        let state = test_state();
        let asset_name = "textures/gui/stale_panel.png";
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Asset Metadata Remove", 176, 166, ModTarget::Forge);
            project.asset_metadata.insert(
                asset_name.to_string(),
                AssetMetadata {
                    width: Some(16),
                    height: Some(16),
                    nine_slice: None,
                },
            );
            sessions.create_session(project)
        };
        let before_revision = {
            let sessions = state.sessions.lock().unwrap();
            sessions.resolve(Some(&project_id)).unwrap().revision
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "asset-remove-metadata",
                "method": "tools/call",
                "params": {
                    "name": "asset_remove",
                    "arguments": {
                        "project_id": project_id,
                        "name": asset_name,
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["removed"], true);

        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert!(session.project.asset_metadata.get(asset_name).is_none());
        assert!(session.revision > before_revision);
    }

    #[test]
    fn asset_import_rejects_explicit_names_that_cannot_round_trip() {
        let state = test_state();
        let png = crate::texture::generated_gui_panel(16, 16).unwrap();
        let path = std::env::temp_dir()
            .join(format!(
                "gui-crafter-mcp-invalid-asset-import-{}.png",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned();
        std::fs::write(&path, &png).unwrap();

        for name in [
            "custom.png",
            "textures/custom",
            "../textures/custom.png",
            "/tmp/custom.png",
            "textures/",
            "textures\\custom.png",
        ] {
            let project_id = {
                let mut sessions = state.sessions.lock().unwrap();
                sessions.create_session(Project::new(
                    "Invalid Asset Import",
                    176,
                    166,
                    ModTarget::Forge,
                ))
            };
            let response = response_for(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": format!("asset-import-{name}"),
                    "method": "tools/call",
                    "params": {
                        "name": "asset_import",
                        "arguments": {
                            "project_id": project_id,
                            "file_path": path,
                            "name": name,
                        }
                    }
                }),
                &state,
            );

            assert!(
                !response["error"].is_null(),
                "expected asset name {name:?} to be rejected"
            );
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn asset_import_with_explicit_name_survives_save_and_reopen() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Asset Round Trip", 176, 166, ModTarget::Forge))
        };
        let asset_name = "textures/generated/custom_panel.png";
        let png = crate::texture::generated_gui_panel(32, 24).unwrap();
        let asset_path = std::env::temp_dir()
            .join(format!(
                "gui-crafter-mcp-round-trip-asset-{}.png",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned();
        let project_path = std::env::temp_dir()
            .join(format!(
                "gui-crafter-mcp-round-trip-project-{}.mcgui",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned();
        std::fs::write(&asset_path, &png).unwrap();

        let import_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "asset-import-round-trip",
                "method": "tools/call",
                "params": {
                    "name": "asset_import",
                    "arguments": {
                        "project_id": project_id,
                        "file_path": asset_path,
                        "name": asset_name,
                    }
                }
            }),
            &state,
        );
        assert!(import_response["error"].is_null());

        let save_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "asset-import-save",
                "method": "tools/call",
                "params": {
                    "name": "project_save_as",
                    "arguments": {
                        "project_id": project_id,
                        "path": project_path,
                    }
                }
            }),
            &state,
        );
        assert!(save_response["error"].is_null());

        let loaded = crate::format::load_from_mcgui(&project_path).unwrap();
        let _ = std::fs::remove_file(&asset_path);
        let _ = std::fs::remove_file(&project_path);

        assert_eq!(loaded.texture_data.get(asset_name), Some(&png));
    }

    #[test]
    fn asset_list_is_compact_and_element_list_includes_default_layer() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let project_id = sessions.create_session(Project::new(
                "Asset List Compact",
                176,
                166,
                ModTarget::Forge,
            ));
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            let asset_name = "textures/generated/gui_panel.png";
            project.assets.push(asset_name.to_string());
            project.texture_data.insert(
                asset_name.to_string(),
                crate::texture::generated_gui_panel(16, 16).unwrap(),
            );
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "slot_without_layer",
                    "type": "slot",
                    "x": 8,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            project_id
        };

        let asset_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "asset-list",
                "method": "tools/call",
                "params": {
                    "name": "asset_list",
                    "arguments": {
                        "project_id": project_id,
                    }
                }
            }),
            &state,
        );
        let element_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "element-list",
                "method": "tools/call",
                "params": {
                    "name": "element_list",
                    "arguments": {
                        "project_id": project_id,
                    }
                }
            }),
            &state,
        );

        assert!(asset_response["error"].is_null());
        let asset_value = tool_text_value(&asset_response);
        let asset = &asset_value["assets"].as_array().unwrap()[0];
        assert_eq!(asset["name"], "textures/generated/gui_panel.png");
        assert_eq!(asset["sha256"].as_str().unwrap().len(), 64);
        assert!(!asset.as_object().unwrap().contains_key("data_url"));

        assert!(element_response["error"].is_null());
        let element_value = tool_text_value(&element_response);
        let element = &element_value["elements"].as_array().unwrap()[0];
        assert_eq!(element["layer"], "background");
    }

    #[test]
    fn tools_call_mutates_live_active_session() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Live", 176, 166, ModTarget::Forge));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "element_add",
                    "arguments": {
                        "id": "slot_1",
                        "type": "slot",
                        "x": 8,
                        "y": 18,
                        "size": 18
                    }
                }
            }),
            &state,
        );

        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert!(response.get("error").is_none());
        assert_eq!(active.project.elements.len(), 1);
        assert_eq!(active.project.elements[0].id, "slot_1");
        assert_eq!(active.revision, 1);
        assert!(active.project.is_dirty);
    }

    #[test]
    fn attached_region_add_and_move_with_elements_mutate_live_session() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Attached Regions", 176, 166, ModTarget::Forge))
        };

        let add_region_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "attached-region-add",
                "method": "tools/call",
                "params": {
                    "name": "attached_region_add",
                    "arguments": {
                        "project_id": project_id,
                        "id": "returns_pocket",
                        "anchor": "right",
                        "x": 100,
                        "y": 18,
                        "width": 54,
                        "height": 72,
                        "state": "static",
                        "kind": "returns_pocket",
                        "semantic_group": "food_returns"
                    }
                }
            }),
            &state,
        );
        assert!(add_region_response["error"].is_null());

        let add_element_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "attached-region-child-add",
                "method": "tools/call",
                "params": {
                    "name": "element_add",
                    "arguments": {
                        "project_id": project_id,
                        "id": "returns_0",
                        "type": "slot",
                        "x": 108,
                        "y": 26,
                        "size": 18,
                        "attached_region": "returns_pocket"
                    }
                }
            }),
            &state,
        );
        assert!(add_element_response["error"].is_null());

        let move_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "attached-region-move",
                "method": "tools/call",
                "params": {
                    "name": "attached_region_move_with_elements",
                    "arguments": {
                        "project_id": project_id,
                        "id": "returns_pocket",
                        "x": 110,
                        "y": 28
                    }
                }
            }),
            &state,
        );
        assert!(move_response["error"].is_null());

        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        let region = active
            .project
            .find_attached_region("returns_pocket")
            .unwrap();
        let child = active.project.find_element("returns_0").unwrap();
        assert_eq!(region.x, 110);
        assert_eq!(child.x, 118);
    }

    #[test]
    fn attached_region_list_does_not_change_revision() {
        let state = test_state();
        let project_id = attached_region_project(&state);

        let response = attached_region_tool_call(
            &state,
            "attached_region_list",
            &project_id,
            serde_json::json!({}),
        );

        assert!(response["error"].is_null());
        assert_eq!(revision_for(&state, &project_id), 0);
    }

    #[test]
    fn attached_region_no_op_update_and_move_do_not_change_revision() {
        let state = test_state();
        let project_id = attached_region_project(&state);
        let before = mutation_snapshot_for(&state, &project_id);

        let update_response = attached_region_tool_call(
            &state,
            "attached_region_update",
            &project_id,
            serde_json::json!({
                "id": "returns_pocket",
                "changes": {
                    "x": 100,
                    "y": 18,
                    "visible": true
                }
            }),
        );
        let move_response = attached_region_tool_call(
            &state,
            "attached_region_move_with_elements",
            &project_id,
            serde_json::json!({
                "id": "returns_pocket",
                "x": 100,
                "y": 18
            }),
        );

        assert!(update_response["error"].is_null());
        assert!(move_response["error"].is_null());
        assert_eq!(revision_for(&state, &project_id), 0);
        assert_eq!(mutation_snapshot_for(&state, &project_id), before);
    }

    #[test]
    fn attached_region_missing_remove_returns_false_without_revision_change() {
        let state = test_state();
        let project_id = attached_region_project(&state);
        let before = mutation_snapshot_for(&state, &project_id);

        let response = attached_region_tool_call(
            &state,
            "attached_region_remove",
            &project_id,
            serde_json::json!({ "id": "missing_region" }),
        );

        assert!(response["error"].is_null());
        let value = tool_text_value(&response);
        assert_eq!(value["removed"], false);
        assert_eq!(revision_for(&state, &project_id), 0);
        assert_eq!(mutation_snapshot_for(&state, &project_id), before);
    }

    #[test]
    fn attached_region_invalid_updates_return_errors_without_mutation() {
        let state = test_state();
        let project_id = attached_region_project(&state);

        let id_change_response = attached_region_tool_call(
            &state,
            "attached_region_update",
            &project_id,
            serde_json::json!({
                "id": "returns_pocket",
                "changes": { "id": "other_region" }
            }),
        );
        let null_required_response = attached_region_tool_call(
            &state,
            "attached_region_update",
            &project_id,
            serde_json::json!({
                "id": "returns_pocket",
                "changes": { "width": null }
            }),
        );
        let bad_enum_response = attached_region_tool_call(
            &state,
            "attached_region_update",
            &project_id,
            serde_json::json!({
                "id": "returns_pocket",
                "changes": { "state": "animated" }
            }),
        );

        assert_eq!(
            id_change_response["error"]["message"],
            "Attached region id cannot be changed"
        );
        assert!(null_required_response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Invalid attached region update"));
        assert!(bad_enum_response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Invalid attached region update"));
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.resolve(Some(&project_id)).unwrap();
        let region = active
            .project
            .find_attached_region("returns_pocket")
            .unwrap();
        assert_eq!(region.width, 54);
        assert_eq!(region.state, crate::project::AttachedRegionState::Static);
        assert_eq!(active.revision, 0);
    }

    #[test]
    fn attached_region_move_overflow_errors_before_mutation() {
        let state = test_state();
        let project_id = attached_region_project(&state);
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions
                .resolve_mut(Some(&project_id))
                .unwrap()
                .project
                .elements
                .push(
                    parse_element_arg(&serde_json::json!({
                        "id": "returns_0",
                        "type": "slot",
                        "x": i32::MAX,
                        "y": 26,
                        "size": 18,
                        "attached_region": "returns_pocket"
                    }))
                    .unwrap(),
                );
        }

        let response = attached_region_tool_call(
            &state,
            "attached_region_move_with_elements",
            &project_id,
            serde_json::json!({
                "id": "returns_pocket",
                "x": 101,
                "y": 18
            }),
        );

        assert_eq!(
            response["error"]["message"],
            "Attached region child move overflow"
        );
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.resolve(Some(&project_id)).unwrap();
        let region = active
            .project
            .find_attached_region("returns_pocket")
            .unwrap();
        let child = active.project.find_element("returns_0").unwrap();
        assert_eq!(region.x, 100);
        assert_eq!(child.x, i32::MAX);
        assert_eq!(active.revision, 0);
    }

    #[test]
    fn attached_region_move_refreshes_group_position_for_attached_children() {
        let state = test_state();
        let project_id = attached_region_project(&state);
        {
            let mut sessions = state.sessions.lock().unwrap();
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "returns_0",
                    "type": "slot",
                    "x": 108,
                    "y": 26,
                    "size": 18,
                    "attached_region": "returns_pocket"
                }))
                .unwrap(),
            );
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "returns_1",
                    "type": "slot",
                    "x": 126,
                    "y": 26,
                    "size": 18,
                    "attached_region": "returns_pocket"
                }))
                .unwrap(),
            );
            project.groups.push(crate::project::Group {
                id: "returns_group".to_string(),
                x: 108,
                y: 26,
                elements: vec!["returns_0".to_string(), "returns_1".to_string()],
                visible: None,
                state_owned: Vec::new(),
            });
        }

        let response = attached_region_tool_call(
            &state,
            "attached_region_move_with_elements",
            &project_id,
            serde_json::json!({
                "id": "returns_pocket",
                "x": 110,
                "y": 30
            }),
        );

        assert!(response["error"].is_null());
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.resolve(Some(&project_id)).unwrap();
        let group = active
            .project
            .groups
            .iter()
            .find(|group| group.id == "returns_group")
            .unwrap();
        assert_eq!(group.x, 118);
        assert_eq!(group.y, 38);
        assert_eq!(active.revision, 1);
    }

    #[test]
    fn element_add_many_is_atomic_for_duplicate_ids() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Bulk Add", 176, 166, ModTarget::Forge));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "element-add-many",
                "method": "tools/call",
                "params": {
                    "name": "element_add_many",
                    "arguments": {
                        "elements": [
                            {
                                "id": "slot_a",
                                "type": "slot",
                                "x": 8,
                                "y": 18,
                                "size": 18
                            },
                            {
                                "id": "slot_a",
                                "type": "slot",
                                "x": 26,
                                "y": 18,
                                "size": 18
                            }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Duplicate element id"));
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert!(active.project.elements.is_empty());
        assert_eq!(active.revision, 0);
    }

    #[test]
    fn element_add_many_rejects_existing_ids_before_mutating() {
        let state = test_state();
        let existing = parse_element_arg(&serde_json::json!({
            "id": "slot_existing",
            "type": "slot",
            "x": 8,
            "y": 18,
            "size": 18
        }))
        .unwrap();
        let original_elements = vec![existing.clone()];
        {
            let mut sessions = state.sessions.lock().unwrap();
            let project_id =
                sessions.create_session(Project::new("Bulk Existing", 176, 166, ModTarget::Forge));
            sessions
                .resolve_mut(Some(&project_id))
                .unwrap()
                .project
                .elements = original_elements.clone();
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "element-add-many-existing",
                "method": "tools/call",
                "params": {
                    "name": "element_add_many",
                    "arguments": {
                        "elements": [
                            {
                                "id": "slot_existing",
                                "type": "slot",
                                "x": 26,
                                "y": 18,
                                "size": 18
                            }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Element already exists: slot_existing"));
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert_eq!(active.project.elements, original_elements);
        assert_eq!(active.revision, 0);
    }

    #[test]
    fn element_add_many_rejects_empty_array_without_history() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Bulk Empty", 176, 166, ModTarget::Forge));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "element-add-many-empty",
                "method": "tools/call",
                "params": {
                    "name": "element_add_many",
                    "arguments": {
                        "elements": []
                    }
                }
            }),
            &state,
        );

        assert_eq!(
            response["error"]["message"],
            "elements array cannot be empty"
        );
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert!(active.project.elements.is_empty());
        assert_eq!(active.revision, 0);
    }

    #[test]
    fn element_add_many_response_includes_effective_layer() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Bulk Effective Layer",
                176,
                166,
                ModTarget::Forge,
            ));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "bulk",
                "method": "tools/call",
                "params": {
                    "name": "element_add_many",
                    "arguments": {
                        "elements": [
                            { "id": "slot_a", "type": "slot", "x": 8, "y": 8, "size": 18 }
                        ]
                    }
                }
            }),
            &state,
        );

        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(value["elements"][0]["layer"], "background");
    }

    #[test]
    fn element_update_many_updates_multiple_elements_in_one_revision() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Many", 176, 166, ModTarget::Forge);
            for (id, x) in [("a", 8), ("b", 26)] {
                project.elements.push(
                    parse_element_arg(&serde_json::json!({
                        "id": id,
                        "type": "slot",
                        "x": x,
                        "y": 18,
                        "size": 18
                    }))
                    .unwrap(),
                );
            }
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "updates": [
                            { "id": "a", "changes": { "x": 10, "y": 20 } },
                            { "id": "b", "changes": { "x": 30, "y": 40, "slot_index": 7 } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["project_id"], project_id);
        assert_eq!(value["updated_count"], 2);
        assert_eq!(value["results"].as_array().unwrap().len(), 2);

        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.find_element("a").unwrap().x, 10);
        assert_eq!(
            session.project.find_element("b").unwrap().slot_index,
            Some(7)
        );
        assert_eq!(session.revision, 1);
    }

    #[test]
    fn element_update_many_counts_only_changed_elements() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Many Count", 176, 166, ModTarget::Forge);
            for (id, x) in [("a", 8), ("b", 26)] {
                project.elements.push(
                    parse_element_arg(&serde_json::json!({
                        "id": id,
                        "type": "slot",
                        "x": x,
                        "y": 18,
                        "size": 18
                    }))
                    .unwrap(),
                );
            }
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many-count",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "updates": [
                            { "id": "a", "changes": { "x": 8, "y": 18 } },
                            { "id": "b", "changes": { "x": 30, "y": 40 } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["updated_count"], 1);
        assert_eq!(value["results"].as_array().unwrap().len(), 2);

        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.find_element("a").unwrap().x, 8);
        assert_eq!(session.project.find_element("b").unwrap().x, 30);
        assert_eq!(session.revision, 1);
    }

    #[test]
    fn element_update_refreshes_group_position_after_coordinate_change() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Group Position", 176, 166, ModTarget::Forge);
            for (id, x) in [("a", 8), ("b", 26)] {
                project.elements.push(
                    parse_element_arg(&serde_json::json!({
                        "id": id,
                        "type": "slot",
                        "x": x,
                        "y": 18,
                        "size": 18
                    }))
                    .unwrap(),
                );
            }
            project.groups.push(crate::project::Group {
                id: "machine".to_string(),
                x: 8,
                y: 18,
                elements: vec!["a".to_string(), "b".to_string()],
                visible: None,
                state_owned: Vec::new(),
            });
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-group-position",
                "method": "tools/call",
                "params": {
                    "name": "element_update",
                    "arguments": {
                        "project_id": project_id,
                        "id": "a",
                        "changes": { "x": 12, "y": 24 }
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(
            (session.project.groups[0].x, session.project.groups[0].y),
            (12, 18)
        );
        assert_eq!(session.revision, 1);
    }

    #[test]
    fn element_update_many_refreshes_group_position_after_coordinate_change() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project =
                Project::new("Update Many Group Position", 176, 166, ModTarget::Forge);
            for (id, x) in [("a", 8), ("b", 26)] {
                project.elements.push(
                    parse_element_arg(&serde_json::json!({
                        "id": id,
                        "type": "slot",
                        "x": x,
                        "y": 18,
                        "size": 18
                    }))
                    .unwrap(),
                );
            }
            project.groups.push(crate::project::Group {
                id: "machine".to_string(),
                x: 8,
                y: 18,
                elements: vec!["a".to_string(), "b".to_string()],
                visible: None,
                state_owned: Vec::new(),
            });
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many-group-position",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "updates": [
                            { "id": "a", "changes": { "x": 12, "y": 24 } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(
            (session.project.groups[0].x, session.project.groups[0].y),
            (12, 18)
        );
        assert_eq!(session.revision, 1);
    }

    #[test]
    fn element_update_many_strict_failure_is_atomic() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Many Atomic", 176, 166, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "a",
                    "type": "slot",
                    "x": 8,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many-bad",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "updates": [
                            { "id": "a", "changes": { "x": 10 } },
                            { "id": "missing", "changes": { "x": 30 } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Element not found: missing"));
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.find_element("a").unwrap().x, 8);
        assert_eq!(session.revision, 0);
    }

    #[test]
    fn element_update_many_no_op_does_not_change_revision() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Many Noop", 176, 166, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "a",
                    "type": "slot",
                    "x": 8,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };
        let before = mutation_snapshot_for(&state, &project_id);

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many-noop",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "updates": [
                            { "id": "a", "changes": { "x": 8, "y": 18 } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        assert_eq!(mutation_snapshot_for(&state, &project_id), before);
    }

    #[test]
    fn element_update_many_rejects_duplicate_ids_without_mutation() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Many Duplicate", 176, 166, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "a",
                    "type": "slot",
                    "x": 8,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many-duplicate",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "updates": [
                            { "id": "a", "changes": { "x": 10 } },
                            { "id": "a", "changes": { "y": 20 } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Duplicate element update id: a"));
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.find_element("a").unwrap().x, 8);
        assert_eq!(session.revision, 0);
    }

    #[test]
    fn project_screenshot_writes_compact_png_metadata() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Screenshot", 64, 32, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "slot_a",
                    "type": "slot",
                    "x": 8,
                    "y": 8,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };
        let output_path = TempPath::new("mc-gui-crafter-test", "png");
        let output_path_string = output_path.path_string();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "screenshot",
                "method": "tools/call",
                "params": {
                    "name": "project_screenshot",
                    "arguments": {
                        "project_id": project_id,
                        "output_path": output_path_string,
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(value["width"], 64);
        assert_eq!(value["height"], 32);
        assert!(value["bytes"].as_u64().unwrap() > 0);
        assert_eq!(value["sha256"].as_str().unwrap().len(), 64);
        assert!(value.get("data_url").is_none());
        assert!(output_path.path().exists());
        assert_eq!(value["path"], output_path.path_string());
    }

    #[test]
    fn project_render_writes_compact_png_metadata() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Render", 64, 32, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "slot_a",
                    "type": "slot",
                    "x": 8,
                    "y": 8,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };
        let output_path = TempPath::new("mc-gui-crafter-render-test", "png");
        let output_path_string = output_path.path_string();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "render",
                "method": "tools/call",
                "params": {
                    "name": "project_render",
                    "arguments": {
                        "project_id": project_id,
                        "output_path": output_path_string
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["project_id"], project_id);
        assert_eq!(value["width"], 64);
        assert_eq!(value["height"], 32);
        assert!(value["bytes"].as_u64().unwrap() > 0);
        assert_eq!(value["sha256"].as_str().unwrap().len(), 64);
        assert!(value.get("data_url").is_none());
        assert_eq!(value["path"], output_path.path_string());
        assert!(output_path.path().exists());
    }

    #[test]
    fn state_override_update_changes_effective_render_not_base() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("State Render", 96, 48, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "panel",
                    "type": "slot",
                    "x": 0,
                    "y": 0,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };

        let add_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-add",
                "method": "tools/call",
                "params": {
                    "name": "state_add",
                    "arguments": {
                        "project_id": project_id,
                        "id": "expanded",
                        "label": "Expanded",
                        "initial": true,
                        "export_role": "expanded"
                    }
                }
            }),
            &state,
        );
        assert!(add_response["error"].is_null(), "{add_response:#}");

        let update_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-override",
                "method": "tools/call",
                "params": {
                    "name": "state_override_update",
                    "arguments": {
                        "project_id": project_id,
                        "state_id": "expanded",
                        "target_type": "element",
                        "target_id": "panel",
                        "fields": { "x": 64, "visible": true }
                    }
                }
            }),
            &state,
        );
        assert!(update_response["error"].is_null(), "{update_response:#}");

        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.find_element("panel").unwrap().x, 0);
        assert_eq!(
            session
                .project
                .effective_for_state(Some("expanded"))
                .unwrap()
                .find_element("panel")
                .unwrap()
                .x,
            64
        );
    }

    #[test]
    fn element_update_with_state_id_writes_override_and_rejects_base_only_fields() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("State Element Update", 96, 48, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "panel",
                    "type": "slot",
                    "x": 0,
                    "y": 0,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };
        let add_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-add-element-update",
                "method": "tools/call",
                "params": {
                    "name": "state_add",
                    "arguments": {
                        "project_id": project_id,
                        "id": "expanded",
                        "label": "Expanded"
                    }
                }
            }),
            &state,
        );
        assert!(add_response["error"].is_null(), "{add_response:#}");

        let update_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "element-update-state",
                "method": "tools/call",
                "params": {
                    "name": "element_update",
                    "arguments": {
                        "project_id": project_id,
                        "id": "panel",
                        "state_id": "expanded",
                        "changes": { "x": 24, "visible": false }
                    }
                }
            }),
            &state,
        );
        assert!(update_response["error"].is_null(), "{update_response:#}");

        let rejected_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "element-update-state-reject",
                "method": "tools/call",
                "params": {
                    "name": "element_update",
                    "arguments": {
                        "project_id": project_id,
                        "id": "panel",
                        "state_id": "expanded",
                        "changes": { "content": "base-only" }
                    }
                }
            }),
            &state,
        );
        assert!(rejected_response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unsupported element state override field(s): content"));

        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.find_element("panel").unwrap().x, 0);
        assert_eq!(
            session.project.state_overrides["expanded"].elements["panel"].x,
            Some(24)
        );
    }

    #[test]
    fn element_update_with_state_id_can_detach_from_attached_region() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("State Element Detach", 96, 48, ModTarget::Forge);
            let mut region = schema_default_attached_region();
            region.id = "attached_panel".into();
            project.attached_regions.push(region);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "panel",
                    "type": "slot",
                    "x": 0,
                    "y": 0,
                    "size": 18,
                    "attached_region": "attached_panel"
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };
        let add_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-add-element-detach",
                "method": "tools/call",
                "params": {
                    "name": "state_add",
                    "arguments": {
                        "project_id": project_id,
                        "id": "collapsed",
                        "label": "Collapsed"
                    }
                }
            }),
            &state,
        );
        assert!(add_response["error"].is_null(), "{add_response:#}");

        let update_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "element-update-state-detach",
                "method": "tools/call",
                "params": {
                    "name": "element_update",
                    "arguments": {
                        "project_id": project_id,
                        "id": "panel",
                        "state_id": "collapsed",
                        "changes": { "attached_region": null }
                    }
                }
            }),
            &state,
        );
        assert!(update_response["error"].is_null(), "{update_response:#}");

        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(
            session
                .project
                .find_element("panel")
                .unwrap()
                .attached_region
                .as_deref(),
            Some("attached_panel")
        );
        assert_eq!(
            session.project.state_overrides["collapsed"].elements["panel"].attached_region,
            ElementAttachedRegionStateOverride::Detached
        );
        assert_eq!(
            session
                .project
                .effective_for_state(Some("collapsed"))
                .unwrap()
                .find_element("panel")
                .unwrap()
                .attached_region,
            None
        );
    }

    #[test]
    fn state_set_active_clears_edit_scope_when_state_id_is_null() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("State Scope", 96, 48, ModTarget::Forge);
            project.states.push(ProjectState {
                id: "expanded".into(),
                label: "Expanded".into(),
                description: None,
                initial: true,
                export_role: None,
            });
            sessions.create_session(project)
        };

        let set_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-set-active",
                "method": "tools/call",
                "params": {
                    "name": "state_set_active",
                    "arguments": {
                        "project_id": project_id,
                        "state_id": "expanded",
                        "edit_scope": "state"
                    }
                }
            }),
            &state,
        );
        assert!(set_response["error"].is_null(), "{set_response:#}");

        let clear_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-clear-active",
                "method": "tools/call",
                "params": {
                    "name": "state_set_active",
                    "arguments": {
                        "project_id": project_id,
                        "state_id": null,
                        "edit_scope": "state"
                    }
                }
            }),
            &state,
        );
        assert!(clear_response["error"].is_null(), "{clear_response:#}");
        let value = tool_text_value(&clear_response);
        assert!(value["active_state_id"].is_null());
        assert_eq!(value["edit_scope"], "base");
    }

    #[test]
    fn element_update_many_with_state_id_writes_state_overrides() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Many State", 176, 166, ModTarget::Forge);
            project.states.push(ProjectState {
                id: "expanded".into(),
                label: "Expanded".into(),
                description: None,
                initial: true,
                export_role: None,
            });
            for (id, x) in [("a", 8), ("b", 26)] {
                project.elements.push(
                    parse_element_arg(&serde_json::json!({
                        "id": id,
                        "type": "slot",
                        "x": x,
                        "y": 18,
                        "size": 18
                    }))
                    .unwrap(),
                );
            }
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many-state",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "state_id": "expanded",
                        "updates": [
                            { "id": "a", "changes": { "x": 10, "visible": false } },
                            { "id": "b", "changes": { "x": 30, "layer": "overlay" } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["updated_count"], 2);
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(session.project.find_element("a").unwrap().x, 8);
        assert_eq!(session.project.find_element("b").unwrap().x, 26);
        assert_eq!(
            session.project.state_overrides["expanded"].elements["a"].x,
            Some(10)
        );
        assert_eq!(
            session.project.state_overrides["expanded"].elements["a"].visible,
            Some(false)
        );
        assert_eq!(
            session.project.state_overrides["expanded"].elements["b"].x,
            Some(30)
        );
        assert_eq!(session.revision, 1);
    }

    #[test]
    fn element_update_many_state_scope_rejects_unsupported_fields_without_mutation() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Many State Reject", 176, 166, ModTarget::Forge);
            project.states.push(ProjectState {
                id: "expanded".into(),
                label: "Expanded".into(),
                description: None,
                initial: true,
                export_role: None,
            });
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "a",
                    "type": "slot",
                    "x": 8,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many-state-unsupported",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "state_id": "expanded",
                        "updates": [
                            { "id": "a", "changes": { "content": "base-only" } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unsupported element state override field(s): content"));
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert!(session.project.state_overrides.is_empty());
        assert_eq!(session.revision, 0);
    }

    #[test]
    fn element_update_many_state_scope_failure_is_atomic() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Many State Atomic", 176, 166, ModTarget::Forge);
            project.states.push(ProjectState {
                id: "expanded".into(),
                label: "Expanded".into(),
                description: None,
                initial: true,
                export_role: None,
            });
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "a",
                    "type": "slot",
                    "x": 8,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many-state-atomic",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "state_id": "expanded",
                        "updates": [
                            { "id": "a", "changes": { "x": 10 } },
                            { "id": "missing", "changes": { "x": 30 } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown element 'missing'"));
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert!(session.project.state_overrides.is_empty());
        assert_eq!(session.project.find_element("a").unwrap().x, 8);
        assert_eq!(session.revision, 0);
    }

    #[test]
    fn element_update_many_rejects_mixed_base_and_state_scopes_without_mutation() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Update Many Mixed Scope", 176, 166, ModTarget::Forge);
            project.states.push(ProjectState {
                id: "expanded".into(),
                label: "Expanded".into(),
                description: None,
                initial: true,
                export_role: None,
            });
            for (id, x) in [("a", 8), ("b", 26)] {
                project.elements.push(
                    parse_element_arg(&serde_json::json!({
                        "id": id,
                        "type": "slot",
                        "x": x,
                        "y": 18,
                        "size": 18
                    }))
                    .unwrap(),
                );
            }
            sessions.create_session(project)
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "update-many-mixed-scope",
                "method": "tools/call",
                "params": {
                    "name": "element_update_many",
                    "arguments": {
                        "project_id": project_id,
                        "updates": [
                            { "id": "a", "state_id": "expanded", "changes": { "x": 10 } },
                            { "id": "b", "changes": { "x": 30 } }
                        ]
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("cannot mix base updates and state-scoped updates"));
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert!(session.project.state_overrides.is_empty());
        assert_eq!(session.project.find_element("a").unwrap().x, 8);
        assert_eq!(session.project.find_element("b").unwrap().x, 26);
        assert_eq!(session.revision, 0);
    }

    #[test]
    fn project_render_accepts_state_id() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            let mut project = Project::new("Render State", 96, 48, ModTarget::Forge);
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": "panel",
                    "type": "slot",
                    "x": 0,
                    "y": 0,
                    "size": 18
                }))
                .unwrap(),
            );
            sessions.create_session(project)
        };
        let add_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-add-render",
                "method": "tools/call",
                "params": {
                    "name": "state_add",
                    "arguments": {
                        "project_id": project_id,
                        "id": "expanded",
                        "label": "Expanded"
                    }
                }
            }),
            &state,
        );
        assert!(add_response["error"].is_null(), "{add_response:#}");
        let override_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "state-override-render",
                "method": "tools/call",
                "params": {
                    "name": "state_override_update",
                    "arguments": {
                        "project_id": project_id,
                        "state_id": "expanded",
                        "target_type": "element",
                        "target_id": "panel",
                        "fields": { "x": 32 }
                    }
                }
            }),
            &state,
        );
        assert!(
            override_response["error"].is_null(),
            "{override_response:#}"
        );

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "render-state",
                "method": "tools/call",
                "params": {
                    "name": "project_render",
                    "arguments": {
                        "project_id": project_id,
                        "state_id": "expanded"
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["project_id"], project_id);
        assert_eq!(value["state_id"], "expanded");
        assert!(value["path"].as_str().unwrap().ends_with(".png"));
    }

    #[test]
    fn project_render_rejects_non_string_state_id() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Render Invalid State",
                96,
                48,
                ModTarget::Forge,
            ));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "render-invalid-state-id",
                "method": "tools/call",
                "params": {
                    "name": "project_render",
                    "arguments": {
                        "state_id": 42
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("state_id must be a string or null"));
    }

    #[test]
    fn project_screenshot_includes_data_url_only_when_requested() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Screenshot Data URL",
                32,
                24,
                ModTarget::Forge,
            ));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "screenshot",
                "method": "tools/call",
                "params": {
                    "name": "project_screenshot",
                    "arguments": { "include_data_url": true }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        let _written_path = TempPath::from_path(PathBuf::from(value["path"].as_str().unwrap()));
        assert!(value["data_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
    }

    #[test]
    fn project_screenshot_accepts_png_extension_case_insensitively() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Screenshot PNG Extension",
                32,
                24,
                ModTarget::Forge,
            ));
        }
        let output_path = TempPath::new("mc-gui-crafter-test", "PNG");
        let output_path_string = output_path.path_string();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "screenshot",
                "method": "tools/call",
                "params": {
                    "name": "project_screenshot",
                    "arguments": { "output_path": output_path_string }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null(), "{response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["path"], output_path.path_string());
        assert!(output_path.path().exists());
    }

    #[test]
    fn project_screenshot_rejects_missing_or_non_png_extension() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Screenshot Bad Extension",
                32,
                24,
                ModTarget::Forge,
            ));
        }

        for output_path in [
            TempPath::new("mc-gui-crafter-test", ""),
            TempPath::new("mc-gui-crafter-test", "jpg"),
        ] {
            let output_path_string = output_path.path_string();
            let response = response_for(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "screenshot",
                    "method": "tools/call",
                    "params": {
                        "name": "project_screenshot",
                        "arguments": { "output_path": output_path_string }
                    }
                }),
                &state,
            );

            assert!(!response["error"].is_null(), "{response:#}");
            assert!(!output_path.path().exists());
        }
    }

    #[test]
    fn slot_grid_add_creates_grouped_player_inventory_grid() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Slot Grid", 176, 166, ModTarget::Forge));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-add",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "player_inv",
                        "x": 8,
                        "y": 84,
                        "columns": 9,
                        "rows": 3,
                        "slot_role": "player_inventory",
                        "inventory_group": "player_inventory",
                        "slot_index_start": 9,
                        "group_id": "player_inventory_grid",
                        "semantic_group_kind": "player_inventory",
                        "slot_count": 27
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null());
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(value["created_count"], 27);
        assert_eq!(value["elements"].as_array().unwrap().len(), 27);
        assert_eq!(value["group"]["id"], "player_inventory_grid");
        assert_eq!(value["semantic_group"]["id"], "player_inventory");

        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert_eq!(active.project.elements.len(), 27);
        assert_eq!(active.project.elements[0].id, "player_inv_0");
        assert_eq!(active.project.elements[0].x, 8);
        assert_eq!(active.project.elements[0].y, 84);
        assert_eq!(active.project.elements[0].slot_index, Some(9));
        assert_eq!(active.project.elements[1].x, 26);
        assert_eq!(active.project.elements[9].x, 8);
        assert_eq!(active.project.elements[9].y, 102);
        assert_eq!(active.project.elements[26].slot_index, Some(35));
        assert_eq!(active.project.groups.len(), 1);
        assert_eq!(active.project.groups[0].id, "player_inventory_grid");
        assert_eq!(active.project.groups[0].elements.len(), 27);
        assert_eq!(active.project.groups[0].elements[0], "player_inv_0");
        assert_eq!(active.project.semantic_groups.len(), 1);
        assert_eq!(active.project.semantic_groups[0].id, "player_inventory");
        assert_eq!(
            active.project.semantic_groups[0].kind,
            crate::project::SemanticGroupKind::PlayerInventory
        );
        assert_eq!(active.project.semantic_groups[0].slot_count, Some(27));
    }

    #[test]
    fn slot_grid_add_uses_default_slot_geometry_and_indices() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Slot Grid Defaults",
                176,
                166,
                ModTarget::Forge,
            ));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-defaults",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "default_slot",
                        "x": 4,
                        "y": 5,
                        "columns": 2,
                        "rows": 2
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null());
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert_eq!(active.project.elements.len(), 4);
        assert_eq!(active.project.elements[0].size, Some(18));
        assert_eq!(active.project.elements[0].slot_index, Some(0));
        assert_eq!(active.project.elements[1].x, 22);
        assert_eq!(active.project.elements[1].y, 5);
        assert_eq!(active.project.elements[2].x, 4);
        assert_eq!(active.project.elements[2].y, 23);
        assert_eq!(active.project.elements[3].slot_index, Some(3));
        assert_eq!(active.revision, 1);
    }

    #[test]
    fn slot_grid_add_rejects_element_id_conflicts_before_mutating() {
        let state = test_state();
        let existing = parse_element_arg(&serde_json::json!({
            "id": "grid_0",
            "type": "slot",
            "x": 8,
            "y": 18,
            "size": 18
        }))
        .unwrap();
        let original_elements = vec![existing.clone()];
        {
            let mut sessions = state.sessions.lock().unwrap();
            let project_id = sessions.create_session(Project::new(
                "Slot Grid Element Conflict",
                176,
                166,
                ModTarget::Forge,
            ));
            sessions
                .resolve_mut(Some(&project_id))
                .unwrap()
                .project
                .elements = original_elements.clone();
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-element-conflict",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "grid",
                        "x": 8,
                        "y": 18,
                        "columns": 1,
                        "rows": 1
                    }
                }
            }),
            &state,
        );

        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Element already exists: grid_0"));
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert_eq!(active.project.elements, original_elements);
        assert!(active.project.groups.is_empty());
        assert!(active.project.semantic_groups.is_empty());
        assert_eq!(active.revision, 0);
    }

    #[test]
    fn slot_grid_add_rejects_group_id_conflicts_before_mutating() {
        let state = test_state();
        let original_groups = vec![crate::project::Group {
            id: "existing_group".to_string(),
            x: 1,
            y: 2,
            elements: vec!["other_a".to_string(), "other_b".to_string()],
            visible: None,
            state_owned: Vec::new(),
        }];
        {
            let mut sessions = state.sessions.lock().unwrap();
            let project_id = sessions.create_session(Project::new(
                "Slot Grid Group Conflict",
                176,
                166,
                ModTarget::Forge,
            ));
            sessions
                .resolve_mut(Some(&project_id))
                .unwrap()
                .project
                .groups = original_groups.clone();
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-group-conflict",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "grid",
                        "x": 8,
                        "y": 18,
                        "columns": 2,
                        "rows": 1,
                        "group_id": "existing_group"
                    }
                }
            }),
            &state,
        );

        assert_eq!(response["error"]["message"], "Group already exists");
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert!(active.project.elements.is_empty());
        assert_eq!(active.project.groups, original_groups);
        assert!(active.project.semantic_groups.is_empty());
        assert_eq!(active.revision, 0);
    }

    #[test]
    fn slot_grid_add_skips_semantic_group_without_inventory_group() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Slot Grid No Semantic Group",
                176,
                166,
                ModTarget::Forge,
            ));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-no-semantic-without-inventory",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "player_inv",
                        "x": 8,
                        "y": 84,
                        "columns": 2,
                        "rows": 1,
                        "group_id": "player_inventory_grid",
                        "semantic_group_kind": "player_inventory",
                        "slot_count": 2
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null());
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(content).unwrap();
        assert!(value["semantic_group"].is_null());
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert!(active.project.semantic_groups.is_empty());
        assert_eq!(active.project.groups.len(), 1);
        assert_eq!(active.revision, 1);
    }

    #[test]
    fn slot_grid_add_replaces_existing_semantic_group_with_same_id() {
        let state = test_state();
        let original_group = SemanticGroup {
            id: "player_inventory".to_string(),
            kind: crate::project::SemanticGroupKind::Hotbar,
            columns: Some(9),
            visible_rows: Some(1),
            total_rows: Some(1),
            slot_count: Some(9),
            member_ids: Vec::new(),
            data_source: Some("old".to_string()),
            scroll_binding: None,
            dynamic_height: false,
        };
        {
            let mut sessions = state.sessions.lock().unwrap();
            let project_id = sessions.create_session(Project::new(
                "Slot Grid Semantic Replacement",
                176,
                166,
                ModTarget::Forge,
            ));
            sessions
                .resolve_mut(Some(&project_id))
                .unwrap()
                .project
                .semantic_groups = vec![original_group];
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-semantic-replace",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "player_inv",
                        "x": 8,
                        "y": 84,
                        "columns": 2,
                        "rows": 1,
                        "inventory_group": "player_inventory",
                        "semantic_group_kind": "player_inventory",
                        "slot_count": 2
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null());
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert_eq!(active.project.semantic_groups.len(), 1);
        assert_eq!(active.project.semantic_groups[0].id, "player_inventory");
        assert_eq!(
            active.project.semantic_groups[0].kind,
            crate::project::SemanticGroupKind::PlayerInventory
        );
        assert_eq!(active.project.semantic_groups[0].columns, Some(2));
        assert_eq!(active.project.semantic_groups[0].slot_count, Some(2));
        assert_eq!(
            active.project.semantic_groups[0].data_source,
            Some("player_inventory".to_string())
        );
        assert_eq!(active.revision, 1);
    }

    #[test]
    fn slot_grid_add_rejects_out_of_range_integer_inputs_before_mutating() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Slot Grid Integer Ranges",
                176,
                166,
                ModTarget::Forge,
            ));
        }

        let x_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-x-range",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "grid",
                        "x": 2147483648i64,
                        "y": 18,
                        "columns": 1,
                        "rows": 1
                    }
                }
            }),
            &state,
        );
        let columns_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-columns-range",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "grid",
                        "x": 8,
                        "y": 18,
                        "columns": 4294967297u64,
                        "rows": 1
                    }
                }
            }),
            &state,
        );
        let slot_size_response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-slot-size-range",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "grid",
                        "x": 8,
                        "y": 18,
                        "columns": 1,
                        "rows": 1,
                        "slot_size": 4294967297u64
                    }
                }
            }),
            &state,
        );

        assert_eq!(x_response["error"]["message"], "x is out of range");
        assert_eq!(
            columns_response["error"]["message"],
            "columns is out of range"
        );
        assert_eq!(
            slot_size_response["error"]["message"],
            "slot_size is out of range"
        );
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert!(active.project.elements.is_empty());
        assert!(active.project.groups.is_empty());
        assert_eq!(active.revision, 0);
    }

    #[test]
    fn slot_grid_add_rejects_coordinate_overflow_before_mutating() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Slot Grid Coordinate Overflow",
                176,
                166,
                ModTarget::Forge,
            ));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slot-grid-coordinate-overflow",
                "method": "tools/call",
                "params": {
                    "name": "slot_grid_add",
                    "arguments": {
                        "id_prefix": "grid",
                        "x": i32::MAX,
                        "y": 18,
                        "columns": 2,
                        "rows": 1,
                        "spacing": 18
                    }
                }
            }),
            &state,
        );

        assert_eq!(
            response["error"]["message"],
            "slot grid x coordinate overflow"
        );
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert!(active.project.elements.is_empty());
        assert!(active.project.groups.is_empty());
        assert!(active.project.semantic_groups.is_empty());
        assert_eq!(active.revision, 0);
    }

    #[test]
    fn project_export_settings_update_changes_live_session() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Settings", 176, 166, ModTarget::Forge))
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "settings-update",
                "method": "tools/call",
                "params": {
                    "name": "project_export_settings_update",
                    "arguments": {
                        "project_id": project_id,
                        "codegen_mode": "modular",
                        "generate_runtime_helpers": false,
                        "generate_semantic_registry": false,
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null());
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(value["codegen_mode"], "modular");
        assert_eq!(value["generate_runtime_helpers"], false);
        assert_eq!(value["generate_semantic_registry"], false);

        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert_eq!(
            active.project.export_settings.codegen_mode,
            CodegenMode::Modular
        );
        assert_eq!(active.revision, 1);
        assert!(active.project.is_dirty);
    }

    #[test]
    fn project_export_settings_update_defaults_semantic_registry_from_codegen_when_unspecified() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new(
                "Settings Defaults",
                176,
                166,
                ModTarget::Forge,
            ))
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "settings-default",
                "method": "tools/call",
                "params": {
                    "name": "project_export_settings_update",
                    "arguments": {
                        "project_id": project_id,
                        "codegen_mode": "modular"
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null());
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(value["codegen_mode"], "modular");
        assert_eq!(value["generate_semantic_registry"], true);
    }

    #[test]
    fn project_export_settings_update_rejects_wrong_typed_boolean() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Settings", 176, 166, ModTarget::Forge))
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "settings-update",
                "method": "tools/call",
                "params": {
                    "name": "project_export_settings_update",
                    "arguments": {
                        "project_id": project_id,
                        "generate_runtime_helpers": "false",
                    }
                }
            }),
            &state,
        );

        assert_eq!(
            response["error"]["message"],
            "generate_runtime_helpers must be boolean"
        );

        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert_eq!(active.revision, 0);
        assert!(active.project.export_settings.generate_runtime_helpers);
    }

    #[test]
    fn project_semantic_groups_update_changes_live_session() {
        let state = test_state();
        let project_id = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Semantic Groups", 176, 166, ModTarget::Forge))
        };

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "semantic-groups-update",
                "method": "tools/call",
                "params": {
                    "name": "project_semantic_groups_update",
                    "arguments": {
                        "project_id": project_id,
                        "semantic_groups": [{
                            "id": "inventory",
                            "kind": "player_inventory",
                            "columns": 9,
                            "visible_rows": 3,
                            "total_rows": 3
                        }],
                    }
                }
            }),
            &state,
        );

        assert!(response["error"].is_null());
        let sessions = state.sessions.lock().unwrap();
        let active = sessions.active_session().unwrap();
        assert_eq!(active.project.semantic_groups.len(), 1);
        assert_eq!(
            active.project.semantic_groups[0].kind,
            crate::project::SemanticGroupKind::PlayerInventory
        );
        assert_eq!(active.revision, 1);
        assert!(active.project.is_dirty);
    }

    #[test]
    fn export_request_parses_codegen_override() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Export Override", 176, 166, ModTarget::Forge));
        let (_, config, _) = export_request(
            &sessions,
            Some(&project_id),
            &serde_json::json!({
                "target": "forge",
                "mod_id": "mcp_test",
                "package": "net.inkyquill.mcptest",
                "class_name": "OverrideScreen",
                "output_dir": "/tmp/gui-crafter-mcp-export",
                "codegen_mode": "modular",
                "generate_runtime_helpers": false,
                "generate_semantic_registry": false,
            }),
        )
        .unwrap();

        let settings = config.settings_override.unwrap();
        assert_eq!(settings.codegen_mode, CodegenMode::Modular);
        assert!(!settings.generate_runtime_helpers);
        assert!(!settings.generate_semantic_registry);
    }

    #[test]
    fn export_request_rejects_unknown_codegen_override() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Export Override", 176, 166, ModTarget::Forge));
        let error = match export_request(
            &sessions,
            Some(&project_id),
            &serde_json::json!({
                "target": "forge",
                "mod_id": "mcp_test",
                "package": "net.inkyquill.mcptest",
                "class_name": "OverrideScreen",
                "output_dir": "/tmp/gui-crafter-mcp-export",
                "codegen_mode": "split",
            }),
        ) {
            Ok(_) => panic!("expected codegen override error"),
            Err(error) => error,
        };

        assert_eq!(error, "Unknown codegen_mode: split");
    }

    #[test]
    fn export_request_rejects_wrong_typed_codegen_override() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Export Override", 176, 166, ModTarget::Forge));
        let error = match export_request(
            &sessions,
            Some(&project_id),
            &serde_json::json!({
                "target": "forge",
                "mod_id": "mcp_test",
                "package": "net.inkyquill.mcptest",
                "class_name": "OverrideScreen",
                "output_dir": "/tmp/gui-crafter-mcp-export",
                "codegen_mode": 123,
            }),
        ) {
            Ok(_) => panic!("expected codegen override type error"),
            Err(error) => error,
        };

        assert_eq!(error, "codegen_mode must be \"simple\" or \"modular\"");
    }

    #[test]
    fn export_request_rejects_wrong_typed_overwrite() {
        let state = test_state();
        {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.create_session(Project::new("Overwrite Type", 176, 166, ModTarget::Forge));
        }

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "preview",
                "method": "tools/call",
                "params": {
                    "name": "project_export_preview",
                    "arguments": {
                        "target": "forge",
                        "mod_id": "overwrite_type",
                        "package": "net.inkyquill.overwrite",
                        "class_name": "OverwriteType",
                        "output_dir": "/tmp/mcgui-overwrite-type",
                        "overwrite": "true"
                    }
                }
            }),
            &state,
        );

        assert_eq!(response["error"]["message"], "overwrite must be boolean");
    }

    #[test]
    fn get_mcp_returns_405_when_sse_is_not_supported() {
        let state = test_state();
        let request = HttpRequest {
            method: "GET",
            path: MCP_PATH,
            headers: vec![("Accept", "text/event-stream")],
            body: &[],
        };

        let (status, _, body) = route_http_request(request, &state);

        assert_eq!(status, 405);
        assert_eq!(String::from_utf8(body).unwrap(), "SSE is not supported");
    }

    #[test]
    fn invalid_jsonrpc_version_returns_invalid_request() {
        let state = test_state();

        let response = response_for(
            serde_json::json!({
                "jsonrpc": "1.0",
                "id": 3,
                "method": "tools/list"
            }),
            &state,
        );

        assert_eq!(response["error"]["code"], -32600);
    }
}
