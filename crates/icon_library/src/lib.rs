use std::path::{Path, PathBuf};

use frizbee::{Config, Matcher};
use gpa_core::ensure_gpa_headers;
use gpa_sys_utils::{DEFAULT_USER_LIBRARY_FOLDER, ensure_default_user_library, get_libraries_dir};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconCategory {
    GpaExample,
    DevKit,
    UserCustom,
}

#[derive(Debug, Clone, Copy)]
pub struct BundledIconEntry {
    pub name: &'static str,
    pub file_name: &'static str,
    pub category: IconCategory,
    pub xml: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/bundled_icons.rs"));

/// Metadata describing a static built-in icon library.
#[derive(Debug, Clone, Copy)]
pub struct IconLibraryDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub category: Option<IconCategory>,
}

pub const ICON_LIBRARIES: &[IconLibraryDescriptor] = &[
    IconLibraryDescriptor {
        id: "dev-kit",
        label: "Icon Dev Kit",
        category: Some(IconCategory::DevKit),
    },
    IconLibraryDescriptor {
        id: "gpa-examples",
        label: "GPA Examples",
        category: Some(IconCategory::GpaExample),
    },
    IconLibraryDescriptor {
        id: "all",
        label: "All Libraries",
        category: None,
    },
];

/// A discovered user icon library folder on disk (e.g. `<config_dir>/libs/my-icons`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserLibraryFolder {
    /// Directory name inside `<config_dir>/libs/` (e.g. `"my-icons"`).
    pub folder_name: String,
    /// Human-friendly display title (e.g. `"My Icons"`).
    pub label: String,
    /// Path to the folder on disk.
    pub path: PathBuf,
}

/// Source backing a selectable library in the Left Sidebar dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibrarySource {
    DevKit,
    GpaExamples,
    UserFolder(UserLibraryFolder),
    All,
}

impl LibrarySource {
    #[must_use]
    pub const fn is_user_folder(&self) -> bool {
        matches!(self, Self::UserFolder(_))
    }

    #[must_use]
    pub const fn as_user_folder(&self) -> Option<&UserLibraryFolder> {
        if let Self::UserFolder(folder) = self {
            Some(folder)
        } else {
            None
        }
    }
}

/// Dynamic library entry combining built-in catalogs and discovered user folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectableLibrary {
    pub id: String,
    pub label: String,
    pub source: LibrarySource,
}

/// Unified icon item returned when browsing or searching any library (built-in or user folder).
#[derive(Debug, Clone)]
pub struct LibraryIconItem {
    pub name: String,
    pub file_name: String,
    pub xml: String,
    pub category: IconCategory,
    /// Populated when this icon comes from a custom user library folder on disk.
    pub user_folder: Option<UserLibraryFolder>,
    /// Populated with the `.gpa` / `.svg` file path on disk when removable.
    pub file_path: Option<PathBuf>,
}

impl LibraryIconItem {
    #[must_use]
    pub fn from_bundled(entry: &'static BundledIconEntry) -> Self {
        Self {
            name: entry.name.to_string(),
            file_name: entry.file_name.to_string(),
            xml: entry.xml.to_string(),
            category: entry.category,
            user_folder: None,
            file_path: None,
        }
    }

    #[must_use]
    pub const fn is_custom_user_icon(&self) -> bool {
        self.file_path.is_some()
    }
}

/// Converts a folder slug like `"my-icons"` or `"weather_pack"` into a friendly display name (`"My Icons"`).
#[must_use]
pub fn format_folder_label(folder_name: &str) -> String {
    let trimmed = folder_name.trim();
    if trimmed.is_empty() {
        return String::from("My Icons");
    }

    let words: Vec<String> = trimmed
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                let mut s = first.to_uppercase().collect::<String>();
                s.push_str(chars.as_str());
                s
            })
        })
        .collect();

    if words.is_empty() {
        trimmed.to_string()
    } else {
        words.join(" ")
    }
}

/// Discovers all user icon library folders inside `libs_root` (`<config_dir>/libs/`).
#[must_use]
pub fn discover_user_libraries_in(libs_root: &Path) -> Vec<UserLibraryFolder> {
    let mut folders = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(libs_root) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(folder_name) = path
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if folder_name.starts_with('.') {
                continue;
            }
            let label = format_folder_label(&folder_name);
            folders.push(UserLibraryFolder {
                folder_name,
                label,
                path,
            });
        }
    }

    folders.sort_by(|a, b| {
        let a_is_default = a.folder_name == DEFAULT_USER_LIBRARY_FOLDER;
        let b_is_default = b.folder_name == DEFAULT_USER_LIBRARY_FOLDER;
        b_is_default
            .cmp(&a_is_default)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });

    folders
}

/// Ensures the default `my-icons` folder exists under `<config_dir>/libs/` and discovers all user library folders.
#[must_use]
pub fn discover_user_libraries() -> Vec<UserLibraryFolder> {
    let _ = ensure_default_user_library();
    discover_user_libraries_in(&get_libraries_dir())
}

/// Returns the full dynamic list of selectable libraries for the Left Sidebar dropdown:
/// `Icon Dev Kit`, `GPA Examples`, all discovered user folders (e.g. `My Icons`), and `All Libraries`.
#[must_use]
pub fn discover_selectable_libraries() -> Vec<SelectableLibrary> {
    let mut list = vec![
        SelectableLibrary {
            id: String::from("dev-kit"),
            label: String::from("Icon Dev Kit"),
            source: LibrarySource::DevKit,
        },
        SelectableLibrary {
            id: String::from("gpa-examples"),
            label: String::from("GPA Examples"),
            source: LibrarySource::GpaExamples,
        },
    ];

    for folder in discover_user_libraries() {
        list.push(SelectableLibrary {
            id: format!("user:{}", folder.folder_name),
            label: folder.label.clone(),
            source: LibrarySource::UserFolder(folder),
        });
    }

    list.push(SelectableLibrary {
        id: String::from("all"),
        label: String::from("All Libraries"),
        source: LibrarySource::All,
    });

    list
}

/// Scans a single user library folder on disk for `.gpa` and `.svg` icon files.
#[must_use]
pub fn load_user_folder_icons(folder: &UserLibraryFolder) -> Vec<LibraryIconItem> {
    let mut items = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(&folder.path) else {
        return items;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase);
        if !matches!(ext.as_deref(), Some("gpa" | "svg")) {
            continue;
        }
        let Some(file_name) = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let Some(name) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let Ok(xml) = std::fs::read_to_string(&path) else {
            continue;
        };

        items.push(LibraryIconItem {
            name,
            file_name,
            xml,
            category: IconCategory::UserCustom,
            user_folder: Some(folder.clone()),
            file_path: Some(path),
        });
    }

    items.sort_by_key(|a| a.name.to_lowercase());
    items
}

/// Searches and fuzzy-ranks icons across any `LibrarySource` (built-in or user folder).
#[must_use]
pub fn search_library_items(
    query: &str,
    source: &LibrarySource,
    limit: usize,
) -> Vec<LibraryIconItem> {
    let candidates: Vec<LibraryIconItem> = match source {
        LibrarySource::DevKit => BUNDLED_ICONS
            .iter()
            .filter(|e| e.category == IconCategory::DevKit)
            .map(LibraryIconItem::from_bundled)
            .collect(),
        LibrarySource::GpaExamples => BUNDLED_ICONS
            .iter()
            .filter(|e| e.category == IconCategory::GpaExample)
            .map(LibraryIconItem::from_bundled)
            .collect(),
        LibrarySource::UserFolder(folder) => load_user_folder_icons(folder),
        LibrarySource::All => {
            let mut all = Vec::new();
            for folder in discover_user_libraries() {
                all.extend(load_user_folder_icons(&folder));
            }
            all.extend(BUNDLED_ICONS.iter().map(LibraryIconItem::from_bundled));
            all
        }
    };

    let trimmed = query.trim();
    if trimmed.is_empty() {
        return candidates.into_iter().take(limit).collect();
    }

    let haystacks: Vec<&str> = candidates.iter().map(|e| e.name.as_str()).collect();
    let mut matcher = Matcher::from_query(trimmed, &Config::default());
    let mut scored = matcher.match_list(&haystacks);
    scored.sort_by_key(|a| std::cmp::Reverse(a.score));

    scored
        .into_iter()
        .take(limit)
        .filter_map(|m| candidates.get(m.index as usize).cloned())
        .collect()
}

/// Sanitizes an icon name into a safe file stem (without `.gpa` / `.svg` extension).
#[must_use]
pub fn sanitize_icon_stem(raw_name: &str) -> String {
    let trimmed = raw_name.trim();
    let without_ext = trimmed
        .strip_suffix(".gpa")
        .or_else(|| trimmed.strip_suffix(".GPA"))
        .or_else(|| trimmed.strip_suffix(".svg"))
        .or_else(|| trimmed.strip_suffix(".SVG"))
        .unwrap_or(trimmed)
        .trim();

    let sanitized: String = without_ext
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();

    let clean = sanitized.trim_matches(['-', '.', ' ']);
    if clean.is_empty() {
        String::from("untitled")
    } else {
        clean.to_string()
    }
}

/// Checks whether an icon with `icon_name` already exists (`.gpa` or `.svg`) inside `folder_path`.
#[must_use]
pub fn custom_icon_exists_in_dir(folder_path: &Path, icon_name: &str) -> bool {
    let stem = sanitize_icon_stem(icon_name);
    let gpa_path = folder_path.join(format!("{stem}.gpa"));
    let svg_path = folder_path.join(format!("{stem}.svg"));
    gpa_path.exists() || svg_path.exists()
}

/// Saves `xml` as `<stem>.gpa` inside `folder_path`, overwriting any existing icon with the same name.
/// Returns `(saved_file_path, was_overwritten)`.
///
/// # Errors
/// Returns an `std::io::Error` if the folder cannot be created or the file cannot be written.
pub fn save_custom_icon_in_dir(
    folder_path: &Path,
    icon_name: &str,
    xml: &str,
) -> std::io::Result<(PathBuf, bool)> {
    std::fs::create_dir_all(folder_path)?;
    let stem = sanitize_icon_stem(icon_name);
    let gpa_path = folder_path.join(format!("{stem}.gpa"));
    let svg_path = folder_path.join(format!("{stem}.svg"));

    let existed = gpa_path.exists() || svg_path.exists();
    if svg_path.exists() && svg_path != gpa_path {
        let _ = std::fs::remove_file(&svg_path);
    }

    let normalized = ensure_gpa_headers(xml);
    std::fs::write(&gpa_path, normalized)?;
    Ok((gpa_path, existed))
}

/// Removes a custom icon file from a user library folder on disk.
///
/// # Errors
/// Returns an `std::io::Error` if the file cannot be removed.
pub fn remove_custom_icon_file(file_path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(file_path)
}

/// Returns the static catalog of built-in icon libraries.
#[must_use]
pub const fn icon_libraries() -> &'static [IconLibraryDescriptor] {
    ICON_LIBRARIES
}

/// Returns the complete slice of bundled icons (GPA examples + Icon Development Kit).
#[must_use]
pub fn all_icons() -> &'static [BundledIconEntry] {
    BUNDLED_ICONS
}

/// Looks up a bundled icon by exact name or filename.
#[must_use]
pub fn find_icon(name_or_file: &str) -> Option<&'static BundledIconEntry> {
    let q = name_or_file.trim();
    BUNDLED_ICONS
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(q) || entry.file_name.eq_ignore_ascii_case(q))
}

/// Filters and fuzzy-ranks bundled icons using `frizbee`.
#[must_use]
pub fn search_icons(
    query: &str,
    category_filter: Option<IconCategory>,
    limit: usize,
) -> Vec<&'static BundledIconEntry> {
    let candidates: Vec<&'static BundledIconEntry> = BUNDLED_ICONS
        .iter()
        .filter(|entry| category_filter.is_none_or(|cat| entry.category == cat))
        .collect();

    let trimmed = query.trim();
    if trimmed.is_empty() {
        return candidates.into_iter().take(limit).collect();
    }

    let haystacks: Vec<&str> = candidates.iter().map(|e| e.name).collect();
    let mut matcher = Matcher::from_query(trimmed, &Config::default());
    let mut scored = matcher.match_list(&haystacks);
    scored.sort_by_key(|a| std::cmp::Reverse(a.score));

    scored
        .into_iter()
        .take(limit)
        .filter_map(|m| candidates.get(m.index as usize).copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_folder_label() {
        assert_eq!(format_folder_label("my-icons"), "My Icons");
        assert_eq!(format_folder_label("weather_pack"), "Weather Pack");
        assert_eq!(format_folder_label("CustomSet"), "CustomSet");
    }

    #[test]
    fn test_sanitize_icon_stem() {
        assert_eq!(sanitize_icon_stem("my-icon.gpa"), "my-icon");
        assert_eq!(sanitize_icon_stem("  fancy icon.svg "), "fancy-icon");
        assert_eq!(sanitize_icon_stem(""), "untitled");
    }

    #[test]
    fn test_user_library_discovery_save_overwrite_and_remove() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let libs_root = tmp.path().join("libs");
        let my_icons_dir = libs_root.join("my-icons");
        let extra_dir = libs_root.join("audio-icons");
        std::fs::create_dir_all(&my_icons_dir).expect("create my-icons");
        std::fs::create_dir_all(&extra_dir).expect("create audio-icons");

        let discovered = discover_user_libraries_in(&libs_root);
        assert_eq!(discovered.len(), 2);
        assert_eq!(discovered[0].folder_name, "my-icons");
        assert_eq!(discovered[0].label, "My Icons");
        assert_eq!(discovered[1].folder_name, "audio-icons");
        assert_eq!(discovered[1].label, "Audio Icons");

        let sample_xml = "<svg width='16' height='16'><path id='p0' d='M 0 0 L 16 16'/></svg>";
        assert!(!custom_icon_exists_in_dir(&my_icons_dir, "sparkle"));

        let (saved_path, overwritten) =
            save_custom_icon_in_dir(&my_icons_dir, "sparkle.gpa", sample_xml).expect("save first");
        assert!(!overwritten);
        assert!(saved_path.exists());
        assert!(custom_icon_exists_in_dir(&my_icons_dir, "sparkle"));

        let items = load_user_folder_icons(&discovered[0]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "sparkle");
        assert_eq!(items[0].file_name, "sparkle.gpa");
        assert!(items[0].is_custom_user_icon());

        let (_, overwritten_second) =
            save_custom_icon_in_dir(&my_icons_dir, "sparkle", sample_xml).expect("save overwrite");
        assert!(overwritten_second);

        remove_custom_icon_file(&saved_path).expect("remove icon");
        assert!(!custom_icon_exists_in_dir(&my_icons_dir, "sparkle"));
        assert!(load_user_folder_icons(&discovered[0]).is_empty());
    }
}
