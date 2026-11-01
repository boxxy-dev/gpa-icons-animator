# GTK Path Animation (`.gpa` / Grappa) Specification & Authoring Guide

This document defines the **GTK Path Animation (GPA)** format (`image/x-gtk-path-animation`, file extension `.gpa`), used by GTK 4.22+ / GTK 4.24 (`GtkSvg`) for state-driven, morphing, stroke-animated, and continuously animated symbolic icons.

When generating or modifying an icon for **GPA Icons Animator** via MCP, follow every rule in this specification strictly so `GtkSvg` parses and renders the icon without errors.

---

## 1. Overview of the Format

A `.gpa` file is a valid XML / SVG document using a strict, deterministic subset of SVG extended with attributes in the `xmlns:gpa="https://www.gtk.org/grappa"` namespace.

- **MIME Type:** `image/x-gtk-path-animation`
- **File Extension:** `.gpa` (or `-symbolic.svg` when exported with backwards-compatibility classes)
- **Standard Canvas Size:** `width="16" height="16"` (16×16 logical pixel grid)
- **Maximum States:** Up to **64 states** (`0` through `63`), represented internally as a 64-bit bitmask.

---

## 2. Root `<svg>` Element

Every `.gpa` document MUST declare the `gpa` namespace on the root `<svg>` element:

```xml
<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:gpa="https://www.gtk.org/grappa"
     gpa:version="1"
     gpa:state-names="play pause empty"
     gpa:state="0"
     width="16"
     height="16">
  <!-- paths, circles, rects -->
</svg>
```

### Supported `<svg>` Attributes
- `width` *(required)*: Icon width in logical pixels (standard is `'16'`).
- `height` *(required)*: Icon height in logical pixels (standard is `'16'`).
- `viewBox` *(optional)*: ViewBox rectangle (e.g., `'0 0 16 16'`).
- `xmlns:gpa` *(required)*: Must be `'https://www.gtk.org/grappa'`.
- `gpa:version`: Must be `'1'`.
- `gpa:state-names`: A space-separated list of state names. The *n*-th token (0-indexed) names state *n* (up to 64 states).
  - **Rules:** `'all'` and `'none'` are reserved keywords and MUST NOT be used as state names. State names MUST NOT start with an ASCII digit (`0-9`).
- `gpa:state`: The initial state of the icon when loaded. Can be an integer between `0` and `63` or one of the names defined in `gpa:state-names`.
- `gpa:rendering`: Controls overlap rendering when alpha is used:
  - `'svg'` *(default)*: Standard per-element SVG compositing.
  - `'symbolic'`: Mask-based symbolic rendering to prevent overlapping strokes/fills from doubling alpha opacity.
- `gpa:keywords` *(optional)*: Space-separated search keywords for the icon.

---

## 3. Allowed Graphical Primitives

Only three graphical primitives are used to define shapes:

1. **`<path>`** — Requires the `d` attribute containing standard SVG path commands (`M`, `L`, `H`, `V`, `C`, `S`, `Q`, `T`, `A`, `Z`).
2. **`<circle>`** — Requires `cx`, `cy`, and `r` attributes.
3. **`<rect>`** — Requires `x`, `y`, `width`, and `height` attributes (optional `rx`, `ry`).

Primitives may be wrapped inside `<g>` elements for organization, though `<g>` attributes are otherwise ignored by traditional symbolic parsers. Avoid `<defs>`, `<clipPath>`, `<mask>`, `<text>`, or Inkscape/Sodipodi namespaces unless required by GTK's internal morph filter serialization.

---

## 4. Styling, Symbolic Colors & Backwards Compatibility

GTK symbolic icons are recolored dynamically at runtime according to the widget's foreground color and semantic state (`success`, `warning`, `error`, `accent`).

### Paint Attributes on `<path>`, `<circle>`, `<rect>`
- `id`: Unique identifier string (e.g., `'path0'`, `'speaker'`, `'wave-inner'`). **Required** when referencing paths or using `gpa:attach-to`.
- `gpa:stroke`: Stroke paint. Either:
  - A symbolic color keyword: `'foreground'`, `'success'`, `'warning'`, `'error'`, `'accent'`, or `'none'`.
  - A fixed CSS color string parseable by `gdk_rgba_parse` (e.g., `'rgb(255,0,0)'`, `'#ff0000'`).
- `gpa:fill`: Fill paint. Same values as `gpa:stroke` (`'foreground'`, `'success'`, `'warning'`, `'error'`, `'accent'`, `'none'`, or fixed color).
- `fill-opacity`: Floating point number in `0.0..=1.0` (default `'1'`).
- `fill-rule`: `'nonzero'` or `'evenodd'`.
- `stroke-opacity`: Floating point number in `0.0..=1.0` (default `'1'`).
- `stroke-width`: Default line width when stroking (typically `'2'` or `'1'` on a 16×16 grid).
- `stroke-linecap`: `'round'`, `'butt'`, or `'square'`.
- `stroke-linejoin`: `'round'`, `'miter'`, or `'bevel'`.
- `gpa:stroke-minwidth`: Minimum stroke width when variable icon weight (`-gtk-icon-weight`, 100..1000) is applied. Can be a number or percentage of `stroke-width` (e.g. `'25%'` or `'0.5'`).
- `gpa:stroke-maxwidth`: Maximum stroke width when variable icon weight is applied (e.g. `'150%'` or `'3'`).
- *(Deprecated)* `gpa:stroke-width`: 3 space-separated numbers `min default max` (e.g. `'0.5 2 3'`). Prefer `stroke-width` + `gpa:stroke-minwidth` + `gpa:stroke-maxwidth`.

### Backwards-Compatibility Attributes (Recommended)
To ensure `.gpa` icons also render cleanly in older SVG tools and older GTK versions:
- For a stroked path with `gpa:stroke="foreground"` and no fill:
  ```xml
  fill="none"
  stroke="rgb(0,0,0)"
  class="foreground-stroke transparent-fill"
  gpa:stroke="foreground"
  ```
- For a filled path with `gpa:fill="foreground"` and no stroke:
  ```xml
  fill="rgb(0,0,0)"
  stroke="none"
  class="foreground-fill"
  gpa:fill="foreground"
  ```
- Recognized compatibility classes:
  - Fill: `'transparent-fill'`, `'foreground-fill'`, `'success-fill'`, `'warning-fill'`, `'error-fill'`
  - Stroke: `'foreground-stroke'`, `'success-stroke'`, `'warning-stroke'`, `'error-stroke'`

---

## 5. States & State Transitions

Each graphical element can be visible in all states, no states (e.g., an invisible motion guide path), or a specific subset of states.

### State Membership
- `gpa:states`: Space-separated list of state names (from `gpa:state-names`) or integer indices (`0`..`63`), or `'all'`, or `'none'`.
  - If `gpa:states` is omitted, the path is present in **all** states (`'all'`).
  - Example: `gpa:states="play"` or `gpa:states="0 1"`.

### State Transition Attributes (Triggered when `state` changes)
When the icon's active state changes from State A to State B:
- Paths present in A but not B **disappear** using their configured transition.
- Paths present in B but not A **appear** using their configured transition.
- Paths present in both A and B remain visible without transitioning.

Attributes:
- `gpa:transition-type`:
  - `'none'`: Instant appearance/disappearance.
  - `'animate'`: Dynamic stroke-drawing animation (draws the path along its length from `gpa:origin`, and un-draws it when leaving the state). Works best on stroked `<path>`, `<circle>`, and `<rect>` elements.
  - `'morph'`: Blobby metaball/SDF-like morphing effect between disappearing and appearing shapes. Ideal for Play ↔ Pause or shape-to-shape transformations.
  - `'fade'`: Smooth opacity crossfade in/out.
- `gpa:transition-duration`: Duration of the transition. Floating point number followed by `'ms'` or `'s'` (e.g. `'200ms'`, `'0.4s'`).
- `gpa:transition-delay`: Delay before the transition starts. Floating point number followed by `'ms'` or `'s'`. Can be negative for overlapping transitions.
- `gpa:transition-easing`: Easing curve. One of:
  - `'linear'`
  - `'ease-in-out'`
  - `'ease-in'`
  - `'ease-out'`
  - `'ease'`
- `gpa:origin`: Floating point number between `0` and `1` (e.g., `'0'`, `'0.5'`, `'1'`). Determines where along the normalized path length (`0.0` = start, `0.5` = middle, `1.0` = end) the `'animate'` stroke transition begins drawing.

---

## 6. Continuous Path Animations

In addition to state transitions, any path can run a continuous trim-path / segment animation while its state is active:

- `gpa:animation-type`:
  - `'none'` *(default)*: No continuous animation.
  - `'automatic'`: Continuously animates the drawn sub-segment of the path.
- `gpa:animation-direction`: Controls how the path segment sweeps over time:
  - `'normal'`: Sweeps forward from start (`0`) to end (`1`).
  - `'alternate'`: Sweeps forward then backward (`0 -> 1 -> 0`).
  - `'reverse'`: Sweeps backward (`1 -> 0`).
  - `'reverse-alternate'`: Sweeps backward then forward (`1 -> 0 -> 1`).
  - `'in-out'`: Draws in from start to full path, then un-draws out toward the end.
  - `'in-out-alternate'`: Alternating in-out sweep.
  - `'in-out-reverse'`: Reversed in-out sweep.
  - `'segment'`: Moves a fixed-length segment (`gpa:animation-segment`) continuously along the path (great for spinners, radar waves, data flow).
  - `'segment-alternate'`: Bounces a fixed-length segment back and forth along the path (great for bell clappers, sliders, pendulums).
- `gpa:animation-duration`: Duration of one cycle, with `'ms'` or `'s'` unit (e.g., `'800ms'`, `'1.5s'`).
- `gpa:animation-repeat`: Number of cycles to run (e.g., `'1'`, `'3'`, or `'indefinite'` — default when `'automatic'` is set).
- `gpa:animation-easing`: `'linear'`, `'ease-in-out'`, `'ease-in'`, `'ease-out'`, or `'ease'`.
- `gpa:animation-segment`: Float between `0` and `1` (e.g., `'0.25'`, `'0.5'`) specifying the fraction of total path length drawn at any instant for `'segment'` and `'segment-alternate'` directions.

---

## 7. Path Attachments (`gpa:attach-to`)

A path can be attached to another path so that it moves and rotates along the parent path as the parent path animates!
- `gpa:attach-to`: The `id` of the parent `<path>`, `<circle>`, or `<rect>`.
- `gpa:attach-pos`: Float between `0` and `1` specifying where along the parent path (or its currently animated segment) this child path is anchored.

**Pro Tip — Invisible Motion Guides:**
You can create an invisible guide path (`stroke="none" fill="none" class="transparent-fill"`) with `gpa:animation-type="automatic"` and `gpa:animation-direction="alternate"`, and attach a visible shape (such as a bell body, arrow head, or indicator dot) to it via `gpa:attach-to="guide0" gpa:attach-pos="0"`.

---

## 8. Complete Examples

### Example A: Multi-State Plus ↔ Checkmark Transition (`animate` + `fade`)
```xml
<svg width='16' height='16'
     xmlns='http://www.w3.org/2000/svg'
     xmlns:gpa='https://www.gtk.org/grappa'
     gpa:state-names='plus check empty'
     gpa:state='0'
     gpa:version='1'
     gpa:keywords='add done animation checkmark plus new'>
  <circle cx='8' cy='8' r='7'
        id='path0'
        gpa:states='plus check'
        gpa:transition-type='fade'
        gpa:transition-duration='200ms'
        gpa:transition-delay='200ms'
        gpa:transition-easing='ease-in-out'
        stroke-width='2'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
  <path d='M 11 8 L 5 8 M 8 5 L 8 11'
        id='path1'
        gpa:states='plus'
        gpa:transition-type='animate'
        gpa:transition-duration='200ms'
        gpa:transition-easing='ease-in-out'
        gpa:origin='1'
        stroke-width='2'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
  <path d='M 5.5 8 L 7.5 10 L 10.5 6'
        id='path2'
        gpa:states='check'
        gpa:transition-type='animate'
        gpa:transition-duration='600ms'
        gpa:transition-easing='ease-out'
        stroke-width='2'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
</svg>
```

### Example B: Speaker Volume Levels with State Overlaps & Continuous Wave Animation
```xml
<svg width='16' height='16'
     xmlns='http://www.w3.org/2000/svg'
     xmlns:gpa='https://www.gtk.org/grappa'
     gpa:version='1'
     gpa:state-names='muted low high'
     gpa:state='2'
     gpa:keywords='sound audio volume high loud'>
  <path d='M 6 3 L 6 13 L 3 10 L 1 10 L 1 6 L 3 6 Z'
        id='speaker'
        stroke-width='2'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
  <path d='M 10.185 4.959 C 11.272 6.841, 11.272 9.159, 10.185 11.041'
        id='wave1-bg'
        gpa:transition-type='fade'
        gpa:transition-duration='200ms'
        stroke-width='2'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke-opacity='0.27'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
  <path d='M 13.579 3.0 C 15.365 6.094, 15.365 9.906, 13.579 13.0'
        id='wave2-bg'
        gpa:transition-type='fade'
        gpa:transition-duration='200ms'
        stroke-width='2'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke-opacity='0.27'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
  <path d='M 10.185 4.959 C 11.272 6.841, 11.272 9.159, 10.185 11.041'
        id='wave1-active'
        gpa:states='low high'
        gpa:transition-type='animate'
        gpa:transition-duration='400ms'
        gpa:transition-easing='ease-out'
        gpa:origin='0.5'
        stroke-width='2'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
  <path d='M 13.579 3.0 C 15.365 6.094, 15.365 9.906, 13.579 13.0'
        id='wave2-active'
        gpa:states='high'
        gpa:transition-type='animate'
        gpa:transition-duration='400ms'
        gpa:transition-easing='ease-out'
        gpa:origin='0.5'
        stroke-width='2'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
</svg>
```
