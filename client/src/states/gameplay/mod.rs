use bevy::prelude::*;

use bevy::asset::RenderAssetUsages;
use bevy::ecs::query::Without;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

use std::cmp::Ordering;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tracing::info;

mod components;
mod layout;
mod minimap;
mod resources;
pub mod ui;
mod world_render;

use components::*;
use layout::*;
use minimap::{spawn_ui_minimap, update_minimap};
use resources::*;
use world_render::*;

pub(crate) use components::BitmapText;
pub use components::GameplayRenderEntity;
pub(crate) use minimap::MiniMapState;
pub(crate) use resources::{GameplayCursorType, GameplayCursorTypeState};
pub(crate) use world_render::{TileLayer, TileRender};

pub(crate) use ui::text::run_gameplay_text_ui;

#[inline]
/// Reads an environment variable as a boolean feature flag.
///
/// Accepts common false-y values like "0", "false", and "no" (case-insensitive).
fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no")
        })
        .unwrap_or(false)
}

#[inline]
/// Returns whether gameplay rendering profiling is enabled.
///
/// This uses a `OnceLock` to read and cache the `MAG_PROFILE_RENDERING` env var once.
fn profile_rendering_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| env_flag("MAG_PROFILE_RENDERING"))
}

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::font_cache::{FontCache, BITMAP_GLYPH_W};
use crate::gfx_cache::GraphicsCache;
use crate::map::{TILEX, TILEY};
use crate::network::{client_commands::ClientCommand, NetworkRuntime};
use crate::player_state::PlayerState;

use mag_core::constants::{
    DEATH, INFRARED, INJURED, INJURED1, INJURED2, INVIS, ISITEM, MF_ARENA, MF_BANK, MF_DEATHTRAP,
    MF_INDOORS, MF_MOVEBLOCK, MF_NOEXPIRE, MF_NOLAG, MF_NOMAGIC, MF_NOMONST, MF_SIGHTBLOCK,
    MF_TAVERN, MF_UWATER, PL_ARMS, PL_BELT, PL_BODY, PL_CLOAK, PL_FEET, PL_HEAD, PL_LEGS, PL_NECK,
    PL_RING, PL_SHIELD, PL_TWOHAND, PL_WEAPON, SPEEDTAB, SPR_EMPTY, STONED, STUNNED, TOMB, UWATER,
    WN_ARMS, WN_BELT, WN_BODY, WN_CLOAK, WN_FEET, WN_HEAD, WN_LEGS, WN_LHAND, WN_LRING, WN_NECK,
    WN_RHAND, WN_RRING, XPOS, YPOS,
};
use mag_core::types::skilltab::{
    get_skill_desc, get_skill_name, get_skill_nr, get_skill_sortkey, MAX_SKILLS,
};

const HIGH_VAL: i32 = i32::MAX;

#[inline]
/// Computes the stat-point cost to raise an attribute to `v`.
///
/// Mirrors the original client's cubic cost formula and returns `HIGH_VAL` if the requested
/// value is at/above the maximum.
fn attrib_needed(pl: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
    let max_v = pl.attrib[n][2] as i32;
    if v >= max_v {
        return HIGH_VAL;
    }
    let diff = pl.attrib[n][3] as i32;
    let v64 = v as i64;
    ((v64 * v64 * v64) * (diff as i64) / 20).clamp(0, i32::MAX as i64) as i32
}

#[inline]
/// Computes the stat-point cost to raise a skill to `v`.
///
/// Mirrors the original client's cost formula and returns `HIGH_VAL` if the requested value is
/// at/above the maximum.
fn skill_needed(pl: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
    let max_v = pl.skill[n][2] as i32;
    if v >= max_v {
        return HIGH_VAL;
    }
    let diff = pl.skill[n][3] as i32;
    let v64 = v as i64;
    let cubic = ((v64 * v64 * v64) * (diff as i64) / 40).clamp(0, i32::MAX as i64) as i32;
    v.max(cubic)
}

#[inline]
/// Computes the stat-point cost to raise max hitpoints to `v`.
///
/// Returns `HIGH_VAL` if `v` is at/above the maximum.
fn hp_needed(pl: &mag_core::types::ClientPlayer, v: i32) -> i32 {
    if v >= pl.hp[2] as i32 {
        return HIGH_VAL;
    }
    (v as i64 * pl.hp[3] as i64).clamp(0, i32::MAX as i64) as i32
}

#[inline]
/// Computes the stat-point cost to raise max endurance to `v`.
///
/// Returns `HIGH_VAL` if `v` is at/above the maximum.
fn end_needed(pl: &mag_core::types::ClientPlayer, v: i32) -> i32 {
    if v >= pl.end[2] as i32 {
        return HIGH_VAL;
    }
    (v as i64 * pl.end[3] as i64 / 2).clamp(0, i32::MAX as i64) as i32
}

#[inline]
/// Computes the stat-point cost to raise max mana to `v`.
///
/// Returns `HIGH_VAL` if `v` is at/above the maximum.
fn mana_needed(pl: &mag_core::types::ClientPlayer, v: i32) -> i32 {
    if v >= pl.mana[2] as i32 {
        return HIGH_VAL;
    }
    (v as i64 * pl.mana[3] as i64).clamp(0, i32::MAX as i64) as i32
}

/// Produces a stable skill ordering for the gameplay UI.
///
/// Sorts unused skills last, learned skills before unlearned, then by the legacy sort key/name.
fn build_sorted_skills(pl: &mag_core::types::ClientPlayer) -> Vec<usize> {
    let mut sorted_skills: Vec<usize> = (0..MAX_SKILLS).collect();
    sorted_skills.sort_by(|&a, &b| {
        let a_unused = get_skill_sortkey(a) == 'Z' || get_skill_name(a).is_empty();
        let b_unused = get_skill_sortkey(b) == 'Z' || get_skill_name(b).is_empty();
        if a_unused != b_unused {
            return if a_unused {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }

        let a_learned = pl.skill[a][0] != 0;
        let b_learned = pl.skill[b][0] != 0;
        if a_learned != b_learned {
            return if a_learned {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        let a_key = get_skill_sortkey(a);
        let b_key = get_skill_sortkey(b);
        if a_key != b_key {
            return a_key.cmp(&b_key);
        }

        get_skill_name(a).cmp(get_skill_name(b))
    });
    sorted_skills
}

#[inline]
/// Returns the HUD bar colors (background + fill colors).
fn ui_bar_colors() -> (Color, Color, Color) {
    // The original dd_showbar does a darkening blend against the framebuffer.
    // For our sprite-rect bars we want the classic readable look: bright green/red
    // over a blue background, with depletion revealing the blue.
    let blue = Color::srgb(0.0, 0.0, 0.90);
    let green = Color::srgb(0.0, 0.85, 0.0);
    let red = Color::srgb(0.90, 0.0, 0.0);
    (blue, green, red)
}

#[inline]
/// Returns the xbuttons hotbar slot index at the given gameplay-space cursor position.
///
/// The hotbar is a 3x4 grid (12 slots) laid out to match the legacy UI.
fn xbuttons_slot_at(x: f32, y: f32) -> Option<usize> {
    let y_rows = [XBUTTONS_Y_ROW1, XBUTTONS_Y_ROW2, XBUTTONS_Y_ROW3];
    for (row, &y0) in y_rows.iter().enumerate() {
        for col in 0..4 {
            let x0 = XBUTTONS_X + (col as f32) * XBUTTONS_STEP_X;
            if (x0..=(x0 + XBUTTONS_BUTTON_W)).contains(&x)
                && (y0..=(y0 + XBUTTONS_BUTTON_H)).contains(&y)
            {
                return Some(row * 4 + col);
            }
        }
    }
    None
}

#[inline]
/// Truncates a skill label for display in the xbuttons hotbar.
fn xbuttons_truncate_label(name: &str) -> String {
    name.chars().take(7).collect()
}

#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct GameplayDebugSettings {
    /// Enables tile flag overlay entities (MoveBlock/Indoors/etc).
    /// These are useful for debugging but expensive if spawned for every tile.
    pub(crate) tile_flag_overlays: bool,
}

impl Default for GameplayDebugSettings {
    /// Reads debug settings from environment variables.
    fn default() -> Self {
        // Set `MAG_DEBUG_TILE_OVERLAYS=1` to enable.
        let enabled = env_flag("MAG_DEBUG_TILE_OVERLAYS");

        Self {
            tile_flag_overlays: enabled,
        }
    }
}

/// Requests an exit from the game, mirroring the legacy client's double-confirm behavior.
fn cmd_exit(
    exit_state: &mut GameplayExitState,
    net: &NetworkRuntime,
    player_state: &mut PlayerState,
) {
    // Ported from orig/engine.c::cmd_exit.
    if !exit_state.firstquit {
        player_state.tlog(0, " ");
        player_state.tlog(
            0,
            "Leaving the game without entering a tavern will make you lose money and possibly life. Click again if you still want to leave the hard way.",
        );
        player_state.tlog(
            0,
            "A tavern is located west of the Temple of Skua (the starting point).",
        );
        exit_state.firstquit = true;
        return;
    }

    if exit_state.wantquit {
        return;
    }

    net.send(ClientCommand::new_exit().to_bytes());
    exit_state.wantquit = true;
    player_state.tlog(0, " ");
    player_state.tlog(
        0,
        "Exit request acknowledged. Please wait for server to enter exit state.",
    );
}

/// Spawns HUD labels used in gameplay (HP/End/Mana, stats, skills, etc.).
fn spawn_ui_hud_labels(commands: &mut Commands) {
    // Hitpoints label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiHitpointsLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(HUD_HITPOINTS_X, HUD_HITPOINTS_Y, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Endurance label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiEnduranceLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(HUD_ENDURANCE_X, HUD_ENDURANCE_Y, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Mana label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiManaLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(HUD_MANA_X, HUD_MANA_Y, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Money label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiMoneyLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(HUD_MONEY_X, HUD_MONEY_Y, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Update label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiUpdateLabel,
        BitmapText {
            text: String::from("Update"),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_UPDATE_LABEL_X,
            HUD_UPDATE_LABEL_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Update value
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiUpdateValue,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_UPDATE_VALUE_X,
            HUD_UPDATE_VALUE_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Weapon value label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiWeaponValueLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_WEAPON_VALUE_X,
            HUD_WEAPON_VALUE_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Armor value label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiArmorValueLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_ARMOR_VALUE_X,
            HUD_ARMOR_VALUE_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Experience label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiExperienceLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_EXPERIENCE_X,
            HUD_EXPERIENCE_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Attribute labels (5 attributes: Braveness, Willpower, Intuition, Agility, Strength)
    for i in 0..5 {
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiAttributeLabel { attrib_index: i },
            BitmapText {
                text: String::new(),
                color: Color::WHITE,
                font: UI_BITMAP_FONT,
            },
            Transform::from_translation(screen_to_world(
                HUD_ATTRIBUTES_X,
                HUD_ATTRIBUTES_Y_START + (i as f32) * HUD_ATTRIBUTES_SPACING,
                Z_UI_TEXT,
            )),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));

        // Attribute +/- and cost columns (engine.c uses x=136/150/162).
        let y = HUD_ATTRIBUTES_Y_START + (i as f32) * HUD_ATTRIBUTES_SPACING;
        for (col, x) in [
            (GameplayUiRaiseStatColumn::Plus, HUD_RAISE_STATS_PLUS_X),
            (GameplayUiRaiseStatColumn::Minus, HUD_RAISE_STATS_MINUS_X),
            (GameplayUiRaiseStatColumn::Cost, HUD_RAISE_STATS_COST_X),
        ] {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiAttributeAuxText {
                    attrib_index: i,
                    col,
                },
                BitmapText {
                    text: String::new(),
                    color: Color::WHITE,
                    font: UI_BITMAP_FONT,
                },
                Transform::from_translation(screen_to_world(x, y, Z_UI_TEXT)),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        }
    }

    // Raise-stat rows (Hitpoints / Endurance / Mana) with +/- markers and exp cost.
    // Matches engine.c lines at y=74,88,102 and columns x=5,136,150,162.
    let raise_rows = [
        (GameplayUiRaiseStat::Hitpoints, HUD_RAISE_HP_Y),
        (GameplayUiRaiseStat::Endurance, HUD_RAISE_END_Y),
        (GameplayUiRaiseStat::Mana, HUD_RAISE_MANA_Y),
    ];
    for (stat, y) in raise_rows {
        for (col, x) in [
            (GameplayUiRaiseStatColumn::Value, HUD_RAISE_STATS_X),
            (GameplayUiRaiseStatColumn::Plus, HUD_RAISE_STATS_PLUS_X),
            (GameplayUiRaiseStatColumn::Minus, HUD_RAISE_STATS_MINUS_X),
            (GameplayUiRaiseStatColumn::Cost, HUD_RAISE_STATS_COST_X),
        ] {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiRaiseStatText { stat, col },
                BitmapText {
                    text: String::new(),
                    color: Color::WHITE,
                    font: UI_BITMAP_FONT,
                },
                Transform::from_translation(screen_to_world(x, y, Z_UI_TEXT)),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        }
    }

    // Skill labels (10 skills visible at a time)
    for i in 0..10 {
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiSkillLabel { skill_index: i },
            BitmapText {
                text: String::new(),
                color: Color::WHITE,
                font: UI_BITMAP_FONT,
            },
            Transform::from_translation(screen_to_world(
                HUD_SKILLS_X,
                HUD_SKILLS_Y_START + (i as f32) * HUD_SKILLS_SPACING,
                Z_UI_TEXT,
            )),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));

        // Skill +/- and cost columns (engine.c uses x=136/150/162 at y=116+n*14).
        let y = HUD_SKILLS_Y_START + (i as f32) * HUD_SKILLS_SPACING;
        for (col, x) in [
            (GameplayUiRaiseStatColumn::Plus, HUD_RAISE_STATS_PLUS_X),
            (GameplayUiRaiseStatColumn::Minus, HUD_RAISE_STATS_MINUS_X),
            (GameplayUiRaiseStatColumn::Cost, HUD_RAISE_STATS_COST_X),
        ] {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiSkillAuxText { row: i, col },
                BitmapText {
                    text: String::new(),
                    color: Color::WHITE,
                    font: UI_BITMAP_FONT,
                },
                Transform::from_translation(screen_to_world(x, y, Z_UI_TEXT)),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        }
    }

    // Top-center selected/player name (engine.c y=28).
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiTopSelectedNameLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_TOP_NAME_AREA_X,
            HUD_TOP_NAME_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // (No "Available Points" label; original client doesn't show it here.)

    // Shop price labels (engine.c):
    // - dd_xputtext(225,549,1,"Sell: %dG %dS", ...)
    // - dd_xputtext(225,559,1,"Buy:  %dG %dS", ...)
    // These are only visible while the shop UI is open.
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiShopSellPriceLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(225.0, 549.0, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiShopBuyPriceLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(225.0, 559.0, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Skill hotbar (xbuttons) labels (engine.c: dd_xputtext(610+(n%4)*49,508+(n/4)*15,...)).
    for n in 0..12 {
        let col = n % 4;
        let row = n / 4;
        let x = XBUTTONS_LABEL_X + (col as f32) * XBUTTONS_STEP_X;
        let y = XBUTTONS_LABEL_Y + (row as f32) * XBUTTONS_LABEL_STEP_Y;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiXButtonLabel { index: n },
            BitmapText {
                text: String::new(),
                color: Color::WHITE,
                font: UI_BITMAP_FONT,
            },
            Transform::from_translation(screen_to_world(x, y, Z_UI_TEXT)),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Updates shop sell/buy price labels based on the currently hovered shop slot.
///
/// When the shop UI is closed, hides the price labels.
pub(crate) fn run_gameplay_update_shop_price_labels(
    player_state: Res<PlayerState>,
    shop_hover: Res<GameplayShopHoverState>,
    mut q_sell: Query<
        (&mut BitmapText, &mut Visibility),
        (
            With<GameplayUiShopSellPriceLabel>,
            Without<GameplayUiShopBuyPriceLabel>,
        ),
    >,
    mut q_buy: Query<
        (&mut BitmapText, &mut Visibility),
        (
            With<GameplayUiShopBuyPriceLabel>,
            Without<GameplayUiShopSellPriceLabel>,
        ),
    >,
) {
    if !player_state.should_show_shop() {
        for (mut text, mut vis) in &mut q_sell {
            if !text.text.is_empty() {
                text.text.clear();
            }
            *vis = Visibility::Hidden;
        }
        for (mut text, mut vis) in &mut q_buy {
            if !text.text.is_empty() {
                text.text.clear();
            }
            *vis = Visibility::Hidden;
        }
        return;
    }

    let shop = player_state.shop_target();

    // Sell price: only when hovering an item slot with a non-zero price.
    let sell_text = shop_hover
        .slot
        .and_then(|idx| {
            let price = shop.price(idx);
            if price == 0 {
                return None;
            }
            Some(format!("Sell: {}G {}S", price / 100, price % 100))
        })
        .unwrap_or_default();

    for (mut text, mut vis) in &mut q_sell {
        if text.text != sell_text {
            text.text = sell_text.clone();
        }
        *vis = if text.text.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }

    // Buy price: when carrying an item and shop has a non-zero pl_price.
    let pl = player_state.character_info();
    let buy_price = if pl.citem != 0 { shop.pl_price() } else { 0 };
    let buy_text = if buy_price != 0 {
        // Keep the original spacing ("Buy:  ") for alignment with the classic UI.
        format!("Buy:  {}G {}S", buy_price / 100, buy_price % 100)
    } else {
        String::new()
    };

    for (mut text, mut vis) in &mut q_buy {
        if text.text != buy_text {
            text.text = buy_text.clone();
        }
        *vis = if text.text.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

/// Spawns the orange outline boxes used for gameplay toggles and mode selection.
fn spawn_ui_toggle_boxes(commands: &mut Commands, image_assets: &mut Assets<Image>) {
    // A single white pixel stretched + tinted to match dd_showbox's outline.
    let pixel = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let pixel_handle = image_assets.add(pixel);

    let orange = Color::srgb(1.0, 0.55, 0.0);
    let z = Z_UI_TEXT - 2.0;

    let spawn_box = |commands: &mut Commands, kind: GameplayToggleBoxKind, sx: f32, sy: f32| {
        let w = TOGGLE_BOX_W;
        let h = TOGGLE_BOX_H;
        let t = 1.0;

        let spawn_seg = |commands: &mut Commands, ox: f32, oy: f32, sw: f32, sh: f32| {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiToggleBox { kind },
                Sprite {
                    image: pixel_handle.clone(),
                    color: orange,
                    custom_size: Some(Vec2::new(sw.max(0.0), sh.max(0.0))),
                    ..default()
                },
                Anchor::TOP_LEFT,
                Transform::from_translation(screen_to_world(sx + ox, sy + oy, z)),
                GlobalTransform::default(),
                Visibility::Hidden,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        };

        // Top / bottom
        spawn_seg(commands, 0.0, 0.0, w, t);
        spawn_seg(commands, 0.0, h - t, w, t);
        // Left / right
        spawn_seg(commands, 0.0, 0.0, t, h);
        spawn_seg(commands, w - t, 0.0, t, h);
    };

    spawn_box(
        commands,
        GameplayToggleBoxKind::ShowProz,
        TOGGLE_SHOW_PROZ_X,
        TOGGLE_SHOW_PROZ_Y,
    );
    spawn_box(
        commands,
        GameplayToggleBoxKind::ShowNames,
        TOGGLE_SHOW_NAMES_X,
        TOGGLE_SHOW_NAMES_Y,
    );
    spawn_box(
        commands,
        GameplayToggleBoxKind::Hide,
        TOGGLE_HIDE_X,
        TOGGLE_HIDE_Y,
    );

    // Mode selection boxes (orig/engine.c: dd_showbox based on pl.mode).
    let spawn_mode_box = |commands: &mut Commands, mode: i32, sx: f32, sy: f32| {
        let w = MODE_BOX_W;
        let h = MODE_BOX_H;
        let t = 1.0;

        let spawn_seg = |commands: &mut Commands, ox: f32, oy: f32, sw: f32, sh: f32| {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiModeBox { mode },
                Sprite {
                    image: pixel_handle.clone(),
                    color: orange,
                    custom_size: Some(Vec2::new(sw.max(0.0), sh.max(0.0))),
                    ..default()
                },
                Anchor::TOP_LEFT,
                Transform::from_translation(screen_to_world(sx + ox, sy + oy, z)),
                GlobalTransform::default(),
                Visibility::Hidden,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        };

        spawn_seg(commands, 0.0, 0.0, w, t);
        spawn_seg(commands, 0.0, h - t, w, t);
        spawn_seg(commands, 0.0, 0.0, t, h);
        spawn_seg(commands, w - t, 0.0, t, h);
    };

    spawn_mode_box(commands, 2, MODE_FAST_X, MODE_BOX_Y);
    spawn_mode_box(commands, 1, MODE_NORMAL_X, MODE_BOX_Y);
    spawn_mode_box(commands, 0, MODE_SLOW_X, MODE_BOX_Y);
}

/// Spawns the HUD stat bars (background + fill rectangles).
fn spawn_ui_stat_bars(commands: &mut Commands, image_assets: &mut Assets<Image>) {
    // A single white pixel stretched + tinted for dd_showbar-like rectangles.
    let pixel = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let pixel_handle = image_assets.add(pixel);

    let (blue, green, _red) = ui_bar_colors();

    let z_bg = Z_UI_TEXT - 6.0;
    let z_fg = Z_UI_TEXT - 5.9;

    let spawn_bar = |commands: &mut Commands,
                     kind: GameplayUiBarKind,
                     layer: GameplayUiBarLayer,
                     x: f32,
                     y: f32,
                     z: f32,
                     color: Color| {
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiBar { kind, layer },
            Sprite {
                image: pixel_handle.clone(),
                color,
                custom_size: Some(Vec2::new(0.0, HUD_BAR_H)),
                ..default()
            },
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(x, y, z)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    };

    for (kind, y) in [
        (GameplayUiBarKind::Hitpoints, HUD_HP_BAR_Y),
        (GameplayUiBarKind::Endurance, HUD_END_BAR_Y),
        (GameplayUiBarKind::Mana, HUD_MANA_BAR_Y),
    ] {
        spawn_bar(
            commands,
            kind,
            GameplayUiBarLayer::Background,
            HUD_BAR_X,
            y,
            z_bg,
            blue,
        );
        // Fill color is updated dynamically (green for self, red for look).
        spawn_bar(
            commands,
            kind,
            GameplayUiBarLayer::Fill,
            HUD_BAR_X,
            y,
            z_fg,
            green,
        );
    }
}

/// Spawns the skill/inventory scroll knobs used by the gameplay UI.
fn spawn_ui_scroll_knobs(commands: &mut Commands, image_assets: &mut Assets<Image>) {
    // A single white pixel stretched + tinted for dd_showbar-like rectangles.
    let pixel = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let pixel_handle = image_assets.add(pixel);

    let (_blue, green, _red) = ui_bar_colors();

    let spawn_knob = |commands: &mut Commands, kind: GameplayUiScrollKnobKind, sx: f32, sy: f32| {
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiScrollKnob { kind },
            Sprite {
                image: pixel_handle.clone(),
                color: green,
                custom_size: Some(Vec2::new(SCROLL_KNOB_W, SCROLL_KNOB_H)),
                ..default()
            },
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_SCROLL)),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    };

    // Initial positions match engine.c's dd_showbar formulas at pos=0.
    spawn_knob(
        commands,
        GameplayUiScrollKnobKind::Skill,
        SKILL_SCROLL_X,
        SKILL_SCROLL_Y_BASE,
    );
    spawn_knob(
        commands,
        GameplayUiScrollKnobKind::Inventory,
        INV_SCROLL_X,
        INV_SCROLL_Y_BASE,
    );
}

pub(crate) fn run_gameplay_bitmap_text_renderer(
    mut commands: Commands,
    font_cache: Res<FontCache>,
    mut perf: Local<BitmapTextPerfAccum>,
    q_text: Query<
        (Entity, &BitmapText, Option<&Children>),
        Or<(Added<BitmapText>, Changed<BitmapText>)>,
    >,
) {
    let perf_enabled = cfg!(debug_assertions) && profile_rendering_enabled();
    let run_start = perf_enabled.then(Instant::now);

    let Some(layout) = font_cache.bitmap_layout() else {
        return;
    };

    for (entity, text, children) in &q_text {
        if perf_enabled {
            perf.entities = perf.entities.saturating_add(1);
        }

        let Some(image) = font_cache.bitmap_font_image(text.font) else {
            continue;
        };

        let desired = text.text.as_str();
        let desired_len = desired.chars().count();

        let existing_children: &[Entity] = if let Some(c) = children { c } else { &[] };

        // Trim extra glyphs.
        if existing_children.len() > desired_len {
            for child in existing_children.iter().skip(desired_len) {
                commands.entity(*child).despawn();
                if perf_enabled {
                    perf.glyph_despawned = perf.glyph_despawned.saturating_add(1);
                }
            }
        }

        // Update existing and spawn missing.
        for (i, ch) in desired.chars().enumerate() {
            let glyph_index = crate::font_cache::FontCache::bitmap_glyph_index(ch);
            let local_x = (i as f32) * BITMAP_GLYPH_W;
            let local_z = (i as f32) * 0.0001;

            if let Some(&child) = existing_children.get(i) {
                commands.entity(child).insert((
                    Sprite {
                        image: image.clone(),
                        texture_atlas: Some(TextureAtlas {
                            layout: layout.clone(),
                            index: glyph_index,
                        }),
                        color: text.color,
                        ..default()
                    },
                    Transform::from_translation(Vec3::new(local_x, 0.0, local_z)),
                    Visibility::Visible,
                ));
            } else {
                let child = commands
                    .spawn((
                        GameplayRenderEntity,
                        BitmapGlyph,
                        Sprite {
                            image: image.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: layout.clone(),
                                index: glyph_index,
                            }),
                            color: text.color,
                            ..default()
                        },
                        Anchor::TOP_LEFT,
                        Transform::from_translation(Vec3::new(local_x, 0.0, local_z)),
                        GlobalTransform::default(),
                        Visibility::Visible,
                        InheritedVisibility::default(),
                        ViewVisibility::default(),
                    ))
                    .id();
                commands.entity(entity).add_child(child);

                if perf_enabled {
                    perf.glyph_spawned = perf.glyph_spawned.saturating_add(1);
                }
            }
        }
    }

    if let Some(start) = run_start {
        perf.runs = perf.runs.saturating_add(1);
        perf.total += start.elapsed();
        perf.maybe_report_and_reset();
    }
}

#[derive(Default)]
pub(crate) struct GameplayPerfAccum {
    last_report: Option<Instant>,
    frames: u32,

    total: Duration,
    engine_tick: Duration,
    send_opt: Duration,
    minimap: Duration,
    world_shadows: Duration,
    world_tiles: Duration,
    world_overlays: Duration,
    ui: Duration,
}

impl GameplayPerfAccum {
    /// Emits periodic gameplay performance logs and resets the counters.
    ///
    /// Only active in debug builds when `MAG_PROFILE_RENDERING` is enabled.
    fn maybe_report_and_reset(&mut self) {
        if !cfg!(debug_assertions) || !profile_rendering_enabled() {
            return;
        }

        let now = Instant::now();
        let Some(last) = self.last_report else {
            self.last_report = Some(now);
            return;
        };

        if now.duration_since(last) < Duration::from_secs(2) {
            return;
        }

        let frames = self.frames.max(1) as f64;
        let to_ms = |d: Duration| d.as_secs_f64() * 1000.0;

        info!(
            "perf gameplay: total={:.2}ms/f (engine={:.2} send_opt={:.2} minimap={:.2} shadows={:.2} tiles={:.2} ovl={:.2} ui={:.2}) over {} frames",
            to_ms(self.total) / frames,
            to_ms(self.engine_tick) / frames,
            to_ms(self.send_opt) / frames,
            to_ms(self.minimap) / frames,
            to_ms(self.world_shadows) / frames,
            to_ms(self.world_tiles) / frames,
            to_ms(self.world_overlays) / frames,
            to_ms(self.ui) / frames,
            self.frames,
        );

        self.last_report = Some(now);
        self.frames = 0;
        self.total = Duration::ZERO;
        self.engine_tick = Duration::ZERO;
        self.send_opt = Duration::ZERO;
        self.minimap = Duration::ZERO;
        self.world_shadows = Duration::ZERO;
        self.world_tiles = Duration::ZERO;
        self.world_overlays = Duration::ZERO;
        self.ui = Duration::ZERO;
    }
}

#[derive(Default)]
pub(crate) struct BitmapTextPerfAccum {
    last_report: Option<Instant>,
    runs: u32,
    total: Duration,
    entities: u32,
    glyph_spawned: u32,
    glyph_despawned: u32,
}

impl BitmapTextPerfAccum {
    /// Emits periodic bitmap-text performance logs and resets the counters.
    ///
    /// Only active in debug builds when `MAG_PROFILE_RENDERING` is enabled.
    fn maybe_report_and_reset(&mut self) {
        if !cfg!(debug_assertions) || !profile_rendering_enabled() {
            return;
        }

        let now = Instant::now();
        let Some(last) = self.last_report else {
            self.last_report = Some(now);
            return;
        };

        if now.duration_since(last) < Duration::from_secs(2) {
            return;
        }

        let runs = self.runs.max(1) as f64;
        let ms_per_run = (self.total.as_secs_f64() * 1000.0) / runs;

        info!(
            "perf bitmap_text: {:.3}ms/run (runs={} entities={} spawned={} despawned={})",
            ms_per_run, self.runs, self.entities, self.glyph_spawned, self.glyph_despawned,
        );

        self.last_report = Some(now);
        self.runs = 0;
        self.total = Duration::ZERO;
        self.entities = 0;
        self.glyph_spawned = 0;
        self.glyph_despawned = 0;
    }
}

#[derive(Default)]
pub(crate) struct EngineClock {
    ticker: u32,
}

#[derive(Default)]
pub(crate) struct SendOptClock {
    optstep: u8,
    state: u8,
}

/// Spawns the main UI overlay sprite (the large fixed UI background).
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

/// Spawns the portrait sprite entity (updated dynamically from player/rank state).
fn spawn_ui_portrait(commands: &mut Commands, gfx: &GraphicsCache) {
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
fn spawn_ui_rank(commands: &mut Commands, gfx: &GraphicsCache) {
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

/// Spawns the visible backpack slot sprite entities.
fn spawn_ui_backpack(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches `eng_display_win`: copyspritex(pl.item[n+inv_pos],220+(n%2)*35,2+(n/2)*35,...)
    // We spawn one stable entity per visible backpack slot and update its sprite each frame.
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    for n in 0..10usize {
        let sx = 220.0 + ((n % 2) as f32) * 35.0;
        let sy = 2.0 + ((n / 2) as f32) * 35.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiBackpackSlot { index: n },
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_INV)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Spawns the equipment (worn item) slot sprite entities.
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
                sx: f32::NAN,
                sy: f32::NAN,
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

/// Spawns overlay entities indicating equipment slots blocked by a carried item.
fn spawn_ui_equipment_blocks(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches engine.c: if (inv_block[wntab[n]]) copyspritex(4,303+(n%2)*35,2+(n/2)*35,0);
    // Use sprite 4 (block overlay) when available.
    let Some(block) = gfx.get_sprite(4) else {
        return;
    };
    for (n, worn_index) in EQUIP_WNTAB.into_iter().enumerate() {
        let sx = 303.0 + ((n % 2) as f32) * 35.0;
        let sy = 2.0 + ((n / 2) as f32) * 35.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiEquipmentBlock { worn_index },
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            block.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_EQUIP + 0.05)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Spawns the carried-item sprite entity (drawn under the cursor).
fn spawn_ui_carried_item(commands: &mut Commands, gfx: &GraphicsCache) {
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

/// Spawns the spell icon slot sprite entities.
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
                sx: f32::NAN,
                sy: f32::NAN,
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

/// Spawns the shop window panel and item slot sprite entities.
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
            sx: f32::NAN,
            sy: f32::NAN,
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
                sx: f32::NAN,
                sy: f32::NAN,
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

/// Converts total points into a rank index using legacy thresholds.
///
/// Ported from the original C client (`engine.c`).
/// TODO: This function is duplicated--fix that.
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

const RANK_NAMES: [&str; 24] = [
    "Private",
    "Private First Class",
    "Lance Corporal",
    "Corporal",
    "Sergeant",
    "Staff Sergeant",
    "Master Sergeant",
    "First Sergeant",
    "Sergeant Major",
    "Second Lieutenant",
    "First Lieutenant",
    "Captain",
    "Major",
    "Lieutenant Colonel",
    "Colonel",
    "Brigadier General",
    "Major General",
    "Lieutenant General",
    "General",
    "Field Marshal",
    "Knight",
    "Baron",
    "Earl",
    "Warlord",
];

/// Returns the human-readable rank name for the given total points.
fn rank_name(points: i32) -> &'static str {
    // NOTE: `points2rank` already clamps via the returned range, but we still clamp
    // here defensively to ensure indexing safety if thresholds change.
    let idx = points2rank(points).clamp(0, 23) as usize;
    RANK_NAMES[idx]
}

/// Returns the left X such that `text` is centered within `[area_x, area_x + area_w]`.
///
/// Uses the classic UI assumption of fixed-width bitmap glyphs.
fn centered_text_x(area_x: f32, area_w: f32, text: &str) -> f32 {
    // Match engine.c centering logic: 6px per character.
    let visible_chars = text
        .as_bytes()
        .iter()
        .filter(|&&b| (32..=126).contains(&b))
        .count() as f32;
    let text_w = visible_chars * BITMAP_GLYPH_W;
    area_x + (area_w - text_w) * 0.5
}

/// Returns the sprite id to use for the rank insignia based on total points.
///
/// This matches the original logic of `10 + min(20, points2rank(points))`.
fn rank_insignia_sprite(points_tot: i32) -> i32 {
    // engine.c: copyspritex(10+min(20,points2rank(pl.points_tot)),463,54-16,0);
    let rank = points2rank(points_tot).clamp(0, 20);
    10 + rank
}

/// Sends the legacy split `CL_CMD_SETUSER` packets that persist option state.
///
/// The original client sends user profile chunks (name/desc) in 18 steps while
/// `pdata.changed` is set; this helper reproduces that throttled behavior.
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

/// Updates backpack UI sprites/visibility based on current inventory scroll/hover state.
fn draw_inventory_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    inv_scroll: &GameplayInventoryScrollState,
    hover: &GameplayInventoryHoverState,
    q: &mut Query<(
        &GameplayUiBackpackSlot,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    let pl = player_state.character_info();
    let inv_pos = inv_scroll.inv_pos.min(30);

    for (slot, mut sprite, mut visibility, mut last) in q.iter_mut() {
        let idx = inv_pos.saturating_add(slot.index);
        let sprite_id = pl.item.get(idx).copied().unwrap_or(0);

        // Highlight the currently hovered visible slot.
        let is_hovered = hover.backpack_slot == Some(slot.index);
        sprite.color = if is_hovered {
            Color::srgba(0.6, 1.0, 0.6, 1.0)
        } else {
            Color::WHITE
        };

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

/// Updates worn equipment UI sprites/visibility based on current hover state.
fn draw_equipment_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    hover: &GameplayInventoryHoverState,
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

        // Hover highlight tint.
        let is_hovered = hover.equipment_worn_index == Some(slot.worn_index);
        sprite.color = if is_hovered {
            Color::srgba(0.6, 1.0, 0.6, 1.0)
        } else {
            Color::WHITE
        };

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

/// Updates the active-spells UI sprites/visibility and applies dimming by duration.
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

/// Updates shop window UI panel/items sprites/visibility and hover highlighting.
fn draw_shop_window_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    shop_hover: &GameplayShopHoverState,
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

                let is_hovered = shop_hover.slot == Some(index);
                sprite.color = if is_hovered {
                    // Approximate engine.c's highlight effect=16.
                    Color::srgba(0.6, 1.0, 0.6, 1.0)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 1.0)
                };
                *visibility = Visibility::Visible;
            }
        }
    }
}

/// Handles mouse interactions with the shop UI (hover, close, buy/sell actions).
pub(crate) fn run_gameplay_shop_input(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    mut shop_hover: ResMut<GameplayShopHoverState>,
    mut cursor_state: ResMut<GameplayCursorTypeState>,
) {
    shop_hover.slot = None;
    shop_hover.over_close = false;

    if !player_state.should_show_shop() {
        return;
    }

    let Some(game) = cursor_game_pos(&windows, &cameras) else {
        return;
    };

    let x = game.x;
    let y = game.y;

    // Close button (orig/inter.c::mouse_shop): x 499..516, y 260..274.
    if (499.0..=516.0).contains(&x) && (260.0..=274.0).contains(&y) {
        shop_hover.over_close = true;
        if mouse.just_released(MouseButton::Left) {
            player_state.close_shop();
        }
        return;
    }

    // Shop grid (orig/inter.c::mouse_shop): x 220..500, y 261..(485+32+35).
    // That bottom bound evaluates to 552.
    if (220.0..=500.0).contains(&x) && (261.0..=552.0).contains(&y) {
        let tx = ((x - 220.0) / 35.0).floor() as i32;
        let ty = ((y - 261.0) / 35.0).floor() as i32;
        if (0..8).contains(&tx) && ty >= 0 {
            let nr = (tx + ty * 8) as usize;
            if nr < 62 {
                shop_hover.slot = Some(nr);

                let shop = player_state.shop_target();
                let pl = player_state.character_info();

                if shop.item(nr) != 0 {
                    cursor_state.cursor = GameplayCursorType::Take;
                } else if pl.citem != 0 {
                    cursor_state.cursor = GameplayCursorType::Drop;
                }

                let shop_nr = shop.nr() as i16;
                if mouse.just_released(MouseButton::Left) {
                    net.send(ClientCommand::new_shop(shop_nr, nr as i32).to_bytes());
                } else if mouse.just_released(MouseButton::Right) {
                    net.send(ClientCommand::new_shop(shop_nr, (nr + 62) as i32).to_bytes());
                }
            }
        }
    }
}

#[inline]
/// Returns whether a sprite ID should receive a shadow overlay.
fn should_draw_shadow(sprite_id: i32) -> bool {
    // dd.c::dd_shadow: only certain sprite id ranges get shadows.
    (2000..16_336).contains(&sprite_id) || sprite_id > 17_360
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

/// Computes the on-screen position for `copysprite`-style isometric sprite drawing.
///
/// Returns `(rx, ry)` in screen pixel coordinates, matching the legacy math from `dd.c`.
fn copysprite_screen_pos(
    sprite_id: usize,
    gfx: &GraphicsCache,
    _images: &Assets<Image>,
    xpos: i32,
    ypos: i32,
    xoff: i32,
    yoff: i32,
) -> Option<(i32, i32)> {
    let (xs, ys) = gfx.get_sprite_tiles_xy(sprite_id)?;

    // Ported from dd.c: copysprite()
    // NOTE: we ignore the negative-coordinate odd-bit adjustments because xpos/ypos
    // are always >= 0 in our current usage (0..TILEX*32).
    let mut rx = (xpos / 2) + (ypos / 2) - (xs * 16) + 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32);
    let mut ry = (xpos / 4) - (ypos / 4) + YPOS - (ys * 32);

    rx += xoff;
    ry += yoff;

    Some((rx, ry))
}

/// Returns whether the local tile should be hidden by auto-hide logic.
///
/// Ported from `engine.c::autohide` and based on the camera-centric tile coords.
fn autohide(x: usize, y: usize) -> bool {
    // Ported from engine.c::autohide.
    // NOTE: engine.c uses TILEX/2 in both comparisons.
    !(x >= (TILEX / 2) || (y <= (TILEX / 2)))
}

/// Returns whether a tile coordinate is directly in front of the player.
///
/// Ported from `engine.c::facing`.
fn facing(x: usize, y: usize, dir: i32) -> bool {
    // Ported from engine.c::facing.
    let cx = TILEX / 2;
    let cy = TILEY / 2;

    match dir {
        1 => x == cx + 1 && y == cy,
        2 => x == cx - 1 && y == cy,
        4 => x == cx && y == cy + 1,
        3 => x == cx && y == cy - 1,
        _ => false,
    }
}

const STATTAB: [i32; 11] = [0, 1, 1, 6, 6, 2, 3, 4, 5, 7, 4];

const ATTRIBUTE_NAMES: [&str; 5] = ["Braveness", "Willpower", "Intuition", "Agility", "Strength"];

#[inline]
/// Returns whether this `ctick` advances animation/movement for `ch_speed`.
fn speedo(ch_speed: u8, ctick: usize) -> bool {
    let speed = (ch_speed as usize).min(19);
    SPEEDTAB[speed][ctick.min(19)] != 0
}

/// Computes a smooth per-frame offset for movement animations.
///
/// When `update` is true, advances the internal stepping based on `SPEEDTAB`.
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
/// Returns an idle animation frame offset for certain sprites.
fn do_idle(idle_ani: i32, sprite: u16) -> i32 {
    if sprite == 22480 {
        idle_ani
    } else {
        0
    }
}

/// Advances an item animation state and returns the sprite id to render.
///
/// Ported from the legacy engine, where `it_status` drives frame selection.
fn eng_item(it_sprite: u16, it_status: &mut u8, ctick: usize, ticker: u32) -> i32 {
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

/// Advances a character animation state for a map tile and returns sprite id.
///
/// This updates tile offsets (`obj_xoff/obj_yoff`) to create smooth movement.
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
            let (start, base_add, _wrap) = if (96..=99).contains(&tile.ch_status) {
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
                    tile.ch_status = _wrap;
                } else {
                    tile.ch_status = tile.ch_status.saturating_add(1);
                }
            }

            tmp
        }

        _ => base,
    }
}

/// Runs one legacy-style animation tick over all visible map tiles.
///
/// Populates `back/obj1/obj2` sprite ids and overlay offsets used by the renderer.
fn engine_tick(player_state: &mut PlayerState, ticker: u32, ctick: usize) {
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
            let sprite = eng_item(tile.it_sprite, &mut tile.it_status, ctick, ticker);
            tile.obj1 = sprite;
        }

        if tile.ch_sprite != 0 {
            let sprite = eng_char(tile, ctick);
            tile.obj2 = sprite;
        }
    }
}

/// Spawns gameplay-world entities and gameplay UI elements.
///
/// This initializes gameplay resources, clears any previous gameplay render entities, and builds
/// the world/UI hierarchy when entering `GameState::Gameplay`.
pub(crate) fn setup_gameplay(
    mut commands: Commands,
    gfx: Res<GraphicsCache>,
    mut font_cache: ResMut<FontCache>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut minimap: ResMut<MiniMapState>,
    mut image_assets: ResMut<Assets<Image>>,
    player_state: Res<PlayerState>,
    debug: Res<GameplayDebugSettings>,
    existing_render: Query<Entity, With<GameplayRenderEntity>>,
) {
    log::debug!("setup_gameplay - start");

    // Pending stat raises/points spent (orig/inter.c statbox bookkeeping).
    commands.insert_resource(GameplayStatboxState::default());
    commands.insert_resource(GameplayInventoryScrollState::default());
    commands.insert_resource(GameplayInventoryHoverState::default());
    commands.insert_resource(GameplayShopHoverState::default());
    commands.insert_resource(GameplayCursorTypeState::default());
    commands.insert_resource(GameplayXButtonsState::default());

    // Clear any previous gameplay sprites (re-entering gameplay, etc.)
    for e in &existing_render {
        commands.entity(e).despawn();
    }

    if !gfx.is_initialized() {
        log::warn!("Gameplay entered before GraphicsCache initialized");
        return;
    }

    let map = player_state.map();

    // World-space root: we move this for smooth camera motion.
    let world_root = commands
        .spawn((
            GameplayRenderEntity,
            GameplayWorldRoot,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();

    // Map hover highlight: a white silhouette overlay matching the exact target sprite.
    crate::systems::map_hover::spawn_map_hover_highlight(&mut commands, &gfx, world_root);

    // Persistent move target marker (orig/engine.c draws sprite 31 at pl.goto_x/pl.goto_y).
    crate::systems::map_hover::spawn_map_move_target_marker(&mut commands, &gfx, world_root);

    // Attack target marker (orig/engine.c draws sprite 34 at attack target).
    crate::systems::map_hover::spawn_map_attack_target_marker(&mut commands, &gfx, world_root);

    // Misc action marker sprites (orig/engine.c draws 32/33/45 based on misc_action).
    crate::systems::map_hover::spawn_map_misc_action_marker(&mut commands, &gfx, world_root);

    // Spawn a stable set of entities once; `run_gameplay` updates them.
    for index in 0..map.len() {
        // Shadows (dd.c::dd_shadow), rendered between background and objects/chars.
        if let Some(e) = spawn_shadow_entity(
            &mut commands,
            &gfx,
            TileShadow {
                index,
                layer: ShadowLayer::Object,
            },
        ) {
            commands.entity(world_root).add_child(e);
        }
        if let Some(e) = spawn_shadow_entity(
            &mut commands,
            &gfx,
            TileShadow {
                index,
                layer: ShadowLayer::Character,
            },
        ) {
            commands.entity(world_root).add_child(e);
        }

        if let Some(e) = spawn_tile_entity(
            &mut commands,
            &gfx,
            TileRender {
                index,
                layer: TileLayer::Background,
            },
        ) {
            commands.entity(world_root).add_child(e);
        }
        if let Some(e) = spawn_tile_entity(
            &mut commands,
            &gfx,
            TileRender {
                index,
                layer: TileLayer::Object,
            },
        ) {
            commands.entity(world_root).add_child(e);
        }
        if let Some(e) = spawn_tile_entity(
            &mut commands,
            &gfx,
            TileRender {
                index,
                layer: TileLayer::Character,
            },
        ) {
            commands.entity(world_root).add_child(e);
        }

        if debug.tile_flag_overlays {
            // Tile flag overlays (ported from engine.c: marker/effect sprites on tiles).
            // Debug-only: spawning these for every tile is expensive.
            let overlay_kinds = [
                TileFlagOverlayKind::MoveBlock,
                TileFlagOverlayKind::SightBlock,
                TileFlagOverlayKind::Indoors,
                TileFlagOverlayKind::Underwater,
                TileFlagOverlayKind::NoLag,
                TileFlagOverlayKind::NoMonsters,
                TileFlagOverlayKind::Bank,
                TileFlagOverlayKind::Tavern,
                TileFlagOverlayKind::NoMagic,
                TileFlagOverlayKind::DeathTrap,
                TileFlagOverlayKind::Arena,
                TileFlagOverlayKind::NoExpire,
                TileFlagOverlayKind::UnknownHighBit,
                TileFlagOverlayKind::Injured,
                TileFlagOverlayKind::Death,
                TileFlagOverlayKind::Tomb,
            ];

            for kind in overlay_kinds {
                if let Some(e) =
                    spawn_tile_overlay_entity(&mut commands, &gfx, TileFlagOverlay { index, kind })
                {
                    commands.entity(world_root).add_child(e);
                }
            }
        }
    }

    // UI frame / background (sprite 00001.png)
    spawn_ui_overlay(&mut commands, &gfx);

    // Mini-map (dd_show_map / xmap)
    let minimap_image = minimap.ensure_initialized(&mut image_assets);
    spawn_ui_minimap(&mut commands, minimap_image);

    // Player portrait + rank badge
    spawn_ui_portrait(&mut commands, &gfx);
    spawn_ui_rank(&mut commands, &gfx);

    // Backpack (inventory) slots
    spawn_ui_backpack(&mut commands, &gfx);

    // Equipment slots + active spells
    spawn_ui_equipment(&mut commands, &gfx);
    spawn_ui_equipment_blocks(&mut commands, &gfx);
    spawn_ui_spells(&mut commands, &gfx);

    // Carried item cursor sprite (engine.c draws pl.citem at the mouse position).
    spawn_ui_carried_item(&mut commands, &gfx);

    // Shop window (panel + item slots)
    spawn_ui_shop_window(&mut commands, &gfx);

    // UI toggle indicators (dd_showbox overlays for buttonbox toggles).
    spawn_ui_toggle_boxes(&mut commands, &mut image_assets);

    // HP/Endurance/Mana bars (dd_showbar overlays).
    spawn_ui_stat_bars(&mut commands, &mut image_assets);

    // Skill/inventory scrollbar knob indicators (engine.c: dd_showbar at x=207 and x=290).
    spawn_ui_scroll_knobs(&mut commands, &mut image_assets);

    // Gameplay text input/log UI state
    commands.insert_resource(GameplayTextInput::default());
    commands.insert_resource(GameplayExitState::default());

    // Bitmap font (sprite atlas) used for UI text.
    font_cache.ensure_bitmap_initialized(&gfx, &mut atlas_layouts);

    // Character name/proz overlays (engine.c: dd_gputtext + lookup/set_look_proz).
    crate::systems::nameplates::spawn_gameplay_nameplates(&mut commands, world_root);

    ui::text::spawn_ui_log_text(&mut commands);
    ui::text::spawn_ui_input_text(&mut commands);
    spawn_ui_hud_labels(&mut commands);

    log::debug!("setup_gameplay - end");
}

/// Updates equipment-slot overlay blocks (e.g., blocked slots for carried items/two-handers).
pub(crate) fn run_gameplay_update_equipment_blocks(
    gfx: Res<GraphicsCache>,
    player_state: Res<PlayerState>,
    mut q: Query<(
        &GameplayUiEquipmentBlock,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    let pl = player_state.character_info();
    let mut inv_block = [false; 20];
    let citem = pl.citem;
    let citem_p = pl.citem_p as u16;

    if citem != 0 {
        inv_block[WN_HEAD] = (citem_p & PL_HEAD) == 0;
        inv_block[WN_NECK] = (citem_p & PL_NECK) == 0;
        inv_block[WN_BODY] = (citem_p & PL_BODY) == 0;
        inv_block[WN_ARMS] = (citem_p & PL_ARMS) == 0;
        inv_block[WN_BELT] = (citem_p & PL_BELT) == 0;
        inv_block[WN_LEGS] = (citem_p & PL_LEGS) == 0;
        inv_block[WN_FEET] = (citem_p & PL_FEET) == 0;
        inv_block[WN_RHAND] = (citem_p & PL_WEAPON) == 0;
        inv_block[WN_LHAND] = (citem_p & PL_SHIELD) == 0;
        inv_block[WN_CLOAK] = (citem_p & PL_CLOAK) == 0;

        let ring_blocked = (citem_p & PL_RING) == 0;
        inv_block[WN_LRING] = ring_blocked;
        inv_block[WN_RRING] = ring_blocked;
    }

    // Two-handed weapon blocks the left hand slot.
    if (pl.worn_p[WN_RHAND] as u16 & PL_TWOHAND) != 0 {
        inv_block[WN_LHAND] = true;
    }

    const BLOCK_SPRITE_ID: i32 = 4;
    for (slot, mut sprite, mut vis, mut last) in &mut q {
        let blocked = inv_block.get(slot.worn_index).copied().unwrap_or(false);
        if !blocked {
            *vis = Visibility::Hidden;
            continue;
        }
        if last.sprite_id != BLOCK_SPRITE_ID {
            if let Some(src) = gfx.get_sprite(BLOCK_SPRITE_ID as usize) {
                *sprite = src.clone();
                last.sprite_id = BLOCK_SPRITE_ID;
                *vis = Visibility::Visible;
            } else {
                *vis = Visibility::Hidden;
            }
        } else {
            *vis = Visibility::Visible;
        }
        sprite.color = Color::WHITE;
    }
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

/// Updates scrollbar knob positions for the skill list and inventory list.
pub(crate) fn run_gameplay_update_scroll_knobs(
    statbox: Res<GameplayStatboxState>,
    inv_scroll: Res<GameplayInventoryScrollState>,
    mut q: Query<(&GameplayUiScrollKnob, &mut Transform)>,
) {
    if !statbox.is_changed() && !inv_scroll.is_changed() {
        return;
    }

    let skill_pos = statbox.skill_pos as i32;
    let inv_pos = inv_scroll.inv_pos as i32;

    // Match original integer math: y = base + (pos * range) / max.
    let skill_y =
        SKILL_SCROLL_Y_BASE + ((skill_pos * SKILL_SCROLL_RANGE) / SKILL_SCROLL_MAX) as f32;
    let inv_y = INV_SCROLL_Y_BASE + ((inv_pos * INV_SCROLL_RANGE) / INV_SCROLL_MAX) as f32;

    for (knob, mut t) in &mut q {
        let (x, y) = match knob.kind {
            GameplayUiScrollKnobKind::Skill => (SKILL_SCROLL_X, skill_y),
            GameplayUiScrollKnobKind::Inventory => (INV_SCROLL_X, inv_y),
        };
        t.translation = screen_to_world(x, y, Z_UI_SCROLL);
    }
}

/// Updates HUD stat bar fill widths and visibility (HP/Endurance/Mana).
pub(crate) fn run_gameplay_update_stat_bars(
    player_state: Res<PlayerState>,
    mut q: Query<(&GameplayUiBar, &mut Sprite, &mut Visibility)>,
) {
    let (blue, green, red) = ui_bar_colors();

    #[inline]
    fn scaled_width(numer: u32, denom: u32) -> f32 {
        if denom == 0 {
            return 0.0;
        }
        let w = ((numer as u64) * (BAR_SCALE_NUM as u64) / (denom as u64)).min(BAR_W_MAX as u64);
        w as f32
    }

    let pl = player_state.character_info();
    let look_mode = player_state.should_show_look();
    let look = player_state.look_target();

    let hp_max = pl.hp[5] as u32;
    let end_max = pl.end[5] as u32;
    let mana_max = pl.mana[5] as u32;

    let self_hp_cur = pl.a_hp.max(0) as u32;
    let self_end_cur = pl.a_end.max(0) as u32;
    let self_mana_cur = pl.a_mana.max(0) as u32;

    let (hp_bg, hp_fg, end_bg, end_fg, mana_bg, mana_fg, fill_color) = if look_mode {
        (
            scaled_width(look.hp(), hp_max),
            scaled_width(look.a_hp(), hp_max),
            scaled_width(look.end(), end_max),
            scaled_width(look.a_end(), end_max),
            scaled_width(look.mana(), mana_max),
            scaled_width(look.a_mana(), mana_max),
            red,
        )
    } else {
        (
            scaled_width(hp_max, hp_max),
            scaled_width(self_hp_cur, hp_max),
            scaled_width(end_max, end_max),
            scaled_width(self_end_cur, end_max),
            scaled_width(mana_max, mana_max),
            scaled_width(self_mana_cur, mana_max),
            green,
        )
    };

    for (bar, mut sprite, mut vis) in &mut q {
        let (w, color) = match (bar.kind, bar.layer) {
            (GameplayUiBarKind::Hitpoints, GameplayUiBarLayer::Background) => (hp_bg, blue),
            (GameplayUiBarKind::Hitpoints, GameplayUiBarLayer::Fill) => (hp_fg, fill_color),
            (GameplayUiBarKind::Endurance, GameplayUiBarLayer::Background) => (end_bg, blue),
            (GameplayUiBarKind::Endurance, GameplayUiBarLayer::Fill) => (end_fg, fill_color),
            (GameplayUiBarKind::Mana, GameplayUiBarLayer::Background) => (mana_bg, blue),
            (GameplayUiBarKind::Mana, GameplayUiBarLayer::Fill) => (mana_fg, fill_color),
        };

        sprite.color = color;
        sprite.custom_size = Some(Vec2::new(w.max(0.0), HUD_BAR_H));
        *vis = if w >= 1.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Returns the cursor position in game/viewport coordinates, if available.
///
/// This accounts for the window scale factor and the 2D camera viewport.
fn cursor_game_pos(
    windows: &Query<&Window, With<bevy::window::PrimaryWindow>>,
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

/// Checks whether a point is inside an axis-aligned rectangle.
fn in_rect(game: Vec2, x: f32, y: f32, w: f32, h: f32) -> bool {
    game.x >= x && game.x <= x + w && game.y >= y && game.y <= y + h
}

/// Handles UI toggle buttons and mode buttons (keyboard + mouse), including exit/reset.
pub(crate) fn run_gameplay_buttonbox_toggles(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    mut exit_state: ResMut<GameplayExitState>,
    mut q_boxes: Query<(&GameplayUiToggleBox, &mut Visibility), Without<GameplayUiModeBox>>,
    mut q_mode_boxes: Query<(&GameplayUiModeBox, &mut Visibility), Without<GameplayUiToggleBox>>,
) {
    // Reset (orig/main.c ESC): cmd(CL_CMD_RESET,0,0); show_shop=0; noshop=QSIZE*12; xmove=0;
    if keys.just_pressed(KeyCode::Escape) {
        net.send(ClientCommand::new_reset().to_bytes());
        player_state.close_shop();
    }

    // Keyboard shortcuts for mode buttons (orig client: F1/F2/F3).
    if keys.just_pressed(KeyCode::F1) {
        net.send(ClientCommand::new_mode(2).to_bytes());
    }
    if keys.just_pressed(KeyCode::F2) {
        net.send(ClientCommand::new_mode(1).to_bytes());
    }
    if keys.just_pressed(KeyCode::F3) {
        net.send(ClientCommand::new_mode(0).to_bytes());
    }

    // Exit (orig/inter.c button_command case 11: cmd_exit() -> cmd1(CL_CMD_EXIT,0)).
    if keys.just_pressed(KeyCode::F12) {
        cmd_exit(&mut exit_state, &net, &mut player_state);
    }

    // Keyboard shortcuts (orig/main.c): F4=show_proz, F6=hide, F7=show_names.
    if keys.just_pressed(KeyCode::F4) {
        let cur = player_state.player_data().show_proz;
        player_state.player_data_mut().show_proz = 1 - cur;
    }
    if keys.just_pressed(KeyCode::F6) {
        let cur = player_state.player_data().hide;
        player_state.player_data_mut().hide = 1 - cur;
    }
    if keys.just_pressed(KeyCode::F7) {
        let cur = player_state.player_data().show_names;
        player_state.player_data_mut().show_names = 1 - cur;
    }

    // Mouse buttonbox click areas (orig/inter.c::trans_button + mouse_buttonbox).
    if mouse.just_released(MouseButton::Left) {
        if let Some(game) = cursor_game_pos(&windows, &cameras) {
            // F1/F2/F3 mode buttons (nr=0..2): row1 col0..2
            let f1_x = BUTTONBOX_X + 0.0 * BUTTONBOX_STEP_X;
            let f1_y = BUTTONBOX_Y_ROW1;
            if in_rect(game, f1_x, f1_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                net.send(ClientCommand::new_mode(2).to_bytes());
            }
            let f2_x = BUTTONBOX_X + 1.0 * BUTTONBOX_STEP_X;
            let f2_y = BUTTONBOX_Y_ROW1;
            if in_rect(game, f2_x, f2_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                net.send(ClientCommand::new_mode(1).to_bytes());
            }
            let f3_x = BUTTONBOX_X + 2.0 * BUTTONBOX_STEP_X;
            let f3_y = BUTTONBOX_Y_ROW1;
            if in_rect(game, f3_x, f3_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                net.send(ClientCommand::new_mode(0).to_bytes());
            }

            // F4 (nr=3): row1 col3
            let f4_x = BUTTONBOX_X + 3.0 * BUTTONBOX_STEP_X;
            let f4_y = BUTTONBOX_Y_ROW1;
            if in_rect(game, f4_x, f4_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                let cur = player_state.player_data().show_proz;
                player_state.player_data_mut().show_proz = 1 - cur;
            }

            // F6 (nr=5): row2 col1
            let f6_x = BUTTONBOX_X + 1.0 * BUTTONBOX_STEP_X;
            let f6_y = BUTTONBOX_Y_ROW2;
            if in_rect(game, f6_x, f6_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                let cur = player_state.player_data().hide;
                player_state.player_data_mut().hide = 1 - cur;
            }

            // F7 (nr=6): row2 col2
            let f7_x = BUTTONBOX_X + 2.0 * BUTTONBOX_STEP_X;
            let f7_y = BUTTONBOX_Y_ROW2;
            if in_rect(game, f7_x, f7_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                let cur = player_state.player_data().show_names;
                player_state.player_data_mut().show_names = 1 - cur;
            }

            // F12 / EXIT (nr=11): row3 col3 ("bottom right").
            let f12_x = BUTTONBOX_X + 3.0 * BUTTONBOX_STEP_X;
            let f12_y = BUTTONBOX_Y_ROW3;
            if in_rect(game, f12_x, f12_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                cmd_exit(&mut exit_state, &net, &mut player_state);
            }
        }
    }

    // Update indicator visibility (orig/engine.c dd_showbox calls).
    let pdata = player_state.player_data();
    for (boxc, mut vis) in &mut q_boxes {
        let enabled = match boxc.kind {
            GameplayToggleBoxKind::ShowProz => pdata.show_proz != 0,
            GameplayToggleBoxKind::ShowNames => pdata.show_names != 0,
            GameplayToggleBoxKind::Hide => pdata.hide != 0,
        };
        *vis = if enabled {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    let mode = player_state.character_info().mode;
    for (boxc, mut vis) in &mut q_mode_boxes {
        *vis = if mode == boxc.mode {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Handles statbox input: raising stats/skills and managing skill hotbar assignments.
pub(crate) fn run_gameplay_statbox_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    mut statbox: ResMut<GameplayStatboxState>,
    mut inv_scroll: ResMut<GameplayInventoryScrollState>,
    mut xbuttons: ResMut<GameplayXButtonsState>,
) {
    let Some(game) = cursor_game_pos(&windows, &cameras) else {
        return;
    };

    // Right-click help texts (orig/inter.c::_mouse_statbox).
    if mouse.just_released(MouseButton::Right) {
        let x = game.x;
        let y = game.y;

        // Skill hotbar (xbuttons) assign/unassign (orig/inter.c via button_help cases 16..27).
        if let Some(slot) = xbuttons_slot_at(x, y) {
            let slot_data = &mut player_state.player_data_mut().skill_buttons[slot];

            if let Some(skill_id) = xbuttons.pending_skill_id {
                let skill_nr = get_skill_nr(skill_id) as u32;
                if !slot_data.is_unassigned() && slot_data.skill_nr() == skill_nr {
                    slot_data.set_unassigned();
                } else {
                    let label = xbuttons_truncate_label(get_skill_name(skill_id));
                    slot_data.set_skill_nr(skill_nr);
                    slot_data.set_name(&label);
                }
                player_state.mark_dirty();
            } else {
                // No pending skill selected: allow clearing the slot.
                if !slot_data.is_unassigned() {
                    slot_data.set_unassigned();
                    player_state.mark_dirty();
                }
            }
            return;
        }

        // Inventory scroll right-click help (orig/inter.c::button_help case 12/13).
        if x > 290.0 && y > 1.0 && x < 300.0 && y < 34.0 {
            player_state.tlog(1, "Scroll inventory contents up.");
            return;
        }
        if x > 290.0 && y > 141.0 && x < 300.0 && y < 174.0 {
            player_state.tlog(1, "Scroll inventory contents down");
            return;
        }

        // Skill list right-click (orig/inter.c::mouse_statbox2): show skill description.
        if (2.0..=108.0).contains(&x) && (114.0..=251.0).contains(&y) {
            let row = ((y - 114.0) / 14.0).floor() as usize;
            if row < 10 {
                let pl = player_state.character_info();
                let sorted = build_sorted_skills(pl);
                let skilltab_index = statbox.skill_pos + row;
                if let Some(&skill_id) = sorted.get(skilltab_index) {
                    if pl.skill[skill_id][0] != 0 {
                        xbuttons.pending_skill_id = Some(skill_id);
                        let desc = get_skill_desc(skill_id);
                        if !desc.is_empty() {
                            player_state.tlog(1, desc);
                        }
                    }
                }
            }
            return;
        }

        if x > 109.0 && y > 254.0 && x < 158.0 && y < 266.0 {
            player_state.tlog(1, "Make the changes permanent");
            return;
        }

        if !(133.0..=157.0).contains(&x) || !(2.0..=251.0).contains(&y) {
            return;
        }

        let n = ((y - 2.0) / 14.0).floor() as usize;
        if x < 145.0 {
            if n < 5 {
                player_state.tlog(1, &format!("Raise {}.", ATTRIBUTE_NAMES[n]));
            } else if n == 5 {
                player_state.tlog(1, "Raise Hitpoints.");
            } else if n == 6 {
                player_state.tlog(1, "Raise Endurance.");
            } else if n == 7 {
                player_state.tlog(1, "Raise Mana.");
            } else {
                let pl = player_state.character_info();
                let sorted = build_sorted_skills(pl);
                let skilltab_index = statbox.skill_pos + (n.saturating_sub(8));
                if let Some(&skill_id) = sorted.get(skilltab_index) {
                    let name = get_skill_name(skill_id);
                    if !name.is_empty() {
                        player_state.tlog(1, &format!("Raise {}.", name));
                    }
                }
            }
        } else {
            if n < 5 {
                player_state.tlog(1, &format!("Lower {}.", ATTRIBUTE_NAMES[n]));
            } else if n == 5 {
                player_state.tlog(1, "Lower Hitpoints.");
            } else if n == 6 {
                player_state.tlog(1, "Lower Endurance.");
            } else if n == 7 {
                player_state.tlog(1, "Lower Mana.");
            } else {
                let pl = player_state.character_info();
                let sorted = build_sorted_skills(pl);
                let skilltab_index = statbox.skill_pos + (n.saturating_sub(8));
                if let Some(&skill_id) = sorted.get(skilltab_index) {
                    let name = get_skill_name(skill_id);
                    if !name.is_empty() {
                        player_state.tlog(1, &format!("Lower {}.", name));
                    }
                }
            }
        }
        return;
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    // Skill hotbar (xbuttons) activate (orig/inter.c via button_command cases 16..27).
    if let Some(slot) = xbuttons_slot_at(game.x, game.y) {
        let btn = &player_state.player_data().skill_buttons[slot];
        if btn.is_unassigned() {
            player_state.tlog(1, "No skill assigned to that button.");
        } else {
            let selected_char = player_state.selected_char() as u32;
            let attrib0 = 1u32;
            net.send(ClientCommand::new_skill(btn.skill_nr(), selected_char, attrib0).to_bytes());
        }
        return;
    }

    // Inventory scroll buttons (orig/inter.c::button_command case 12/13 via trans_button).
    if game.x > 290.0 && game.y > 1.0 && game.x < 300.0 && game.y < 34.0 {
        if inv_scroll.inv_pos > 1 {
            inv_scroll.inv_pos = inv_scroll.inv_pos.saturating_sub(2);
        }
        return;
    }
    if game.x > 290.0 && game.y > 141.0 && game.x < 300.0 && game.y < 174.0 {
        if inv_scroll.inv_pos < 30 {
            inv_scroll.inv_pos = (inv_scroll.inv_pos + 2).min(30);
        }
        return;
    }

    // Skill list scroll buttons (orig/inter.c::button_command case 14/15 via trans_button).
    // Up: if (skill_pos>1) skill_pos-=2;  Down: if (skill_pos<40) skill_pos+=2;
    if game.x > 206.0 && game.x < 218.0 && game.y > 113.0 && game.y < 148.0 {
        if statbox.skill_pos > 1 {
            statbox.skill_pos = statbox.skill_pos.saturating_sub(2);
        }
        return;
    }
    if game.x > 206.0 && game.x < 218.0 && game.y > 218.0 && game.y < 252.0 {
        if statbox.skill_pos < 40 {
            statbox.skill_pos = (statbox.skill_pos + 2).min(40);
        }
        return;
    }

    // Skill click (orig/inter.c::mouse_statbox2): clicking a skill row sends CL_CMD_SKILL.
    // The original client always sends attrib0=skilltab[..].attrib[0], which is initialized to 1
    // for all skills (and can be modified for spells via commented-out UI).
    if (2.0..=108.0).contains(&game.x) && (114.0..=251.0).contains(&game.y) {
        let row = ((game.y - 114.0) / 14.0).floor() as usize;
        if row < 10 {
            let pl = player_state.character_info();
            let sorted = build_sorted_skills(pl);
            let skilltab_index = statbox.skill_pos + row;
            if let Some(&skill_id) = sorted.get(skilltab_index) {
                let skill_nr = get_skill_nr(skill_id) as u32;
                let selected_char = player_state.selected_char() as u32;
                let attrib0 = 1u32;
                net.send(ClientCommand::new_skill(skill_nr, selected_char, attrib0).to_bytes());
            }
        }
        return;
    }

    // orig/inter.c::mouse_statbox: Shift=10 repeats, Ctrl=90 repeats.
    let repeat = if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        90
    } else if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        10
    } else {
        1
    };

    for _ in 0..repeat {
        let x = game.x;
        let y = game.y;

        // Commit button.
        if x > 109.0 && y > 254.0 && x < 158.0 && y < 266.0 {
            let pl = player_state.character_info();
            let sorted = build_sorted_skills(pl);
            for n in 0..108 {
                let v = statbox.stat_raised[n];
                if v == 0 {
                    continue;
                }
                let which = if n > 7 {
                    let skilltab_index = n - 8;
                    let Some(&skill_id) = sorted.get(skilltab_index) else {
                        continue;
                    };
                    (get_skill_nr(skill_id) + 8) as i16
                } else {
                    n as i16
                };
                net.send(ClientCommand::new_stat(which, v).to_bytes());
            }
            statbox.clear();
            return;
        }

        if !(133.0..=157.0).contains(&x) || !(2.0..=251.0).contains(&y) {
            return;
        }

        let n = ((y - 2.0) / 14.0).floor() as usize;
        let raising = x < 145.0;

        let pl = player_state.character_info();
        let available = statbox.available_points(pl);

        if raising {
            if n < 5 {
                let idx = n;
                let need = attrib_needed(pl, n, pl.attrib[n][0] as i32 + statbox.stat_raised[idx]);
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[idx] += 1;
                }
            } else if n == 5 {
                let idx = 5;
                let need = hp_needed(pl, pl.hp[0] as i32 + statbox.stat_raised[idx]);
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[idx] += 1;
                }
            } else if n == 6 {
                let idx = 6;
                let need = end_needed(pl, pl.end[0] as i32 + statbox.stat_raised[idx]);
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[idx] += 1;
                }
            } else if n == 7 {
                let idx = 7;
                let need = mana_needed(pl, pl.mana[0] as i32 + statbox.stat_raised[idx]);
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[idx] += 1;
                }
            } else {
                let skill_row = n.saturating_sub(8);
                let skilltab_index = statbox.skill_pos + skill_row;
                let raised_idx = 8 + skilltab_index;
                if raised_idx >= statbox.stat_raised.len() {
                    continue;
                }
                let sorted = build_sorted_skills(pl);
                let Some(&skill_id) = sorted.get(skilltab_index) else {
                    continue;
                };
                if pl.skill[skill_id][0] == 0 {
                    continue;
                }
                let need = skill_needed(
                    pl,
                    skill_id,
                    pl.skill[skill_id][0] as i32 + statbox.stat_raised[raised_idx],
                );
                if need != HIGH_VAL && need <= available {
                    statbox.stat_points_used += need;
                    statbox.stat_raised[raised_idx] += 1;
                }
            }
        } else {
            if n < 5 {
                let idx = n;
                if statbox.stat_raised[idx] > 0 {
                    statbox.stat_raised[idx] -= 1;
                    let refund =
                        attrib_needed(pl, n, pl.attrib[n][0] as i32 + statbox.stat_raised[idx]);
                    if refund != HIGH_VAL {
                        statbox.stat_points_used -= refund;
                    }
                }
            } else if n == 5 {
                let idx = 5;
                if statbox.stat_raised[idx] > 0 {
                    statbox.stat_raised[idx] -= 1;
                    let refund = hp_needed(pl, pl.hp[0] as i32 + statbox.stat_raised[idx]);
                    if refund != HIGH_VAL {
                        statbox.stat_points_used -= refund;
                    }
                }
            } else if n == 6 {
                let idx = 6;
                if statbox.stat_raised[idx] > 0 {
                    statbox.stat_raised[idx] -= 1;
                    let refund = end_needed(pl, pl.end[0] as i32 + statbox.stat_raised[idx]);
                    if refund != HIGH_VAL {
                        statbox.stat_points_used -= refund;
                    }
                }
            } else if n == 7 {
                let idx = 7;
                if statbox.stat_raised[idx] > 0 {
                    statbox.stat_raised[idx] -= 1;
                    let refund = mana_needed(pl, pl.mana[0] as i32 + statbox.stat_raised[idx]);
                    if refund != HIGH_VAL {
                        statbox.stat_points_used -= refund;
                    }
                }
            } else {
                let skill_row = n.saturating_sub(8);
                let skilltab_index = statbox.skill_pos + skill_row;
                let raised_idx = 8 + skilltab_index;
                if raised_idx >= statbox.stat_raised.len() {
                    continue;
                }
                if statbox.stat_raised[raised_idx] <= 0 {
                    continue;
                }
                let sorted = build_sorted_skills(pl);
                let Some(&skill_id) = sorted.get(skilltab_index) else {
                    continue;
                };
                statbox.stat_raised[raised_idx] -= 1;
                let refund = skill_needed(
                    pl,
                    skill_id,
                    pl.skill[skill_id][0] as i32 + statbox.stat_raised[raised_idx],
                );
                if refund != HIGH_VAL {
                    statbox.stat_points_used -= refund;
                }
            }
        }

        if statbox.stat_points_used < 0 {
            statbox.stat_points_used = 0;
        }
    }
}

/// Handles inventory UI hover and click interactions (equipment, backpack, money).
pub(crate) fn run_gameplay_inventory_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    inv_scroll: Res<GameplayInventoryScrollState>,
    mut hover: ResMut<GameplayInventoryHoverState>,
    mut cursor_state: ResMut<GameplayCursorTypeState>,
) {
    let Some(game) = cursor_game_pos(&windows, &cameras) else {
        hover.backpack_slot = None;
        hover.equipment_worn_index = None;
        hover.money = None;
        cursor_state.cursor = GameplayCursorType::None;
        return;
    };

    // Hover detection order mirrors orig/inter.c::mouse_inventory: money first, then backpack.
    hover.backpack_slot = None;
    hover.equipment_worn_index = None;
    hover.money = None;
    cursor_state.cursor = GameplayCursorType::None;

    let x = game.x;
    let y = game.y;

    // Money (orig/inter.c::mouse_inventory): click regions around x=219..301,y=176..259.
    if (176.0..=203.0).contains(&y) {
        if (219.0..=246.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Silver1);
        } else if (247.0..=274.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Silver10);
        } else if (275.0..=301.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Gold1);
        }
    } else if (205.0..=231.0).contains(&y) {
        if (219.0..=246.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Gold10);
        } else if (247.0..=274.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Gold100);
        } else if (275.0..=301.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Gold1000);
        }
    } else if (232.0..=259.0).contains(&y) && (219.0..=246.0).contains(&x) {
        hover.money = Some(GameplayMoneyHoverKind::Gold10000);
    }

    // Backpack hover (orig/inter.c::mouse_inventory): x 219..288, y 1..175.
    if hover.money.is_none() && (219.0..=288.0).contains(&x) && (1.0..=175.0).contains(&y) {
        let tx = ((x - 219.0) / 35.0).floor() as i32;
        let ty = ((y - 1.0) / 35.0).floor() as i32;
        if (0..2).contains(&tx) && (0..5).contains(&ty) {
            let slot = (tx + ty * 2) as usize;
            if slot < 10 {
                hover.backpack_slot = Some(slot);
            }
        }
    }

    // Worn equipment hover (orig/inter.c::mouse_inventory): x>302 && x<371 && y>1 && y<175+35.
    if hover.money.is_none()
        && hover.backpack_slot.is_none()
        && (302.0..=371.0).contains(&x)
        && (1.0..=210.0).contains(&y)
    {
        let tx = ((x - 302.0) / 35.0).floor() as i32;
        let ty = ((y - 1.0) / 35.0).floor() as i32;
        if (0..2).contains(&tx) && (0..6).contains(&ty) {
            let n = (tx + ty * 2) as usize;
            if n < EQUIP_WNTAB.len() {
                hover.equipment_worn_index = Some(EQUIP_WNTAB[n]);
            }
        }
    }

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let keys_mask = (shift as u32) | ((ctrl as u32) << 1);

    // Cursor type (orig/inter.c::mouse_inventory cursor_type logic).
    let pl = player_state.character_info();
    let has_citem = pl.citem != 0;
    if hover.money.is_some() {
        cursor_state.cursor = GameplayCursorType::Take;
    } else if let Some(slot) = hover.backpack_slot {
        let idx = inv_scroll.inv_pos.saturating_add(slot);
        let has_item = pl.item.get(idx).copied().unwrap_or(0) != 0;
        cursor_state.cursor = match keys_mask {
            1 => {
                if has_item {
                    if has_citem {
                        GameplayCursorType::Swap
                    } else {
                        GameplayCursorType::Take
                    }
                } else if has_citem {
                    GameplayCursorType::Drop
                } else {
                    GameplayCursorType::None
                }
            }
            0 => {
                if has_item {
                    GameplayCursorType::Use
                } else {
                    GameplayCursorType::None
                }
            }
            _ => GameplayCursorType::None,
        };
    } else if let Some(worn_index) = hover.equipment_worn_index {
        let has_item = pl.worn.get(worn_index).copied().unwrap_or(0) != 0;
        cursor_state.cursor = match keys_mask {
            1 => {
                if has_item {
                    if has_citem {
                        GameplayCursorType::Swap
                    } else {
                        GameplayCursorType::Take
                    }
                } else if has_citem {
                    GameplayCursorType::Drop
                } else {
                    GameplayCursorType::None
                }
            }
            0 => {
                if has_item {
                    GameplayCursorType::Use
                } else {
                    GameplayCursorType::None
                }
            }
            _ => GameplayCursorType::None,
        };
    }

    // Right-click behavior: money tooltips + backpack look.
    if mouse.just_released(MouseButton::Right) {
        if let Some(kind) = hover.money {
            match kind {
                GameplayMoneyHoverKind::Silver1 => player_state.tlog(1, "1 silver coin."),
                GameplayMoneyHoverKind::Silver10 => player_state.tlog(1, "10 silver coins."),
                GameplayMoneyHoverKind::Gold1 => player_state.tlog(1, "1 gold coin."),
                GameplayMoneyHoverKind::Gold10 => player_state.tlog(1, "10 gold coins."),
                GameplayMoneyHoverKind::Gold100 => player_state.tlog(1, "100 gold coins."),
                GameplayMoneyHoverKind::Gold1000 => player_state.tlog(1, "1,000 gold coins."),
                GameplayMoneyHoverKind::Gold10000 => player_state.tlog(1, "10,000 gold coins."),
            }
            return;
        }

        if let Some(slot) = hover.backpack_slot {
            let selected_char = player_state.selected_char() as u32;
            let idx = inv_scroll.inv_pos.saturating_add(slot);
            net.send(ClientCommand::new_inv_look(idx as u32, 0, selected_char).to_bytes());
            return;
        }

        // Worn equipment right-click (orig/inter.c::mouse_inventory): cmd3(CL_CMD_INV,7,slot,selected_char).
        // Slot numbering here matches the original cmd3 usage (0..11), not the WN_* indices.
        if hover.equipment_worn_index.is_some() && (keys_mask == 0 || keys_mask == 1) {
            let tx = ((x - 302.0) / 35.0).floor() as i32;
            let ty = ((y - 1.0) / 35.0).floor() as i32;
            let slot_nr: Option<u32> = match (tx, ty) {
                (0, 0) => Some(0),  // head
                (1, 0) => Some(9),  // cloak
                (0, 1) => Some(2),  // body
                (1, 1) => Some(3),  // arms
                (0, 2) => Some(1),  // neck
                (1, 2) => Some(4),  // belt
                (0, 3) => Some(8),  // right hand
                (1, 3) => Some(7),  // left hand
                (0, 4) => Some(10), // right ring
                (1, 4) => Some(11), // left ring
                (0, 5) => Some(5),  // legs
                (1, 5) => Some(6),  // feet
                _ => None,
            };
            if let Some(slot_nr) = slot_nr {
                let selected_char = player_state.selected_char() as u32;
                net.send(ClientCommand::new_inv(7, slot_nr, selected_char).to_bytes());
            }
            return;
        }

        return;
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    // Left-click behavior.
    if let Some(kind) = hover.money {
        let selected_char = player_state.selected_char() as u32;
        let amount = match kind {
            GameplayMoneyHoverKind::Silver1 => 1u32,
            GameplayMoneyHoverKind::Silver10 => 10u32,
            GameplayMoneyHoverKind::Gold1 => 100u32,
            GameplayMoneyHoverKind::Gold10 => 1_000u32,
            GameplayMoneyHoverKind::Gold100 => 10_000u32,
            GameplayMoneyHoverKind::Gold1000 => 100_000u32,
            GameplayMoneyHoverKind::Gold10000 => 1_000_000u32,
        };
        // orig/inter.c uses: cmd3(CL_CMD_INV,2,amount,selected_char)
        net.send(ClientCommand::new_inv(2, amount, selected_char).to_bytes());
        return;
    }

    if let Some(slot) = hover.backpack_slot {
        // Backpack click behavior depends on Shift (orig/inter.c::mouse_inventory).
        // - Shift + LB_UP: cmd3(CL_CMD_INV,0,nr+inv_pos,selected_char)
        // - No mods + LB_UP: cmd3(CL_CMD_INV,6,nr+inv_pos,selected_char)
        // - Ctrl or other combos: do nothing.
        if keys_mask == 0 || keys_mask == 1 {
            let selected_char = player_state.selected_char() as u32;
            let idx = inv_scroll.inv_pos.saturating_add(slot) as u32;
            let a = if keys_mask == 1 { 0u32 } else { 6u32 };
            net.send(ClientCommand::new_inv(a, idx, selected_char).to_bytes());
        }
        return;
    }

    // Worn equipment left-click (orig/inter.c::mouse_inventory):
    // - Shift + LB_UP: cmd3(CL_CMD_INV,1,slot,selected_char)
    // - No mods + LB_UP: cmd3(CL_CMD_INV,5,slot,selected_char)
    if hover.equipment_worn_index.is_some() && (keys_mask == 0 || keys_mask == 1) {
        let tx = ((x - 302.0) / 35.0).floor() as i32;
        let ty = ((y - 1.0) / 35.0).floor() as i32;
        let slot_nr: Option<u32> = match (tx, ty) {
            (0, 0) => Some(0),  // head
            (1, 0) => Some(9),  // cloak
            (0, 1) => Some(2),  // body
            (1, 1) => Some(3),  // arms
            (0, 2) => Some(1),  // neck
            (1, 2) => Some(4),  // belt
            (0, 3) => Some(8),  // right hand
            (1, 3) => Some(7),  // left hand
            (0, 4) => Some(10), // right ring
            (1, 4) => Some(11), // left ring
            (0, 5) => Some(5),  // legs
            (1, 5) => Some(6),  // feet
            _ => None,
        };
        if let Some(slot_nr) = slot_nr {
            let selected_char = player_state.selected_char() as u32;
            let a = if keys_mask == 1 { 1u32 } else { 5u32 };
            net.send(ClientCommand::new_inv(a, slot_nr, selected_char).to_bytes());
        }
    }
}

/// Updates HUD labels (stats, attributes, skills, hotbar labels, etc.).
pub(crate) fn run_gameplay_update_hud_labels(
    player_state: Res<PlayerState>,
    statbox: Res<GameplayStatboxState>,
    mut last_state_rev: Local<u64>,
    mut q: ParamSet<(
        Query<
            &mut BitmapText,
            (
                With<GameplayUiHitpointsLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiEnduranceLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiManaLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiMoneyLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiUpdateValue>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiWeaponValueLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiArmorValueLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiExperienceLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
    )>,
    mut q_attrib: Query<
        (&GameplayUiAttributeLabel, &mut BitmapText),
        (
            With<GameplayUiAttributeLabel>,
            Without<GameplayUiSkillLabel>,
            Without<GameplayUiRaiseStatText>,
            Without<GameplayUiAttributeAuxText>,
            Without<GameplayUiSkillAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_attrib_aux: Query<
        (&GameplayUiAttributeAuxText, &mut BitmapText),
        (
            With<GameplayUiAttributeAuxText>,
            Without<GameplayUiAttributeLabel>,
            Without<GameplayUiSkillLabel>,
            Without<GameplayUiRaiseStatText>,
            Without<GameplayUiSkillAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_skill: Query<
        (&GameplayUiSkillLabel, &mut BitmapText),
        (
            With<GameplayUiSkillLabel>,
            Without<GameplayUiRaiseStatText>,
            Without<GameplayUiAttributeLabel>,
            Without<GameplayUiAttributeAuxText>,
            Without<GameplayUiSkillAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_skill_aux: Query<
        (&GameplayUiSkillAuxText, &mut BitmapText),
        (
            With<GameplayUiSkillAuxText>,
            Without<GameplayUiSkillLabel>,
            Without<GameplayUiAttributeLabel>,
            Without<GameplayUiRaiseStatText>,
            Without<GameplayUiAttributeAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_raise_stats: Query<
        (&GameplayUiRaiseStatText, &mut BitmapText),
        (
            With<GameplayUiRaiseStatText>,
            Without<GameplayUiAttributeLabel>,
            Without<GameplayUiSkillLabel>,
            Without<GameplayUiAttributeAuxText>,
            Without<GameplayUiSkillAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_xbuttons: Query<(&GameplayUiXButtonLabel, &mut BitmapText)>,
) {
    let rev = player_state.state_revision();
    if *last_state_rev == rev && !statbox.is_changed() {
        return;
    }
    *last_state_rev = rev;

    const HIGH_VAL: i32 = i32::MAX;

    #[inline]
    fn attrib_needed(pl: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
        let max_v = pl.attrib[n][2] as i32;
        if v >= max_v {
            return HIGH_VAL;
        }
        let diff = pl.attrib[n][3] as i32;
        let v64 = v as i64;
        ((v64 * v64 * v64) * (diff as i64) / 20).clamp(0, i32::MAX as i64) as i32
    }

    #[inline]
    fn skill_needed(pl: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
        let max_v = pl.skill[n][2] as i32;
        if v >= max_v {
            return HIGH_VAL;
        }
        let diff = pl.skill[n][3] as i32;
        let v64 = v as i64;
        let cubic = ((v64 * v64 * v64) * (diff as i64) / 40).clamp(0, i32::MAX as i64) as i32;
        v.max(cubic)
    }

    let pl = player_state.character_info();

    // Skill hotbar (xbuttons) labels.
    for (slot, mut text) in &mut q_xbuttons {
        let btn = &player_state.player_data().skill_buttons[slot.index];
        let mut name = btn.name_str();
        if name.is_empty() {
            name = "-".to_string();
        }
        if text.text != name {
            text.text = name;
        }
    }

    // Hitpoints (current / max). Server already sends a_* as integer current values.
    if let Some(mut text) = q.p0().iter_mut().next() {
        let cur = pl.a_hp.max(0);
        let desired = format!("Hitpoints         {:3} {:3}", cur, pl.hp[5]);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Endurance (current / max)
    if let Some(mut text) = q.p1().iter_mut().next() {
        let cur = pl.a_end.max(0);
        let desired = format!("Endurance         {:3} {:3}", cur, pl.end[5]);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Mana (current / max)
    if let Some(mut text) = q.p2().iter_mut().next() {
        let cur = pl.a_mana.max(0);
        let desired = format!("Mana              {:3} {:3}", cur, pl.mana[5]);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Money (gold/100 and silver remainder)
    if let Some(mut text) = q.p3().iter_mut().next() {
        let desired = format!("Money  {:8}G {:2}S", pl.gold / 100, pl.gold % 100);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Update points remaining
    if let Some(mut text) = q.p4().iter_mut().next() {
        let desired = format!("{:7}", (pl.points - statbox.stat_points_used).max(0));
        if text.text != desired {
            text.text = desired;
        }
    }

    // Weapon value
    if let Some(mut text) = q.p5().iter_mut().next() {
        let desired = format!("Weapon value   {:10}", pl.weapon);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Armor value
    if let Some(mut text) = q.p6().iter_mut().next() {
        let desired = format!("Armor value    {:10}", pl.armor);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Experience (total points)
    if let Some(mut text) = q.p7().iter_mut().next() {
        let desired = format!("Experience     {:10}", pl.points_tot);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Attributes (5 of them: Braveness, Willpower, Intuition, Agility, Strength)
    // Mirrors engine.c: value uses total[5] + stat_raised[n], and cost uses bare[0] + stat_raised[n].
    let stat_points_used: i32 = statbox.stat_points_used;
    let available_points: i32 = (pl.points - stat_points_used).max(0);
    let stat_raised_attrib: [i32; 5] = [
        statbox.stat_raised[0],
        statbox.stat_raised[1],
        statbox.stat_raised[2],
        statbox.stat_raised[3],
        statbox.stat_raised[4],
    ];

    for (attr_label, mut text) in &mut q_attrib {
        if attr_label.attrib_index < 5 {
            let name = ATTRIBUTE_NAMES[attr_label.attrib_index];
            let v_total = pl.attrib[attr_label.attrib_index][5] as i32
                + stat_raised_attrib[attr_label.attrib_index];
            let desired = format!("{:<16}  {:3}", name, v_total);
            if text.text != desired {
                text.text = desired;
            }
        }
    }

    for (cfg, mut text) in &mut q_attrib_aux {
        if cfg.attrib_index >= 5 {
            if !text.text.is_empty() {
                text.text.clear();
            }
            continue;
        }
        let v_bare = pl.attrib[cfg.attrib_index][0] as i32 + stat_raised_attrib[cfg.attrib_index];
        let need = attrib_needed(pl, cfg.attrib_index, v_bare);
        match cfg.col {
            GameplayUiRaiseStatColumn::Plus => {
                let desired = if need != HIGH_VAL && need <= available_points {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            GameplayUiRaiseStatColumn::Minus => {
                let desired = if stat_raised_attrib[cfg.attrib_index] > 0 {
                    "-"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            GameplayUiRaiseStatColumn::Cost => {
                if need != HIGH_VAL {
                    let desired = format!("{:7}", need);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
            GameplayUiRaiseStatColumn::Value => {
                if !text.text.is_empty() {
                    text.text.clear();
                }
            }
        }
    }

    // Skills (10 visible at a time, typically from skill_pos 0..10 in original)
    // Original engine.c behavior:
    // - Sort skills with a custom comparator (skill_cmp)
    // - Push unlearned skills (pl.skill[m][0] == 0) below learned
    // - Push reserved/unused entries (category 'Z' / empty name) to the bottom
    // - Then sort by sortkey (category) and name
    let sorted_skills = build_sorted_skills(pl);

    let skill_pos = statbox.skill_pos;
    let mut rows: Vec<String> = Vec::with_capacity(10);
    let mut row_skill_ids: [Option<usize>; 10] = [None; 10];
    let mut stat_raised_skill_rows: [i32; 10] = [0; 10];
    for row in 0..10 {
        let skilltab_index = skill_pos + row;
        let Some(&skill_id) = sorted_skills.get(skilltab_index) else {
            rows.push(String::from("unused"));
            row_skill_ids[row] = None;
            continue;
        };

        let name = get_skill_name(skill_id);
        let is_unused = get_skill_sortkey(skill_id) == 'Z' || name.is_empty();
        if is_unused {
            rows.push(String::from("unused"));
            row_skill_ids[row] = None;
            continue;
        }

        // In engine.c, "unused" also covers unlearned skills (pl.skill[m][0]==0).
        if pl.skill[skill_id][0] == 0 {
            rows.push(String::from("unused"));
            row_skill_ids[row] = None;
            continue;
        }

        let raised = statbox
            .stat_raised
            .get(8 + skilltab_index)
            .copied()
            .unwrap_or(0);
        stat_raised_skill_rows[row] = raised;

        let skill_display = pl.skill[skill_id][5] as i32 + raised;
        rows.push(format!("{:<16}  {:3}", name, skill_display));
        row_skill_ids[row] = Some(skill_id);
    }

    for (skill_label, mut text) in &mut q_skill {
        let row = skill_label.skill_index;
        if row < rows.len() {
            let desired = rows[row].as_str();
            if text.text != desired {
                text.text.clear();
                text.text.push_str(desired);
            }
        } else if !text.text.is_empty() {
            text.text.clear();
        }
    }

    // Skill +/- and cost columns (mirrors engine.c).
    for (cfg, mut text) in &mut q_skill_aux {
        if cfg.row >= 10 {
            if !text.text.is_empty() {
                text.text.clear();
            }
            continue;
        }
        let Some(skill_id) = row_skill_ids[cfg.row] else {
            if !text.text.is_empty() {
                text.text.clear();
            }
            continue;
        };

        let v_bare = pl.skill[skill_id][0] as i32 + stat_raised_skill_rows[cfg.row];
        let need = skill_needed(pl, skill_id, v_bare);
        match cfg.col {
            GameplayUiRaiseStatColumn::Plus => {
                let desired = if need != HIGH_VAL && need <= available_points {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            GameplayUiRaiseStatColumn::Minus => {
                let desired = if stat_raised_skill_rows[cfg.row] > 0 {
                    "-"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            GameplayUiRaiseStatColumn::Cost => {
                if need != HIGH_VAL {
                    let desired = format!("{:7}", need);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
            GameplayUiRaiseStatColumn::Value => {
                if !text.text.is_empty() {
                    text.text.clear();
                }
            }
        }
    }

    // Raise-stat rows (Hitpoints/Endurance/Mana) with +, -, and cost columns.
    let available_points = available_points;
    let stat_raised_hp: i32 = statbox.stat_raised[5];
    let stat_raised_end: i32 = statbox.stat_raised[6];
    let stat_raised_mana: i32 = statbox.stat_raised[7];

    let hp_base = pl.hp[0] as i32 + stat_raised_hp;
    let end_base = pl.end[0] as i32 + stat_raised_end;
    let mana_base = pl.mana[0] as i32 + stat_raised_mana;

    let hp_needed = {
        let v = hp_needed(pl, hp_base);
        if v == HIGH_VAL {
            None
        } else {
            Some(v)
        }
    };
    let end_needed = {
        let v = end_needed(pl, end_base);
        if v == HIGH_VAL {
            None
        } else {
            Some(v)
        }
    };
    let mana_needed = {
        let v = mana_needed(pl, mana_base);
        if v == HIGH_VAL {
            None
        } else {
            Some(v)
        }
    };

    for (cfg, mut text) in &mut q_raise_stats {
        match (cfg.stat, cfg.col) {
            (GameplayUiRaiseStat::Hitpoints, GameplayUiRaiseStatColumn::Value) => {
                let desired = format!("Hitpoints         {:3}", (pl.hp[5] as i32) + stat_raised_hp);
                if text.text != desired {
                    text.text = desired;
                }
            }
            (GameplayUiRaiseStat::Endurance, GameplayUiRaiseStatColumn::Value) => {
                let desired = format!(
                    "Endurance         {:3}",
                    (pl.end[5] as i32) + stat_raised_end
                );
                if text.text != desired {
                    text.text = desired;
                }
            }
            (GameplayUiRaiseStat::Mana, GameplayUiRaiseStatColumn::Value) => {
                let desired = format!(
                    "Mana              {:3}",
                    (pl.mana[5] as i32) + stat_raised_mana
                );
                if text.text != desired {
                    text.text = desired;
                }
            }

            (GameplayUiRaiseStat::Hitpoints, GameplayUiRaiseStatColumn::Plus) => {
                let desired = if hp_needed.is_some_and(|n| n <= available_points) {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            (GameplayUiRaiseStat::Endurance, GameplayUiRaiseStatColumn::Plus) => {
                let desired = if end_needed.is_some_and(|n| n <= available_points) {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            (GameplayUiRaiseStat::Mana, GameplayUiRaiseStatColumn::Plus) => {
                let desired = if mana_needed.is_some_and(|n| n <= available_points) {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }

            (GameplayUiRaiseStat::Hitpoints, GameplayUiRaiseStatColumn::Minus) => {
                let desired = if stat_raised_hp > 0 { "-" } else { "" };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            (GameplayUiRaiseStat::Endurance, GameplayUiRaiseStatColumn::Minus) => {
                let desired = if stat_raised_end > 0 { "-" } else { "" };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            (GameplayUiRaiseStat::Mana, GameplayUiRaiseStatColumn::Minus) => {
                let desired = if stat_raised_mana > 0 { "-" } else { "" };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }

            (GameplayUiRaiseStat::Hitpoints, GameplayUiRaiseStatColumn::Cost) => {
                if let Some(n) = hp_needed {
                    let desired = format!("{:7}", n);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
            (GameplayUiRaiseStat::Endurance, GameplayUiRaiseStatColumn::Cost) => {
                if let Some(n) = end_needed {
                    let desired = format!("{:7}", n);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
            (GameplayUiRaiseStat::Mana, GameplayUiRaiseStatColumn::Cost) => {
                if let Some(n) = mana_needed {
                    let desired = format!("{:7}", n);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
        }
    }
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

    let rank = rank_name(points_tot);

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

/// Updates the money label text (gold/silver) in the HUD.
fn update_ui_money_text(
    player_state: &PlayerState,
    mut q: Query<&mut BitmapText, With<GameplayUiMoneyLabel>>,
) {
    // Display gold and silver. This mirrors the money display in run_gameplay_update_hud_labels
    // but can be called separately if needed.
    let pl = player_state.character_info();

    if let Some(mut text) = q.iter_mut().next() {
        let desired = format!("Money  {:8}G {:2}S", pl.gold / 100, pl.gold % 100);
        if text.text != desired {
            text.text = desired;
        }
    }
}

/// Updates smaller auxiliary UI elements that don't fit elsewhere.
///
/// Currently updates the money text (also covered by the main HUD-label system).
pub(crate) fn run_gameplay_update_extra_ui(
    player_state: Res<PlayerState>,
    mut q: ParamSet<(Query<&mut BitmapText, With<GameplayUiMoneyLabel>>,)>,
) {
    // Keep as a thin shim; money is also updated in run_gameplay_update_hud_labels.
    update_ui_money_text(&player_state, q.p0());
}

/// Runs the core gameplay update loop (rendering + simulation + UI glue).
///
/// This is the main system for `GameState::Gameplay` and is intended to mirror the legacy
/// client's per-frame behavior.
pub(crate) fn run_gameplay(
    net: Res<NetworkRuntime>,
    gfx: Res<GraphicsCache>,
    mut images: ResMut<Assets<Image>>,
    mut player_state: ResMut<PlayerState>,
    mut minimap: ResMut<MiniMapState>,
    mut clock: Local<EngineClock>,
    mut opt_clock: Local<SendOptClock>,
    mut perf: Local<GameplayPerfAccum>,
    inv_scroll: Res<GameplayInventoryScrollState>,
    inv_hover: Res<GameplayInventoryHoverState>,
    shop_hover: Res<GameplayShopHoverState>,
    mut q_world_root: Query<
        &mut Transform,
        (
            With<GameplayWorldRoot>,
            Without<TileShadow>,
            Without<TileRender>,
        ),
    >,
    mut q_world: ParamSet<(
        Query<
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
        Query<
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
        Query<
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
    )>,
    mut q_ui: ParamSet<(
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
        Query<(
            &GameplayUiBackpackSlot,
            &mut Sprite,
            &mut Visibility,
            &mut LastRender,
        )>,
    )>,
) {
    if !gfx.is_initialized() {
        return;
    }

    let perf_enabled = cfg!(debug_assertions) && profile_rendering_enabled();
    let frame_start = perf_enabled.then(Instant::now);

    // Match original client behavior: advance the engine visuals only when a full server tick
    // packet has been processed (network tick defines animation rate).
    let net_ticker = net.client_ticker();

    let mut did_tick = false;

    // Only call engine_tick when we've received a new server tick packet.
    // This matches the original client where engine_tick() is called once per tick packet.
    if net_ticker != clock.ticker {
        let t0 = perf_enabled.then(Instant::now);
        let ctick = player_state.local_ctick().min(19) as usize;
        clock.ticker = net_ticker;
        engine_tick(&mut player_state, clock.ticker, ctick);
        did_tick = true;

        if let Some(t0) = t0 {
            perf.engine_tick += t0.elapsed();
        }
    }

    // Ported options transfer behavior (engine.c::send_opt).
    {
        let t0 = perf_enabled.then(Instant::now);
        send_opt(&net, &mut player_state, &mut opt_clock);
        if let Some(t0) = t0 {
            perf.send_opt += t0.elapsed();
        }
    }

    let map = player_state.map();

    // Update the mini-map buffer + render the 128x128 window.
    // This is relatively expensive (16k px upload), so only do it when we advance
    // a server tick (or the minimap image hasn't been created yet).
    if did_tick || minimap.image.is_none() {
        let t0 = perf_enabled.then(Instant::now);
        update_minimap(&mut minimap, &gfx, &mut images, map);
        if let Some(t0) = t0 {
            perf.minimap += t0.elapsed();
        }
    }

    let shadows_enabled = player_state.player_data().are_shadows_enabled != 0;

    // Camera offset matches original engine.c: based on center tile's current obj offsets.
    let (global_xoff, global_yoff) = map
        .tile_at_xy(TILEX / 2, TILEY / 2)
        .map(|center| {
            (
                -(center.obj_xoff as f32) + MAP_X_SHIFT,
                -(center.obj_yoff as f32),
            )
        })
        .unwrap_or((MAP_X_SHIFT, 0.0));

    if let Some(mut root) = q_world_root.iter_mut().next() {
        // Apply screen-space offsets in world coordinates (+X right, +Y up).
        root.translation = Vec3::new(global_xoff, -global_yoff, 0.0);
    }

    // UI: player portrait sprite is the center tile's obj2 (engine.c passes plr_sprite)
    let base_portrait_sprite_id = map
        .tile_at_xy(TILEX / 2, TILEY / 2)
        .map(|t| t.obj2)
        .unwrap_or(0);

    let base_rank_sprite_id = rank_insignia_sprite(player_state.character_info().points_tot);

    // Match engine.c: when shop/look is open, the right-side portrait/rank reflect that target.
    let mut ui_portrait_sprite_id = base_portrait_sprite_id;
    let mut ui_rank_sprite_id = base_rank_sprite_id;
    if player_state.should_show_shop() {
        let shop = player_state.shop_target();
        if shop.sprite() != 0 {
            ui_portrait_sprite_id = shop.sprite() as i32;
        }
        let shop_points = shop.points().min(i32::MAX as u32) as i32;
        ui_rank_sprite_id = rank_insignia_sprite(shop_points);
    } else if player_state.should_show_look() {
        let look = player_state.look_target();
        if look.sprite() != 0 {
            ui_portrait_sprite_id = look.sprite() as i32;
        }
        let look_points = look.points().min(i32::MAX as u32) as i32;
        ui_rank_sprite_id = rank_insignia_sprite(look_points);
    }

    // Update shadows (dd.c::dd_shadow)
    let t_shadows = perf_enabled.then(Instant::now);
    for (shadow, mut sprite, mut transform, mut visibility, mut last) in &mut q_world.p0() {
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

        let Some((sx_i, sy_i)) =
            copysprite_screen_pos(sprite_id as usize, &gfx, &images, xpos, ypos, xoff, yoff)
        else {
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
    if let Some(t0) = t_shadows {
        perf.world_shadows += t0.elapsed();
    }

    let t_tiles = perf_enabled.then(Instant::now);
    for (render, mut sprite, mut transform, mut visibility, mut last) in &mut q_world.p1() {
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
                if hide_enabled && id > 0 && !is_item && !autohide(x, y) {
                    // engine.c mine hack: substitute special sprites for certain mine-wall IDs
                    // when hide is enabled and tile isn't directly in front of the player.
                    let is_mine_wall = id > 16335
                        && id < 16422
                        && !matches!(
                            id,
                            16357 | 16365 | 16373 | 16381 | 16389 | 16397 | 16405 | 16413 | 16421
                        )
                        && !facing(x, y, player_state.character_info().dir);

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
        let Some((sx_i, sy_i)) = copysprite_screen_pos(
            sprite_id as usize,
            &gfx,
            &images,
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
    if let Some(t0) = t_tiles {
        perf.world_tiles += t0.elapsed();
    }

    // Map flag overlays (ported from engine.c): draw above characters on the same tile.
    let t_ovl = perf_enabled.then(Instant::now);
    for (ovl, mut sprite, mut transform, mut visibility, mut last) in &mut q_world.p2() {
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

        let Some((sx_i, sy_i)) = copysprite_screen_pos(
            sprite_id as usize,
            &gfx,
            &images,
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
    if let Some(t0) = t_ovl {
        perf.world_overlays += t0.elapsed();
    }

    let t_ui = perf_enabled.then(Instant::now);
    // Update UI portrait
    if let Some((mut sprite, mut visibility, mut last)) = q_ui.p0().iter_mut().next() {
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
    if let Some((mut sprite, mut visibility, mut last)) = q_ui.p1().iter_mut().next() {
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

    draw_inventory_ui(&gfx, &player_state, &inv_scroll, &inv_hover, &mut q_ui.p5());
    draw_equipment_ui(&gfx, &player_state, &inv_hover, &mut q_ui.p2());
    draw_active_spells_ui(&gfx, &player_state, &mut q_ui.p3());
    draw_shop_window_ui(&gfx, &player_state, &shop_hover, &mut q_ui.p4());

    if let Some(t0) = t_ui {
        perf.ui += t0.elapsed();
    }

    if let Some(start) = frame_start {
        perf.frames = perf.frames.saturating_add(1);
        perf.total += start.elapsed();
        perf.maybe_report_and_reset();
    }
}
