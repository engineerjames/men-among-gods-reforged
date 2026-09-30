//! Tile, flag, and item mutations shared by palette painting and the tile inspector.

use super::MapViewerApp;
use super::PendingItemAction;
use super::geometry::tile_index;
use super::palette::{PaletteEntry, PaletteEntryKind, SpriteLayer};
use mag_core::constants::{ItemFlags, MF_DEATHTRAP, MF_MOVEBLOCK, USE_EMPTY};
use mag_core::types::{Item, Map};

impl MapViewerApp {
    /// Mutate one tile in place and mark it dirty when the edit changed anything.
    ///
    /// # Arguments
    ///
    /// * `x` - Tile X coordinate.
    /// * `y` - Tile Y coordinate.
    /// * `edit` - Mutation applied to a copy of the tile.
    ///
    /// # Returns
    ///
    /// * `true` when the tile changed.
    pub(super) fn modify_tile(&mut self, x: usize, y: usize, edit: impl FnOnce(&mut Map)) -> bool {
        let idx = tile_index(x, y);
        let Some(current) = self.map_tiles.get(idx).copied() else {
            return false;
        };
        let mut updated = current;
        edit(&mut updated);
        if updated == current {
            return false;
        }
        self.map_tiles[idx] = updated;
        self.mark_tile_dirty(x, y);
        true
    }

    /// Apply a palette entry to one map tile and mark it dirty when changed.
    ///
    /// # Returns
    ///
    /// * `true` when the tile (or its item) changed.
    pub(super) fn apply_palette_to_tile(
        &mut self,
        x: usize,
        y: usize,
        entry: PaletteEntry,
    ) -> bool {
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

    /// Write a sprite to one tile's floor or object layer.
    ///
    /// # Returns
    ///
    /// * `true` when the tile changed.
    pub(super) fn apply_sprite_to_tile(
        &mut self,
        x: usize,
        y: usize,
        sprite: u16,
        layer: SpriteLayer,
    ) -> bool {
        if sprite == 0 {
            return false;
        }
        self.modify_tile(x, y, |tile| match layer {
            SpriteLayer::Floor => tile.sprite = sprite,
            SpriteLayer::Object => tile.fsprite = sprite,
        })
    }

    /// Set or clear a mask of map flags on one tile.
    ///
    /// # Returns
    ///
    /// * `true` when the tile changed.
    fn apply_flags_to_tile(&mut self, x: usize, y: usize, mask: u64, clear: bool) -> bool {
        if mask == 0 {
            return false;
        }
        self.modify_tile(x, y, |tile| {
            if clear {
                tile.flags &= !mask;
            } else {
                tile.flags |= mask;
            }
        })
    }

    /// Ensure a live-API item template slot contains the full template payload.
    ///
    /// # Returns
    ///
    /// * `Err` with a user-facing message when the template can't be fetched.
    pub(super) fn ensure_item_template_loaded(&mut self, template_id: u16) -> Result<(), String> {
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

        if self.place_item_template_locally(x, y, template_id) {
            self.mark_tile_dirty(x, y);
            true
        } else {
            false
        }
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

        let Some(tile) = self.map_tiles.get(tile_index(x, y)).copied() else {
            self.save_status = Some(format!("Tile ({}, {}) is out of range", x, y));
            return false;
        };
        if tile.ch != 0 || tile.to_ch != 0 {
            self.save_status = Some(format!("Tile ({}, {}) is occupied by a character", x, y));
            return false;
        }
        if tile.flags & u64::from(MF_MOVEBLOCK | MF_DEATHTRAP) != 0 {
            self.save_status = Some(format!("Tile ({}, {}) blocks item placement", x, y));
            return false;
        }
        if tile.fsprite != 0 {
            self.save_status = Some(format!("Tile ({}, {}) has a foreground sprite", x, y));
            return false;
        }

        if !self.place_item_template_locally(x, y, template_id) {
            return false;
        }
        self.mark_item_action_pending(PendingItemAction::Place { x, y, template_id });
        self.save_status = Some(format!(
            "Queued item placement ({}, {}, template {}). Save to API to apply.",
            x, y, template_id
        ));
        true
    }

    /// Allocate a free runtime item slot locally and place it on one tile,
    /// replacing any item already there.
    ///
    /// # Returns
    ///
    /// * `true` when the item was placed.
    pub(super) fn place_item_template_locally(
        &mut self,
        x: usize,
        y: usize,
        template_id: u16,
    ) -> bool {
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

        let tile = &mut self.map_tiles[map_idx];
        tile.it = item_id as u32;
        tile.fsprite = 0;
        true
    }

    /// Remove the item on one tile (queued as a world action in LiveApi mode).
    ///
    /// # Returns
    ///
    /// * `true` when an item was removed or a queued placement was canceled.
    pub(super) fn clear_item_from_tile(&mut self, x: usize, y: usize) -> bool {
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

    /// LiveApi item removal: cancels a pending placement or queues a clear action.
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

    /// Free the item slot referenced by one tile and unlink it.
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
        self.map_tiles[map_idx].it = 0;
        true
    }

    /// Lowest free runtime item slot not referenced by any tile.
    fn find_free_snapshot_item_slot(&self) -> Option<usize> {
        (1..self.items.len()).find(|&item_id| {
            let item = self.items[item_id];
            item.used == USE_EMPTY
                && item.carried == 0
                && !self.map_tiles.iter().any(|tile| tile.it == item_id as u32)
        })
    }
}

/// Map sprite an item instance shows on the ground, mirroring server map population.
///
/// # Returns
///
/// * `None` for hidden items or items without a positive sprite.
pub(super) fn item_map_sprite(item: Item) -> Option<i16> {
    if (item.flags & ItemFlags::IF_HIDDEN.bits()) != 0 {
        return None;
    }
    let sprite = if item.active != 0 {
        item.sprite[1]
    } else {
        item.sprite[0]
    };
    (sprite > 0).then_some(sprite)
}

/// Preview sprite for an item template, ignoring the runtime hidden flag.
///
/// # Returns
///
/// * The first positive sprite, else the magnitude of the first negative one.
pub(super) fn template_preview_sprite(item: Item) -> Option<usize> {
    let sprites = [item.sprite[0], item.sprite[1]];
    sprites
        .iter()
        .find(|s| **s > 0)
        .or_else(|| sprites.iter().find(|s| **s < 0))
        .map(|s| s.unsigned_abs() as usize)
}

#[cfg(test)]
mod tests {
    use super::super::MapViewerApp;
    use super::super::geometry::tile_index;
    use super::super::palette::{PaletteEntry, PaletteEntryKind, SpriteLayer};
    use super::template_preview_sprite;
    use mag_core::constants::{USE_ACTIVE, USE_EMPTY};
    use mag_core::types::Item;

    #[test]
    fn apply_sprite_to_tile_writes_floor_layer() {
        let mut app = MapViewerApp::for_tests(4, 4);
        let idx = tile_index(0, 0);
        assert!(app.apply_sprite_to_tile(0, 0, 5, SpriteLayer::Floor));
        assert_eq!(app.map_tiles[idx].sprite, 5);
        assert_eq!(app.map_tiles[idx].fsprite, 0);
    }

    #[test]
    fn apply_sprite_to_tile_writes_object_layer() {
        let mut app = MapViewerApp::for_tests(4, 4);
        let idx = tile_index(0, 0);
        assert!(app.apply_sprite_to_tile(0, 0, 7, SpriteLayer::Object));
        assert_eq!(app.map_tiles[idx].fsprite, 7);
        assert_eq!(app.map_tiles[idx].sprite, 0);
    }

    #[test]
    fn apply_palette_to_tile_dispatches_by_kind() {
        let mut app = MapViewerApp::for_tests(4, 4);
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
        assert!(!app.apply_palette_to_tile(0, 0, clear_flags));
    }

    #[test]
    fn place_item_template_locally_replaces_existing_item_instead_of_erroring() {
        let mut app = MapViewerApp::for_tests(4, 10);
        app.item_templates[1].used = USE_ACTIVE;

        let idx = tile_index(0, 0);
        app.items[5].used = USE_ACTIVE;
        app.map_tiles[idx].it = 5;

        assert!(app.place_item_template_locally(0, 0, 1));
        assert_eq!(app.items[5].used, USE_EMPTY);

        let new_it = app.map_tiles[idx].it as usize;
        assert_eq!(app.items[new_it].temp, 1);
    }

    #[test]
    fn clear_item_from_tile_frees_slot_in_snapshot_mode() {
        let mut app = MapViewerApp::for_tests(4, 10);
        app.item_templates[1].used = USE_ACTIVE;
        assert!(app.place_item_template_locally(0, 0, 1));
        let it = app.map_tiles[tile_index(0, 0)].it as usize;

        assert!(app.clear_item_from_tile(0, 0));
        assert_eq!(app.map_tiles[tile_index(0, 0)].it, 0);
        assert_eq!(app.items[it].used, USE_EMPTY);
        assert!(!app.clear_item_from_tile(0, 0));
    }

    #[test]
    fn template_preview_sprite_prefers_positive_then_negative() {
        let mut item = Item::default();
        assert_eq!(template_preview_sprite(item), None);
        item.sprite = [-4, 0];
        assert_eq!(template_preview_sprite(item), Some(4));
        item.sprite = [-4, 9];
        assert_eq!(template_preview_sprite(item), Some(9));
    }
}
