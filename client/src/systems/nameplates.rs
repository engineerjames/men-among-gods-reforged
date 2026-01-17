use bevy::prelude::*;
use bevy::sprite::{Anchor, Text2d};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::map::{TILEX, TILEY};
use crate::network::{client_commands::ClientCommand, NetworkRuntime};
use crate::player_state::PlayerState;
use crate::states::gameplay::GameplayRenderEntity;

use mag_core::constants::{XPOS, YPOS};

// Keep these in-sync with the draw ordering in `states/gameplay.rs`.
const Z_WORLD_STEP: f32 = 0.01;
const Z_CHAR_BASE: f32 = 100.2;
const Z_NAMEPLATE_BIAS: f32 = 0.02;

// dd_gputtext uses YPOS-64.
const NAMEPLATE_Y_SHIFT: i32 = 64;

#[derive(Component)]
pub(crate) struct GameplayNameplate {
    pub index: usize,
}

#[inline]
fn screen_to_world(sx: f32, sy: f32, z: f32) -> Vec3 {
    Vec3::new(sx - TARGET_WIDTH * 0.5, TARGET_HEIGHT * 0.5 - sy, z)
}

fn dd_gputtext_screen_pos(
    xpos: i32,
    ypos: i32,
    text_len: usize,
    xoff: i32,
    yoff: i32,
) -> (i32, i32) {
    // Ported from `orig/dd.c::dd_gputtext`.
    // We don't need the negative-coordinate odd-bit adjustments in our usage.
    let rx = (xpos / 2) + (ypos / 2) + 32 - (((text_len as i32) * 5) / 2) + XPOS + xoff;
    let ry = (xpos / 4) - (ypos / 4) + YPOS - NAMEPLATE_Y_SHIFT + yoff;
    (rx, ry)
}

fn bytes_to_trimmed_str(bytes: &[u8]) -> Option<&str> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let s = std::str::from_utf8(&bytes[..end]).ok()?.trim();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn player_display_name(player_state: &PlayerState) -> String {
    // Prefer the runtime player name, but fall back to persisted pdata.cname.
    if let Some(s) = bytes_to_trimmed_str(&player_state.character_info().name) {
        return s.to_string();
    }
    if let Some(s) = bytes_to_trimmed_str(&player_state.player_data().cname) {
        return s.to_string();
    }
    "Player".to_string()
}

pub(crate) fn spawn_gameplay_nameplates(
    commands: &mut Commands,
    world_root: Entity,
    font: Handle<Font>,
) {
    for index in 0..(TILEX * TILEY) {
        let id = commands
            .spawn((
                GameplayRenderEntity,
                GameplayNameplate { index },
                Text2d::new(""),
                TextFont {
                    font: font.clone(),
                    // Slightly smaller than UI text for readability.
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Anchor::TOP_LEFT,
                Transform::default(),
                GlobalTransform::default(),
                Visibility::Hidden,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ))
            .id();

        commands.entity(world_root).add_child(id);
    }
}

pub(crate) fn run_gameplay_nameplates(
    net: Res<NetworkRuntime>,
    player_state: Res<PlayerState>,
    mut q: Query<(
        &GameplayNameplate,
        &mut Text2d,
        &mut Transform,
        &mut Visibility,
    )>,
    mut last_sent_ticker: Local<u32>,
) {
    let pdata = player_state.player_data();
    let show_names = pdata.show_names != 0;
    let show_proz = pdata.show_proz != 0;

    let mut first_unknown: Option<u16> = None;

    for (plate, mut text2d, mut transform, mut visibility) in &mut q {
        let Some(tile) = player_state.map().tile_at_index(plate.index) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        if tile.ch_nr == 0 || (!show_names && !show_proz) {
            **text2d = String::new();
            *visibility = Visibility::Hidden;
            continue;
        }

        let is_center = {
            let x = plate.index % TILEX;
            let y = plate.index / TILEX;
            x == TILEX / 2 && y == TILEY / 2
        };

        let name = if show_names {
            if is_center {
                Some(player_display_name(&player_state))
            } else {
                let cached = player_state.lookup_name(tile.ch_nr, tile.ch_id);
                if cached.is_none() {
                    first_unknown.get_or_insert(tile.ch_nr);
                }
                cached.map(|s| s.to_string())
            }
        } else {
            None
        };

        let proz = if show_proz && tile.ch_proz != 0 {
            Some(tile.ch_proz)
        } else {
            None
        };

        let text = match (show_names, show_proz, name.as_deref(), proz) {
            (true, true, Some(n), Some(p)) if !n.is_empty() => format!("{n} {p}%"),
            (true, true, _, Some(p)) => format!("{p}%"),
            (true, true, Some(n), None) => n.to_string(),
            (true, false, Some(n), _) => n.to_string(),
            (false, true, _, Some(p)) => format!("{p}%"),
            _ => String::new(),
        };

        if text.is_empty() {
            **text2d = String::new();
            *visibility = Visibility::Hidden;
            continue;
        }

        // Ported from engine.c: dd_gputtext(x*32, y*32, ..., xoff+obj_xoff, yoff+obj_yoff)
        // We omit global xoff/yoff because the gameplay world root already applies it.
        let view_x = (plate.index % TILEX) as i32;
        let view_y = (plate.index / TILEX) as i32;
        let xpos = view_x * 32;
        let ypos = view_y * 32;

        let (sx_i, sy_i) =
            dd_gputtext_screen_pos(xpos, ypos, text.len(), tile.obj_xoff, tile.obj_yoff);

        let draw_order = ((TILEY - 1 - (view_y as usize)) * TILEX + (view_x as usize)) as f32;
        let z = Z_CHAR_BASE + draw_order * Z_WORLD_STEP + Z_NAMEPLATE_BIAS;

        **text2d = text;
        transform.translation = screen_to_world(sx_i as f32, sy_i as f32, z);
        *visibility = Visibility::Visible;
    }

    // Mirror engine.c's "autolook" behavior: request missing names slowly.
    if show_names {
        if let Some(target) = first_unknown {
            let ticker = net.client_ticker();
            // Throttle to avoid spamming; engine.c gates this with lookstep>QSIZE*3.
            if ticker.saturating_sub(*last_sent_ticker) > 15 {
                let cmd = ClientCommand::new_autolook(target as u32);
                net.send(cmd.to_bytes());
                *last_sent_ticker = ticker;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_trimmed_str_handles_null_and_whitespace() {
        assert_eq!(bytes_to_trimmed_str(b"Bob\0"), Some("Bob"));
        assert_eq!(bytes_to_trimmed_str(b"  Bob  \0"), Some("Bob"));
        assert_eq!(bytes_to_trimmed_str(b"\0"), None);
        assert_eq!(bytes_to_trimmed_str(b"   \0"), None);
    }

    #[test]
    fn dd_gputtext_screen_pos_matches_expected_formula() {
        // xpos=0,ypos=0, text_len=10, xoff=yoff=0
        let (rx, ry) = dd_gputtext_screen_pos(0, 0, 10, 0, 0);
        let expected_rx = 32 - (((10i32) * 5) / 2) + mag_core::constants::XPOS;
        let expected_ry = mag_core::constants::YPOS - NAMEPLATE_Y_SHIFT;
        assert_eq!(rx, expected_rx);
        assert_eq!(ry, expected_ry);
    }
}
