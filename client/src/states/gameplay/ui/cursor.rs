// Cursor UI systems live here.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

use crate::gfx_cache::GraphicsCache;
use crate::player_state::PlayerState;
use crate::states::gameplay::components::*;
use crate::states::gameplay::layout::*;
use crate::states::gameplay::resources::*;
use crate::states::gameplay::LastRender;

use mag_core::constants::SPR_EMPTY;

use super::super::cursor_game_pos;
use super::super::world_render::screen_to_world;

/// Spawns the carried-item sprite entity (drawn under the cursor).
pub(crate) fn spawn_ui_carried_item(commands: &mut Commands, gfx: &GraphicsCache) {
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiCarriedItem,
        LastRender {
            sprite_id: i32::MIN,
            sx: f32::NAN,
            sy: f32::NAN,
        },
        empty.clone(),
        Anchor::TOP_LEFT,
        Transform::from_translation(screen_to_world(0.0, 0.0, Z_UI_CURSOR)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

/// Updates the OS cursor and draws the carried-item sprite under the mouse.
pub(crate) fn run_gameplay_update_cursor_and_carried_item(
    mut commands: Commands,
    window_entities: Query<Entity, With<PrimaryWindow>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
    gfx: Res<GraphicsCache>,
    player_state: Res<PlayerState>,
    cursor_state: Res<GameplayCursorTypeState>,
    mut q: Query<
        (
            &mut Sprite,
            &mut Transform,
            &mut Visibility,
            &mut LastRender,
        ),
        With<GameplayUiCarriedItem>,
    >,
) {
    // Map gameplay cursor types onto the OS cursor by inserting a CursorIcon component.
    let Ok(window_entity) = window_entities.single() else {
        return;
    };
    let system_icon = match cursor_state.cursor {
        GameplayCursorType::None => SystemCursorIcon::Default,
        GameplayCursorType::Take => SystemCursorIcon::Grab,
        GameplayCursorType::Drop => SystemCursorIcon::Grabbing,
        GameplayCursorType::Swap => SystemCursorIcon::Move,
        GameplayCursorType::Use => SystemCursorIcon::Pointer,
    };
    commands
        .entity(window_entity)
        .insert(CursorIcon::from(system_icon));

    let Some((mut sprite, mut t, mut vis, mut last)) = q.iter_mut().next() else {
        return;
    };

    let Some(game) = cursor_game_pos(&windows, &cameras) else {
        *vis = Visibility::Hidden;
        return;
    };

    let pl = player_state.character_info();
    let citem = pl.citem;

    if citem <= 0 {
        *vis = Visibility::Hidden;
        last.sprite_id = citem;
        return;
    }

    if last.sprite_id != citem {
        if let Some(src) = gfx.get_sprite(citem as usize) {
            *sprite = src.clone();
            last.sprite_id = citem;
        } else {
            *vis = Visibility::Hidden;
            return;
        }
    }

    // engine.c draws at (mouse_x-16,mouse_y-16). Alpha-ish effect for drop/swap/use.
    t.translation = screen_to_world(game.x - 16.0, game.y - 16.0, Z_UI_CURSOR);
    sprite.color = match cursor_state.cursor {
        GameplayCursorType::Drop | GameplayCursorType::Swap | GameplayCursorType::Use => {
            Color::srgba(1.0, 1.0, 1.0, 0.75)
        }
        _ => Color::WHITE,
    };
    *vis = Visibility::Visible;
}
