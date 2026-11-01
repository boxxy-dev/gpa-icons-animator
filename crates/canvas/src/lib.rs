use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpa_core::{
    GpaAutoCustomize, GpaDocumentSummary, add_state_to_gpa, apply_auto_customize,
    build_xray_gpa_xml, remove_state_from_gpa, rename_state_in_gpa, with_initial_state_in_gpa,
};
use gpa_icon_library::all_icons;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;

type StateChangedCallback = Rc<RefCell<Option<Box<dyn Fn(u32)>>>>;
type DocMutatedCallback = Rc<RefCell<Option<Box<dyn Fn(GpaDocumentSummary)>>>>;

const TRANSITION_TYPES: [&str; 4] = ["animate", "morph", "fade", "none"];
const ANIMATION_DIRS: [&str; 7] = [
    "none",
    "normal",
    "alternate",
    "in-out",
    "in-out-alternate",
    "segment",
    "segment-alternate",
];
const SYMBOLIC_COLORS: [&str; 5] = ["foreground", "accent", "success", "warning", "error"];
const RENDERING_MODES: [&str; 2] = ["svg", "symbolic"];

/// Compact, discrete Studio Canvas stacked vertically so minimum width is ~300px:
/// 1. Compact Showcase Card (Icon preview + title + circular transport buttons)
/// 2. Compact State Preview Boxes (`0..N`) with inline editable names
/// 3. Discrete Customize Card below Showcase/States
/// 4. Scrollable Context Scale Preview at the bottom
#[derive(Clone)]
pub struct StudioCanvas {
    root: gtk4::ScrolledWindow,
    hero_frame: gtk4::Box,
    grid_area: gtk4::DrawingArea,
    hero_svg: Rc<RefCell<gtk4::Svg>>,
    xray_svg: Rc<RefCell<gtk4::Svg>>,
    xray_image: gtk4::Image,
    title_label: gtk4::Label,
    subtitle_label: gtk4::Label,
    active_state_pill: gtk4::Label,
    play_pause_btn: gtk4::ToggleButton,
    state_boxes_row: gtk4::FlowBox,
    state_card_buttons: Rc<RefCell<Vec<(u32, gtk4::Widget)>>>,
    context_grid_frame: gtk4::Box,
    context_svgs: Rc<RefCell<Vec<gtk4::Svg>>>,
    active_clone_svgs: Rc<RefCell<Vec<gtk4::Svg>>>,
    state_box_svgs: Rc<RefCell<Vec<gtk4::Svg>>>,
    state_preview_wells: Rc<RefCell<Vec<gtk4::Box>>>,
    current_doc: Rc<RefCell<GpaDocumentSummary>>,
    base_xml: Rc<RefCell<String>>,
    current_state: Rc<Cell<u32>>,
    playing: Rc<Cell<bool>>,
    transition_speed_ms: Rc<Cell<u32>>,
    playback_timer: Rc<RefCell<Option<glib::SourceId>>>,
    weight: Rc<Cell<f64>>,
    show_grid: Rc<Cell<bool>>,
    show_xray: Rc<Cell<bool>>,
    light_canvas: Rc<Cell<bool>>,
    on_state_changed: StateChangedCallback,
    on_doc_mutated: DocMutatedCallback,
}

impl StudioCanvas {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(initial_doc: GpaDocumentSummary) -> Self {
        let current_state = Rc::new(Cell::new(initial_doc.initial_state));
        let playing = Rc::new(Cell::new(false));
        let transition_speed_ms = Rc::new(Cell::new(400));
        let playback_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let weight = Rc::new(Cell::new(400.0));
        let show_grid = Rc::new(Cell::new(true));
        let show_xray = Rc::new(Cell::new(false));
        let light_canvas = Rc::new(Cell::new(false));
        let base_xml = Rc::new(RefCell::new(initial_doc.xml.clone()));
        let current_doc = Rc::new(RefCell::new(initial_doc));
        let on_state_changed: StateChangedCallback = Rc::new(RefCell::new(None));
        let on_doc_mutated: DocMutatedCallback = Rc::new(RefCell::new(None));
        let suppress_customize = Rc::new(Cell::new(false));

        let outer_box = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
        outer_box.set_valign(gtk4::Align::Start);
        outer_box.set_vexpand(false);
        outer_box.set_margin_top(14);
        outer_box.set_margin_bottom(20);
        outer_box.set_margin_start(14);
        outer_box.set_margin_end(14);

        // =========================================================================
        // 1. COMPACT SHOWCASE BANNER (Discrete ~300px min-width)
        // =========================================================================
        let hero_banner = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
        hero_banner.set_valign(gtk4::Align::Start);
        hero_banner.set_vexpand(false);
        hero_banner.add_css_class("gpa-hero-banner");

        let hero_frame = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        hero_frame.set_size_request(144, 144);
        hero_frame.set_halign(gtk4::Align::Center);
        hero_frame.set_valign(gtk4::Align::Center);
        hero_frame.set_vexpand(false);
        hero_frame.add_css_class("gpa-hero-frame");
        hero_frame.set_tooltip_text(Some("Click to transition to the next icon state"));

        // Exact 128×128 (16 × 8px) coordinate box so the 16×16 grid, icon, and X-Ray spine align 1:1
        let hero_overlay = gtk4::Overlay::builder()
            .width_request(128)
            .height_request(128)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .vexpand(true)
            .build();

        let grid_area = gtk4::DrawingArea::builder()
            .content_width(128)
            .content_height(128)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();
        {
            let grid_flag = show_grid.clone();
            let light_flag = light_canvas.clone();
            grid_area.set_draw_func(move |_, cr, width, height| {
                if !grid_flag.get() {
                    return;
                }
                let w = f64::from(width);
                let h = f64::from(height);
                let step_x = w / 16.0;
                let step_y = h / 16.0;
                let is_light = light_flag.get();

                cr.set_line_width(1.0);
                if is_light {
                    cr.set_source_rgba(0.0, 0.0, 0.0, 0.16);
                } else {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.15);
                }

                for i in 1..16 {
                    let x = (f64::from(i) * step_x).floor() + 0.5;
                    cr.move_to(x, 0.0);
                    cr.line_to(x, h);

                    let y = (f64::from(i) * step_y).floor() + 0.5;
                    cr.move_to(0.0, y);
                    cr.line_to(w, y);
                }
                let _ = cr.stroke();

                // Outer 16×16 boundary box
                if is_light {
                    cr.set_source_rgba(0.0, 0.0, 0.0, 0.28);
                } else {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.26);
                }
                cr.rectangle(0.5, 0.5, (w - 1.0).max(0.0), (h - 1.0).max(0.0));
                let _ = cr.stroke();
            });
        }

        let hero_svg_obj = gtk4::Svg::new();
        let hero_image = gtk4::Image::builder()
            .pixel_size(128)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .paintable(&hero_svg_obj)
            .build();

        let xray_svg_obj = gtk4::Svg::new();
        let xray_image = gtk4::Image::builder()
            .pixel_size(128)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .can_target(false)
            .visible(false)
            .paintable(&xray_svg_obj)
            .build();

        hero_overlay.set_child(Some(&grid_area));
        hero_overlay.add_overlay(&hero_image);
        hero_overlay.add_overlay(&xray_image);
        hero_frame.append(&hero_overlay);

        {
            let svg_for_realize = hero_svg_obj.clone();
            let xray_for_realize = xray_svg_obj.clone();
            let play_flag = playing.clone();
            hero_image.connect_realize(move |img| {
                if let Some(clock) = img.frame_clock() {
                    svg_for_realize.set_frame_clock(&clock);
                    xray_for_realize.set_frame_clock(&clock);
                }
                if play_flag.get() {
                    svg_for_realize.play();
                    xray_for_realize.play();
                } else {
                    svg_for_realize.pause();
                    xray_for_realize.pause();
                }
            });
        }

        let hero_info_col = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        hero_info_col.set_valign(gtk4::Align::Center);
        hero_info_col.set_hexpand(true);

        let overline_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let badge = gtk4::Label::new(Some("SHOWCASE"));
        badge.add_css_class("gpa-overline-badge");
        let active_state_pill = gtk4::Label::new(Some("State #0"));
        active_state_pill.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        active_state_pill.set_max_width_chars(18);
        active_state_pill.add_css_class("gpa-state-pill");
        overline_row.append(&badge);
        overline_row.append(&active_state_pill);

        let title_label = gtk4::Label::new(Some("example3.gpa"));
        title_label.set_halign(gtk4::Align::Start);
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title_label.add_css_class("title-3");

        let subtitle_label = gtk4::Label::new(Some("16 × 16 px • 5 shapes • 3 states"));
        subtitle_label.set_halign(gtk4::Align::Start);
        subtitle_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        subtitle_label.add_css_class("caption");
        subtitle_label.add_css_class("dim-label");

        let hero_actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        hero_actions.set_margin_top(4);
        hero_actions.set_valign(gtk4::Align::Center);

        let play_pause_btn = gtk4::ToggleButton::builder()
            .icon_name("media-playback-start-symbolic")
            .active(false)
            .css_classes(["circular", "gpa-showcase-primary-btn"])
            .tooltip_text("Play / Pause GPA State & Loop Animations")
            .build();

        let prev_state_btn = gtk4::Button::builder()
            .icon_name("media-skip-backward-symbolic")
            .css_classes(["circular", "gpa-showcase-circle-btn"])
            .tooltip_text("Transition to Previous State")
            .build();

        let next_state_btn = gtk4::Button::builder()
            .icon_name("media-skip-forward-symbolic")
            .css_classes(["circular", "gpa-showcase-circle-btn"])
            .tooltip_text("Transition to Next State")
            .build();

        let toggle_grid_btn = gtk4::ToggleButton::builder()
            .icon_name("view-grid-symbolic")
            .active(true)
            .css_classes(["circular", "gpa-showcase-circle-btn"])
            .tooltip_text("Toggle 16×16 Pixel Grid Overlay")
            .build();

        let toggle_xray_btn = gtk4::ToggleButton::builder()
            .icon_name("view-reveal-symbolic")
            .active(false)
            .css_classes(["circular", "gpa-showcase-circle-btn"])
            .tooltip_text("Toggle X-Ray Shape Paths (Red Hairline Centerline)")
            .build();

        let toggle_bg_btn = gtk4::ToggleButton::builder()
            .icon_name("weather-clear-symbolic")
            .active(false)
            .css_classes(["circular", "gpa-showcase-circle-btn"])
            .tooltip_text("Toggle Light / Dark Preview Surface")
            .build();

        hero_actions.append(&play_pause_btn);
        hero_actions.append(&prev_state_btn);
        hero_actions.append(&next_state_btn);
        hero_actions.append(&toggle_grid_btn);
        hero_actions.append(&toggle_xray_btn);
        hero_actions.append(&toggle_bg_btn);

        hero_info_col.append(&overline_row);
        hero_info_col.append(&title_label);
        hero_info_col.append(&subtitle_label);
        hero_info_col.append(&hero_actions);

        hero_banner.append(&hero_frame);
        hero_banner.append(&hero_info_col);

        // =========================================================================
        // 2. SMALL, DISCRETE STATE PREVIEW BOXES
        // =========================================================================
        let states_section = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        states_section.set_valign(gtk4::Align::Start);
        states_section.set_vexpand(false);

        let states_title = gtk4::Label::new(Some("STATES & TRANSITIONS"));
        states_title.add_css_class("gpa-section-heading");
        states_title.set_halign(gtk4::Align::Start);

        let state_boxes_row = gtk4::FlowBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .homogeneous(false)
            .valign(gtk4::Align::Start)
            .vexpand(false)
            .max_children_per_line(10)
            .min_children_per_line(1)
            .column_spacing(8)
            .row_spacing(8)
            .build();

        states_section.append(&states_title);
        states_section.append(&state_boxes_row);

        // =========================================================================
        // 3. SUBTLE CUSTOMIZE CARD BELOW SHOWCASE & STATES
        // =========================================================================
        let customize_card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
        customize_card.set_valign(gtk4::Align::Start);
        customize_card.set_vexpand(false);
        customize_card.add_css_class("gpa-customize-card");

        let cust_top_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        cust_top_row.set_valign(gtk4::Align::Center);

        let cust_title = gtk4::Label::new(Some("CUSTOMIZE"));
        cust_title.add_css_class("gpa-section-heading");
        cust_title.set_halign(gtk4::Align::Start);
        cust_title.set_hexpand(true);

        let fill_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        fill_box.set_valign(gtk4::Align::Center);
        let fill_lbl = gtk4::Label::new(Some("Fill"));
        fill_lbl.add_css_class("caption");
        let fill_switch = gtk4::Switch::builder()
            .valign(gtk4::Align::Center)
            .active(false)
            .tooltip_text("Toggle gpa:fill alongside stroke")
            .build();
        fill_box.append(&fill_lbl);
        fill_box.append(&fill_switch);

        let reset_btn = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .label("Reset")
            .css_classes(["flat", "gpa-reset-btn"])
            .tooltip_text("Reset all customizations to original icon")
            .build();

        cust_top_row.append(&cust_title);
        cust_top_row.append(&fill_box);
        cust_top_row.append(&reset_btn);

        let (weight_row, weight_scale) =
            Self::build_compact_slider("Weight", 100.0, 1000.0, 25.0, 400.0, "400");
        let (sw_row, sw_scale) = Self::build_compact_slider("Stroke", 0.5, 4.0, 0.25, 2.0, "2px");
        let (tdur_row, tdur_scale) =
            Self::build_compact_slider("Speed", 100.0, 1200.0, 50.0, 400.0, "400ms");

        // 2x2 grid of compact dropdowns so width stays very small (~280px min)
        let dd_grid = gtk4::Grid::builder()
            .row_spacing(6)
            .column_spacing(12)
            .column_homogeneous(true)
            .hexpand(true)
            .build();

        let trans_dropdown = gtk4::DropDown::from_strings(&["Animate", "Morph", "Fade", "None"]);
        trans_dropdown.set_selected(0);

        let anim_dropdown = gtk4::DropDown::from_strings(&[
            "None",
            "Normal",
            "Alternate",
            "In-Out",
            "In-Out Alt",
            "Segment",
            "Segment Alt",
        ]);
        anim_dropdown.set_selected(0);

        let color_dropdown = gtk4::DropDown::from_strings(&[
            "Foreground",
            "Accent",
            "Success",
            "Warning",
            "Error",
        ]);
        color_dropdown.set_selected(0);

        let render_dropdown = gtk4::DropDown::from_strings(&["SVG", "Symbolic"]);
        render_dropdown.set_selected(0);

        dd_grid.attach(&Self::build_compact_dropdown("Transition", &trans_dropdown), 0, 0, 1, 1);
        dd_grid.attach(&Self::build_compact_dropdown("Loop Anim", &anim_dropdown), 1, 0, 1, 1);
        dd_grid.attach(&Self::build_compact_dropdown("Paint", &color_dropdown), 0, 1, 1, 1);
        dd_grid.attach(&Self::build_compact_dropdown("Rendering", &render_dropdown), 1, 1, 1, 1);

        customize_card.append(&cust_top_row);
        customize_card.append(&weight_row);
        customize_card.append(&sw_row);
        customize_card.append(&tdur_row);
        customize_card.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
        customize_card.append(&dd_grid);

        // =========================================================================
        // 4. SCROLLABLE CONTEXT COMPARISON GRID AT THE BOTTOM
        // =========================================================================
        let context_section = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        context_section.set_valign(gtk4::Align::Start);
        context_section.set_vexpand(false);

        let context_title = gtk4::Label::new(Some("CONTEXT SCALE PREVIEW (48 PX & 24 PX)"));
        context_title.add_css_class("gpa-section-heading");
        context_title.set_halign(gtk4::Align::Start);

        let context_grid_frame = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
        context_grid_frame.set_valign(gtk4::Align::Start);
        context_grid_frame.set_vexpand(false);
        context_grid_frame.add_css_class("gpa-context-card");

        let grid = gtk4::Grid::builder()
            .row_spacing(10)
            .column_spacing(6)
            .column_homogeneous(true)
            .halign(gtk4::Align::Fill)
            .valign(gtk4::Align::Start)
            .hexpand(true)
            .vexpand(false)
            .build();

        let context_svgs: Rc<RefCell<Vec<gtk4::Svg>>> = Rc::new(RefCell::new(Vec::new()));
        let active_clone_svgs: Rc<RefCell<Vec<gtk4::Svg>>> = Rc::new(RefCell::new(Vec::new()));

        let kit_icons = all_icons();
        let active_slots_48 = [1_usize, 5_usize];
        let active_slots_24 = [2_usize, 6_usize, 9_usize, 14_usize];

        for i in 0..8_usize {
            let row = i32::try_from(i / 4).unwrap_or(0);
            let col = i32::try_from((i % 4) * 2).unwrap_or(0);
            let is_active_slot = active_slots_48.contains(&i);

            let cell_frame = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
            cell_frame.set_halign(gtk4::Align::Center);
            cell_frame.set_valign(gtk4::Align::Center);
            cell_frame.add_css_class("gpa-context-cell");
            if is_active_slot {
                cell_frame.add_css_class("gpa-context-cell-active");
            }

            let svg = gtk4::Svg::new();
            if !is_active_slot && let Some(entry) = kit_icons.get((i * 17 + 9) % kit_icons.len()) {
                let bytes = glib::Bytes::from(entry.xml.as_bytes());
                svg.load_from_bytes(&bytes);
                cell_frame.set_tooltip_text(Some(entry.file_name));
            } else {
                cell_frame.set_tooltip_text(Some("Active GPA Icon (48 px)"));
            }

            let img = gtk4::Image::builder().pixel_size(36).paintable(&svg).build();
            {
                let svg_realize = svg.clone();
                let play_flag = playing.clone();
                img.connect_realize(move |w| {
                    if let Some(clock) = w.frame_clock() {
                        svg_realize.set_frame_clock(&clock);
                    }
                    if play_flag.get() {
                        svg_realize.play();
                    } else {
                        svg_realize.pause();
                    }
                });
            }
            cell_frame.append(&img);
            grid.attach(&cell_frame, col, row, 2, 1);

            context_svgs.borrow_mut().push(svg.clone());
            if is_active_slot {
                active_clone_svgs.borrow_mut().push(svg);
            }
        }

        for i in 0..16_usize {
            let row = 2 + i32::try_from(i / 8).unwrap_or(0);
            let col = i32::try_from(i % 8).unwrap_or(0);
            let is_active_slot = active_slots_24.contains(&i);

            let cell_frame = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
            cell_frame.set_halign(gtk4::Align::Center);
            cell_frame.set_valign(gtk4::Align::Center);
            cell_frame.add_css_class("gpa-context-cell");
            if is_active_slot {
                cell_frame.add_css_class("gpa-context-cell-active");
            }

            let svg = gtk4::Svg::new();
            if !is_active_slot && let Some(entry) = kit_icons.get((i * 23 + 31) % kit_icons.len()) {
                let bytes = glib::Bytes::from(entry.xml.as_bytes());
                svg.load_from_bytes(&bytes);
                cell_frame.set_tooltip_text(Some(entry.file_name));
            } else {
                cell_frame.set_tooltip_text(Some("Active GPA Icon (24 px)"));
            }

            let img = gtk4::Image::builder().pixel_size(20).paintable(&svg).build();
            {
                let svg_realize = svg.clone();
                let play_flag = playing.clone();
                img.connect_realize(move |w| {
                    if let Some(clock) = w.frame_clock() {
                        svg_realize.set_frame_clock(&clock);
                    }
                    if play_flag.get() {
                        svg_realize.play();
                    } else {
                        svg_realize.pause();
                    }
                });
            }
            cell_frame.append(&img);
            grid.attach(&cell_frame, col, row, 1, 1);

            context_svgs.borrow_mut().push(svg.clone());
            if is_active_slot {
                active_clone_svgs.borrow_mut().push(svg);
            }
        }

        context_grid_frame.append(&grid);
        context_section.append(&context_title);
        context_section.append(&context_grid_frame);

        outer_box.append(&hero_banner);
        outer_box.append(&states_section);
        outer_box.append(&customize_card);
        outer_box.append(&context_section);

        let clamp = adw::Clamp::builder()
            .maximum_size(580)
            .tightening_threshold(340)
            .child(&outer_box)
            .build();

        let root = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .propagate_natural_width(false)
            .min_content_width(290)
            .hexpand(true)
            .vexpand(true)
            .css_classes(["gpa-main-island"])
            .child(&clamp)
            .build();

        let canvas = Self {
            root,
            hero_frame: hero_frame.clone(),
            grid_area,
            hero_svg: Rc::new(RefCell::new(hero_svg_obj)),
            xray_svg: Rc::new(RefCell::new(xray_svg_obj)),
            xray_image,
            title_label,
            subtitle_label,
            active_state_pill,
            play_pause_btn: play_pause_btn.clone(),
            state_boxes_row,
            state_card_buttons: Rc::new(RefCell::new(Vec::new())),
            context_grid_frame,
            context_svgs,
            active_clone_svgs,
            state_box_svgs: Rc::new(RefCell::new(Vec::new())),
            state_preview_wells: Rc::new(RefCell::new(Vec::new())),
            current_doc,
            base_xml,
            current_state,
            playing,
            transition_speed_ms,
            playback_timer,
            weight,
            show_grid,
            show_xray,
            light_canvas,
            on_state_changed,
            on_doc_mutated,
        };

        // Wire Weight Slider
        {
            let c = canvas.clone();
            weight_scale.connect_value_changed(move |sc| {
                c.set_weight(sc.value());
            });
        }

        // Wire Customize controls
        let emit_customize: Rc<dyn Fn()> = {
            let c = canvas.clone();
            let sup = suppress_customize.clone();
            let f_sw = fill_switch.clone();
            let s_sc = sw_scale.clone();
            let t_sc = tdur_scale.clone();
            let tr_dd = trans_dropdown.clone();
            let an_dd = anim_dropdown.clone();
            let col_dd = color_dropdown.clone();
            let ren_dd = render_dropdown.clone();

            Rc::new(move || {
                if sup.get() {
                    return;
                }
                let tr_idx = tr_dd.selected() as usize;
                let an_idx = an_dd.selected() as usize;
                let col_idx = col_dd.selected() as usize;
                let ren_idx = ren_dd.selected() as usize;

                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let transition_duration_ms = t_sc.value().round() as u32;

                let cfg = GpaAutoCustomize {
                    fill_enabled: f_sw.is_active(),
                    stroke_width: s_sc.value(),
                    transition_type: (*TRANSITION_TYPES.get(tr_idx).unwrap_or(&"animate"))
                        .to_string(),
                    transition_duration_ms,
                    transition_easing: String::from("ease-in-out"),
                    animation_direction: (*ANIMATION_DIRS.get(an_idx).unwrap_or(&"none"))
                        .to_string(),
                    animation_duration_ms: 800,
                    animation_segment: 0.5,
                    symbolic_color: (*SYMBOLIC_COLORS.get(col_idx).unwrap_or(&"foreground"))
                        .to_string(),
                    rendering_mode: (*RENDERING_MODES.get(ren_idx).unwrap_or(&"svg")).to_string(),
                };

                c.apply_customization(&cfg);
            })
        };

        {
            let emit = emit_customize.clone();
            fill_switch.connect_state_set(move |_, _| {
                emit();
                glib::Propagation::Proceed
            });
        }
        {
            let emit = emit_customize.clone();
            sw_scale.connect_value_changed(move |_| emit());
        }
        {
            let emit = emit_customize.clone();
            let c_speed = canvas.clone();
            tdur_scale.connect_value_changed(move |sc| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let ms = sc.value().round() as u32;
                c_speed.transition_speed_ms.set(ms.clamp(100, 2000));
                emit();
                if c_speed.playing.get() {
                    c_speed.restart_playback_timer();
                }
            });
        }
        {
            let emit = emit_customize.clone();
            trans_dropdown.connect_selected_notify(move |_| emit());
        }
        {
            let emit = emit_customize.clone();
            anim_dropdown.connect_selected_notify(move |_| emit());
        }
        {
            let emit = emit_customize.clone();
            color_dropdown.connect_selected_notify(move |_| emit());
        }
        {
            let emit = emit_customize;
            render_dropdown.connect_selected_notify(move |_| emit());
        }

        // Wire Reset button
        {
            let c = canvas.clone();
            let sup = suppress_customize;
            reset_btn.connect_clicked(move |_| {
                sup.set(true);
                fill_switch.set_active(false);
                weight_scale.set_value(400.0);
                sw_scale.set_value(2.0);
                tdur_scale.set_value(400.0);
                trans_dropdown.set_selected(0);
                anim_dropdown.set_selected(0);
                color_dropdown.set_selected(0);
                render_dropdown.set_selected(0);
                sup.set(false);

                c.set_weight(400.0);
                c.reset_to_base_xml();
            });
        }

        // Wire click on Hero Frame to cycle states
        {
            let c = canvas.clone();
            let click = gtk4::GestureClick::new();
            click.connect_released(move |_, _, _, _| {
                c.cycle_next_state();
            });
            hero_frame.add_controller(click);
        }

        // Wire Play/Pause button
        {
            let c = canvas.clone();
            play_pause_btn.connect_toggled(move |btn| {
                let is_playing = btn.is_active();
                btn.set_icon_name(if is_playing {
                    "media-playback-pause-symbolic"
                } else {
                    "media-playback-start-symbolic"
                });
                c.set_playing(is_playing);
            });
        }

        // Wire Prev / Next State buttons
        {
            let c = canvas.clone();
            prev_state_btn.connect_clicked(move |_| {
                c.cycle_prev_state();
            });
        }
        {
            let c = canvas.clone();
            next_state_btn.connect_clicked(move |_| {
                c.cycle_next_state();
            });
        }

        // Wire Grid Toggle button
        {
            let c = canvas.clone();
            toggle_grid_btn.connect_toggled(move |btn| {
                c.set_show_grid(btn.is_active());
            });
        }

        // Wire X-Ray Shape Paths Toggle button
        {
            let c = canvas.clone();
            toggle_xray_btn.connect_toggled(move |btn| {
                c.set_show_xray(btn.is_active());
            });
        }

        // Wire Light/Dark surface toggle
        {
            let c = canvas.clone();
            toggle_bg_btn.connect_toggled(move |btn| {
                c.set_light_surface(btn.is_active());
            });
        }

        let initial = canvas.current_doc.borrow().clone();
        canvas.load_document(&initial);

        canvas
    }

    #[must_use]
    pub const fn widget(&self) -> &gtk4::ScrolledWindow {
        &self.root
    }

    pub fn connect_state_changed<F: Fn(u32) + 'static>(&self, f: F) {
        *self.on_state_changed.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_doc_mutated<F: Fn(GpaDocumentSummary) + 'static>(&self, f: F) {
        *self.on_doc_mutated.borrow_mut() = Some(Box::new(f));
    }

    pub fn apply_customization(&self, cfg: &GpaAutoCustomize) {
        let updated_doc = {
            let doc = self.current_doc.borrow();
            let orig = self.base_xml.borrow();
            let new_xml = apply_auto_customize(&orig, cfg);
            GpaDocumentSummary::parse(doc.name.clone(), new_xml)
        };
        self.render_document_internal(&updated_doc);
        if let Some(cb) = self.on_doc_mutated.borrow().as_ref() {
            cb(updated_doc);
        }
    }

    pub fn reset_to_base_xml(&self) {
        let updated_doc = {
            let doc = self.current_doc.borrow();
            let orig = self.base_xml.borrow().clone();
            GpaDocumentSummary::parse(doc.name.clone(), orig)
        };
        self.render_document_internal(&updated_doc);
        if let Some(cb) = self.on_doc_mutated.borrow().as_ref() {
            cb(updated_doc);
        }
    }

    pub fn add_new_state(&self, custom_name: Option<&str>) {
        let updated_doc = {
            let doc = self.current_doc.borrow();
            let new_xml = add_state_to_gpa(&doc, custom_name);
            (*self.base_xml.borrow_mut()).clone_from(&new_xml);
            GpaDocumentSummary::parse(doc.name.clone(), new_xml)
        };
        let last_idx = updated_doc.states.last().map_or(0, |s| s.index);
        self.render_document_internal(&updated_doc);
        self.set_active_state(last_idx);
        if let Some(cb) = self.on_doc_mutated.borrow().as_ref() {
            cb(updated_doc);
        }
    }

    pub fn remove_state(&self, target_idx: u32) {
        let updated_doc = {
            let doc = self.current_doc.borrow();
            if doc.states.len() <= 1 {
                return;
            }
            let new_xml = remove_state_from_gpa(&doc, target_idx);
            (*self.base_xml.borrow_mut()).clone_from(&new_xml);
            GpaDocumentSummary::parse(doc.name.clone(), new_xml)
        };
        self.render_document_internal(&updated_doc);
        if let Some(cb) = self.on_doc_mutated.borrow().as_ref() {
            cb(updated_doc);
        }
    }

    pub fn rename_state(&self, target_idx: u32, new_name: &str) {
        let updated_doc = {
            let doc = self.current_doc.borrow();
            let new_xml = rename_state_in_gpa(&doc, target_idx, new_name);
            if new_xml == doc.xml {
                return;
            }
            (*self.base_xml.borrow_mut()).clone_from(&new_xml);
            GpaDocumentSummary::parse(doc.name.clone(), new_xml)
        };
        let cur_st = self.current_state.get();
        self.render_document_internal(&updated_doc);
        self.set_active_state(cur_st);
        if let Some(cb) = self.on_doc_mutated.borrow().as_ref() {
            cb(updated_doc);
        }
    }

    pub fn load_document(&self, doc: &GpaDocumentSummary) {
        (*self.base_xml.borrow_mut()).clone_from(&doc.xml);
        self.render_document_internal(doc);
    }

    fn render_document_internal(&self, doc: &GpaDocumentSummary) {
        let initial_state = doc.initial_state;
        self.current_state.set(initial_state);

        self.title_label.set_label(&doc.name);
        let subtitle = format!(
            "{} × {} px  •  {} shapes  •  {} states",
            doc.width,
            doc.height,
            doc.shapes.len(),
            doc.states.len(),
        );
        self.subtitle_label.set_label(&subtitle);

        *self.current_doc.borrow_mut() = doc.clone();
        self.sync_svg_playback_state();
        self.rebuild_state_boxes(doc);
        self.update_state_pill();

        if self.playing.get() {
            self.restart_playback_timer();
        }
    }

    /// Synchronizes `hero_svg`, `xray_svg`, and `active_clone_svgs` with the universal `playing` state:
    /// - When `playing == false` (Stopped/Paused), loads a static snapshot of `current_state` with all
    ///   loop/transition animations stripped and calls `.pause()` on every SVG so nothing moves on screen.
    /// - When `playing == true`, loads the full animated `.gpa` XML and calls `.play()` on every SVG.
    fn sync_svg_playback_state(&self) {
        let is_playing = self.playing.get();
        let active_st = self.current_state.get();
        let raw_xml = self.current_doc.borrow().xml.clone();

        let effective_xml = with_initial_state_in_gpa(&raw_xml, active_st, !is_playing);
        let bytes = glib::Bytes::from(effective_xml.as_bytes());

        {
            let hero_svg = self.hero_svg.borrow();
            hero_svg.load_from_bytes(&bytes);
            hero_svg.set_weight(self.weight.get());
            hero_svg.set_state(active_st);
            if is_playing {
                hero_svg.play();
            } else {
                hero_svg.pause();
            }
        }
        {
            let xray_xml = build_xray_gpa_xml(&effective_xml);
            let xray_bytes = glib::Bytes::from(xray_xml.as_bytes());
            let xray_svg = self.xray_svg.borrow();
            xray_svg.load_from_bytes(&xray_bytes);
            xray_svg.set_state(active_st);
            if is_playing {
                xray_svg.play();
            } else {
                xray_svg.pause();
            }
        }

        for clone_svg in self.active_clone_svgs.borrow().iter() {
            clone_svg.load_from_bytes(&bytes);
            clone_svg.set_weight(self.weight.get());
            clone_svg.set_state(active_st);
            if is_playing {
                clone_svg.play();
            } else {
                clone_svg.pause();
            }
        }
    }

    /// Rebuilds the compact State Cards (`96px` wide, `52px` preview well, editable name, constant 2px border).
    #[allow(clippy::too_many_lines)]
    fn rebuild_state_boxes(&self, doc: &GpaDocumentSummary) {
        while let Some(child) = self.state_boxes_row.first_child() {
            self.state_boxes_row.remove(&child);
        }
        self.state_card_buttons.borrow_mut().clear();
        self.state_box_svgs.borrow_mut().clear();
        self.state_preview_wells.borrow_mut().clear();

        let active_st = self.current_state.get();
        let is_light = self.light_canvas.get();
        let can_remove = doc.states.len() > 1;

        for state_info in &doc.states {
            let state_idx = state_info.index;

            let card_shell = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            card_shell.set_size_request(96, -1);
            card_shell.set_valign(gtk4::Align::Start);
            card_shell.set_vexpand(false);
            card_shell.add_css_class("gpa-state-card");
            if state_idx == active_st {
                card_shell.add_css_class("gpa-state-card-active");
            }

            let top_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 3);
            top_row.set_valign(gtk4::Align::Center);

            let idx_badge = gtk4::Label::new(Some(&format!("#{state_idx}")));
            idx_badge.add_css_class("gpa-state-index-badge");

            let name_entry = gtk4::Entry::builder()
                .text(&state_info.name)
                .width_chars(5)
                .max_width_chars(7)
                .has_frame(false)
                .hexpand(true)
                .css_classes(["gpa-state-name-entry"])
                .tooltip_text("Click to rename state, press Enter to apply")
                .build();

            {
                let c = self.clone();
                name_entry.connect_activate(move |entry| {
                    c.rename_state(state_idx, &entry.text());
                });
            }

            top_row.append(&idx_badge);
            top_row.append(&name_entry);

            if can_remove {
                let del_btn = gtk4::Button::builder()
                    .icon_name("window-close-symbolic")
                    .css_classes(["flat", "circular", "gpa-state-remove-btn"])
                    .tooltip_text(format!("Remove state #{state_idx} ({})", state_info.name))
                    .build();
                let c = self.clone();
                del_btn.connect_clicked(move |_| {
                    c.remove_state(state_idx);
                });
                top_row.append(&del_btn);
            }

            // Compact 52x52 preview well
            let preview_holder = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
            preview_holder.set_size_request(52, 52);
            preview_holder.set_halign(gtk4::Align::Center);
            preview_holder.set_valign(gtk4::Align::Center);
            preview_holder.set_vexpand(false);
            preview_holder.set_cursor_from_name(Some("pointer"));
            preview_holder.add_css_class("gpa-state-preview-well");
            if is_light {
                preview_holder.add_css_class("gpa-light-surface");
            }
            self.state_preview_wells
                .borrow_mut()
                .push(preview_holder.clone());

            // Static thumbnail XML locked to `state_idx` with continuous loops stripped
            let static_state_xml = with_initial_state_in_gpa(&doc.xml, state_idx, true);
            let state_bytes = glib::Bytes::from(static_state_xml.as_bytes());
            let state_svg = gtk4::Svg::from_bytes(&state_bytes);
            state_svg.set_weight(self.weight.get());
            state_svg.set_state(state_idx);
            state_svg.pause();

            let state_img = gtk4::Image::builder()
                .pixel_size(32)
                .halign(gtk4::Align::Center)
                .valign(gtk4::Align::Center)
                .hexpand(true)
                .vexpand(true)
                .paintable(&state_svg)
                .build();
            preview_holder.append(&state_img);
            self.state_box_svgs.borrow_mut().push(state_svg);

            {
                let c = self.clone();
                let click = gtk4::GestureClick::new();
                click.connect_released(move |_, _, _, _| {
                    c.set_active_state(state_idx);
                    if c.playing.get() {
                        c.restart_playback_timer();
                    }
                });
                preview_holder.add_controller(click);
            }

            let count_lbl = gtk4::Label::new(Some(&format!(
                "{}/{}",
                state_info.visible_shape_count,
                doc.shapes.len()
            )));
            count_lbl.add_css_class("caption");
            count_lbl.add_css_class("dim-label");

            card_shell.append(&top_row);
            card_shell.append(&preview_holder);
            card_shell.append(&count_lbl);

            let widget: gtk4::Widget = card_shell.upcast();
            self.state_boxes_row.insert(&widget, -1);
            self.state_card_buttons
                .borrow_mut()
                .push((state_idx, widget));
        }

        // Compact Trailing "+" State Card
        let add_card_inner = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        add_card_inner.set_valign(gtk4::Align::Center);
        add_card_inner.set_halign(gtk4::Align::Center);
        let plus_icon = gtk4::Image::builder()
            .icon_name("list-add-symbolic")
            .pixel_size(18)
            .build();
        let plus_lbl = gtk4::Label::new(Some("Add"));
        plus_lbl.add_css_class("caption");
        plus_lbl.add_css_class("dim-label");
        add_card_inner.append(&plus_icon);
        add_card_inner.append(&plus_lbl);

        let add_card_btn = gtk4::Button::builder()
            .child(&add_card_inner)
            .width_request(78)
            .height_request(102)
            .valign(gtk4::Align::Start)
            .css_classes(["gpa-state-add-card"])
            .tooltip_text("Add a new GPA state")
            .build();
        {
            let c = self.clone();
            add_card_btn.connect_clicked(move |_| {
                c.add_new_state(None);
            });
        }
        self.state_boxes_row.insert(&add_card_btn, -1);
    }

    pub fn set_active_state(&self, state: u32) {
        self.current_state.set(state);
        if self.playing.get() {
            self.hero_svg.borrow().set_state(state);
            self.xray_svg.borrow().set_state(state);
            for clone_svg in self.active_clone_svgs.borrow().iter() {
                clone_svg.set_state(state);
            }
        } else {
            self.sync_svg_playback_state();
        }

        for (idx, widget) in self.state_card_buttons.borrow().iter() {
            if *idx == state {
                widget.add_css_class("gpa-state-card-active");
            } else {
                widget.remove_css_class("gpa-state-card-active");
            }
        }

        self.update_state_pill();

        let cb_opt = self.on_state_changed.borrow();
        if let Some(cb) = cb_opt.as_ref() {
            cb(state);
        }
    }

    pub fn cycle_next_state(&self) {
        let next_state = {
            let doc = self.current_doc.borrow();
            if doc.states.is_empty() {
                0
            } else {
                let cur = self.current_state.get();
                let pos = doc
                    .states
                    .iter()
                    .position(|s| s.index == cur)
                    .unwrap_or(0);
                doc.states[(pos + 1) % doc.states.len()].index
            }
        };
        self.set_active_state(next_state);
    }

    pub fn cycle_prev_state(&self) {
        let prev_state = {
            let doc = self.current_doc.borrow();
            if doc.states.is_empty() {
                0
            } else {
                let cur = self.current_state.get();
                let pos = doc
                    .states
                    .iter()
                    .position(|s| s.index == cur)
                    .unwrap_or(0);
                let prev_pos = if pos == 0 {
                    doc.states.len() - 1
                } else {
                    pos - 1
                };
                doc.states[prev_pos].index
            }
        };
        self.set_active_state(prev_state);
    }

    pub fn set_playing(&self, is_playing: bool) {
        let was_playing = self.playing.replace(is_playing);
        if self.play_pause_btn.is_active() != is_playing {
            self.play_pause_btn.set_active(is_playing);
        }
        self.play_pause_btn.set_icon_name(if is_playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        });

        if is_playing {
            self.sync_svg_playback_state();
            if !was_playing {
                self.cycle_next_state();
            }
            self.restart_playback_timer();
        } else {
            self.stop_playback_timer();
            let should_restore_initial = {
                let doc = self.current_doc.borrow();
                let cur = self.current_state.get();
                doc.visible_shapes_in_state(cur) == 0
                    && doc.visible_shapes_in_state(doc.initial_state) > 0
            };
            if should_restore_initial {
                let init_st = self.current_doc.borrow().initial_state;
                self.current_state.set(init_st);
            }
            // Re-sync active state pill & card borders and freeze all SVG animations
            let final_st = self.current_state.get();
            self.set_active_state(final_st);
        }
    }

    fn stop_playback_timer(&self) {
        if let Some(source_id) = self.playback_timer.borrow_mut().take() {
            source_id.remove();
        }
    }

    fn restart_playback_timer(&self) {
        self.stop_playback_timer();
        if !self.playing.get() {
            return;
        }
        let interval_ms = u64::from(self.transition_speed_ms.get().saturating_add(450).max(350));
        let c = self.clone();
        let source_id = glib::timeout_add_local(
            std::time::Duration::from_millis(interval_ms),
            move || {
                if !c.playing.get() {
                    return glib::ControlFlow::Break;
                }
                c.cycle_next_state();
                glib::ControlFlow::Continue
            },
        );
        *self.playback_timer.borrow_mut() = Some(source_id);
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing.get()
    }

    pub fn set_weight(&self, weight: f64) {
        self.weight.set(weight);
        self.hero_svg.borrow().set_weight(weight);
        for svg in self.context_svgs.borrow().iter() {
            svg.set_weight(weight);
        }
        for svg in self.state_box_svgs.borrow().iter() {
            svg.set_weight(weight);
        }
    }

    pub fn set_show_grid(&self, show: bool) {
        self.show_grid.set(show);
        self.grid_area.queue_draw();
    }

    pub fn set_show_xray(&self, show: bool) {
        self.show_xray.set(show);
        self.xray_image.set_visible(show);
    }

    pub fn set_light_surface(&self, light: bool) {
        self.light_canvas.set(light);
        if light {
            self.hero_frame.add_css_class("gpa-light-surface");
            self.context_grid_frame.add_css_class("gpa-light-surface");
            for well in self.state_preview_wells.borrow().iter() {
                well.add_css_class("gpa-light-surface");
            }
        } else {
            self.hero_frame.remove_css_class("gpa-light-surface");
            self.context_grid_frame.remove_css_class("gpa-light-surface");
            for well in self.state_preview_wells.borrow().iter() {
                well.remove_css_class("gpa-light-surface");
            }
        }
        self.grid_area.queue_draw();
    }

    fn update_state_pill(&self) {
        let st = self.current_state.get();
        let doc = self.current_doc.borrow();
        let state_name = doc
            .states
            .iter()
            .find(|s| s.index == st)
            .map_or("default", |s| s.name.as_str());
        self.active_state_pill
            .set_label(&format!("State #{st} ({state_name})"));
    }

    fn build_compact_slider(
        title: &str,
        min: f64,
        max: f64,
        step: f64,
        initial: f64,
        initial_badge: &str,
    ) -> (gtk4::Box, gtk4::Scale) {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        row.set_valign(gtk4::Align::Center);

        let lbl = gtk4::Label::new(Some(title));
        lbl.set_width_chars(6);
        lbl.set_xalign(0.0);
        lbl.add_css_class("caption");

        let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, min, max, step);
        scale.set_value(initial);
        scale.set_draw_value(false);
        scale.set_hexpand(true);

        let val_lbl = gtk4::Label::new(Some(initial_badge));
        val_lbl.set_width_chars(6);
        val_lbl.set_xalign(1.0);
        val_lbl.add_css_class("caption");
        val_lbl.add_css_class("dim-label");

        {
            let v_lbl = val_lbl.clone();
            let is_ms = initial_badge.ends_with("ms");
            let is_px = initial_badge.ends_with("px");
            scale.connect_value_changed(move |sc| {
                let v = sc.value();
                if is_ms {
                    v_lbl.set_label(&format!("{v:.0}ms"));
                } else if is_px {
                    v_lbl.set_label(&format!("{v:.1}px"));
                } else {
                    v_lbl.set_label(&format!("{v:.0}"));
                }
            });
        }

        row.append(&lbl);
        row.append(&scale);
        row.append(&val_lbl);
        (row, scale)
    }

    fn build_compact_dropdown(title: &str, dropdown: &gtk4::DropDown) -> gtk4::Box {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        row.set_valign(gtk4::Align::Center);
        let lbl = gtk4::Label::new(Some(title));
        lbl.set_xalign(0.0);
        lbl.set_hexpand(true);
        lbl.add_css_class("caption");
        lbl.add_css_class("dim-label");

        dropdown.add_css_class("gpa-compact-dropdown");
        row.append(&lbl);
        row.append(dropdown);
        row
    }
}
