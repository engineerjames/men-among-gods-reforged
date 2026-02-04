use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::network::{client_commands::ClientCommand, NetworkRuntime};
use crate::player_state::PlayerState;
use crate::states::gameplay::{
    dd_effect_tint, GameplayCursorType, GameplayCursorTypeState, GameplayRenderEntity, TileLayer,
    TileRender,
};
use crate::systems::magic_postprocess::MagicScreenCamera;
use crate::systems::sound::SoundEventQueue;

use mag_core::constants::{
    DR_DROP, DR_GIVE, DR_PICKUP, DR_USE, INFRARED, INVIS, ISCHAR, ISITEM, ISUSABLE, STONED, TILEX,
    TILEY, UWATER, XPOS, YPOS,
};

// Keep these in-sync with the draw ordering in `states/gameplay.rs`.
const Z_WORLD_STEP: f32 = 0.01;
const Z_BG_BASE: f32 = 0.0;
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

#[derive(Default)]
pub(crate) struct MoveTargetMarkerTickGate {
    initialized: bool,
    last_goto_x: i32,
    last_goto_y: i32,
    last_origin_x: i32,
    last_origin_y: i32,
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct GameplayHoveredTile {
    pub tile_x: i32,
    pub tile_y: i32,
}

impl GameplayHoveredTile {
    /// Reset hovered tile coordinates to "none".
    fn clear(&mut self) {
        self.tile_x = -1;
        self.tile_y = -1;
    }

    /// Set the hovered tile coordinates.
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

#[inline]
/// Convert screen-space pixels into world-space coordinates.
fn screen_to_world(sx: f32, sy: f32, z: f32) -> Vec3 {
    Vec3::new(sx - TARGET_WIDTH * 0.5, TARGET_HEIGHT * 0.5 - sy, z)
}

/// Convert cursor position into the game's fixed 800x600 "logical" coordinates.
///
/// Mirrors `systems/debug.rs::print_click_coords`.
fn cursor_game_pos(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<&Camera, (With<Camera2d>, With<MagicScreenCamera>)>,
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

/// Map world grid coordinates to view-tile coordinates, with a stable fallback.
fn find_view_tile_for_world(map: &crate::map::GameMap, wx: i32, wy: i32) -> Option<(i32, i32)> {
    if wx < 0 || wy < 0 {
        return None;
    }

    // Primary path: compute view-tile coordinates from the current top-left origin.
    // This is O(1) and is the correct mapping when the map's (x,y) grid is consistent.
    let origin = map.tile_at_xy(0, 0)?;
    let ox = origin.x as i32;
    let oy = origin.y as i32;

    let expected_mx = wx - ox;
    let expected_my = wy - oy;

    if expected_mx >= 0
        && expected_my >= 0
        && (expected_mx as usize) < TILEX
        && (expected_my as usize) < TILEY
    {
        if let Some(tile) = map.tile_at_xy(expected_mx as usize, expected_my as usize) {
            if tile.x as i32 == wx && tile.y as i32 == wy {
                return Some((expected_mx, expected_my));
            }
        }
    }

    // Fallback: scan for a matching (x,y).
    // During scroll/memmove transitions the client map can temporarily contain duplicates/stale
    // world coords; when that happens we pick the match *closest to the expected coords* to keep
    // the marker stable rather than flickering between equally-valid matches.
    let wx_u = wx as u16;
    let wy_u = wy as u16;

    let mut best: Option<(i32, i32, i32)> = None; // (mx,my,score)
    for my in 0..(TILEY as i32) {
        for mx in 0..(TILEX as i32) {
            let Some(tile) = map.tile_at_xy(mx as usize, my as usize) else {
                continue;
            };
            if tile.x != wx_u || tile.y != wy_u {
                continue;
            }

            let score = (mx - expected_mx).abs() + (my - expected_my).abs();
            match best {
                None => best = Some((mx, my, score)),
                Some((_, _, best_score)) if score < best_score => best = Some((mx, my, score)),
                _ => {}
            }
        }
    }

    best.map(|(mx, my, _)| (mx, my))
}

/// Find the view-tile coordinates for a character by unique id.
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

/// Find the view-tile coordinates for a character by character number.
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
/// Check whether a map tile at view coords has a given flag.
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
}

/// Spawn the move target marker sprite as a child of the world root.
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

/// Spawn the attack target marker sprite as a child of the world root.
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

/// Update move target marker visibility and position from player state.
pub(crate) fn run_gameplay_move_target_marker(
    gfx: Res<GraphicsCache>,
    player_state: Res<PlayerState>,
    mut gate: Local<MoveTargetMarkerTickGate>,
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

    // Keep the marker pinned to a WORLD position.
    // The client map view shifts via scroll/memmove commands, so we must also update whenever
    // the view origin changes (tile 0,0 x/y), not just when goto_x/goto_y changes.
    let (origin_x, origin_y) = player_state
        .map()
        .tile_at_xy(0, 0)
        .map(|t| (t.x as i32, t.y as i32))
        .unwrap_or((0, 0));

    let should_update = !gate.initialized
        || wx != gate.last_goto_x
        || wy != gate.last_goto_y
        || origin_x != gate.last_origin_x
        || origin_y != gate.last_origin_y;
    if !should_update {
        return;
    }
    gate.initialized = true;
    gate.last_goto_x = wx;
    gate.last_goto_y = wy;
    gate.last_origin_x = origin_x;
    gate.last_origin_y = origin_y;

    // The server uses (0,0) as "no move target" in several places.
    // Don't draw the marker at a random tile when idle.
    if wx == 0 && wy == 0 {
        *visibility = Visibility::Hidden;
        return;
    }

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

/// Update attack target marker visibility and position from player state.
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

/// Spawn the misc-action marker used for drop/pickup/use/give.
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

/// Update misc-action marker sprite, position, and visibility.
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

/// Handle hover detection, cursor state, and click-to-command input.
pub(crate) fn run_gameplay_map_hover_and_click(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Camera, (With<Camera2d>, With<MagicScreenCamera>)>,
    mouse: Res<ButtonInput<MouseButton>>,
    net: Res<NetworkRuntime>,
    mut sound_queue: ResMut<SoundEventQueue>,
    mut player_state: ResMut<PlayerState>,
    mut hovered: ResMut<GameplayHoveredTile>,
    mut hover_target: ResMut<GameplayHoverTarget>,
    mut cursor_state: ResMut<GameplayCursorTypeState>,
    mut click_capture: ResMut<crate::states::gameplay::resources::GameplayUiClickCapture>,
) {
    // Some UI actions (like closing the shop) should consume the click and prevent it from also
    // being interpreted as a world action.
    if click_capture.consume_world_click
        && (mouse.just_released(MouseButton::Left) || mouse.just_released(MouseButton::Right))
    {
        click_capture.consume_world_click = false;
        return;
    }

    hovered.clear();
    hover_target.kind = GameplayHoverTargetKind::None;
    hover_target.tile_x = -1;
    hover_target.tile_y = -1;

    if player_state.selected_char() != 0 {
        let selected_nr = player_state.selected_char();
        let selected_id = player_state.selected_char_id();
        let map = player_state.map();
        let still_present = if selected_id != 0 {
            find_view_tile_for_char_id(map, selected_id as i32).is_some()
        } else {
            find_view_tile_for_char_nr(map, selected_nr as i32).is_some()
        };
        if !still_present {
            player_state.clear_selected_char();
        }
    }

    // If a UI system already claimed the cursor (inventory hover, etc), don't override it.
    let ui_has_cursor = cursor_state.cursor != GameplayCursorType::None;

    let Some(game_pos) = cursor_game_pos(&windows, &cameras) else {
        return;
    };

    // Shop UI captures mouse events (orig/inter.c checks mouse_shop() before mouse_mapbox()).
    // If the cursor is over the shop panel/grid while shop is open, ignore map hover/click.
    if player_state.should_show_shop()
        && game_pos.x >= 220.0
        && game_pos.x <= 516.0
        && game_pos.y >= 260.0
        && game_pos.y <= 552.0
    {
        return;
    }
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
    let (use_mx, use_my, has_item, has_usable, has_char, char_nr, char_id, world_x, world_y) = {
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
        let char_id = tile.ch_id as u32;

        (
            use_mx,
            use_my,
            has_item,
            has_usable,
            has_char,
            char_nr,
            char_id,
            tile.x as i16,
            tile.y as i32,
        )
    };

    hovered.set(use_mx, use_my);
    hover_target.tile_x = use_mx;
    hover_target.tile_y = use_my;
    hover_target.kind = if matches!(keys_mask, 2 | 4) && has_char {
        GameplayHoverTargetKind::Character
    } else if keys_mask == 1 {
        // Shift key: only highlight if there's an item or usable object
        if has_item || has_usable {
            GameplayHoverTargetKind::Object
        } else {
            GameplayHoverTargetKind::None
        }
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
            sound_queue.push_click();
            net.send(ClientCommand::new_drop(world_x, world_y).to_bytes());
        }
        if lb_up {
            sound_queue.push_click();
            net.send(ClientCommand::new_pickup(world_x, world_y).to_bytes());
        }
        return;
    }

    match keys_mask {
        0 => {
            if lb_up {
                sound_queue.push_click();
                net.send(ClientCommand::new_move(world_x, world_y).to_bytes());
            } else if rb_up {
                sound_queue.push_click();
                net.send(ClientCommand::new_turn(world_x, world_y).to_bytes());
            }
        }
        1 => {
            if citem != 0 {
                if !has_item {
                    if lb_up {
                        sound_queue.push_click();
                        net.send(ClientCommand::new_drop(world_x, world_y).to_bytes());
                    }
                }
            }

            if has_item {
                if lb_up {
                    if has_usable {
                        sound_queue.push_click();
                        net.send(ClientCommand::new_use(world_x, world_y).to_bytes());
                    } else {
                        sound_queue.push_click();
                        net.send(ClientCommand::new_pickup(world_x, world_y).to_bytes());
                    }
                } else if rb_up {
                    sound_queue.push_click();
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
                    sound_queue.push_click();
                    net.send(ClientCommand::new_give(char_nr).to_bytes());
                } else {
                    sound_queue.push_click();
                    net.send(ClientCommand::new_attack(char_nr).to_bytes());
                }
            } else if rb_up {
                sound_queue.push_click();
                net.send(ClientCommand::new_look(char_nr).to_bytes());
            }
        }
        4 => {
            if has_char {
                if lb_up {
                    let curr = player_state.selected_char();
                    if curr as u32 == char_nr {
                        player_state.clear_selected_char();
                    } else {
                        player_state.set_selected_char_with_id(char_nr as u16, char_id as u16);
                        if player_state
                            .lookup_name(char_nr as u16, char_id as u16)
                            .is_none()
                        {
                            net.send(ClientCommand::new_autolook(char_nr).to_bytes());
                        }
                    }
                } else if rb_up {
                    sound_queue.push_click();
                    net.send(ClientCommand::new_look(char_nr).to_bytes());
                }
            } else if lb_up {
                player_state.clear_selected_char();
            }
        }
        _ => {}
    }
}

/// Tint the hovered tile sprite based on target kind and effects.
pub(crate) fn run_gameplay_sprite_highlight(
    hover_target: Res<GameplayHoverTarget>,
    player_state: Res<PlayerState>,
    mut q_tiles: Query<(&TileRender, &mut Sprite)>,
) {
    let mut hovered_tile: Option<(i32, i32, TileLayer)> = None;

    if hover_target.kind != GameplayHoverTargetKind::None {
        let layer = match hover_target.kind {
            GameplayHoverTargetKind::Background => TileLayer::Background,
            GameplayHoverTargetKind::Object => TileLayer::Object,
            GameplayHoverTargetKind::Character => TileLayer::Character,
            GameplayHoverTargetKind::None => TileLayer::Background,
        };

        let tx = hover_target.tile_x;
        let ty = hover_target.tile_y;
        if tx >= 0 && ty >= 0 {
            if let Some(tile) = player_state.map().tile_at_xy(tx as usize, ty as usize) {
                // Compute engine.c effect bits for this layer, then apply highlight (|16).
                let mut effect: u32 = tile.light as u32;
                match layer {
                    TileLayer::Background => {
                        if (tile.flags & INVIS) != 0 {
                            effect |= 64;
                        }
                        if (tile.flags & INFRARED) != 0 {
                            effect |= 256;
                        }
                        if (tile.flags & UWATER) != 0 {
                            effect |= 512;
                        }
                    }
                    TileLayer::Object => {
                        if (tile.flags & INFRARED) != 0 {
                            effect |= 256;
                        }
                        if (tile.flags & UWATER) != 0 {
                            effect |= 512;
                        }
                    }
                    TileLayer::Character => {
                        if tile.ch_nr != 0 && tile.ch_nr == player_state.selected_char() {
                            effect |= 32;
                        }
                        if (tile.flags & STONED) != 0 {
                            effect |= 128;
                        }
                        if (tile.flags & INFRARED) != 0 {
                            effect |= 256;
                        }
                        if (tile.flags & UWATER) != 0 {
                            effect |= 512;
                        }
                    }
                }

                effect |= 16;
                let tint = dd_effect_tint(effect);

                for (render, mut sprite) in &mut q_tiles {
                    let x = (render.index % TILEX) as i32;
                    let y = (render.index / TILEX) as i32;
                    if x == tx && y == ty && render.layer == layer {
                        sprite.color = tint;
                        hovered_tile = Some((tx, ty, layer));
                        break;
                    }
                }
            }
        }
    }

    let selected = player_state.selected_char();
    if selected == 0 {
        return;
    }

    let Some((sel_mx, sel_my)) = find_view_tile_for_char_nr(player_state.map(), selected as i32)
    else {
        return;
    };

    if let Some((hx, hy, layer)) = hovered_tile {
        if hx == sel_mx && hy == sel_my && layer == TileLayer::Character {
            return;
        }
    }

    let Some(tile) = player_state
        .map()
        .tile_at_xy(sel_mx as usize, sel_my as usize)
    else {
        return;
    };

    let mut effect: u32 = tile.light as u32;
    effect |= 32;
    if (tile.flags & STONED) != 0 {
        effect |= 128;
    }
    if (tile.flags & INFRARED) != 0 {
        effect |= 256;
    }
    if (tile.flags & UWATER) != 0 {
        effect |= 512;
    }
    let tint = dd_effect_tint(effect);

    for (render, mut sprite) in &mut q_tiles {
        let x = (render.index % TILEX) as i32;
        let y = (render.index / TILEX) as i32;
        if x == sel_mx && y == sel_my && render.layer == TileLayer::Character {
            sprite.color = tint;
            break;
        }
    }
}
