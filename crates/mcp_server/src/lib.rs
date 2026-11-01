pub mod http_server;

use std::sync::Arc;

use arc_swap::ArcSwap;
use async_channel::Sender;
use gpa_core::{
    GPA_SPECS_MARKDOWN, GpaDocumentSummary, GpaShapePatch, add_state_to_gpa, default_blank_gpa,
    delete_shape_from_gpa, duplicate_shape_in_gpa, ensure_gpa_headers, patch_shape_in_gpa,
    remove_state_from_gpa, rename_state_in_gpa, set_all_states_in_gpa, upsert_shape_in_gpa,
    validate_gpa_xml,
};
use gpa_icon_library::{find_icon, search_icons};
pub use http_server::{bind_and_spawn_http_server, spawn_mcp_http_server};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{
        CallToolResult, ContentBlock, ListResourcesResult, PaginatedRequestParams,
        ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
        ResourceContents, ServerCapabilities, ServerConfig,
    },
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Commands sent from the MCP server (or internal actions) to the GTK main thread.
#[derive(Debug, Clone)]
pub enum McpUiCommand {
    LoadDocument {
        name: String,
        xml: String,
        source_note: String,
    },
    SetViewerState(u32),
    SetViewerPlaying(bool),
    SetViewerWeight(f64),
    SetViewerDark(bool),
    SetViewerGrid(bool),
    SetViewerXray(bool),
    LogEvent(String),
}

/// Snapshot of the active studio state readable lock-free by MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioSnapshot {
    pub document: GpaDocumentSummary,
    pub active_state: u32,
    pub playing: bool,
    pub weight: f64,
    pub show_grid: bool,
    pub show_bounds: bool,
}

/// Shared bridge between the GTK4 UI and the MCP Server.
#[derive(Clone)]
pub struct McpBridge {
    pub snapshot: Arc<ArcSwap<StudioSnapshot>>,
    pub ui_tx: Sender<McpUiCommand>,
    pub port: u16,
}

impl McpBridge {
    #[must_use]
    pub fn new(initial_doc: GpaDocumentSummary, ui_tx: Sender<McpUiCommand>, port: u16) -> Self {
        let initial_state = initial_doc.initial_state;
        let snap = StudioSnapshot {
            document: initial_doc,
            active_state: initial_state,
            playing: true,
            weight: 400.0,
            show_grid: true,
            show_bounds: false,
        };
        Self {
            snapshot: Arc::new(ArcSwap::from_pointee(snap)),
            ui_tx,
            port,
        }
    }

    pub fn update_snapshot(&self, next: StudioSnapshot) {
        self.snapshot.store(Arc::new(next));
    }

    pub fn update_active_state_in_snapshot(&self, new_state: u32) {
        let cur = self.snapshot.load_full();
        let mut next = (*cur).clone();
        next.active_state = new_state;
        self.snapshot.store(Arc::new(next));
    }

    /// Spawns the background Streamable HTTP MCP server on `127.0.0.1:<port>`.
    pub fn spawn_http_server(&self) {
        spawn_mcp_http_server(self.clone());
    }

    /// Returns the embedded `GPA_SPECS.md` content for MCP prompts/resources.
    #[must_use]
    pub const fn specs_markdown(&self) -> &'static str {
        GPA_SPECS_MARKDOWN
    }

    /// Helper that executes a library load request (callable from MCP or test harness).
    ///
    /// # Errors
    /// Returns an error if no bundled icon matching `name` exists.
    pub fn load_library_icon(&self, name: &str) -> Result<GpaDocumentSummary, String> {
        let entry =
            find_icon(name).ok_or_else(|| format!("Icon '{name}' not found in bundled library"))?;
        let xml = ensure_gpa_headers(entry.xml);
        let summary = GpaDocumentSummary::parse(entry.file_name, xml.clone());
        self.commit_document(
            summary.clone(),
            xml,
            format!("MCP loaded bundled icon '{}'", entry.file_name),
            Some(summary.initial_state),
        );
        Ok(summary)
    }

    /// Helper that replaces the active GPA XML from an MCP tool call.
    #[must_use]
    pub fn apply_gpa_xml(&self, name: Option<String>, xml: &str) -> GpaDocumentSummary {
        let current = self.snapshot.load();
        let doc_name = name.unwrap_or_else(|| current.document.name.clone());
        let normalized = ensure_gpa_headers(xml);
        let summary = GpaDocumentSummary::parse(doc_name, normalized.clone());
        self.commit_document(
            summary.clone(),
            normalized,
            String::from("MCP updated GPA XML"),
            None,
        );
        summary
    }

    /// Helper that creates a blank GPA icon.
    #[must_use]
    pub fn create_blank(&self, name: &str) -> GpaDocumentSummary {
        let xml = default_blank_gpa(name);
        let file_name = if std::path::Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gpa"))
        {
            name.to_string()
        } else {
            format!("{name}.gpa")
        };
        let summary = GpaDocumentSummary::parse(file_name, xml.clone());
        self.commit_document(
            summary.clone(),
            xml,
            String::from("MCP created new blank GPA icon"),
            Some(0),
        );
        summary
    }

    /// Searches the bundled icon library.
    #[must_use]
    pub fn search_bundled(&self, query: &str, limit: usize) -> Vec<&'static str> {
        search_icons(query, None, limit)
            .into_iter()
            .map(|e| e.file_name)
            .collect()
    }

    /// Atomically updates `self.snapshot` (so sequential MCP tool calls never read stale state)
    /// and dispatches `McpUiCommand::LoadDocument` to the GTK main thread.
    pub fn commit_document(
        &self,
        summary: GpaDocumentSummary,
        xml: String,
        source_note: String,
        override_state: Option<u32>,
    ) {
        let cur = self.snapshot.load_full();
        let next_state = override_state.unwrap_or_else(|| {
            if summary.states.iter().any(|s| s.index == cur.active_state) {
                cur.active_state
            } else {
                summary.initial_state
            }
        });
        let doc_name = summary.name.clone();
        self.snapshot.store(Arc::new(StudioSnapshot {
            document: summary,
            active_state: next_state,
            playing: cur.playing,
            weight: cur.weight,
            show_grid: cur.show_grid,
            show_bounds: cur.show_bounds,
        }));
        let _ = self.ui_tx.try_send(McpUiCommand::LoadDocument {
            name: doc_name,
            xml,
            source_note,
        });
        if let Some(st) = override_state {
            let _ = self.ui_tx.try_send(McpUiCommand::SetViewerState(st));
        }
    }
}

// =========================================================================
// MCP TOOL REQUEST SCHEMAS
// =========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PatchShapeRequest {
    /// ID of the shape to modify (e.g. `"border"`, `"split"`, `"path0"`).
    pub shape_id: String,
    /// Optional new ID to rename the shape to.
    pub new_id: Option<String>,
    /// Optional SVG path `d` command string (for `<path>`).
    pub d: Option<String>,
    /// Optional circle `cx` coordinate.
    pub cx: Option<String>,
    /// Optional circle `cy` coordinate.
    pub cy: Option<String>,
    /// Optional circle `r` radius.
    pub r: Option<String>,
    /// Optional rect `x` coordinate.
    pub x: Option<String>,
    /// Optional rect `y` coordinate.
    pub y: Option<String>,
    /// Optional rect `width`.
    pub width: Option<String>,
    /// Optional rect `height`.
    pub height: Option<String>,
    /// `gpa:states` token list (space-separated state names or indices, e.g. `"outline"`, `"0 1"`, `"all"`, `"none"`).
    pub states: Option<String>,
    /// `gpa:transition-type`: `"animate"` (trim-path draw), `"morph"`, `"fade"`, or `"none"`.
    pub transition_type: Option<String>,
    /// `gpa:transition-duration` (e.g. `"400ms"`).
    pub transition_duration: Option<String>,
    /// `gpa:transition-delay` (e.g. `"120ms"` or `"0ms"`).
    pub transition_delay: Option<String>,
    /// `gpa:transition-easing` (`"linear"`, `"ease-in"`, `"ease-out"`, `"ease-in-out"`, or `"cubic-bezier(...)"`).
    pub transition_easing: Option<String>,
    /// `gpa:origin` (`"0"` to `"1"`, e.g. `"0.5"` to draw/trim from center).
    pub origin: Option<String>,
    /// Continuous loop `gpa:animation-direction`: `"none"`, `"normal"`, `"alternate"`, `"reverse"`, `"reverse-alternate"`, `"in-out"`, `"in-out-alternate"`, `"in-out-reverse"`, `"segment"`, `"segment-alternate"`.
    pub animation_direction: Option<String>,
    /// `gpa:animation-duration` (e.g. `"800ms"`).
    pub animation_duration: Option<String>,
    /// `gpa:animation-repeat` (e.g. `"indefinite"` or `"2"`).
    pub animation_repeat: Option<String>,
    /// `gpa:animation-segment` fraction (`"0.1"`..`"1.0"`, e.g. `"0.35"` for traveling dash segments).
    pub animation_segment: Option<String>,
    /// `gpa:animation-easing` (`"linear"`, `"ease-in-out"`, etc.).
    pub animation_easing: Option<String>,
    /// Symbolic stroke color (`"foreground"`, `"accent"`, `"success"`, `"warning"`, `"error"`, `"none"`).
    pub stroke: Option<String>,
    /// Symbolic fill color (`"none"`, `"foreground"`, `"accent"`, `"success"`, `"warning"`, `"error"`).
    pub fill: Option<String>,
    /// `stroke-width` in px (e.g. `"2"`, `"1.5"`).
    pub stroke_width: Option<String>,
    /// `stroke-opacity` (`"0.0"`..`"1.0"`).
    pub stroke_opacity: Option<String>,
    /// `fill-opacity` (`"0.0"`..`"1.0"`).
    pub fill_opacity: Option<String>,
    /// `gpa:attach-to` target shape ID for motion-along-path attachment.
    pub attach_to: Option<String>,
    /// `gpa:attach-pos` normalized position (`"0.0"`..`"1.0"`) along the attached path.
    pub attach_pos: Option<String>,
}

impl PatchShapeRequest {
    fn into_core_patch(self) -> GpaShapePatch {
        GpaShapePatch {
            new_id: self.new_id,
            d: self.d,
            cx: self.cx,
            cy: self.cy,
            r: self.r,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            states: self.states,
            stroke: self.stroke,
            fill: self.fill,
            stroke_width: self.stroke_width,
            stroke_linecap: None,
            stroke_linejoin: None,
            stroke_opacity: self.stroke_opacity,
            fill_opacity: self.fill_opacity,
            origin: self.origin,
            transition_type: self.transition_type,
            transition_duration: self.transition_duration,
            transition_delay: self.transition_delay,
            transition_easing: self.transition_easing,
            animation_type: None,
            animation_direction: self.animation_direction,
            animation_duration: self.animation_duration,
            animation_repeat: self.animation_repeat,
            animation_segment: self.animation_segment,
            animation_easing: self.animation_easing,
            attach_to: self.attach_to,
            attach_pos: self.attach_pos,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpsertShapeRequest {
    /// ID for the new or updated shape (e.g. `"border-glow"` or `"guide0"`).
    pub shape_id: String,
    /// Element kind: `"path"` (default), `"circle"`, or `"rect"`.
    pub kind: Option<String>,
    /// Optional ID of an existing shape to clone before applying the attribute patch.
    pub duplicate_from_id: Option<String>,
    /// Optional SVG path `d` command string.
    pub d: Option<String>,
    /// Optional circle `cx`.
    pub cx: Option<String>,
    /// Optional circle `cy`.
    pub cy: Option<String>,
    /// Optional circle `r`.
    pub r: Option<String>,
    /// Optional rect `x`.
    pub x: Option<String>,
    /// Optional rect `y`.
    pub y: Option<String>,
    /// Optional rect `width`.
    pub width: Option<String>,
    /// Optional rect `height`.
    pub height: Option<String>,
    /// `gpa:states` token list (e.g. `"outline"`, `"active"`, `"all"`, `"none"`).
    pub states: Option<String>,
    /// `gpa:transition-type`: `"animate"`, `"morph"`, `"fade"`, or `"none"`.
    pub transition_type: Option<String>,
    /// `gpa:transition-duration` (e.g. `"400ms"`).
    pub transition_duration: Option<String>,
    /// `gpa:transition-delay` (e.g. `"100ms"`).
    pub transition_delay: Option<String>,
    /// `gpa:transition-easing` (`"linear"`, `"ease-in"`, `"ease-out"`, `"ease-in-out"`).
    pub transition_easing: Option<String>,
    /// `gpa:origin` (`"0"`..`"1"`, e.g. `"0.5"`).
    pub origin: Option<String>,
    /// Continuous loop `gpa:animation-direction`: `"none"`, `"normal"`, `"alternate"`, `"in-out"`, `"in-out-alternate"`, `"segment"`, `"segment-alternate"`, etc.
    pub animation_direction: Option<String>,
    /// `gpa:animation-duration` (e.g. `"800ms"`).
    pub animation_duration: Option<String>,
    /// `gpa:animation-repeat` (e.g. `"indefinite"`).
    pub animation_repeat: Option<String>,
    /// `gpa:animation-segment` (`"0.1"`..`"1.0"`).
    pub animation_segment: Option<String>,
    /// `gpa:animation-easing`.
    pub animation_easing: Option<String>,
    /// Symbolic stroke (`"foreground"`, `"accent"`, `"success"`, `"warning"`, `"error"`, `"none"`).
    pub stroke: Option<String>,
    /// Symbolic fill (`"none"`, `"foreground"`, `"accent"`, `"success"`, `"warning"`, `"error"`).
    pub fill: Option<String>,
    /// `stroke-width` in px.
    pub stroke_width: Option<String>,
    /// `stroke-opacity`.
    pub stroke_opacity: Option<String>,
    /// `gpa:attach-to` target shape ID.
    pub attach_to: Option<String>,
    /// `gpa:attach-pos` (`"0.0"`..`"1.0"`).
    pub attach_pos: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteShapeRequest {
    /// ID of the shape to remove from the active `.gpa` icon.
    pub shape_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ManageStatesRequest {
    /// Action to perform: `"add"`, `"rename"`, `"remove"`, `"set_all"` (sets all state names from space-separated `state_name`), or `"switch"` (triggers a live state transition on the canvas).
    pub action: String,
    /// Target state index (`0..63`) for `"rename"`, `"remove"`, or `"switch"`.
    pub state_index: Option<u32>,
    /// State name (e.g. `"outline"`, `"spinning"`, or space-separated `"idle hover active"` for `"set_all"`).
    pub state_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpdateGpaXmlRequest {
    /// Complete `.gpa` / `.svg` XML source to load into the GPA Icons Animator editor.
    pub xml: String,
    /// Optional document name override (defaults to the current icon's name).
    pub name: Option<String>,
    /// Optional state index (`0..63`) to immediately transition the preview canvas to after applying the XML.
    pub preview_state: Option<u32>,
    /// Whether to automatically start playing the animation on the canvas after loading (defaults to `true`).
    pub auto_play: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchOrLoadIconRequest {
    /// Optional fuzzy search query across the 1,100+ bundled GNOME & GPA icons.
    pub query: Option<String>,
    /// Optional icon name (e.g. `"3d-box.svg"`, `"example1.gpa"`) to load or inspect.
    pub load_name: Option<String>,
    /// If `true`, returns the bundled icon's parsed shapes and XML as reference WITHOUT overwriting the user's active canvas.
    pub inspect_only: Option<bool>,
    /// Maximum search results to return (default 20).
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ViewerControlsRequest {
    /// Play (`true`) or pause (`false`) continuous & state-cycling GPA animations.
    pub playing: Option<bool>,
    /// Variable stroke weight (`100.0`..`1000.0`, default `400.0`).
    pub weight: Option<f64>,
    /// Preview surface theme: `true` for dark studio surface, `false` for light surface.
    pub dark_surface: Option<bool>,
    /// Show (`true`) or hide (`false`) the 16×16 coordinate grid overlay.
    pub show_grid: Option<bool>,
    /// Show (`true`) or hide (`false`) the red X-Ray shape centerline spine overlay.
    pub show_xray: Option<bool>,
}

// =========================================================================
// RMCP SERVER HANDLER IMPLEMENTATION
// =========================================================================

#[derive(Clone)]
pub struct GpaMcpServer {
    pub bridge: McpBridge,
}

#[allow(
    clippy::missing_errors_doc,
    clippy::option_if_let_else,
    clippy::too_many_lines
)]
#[tool_router]
impl GpaMcpServer {
    #[must_use]
    pub const fn new(bridge: McpBridge) -> Self {
        Self { bridge }
    }

    #[tool(
        name = "get_active_icon",
        description = "Inspect the currently selected icon in the GPA Icons Animator studio (e.g. '3d-box.svg'). Returns its name, dimensions, states, all graphical shapes with full geometry ('d', 'cx'/'cy'/'r', 'x'/'y'/'width'/'height') and 'gpa:*' attributes, viewer state, and raw '.gpa' XML."
    )]
    pub fn get_active_icon(&self) -> Result<CallToolResult, McpError> {
        let snap = self.bridge.snapshot.load_full();
        Ok(CallToolResult::structured(
            serde_json::to_value(&*snap).unwrap_or_else(|_| json!({})),
        ))
    }

    #[tool(
        name = "get_gpa_specs",
        description = "Read the complete GTK 4.24 Path Animation (.gpa / Grappa) authoring specification (GPA_SPECS.md). Call this to verify exact attributes for state transitions (gpa:transition-type='animate|morph|fade', gpa:origin, gpa:transition-delay), continuous loops (gpa:animation-type='automatic', gpa:animation-direction, gpa:animation-segment), and path attachment (gpa:attach-to)."
    )]
    pub fn get_gpa_specs(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            GPA_SPECS_MARKDOWN,
        )]))
    }

    #[tool(
        name = "patch_shape_gpa",
        description = "Surgically update GPA animation, transition, state visibility, stroke/fill, or geometry attributes on a single shape by ID (e.g. 'border' or 'split' in '3d-box.svg') and immediately update the live GTK4 studio canvas."
    )]
    pub fn patch_shape_gpa(
        &self,
        Parameters(req): Parameters<PatchShapeRequest>,
    ) -> Result<CallToolResult, McpError> {
        let shape_id = req.shape_id.clone();
        let patch = req.into_core_patch();
        let cur = self.bridge.snapshot.load_full();

        match patch_shape_in_gpa(&cur.document.xml, &shape_id, &patch) {
            Ok(new_xml) => {
                let summary = GpaDocumentSummary::parse(cur.document.name.clone(), new_xml.clone());
                self.bridge.commit_document(
                    summary.clone(),
                    new_xml,
                    format!("MCP animated shape '{shape_id}' in {}", cur.document.name),
                    None,
                );
                Ok(CallToolResult::structured(json!({
                    "status": "ok",
                    "updated_shape": shape_id,
                    "document": summary,
                })))
            }
            Err(err) => Ok(CallToolResult::structured_error(json!({
                "error": err,
                "available_shapes": cur.document.shapes.iter().map(|s| &s.id).collect::<Vec<_>>(),
            }))),
        }
    }

    #[tool(
        name = "upsert_shape_gpa",
        description = "Insert a new <path>, <circle>, or <rect> into the active .gpa icon (or update it if shape_id exists). Set duplicate_from_id to clone an existing shape (e.g. 'border') into a new shape for state-specific morphs or layered animations."
    )]
    pub fn upsert_shape_gpa(
        &self,
        Parameters(req): Parameters<UpsertShapeRequest>,
    ) -> Result<CallToolResult, McpError> {
        let shape_id = req.shape_id.clone();
        let kind = req.kind.clone().unwrap_or_else(|| String::from("path"));
        let dup_from = req.duplicate_from_id.clone();
        let patch = PatchShapeRequest {
            shape_id: shape_id.clone(),
            new_id: None,
            d: req.d,
            cx: req.cx,
            cy: req.cy,
            r: req.r,
            x: req.x,
            y: req.y,
            width: req.width,
            height: req.height,
            states: req.states,
            transition_type: req.transition_type,
            transition_duration: req.transition_duration,
            transition_delay: req.transition_delay,
            transition_easing: req.transition_easing,
            origin: req.origin,
            animation_direction: req.animation_direction,
            animation_duration: req.animation_duration,
            animation_repeat: req.animation_repeat,
            animation_segment: req.animation_segment,
            animation_easing: req.animation_easing,
            stroke: req.stroke,
            fill: req.fill,
            stroke_width: req.stroke_width,
            stroke_opacity: req.stroke_opacity,
            fill_opacity: None,
            attach_to: req.attach_to,
            attach_pos: req.attach_pos,
        }
        .into_core_patch();

        let cur = self.bridge.snapshot.load_full();
        let result = if let Some(src_id) = dup_from {
            duplicate_shape_in_gpa(&cur.document.xml, &src_id, &shape_id, &patch)
        } else {
            upsert_shape_in_gpa(&cur.document.xml, &shape_id, &kind, &patch)
        };

        match result {
            Ok(new_xml) => {
                let summary = GpaDocumentSummary::parse(cur.document.name.clone(), new_xml.clone());
                self.bridge.commit_document(
                    summary.clone(),
                    new_xml,
                    format!("MCP upserted shape '{shape_id}' in {}", cur.document.name),
                    None,
                );
                Ok(CallToolResult::structured(json!({
                    "status": "ok",
                    "shape_id": shape_id,
                    "document": summary,
                })))
            }
            Err(err) => Ok(CallToolResult::structured_error(json!({
                "error": err,
            }))),
        }
    }

    #[tool(
        name = "delete_shape_gpa",
        description = "Remove a graphical shape by shape_id from the active .gpa icon."
    )]
    pub fn delete_shape_gpa(
        &self,
        Parameters(req): Parameters<DeleteShapeRequest>,
    ) -> Result<CallToolResult, McpError> {
        let cur = self.bridge.snapshot.load_full();
        match delete_shape_from_gpa(&cur.document.xml, &req.shape_id) {
            Ok(new_xml) => {
                let summary = GpaDocumentSummary::parse(cur.document.name.clone(), new_xml.clone());
                self.bridge.commit_document(
                    summary.clone(),
                    new_xml,
                    format!("MCP deleted shape '{}' from {}", req.shape_id, cur.document.name),
                    None,
                );
                Ok(CallToolResult::structured(json!({
                    "status": "ok",
                    "deleted_shape": req.shape_id,
                    "document": summary,
                })))
            }
            Err(err) => Ok(CallToolResult::structured_error(json!({
                "error": err,
            }))),
        }
    }

    #[tool(
        name = "manage_states",
        description = "Add, rename, remove, or transition to a state in the active .gpa icon. Use action='switch' (with state_index or state_name) to trigger a live state transition on the GTK4 studio canvas."
    )]
    pub fn manage_states(
        &self,
        Parameters(req): Parameters<ManageStatesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let cur = self.bridge.snapshot.load_full();
        let action = req.action.trim().to_ascii_lowercase();

        match action.as_str() {
            "add" => {
                let new_xml = add_state_to_gpa(&cur.document, req.state_name.as_deref());
                let summary = GpaDocumentSummary::parse(cur.document.name.clone(), new_xml.clone());
                let new_idx = summary.states.last().map_or(0, |s| s.index);
                self.bridge.commit_document(
                    summary.clone(),
                    new_xml,
                    format!("MCP added state #{new_idx} to {}", cur.document.name),
                    Some(new_idx),
                );
                Ok(CallToolResult::structured(json!({
                    "status": "ok",
                    "action": "add",
                    "active_state": new_idx,
                    "states": summary.states,
                })))
            }
            "rename" => {
                let Some(idx) = req.state_index else {
                    return Ok(CallToolResult::structured_error(json!({
                        "error": "state_index is required for action='rename'",
                    })));
                };
                let Some(name) = req.state_name.as_deref() else {
                    return Ok(CallToolResult::structured_error(json!({
                        "error": "state_name is required for action='rename'",
                    })));
                };
                let new_xml = rename_state_in_gpa(&cur.document, idx, name);
                let summary = GpaDocumentSummary::parse(cur.document.name.clone(), new_xml.clone());
                self.bridge.commit_document(
                    summary.clone(),
                    new_xml,
                    format!("MCP renamed state #{idx} to '{name}'"),
                    None,
                );
                Ok(CallToolResult::structured(json!({
                    "status": "ok",
                    "action": "rename",
                    "states": summary.states,
                })))
            }
            "remove" => {
                let Some(idx) = req.state_index else {
                    return Ok(CallToolResult::structured_error(json!({
                        "error": "state_index is required for action='remove'",
                    })));
                };
                let new_xml = remove_state_from_gpa(&cur.document, idx);
                let summary = GpaDocumentSummary::parse(cur.document.name.clone(), new_xml.clone());
                self.bridge.commit_document(
                    summary.clone(),
                    new_xml,
                    format!("MCP removed state #{idx}"),
                    Some(0),
                );
                Ok(CallToolResult::structured(json!({
                    "status": "ok",
                    "action": "remove",
                    "states": summary.states,
                })))
            }
            "set_all" => {
                let Some(names) = req.state_name.as_deref() else {
                    return Ok(CallToolResult::structured_error(json!({
                        "error": "state_name (space-separated state names, e.g. 'idle hover active') is required for action='set_all'",
                    })));
                };
                let new_xml = set_all_states_in_gpa(&cur.document, names);
                let summary = GpaDocumentSummary::parse(cur.document.name.clone(), new_xml.clone());
                self.bridge.commit_document(
                    summary.clone(),
                    new_xml,
                    format!("MCP set states to '{names}'"),
                    Some(0),
                );
                Ok(CallToolResult::structured(json!({
                    "status": "ok",
                    "action": "set_all",
                    "states": summary.states,
                })))
            }
            "switch" => {
                let resolved_idx = req.state_index.or_else(|| {
                    let target_name = req.state_name.as_deref()?;
                    cur.document
                        .states
                        .iter()
                        .find(|s| s.name == target_name)
                        .map(|s| s.index)
                });
                let Some(idx) = resolved_idx else {
                    return Ok(CallToolResult::structured_error(json!({
                        "error": "Provide a valid state_index or state_name to switch to",
                        "available_states": cur.document.states,
                    })));
                };
                self.bridge.update_active_state_in_snapshot(idx);
                let _ = self.bridge.ui_tx.try_send(McpUiCommand::SetViewerState(idx));
                let _ = self.bridge.ui_tx.try_send(McpUiCommand::LogEvent(format!(
                    "MCP transitioned '{}' to state #{idx}",
                    cur.document.name
                )));
                Ok(CallToolResult::structured(json!({
                    "status": "ok",
                    "action": "switch",
                    "active_state": idx,
                })))
            }
            other => Ok(CallToolResult::structured_error(json!({
                "error": format!("Unknown manage_states action '{other}' (expected 'add', 'rename', 'remove', or 'switch')"),
            }))),
        }
    }

    #[tool(
        name = "update_gpa_xml",
        description = "Replace the complete .gpa / .svg XML of the active icon in the GPA Icons Animator studio. Validates XML, ensures xmlns:gpa and variable stroke-weight headers, and immediately hot-reloads the live GTK4 preview, state boxes, and XML editor."
    )]
    pub fn update_gpa_xml(
        &self,
        Parameters(req): Parameters<UpdateGpaXmlRequest>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(err) = validate_gpa_xml(&req.xml) {
            return Ok(CallToolResult::structured_error(json!({
                "error": err,
            })));
        }
        let cur = self.bridge.snapshot.load_full();
        let doc_name = req.name.unwrap_or_else(|| cur.document.name.clone());
        let normalized = ensure_gpa_headers(&req.xml);
        let summary = GpaDocumentSummary::parse(doc_name.clone(), normalized.clone());
        self.bridge.commit_document(
            summary.clone(),
            normalized,
            format!("MCP updated GPA animation for '{doc_name}'"),
            req.preview_state,
        );
        if req.auto_play.unwrap_or(true) {
            let _ = self.bridge.ui_tx.try_send(McpUiCommand::SetViewerPlaying(true));
        }
        Ok(CallToolResult::structured(json!({
            "status": "ok",
            "document": summary,
        })))
    }

    #[tool(
        name = "search_and_load_icon",
        description = "Search the 1,100+ bundled GNOME Icon Development Kit icons and GPA examples by query, or load/inspect a bundled icon by name (e.g. '3d-box.svg', 'example2.gpa'). Set inspect_only=true to read a bundled icon's XML and shapes as reference without overwriting the active studio canvas."
    )]
    pub fn search_and_load_icon(
        &self,
        Parameters(req): Parameters<SearchOrLoadIconRequest>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(load_name) = req.load_name.as_deref() {
            if req.inspect_only.unwrap_or(false) {
                let Some(entry) = find_icon(load_name) else {
                    return Ok(CallToolResult::structured_error(json!({
                        "error": format!("Icon '{load_name}' not found in bundled library"),
                    })));
                };
                let xml = ensure_gpa_headers(entry.xml);
                let summary = GpaDocumentSummary::parse(entry.file_name, xml);
                return Ok(CallToolResult::structured(json!({
                    "status": "inspected",
                    "document": summary,
                })));
            }

            match self.bridge.load_library_icon(load_name) {
                Ok(summary) => {
                    return Ok(CallToolResult::structured(json!({
                        "status": "loaded",
                        "document": summary,
                    })));
                }
                Err(err) => {
                    return Ok(CallToolResult::structured_error(json!({
                        "error": err,
                    })));
                }
            }
        }

        let query = req.query.as_deref().unwrap_or("");
        let limit = req.limit.unwrap_or(20).clamp(1, 100);
        let matches = self.bridge.search_bundled(query, limit);
        Ok(CallToolResult::structured(json!({
            "query": query,
            "count": matches.len(),
            "icons": matches,
        })))
    }

    #[tool(
        name = "set_viewer_controls",
        description = "Control the live GTK4 studio viewer: toggle play/pause (playing), adjust variable stroke weight (weight: 100..1000), or toggle dark/light surface."
    )]
    pub fn set_viewer_controls(
        &self,
        Parameters(req): Parameters<ViewerControlsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let cur = self.bridge.snapshot.load_full();
        let mut next = (*cur).clone();

        if let Some(p) = req.playing {
            next.playing = p;
            let _ = self.bridge.ui_tx.try_send(McpUiCommand::SetViewerPlaying(p));
        }
        if let Some(w) = req.weight {
            let clamped = w.clamp(100.0, 1000.0);
            next.weight = clamped;
            let _ = self
                .bridge
                .ui_tx
                .try_send(McpUiCommand::SetViewerWeight(clamped));
        }
        if let Some(dark) = req.dark_surface {
            let _ = self
                .bridge
                .ui_tx
                .try_send(McpUiCommand::SetViewerDark(dark));
        }
        if let Some(grid) = req.show_grid {
            next.show_grid = grid;
            let _ = self
                .bridge
                .ui_tx
                .try_send(McpUiCommand::SetViewerGrid(grid));
        }
        if let Some(xray) = req.show_xray {
            next.show_bounds = xray;
            let _ = self
                .bridge
                .ui_tx
                .try_send(McpUiCommand::SetViewerXray(xray));
        }

        self.bridge.update_snapshot(next.clone());
        Ok(CallToolResult::structured(json!({
            "status": "ok",
            "playing": next.playing,
            "weight": next.weight,
            "active_state": next.active_state,
        })))
    }
}

#[allow(clippy::unused_async_trait_impl)]
#[tool_handler]
impl ServerHandler for GpaMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_instructions(
            "GPA Icons Animator MCP Server. Call `get_active_icon` to inspect the icon currently selected by the user (e.g. '3d-box.svg'), `get_gpa_specs` (or read resource `gpa://specs`) for GTK 4.24 Path Animation (.gpa / Grappa) rules, and `patch_shape_gpa` / `upsert_shape_gpa` / `manage_states` / `update_gpa_xml` to animate the icon live in the GTK4 editor.",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let items = vec![
            Resource::new("gpa://specs", "GPA_SPECS.md")
                .with_description(
                    "Complete GTK 4.24 Path Animation (.gpa / Grappa) authoring specification",
                )
                .with_mime_type("text/markdown"),
            Resource::new("gpa://document/current", "Current Studio Icon Snapshot")
                .with_description(
                    "Live JSON summary of the currently selected icon, states, shapes, and XML",
                )
                .with_mime_type("application/json"),
            Resource::new("gpa://document/current.gpa", "Current Icon .gpa XML")
                .with_description("Raw .gpa XML of the currently selected icon in the studio")
                .with_mime_type("image/x-gtk-path-animation"),
        ];
        Ok(ListResourcesResult::with_all_items(items))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        match request.uri.as_str() {
            "gpa://specs" => Ok(ReadResourceResult::new(vec![ResourceContents::text(
                GPA_SPECS_MARKDOWN,
                "gpa://specs",
            )])
            .into()),
            "gpa://document/current" => {
                let snap = self.bridge.snapshot.load_full();
                let text = serde_json::to_string_pretty(&*snap).unwrap_or_default();
                Ok(ReadResourceResult::new(vec![ResourceContents::text(
                    text,
                    "gpa://document/current",
                )])
                .into())
            }
            "gpa://document/current.gpa" => {
                let snap = self.bridge.snapshot.load_full();
                Ok(ReadResourceResult::new(vec![ResourceContents::text(
                    snap.document.xml.clone(),
                    "gpa://document/current.gpa",
                )])
                .into())
            }
            other => Err(McpError::resource_not_found(
                format!("Unknown GPA resource URI: {other}"),
                None,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    #[test]
    fn mcp_tools_animate_3d_box_end_to_end() {
        let entry = find_icon("3d-box.svg").expect("bundled 3d-box.svg exists");
        let initial_doc =
            GpaDocumentSummary::parse(entry.file_name, ensure_gpa_headers(entry.xml));
        let (ui_tx, ui_rx) = async_channel::unbounded();
        let bridge = McpBridge::new(initial_doc, ui_tx, 0);
        let server = GpaMcpServer::new(bridge.clone());

        // 1. Agent inspects active icon (3d-box.svg)
        let snap = bridge.snapshot.load();
        assert_eq!(snap.document.name, "3d-box.svg");
        assert_eq!(snap.document.shapes.len(), 2);
        assert_eq!(snap.document.shapes[0].id, "border");
        assert_eq!(snap.document.shapes[1].id, "split");

        // 2. Agent patches 'border' and 'split' with staggered trim-path draw transitions & loop animation
        let _ = server
            .patch_shape_gpa(Parameters(PatchShapeRequest {
                shape_id: String::from("border"),
                transition_type: Some(String::from("animate")),
                transition_duration: Some(String::from("450ms")),
                transition_easing: Some(String::from("ease-in-out")),
                origin: Some(String::from("0.5")),
                ..Default::default()
            }))
            .expect("patch border");

        let _ = server
            .patch_shape_gpa(Parameters(PatchShapeRequest {
                shape_id: String::from("split"),
                transition_type: Some(String::from("animate")),
                transition_duration: Some(String::from("400ms")),
                transition_delay: Some(String::from("150ms")),
                transition_easing: Some(String::from("ease-out")),
                origin: Some(String::from("0.5")),
                animation_direction: Some(String::from("segment-alternate")),
                animation_duration: Some(String::from("900ms")),
                animation_segment: Some(String::from("0.4")),
                ..Default::default()
            }))
            .expect("patch split");

        // 3. Agent switches state to trigger the transition on the canvas
        let _ = server
            .manage_states(Parameters(ManageStatesRequest {
                action: String::from("switch"),
                state_index: Some(1),
                state_name: None,
            }))
            .expect("switch to state 1");

        let after = bridge.snapshot.load();
        assert_eq!(after.active_state, 1);
        assert_eq!(after.document.shapes[0].transition_type, "animate");
        assert_eq!(after.document.shapes[0].origin.as_deref(), Some("0.5"));
        assert_eq!(after.document.shapes[1].transition_delay, "150ms");
        assert_eq!(
            after.document.shapes[1].animation_direction,
            "segment-alternate"
        );

        // Verify UI commands were queued for GTK main loop
        assert!(!ui_rx.is_empty());
    }

    #[tokio::test]
    async fn http_mcp_server_serves_status_and_jsonrpc() {
        let entry = find_icon("3d-box.svg").expect("bundled 3d-box.svg exists");
        let initial_doc =
            GpaDocumentSummary::parse(entry.file_name, ensure_gpa_headers(entry.xml));
        let (ui_tx, _ui_rx) = async_channel::unbounded();
        let bridge = McpBridge::new(initial_doc, ui_tx, 0);

        let addr = bind_and_spawn_http_server(bridge, 0)
            .await
            .expect("bind ephemeral port");

        let mut stream = TcpStream::connect(addr).await.expect("connect");
        stream
            .write_all(b"GET /status HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .expect("write");
        let mut resp = String::new();
        stream.read_to_string(&mut resp).await.expect("read");
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("3d-box.svg"));

        // Send MCP initialize POST to /mcp
        let init_body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test-agent","version":"1.0"}}}"#;
        let mut mcp_stream = TcpStream::connect(addr).await.expect("connect mcp");
        let req_str = format!(
            "POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            init_body.len(),
            init_body
        );
        mcp_stream
            .write_all(req_str.as_bytes())
            .await
            .expect("write mcp init");
        let mut init_resp = String::new();
        mcp_stream
            .read_to_string(&mut init_resp)
            .await
            .expect("read mcp init");
        assert!(init_resp.contains("200 OK"));
        assert!(init_resp.contains("GPA Icons Animator MCP Server"));
    }
}
