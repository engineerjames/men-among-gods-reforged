use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;

use mag_core::constants::SPR_EMPTY;

use super::components::GameplayRenderEntity;

#[inline]
pub(crate) fn screen_to_world(sx: f32, sy: f32, z: f32) -> Vec3 {
    Vec3::new(sx - TARGET_WIDTH * 0.5, TARGET_HEIGHT * 0.5 - sy, z)
}

#[derive(Component)]
pub(crate) struct GameplayShadowEntity;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileShadow {
    pub(crate) index: usize,
    pub(crate) layer: ShadowLayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShadowLayer {
    Object,
    Character,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileRender {
    pub(crate) index: usize,
    pub(crate) layer: TileLayer,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileFlagOverlay {
    pub(crate) index: usize,
    pub(crate) kind: TileFlagOverlayKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileFlagOverlayKind {
    MoveBlock,
    SightBlock,
    Indoors,
    Underwater,
    NoMonsters,
    Bank,
    Tavern,
    NoMagic,
    DeathTrap,
    NoLag,
    Arena,
    NoExpire,
    UnknownHighBit,
    Injured,
    Death,
    Tomb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileLayer {
    Background,
    Object,
    Character,
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct LastRender {
    pub(crate) sprite_id: i32,
    pub(crate) sx: f32,
    pub(crate) sy: f32,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayWorldRoot;

pub(crate) fn spawn_tile_entity(
    commands: &mut Commands,
    gfx: &GraphicsCache,
    render: TileRender,
) -> Option<Entity> {
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return None;
    };

    let initial_visibility = match render.layer {
        TileLayer::Background => Visibility::Visible,
        TileLayer::Object | TileLayer::Character => Visibility::Hidden,
    };

    let id = commands
        .spawn((
            GameplayRenderEntity,
            render,
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::default(),
            GlobalTransform::default(),
            initial_visibility,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();

    Some(id)
}

pub(crate) fn spawn_tile_overlay_entity(
    commands: &mut Commands,
    gfx: &GraphicsCache,
    overlay: TileFlagOverlay,
) -> Option<Entity> {
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return None;
    };

    let id = commands
        .spawn((
            GameplayRenderEntity,
            overlay,
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();

    Some(id)
}

pub(crate) fn spawn_shadow_entity(
    commands: &mut Commands,
    gfx: &GraphicsCache,
    shadow: TileShadow,
) -> Option<Entity> {
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return None;
    };

    let id = commands
        .spawn((
            GameplayRenderEntity,
            GameplayShadowEntity,
            shadow,
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();

    Some(id)
}
