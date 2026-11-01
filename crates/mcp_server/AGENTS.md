# `gpa-mcp-server` (`crates/mcp_server`) — Model Context Protocol Bridge & Server

## Responsibilities
Bridges external MCP-compatible AI agents (Boxxy Claw, OpenCode, Claude Code, Cursor, etc.) to the live GTK studio canvas without embedding an LLM inside the app:

- **Lock-Free Snapshot (`McpBridge`):** Holds an `Arc<ArcSwap<StudioSnapshot>>` containing the current `GpaDocumentSummary` and viewer state (`active_state`, `playing`, `weight`, `show_grid`, `show_bounds`). Mutations via `McpBridge::commit_document` atomically update `snapshot` immediately so chained MCP tool calls never read stale state while queuing `McpUiCommand` updates to GTK.
- **UI Command Channel (`McpUiCommand`):** Sends mutations (`LoadDocument`, `SetViewerState`, `SetViewerPlaying`, `SetViewerWeight`, `SetViewerDark`, `SetViewerGrid`, `SetViewerXray`, `LogEvent`) over an unbounded `async_channel::Sender<McpUiCommand>` consumed by `GpaWindow` via `glib::spawn_future_local`.
- **Streamable HTTP Server (`http_server.rs`):** Binds `127.0.0.1:<port>` (default `44820`) on the global Tokio runtime with HTTP/1.1 keep-alive and SSE streaming support, serving `rmcp`'s `StreamableHttpService<GpaMcpServer, LocalSessionManager>` at `/mcp` (plus `GET /status` / `GET /health`).
- **`GpaMcpServer` (`ServerHandler`):**
  - **Resources:** `gpa://specs` (`GPA_SPECS.md`), `gpa://document/current` (JSON snapshot), and `gpa://document/current.gpa` (raw XML).
  - **Tools:** `get_active_icon`, `get_gpa_specs`, `patch_shape_gpa`, `upsert_shape_gpa`, `delete_shape_gpa`, `manage_states` (`add`, `rename`, `remove`, `set_all`, `switch`), `update_gpa_xml` (with `auto_play`), `search_and_load_icon` (with `inspect_only` reference mode), and `set_viewer_controls` (`playing`, `weight`, `dark_surface`, `show_grid`, `show_xray`).

## Guidelines
- Never perform GTK widget calls inside `gpa-mcp-server`; all UI updates must go through `McpUiCommand`.
- Always update this `AGENTS.md` when adding MCP tools, resources, or prompts.
