use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::animation::Animation;
use crate::project::{
    CodegenMode, Element, ModTarget, Project, ProjectSessionManager, SemanticGroup,
};
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

            match execute_tool(tool_name, &arguments, state) {
                Ok(content) => {
                    if is_mutating_tool(tool_name) {
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
                Err(message) => json_rpc_error(request.id, -32000, message),
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
            | "project_list_sessions"
            | "project_get_active"
            | "project_export_preview"
            | "project_export"
            | "element_list"
            | "group_list"
            | "animation_list"
            | "asset_list"
            | "asset_get_data_url"
            | "gui_template_list"
    )
}

fn get_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        td(
            "project_new",
            "Create a new GUI project",
            props(&[
                ("name", "string", "Project name", true),
                ("width", "integer", "GUI width", false),
                ("height", "integer", "GUI height", false),
                ("template", "string", "Template name", false),
                ("mod_target", "string", "forge, fabric, or neoforge", false),
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
            "project_summary",
            "Get a project summary",
            project_props(&[]),
        ),
        td(
            "project_export_settings_update",
            "Update project code generation/export settings",
            project_props(&[
                ("codegen_mode", "string", "simple or modular", false),
                (
                    "generate_runtime_helpers",
                    "boolean",
                    "Generate runtime helper hooks",
                    false,
                ),
                (
                    "generate_semantic_registry",
                    "boolean",
                    "Generate semantic registry in modular mode",
                    false,
                ),
            ]),
        ),
        td(
            "project_semantic_groups_update",
            "Replace project semantic group definitions",
            project_props(&[("semantic_groups", "array", "Semantic group array", true)]),
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
            "group_ungroup",
            "Remove a group while keeping its elements",
            project_props(&[("group_id", "string", "Group ID", true)]),
        ),
        td("group_list", "List groups", project_props(&[])),
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
            project_props(&[("file_path", "string", "PNG path", true)]),
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

fn export_props() -> serde_json::Value {
    project_props(&[
        ("target", "string", "forge, fabric, or neoforge", true),
        ("mod_id", "string", "Minecraft mod id", true),
        ("package", "string", "Java package name", true),
        ("class_name", "string", "Generated Screen class name", true),
        (
            "output_dir",
            "string",
            "Directory where export files are written",
            true,
        ),
        ("codegen_mode", "string", "simple or modular", false),
        (
            "generate_runtime_helpers",
            "boolean",
            "Generate runtime helper hooks",
            false,
        ),
        (
            "generate_semantic_registry",
            "boolean",
            "Generate semantic registry in modular mode",
            false,
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

fn execute_tool(
    name: &str,
    args: &serde_json::Value,
    state: &AppState,
) -> Result<serde_json::Value, String> {
    let mut sessions = state.sessions.lock().unwrap();
    let project_id = optional_string(args, "project_id");
    let project_id = project_id.as_deref();

    match name {
        "project_new" => project_new(&mut sessions, args),
        "project_open" => project_open(&mut sessions, args),
        "project_save" => project_save(&mut sessions, project_id),
        "project_save_as" => project_save_as(&mut sessions, project_id, args),
        "project_export_preview" => project_export_preview(&sessions, project_id, args),
        "project_export" => project_export(&sessions, project_id, args),
        "project_summary" => project_summary(&sessions, project_id),
        "project_export_settings_update" => {
            project_export_settings_update(&mut sessions, project_id, args)
        }
        "project_semantic_groups_update" => {
            project_semantic_groups_update(&mut sessions, project_id, args)
        }
        "project_list_sessions" => Ok(serde_json::json!({ "sessions": sessions.list_sessions() })),
        "project_get_active" => project_get_active(&sessions),
        "project_undo" => Ok(serde_json::to_value(sessions.undo(project_id)?).unwrap()),
        "project_redo" => Ok(serde_json::to_value(sessions.redo(project_id)?).unwrap()),
        "element_add" => element_add(&mut sessions, project_id, args),
        "element_move" => element_move(&mut sessions, project_id, args),
        "element_update" => element_update(&mut sessions, project_id, args),
        "element_resize" => element_resize(&mut sessions, project_id, args),
        "element_reorder" => element_reorder(&mut sessions, project_id, args),
        "element_remove" => element_remove(&mut sessions, project_id, args),
        "element_list" => {
            let session = sessions.resolve(project_id)?;
            Ok(serde_json::json!({ "elements": session.project.elements }))
        }
        "group_create" => group_create(&mut sessions, project_id, args),
        "group_ungroup" => group_ungroup(&mut sessions, project_id, args),
        "group_list" => {
            let session = sessions.resolve(project_id)?;
            Ok(serde_json::json!({ "groups": session.project.groups }))
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
    serde_json::to_value(crate::export::preview_export(project, &config, target)?)
        .map_err(|error| error.to_string())
}

fn project_export(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let (project, config, target) = export_request(sessions, project_id, args)?;
    Ok(serde_json::json!({
        "files": crate::export::export_project(project, &config, target)?,
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
    let payload = args.get("element").unwrap_or(args).clone();
    let element: Element = serde_json::from_value(payload)
        .map_err(|error| format!("Invalid element payload: {error}"))?;
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.add_element(element.clone());
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(element).unwrap())
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
    let current = sessions
        .resolve(project_id)?
        .project
        .find_element(id)
        .ok_or("Element not found")?;
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
    let updated: Element = serde_json::from_value(value)
        .map_err(|error| format!("Invalid element update: {error}"))?;
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
    let element_ids = args
        .get("element_ids")
        .and_then(|value| value.as_array())
        .ok_or("Missing element_ids")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or("element_ids must contain only strings".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let group_id = args
        .get("group_id")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("group_{}", uuid::Uuid::new_v4()));

    validate_group_create(sessions, project_id, &group_id, &element_ids)?;

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let group = session.project.group_elements(group_id, element_ids)?;
    sessions.mark_changed(project_id)?;
    Ok(serde_json::to_value(group).unwrap())
}

fn validate_group_create(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    group_id: &str,
    element_ids: &[String],
) -> Result<(), String> {
    let project = &sessions.resolve(project_id)?.project;
    if project.groups.iter().any(|group| group.id == group_id) {
        return Err("Group already exists".to_string());
    }
    let mut unique_ids: Vec<&String> = Vec::new();
    for id in element_ids {
        if !unique_ids.contains(&id) {
            unique_ids.push(id);
        }
        if project.find_element(id).is_none() {
            return Err(format!("Element not found: {id}"));
        }
    }
    if unique_ids.len() < 2 {
        return Err("At least two elements are required to create a group".to_string());
    }
    Ok(())
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
    let name = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("texture");
    let asset_path = format!("textures/{name}.png");
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

    use base64::Engine;
    Ok(serde_json::json!({
        "name": asset_path,
        "width": image.width(),
        "height": image.height(),
        "data_url": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(data)),
    }))
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

fn asset_remove(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let name = required_str(args, "name")?;
    let exists = {
        let project = &sessions.resolve(project_id)?.project;
        project.assets.iter().any(|asset| asset == name) || project.texture_data.contains_key(name)
    };
    if !exists {
        return Ok(serde_json::json!({ "removed": false }));
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let removed_texture = session.project.texture_data.remove(name).is_some();
    let old_len = session.project.assets.len();
    session.project.assets.retain(|asset| asset != name);
    let removed_asset = session.project.assets.len() != old_len;
    if removed_texture || removed_asset {
        sessions.mark_changed(project_id)?;
    }
    Ok(serde_json::json!({ "removed": removed_texture || removed_asset }))
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
    use base64::Engine;
    Ok(serde_json::json!({
        "name": name,
        "data_url": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(data)),
    }))
}

fn asset_list(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let project = &sessions.resolve(project_id)?.project;
    use base64::Engine;
    let assets = project
        .assets
        .iter()
        .map(|name| {
            let (width, height, data_url) = if let Some(data) = project.texture_data.get(name) {
                let image = image::load_from_memory(data).ok();
                (
                    image.as_ref().map(|image| image.width()).unwrap_or(16),
                    image.as_ref().map(|image| image.height()).unwrap_or(16),
                    format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(data)
                    ),
                )
            } else {
                (16, 16, String::new())
            };
            serde_json::json!({
                "name": name,
                "width": width,
                "height": height,
                "data_url": data_url,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "assets": assets }))
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

fn required_str<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .ok_or(format!("Missing {key}"))
}

fn required_i32(value: &serde_json::Value, key: &str) -> Result<i32, String> {
    value
        .get(key)
        .and_then(|value| value.as_i64())
        .map(|value| value as i32)
        .ok_or(format!("Missing {key}"))
}

fn required_u32(value: &serde_json::Value, key: &str) -> Result<u32, String> {
    value
        .get(key)
        .and_then(|value| value.as_u64())
        .map(|value| value as u32)
        .ok_or(format!("Missing {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, TcpListener};
    use std::sync::Mutex;

    fn test_state() -> AppState {
        AppState {
            sessions: Mutex::new(ProjectSessionManager::default()),
            mcp_handle: Mutex::new(None),
            app_handle: Mutex::new(None),
        }
    }

    fn response_for(value: serde_json::Value, state: &AppState) -> serde_json::Value {
        match handle_json_rpc_value(value, state) {
            RpcReply::Response(response) => serde_json::to_value(response).unwrap(),
            RpcReply::Notification => panic!("expected JSON-RPC response"),
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
    fn export_props_accept_codegen_override() {
        let schema = export_props();
        let properties = schema["properties"].as_object().unwrap();

        assert!(properties.contains_key("codegen_mode"));
        assert!(properties.contains_key("generate_runtime_helpers"));
        assert!(properties.contains_key("generate_semantic_registry"));
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
        assert_eq!(value["generate_semantic_registry"], true);

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
        assert!(settings.generate_semantic_registry);
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
