# `gpa-core` (`crates/gpa_core`) — Headless GPA Parser, Transformer & Spec Provider

## Responsibilities
Pure-Rust, GTK-independent library crate for inspecting and transforming `.gpa` (`image/x-gtk-path-animation`) and symbolic `.svg` XML documents:

- **Embedded Specification (`GPA_SPECS_MARKDOWN`):** Exports the root `GPA_SPECS.md` file via `include_str!("../../../GPA_SPECS.md")` for both the UI (`MCP Specs` tab) and the MCP server.
- **Document & State Discovery (`GpaDocumentSummary::parse`):** Parses root `<svg>` attributes (`width`, `height`, `gpa:version`, `gpa:state-names`, `gpa:state`, `gpa:rendering`, `gpa:keywords`) and graphical primitives (`<path>`, `<circle>`, `<rect>`) including full geometry (`d`, `cx`/`cy`/`r`, `x`/`y`/`width`/`height`) and timing attributes (`transition_delay`, `origin`, `animation_segment`, `attach_pos`), computing 64-bit state bitmasks (`state_mask`) and per-state visible shape counts (`GpaStateInfo`).
- **GPA Normalization (`ensure_gpa_headers`):** Upgrades standard symbolic SVGs with `xmlns:gpa="https://www.gtk.org/grappa" gpa:version="1"`, injects `gpa:stroke-minwidth="25%" gpa:stroke-maxwidth="150%"` on stroked shapes so variable `-gtk-icon-weight` works out of the box, and injects default `gpa:transition-type='animate' gpa:transition-duration='400ms' gpa:transition-easing='ease-in-out'` on stroked shapes lacking transition attributes so all bundled Dev Kit icons animate smoothly out of the box.
- **Automatic & Surgical GPA Mutations:**
  - `apply_auto_customize(&str, &GpaAutoCustomize)` — rewrites `gpa:fill`, `stroke-width`, `gpa:transition-*`, `gpa:animation-*`, `gpa:stroke`, compat `class`es, and `gpa:rendering`.
  - `patch_shape_in_gpa`, `upsert_shape_in_gpa`, `duplicate_shape_in_gpa`, `delete_shape_from_gpa`, `build_xray_gpa_xml`, `with_initial_state_in_gpa`, and `validate_gpa_xml` — surgical shape-level XML mutators (`GpaShapePatch`), X-Ray red centerline generator, static state snapshot generator, and XML validator for MCP tools and studio overlays.
  - `add_state_to_gpa`, `remove_state_from_gpa`, `rename_state_in_gpa`, `set_all_states_in_gpa`, and `empty_scratch_gpa` — surgical state-list/token mutators and blank 0-shape `untitled.gpa` template generator.

## Guidelines
- Keep `gpa-core` 100% free of GTK/Adwaita dependencies so unit tests run headlessly and fast.
- Every new XML transformation or parser feature must include a unit test in `src/lib.rs`.
- Always update this `AGENTS.md` when adding GPA attributes, validators, or transformations.
