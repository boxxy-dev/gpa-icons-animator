use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Persistent window and sidebar geometry state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiWindowState {
    pub window_width: i32,
    pub window_height: i32,
    pub left_sidebar_width: i32,
    pub right_sidebar_width: i32,
    pub left_sidebar_open: bool,
    pub right_sidebar_open: bool,
}

impl Default for UiWindowState {
    fn default() -> Self {
        Self {
            window_width: 1260,
            window_height: 820,
            left_sidebar_width: 240,
            right_sidebar_width: 320,
            left_sidebar_open: true,
            right_sidebar_open: true,
        }
    }
}

impl UiWindowState {
    #[must_use]
    pub fn load() -> Self {
        let path = get_config_dir().join("ui_state.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let dir = get_config_dir();
        if std::fs::create_dir_all(&dir).is_ok() {
            let path = dir.join("ui_state.json");
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }
}

/// Returns a reference to the global multi-threaded Tokio runtime.
///
/// # Panics
///
/// Panics if the Tokio runtime cannot be initialized.
pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("gpa-tokio-worker")
            .build()
            .expect("Failed to initialize global Tokio runtime")
    })
}

/// Resolves the active configuration directory for `gpa-icons-animator`.
/// Honours `GPA_CONFIG_DIR` when set to an absolute path.
#[must_use]
pub fn get_config_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var("GPA_CONFIG_DIR") {
        let trimmed = override_dir.trim();
        let path = Path::new(trimmed);
        if !trimmed.is_empty() && path.is_absolute() {
            return path.to_path_buf();
        }
        log::warn!("Ignoring non-absolute GPA_CONFIG_DIR='{override_dir}'");
    }

    if let Some(proj_dirs) = directories::ProjectDirs::from("dev", "boxxy", "gpa-icons-animator") {
        return proj_dirs.config_dir().to_path_buf();
    }

    home::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".config")
        .join("gpa-icons-animator")
}

/// Default folder name for the user's primary custom icon library (`<config_dir>/libs/my-icons`).
pub const DEFAULT_USER_LIBRARY_FOLDER: &str = "my-icons";

/// Resolves the custom user libraries root directory (`<config_dir>/libs`).
/// Every subdirectory inside this directory is discovered as a user icon library.
#[must_use]
pub fn get_libraries_dir() -> PathBuf {
    get_config_dir().join("libs")
}

/// Ensures `<config_dir>/libs/my-icons` exists on disk and returns its path.
#[must_use]
pub fn ensure_default_user_library() -> PathBuf {
    let default_lib = get_libraries_dir().join(DEFAULT_USER_LIBRARY_FOLDER);
    let _ = std::fs::create_dir_all(&default_lib);
    default_lib
}

/// Resolves the Streamable HTTP port for the embedded MCP server.
/// Defaults to `44820` unless `GPA_MCP_PORT` is set.
#[must_use]
pub fn get_mcp_port() -> u16 {
    std::env::var("GPA_MCP_PORT")
        .ok()
        .and_then(|v| v.trim().parse::<u16>().ok())
        .unwrap_or(44820)
}
