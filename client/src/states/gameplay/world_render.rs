use bevy::ecs::query::Without;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::map::GameMap;
use crate::player_state::PlayerState;

use mag_core::constants::{
    DEATH, INFRARED, INJURED, INJURED1, INJURED2, INVIS, ISITEM, MF_ARENA, MF_BANK, MF_DEATHTRAP,
    MF_INDOORS, MF_MOVEBLOCK, MF_NOEXPIRE, MF_NOLAG, MF_NOMAGIC, MF_NOMONST, MF_SIGHTBLOCK,
    MF_TAVERN, MF_UWATER, SPR_EMPTY, STONED, TILEX, TILEY, TOMB, UWATER,
};

use super::components::{
    GameplayRenderEntity, GameplayUiBackpackSlot, GameplayUiEquipmentSlot, GameplayUiPortrait,
    GameplayUiRank, GameplayUiShop, GameplayUiSpellSlot,
};
use super::layout::{Z_BG_BASE, Z_CHAR_BASE, Z_FX_BASE, Z_OBJ_BASE, Z_SHADOW_BASE, Z_WORLD_STEP};
use super::legacy_engine;

#[inline]
pub(crate) fn should_draw_shadow(sprite_id: i32) -> bool {
    // dd.c::dd_shadow: only certain sprite id ranges get shadows.
    (2000..16_336).contains(&sprite_id) || sprite_id > 17_360
}

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

pub(crate) fn update_world_shadows(
    gfx: &GraphicsCache,
    images: &Assets<Image>,
    map: &GameMap,
    shadows_enabled: bool,
    q: &mut Query<
        (
            &TileShadow,
            &mut Sprite,
            &mut Transform,
            &mut Visibility,
            &mut LastRender,
        ),
        (
            Without<GameplayWorldRoot>,
            Without<GameplayUiPortrait>,
            Without<GameplayUiRank>,
            Without<GameplayUiEquipmentSlot>,
            Without<GameplayUiSpellSlot>,
            Without<GameplayUiShop>,
            Without<GameplayUiBackpackSlot>,
        ),
    >,
) {
    // Ported from dd.c::dd_shadow.
    for (shadow, mut sprite, mut transform, mut visibility, mut last) in q.iter_mut() {
        if !shadows_enabled {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }

        let Some(tile) = map.tile_at_index(shadow.index) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        let x = shadow.index % TILEX;
        let y = shadow.index / TILEX;
        let draw_order = ((TILEY - 1 - y) * TILEX + x) as f32;

        let xpos = (x as i32) * 32;
        let ypos = (y as i32) * 32;

        let (sprite_id, xoff, yoff) = match shadow.layer {
            ShadowLayer::Object => {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
                continue;
            }
            ShadowLayer::Character => (tile.obj2, tile.obj_xoff, tile.obj_yoff),
        };

        if sprite_id <= 0 || !should_draw_shadow(sprite_id) {
            if sprite_id != last.sprite_id {
                last.sprite_id = sprite_id;
            }
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }

        let Some((sx_i, sy_i)) = legacy_engine::copysprite_screen_pos(
            sprite_id as usize,
            gfx,
            images,
            xpos,
            ypos,
            xoff,
            yoff,
        ) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        let Some(src) = gfx.get_sprite(sprite_id as usize) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        let Some((_xs, ys)) = gfx.get_sprite_tiles_xy(sprite_id as usize) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        // Ported positioning from dd.c::dd_shadow:
        // ry += ys*32 - disp; with disp=14.
        const DISP: i32 = 14;
        let sx_f = sx_i as f32;
        let shadow_sy_f = (sy_i as f32) + (ys * 32 - DISP) as f32;

        if sprite_id == last.sprite_id
            && (sx_f - last.sx).abs() < 0.01
            && (shadow_sy_f - last.sy).abs() < 0.01
        {
            if *visibility != Visibility::Visible {
                *visibility = Visibility::Visible;
            }
            continue;
        }

        last.sprite_id = sprite_id;
        last.sx = sx_f;
        last.sy = shadow_sy_f;

        let mut shadow_sprite = src.clone();
        shadow_sprite.color = Color::srgba(0.0, 0.0, 0.0, 0.5);
        *sprite = shadow_sprite;

        if *visibility != Visibility::Visible {
            *visibility = Visibility::Visible;
        }
        let z = Z_SHADOW_BASE + draw_order * Z_WORLD_STEP;
        transform.translation = screen_to_world(sx_f, shadow_sy_f, z);
        transform.scale = Vec3::new(1.0, 0.25, 1.0);
    }
}

pub(crate) fn update_world_tiles(
    gfx: &GraphicsCache,
    images: &Assets<Image>,
    map: &GameMap,
    player_state: &PlayerState,
    q: &mut Query<
        (
            &TileRender,
            &mut Sprite,
            &mut Transform,
            &mut Visibility,
            &mut LastRender,
        ),
        (
            Without<GameplayWorldRoot>,
            Without<GameplayUiPortrait>,
            Without<GameplayUiRank>,
            Without<GameplayUiEquipmentSlot>,
            Without<GameplayUiSpellSlot>,
            Without<GameplayUiShop>,
            Without<GameplayUiBackpackSlot>,
        ),
    >,
) {
    for (render, mut sprite, mut transform, mut visibility, mut last) in q.iter_mut() {
        let Some(tile) = map.tile_at_index(render.index) else {
            continue;
        };

        let x = render.index % TILEX;
        let y = render.index / TILEX;
        let draw_order = ((TILEY - 1 - y) * TILEX + x) as f32;

        // dd.c uses x*32/y*32 as "map space" inputs to the isometric projection.
        let xpos = (x as i32) * 32;
        let ypos = (y as i32) * 32;

        let (sprite_id, xoff_i, yoff_i) = match render.layer {
            TileLayer::Background => {
                let id = if tile.back != 0 {
                    tile.back
                } else {
                    SPR_EMPTY as i32
                };
                (id, 0, 0)
            }
            TileLayer::Object => {
                let mut id = tile.obj1;

                // engine.c: if (pdata.hide==0 || (map[m].flags&ISITEM) || autohide(x,y)) draw obj1
                // else draw obj1+1 (hide walls/high objects).
                let hide_enabled = player_state.player_data().hide != 0;
                let is_item = (tile.flags & ISITEM) != 0;
                if hide_enabled && id > 0 && !is_item && !legacy_engine::autohide(x, y) {
                    // engine.c mine hack: substitute special sprites for certain mine-wall IDs
                    // when hide is enabled and tile isn't directly in front of the player.
                    let is_mine_wall = id > 16335
                        && id < 16422
                        && !matches!(
                            id,
                            16357 | 16365 | 16373 | 16381 | 16389 | 16397 | 16405 | 16413 | 16421
                        )
                        && !legacy_engine::facing(x, y, player_state.character_info().dir);

                    if is_mine_wall {
                        let tmp2 = if id < 16358 {
                            457
                        } else if id < 16366 {
                            456
                        } else if id < 16374 {
                            455
                        } else if id < 16382 {
                            466
                        } else if id < 16390 {
                            459
                        } else if id < 16398 {
                            458
                        } else if id < 16406 {
                            468
                        } else {
                            467
                        };
                        id = tmp2;
                    } else {
                        id += 1;
                    }
                }

                (id, 0, 0)
            }
            TileLayer::Character => (tile.obj2, tile.obj_xoff, tile.obj_yoff),
        };

        if sprite_id <= 0 {
            if sprite_id != last.sprite_id {
                last.sprite_id = sprite_id;
            }
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }

        // Resolve the final screen pixel position using dd.c's copysprite math.
        let Some((sx_i, sy_i)) = legacy_engine::copysprite_screen_pos(
            sprite_id as usize,
            gfx,
            images,
            xpos,
            ypos,
            xoff_i,
            yoff_i,
        ) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        let (sx_f, sy_f) = (sx_i as f32, sy_i as f32);

        let z = match render.layer {
            TileLayer::Background => Z_BG_BASE + draw_order * Z_WORLD_STEP,
            TileLayer::Object => Z_OBJ_BASE + draw_order * Z_WORLD_STEP,
            TileLayer::Character => Z_CHAR_BASE + draw_order * Z_WORLD_STEP,
        };

        // Match engine.c's per-layer effect flags.
        // Background: map[m].light | (invis?64) | (infra?256) | (uwater?512)
        // Object:      map[m].light | (infra?256) | (uwater?512)
        // Character:   map[m].light | (selected?32) | (stoned?128) | (infra?256) | (uwater?512)
        let mut effect: u32 = tile.light as u32;
        match render.layer {
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
                // engine.c skips object/character pass entirely if INVIS.
                if (tile.flags & INVIS) != 0 {
                    if *visibility != Visibility::Hidden {
                        *visibility = Visibility::Hidden;
                    }
                    continue;
                }
                if (tile.flags & INFRARED) != 0 {
                    effect |= 256;
                }
                if (tile.flags & UWATER) != 0 {
                    effect |= 512;
                }
            }
            TileLayer::Character => {
                // engine.c skips object/character pass entirely if INVIS.
                if (tile.flags & INVIS) != 0 {
                    if *visibility != Visibility::Hidden {
                        *visibility = Visibility::Hidden;
                    }
                    continue;
                }
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

        let tint = dd_effect_tint(effect);

        if sprite_id == last.sprite_id
            && (sx_f - last.sx).abs() < 0.01
            && (sy_f - last.sy).abs() < 0.01
        {
            // Even if the sprite/position didn't change, we must ensure visibility/z stay correct.
            if *visibility != Visibility::Visible {
                *visibility = Visibility::Visible;
            }
            if sprite.color != tint {
                sprite.color = tint;
            }
            continue;
        }

        last.sprite_id = sprite_id;
        last.sx = sx_f;
        last.sy = sy_f;

        let Some(src) = gfx.get_sprite(sprite_id as usize) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        *sprite = src.clone();
        sprite.color = tint;
        if *visibility != Visibility::Visible {
            *visibility = Visibility::Visible;
        }
        transform.translation = screen_to_world(sx_f, sy_f, z);
    }
}

pub(crate) fn update_world_overlays(
    gfx: &GraphicsCache,
    images: &Assets<Image>,
    map: &GameMap,
    q: &mut Query<
        (
            &TileFlagOverlay,
            &mut Sprite,
            &mut Transform,
            &mut Visibility,
            &mut LastRender,
        ),
        (
            Without<GameplayWorldRoot>,
            Without<GameplayUiPortrait>,
            Without<GameplayUiRank>,
            Without<GameplayUiEquipmentSlot>,
            Without<GameplayUiSpellSlot>,
            Without<GameplayUiShop>,
            Without<GameplayUiBackpackSlot>,
        ),
    >,
) {
    // Ported from engine.c: marker/effect sprites on tiles.
    for (ovl, mut sprite, mut transform, mut visibility, mut last) in q.iter_mut() {
        let Some(tile) = map.tile_at_index(ovl.index) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        let x = ovl.index % TILEX;
        let y = ovl.index / TILEX;
        let draw_order = ((TILEY - 1 - y) * TILEX + x) as f32;

        let xpos = (x as i32) * 32;
        let ypos = (y as i32) * 32;

        let mut sprite_id: i32 = 0;
        let mut xoff_i: i32 = 0;
        let mut yoff_i: i32 = 0;
        let mut z_bias: f32 = 0.0;

        match ovl.kind {
            TileFlagOverlayKind::MoveBlock => {
                if (tile.flags2 & MF_MOVEBLOCK) != 0 {
                    sprite_id = 55;
                    z_bias = 0.0000;
                }
            }
            TileFlagOverlayKind::SightBlock => {
                if (tile.flags2 & MF_SIGHTBLOCK) != 0 {
                    sprite_id = 84;
                    z_bias = 0.0001;
                }
            }
            TileFlagOverlayKind::Indoors => {
                if (tile.flags2 & MF_INDOORS) != 0 {
                    sprite_id = 56;
                    z_bias = 0.0002;
                }
            }
            TileFlagOverlayKind::Underwater => {
                if (tile.flags2 & MF_UWATER) != 0 {
                    sprite_id = 75;
                    z_bias = 0.0003;
                }
            }
            TileFlagOverlayKind::NoLag => {
                if (tile.flags2 & MF_NOLAG) != 0 {
                    sprite_id = 57;
                    z_bias = 0.0004;
                }
            }
            TileFlagOverlayKind::NoMonsters => {
                if (tile.flags2 & MF_NOMONST) != 0 {
                    sprite_id = 59;
                    z_bias = 0.0005;
                }
            }
            TileFlagOverlayKind::Bank => {
                if (tile.flags2 & MF_BANK) != 0 {
                    sprite_id = 60;
                    z_bias = 0.0006;
                }
            }
            TileFlagOverlayKind::Tavern => {
                if (tile.flags2 & MF_TAVERN) != 0 {
                    sprite_id = 61;
                    z_bias = 0.0007;
                }
            }
            TileFlagOverlayKind::NoMagic => {
                if (tile.flags2 & MF_NOMAGIC) != 0 {
                    sprite_id = 62;
                    z_bias = 0.0008;
                }
            }
            TileFlagOverlayKind::DeathTrap => {
                if (tile.flags2 & MF_DEATHTRAP) != 0 {
                    sprite_id = 73;
                    z_bias = 0.0009;
                }
            }
            TileFlagOverlayKind::Arena => {
                if (tile.flags2 & MF_ARENA) != 0 {
                    sprite_id = 76;
                    z_bias = 0.0010;
                }
            }
            TileFlagOverlayKind::NoExpire => {
                if (tile.flags2 & MF_NOEXPIRE) != 0 {
                    sprite_id = 82;
                    z_bias = 0.0011;
                }
            }
            TileFlagOverlayKind::UnknownHighBit => {
                if (tile.flags2 & 0x8000_0000) != 0 {
                    sprite_id = 72;
                    z_bias = 0.0012;
                }
            }
            TileFlagOverlayKind::Injured => {
                if (tile.flags & INJURED) != 0 {
                    let mut variant = 0;
                    if (tile.flags & INJURED1) != 0 {
                        variant += 1;
                    }
                    if (tile.flags & INJURED2) != 0 {
                        variant += 2;
                    }
                    sprite_id = 1079 + variant;
                    xoff_i = tile.obj_xoff;
                    yoff_i = tile.obj_yoff;
                    z_bias = 0.0020;
                } else {
                    sprite_id = 0;
                }
            }
            TileFlagOverlayKind::Death => {
                if (tile.flags & DEATH) != 0 {
                    let n = ((tile.flags & DEATH) >> 17) as i32;
                    if n > 0 {
                        sprite_id = 280 + (n - 1);
                        if tile.obj2 != 0 {
                            xoff_i = tile.obj_xoff;
                            yoff_i = tile.obj_yoff;
                        }
                        z_bias = 0.0021;
                    }
                }
            }
            TileFlagOverlayKind::Tomb => {
                if (tile.flags & TOMB) != 0 {
                    let n = ((tile.flags & TOMB) >> 12) as i32;
                    if n > 0 {
                        sprite_id = 240 + (n - 1);
                        z_bias = 0.0022;
                    }
                }
            }
        }

        if sprite_id <= 0 {
            if sprite_id != last.sprite_id {
                last.sprite_id = sprite_id;
            }
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }

        let Some((sx_i, sy_i)) = legacy_engine::copysprite_screen_pos(
            sprite_id as usize,
            gfx,
            images,
            xpos,
            ypos,
            xoff_i,
            yoff_i,
        ) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        let (sx_f, sy_f) = (sx_i as f32, sy_i as f32);
        let z = Z_FX_BASE + draw_order * Z_WORLD_STEP + z_bias;

        if sprite_id == last.sprite_id
            && (sx_f - last.sx).abs() < 0.01
            && (sy_f - last.sy).abs() < 0.01
        {
            if *visibility != Visibility::Visible {
                *visibility = Visibility::Visible;
            }
            continue;
        }

        last.sprite_id = sprite_id;
        last.sx = sx_f;
        last.sy = sy_f;

        let Some(src) = gfx.get_sprite(sprite_id as usize) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        *sprite = src.clone();
        if *visibility != Visibility::Visible {
            *visibility = Visibility::Visible;
        }
        transform.translation = screen_to_world(sx_f, sy_f, z);
    }
}
