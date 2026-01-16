use bevy::prelude::*;

use bevy::sprite::Anchor;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::map::{TILEX, TILEY};
use crate::network::{client_commands::ClientCommand, NetworkRuntime};
use crate::player_state::PlayerState;

use mag_core::constants::{
    SPEEDTAB, SPR_EMPTY, STUNNED, TICKS, WN_ARMS, WN_BELT, WN_BODY, WN_CLOAK, WN_FEET, WN_HEAD,
    WN_LEGS, WN_LHAND, WN_LRING, WN_NECK, WN_RHAND, WN_RRING, XPOS, YPOS,
};

// In the original client, xoff starts with `-176` (to account for UI layout).
// Keeping this makes it easier to compare screenshots while we port rendering.
const MAP_X_SHIFT: f32 = -176.0;

const Z_BG: f32 = 0.0;
const Z_SHADOW: f32 = 50.0;
const Z_OBJ: f32 = 100.0;
const Z_CHAR: f32 = 200.0;
// Must stay within the Camera2d default orthographic near/far (default_2d far is 1000).
const Z_UI: f32 = 900.0;
const Z_UI_PORTRAIT: f32 = 910.0;
const Z_UI_RANK: f32 = 911.0;
const Z_UI_EQUIP: f32 = 920.0;
const Z_UI_SPELLS: f32 = 921.0;
const Z_UI_SHOP_PANEL: f32 = 930.0;
const Z_UI_SHOP_ITEMS: f32 = 931.0;

#[derive(Component)]
pub struct GameplayRenderEntity;

#[derive(Component)]
struct GameplayUiOverlay;

#[derive(Component)]
pub(crate) struct GameplayUiPortrait;

#[derive(Component)]
pub(crate) struct GameplayUiRank;

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiEquipmentSlot {
    worn_index: usize,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiSpellSlot {
    index: usize,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiShop {
    kind: ShopUiKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShopUiKind {
    Panel,
    Slot { index: usize },
}

#[derive(Component)]
pub(crate) struct GameplayShadowEntity;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileShadow {
    index: usize,
    layer: ShadowLayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShadowLayer {
    Object,
    Character,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileRender {
    index: usize,
    layer: TileLayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileLayer {
    Background,
    Object,
    Character,
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct LastRender {
    sprite_id: i32,
    sx: i32,
    sy: i32,
}

#[derive(Default)]
pub(crate) struct EngineClock {
    accumulator: f32,
    ticker: u32,
}

#[derive(Default)]
pub(crate) struct SendOptClock {
    optstep: u8,
    state: u8,
}

#[inline]
fn screen_to_world(sx: f32, sy: f32, z: f32) -> Vec3 {
    // Treat (0,0) as top-left in "screen" pixels like the original client.
    // Convert into Bevy world coordinates (origin centered, +Y up).
    Vec3::new(sx - TARGET_WIDTH * 0.5, TARGET_HEIGHT * 0.5 - sy, z)
}

fn spawn_tile_entity(commands: &mut Commands, gfx: &GraphicsCache, render: TileRender) {
    // Always spawn with a valid sprite handle; we'll swap it during updates.
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    let initial_visibility = match render.layer {
        TileLayer::Background => Visibility::Visible,
        TileLayer::Object | TileLayer::Character => Visibility::Hidden,
    };

    commands.spawn((
        GameplayRenderEntity,
        render,
        LastRender {
            sprite_id: i32::MIN,
            sx: i32::MIN,
            sy: i32::MIN,
        },
        empty.clone(),
        Anchor::TOP_LEFT,
        Transform::default(),
        GlobalTransform::default(),
        initial_visibility,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

fn spawn_shadow_entity(commands: &mut Commands, gfx: &GraphicsCache, shadow: TileShadow) {
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    commands.spawn((
        GameplayRenderEntity,
        GameplayShadowEntity,
        shadow,
        LastRender {
            sprite_id: i32::MIN,
            sx: i32::MIN,
            sy: i32::MIN,
        },
        empty.clone(),
        Anchor::TOP_LEFT,
        Transform::default(),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

fn spawn_ui_overlay(commands: &mut Commands, gfx: &GraphicsCache) {
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

fn spawn_ui_portrait(commands: &mut Commands, gfx: &GraphicsCache) {
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiPortrait,
        LastRender {
            sprite_id: i32::MIN,
            sx: i32::MIN,
            sy: i32::MIN,
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

fn spawn_ui_rank(commands: &mut Commands, gfx: &GraphicsCache) {
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiRank,
        LastRender {
            sprite_id: i32::MIN,
            sx: i32::MIN,
            sy: i32::MIN,
        },
        empty.clone(),
        Anchor::TOP_LEFT,
        Transform::from_translation(screen_to_world(463.0, 38.0, Z_UI_RANK)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

fn spawn_ui_equipment(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches `eng_display_win`: copyspritex(pl.worn[wntab[n]],303+(n%2)*35,2+(n/2)*35,...)
    // We spawn one stable entity per slot and update its sprite each frame.
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    let wntab: [usize; 12] = [
        WN_HEAD, WN_CLOAK, WN_BODY, WN_ARMS, WN_NECK, WN_BELT, WN_RHAND, WN_LHAND, WN_RRING,
        WN_LRING, WN_LEGS, WN_FEET,
    ];

    for (n, worn_index) in wntab.into_iter().enumerate() {
        let sx = 303.0 + (n as f32 % 2.0) * 35.0;
        let sy = 2.0 + ((n / 2) as f32) * 35.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiEquipmentSlot { worn_index },
            LastRender {
                sprite_id: i32::MIN,
                sx: i32::MIN,
                sy: i32::MIN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_EQUIP)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

fn spawn_ui_spells(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches `eng_display_win`: copyspritex(pl.spell[n],374+(n%5)*24,4+(n/5)*24,...)
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    for n in 0..20usize {
        let sx = 374.0 + ((n % 5) as f32) * 24.0;
        let sy = 4.0 + ((n / 5) as f32) * 24.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiSpellSlot { index: n },
            LastRender {
                sprite_id: i32::MIN,
                sx: i32::MIN,
                sy: i32::MIN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_SPELLS)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

fn spawn_ui_shop_window(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches `eng_display_win` shop layout:
    // - copyspritex(92,220,260,0);
    // - for n in 0..62: copyspritex(shop.item[n],222+(n%8)*35,262+(n/8)*35, ...)
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiShop {
            kind: ShopUiKind::Panel,
        },
        LastRender {
            sprite_id: i32::MIN,
            sx: i32::MIN,
            sy: i32::MIN,
        },
        empty.clone(),
        Anchor::TOP_LEFT,
        Transform::from_translation(screen_to_world(220.0, 260.0, Z_UI_SHOP_PANEL)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    for n in 0..62usize {
        let sx = 222.0 + ((n % 8) as f32) * 35.0;
        let sy = 262.0 + ((n / 8) as f32) * 35.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiShop {
                kind: ShopUiKind::Slot { index: n },
            },
            LastRender {
                sprite_id: i32::MIN,
                sx: i32::MIN,
                sy: i32::MIN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_SHOP_ITEMS)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

fn points2rank(v: i32) -> i32 {
    // Ported from client/src/orig/engine.c
    if v < 50 {
        return 0;
    }
    if v < 850 {
        return 1;
    }
    if v < 4_900 {
        return 2;
    }
    if v < 17_700 {
        return 3;
    }
    if v < 48_950 {
        return 4;
    }
    if v < 113_750 {
        return 5;
    }
    if v < 233_800 {
        return 6;
    }
    if v < 438_600 {
        return 7;
    }
    if v < 766_650 {
        return 8;
    }
    if v < 1_266_650 {
        return 9;
    }
    if v < 1_998_700 {
        return 10;
    }
    if v < 3_035_500 {
        return 11;
    }
    if v < 4_463_550 {
        return 12;
    }
    if v < 6_384_350 {
        return 13;
    }
    if v < 8_915_600 {
        return 14;
    }
    if v < 12_192_400 {
        return 15;
    }
    if v < 16_368_450 {
        return 16;
    }
    if v < 21_617_250 {
        return 17;
    }
    if v < 28_133_300 {
        return 18;
    }
    if v < 36_133_300 {
        return 19;
    }
    if v < 49_014_500 {
        return 20;
    }
    if v < 63_000_600 {
        return 21;
    }
    if v < 80_977_100 {
        return 22;
    }
    23
}

fn rank_insignia_sprite(points_tot: i32) -> i32 {
    // engine.c: copyspritex(10+min(20,points2rank(pl.points_tot)),463,54-16,0);
    let rank = points2rank(points_tot).clamp(0, 20);
    10 + rank
}

fn send_opt(net: &NetworkRuntime, player_state: &mut PlayerState, clock: &mut SendOptClock) {
    // Ported from `client/src/orig/engine.c::send_opt()`.
    //
    // Original behavior:
    // - called every few frames while `pdata.changed` is set
    // - sends 18 packets (state 0..17), each containing:
    //   [group:1][offset:1][data:13] as `CL_CMD_SETUSER`
    // - clears `pdata.changed` when done.

    // Throttle like engine.c's `optstep>4` gate.
    clock.optstep = clock.optstep.wrapping_add(1);
    if clock.optstep <= 4 {
        return;
    }
    clock.optstep = 0;

    let pdata_changed = player_state.player_data().changed;
    if pdata_changed == 0 {
        clock.state = 0;
        return;
    }

    let (group, offset, data): (u8, u8, [u8; 13]) = match clock.state {
        // cname: 6 chunks of 13 bytes (0..77)
        0..=5 => {
            let off = clock.state.saturating_mul(13);
            let mut buf = [0u8; 13];
            buf.copy_from_slice(
                &player_state.player_data().cname[off as usize..(off as usize + 13)],
            );
            (0, off, buf)
        }

        // desc: 6 chunks of 13 bytes (0..77)
        6..=11 => {
            let off = (clock.state - 6).saturating_mul(13);
            let mut buf = [0u8; 13];
            buf.copy_from_slice(
                &player_state.player_data().desc[off as usize..(off as usize + 13)],
            );
            (1, off, buf)
        }

        // desc continuation: 6 chunks of 13 bytes starting at 78 (78..155)
        12..=17 => {
            let off = (clock.state - 12).saturating_mul(13);
            let start = 78usize + off as usize;
            let mut buf = [0u8; 13];
            buf.copy_from_slice(&player_state.player_data().desc[start..start + 13]);
            (2, off, buf)
        }

        // Be robust vs repeated option sends across sessions.
        _ => {
            clock.state = 0;
            return;
        }
    };

    let cmd = ClientCommand::new_setuser(group, offset, &data);
    net.send(cmd.to_bytes());

    if clock.state >= 17 {
        player_state.player_data_mut().changed = 0;
        clock.state = 0;
    } else {
        clock.state += 1;
    }
}

fn draw_inventory_ui(_gfx: &GraphicsCache, _player_state: &PlayerState) {
    // TODO: Port eng_display_win() inventory drawing (copyspritex calls around x=220,y=2).
    // TODO: Handle highlight/effects (effect=16 for selection, etc).
}

fn draw_equipment_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    q: &mut Query<(
        &GameplayUiEquipmentSlot,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    let pl = player_state.character_info();

    for (slot, mut sprite, mut visibility, mut last) in q.iter_mut() {
        let sprite_id = pl.worn.get(slot.worn_index).copied().unwrap_or(0);
        if sprite_id <= 0 {
            last.sprite_id = sprite_id;
            *visibility = Visibility::Hidden;
            continue;
        }

        if last.sprite_id != sprite_id {
            if let Some(src) = gfx.get_sprite(sprite_id as usize) {
                *sprite = src.clone();
                last.sprite_id = sprite_id;
                *visibility = Visibility::Visible;
            } else {
                *visibility = Visibility::Hidden;
            }
        } else {
            *visibility = Visibility::Visible;
        }
    }
}

fn draw_active_spells_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    q: &mut Query<(
        &GameplayUiSpellSlot,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    let pl = player_state.character_info();

    for (slot, mut sprite, mut visibility, mut last) in q.iter_mut() {
        let sprite_id = pl.spell.get(slot.index).copied().unwrap_or(0);
        if sprite_id <= 0 {
            last.sprite_id = sprite_id;
            *visibility = Visibility::Hidden;
            continue;
        }

        if last.sprite_id != sprite_id {
            if let Some(src) = gfx.get_sprite(sprite_id as usize) {
                *sprite = src.clone();
                last.sprite_id = sprite_id;
            } else {
                *visibility = Visibility::Hidden;
                continue;
            }
        }

        // dd.c shading (approx): engine.c uses effect = 15 - min(15, active[n]).
        // active==0 => effect=15 => dim; active>=15 => effect=0 => bright.
        let active = pl.active.get(slot.index).copied().unwrap_or(0).max(0) as i32;
        let effect = 15 - active.min(15);
        let shade = 1.0 - (effect as f32 / 15.0) * 0.6;
        sprite.color = Color::srgba(shade, shade, shade, 1.0);
        *visibility = Visibility::Visible;
    }
}

fn draw_shop_window_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    q: &mut Query<(
        &GameplayUiShop,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    let show_shop = player_state.should_show_shop();

    if !show_shop {
        for (_shop_ui, _sprite, mut visibility, mut last) in q.iter_mut() {
            last.sprite_id = 0;
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let shop = player_state.shop_target();

    for (shop_ui, mut sprite, mut visibility, mut last) in q.iter_mut() {
        match shop_ui.kind {
            ShopUiKind::Panel => {
                const SHOP_PANEL_SPRITE: i32 = 92;
                if last.sprite_id != SHOP_PANEL_SPRITE {
                    if let Some(src) = gfx.get_sprite(SHOP_PANEL_SPRITE as usize) {
                        *sprite = src.clone();
                        last.sprite_id = SHOP_PANEL_SPRITE;
                    } else {
                        *visibility = Visibility::Hidden;
                        continue;
                    }
                }
                *visibility = Visibility::Visible;
            }
            ShopUiKind::Slot { index } => {
                let sprite_id = shop.item(index) as i32;
                if sprite_id <= 0 {
                    last.sprite_id = sprite_id;
                    *visibility = Visibility::Hidden;
                    continue;
                }

                if last.sprite_id != sprite_id {
                    if let Some(src) = gfx.get_sprite(sprite_id as usize) {
                        *sprite = src.clone();
                        last.sprite_id = sprite_id;
                    } else {
                        *visibility = Visibility::Hidden;
                        continue;
                    }
                }

                sprite.color = Color::srgba(1.0, 1.0, 1.0, 1.0);
                *visibility = Visibility::Visible;
            }
        }
    }
}

fn sprite_tiles_xy(sprite: &Sprite, images: &Assets<Image>) -> Option<(i32, i32)> {
    let image = images.get(&sprite.image)?;
    let size = image.size();
    let w = (size.x.max(1) as i32).max(1);
    let h = (size.y.max(1) as i32).max(1);

    // dd.c treats sprites as being composed of 32x32 "blocks"; xs/ys are those counts.
    let xs = (w + 31) / 32;
    let ys = (h + 31) / 32;

    Some((xs.max(1), ys.max(1)))
}

#[inline]
fn should_draw_shadow(sprite_id: i32) -> bool {
    // dd.c::dd_shadow: only certain sprite id ranges get shadows.
    (2000..16_336).contains(&sprite_id) || sprite_id > 17_360
}

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

    // Ported from dd.c: copysprite()
    // NOTE: we ignore the negative-coordinate odd-bit adjustments because xpos/ypos
    // are always >= 0 in our current usage (0..TILEX*32).
    let mut rx = (xpos / 2) + (ypos / 2) - (xs * 16) + 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32);
    let mut ry = (xpos / 4) - (ypos / 4) + YPOS - (ys * 32);

    rx += xoff;
    ry += yoff;

    Some((rx, ry))
}

const STATTAB: [i32; 11] = [0, 1, 1, 6, 6, 2, 3, 4, 5, 7, 4];

#[inline]
fn speedo(ch_speed: u8, ctick: usize) -> bool {
    let speed = (ch_speed as usize).min(19);
    SPEEDTAB[speed][ctick.min(19)] != 0
}

fn speedstep(ch_speed: u8, ch_status: u8, d: i32, s: i32, update: bool, ctick: usize) -> i32 {
    let speed = (ch_speed as usize).min(19);
    let hard_step = (ch_status as i32) - d;
    if !update {
        return 32 * hard_step / s;
    }

    let mut z = ctick as i32;
    let mut soft_step = 0i32;
    let mut m = hard_step;

    while m != 0 {
        z -= 1;
        if z < 0 {
            z = 19;
        }
        soft_step += 1;
        if SPEEDTAB[speed][z as usize] != 0 {
            m -= 1;
        }
    }
    loop {
        z -= 1;
        if z < 0 {
            z = 19;
        }
        if SPEEDTAB[speed][z as usize] != 0 {
            break;
        }
        soft_step += 1;
    }

    let mut z = ctick as i32;
    let total_step_start = soft_step;
    let mut total_step = total_step_start;
    let mut m = s - hard_step;

    loop {
        if SPEEDTAB[speed][z as usize] != 0 {
            m -= 1;
        }
        if m < 1 {
            break;
        }
        z += 1;
        if z > 19 {
            z = 0;
        }
        total_step += 1;
    }

    32 * total_step_start / (total_step + 1)
}

#[inline]
fn do_idle(idle_ani: i32, sprite: u16) -> i32 {
    if sprite == 22480 {
        idle_ani
    } else {
        0
    }
}

fn eng_item(it_sprite: i16, it_status: &mut u8, ctick: usize, ticker: u32) -> i32 {
    let base = it_sprite as i32;
    match *it_status {
        0 | 1 => base,

        2 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 3;
            }
            base
        }
        3 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 4;
            }
            base + 2
        }
        4 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 5;
            }
            base + 4
        }
        5 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 2;
            }
            base + 6
        }

        6 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 7;
            }
            base
        }
        7 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 6;
            }
            base + 1
        }

        8 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 9;
            }
            base
        }
        9 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 10;
            }
            base + 1
        }
        10 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 11;
            }
            base + 2
        }
        11 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 12;
            }
            base + 3
        }
        12 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 13;
            }
            base + 4
        }
        13 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 14;
            }
            base + 5
        }
        14 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 15;
            }
            base + 6
        }
        15 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 8;
            }
            base + 7
        }

        16 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 17;
            }
            base
        }
        17 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 18;
            }
            base + 1
        }
        18 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 19;
            }
            base + 2
        }
        19 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 20;
            }
            base + 3
        }
        20 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 16;
            }
            base + 4
        }
        21 => base + ((ticker & 63) as i32),

        _ => base,
    }
}

fn eng_char(tile: &mut crate::types::map::CMapTile, ctick: usize) -> i32 {
    let mut update = true;
    if (tile.flags & STUNNED) != 0 {
        update = false;
    }

    let ch_status = tile.ch_status;
    let base = tile.ch_sprite as i32;

    match ch_status {
        0 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            tile.idle_ani += 1;
            if tile.idle_ani > 7 {
                tile.idle_ani = 0;
            }
            base + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        1 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 8 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        2 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 16 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        3 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 24 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        4 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 32 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        5 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 40 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        6 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 48 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        7 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 56 + do_idle(tile.idle_ani, tile.ch_sprite)
        }

        16..=23 => {
            tile.obj_xoff = -speedstep(tile.ch_speed, tile.ch_status, 16, 8, update, ctick) / 2;
            tile.obj_yoff = speedstep(tile.ch_speed, tile.ch_status, 16, 8, update, ctick) / 4;
            let tmp = base + (tile.ch_status as i32 - 16) + 64;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 23 {
                    16
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        24..=31 => {
            tile.obj_xoff = speedstep(tile.ch_speed, tile.ch_status, 24, 8, update, ctick) / 2;
            tile.obj_yoff = -speedstep(tile.ch_speed, tile.ch_status, 24, 8, update, ctick) / 4;
            let tmp = base + (tile.ch_status as i32 - 24) + 72;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 31 {
                    24
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        32..=39 => {
            tile.obj_xoff = -speedstep(tile.ch_speed, tile.ch_status, 32, 8, update, ctick) / 2;
            tile.obj_yoff = -speedstep(tile.ch_speed, tile.ch_status, 32, 8, update, ctick) / 4;
            let tmp = base + (tile.ch_status as i32 - 32) + 80;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 39 {
                    32
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        40..=47 => {
            tile.obj_xoff = speedstep(tile.ch_speed, tile.ch_status, 40, 8, update, ctick) / 2;
            tile.obj_yoff = speedstep(tile.ch_speed, tile.ch_status, 40, 8, update, ctick) / 4;
            let tmp = base + (tile.ch_status as i32 - 40) + 88;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 47 {
                    40
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }

        48..=59 => {
            tile.obj_xoff = -speedstep(tile.ch_speed, tile.ch_status, 48, 12, update, ctick);
            tile.obj_yoff = 0;
            let tmp = base + ((tile.ch_status as i32 - 48) * 8 / 12) + 96;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 59 {
                    48
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        60..=71 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = -speedstep(tile.ch_speed, tile.ch_status, 60, 12, update, ctick) / 2;
            let tmp = base + ((tile.ch_status as i32 - 60) * 8 / 12) + 104;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 71 {
                    60
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        72..=83 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = speedstep(tile.ch_speed, tile.ch_status, 72, 12, update, ctick) / 2;
            let tmp = base + ((tile.ch_status as i32 - 72) * 8 / 12) + 112;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 83 {
                    72
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        84..=95 => {
            tile.obj_xoff = speedstep(tile.ch_speed, tile.ch_status, 84, 12, update, ctick);
            tile.obj_yoff = 0;
            let tmp = base + ((tile.ch_status as i32 - 84) * 8 / 12) + 120;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 95 {
                    84
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }

        96..=191 => {
            // Turns + misc animations. These all have zero offsets.
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;

            let status = tile.ch_status as i32;
            let (start, base_add, wrap) = if (96..=99).contains(&tile.ch_status) {
                (96, 128, 96)
            } else if (100..=103).contains(&tile.ch_status) {
                (100, 132, 100)
            } else if (104..=107).contains(&tile.ch_status) {
                (104, 136, 104)
            } else if (108..=111).contains(&tile.ch_status) {
                (108, 140, 108)
            } else if (112..=115).contains(&tile.ch_status) {
                (112, 144, 112)
            } else if (116..=119).contains(&tile.ch_status) {
                (116, 148, 116)
            } else if (120..=123).contains(&tile.ch_status) {
                (120, 152, 120)
            } else if (124..=127).contains(&tile.ch_status) {
                (124, 156, 124)
            } else if (128..=131).contains(&tile.ch_status) {
                (128, 160, 128)
            } else if (132..=135).contains(&tile.ch_status) {
                (132, 164, 132)
            } else if (136..=139).contains(&tile.ch_status) {
                (136, 168, 136)
            } else if (140..=143).contains(&tile.ch_status) {
                (140, 172, 140)
            } else if (144..=147).contains(&tile.ch_status) {
                (144, 176, 144)
            } else if (148..=151).contains(&tile.ch_status) {
                (148, 180, 148)
            } else if (152..=155).contains(&tile.ch_status) {
                (152, 184, 152)
            } else if (156..=159).contains(&tile.ch_status) {
                (156, 188, 156)
            } else if (160..=167).contains(&tile.ch_status) {
                (160, 192, 160)
            } else if (168..=175).contains(&tile.ch_status) {
                (168, 200, 168)
            } else if (176..=183).contains(&tile.ch_status) {
                (176, 208, 176)
            } else {
                (184, 216, 184)
            };

            let stat_off = (tile.ch_stat_off as usize).min(STATTAB.len() - 1);
            let stat_add = if (160..=191).contains(&tile.ch_status) {
                STATTAB[stat_off] << 5
            } else {
                0
            };

            let frame = status - start;
            let tmp = base + frame + base_add + stat_add;

            if speedo(tile.ch_speed, ctick) && update {
                // Wrap points: last frame is +3 for turns, +7 for misc.
                let max = if (160..=191).contains(&tile.ch_status) {
                    start + 7
                } else {
                    start + 3
                };
                if tile.ch_status as i32 >= max {
                    tile.ch_status = wrap;
                } else {
                    tile.ch_status = tile.ch_status.saturating_add(1);
                }
            }

            tmp
        }

        _ => base,
    }
}

fn engine_tick(player_state: &mut PlayerState, clock: &mut EngineClock) {
    // Step at 20Hz like the original client.
    clock.ticker = clock.ticker.wrapping_add(1);
    let ctick = (clock.ticker % 20) as usize;

    let map = player_state.map_mut();
    let len = map.len();

    for i in 0..len {
        let Some(tile) = map.tile_at_index_mut(i) else {
            continue;
        };
        tile.back = 0;
        tile.obj1 = 0;
        tile.obj2 = 0;
        tile.ovl_xoff = 0;
        tile.ovl_yoff = 0;
    }

    for i in 0..len {
        let Some(tile) = map.tile_at_index_mut(i) else {
            continue;
        };

        tile.back = tile.ba_sprite as i32;

        if tile.it_sprite != 0 {
            let sprite = eng_item(tile.it_sprite, &mut tile.it_status, ctick, clock.ticker);
            tile.obj1 = sprite;
        }

        if tile.ch_sprite != 0 {
            let sprite = eng_char(tile, ctick);
            tile.obj2 = sprite;
        }
    }
}

pub(crate) fn setup_gameplay(
    mut commands: Commands,
    gfx: Res<GraphicsCache>,
    player_state: Res<PlayerState>,
    existing_render: Query<Entity, With<GameplayRenderEntity>>,
) {
    log::debug!("setup_gameplay - start");

    // Clear any previous gameplay sprites (re-entering gameplay, etc.)
    for e in &existing_render {
        commands.entity(e).despawn();
    }

    if !gfx.is_initialized() {
        log::warn!("Gameplay entered before GraphicsCache initialized");
        return;
    }

    let map = player_state.map();

    // Spawn a stable set of entities once; `run_gameplay` updates them.
    for index in 0..map.len() {
        // Shadows (dd.c::dd_shadow), rendered between background and objects/chars.
        spawn_shadow_entity(
            &mut commands,
            &gfx,
            TileShadow {
                index,
                layer: ShadowLayer::Object,
            },
        );
        spawn_shadow_entity(
            &mut commands,
            &gfx,
            TileShadow {
                index,
                layer: ShadowLayer::Character,
            },
        );

        spawn_tile_entity(
            &mut commands,
            &gfx,
            TileRender {
                index,
                layer: TileLayer::Background,
            },
        );
        spawn_tile_entity(
            &mut commands,
            &gfx,
            TileRender {
                index,
                layer: TileLayer::Object,
            },
        );
        spawn_tile_entity(
            &mut commands,
            &gfx,
            TileRender {
                index,
                layer: TileLayer::Character,
            },
        );
    }

    // UI frame / background (sprite 00001.png)
    spawn_ui_overlay(&mut commands, &gfx);
    // Player portrait + rank badge
    spawn_ui_portrait(&mut commands, &gfx);
    spawn_ui_rank(&mut commands, &gfx);

    // Equipment slots + active spells
    spawn_ui_equipment(&mut commands, &gfx);
    spawn_ui_spells(&mut commands, &gfx);

    // Shop window (panel + item slots)
    spawn_ui_shop_window(&mut commands, &gfx);

    log::debug!("setup_gameplay - end");
}

pub(crate) fn run_gameplay(
    time: Res<Time>,
    net: Res<NetworkRuntime>,
    gfx: Res<GraphicsCache>,
    images: Res<Assets<Image>>,
    mut player_state: ResMut<PlayerState>,
    mut clock: Local<EngineClock>,
    mut opt_clock: Local<SendOptClock>,
    mut q: ParamSet<(
        Query<(
            &TileShadow,
            &mut Sprite,
            &mut Transform,
            &mut Visibility,
            &mut LastRender,
        )>,
        Query<(
            &TileRender,
            &mut Sprite,
            &mut Transform,
            &mut Visibility,
            &mut LastRender,
        )>,
        Query<(&mut Sprite, &mut Visibility, &mut LastRender), With<GameplayUiPortrait>>,
        Query<(&mut Sprite, &mut Visibility, &mut LastRender), With<GameplayUiRank>>,
        Query<(
            &GameplayUiEquipmentSlot,
            &mut Sprite,
            &mut Visibility,
            &mut LastRender,
        )>,
        Query<(
            &GameplayUiSpellSlot,
            &mut Sprite,
            &mut Visibility,
            &mut LastRender,
        )>,
        Query<(
            &GameplayUiShop,
            &mut Sprite,
            &mut Visibility,
            &mut LastRender,
        )>,
    )>,
) {
    if !gfx.is_initialized() {
        return;
    }

    // Ensure we have computed `back/obj1/obj2` at least once so gameplay doesn't start
    // with a fully empty screen until the first 20Hz tick.
    if clock.ticker == 0 {
        engine_tick(&mut player_state, &mut clock);
    }

    // Fixed-step the original engine_tick at 20Hz.
    let tick_hz = TICKS as f32;
    let tick_dt = 1.0 / tick_hz;
    clock.accumulator += time.delta_secs();
    while clock.accumulator >= tick_dt {
        engine_tick(&mut player_state, &mut clock);
        clock.accumulator -= tick_dt;
    }

    // Ported options transfer behavior (engine.c::send_opt).
    send_opt(&net, &mut player_state, &mut opt_clock);

    let map = player_state.map();

    let shadows_enabled = player_state.player_data().are_shadows_enabled != 0;

    // Match original engine.c: xoff/yoff are based on the center tile's obj offsets.
    let (xoff, yoff) = map
        .tile_at_xy(TILEX / 2, TILEY / 2)
        .map(|center| {
            (
                -(center.obj_xoff as f32) + MAP_X_SHIFT,
                -(center.obj_yoff as f32),
            )
        })
        .unwrap_or((MAP_X_SHIFT, 0.0));

    // UI: player portrait sprite is the center tile's obj2 (engine.c passes plr_sprite)
    let base_portrait_sprite_id = map
        .tile_at_xy(TILEX / 2, TILEY / 2)
        .map(|t| t.obj2)
        .unwrap_or(0);

    let base_rank_sprite_id = rank_insignia_sprite(player_state.character_info().points_tot);

    // Match engine.c: when shop is open, the right-side portrait/rank reflect the shop target.
    let mut ui_portrait_sprite_id = base_portrait_sprite_id;
    let mut ui_rank_sprite_id = base_rank_sprite_id;
    if player_state.should_show_shop() {
        let shop = player_state.shop_target();
        if shop.sprite() != 0 {
            ui_portrait_sprite_id = shop.sprite() as i32;
        }
        let shop_points = shop.points().min(i32::MAX as u32) as i32;
        ui_rank_sprite_id = rank_insignia_sprite(shop_points);
    }

    // Update shadows (dd.c::dd_shadow)
    for (shadow, mut sprite, mut transform, mut visibility, mut last) in &mut q.p0() {
        if !shadows_enabled {
            *visibility = Visibility::Hidden;
            continue;
        }

        let Some(tile) = map.tile_at_index(shadow.index) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let x = shadow.index % TILEX;
        let y = shadow.index / TILEX;
        let draw_order = ((TILEY - 1 - y) * TILEX + x) as f32;
        let z = Z_SHADOW + draw_order * 0.01;

        let xpos = (x as i32) * 32;
        let ypos = (y as i32) * 32;

        let (sprite_id, xoff_total, yoff_total) = match shadow.layer {
            ShadowLayer::Object => (tile.obj1, xoff.round() as i32, yoff.round() as i32),
            ShadowLayer::Character => (
                tile.obj2,
                xoff.round() as i32 + tile.obj_xoff,
                yoff.round() as i32 + tile.obj_yoff,
            ),
        };

        if sprite_id <= 0 || !should_draw_shadow(sprite_id) {
            if sprite_id != last.sprite_id {
                last.sprite_id = sprite_id;
            }
            *visibility = Visibility::Hidden;
            continue;
        }

        let Some((sx_i, sy_i)) = copysprite_screen_pos(
            sprite_id as usize,
            &gfx,
            &images,
            xpos,
            ypos,
            xoff_total,
            yoff_total,
        ) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let Some(src) = gfx.get_sprite(sprite_id as usize) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some((_xs, ys)) = sprite_tiles_xy(src, &images) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        // Ported positioning from dd.c::dd_shadow:
        // ry += ys*32 - disp; with disp=14.
        const DISP: i32 = 14;
        let shadow_sy = sy_i + ys * 32 - DISP;

        if sprite_id == last.sprite_id && sx_i == last.sx && shadow_sy == last.sy {
            // Ensure our squash stays applied even when sprite id/pos unchanged.
            transform.scale = Vec3::new(1.0, 0.25, 1.0);
            *visibility = Visibility::Visible;
            continue;
        }

        last.sprite_id = sprite_id;
        last.sx = sx_i;
        last.sy = shadow_sy;

        let mut shadow_sprite = src.clone();
        shadow_sprite.color = Color::srgba(0.0, 0.0, 0.0, 0.5);
        *sprite = shadow_sprite;

        *visibility = Visibility::Visible;
        transform.translation = screen_to_world(sx_i as f32, shadow_sy as f32, z);
        transform.scale = Vec3::new(1.0, 0.25, 1.0);
    }

    for (render, mut sprite, mut transform, mut visibility, mut last) in &mut q.p1() {
        let Some(tile) = map.tile_at_index(render.index) else {
            continue;
        };

        let x = render.index % TILEX;
        let y = render.index / TILEX;

        let draw_order = ((TILEY - 1 - y) * TILEX + x) as f32;
        let z = match render.layer {
            TileLayer::Background => Z_BG,
            TileLayer::Object => Z_OBJ,
            TileLayer::Character => Z_CHAR,
        } + draw_order * 0.01;

        // dd.c uses x*32/y*32 as "map space" inputs to the isometric projection.
        let xpos = (x as i32) * 32;
        let ypos = (y as i32) * 32;

        let (sprite_id, xoff_total, yoff_total) = match render.layer {
            TileLayer::Background => {
                let id = if tile.back != 0 {
                    tile.back
                } else {
                    SPR_EMPTY as i32
                };
                (id, xoff.round() as i32, yoff.round() as i32)
            }
            TileLayer::Object => (tile.obj1, xoff.round() as i32, yoff.round() as i32),
            TileLayer::Character => (
                tile.obj2,
                xoff.round() as i32 + tile.obj_xoff,
                yoff.round() as i32 + tile.obj_yoff,
            ),
        };

        if sprite_id <= 0 {
            if sprite_id != last.sprite_id {
                last.sprite_id = sprite_id;
            }
            *visibility = Visibility::Hidden;
            continue;
        }

        // Resolve the final screen pixel position using dd.c's copysprite math.
        let Some((sx_i, sy_i)) = copysprite_screen_pos(
            sprite_id as usize,
            &gfx,
            &images,
            xpos,
            ypos,
            xoff_total,
            yoff_total,
        ) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        if sprite_id == last.sprite_id && sx_i == last.sx && sy_i == last.sy {
            continue;
        }

        last.sprite_id = sprite_id;
        last.sx = sx_i;
        last.sy = sy_i;

        let Some(src) = gfx.get_sprite(sprite_id as usize) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        *sprite = src.clone();
        *visibility = Visibility::Visible;
        transform.translation = screen_to_world(sx_i as f32, sy_i as f32, z);
    }

    // Update UI portrait
    if let Some((mut sprite, mut visibility, mut last)) = q.p2().iter_mut().next() {
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

    // Update UI rank badge
    if let Some((mut sprite, mut visibility, mut last)) = q.p3().iter_mut().next() {
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

    // UI stubs (no implementation yet)
    draw_inventory_ui(&gfx, &player_state);
    draw_equipment_ui(&gfx, &player_state, &mut q.p4());
    draw_active_spells_ui(&gfx, &player_state, &mut q.p5());
    draw_shop_window_ui(&gfx, &player_state, &mut q.p6());
}
