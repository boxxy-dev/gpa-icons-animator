use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

/// The complete bundled `GPA_SPECS.md` specification document for MCP prompts and UI reference.
pub const GPA_SPECS_MARKDOWN: &str = include_str!("../../../GPA_SPECS.md");

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<(svg|path|circle|rect)\b([^>]*?)/?>").expect("valid tag regex")
});

static SHAPE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<(path|circle|rect)\b([^>]*?)(/?>)").expect("valid shape tag regex")
});

static SHAPE_FULL_ELEMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<(path|circle|rect)\b([^>]*?)(?:/>|>\s*</(?:path|circle|rect)>)")
        .expect("valid full shape element regex")
});

static SVG_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<svg\b([^>]*?)>").expect("valid svg tag regex"));

static ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"([a-zA-Z0-9_:-]+)\s*=\s*(?:'([^']*)'|"([^"]*)")"#).expect("valid attr regex")
});

/// Automatic customization parameters that can be applied to any GPA / SVG icon.
#[derive(Debug, Clone, PartialEq)]
pub struct GpaAutoCustomize {
    pub fill_enabled: bool,
    pub stroke_width: f64,
    pub transition_type: String,
    pub transition_duration_ms: u32,
    pub transition_easing: String,
    pub animation_direction: String,
    pub animation_duration_ms: u32,
    pub animation_segment: f64,
    pub symbolic_color: String,
    pub rendering_mode: String,
}

impl Default for GpaAutoCustomize {
    fn default() -> Self {
        Self {
            fill_enabled: false,
            stroke_width: 2.0,
            transition_type: String::from("animate"),
            transition_duration_ms: 400,
            transition_easing: String::from("ease-in-out"),
            animation_direction: String::from("none"),
            animation_duration_ms: 800,
            animation_segment: 0.5,
            symbolic_color: String::from("foreground"),
            rendering_mode: String::from("svg"),
        }
    }
}

/// Represents a single state in a `.gpa` icon (0..63).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpaStateInfo {
    pub index: u32,
    pub name: String,
    pub visible_shape_count: usize,
}

/// Summary of a graphical primitive (`<path>`, `<circle>`, `<rect>`) inside a `.gpa` icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpaShapeInfo {
    pub id: String,
    pub kind: String,
    pub states_raw: String,
    pub state_mask: u64,
    pub stroke: String,
    pub fill: String,
    pub stroke_width: String,
    pub transition_type: String,
    pub transition_duration: String,
    pub transition_delay: String,
    pub transition_easing: String,
    pub origin: Option<String>,
    pub animation_type: String,
    pub animation_direction: String,
    pub animation_duration: String,
    pub animation_repeat: String,
    pub animation_segment: Option<String>,
    pub animation_easing: String,
    pub attach_to: Option<String>,
    pub attach_pos: Option<String>,
    pub d: Option<String>,
    pub cx: Option<String>,
    pub cy: Option<String>,
    pub r: Option<String>,
    pub x: Option<String>,
    pub y: Option<String>,
    pub width: Option<String>,
    pub height: Option<String>,
}

/// Surgical attribute patch for a single shape inside a `.gpa` document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpaShapePatch {
    pub new_id: Option<String>,
    pub d: Option<String>,
    pub cx: Option<String>,
    pub cy: Option<String>,
    pub r: Option<String>,
    pub x: Option<String>,
    pub y: Option<String>,
    pub width: Option<String>,
    pub height: Option<String>,
    pub states: Option<String>,
    pub stroke: Option<String>,
    pub fill: Option<String>,
    pub stroke_width: Option<String>,
    pub stroke_linecap: Option<String>,
    pub stroke_linejoin: Option<String>,
    pub stroke_opacity: Option<String>,
    pub fill_opacity: Option<String>,
    pub origin: Option<String>,
    pub transition_type: Option<String>,
    pub transition_duration: Option<String>,
    pub transition_delay: Option<String>,
    pub transition_easing: Option<String>,
    pub animation_type: Option<String>,
    pub animation_direction: Option<String>,
    pub animation_duration: Option<String>,
    pub animation_repeat: Option<String>,
    pub animation_segment: Option<String>,
    pub animation_easing: Option<String>,
    pub attach_to: Option<String>,
    pub attach_pos: Option<String>,
}

/// Parsed metadata, states, and shapes for a `.gpa` or symbolic `.svg` icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpaDocumentSummary {
    pub name: String,
    pub width: String,
    pub height: String,
    pub gpa_version: String,
    pub initial_state: u32,
    pub rendering: String,
    pub keywords: String,
    pub state_names: Vec<String>,
    pub states: Vec<GpaStateInfo>,
    pub shapes: Vec<GpaShapeInfo>,
    pub xml: String,
}

impl GpaDocumentSummary {
    /// Parses a `.gpa` or `.svg` XML string into a structured `GpaDocumentSummary`.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn parse(name: impl Into<String>, xml: impl Into<String>) -> Self {
        let name = name.into();
        let xml = xml.into();

        let mut width = String::from("16");
        let mut height = String::from("16");
        let mut gpa_version = String::from("1");
        let mut state_names: Vec<String> = Vec::new();
        let mut initial_state_raw = String::from("0");
        let mut rendering = String::from("svg");
        let mut keywords = String::new();

        let mut raw_shapes: Vec<(String, Vec<(String, String)>)> = Vec::new();

        for cap in TAG_RE.captures_iter(&xml) {
            let tag = &cap[1];
            let attrs_str = &cap[2];
            let attrs = parse_attrs(attrs_str);

            if tag == "svg" {
                if let Some(w) = get_attr(&attrs, "width") {
                    width = w.to_string();
                }
                if let Some(h) = get_attr(&attrs, "height") {
                    height = h.to_string();
                }
                if let Some(v) = get_attr(&attrs, "gpa:version") {
                    gpa_version = v.to_string();
                }
                if let Some(names) = get_attr(&attrs, "gpa:state-names") {
                    state_names = names
                        .split_whitespace()
                        .map(ToOwned::to_owned)
                        .collect();
                }
                if let Some(st) = get_attr(&attrs, "gpa:state") {
                    initial_state_raw = st.to_string();
                }
                if let Some(r) = get_attr(&attrs, "gpa:rendering") {
                    rendering = r.to_string();
                }
                if let Some(kw) = get_attr(&attrs, "gpa:keywords") {
                    keywords = kw.to_string();
                }
            } else {
                raw_shapes.push((tag.to_string(), attrs));
            }
        }

        let mut discovered_indices: BTreeSet<u32> = BTreeSet::new();
        discovered_indices.insert(0);
        for idx in 0..u32::try_from(state_names.len()).unwrap_or(0) {
            if idx < 64 {
                discovered_indices.insert(idx);
            }
        }

        let initial_state = resolve_state_token(&initial_state_raw, &state_names).unwrap_or(0);
        discovered_indices.insert(initial_state);

        let mut shapes = Vec::with_capacity(raw_shapes.len());
        for (idx, (kind, attrs)) in raw_shapes.into_iter().enumerate() {
            let id = get_attr(&attrs, "id")
                .map_or_else(|| format!("path{idx}"), ToOwned::to_owned);
            let states_raw = get_attr(&attrs, "gpa:states")
                .map_or_else(|| String::from("all"), ToOwned::to_owned);

            let (state_mask, explicit_indices) = parse_state_mask(&states_raw, &state_names);
            for st_idx in explicit_indices {
                discovered_indices.insert(st_idx);
            }

            let stroke = get_attr(&attrs, "gpa:stroke")
                .or_else(|| get_attr(&attrs, "stroke"))
                .unwrap_or("foreground")
                .to_string();
            let fill = get_attr(&attrs, "gpa:fill")
                .or_else(|| get_attr(&attrs, "fill"))
                .unwrap_or("none")
                .to_string();
            let stroke_width = get_attr(&attrs, "stroke-width")
                .or_else(|| get_attr(&attrs, "gpa:stroke-width"))
                .unwrap_or("2")
                .to_string();

            let transition_type = get_attr(&attrs, "gpa:transition-type")
                .unwrap_or("none")
                .to_string();
            let transition_duration = get_attr(&attrs, "gpa:transition-duration")
                .unwrap_or("0ms")
                .to_string();
            let transition_delay = get_attr(&attrs, "gpa:transition-delay")
                .unwrap_or("0ms")
                .to_string();
            let transition_easing = get_attr(&attrs, "gpa:transition-easing")
                .unwrap_or("linear")
                .to_string();
            let origin = get_attr(&attrs, "gpa:origin").map(ToOwned::to_owned);

            let animation_type = get_attr(&attrs, "gpa:animation-type")
                .unwrap_or("none")
                .to_string();
            let animation_direction = get_attr(&attrs, "gpa:animation-direction")
                .unwrap_or("normal")
                .to_string();
            let animation_duration = get_attr(&attrs, "gpa:animation-duration")
                .unwrap_or("0ms")
                .to_string();
            let animation_repeat = get_attr(&attrs, "gpa:animation-repeat")
                .unwrap_or("indefinite")
                .to_string();
            let animation_segment = get_attr(&attrs, "gpa:animation-segment").map(ToOwned::to_owned);
            let animation_easing = get_attr(&attrs, "gpa:animation-easing")
                .unwrap_or("linear")
                .to_string();

            let attach_to = get_attr(&attrs, "gpa:attach-to").map(ToOwned::to_owned);
            let attach_pos = get_attr(&attrs, "gpa:attach-pos").map(ToOwned::to_owned);

            let d = get_attr(&attrs, "d").map(ToOwned::to_owned);
            let cx = get_attr(&attrs, "cx").map(ToOwned::to_owned);
            let cy = get_attr(&attrs, "cy").map(ToOwned::to_owned);
            let r = get_attr(&attrs, "r").map(ToOwned::to_owned);
            let x = get_attr(&attrs, "x").map(ToOwned::to_owned);
            let y = get_attr(&attrs, "y").map(ToOwned::to_owned);
            let width = get_attr(&attrs, "width").map(ToOwned::to_owned);
            let height = get_attr(&attrs, "height").map(ToOwned::to_owned);

            shapes.push(GpaShapeInfo {
                id,
                kind,
                states_raw,
                state_mask,
                stroke,
                fill,
                stroke_width,
                transition_type,
                transition_duration,
                transition_delay,
                transition_easing,
                origin,
                animation_type,
                animation_direction,
                animation_duration,
                animation_repeat,
                animation_segment,
                animation_easing,
                attach_to,
                attach_pos,
                d,
                cx,
                cy,
                r,
                x,
                y,
                width,
                height,
            });
        }

        let states: Vec<GpaStateInfo> = discovered_indices
            .into_iter()
            .map(|index| {
                let name = state_names
                    .get(index as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("state-{index}"));
                let visible_shape_count = shapes
                    .iter()
                    .filter(|s| (s.state_mask & (1_u64 << index)) != 0)
                    .count();
                GpaStateInfo {
                    index,
                    name,
                    visible_shape_count,
                }
            })
            .collect();

        Self {
            name,
            width,
            height,
            gpa_version,
            initial_state,
            rendering,
            keywords,
            state_names,
            states,
            shapes,
            xml,
        }
    }

    /// Returns the number of shapes visible in `state`.
    #[must_use]
    pub fn visible_shapes_in_state(&self, state: u32) -> usize {
        if state >= 64 {
            return 0;
        }
        self.shapes
            .iter()
            .filter(|s| (s.state_mask & (1_u64 << state)) != 0)
            .count()
    }
}

/// Normalizes a standard symbolic SVG into a `.gpa`-ready XML string.
///
/// Ensures variable weight attributes (`gpa:stroke-minwidth` / `gpa:stroke-maxwidth`) and default
/// transition attributes are present on stroked shapes so `-gtk-icon-weight` and state transitions work automatically.
#[must_use]
pub fn ensure_gpa_headers(xml: &str) -> String {
    let mut current = if xml.contains("xmlns:gpa") {
        xml.to_string()
    } else if let Some(pos) = xml.find("<svg") {
        let insert_at = pos + 4;
        let mut out = String::with_capacity(xml.len() + 140);
        out.push_str(&xml[..insert_at]);
        out.push_str(
            "\n     xmlns:gpa='https://www.gtk.org/grappa'\n     gpa:version='1'\n     gpa:state-names='default active'\n     gpa:state='0'",
        );
        out.push_str(&xml[insert_at..]);
        out
    } else {
        xml.to_string()
    };

    // Ensure variable stroke-minwidth / stroke-maxwidth and default gpa:transition-type='animate'
    // on stroked elements so state transitions and variable weight work out of the box.
    current = SHAPE_TAG_RE
        .replace_all(&current, |caps: &Captures<'_>| {
            let tag = &caps[1];
            let attrs_str = &caps[2];
            let close = &caps[3];
            let attrs = parse_attrs(attrs_str);
            let has_stroke = get_attr(&attrs, "stroke").is_some_and(|s| s != "none")
                || get_attr(&attrs, "gpa:stroke").is_some_and(|s| s != "none");
            if !has_stroke {
                return caps[0].to_string();
            }

            let has_min = get_attr(&attrs, "gpa:stroke-minwidth").is_some()
                || get_attr(&attrs, "gpa:stroke-width").is_some();
            let has_trans = get_attr(&attrs, "gpa:transition-type").is_some();

            let mut extra = String::new();
            if !has_min {
                extra.push_str(" gpa:stroke-minwidth='25%' gpa:stroke-maxwidth='150%'");
            }
            if !has_trans {
                extra.push_str(
                    " gpa:transition-type='animate' gpa:transition-duration='400ms' gpa:transition-easing='ease-in-out'",
                );
            }

            if extra.is_empty() {
                caps[0].to_string()
            } else {
                format!("<{tag}{attrs_str}{extra}{close}")
            }
        })
        .into_owned();

    current
}

/// Applies automatic GPA customizations (`Fill`, `Stroke Width`, `Transition`, `Animation`,
/// `Symbolic Color`, `Rendering Mode`) to a `.gpa` XML string.
#[must_use]
pub fn apply_auto_customize(xml: &str, cfg: &GpaAutoCustomize) -> String {
    let base = ensure_gpa_headers(xml);

    // 1. Update root <svg> gpa:rendering attribute
    let with_svg = SVG_TAG_RE
        .replace(&base, |caps: &Captures<'_>| {
            let mut attrs = parse_attrs(&caps[1]);
            set_or_insert_attr(&mut attrs, "gpa:rendering", &cfg.rendering_mode);
            format!("<svg {}>", format_attrs(&attrs))
        })
        .into_owned();

    // 2. Update shape attributes
    SHAPE_TAG_RE
        .replace_all(&with_svg, |caps: &Captures<'_>| {
            let tag = &caps[1];
            let attrs_str = &caps[2];
            let close = &caps[3];
            let mut attrs = parse_attrs(attrs_str);

            // Skip invisible motion-guide paths
            let is_guide = get_attr(&attrs, "stroke") == Some("none")
                && get_attr(&attrs, "fill") == Some("none");
            if is_guide {
                return caps[0].to_string();
            }

            let sw_str = format!("{:.2}", cfg.stroke_width)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string();
            set_or_insert_attr(&mut attrs, "stroke-width", &sw_str);
            set_or_insert_attr(&mut attrs, "gpa:stroke-minwidth", "25%");
            set_or_insert_attr(&mut attrs, "gpa:stroke-maxwidth", "150%");

            if cfg.fill_enabled {
                set_or_insert_attr(&mut attrs, "gpa:fill", &cfg.symbolic_color);
                set_or_insert_attr(&mut attrs, "fill", "rgb(0,0,0)");
                set_or_insert_attr(&mut attrs, "gpa:stroke", &cfg.symbolic_color);
                set_or_insert_attr(
                    &mut attrs,
                    "class",
                    &format!("{}-fill {}-stroke", cfg.symbolic_color, cfg.symbolic_color),
                );
            } else {
                set_or_insert_attr(&mut attrs, "fill", "none");
                remove_attr(&mut attrs, "gpa:fill");
                set_or_insert_attr(&mut attrs, "stroke", "rgb(0,0,0)");
                set_or_insert_attr(&mut attrs, "gpa:stroke", &cfg.symbolic_color);
                set_or_insert_attr(
                    &mut attrs,
                    "class",
                    &format!("transparent-fill {}-stroke", cfg.symbolic_color),
                );
            }

            // Transition attributes
            set_or_insert_attr(&mut attrs, "gpa:transition-type", &cfg.transition_type);
            if cfg.transition_type != "none" {
                set_or_insert_attr(
                    &mut attrs,
                    "gpa:transition-duration",
                    &format!("{}ms", cfg.transition_duration_ms),
                );
                set_or_insert_attr(&mut attrs, "gpa:transition-easing", &cfg.transition_easing);
            }

            // Continuous animation attributes
            if cfg.animation_direction == "none" {
                set_or_insert_attr(&mut attrs, "gpa:animation-type", "none");
                remove_attr(&mut attrs, "gpa:animation-direction");
            } else {
                set_or_insert_attr(&mut attrs, "gpa:animation-type", "automatic");
                set_or_insert_attr(
                    &mut attrs,
                    "gpa:animation-direction",
                    &cfg.animation_direction,
                );
                set_or_insert_attr(
                    &mut attrs,
                    "gpa:animation-duration",
                    &format!("{}ms", cfg.animation_duration_ms),
                );
                set_or_insert_attr(&mut attrs, "gpa:animation-repeat", "indefinite");
                let seg_str = format!("{:.2}", cfg.animation_segment);
                set_or_insert_attr(&mut attrs, "gpa:animation-segment", &seg_str);
            }

            format!("<{tag} {}{close}", format_attrs(&attrs))
        })
        .into_owned()
}

/// Replaces `gpa:state-names` on the root `<svg>` element with a custom space-separated list
/// of state names (e.g. `"idle hover active"`).
#[must_use]
pub fn set_all_states_in_gpa(doc: &GpaDocumentSummary, raw_names: &str) -> String {
    let valid_tokens: Vec<String> = raw_names
        .split_whitespace()
        .filter(|s| !s.is_empty() && *s != "all" && *s != "none")
        .take(64)
        .map(|tok| {
            if tok.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                format!("s-{tok}")
            } else {
                tok.to_string()
            }
        })
        .collect();

    if valid_tokens.is_empty() {
        return doc.xml.clone();
    }
    let joined = valid_tokens.join(" ");
    let base = ensure_gpa_headers(&doc.xml);

    SVG_TAG_RE
        .replace(&base, |caps: &Captures<'_>| {
            let mut attrs = parse_attrs(&caps[1]);
            set_or_insert_attr(&mut attrs, "gpa:state-names", &joined);
            set_or_insert_attr(&mut attrs, "gpa:state", "0");
            format!("<svg {}>", format_attrs(&attrs))
        })
        .into_owned()
}

/// Appends a new state to `gpa:state-names` on the root `<svg>` element.
#[must_use]
pub fn add_state_to_gpa(doc: &GpaDocumentSummary, custom_name: Option<&str>) -> String {
    let base = ensure_gpa_headers(&doc.xml);
    let mut names: Vec<String> = if doc.state_names.is_empty() {
        doc.states.iter().map(|s| s.name.clone()).collect()
    } else {
        doc.state_names.clone()
    };
    if names.is_empty() {
        names.push(String::from("state-0"));
    }

    let next_idx = names.len();
    if next_idx >= 64 {
        return base;
    }

    let raw_candidate = custom_name
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "all" && *s != "none")
        .map_or_else(|| format!("state-{next_idx}"), ToOwned::to_owned);

    // State names in GPA cannot start with an ASCII digit
    let valid_name = if raw_candidate.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("s-{raw_candidate}")
    } else {
        raw_candidate.replace(' ', "-")
    };

    names.push(valid_name);
    let joined = names.join(" ");

    SVG_TAG_RE
        .replace(&base, |caps: &Captures<'_>| {
            let mut attrs = parse_attrs(&caps[1]);
            set_or_insert_attr(&mut attrs, "gpa:state-names", &joined);
            format!("<svg {}>", format_attrs(&attrs))
        })
        .into_owned()
}

/// Removes a state by index (`target_idx`) from `gpa:state-names` (if more than 1 state exists).
#[must_use]
pub fn remove_state_from_gpa(doc: &GpaDocumentSummary, target_idx: u32) -> String {
    if doc.states.len() <= 1 {
        return doc.xml.clone();
    }
    let base = ensure_gpa_headers(&doc.xml);
    let mut names: Vec<String> = doc.states.iter().map(|s| s.name.clone()).collect();
    if (target_idx as usize) < names.len() {
        names.remove(target_idx as usize);
    }
    let joined = names.join(" ");

    SVG_TAG_RE
        .replace(&base, |caps: &Captures<'_>| {
            let mut attrs = parse_attrs(&caps[1]);
            set_or_insert_attr(&mut attrs, "gpa:state-names", &joined);
            set_or_insert_attr(&mut attrs, "gpa:state", "0");
            format!("<svg {}>", format_attrs(&attrs))
        })
        .into_owned()
}

/// Renames state `target_idx` in `gpa:state-names` and updates any matching token in `gpa:states`.
#[must_use]
pub fn rename_state_in_gpa(doc: &GpaDocumentSummary, target_idx: u32, raw_new_name: &str) -> String {
    let trimmed = raw_new_name.trim();
    if trimmed.is_empty() || trimmed == "all" || trimmed == "none" {
        return doc.xml.clone();
    }
    let valid_name = if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("s-{trimmed}")
    } else {
        trimmed.replace(' ', "-")
    };

    let mut names: Vec<String> = doc.states.iter().map(|s| s.name.clone()).collect();
    let Some(old_name) = names.get(target_idx as usize).cloned() else {
        return doc.xml.clone();
    };
    if old_name == valid_name {
        return doc.xml.clone();
    }
    names[target_idx as usize].clone_from(&valid_name);
    let joined = names.join(" ");

    let base = ensure_gpa_headers(&doc.xml);
    let with_svg = SVG_TAG_RE
        .replace(&base, |caps: &Captures<'_>| {
            let mut attrs = parse_attrs(&caps[1]);
            set_or_insert_attr(&mut attrs, "gpa:state-names", &joined);
            format!("<svg {}>", format_attrs(&attrs))
        })
        .into_owned();

    SHAPE_TAG_RE
        .replace_all(&with_svg, |caps: &Captures<'_>| {
            let tag = &caps[1];
            let attrs_str = &caps[2];
            let close = &caps[3];
            let mut attrs = parse_attrs(attrs_str);
            if let Some(states_val) = get_attr(&attrs, "gpa:states").map(ToOwned::to_owned) {
                let updated_tokens: Vec<String> = states_val
                    .split_whitespace()
                    .map(|tok| {
                        if tok == old_name {
                            valid_name.clone()
                        } else {
                            tok.to_string()
                        }
                    })
                    .collect();
                set_or_insert_attr(&mut attrs, "gpa:states", &updated_tokens.join(" "));
            }
            format!("<{tag} {}{close}", format_attrs(&attrs))
        })
        .into_owned()
}

/// Validates that a candidate `.gpa` / `.svg` XML string has a valid `<svg ...>...</svg>` root.
///
/// # Errors
/// Returns an error string describing why the XML is malformed if it lacks `<svg` or `</svg>`.
pub fn validate_gpa_xml(xml: &str) -> Result<(), String> {
    let trimmed = xml.trim();
    if !trimmed.contains("<svg") {
        return Err(String::from("Missing root <svg> opening tag"));
    }
    if !trimmed.contains("</svg>") {
        return Err(String::from("Missing closing </svg> tag"));
    }
    Ok(())
}

/// Surgically updates attributes on a single shape (`<path>`, `<circle>`, `<rect>`) identified
/// by `shape_id` (matching either its `id` attribute or its fallback index id `path{idx}`).
///
/// # Errors
/// Returns an error if no shape matching `shape_id` exists in the document.
pub fn patch_shape_in_gpa(
    xml: &str,
    shape_id: &str,
    patch: &GpaShapePatch,
) -> Result<String, String> {
    let base = ensure_gpa_headers(xml);
    let mut found = false;
    let mut shape_idx = 0_usize;

    let updated = SHAPE_TAG_RE
        .replace_all(&base, |caps: &Captures<'_>| {
            let current_idx = shape_idx;
            shape_idx += 1;

            let tag = &caps[1];
            let attrs_str = &caps[2];
            let close = &caps[3];
            let mut attrs = parse_attrs(attrs_str);

            let effective_id = get_attr(&attrs, "id")
                .map_or_else(|| format!("path{current_idx}"), ToOwned::to_owned);
            if effective_id != shape_id {
                return caps[0].to_string();
            }

            found = true;
            let final_id = patch.new_id.as_deref().unwrap_or(&effective_id);
            set_or_insert_attr(&mut attrs, "id", final_id);
            apply_shape_patch_to_attrs(&mut attrs, patch);

            format!("<{tag} {}{close}", format_attrs(&attrs))
        })
        .into_owned();

    if !found {
        return Err(format!("Shape '{shape_id}' not found in active GPA icon"));
    }
    Ok(updated)
}

/// Inserts a new shape (`<path>`, `<circle>`, or `<rect>`) or updates an existing shape if `shape_id`
/// already exists in the document.
///
/// # Errors
/// Returns an error if `kind` is not one of `"path"`, `"circle"`, or `"rect"`, or if `<svg>` is missing.
pub fn upsert_shape_in_gpa(
    xml: &str,
    shape_id: &str,
    kind: &str,
    patch: &GpaShapePatch,
) -> Result<String, String> {
    let normalized_kind = kind.trim().to_ascii_lowercase();
    if !matches!(normalized_kind.as_str(), "path" | "circle" | "rect") {
        return Err(format!(
            "Unsupported shape kind '{kind}' (expected 'path', 'circle', or 'rect')"
        ));
    }

    if let Ok(updated) = patch_shape_in_gpa(xml, shape_id, patch) {
        return Ok(updated);
    }

    let base = ensure_gpa_headers(xml);
    let Some(close_pos) = base.rfind("</svg>") else {
        return Err(String::from("Missing closing </svg> tag in document"));
    };

    let mut attrs: Vec<(String, String)> = vec![
        (
            String::from("id"),
            patch.new_id.as_deref().unwrap_or(shape_id).to_string(),
        ),
        (String::from("fill"), String::from("none")),
        (String::from("stroke"), String::from("rgb(0,0,0)")),
        (String::from("gpa:stroke"), String::from("foreground")),
        (String::from("stroke-width"), String::from("2")),
        (String::from("gpa:stroke-minwidth"), String::from("25%")),
        (String::from("gpa:stroke-maxwidth"), String::from("150%")),
        (String::from("stroke-linecap"), String::from("round")),
        (String::from("stroke-linejoin"), String::from("round")),
        (
            String::from("class"),
            String::from("foreground-stroke transparent-fill"),
        ),
    ];
    apply_shape_patch_to_attrs(&mut attrs, patch);

    let new_element = format!("  <{normalized_kind} {}/>\n", format_attrs(&attrs));
    let mut out = String::with_capacity(base.len() + new_element.len() + 4);
    out.push_str(&base[..close_pos]);
    out.push_str(&new_element);
    out.push_str(&base[close_pos..]);
    Ok(out)
}

/// Clones an existing shape `source_id` into a new shape `new_id` (optionally applying `patch`),
/// useful for creating state-specific variants or morph targets.
///
/// # Errors
/// Returns an error if `source_id` is not found in the document.
pub fn duplicate_shape_in_gpa(
    xml: &str,
    source_id: &str,
    new_id: &str,
    patch: &GpaShapePatch,
) -> Result<String, String> {
    let base = ensure_gpa_headers(xml);
    let mut cloned_element: Option<String> = None;

    for (shape_idx, cap) in SHAPE_TAG_RE.captures_iter(&base).enumerate() {
        let tag = &cap[1];
        let attrs_str = &cap[2];
        let mut attrs = parse_attrs(attrs_str);
        let effective_id = get_attr(&attrs, "id")
            .map_or_else(|| format!("path{shape_idx}"), ToOwned::to_owned);

        if effective_id == source_id {
            set_or_insert_attr(&mut attrs, "id", new_id);
            apply_shape_patch_to_attrs(&mut attrs, patch);
            cloned_element = Some(format!("  <{tag} {}/>\n", format_attrs(&attrs)));
            break;
        }
    }

    let Some(element_str) = cloned_element else {
        return Err(format!("Source shape '{source_id}' not found"));
    };
    let Some(close_pos) = base.rfind("</svg>") else {
        return Err(String::from("Missing closing </svg> tag in document"));
    };

    let mut out = String::with_capacity(base.len() + element_str.len() + 4);
    out.push_str(&base[..close_pos]);
    out.push_str(&element_str);
    out.push_str(&base[close_pos..]);
    Ok(out)
}

/// Removes a shape by `shape_id` from the `.gpa` document.
///
/// # Errors
/// Returns an error if `shape_id` is not found.
pub fn delete_shape_from_gpa(xml: &str, shape_id: &str) -> Result<String, String> {
    let base = ensure_gpa_headers(xml);
    let mut found = false;
    let mut shape_idx = 0_usize;

    let updated = SHAPE_FULL_ELEMENT_RE
        .replace_all(&base, |caps: &Captures<'_>| {
            let current_idx = shape_idx;
            shape_idx += 1;

            let attrs = parse_attrs(&caps[2]);
            let effective_id = get_attr(&attrs, "id")
                .map_or_else(|| format!("path{current_idx}"), ToOwned::to_owned);
            if effective_id == shape_id {
                found = true;
                String::new()
            } else {
                caps[0].to_string()
            }
        })
        .into_owned();

    if !found {
        return Err(format!("Shape '{shape_id}' not found in active GPA icon"));
    }
    Ok(updated)
}

#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn apply_shape_patch_to_attrs(attrs: &mut Vec<(String, String)>, patch: &GpaShapePatch) {
    if let Some(d) = &patch.d {
        set_or_insert_attr(attrs, "d", d);
    }
    if let Some(cx) = &patch.cx {
        set_or_insert_attr(attrs, "cx", cx);
    }
    if let Some(cy) = &patch.cy {
        set_or_insert_attr(attrs, "cy", cy);
    }
    if let Some(r) = &patch.r {
        set_or_insert_attr(attrs, "r", r);
    }
    if let Some(x) = &patch.x {
        set_or_insert_attr(attrs, "x", x);
    }
    if let Some(y) = &patch.y {
        set_or_insert_attr(attrs, "y", y);
    }
    if let Some(w) = &patch.width {
        set_or_insert_attr(attrs, "width", w);
    }
    if let Some(h) = &patch.height {
        set_or_insert_attr(attrs, "height", h);
    }
    if let Some(states) = &patch.states {
        if states == "all" {
            remove_attr(attrs, "gpa:states");
        } else {
            set_or_insert_attr(attrs, "gpa:states", states);
        }
    }
    if let Some(sw) = &patch.stroke_width {
        set_or_insert_attr(attrs, "stroke-width", sw);
    }
    if let Some(lc) = &patch.stroke_linecap {
        set_or_insert_attr(attrs, "stroke-linecap", lc);
    }
    if let Some(lj) = &patch.stroke_linejoin {
        set_or_insert_attr(attrs, "stroke-linejoin", lj);
    }
    if let Some(so) = &patch.stroke_opacity {
        set_or_insert_attr(attrs, "stroke-opacity", so);
    }
    if let Some(fo) = &patch.fill_opacity {
        set_or_insert_attr(attrs, "fill-opacity", fo);
    }
    if let Some(stroke) = &patch.stroke {
        if stroke == "none" {
            set_or_insert_attr(attrs, "stroke", "none");
            remove_attr(attrs, "gpa:stroke");
        } else {
            set_or_insert_attr(attrs, "stroke", "rgb(0,0,0)");
            set_or_insert_attr(attrs, "gpa:stroke", stroke);
        }
    }
    if let Some(fill) = &patch.fill {
        if fill == "none" {
            set_or_insert_attr(attrs, "fill", "none");
            remove_attr(attrs, "gpa:fill");
        } else {
            set_or_insert_attr(attrs, "fill", "rgb(0,0,0)");
            set_or_insert_attr(attrs, "gpa:fill", fill);
        }
    }
    if let Some(origin) = &patch.origin {
        if origin.is_empty() {
            remove_attr(attrs, "gpa:origin");
        } else {
            set_or_insert_attr(attrs, "gpa:origin", origin);
        }
    }
    if let Some(tt) = &patch.transition_type {
        set_or_insert_attr(attrs, "gpa:transition-type", tt);
    }
    if let Some(td) = &patch.transition_duration {
        set_or_insert_attr(attrs, "gpa:transition-duration", td);
    }
    if let Some(tdel) = &patch.transition_delay {
        if tdel == "0ms" || tdel.is_empty() {
            remove_attr(attrs, "gpa:transition-delay");
        } else {
            set_or_insert_attr(attrs, "gpa:transition-delay", tdel);
        }
    }
    if let Some(te) = &patch.transition_easing {
        set_or_insert_attr(attrs, "gpa:transition-easing", te);
    }
    if let Some(at) = &patch.animation_type {
        set_or_insert_attr(attrs, "gpa:animation-type", at);
    }
    if let Some(adir) = &patch.animation_direction {
        if adir == "none" || adir.is_empty() {
            remove_attr(attrs, "gpa:animation-direction");
            set_or_insert_attr(attrs, "gpa:animation-type", "none");
        } else {
            set_or_insert_attr(attrs, "gpa:animation-type", "automatic");
            set_or_insert_attr(attrs, "gpa:animation-direction", adir);
        }
    }
    if let Some(adur) = &patch.animation_duration {
        set_or_insert_attr(attrs, "gpa:animation-duration", adur);
    }
    if let Some(arep) = &patch.animation_repeat {
        set_or_insert_attr(attrs, "gpa:animation-repeat", arep);
    }
    if let Some(aseg) = &patch.animation_segment {
        set_or_insert_attr(attrs, "gpa:animation-segment", aseg);
    }
    if let Some(aeas) = &patch.animation_easing {
        set_or_insert_attr(attrs, "gpa:animation-easing", aeas);
    }
    if let Some(att) = &patch.attach_to {
        if att.is_empty() {
            remove_attr(attrs, "gpa:attach-to");
        } else {
            set_or_insert_attr(attrs, "gpa:attach-to", att);
        }
    }
    if let Some(apos) = &patch.attach_pos {
        if apos.is_empty() {
            remove_attr(attrs, "gpa:attach-pos");
        } else {
            set_or_insert_attr(attrs, "gpa:attach-pos", apos);
        }
    }
}

/// Returns a clean blank 16×16 `.gpa` template with two sample states (`default` and `active`).
#[must_use]
pub fn default_blank_gpa(name: &str) -> String {
    format!(
        r"<svg width='16' height='16'
     xmlns='http://www.w3.org/2000/svg'
     xmlns:gpa='https://www.gtk.org/grappa'
     gpa:version='1'
     gpa:state-names='default active'
     gpa:state='0'
     gpa:keywords='{name} symbolic'>
  <circle cx='8' cy='8' r='6'
        id='path0'
        gpa:states='default active'
        gpa:transition-type='animate'
        gpa:transition-duration='350ms'
        gpa:transition-easing='ease-in-out'
        stroke-width='2'
        gpa:stroke-minwidth='25%'
        gpa:stroke-maxwidth='150%'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
  <path d='M 5.5 8 L 7.5 10 L 10.5 6'
        id='path1'
        gpa:states='active'
        gpa:transition-type='animate'
        gpa:transition-duration='350ms'
        gpa:transition-easing='ease-out'
        gpa:origin='0'
        stroke-width='2'
        gpa:stroke-minwidth='25%'
        gpa:stroke-maxwidth='150%'
        stroke-linecap='round'
        stroke-linejoin='round'
        stroke='rgb(0,0,0)'
        fill='none'
        class='foreground-stroke transparent-fill'
        gpa:stroke='foreground'/>
</svg>
"
    )
}

/// Generates an X-Ray wireframe `.gpa` XML string with 1px red centerlines.
///
/// Every shape is rendered with `rgb(237,51,59)` and `stroke-width='0.14'` while preserving all GPA
/// states, transitions, and continuous animations.
#[must_use]
pub fn build_xray_gpa_xml(xml: &str) -> String {
    let base = ensure_gpa_headers(xml);
    let with_svg = SVG_TAG_RE
        .replace(&base, |caps: &Captures<'_>| {
            let mut attrs = parse_attrs(&caps[1]);
            set_or_insert_attr(&mut attrs, "gpa:rendering", "svg");
            format!("<svg {}>", format_attrs(&attrs))
        })
        .into_owned();

    SHAPE_TAG_RE
        .replace_all(&with_svg, |caps: &Captures<'_>| {
            let tag = &caps[1];
            let attrs_str = &caps[2];
            let close = &caps[3];
            let mut attrs = parse_attrs(attrs_str);

            set_or_insert_attr(&mut attrs, "fill", "none");
            remove_attr(&mut attrs, "gpa:fill");
            set_or_insert_attr(&mut attrs, "stroke", "rgb(237,51,59)");
            set_or_insert_attr(&mut attrs, "gpa:stroke", "rgb(237,51,59)");
            set_or_insert_attr(&mut attrs, "stroke-width", "0.14");
            set_or_insert_attr(&mut attrs, "gpa:stroke-minwidth", "0.14");
            set_or_insert_attr(&mut attrs, "gpa:stroke-maxwidth", "0.14");
            remove_attr(&mut attrs, "gpa:stroke-width");
            set_or_insert_attr(&mut attrs, "stroke-opacity", "1");
            set_or_insert_attr(&mut attrs, "class", "error-stroke transparent-fill");

            format!("<{tag} {}{close}", format_attrs(&attrs))
        })
        .into_owned()
}

/// Rewrites a `.gpa` XML string so its root `gpa:state` starts directly at `state_idx`.
///
/// When `freeze_animations` is `true`, strips continuous `gpa:animation-*` and `gpa:transition-*`
/// attributes so `GtkSvg` renders a 100% still, complete snapshot of `state_idx`.
#[must_use]
pub fn with_initial_state_in_gpa(xml: &str, state_idx: u32, freeze_animations: bool) -> String {
    let base = ensure_gpa_headers(xml);
    let state_str = state_idx.to_string();

    let with_svg = SVG_TAG_RE
        .replace(&base, |caps: &Captures<'_>| {
            let mut attrs = parse_attrs(&caps[1]);
            set_or_insert_attr(&mut attrs, "gpa:state", &state_str);
            format!("<svg {}>", format_attrs(&attrs))
        })
        .into_owned();

    if !freeze_animations {
        return with_svg;
    }

    SHAPE_TAG_RE
        .replace_all(&with_svg, |caps: &Captures<'_>| {
            let tag = &caps[1];
            let attrs_str = &caps[2];
            let close = &caps[3];
            let mut attrs = parse_attrs(attrs_str);

            set_or_insert_attr(&mut attrs, "gpa:transition-type", "none");
            remove_attr(&mut attrs, "gpa:transition-duration");
            remove_attr(&mut attrs, "gpa:transition-delay");
            remove_attr(&mut attrs, "gpa:animation-type");
            remove_attr(&mut attrs, "gpa:animation-direction");
            remove_attr(&mut attrs, "gpa:animation-duration");
            remove_attr(&mut attrs, "gpa:animation-repeat");
            remove_attr(&mut attrs, "gpa:animation-segment");

            format!("<{tag} {}{close}", format_attrs(&attrs))
        })
        .into_owned()
}

/// Returns a completely blank 16×16 `.gpa` canvas with 0 shapes and two initial states
/// (`default` and `active`), ready for an MCP AI agent or user to build an icon from scratch.
#[must_use]
pub fn empty_scratch_gpa(name: &str) -> String {
    format!(
        r"<svg width='16' height='16'
     xmlns='http://www.w3.org/2000/svg'
     xmlns:gpa='https://www.gtk.org/grappa'
     gpa:version='1'
     gpa:state-names='default active'
     gpa:state='0'
     gpa:keywords='{name} symbolic'>
</svg>
"
    )
}

fn parse_attrs(raw: &str) -> Vec<(String, String)> {
    ATTR_RE
        .captures_iter(raw)
        .map(|cap| {
            let key = cap[1].to_string();
            let val = cap
                .get(2)
                .or_else(|| cap.get(3))
                .map_or("", |m| m.as_str())
                .to_string();
            (key, val)
        })
        .collect()
}

fn get_attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn set_or_insert_attr(attrs: &mut Vec<(String, String)>, key: &str, value: &str) {
    if let Some((_, v)) = attrs.iter_mut().find(|(k, _)| k == key) {
        *v = value.to_string();
    } else {
        attrs.push((key.to_string(), value.to_string()));
    }
}

fn remove_attr(attrs: &mut Vec<(String, String)>, key: &str) {
    attrs.retain(|(k, _)| k != key);
}

fn format_attrs(attrs: &[(String, String)]) -> String {
    attrs
        .iter()
        .map(|(k, v)| format!("{k}='{v}'"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve_state_token(token: &str, state_names: &[String]) -> Option<u32> {
    if let Ok(idx) = token.parse::<u32>()
        && idx < 64
    {
        return Some(idx);
    }
    state_names
        .iter()
        .position(|n| n == token)
        .and_then(|pos| u32::try_from(pos).ok())
        .filter(|&idx| idx < 64)
}

fn parse_state_mask(states_raw: &str, state_names: &[String]) -> (u64, Vec<u32>) {
    let trimmed = states_raw.trim();
    if trimmed.is_empty() || trimmed == "all" {
        return (u64::MAX, Vec::new());
    }
    if trimmed == "none" {
        return (0, Vec::new());
    }

    let mut mask = 0_u64;
    let mut indices = Vec::new();
    for token in trimmed.split_whitespace() {
        if let Some(idx) = resolve_state_token(token, state_names) {
            mask |= 1_u64 << idx;
            indices.push(idx);
        }
    }
    (mask, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_states_and_shapes() {
        let xml = default_blank_gpa("test-icon");
        let summary = GpaDocumentSummary::parse("test-icon.gpa", xml);
        assert_eq!(summary.states.len(), 2);
        assert_eq!(summary.states[0].name, "default");
        assert_eq!(summary.states[1].name, "active");
        assert_eq!(summary.shapes.len(), 2);
        assert_eq!(summary.visible_shapes_in_state(0), 1);
        assert_eq!(summary.visible_shapes_in_state(1), 2);
    }

    #[test]
    fn auto_customizes_gpa_attributes() {
        let xml = default_blank_gpa("custom-test");
        let cfg = GpaAutoCustomize {
            fill_enabled: true,
            stroke_width: 1.5,
            transition_type: String::from("morph"),
            transition_duration_ms: 550,
            transition_easing: String::from("ease-out"),
            animation_direction: String::from("segment-alternate"),
            animation_duration_ms: 900,
            animation_segment: 0.4,
            symbolic_color: String::from("accent"),
            rendering_mode: String::from("symbolic"),
        };
        let updated = apply_auto_customize(&xml, &cfg);
        let summary = GpaDocumentSummary::parse("custom-test.gpa", updated);
        assert_eq!(summary.rendering, "symbolic");
        assert_eq!(summary.shapes[0].transition_type, "morph");
        assert_eq!(summary.shapes[0].animation_direction, "segment-alternate");
        assert_eq!(summary.shapes[0].stroke, "accent");
    }

    #[test]
    fn surgical_shape_patch_and_lifecycle() {
        let box_xml = r#"<svg width="16" height="16" xmlns="http://www.w3.org/2000/svg" xmlns:gpa="https://www.gtk.org/grappa" gpa:version="1" gpa:state="0" gpa:state-names="outline empty">
  <path id="border" fill="none" stroke="rgb(0,0,0)" gpa:stroke="foreground" stroke-width="2" d="M 1 6 L 15 6 Z" gpa:states="outline">
  </path>
  <path id="split" fill="none" stroke="rgb(0,0,0)" gpa:stroke="foreground" stroke-width="1" d="M 8 8 L 8 15" gpa:states="outline">
  </path>
</svg>"#;

        let patch = GpaShapePatch {
            transition_type: Some(String::from("animate")),
            transition_duration: Some(String::from("450ms")),
            transition_delay: Some(String::from("120ms")),
            origin: Some(String::from("0.5")),
            animation_direction: Some(String::from("segment-alternate")),
            animation_duration: Some(String::from("900ms")),
            animation_segment: Some(String::from("0.35")),
            ..Default::default()
        };
        let patched = patch_shape_in_gpa(box_xml, "split", &patch).expect("patch split");
        let summary = GpaDocumentSummary::parse("3d-box.svg", &patched);
        assert_eq!(summary.shapes.len(), 2);
        assert_eq!(summary.shapes[0].d.as_deref(), Some("M 1 6 L 15 6 Z"));
        assert_eq!(summary.shapes[1].id, "split");
        assert_eq!(summary.shapes[1].transition_type, "animate");
        assert_eq!(summary.shapes[1].transition_duration, "450ms");
        assert_eq!(summary.shapes[1].transition_delay, "120ms");
        assert_eq!(summary.shapes[1].origin.as_deref(), Some("0.5"));
        assert_eq!(summary.shapes[1].animation_direction, "segment-alternate");
        assert_eq!(summary.shapes[1].animation_segment.as_deref(), Some("0.35"));

        // Duplicate 'border' for state 'empty'
        let dup_patch = GpaShapePatch {
            states: Some(String::from("empty")),
            stroke: Some(String::from("accent")),
            ..Default::default()
        };
        let with_dup =
            duplicate_shape_in_gpa(&patched, "border", "border-alt", &dup_patch).expect("dup");
        let summary_dup = GpaDocumentSummary::parse("3d-box.svg", &with_dup);
        assert_eq!(summary_dup.shapes.len(), 3);
        assert_eq!(summary_dup.shapes[2].id, "border-alt");
        assert_eq!(summary_dup.shapes[2].stroke, "accent");
        assert_eq!(summary_dup.visible_shapes_in_state(1), 1);

        // Delete 'border-alt'
        let after_del = delete_shape_from_gpa(&with_dup, "border-alt").expect("delete");
        let summary_del = GpaDocumentSummary::parse("3d-box.svg", after_del);
        assert_eq!(summary_del.shapes.len(), 2);
    }
}
