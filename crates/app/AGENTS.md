# `gpa-app` (`crates/app`) — Binary Entry Point & Resource Bootstrap

## Responsibilities
- **Application Lifecycle:** Binary crate (`gpa-icons-animator`) that initializes `env_logger`, warms up the global multi-threaded Tokio runtime (`gpa_sys_utils::runtime()`), resolves the MCP server port (`gpa_sys_utils::get_mcp_port()`), and starts `libadwaita::Application` (`dev.boxxy.GpaIconsAnimator`).
- **GResource Compilation (`build.rs`):** Generates and compiles `../../resources/resources.gresource.xml` (bundling `resources/style.css`) into `compiled.gresource` via `glib_build_tools::compile_resources`.
- **Theme & SourceView Initialization:** In `connect_startup`, initializes `sourceview5`, sets `adw::ColorScheme::PreferDark` on `adw::StyleManager::default()`, and registers `/dev/boxxy/GpaIconsAnimator/style.css` at `STYLE_PROVIDER_PRIORITY_APPLICATION`.

## Guidelines
- Keep `src/main.rs` minimal — all window, sidebar, canvas, and MCP logic belongs in their dedicated library crates.
- Whenever new CSS or UI files are added under `resources/`, register them in `crates/app/build.rs` with `cargo:rerun-if-changed`.
- Always update this `AGENTS.md` when modifying startup, CLI flags, or GResource compilation.
