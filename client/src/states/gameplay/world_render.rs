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

// dd.c lighting approximation:
// do_effect() scales RGB by: LEFFECT / (effect^2 + LEFFECT), with LEFFECT = gamma - 4880.
// At default gamma=5000, LEFFECT=120.
const DD_LEFFECT: f32 = 120.0;

/// Approximates legacy dd.c lighting/effect flags as a per-sprite tint color.
///
/// This implements darkness, highlight, and other effect bits from the original renderer.
pub(crate) fn dd_effect_tint(effect: u32) -> Color {
    // We approximate the dd.c per-pixel effect with a per-sprite tint.
    // This matches the most important behavior: darkness from `effect` and
    // the highlight bit (16) which doubles brightness.

    let mut base = effect;
    let highlight = (base & 16) != 0;
    let green = (base & 32) != 0;
    let invis = (base & 64) != 0;
    let grey = (base & 128) != 0;
    let infra = (base & 256) != 0;
    let water = (base & 512) != 0;

    // Strip known flag bits to recover the numeric light level.
    if highlight {
        base = base.saturating_sub(16);
    }
    if green {
        base = base.saturating_sub(32);
    }
    if invis {
        base = base.saturating_sub(64);
    }
    if grey {
        base = base.saturating_sub(128);
    }
    if infra {
        base = base.saturating_sub(256);
    }
    if water {
        base = base.saturating_sub(512);
    }

    let e = (base.min(1023)) as f32;
    let shade = if e <= 0.0 {
        1.0
    } else {
        DD_LEFFECT / (e * e + DD_LEFFECT)
    };

    let mut r = shade;
    let mut g = shade;
    let mut b = shade;

    // dd.c's "grey" effect is a greyscale conversion. Since we're tinting a full sprite
    // (not per-pixel), approximate it by reducing saturation.
    if grey {
        // Slightly greenish grayscale like RGB565 tends to look.
        r *= 0.85;
        g *= 0.95;
        b *= 0.85;
    }

    // Approximate a few legacy effect flags used by engine.c (notably infra/water).
    if infra {
        g = 0.0;
        b = 0.0;
    }
    if water {
        r *= 0.7;
        g *= 0.85;
        // b stays as-is
    }

    // engine.c highlight uses `|16`, dd.c then doubles channels.
    if highlight {
        r *= 2.0;
        g *= 2.0;
        b *= 2.0;
    }

    // engine.c selection uses `|32` for characters; dd.c bumps green.
    if green {
        g = (g + 0.5).min(1.0);
    }

    if invis {
        r = 0.0;
        g = 0.0;
        b = 0.0;
    }

    // Bevy will clamp in the shader, but we keep values reasonable.
    let clamp = |v: f32| v.clamp(0.0, 1.35);
    Color::srgba(clamp(r), clamp(g), clamp(b), 1.0)
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
