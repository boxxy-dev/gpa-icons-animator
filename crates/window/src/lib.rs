pub mod inspector_sidebar;
pub mod library_sidebar;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpa_canvas::StudioCanvas;
use gpa_core::{GpaDocumentSummary, empty_scratch_gpa, ensure_gpa_headers};
use gpa_icon_library::{
    UserLibraryFolder, custom_icon_exists_in_dir, discover_user_libraries, sanitize_icon_stem,
    save_custom_icon_in_dir,
};
use gpa_mcp_server::{McpBridge, McpUiCommand, StudioSnapshot};
use gpa_sys_utils::{UiWindowState, ensure_default_user_library};
use gtk4::glib;
use gtk4::prelude::*;
pub use inspector_sidebar::InspectorSidebar;
use libadwaita as adw;
pub use library_sidebar::LibrarySidebar;

/// Main application window with a unified `HeaderBar`, dual resizable sidebars, and Studio Canvas.
pub struct GpaWindow {
    window: adw::ApplicationWindow,
}

impl GpaWindow {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(app: &adw::Application, mcp_port: u16) -> Self {
        let _ = ensure_default_user_library();
        let saved_state = UiWindowState::load();

        let initial_xml = empty_scratch_gpa("untitled");
        let initial_doc = GpaDocumentSummary::parse("untitled.gpa", initial_xml);

        let (ui_tx, ui_rx) = async_channel::unbounded::<McpUiCommand>();
        let mcp_bridge = McpBridge::new(initial_doc.clone(), ui_tx, mcp_port);
        mcp_bridge.spawn_http_server();

        let active_doc = Rc::new(RefCell::new(initial_doc.clone()));

        // --- 1. UNIFIED TOP HEADERBAR ---
        let header_bar = adw::HeaderBar::new();
        header_bar.add_css_class("flat");

        let left_toggle_btn = gtk4::ToggleButton::builder()
            .icon_name("sidebar-show-symbolic")
            .active(saved_state.left_sidebar_open)
            .tooltip_text("Toggle Icon Library Sidebar")
            .build();

        let new_icon_btn = gtk4::Button::builder()
            .icon_name("document-new-symbolic")
            .tooltip_text("Create Blank 16×16 GPA Icon")
            .build();

        let import_btn = gtk4::Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Import SVG or GPA Icon from Disk")
            .build();

        let save_lib_btn = gtk4::MenuButton::builder()
            .icon_name("folder-download-symbolic")
            .tooltip_text("Save Current Icon to Custom Library")
            .build();

        let export_btn = gtk4::Button::builder()
            .icon_name("document-save-as-symbolic")
            .tooltip_text("Export Current GPA Icon to File…")
            .build();

        header_bar.pack_start(&left_toggle_btn);
        header_bar.pack_start(&new_icon_btn);
        header_bar.pack_start(&import_btn);
        header_bar.pack_start(&save_lib_btn);
        header_bar.pack_start(&export_btn);

        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Search 1,100+ bundled GNOME & GPA icons…")
            .width_chars(34)
            .build();
        search_entry.add_css_class("gpa-header-search");
        header_bar.set_title_widget(Some(&search_entry));

        let mcp_pill_btn = gtk4::Button::builder()
            .label(format!("MCP :{mcp_port}"))
            .css_classes(["flat", "gpa-mcp-pill"])
            .tooltip_text("Embedded MCP Server active — click to view GPA_SPECS.md")
            .build();

        let right_toggle_btn = gtk4::ToggleButton::builder()
            .icon_name("sidebar-show-right-symbolic")
            .active(saved_state.right_sidebar_open)
            .tooltip_text("Toggle Shapes & GPA XML Sidebar")
            .build();

        header_bar.pack_end(&right_toggle_btn);
        header_bar.pack_end(&mcp_pill_btn);

        // --- 2. MAIN 3-ISLAND WORKSPACE WITH VERTICAL-DOTS RESIZE HANDLES ---
        let library_sidebar = LibrarySidebar::new(saved_state.left_sidebar_width);
        library_sidebar.set_revealed(saved_state.left_sidebar_open);

        let studio_canvas = StudioCanvas::new(initial_doc.clone());

        let inspector_sidebar = InspectorSidebar::new(mcp_port, saved_state.right_sidebar_width);
        inspector_sidebar.set_revealed(saved_state.right_sidebar_open);
        inspector_sidebar.update_document(&initial_doc, initial_doc.initial_state);

        let left_resize_handle = Self::build_resize_handle();
        left_resize_handle.set_visible(saved_state.left_sidebar_open);

        let right_resize_handle = Self::build_resize_handle();
        right_resize_handle.set_visible(saved_state.right_sidebar_open);

        let center_split_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        center_split_row.set_hexpand(true);
        center_split_row.set_vexpand(true);
        center_split_row.add_css_class("gpa-workspace-shell");
        center_split_row.append(library_sidebar.widget());
        center_split_row.append(&left_resize_handle);
        center_split_row.append(studio_canvas.widget());
        center_split_row.append(&right_resize_handle);
        center_split_row.append(inspector_sidebar.widget());

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(&center_split_row));

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&toolbar_view));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("GPA Icons Animator")
            .default_width(saved_state.window_width.clamp(860, 2560))
            .default_height(saved_state.window_height.clamp(560, 1600))
            .content(&toast_overlay)
            .build();

        let persist_ui_state: Rc<dyn Fn()> = {
            let win = window.clone();
            let lib = library_sidebar.clone();
            let insp = inspector_sidebar.clone();
            let l_btn = left_toggle_btn.clone();
            let r_btn = right_toggle_btn.clone();
            Rc::new(move || {
                let state = UiWindowState {
                    window_width: win.width().max(860),
                    window_height: win.height().max(560),
                    left_sidebar_width: lib.width(),
                    right_sidebar_width: insp.width(),
                    left_sidebar_open: l_btn.is_active(),
                    right_sidebar_open: r_btn.is_active(),
                };
                state.save();
            })
        };

        {
            let save_fn = persist_ui_state.clone();
            window.connect_close_request(move |_| {
                save_fn();
                glib::Propagation::Proceed
            });
        }

        // Wire both Left and Right Resize Handles using a single stationary GestureDrag on `center_split_row`
        {
            #[derive(Clone, Copy)]
            enum ActiveResize {
                None,
                Left(i32),
                Right(i32),
            }

            let drag = gtk4::GestureDrag::new();
            drag.set_propagation_phase(gtk4::PropagationPhase::Capture);
            let active_resize = Rc::new(Cell::new(ActiveResize::None));

            let lib_begin = library_sidebar.clone();
            let insp_begin = inspector_sidebar.clone();
            let l_handle_begin = left_resize_handle.clone();
            let r_handle_begin = right_resize_handle.clone();
            let parent_begin = center_split_row.clone();
            let state_begin = active_resize.clone();

            drag.connect_drag_begin(move |gesture, start_x, _| {
                #[allow(clippy::cast_possible_truncation)]
                let sx = start_x as f32;
                if l_handle_begin.is_visible()
                    && let Some(bounds) = l_handle_begin.compute_bounds(&parent_begin)
                    && sx >= bounds.x() - 8.0
                    && sx <= bounds.x() + bounds.width() + 8.0
                {
                    state_begin.set(ActiveResize::Left(lib_begin.width()));
                    gesture.set_state(gtk4::EventSequenceState::Claimed);
                    return;
                }
                if r_handle_begin.is_visible()
                    && let Some(bounds) = r_handle_begin.compute_bounds(&parent_begin)
                    && sx >= bounds.x() - 8.0
                    && sx <= bounds.x() + bounds.width() + 8.0
                {
                    state_begin.set(ActiveResize::Right(insp_begin.width()));
                    gesture.set_state(gtk4::EventSequenceState::Claimed);
                    return;
                }
                state_begin.set(ActiveResize::None);
            });

            let lib_upd = library_sidebar.clone();
            let insp_upd = inspector_sidebar.clone();
            let state_upd = active_resize.clone();
            drag.connect_drag_update(move |_, offset_x, _| {
                #[allow(clippy::cast_possible_truncation)]
                let dx = offset_x.round() as i32;
                match state_upd.get() {
                    ActiveResize::Left(start_w) => {
                        lib_upd.set_width((start_w + dx).clamp(170, 480));
                    }
                    ActiveResize::Right(start_w) => {
                        insp_upd.set_width((start_w - dx).clamp(200, 560));
                    }
                    ActiveResize::None => {}
                }
            });

            let save_fn = persist_ui_state.clone();
            let state_end = active_resize;
            drag.connect_drag_end(move |_, _, _| {
                if !matches!(state_end.get(), ActiveResize::None) {
                    state_end.set(ActiveResize::None);
                    save_fn();
                }
            });
            center_split_row.add_controller(drag);
        }

        // 1. Left & Right Sidebar Toggle Buttons
        {
            let lib = library_sidebar.clone();
            let l_handle = left_resize_handle;
            let save_fn = persist_ui_state.clone();
            left_toggle_btn.connect_toggled(move |btn| {
                let open = btn.is_active();
                lib.set_revealed(open);
                l_handle.set_visible(open);
                save_fn();
            });
        }
        {
            let insp = inspector_sidebar.clone();
            let r_handle = right_resize_handle;
            let save_fn = persist_ui_state;
            right_toggle_btn.connect_toggled(move |btn| {
                let open = btn.is_active();
                insp.set_revealed(open);
                r_handle.set_visible(open);
                save_fn();
            });
        }

        // 2. HeaderBar Search Entry -> LibrarySidebar
        {
            let lib = library_sidebar.clone();
            let left_btn = left_toggle_btn.clone();
            search_entry.connect_search_changed(move |entry| {
                if !left_btn.is_active() {
                    left_btn.set_active(true);
                }
                lib.set_search_query(&entry.text());
            });
        }

        // Helper closure to load any GpaDocumentSummary into Canvas + Inspector + MCP Snapshot
        let load_doc_into_ui: Rc<dyn Fn(GpaDocumentSummary)> = {
            let canvas = studio_canvas.clone();
            let inspector = inspector_sidebar.clone();
            let doc_slot = active_doc.clone();
            let bridge = mcp_bridge.clone();

            Rc::new(move |doc: GpaDocumentSummary| {
                canvas.load_document(&doc);
                inspector.update_document(&doc, doc.initial_state);
                *doc_slot.borrow_mut() = doc.clone();

                bridge.update_snapshot(StudioSnapshot {
                    active_state: doc.initial_state,
                    document: doc,
                    playing: canvas.is_playing(),
                    weight: 400.0,
                    show_grid: true,
                    show_bounds: false,
                });
            })
        };

        // 3. Clicking an icon in the Left Sidebar loads it; removing a custom icon shows a toast
        {
            let loader = load_doc_into_ui.clone();
            library_sidebar.connect_icon_selected(move |entry| {
                let normalized = ensure_gpa_headers(&entry.xml);
                let doc = GpaDocumentSummary::parse(entry.file_name, normalized);
                loader(doc);
            });
        }
        {
            let overlay = toast_overlay.clone();
            library_sidebar.connect_icon_removed(move |file_name, lib_label| {
                overlay.add_toast(adw::Toast::new(&format!(
                    "Removed '{file_name}' from {lib_label}"
                )));
            });
        }

        // 4. State changes on the Canvas update the Right Inspector + MCP Snapshot
        {
            let inspector = inspector_sidebar.clone();
            let doc_slot = active_doc.clone();
            let bridge = mcp_bridge.clone();
            studio_canvas.connect_state_changed(move |new_state| {
                let doc = doc_slot.borrow();
                inspector.update_active_state(&doc, new_state);
                bridge.update_active_state_in_snapshot(new_state);
            });
        }

        // 5. Any customization or state add/remove on Canvas syncs to Right Inspector + MCP Snapshot
        {
            let inspector = inspector_sidebar.clone();
            let doc_slot = active_doc.clone();
            let bridge = mcp_bridge;
            let canvas_ref = studio_canvas.clone();
            studio_canvas.connect_doc_mutated(move |updated_doc| {
                inspector.update_document(&updated_doc, updated_doc.initial_state);
                *doc_slot.borrow_mut() = updated_doc.clone();
                bridge.update_snapshot(StudioSnapshot {
                    active_state: updated_doc.initial_state,
                    document: updated_doc,
                    playing: canvas_ref.is_playing(),
                    weight: 400.0,
                    show_grid: true,
                    show_bounds: false,
                });
            });
        }

        // 6. Apply XML from the Right Sidebar XML editor
        {
            let loader = load_doc_into_ui.clone();
            let doc_slot = active_doc.clone();
            let overlay = toast_overlay.clone();
            inspector_sidebar.connect_apply_xml(move |new_xml| {
                let current_name = doc_slot.borrow().name.clone();
                let normalized = ensure_gpa_headers(&new_xml);
                let doc = GpaDocumentSummary::parse(current_name, normalized);
                loader(doc);
                overlay.add_toast(adw::Toast::new("Applied GPA XML to canvas"));
            });
        }

        // 7. New Blank Icon, Import SVG/GPA, and Export GPA buttons
        {
            let loader = load_doc_into_ui.clone();
            let overlay = toast_overlay.clone();
            new_icon_btn.connect_clicked(move |_| {
                let xml = empty_scratch_gpa("untitled");
                let doc = GpaDocumentSummary::parse("untitled.gpa", xml);
                loader(doc);
                overlay.add_toast(adw::Toast::new("Reset to blank 16×16 GPA canvas"));
            });
        }
        {
            let loader = load_doc_into_ui.clone();
            let overlay = toast_overlay.clone();
            let win_ref = window.clone();
            import_btn.connect_clicked(move |_| {
                let dialog = gtk4::FileDialog::builder()
                    .title("Import SVG or GPA Icon")
                    .modal(true)
                    .build();
                let loader_inner = loader.clone();
                let overlay_inner = overlay.clone();
                dialog.open(Some(&win_ref), gtk4::gio::Cancellable::NONE, move |res| {
                    if let Ok(file) = res
                        && let Some(path) = file.path()
                        && let Ok(raw_xml) = std::fs::read_to_string(&path)
                    {
                        let name = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("imported.gpa")
                            .to_string();
                        let normalized = ensure_gpa_headers(&raw_xml);
                        let doc = GpaDocumentSummary::parse(name.clone(), normalized);
                        loader_inner(doc);
                        overlay_inner.add_toast(adw::Toast::new(&format!("Imported '{name}'")));
                    }
                });
            });
        }
        Self::attach_save_to_library_popover(
            &save_lib_btn,
            &active_doc,
            &library_sidebar,
            &left_toggle_btn,
            &load_doc_into_ui,
            &toast_overlay,
        );

        {
            let doc_slot = active_doc;
            let overlay = toast_overlay.clone();
            let win_ref = window.clone();
            export_btn.connect_clicked(move |_| {
                let (name, xml) = {
                    let doc = doc_slot.borrow();
                    let base_stem = std::path::Path::new(&doc.name)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("animated-icon");
                    (format!("{base_stem}.gpa"), doc.xml.clone())
                };
                let dialog = gtk4::FileDialog::builder()
                    .title("Export GPA Icon")
                    .initial_name(&name)
                    .modal(true)
                    .build();
                let overlay_inner = overlay.clone();
                dialog.save(Some(&win_ref), gtk4::gio::Cancellable::NONE, move |res| {
                    if let Ok(file) = res
                        && let Some(path) = file.path()
                        && std::fs::write(&path, &xml).is_ok()
                    {
                        let saved_name = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("icon.gpa");
                        overlay_inner
                            .add_toast(adw::Toast::new(&format!("Exported '{saved_name}'")));
                    }
                });
            });
        }

        // 8. MCP Pill button -> reveals right sidebar & opens GPA_SPECS.md tab
        {
            let right_btn = right_toggle_btn;
            let insp = inspector_sidebar;
            mcp_pill_btn.connect_clicked(move |_| {
                if !right_btn.is_active() {
                    right_btn.set_active(true);
                }
                insp.show_specs_tab();
            });
        }

        // 9. Listen for live commands from the MCP Server on the GTK main thread
        {
            let loader = load_doc_into_ui;
            let canvas = studio_canvas;
            let overlay = toast_overlay;
            glib::spawn_future_local(async move {
                while let Ok(cmd) = ui_rx.recv().await {
                    match cmd {
                        McpUiCommand::LoadDocument {
                            name,
                            xml,
                            source_note,
                        } => {
                            let doc = GpaDocumentSummary::parse(name, xml);
                            loader(doc);
                            overlay.add_toast(adw::Toast::new(&source_note));
                        }
                        McpUiCommand::SetViewerState(st) => {
                            canvas.set_active_state(st);
                        }
                        McpUiCommand::SetViewerPlaying(p) => {
                            canvas.set_playing(p);
                        }
                        McpUiCommand::SetViewerWeight(w) => {
                            canvas.set_weight(w);
                        }
                        McpUiCommand::SetViewerDark(dark) => {
                            canvas.set_light_surface(!dark);
                        }
                        McpUiCommand::SetViewerGrid(show) => {
                            canvas.set_show_grid(show);
                        }
                        McpUiCommand::SetViewerXray(show) => {
                            canvas.set_show_xray(show);
                        }
                        McpUiCommand::LogEvent(msg) => {
                            overlay.add_toast(adw::Toast::new(&msg));
                        }
                    }
                }
            });
        }

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }

    /// Creates a subtle vertical 3-dots resize handle (`⋮`) in the gap between islands (no hover effect).
    fn build_resize_handle() -> gtk4::Box {
        let handle = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        handle.set_size_request(10, -1);
        handle.set_valign(gtk4::Align::Fill);
        handle.set_halign(gtk4::Align::Center);
        handle.add_css_class("gpa-resize-handle");
        handle.set_cursor_from_name(Some("ew-resize"));

        let grip_dots = gtk4::Label::new(Some("⋮"));
        grip_dots.set_valign(gtk4::Align::Center);
        grip_dots.set_halign(gtk4::Align::Center);
        grip_dots.set_vexpand(true);
        grip_dots.add_css_class("gpa-resize-dots");

        handle.append(&grip_dots);
        handle
    }

    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    fn attach_save_to_library_popover(
        save_lib_btn: &gtk4::MenuButton,
        active_doc: &Rc<RefCell<GpaDocumentSummary>>,
        library_sidebar: &LibrarySidebar,
        left_toggle_btn: &gtk4::ToggleButton,
        load_doc_into_ui: &Rc<dyn Fn(GpaDocumentSummary)>,
        toast_overlay: &adw::ToastOverlay,
    ) {
        let pop_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        pop_box.set_size_request(260, -1);
        pop_box.set_margin_top(10);
        pop_box.set_margin_bottom(10);
        pop_box.set_margin_start(10);
        pop_box.set_margin_end(10);

        let heading = gtk4::Label::new(Some("SAVE TO USER LIBRARY"));
        heading.add_css_class("gpa-section-heading");
        heading.set_halign(gtk4::Align::Start);

        let initial_folders = discover_user_libraries();
        let folder_labels: Vec<&str> = initial_folders.iter().map(|f| f.label.as_str()).collect();
        let folder_string_list = gtk4::StringList::new(&folder_labels);
        let folder_dropdown =
            gtk4::DropDown::new(Some(folder_string_list.clone()), gtk4::Expression::NONE);
        folder_dropdown.set_hexpand(true);
        folder_dropdown.add_css_class("gpa-compact-dropdown");

        let folders_slot: Rc<RefCell<Vec<UserLibraryFolder>>> =
            Rc::new(RefCell::new(initial_folders));

        let name_entry = gtk4::Entry::builder()
            .placeholder_text("icon-name")
            .hexpand(true)
            .build();

        let warning_label = gtk4::Label::new(None);
        warning_label.add_css_class("gpa-overwrite-warning");
        warning_label.set_wrap(true);
        warning_label.set_xalign(0.0);
        warning_label.set_halign(gtk4::Align::Start);
        warning_label.set_visible(false);

        let action_btn = gtk4::Button::builder()
            .label("Save to Library")
            .css_classes(["suggested-action", "gpa-save-lib-confirm-btn"])
            .build();

        let folder_hint = gtk4::Label::new(None);
        folder_hint.add_css_class("caption");
        folder_hint.add_css_class("dim-label");
        folder_hint.set_halign(gtk4::Align::Start);
        folder_hint.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);

        pop_box.append(&heading);
        pop_box.append(&folder_dropdown);
        pop_box.append(&name_entry);
        pop_box.append(&warning_label);
        pop_box.append(&action_btn);
        pop_box.append(&folder_hint);

        let popover = gtk4::Popover::builder().child(&pop_box).build();
        save_lib_btn.set_popover(Some(&popover));

        let refresh_warning_state: Rc<dyn Fn()> = {
            let folders_ref = folders_slot.clone();
            let dd = folder_dropdown.clone();
            let entry = name_entry.clone();
            let warn_lbl = warning_label;
            let btn = action_btn.clone();
            let hint_lbl = folder_hint;

            Rc::new(move || {
                let folders = folders_ref.borrow();
                let idx = dd.selected() as usize;
                let Some(folder) = folders.get(idx) else {
                    warn_lbl.set_visible(false);
                    return;
                };

                let stem = sanitize_icon_stem(&entry.text());
                hint_lbl.set_label(&format!("libs/{}/{stem}.gpa", folder.folder_name));

                if custom_icon_exists_in_dir(&folder.path, &stem) {
                    warn_lbl.set_label(&format!(
                        "⚠ '{stem}.gpa' already exists in '{}' and will be overwritten.",
                        folder.label
                    ));
                    warn_lbl.set_visible(true);
                    btn.set_label("Overwrite Icon");
                    btn.remove_css_class("suggested-action");
                    btn.add_css_class("gpa-overwrite-btn");
                } else {
                    warn_lbl.set_visible(false);
                    btn.set_label("Save to Library");
                    btn.remove_css_class("gpa-overwrite-btn");
                    btn.add_css_class("suggested-action");
                }
            })
        };

        // Re-scan user library folders and pre-fill current icon name whenever the popover opens
        {
            let folders_ref = folders_slot.clone();
            let str_list = folder_string_list;
            let dd = folder_dropdown.clone();
            let entry = name_entry.clone();
            let doc_slot = active_doc.clone();
            let lib_sidebar = library_sidebar.clone();
            let update_warn = refresh_warning_state.clone();

            popover.connect_show(move |_| {
                let active_user_folder = lib_sidebar
                    .selected_user_folder()
                    .map(|f| f.folder_name);

                let fresh_folders = discover_user_libraries();
                let labels: Vec<&str> = fresh_folders.iter().map(|f| f.label.as_str()).collect();
                let old_len = str_list.n_items();
                str_list.splice(0, old_len, &labels);

                let sel_idx = active_user_folder
                    .and_then(|target| {
                        fresh_folders.iter().position(|f| f.folder_name == target)
                    })
                    .unwrap_or(0);
                dd.set_selected(sel_idx as u32);
                *folders_ref.borrow_mut() = fresh_folders;

                let cur_stem = sanitize_icon_stem(&doc_slot.borrow().name);
                entry.set_text(&cur_stem);
                update_warn();
            });
        }

        {
            let update_warn = refresh_warning_state.clone();
            name_entry.connect_changed(move |_| {
                update_warn();
            });
        }
        {
            let update_warn = refresh_warning_state;
            folder_dropdown.connect_selected_notify(move |_| {
                update_warn();
            });
        }

        let perform_save: Rc<dyn Fn()> = {
            let folders_ref = folders_slot;
            let dd = folder_dropdown;
            let entry = name_entry.clone();
            let doc_slot = active_doc.clone();
            let lib_sidebar = library_sidebar.clone();
            let left_btn = left_toggle_btn.clone();
            let loader = load_doc_into_ui.clone();
            let overlay = toast_overlay.clone();
            let pop = popover;

            Rc::new(move || {
                let folders = folders_ref.borrow();
                let idx = dd.selected() as usize;
                let Some(folder) = folders.get(idx).cloned() else {
                    return;
                };
                drop(folders);

                let stem = sanitize_icon_stem(&entry.text());
                let xml = doc_slot.borrow().xml.clone();

                if let Ok((saved_path, overwritten)) =
                    save_custom_icon_in_dir(&folder.path, &stem, &xml)
                {
                    let file_name = saved_path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("icon.gpa")
                        .to_string();

                    let normalized = ensure_gpa_headers(&xml);
                    let updated_doc = GpaDocumentSummary::parse(file_name.clone(), normalized);
                    loader(updated_doc);

                    if !left_btn.is_active() {
                        left_btn.set_active(true);
                    }
                    lib_sidebar.refresh_and_select_user_folder(&folder.folder_name);
                    pop.popdown();

                    let msg = if overwritten {
                        format!("Overwrote '{file_name}' in {}", folder.label)
                    } else {
                        format!("Saved '{file_name}' to {}", folder.label)
                    };
                    overlay.add_toast(adw::Toast::new(&msg));
                }
            })
        };

        {
            let do_save = perform_save.clone();
            action_btn.connect_clicked(move |_| {
                do_save();
            });
        }
        {
            let do_save = perform_save;
            name_entry.connect_activate(move |_| {
                do_save();
            });
        }
    }
}
