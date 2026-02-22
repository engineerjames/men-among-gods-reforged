//! Click-handling helpers for the stat/inventory/skill/shop UI panels and
//! the mode/skill-button bar.

use sdl2::mouse::MouseButton;

use mag_core::types::skilltab::{get_skill_name, get_skill_nr};

use crate::network::client_commands::ClientCommand;
use crate::state::AppState;

use super::GameScene;

impl GameScene {
    /// Handle clicks on the mode/skill-button bar at the bottom of the game HUD.
    ///
    /// Right-clicking a skill-button slot either assigns `pending_skill_assignment` to it
    /// or clears it. Left-clicking a slot fires the bound skill. The second row of
    /// buttons toggles combat mode, %-display, hide, and show-names.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Mutable reference to the shared application state.
    /// * `mouse_btn` - Which mouse button was pressed.
    /// * `x` - Click x coordinate in HUD-local space.
    /// * `y` - Click y coordinate in HUD-local space.
    ///
    /// # Returns
    ///
    /// `true` if the click was consumed (within a button region), `false` otherwise.
    pub(super) fn click_mode_or_skill_button(
        &mut self,
        app_state: &mut AppState,
        mouse_btn: MouseButton,
        x: i32,
        y: i32,
    ) -> bool {
        let mut profile_changed = false;

        // --- Right-click on skill button slot: assign or clear ---
        if mouse_btn == MouseButton::Right && (610..=798).contains(&x) && (504..=548).contains(&y) {
            let col = ((x - 610) / 49) as usize;
            let row = ((y - 504) / 15) as usize;
            if col < 4 && row < 3 {
                let idx = row * 4 + col;
                if let Some(skill_id) = self.pending_skill_assignment.take() {
                    // Complete the assignment.
                    if let Some(ps) = app_state.player_state.as_mut() {
                        let name = get_skill_name(skill_id);
                        ps.player_data_mut().skill_buttons[idx].set_name(name);
                        ps.player_data_mut().skill_buttons[idx]
                            .set_skill_nr(get_skill_nr(skill_id) as u32);
                        ps.tlog(1, &format!("Assigned {} to slot {}.", name, idx + 1));
                        profile_changed = true;
                    }
                } else {
                    // No pending assignment — clear the slot.
                    if let Some(ps) = app_state.player_state.as_mut() {
                        if !ps.player_data().skill_buttons[idx].is_unassigned() {
                            ps.player_data_mut().skill_buttons[idx].set_unassigned();
                            ps.tlog(1, &format!("Cleared slot {}.", idx + 1));
                            profile_changed = true;
                        }
                    }
                }
                if profile_changed {
                    self.save_active_profile(app_state);
                }
                return true;
            }
        }

        if mouse_btn != MouseButton::Left {
            return false;
        }

        // Skill button labels area: 4x3
        if (610..=798).contains(&x) && (504..=548).contains(&y) {
            let col = ((x - 610) / 49) as usize;
            let row = ((y - 504) / 15) as usize;
            if col < 4 && row < 3 {
                let idx = row * 4 + col;
                if let (Some(net), Some(ps)) =
                    (app_state.network.as_ref(), app_state.player_state.as_ref())
                {
                    let btn = ps.player_data().skill_buttons[idx];
                    if !btn.is_unassigned() {
                        net.send(ClientCommand::new_skill(
                            btn.skill_nr(),
                            Self::default_skill_target(ps),
                            ps.character_info().attrib[0][0] as u32,
                        ));
                    }
                }
                return true;
            }
        }

        // Mode/toggle buttons area: two rows, 4 cols, trans_button geometry.
        if (604..=798).contains(&x) && (552..=582).contains(&y) {
            let col = (x - 604) / 49;
            let row = (y - 552) / 16;
            if let Some(net) = app_state.network.as_ref() {
                if row == 0 {
                    match col {
                        0 => net.send(ClientCommand::new_mode(2)),
                        1 => net.send(ClientCommand::new_mode(1)),
                        2 => net.send(ClientCommand::new_mode(0)),
                        3 => {
                            if let Some(ps) = app_state.player_state.as_mut() {
                                let cur = ps.player_data().show_proz;
                                ps.player_data_mut().show_proz = 1 - cur;
                                profile_changed = true;
                            }
                        }
                        _ => {}
                    }
                } else if row == 1 {
                    match col {
                        1 => {
                            if let Some(ps) = app_state.player_state.as_mut() {
                                let cur = ps.player_data().hide;
                                ps.player_data_mut().hide = 1 - cur;
                                profile_changed = true;
                            }
                        }
                        2 => {
                            if let Some(ps) = app_state.player_state.as_mut() {
                                let cur = ps.player_data().show_names;
                                ps.player_data_mut().show_names = 1 - cur;
                                profile_changed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            if profile_changed {
                self.save_active_profile(app_state);
            }
            return true;
        }

        false
    }

    /// Handle clicks on the stat/inventory/worn/shop panels (left-hand side + shop overlay).
    ///
    /// Covers the stat +/- buttons, the "Update" commit button, scroll arrows for
    /// inventory and skills, skill-row activation (LMB) and assignment (RMB),
    /// inventory backpack clicks, worn-equipment clicks, and the shop/depot/grave
    /// overlay grid.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Mutable reference to the shared application state.
    /// * `mouse_btn` - Which mouse button was pressed.
    /// * `x` - Click x coordinate in HUD-local space.
    /// * `y` - Click y coordinate in HUD-local space.
    ///
    /// # Returns
    ///
    /// `true` if the click was consumed, `false` otherwise.
    pub(super) fn click_stat_or_inv(
        &mut self,
        app_state: &mut AppState,
        mouse_btn: MouseButton,
        x: i32,
        y: i32,
    ) -> bool {
        // Extract all data we need from player_state up front to avoid borrow conflicts.
        let (ci, selected_char) = {
            let Some(ps) = app_state.player_state.as_ref() else {
                return false;
            };
            (ps.character_info().clone(), ps.selected_char() as u32)
        };
        let skill_target = if selected_char != 0 {
            selected_char
        } else {
            ci.attack_cn.max(0) as u32
        };

        // --- Stat/skill commit ("Update") button: x=109..158, y=254..266 (LMB only) ---
        if mouse_btn == MouseButton::Left && (109..=158).contains(&x) && (254..=266).contains(&y) {
            let sorted = Self::sorted_skills(&ci);
            for n in 0usize..108 {
                let v = self.stat_raised[n];
                if v == 0 {
                    continue;
                }
                let which: i16 = if n >= 8 {
                    let Some(&skill_id) = sorted.get(n - 8) else {
                        continue;
                    };
                    (get_skill_nr(skill_id) + 8) as i16
                } else {
                    n as i16
                };
                if let Some(net) = app_state.network.as_ref() {
                    net.send(ClientCommand::new_stat(which, v));
                }
            }
            self.stat_raised = [0; 108];
            self.stat_points_used = 0;
            return true;
        }

        // --- Scroll arrow buttons (orig/inter.c::button_command cases 12-15) ---
        // Inventory up/down arrows.
        if mouse_btn == MouseButton::Left && x > 290 && y > 1 && x < 300 && y < 34 {
            if self.inv_scroll > 1 {
                self.inv_scroll = self.inv_scroll.saturating_sub(2);
            }
            return true;
        }
        if mouse_btn == MouseButton::Left && x > 290 && y > 141 && x < 300 && y < 174 {
            if self.inv_scroll < 30 {
                self.inv_scroll = (self.inv_scroll + 2).min(30);
            }
            return true;
        }

        // Skill list up/down arrows.
        if mouse_btn == MouseButton::Left && x > 206 && x < 218 && y > 113 && y < 148 {
            if self.skill_scroll > 1 {
                self.skill_scroll = self.skill_scroll.saturating_sub(2);
            }
            return true;
        }
        if mouse_btn == MouseButton::Left && x > 206 && x < 218 && y > 218 && y < 252 {
            if self.skill_scroll < 40 {
                self.skill_scroll = (self.skill_scroll + 2).min(40);
            }
            return true;
        }

        // --- Skill row click: x=2..108, y=114..251 (10 visible rows) ---
        // Matches orig/inter.c::mouse_statbox2 (left click sends CL_CMD_SKILL for clicked row).
        if mouse_btn == MouseButton::Left && (2..=108).contains(&x) && (114..=251).contains(&y) {
            let row = ((y - 114) / 14) as usize;
            if row < 10 {
                let sorted = Self::sorted_skills(&ci);
                let skilltab_index = self.skill_scroll + row;
                if let Some(&skill_id) = sorted.get(skilltab_index) {
                    if !get_skill_name(skill_id).is_empty() && ci.skill[skill_id][0] != 0 {
                        if let Some(net) = app_state.network.as_ref() {
                            net.send(ClientCommand::new_skill(
                                get_skill_nr(skill_id) as u32,
                                skill_target,
                                1,
                            ));
                        }
                    }
                }
            }
            return true;
        }

        // --- Right-click skill row: begin spell-bar assignment ---
        if mouse_btn == MouseButton::Right && (2..=108).contains(&x) && (114..=251).contains(&y) {
            let row = ((y - 114) / 14) as usize;
            if row < 10 {
                let sorted = Self::sorted_skills(&ci);
                let skilltab_index = self.skill_scroll + row;
                if let Some(&skill_id) = sorted.get(skilltab_index) {
                    let name = get_skill_name(skill_id);
                    if !name.is_empty() && ci.skill[skill_id][0] != 0 {
                        self.pending_skill_assignment = Some(skill_id);
                        if let Some(ps) = app_state.player_state.as_mut() {
                            ps.tlog(1, &format!("Right-click a spell slot to assign {}.", name));
                        }
                    }
                }
            }
            return true;
        }

        // --- Stat +/- buttons: x=133..157, y=2..251 ---
        // + button: x < 145  |  - button: x >= 145
        // Row n = (y-2)/14.  Rows 0-4 = attrib, 5=HP, 6=End, 7=Mana, 8+ = skills
        if (133..=157).contains(&x)
            && (2..=251).contains(&y)
            && matches!(mouse_btn, MouseButton::Left)
        {
            let n = ((y - 2) / 14) as usize;
            let raising = x < 145;
            let repeat = if self.ctrl_held {
                90
            } else if self.shift_held {
                10
            } else {
                1
            };
            let sorted = Self::sorted_skills(&ci);

            let avail_now = ci.points - self.stat_points_used;
            let button_visible = if raising {
                match n {
                    0..=4 => {
                        let cur = ci.attrib[n][0] as i32 + self.stat_raised[n];
                        let need = Self::attrib_needed(&ci, n, cur);
                        need != i32::MAX && need <= avail_now
                    }
                    5 => {
                        let cur = ci.hp[0] as i32 + self.stat_raised[5];
                        let need = Self::hp_needed(&ci, cur);
                        need != i32::MAX && need <= avail_now
                    }
                    6 => {
                        let cur = ci.end[0] as i32 + self.stat_raised[6];
                        let need = Self::end_needed(&ci, cur);
                        need != i32::MAX && need <= avail_now
                    }
                    7 => {
                        let cur = ci.mana[0] as i32 + self.stat_raised[7];
                        let need = Self::mana_needed(&ci, cur);
                        need != i32::MAX && need <= avail_now
                    }
                    _ => {
                        let skilltab_index = (self.skill_scroll + n.saturating_sub(8)).min(99);
                        let raised_idx = 8 + skilltab_index;
                        if raised_idx >= 108 {
                            false
                        } else if let Some(&skill_id) = sorted.get(skilltab_index) {
                            if ci.skill[skill_id][0] == 0 {
                                false
                            } else {
                                let cur =
                                    ci.skill[skill_id][0] as i32 + self.stat_raised[raised_idx];
                                let need = Self::skill_needed(&ci, skill_id, cur);
                                need != i32::MAX && need <= avail_now
                            }
                        } else {
                            false
                        }
                    }
                }
            } else {
                match n {
                    0..=7 => self.stat_raised[n] > 0,
                    _ => {
                        let skilltab_index = (self.skill_scroll + n.saturating_sub(8)).min(99);
                        let raised_idx = 8 + skilltab_index;
                        raised_idx < 108 && self.stat_raised[raised_idx] > 0
                    }
                }
            };

            if !button_visible {
                return true;
            }

            for _ in 0..repeat {
                let avail = ci.points - self.stat_points_used;
                if raising {
                    match n {
                        0..=4 => {
                            let cur = ci.attrib[n][0] as i32 + self.stat_raised[n];
                            let need = Self::attrib_needed(&ci, n, cur);
                            if need != i32::MAX && need <= avail {
                                self.stat_points_used += need;
                                self.stat_raised[n] += 1;
                            }
                        }
                        5 => {
                            let cur = ci.hp[0] as i32 + self.stat_raised[5];
                            let need = Self::hp_needed(&ci, cur);
                            if need != i32::MAX && need <= avail {
                                self.stat_points_used += need;
                                self.stat_raised[5] += 1;
                            }
                        }
                        6 => {
                            let cur = ci.end[0] as i32 + self.stat_raised[6];
                            let need = Self::end_needed(&ci, cur);
                            if need != i32::MAX && need <= avail {
                                self.stat_points_used += need;
                                self.stat_raised[6] += 1;
                            }
                        }
                        7 => {
                            let cur = ci.mana[0] as i32 + self.stat_raised[7];
                            let need = Self::mana_needed(&ci, cur);
                            if need != i32::MAX && need <= avail {
                                self.stat_points_used += need;
                                self.stat_raised[7] += 1;
                            }
                        }
                        _ => {
                            let skilltab_index = (self.skill_scroll + n.saturating_sub(8)).min(99);
                            let raised_idx = 8 + skilltab_index;
                            if raised_idx < 108 {
                                if let Some(&skill_id) = sorted.get(skilltab_index) {
                                    if ci.skill[skill_id][0] != 0 {
                                        let cur = ci.skill[skill_id][0] as i32
                                            + self.stat_raised[raised_idx];
                                        let need = Self::skill_needed(&ci, skill_id, cur);
                                        if need != i32::MAX && need <= avail {
                                            self.stat_points_used += need;
                                            self.stat_raised[raised_idx] += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Lowering
                    match n {
                        0..=4 => {
                            if self.stat_raised[n] > 0 {
                                self.stat_raised[n] -= 1;
                                let cur = ci.attrib[n][0] as i32 + self.stat_raised[n];
                                let refund = Self::attrib_needed(&ci, n, cur);
                                if refund != i32::MAX {
                                    self.stat_points_used -= refund;
                                }
                            }
                        }
                        5 => {
                            if self.stat_raised[5] > 0 {
                                self.stat_raised[5] -= 1;
                                let cur = ci.hp[0] as i32 + self.stat_raised[5];
                                let refund = Self::hp_needed(&ci, cur);
                                if refund != i32::MAX {
                                    self.stat_points_used -= refund;
                                }
                            }
                        }
                        6 => {
                            if self.stat_raised[6] > 0 {
                                self.stat_raised[6] -= 1;
                                let cur = ci.end[0] as i32 + self.stat_raised[6];
                                let refund = Self::end_needed(&ci, cur);
                                if refund != i32::MAX {
                                    self.stat_points_used -= refund;
                                }
                            }
                        }
                        7 => {
                            if self.stat_raised[7] > 0 {
                                self.stat_raised[7] -= 1;
                                let cur = ci.mana[0] as i32 + self.stat_raised[7];
                                let refund = Self::mana_needed(&ci, cur);
                                if refund != i32::MAX {
                                    self.stat_points_used -= refund;
                                }
                            }
                        }
                        _ => {
                            let skilltab_index = (self.skill_scroll + n.saturating_sub(8)).min(99);
                            let raised_idx = 8 + skilltab_index;
                            if raised_idx < 108 && self.stat_raised[raised_idx] > 0 {
                                self.stat_raised[raised_idx] -= 1;
                                if let Some(&skill_id) = sorted.get(skilltab_index) {
                                    let cur =
                                        ci.skill[skill_id][0] as i32 + self.stat_raised[raised_idx];
                                    let refund = Self::skill_needed(&ci, skill_id, cur);
                                    if refund != i32::MAX {
                                        self.stat_points_used -= refund;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return true;
        }

        // --- Inventory backpack click: x=220..290, y=2..177 (2 cols × 35px, 5 rows × 35px) ---
        if (220..=290).contains(&x) && (2..=177).contains(&y) {
            let col = (x - 220) / 35;
            let row = (y - 2) / 35;
            if col < 2 && row < 5 {
                let idx = (self.inv_scroll + (row * 2 + col) as usize) as u32;
                if let Some(net) = app_state.network.as_ref() {
                    if mouse_btn == MouseButton::Right {
                        net.send(ClientCommand::new_inv_look(idx, 0, selected_char));
                    } else {
                        let a = if self.shift_held { 0u32 } else { 6u32 };
                        net.send(ClientCommand::new_inv(a, idx, selected_char));
                    }
                }
                return true;
            }
        }

        // --- Worn equipment click: x=303..373, y=2..212 (2 cols × 35px, 6 rows × 35px) ---
        // Slot remapping from orig/inter.c::mouse_inventory.
        if (303..=373).contains(&x) && (2..=212).contains(&y) {
            let tx = (x - 303) / 35;
            let ty = (y - 2) / 35;
            let slot_nr: Option<u32> = match (tx, ty) {
                (0, 0) => Some(0),  // head
                (1, 0) => Some(9),  // cloak
                (0, 1) => Some(2),  // body
                (1, 1) => Some(3),  // arms
                (0, 2) => Some(1),  // neck
                (1, 2) => Some(4),  // belt
                (0, 3) => Some(8),  // right hand
                (1, 3) => Some(7),  // left hand
                (0, 4) => Some(10), // left ring
                (1, 4) => Some(11), // right ring
                (0, 5) => Some(5),  // legs
                (1, 5) => Some(6),  // feet
                _ => None,
            };
            if let Some(slot_nr) = slot_nr {
                if let Some(net) = app_state.network.as_ref() {
                    // RMB=7 (right-click worn), LMB+Shift=1 (shift-equip), LMB=5 (normal equip)
                    let a = match mouse_btn {
                        MouseButton::Right => 7u32,
                        MouseButton::Left if self.shift_held => 1u32,
                        _ => 5u32,
                    };
                    net.send(ClientCommand::new_inv(a, slot_nr, selected_char));
                }
                return true;
            }
        }

        // --- Shop / depot / grave overlay clicks (orig/inter.c::mouse_shop) ---
        let Some(ps) = app_state.player_state.as_ref() else {
            return false;
        };
        if ps.should_show_shop() {
            let in_shop_window = x > 220 && x < 516 && y > 260 && y < 485 + 32 + 35;

            // Close button: x 499..516, y 260..274 (LMB closes).
            if x > 499 && x < 516 && y > 260 && y < 274 {
                if mouse_btn == MouseButton::Left {
                    if let Some(ps_mut) = app_state.player_state.as_mut() {
                        ps_mut.close_shop();
                    }
                }
                return true;
            }

            // Clicking outside the shop window always closes it.
            if !in_shop_window {
                if let Some(ps_mut) = app_state.player_state.as_mut() {
                    ps_mut.close_shop();
                }
                return true;
            }

            // Grid: x 220..500, y 261..552; send CmdShop(shop_nr, nr) / CmdShop(shop_nr, nr+62).
            if x > 220 && x < 500 && y > 261 && y < 485 + 32 + 35 {
                let tx = ((x - 220) / 35) as usize;
                let ty = ((y - 261) / 35) as usize;
                let nr = tx + ty * 8;

                if nr < 62 {
                    if let Some(net) = app_state.network.as_ref() {
                        let shop_nr = ps.shop_target().nr() as i16;
                        match mouse_btn {
                            MouseButton::Left => {
                                net.send(ClientCommand::new_shop(shop_nr, nr as i32));
                            }
                            MouseButton::Right => {
                                net.send(ClientCommand::new_shop(shop_nr, (nr + 62) as i32));
                            }
                            _ => {}
                        }
                    }
                }
                return true;
            }
        }

        false
    }
}
