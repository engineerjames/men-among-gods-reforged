//! Bounded, local-only undo history for map edits.

use super::geometry::tile_index;
use super::{MapViewerApp, PendingItemAction};
use mag_core::types::{Item, Map};
use std::collections::BTreeSet;

/// Maximum number of edits kept in the undo history.
pub(super) const MAX_UNDO_HISTORY: usize = 10;

/// Pre-mutation snapshot of one tile.
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
pub(super) struct UndoAction {
    tiles: Vec<UndoTileSnapshot>,
    dirty_before: bool,
    dirty_tiles_before: BTreeSet<(usize, usize)>,
    pending_item_actions_before: Vec<PendingItemAction>,
}

impl MapViewerApp {
    /// Capture tile and dirty-tracking state before an edit, deduping coordinates.
    ///
    /// # Arguments
    ///
    /// * `coords` - Tiles about to be modified.
    ///
    /// # Returns
    ///
    /// * An action to hand to [`Self::push_undo`] once the edit is known to have changed something.
    pub(super) fn snapshot_for_undo(&self, coords: &[(usize, usize)]) -> UndoAction {
        let mut seen = BTreeSet::new();
        let mut tiles = Vec::new();
        for &(x, y) in coords {
            if !seen.insert((x, y)) {
                continue;
            }
            let Some(tile) = self.map_tiles.get(tile_index(x, y)).copied() else {
                continue;
            };
            let item = (tile.it != 0)
                .then(|| {
                    self.items
                        .get(tile.it as usize)
                        .map(|i| (tile.it as usize, *i))
                })
                .flatten();
            tiles.push(UndoTileSnapshot { x, y, tile, item });
        }
        UndoAction {
            tiles,
            dirty_before: self.dirty,
            dirty_tiles_before: self.dirty_tiles.clone(),
            pending_item_actions_before: self.pending_item_actions.clone(),
        }
    }

    /// Push one undo action onto the bounded history. No-op when it captured no tiles.
    ///
    /// # Arguments
    ///
    /// * `action` - Pre-edit state from [`Self::snapshot_for_undo`].
    pub(super) fn push_undo(&mut self, action: UndoAction) {
        if action.tiles.is_empty() {
            return;
        }
        self.undo_stack.push_back(action);
        while self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.pop_front();
        }
    }

    /// Apply `edit` to one tile as a single undoable action.
    ///
    /// # Arguments
    ///
    /// * `x` - Tile X coordinate.
    /// * `y` - Tile Y coordinate.
    /// * `edit` - Mutation applied to a copy of the tile.
    ///
    /// # Returns
    ///
    /// * `true` when the tile changed (and an undo entry was recorded).
    pub(super) fn edit_tile_with_undo(
        &mut self,
        x: usize,
        y: usize,
        edit: impl FnOnce(&mut Map),
    ) -> bool {
        let snapshot = self.snapshot_for_undo(&[(x, y)]);
        let changed = self.modify_tile(x, y, edit);
        if changed {
            self.push_undo(snapshot);
        }
        changed
    }

    /// Revert the most recent undoable action, if any.
    ///
    /// Only reverts local/unsaved editor state (map tiles, item slots, dirty
    /// bookkeeping and queued item actions) — mirrors "Revert (discard changes)".
    pub(super) fn undo(&mut self) {
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
}

#[cfg(test)]
mod tests {
    use super::super::MapViewerApp;
    use super::super::geometry::tile_index;
    use super::super::palette::SpriteLayer;
    use super::MAX_UNDO_HISTORY;
    use mag_core::constants::{USE_ACTIVE, USE_EMPTY};

    #[test]
    fn undo_reverts_sprite_paint_and_item_placement() {
        let mut app = MapViewerApp::for_tests(4, 10);
        app.item_templates[1].used = USE_ACTIVE;
        let idx = tile_index(0, 0);

        let snap = app.snapshot_for_undo(&[(0, 0)]);
        assert!(app.apply_sprite_to_tile(0, 0, 9, SpriteLayer::Floor));
        app.push_undo(snap);
        assert_eq!(app.map_tiles[idx].sprite, 9);

        app.undo();
        assert_eq!(app.map_tiles[idx].sprite, 0);
        assert!(app.undo_stack.is_empty());

        let snap = app.snapshot_for_undo(&[(0, 0)]);
        assert!(app.place_item_template_locally(0, 0, 1));
        app.push_undo(snap);
        let placed_it = app.map_tiles[idx].it as usize;
        assert_ne!(placed_it, 0);

        app.undo();
        assert_eq!(app.map_tiles[idx].it, 0);
        assert_eq!(app.items[placed_it].used, USE_EMPTY);
    }

    #[test]
    fn undo_restores_pre_edit_dirty_state() {
        let mut app = MapViewerApp::for_tests(4, 4);
        assert!(app.edit_tile_with_undo(0, 0, |t| t.flags = 0b1));
        assert!(app.dirty);
        assert_eq!(app.dirty_tiles.len(), 1);

        app.undo();
        assert!(!app.dirty);
        assert!(app.dirty_tiles.is_empty());
        assert_eq!(app.map_tiles[tile_index(0, 0)].flags, 0);
    }

    #[test]
    fn edit_tile_with_undo_skips_noop_edits() {
        let mut app = MapViewerApp::for_tests(4, 4);
        assert!(!app.edit_tile_with_undo(0, 0, |t| t.sprite = 0));
        assert!(app.undo_stack.is_empty());
        assert!(!app.dirty);
    }

    #[test]
    fn undo_history_is_capped_at_max_undo_history() {
        let mut app = MapViewerApp::for_tests(4, 4);
        for sprite in 1..=(MAX_UNDO_HISTORY as u16 + 5) {
            let snap = app.snapshot_for_undo(&[(0, 0)]);
            app.apply_sprite_to_tile(0, 0, sprite, SpriteLayer::Floor);
            app.push_undo(snap);
        }
        assert_eq!(app.undo_stack.len(), MAX_UNDO_HISTORY);
    }
}
