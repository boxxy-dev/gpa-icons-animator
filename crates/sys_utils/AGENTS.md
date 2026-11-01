# `gpa-sys-utils` (`crates/sys_utils`) — Runtime, Config Paths & Persistent UI State

## Responsibilities
Leaf-level utility crate with zero GTK or AI dependencies:

- **Global Tokio Runtime (`runtime()`):** Lazily initializes and returns `&'static tokio::runtime::Runtime` (`gpa-tokio-worker`).
- **Config Directory Resolution (`get_config_dir()`):** Resolves the configuration directory via `GPA_CONFIG_DIR` (if set to an absolute path), `directories::ProjectDirs::from("dev", "boxxy", "gpa-icons-animator")`, or `~/.config/gpa-icons-animator`.
- **User Icon Libraries Directory (`get_libraries_dir()` & `ensure_default_user_library()`):** Resolves `<config_dir>/libs` (never `.local/share`) and ensures the default `<config_dir>/libs/my-icons` (`DEFAULT_USER_LIBRARY_FOLDER`) folder exists on disk.
- **Window & Sidebar Persistence (`UiWindowState`):** Loads and saves `ui_state.json` inside `get_config_dir()`, persisting `window_width`, `window_height`, `left_sidebar_width`, `right_sidebar_width`, `left_sidebar_open`, and `right_sidebar_open`.
- **MCP Port Resolution (`get_mcp_port()`):** Reads `GPA_MCP_PORT` or defaults to `44820`.

## Guidelines
- Never hardcode machine-specific user paths; always resolve through `get_config_dir()`.
- Always update this `AGENTS.md` when adding environment variables or persistent config structs.
