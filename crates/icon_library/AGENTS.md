# `gpa-icon-library` (`crates/icon_library`) — Self-Contained Bundled Icon Catalog

## Responsibilities
- **Compile-Time Asset Embedding (`build.rs`):** Scans `../../resources/examples/*.gpa` and `../../resources/kit_icons/*.{svg,gpa}` at build time and generates `bundled_icons.rs` using `include_str!`.
- **Zero External Path Dependencies:** All 1,110+ GNOME Icon Development Kit icons and built-in animated `.gpa` examples are compiled directly into the binary (`BUNDLED_ICONS`). No runtime filesystem paths are hardcoded or required.
- **Fast Fuzzy Search & Library Catalog (`src/lib.rs`):** Exposes `icon_libraries()` (`IconLibraryDescriptor` for `Icon Dev Kit`, `GPA Examples`, and `All Libraries`) and `discover_selectable_libraries()` (which includes discovered custom user folders under `<config_dir>/libs/` such as `My Icons`), using `frizbee::Matcher::from_query` (SIMD Smith-Waterman alignment) in `search_icons` and `search_library_items` to filter and rank icons in sub-millisecond time.
- **Filesystem User Libraries (`UserLibraryFolder`):** Automatically discovers subdirectories in `gpa_sys_utils::get_libraries_dir()` (`<config_dir>/libs/<folder-name>`, defaulting to `libs/my-icons`), loads `.gpa` and `.svg` files (`load_user_folder_icons`), checks for existing icons before overwriting (`custom_icon_exists_in_dir`), saves icons (`save_custom_icon_in_dir`), and removes icons (`remove_custom_icon_file`).

## Guidelines
- Never introduce hardcoded local filesystem paths in `build.rs` or `src/lib.rs`; always resolve relative to `CARGO_MANIFEST_DIR`.
- Always update this `AGENTS.md` when adding icon categories, metadata fields, or search ranking rules.
