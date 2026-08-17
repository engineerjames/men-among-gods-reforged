use super::graphics::GraphicsZipCache;
use eframe::egui;
use egui::{Pos2, Rect, Vec2};
use mag_core::constants::{ItemFlags, SERVER_MAPX, SERVER_MAPY, TILEX, USE_EMPTY, XPOS, YPOS};
use mag_core::map_store::MapPatch;
use mag_core::types::{Item, Map};
use mag_core::world_action_store::WorldActionKind;
use serde::{Deserialize, Serialize};
use server::keydb::snapshot::WorldSnapshot;
use server_utils::admin_client::AdminClient;
use server_utils::{DataSource, load_world_snapshot, save_world_snapshot};
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

/// Which `Map` sprite field a palette sprite entry paints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
enum SpriteLayer {
    /// Background/floor sprite (`Map::sprite`).
    #[default]
    Floor,
    /// Foreground/wall/object sprite (`Map::fsprite`).
    Object,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum PaletteEntryKind {
    Sprite {
        sprite: u16,
        layer: SpriteLayer,
    },
    ItemTemplate(u16),
    /// Set (`clear == false`) or clear (`clear == true`) a mask of map flags.
    Flags {
        mask: u64,
        clear: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PaletteEntry {
    kind: PaletteEntryKind,
}

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

/// Pre-mutation snapshot of one tile, used to support undo.
#[derive(Clone, Copy, Debug)]
struct UndoTileSnapshot {
    x: usize,
    y: usize,
    tile: Map,
    /// The runtime item slot referenced by `tile.it` before the edit, if any.
    item: Option<(usize, Item)>,
}

/// One user-initiated edit (a click, or a whole Shift+click line stroke),
/// captured before mutation so [`MapViewerApp::undo`] can revert it exactly.
struct UndoAction {
    tiles: Vec<UndoTileSnapshot>,
    dirty_before: bool,
    dirty_tiles_before: BTreeSet<(usize, usize)>,
    pending_item_actions_before: Vec<PendingItemAction>,
}

/// Maximum number of edits kept in the undo history.
const MAX_UNDO_HISTORY: usize = 10;

#[derive(Default)]
pub(crate) struct MapViewerApp {
    loaded_world: Option<WorldSnapshot>,
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

    // Camera pan in screen pixels.
    pan: Vec2,

    // Camera zoom applied to map-space pixels.
    zoom: f32,

    // True once we auto-center after loading map/graphics.
    pan_initialized: bool,

    // Cached hover state for the right panel.
    hovered_tile: Option<(usize, usize)>,

    // Frozen selection (click on map when no palette entry is selected).
    selected_tile: Option<(usize, usize)>,

    // Hide mode: clips non-background sprites to show only top half
    hide_enabled: bool,

    // Track if we've done initial load
    initial_load_done: bool,

    // Track frames to delay loading slightly so window appears first
    frame_count: u32,

    // Palette / painting
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
    /// Bounded undo history (most recent action at the back), capped at [`MAX_UNDO_HISTORY`].
    undo_stack: VecDeque<UndoAction>,
}

impl MapViewerApp {
    pub(crate) fn new(data_source: DataSource) -> Self {
        // Don't load map/graphics in constructor — it blocks window creation.
        // We load on first update instead, dispatching on data_source.
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
            ..Self::default()
        }
    }

    fn clear_loaded_world(&mut self) {
        self.loaded_world = None;
        self.map_tiles.clear();
        self.items.clear();
        self.item_templates.clear();
        self.fully_loaded_item_template_slots.clear();
        self.hovered_tile = None;
        self.selected_tile = None;
        self.selected_palette_index = None;
        self.line_anchor = None;
        self.dirty = false;
        self.dirty_tiles.clear();
        self.pending_item_actions.clear();
        self.undo_stack.clear();
    }

    fn apply_loaded_world(&mut self, world: WorldSnapshot, status: String) {
        self.map_tiles = world.map.clone();
        self.items = world.items.clone();
        self.item_templates = world.item_templates.clone();
        self.fully_loaded_item_template_slots.clear();
        self.loaded_world = Some(world);
        self.save_status = Some(status);
        self.pan_initialized = false;
        self.hovered_tile = None;
        self.selected_tile = None;
        self.selected_palette_index = None;
        self.line_anchor = None;
        self.dirty = false;
        self.dirty_tiles.clear();
        self.pending_item_actions.clear();
        self.undo_stack.clear();
    }

    fn load_current_source(&mut self) {
        if matches!(self.data_source, DataSource::NotLoaded) {
            return;
        }

        self.map_error = None;
        self.items_error = None;
        self.item_templates_error = None;
        self.save_status = None;
        self.pan_initialized = false;

        match load_world_snapshot(&self.data_source) {
            Ok(world) => {
                let status = if let Some(path) = self.data_source.snapshot_path() {
                    format!("Loaded snapshot: {}", path.display())
                } else {
                    "Loaded world state".to_owned()
                };
                log::info!(
                    "Loaded world for map viewer: map={} items={} templates={} source={}",
                    world.map.len(),
                    world.items.len(),
                    world.item_templates.len(),
                    self.data_source.display_label()
                );
                self.apply_loaded_world(world, status);
            }
            Err(e) => {
                self.clear_loaded_world();
                self.map_error = Some(e);
            }
        }
    }

    fn sync_loaded_world_from_views(&mut self) -> Result<(), String> {
        let Some(world) = self.loaded_world.as_mut() else {
            return Err("No world loaded".to_owned());
        };

        world.map = self.map_tiles.clone();
        world.items = self.items.clone();
        world.item_templates = self.item_templates.clone();
        Ok(())
    }

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

    fn ui_tile_preview_row(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        sprite: u16,
        fsprite: u16,
        it: u32,
        preview_size: Vec2,
    ) {
        ui.horizontal(|ui| {
            if let Some(cache) = self.graphics_zip.as_mut() {
                let mut try_draw = |ui: &mut egui::Ui, sprite_id: usize| -> bool {
                    if let Ok(Some(texture)) = cache.texture_for(ctx, sprite_id) {
                        ui.add(
                            egui::Image::new(texture)
                                .fit_to_exact_size(preview_size)
                                .maintain_aspect_ratio(true),
                        );
                        true
                    } else {
                        false
                    }
                };

                // Background
                if sprite != 0 {
                    if !try_draw(ui, sprite as usize) {
                        ui.allocate_exact_size(preview_size, egui::Sense::hover());
                    }
                } else {
                    ui.allocate_exact_size(preview_size, egui::Sense::hover());
                }

                // Foreground
                if fsprite != 0 {
                    let sprite_id = if self.hide_enabled {
                        fsprite + 1
                    } else {
                        fsprite
                    };
                    if !try_draw(ui, sprite_id as usize) {
                        ui.allocate_exact_size(preview_size, egui::Sense::hover());
                    }
                } else {
                    ui.allocate_exact_size(preview_size, egui::Sense::hover());
                }

                // Item (instance)
                if it != 0 {
                    let it_idx = it as usize;
                    let item_sprite = if it_idx < self.items.len() {
                        item_map_sprite(self.items[it_idx])
                    } else {
                        None
                    };
                    if let Some(item_sprite) = item_sprite {
                        if !try_draw(ui, item_sprite as usize) {
                            ui.allocate_exact_size(preview_size, egui::Sense::hover());
                        }
                    } else {
                        ui.allocate_exact_size(preview_size, egui::Sense::hover());
                    }
                } else {
                    ui.allocate_exact_size(preview_size, egui::Sense::hover());
                }
            } else {
                ui.allocate_exact_size(preview_size, egui::Sense::hover());
                ui.allocate_exact_size(preview_size, egui::Sense::hover());
                ui.allocate_exact_size(preview_size, egui::Sense::hover());
            }
        });
    }

    pub(crate) fn load_graphics_zip(&mut self, zip_path: PathBuf) {
        self.graphics_zip_error = None;
        match GraphicsZipCache::load(zip_path) {
            Ok(cache) => {
                self.graphics_zip = Some(cache);
            }
            Err(e) => {
                self.graphics_zip = None;
                self.graphics_zip_error = Some(e);
            }
        }
    }

    pub(crate) fn load_from_snapshot(&mut self, path: PathBuf) {
        self.data_source = DataSource::SnapshotFile(path);
        self.load_current_source();
    }

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
            Err(e) => {
                self.save_status = Some(format!("Save failed: {e}"));
            }
        }
    }

    fn revert_unsaved_changes(&mut self) {
        self.load_current_source();
        self.dirty = false;
        self.dirty_tiles.clear();
        self.pending_item_actions.clear();
        self.save_status = Some("Reverted (discarded unsaved changes)".to_owned());
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

    fn mark_item_action_pending(&mut self, action: PendingItemAction) {
        self.dirty = true;
        self.pending_item_actions.push(action);
    }

    fn mark_clean_if_no_pending_changes(&mut self) {
        self.dirty = !self.dirty_tiles.is_empty() || !self.pending_item_actions.is_empty();
    }

    /// Capture "before" state for a set of tiles about to be painted, deduping coordinates.
    fn snapshot_tiles_for_undo(&self, coords: &[(usize, usize)]) -> Vec<UndoTileSnapshot> {
        let mut seen = BTreeSet::new();
        let mut snapshots = Vec::new();
        for &(x, y) in coords {
            if !seen.insert((x, y)) {
                continue;
            }
            let idx = tile_index(x, y);
            let Some(tile) = self.map_tiles.get(idx).copied() else {
                continue;
            };
            let item = if tile.it != 0 {
                self.items
                    .get(tile.it as usize)
                    .map(|i| (tile.it as usize, *i))
            } else {
                None
            };
            snapshots.push(UndoTileSnapshot { x, y, tile, item });
        }
        snapshots
    }

    /// Push one undo action onto the bounded history stack. No-op for an empty snapshot.
    fn push_undo(&mut self, tiles: Vec<UndoTileSnapshot>) {
        if tiles.is_empty() {
            return;
        }
        self.undo_stack.push_back(UndoAction {
            tiles,
            dirty_before: self.dirty,
            dirty_tiles_before: self.dirty_tiles.clone(),
            pending_item_actions_before: self.pending_item_actions.clone(),
        });
        while self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.pop_front();
        }
    }

    /// Revert the most recent undoable action, if any.
    ///
    /// Only reverts local/unsaved editor state (map tiles, item slots, dirty
    /// bookkeeping and queued item actions) — mirrors "Revert (discard changes)".
    fn undo(&mut self) {
        let Some(action) = self.undo_stack.pop_back() else {
            self.save_status = Some("Nothing to undo".to_owned());
            return;
        };

        for snapshot in &action.tiles {
            let idx = tile_index(snapshot.x, snapshot.y);
            let Some(current_tile) = self.map_tiles.get(idx).copied() else {
                continue;
            };

            // Free any item slot the undone action allocated/reassigned.
            let current_it = current_tile.it as usize;
            let restored_it = snapshot.item.map(|(slot, _)| slot).unwrap_or(0);
            if current_it != 0 && current_it != restored_it && current_it < self.items.len() {
                self.items[current_it] = Item::default();
            }

            self.map_tiles[idx] = snapshot.tile;
            if let Some((slot, item)) = snapshot.item
                && slot < self.items.len()
            {
                self.items[slot] = item;
            }
        }

        self.dirty = action.dirty_before;
        self.dirty_tiles = action.dirty_tiles_before;
        self.pending_item_actions = action.pending_item_actions_before;
        self.save_status = Some(format!("Undid last edit ({} left)", self.undo_stack.len()));
    }

    /// Return the currently selected palette entry, clearing stale selection.
    fn selected_palette_entry(&mut self) -> Option<PaletteEntry> {
        let index = self.selected_palette_index?;
        let Some(entry) = self.palette.get(index).copied() else {
            self.selected_palette_index = None;
            return None;
        };
        Some(entry)
    }

    /// Ensure a live-API item template slot contains the full template payload.
    fn ensure_item_template_loaded(&mut self, template_id: u16) -> Result<(), String> {
        if !self.data_source.is_live_api() {
            return Ok(());
        }

        let idx = template_id as usize;
        if self.fully_loaded_item_template_slots.contains(&idx) {
            return Ok(());
        }
        if idx >= self.item_templates.len() {
            return Err(format!("Template id {} is out of range", template_id));
        }

        let Some(client) = self.admin_client.as_ref().cloned() else {
            return Err("Admin client not initialized".to_owned());
        };

        let item = client.fetch_single_item_template(idx)?;
        self.item_templates[idx] = item;
        self.fully_loaded_item_template_slots.insert(idx);
        Ok(())
    }

    /// Return whether a sprite id can be added to the map palette.
    fn can_add_palette_sprite(&mut self, ctx: &egui::Context, sprite: u16) -> Result<(), String> {
        if !sprite_is_in_allowed_palette_ranges(sprite) {
            return Err(format!(
                "Sprite {} is outside allowed map palette ranges",
                sprite
            ));
        }

        let Some(cache) = self.graphics_zip.as_mut() else {
            return Err("No graphics zip loaded".to_owned());
        };

        match cache.texture_for(ctx, sprite as usize) {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(format!("Sprite {} is not present in graphics zip", sprite)),
            Err(e) => Err(format!("Sprite {} could not be loaded: {}", sprite, e)),
        }
    }

    /// Apply a palette entry to one map tile and mark it dirty when changed.
    fn apply_palette_to_tile(&mut self, x: usize, y: usize, entry: PaletteEntry) -> bool {
        match entry.kind {
            PaletteEntryKind::Sprite { sprite, layer } => {
                self.apply_sprite_to_tile(x, y, sprite, layer)
            }
            PaletteEntryKind::ItemTemplate(template_id) => {
                self.apply_item_template_to_tile(x, y, template_id)
            }
            PaletteEntryKind::Flags { mask, clear } => self.apply_flags_to_tile(x, y, mask, clear),
        }
    }

    /// Apply a sprite to one map tile's floor or object layer and mark it dirty when changed.
    fn apply_sprite_to_tile(
        &mut self,
        x: usize,
        y: usize,
        sprite: u16,
        layer: SpriteLayer,
    ) -> bool {
        if sprite == 0 {
            return false;
        }

        let idx = tile_index(x, y);
        let Some(current) = self.map_tiles.get(idx).copied() else {
            return false;
        };

        let mut tile = current;
        match layer {
            SpriteLayer::Floor => tile.sprite = sprite,
            SpriteLayer::Object => tile.fsprite = sprite,
        }

        if tile == current {
            return false;
        }

        self.map_tiles[idx] = tile;
        self.mark_tile_dirty(x, y);
        true
    }

    /// Set or clear a mask of map flags on one tile and mark it dirty when changed.
    fn apply_flags_to_tile(&mut self, x: usize, y: usize, mask: u64, clear: bool) -> bool {
        if mask == 0 {
            return false;
        }

        let idx = tile_index(x, y);
        let Some(current) = self.map_tiles.get(idx).copied() else {
            return false;
        };

        let mut tile = current;
        if clear {
            tile.flags &= !mask;
        } else {
            tile.flags |= mask;
        }

        if tile == current {
            return false;
        }

        self.map_tiles[idx] = tile;
        self.mark_tile_dirty(x, y);
        true
    }

    /// Apply an item template to one tile.
    ///
    /// LiveApi mode queues a world action for server-managed allocation.
    /// Snapshot mode allocates a free item slot locally and patches map/items.
    fn apply_item_template_to_tile(&mut self, x: usize, y: usize, template_id: u16) -> bool {
        if template_id == 0 {
            return false;
        }

        if self.data_source.is_live_api() {
            return self.apply_item_template_to_tile_live(x, y, template_id);
        }

        self.apply_item_template_to_tile_snapshot(x, y, template_id)
    }

    /// Queue a server world action to place one map item from template.
    fn apply_item_template_to_tile_live(&mut self, x: usize, y: usize, template_id: u16) -> bool {
        if self.admin_client.is_none() {
            self.save_status = Some("Admin client not initialized".to_owned());
            return false;
        }
        if let Err(e) = self.ensure_item_template_loaded(template_id) {
            self.save_status = Some(e);
            return false;
        }

        let idx = tile_index(x, y);
        let Some(tile) = self.map_tiles.get(idx).copied() else {
            self.save_status = Some(format!("Tile ({}, {}) is out of range", x, y));
            return false;
        };
        if tile.ch != 0 || tile.to_ch != 0 {
            self.save_status = Some(format!("Tile ({}, {}) is occupied by a character", x, y));
            return false;
        }
        if (tile.flags
            & u64::from(mag_core::constants::MF_MOVEBLOCK | mag_core::constants::MF_DEATHTRAP))
            != 0
        {
            self.save_status = Some(format!("Tile ({}, {}) blocks item placement", x, y));
            return false;
        }
        if tile.fsprite != 0 {
            self.save_status = Some(format!("Tile ({}, {}) has a foreground sprite", x, y));
            return false;
        }

        if self.place_item_template_locally(x, y, template_id) {
            self.mark_item_action_pending(PendingItemAction::Place { x, y, template_id });
            self.save_status = Some(format!(
                "Queued item placement ({}, {}, template {}). Save to API to apply.",
                x, y, template_id
            ));
            true
        } else {
            false
        }
    }

    /// Allocate a free runtime item slot locally and place it on one tile.
    fn apply_item_template_to_tile_snapshot(
        &mut self,
        x: usize,
        y: usize,
        template_id: u16,
    ) -> bool {
        if self.place_item_template_locally(x, y, template_id) {
            self.mark_tile_dirty(x, y);
            true
        } else {
            false
        }
    }

    fn place_item_template_locally(&mut self, x: usize, y: usize, template_id: u16) -> bool {
        let template_idx = template_id as usize;
        if template_idx >= self.item_templates.len() {
            self.save_status = Some(format!("Item template {} is out of range", template_id));
            return false;
        }
        if self.item_templates[template_idx].used == USE_EMPTY {
            self.save_status = Some(format!("Item template {} is unused", template_id));
            return false;
        }

        let map_idx = tile_index(x, y);
        let Some(current_tile) = self.map_tiles.get(map_idx).copied() else {
            return false;
        };

        // Placing over an existing item replaces it rather than erroring.
        let replaced_item_id = current_tile.it as usize;
        if replaced_item_id != 0 && replaced_item_id < self.items.len() {
            self.items[replaced_item_id] = Item::default();
        }

        let Some(item_id) = self.find_free_snapshot_item_slot() else {
            self.save_status = Some("No free runtime item slots available".to_owned());
            return false;
        };

        let mut item = self.item_templates[template_idx];
        item.temp = template_id;
        item.x = x as u16;
        item.y = y as u16;
        item.carried = 0;

        self.items[item_id] = item;

        let mut updated_tile = current_tile;
        updated_tile.it = item_id as u32;
        updated_tile.fsprite = 0;
        self.map_tiles[map_idx] = updated_tile;
        true
    }

    fn clear_item_from_tile(&mut self, x: usize, y: usize) -> bool {
        if self.data_source.is_live_api() {
            return self.clear_item_from_tile_live(x, y);
        }

        if self.clear_item_from_tile_locally(x, y) {
            self.mark_tile_dirty(x, y);
            true
        } else {
            false
        }
    }

    fn clear_item_from_tile_live(&mut self, x: usize, y: usize) -> bool {
        if self.admin_client.is_none() {
            self.save_status = Some("Admin client not initialized".to_owned());
            return false;
        }

        let had_pending_place = self.pending_item_actions.iter().rposition(|action| {
            matches!(action, PendingItemAction::Place { x: px, y: py, .. } if *px == x && *py == y)
        });

        if !self.clear_item_from_tile_locally(x, y) {
            return false;
        }

        if let Some(action_idx) = had_pending_place {
            self.pending_item_actions.remove(action_idx);
            self.save_status = Some(format!("Canceled queued item placement at ({}, {})", x, y));
        } else {
            self.mark_item_action_pending(PendingItemAction::Clear { x, y });
            self.save_status = Some(format!(
                "Queued item clear ({}, {}). Save to API to apply.",
                x, y
            ));
        }
        self.mark_clean_if_no_pending_changes();
        true
    }

    fn clear_item_from_tile_locally(&mut self, x: usize, y: usize) -> bool {
        let map_idx = tile_index(x, y);
        let Some(current_tile) = self.map_tiles.get(map_idx).copied() else {
            return false;
        };
        let item_id = current_tile.it as usize;
        if item_id == 0 {
            self.save_status = Some(format!("Tile ({}, {}) has no item", x, y));
            return false;
        }
        if item_id < self.items.len() {
            self.items[item_id] = Item::default();
        }

        let mut updated_tile = current_tile;
        updated_tile.it = 0;
        self.map_tiles[map_idx] = updated_tile;
        true
    }

    /// Return a free snapshot-mode runtime item slot.
    fn find_free_snapshot_item_slot(&self) -> Option<usize> {
        for item_id in 1..self.items.len() {
            let item = self.items[item_id];
            if item.used != USE_EMPTY {
                continue;
            }
            if item.carried != 0 {
                continue;
            }
            if self.map_tiles.iter().any(|tile| tile.it == item_id as u32) {
                continue;
            }
            return Some(item_id);
        }
        None
    }

    /// Push every dirty map tile and queued item action to the admin API.
    ///
    /// Called instead of snapshot save in LiveApi mode. Each static tile edit
    /// produces one PUT request; item placements/removals enqueue server world
    /// actions so the running game owns runtime item slot allocation.
    fn save_to_api(&mut self) {
        self.save_status = None;
        if let Err(e) = self.sync_loaded_world_from_views() {
            self.save_status = Some(format!("Save failed: {e}"));
            return;
        }
        let Some(client) = self.admin_client.as_ref().cloned() else {
            self.save_status = Some("Admin client not initialized".to_owned());
            return;
        };

        let targets: Vec<(usize, usize)> = self.dirty_tiles.iter().copied().collect();
        let item_actions = self.pending_item_actions.clone();
        let mut pushed = 0usize;
        let mut queued_item_actions = 0usize;
        let mut errors: Vec<String> = Vec::new();

        if targets.is_empty() && item_actions.is_empty() {
            self.save_status = Some("No changes to save".to_owned());
            self.mark_clean_if_no_pending_changes();
            return;
        }

        for (x, y) in &targets {
            let idx = tile_index(*x, *y);
            let Some(tile) = self.map_tiles.get(idx) else {
                errors.push(format!("({x},{y}): out of range"));
                continue;
            };
            let patch = MapPatch {
                x: *x as u32,
                y: *y as u32,
                sprite: tile.sprite,
                fsprite: tile.fsprite,
                flags: tile.flags,
            };
            match client.put_map_tile_patch(*x, *y, &patch) {
                Ok(_) => {
                    pushed += 1;
                    self.dirty_tiles.remove(&(*x, *y));
                }
                Err(e) => errors.push(format!("({x},{y}): {e}")),
            }
        }

        let mut successful_item_action_indices = BTreeSet::new();
        for (idx, action) in item_actions.iter().enumerate() {
            let world_action = match *action {
                PendingItemAction::Place { x, y, template_id } => {
                    WorldActionKind::PlaceMapItemFromTemplate {
                        x,
                        y,
                        template_id: template_id as usize,
                    }
                }
                PendingItemAction::Clear { x, y } => WorldActionKind::ClearMapItem { x, y },
            };

            match client.request_world_action(&world_action) {
                Ok(_) => {
                    queued_item_actions += 1;
                    successful_item_action_indices.insert(idx);
                }
                Err(e) => errors.push(format!("{}: {e}", world_action.name())),
            }
        }

        if !successful_item_action_indices.is_empty() {
            let mut action_idx = 0usize;
            self.pending_item_actions.retain(|_| {
                let keep = !successful_item_action_indices.contains(&action_idx);
                action_idx += 1;
                keep
            });
        }

        if errors.is_empty() {
            self.mark_clean_if_no_pending_changes();
            self.save_status = Some(format!(
                "Saved to API: {pushed} tile(s), queued {queued_item_actions} item action(s). Use 'Reload server map' to apply."
            ));
        } else {
            self.mark_clean_if_no_pending_changes();
            self.save_status = Some(format!(
                "Save partial: {pushed} tile(s), queued {queued_item_actions} item action(s); {} error(s): {}",
                errors.len(),
                errors.join("; ")
            ));
        }
    }

    /// Open the modal dialog used to connect to the admin API.
    ///
    /// Pre-fills the form with the current LiveApi credentials when one is
    /// active, otherwise falls back to `MAG_API_BASE_URL` /
    /// `MAG_ADMIN_API_TOKEN` env vars, then to safe local-dev defaults.
    fn open_connect_dialog(&mut self) {
        match &self.data_source {
            DataSource::LiveApi { base_url, token } => {
                self.connect_form_base_url = base_url.clone();
                self.connect_form_token = token.clone();
            }
            _ => {
                if self.connect_form_base_url.is_empty() {
                    self.connect_form_base_url = std::env::var("MAG_API_BASE_URL")
                        .unwrap_or_else(|_| "https://127.0.0.1:5554".to_owned());
                }
                if self.connect_form_token.is_empty() {
                    self.connect_form_token =
                        std::env::var("MAG_ADMIN_API_TOKEN").unwrap_or_default();
                }
            }
        }
        self.connect_dialog_error = None;
        self.connect_dialog_open = true;
    }

    /// Switch the data source to LiveApi using the values in the connect
    /// dialog form, build the admin client, and reload the world.
    fn connect_to_api_from_form(&mut self) {
        let base_url = self.connect_form_base_url.trim().to_owned();
        let token = self.connect_form_token.trim().to_owned();

        if base_url.is_empty() {
            self.connect_dialog_error = Some("Base URL is required".to_owned());
            return;
        }
        if token.is_empty() {
            self.connect_dialog_error = Some("Admin token is required".to_owned());
            return;
        }

        let client = match AdminClient::new(base_url.clone(), token.clone()) {
            Ok(c) => c,
            Err(e) => {
                self.connect_dialog_error = Some(format!("Build client failed: {e}"));
                return;
            }
        };

        self.admin_client = Some(client);
        self.data_source = DataSource::LiveApi {
            base_url: base_url.clone(),
            token,
        };
        self.load_current_source();

        if let Some(err) = self.map_error.clone() {
            self.connect_dialog_error = Some(format!("Connection test failed: {err}"));
            self.admin_client = None;
            return;
        }

        self.connect_dialog_open = false;
        self.connect_dialog_error = None;
        self.save_status = Some("Connected to admin API".to_owned());
    }

    /// Fire a server-side map reload and remember the request id.
    fn request_server_map_reload(&mut self) {
        let Some(client) = self.admin_client.as_ref().cloned() else {
            self.save_status = Some("Admin client not initialized".to_owned());
            return;
        };
        match client.request_map_reload() {
            Ok(resp) => {
                self.pending_map_reload_request_id = Some(resp.request_id.clone());
                self.pending_reload_since = Some(std::time::Instant::now());
                self.last_reload_poll = None;
                self.save_status = Some(format!("Map reload requested ({})", resp.request_id));
            }
            Err(e) => {
                self.save_status = Some(format!("Map reload failed: {e}"));
            }
        }
    }

    /// Poll the most recent map-reload request once (best effort).
    fn poll_map_reload_status(&mut self) {
        let Some(request_id) = self.pending_map_reload_request_id.clone() else {
            return;
        };
        let Some(client) = self.admin_client.as_ref().cloned() else {
            return;
        };
        match client.map_reload_status(&request_id) {
            Ok(status) => {
                if status.status == "applied" {
                    self.pending_map_reload_request_id = None;
                    self.pending_reload_since = None;
                    self.last_reload_poll = None;
                    self.save_status = Some(format!("Map reload applied ({})", status.request_id));
                } else {
                    self.save_status = Some(format!(
                        "Map reload status ({request_id}): {}",
                        status.status
                    ));
                }
            }
            Err(e) => {
                self.save_status = Some(format!("Map reload status error: {e}"));
            }
        }
    }

    /// Render the modal dialog used to enter admin API connection details.
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context used to host the modal window.
    fn render_connect_dialog(&mut self, ctx: &egui::Context) {
        if !self.connect_dialog_open {
            return;
        }

        let mut still_open = true;
        let mut apply_clicked = false;
        let mut cancel_clicked = false;

        egui::Window::new("Connect to Admin API")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut still_open)
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.label(
                    "Point the map viewer at a running API service. \
                     Use a local URL when developing, or your production URL.",
                );
                ui.add_space(6.0);

                egui::Grid::new("map_connect_dialog_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Base URL:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.connect_form_base_url)
                                .hint_text("https://127.0.0.1:5554")
                                .desired_width(280.0),
                        );
                        ui.end_row();

                        ui.label("Admin token:");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.connect_form_token)
                                    .password(!self.connect_form_show_token)
                                    .hint_text("MAG_ADMIN_API_TOKEN")
                                    .desired_width(220.0),
                            );
                            ui.checkbox(&mut self.connect_form_show_token, "Show");
                        });
                        ui.end_row();
                    });

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Defaults are read from MAG_API_BASE_URL and MAG_ADMIN_API_TOKEN.",
                    )
                    .small()
                    .weak(),
                );

                if let Some(err) = &self.connect_dialog_error {
                    ui.add_space(6.0);
                    ui.colored_label(egui::Color32::RED, err);
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Connect").clicked() {
                        apply_clicked = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            });

        if cancel_clicked || !still_open {
            self.connect_dialog_open = false;
            self.connect_dialog_error = None;
        } else if apply_clicked {
            self.connect_to_api_from_form();
        }
    }

    /// Render the confirmation modal for triggering a server-side map reload.
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context used to host the modal window.
    fn render_reload_confirm_dialog(&mut self, ctx: &egui::Context) {
        if !self.reload_confirm_open {
            return;
        }

        let mut still_open = true;
        let mut confirm_clicked = false;
        let mut cancel_clicked = false;
        let has_unsaved = !self.dirty_tiles.is_empty() || !self.pending_item_actions.is_empty();

        egui::Window::new("Reload Server Map?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut still_open)
            .show(ctx, |ui| {
                ui.set_min_width(440.0);
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "\u{26A0}  This will drain pending map patches on the running server.",
                );
                ui.add_space(6.0);
                ui.label(
                    "Existing players in the affected areas will see the new tiles on their \
                     next tick. Run this only after 'Save to API' has succeeded.",
                );

                if has_unsaved {
                    ui.add_space(6.0);
                    ui.colored_label(
                        egui::Color32::RED,
                        "You have unsaved local edits. Save to API first or they will not be reloaded.",
                    );
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Reload now").color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(160, 60, 60)),
                        )
                        .clicked()
                    {
                        confirm_clicked = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            });

        if cancel_clicked || !still_open {
            self.reload_confirm_open = false;
        } else if confirm_clicked {
            self.reload_confirm_open = false;
            self.request_server_map_reload();
        }
    }

    /// Return whether a texture exists for `sprite + 1`, the companion frame the
    /// real client substitutes for foreground/object sprites in Hide Walls mode.
    fn has_object_hide_companion(&mut self, ctx: &egui::Context, sprite: u16) -> bool {
        let Some(companion) = sprite.checked_add(1) else {
            return false;
        };
        let Some(cache) = self.graphics_zip.as_mut() else {
            return true; // Can't validate without a loaded graphics zip; don't warn.
        };
        matches!(cache.texture_for(ctx, companion as usize), Ok(Some(_)))
    }

    /// Remove the currently selected palette entry.
    fn remove_selected_palette_entry(&mut self) {
        let Some(index) = self.selected_palette_index else {
            return;
        };
        if index < self.palette.len() {
            self.palette.remove(index);
        }
        self.selected_palette_index = None;
    }

    /// Save the current palette to a JSON file chosen via a file dialog.
    fn save_palette_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Palette JSON", &["json"])
            .set_file_name("palette.json")
            .save_file()
        else {
            return;
        };

        let result = serde_json::to_string_pretty(&self.palette)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));

        self.save_status = Some(match result {
            Ok(()) => format!("Saved palette: {}", path.display()),
            Err(e) => format!("Save palette failed: {e}"),
        });
    }

    /// Load a palette from a JSON file chosen via a file dialog, replacing the current one.
    fn load_palette_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Palette JSON", &["json"])
            .pick_file()
        else {
            return;
        };

        let result = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|contents| {
                serde_json::from_str::<Vec<PaletteEntry>>(&contents).map_err(|e| e.to_string())
            });

        match result {
            Ok(palette) => {
                self.palette = palette;
                self.selected_palette_index = None;
                self.save_status = Some(format!("Loaded palette: {}", path.display()));
            }
            Err(e) => {
                self.save_status = Some(format!("Load palette failed: {e}"));
            }
        }
    }

    fn render_palette_overlay(&mut self, ctx: &egui::Context, anchor: Pos2) -> Rect {
        let response = egui::Window::new("Palette")
            .id(egui::Id::new("map_palette_overlay_window"))
            .default_pos(anchor)
            .default_size(Vec2::new(360.0, 420.0))
            .resizable(true)
            .movable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.set_min_width(260.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                self.selected_palette_index.is_some(),
                                egui::Button::new("Remove selected"),
                            )
                            .clicked()
                        {
                            self.remove_selected_palette_entry();
                        }
                        if ui.small_button("Save palette...").clicked() {
                            self.save_palette_dialog();
                        }
                        if ui.small_button("Load palette...").clicked() {
                            self.load_palette_dialog();
                        }
                    });

                    ui.separator();

                    ui.add_enabled_ui(true, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("sprite:");
                            ui.add(egui::DragValue::new(&mut self.draft_sprite));
                            ui.selectable_value(
                                &mut self.draft_sprite_layer,
                                SpriteLayer::Floor,
                                "Floor",
                            );
                            ui.selectable_value(
                                &mut self.draft_sprite_layer,
                                SpriteLayer::Object,
                                "Object",
                            );

                            let preview_size = Vec2::new(96.0, 96.0);
                            let mut preview_drawn = false;

                            if let Some(cache) = self.graphics_zip.as_mut()
                                && let Ok(Some(texture)) =
                                    cache.texture_for(ctx, self.draft_sprite as usize)
                            {
                                ui.add(
                                    egui::Image::new(texture)
                                        .fit_to_exact_size(preview_size)
                                        .maintain_aspect_ratio(true),
                                );
                                preview_drawn = true;
                            }

                            if !preview_drawn {
                                ui.allocate_exact_size(preview_size, egui::Sense::hover());
                            }

                            if ui.small_button("Add").clicked() && self.draft_sprite != 0 {
                                match self.can_add_palette_sprite(ctx, self.draft_sprite) {
                                    Ok(()) => {
                                        let sprite = self.draft_sprite;
                                        let layer = self.draft_sprite_layer;
                                        self.palette
                                            .push(PaletteEntry { kind: PaletteEntryKind::Sprite { sprite, layer } });
                                        self.selected_palette_index = Some(self.palette.len() - 1);
                                        if layer == SpriteLayer::Object
                                            && !self.has_object_hide_companion(ctx, sprite)
                                        {
                                            self.save_status = Some(format!(
                                                "Added sprite {sprite} (Object); no companion texture at {} — Hide Walls will show an error texture near this tile.",
                                                u32::from(sprite) + 1
                                            ));
                                        } else {
                                            self.save_status =
                                                Some(format!("Added sprite {sprite} ({layer:?}) to palette"));
                                        }
                                    }
                                    Err(e) => {
                                        self.save_status = Some(e);
                                    }
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("template:");
                            ui.add(egui::DragValue::new(&mut self.draft_item_template_id));

                            let preview_size = Vec2::new(96.0, 96.0);
                            let mut preview_drawn = false;
                            let it_idx = self.draft_item_template_id as usize;

                            if self.draft_item_template_id != 0
                                && it_idx < self.item_templates.len()
                                && self.item_templates[it_idx].used != USE_EMPTY
                                && let Err(e) =
                                    self.ensure_item_template_loaded(self.draft_item_template_id)
                            {
                                self.item_templates_error = Some(e);
                            }

                            let preview_sprite_id = if it_idx < self.item_templates.len()
                                && self.item_templates[it_idx].used != USE_EMPTY
                            {
                                template_preview_sprite(self.item_templates[it_idx])
                            } else {
                                None
                            };

                            if let Some(sprite) = preview_sprite_id
                                && let Some(cache) = self.graphics_zip.as_mut()
                                && let Ok(Some(texture)) = cache.texture_for(ctx, sprite)
                            {
                                ui.add(
                                    egui::Image::new(texture)
                                        .fit_to_exact_size(preview_size)
                                        .maintain_aspect_ratio(true),
                                );
                                preview_drawn = true;
                            }

                            if !preview_drawn {
                                ui.allocate_exact_size(preview_size, egui::Sense::hover());
                            }

                            if let Some(sprite) = preview_sprite_id {
                                ui.label(format!("sprite: {}", sprite));
                            }

                            if ui.small_button("Add").clicked() && self.draft_item_template_id != 0
                            {
                                if it_idx >= self.item_templates.len() {
                                    self.save_status =
                                        Some("Template id is out of range".to_owned());
                                } else if let Err(e) =
                                    self.ensure_item_template_loaded(self.draft_item_template_id)
                                {
                                    self.save_status = Some(e);
                                } else if self.item_templates[it_idx].used == USE_EMPTY {
                                    self.save_status = Some("Template slot is unused".to_owned());
                                } else {
                                    self.palette.push(PaletteEntry {
                                        kind: PaletteEntryKind::ItemTemplate(
                                            self.draft_item_template_id,
                                        ),
                                    });
                                    self.selected_palette_index = Some(self.palette.len() - 1);
                                    self.save_status = Some(format!(
                                        "Added template {} to palette",
                                        self.draft_item_template_id
                                    ));
                                }
                            }

                            if self.draft_item_template_id != 0 {
                                if it_idx >= self.item_templates.len() {
                                    ui.colored_label(
                                        egui::Color32::LIGHT_RED,
                                        "Template id is out of range",
                                    );
                                } else if self.item_templates[it_idx].used == USE_EMPTY {
                                    ui.colored_label(
                                        egui::Color32::LIGHT_RED,
                                        "Template slot is unused",
                                    );
                                }
                            }
                        });

                        ui.separator();

                        ui.horizontal_wrapped(|ui| {
                            ui.label("flags:");
                            for (mask, name) in map_flag_defs() {
                                let mut on = (self.draft_flag_mask & *mask) != 0;
                                if ui.checkbox(&mut on, *name).changed() {
                                    if on {
                                        self.draft_flag_mask |= *mask;
                                    } else {
                                        self.draft_flag_mask &= !*mask;
                                    }
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.draft_flag_clear, false, "Set");
                            ui.selectable_value(&mut self.draft_flag_clear, true, "Clear");

                            if ui.small_button("Add").clicked() && self.draft_flag_mask != 0 {
                                let mask = self.draft_flag_mask;
                                let clear = self.draft_flag_clear;
                                self.palette
                                    .push(PaletteEntry { kind: PaletteEntryKind::Flags { mask, clear } });
                                self.selected_palette_index = Some(self.palette.len() - 1);
                                self.draft_flag_mask = 0;
                                self.save_status = Some(format!(
                                    "Added {} flags entry to palette",
                                    if clear { "clear" } else { "set" }
                                ));
                            }
                        });

                        ui.separator();

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let icon_size = Vec2::new(48.0, 48.0);
                            egui::Grid::new("palette_image_grid")
                                .num_columns(4)
                                .spacing([6.0, 6.0])
                                .show(ui, |ui| {
                                    let mut col = 0;
                                    for idx in 0..self.palette.len() {
                                        let entry = self.palette[idx];
                                        let sprite_id: Option<usize> = match entry.kind {
                                            PaletteEntryKind::Sprite { sprite, .. } => {
                                                if sprite == 0 {
                                                    None
                                                } else {
                                                    Some(sprite as usize)
                                                }
                                            }
                                            PaletteEntryKind::ItemTemplate(template_id) => {
                                                if template_id == 0 {
                                                    None
                                                } else {
                                                    let it_idx = template_id as usize;
                                                    if it_idx < self.item_templates.len()
                                                        && self.item_templates[it_idx].used
                                                            != USE_EMPTY
                                                    {
                                                        if let Err(e) = self
                                                            .ensure_item_template_loaded(
                                                                template_id,
                                                            )
                                                        {
                                                            self.item_templates_error = Some(e);
                                                        }
                                                        let item = self.item_templates[it_idx];
                                                        template_preview_sprite(item)
                                                    } else {
                                                        None
                                                    }
                                                }
                                            }
                                            PaletteEntryKind::Flags { .. } => None,
                                        };

                                        let selected = self.selected_palette_index == Some(idx);
                                        let tint = if selected {
                                            egui::Color32::from_rgb(180, 255, 180)
                                        } else {
                                            egui::Color32::WHITE
                                        };

                                        let clicked = if let Some(sprite_id) = sprite_id {
                                            if let Some(cache) = self.graphics_zip.as_mut() {
                                                if let Ok(Some(texture)) =
                                                    cache.texture_for(ctx, sprite_id)
                                                {
                                                    ui.add(
                                                        egui::Image::new(texture)
                                                            .fit_to_exact_size(icon_size)
                                                            .maintain_aspect_ratio(true)
                                                            .tint(tint)
                                                            .sense(egui::Sense::click()),
                                                    )
                                                    .clicked()
                                                } else {
                                                    let label = palette_entry_label(entry, Some(sprite_id));
                                                    ui.add_sized(
                                                        icon_size,
                                                        egui::Button::new(label).fill(
                                                            if selected {
                                                                egui::Color32::from_rgb(70, 110, 70)
                                                            } else {
                                                                egui::Color32::from_rgb(55, 55, 55)
                                                            },
                                                        ),
                                                    )
                                                    .clicked()
                                                }
                                            } else {
                                                false
                                            }
                                        } else {
                                            let label = palette_entry_label(entry, None);
                                            ui.add_sized(
                                                icon_size,
                                                egui::Button::new(label).fill(if selected {
                                                    egui::Color32::from_rgb(70, 110, 70)
                                                } else {
                                                    egui::Color32::from_rgb(55, 55, 55)
                                                }),
                                            )
                                            .clicked()
                                        };

                                        if clicked {
                                            if selected {
                                                self.selected_palette_index = None;
                                            } else {
                                                self.selected_palette_index = Some(idx);
                                            }
                                        }

                                        col += 1;
                                        if col == 4 {
                                            ui.end_row();
                                            col = 0;
                                        }
                                    }
                                    if col != 0 {
                                        ui.end_row();
                                    }
                                });
                        });
                    });
                });
            });

        response
            .map(|inner| inner.response.rect)
            .unwrap_or_else(|| {
                self.palette_rect
                    .unwrap_or(Rect::from_min_size(anchor, Vec2::new(260.0, 260.0)))
            })
    }
}

/// Fallback label for a palette grid cell when no texture preview is available.
fn palette_entry_label(entry: PaletteEntry, sprite_id: Option<usize>) -> String {
    match entry.kind {
        PaletteEntryKind::Sprite { sprite, layer } => {
            let prefix = match layer {
                SpriteLayer::Floor => "F",
                SpriteLayer::Object => "O",
            };
            format!("{prefix}{sprite}")
        }
        PaletteEntryKind::ItemTemplate(template_id) => match sprite_id {
            Some(sprite_id) => format!("T{template_id}\nS{sprite_id}"),
            None => format!("T{template_id}"),
        },
        PaletteEntryKind::Flags { mask, clear } => {
            let count = mask.count_ones();
            let verb = if clear { "Clear" } else { "Set" };
            format!("{verb}\n{count} flag(s)")
        }
    }
}

#[inline]
fn item_map_sprite(item: Item) -> Option<i16> {
    // Mirror server logic used to populate client map tiles.
    let hidden = (item.flags & ItemFlags::IF_HIDDEN.bits()) != 0;
    if hidden {
        return None;
    }

    let sprite = if item.active != 0 {
        item.sprite[1]
    } else {
        item.sprite[0]
    };

    if sprite > 0 { Some(sprite) } else { None }
}

#[inline]
fn template_preview_sprite(item: Item) -> Option<usize> {
    // Template preview should not hide sprites based on runtime hidden flag.
    for sprite in [item.sprite[0], item.sprite[1]] {
        if sprite > 0 {
            return Some(sprite as usize);
        }
    }
    for sprite in [item.sprite[0], item.sprite[1]] {
        if sprite < 0 {
            return Some(sprite.unsigned_abs() as usize);
        }
    }
    None
}

#[inline]
fn tile_index(x: usize, y: usize) -> usize {
    y * (SERVER_MAPX as usize) + x
}

/// Shared map flag definitions, aligned with `core/src/constants.rs`.
///
/// Used by both the per-tile flag checkboxes and the palette's flag builder.
fn map_flag_defs() -> &'static [(u64, &'static str)] {
    const DEFS: &[(u64, &str)] = &[
        (mag_core::constants::MF_MOVEBLOCK as u64, "MF_MOVEBLOCK"),
        (mag_core::constants::MF_SIGHTBLOCK as u64, "MF_SIGHTBLOCK"),
        (mag_core::constants::MF_INDOORS as u64, "MF_INDOORS"),
        (mag_core::constants::MF_UWATER as u64, "MF_UWATER"),
        (mag_core::constants::MF_NOLAG as u64, "MF_NOLAG"),
        (mag_core::constants::MF_NOMONST as u64, "MF_NOMONST"),
        (mag_core::constants::MF_BANK as u64, "MF_BANK"),
        (mag_core::constants::MF_TAVERN as u64, "MF_TAVERN"),
        (mag_core::constants::MF_NOMAGIC as u64, "MF_NOMAGIC"),
        (mag_core::constants::MF_DEATHTRAP as u64, "MF_DEATHTRAP"),
        (mag_core::constants::MF_ARENA as u64, "MF_ARENA"),
        (mag_core::constants::MF_NOEXPIRE as u64, "MF_NOEXPIRE"),
        (mag_core::constants::MF_NOFIGHT, "MF_NOFIGHT"),
        (mag_core::constants::MF_GFX_INJURED, "MF_GFX_INJURED"),
        (mag_core::constants::MF_GFX_INJURED1, "MF_GFX_INJURED1"),
        (mag_core::constants::MF_GFX_INJURED2, "MF_GFX_INJURED2"),
        (mag_core::constants::MF_GFX_TOMB, "MF_GFX_TOMB"),
        (mag_core::constants::MF_GFX_TOMB1, "MF_GFX_TOMB1"),
        (mag_core::constants::MF_GFX_DEATH, "MF_GFX_DEATH"),
        (mag_core::constants::MF_GFX_DEATH1, "MF_GFX_DEATH1"),
        (mag_core::constants::MF_GFX_EMAGIC, "MF_GFX_EMAGIC"),
        (mag_core::constants::MF_GFX_EMAGIC1, "MF_GFX_EMAGIC1"),
        (mag_core::constants::MF_GFX_GMAGIC, "MF_GFX_GMAGIC"),
        (mag_core::constants::MF_GFX_GMAGIC1, "MF_GFX_GMAGIC1"),
        (mag_core::constants::MF_GFX_CMAGIC, "MF_GFX_CMAGIC"),
        (mag_core::constants::MF_GFX_CMAGIC1, "MF_GFX_CMAGIC1"),
    ];
    DEFS
}

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 4.0;
const ZOOM_STEP: f32 = 1.15;

/// Allowed sprite-id ranges for map painting palette entries.
///
/// Keep this list curated so operators do not have to sift through unrelated
/// sprite ids from the full graphics archive.
const PALETTE_ALLOWED_SPRITE_RANGES: &[(u16, u16)] = &[(1, u16::MAX)];

#[inline]
fn sprite_is_in_allowed_palette_ranges(sprite: u16) -> bool {
    if sprite == 0 {
        return false;
    }
    PALETTE_ALLOWED_SPRITE_RANGES
        .iter()
        .any(|(start, end)| sprite >= *start && sprite <= *end)
}

#[inline]
/// Convert map-space pixels into screen-space coordinates.
fn map_to_screen(rect: Rect, pan: Vec2, zoom: f32, map_pos: Vec2) -> Pos2 {
    rect.min + pan + map_pos * zoom
}

#[inline]
/// Convert screen-space coordinates into map-space pixels.
fn screen_to_map(rect: Rect, pan: Vec2, zoom: f32, screen_pos: Pos2) -> Vec2 {
    (screen_pos - rect.min - pan) / zoom
}

#[inline]
fn dd_tile_origin_screen_pos(xpos: i32, ypos: i32) -> (i32, i32) {
    // Ported from client gameplay `legacy_engine::copysprite_screen_pos` (dd.c copysprite).
    // Returns the tile origin BEFORE sprite-size offsets.
    // NOTE: we ignore the negative-coordinate odd-bit adjustments because xpos/ypos are >= 0.
    let rx = (xpos / 2) + (ypos / 2) + 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32);
    let ry = (xpos / 4) - (ypos / 4) + YPOS;
    (rx, ry)
}

#[inline]
/// Return the visual center of the isometric floor diamond in map-space pixels.
///
/// `dd_tile_origin_screen_pos` is the pre-sprite-offset anchor (`ry`). The
/// background floor sprite is a 32x32 image drawn with its top-left at
/// `(rx - 16, ry - 32)` (see `dd_copysprite_screen_pos` with `xs == ys == 1`),
/// so the sprite spans `y` in `[ry - 32, ry]`. The actual floor diamond is the
/// bottom half of that sprite, giving a visual center of `ry - 8`. Using this
/// exact center keeps the hover/selection markers and tile picking aligned with
/// the rendered tile.
fn dd_tile_center_screen_pos(xpos: i32, ypos: i32) -> (i32, i32) {
    let (rx, ry) = dd_tile_origin_screen_pos(xpos, ypos);
    (rx, ry - 8)
}

/// Return the tile containing a map-space point inside the isometric diamond grid.
///
/// This inverts the isometric projection exactly. In screen space, stepping one
/// tile in `x` moves the diamond center by `(+16, +8)` and one tile in `y` moves
/// it by `(+16, -8)`. Expressed in those basis vectors, the floor diamonds form
/// a unit square grid, so the containing tile is found by rounding — there are no
/// boundary ties or neighbour bias.
fn map_point_to_tile(map_pos: Vec2) -> Option<(usize, usize)> {
    // Anchor: the center of tile (0, 0). Derived from the same helper used to
    // draw the markers so picking and rendering can never drift apart.
    let (ax, ay) = dd_tile_center_screen_pos(0, 0);
    // `u = x + y` and `v = x - y` recovered from the projection.
    let u = (map_pos.x - ax as f32) / 16.0;
    let v = (map_pos.y - ay as f32) / 8.0;

    let x = ((u + v) * 0.5).round() as i32;
    let y = ((u - v) * 0.5).round() as i32;

    if x < 0 || y < 0 || x >= SERVER_MAPX || y >= SERVER_MAPY {
        return None;
    }

    Some((x as usize, y as usize))
}

#[inline]
fn dd_copysprite_screen_pos(
    xpos: i32,
    ypos: i32,
    xoff: i32,
    yoff: i32,
    xs: i32,
    ys: i32,
) -> (i32, i32) {
    // Ported from client gameplay `legacy_engine::copysprite_screen_pos` (dd.c copysprite).
    let (mut rx, mut ry) = dd_tile_origin_screen_pos(xpos, ypos);
    rx -= xs * 16;
    ry -= ys * 32;
    rx += xoff;
    ry += yoff;
    (rx, ry)
}

fn clamp_range(min: i32, max: i32, lo: i32, hi: i32) -> (usize, usize) {
    let min = min.clamp(lo, hi);
    let max = max.clamp(lo, hi);
    (min as usize, max as usize)
}

/// Return every map tile touched by a straight Bresenham line.
fn line_tiles(start: (usize, usize), end: (usize, usize)) -> Vec<(usize, usize)> {
    let (mut x0, mut y0) = (start.0 as i32, start.1 as i32);
    let (x1, y1) = (end.0 as i32, end.1 as i32);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut points = Vec::new();

    loop {
        if x0 >= 0 && y0 >= 0 && x0 < SERVER_MAPX && y0 < SERVER_MAPY {
            points.push((x0 as usize, y0 as usize));
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }

    points
}

impl eframe::App for MapViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;

        // Save shortcut (Cmd+S on macOS, Ctrl+S elsewhere).
        let save_shortcut = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S));
        if save_shortcut && self.loaded_world.is_some() {
            if self.data_source.is_live_api() {
                self.save_to_api();
            } else {
                self.save_snapshot_as_dialog();
            }
        }

        // Undo shortcut (Cmd+Z on macOS, Ctrl+Z elsewhere).
        let undo_shortcut = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z));
        if undo_shortcut {
            self.undo();
            ctx.request_repaint();
        }

        // Auto-poll map-reload status every ~2 s while a request is pending.
        if self.pending_map_reload_request_id.is_some() {
            const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
            const GIVE_UP: std::time::Duration = std::time::Duration::from_secs(300);

            let since_start = self
                .pending_reload_since
                .map(|t| t.elapsed())
                .unwrap_or(GIVE_UP);

            if since_start >= GIVE_UP {
                self.pending_map_reload_request_id = None;
                self.pending_reload_since = None;
                self.last_reload_poll = None;
                self.save_status =
                    Some("Map reload status: timed out waiting for server".to_owned());
            } else {
                let should_poll = self
                    .last_reload_poll
                    .map(|t| t.elapsed() >= POLL_INTERVAL)
                    .unwrap_or(true);
                if should_poll {
                    self.last_reload_poll = Some(std::time::Instant::now());
                    self.poll_map_reload_status();
                }
                ctx.request_repaint_after(POLL_INTERVAL);
            }
        }

        self.render_connect_dialog(ctx);
        self.render_reload_confirm_dialog(ctx);

        // Load map/graphics after a couple frames (window has appeared)
        if !self.initial_load_done && self.frame_count > 2 {
            self.initial_load_done = true;
            self.load_current_source();
            if let Some(zip_path) = server_utils::graphics_zip_from_args()
                .or_else(server_utils::default_graphics_zip_path)
            {
                self.load_graphics_zip(zip_path);
            }
        }

        // Keyboard pan (WASD).
        let dt = ctx.input(|i| i.stable_dt).max(1.0 / 240.0);
        let speed = 750.0; // px/sec
        let mut delta = Vec2::ZERO;
        ctx.input(|i| {
            if i.key_down(egui::Key::W) {
                delta.y += 1.0;
            }
            if i.key_down(egui::Key::S) {
                delta.y -= 1.0;
            }
            if i.key_down(egui::Key::A) {
                delta.x += 1.0;
            }
            if i.key_down(egui::Key::D) {
                delta.x -= 1.0;
            }
        });
        if delta != Vec2::ZERO {
            self.pan += delta.normalized() * speed * dt;
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open snapshot...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("World Snapshot", &["wsnap"])
                            .pick_file()
                        {
                            self.load_from_snapshot(path);
                        }
                    }

                    let reload_label = "Reload snapshot";
                    if ui.button(reload_label).clicked() {
                        self.load_current_source();
                        ui.close_menu();
                    }

                    if ui.button("Open graphics zip...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("zip", &["zip", "ZIP"])
                            .pick_file()
                        {
                            self.load_graphics_zip(path);
                        }
                    }

                    ui.separator();

                    let save_enabled = self.loaded_world.is_some();
                    let is_live_api = self.data_source.is_live_api();
                    let save_label = if is_live_api {
                        "Save to API\tCtrl+S"
                    } else {
                        "Save Snapshot As..."
                    };
                    if ui
                        .add_enabled(save_enabled, egui::Button::new(save_label))
                        .clicked()
                    {
                        ui.close_menu();
                        if is_live_api {
                            self.save_to_api();
                        } else {
                            self.save_snapshot_as_dialog();
                        }
                    }

                    if is_live_api {
                        if ui
                            .add_enabled(
                                self.admin_client.is_some(),
                                egui::Button::new("Reload server map..."),
                            )
                            .clicked()
                        {
                            self.reload_confirm_open = true;
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(
                                self.pending_map_reload_request_id.is_some(),
                                egui::Button::new("Poll reload status"),
                            )
                            .clicked()
                        {
                            self.poll_map_reload_status();
                            ui.close_menu();
                        }
                    }

                    let revert_enabled = self.dirty;
                    if ui
                        .add_enabled(
                            revert_enabled,
                            egui::Button::new("Revert (discard changes)"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                        self.revert_unsaved_changes();
                    }

                    ui.separator();

                    ui.menu_button("Data Source", |ui| {
                        let is_snap = matches!(self.data_source, DataSource::SnapshotFile(_));
                        if ui.selectable_label(is_snap, ".wsnap Snapshot").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("World Snapshot", &["wsnap"])
                                .pick_file()
                            {
                                self.load_from_snapshot(path);
                            }
                            ui.close_menu();
                        }
                        let is_api = self.data_source.is_live_api();
                        if ui.selectable_label(is_api, "Live Admin API...").clicked() {
                            self.open_connect_dialog();
                            ui.close_menu();
                        }
                    });
                });

                ui.separator();

                if ui.button("Reset view").clicked() {
                    self.pan = Vec2::ZERO;
                    self.pan_initialized = false;
                }

                if ui
                    .button(if self.hide_enabled {
                        "Hide: ON"
                    } else {
                        "Hide: OFF"
                    })
                    .clicked()
                {
                    self.hide_enabled = !self.hide_enabled;
                    ctx.request_repaint();
                }

                if ui
                    .add_enabled(
                        !self.undo_stack.is_empty(),
                        egui::Button::new(format!("Undo ({})", self.undo_stack.len())),
                    )
                    .on_hover_text("Ctrl+Z / Cmd+Z")
                    .clicked()
                {
                    self.undo();
                }

                if self.dirty {
                    ui.separator();
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!(
                            "Unsaved: {} tile(s), {} item action(s)",
                            self.dirty_tiles.len(),
                            self.pending_item_actions.len()
                        ),
                    );
                }

                if let Some(status) = self.save_status.as_ref() {
                    ui.separator();
                    let color = if status.starts_with("Save failed")
                        || status.starts_with("Map reload failed")
                        || status.starts_with("Save partial")
                    {
                        egui::Color32::LIGHT_RED
                    } else {
                        egui::Color32::LIGHT_GREEN
                    };
                    ui.colored_label(color, status);
                }

                // Right-aligned action buttons for connection and reload.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_live_api = self.data_source.is_live_api();
                    if is_live_api {
                        let reload_btn = egui::Button::new(
                            egui::RichText::new("Reload Server Map").color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(160, 60, 60));
                        if ui
                            .add_enabled(self.admin_client.is_some(), reload_btn)
                            .on_hover_text(
                                "Ask the running server to drain pending map patches. \
                                 You will be asked to confirm.",
                            )
                            .clicked()
                        {
                            self.reload_confirm_open = true;
                        }
                    }

                    let connect_label = if is_live_api {
                        "API: Connected"
                    } else {
                        "Connect to API..."
                    };
                    if ui
                        .button(connect_label)
                        .on_hover_text(
                            "Point this viewer at a running API service \
                             (local dev or production).",
                        )
                        .clicked()
                    {
                        self.open_connect_dialog();
                    }
                });
            });
        });

        egui::SidePanel::right("side_panel")
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.heading("Map Viewer");

                ui.separator();
                ui.label(format!("Source: {}", self.data_source.display_label()));

                if let Some(err) = &self.map_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
                if let Some(err) = &self.graphics_zip_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
                if let Some(err) = &self.items_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
                if let Some(err) = &self.item_templates_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }

                ui.separator();
                ui.label(format!("Map size: {} x {}", SERVER_MAPX, SERVER_MAPY));
                ui.label(format!("Loaded tiles: {}", self.map_tiles.len()));

                ui.separator();
                ui.label("Controls:");
                ui.label("- WASD: pan");
                ui.label("- Drag: pan");
                ui.label("- Mouse wheel: zoom");
                ui.label("- Shift + left click: line mode");

                ui.separator();
                ui.label(format!("Pan: [{:.1}, {:.1}]", self.pan.x, self.pan.y));
                ui.label(format!("Zoom: {:.0}%", self.zoom * 100.0));
                if let Some((x, y)) = self.line_anchor {
                    ui.label(format!("Line anchor: ({}, {})", x, y));
                }

                ui.separator();
                {
                    let (hx, hy, hover_tile) = if let Some((x, y)) = self.hovered_tile {
                        if !self.map_tiles.is_empty() {
                            let idx = tile_index(x, y);
                            if idx < self.map_tiles.len() {
                                (Some(x), Some(y), Some(self.map_tiles[idx]))
                            } else {
                                (Some(x), Some(y), None)
                            }
                        } else {
                            (Some(x), Some(y), None)
                        }
                    } else {
                        (None, None, None)
                    };

                    if let (Some(x), Some(y)) = (hx, hy) {
                        ui.label(format!("Hover tile: ({}, {})", x, y));
                    } else {
                        ui.label("Hover tile: (N/A)");
                    }

                    let preview_size = Vec2::new(64.0, 64.0);
                    if let Some(tile) = hover_tile {
                        let sprite = tile.sprite;
                        let fsprite = tile.fsprite;
                        let flags = tile.flags;
                        let light = tile.light;
                        let dlight = tile.dlight;
                        let ch = tile.ch;
                        let to_ch = tile.to_ch;
                        let it = tile.it;

                        ui.label(format!("sprite: {}", sprite));
                        ui.label(format!("fsprite: {}", fsprite));
                        ui.label(format!("flags: 0x{:016X}", flags));
                        ui.label(format!("light: {} (dlight {})", light, dlight));
                        ui.label(format!("ch: {} to_ch: {} it: {}", ch, to_ch, it));
                        self.ui_tile_preview_row(ui, ctx, sprite, fsprite, it, preview_size);

                        if it != 0 {
                            let it_idx = it as usize;
                            if it_idx < self.items.len() {
                                let item = self.items[it_idx];
                                let sprite = item_map_sprite(item).unwrap_or(0);
                                ui.label(format!("item sprite: {}", sprite));
                                ui.label(format!("item template: {}", item.temp));
                            } else {
                                ui.label("item sprite: (item data not loaded)");
                                ui.label("item template: (item data not loaded)");
                            }
                        } else {
                            ui.label("item sprite: N/A");
                            ui.label("item template: N/A");
                        }
                    } else {
                        ui.label("sprite: N/A");
                        ui.label("fsprite: N/A");
                        ui.label("flags: N/A");
                        ui.label("light: N/A");
                        ui.label("ch: N/A to_ch: N/A it: N/A");
                        self.ui_tile_preview_row(ui, ctx, 0, 0, 0, preview_size);
                        ui.label("item sprite: N/A");
                        ui.label("item template: N/A");
                    }
                }

                ui.separator();
                if let Some((x, y)) = self.selected_tile {
                    ui.label(format!("Selected tile: ({}, {})", x, y));
                    if !self.map_tiles.is_empty() {
                        let idx = tile_index(x, y);
                        if idx < self.map_tiles.len() {
                            let tile = self.map_tiles[idx];
                            let sprite = tile.sprite;
                            let fsprite = tile.fsprite;
                            let mut flags = tile.flags;
                            let light = tile.light;
                            let dlight = tile.dlight;
                            let ch = tile.ch;
                            let to_ch = tile.to_ch;
                            let it = tile.it;

                            ui.label(format!("sprite: {}", sprite));
                            ui.label(format!("fsprite: {}", fsprite));
                            ui.label(format!("flags: 0x{:016X}", flags));
                            ui.label(format!("light: {} (dlight {})", light, dlight));
                            ui.label(format!("ch: {} to_ch: {} it: {}", ch, to_ch, it));

                            // Visual preview of the selected tile's sprites.
                            let preview_size = Vec2::new(64.0, 64.0);
                            self.ui_tile_preview_row(ui, ctx, sprite, fsprite, it, preview_size);

                            ui.horizontal(|ui| {
                                if sprite != 0 && ui.button("Clear sprite").clicked() {
                                    let mut updated = self.map_tiles[idx];
                                    updated.sprite = 0;
                                    if updated != self.map_tiles[idx] {
                                        let undo_snapshot = self.snapshot_tiles_for_undo(&[(x, y)]);
                                        self.map_tiles[idx] = updated;
                                        self.mark_tile_dirty(x, y);
                                        self.push_undo(undo_snapshot);
                                        ctx.request_repaint();
                                    }
                                }

                                if fsprite != 0 && ui.button("Clear fsprite").clicked() {
                                    let mut updated = self.map_tiles[idx];
                                    updated.fsprite = 0;
                                    if updated != self.map_tiles[idx] {
                                        let undo_snapshot = self.snapshot_tiles_for_undo(&[(x, y)]);
                                        self.map_tiles[idx] = updated;
                                        self.mark_tile_dirty(x, y);
                                        self.push_undo(undo_snapshot);
                                        ctx.request_repaint();
                                    }
                                }
                            });

                            if it != 0 {
                                let it_idx = it as usize;
                                if it_idx < self.items.len() {
                                    let item = self.items[it_idx];
                                    let sprite = item_map_sprite(item).unwrap_or(0);
                                    ui.label(format!("item sprite: {}", sprite));
                                    ui.label(format!("item template: {}", item.temp));
                                } else {
                                    ui.label("item sprite: (item data not loaded)");
                                    ui.label("item template: (item data not loaded)");
                                }
                                if ui.button("Clear item").clicked() {
                                    let undo_snapshot = self.snapshot_tiles_for_undo(&[(x, y)]);
                                    if self.clear_item_from_tile(x, y) {
                                        self.push_undo(undo_snapshot);
                                        ctx.request_repaint();
                                    }
                                }
                            } else {
                                ui.label("item sprite: N/A");
                                ui.label("item template: N/A");
                            }

                            ui.separator();
                            ui.label("Map flags:");
                            let original_flags = flags;

                            let defs = map_flag_defs();

                            ui.add_enabled_ui(true, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(220.0)
                                    .show(ui, |ui| {
                                        egui::Grid::new("selected_tile_map_flags")
                                            .num_columns(2)
                                            .spacing([10.0, 4.0])
                                            .show(ui, |ui| {
                                                for (i, (mask, name)) in defs.iter().enumerate() {
                                                    let mut on = (flags & *mask) != 0;
                                                    if ui.checkbox(&mut on, *name).changed() {
                                                        if on {
                                                            flags |= *mask;
                                                        } else {
                                                            flags &= !*mask;
                                                        }
                                                    }
                                                    if i % 2 == 1 {
                                                        ui.end_row();
                                                    }
                                                }
                                                if defs.len() % 2 == 1 {
                                                    ui.end_row();
                                                }
                                            });
                                    });
                            });

                            if flags != original_flags {
                                let mut updated = self.map_tiles[idx];
                                updated.flags = flags;
                                if updated != self.map_tiles[idx] {
                                    let undo_snapshot = self.snapshot_tiles_for_undo(&[(x, y)]);
                                    self.map_tiles[idx] = updated;
                                    self.mark_tile_dirty(x, y);
                                    self.push_undo(undo_snapshot);
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                } else {
                    ui.label("Selected tile: (none)");
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

            // Overlay palette anchored in the map canvas.
            let palette_rect =
                self.render_palette_overlay(ctx, rect.left_top() + Vec2::new(12.0, 12.0));
            self.palette_rect = Some(palette_rect);

            if response.dragged() {
                self.pan += response.drag_delta();
                ctx.request_repaint();
            }

            if response.clicked_by(egui::PointerButton::Primary) {
                let pointer_pos = ctx.pointer_latest_pos();
                let clicked_palette = pointer_pos.is_some_and(|p| palette_rect.contains(p));

                if !clicked_palette {
                    let Some(entry) = self.selected_palette_entry() else {
                        self.line_anchor = None;
                        // No palette selection => select the tile (freeze details).
                        if let Some((x, y)) = self.hovered_tile {
                            self.selected_tile = Some((x, y));
                            ctx.request_repaint();
                        }
                        return;
                    };
                    let Some((x, y)) = self.hovered_tile else {
                        self.line_anchor = None;
                        return;
                    };

                    let shift_held = ctx.input(|i| i.modifiers.shift);
                    let coords: Vec<(usize, usize)> = if shift_held {
                        let start = self.line_anchor.unwrap_or((x, y));
                        line_tiles(start, (x, y))
                    } else {
                        vec![(x, y)]
                    };

                    let undo_snapshot = self.snapshot_tiles_for_undo(&coords);
                    let mut changed = false;
                    for (line_x, line_y) in &coords {
                        changed |= self.apply_palette_to_tile(*line_x, *line_y, entry);
                    }
                    self.line_anchor = if shift_held { Some((x, y)) } else { None };

                    if changed {
                        self.push_undo(undo_snapshot);
                        ctx.request_repaint();
                    }
                } else {
                    self.line_anchor = None;
                }
            }

            if !ctx.input(|i| i.modifiers.shift) {
                self.line_anchor = None;
            }

            // Auto-center on first paint after load.
            if !self.pan_initialized && !self.map_tiles.is_empty() {
                let mid_x = (SERVER_MAPX as usize) / 2;
                let mid_y = (SERVER_MAPY as usize) / 2;
                let xpos = (mid_x as i32) * 32;
                let ypos = (mid_y as i32) * 32;
                let (tx, ty) = dd_tile_origin_screen_pos(xpos, ypos);
                self.pan = (rect.center() - rect.min) - Vec2::new(tx as f32, ty as f32) * self.zoom;
                self.pan_initialized = true;
            }

            let zoom_delta = ctx.input(|i| i.raw_scroll_delta.y);
            if zoom_delta != 0.0
                && let Some(pointer_pos) = ctx.pointer_latest_pos()
                && rect.contains(pointer_pos)
                && !palette_rect.contains(pointer_pos)
            {
                let old_zoom = self.zoom;
                let factor = if zoom_delta > 0.0 {
                    ZOOM_STEP
                } else {
                    1.0 / ZOOM_STEP
                };
                let new_zoom = (old_zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
                if (new_zoom - old_zoom).abs() > f32::EPSILON {
                    let map_under_pointer = screen_to_map(rect, self.pan, old_zoom, pointer_pos);
                    self.zoom = new_zoom;
                    self.pan = pointer_pos - rect.min - map_under_pointer * new_zoom;
                    ctx.request_repaint();
                }
            }

            // Compute hovered tile from mouse position using the isometric floor diamond.
            self.hovered_tile = ctx.pointer_latest_pos().and_then(|pos| {
                if !rect.contains(pos) {
                    return None;
                }

                map_point_to_tile(screen_to_map(rect, self.pan, self.zoom, pos))
            });

            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 22, 26));

            if self.map_tiles.is_empty() {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No map loaded (Open dat dir...) ",
                    egui::TextStyle::Heading.resolve(ui.style()),
                    egui::Color32::GRAY,
                );
                return;
            }

            // Visible range estimation: compute tile coord bounds for the canvas corners.
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.left_bottom(),
                rect.right_bottom(),
            ];
            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for c in corners {
                let local = screen_to_map(rect, self.pan, self.zoom, c);
                let base_x = local.x - (32 + XPOS - (((TILEX as i32 - 34) / 2) * 32)) as f32;
                let base_y = local.y - (YPOS as f32);
                let xf = 0.5 * (base_x / 16.0 + base_y / 8.0);
                let yf = 0.5 * (base_x / 16.0 - base_y / 8.0);
                min_x = min_x.min(xf);
                max_x = max_x.max(xf);
                min_y = min_y.min(yf);
                max_y = max_y.max(yf);
            }

            // Expand to be safe (sprites extend beyond the anchor).
            let margin = 6;
            let (x0, x1) = clamp_range(
                min_x.floor() as i32 - margin,
                max_x.ceil() as i32 + margin,
                0,
                SERVER_MAPX - 1,
            );
            let (y0, y1) = clamp_range(
                min_y.floor() as i32 - margin,
                max_y.ceil() as i32 + margin,
                0,
                SERVER_MAPY - 1,
            );

            let Some(cache) = self.graphics_zip.as_mut() else {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No graphics zip loaded (Open graphics zip...) ",
                    egui::TextStyle::Heading.resolve(ui.style()),
                    egui::Color32::GRAY,
                );
                return;
            };

            // Draw order: match the legacy scan order used by gameplay's pass-2 painter ordering.
            // Equivalent shape to `tile_draw_order = (TILEY-1-y)*TILEX + x` but for SERVER_MAP dims.
            // Larger `y` is higher on screen (ry ~= 8*x - 8*y), so it must be drawn first.
            let w = SERVER_MAPX as usize;
            let h = SERVER_MAPY as usize;

            for y in (y0..=y1).rev() {
                if y >= h {
                    continue;
                }
                for x in x0..=x1 {
                    if x >= w {
                        continue;
                    }

                    let idx = tile_index(x, y);
                    if idx >= self.map_tiles.len() {
                        continue;
                    }

                    let tile = self.map_tiles[idx];
                    let xpos = (x as i32) * 32;
                    let ypos = (y as i32) * 32;

                    // Background
                    if tile.sprite != 0
                        && let Err(e) = paint_sprite_dd(
                            &painter,
                            ctx,
                            cache,
                            tile.sprite as usize,
                            rect,
                            self.pan,
                            self.zoom,
                            xpos,
                            ypos,
                            0,
                            0,
                            egui::Color32::WHITE,
                        )
                    {
                        self.graphics_zip_error = Some(e);
                    }

                    // Foreground
                    if tile.fsprite != 0 {
                        // Match client hide logic: substitute sprite_id + 1 when hide is enabled
                        let sprite_id = if self.hide_enabled {
                            tile.fsprite + 1
                        } else {
                            tile.fsprite
                        };
                        if let Err(e) = paint_sprite_dd(
                            &painter,
                            ctx,
                            cache,
                            sprite_id as usize,
                            rect,
                            self.pan,
                            self.zoom,
                            xpos,
                            ypos,
                            0,
                            0,
                            egui::Color32::WHITE,
                        ) {
                            self.graphics_zip_error = Some(e);
                        }
                    } else if tile.it != 0 {
                        // Item overlay (Map.it is an item instance id).
                        let it_idx = tile.it as usize;
                        if it_idx < self.items.len() {
                            let item = self.items[it_idx];
                            let item_sprite = item_map_sprite(item);
                            if let Some(item_sprite) = item_sprite {
                                // Highlight items red when hovering over them
                                let is_hovered = self.hovered_tile == Some((x, y));
                                let is_selected = self.selected_tile == Some((x, y));
                                let tint = if is_hovered || is_selected {
                                    egui::Color32::from_rgb(255, 50, 50)
                                } else {
                                    egui::Color32::WHITE
                                };
                                if let Err(e) = paint_sprite_dd(
                                    &painter,
                                    ctx,
                                    cache,
                                    item_sprite as usize,
                                    rect,
                                    self.pan,
                                    self.zoom,
                                    xpos,
                                    ypos,
                                    0,
                                    0,
                                    tint,
                                ) {
                                    self.graphics_zip_error = Some(e);
                                }
                            }
                        }
                    }
                }
            }

            // Highlight hovered tile.
            if let Some((x, y)) = self.hovered_tile {
                let xpos = (x as i32) * 32;
                let ypos = (y as i32) * 32;
                let (tx, ty) = dd_tile_center_screen_pos(xpos, ypos);
                let pos = map_to_screen(rect, self.pan, self.zoom, Vec2::new(tx as f32, ty as f32));
                let radius = (6.0 * self.zoom).clamp(4.0, 14.0);
                painter.circle_stroke(pos, radius, (2.0, egui::Color32::YELLOW));
            }

            if let (Some((ax, ay)), Some((hx, hy))) = (self.line_anchor, self.hovered_tile) {
                let start_xpos = (ax as i32) * 32;
                let start_ypos = (ay as i32) * 32;
                let end_xpos = (hx as i32) * 32;
                let end_ypos = (hy as i32) * 32;
                let (sx, sy) = dd_tile_center_screen_pos(start_xpos, start_ypos);
                let (ex, ey) = dd_tile_center_screen_pos(end_xpos, end_ypos);
                let start =
                    map_to_screen(rect, self.pan, self.zoom, Vec2::new(sx as f32, sy as f32));
                let end = map_to_screen(rect, self.pan, self.zoom, Vec2::new(ex as f32, ey as f32));
                painter.line_segment([start, end], (2.0, egui::Color32::LIGHT_GREEN));
                painter.circle_stroke(
                    start,
                    (5.0 * self.zoom).clamp(4.0, 12.0),
                    (2.0, egui::Color32::LIGHT_GREEN),
                );
            }

            // Highlight selected tile (persistent).
            if let Some((x, y)) = self.selected_tile {
                let xpos = (x as i32) * 32;
                let ypos = (y as i32) * 32;
                let (tx, ty) = dd_tile_center_screen_pos(xpos, ypos);
                let pos = map_to_screen(rect, self.pan, self.zoom, Vec2::new(tx as f32, ty as f32));
                let radius = (7.0 * self.zoom).clamp(5.0, 16.0);
                painter.circle_stroke(pos, radius, (3.0, egui::Color32::from_rgb(255, 50, 50)));
            }
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_sprite_dd(
    painter: &egui::Painter,
    ctx: &egui::Context,
    cache: &mut GraphicsZipCache,
    sprite_id: usize,
    rect: Rect,
    pan: Vec2,
    zoom: f32,
    xpos: i32,
    ypos: i32,
    xoff: i32,
    yoff: i32,
    tint: egui::Color32,
) -> Result<(), String> {
    let Some(texture) = cache.texture_for(ctx, sprite_id)? else {
        return Ok(());
    };

    let size = texture.size();
    let xs = (size[0] as i32) / 32;
    let ys = (size[1] as i32) / 32;
    let (rx, ry) = dd_copysprite_screen_pos(xpos, ypos, xoff, yoff, xs, ys);
    let top_left = map_to_screen(rect, pan, zoom, Vec2::new(rx as f32, ry as f32));
    let dst = Rect::from_min_size(top_left, texture.size_vec2() * zoom);

    painter.image(
        texture.id(),
        dst,
        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
        tint,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MapViewerApp, PaletteEntry, PaletteEntryKind, SpriteLayer, dd_tile_center_screen_pos,
        line_tiles, map_point_to_tile, tile_index,
    };
    use eframe::egui::Vec2;
    use mag_core::constants::{USE_ACTIVE, USE_EMPTY};
    use mag_core::types::Item;

    /// Build a bare `MapViewerApp` with just enough map/item state for painting tests.
    fn test_app(tile_count: usize, item_count: usize) -> MapViewerApp {
        let mut app = MapViewerApp::default();
        app.map_tiles = vec![super::Map::default(); tile_count];
        app.items = vec![Item::default(); item_count];
        app.item_templates = vec![Item::default(); 4];
        app
    }

    #[test]
    fn apply_sprite_to_tile_writes_floor_layer() {
        let mut app = test_app(4, 4);
        let idx = tile_index(0, 0);
        assert!(app.apply_sprite_to_tile(0, 0, 5, SpriteLayer::Floor));
        assert_eq!(app.map_tiles[idx].sprite, 5);
        assert_eq!(app.map_tiles[idx].fsprite, 0);
    }

    #[test]
    fn apply_sprite_to_tile_writes_object_layer() {
        let mut app = test_app(4, 4);
        let idx = tile_index(0, 0);
        assert!(app.apply_sprite_to_tile(0, 0, 7, SpriteLayer::Object));
        assert_eq!(app.map_tiles[idx].fsprite, 7);
        assert_eq!(app.map_tiles[idx].sprite, 0);
    }

    #[test]
    fn apply_palette_to_tile_dispatches_by_kind() {
        let mut app = test_app(4, 4);
        let idx = tile_index(0, 0);

        let floor = PaletteEntry {
            kind: PaletteEntryKind::Sprite {
                sprite: 3,
                layer: SpriteLayer::Floor,
            },
        };
        assert!(app.apply_palette_to_tile(0, 0, floor));
        assert_eq!(app.map_tiles[idx].sprite, 3);

        let set_flags = PaletteEntry {
            kind: PaletteEntryKind::Flags {
                mask: 0b101,
                clear: false,
            },
        };
        assert!(app.apply_palette_to_tile(0, 0, set_flags));
        assert_eq!(app.map_tiles[idx].flags, 0b101);

        let clear_flags = PaletteEntry {
            kind: PaletteEntryKind::Flags {
                mask: 0b001,
                clear: true,
            },
        };
        assert!(app.apply_palette_to_tile(0, 0, clear_flags));
        assert_eq!(app.map_tiles[idx].flags, 0b100);
    }

    #[test]
    fn place_item_template_locally_replaces_existing_item_instead_of_erroring() {
        let mut app = test_app(4, 10);
        app.item_templates[1].used = USE_ACTIVE;

        let idx = tile_index(0, 0);
        app.items[5].used = USE_ACTIVE;
        app.map_tiles[idx].it = 5;

        assert!(app.place_item_template_locally(0, 0, 1));

        // Old slot is freed rather than the placement erroring out.
        assert_eq!(app.items[5].used, USE_EMPTY);

        let new_it = app.map_tiles[idx].it as usize;
        assert_ne!(new_it, 5);
        assert_eq!(app.items[new_it].temp, 1);
    }

    #[test]
    fn undo_reverts_sprite_paint_and_item_placement() {
        let mut app = test_app(4, 10);
        app.item_templates[1].used = USE_ACTIVE;
        let idx = tile_index(0, 0);

        let snap = app.snapshot_tiles_for_undo(&[(0, 0)]);
        assert!(app.apply_sprite_to_tile(0, 0, 9, SpriteLayer::Floor));
        app.push_undo(snap);
        assert_eq!(app.map_tiles[idx].sprite, 9);

        app.undo();
        assert_eq!(app.map_tiles[idx].sprite, 0);
        assert!(app.undo_stack.is_empty());

        let snap = app.snapshot_tiles_for_undo(&[(0, 0)]);
        assert!(app.place_item_template_locally(0, 0, 1));
        app.push_undo(snap);
        let placed_it = app.map_tiles[idx].it as usize;
        assert_ne!(placed_it, 0);

        app.undo();
        assert_eq!(app.map_tiles[idx].it, 0);
        assert_eq!(app.items[placed_it].used, USE_EMPTY);
    }

    #[test]
    fn undo_history_is_capped_at_max_undo_history() {
        let mut app = test_app(4, 4);
        for sprite in 1..=(super::MAX_UNDO_HISTORY as u16 + 5) {
            let snap = app.snapshot_tiles_for_undo(&[(0, 0)]);
            app.apply_sprite_to_tile(0, 0, sprite, SpriteLayer::Floor);
            app.push_undo(snap);
        }
        assert_eq!(app.undo_stack.len(), super::MAX_UNDO_HISTORY);
    }

    #[test]
    fn line_tiles_single_point() {
        assert_eq!(line_tiles((7, 9), (7, 9)), vec![(7, 9)]);
    }

    #[test]
    fn line_tiles_horizontal_includes_endpoints() {
        assert_eq!(
            line_tiles((2, 4), (6, 4)),
            vec![(2, 4), (3, 4), (4, 4), (5, 4), (6, 4)]
        );
    }

    #[test]
    fn line_tiles_vertical_includes_endpoints() {
        assert_eq!(
            line_tiles((3, 2), (3, 6)),
            vec![(3, 2), (3, 3), (3, 4), (3, 5), (3, 6)]
        );
    }

    #[test]
    fn line_tiles_diagonal_includes_endpoints() {
        assert_eq!(
            line_tiles((1, 1), (4, 4)),
            vec![(1, 1), (2, 2), (3, 3), (4, 4)]
        );
    }

    #[test]
    fn line_tiles_shallow_slope() {
        assert_eq!(
            line_tiles((1, 1), (5, 3)),
            vec![(1, 1), (2, 2), (3, 2), (4, 3), (5, 3)]
        );
    }

    #[test]
    fn line_tiles_steep_slope() {
        assert_eq!(
            line_tiles((1, 1), (3, 5)),
            vec![(1, 1), (2, 2), (2, 3), (3, 4), (3, 5)]
        );
    }

    #[test]
    fn line_tiles_reversed_matches_reverse_order() {
        assert_eq!(
            line_tiles((5, 3), (1, 1)),
            vec![(5, 3), (4, 2), (3, 2), (2, 1), (1, 1)]
        );
    }

    #[test]
    fn map_point_to_tile_resolves_tile_center_to_same_tile() {
        let (cx, cy) = dd_tile_center_screen_pos(10 * 32, 20 * 32);
        assert_eq!(
            map_point_to_tile(Vec2::new(cx as f32, cy as f32)),
            Some((10, 20))
        );
    }

    #[test]
    fn map_point_to_tile_separates_vertical_neighbor_centers() {
        let (cx, cy) = dd_tile_center_screen_pos(10 * 32, 20 * 32);
        let (below_cx, below_cy) = dd_tile_center_screen_pos(10 * 32, 21 * 32);
        assert_eq!(
            map_point_to_tile(Vec2::new(cx as f32, cy as f32)),
            Some((10, 20))
        );
        assert_eq!(
            map_point_to_tile(Vec2::new(below_cx as f32, below_cy as f32)),
            Some((10, 21))
        );
    }

    #[test]
    fn map_point_to_tile_matches_screen_stacked_neighbors() {
        // Tiles stack vertically on screen: a point one diamond-height below a
        // tile center belongs to (x + 1, y - 1); one above belongs to
        // (x - 1, y + 1). This locks the floor-diamond center orientation so it
        // cannot silently flip sign again.
        let (cx, cy) = dd_tile_center_screen_pos(10 * 32, 20 * 32);
        assert_eq!(
            map_point_to_tile(Vec2::new(cx as f32, cy as f32 + 16.0)),
            Some((11, 19))
        );
        assert_eq!(
            map_point_to_tile(Vec2::new(cx as f32, cy as f32 - 16.0)),
            Some((9, 21))
        );
    }

    #[test]
    fn map_point_to_tile_stays_on_tile_across_interior() {
        // Every point inside a tile's diamond must resolve to that tile.
        let (cx, cy) = dd_tile_center_screen_pos(40 * 32, 25 * 32);
        let interior = [
            (0.0, 0.0),
            (10.0, 0.0),
            (-10.0, 0.0),
            (0.0, 5.0),
            (0.0, -5.0),
            (6.0, 3.0),
            (-6.0, -3.0),
        ];
        for (dx, dy) in interior {
            assert_eq!(
                map_point_to_tile(Vec2::new(cx as f32 + dx, cy as f32 + dy)),
                Some((40, 25)),
                "interior offset ({dx}, {dy}) should stay on (40, 25)"
            );
        }
    }
}
