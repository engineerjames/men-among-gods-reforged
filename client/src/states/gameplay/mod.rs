use bevy::prelude::*;

use bevy::ecs::query::Without;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;

use std::cmp::Ordering;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tracing::info;

mod components;
mod layout;
mod legacy_engine;
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
    MF_TAVERN, MF_UWATER, SPEEDTAB, SPR_EMPTY, STONED, STUNNED, TOMB, UWATER, XPOS, YPOS,
};
use mag_core::types::skilltab::{get_skill_name, get_skill_sortkey, MAX_SKILLS};

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
    ui::hud::spawn_ui_hud_labels(commands);
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
    ui::shop::run_gameplay_update_shop_price_labels(player_state, shop_hover, q_sell, q_buy);
}

/// Spawns the orange outline boxes used for gameplay toggles and mode selection.
fn spawn_ui_toggle_boxes(commands: &mut Commands, image_assets: &mut Assets<Image>) {
    ui::hud::spawn_ui_toggle_boxes(commands, image_assets);
}

/// Spawns the HUD stat bars (background + fill rectangles).
fn spawn_ui_stat_bars(commands: &mut Commands, image_assets: &mut Assets<Image>) {
    ui::hud::spawn_ui_stat_bars(commands, image_assets);
}

/// Spawns the skill/inventory scroll knobs used by the gameplay UI.
fn spawn_ui_scroll_knobs(commands: &mut Commands, image_assets: &mut Assets<Image>) {
    ui::statbox::spawn_ui_scroll_knobs(commands, image_assets);
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
    ui::inventory::spawn_ui_backpack(commands, gfx);
}

/// Spawns the equipment (worn item) slot sprite entities.
fn spawn_ui_equipment(commands: &mut Commands, gfx: &GraphicsCache) {
    ui::inventory::spawn_ui_equipment(commands, gfx);
}

/// Spawns overlay entities indicating equipment slots blocked by a carried item.
fn spawn_ui_equipment_blocks(commands: &mut Commands, gfx: &GraphicsCache) {
    ui::inventory::spawn_ui_equipment_blocks(commands, gfx);
}

/// Spawns the carried-item sprite entity (drawn under the cursor).
fn spawn_ui_carried_item(commands: &mut Commands, gfx: &GraphicsCache) {
    ui::cursor::spawn_ui_carried_item(commands, gfx);
}

/// Spawns the spell icon slot sprite entities.
fn spawn_ui_spells(commands: &mut Commands, gfx: &GraphicsCache) {
    ui::inventory::spawn_ui_spells(commands, gfx);
}

/// Spawns the shop window panel and item slot sprite entities.
fn spawn_ui_shop_window(commands: &mut Commands, gfx: &GraphicsCache) {
    ui::shop::spawn_ui_shop_window(commands, gfx);
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
    ui::shop::draw_shop_window_ui(gfx, player_state, shop_hover, q);
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
    ui::shop::run_gameplay_shop_input(
        mouse,
        windows,
        cameras,
        net,
        player_state,
        shop_hover,
        cursor_state,
    );
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

const ATTRIBUTE_NAMES: [&str; 5] = ["Braveness", "Willpower", "Intuition", "Agility", "Strength"];
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
    ui::inventory::run_gameplay_update_equipment_blocks(gfx, player_state, q);
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
    ui::cursor::run_gameplay_update_cursor_and_carried_item(
        commands,
        window_entities,
        windows,
        cameras,
        gfx,
        player_state,
        cursor_state,
        q,
    );
}

/// Updates scrollbar knob positions for the skill list and inventory list.
pub(crate) fn run_gameplay_update_scroll_knobs(
    statbox: Res<GameplayStatboxState>,
    inv_scroll: Res<GameplayInventoryScrollState>,
    mut q: Query<(&GameplayUiScrollKnob, &mut Transform)>,
) {
    ui::statbox::run_gameplay_update_scroll_knobs(statbox, inv_scroll, q);
}

/// Updates HUD stat bar fill widths and visibility (HP/Endurance/Mana).
pub(crate) fn run_gameplay_update_stat_bars(
    player_state: Res<PlayerState>,
    mut q: Query<(&GameplayUiBar, &mut Sprite, &mut Visibility)>,
) {
    ui::hud::run_gameplay_update_stat_bars(player_state, q);
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
    ui::hud::run_gameplay_buttonbox_toggles(
        keys,
        mouse,
        windows,
        cameras,
        net,
        player_state,
        exit_state,
        q_boxes,
        q_mode_boxes,
    );
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
    ui::statbox::run_gameplay_statbox_input(
        keys,
        mouse,
        windows,
        cameras,
        net,
        player_state,
        statbox,
        inv_scroll,
        xbuttons,
    );
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
    ui::inventory::run_gameplay_inventory_input(
        keys,
        mouse,
        windows,
        cameras,
        net,
        player_state,
        inv_scroll,
        hover,
        cursor_state,
    );
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
    ui::hud::run_gameplay_update_hud_labels(
        player_state,
        statbox,
        last_state_rev,
        q,
        q_attrib,
        q_attrib_aux,
        q_skill,
        q_skill_aux,
        q_raise_stats,
        q_xbuttons,
    );
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
        legacy_engine::engine_tick(&mut player_state, clock.ticker, ctick);
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

        let Some((sx_i, sy_i)) = legacy_engine::copysprite_screen_pos(
            sprite_id as usize,
            &gfx,
            &images,
            xpos,
            ypos,
            xoff,
            yoff,
        )
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
        let Some((sx_i, sy_i)) = legacy_engine::copysprite_screen_pos(
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
