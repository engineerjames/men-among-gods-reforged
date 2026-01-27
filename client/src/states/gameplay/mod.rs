use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use bevy::ecs::query::Without;
use bevy::sprite::Anchor;
use mag_core::constants::{TILEX, TILEY};

use std::cmp::Ordering;
use std::time::Instant;

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
pub(crate) use resources::CursorActionTextSettings;
pub(crate) use resources::{GameplayCursorType, GameplayCursorTypeState};
pub(crate) use world_render::{TileLayer, TileRender};

pub(crate) use ui::text::run_gameplay_text_ui;
pub(crate) use world_render::dd_effect_tint;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::font_cache::{FontCache, BITMAP_GLYPH_W};
use crate::gfx_cache::GraphicsCache;
use crate::network::{client_commands::ClientCommand, NetworkRuntime};
use crate::player_state::PlayerState;
use crate::systems::debug::{
    profile_rendering_enabled, BitmapTextPerfAccum, GameplayDebugSettings, GameplayPerfAccum,
};
use crate::systems::magic_postprocess::MagicScreenCamera;

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
    let blue = Color::srgb_u8(9, 4, 58);
    let green = Color::srgb_u8(8, 77, 23);
    let red = Color::srgb_u8(155, 7, 7);
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

pub(crate) fn run_gameplay_bitmap_text_renderer(
    mut commands: Commands,
    font_cache: Res<FontCache>,
    mut perf: Local<BitmapTextPerfAccum>,
    q_text: Query<
        (
            Entity,
            &BitmapText,
            Option<&Children>,
            Option<&RenderLayers>,
        ),
        Or<(Added<BitmapText>, Changed<BitmapText>)>,
    >,
) {
    let perf_enabled = cfg!(debug_assertions) && profile_rendering_enabled();
    let run_start = perf_enabled.then(Instant::now);

    let Some(layout) = font_cache.bitmap_layout() else {
        return;
    };

    for (entity, text, children, parent_layers) in &q_text {
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
                commands.entity(*child).queue_silenced(|e: EntityWorldMut| {
                    e.despawn();
                });
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
                if let Some(layers) = parent_layers {
                    commands.entity(child).insert(layers.clone());
                }

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

                if let Some(layers) = parent_layers {
                    commands.entity(child).insert(layers.clone());
                }
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
pub(crate) struct EngineClock {
    ticker: u32,
}

#[derive(Default)]
pub(crate) struct SendOptClock {
    optstep: u8,
    state: u8,
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
    let rank = mag_core::ranks::points2rank(points_tot as u32).clamp(0, 20);
    10 + rank as i32
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

// TODO: Move to common
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
        commands.entity(e).queue_silenced(|e: EntityWorldMut| {
            e.despawn();
        });
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

        // Tile flag overlays (ported from engine.c: marker/effect sprites on tiles).
        // We always spawn gameplay-critical overlays. Debug-only overlays remain optional
        // since spawning them for every tile is expensive.
        let gameplay_overlay_kinds = [
            TileFlagOverlayKind::Injured,
            TileFlagOverlayKind::Death,
            TileFlagOverlayKind::Tomb,
        ];
        for kind in gameplay_overlay_kinds {
            if let Some(e) =
                spawn_tile_overlay_entity(&mut commands, &gfx, TileFlagOverlay { index, kind })
            {
                commands.entity(world_root).add_child(e);
            }
        }

        if debug.tile_flag_overlays {
            let debug_overlay_kinds = [
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
            ];
            for kind in debug_overlay_kinds {
                if let Some(e) =
                    spawn_tile_overlay_entity(&mut commands, &gfx, TileFlagOverlay { index, kind })
                {
                    commands.entity(world_root).add_child(e);
                }
            }
        }
    }

    // UI frame / background (sprite 00001.png)
    ui::portrait::spawn_ui_overlay(&mut commands, &gfx);

    // Mini-map (dd_show_map / xmap)
    let minimap_image = minimap.ensure_initialized(&mut image_assets);
    spawn_ui_minimap(&mut commands, minimap_image);

    // Player portrait + rank badge
    ui::portrait::spawn_ui_portrait(&mut commands, &gfx);
    ui::portrait::spawn_ui_rank(&mut commands, &gfx);

    // Backpack (inventory) slots
    ui::inventory::spawn_ui_backpack(&mut commands, &gfx);

    // Equipment slots + active spells
    ui::inventory::spawn_ui_equipment(&mut commands, &gfx);
    ui::inventory::spawn_ui_equipment_blocks(&mut commands, &gfx);
    ui::inventory::spawn_ui_spells(&mut commands, &gfx);

    // Carried item cursor sprite (engine.c draws pl.citem at the mouse position).
    ui::cursor::spawn_ui_carried_item(&mut commands, &gfx);

    // Cursor action label (small hint text near mouse).
    ui::cursor::spawn_ui_cursor_action_text(&mut commands);

    // Shop window (panel + item slots)
    ui::shop::spawn_ui_shop_window(&mut commands, &gfx);

    // UI toggle indicators (dd_showbox overlays for buttonbox toggles).
    ui::hud::spawn_ui_toggle_boxes(&mut commands, &mut image_assets);

    // HP/Endurance/Mana bars (dd_showbar overlays).
    ui::hud::spawn_ui_stat_bars(&mut commands, &mut image_assets);

    // Skill/inventory scrollbar knob indicators (engine.c: dd_showbar at x=207 and x=290).
    ui::statbox::spawn_ui_scroll_knobs(&mut commands, &mut image_assets);

    // Gameplay text input/log UI state
    commands.insert_resource(GameplayTextInput::default());
    commands.insert_resource(GameplayExitState::default());

    // Bitmap font (sprite atlas) used for UI text.
    once!(font_cache.ensure_bitmap_initialized(&gfx, &mut atlas_layouts));

    // Character name/proz overlays (engine.c: dd_gputtext + lookup/set_look_proz).
    crate::systems::nameplates::spawn_gameplay_nameplates(&mut commands, world_root);

    ui::text::spawn_ui_log_text(&mut commands);
    ui::text::spawn_ui_input_text(&mut commands);
    ui::hud::spawn_ui_hud_labels(&mut commands);

    log::debug!("setup_gameplay - end");
}

/// Returns the cursor position in game/viewport coordinates, if available.
///
/// This accounts for the window scale factor and the 2D camera viewport.
fn cursor_game_pos(
    windows: &Query<&Window, With<bevy::window::PrimaryWindow>>,
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

/// Checks whether a point is inside an axis-aligned rectangle.
fn in_rect(game: Vec2, x: f32, y: f32, w: f32, h: f32) -> bool {
    game.x >= x && game.x <= x + w && game.y >= y && game.y <= y + h
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

    // World rendering (dd.c + engine.c ports).
    let t_shadows = perf_enabled.then(Instant::now);
    world_render::update_world_shadows(&gfx, &images, map, shadows_enabled, &mut q_world.p0());
    if let Some(t0) = t_shadows {
        perf.world_shadows += t0.elapsed();
    }

    let t_tiles = perf_enabled.then(Instant::now);
    world_render::update_world_tiles(&gfx, &images, map, &player_state, &mut q_world.p1());
    if let Some(t0) = t_tiles {
        perf.world_tiles += t0.elapsed();
    }

    // Map flag overlays (ported from engine.c): draw above characters on the same tile.
    let t_ovl = perf_enabled.then(Instant::now);
    world_render::update_world_overlays(&gfx, &images, map, &mut q_world.p2());
    if let Some(t0) = t_ovl {
        perf.world_overlays += t0.elapsed();
    }

    let t_ui = perf_enabled.then(Instant::now);
    ui::portrait::update_ui_portrait_sprite(&gfx, ui_portrait_sprite_id, &mut q_ui.p0());
    ui::portrait::update_ui_rank_sprite(&gfx, ui_rank_sprite_id, &mut q_ui.p1());

    ui::inventory_draw::draw_inventory_ui(
        &gfx,
        &player_state,
        &inv_scroll,
        &inv_hover,
        &mut q_ui.p5(),
    );
    ui::inventory_draw::draw_equipment_ui(&gfx, &player_state, &inv_hover, &mut q_ui.p2());
    ui::inventory_draw::draw_active_spells_ui(&gfx, &player_state, &mut q_ui.p3());
    ui::shop::draw_shop_window_ui(&gfx, &player_state, &shop_hover, &mut q_ui.p4());

    if let Some(t0) = t_ui {
        perf.ui += t0.elapsed();
    }

    if let Some(start) = frame_start {
        perf.frames = perf.frames.saturating_add(1);
        perf.total += start.elapsed();
        perf.maybe_report_and_reset();
    }
}
