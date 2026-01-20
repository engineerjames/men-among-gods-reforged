// Portrait/rank/overlay UI helpers live here.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::gfx_cache::GraphicsCache;
use crate::player_state::PlayerState;
use crate::states::gameplay::components::*;
use crate::states::gameplay::layout::*;
use crate::states::gameplay::LastRender;

use mag_core::constants::SPR_EMPTY;

use super::super::centered_text_x;
use super::super::world_render::screen_to_world;

/// Spawns the main UI overlay sprite (the large fixed UI background).
pub(crate) fn spawn_ui_overlay(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches `copyspritex(1,0,0,0)` in engine.c
    let Some(sprite) = gfx.get_sprite(1) else {
        return;
    };

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiOverlay,
        sprite.clone(),
        Anchor::TOP_LEFT,
        Transform::from_translation(screen_to_world(0.0, 0.0, Z_UI)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

/// Spawns the portrait sprite entity (updated dynamically from player/rank state).
pub(crate) fn spawn_ui_portrait(commands: &mut Commands, gfx: &GraphicsCache) {
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiPortrait,
        LastRender {
            sprite_id: i32::MIN,
            sx: f32::NAN,
            sy: f32::NAN,
        },
        empty.clone(),
        Anchor::TOP_LEFT,
        Transform::from_translation(screen_to_world(402.0, 32.0, Z_UI_PORTRAIT)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

/// Spawns the rank insignia sprite and portrait name/rank labels.
pub(crate) fn spawn_ui_rank(commands: &mut Commands, gfx: &GraphicsCache) {
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiRank,
        LastRender {
            sprite_id: i32::MIN,
            sx: f32::NAN,
            sy: f32::NAN,
        },
        empty.clone(),
        Anchor::TOP_LEFT,
        Transform::from_translation(screen_to_world(463.0, 38.0, Z_UI_RANK)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Portrait name + rank strings (engine.c y=152 and y=172), centered within 125px.
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiPortraitNameLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_PORTRAIT_TEXT_AREA_X,
            HUD_PORTRAIT_NAME_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiPortraitRankLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_PORTRAIT_TEXT_AREA_X,
            HUD_PORTRAIT_RANK_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

/// Updates the "top selected name" label shown in the HUD.
pub(crate) fn run_gameplay_update_top_selected_name(
    player_state: Res<PlayerState>,
    mut q: Query<(&mut BitmapText, &mut Transform), With<GameplayUiTopSelectedNameLabel>>,
) {
    let mut name: &str = "";

    let selected = player_state.selected_char();
    if selected != 0 {
        // engine.c uses lookup(selected_char, 0) (0 means "ignore id")
        if let Some(n) = player_state.lookup_name(selected, 0) {
            name = n;
        }
    }

    if name.is_empty() {
        // Fallback to local player name
        let pl = player_state.character_info();
        let end = pl
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(pl.name.len());
        name = std::str::from_utf8(&pl.name[..end]).unwrap_or("");
    }

    let sx = centered_text_x(HUD_TOP_NAME_AREA_X, HUD_TOP_NAME_AREA_W, name);

    for (mut text, mut t) in &mut q {
        if text.text != name {
            text.text.clear();
            text.text.push_str(name);
        }
        t.translation = screen_to_world(sx, HUD_TOP_NAME_Y, Z_UI_TEXT);
    }
}

/// Updates the portrait area name and rank labels.
///
/// Uses shop target or look target when those UIs are active, otherwise the player.
pub(crate) fn run_gameplay_update_portrait_name_and_rank(
    player_state: Res<PlayerState>,
    mut q: ParamSet<(
        Query<(&mut BitmapText, &mut Transform), With<GameplayUiPortraitNameLabel>>,
        Query<(&mut BitmapText, &mut Transform), With<GameplayUiPortraitRankLabel>>,
    )>,
) {
    // Matches engine.c behavior:
    // - If shop is open: use shop target name/rank
    // - Else if look is active: use look target name/rank
    // - Else: use player name/rank
    let (name, points_tot) = if player_state.should_show_shop() {
        let shop = player_state.shop_target();
        (
            shop.name().unwrap_or(""),
            shop.points().min(i32::MAX as u32) as i32,
        )
    } else if player_state.should_show_look() {
        let look = player_state.look_target();
        (
            look.name().unwrap_or(""),
            look.points().min(i32::MAX as u32) as i32,
        )
    } else {
        let pl = player_state.character_info();
        let end = pl
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(pl.name.len());
        (
            std::str::from_utf8(&pl.name[..end]).unwrap_or(""),
            pl.points_tot,
        )
    };

    let rank = mag_core::ranks::rank_name(points_tot as u32);

    let name_x = centered_text_x(HUD_PORTRAIT_TEXT_AREA_X, HUD_PORTRAIT_TEXT_AREA_W, name);
    let rank_x = centered_text_x(HUD_PORTRAIT_TEXT_AREA_X, HUD_PORTRAIT_TEXT_AREA_W, rank);

    for (mut text, mut t) in q.p0().iter_mut() {
        if text.text != name {
            text.text.clear();
            text.text.push_str(name);
        }
        t.translation = screen_to_world(name_x, HUD_PORTRAIT_NAME_Y, Z_UI_TEXT);
    }
    for (mut text, mut t) in q.p1().iter_mut() {
        if text.text != rank {
            text.text.clear();
            text.text.push_str(rank);
        }
        t.translation = screen_to_world(rank_x, HUD_PORTRAIT_RANK_Y, Z_UI_TEXT);
    }
}

pub(crate) fn update_ui_portrait_sprite(
    gfx: &GraphicsCache,
    ui_portrait_sprite_id: i32,
    q_portrait: &mut Query<
        (&mut Sprite, &mut Visibility, &mut LastRender),
        With<GameplayUiPortrait>,
    >,
) {
    if let Some((mut sprite, mut visibility, mut last)) = q_portrait.iter_mut().next() {
        if ui_portrait_sprite_id > 0 {
            if last.sprite_id != ui_portrait_sprite_id {
                if let Some(src) = gfx.get_sprite(ui_portrait_sprite_id as usize) {
                    *sprite = src.clone();
                    last.sprite_id = ui_portrait_sprite_id;
                    *visibility = Visibility::Visible;
                } else {
                    *visibility = Visibility::Hidden;
                }
            } else {
                *visibility = Visibility::Visible;
            }
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

pub(crate) fn update_ui_rank_sprite(
    gfx: &GraphicsCache,
    ui_rank_sprite_id: i32,
    q_rank: &mut Query<(&mut Sprite, &mut Visibility, &mut LastRender), With<GameplayUiRank>>,
) {
    if let Some((mut sprite, mut visibility, mut last)) = q_rank.iter_mut().next() {
        if ui_rank_sprite_id > 0 {
            if last.sprite_id != ui_rank_sprite_id {
                if let Some(src) = gfx.get_sprite(ui_rank_sprite_id as usize) {
                    *sprite = src.clone();
                    last.sprite_id = ui_rank_sprite_id;
                    *visibility = Visibility::Visible;
                } else {
                    *visibility = Visibility::Hidden;
                }
            } else {
                *visibility = Visibility::Visible;
            }
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}
