//! Map viewer/editor application state and per-frame orchestration.
//!
//! Feature areas live in submodules that each extend [`MapViewerApp`]:
//! `canvas` (map rendering/input), `panels` (menus and inspector), `palette`,
//! `editing` (tile/item mutations), `undo`, `live_api`, `flags`, and `geometry`.

mod canvas;
mod editing;
mod flags;
mod geometry;
mod live_api;
mod palette;
mod panels;
mod undo;

use crate::map_viewer_app::graphics::GraphicsZipCache;
use eframe::egui;
use egui::{Rect, Vec2};
use mag_core::types::{Item, Map};
use palette::{PaletteEntry, SpriteLayer};
use server::keydb::snapshot::WorldSnapshot;
use server_utils::admin_client::AdminClient;
use server_utils::{DataSource, load_world_snapshot, save_world_snapshot};
use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;
use undo::UndoAction;

/// Keyboard pan speed in screen pixels per second.
const KEYBOARD_PAN_SPEED: f32 = 750.0;

/// How often the UI wakes to check on a background world load.
const WORLD_LOAD_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Why a background world load was started; decides how its result is reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadPurpose {
    /// Initial load, opening a snapshot, or "Reload snapshot".
    Open,
    /// "Revert (discard changes)".
    Revert,
    /// Connection test from the connect dialog.
    Connect,
}

/// A world load running on a background thread.
struct PendingWorldLoad {
    purpose: LoadPurpose,
    result: Receiver<Result<WorldSnapshot, String>>,
}

/// Item placement/removal queued locally for the next LiveApi save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingItemAction {
    Place {
        x: usize,
        y: usize,
        template_id: u16,
    },
    Clear {
        x: usize,
        y: usize,
    },
}

/// Map viewer/editor application state.
#[derive(Default)]
pub(crate) struct MapViewerApp {
    loaded_world: Option<WorldSnapshot>,
    /// World load in progress on a background thread, if any.
    pending_world_load: Option<PendingWorldLoad>,
    map_tiles: Vec<Map>,
    map_error: Option<String>,

    dirty: bool,
    save_status: Option<String>,

    items: Vec<Item>,
    items_error: Option<String>,
    item_templates: Vec<Item>,
    item_templates_error: Option<String>,
    fully_loaded_item_template_slots: BTreeSet<usize>,

    graphics_zip: Option<GraphicsZipCache>,
    graphics_zip_error: Option<String>,

    /// Camera pan in screen pixels.
    pan: Vec2,
    /// Camera zoom applied to map-space pixels.
    zoom: f32,
    /// True once we auto-center after loading map/graphics.
    pan_initialized: bool,

    /// Tile under the cursor, for the inspector.
    hovered_tile: Option<(usize, usize)>,
    /// Frozen selection (click on map when no palette entry is selected).
    selected_tile: Option<(usize, usize)>,

    /// Preview the client's Hide Walls mode (object sprites drawn as `sprite + 1`).
    hide_enabled: bool,

    /// Whether the deferred first load has run.
    initial_load_done: bool,

    palette: Vec<PaletteEntry>,
    selected_palette_index: Option<usize>,
    draft_sprite: u16,
    draft_sprite_layer: SpriteLayer,
    draft_item_template_id: u16,
    draft_flag_mask: u64,
    draft_flag_clear: bool,
    palette_rect: Option<Rect>,
    line_anchor: Option<(usize, usize)>,

    /// Active data backend (live KeyDB or snapshot file).
    data_source: DataSource,

    /// Tiles with unsaved edits (LiveApi mode). Keyed by `(x, y)`.
    dirty_tiles: BTreeSet<(usize, usize)>,
    /// Item placement/removal actions queued locally for the next LiveApi save.
    pending_item_actions: Vec<PendingItemAction>,
    /// Cached admin API client for LiveApi mode.
    admin_client: Option<AdminClient>,
    /// Pending map-reload request id awaiting a status update.
    pending_map_reload_request_id: Option<String>,
    /// Wall-clock instant when the most recent reload request was fired.
    pending_reload_since: Option<std::time::Instant>,
    /// Wall-clock instant of the last automatic reload-status poll.
    last_reload_poll: Option<std::time::Instant>,
    /// Whether the "Connect to admin API" modal dialog is open.
    connect_dialog_open: bool,
    /// Working draft of the API base URL inside the connect dialog.
    connect_form_base_url: String,
    /// Working draft of the admin token inside the connect dialog.
    connect_form_token: String,
    /// Whether the admin-token field is currently shown in plaintext.
    connect_form_show_token: bool,
    /// Last error reported by the connect dialog (e.g. failed fetch).
    connect_dialog_error: Option<String>,
    /// Whether the "confirm server map reload" modal dialog is open.
    reload_confirm_open: bool,
    /// Bounded undo history (most recent action at the back).
    undo_stack: VecDeque<UndoAction>,

    /// Settings: tint each tile by its (filtered) map flag combination.
    flag_viz_enabled: bool,
    /// Alpha of the flag tint overlay in `[0, 1]`.
    flag_viz_opacity: f32,
    /// Flag bits that participate in the visualization.
    flag_viz_mask: u64,
    /// Flag combinations visible in the last rendered frame with tile counts, for the legend.
    flag_viz_legend: Vec<(u64, usize)>,
}

impl MapViewerApp {
    /// Create the app for a data source; loading is deferred to the first frames.
    ///
    /// # Arguments
    ///
    /// * `data_source` - Snapshot file or live admin API to load from.
    ///
    /// # Returns
    ///
    /// * A new, not-yet-loaded app.
    pub(crate) fn new(data_source: DataSource) -> Self {
        let admin_client = match &data_source {
            DataSource::LiveApi { base_url, token } => {
                AdminClient::new(base_url.clone(), token.clone()).ok()
            }
            _ => None,
        };
        Self {
            data_source,
            admin_client,
            zoom: 1.0,
            flag_viz_opacity: flags::DEFAULT_FLAG_VIZ_OPACITY,
            flag_viz_mask: flags::GAMEPLAY_MAP_FLAG_MASK,
            ..Self::default()
        }
    }

    /// Reset selection, line, dirty, and undo state after the world changed underneath us.
    fn reset_edit_state(&mut self) {
        self.hovered_tile = None;
        self.selected_tile = None;
        self.selected_palette_index = None;
        self.line_anchor = None;
        self.dirty = false;
        self.dirty_tiles.clear();
        self.pending_item_actions.clear();
        self.undo_stack.clear();
    }

    /// Drop all loaded world data (after a failed load).
    fn clear_loaded_world(&mut self) {
        self.loaded_world = None;
        self.map_tiles.clear();
        self.items.clear();
        self.item_templates.clear();
        self.fully_loaded_item_template_slots.clear();
        self.reset_edit_state();
    }

    /// Install a freshly loaded world and report `status`.
    fn apply_loaded_world(&mut self, mut world: WorldSnapshot, status: String) {
        // Views own the big vectors; `sync_loaded_world_from_views` puts them back before saving.
        self.map_tiles = std::mem::take(&mut world.map);
        self.items = std::mem::take(&mut world.items);
        self.item_templates = std::mem::take(&mut world.item_templates);
        self.fully_loaded_item_template_slots.clear();
        self.loaded_world = Some(world);
        self.save_status = Some(status);
        self.pan_initialized = false;
        self.reset_edit_state();
    }

    /// Start (re)loading the world from the current data source on a background thread.
    ///
    /// The current world is cleared immediately; [`Self::poll_world_load`]
    /// installs the result. Starting a new load supersedes any pending one.
    ///
    /// # Arguments
    ///
    /// * `purpose` - Decides the status/dialog handling once the load finishes.
    fn load_current_source(&mut self, purpose: LoadPurpose) {
        if matches!(self.data_source, DataSource::NotLoaded) {
            return;
        }

        self.map_error = None;
        self.items_error = None;
        self.item_templates_error = None;
        self.clear_loaded_world();
        self.pan_initialized = false;
        self.save_status = Some(format!("Loading {}...", self.data_source.display_label()));

        let source = self.data_source.clone();
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("map-viewer-world-load".to_owned())
            .spawn(move || {
                let _ = tx.send(load_world_snapshot(&source));
            });
        match spawned {
            Ok(_) => {
                self.pending_world_load = Some(PendingWorldLoad {
                    purpose,
                    result: rx,
                });
            }
            Err(e) => {
                self.pending_world_load = None;
                self.finish_world_load(purpose, Err(format!("Failed to start world loader: {e}")));
            }
        }
    }

    /// Whether a background world load is in progress.
    fn is_loading_world(&self) -> bool {
        self.pending_world_load.is_some()
    }

    /// Install the background world load's result once it arrives.
    fn poll_world_load(&mut self) {
        let Some(pending) = &self.pending_world_load else {
            return;
        };
        let result = match pending.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                Err("World loader thread exited unexpectedly".to_owned())
            }
        };
        let purpose = pending.purpose;
        self.pending_world_load = None;
        self.finish_world_load(purpose, result);
    }

    /// Apply a finished world load, reporting it according to `purpose`.
    fn finish_world_load(&mut self, purpose: LoadPurpose, result: Result<WorldSnapshot, String>) {
        match result {
            Ok(world) => {
                log::info!(
                    "Loaded world for map viewer: map={} items={} templates={} source={}",
                    world.map.len(),
                    world.items.len(),
                    world.item_templates.len(),
                    self.data_source.display_label()
                );
                let status = match (purpose, self.data_source.snapshot_path()) {
                    (LoadPurpose::Revert, _) => "Reverted (discarded unsaved changes)".to_owned(),
                    (LoadPurpose::Connect, _) => "Connected to admin API".to_owned(),
                    (LoadPurpose::Open, Some(path)) => {
                        format!("Loaded snapshot: {}", path.display())
                    }
                    (LoadPurpose::Open, None) => "Loaded world state".to_owned(),
                };
                self.apply_loaded_world(world, status);
                if purpose == LoadPurpose::Connect {
                    self.connect_dialog_open = false;
                    self.connect_dialog_error = None;
                }
            }
            Err(e) => {
                self.clear_loaded_world();
                self.save_status = None;
                if purpose == LoadPurpose::Connect {
                    self.connect_dialog_error = Some(format!("Connection test failed: {e}"));
                    self.admin_client = None;
                }
                self.map_error = Some(e);
            }
        }
    }

    /// Copy edited views back into the loaded world snapshot.
    fn sync_loaded_world_from_views(&mut self) -> Result<(), String> {
        let Some(world) = self.loaded_world.as_mut() else {
            return Err("No world loaded".to_owned());
        };
        world.map = self.map_tiles.clone();
        world.items = self.items.clone();
        world.item_templates = self.item_templates.clone();
        Ok(())
    }

    /// Write the current world to `path` and switch the data source to it.
    fn save_snapshot_as(&mut self, path: &Path) -> Result<(), String> {
        self.sync_loaded_world_from_views()?;
        let world = self
            .loaded_world
            .as_ref()
            .ok_or_else(|| "No world loaded".to_owned())?;
        save_world_snapshot(world, path)?;
        self.data_source = DataSource::SnapshotFile(path.to_path_buf());
        Ok(())
    }

    /// Load the graphics zip used for sprite textures.
    ///
    /// # Arguments
    ///
    /// * `zip_path` - Path to the graphics archive.
    pub(crate) fn load_graphics_zip(&mut self, zip_path: PathBuf) {
        match GraphicsZipCache::load(zip_path) {
            Ok(cache) => {
                self.graphics_zip = Some(cache);
                self.graphics_zip_error = None;
            }
            Err(e) => {
                self.graphics_zip = None;
                self.graphics_zip_error = Some(e);
            }
        }
    }

    /// Switch to a snapshot file and load it.
    ///
    /// # Arguments
    ///
    /// * `path` - `.wsnap` file to load.
    pub(crate) fn load_from_snapshot(&mut self, path: PathBuf) {
        self.data_source = DataSource::SnapshotFile(path);
        self.load_current_source(LoadPurpose::Open);
    }

    /// Pick a `.wsnap` file and load it.
    fn open_snapshot_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("World Snapshot", &["wsnap"])
            .pick_file()
        {
            self.load_from_snapshot(path);
        }
    }

    /// Save to the API in LiveApi mode, otherwise to a snapshot file.
    fn save_current(&mut self) {
        if self.data_source.is_live_api() {
            self.save_to_api();
        } else {
            self.save_snapshot_as_dialog();
        }
    }

    /// Pick a destination and save the world snapshot there.
    fn save_snapshot_as_dialog(&mut self) {
        self.save_status = None;

        let Some(path) = rfd::FileDialog::new()
            .add_filter("World Snapshot", &["wsnap"])
            .set_file_name("world_snapshot.wsnap")
            .save_file()
        else {
            return;
        };

        match self.save_snapshot_as(&path) {
            Ok(()) => {
                self.dirty = false;
                self.pending_item_actions.clear();
                self.save_status = Some(format!("Saved snapshot: {}", path.display()));
            }
            Err(e) => self.save_status = Some(format!("Save failed: {e}")),
        }
    }

    /// Reload from the data source, discarding local edits.
    fn revert_unsaved_changes(&mut self) {
        self.load_current_source(LoadPurpose::Revert);
    }

    /// Mark tile `(x, y)` as having unsaved static-field changes.
    ///
    /// Used by the LiveApi "Save to API" flow to avoid pushing untouched
    /// tiles back over the 1-req/sec admin rate limiter.
    ///
    /// # Arguments
    ///
    /// * `x` - Tile X coordinate.
    /// * `y` - Tile Y coordinate.
    fn mark_tile_dirty(&mut self, x: usize, y: usize) {
        self.dirty = true;
        self.dirty_tiles.insert((x, y));
    }

    /// Queue an item action for the next LiveApi save.
    fn mark_item_action_pending(&mut self, action: PendingItemAction) {
        self.dirty = true;
        self.pending_item_actions.push(action);
    }

    /// Recompute `dirty` from the outstanding tile and item changes.
    fn mark_clean_if_no_pending_changes(&mut self) {
        self.dirty = !self.dirty_tiles.is_empty() || !self.pending_item_actions.is_empty();
    }

    /// Cmd/Ctrl+S saves, Cmd/Ctrl+Z undoes.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (save, undo) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::S),
                i.modifiers.command && i.key_pressed(egui::Key::Z),
            )
        });
        if save && self.loaded_world.is_some() {
            self.save_current();
        }
        if undo {
            self.undo();
            ctx.request_repaint();
        }
    }

    /// WASD camera pan.
    fn handle_keyboard_pan(&mut self, ctx: &egui::Context) {
        let (dt, delta) = ctx.input(|i| {
            let axis = |pos: egui::Key, neg: egui::Key| {
                f32::from(u8::from(i.key_down(pos))) - f32::from(u8::from(i.key_down(neg)))
            };
            (
                i.stable_dt.max(1.0 / 240.0),
                Vec2::new(
                    axis(egui::Key::A, egui::Key::D),
                    axis(egui::Key::W, egui::Key::S),
                ),
            )
        });
        if delta != Vec2::ZERO {
            self.pan += delta.normalized() * KEYBOARD_PAN_SPEED * dt;
            ctx.request_repaint();
        }
    }

    /// Bare app with blank map/item state, for unit tests.
    #[cfg(test)]
    fn for_tests(tile_count: usize, item_count: usize) -> Self {
        Self {
            map_tiles: vec![Map::default(); tile_count],
            items: vec![Item::default(); item_count],
            item_templates: vec![Item::default(); 4],
            ..Self::default()
        }
    }
}

impl eframe::App for MapViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_world_load();
        if self.is_loading_world() {
            ctx.request_repaint_after(WORLD_LOAD_POLL_INTERVAL);
        }

        self.handle_shortcuts(ctx);
        self.tick_map_reload_poll(ctx);
        self.render_connect_dialog(ctx);
        self.render_reload_confirm_dialog(ctx);

        if !self.initial_load_done {
            self.initial_load_done = true;
            self.load_current_source(LoadPurpose::Open);
            if let Some(zip_path) = server_utils::graphics_zip_from_args()
                .or_else(server_utils::default_graphics_zip_path)
            {
                self.load_graphics_zip(zip_path);
            }
        }

        self.handle_keyboard_pan(ctx);
        self.ui_top_bar(ctx);
        self.ui_side_panel(ctx);
        self.ui_map_canvas(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::MapViewerApp;

    #[test]
    fn revert_state_reset_clears_edit_tracking() {
        let mut app = MapViewerApp::for_tests(4, 4);
        app.selected_tile = Some((1, 1));
        app.line_anchor = Some((0, 0));
        assert!(app.edit_tile_with_undo(0, 0, |t| t.sprite = 3));
        assert!(app.dirty);

        app.reset_edit_state();
        assert!(!app.dirty);
        assert!(app.dirty_tiles.is_empty());
        assert!(app.undo_stack.is_empty());
        assert_eq!(app.selected_tile, None);
        assert_eq!(app.line_anchor, None);
    }

    #[test]
    fn mark_clean_tracks_outstanding_changes() {
        let mut app = MapViewerApp::for_tests(4, 4);
        app.mark_tile_dirty(0, 0);
        app.mark_clean_if_no_pending_changes();
        assert!(app.dirty);
        app.dirty_tiles.clear();
        app.mark_clean_if_no_pending_changes();
        assert!(!app.dirty);
    }
}
