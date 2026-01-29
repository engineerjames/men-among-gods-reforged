use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use std::collections::HashMap;
use std::fmt::Write;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::network::{client_commands::ClientCommand, NetworkRuntime};
use crate::player_state::PlayerState;
use crate::states::gameplay::{BitmapText, GameplayRenderEntity};
use crate::systems::magic_postprocess::UI_LAYER;

use mag_core::constants::{TILEX, TILEY, XPOS, YPOS};

// Keep these in-sync with the draw ordering in `states/gameplay.rs`.
const Z_WORLD_STEP: f32 = 0.01;
const Z_CHAR_BASE: f32 = 100.2;
const Z_NAMEPLATE_BIAS: f32 = 0.02;
const Z_NAMEPLATE_SHADOW_BIAS: f32 = 0.019;
const NAMEPLATE_SHADOW_OFFSET_X: i32 = 1;
const NAMEPLATE_SHADOW_OFFSET_Y: i32 = 1;

// dd_gputtext uses YPOS-64.
const NAMEPLATE_Y_SHIFT: i32 = 64;

#[derive(Component)]
pub(crate) struct GameplayNameplate {
    pub index: usize,
    pub is_shadow: bool,
}

#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NameplateLabelKey {
    ch_nr: u16,
    ch_id: u16,
    ch_proz: u8,
    show_names: bool,
    show_proz: bool,
    has_name: bool,
}

#[inline]
/// Convert screen-space pixels into world-space coordinates.
fn screen_to_world(sx: f32, sy: f32, z: f32) -> Vec3 {
    Vec3::new(sx - TARGET_WIDTH * 0.5, TARGET_HEIGHT * 0.5 - sy, z)
}

/// Compute dd_gputtext-style screen-space placement for nameplates.
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

/// Decode a NUL-terminated byte string, trimming whitespace.
fn bytes_to_trimmed_str(bytes: &[u8]) -> Option<&str> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let s = std::str::from_utf8(&bytes[..end]).ok()?.trim();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Resolve the preferred player display name from runtime or saved data.
fn player_display_name_str<'a>(player_state: &'a PlayerState) -> &'a str {
    // Prefer the runtime player name, but fall back to persisted pdata.cname.
    if let Some(s) = bytes_to_trimmed_str(&player_state.character_info().name) {
        return s;
    }
    if let Some(s) = bytes_to_trimmed_str(&player_state.player_data().cname) {
        return s;
    }
    "Player"
}

/// Spawn hidden nameplate entities for all map tiles.
pub(crate) fn spawn_gameplay_nameplates(commands: &mut Commands, world_root: Entity) {
    for index in 0..(TILEX * TILEY) {
        for is_shadow in [true, false] {
            let color = if is_shadow {
                Color::BLACK
            } else {
                Color::WHITE
            };

            let id = commands
                .spawn((
                    GameplayRenderEntity,
                    GameplayNameplate { index, is_shadow },
                    NameplateLabelKey::default(),
                    // Draw nameplates as an overlay on the on-screen camera to avoid postprocess
                    // distortion/jitter from render-to-texture.
                    RenderLayers::layer(UI_LAYER),
                    BitmapText {
                        text: String::new(),
                        color,
                        // Yellow is 701 => index 1.
                        font: 1,
                    },
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
}

/// Update nameplate text, positioning, and visibility each frame.
pub(crate) fn run_gameplay_nameplates(
    net: Res<NetworkRuntime>,
    player_state: Res<PlayerState>,
    mut q: Query<(
        &GameplayNameplate,
        &mut BitmapText,
        &mut Transform,
        &mut Visibility,
        &mut NameplateLabelKey,
    )>,
    mut last_sent_ticker: Local<u32>,
    mut name_cache: Local<HashMap<u16, String>>,
) {
    let pdata = player_state.player_data();
    let show_names = pdata.show_names != 0;
    let show_proz = pdata.show_proz != 0;

    let mut first_unknown: Option<u16> = None;

    let player_name = player_display_name_str(&player_state);

    for (plate, mut text2d, mut transform, mut visibility, mut last_key) in &mut q {
        let Some(tile) = player_state.map().tile_at_index(plate.index) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        if tile.ch_nr == 0 || (!show_names && !show_proz) {
            text2d.text.clear();
            *visibility = Visibility::Hidden;
            continue;
        }

        let is_center = {
            let x = plate.index % TILEX;
            let y = plate.index / TILEX;
            x == TILEX / 2 && y == TILEY / 2
        };

        // Lifetime-of-app cache: map character integer (nr) -> resolved name.
        // Names are stable for a given character id, so once inserted they never change.
        let (name_str, has_name) = if show_names {
            if is_center {
                (Some(player_name), true)
            } else if let Some(cached) = name_cache.get(&tile.ch_nr) {
                (Some(cached.as_str()), true)
            } else if let Some(resolved) = player_state.lookup_name(tile.ch_nr, tile.ch_id) {
                name_cache.insert(tile.ch_nr, resolved.to_string());
                // Pull back out to borrow from the cache.
                let s = name_cache
                    .get(&tile.ch_nr)
                    .map(|v| v.as_str())
                    .unwrap_or(resolved);
                (Some(s), true)
            } else {
                first_unknown.get_or_insert(tile.ch_nr);
                (None, false)
            }
        } else {
            (None, false)
        };

        let proz = if show_proz && tile.ch_proz != 0 {
            Some(tile.ch_proz)
        } else {
            None
        };

        let desired_key = NameplateLabelKey {
            ch_nr: tile.ch_nr,
            ch_id: tile.ch_id,
            ch_proz: tile.ch_proz,
            show_names,
            show_proz,
            has_name,
        };

        // If the label inputs didn't change, don't touch the BitmapText.
        // Avoids triggering the bitmap text renderer every frame.
        if *last_key != desired_key {
            text2d.text.clear();

            match (show_names, show_proz, name_str, proz) {
                (true, true, Some(n), Some(p)) if !n.is_empty() => {
                    text2d.text.push_str(n);
                    text2d.text.push(' ');
                    let _ = write!(&mut text2d.text, "{p}%");
                }
                (true, true, _, Some(p)) => {
                    let _ = write!(&mut text2d.text, "{p}%");
                }
                (true, true, Some(n), None) => {
                    text2d.text.push_str(n);
                }
                (true, false, Some(n), _) => {
                    text2d.text.push_str(n);
                }
                (false, true, _, Some(p)) => {
                    let _ = write!(&mut text2d.text, "{p}%");
                }
                _ => {}
            }

            *last_key = desired_key;
        }

        if text2d.text.is_empty() {
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
            dd_gputtext_screen_pos(xpos, ypos, text2d.text.len(), tile.obj_xoff, tile.obj_yoff);

        let draw_order = ((TILEY - 1 - (view_y as usize)) * TILEX + (view_x as usize)) as f32;
        let z_bias = if plate.is_shadow {
            Z_NAMEPLATE_SHADOW_BIAS
        } else {
            Z_NAMEPLATE_BIAS
        };
        let z = Z_CHAR_BASE + draw_order * Z_WORLD_STEP + z_bias;

        text2d.font = 1;
        if plate.is_shadow {
            text2d.color = Color::BLACK;
            transform.translation = screen_to_world(
                (sx_i + NAMEPLATE_SHADOW_OFFSET_X) as f32,
                (sy_i + NAMEPLATE_SHADOW_OFFSET_Y) as f32,
                z,
            );
        } else {
            text2d.color = Color::WHITE;
            transform.translation = screen_to_world(sx_i as f32, sy_i as f32, z);
        }
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
    /// Validate trimming logic for NUL-terminated name data.
    fn bytes_to_trimmed_str_handles_null_and_whitespace() {
        assert_eq!(bytes_to_trimmed_str(b"Bob\0"), Some("Bob"));
        assert_eq!(bytes_to_trimmed_str(b"  Bob  \0"), Some("Bob"));
        assert_eq!(bytes_to_trimmed_str(b"\0"), None);
        assert_eq!(bytes_to_trimmed_str(b"   \0"), None);
    }

    #[test]
    /// Ensure screen-space nameplate math matches original formula.
    fn dd_gputtext_screen_pos_matches_expected_formula() {
        // xpos=0,ypos=0, text_len=10, xoff=yoff=0
        let (rx, ry) = dd_gputtext_screen_pos(0, 0, 10, 0, 0);
        let expected_rx = 32 - (((10i32) * 5) / 2) + mag_core::constants::XPOS;
        let expected_ry = mag_core::constants::YPOS - NAMEPLATE_Y_SHIFT;
        assert_eq!(rx, expected_rx);
        assert_eq!(ry, expected_ry);
    }
}
