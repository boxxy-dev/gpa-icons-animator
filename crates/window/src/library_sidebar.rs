use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::SystemTime;

use gpa_icon_library::{
    IconCategory, LibraryIconItem, LibrarySource, SelectableLibrary, UserLibraryFolder,
    discover_selectable_libraries, discover_user_libraries, remove_custom_icon_file,
    search_library_items,
};
use gtk4::glib;
use gtk4::prelude::*;

type IconSelectCallback = Rc<RefCell<Option<Box<dyn Fn(LibraryIconItem)>>>>;
type IconRemoveCallback = Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>;
type FsFingerprint = Rc<RefCell<Vec<(String, usize, Option<SystemTime>)>>>;

const LIST_BATCH_SIZE: usize = 80;
const GRID_BATCH_SIZE: usize = 120;

/// Left collapsible & resizable sidebar with a rounded island surface:
/// - Top Header: Library Selector Dropdown (`Icon Dev Kit`, `GPA Examples`, discovered user folders
///   like `My Icons` in `<config_dir>/libs/`, and `All Libraries`), true total icon count badge,
///   and List vs Grid view switcher.
/// - Custom user folders support opening and removing icons directly from the sidebar.
/// - Scroll-driven lazy loading (`append_next_batch`) so 1,100+ icons load instantaneously.
#[derive(Clone)]
pub struct LibrarySidebar {
    revealer: gtk4::Revealer,
    container: gtk4::Box,
    lib_dropdown: gtk4::DropDown,
    lib_string_list: gtk4::StringList,
    libraries: Rc<RefCell<Vec<SelectableLibrary>>>,
    updating_dropdown: Rc<Cell<bool>>,
    view_stack: gtk4::Stack,
    list_scrolled: gtk4::ScrolledWindow,
    grid_scrolled: gtk4::ScrolledWindow,
    kit_list_box: gtk4::ListBox,
    kit_grid_box: gtk4::FlowBox,
    count_label: gtk4::Label,
    grid_mode: Rc<Cell<bool>>,
    current_query: Rc<RefCell<String>>,
    filtered_entries: Rc<RefCell<Vec<LibraryIconItem>>>,
    rendered_count: Rc<Cell<usize>>,
    last_fs_fingerprint: FsFingerprint,
    on_select: IconSelectCallback,
    on_remove: IconRemoveCallback,
}

impl LibrarySidebar {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(initial_width: i32) -> Self {
        let current_query = Rc::new(RefCell::new(String::new()));
        let grid_mode = Rc::new(Cell::new(false));
        let filtered_entries = Rc::new(RefCell::new(Vec::new()));
        let rendered_count = Rc::new(Cell::new(0));
        let updating_dropdown = Rc::new(Cell::new(false));
        let on_select: IconSelectCallback = Rc::new(RefCell::new(None));
        let on_remove: IconRemoveCallback = Rc::new(RefCell::new(None));

        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        container.set_size_request(initial_width.clamp(170, 480), -1);
        container.set_hexpand(false);
        container.set_overflow(gtk4::Overflow::Hidden);
        container.add_css_class("gpa-island-panel");

        // Top Header with Library Selector DropDown + True Count + List / Grid Mode Toggle
        let header_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        header_row.set_margin_start(4);
        header_row.set_margin_end(4);
        header_row.set_margin_top(2);
        header_row.set_valign(gtk4::Align::Center);

        let initial_libs = discover_selectable_libraries();
        let lib_labels: Vec<&str> = initial_libs.iter().map(|l| l.label.as_str()).collect();
        let lib_string_list = gtk4::StringList::new(&lib_labels);
        let lib_dropdown = gtk4::DropDown::new(Some(lib_string_list.clone()), gtk4::Expression::NONE);
        lib_dropdown.set_selected(0);
        lib_dropdown.set_hexpand(true);
        lib_dropdown.set_halign(gtk4::Align::Fill);
        lib_dropdown.add_css_class("gpa-compact-dropdown");
        lib_dropdown.set_tooltip_text(Some(
            "Switch Icon Library (Icon Dev Kit, GPA Examples, or Custom User Libraries)",
        ));

        let libraries = Rc::new(RefCell::new(initial_libs));
        let last_fs_fingerprint = Rc::new(RefCell::new(Self::compute_fs_fingerprint()));

        let count_label = gtk4::Label::new(Some(""));
        count_label.add_css_class("caption");
        count_label.add_css_class("dim-label");

        let mode_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        let list_mode_btn = gtk4::ToggleButton::builder()
            .icon_name("view-list-symbolic")
            .active(true)
            .css_classes(["flat", "gpa-view-mode-btn"])
            .tooltip_text("List View (with icon names)")
            .build();
        let grid_mode_btn = gtk4::ToggleButton::builder()
            .icon_name("view-grid-symbolic")
            .group(&list_mode_btn)
            .css_classes(["flat", "gpa-view-mode-btn"])
            .tooltip_text("Grid View (icons only)")
            .build();
        mode_box.append(&list_mode_btn);
        mode_box.append(&grid_mode_btn);

        header_row.append(&lib_dropdown);
        header_row.append(&count_label);
        header_row.append(&mode_box);

        let kit_list_box = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .css_classes(["navigation-sidebar", "gpa-icon-list"])
            .build();

        let list_scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .propagate_natural_width(false)
            .vexpand(true)
            .child(&kit_list_box)
            .build();

        let kit_grid_box = gtk4::FlowBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .homogeneous(true)
            .valign(gtk4::Align::Start)
            .max_children_per_line(8)
            .min_children_per_line(3)
            .column_spacing(4)
            .row_spacing(4)
            .css_classes(["gpa-icon-grid"])
            .build();

        let grid_scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .propagate_natural_width(false)
            .vexpand(true)
            .child(&kit_grid_box)
            .build();

        let view_stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .transition_duration(120)
            .vexpand(true)
            .build();
        view_stack.add_named(&list_scrolled, Some("list"));
        view_stack.add_named(&grid_scrolled, Some("grid"));

        container.append(&header_row);
        container.append(&view_stack);

        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideRight)
            .transition_duration(180)
            .reveal_child(true)
            .hexpand(false)
            .child(&container)
            .build();

        revealer.connect_child_revealed_notify(|rev| {
            if !rev.is_child_revealed() && !rev.reveals_child() {
                rev.set_visible(false);
            }
        });

        let sidebar = Self {
            revealer,
            container,
            lib_dropdown: lib_dropdown.clone(),
            lib_string_list,
            libraries,
            updating_dropdown,
            view_stack,
            list_scrolled: list_scrolled.clone(),
            grid_scrolled: grid_scrolled.clone(),
            kit_list_box,
            kit_grid_box,
            count_label,
            grid_mode,
            current_query,
            filtered_entries,
            rendered_count,
            last_fs_fingerprint,
            on_select,
            on_remove,
        };

        // Switch library dropdown -> reset and lazy-populate
        {
            let s = sidebar.clone();
            lib_dropdown.connect_selected_notify(move |_| {
                if !s.updating_dropdown.get() {
                    s.reset_and_populate();
                }
            });
        }

        // Switch List / Grid mode -> reset and lazy-populate (keeping identical total count)
        {
            let s = sidebar.clone();
            list_mode_btn.connect_toggled(move |btn| {
                if btn.is_active() {
                    s.grid_mode.set(false);
                    s.view_stack.set_visible_child_name("list");
                    s.reset_and_populate();
                }
            });
        }
        {
            let s = sidebar.clone();
            grid_mode_btn.connect_toggled(move |btn| {
                if btn.is_active() {
                    s.grid_mode.set(true);
                    s.view_stack.set_visible_child_name("grid");
                    s.reset_and_populate();
                }
            });
        }

        // Scroll-driven lazy loading on List ScrolledWindow
        {
            let s = sidebar.clone();
            let adj = list_scrolled.vadjustment();
            adj.connect_value_changed(move |adj| {
                if !s.grid_mode.get() && adj.value() + adj.page_size() >= adj.upper() - 260.0 {
                    s.append_next_batch();
                }
            });
            let s_up = sidebar.clone();
            adj.connect_upper_notify(move |adj| {
                if !s_up.grid_mode.get()
                    && adj.page_size() > 0.0
                    && adj.value() + adj.page_size() >= adj.upper() - 260.0
                {
                    s_up.append_next_batch();
                }
            });
        }

        // Scroll-driven lazy loading on Grid ScrolledWindow
        {
            let s = sidebar.clone();
            let adj = grid_scrolled.vadjustment();
            adj.connect_value_changed(move |adj| {
                if s.grid_mode.get() && adj.value() + adj.page_size() >= adj.upper() - 260.0 {
                    s.append_next_batch();
                }
            });
            let s_up = sidebar.clone();
            adj.connect_upper_notify(move |adj| {
                if s_up.grid_mode.get()
                    && adj.page_size() > 0.0
                    && adj.value() + adj.page_size() >= adj.upper() - 260.0
                {
                    s_up.append_next_batch();
                }
            });
        }

        // Lightweight filesystem poll so manually created folders or icons in `<config_dir>/libs/`
        // are automatically discovered without needing to restart the app.
        {
            let s = sidebar.clone();
            glib::timeout_add_seconds_local(2, move || {
                s.check_filesystem_changes();
                glib::ControlFlow::Continue
            });
        }

        sidebar.reset_and_populate();
        sidebar
    }

    #[must_use]
    pub const fn widget(&self) -> &gtk4::Revealer {
        &self.revealer
    }

    pub fn set_width(&self, width: i32) {
        self.container.set_size_request(width.clamp(170, 480), -1);
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        let req = self.container.width_request();
        if req >= 170 {
            req
        } else {
            self.container.width().clamp(170, 480)
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

    pub fn connect_icon_selected<F: Fn(LibraryIconItem) + 'static>(&self, f: F) {
        *self.on_select.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_icon_removed<F: Fn(String, String) + 'static>(&self, f: F) {
        *self.on_remove.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_search_query(&self, query: &str) {
        *self.current_query.borrow_mut() = query.to_string();
        self.reset_and_populate();
    }

    /// Returns the currently selected `UserLibraryFolder` if the user is viewing a custom folder.
    #[must_use]
    pub fn selected_user_folder(&self) -> Option<UserLibraryFolder> {
        let libs = self.libraries.borrow();
        let idx = self.lib_dropdown.selected() as usize;
        libs.get(idx)
            .and_then(|l| l.source.as_user_folder())
            .cloned()
    }

    /// Re-scans `<config_dir>/libs/` for user folders and selects `folder_name` (e.g. `"my-icons"`).
    pub fn refresh_and_select_user_folder(&self, folder_name: &str) {
        let target_id = format!("user:{folder_name}");
        self.reload_library_dropdown(Some(&target_id));
        *self.last_fs_fingerprint.borrow_mut() = Self::compute_fs_fingerprint();
        self.reset_and_populate();
    }

    /// Re-scans `<config_dir>/libs/` while preserving the current dropdown selection.
    pub fn refresh_libraries(&self) {
        self.reload_library_dropdown(None);
        *self.last_fs_fingerprint.borrow_mut() = Self::compute_fs_fingerprint();
        self.reset_and_populate();
    }

    fn compute_fs_fingerprint() -> Vec<(String, usize, Option<SystemTime>)> {
        discover_user_libraries()
            .into_iter()
            .map(|folder| {
                let mut file_count = 0usize;
                let mut latest_mtime: Option<SystemTime> = None;
                if let Ok(rd) = std::fs::read_dir(&folder.path) {
                    for entry in rd.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            file_count += 1;
                            if let Ok(meta) = entry.metadata()
                                && let Ok(mtime) = meta.modified()
                            {
                                latest_mtime = Some(
                                    latest_mtime.map_or(mtime, |prev| prev.max(mtime)),
                                );
                            }
                        }
                    }
                }
                (folder.folder_name, file_count, latest_mtime)
            })
            .collect()
    }

    fn check_filesystem_changes(&self) {
        let current_fp = Self::compute_fs_fingerprint();
        let changed = *self.last_fs_fingerprint.borrow() != current_fp;
        if !changed {
            return;
        }
        *self.last_fs_fingerprint.borrow_mut() = current_fp;
        self.reload_library_dropdown(None);
        if matches!(
            self.selected_source(),
            LibrarySource::UserFolder(_) | LibrarySource::All
        ) {
            self.reset_and_populate();
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reload_library_dropdown(&self, force_select_id: Option<&str>) {
        let prev_id = force_select_id.map(str::to_string).or_else(|| {
            let libs = self.libraries.borrow();
            let idx = self.lib_dropdown.selected() as usize;
            libs.get(idx).map(|l| l.id.clone())
        });

        let new_libs = discover_selectable_libraries();
        let new_labels: Vec<&str> = new_libs.iter().map(|l| l.label.as_str()).collect();

        self.updating_dropdown.set(true);
        let old_len = self.lib_string_list.n_items();
        self.lib_string_list.splice(0, old_len, &new_labels);

        let selected_idx = prev_id
            .and_then(|id| new_libs.iter().position(|l| l.id == id))
            .unwrap_or(0);
        self.lib_dropdown.set_selected(selected_idx as u32);
        *self.libraries.borrow_mut() = new_libs;
        self.updating_dropdown.set(false);
    }

    fn selected_source(&self) -> LibrarySource {
        let libs = self.libraries.borrow();
        let idx = self.lib_dropdown.selected() as usize;
        libs.get(idx)
            .map_or(LibrarySource::DevKit, |l| l.source.clone())
    }

    fn reset_and_populate(&self) {
        let query = self.current_query.borrow().clone();
        let source = self.selected_source();
        let matches = search_library_items(&query, &source, usize::MAX);

        // Always display the true total count of matching icons in this library
        let is_empty = matches.is_empty();
        self.count_label.set_label(&format!("{}", matches.len()));

        *self.filtered_entries.borrow_mut() = matches;
        self.rendered_count.set(0);

        if self.grid_mode.get() {
            while let Some(child) = self.kit_grid_box.first_child() {
                self.kit_grid_box.remove(&child);
            }
            self.grid_scrolled.vadjustment().set_value(0.0);
        } else {
            while let Some(child) = self.kit_list_box.first_child() {
                self.kit_list_box.remove(&child);
            }
            self.list_scrolled.vadjustment().set_value(0.0);
        }

        if is_empty
            && let LibrarySource::UserFolder(folder) = &source
            && !self.grid_mode.get()
        {
            let empty_row = Self::build_empty_user_folder_row(folder);
            self.kit_list_box.append(&empty_row);
            return;
        }

        self.append_next_batch();
    }

    fn build_empty_user_folder_row(folder: &UserLibraryFolder) -> gtk4::ListBoxRow {
        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        vbox.set_margin_top(18);
        vbox.set_margin_bottom(18);
        vbox.set_margin_start(10);
        vbox.set_margin_end(10);

        let title = gtk4::Label::new(Some(&format!("No icons in '{}'", folder.label)));
        title.add_css_class("heading");
        title.set_halign(gtk4::Align::Start);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let hint = gtk4::Label::new(Some(&format!(
            "Use the Save to Library button in the headerbar or place .gpa/.svg files in:\nlibs/{}",
            folder.folder_name
        )));
        hint.add_css_class("caption");
        hint.add_css_class("dim-label");
        hint.set_wrap(true);
        hint.set_xalign(0.0);
        hint.set_halign(gtk4::Align::Start);

        vbox.append(&title);
        vbox.append(&hint);

        gtk4::ListBoxRow::builder()
            .selectable(false)
            .activatable(false)
            .child(&vbox)
            .build()
    }

    fn append_next_batch(&self) {
        let entries = self.filtered_entries.borrow();
        let start = self.rendered_count.get();
        if start >= entries.len() {
            return;
        }

        let is_grid = self.grid_mode.get();
        let batch_size = if is_grid {
            GRID_BATCH_SIZE
        } else {
            LIST_BATCH_SIZE
        };
        let end = (start + batch_size).min(entries.len());

        if is_grid {
            for entry in &entries[start..end] {
                let cell = self.build_grid_icon_cell(entry);
                self.kit_grid_box.insert(&cell, -1);
            }
        } else {
            for entry in &entries[start..end] {
                let is_gpa = entry.category == IconCategory::GpaExample;
                let row = self.build_icon_row(entry, is_gpa);
                self.kit_list_box.append(&row);
            }
        }

        self.rendered_count.set(end);
    }

    fn remove_custom_entry(&self, item: &LibraryIconItem) {
        let Some(path) = item.file_path.as_deref() else {
            return;
        };
        if remove_custom_icon_file(path).is_ok() {
            let folder_label = item
                .user_folder
                .as_ref()
                .map_or_else(|| String::from("Custom Library"), |f| f.label.clone());
            *self.last_fs_fingerprint.borrow_mut() = Self::compute_fs_fingerprint();
            self.reset_and_populate();
            if let Some(cb) = self.on_remove.borrow().as_ref() {
                cb(item.file_name.clone(), folder_label);
            }
        }
    }

    fn attach_custom_context_popover(
        &self,
        target_widget: &impl IsA<gtk4::Widget>,
        item: &LibraryIconItem,
    ) {
        if !item.is_custom_user_icon() {
            return;
        }

        let menu_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        menu_box.set_margin_top(4);
        menu_box.set_margin_bottom(4);
        menu_box.set_margin_start(4);
        menu_box.set_margin_end(4);

        let open_btn = gtk4::Button::builder()
            .label("Open Icon")
            .css_classes(["flat", "gpa-context-menu-btn"])
            .build();
        let remove_btn = gtk4::Button::builder()
            .label("Remove Icon")
            .css_classes(["flat", "gpa-context-menu-btn", "gpa-context-menu-danger"])
            .build();

        menu_box.append(&open_btn);
        menu_box.append(&remove_btn);

        let popover = gtk4::Popover::builder()
            .has_arrow(false)
            .child(&menu_box)
            .build();
        popover.set_parent(target_widget);

        {
            let pop = popover.clone();
            let cb_ref = self.on_select.clone();
            let item_clone = item.clone();
            open_btn.connect_clicked(move |_| {
                pop.popdown();
                if let Some(cb) = cb_ref.borrow().as_ref() {
                    cb(item_clone.clone());
                }
            });
        }

        {
            let pop = popover.clone();
            let sidebar = self.clone();
            let item_clone = item.clone();
            remove_btn.connect_clicked(move |_| {
                pop.popdown();
                sidebar.remove_custom_entry(&item_clone);
            });
        }

        let right_click = gtk4::GestureClick::builder().button(3).build();
        right_click.connect_released(move |gesture, _, _, _| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            popover.popup();
        });
        target_widget.add_controller(right_click);
    }

    fn build_icon_row(&self, entry: &LibraryIconItem, show_gpa_badge: bool) -> gtk4::ListBoxRow {
        let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        row_box.set_margin_top(5);
        row_box.set_margin_bottom(5);
        row_box.set_margin_start(8);
        row_box.set_margin_end(6);

        let bytes = glib::Bytes::from(entry.xml.as_bytes());
        let svg = gtk4::Svg::from_bytes(&bytes);
        let img = gtk4::Image::builder()
            .pixel_size(20)
            .paintable(&svg)
            .build();

        let name_lbl = gtk4::Label::new(Some(&entry.name));
        name_lbl.set_halign(gtk4::Align::Start);
        name_lbl.set_hexpand(true);
        name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        row_box.append(&img);
        row_box.append(&name_lbl);

        if show_gpa_badge {
            let gpa_tag = gtk4::Label::new(Some("GPA"));
            gpa_tag.add_css_class("gpa-mini-tag");
            row_box.append(&gpa_tag);
        }

        if entry.is_custom_user_icon() {
            let remove_btn = gtk4::Button::builder()
                .icon_name("user-trash-symbolic")
                .valign(gtk4::Align::Center)
                .css_classes(["flat", "gpa-custom-remove-btn"])
                .tooltip_text("Remove icon from custom library")
                .build();
            let sidebar = self.clone();
            let item_clone = entry.clone();
            remove_btn.connect_clicked(move |_| {
                sidebar.remove_custom_entry(&item_clone);
            });
            row_box.append(&remove_btn);
        }

        let row = gtk4::ListBoxRow::builder().child(&row_box).build();

        let click = gtk4::GestureClick::builder().button(1).build();
        let cb_ref = self.on_select.clone();
        let item_clone = entry.clone();
        click.connect_released(move |_, _, _, _| {
            if let Some(cb) = cb_ref.borrow().as_ref() {
                cb(item_clone.clone());
            }
        });
        row.add_controller(click);

        self.attach_custom_context_popover(&row, entry);
        row
    }

    fn build_grid_icon_cell(&self, entry: &LibraryIconItem) -> gtk4::FlowBoxChild {
        let bytes = glib::Bytes::from(entry.xml.as_bytes());
        let svg = gtk4::Svg::from_bytes(&bytes);
        let img = gtk4::Image::builder()
            .pixel_size(22)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .paintable(&svg)
            .build();

        let tooltip = if entry.is_custom_user_icon() {
            format!("{} (Right-click to Open or Remove)", entry.name)
        } else {
            entry.name.clone()
        };

        let cell_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        cell_box.set_size_request(38, 38);
        cell_box.set_halign(gtk4::Align::Center);
        cell_box.set_valign(gtk4::Align::Center);
        cell_box.set_tooltip_text(Some(&tooltip));
        cell_box.append(&img);

        let child = gtk4::FlowBoxChild::builder()
            .child(&cell_box)
            .css_classes(["gpa-grid-icon-cell"])
            .build();

        let click = gtk4::GestureClick::builder().button(1).build();
        let cb_ref = self.on_select.clone();
        let item_clone = entry.clone();
        click.connect_released(move |_, _, _, _| {
            if let Some(cb) = cb_ref.borrow().as_ref() {
                cb(item_clone.clone());
            }
        });
        child.add_controller(click);

        self.attach_custom_context_popover(&child, entry);
        child
    }
}

impl Default for LibrarySidebar {
    fn default() -> Self {
        Self::new(240)
    }
}
