# GPA Icons Animator (`gpa-icons-animator`) — Architecture & Specifications

## 1. Overview & Vision

**GPA Icons Animator** is a native GNOME application built with **Rust 2024**, **GTK 4.24**, and **Libadwaita 1.10**, inspired by GTK's **Shaper** (`org.gtk.Shaper` / `gtk4-icon-editor`) and modern adaptive GNOME media/studio layouts. Its purpose is to create, preview, animate, and export **GTK Path Animation (`.gpa` / Grappa)** symbolic icons.

### Core Philosophy: Self-Contained Studio + AI-Driven via MCP
1. **100% Self-Contained Icon Library:** All 1,100+ GNOME Icon Development Kit symbolic icons (`resources/kit_icons/`), built-in animated `.gpa` examples (`resources/examples/`), and the complete `GPA_SPECS.md` authoring specification are **bundled directly inside the application repository and GResource/embedded assets**. The application never relies on external or hardcoded host paths to load its icon library.
2. **Zero Embedded LLM — Driven by MCP (`rmcp`):** The app itself contains no AI SDK. Instead, it runs an embedded **Model Context Protocol (MCP) server** (`rmcp 3.8.0`) that serves `GPA_SPECS.md` and exposes tools to inspect, create, animate, and mutate the active `.gpa` icon in real time.
3. **Live Visual Feedback:** Every state change or MCP tool mutation immediately updates the `gtk4::Svg` (`GtkSvg`) hero preview, the **Dynamic State Preview Boxes**, the 48px/24px context grid, and the live XML inspector.

---

## 2. Critical Development Rules (MUST FOLLOW)

1. **NEVER Hardcode Local Paths (CRITICAL):**
   - NEVER hardcode machine-specific paths (e.g., `/home/...`, `/Users/...`), local user directories, or absolute host filesystem paths anywhere in application code, build scripts, tests, or documentation.
   - All bundled icons, UI definitions, stylesheets, examples, and `GPA_SPECS.md` must be loaded from workspace-relative paths at build time or embedded GResources / `include_str!` / `include_bytes!` at runtime.
   - Runtime configuration paths must exclusively resolve through `gpa_sys_utils::get_config_dir()` using `directories::ProjectDirs` (with optional `GPA_CONFIG_DIR` env override).
   - Unit and integration tests must strictly use synthetic in-memory fixtures or `tempfile::tempdir()`.
2. **NEVER Commit to Git (CRITICAL):**
   - **NEVER run `git commit`, `git push`, or create branches automatically.** All Git commits and repository operations are performed manually by the user ONLY.
3. **Always Use Context7 MCP:**
   - Query the **Context7 MCP** server whenever verifying APIs for `gtk4` (4.24 `GtkSvg`), `libadwaita` (1.10), `resvg` (0.49), `rmcp` (3.8), or `sourceview5` (0.11).
4. **Always Update `AGENTS.md` Files (CRITICAL):**
   - Every crate in `crates/*/` has its own `AGENTS.md` describing its architecture, invariants, and responsibilities.
   - Whenever you add, modify, or refactor features in any crate or the workspace, you **MUST** update both the root `AGENTS.md` and the affected crate's `AGENTS.md` (`crates/<crate>/AGENTS.md`) to keep documentation 100% in sync with the code.

---

## 3. Technology Stack & Workspace Dependencies

- **Language:** Rust 2024 Edition (`rust-version = "1.98"`)
- **UI Toolkit:** GTK 4.24 (`gtk4 = { version = "0.11", features = ["v4_24"] }`, `gsk4 = { version = "0.11", features = ["v4_24"] }`)
- **Design System:** Libadwaita 1.10 (`libadwaita = { version = "0.9", features = ["v1_10", "gtk_v4_24"] }`)
- **SVG Geometry & Rasterization:** `resvg = "0.49.0"` (alongside native `gtk4::Svg` for live GPA state/animation rendering)
- **Async Runtime & Channels:** `tokio = { version = "1.54.1", features = ["full"] }`, `async-channel = "2.5.0"`, `futures-util = "0.3.34"`
- **MCP Server Protocol:** `rmcp = { version = "3.8.0", default-features = false, features = ["server", "transport-io", "transport-streamable-http-server", "macros", "schemars"] }`
- **Fast Search & Filtering:** `frizbee = "0.13.0"` (SIMD Smith-Waterman fuzzy matching) + `ignore = "0.4.34"`
- **Code Editor / XML View:** `sourceview5 = "0.11"`

All crates inherit versions and lints from the root `Cargo.toml` `[workspace.dependencies]` and `[workspace.lints]`.

---

## 4. UI Architecture & Layout Design

The application uses a unified `#181922` shell with a flat top `AdwHeaderBar` and three floating rounded-corner islands (`#12131a`, `border-radius: 14px`) separated by subtle vertical-dots resize handles (`⋮`):

1. **Unified Top `AdwHeaderBar`:**
   - **Start (Left):**
     - **Left Sidebar Toggle** (`sidebar-show-symbolic` `GtkToggleButton`) to collapse/expand the bundled Icon Library sidebar.
     - Quick actions: **New Blank Icon**, **Import SVG/GPA…**, **Save to Custom Library** (`folder-download-symbolic` `GtkMenuButton` popover targeting user library folders in `<config_dir>/libs/` with a live overwrite warning when an icon with the same name exists), and **Export `.gpa`…**.
   - **Center (`title-widget`):**
     - Centered **Search Entry** (`GtkSearchEntry`) for instant fuzzy filtering of the bundled Icon Development Kit and custom user library icons (`frizbee`).
   - **End (Right):**
     - **MCP Status Badge** (`MCP :44820`) opening the `MCP Specs` tab (`GPA_SPECS.md`).
     - **Right Sidebar Toggle** (`sidebar-show-right-symbolic` `GtkToggleButton`) to collapse/expand the Right Shapes & XML sidebar.

2. **Left Sidebar Island (`LibrarySidebar` — Collapsible & Resizable `170..480 px`):**
   - **Top Header Bar:** Features a **Library Selector Dropdown** (`GtkDropDown`: `Icon Dev Kit`, `GPA Examples`, discovered user folders under `<config_dir>/libs/` such as `My Icons` (`libs/my-icons`), and `All Libraries`), an accurate total icon count badge (`matches.len()`, identical across List and Grid views), and a **List vs Grid** view toggle (`view-list-symbolic` vs `view-grid-symbolic`, where Grid mode displays icons without names and shows the name on tooltip hover).
   - **Custom User Libraries (`<config_dir>/libs/<folder-name>`):** Automatically discovers any subdirectory created inside `<config_dir>/libs/` (ensuring `libs/my-icons` exists by default) and lists its `.gpa` / `.svg` files. On custom user libraries, users can **Open Icon** (click or right-click menu) or **Remove Icon** (subtle row trash button or right-click menu).
   - **Scroll-Driven Lazy Loading:** Renders the first batch immediately (`80` rows in List mode / `120` cells in Grid mode) and automatically appends batches via `vadjustment` scroll listeners so all 1,110+ icons (and future libraries) load instantaneously without UI lag.

3. **Center Studio Canvas Island (`StudioCanvas` — Compact & Vertically Stacked):**
   - Designed so users can work comfortably with both sidebars closed (`adw::Clamp` `maximum_size = 580`, `min_content_width = 290`):
   - **Compact Showcase Banner:** `144×144` showcase well containing a centered `128×128` (`16 × 8 px` per SVG unit) `gtk4::Overlay` with a Cairo-rendered 16×16 coordinate grid (`grid_area`, crisp on both dark and light surfaces), `128×128` `gtk4::Svg` preview, live red-hairline X-Ray path spine overlay (`xray_svg`), title, active state pill, and circular showcase buttons (`Play/Pause` which actively cycles states and runs loop animations, `Prev State`, `Next State`, `16×16 Grid`, `X-Ray Shapes`, `Light/Dark Surface`).
   - **Dynamic State Preview Boxes (`STATES & TRANSITIONS`):**
     - Compact `96 px` cards dynamically generated for every state (`0..N`), plus a trailing `+ Add` state card.
     - Each card features a state index badge (`#0`), an **inline editable state name** (`gtk4::Entry` that renames the state in `gpa:state-names` and `gpa:states` on `Enter`), a `×` remove button, and a `52×52` `gtk4::Svg` preview well locked to that state.
     - Uses a constant `2px` border width on both active and inactive cards (and no inner button hover box) so selecting states produces zero layout movement.
   - **Stacked Customize Card (`CUSTOMIZE`):**
     - Built directly into the main view below the state boxes: `Fill` switch, `Reset` button, `Weight` (`100..1000`), `Stroke` (`0.5..4.0 px`), `Speed` (`100..1200 ms`) sliders, and a 2×2 grid of dropdowns (`Transition`, `Loop Anim`, `Paint`, `Rendering`).
   - **Scrollable Context Scale Preview (`48 px & 24 px`):**
     - Located at the bottom of the scroll view, comparing active `.gpa` clones against neighbor symbolic icons.

4. **Right Sidebar Island (`InspectorSidebar` — Collapsible & Resizable `200..560 px`):**
   - Three tabs (`Shapes`, `GPA XML` via `sourceview5::View`, and `MCP Specs` displaying `GPA_SPECS.md` plus 1-click `URL` and `Prompt` clipboard buttons for `http://127.0.0.1:44820/mcp`).
   - Keeps `stack.set_hhomogeneous(false)` and caps label natural widths (`max_width_chars`) so two-way drag resizing via the stationary `center_split_row` `GtkGestureDrag` controller is always authoritative.
   - Window and sidebar geometry (`UiWindowState`) is automatically saved to `ui_state.json` on resize/toggle/close and restored on startup.

5. **Embedded Streamable HTTP MCP Server (`GpaMcpServer` on `http://127.0.0.1:44820/mcp`):**
   - Automatically starts on `127.0.0.1:<port>` inside `gpa_sys_utils::runtime()` when the window initializes.
   - Exposes MCP Resources (`gpa://specs`, `gpa://document/current`, `gpa://document/current.gpa`) and MCP Tools (`get_active_icon`, `get_gpa_specs`, `patch_shape_gpa`, `upsert_shape_gpa`, `delete_shape_gpa`, `manage_states`, `update_gpa_xml`, `search_and_load_icon`, `set_viewer_controls`) so any connected AI agent can inspect the active icon (e.g. `3d-box.svg`), read `GPA_SPECS.md`, and surgically animate shapes/states in real time.

---

## 5. Modular Crate Structure

```text
gpa-icons-animator/
├── Cargo.toml
├── AGENTS.md
├── GPA_SPECS.md
├── resources/
│   ├── resources.gresource.xml
│   ├── style.css
│   ├── ui/
│   │   └── window.ui
│   ├── examples/
│   │   └── example1.gpa .. example6.gpa
│   └── kit_icons/
│       └── (*.svg, *.gpa bundled from icon-development-kit)
└── crates/
    ├── app/              # (Binary: gpa-icons-animator) Entry point, GResources, CSS, app lifecycle
    ├── window/           # (Lib: gpa-window) Main AdwApplicationWindow, dual sidebars, headerbar, bottom bar
    ├── canvas/           # (Lib: gpa-canvas) Hero GtkSvg preview, Dynamic State Boxes, 48px/24px context grid
    ├── icon_library/     # (Lib: gpa-icon-library) Embedded/bundled icon catalog, frizbee search, import/export
    ├── gpa_core/         # (Lib: gpa-core) Pure-Rust GPA parser, state/path inspector, GPA_SPECS.md provider
    ├── mcp_server/       # (Lib: gpa-mcp-server) rmcp ServerHandler exposing GPA tools, resources, and prompts
    └── sys_utils/        # (Lib: gpa-sys-utils) Global Tokio runtime and config directory resolution
```

---

## 6. Coding Guidelines

- **File Length (~700 LOC):** Keep files focused around ~700 lines of code where natural domain boundaries exist; use directory submodules (`mod.rs`) when components grow.
- **Declarative UI & CSS:** Keep CSS in `resources/style.css` without `!important` (use higher selector specificity instead). Avoid raw `&` in `.ui` XML attributes.
- **Non-Blocking UI:** Never block the GTK main thread; use `glib::spawn_future_local` and avoid holding `RefCell` borrows across `.await` points.
