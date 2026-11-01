use std::cell::RefCell;
use std::rc::Rc;

use gpa_core::{GPA_SPECS_MARKDOWN, GpaDocumentSummary};
use gtk4::prelude::*;
use sourceview5::prelude::*;

type ApplyXmlCallback = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

/// Right collapsible & resizable sidebar focusing on:
/// 1. Graphical Shapes & GPA Attributes Inspector
/// 2. Live `.gpa` XML (`sourceview5::View` with Apply)
/// 3. Bundled `GPA_SPECS.md` & MCP Server Info
#[derive(Clone)]
pub struct InspectorSidebar {
    revealer: gtk4::Revealer,
    container: gtk4::Box,
    stack: gtk4::Stack,
    tab_specs_btn: gtk4::ToggleButton,
    meta_summary_lbl: gtk4::Label,
    meta_state_lbl: gtk4::Label,
    shapes_list: gtk4::Box,
    xml_buffer: sourceview5::Buffer,
    on_apply_xml: ApplyXmlCallback,
}

impl InspectorSidebar {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(mcp_port: u16, initial_width: i32) -> Self {
        let on_apply_xml: ApplyXmlCallback = Rc::new(RefCell::new(None));

        let clamped_init = initial_width.clamp(200, 560);
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        container.set_size_request(clamped_init, -1);
        container.set_hexpand(false);
        container.set_overflow(gtk4::Overflow::Hidden);
        container.add_css_class("gpa-island-panel");

        let top_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        top_bar.add_css_class("gpa-inspector-toolbar");

        let tab_inspector_btn = gtk4::ToggleButton::builder()
            .label("Shapes")
            .active(true)
            .hexpand(true)
            .css_classes(["flat", "gpa-tab-btn"])
            .tooltip_text("Inspect Graphical Shapes and GPA Attributes")
            .build();

        let tab_xml_btn = gtk4::ToggleButton::builder()
            .label("GPA XML")
            .group(&tab_inspector_btn)
            .hexpand(true)
            .css_classes(["flat", "gpa-tab-btn"])
            .tooltip_text("View and edit live .gpa XML source")
            .build();

        let tab_specs_btn = gtk4::ToggleButton::builder()
            .label("MCP Specs")
            .group(&tab_inspector_btn)
            .hexpand(true)
            .css_classes(["flat", "gpa-tab-btn"])
            .tooltip_text("GPA_SPECS.md reference served to MCP AI clients")
            .build();

        top_bar.append(&tab_inspector_btn);
        top_bar.append(&tab_xml_btn);
        top_bar.append(&tab_specs_btn);

        // CRITICAL: hhomogeneous(false) so inactive pages never inflate the active page's natural width!
        let stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .hhomogeneous(false)
            .vhomogeneous(false)
            .vexpand(true)
            .hexpand(true)
            .build();

        // --- PAGE 1: SHAPES & ATTRIBUTES INSPECTOR ---
        let inspector_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        inspector_box.set_margin_top(6);
        inspector_box.set_margin_bottom(14);
        inspector_box.set_margin_start(8);
        inspector_box.set_margin_end(8);

        let summary_card = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        summary_card.add_css_class("gpa-inspector-card");
        let meta_summary_lbl = gtk4::Label::new(Some("5 graphical shapes, 3 in current state"));
        meta_summary_lbl.set_halign(gtk4::Align::Start);
        meta_summary_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        meta_summary_lbl.set_width_chars(10);
        meta_summary_lbl.set_max_width_chars(18);
        meta_summary_lbl.add_css_class("caption");
        meta_summary_lbl.add_css_class("dim-label");

        let meta_state_lbl = gtk4::Label::new(Some("Current state: 0"));
        meta_state_lbl.set_halign(gtk4::Align::Start);
        meta_state_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        meta_state_lbl.set_width_chars(10);
        meta_state_lbl.set_max_width_chars(18);
        meta_state_lbl.add_css_class("heading");

        summary_card.append(&meta_state_lbl);
        summary_card.append(&meta_summary_lbl);

        let shapes_header = gtk4::Label::new(Some("SHAPES & GPA ATTRIBUTES"));
        shapes_header.set_halign(gtk4::Align::Start);
        shapes_header.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        shapes_header.set_width_chars(10);
        shapes_header.set_max_width_chars(18);
        shapes_header.add_css_class("gpa-section-heading");

        let shapes_list = gtk4::Box::new(gtk4::Orientation::Vertical, 8);

        inspector_box.append(&summary_card);
        inspector_box.append(&shapes_header);
        inspector_box.append(&shapes_list);

        let inspector_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .propagate_natural_width(false)
            .min_content_width(120)
            .vexpand(true)
            .child(&inspector_box)
            .build();

        // --- PAGE 2: LIVE GPA XML EDITOR (sourceview5) ---
        let xml_page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        xml_page.set_margin_top(6);
        xml_page.set_margin_bottom(8);
        xml_page.set_margin_start(6);
        xml_page.set_margin_end(6);

        let xml_buffer = sourceview5::Buffer::new(None);
        if let Some(lang) = sourceview5::LanguageManager::default().language("xml") {
            xml_buffer.set_language(Some(&lang));
        }
        if let Some(scheme) = sourceview5::StyleSchemeManager::default().scheme("Adwaita-dark") {
            xml_buffer.set_style_scheme(Some(&scheme));
        }

        let xml_view = sourceview5::View::with_buffer(&xml_buffer);
        xml_view.set_monospace(true);
        xml_view.set_show_line_numbers(true);
        xml_view.set_tab_width(2);
        xml_view.set_top_margin(8);
        xml_view.set_bottom_margin(8);
        xml_view.set_left_margin(8);
        xml_view.set_right_margin(8);

        let xml_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .propagate_natural_width(false)
            .min_content_width(120)
            .vexpand(true)
            .hexpand(true)
            .css_classes(["gpa-xml-scroll"])
            .child(&xml_view)
            .build();

        let xml_actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let apply_xml_btn = gtk4::Button::builder()
            .icon_name("object-select-symbolic")
            .label("Apply XML")
            .hexpand(true)
            .css_classes(["suggested-action"])
            .build();
        xml_actions.append(&apply_xml_btn);

        xml_page.append(&xml_scroll);
        xml_page.append(&xml_actions);

        // --- PAGE 3: MCP SERVER & GPA_SPECS.md VIEW ---
        let specs_page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        specs_page.set_margin_top(6);
        specs_page.set_margin_bottom(8);
        specs_page.set_margin_start(8);
        specs_page.set_margin_end(8);

        let mcp_card = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        mcp_card.add_css_class("gpa-inspector-card");
        let mcp_url = format!("http://127.0.0.1:{mcp_port}/mcp");
        let mcp_title = gtk4::Label::new(Some(&format!("MCP Live ({mcp_url})")));
        mcp_title.set_halign(gtk4::Align::Start);
        mcp_title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        mcp_title.set_width_chars(10);
        mcp_title.set_max_width_chars(18);
        mcp_title.add_css_class("heading");

        let mcp_desc = gtk4::Label::new(Some(&format!(
            "Connect your AI agent to {mcp_url} to inspect and animate the selected icon live."
        )));
        mcp_desc.set_halign(gtk4::Align::Start);
        mcp_desc.set_xalign(0.0);
        mcp_desc.set_wrap(true);
        mcp_desc.set_width_chars(12);
        mcp_desc.set_max_width_chars(18);
        mcp_desc.add_css_class("caption");
        mcp_desc.add_css_class("dim-label");

        let mcp_btn_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let copy_url_btn = gtk4::Button::builder()
            .icon_name("edit-copy-symbolic")
            .label("URL")
            .hexpand(true)
            .css_classes(["flat", "gpa-reset-btn"])
            .tooltip_text(format!("Copy MCP endpoint URL ({mcp_url}) to clipboard"))
            .build();
        let copy_prompt_btn = gtk4::Button::builder()
            .icon_name("starred-symbolic")
            .label("Prompt")
            .hexpand(true)
            .css_classes(["flat", "gpa-reset-btn"])
            .tooltip_text("Copy starter prompt for your MCP agent to animate the active icon")
            .build();

        {
            let url_str = mcp_url;
            copy_url_btn.connect_clicked(move |btn| {
                btn.clipboard().set_text(&url_str);
            });
        }
        {
            copy_prompt_btn.connect_clicked(move |btn| {
                let prompt = "Use the gpa-icons-animator MCP server: call `get_active_icon` to inspect the currently selected icon and its shapes/states, call `get_gpa_specs` for GTK 4.24 .gpa animation rules, and animate the icon live in the studio using `patch_shape_gpa`, `upsert_shape_gpa`, `manage_states`, or `update_gpa_xml`.";
                btn.clipboard().set_text(prompt);
            });
        }

        mcp_btn_row.append(&copy_url_btn);
        mcp_btn_row.append(&copy_prompt_btn);

        mcp_card.append(&mcp_title);
        mcp_card.append(&mcp_desc);
        mcp_card.append(&mcp_btn_row);

        let specs_buf = sourceview5::Buffer::new(None);
        if let Some(lang) = sourceview5::LanguageManager::default().language("markdown") {
            specs_buf.set_language(Some(&lang));
        }
        if let Some(scheme) = sourceview5::StyleSchemeManager::default().scheme("Adwaita-dark") {
            specs_buf.set_style_scheme(Some(&scheme));
        }
        specs_buf.set_text(GPA_SPECS_MARKDOWN);

        let specs_view = sourceview5::View::with_buffer(&specs_buf);
        specs_view.set_editable(false);
        specs_view.set_monospace(true);
        specs_view.set_wrap_mode(gtk4::WrapMode::WordChar);
        specs_view.set_top_margin(8);
        specs_view.set_bottom_margin(8);
        specs_view.set_left_margin(8);
        specs_view.set_right_margin(8);

        let specs_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .propagate_natural_width(false)
            .min_content_width(120)
            .vexpand(true)
            .hexpand(true)
            .css_classes(["gpa-xml-scroll"])
            .child(&specs_view)
            .build();

        specs_page.append(&mcp_card);
        specs_page.append(&specs_scroll);

        stack.add_named(&inspector_scroll, Some("inspector"));
        stack.add_named(&xml_page, Some("xml"));
        stack.add_named(&specs_page, Some("specs"));

        container.append(&top_bar);
        container.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
        container.append(&stack);

        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideLeft)
            .transition_duration(180)
            .reveal_child(true)
            .hexpand(false)
            .overflow(gtk4::Overflow::Hidden)
            .child(&container)
            .build();

        revealer.connect_child_revealed_notify(|rev| {
            if !rev.is_child_revealed() && !rev.reveals_child() {
                rev.set_visible(false);
            }
        });

        {
            let st = stack.clone();
            tab_inspector_btn.connect_toggled(move |b| {
                if b.is_active() {
                    st.set_visible_child_name("inspector");
                }
            });
        }
        {
            let st = stack.clone();
            tab_xml_btn.connect_toggled(move |b| {
                if b.is_active() {
                    st.set_visible_child_name("xml");
                }
            });
        }
        {
            let st = stack.clone();
            tab_specs_btn.connect_toggled(move |b| {
                if b.is_active() {
                    st.set_visible_child_name("specs");
                }
            });
        }

        {
            let buf = xml_buffer.clone();
            let cb_ref = on_apply_xml.clone();
            apply_xml_btn.connect_clicked(move |_| {
                let start = buf.start_iter();
                let end = buf.end_iter();
                let text = buf.text(&start, &end, false).to_string();
                if let Some(cb) = cb_ref.borrow().as_ref() {
                    cb(text);
                }
            });
        }

        Self {
            revealer,
            container,
            stack,
            tab_specs_btn,
            meta_summary_lbl,
            meta_state_lbl,
            shapes_list,
            xml_buffer,
            on_apply_xml,
        }
    }

    #[must_use]
    pub const fn widget(&self) -> &gtk4::Revealer {
        &self.revealer
    }

    pub fn set_width(&self, width: i32) {
        self.container.set_size_request(width.clamp(200, 560), -1);
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        let req = self.container.width_request();
        if req >= 200 {
            req
        } else {
            self.container.width().clamp(200, 560)
        }
    }

    pub fn set_revealed(&self, revealed: bool) {
        if revealed {
            self.revealer.set_visible(true);
            self.revealer.set_reveal_child(true);
        } else {
            self.revealer.set_reveal_child(false);
            if !self.revealer.is_mapped() {
                self.revealer.set_visible(false);
            }
        }
    }

    pub fn connect_apply_xml<F: Fn(String) + 'static>(&self, f: F) {
        *self.on_apply_xml.borrow_mut() = Some(Box::new(f));
    }

    pub fn update_document(&self, doc: &GpaDocumentSummary, active_state: u32) {
        self.update_active_state(doc, active_state);
        self.xml_buffer.set_text(&doc.xml);

        while let Some(child) = self.shapes_list.first_child() {
            self.shapes_list.remove(&child);
        }

        for shape in &doc.shapes {
            let card = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
            card.add_css_class("gpa-shape-row");

            let top_line = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            let kind_badge = gtk4::Label::new(Some(&format!("<{}>", shape.kind)));
            kind_badge.add_css_class("gpa-mini-tag");

            let id_lbl = gtk4::Label::new(Some(&shape.id));
            id_lbl.add_css_class("heading");
            id_lbl.set_halign(gtk4::Align::Start);
            id_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            id_lbl.set_width_chars(4);
            id_lbl.set_max_width_chars(10);
            id_lbl.set_hexpand(true);

            let states_badge = gtk4::Label::new(Some(&format!("states: {}", shape.states_raw)));
            states_badge.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            states_badge.set_width_chars(6);
            states_badge.set_max_width_chars(12);
            states_badge.add_css_class("gpa-state-index-badge");

            top_line.append(&kind_badge);
            top_line.append(&id_lbl);
            top_line.append(&states_badge);

            let trans_info = if shape.transition_delay == "0ms" {
                format!("{} ({})", shape.transition_type, shape.transition_duration)
            } else {
                format!(
                    "{} ({}, delay {})",
                    shape.transition_type, shape.transition_duration, shape.transition_delay
                )
            };
            let origin_info = shape
                .origin
                .as_deref()
                .map_or_else(String::new, |o| format!(" • origin: {o}"));
            let details_str = format!(
                "stroke: {} ({}px) • trans: {}{} • anim: {}",
                shape.stroke,
                shape.stroke_width,
                trans_info,
                origin_info,
                if shape.animation_type == "none" {
                    String::from("none")
                } else {
                    format!("{} ({})", shape.animation_direction, shape.animation_duration)
                }
            );
            if let Some(d_str) = &shape.d {
                card.set_tooltip_text(Some(&format!("id='{}' d='{}'", shape.id, d_str)));
            }
            let details_lbl = gtk4::Label::new(Some(&details_str));
            details_lbl.set_halign(gtk4::Align::Start);
            details_lbl.set_xalign(0.0);
            details_lbl.set_wrap(true);
            details_lbl.set_width_chars(12);
            details_lbl.set_max_width_chars(18);
            details_lbl.add_css_class("caption");
            details_lbl.add_css_class("dim-label");

            card.append(&top_line);
            card.append(&details_lbl);

            if let Some(parent_id) = &shape.attach_to {
                let attach_lbl =
                    gtk4::Label::new(Some(&format!("↳ Attached to path '{parent_id}'")));
                attach_lbl.set_halign(gtk4::Align::Start);
                attach_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                attach_lbl.set_width_chars(10);
                attach_lbl.set_max_width_chars(18);
                attach_lbl.add_css_class("caption");
                card.append(&attach_lbl);
            }

            self.shapes_list.append(&card);
        }
    }

    pub fn update_active_state(&self, doc: &GpaDocumentSummary, active_state: u32) {
        let vis = doc.visible_shapes_in_state(active_state);
        self.meta_summary_lbl.set_label(&format!(
            "{} graphical shapes, {} in current state",
            doc.shapes.len(),
            vis
        ));
        let st_name = doc
            .states
            .iter()
            .find(|s| s.index == active_state)
            .map_or("default", |s| s.name.as_str());
        self.meta_state_lbl
            .set_label(&format!("Current state: {active_state} ({st_name})"));
    }

    pub fn show_specs_tab(&self) {
        self.tab_specs_btn.set_active(true);
        self.stack.set_visible_child_name("specs");
    }
}
