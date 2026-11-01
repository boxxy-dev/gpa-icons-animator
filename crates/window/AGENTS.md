# `gpa-window` (`crates/window`) — Main Window & Dual Resizable Island Sidebars

## Responsibilities
Orchestrates the `adw::ApplicationWindow` using a unified dark `#181922` shell containing three floating rounded-corner islands (`#12131a`, `border-radius: 14px`):

1. **`lib.rs` (`GpaWindow`)**
   - Starts directly on a blank `16×16` `.gpa` document (`untitled.gpa` via `empty_scratch_gpa`) with 0 shapes and 2 states (`#0 default` and `#1 active`) so the user can immediately start from scratch with an MCP agent, select an icon from the library, or import a file.
   - Builds the flat top `adw::HeaderBar` with Left Sidebar toggle, New Blank Icon, Import (`gtk4::FileDialog`), **Save to Custom Library** (`gtk4::MenuButton` popover targeting user library folders in `<config_dir>/libs/` with a live overwrite warning when an icon with the same name already exists), Export (`gtk4::FileDialog`), centered `GtkSearchEntry` (fuzzy-filtering the active library), `MCP :<port>` status pill, and Right Sidebar toggle.
   - Spawns the embedded MCP Streamable HTTP server (`mcp_bridge.spawn_http_server()`) on `127.0.0.1:<port>`.
   - Manages the 3-pane horizontal workspace (`LibrarySidebar` | `left_resize_handle` | `StudioCanvas` | `right_resize_handle` | `InspectorSidebar`).
   - Implements stationary two-way sidebar resizing via a single `GtkGestureDrag` (`PropagationPhase::Capture`) attached to `center_split_row`, detecting hits on `left_resize_handle` or `right_resize_handle` via `compute_bounds`.
   - Persists and restores `UiWindowState` (`window_width`, `window_height`, `left_sidebar_width`, `right_sidebar_width`, `left_sidebar_open`, `right_sidebar_open`) via `gpa_sys_utils::UiWindowState`.
   - Bridges live `McpUiCommand` events from `gpa-mcp-server` onto the GTK main thread via `glib::spawn_future_local`.

2. **`library_sidebar.rs` (`LibrarySidebar`)**
   - Left collapsible & resizable island (`170..480 px`).
   - Top header row contains a **Library Selector Dropdown** (`GtkDropDown`: `Icon Dev Kit`, `GPA Examples`, discovered user library folders under `<config_dir>/libs/` such as `My Icons`, and `All Libraries`), the true total icon count (`matches.len()`, identical across List and Grid views), and a **List vs Grid** view switcher (`view-list-symbolic` vs `view-grid-symbolic`). Grid mode renders icon-only cells with hover tooltips.
   - Custom user library items support two actions: **Open Icon** (click or right-click popover) and **Remove Icon** (subtle row trash button in List view or right-click popover in List/Grid view). Periodically checks `<config_dir>/libs/` so manually created folders or dropped icon files appear automatically.
   - Implements scroll-driven lazy loading (`append_next_batch` triggered by `vadjustment` `value-changed` and `upper` notifications) so 1,110+ icons and future icon libraries paginate smoothly on demand.
   - Collapses to `0 px` (`set_visible(false)` when unrevealed) so no blank space remains when closed.

3. **`inspector_sidebar.rs` (`InspectorSidebar`)**
   - Right collapsible & resizable island (`200..560 px`) with 3 tabs (`Shapes`, `GPA XML` via `sourceview5::View`, and `MCP Specs` showing `GPA_SPECS.md` plus 1-click `URL` and `Prompt` clipboard helpers).
   - **Important GTK4 Sizing Invariant:** `stack` MUST keep `hhomogeneous(false)` and `vhomogeneous(false)`, and all labels inside `InspectorSidebar` MUST set `width_chars` + `max_width_chars` (or `ellipsize(End)`) with `Overflow::Hidden` on the container/revealer so the natural width stays `< 190 px`. This ensures `set_size_request(w, -1)` is authoritative during two-way drag resizing.

## Guidelines
- Always update this `AGENTS.md` when changing window layout, sidebar tabs, or resize handle mechanics.
