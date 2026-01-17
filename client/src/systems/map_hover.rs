use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::map::{TILEX, TILEY};
use crate::network::{client_commands::ClientCommand, NetworkRuntime};
use crate::player_state::PlayerState;
use crate::states::gameplay::GameplayRenderEntity;

use mag_core::constants::{ISCHAR, ISITEM, XPOS, YPOS};

// Keep these in-sync with the draw ordering in `states/gameplay.rs`.
const Z_WORLD_STEP: f32 = 0.01;
const Z_BG_BASE: f32 = 0.0;
const Z_HOVER_BIAS: f32 = 0.005;
const Z_GOTO_BIAS: f32 = 0.001;

// orig/engine.c draws sprite 31 at pl.goto_x/pl.goto_y.
const MOVE_TARGET_SPRITE_ID: usize = 31;

// Ground tiles are rendered as an isometric diamond occupying the lower half
// of a 32x32 tile cell. The visible diamond is 32px wide x 16px tall.
const GROUND_DIAMOND_W: u32 = 32;
const GROUND_DIAMOND_H: u32 = 16;
const GROUND_DIAMOND_Y_OFFSET: f32 = 16.0;

#[derive(Component)]
pub(crate) struct GameplayMapHoverHighlight;

#[derive(Component)]
pub(crate) struct GameplayMoveTargetMarker;

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

    // Intentionally minimal: we mirror the C globals and only need set/clear.
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

/// Screen-space top-left for a 1x1 (32x32) tile at view coords (mx,my).
///
/// This mirrors `dd.c::copysprite` positioning for xs=1, ys=1, with xoff/yoff=0.
fn tile_screen_pos(mx: i32, my: i32) -> (f32, f32) {
    let xpos = mx * 32;
    let ypos = my * 32;

    let rx = (xpos / 2) + (ypos / 2) - 16 + 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32);

    let ry = (xpos / 4) - (ypos / 4) + YPOS - 32;

    (rx as f32, ry as f32)
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

fn sprite_tiles_xy(sprite: &Sprite, images: &Assets<Image>) -> Option<(i32, i32)> {
    let image = images.get(&sprite.image)?;
    let size = image.size();

    // dd.c treats sprites as being composed of 32x32 "blocks".
    let w = (size.x.max(1) as i32).max(1);
    let h = (size.y.max(1) as i32).max(1);

    let xs = (w + 31) / 32;
    let ys = (h + 31) / 32;
    Some((xs.max(1), ys.max(1)))
}

/// A minimal copy of `states/gameplay.rs::copysprite_screen_pos`, used here so
/// this overlay aligns exactly with the rest of the map rendering.
fn copysprite_screen_pos(
    sprite_id: usize,
    gfx: &GraphicsCache,
    images: &Assets<Image>,
    xpos: i32,
    ypos: i32,
    xoff: i32,
    yoff: i32,
) -> Option<(i32, i32)> {
    let sprite = gfx.get_sprite(sprite_id)?;
    let (xs, ys) = sprite_tiles_xy(sprite, images)?;

    let mut rx = (xpos / 2) + (ypos / 2) - (xs * 16) + 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32);
    let mut ry = (xpos / 4) - (ypos / 4) + YPOS - (ys * 32);

    rx += xoff;
    ry += yoff;
    Some((rx, ry))
}

pub(crate) fn spawn_map_hover_highlight(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    world_root: Entity,
) {
    // Create a 32x16 alpha mask shaped like an isometric diamond.
    // This matches the *visible* ground tile shape (not the 32x32 tile cell).
    let mut data = vec![0u8; (GROUND_DIAMOND_W * GROUND_DIAMOND_H * 4) as usize];
    for y in 0..GROUND_DIAMOND_H {
        for x in 0..GROUND_DIAMOND_W {
            // Diamond condition in normalized coordinates:
            // |(x-cx)/16| + |(y-cy)/8| <= 1
            let fx = (x as f32 + 0.5) - (GROUND_DIAMOND_W as f32 / 2.0);
            let fy = (y as f32 + 0.5) - (GROUND_DIAMOND_H as f32 / 2.0);
            let nx = (fx / (GROUND_DIAMOND_W as f32 / 2.0)).abs();
            let ny = (fy / (GROUND_DIAMOND_H as f32 / 2.0)).abs();
            let inside = (nx + ny) <= 1.0;

            let idx = ((y * GROUND_DIAMOND_W + x) * 4) as usize;
            data[idx] = 255;
            data[idx + 1] = 255;
            data[idx + 2] = 255;
            data[idx + 3] = if inside { 255 } else { 0 };
        }
    }

    let image = Image::new(
        Extent3d {
            width: GROUND_DIAMOND_W,
            height: GROUND_DIAMOND_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    );

    let handle = images.add(image);

    let id = commands
        .spawn((
            GameplayRenderEntity,
            GameplayMapHoverHighlight,
            Sprite {
                image: handle,
                // Neutral highlight (not red). This is a subtle brighten overlay.
                color: Color::srgba(1.0, 1.0, 1.0, 0.22),
                custom_size: Some(Vec2::new(GROUND_DIAMOND_W as f32, GROUND_DIAMOND_H as f32)),
                ..default()
            },
            Anchor::TOP_LEFT,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();

    commands.entity(world_root).add_child(id);

    // Track hover in a resource (C has globals tile_x/tile_y).
    commands.insert_resource(GameplayHoveredTile {
        tile_x: -1,
        tile_y: -1,
    });
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

pub(crate) fn run_gameplay_move_target_marker(
    gfx: Res<GraphicsCache>,
    images: Res<Assets<Image>>,
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
    let Some((sx_i, sy_i)) =
        copysprite_screen_pos(MOVE_TARGET_SPRITE_ID, &gfx, &images, xpos, ypos, 0, 0)
    else {
        *visibility = Visibility::Hidden;
        return;
    };

    let draw_order = ((TILEY - 1 - my as usize) * TILEX + (mx as usize)) as f32;
    let z = Z_BG_BASE + draw_order * Z_WORLD_STEP + Z_GOTO_BIAS;
    transform.translation = screen_to_world(sx_i as f32, sy_i as f32, z);
    *visibility = Visibility::Visible;
}

pub(crate) fn run_gameplay_map_hover_and_click(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
    mouse: Res<ButtonInput<MouseButton>>,
    net: Res<NetworkRuntime>,
    player_state: Res<PlayerState>,
    hovered: Option<ResMut<GameplayHoveredTile>>,
    mut q_highlight: Query<(&mut Transform, &mut Visibility), With<GameplayMapHoverHighlight>>,
) {
    let Some(mut hovered) = hovered else {
        return;
    };

    let Some((mut transform, mut visibility)) = q_highlight.iter_mut().next() else {
        return;
    };

    // Default: no hover.
    hovered.clear();
    *visibility = Visibility::Hidden;

    let Some(game_pos) = cursor_game_pos(&windows, &cameras) else {
        return;
    };

    let Some((mx, my)) = hovered_view_tile(game_pos) else {
        return;
    };

    // Only highlight "ground tiles" with nothing on top.
    let tile = player_state.map().tile_at_xy(mx as usize, my as usize);
    let Some(tile) = tile else {
        return;
    };

    hovered.set(mx, my);

    // Update highlight placement.
    let (sx, sy) = tile_screen_pos(mx, my);
    let draw_order = ((TILEY - 1 - my as usize) * TILEX + (mx as usize)) as f32;
    let z = Z_BG_BASE + draw_order * Z_WORLD_STEP + Z_HOVER_BIAS;
    // `tile_screen_pos` is for the 32x32 tile cell; ground diamond lives in the lower half.
    transform.translation = screen_to_world(sx, sy + GROUND_DIAMOND_Y_OFFSET, z);
    *visibility = Visibility::Visible;

    // Mouse up -> command (matches inter.c's MS_LB_UP / MS_RB_UP behavior).
    if mouse.just_released(MouseButton::Left) {
        let cmd = ClientCommand::new_move(tile.x as i16, tile.y as i32);
        net.send(cmd.to_bytes());
    } else if mouse.just_released(MouseButton::Right) {
        let cmd = ClientCommand::new_turn(tile.x as i16, tile.y as i32);
        net.send(cmd.to_bytes());
    }
}
