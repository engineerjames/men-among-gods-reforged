// Statbox (raise stats/skills) systems live here.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;

use crate::network::client_commands::ClientCommand;
use crate::network::NetworkRuntime;
use crate::player_state::PlayerState;
use crate::settings::UserSettingsState;
use crate::states::gameplay::components::*;
use crate::states::gameplay::layout::*;
use crate::states::gameplay::resources::*;
use crate::systems::magic_postprocess::MagicScreenCamera;

use mag_core::types::skilltab::{get_skill_desc, get_skill_name, get_skill_nr};

use super::super::world_render::screen_to_world;

use super::super::{
    attrib_needed, build_sorted_skills, cursor_game_pos, end_needed, hp_needed, mana_needed,
    skill_needed, ui_bar_colors, xbuttons_slot_at, xbuttons_truncate_label, ATTRIBUTE_NAMES,
    HIGH_VAL,
};

/// Spawns the skill/inventory scroll knobs used by the gameplay UI.
pub(crate) fn spawn_ui_scroll_knobs(commands: &mut Commands, image_assets: &mut Assets<Image>) {
    // A single white pixel stretched + tinted for dd_showbar-like rectangles.
    let pixel = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let pixel_handle = image_assets.add(pixel);

    let (_blue, green, _red) = ui_bar_colors();

    let spawn_knob = |commands: &mut Commands, kind: GameplayUiScrollKnobKind, sx: f32, sy: f32| {
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiScrollKnob { kind },
            Sprite {
                image: pixel_handle.clone(),
                color: green,
                custom_size: Some(Vec2::new(SCROLL_KNOB_W, SCROLL_KNOB_H)),
                ..default()
            },
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_SCROLL)),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    };

    // Initial positions match engine.c's dd_showbar formulas at pos=0.
    spawn_knob(
        commands,
        GameplayUiScrollKnobKind::Skill,
        SKILL_SCROLL_X,
        SKILL_SCROLL_Y_BASE,
    );
    spawn_knob(
        commands,
        GameplayUiScrollKnobKind::Inventory,
        INV_SCROLL_X,
        INV_SCROLL_Y_BASE,
    );
}

/// Updates scrollbar knob positions for the skill list and inventory list.
pub(crate) fn run_gameplay_update_scroll_knobs(
    statbox: Res<GameplayStatboxState>,
    inv_scroll: Res<GameplayInventoryScrollState>,
    mut q: Query<(&GameplayUiScrollKnob, &mut Transform)>,
) {
    if !statbox.is_changed() && !inv_scroll.is_changed() {
        return;
    }

    let skill_pos = statbox.skill_pos as i32;
    let inv_pos = inv_scroll.inv_pos as i32;

    // Match original integer math: y = base + (pos * range) / max.
    let skill_y =
        SKILL_SCROLL_Y_BASE + ((skill_pos * SKILL_SCROLL_RANGE) / SKILL_SCROLL_MAX) as f32;
    let inv_y = INV_SCROLL_Y_BASE + ((inv_pos * INV_SCROLL_RANGE) / INV_SCROLL_MAX) as f32;

    for (knob, mut t) in &mut q {
        let (x, y) = match knob.kind {
            GameplayUiScrollKnobKind::Skill => (SKILL_SCROLL_X, skill_y),
            GameplayUiScrollKnobKind::Inventory => (INV_SCROLL_X, inv_y),
        };
        t.translation = screen_to_world(x, y, Z_UI_SCROLL);
    }
}

/// Handles statbox input: raising stats/skills and managing skill hotbar assignments.
pub(crate) fn run_gameplay_statbox_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<&Camera, (With<Camera2d>, With<MagicScreenCamera>)>,
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    mut user_settings: ResMut<UserSettingsState>,
    mut statbox: ResMut<GameplayStatboxState>,
    mut inv_scroll: ResMut<GameplayInventoryScrollState>,
    mut xbuttons: ResMut<GameplayXButtonsState>,
) {
    let Some(game) = cursor_game_pos(&windows, &cameras) else {
        return;
    };

    // Right-click help texts (orig/inter.c::_mouse_statbox).
    if mouse.just_released(MouseButton::Right) {
        let x = game.x;
        let y = game.y;

        // Skill hotbar (xbuttons) assign/unassign (orig/inter.c via button_help cases 16..27).
        if let Some(slot) = xbuttons_slot_at(x, y) {
            let mut changed_xbuttons = false;

            {
                let slot_data = &mut player_state.player_data_mut().skill_buttons[slot];

                if let Some(skill_id) = xbuttons.pending_skill_id {
                    let skill_nr = get_skill_nr(skill_id) as u32;
                    if !slot_data.is_unassigned() && slot_data.skill_nr() == skill_nr {
                        slot_data.set_unassigned();
                    } else {
                        let label = xbuttons_truncate_label(get_skill_name(skill_id));
                        slot_data.set_skill_nr(skill_nr);
                        slot_data.set_name(&label);
                    }
                    changed_xbuttons = true;
                    player_state.mark_dirty();
                } else {
                    // No pending skill selected: allow clearing the slot.
                    if !slot_data.is_unassigned() {
                        slot_data.set_unassigned();
                        changed_xbuttons = true;
                        player_state.mark_dirty();
                    }
                }
            }

            // Persist updated xbuttons into settings.json so they survive restarts and
            // subsequent character `.mag` saves.
            if changed_xbuttons {
                user_settings.sync_character_from_player_state(&player_state);
                user_settings.request_save();
            }
            return;
        }

        // Inventory scroll right-click help (orig/inter.c::button_help case 12/13).
        if x > 290.0 && y > 1.0 && x < 300.0 && y < 34.0 {
            player_state.tlog(1, "Scroll inventory contents up.");
            return;
        }
        if x > 290.0 && y > 141.0 && x < 300.0 && y < 174.0 {
            player_state.tlog(1, "Scroll inventory contents down");
            return;
        }

        // Skill list right-click (orig/inter.c::mouse_statbox2): show skill description.
        if (2.0..=108.0).contains(&x) && (114.0..=251.0).contains(&y) {
            let row = ((y - 114.0) / 14.0).floor() as usize;
            if row < 10 {
                let pl = player_state.character_info();
                let sorted = build_sorted_skills(pl);
                let skilltab_index = statbox.skill_pos + row;
                if let Some(&skill_id) = sorted.get(skilltab_index) {
                    if pl.skill[skill_id][0] != 0 {
                        xbuttons.pending_skill_id = Some(skill_id);
                        let desc = get_skill_desc(skill_id);
                        if !desc.is_empty() {
                            player_state.tlog(1, desc);
                        }
                    }
                }
            }
            return;
        }

        if x > 109.0 && y > 254.0 && x < 158.0 && y < 266.0 {
            player_state.tlog(1, "Make the changes permanent");
            return;
        }

        if !(133.0..=157.0).contains(&x) || !(2.0..=251.0).contains(&y) {
            return;
        }

        let n = ((y - 2.0) / 14.0).floor() as usize;
        if x < 145.0 {
            if n < 5 {
                player_state.tlog(1, &format!("Raise {}.", ATTRIBUTE_NAMES[n]));
            } else if n == 5 {
                player_state.tlog(1, "Raise Hitpoints.");
            } else if n == 6 {
                player_state.tlog(1, "Raise Endurance.");
            } else if n == 7 {
                player_state.tlog(1, "Raise Mana.");
            } else {
                let pl = player_state.character_info();
                let sorted = build_sorted_skills(pl);
                let skilltab_index = statbox.skill_pos + (n.saturating_sub(8));
                if let Some(&skill_id) = sorted.get(skilltab_index) {
                    let name = get_skill_name(skill_id);
                    if !name.is_empty() {
                        player_state.tlog(1, &format!("Raise {}.", name));
                    }
                }
            }
        } else {
            if n < 5 {
                player_state.tlog(1, &format!("Lower {}.", ATTRIBUTE_NAMES[n]));
            } else if n == 5 {
                player_state.tlog(1, "Lower Hitpoints.");
            } else if n == 6 {
                player_state.tlog(1, "Lower Endurance.");
            } else if n == 7 {
                player_state.tlog(1, "Lower Mana.");
            } else {
                let pl = player_state.character_info();
                let sorted = build_sorted_skills(pl);
                let skilltab_index = statbox.skill_pos + (n.saturating_sub(8));
                if let Some(&skill_id) = sorted.get(skilltab_index) {
                    let name = get_skill_name(skill_id);
                    if !name.is_empty() {
                        player_state.tlog(1, &format!("Lower {}.", name));
                    }
                }
            }
        }
        return;
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    // Skill hotbar (xbuttons) activate (orig/inter.c via button_command cases 16..27).
    if let Some(slot) = xbuttons_slot_at(game.x, game.y) {
        let btn = &player_state.player_data().skill_buttons[slot];
        if btn.is_unassigned() {
            player_state.tlog(1, "No skill assigned to that button.");
        } else {
            let selected_char = player_state.selected_char() as u32;
            let attrib0 = 1u32;
            net.send(ClientCommand::new_skill(btn.skill_nr(), selected_char, attrib0).to_bytes());
        }
        return;
    }

    // Inventory scroll buttons (orig/inter.c::button_command case 12/13 via trans_button).
    if game.x > 290.0 && game.y > 1.0 && game.x < 300.0 && game.y < 34.0 {
        if inv_scroll.inv_pos > 1 {
            inv_scroll.inv_pos = inv_scroll.inv_pos.saturating_sub(2);
        }
        return;
    }
    if game.x > 290.0 && game.y > 141.0 && game.x < 300.0 && game.y < 174.0 {
        if inv_scroll.inv_pos < 30 {
            inv_scroll.inv_pos = (inv_scroll.inv_pos + 2).min(30);
        }
        return;
    }

    // Skill list scroll buttons (orig/inter.c::button_command case 14/15 via trans_button).
    // Up: if (skill_pos>1) skill_pos-=2;  Down: if (skill_pos<40) skill_pos+=2;
    if game.x > 206.0 && game.x < 218.0 && game.y > 113.0 && game.y < 148.0 {
        if statbox.skill_pos > 1 {
            statbox.skill_pos = statbox.skill_pos.saturating_sub(2);
        }
        return;
    }
    if game.x > 206.0 && game.x < 218.0 && game.y > 218.0 && game.y < 252.0 {
        if statbox.skill_pos < 40 {
            statbox.skill_pos = (statbox.skill_pos + 2).min(40);
        }
        return;
    }

    // Skill click (orig/inter.c::mouse_statbox2): clicking a skill row sends CL_CMD_SKILL.
    // The original client always sends attrib0=skilltab[..].attrib[0], which is initialized to 1
    // for all skills (and can be modified for spells via commented-out UI).
    if (2.0..=108.0).contains(&game.x) && (114.0..=251.0).contains(&game.y) {
        let row = ((game.y - 114.0) / 14.0).floor() as usize;
        if row < 10 {
            let pl = player_state.character_info();
            let sorted = build_sorted_skills(pl);
            let skilltab_index = statbox.skill_pos + row;
            if let Some(&skill_id) = sorted.get(skilltab_index) {
                let skill_nr = get_skill_nr(skill_id) as u32;
                let selected_char = player_state.selected_char() as u32;
                let attrib0 = 1u32;
                net.send(ClientCommand::new_skill(skill_nr, selected_char, attrib0).to_bytes());
            }
        }
        return;
    }

    // orig/inter.c::mouse_statbox: Shift=10 repeats, Ctrl=90 repeats.
    let repeat = if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        90
    } else if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        10
    } else {
        1
    };

    for _ in 0..repeat {
        let x = game.x;
        let y = game.y;

        // Commit button.
        if x > 109.0 && y > 254.0 && x < 158.0 && y < 266.0 {
            let pl = player_state.character_info();
            let sorted = build_sorted_skills(pl);
            for n in 0..108 {
                let v = statbox.stat_raised[n];
                if v == 0 {
                    continue;
                }
                let which = if n > 7 {
                    let skilltab_index = n - 8;
                    let Some(&skill_id) = sorted.get(skilltab_index) else {
                        continue;
                    };
                    (get_skill_nr(skill_id) + 8) as i16
                } else {
                    n as i16
                };
                net.send(ClientCommand::new_stat(which, v).to_bytes());
            }
            statbox.clear();
            return;
        }

        if !(133.0..=157.0).contains(&x) || !(2.0..=251.0).contains(&y) {
            return;
        }

        let n = ((y - 2.0) / 14.0).floor() as usize;
        let raising = x < 145.0;

        let pl = player_state.character_info();
        let available = statbox.available_points(pl);

        if raising {
            if n < 5 {
                let idx = n;
                let need = attrib_needed(pl, n, pl.attrib[n][0] as i32 + statbox.stat_raised[idx]);
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[idx] += 1;
                }
            } else if n == 5 {
                let idx = 5;
                let need = hp_needed(pl, pl.hp[0] as i32 + statbox.stat_raised[idx]);
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[idx] += 1;
                }
            } else if n == 6 {
                let idx = 6;
                let need = end_needed(pl, pl.end[0] as i32 + statbox.stat_raised[idx]);
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[idx] += 1;
                }
            } else if n == 7 {
                let idx = 7;
                let need = mana_needed(pl, pl.mana[0] as i32 + statbox.stat_raised[idx]);
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[idx] += 1;
                }
            } else {
                let skill_row = n.saturating_sub(8);
                let skilltab_index = statbox.skill_pos + skill_row;
                let raised_idx = 8 + skilltab_index;
                if raised_idx >= statbox.stat_raised.len() {
                    continue;
                }
                let sorted = build_sorted_skills(pl);
                let Some(&skill_id) = sorted.get(skilltab_index) else {
                    continue;
                };
                if pl.skill[skill_id][0] == 0 {
                    continue;
                }
                let need = skill_needed(
                    pl,
                    skill_id,
                    pl.skill[skill_id][0] as i32 + statbox.stat_raised[raised_idx],
                );
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[raised_idx] += 1;
                }
            }
        } else {
            if n < 5 {
                let idx = n;
                if statbox.stat_raised[idx] > 0 {
                    statbox.stat_raised[idx] -= 1;
                    let refund =
                        attrib_needed(pl, n, pl.attrib[n][0] as i32 + statbox.stat_raised[idx]);
                    if refund != HIGH_VAL {
                        statbox.stat_points_used -= refund;
                    }
                }
            } else if n == 5 {
                let idx = 5;
                if statbox.stat_raised[idx] > 0 {
                    statbox.stat_raised[idx] -= 1;
                    let refund = hp_needed(pl, pl.hp[0] as i32 + statbox.stat_raised[idx]);
                    if refund != HIGH_VAL {
                        statbox.stat_points_used -= refund;
                    }
                }
            } else if n == 6 {
                let idx = 6;
                if statbox.stat_raised[idx] > 0 {
                    statbox.stat_raised[idx] -= 1;
                    let refund = end_needed(pl, pl.end[0] as i32 + statbox.stat_raised[idx]);
                    if refund != HIGH_VAL {
                        statbox.stat_points_used -= refund;
                    }
                }
            } else if n == 7 {
                let idx = 7;
                if statbox.stat_raised[idx] > 0 {
                    statbox.stat_raised[idx] -= 1;
                    let refund = mana_needed(pl, pl.mana[0] as i32 + statbox.stat_raised[idx]);
                    if refund != HIGH_VAL {
                        statbox.stat_points_used -= refund;
                    }
                }
            } else {
                let skill_row = n.saturating_sub(8);
                let skilltab_index = statbox.skill_pos + skill_row;
                let raised_idx = 8 + skilltab_index;
                if raised_idx >= statbox.stat_raised.len() {
                    continue;
                }
                if statbox.stat_raised[raised_idx] <= 0 {
                    continue;
                }
                let sorted = build_sorted_skills(pl);
                let Some(&skill_id) = sorted.get(skilltab_index) else {
                    continue;
                };
                statbox.stat_raised[raised_idx] -= 1;
                let refund = skill_needed(
                    pl,
                    skill_id,
                    pl.skill[skill_id][0] as i32 + statbox.stat_raised[raised_idx],
                );
                if refund != HIGH_VAL {
                    statbox.stat_points_used -= refund;
                }
            }
        }

        if statbox.stat_points_used < 0 {
            statbox.stat_points_used = 0;
        }
    }
}
