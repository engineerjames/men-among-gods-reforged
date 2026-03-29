//! World (map) input handling for [`GameScene`].
//!
//! Keyboard number hotkeys and mouse-button-up world interactions are split
//! out here so that the main `handle_event` in `mod.rs` stays readable.

use sdl2::{keyboard::Keycode, mouse::MouseButton};

use mag_core::constants::{ISCHAR, ISITEM, ISUSABLE};

use crate::{network::client_commands::ClientCommand, scenes::scene::SceneType, state::AppState};

use super::GameScene;

impl GameScene {
    /// Dispatch a `KeyDown` Num1–Num9 event to the appropriate skill keybind slot.
    ///
    /// Silently no-ops when chat is focused or no network/player-state is
    /// available, so callers do not need to pre-check those conditions.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    /// * `kc` - The number keycode (`Num1`–`Num9`).
    pub(super) fn handle_num_hotkey(&mut self, app_state: &mut AppState<'_>, kc: Keycode) {
        if self.chat_box.is_focused() {
            return;
        }
        let key_slot = (i32::from(kc) - i32::from(Keycode::Num1)) as usize;
        if let (Some(net), Some(ps)) = (app_state.network.as_ref(), app_state.player_state.as_ref())
        {
            if let Some(skill_nr) = app_state.settings.character.skill_keybinds[key_slot] {
                self.play_click_sound(app_state);
                net.send(ClientCommand::new_skill(
                    skill_nr as u32,
                    Self::default_skill_target(ps),
                    ps.character_info().attrib[0][0] as u32,
                ));
            }
        }
    }

    /// Resolve a `MouseButtonUp` event against the world map and send the
    /// appropriate network command.
    ///
    /// Returns early (returning `None`) when the click falls outside the
    /// visible map area, or when no player/network state is available.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    /// * `mouse_btn` - Which mouse button was released.
    /// * `x` - Screen X coordinate of the release point.
    /// * `y` - Screen Y coordinate of the release point.
    ///
    /// # Returns
    ///
    /// * `None` in all normal cases (world clicks do not trigger scene transitions).
    pub(super) fn handle_world_click(
        &mut self,
        app_state: &mut AppState<'_>,
        mouse_btn: MouseButton,
        x: i32,
        y: i32,
    ) -> Option<SceneType> {
        let Some(ps) = app_state.player_state.as_ref() else {
            log::warn!("Mouse click with no player state");
            return None;
        };

        let (cam_xoff, cam_yoff) = Self::camera_offsets(ps);

        let Some((mx, my)) = Self::screen_to_map_tile(x, y, cam_xoff, cam_yoff) else {
            log::warn!("Click outside of map area: screen=({}, {})", x, y);
            return None;
        };

        let has_ctrl = self.ctrl_held;
        let has_shift = self.shift_held;
        let has_alt = self.alt_held;

        // Read citem early so we can suppress ISITEM snapping when the
        // player is carrying an item and wants to drop, not pick up.
        let citem = ps.character_info().citem;

        let snapped = if has_ctrl || has_alt {
            Self::nearest_tile_with_flag(ps, mx, my, ISCHAR).unwrap_or((mx, my))
        } else if has_shift && citem == 0 {
            // Only snap to the nearest item tile when the hand is empty.
            // With a citem held, use the raw tile so the drop lands where
            // the player clicked rather than locking onto a nearby item.
            Self::nearest_tile_with_flag(ps, mx, my, ISITEM).unwrap_or((mx, my))
        } else {
            (mx, my)
        };

        let (sx, sy) = snapped;
        let tile = ps.map().tile_at_xy(sx, sy);
        let target_cn = tile.map(|t| t.ch_nr as u32).unwrap_or(0);
        let target_id = tile.map(|t| t.ch_id).unwrap_or(0);
        let (world_x, world_y) = tile.map(|t| (t.x as i16, t.y as i32)).unwrap_or((0, 0));
        // citem already read above.
        let selected_char = ps.selected_char();

        let Some(net) = app_state.network.as_ref() else {
            return None;
        };

        match mouse_btn {
            MouseButton::Left if has_alt => {
                if let Some(ps_mut) = app_state.player_state.as_mut() {
                    if target_cn != 0 {
                        if selected_char == target_cn as u16 {
                            ps_mut.clear_selected_char();
                        } else {
                            ps_mut.set_selected_char_with_id(target_cn as u16, target_id);
                        }
                    } else {
                        ps_mut.clear_selected_char();
                    }
                }
            }
            MouseButton::Right if has_alt => {
                if target_cn != 0 {
                    self.play_click_sound(app_state);
                    net.send(ClientCommand::new_look(target_cn));
                }
            }
            MouseButton::Left if has_ctrl => {
                if target_cn != 0 {
                    self.play_click_sound(app_state);
                    if citem != 0 {
                        net.send(ClientCommand::new_give(target_cn));
                    } else {
                        net.send(ClientCommand::new_attack(target_cn));
                    }
                }
            }
            MouseButton::Right if has_ctrl => {
                if target_cn != 0 {
                    self.play_click_sound(app_state);
                    net.send(ClientCommand::new_look(target_cn));
                }
            }
            MouseButton::Left if has_shift => {
                let tile_flags = tile.map(|t| t.flags).unwrap_or(0);
                let is_item = (tile_flags & ISITEM) != 0;
                let is_usable = (tile_flags & ISUSABLE) != 0;
                if citem != 0 && !is_item {
                    // Holding item, clicked non-item tile --> drop
                    self.play_click_sound(app_state);
                    net.send(ClientCommand::new_drop(world_x, world_y));
                } else if is_item && is_usable {
                    // Item is usable --> use
                    self.play_click_sound(app_state);
                    net.send(ClientCommand::new_use(world_x, world_y));
                } else if is_item {
                    // Item not usable --> pickup
                    self.play_click_sound(app_state);
                    net.send(ClientCommand::new_pickup(world_x, world_y));
                }
            }
            MouseButton::Right if has_shift => {
                self.play_click_sound(app_state);
                net.send(ClientCommand::new_look_item(world_x, world_y));
            }
            MouseButton::Left => {
                self.play_click_sound(app_state);
                net.send(ClientCommand::new_move(world_x, world_y));
            }
            MouseButton::Right => {
                self.play_click_sound(app_state);
                net.send(ClientCommand::new_turn(world_x, world_y));
            }
            _ => {}
        }

        None
    }
}
