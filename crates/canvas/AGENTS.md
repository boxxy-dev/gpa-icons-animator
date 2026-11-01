# `gpa-canvas` (`crates/canvas`) — Studio Canvas, Dynamic State Boxes & Customize Card

## Responsibilities
Implements `StudioCanvas`, the central scrollable rounded island (`min_content_width = 290`, `adw::Clamp` `maximum_size = 580`) with four vertically stacked sections so the user can work comfortably even with both sidebars closed:

1. **Compact Showcase Banner (`gpa-hero-banner`):**
   - Renders the active icon inside a centered `128×128` (`16 × 8 px` per SVG unit) `gtk4::Overlay` inside a `144×144` showcase frame (`gpa-hero-frame`).
   - **Exact 16×16 Coordinate Grid (`grid_area: gtk4::DrawingArea`):** Draws the 16×16 logical pixel grid (`0..=16` lines at `8.0 px` intervals) and outer boundary box using Cairo so it aligns 1:1 with the `128 px` `gtk4::Svg` icon and remains clearly visible on both dark (`rgba(255,255,255,0.15)`) and light (`rgba(0,0,0,0.16)`) surfaces.
   - **Live X-Ray Shape Spine (`xray_svg` / `toggle_xray_btn`):** Layers a second `128 px` `gtk4::Svg` (`build_xray_gpa_xml`) rendering a crisp 1px red (`rgb(237,51,59)`) centerline for every shape in the icon, synchronized with the active state and frame clock.
   - Embeds circular transport & surface buttons (`Play/Pause`, `Prev State`, `Next State`, `16×16 Grid`, `X-Ray Shapes`, `Light/Dark Surface`) and active state pill.
   - **Universal Play / Stop (`set_playing` / `sync_svg_playback_state`):**
     - When **Stopped/Paused (`playing == false`, `▶`)**, stops the state-cycling timer, loads `with_initial_state_in_gpa(&xml, active_state, true)` (stripping continuous `gpa:animation-*` and `gpa:transition-*` attributes so shapes with `segment` or `alternate` loops show their full static geometry), and calls `.pause()` on `hero_svg`, `xray_svg`, and `active_clone_svgs`. Clicking any State Card while paused updates the hero preview to that state in complete stillness.
     - When **Playing (`playing == true`, `⏸`)**, loads the live animated `.gpa` XML, calls `.play()` on all active SVGs, immediately advances to the next state, and runs a `glib::timeout_add_local` loop cycling through the icon's states (`0..N`) at `(Speed + 450) ms`.
     - State Preview Wells (`#0`, `#1`, …) always use `with_initial_state_in_gpa(&xml, state_idx, true)` with `.pause()` so state cards are always clean, static thumbnails.

2. **Dynamic State Preview Boxes (`STATES & TRANSITIONS`):**
   - Dynamically generates a compact `96 px` card for every state in `GpaDocumentSummary.states` (`0..N`), plus a trailing `+ Add` state card.
   - Each card includes:
     - State index badge (`#0`), **inline editable state name** (`gtk4::Entry` calling `rename_state_in_gpa` on `Enter`), and a `×` remove button (when `states.len() > 1`).
     - A `52×52` preview well (`32 px` `gtk4::Svg` locked to `set_state(state_idx)`). Clicking the well triggers a live GPA transition on the hero icon and context grid.
   - **Zero-Jitter Invariant:** Both `.gpa-state-card` and `.gpa-state-card-active` MUST use the exact same `2px` border width, and `state_boxes_row` (`GtkFlowBox`) MUST keep `valign(Align::Start)` and `vexpand(false)` so cards never stretch vertically or jump when clicked.

3. **Stacked Customize Card (`gpa-customize-card`):**
   - Provides automatic real-time GPA customization controls: `Fill` switch, `Reset` button, `Weight` (`100..1000`), `Stroke` (`0.5..4.0 px`), `Speed` (`100..1200 ms`) sliders, and a 2×2 grid of dropdowns (`Transition`, `Loop Anim`, `Paint`, `Rendering`).

4. **Context Scale Preview (`48 px & 24 px`):**
   - Scrollable bottom card with 8 slots at `36/48 px` and 16 slots at `20/24 px` comparing live clones of the active `.gpa` icon against bundled reference icons from `gpa-icon-library`.

## Guidelines
- Always update this `AGENTS.md` when modifying canvas controls, state cards, or preview rendering.
