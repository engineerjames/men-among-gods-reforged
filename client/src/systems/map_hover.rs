use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::map::{TILEX, TILEY};
use crate::network::{client_commands::ClientCommand, NetworkRuntime};
use crate::player_state::PlayerState;
use crate::states::gameplay::{
    dd_effect_tint, GameplayCursorType, GameplayCursorTypeState, GameplayRenderEntity, TileLayer,
    TileRender,
};

use mag_core::constants::{
    DR_DROP, DR_GIVE, DR_PICKUP, DR_USE, ISCHAR, ISITEM, ISUSABLE, XPOS, YPOS,
};

// Keep these in-sync with the draw ordering in `states/gameplay.rs`.
const Z_WORLD_STEP: f32 = 0.01;
const Z_BG_BASE: f32 = 0.0;
const Z_OBJ_BASE: f32 = 100.0;
const Z_CHAR_BASE: f32 = 100.2;
const Z_FX_BASE: f32 = 100.25;

const Z_GOTO_BIAS: f32 = 0.001;

// orig/engine.c draws sprite 31 at pl.goto_x/pl.goto_y.
const MOVE_TARGET_SPRITE_ID: usize = 31;
// orig/engine.c draws sprite 34 at the attack target character.
const ATTACK_TARGET_SPRITE_ID: usize = 34;
// orig/engine.c draws these based on pl.misc_action.
const DROP_MARKER_SPRITE_ID: usize = 32;
const PICKUP_MARKER_SPRITE_ID: usize = 33;
const USE_MARKER_SPRITE_ID: usize = 45;

#[derive(Component)]
pub(crate) struct GameplayMoveTargetMarker;

#[derive(Component)]
pub(crate) struct GameplayMiscActionMarker;

#[derive(Component)]
pub(crate) struct GameplayAttackTargetMarker;

#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct GameplayHoveredTile {
    pub tile_x: i32,
    pub tile_y: i32,
}

impl GameplayHoveredTile {
    fn clear(&mut self) {
        self.tile_x = -1;
        self.tile_y = -1;
    }

    fn set(&mut self, x: i32, y: i32) {
        self.tile_x = x;
        self.tile_y = y;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum GameplayHoverTargetKind {
    #[default]
    None,
    Background,
    Object,
    Character,
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct GameplayHoverTarget {
    pub tile_x: i32,
    pub tile_y: i32,
    pub kind: GameplayHoverTargetKind,
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct GameplaySpriteHighlightState {
    pub last_tile_x: i32,
    pub last_tile_y: i32,
    pub last_layer: GameplayHoverTargetKind,
}

#[inline]
fn screen_to_world(sx: f32, sy: f32, z: f32) -> Vec3 {
    Vec3::new(sx - TARGET_WIDTH * 0.5, TARGET_HEIGHT * 0.5 - sy, z)
}

/// Convert cursor position into the game's fixed 800x600 "logical" coordinates.
///
/// Mirrors `systems/debug.rs::print_click_coords`.
fn cursor_game_pos(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<&Camera, With<Camera2d>>,
) -> Option<Vec2> {
    let window = windows.single().ok()?;
    let cursor_logical = window.cursor_position()?;

    let scale_factor = window.resolution.scale_factor();
    let cursor_physical = cursor_logical * scale_factor;

    let camera = cameras.single().ok()?;

    let (vp_pos, vp_size) = if let Some(viewport) = camera.viewport.as_ref() {
        (
            Vec2::new(
                viewport.physical_position.x as f32,
                viewport.physical_position.y as f32,
            ),
            Vec2::new(
                viewport.physical_size.x as f32,
                viewport.physical_size.y as f32,
            ),
        )
    } else {
        (
            Vec2::ZERO,
            Vec2::new(
                window.resolution.physical_width() as f32,
                window.resolution.physical_height() as f32,
            ),
        )
    };

    if vp_size.x <= 0.0 || vp_size.y <= 0.0 {
        return None;
    }

    let vp_max = vp_pos + vp_size;
    if cursor_physical.x < vp_pos.x
        || cursor_physical.x >= vp_max.x
        || cursor_physical.y < vp_pos.y
        || cursor_physical.y >= vp_max.y
    {
        return None;
    }

    let in_viewport = cursor_physical - vp_pos;
    Some(Vec2::new(
        in_viewport.x / vp_size.x * TARGET_WIDTH,
        in_viewport.y / vp_size.y * TARGET_HEIGHT,
    ))
}

/// Port of `orig/inter.c::mouse_mapbox`'s cursor -> (mx,my) tile math.
///
/// Returns the view-tile coordinates (0..TILEX-1, 0..TILEY-1).
fn hovered_view_tile(game: Vec2) -> Option<(i32, i32)> {
    let mut x = game.x as i32;
    let mut y = game.y as i32;

    // `mouse_mapbox`: x+=176-16; y+=8;
    x += 176 - 16;
    y += 8;

    // These are pixel-space values in the isometric map coordinate system.
    let mx_pix = 2 * y + x - (YPOS * 2) - XPOS + (((TILEX as i32 - 34) / 2) * 32);
    let my_pix = x - 2 * y + (YPOS * 2) - XPOS + (((TILEX as i32 - 34) / 2) * 32);

    // Mapbox active region check (matches C).
    if mx_pix < 3 * 32 + 12
        || mx_pix > (TILEX as i32 - 7) * 32 + 20
        || my_pix < 7 * 32 + 12
        || my_pix > (TILEY as i32 - 3) * 32 + 20
    {
        return None;
    }

    let mx = mx_pix / 32;
    let my = my_pix / 32;

    if mx < 3 || mx > TILEX as i32 - 7 {
        return None;
    }
    if my < 7 || my > TILEY as i32 - 3 {
        return None;
    }

    Some((mx, my))
}

fn find_view_tile_for_world(map: &crate::map::GameMap, wx: i32, wy: i32) -> Option<(i32, i32)> {
    if wx < 0 || wy < 0 {
        return None;
    }
    let wx_u = wx as u16;
    let wy_u = wy as u16;

    for my in 0..(TILEY as i32) {
        for mx in 0..(TILEX as i32) {
            let Some(tile) = map.tile_at_xy(mx as usize, my as usize) else {
                continue;
            };
            if tile.x == wx_u && tile.y == wy_u {
                return Some((mx, my));
            }
        }
    }
    None
}

fn find_view_tile_for_char_id(map: &crate::map::GameMap, ch_id: i32) -> Option<(i32, i32)> {
    if ch_id <= 0 {
        return None;
    }
    let id_u = ch_id as u16;

    for my in 0..(TILEY as i32) {
        for mx in 0..(TILEX as i32) {
            let Some(tile) = map.tile_at_xy(mx as usize, my as usize) else {
                continue;
            };
            if tile.ch_id == id_u {
                return Some((mx, my));
            }
        }
    }
    None
}

fn find_view_tile_for_char_nr(map: &crate::map::GameMap, ch_nr: i32) -> Option<(i32, i32)> {
    if ch_nr <= 0 {
        return None;
    }
    let nr_u = ch_nr as u16;

    for my in 0..(TILEY as i32) {
        for mx in 0..(TILEX as i32) {
            let Some(tile) = map.tile_at_xy(mx as usize, my as usize) else {
                continue;
            };
            if tile.ch_nr == nr_u {
                return Some((mx, my));
            }
        }
    }
    None
}

#[inline]
fn has_flag(map: &crate::map::GameMap, mx: i32, my: i32, flag: u32) -> bool {
    if mx < 0 || my < 0 {
        return false;
    }
    let Some(tile) = map.tile_at_xy(mx as usize, my as usize) else {
        return false;
    };
    (tile.flags & flag) != 0
}

/// Mirrors the original `mouse_mapbox` neighbor scan order.
///
/// Returns `(mx,my,found)` where `mx,my` may be adjusted.
fn snap_to_nearby_flag(map: &crate::map::GameMap, mx: i32, my: i32, flag: u32) -> (i32, i32, bool) {
    const OFFSETS: &[(i32, i32)] = &[
        (0, 0),
        (1, -1),
        (2, -2),
        (1, 0),
        (0, 1),
        (-1, 0),
        (0, -1),
        (1, 1),
        (-1, 1),
        (-1, -1),
        (2, 0),
        (0, 2),
        (-2, 0),
        (0, -2),
        (1, 2),
        (-1, 2),
        (1, -2),
        (-1, -2),
        (2, 1),
        (-2, 1),
        (2, -1),
        (-2, -1),
        (2, 2),
        (-2, 2),
        (-2, -2),
    ];

    for (dx, dy) in OFFSETS {
        let nx = mx + dx;
        let ny = my + dy;
        if has_flag(map, nx, ny, flag) {
            return (nx, ny, true);
        }
    }

    (mx, my, false)
}

/// dd.c `copysprite` positioning for arbitrary sprite tile sizes.
fn copysprite_screen_pos(
    sprite_id: usize,
    gfx: &GraphicsCache,
    xpos: i32,
    ypos: i32,
    xoff: i32,
    yoff: i32,
) -> Option<(i32, i32)> {
    let (xs, ys) = gfx.get_sprite_tiles_xy(sprite_id)?;

    let mut rx = (xpos / 2) + (ypos / 2) - (xs * 16) + 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32);
    let mut ry = (xpos / 4) - (ypos / 4) + YPOS - (ys * 32);

    rx += xoff;
    ry += yoff;
    Some((rx, ry))
}

/// Insert gameplay hover/highlight resources.
///
/// (The name is kept for wiring compatibility from setup_gameplay.)
pub(crate) fn spawn_map_hover_highlight(
    commands: &mut Commands,
    _gfx: &GraphicsCache,
    _world_root: Entity,
) {
    commands.insert_resource(GameplayHoveredTile::default());
    commands.insert_resource(GameplayHoverTarget::default());
    commands.insert_resource(GameplaySpriteHighlightState::default());
}

pub(crate) fn spawn_map_move_target_marker(
    commands: &mut Commands,
    gfx: &GraphicsCache,
    world_root: Entity,
) {
    let Some(src) = gfx.get_sprite(MOVE_TARGET_SPRITE_ID) else {
        log::warn!(
            "Missing sprite {} in GraphicsCache; move target marker disabled",
            MOVE_TARGET_SPRITE_ID
        );
        return;
    };

    let id = commands
        .spawn((
            GameplayRenderEntity,
            GameplayMoveTargetMarker,
            src.clone(),
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

pub(crate) fn spawn_map_attack_target_marker(
    commands: &mut Commands,
    gfx: &GraphicsCache,
    world_root: Entity,
) {
    let Some(src) = gfx.get_sprite(ATTACK_TARGET_SPRITE_ID) else {
        log::warn!(
            "Missing sprite {} in GraphicsCache; attack target marker disabled",
            ATTACK_TARGET_SPRITE_ID
        );
        return;
    };

    let id = commands
        .spawn((
            GameplayRenderEntity,
            GameplayAttackTargetMarker,
            src.clone(),
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

pub(crate) fn run_gameplay_move_target_marker(
    gfx: Res<GraphicsCache>,
    player_state: Res<PlayerState>,
    mut q_marker: Query<(&mut Transform, &mut Visibility), With<GameplayMoveTargetMarker>>,
) {
    let Some((mut transform, mut visibility)) = q_marker.iter_mut().next() else {
        return;
    };

    if !gfx.is_initialized() {
        *visibility = Visibility::Hidden;
        return;
    }

    let pl = player_state.character_info();
    let wx = pl.goto_x;
    let wy = pl.goto_y;

    let Some((mx, my)) = find_view_tile_for_world(player_state.map(), wx, wy) else {
        *visibility = Visibility::Hidden;
        return;
    };

    let xpos = mx * 32;
    let ypos = my * 32;
    let Some((sx_i, sy_i)) = copysprite_screen_pos(MOVE_TARGET_SPRITE_ID, &gfx, xpos, ypos, 0, 0)
    else {
        *visibility = Visibility::Hidden;
        return;
    };

    let draw_order = ((TILEY - 1 - my as usize) * TILEX + (mx as usize)) as f32;
    let z = Z_BG_BASE + draw_order * Z_WORLD_STEP + Z_GOTO_BIAS;
    transform.translation = screen_to_world(sx_i as f32, sy_i as f32, z);
    *visibility = Visibility::Visible;
}

pub(crate) fn run_gameplay_attack_target_marker(
    gfx: Res<GraphicsCache>,
    player_state: Res<PlayerState>,
    mut q_marker: Query<
        (&mut Transform, &mut Visibility, &mut Sprite),
        With<GameplayAttackTargetMarker>,
    >,
) {
    let Some((mut transform, mut visibility, mut sprite)) = q_marker.iter_mut().next() else {
        return;
    };

    if !gfx.is_initialized() {
        *visibility = Visibility::Hidden;
        return;
    }

    let target = player_state.character_info().attack_cn;
    if target <= 0 {
        *visibility = Visibility::Hidden;
        return;
    }

    let Some((mx, my)) = find_view_tile_for_char_nr(player_state.map(), target) else {
        *visibility = Visibility::Hidden;
        return;
    };

    let (xoff_i, yoff_i) = player_state
        .map()
        .tile_at_xy(mx as usize, my as usize)
        .map(|t| (t.obj_xoff, t.obj_yoff))
        .unwrap_or((0, 0));

    let Some(src) = gfx.get_sprite(ATTACK_TARGET_SPRITE_ID) else {
        *visibility = Visibility::Hidden;
        return;
    };
    *sprite = src.clone();
    sprite.color = Color::WHITE;

    let xpos = mx * 32;
    let ypos = my * 32;
    let Some((sx_i, sy_i)) =
        copysprite_screen_pos(ATTACK_TARGET_SPRITE_ID, &gfx, xpos, ypos, xoff_i, yoff_i)
    else {
        *visibility = Visibility::Hidden;
        return;
    };

    let draw_order = ((TILEY - 1 - my as usize) * TILEX + (mx as usize)) as f32;
    let z = Z_FX_BASE + draw_order * Z_WORLD_STEP + 0.0025;
    transform.translation = screen_to_world(sx_i as f32, sy_i as f32, z);
    *visibility = Visibility::Visible;
}

pub(crate) fn spawn_map_misc_action_marker(
    commands: &mut Commands,
    gfx: &GraphicsCache,
    world_root: Entity,
) {
    let Some(src) = gfx
        .get_sprite(DROP_MARKER_SPRITE_ID)
        .or_else(|| gfx.get_sprite(USE_MARKER_SPRITE_ID))
    else {
        log::warn!(
            "Missing misc-action marker sprites (32/45) in GraphicsCache; misc marker disabled"
        );
        return;
    };

    let id = commands
        .spawn((
            GameplayRenderEntity,
            GameplayMiscActionMarker,
            src.clone(),
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

pub(crate) fn run_gameplay_misc_action_marker(
    gfx: Res<GraphicsCache>,
    player_state: Res<PlayerState>,
    mut q_marker: Query<
        (&mut Sprite, &mut Transform, &mut Visibility),
        With<GameplayMiscActionMarker>,
    >,
) {
    let Some((mut sprite, mut transform, mut visibility)) = q_marker.iter_mut().next() else {
        return;
    };

    if !gfx.is_initialized() {
        *visibility = Visibility::Hidden;
        return;
    }

    let pl = player_state.character_info();
    let action = pl.misc_action as u32;

    let (marker_sprite_id, mx, my, xoff_i, yoff_i) = match action {
        DR_DROP => {
            let Some((mx, my)) =
                find_view_tile_for_world(player_state.map(), pl.misc_target1, pl.misc_target2)
            else {
                *visibility = Visibility::Hidden;
                return;
            };
            (DROP_MARKER_SPRITE_ID, mx, my, 0, 0)
        }
        DR_PICKUP => {
            let Some((mx, my)) =
                find_view_tile_for_world(player_state.map(), pl.misc_target1, pl.misc_target2)
            else {
                *visibility = Visibility::Hidden;
                return;
            };
            (PICKUP_MARKER_SPRITE_ID, mx, my, 0, 0)
        }
        DR_USE => {
            let Some((mx, my)) =
                find_view_tile_for_world(player_state.map(), pl.misc_target1, pl.misc_target2)
            else {
                *visibility = Visibility::Hidden;
                return;
            };
            (USE_MARKER_SPRITE_ID, mx, my, 0, 0)
        }
        DR_GIVE => {
            let Some((mx, my)) = find_view_tile_for_char_id(player_state.map(), pl.misc_target1)
            else {
                *visibility = Visibility::Hidden;
                return;
            };
            let (xoff_i, yoff_i) = player_state
                .map()
                .tile_at_xy(mx as usize, my as usize)
                .map(|t| (t.obj_xoff, t.obj_yoff))
                .unwrap_or((0, 0));
            (USE_MARKER_SPRITE_ID, mx, my, xoff_i, yoff_i)
        }
        _ => {
            *visibility = Visibility::Hidden;
            return;
        }
    };

    let Some(src) = gfx.get_sprite(marker_sprite_id) else {
        *visibility = Visibility::Hidden;
        return;
    };
    *sprite = src.clone();
    sprite.color = Color::WHITE;

    let xpos = mx * 32;
    let ypos = my * 32;
    let Some((sx_i, sy_i)) =
        copysprite_screen_pos(marker_sprite_id, &gfx, xpos, ypos, xoff_i, yoff_i)
    else {
        *visibility = Visibility::Hidden;
        return;
    };

    let draw_order = ((TILEY - 1 - my as usize) * TILEX + (mx as usize)) as f32;
    let z = Z_FX_BASE + draw_order * Z_WORLD_STEP + 0.002;
    transform.translation = screen_to_world(sx_i as f32, sy_i as f32, z);
    *visibility = Visibility::Visible;
}

pub(crate) fn run_gameplay_map_hover_and_click(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
    mouse: Res<ButtonInput<MouseButton>>,
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    mut hovered: ResMut<GameplayHoveredTile>,
    mut hover_target: ResMut<GameplayHoverTarget>,
    mut cursor_state: ResMut<GameplayCursorTypeState>,
) {
    hovered.clear();
    hover_target.kind = GameplayHoverTargetKind::None;
    hover_target.tile_x = -1;
    hover_target.tile_y = -1;

    // If a UI system already claimed the cursor (inventory hover, etc), don't override it.
    let ui_has_cursor = cursor_state.cursor != GameplayCursorType::None;

    let Some(game_pos) = cursor_game_pos(&windows, &cameras) else {
        return;
    };
    let Some((mx, my)) = hovered_view_tile(game_pos) else {
        return;
    };

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let keys_mask = (shift as u8) | ((ctrl as u8) << 1) | ((alt as u8) << 2);

    let (citem, base_map_mx, base_map_my) = {
        let pl = player_state.character_info();
        (pl.citem, mx, my)
    };

    // Resolve which tile we are *actually* interacting with (may snap to nearby item/char).
    let (use_mx, use_my, has_item, has_usable, has_char, char_nr, world_x, world_y) = {
        let map = player_state.map();
        let mut use_mx = base_map_mx;
        let mut use_my = base_map_my;

        // Build mode special-case (orig: pl.citem==46). No snapping.
        if citem != 46 {
            match keys_mask {
                1 => {
                    if citem == 0 {
                        (use_mx, use_my, _) = snap_to_nearby_flag(map, use_mx, use_my, ISITEM);
                    }
                }
                2 | 4 => {
                    (use_mx, use_my, _) = snap_to_nearby_flag(map, use_mx, use_my, ISCHAR);
                }
                _ => {}
            }
        }

        let Some(tile) = map.tile_at_xy(use_mx as usize, use_my as usize) else {
            return;
        };

        let has_item = (tile.flags & ISITEM) != 0;
        let has_usable = (tile.flags & ISUSABLE) != 0;
        let has_char = (tile.flags & ISCHAR) != 0;
        let char_nr = tile.ch_nr as u32;

        (
            use_mx,
            use_my,
            has_item,
            has_usable,
            has_char,
            char_nr,
            tile.x as i16,
            tile.y as i32,
        )
    };

    hovered.set(use_mx, use_my);
    hover_target.tile_x = use_mx;
    hover_target.tile_y = use_my;
    hover_target.kind = if matches!(keys_mask, 2 | 4) && has_char {
        GameplayHoverTargetKind::Character
    } else if keys_mask == 1 && has_item {
        GameplayHoverTargetKind::Object
    } else {
        GameplayHoverTargetKind::Background
    };

    if !ui_has_cursor {
        cursor_state.cursor = match keys_mask {
            1 => {
                if citem != 0 {
                    if has_item {
                        if has_usable {
                            GameplayCursorType::Use
                        } else {
                            GameplayCursorType::None
                        }
                    } else {
                        GameplayCursorType::Drop
                    }
                } else if has_item {
                    if has_usable {
                        GameplayCursorType::Use
                    } else {
                        GameplayCursorType::Take
                    }
                } else {
                    GameplayCursorType::None
                }
            }
            _ => GameplayCursorType::None,
        };
    }

    let lb_up = mouse.just_released(MouseButton::Left);
    let rb_up = mouse.just_released(MouseButton::Right);

    // Build mode: hardwired drop/pickup on the clicked tile.
    if citem == 46 {
        if rb_up {
            net.send(ClientCommand::new_drop(world_x, world_y).to_bytes());
        }
        if lb_up {
            net.send(ClientCommand::new_pickup(world_x, world_y).to_bytes());
        }
        return;
    }

    match keys_mask {
        0 => {
            if lb_up {
                net.send(ClientCommand::new_move(world_x, world_y).to_bytes());
            } else if rb_up {
                net.send(ClientCommand::new_turn(world_x, world_y).to_bytes());
            }
        }
        1 => {
            if citem != 0 {
                if !has_item {
                    if lb_up {
                        net.send(ClientCommand::new_drop(world_x, world_y).to_bytes());
                    }
                }
            }

            if has_item {
                if lb_up {
                    if has_usable {
                        net.send(ClientCommand::new_use(world_x, world_y).to_bytes());
                    } else {
                        net.send(ClientCommand::new_pickup(world_x, world_y).to_bytes());
                    }
                } else if rb_up {
                    net.send(ClientCommand::new_look_item(world_x, world_y).to_bytes());
                }
            }
        }
        2 => {
            if !has_char {
                return;
            }

            if lb_up {
                if citem != 0 {
                    net.send(ClientCommand::new_give(char_nr).to_bytes());
                } else {
                    net.send(ClientCommand::new_attack(char_nr).to_bytes());
                }
            } else if rb_up {
                net.send(ClientCommand::new_look(char_nr).to_bytes());
            }
        }
        4 => {
            if has_char {
                if lb_up {
                    let curr = player_state.selected_char();
                    if curr as u32 == char_nr {
                        player_state.set_selected_char(0);
                    } else {
                        player_state.set_selected_char(char_nr as u16);
                    }
                } else if rb_up {
                    net.send(ClientCommand::new_look(char_nr).to_bytes());
                }
            } else if lb_up {
                player_state.set_selected_char(0);
            }
        }
        _ => {}
    }
}

pub(crate) fn run_gameplay_sprite_highlight(
    hover_target: Res<GameplayHoverTarget>,
    player_state: Res<PlayerState>,
    mut state: ResMut<GameplaySpriteHighlightState>,
    mut q_tiles: Query<(&TileRender, &mut Sprite)>,
) {
    // Reset previously highlighted sprite if any.
    if state.last_layer != GameplayHoverTargetKind::None {
        let last_layer = match state.last_layer {
            GameplayHoverTargetKind::Background => Some(TileLayer::Background),
            GameplayHoverTargetKind::Object => Some(TileLayer::Object),
            GameplayHoverTargetKind::Character => Some(TileLayer::Character),
            GameplayHoverTargetKind::None => None,
        };

        if let Some(layer) = last_layer {
            for (render, mut sprite) in &mut q_tiles {
                let x = (render.index % TILEX) as i32;
                let y = (render.index / TILEX) as i32;
                if x == state.last_tile_x && y == state.last_tile_y && render.layer == layer {
                    let tint = player_state
                        .map()
                        .tile_at_xy(x as usize, y as usize)
                        .map(|t| dd_effect_tint(t.light as u32))
                        .unwrap_or(Color::WHITE);
                    sprite.color = tint;
                    break;
                }
            }
        }
    }

    // Apply current highlight.
    let (layer, z_base) = match hover_target.kind {
        GameplayHoverTargetKind::Background => (TileLayer::Background, Z_BG_BASE),
        GameplayHoverTargetKind::Object => (TileLayer::Object, Z_OBJ_BASE),
        GameplayHoverTargetKind::Character => (TileLayer::Character, Z_CHAR_BASE),
        GameplayHoverTargetKind::None => {
            state.last_layer = GameplayHoverTargetKind::None;
            state.last_tile_x = -1;
            state.last_tile_y = -1;
            return;
        }
    };

    // Find matching tile entity and brighten its sprite.
    for (render, mut sprite) in &mut q_tiles {
        let x = (render.index % TILEX) as i32;
        let y = (render.index / TILEX) as i32;
        if x == hover_target.tile_x && y == hover_target.tile_y && render.layer == layer {
            // Match engine.c highlight behavior: `copysprite(..., map[m].light|16|tmp, ...)`.
            // We approximate dd.c's effect by applying the highlight bit (16) on top of the
            // tile's current light level.
            let tint = player_state
                .map()
                .tile_at_xy(x as usize, y as usize)
                .map(|t| dd_effect_tint((t.light as u32) | 16))
                .unwrap_or(Color::srgba(1.35, 1.35, 1.35, 1.0));
            sprite.color = tint;
            break;
        }
    }

    // Store last.
    state.last_layer = hover_target.kind;
    state.last_tile_x = hover_target.tile_x;
    state.last_tile_y = hover_target.tile_y;

    // Keep z_base referenced to avoid "unused" if compiler gets clever.
    let _ = z_base;
}
