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
    CodegenMode, Element, ModTarget, Project, ProjectSessionManager, SemanticGroup,
};
use crate::{templates, AppState};

const MCP_PATH: &str = "/mcp";
const SERVER_NAME: &str = "mc-gui-crafter";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const SLOT_ROLE_DESCRIPTION: &str = "Accepted values: machine, player_inventory, hotbar, scrollable_inventory, virtual_storage, upgrade, upgrade_settings, filter, ghost, offhand.";
const SEMANTIC_GROUP_KIND_DESCRIPTION: &str = "Accepted values: fixed_slots, virtual_slot_grid, player_inventory, hotbar, upgrade_slots, upgrade_panel, search_field, control_buttons.";

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
                    "kind": { "type": "string", "description": SEMANTIC_GROUP_KIND_DESCRIPTION },
                    "columns": { "type": "integer", "description": "Grid column count" },
                    "visible_rows": { "type": "integer", "description": "Visible row count" },
                    "total_rows": { "type": "integer", "description": "Total row count" },
                    "slot_count": { "type": "integer", "description": "Total slot count" },
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
            serde_json::json!({ "type": "string", "description": SLOT_ROLE_DESCRIPTION }),
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
            serde_json::json!({ "type": "string", "description": SEMANTIC_GROUP_KIND_DESCRIPTION }),
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
        "element_add_many" => element_add_many(&mut sessions, project_id, args),
        "slot_grid_add" => slot_grid_add(&mut sessions, project_id, args),
        "element_move" => element_move(&mut sessions, project_id, args),
        "element_update" => element_update(&mut sessions, project_id, args),
        "element_resize" => element_resize(&mut sessions, project_id, args),
        "element_reorder" => element_reorder(&mut sessions, project_id, args),
        "element_remove" => element_remove(&mut sessions, project_id, args),
        "element_list" => {
            let session = sessions.resolve(project_id)?;
            let elements = session
                .project
                .elements
                .iter()
                .map(element_with_effective_layer)
                .collect::<Vec<_>>();
            Ok(serde_json::json!({ "elements": elements }))
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
    Ok(serde_json::json!({
        "created_count": elements.len(),
        "elements": elements,
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
    });
    let semantic_group = match (semantic_group_kind, inventory_group.clone()) {
        (Some(kind), Some(inventory_group)) => Some(SemanticGroup {
            id: inventory_group.clone(),
            kind,
            columns: Some(columns),
            visible_rows: Some(rows),
            total_rows: Some(rows),
            slot_count: Some(semantic_slot_count.unwrap_or(elements.len() as u32)),
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

fn element_with_effective_layer(element: &Element) -> serde_json::Value {
    let mut value = serde_json::to_value(element).unwrap();
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "layer".to_string(),
            serde_json::to_value(&element.layer).unwrap(),
        );
    }
    value
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

    fn tool_text_value(response: &serde_json::Value) -> serde_json::Value {
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(content).unwrap()
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
