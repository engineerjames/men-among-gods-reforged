//! Per-player view/update fan-out.
//!
//! `plr_getmap` and `plr_change` read the shared world (map, characters,
//! items, globals) and write only the player's own slot (`ServerPlayer`) plus
//! that character's `SeeMap`. This module makes that split explicit so the
//! per-player pass can run on a worker pool: workers get an immutable
//! [`WorldView`] and exclusive access to their player, and every world
//! mutation the legacy code performed inline (clearing `IF_UPDATE`,
//! disconnecting on tick-buffer overflow, visibility counters) is recorded in
//! [`Deferred`] and applied on the tick thread after the join.

use core::constants::{
    CharacterFlags, ItemFlags, MF_INDOORS, MF_NOMONST, MF_SIGHTBLOCK, SERVER_MAPX, SERVER_MAPY,
    ST_NORMAL, USE_ACTIVE, VISI_BUFFER_LEN, VISI_CENTER, VISI_STRIDE,
};
use core::skills;
use core::traits::KIN_MONSTER;
use core::types::{Character, Global, Item, Map, SeeMap};

use crate::game_state::GameState;
use crate::network_manager;
use crate::server::ServerPlayer;

/// Environment variable overriding the tick worker count.
pub const TICK_WORKERS_ENV: &str = "MAG_TICK_WORKERS";

/// Below this many players in `ST_NORMAL` the fan-out runs serially; the
/// pool wake-up and join cost more than the work they would spread.
pub const PARALLEL_MIN_PLAYERS: usize = 4;

/// Immutable snapshot of the world data the per-player update reads.
///
/// All fields borrow from `GameState`; the struct is `Sync` because it only
/// holds shared references to plain data.
#[derive(Clone, Copy)]
pub struct WorldView<'a> {
    /// Map tiles indexed by `x + y * SERVER_MAPX`.
    pub map: &'a [Map],
    /// All character instances.
    pub characters: &'a [Character],
    /// All item instances.
    pub items: &'a [Item],
    /// Global server state (`ticker`, `dlight`, `load`, ...).
    pub globals: &'a Global,
}

impl<'a> WorldView<'a> {
    /// Daylight at `(x, y)`, attenuated for indoor tiles.
    ///
    /// # Arguments
    ///
    /// * `x` - Tile x coordinate.
    /// * `y` - Tile y coordinate.
    ///
    /// # Returns
    ///
    /// * Effective daylight value for the tile.
    #[inline]
    pub fn check_dlight(&self, x: usize, y: usize) -> i32 {
        let m = x + y * SERVER_MAPX as usize;
        if self.map[m].flags & u64::from(MF_INDOORS) == 0 {
            self.globals.dlight
        } else {
            (self.globals.dlight * i32::from(self.map[m].dlight)) / 256
        }
    }
}

/// World mutations a per-player update wants, replayed on the tick thread.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Deferred {
    /// Item indices whose `IF_UPDATE` flag must be cleared.
    pub clear_item_update: Vec<usize>,
    /// The player's tick buffer overflowed; disconnect once the pass ends.
    pub overflow: bool,
    /// Visibility cache hits to add to `GameState::see_hit`.
    pub see_hit: u64,
    /// Visibility cache misses to add to `GameState::see_miss`.
    pub see_miss: u64,
}

/// Per-player wall-clock split of a single update.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhaseTimings {
    /// Time spent in `plr_getmap`.
    #[cfg(feature = "measure-time")]
    pub getmap: std::time::Duration,
    /// Time spent in `plr_change`.
    #[cfg(feature = "measure-time")]
    pub change: std::time::Duration,
}

impl std::ops::AddAssign for PhaseTimings {
    #[allow(unused_variables)]
    fn add_assign(&mut self, rhs: Self) {
        #[cfg(feature = "measure-time")]
        {
            self.getmap += rhs.getmap;
            self.change += rhs.change;
        }
    }
}

/// Everything one worker needs to update one player.
pub struct PlayerUpdateCtx<'a> {
    /// Shared read-only world.
    pub world: &'a WorldView<'a>,
    /// Player slot index.
    pub nr: usize,
    /// Character controlled by the player (`players[nr].usnr`).
    pub cn: usize,
    /// The player's own slot.
    pub player: &'a mut ServerPlayer,
    /// The character's visibility cache.
    pub see: &'a mut SeeMap,
    /// World mutations to apply after the pass.
    pub deferred: Deferred,
}

impl<'a> PlayerUpdateCtx<'a> {
    /// Append bytes to the player's tick buffer.
    ///
    /// Mirrors [`network_manager::xsend`] but records an overflow instead of
    /// disconnecting inline; the tick thread performs the logout afterwards.
    /// Once an overflow is recorded, further sends are dropped.
    ///
    /// # Arguments
    ///
    /// * `data` - Bytes to append.
    /// * `length` - Number of bytes from `data` to append.
    pub fn xsend(&mut self, data: &[u8], length: usize) {
        if self.deferred.overflow {
            return;
        }
        let send_len = length.min(data.len());
        let p = &mut *self.player;

        if p.sock.is_none() {
            log::warn!("xsend: no socket for player {}", self.nr);
            return;
        }

        if p.tptr + send_len >= p.tbuf.len() {
            log::error!(
                "#INTERNAL ERROR# ticksize too large for player {}, terminating connection",
                self.nr
            );
            self.deferred.overflow = true;
            return;
        }

        let start = p.tptr;
        p.tbuf[start..start + send_len].copy_from_slice(&data[..send_len]);
        p.tptr = start + send_len;
    }

    /// Schedule clearing `IF_UPDATE` on `item_idx` after the pass.
    ///
    /// # Arguments
    ///
    /// * `item_idx` - Item index whose flag should be cleared.
    #[inline]
    pub fn defer_clear_item_update(&mut self, item_idx: usize) {
        self.deferred.clear_item_update.push(item_idx);
    }

    /// Line-of-sight from the player's character to `(tx, ty)`.
    ///
    /// Rebuilds the character's `SeeMap` when its origin moved.
    ///
    /// # Arguments
    ///
    /// * `fx` - Origin x (the character's position).
    /// * `fy` - Origin y.
    /// * `tx` - Target x.
    /// * `ty` - Target y.
    /// * `max_distance` - Radius of the see-map to build on a miss.
    ///
    /// # Returns
    ///
    /// * `0` when not visible, otherwise a positive metric (`1` = best).
    pub fn can_see(&mut self, fx: i32, fy: i32, tx: i32, ty: i32, max_distance: i32) -> i32 {
        can_see(
            self.world,
            self.see,
            self.cn,
            fx,
            fy,
            tx,
            ty,
            max_distance,
            &mut self.deferred,
        )
    }

    /// Whether the player's character can perceive character `co`.
    ///
    /// # Arguments
    ///
    /// * `co` - Target character index.
    ///
    /// # Returns
    ///
    /// * `0` when not visible, `1` when adjacent, otherwise a distance metric.
    pub fn char_can_see(&mut self, co: usize) -> i32 {
        char_can_see(self.world, self.see, self.cn, co, &mut self.deferred)
    }
}

// ---------------------------------------------------------------------------
// Visibility over a WorldView (ports of the GameState methods in
// state/visibility.rs that only read world data).
// ---------------------------------------------------------------------------

/// Adjust a raw light value by the viewer's perception and infrared.
///
/// # Arguments
///
/// * `viewer` - Observing character.
/// * `light` - Raw tile light.
///
/// # Returns
///
/// * Adjusted light value in `0..=255`.
#[inline]
pub fn calculate_light(viewer: &Character, light: i32) -> i32 {
    let percept = i32::from(viewer.skill[skills::SK_PERCEPT][5]);
    let mut adjusted = light;

    if light == 0 && percept > 150 {
        adjusted = 1;
    }

    adjusted = adjusted * std::cmp::min(percept, 10) / 10;

    if adjusted > 255 {
        adjusted = 255;
    }

    if viewer.flags & CharacterFlags::Infrared.bits() != 0 && adjusted < 5 {
        adjusted = 5;
    }

    adjusted
}

#[inline]
fn vis_index(ox: i32, oy: i32, x: i32, y: i32) -> Option<usize> {
    let rx = x - ox + VISI_CENTER;
    let ry = y - oy + VISI_CENTER;
    let stride = VISI_STRIDE as i32;

    if !(0..stride).contains(&rx) || !(0..stride).contains(&ry) {
        None
    } else {
        Some((rx + ry * stride) as usize)
    }
}

#[inline]
fn add_vis(vis: &mut [i8; VISI_BUFFER_LEN], ox: i32, oy: i32, x: i32, y: i32, value: i32) {
    if let Some(index) = vis_index(ox, oy, x, y)
        && vis[index] == 0
    {
        vis[index] = value as i8;
    }
}

fn check_map_see(world: &WorldView, is_monster: bool, x: i32, y: i32) -> bool {
    if x <= 0 || x >= SERVER_MAPX || y <= 0 || y >= SERVER_MAPY {
        return false;
    }

    let m = (x + y * SERVER_MAPX) as usize;
    let tile = &world.map[m];

    let block_mask = if is_monster {
        u64::from(MF_SIGHTBLOCK | MF_NOMONST)
    } else {
        u64::from(MF_SIGHTBLOCK)
    };
    if tile.flags & block_mask != 0 {
        return false;
    }

    let item_idx = tile.it as usize;
    if item_idx != 0
        && item_idx < world.items.len()
        && world.items[item_idx].flags & ItemFlags::IF_SIGHTBLOCK.bits() != 0
    {
        return false;
    }

    true
}

#[allow(clippy::too_many_arguments)]
fn close_vis_see(
    world: &WorldView,
    vis: &[i8; VISI_BUFFER_LEN],
    ox: i32,
    oy: i32,
    is_monster: bool,
    x: i32,
    y: i32,
    value: i8,
) -> bool {
    if !check_map_see(world, is_monster, x, y) {
        return false;
    }

    let x = x - ox + VISI_CENTER;
    let y = y - oy + VISI_CENTER;
    let stride = VISI_STRIDE as i32;
    let edge = stride - 1;

    if x <= 0 || x >= edge || y <= 0 || y >= edge {
        return false;
    }

    let at = |dx: i32, dy: i32| vis[((x + dx) + (y + dy) * stride) as usize];

    at(1, 0) == value
        || at(-1, 0) == value
        || at(0, 1) == value
        || at(0, -1) == value
        || at(1, 1) == value
        || at(1, -1) == value
        || at(-1, 1) == value
        || at(-1, -1) == value
}

/// Rebuild `see` as a line-of-sight map centred on `(fx, fy)`.
fn build_see_map(
    world: &WorldView,
    see: &mut SeeMap,
    fx: i32,
    fy: i32,
    is_monster: bool,
    max_distance: i32,
) {
    see.vis.fill(0);
    see.x = fx;
    see.y = fy;

    add_vis(&mut see.vis, fx, fy, fx, fy, 1);

    for dist in 1..=max_distance {
        let value = dist as i8;

        for x in (fx - dist)..=(fx + dist) {
            let y = fy - dist;
            if close_vis_see(world, &see.vis, fx, fy, is_monster, x, y, value) {
                add_vis(&mut see.vis, fx, fy, x, y, dist + 1);
            }

            let y = fy + dist;
            if close_vis_see(world, &see.vis, fx, fy, is_monster, x, y, value) {
                add_vis(&mut see.vis, fx, fy, x, y, dist + 1);
            }
        }

        for y in (fy - dist + 1)..=(fy + dist - 1) {
            let x = fx - dist;
            if close_vis_see(world, &see.vis, fx, fy, is_monster, x, y, value) {
                add_vis(&mut see.vis, fx, fy, x, y, dist + 1);
            }

            let x = fx + dist;
            if close_vis_see(world, &see.vis, fx, fy, is_monster, x, y, value) {
                add_vis(&mut see.vis, fx, fy, x, y, dist + 1);
            }
        }
    }
}

/// Best visibility metric for `(tx, ty)` from the see-map's origin.
///
/// # Arguments
///
/// * `see` - Visibility cache whose `x`/`y` is the origin.
/// * `tx` - Target x.
/// * `ty` - Target y.
///
/// # Returns
///
/// * `0` when not visible, otherwise the smallest non-zero neighbour value.
pub fn check_vis(see: &SeeMap, tx: i32, ty: i32) -> i32 {
    let x = tx - see.x + VISI_CENTER;
    let y = ty - see.y + VISI_CENTER;
    let stride = VISI_STRIDE as i32;
    let edge = stride - 1;

    if x <= 0 || x >= edge || y <= 0 || y >= edge {
        return 0;
    }

    let mut best: i8 = 99;
    let at = |dx: i32, dy: i32| see.vis[((x + dx) + (y + dy) * stride) as usize];

    for (dx, dy) in [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ] {
        let v = at(dx, dy);
        if v != 0 && v < best {
            best = v;
        }
    }

    if best == 99 { 0 } else { i32::from(best) }
}

/// Port of `can_see(cn, fx, fy, tx, ty, max_distance)` for a per-character
/// see-map held outside `GameState`.
///
/// # Arguments
///
/// * `world` - Read-only world.
/// * `see` - The viewer's visibility cache.
/// * `cn` - Viewer character index (for monster sight rules).
/// * `fx` - Origin x.
/// * `fy` - Origin y.
/// * `tx` - Target x.
/// * `ty` - Target y.
/// * `max_distance` - Radius built on a cache miss.
/// * `deferred` - Receives hit/miss counter increments.
///
/// # Returns
///
/// * `0` when not visible, otherwise a positive metric (`1` = best).
#[allow(clippy::too_many_arguments)]
pub fn can_see(
    world: &WorldView,
    see: &mut SeeMap,
    cn: usize,
    fx: i32,
    fy: i32,
    tx: i32,
    ty: i32,
    max_distance: i32,
    deferred: &mut Deferred,
) -> i32 {
    if fx != see.x || fy != see.y {
        let ch = &world.characters[cn];
        let is_monster = ch.kindred & KIN_MONSTER as i32 != 0
            && (ch.flags & (CharacterFlags::Usurp.bits() | CharacterFlags::Thrall.bits())) == 0;
        build_see_map(world, see, fx, fy, is_monster, max_distance);
        deferred.see_miss += 1;
    } else {
        deferred.see_hit += 1;
    }

    check_vis(see, tx, ty)
}

/// Port of `do_char_can_see(cn, co)` over a [`WorldView`].
///
/// # Arguments
///
/// * `world` - Read-only world.
/// * `see` - The viewer's visibility cache.
/// * `cn` - Viewer character index.
/// * `co` - Target character index.
/// * `deferred` - Receives hit/miss counter increments.
///
/// # Returns
///
/// * `0` when not visible, `1` when adjacent, otherwise a distance metric.
pub fn char_can_see(
    world: &WorldView,
    see: &mut SeeMap,
    cn: usize,
    co: usize,
    deferred: &mut Deferred,
) -> i32 {
    if cn == co {
        return 1;
    }

    if co == 0 || cn == 0 {
        log::debug!(
            "do_char_can_see called with invalid character id(s): cn={}, co={}",
            cn,
            co
        );
        return 0;
    }

    let viewer = &world.characters[cn];
    let target = &world.characters[co];

    if target.used != USE_ACTIVE {
        return 0;
    }

    if target.flags & CharacterFlags::Invisible.bits() != 0
        && viewer.get_invisibility_level() < target.get_invisibility_level()
    {
        return 0;
    }

    if target.flags & CharacterFlags::Body.bits() != 0 {
        return 0;
    }

    let d1 = i32::from((viewer.x - target.x).abs());
    let d2 = i32::from((viewer.y - target.y).abs());

    let rd = d1 * d1 + d2 * d2;
    let mut d = rd;

    if d > 1000 {
        return 0;
    }

    let stealth = i32::from(target.skill[skills::SK_STEALTH][5]);
    d = match target.mode {
        0 => (d * (stealth + 20)) / 20,
        1 => (d * (stealth + 50)) / 50,
        _ => (d * (stealth + 100)) / 100,
    };

    d -= i32::from(viewer.skill[skills::SK_PERCEPT][5]) * 2;

    if viewer.flags & CharacterFlags::Infrared.bits() == 0 {
        let tx = target.x as usize;
        let ty = target.y as usize;
        let m = tx + ty * SERVER_MAPX as usize;
        let mut light = std::cmp::max(i32::from(world.map[m].light), world.check_dlight(tx, ty));

        light = calculate_light(viewer, light);

        if light == 0 {
            return 0;
        }

        if light > 64 {
            light = 64;
        }

        d += (64 - light) * 2;
    }

    if rd < 3 && d > 70 {
        d = 70;
    }

    if d > 200 {
        return 0;
    }

    let (vx, vy) = (i32::from(viewer.x), i32::from(viewer.y));
    let (tx, ty) = (i32::from(target.x), i32::from(target.y));
    if can_see(
        world,
        see,
        cn,
        vx,
        vy,
        tx,
        ty,
        (core::constants::TILEX / 2) as i32,
        deferred,
    ) == 0
    {
        return 0;
    }

    if d < 1 { 1 } else { d }
}

// ---------------------------------------------------------------------------
// Running updates
// ---------------------------------------------------------------------------

/// Run one player's full view update (`plr_getmap` then `plr_change`).
///
/// # Arguments
///
/// * `ctx` - Per-player update context.
///
/// # Returns
///
/// * Wall-clock split of the two phases (empty without `measure-time`).
pub fn update_player(ctx: &mut PlayerUpdateCtx) -> PhaseTimings {
    #[allow(unused_mut)]
    let mut timings = PhaseTimings::default();

    #[cfg(feature = "measure-time")]
    {
        let started = std::time::Instant::now();
        super::map::plr_getmap_ctx(ctx);
        timings.getmap = started.elapsed();

        let started = std::time::Instant::now();
        super::tick::plr_change_ctx(ctx);
        timings.change = started.elapsed();
    }

    #[cfg(not(feature = "measure-time"))]
    {
        super::map::plr_getmap_ctx(ctx);
        super::tick::plr_change_ctx(ctx);
    }

    timings
}

/// Split a `GameState` into the read-only world and the two mutable slices
/// the per-player update writes.
///
/// # Arguments
///
/// * `gs` - Game state to borrow from.
///
/// # Returns
///
/// * `(world, players, see_map)` with disjoint borrows.
pub fn split_world(gs: &mut GameState) -> (WorldView<'_>, &mut [ServerPlayer], &mut [SeeMap]) {
    let GameState {
        map,
        characters,
        items,
        globals,
        players,
        see_map,
        ..
    } = gs;

    let world = WorldView {
        map: map.as_slice(),
        characters: characters.as_slice(),
        items: items.as_slice(),
        globals,
    };

    (world, players.as_mut_slice(), see_map.as_mut_slice())
}

/// Apply the deferred world mutations recorded for one player.
///
/// # Arguments
///
/// * `gs` - Game state to mutate.
/// * `nr` - Player slot the effects belong to.
/// * `deferred` - Effects recorded during the update.
pub fn apply_deferred(gs: &mut GameState, nr: usize, deferred: Deferred) {
    for item_idx in deferred.clear_item_update {
        if let Some(item) = gs.items.get_mut(item_idx) {
            item.flags &= !ItemFlags::IF_UPDATE.bits();
        }
    }

    gs.see_hit += deferred.see_hit;
    gs.see_miss += deferred.see_miss;

    if deferred.overflow {
        network_manager::disconnect_after_tick_overflow(gs, nr);
    }
}

/// Run `f` against a per-player context for `nr`, then apply its deferred
/// effects. This is the serial path and the shim used by the legacy
/// `(gs, nr)` entry points.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `nr` - Player slot to update.
/// * `f` - Closure receiving the context.
///
/// # Returns
///
/// * The closure's return value.
///
/// # Panics
///
/// * Panics if `players[nr].usnr` is out of range for `see_map`.
pub fn with_player_ctx<R>(
    gs: &mut GameState,
    nr: usize,
    f: impl FnOnce(&mut PlayerUpdateCtx) -> R,
) -> R {
    let cn = gs.players[nr].usnr;

    let (result, deferred) = {
        let (world, players, see_map) = split_world(gs);
        // An out-of-range `usnr` is rejected inside `plr_change`; give it a
        // throwaway see-map so the guard is reached instead of a panic here.
        let mut scratch = SeeMap::default();
        let see = match see_map.get_mut(cn) {
            Some(see) => see,
            None => &mut scratch,
        };
        let mut ctx = PlayerUpdateCtx {
            world: &world,
            nr,
            cn,
            player: &mut players[nr],
            see,
            deferred: Deferred::default(),
        };
        let result = f(&mut ctx);
        (result, ctx.deferred)
    };

    apply_deferred(gs, nr, deferred);
    result
}

/// Summary of one fan-out pass.
#[derive(Debug, Default, Clone, Copy)]
pub struct UpdateSummary {
    /// Players updated this pass.
    pub players_updated: usize,
    /// Whether the worker pool was used.
    pub parallel: bool,
    /// Summed per-player phase timings (CPU time across workers when parallel).
    pub timings: PhaseTimings,
}

/// One unit of parallel work: a player slot plus its character's see-map.
struct PlayerUpdateJob<'a> {
    nr: usize,
    cn: usize,
    player: &'a mut ServerPlayer,
    see: &'a mut SeeMap,
    deferred: Deferred,
    timings: PhaseTimings,
}

impl PlayerUpdateJob<'_> {
    fn run(&mut self, world: &WorldView) {
        let mut ctx = PlayerUpdateCtx {
            world,
            nr: self.nr,
            cn: self.cn,
            player: &mut *self.player,
            see: &mut *self.see,
            deferred: std::mem::take(&mut self.deferred),
        };
        self.timings = update_player(&mut ctx);
        self.deferred = ctx.deferred;
    }
}

/// Player slots that receive a view update this tick.
fn collect_update_targets(gs: &GameState) -> Vec<usize> {
    (1..gs.players.len())
        .filter(|&n| gs.players[n].sock.is_some() && gs.players[n].state == ST_NORMAL)
        .collect()
}

/// Update every `ST_NORMAL` player, on the pool when one is given and enough
/// players are online, otherwise serially in slot order.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `pool` - Worker pool, or `None` to force the serial path.
///
/// # Returns
///
/// * Summary of the pass.
pub fn run_player_updates(gs: &mut GameState, pool: Option<&rayon::ThreadPool>) -> UpdateSummary {
    let targets = collect_update_targets(gs);

    match pool {
        Some(pool) if targets.len() >= PARALLEL_MIN_PLAYERS => run_parallel(gs, pool, &targets),
        _ => run_serial(gs, &targets),
    }
}

fn run_serial(gs: &mut GameState, targets: &[usize]) -> UpdateSummary {
    let mut summary = UpdateSummary::default();
    for &nr in targets {
        let timings = with_player_ctx(gs, nr, update_player);
        summary.timings += timings;
        summary.players_updated += 1;
    }
    summary
}

fn run_parallel(gs: &mut GameState, pool: &rayon::ThreadPool, targets: &[usize]) -> UpdateSummary {
    use rayon::prelude::*;

    let mut summary = UpdateSummary {
        parallel: true,
        ..UpdateSummary::default()
    };
    let mut finished: Vec<(usize, Deferred)> = Vec::with_capacity(targets.len());
    // Players whose character slot could not be claimed (duplicate `usnr`).
    let mut leftovers: Vec<usize> = Vec::new();

    {
        let (world, players, see_map) = split_world(gs);

        let mut wanted = vec![false; players.len()];
        for &nr in targets {
            wanted[nr] = true;
        }

        let mut see_slots: Vec<Option<&mut SeeMap>> = see_map.iter_mut().map(Some).collect();

        let mut jobs: Vec<PlayerUpdateJob> = Vec::with_capacity(targets.len());
        for (nr, player) in players.iter_mut().enumerate() {
            if !wanted[nr] {
                continue;
            }
            let cn = player.usnr;
            match see_slots.get_mut(cn).and_then(Option::take) {
                Some(see) => jobs.push(PlayerUpdateJob {
                    nr,
                    cn,
                    player,
                    see,
                    deferred: Deferred::default(),
                    timings: PhaseTimings::default(),
                }),
                None => leftovers.push(nr),
            }
        }

        pool.install(|| {
            jobs.par_iter_mut().for_each(|job| job.run(&world));
        });

        for job in jobs {
            summary.timings += job.timings;
            summary.players_updated += 1;
            finished.push((job.nr, job.deferred));
        }
    }

    for (nr, deferred) in finished {
        apply_deferred(gs, nr, deferred);
    }

    for nr in leftovers {
        log::warn!(
            "player {} could not claim its see-map (usnr {}); updating serially",
            nr,
            gs.players[nr].usnr
        );
        summary.timings += with_player_ctx(gs, nr, update_player);
        summary.players_updated += 1;
    }

    summary
}

// ---------------------------------------------------------------------------
// Worker pool configuration
// ---------------------------------------------------------------------------

/// Parse a `MAG_TICK_WORKERS` value.
///
/// # Arguments
///
/// * `raw` - Environment value, if set.
///
/// # Returns
///
/// * `Some(n)` for a positive integer, `None` when unset or invalid.
pub fn parse_tick_worker_override(raw: Option<&str>) -> Option<usize> {
    let raw = raw?.trim();
    match raw.parse::<usize>() {
        Ok(n) if n >= 1 => Some(n),
        _ => {
            log::warn!(
                "Ignoring invalid {}='{}' (expected a positive integer)",
                TICK_WORKERS_ENV,
                raw
            );
            None
        }
    }
}

/// Number of tick workers to use: the env override when valid, else the
/// machine's available parallelism.
///
/// # Returns
///
/// * Worker count, always `>= 1`.
pub fn resolve_tick_worker_count() -> usize {
    let override_value = std::env::var(TICK_WORKERS_ENV).ok();
    if let Some(n) = parse_tick_worker_override(override_value.as_deref()) {
        return n;
    }

    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Build the tick worker pool.
///
/// # Arguments
///
/// * `workers` - Requested thread count; `<= 1` disables the pool.
///
/// # Returns
///
/// * `Some(pool)` when parallel updates are enabled, `None` for serial.
pub fn build_tick_worker_pool(workers: usize) -> Option<rayon::ThreadPool> {
    if workers <= 1 {
        log::info!("Tick worker pool disabled; per-player updates run serially");
        return None;
    }

    match rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .thread_name(|i| format!("tick-worker-{i}"))
        .build()
    {
        Ok(pool) => {
            log::info!(
                "Tick worker pool started with {} threads ({} players or more fan out)",
                workers,
                PARALLEL_MIN_PLAYERS
            );
            Some(pool)
        }
        Err(e) => {
            log::error!("Failed to build tick worker pool: {e}; running serially");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_test_gs;
    use crate::tls::GameStream;
    use core::constants::{MF_SIGHTBLOCK, TILEX};
    use core::types::Character;
    use std::net::{TcpListener, TcpStream};

    fn attach_test_socket(gs: &mut GameState, nr: usize) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let client = TcpStream::connect(addr).expect("connect client");
        let (server, _) = listener.accept().expect("accept client");
        drop(client);
        gs.players[nr].sock = Some(GameStream::Plain(server));
    }

    fn map_index(x: i32, y: i32) -> usize {
        (x + y * SERVER_MAPX) as usize
    }

    fn place_player(gs: &mut GameState, nr: usize, cn: usize, x: i16, y: i16) {
        gs.players[nr].state = ST_NORMAL;
        gs.players[nr].usnr = cn;
        attach_test_socket(gs, nr);

        let ch = &mut gs.characters[cn];
        *ch = Character::default();
        ch.used = USE_ACTIVE;
        ch.flags = CharacterFlags::Player.bits();
        ch.player = nr as i32;
        ch.x = x;
        ch.y = y;
        ch.skill[skills::SK_PERCEPT][5] = 10;
        ch.hp[5] = 10;
        ch.a_hp = 10_000;
        gs.map[map_index(i32::from(x), i32::from(y))].ch = cn as u32;
    }

    #[test]
    fn parse_tick_worker_override_accepts_positive_and_rejects_rest() {
        assert_eq!(parse_tick_worker_override(None), None);
        assert_eq!(parse_tick_worker_override(Some("")), None);
        assert_eq!(parse_tick_worker_override(Some("0")), None);
        assert_eq!(parse_tick_worker_override(Some("abc")), None);
        assert_eq!(parse_tick_worker_override(Some("-3")), None);
        assert_eq!(parse_tick_worker_override(Some(" 6 ")), Some(6));
        assert_eq!(parse_tick_worker_override(Some("1")), Some(1));
    }

    #[test]
    fn build_tick_worker_pool_disabled_for_single_worker() {
        assert!(build_tick_worker_pool(0).is_none());
        assert!(build_tick_worker_pool(1).is_none());
        let pool = build_tick_worker_pool(2).expect("pool");
        assert_eq!(pool.current_num_threads(), 2);
    }

    #[test]
    fn resolve_tick_worker_count_is_at_least_one() {
        assert!(resolve_tick_worker_count() >= 1);
    }

    #[test]
    fn check_vis_respects_border_and_picks_smallest_neighbour() {
        let mut see = SeeMap {
            x: 100,
            y: 100,
            ..SeeMap::default()
        };
        // Target at origin: neighbours are unset -> not visible.
        assert_eq!(check_vis(&see, 100, 100), 0);

        let stride = VISI_STRIDE as i32;
        let c = VISI_CENTER;
        see.vis[((c + 1) + c * stride) as usize] = 5;
        see.vis[(c + (c + 1) * stride) as usize] = 3;
        assert_eq!(check_vis(&see, 100, 100), 3);

        // Outside the 1-tile border.
        assert_eq!(check_vis(&see, 100 + c, 100), 0);
        assert_eq!(check_vis(&see, 100 - c, 100), 0);
    }

    #[test]
    fn view_can_see_matches_game_state_can_see() {
        with_test_gs(|gs| {
            let cn = 1;
            let (fx, fy) = (200, 200);
            gs.characters[cn] = Character::default();
            gs.characters[cn].used = USE_ACTIVE;

            // A short wall east of the viewer.
            for dy in -3..=3 {
                gs.map[map_index(fx + 4, fy + dy)].flags |= u64::from(MF_SIGHTBLOCK);
            }

            let radius = (TILEX / 2) as i32;
            let mut expected = Vec::new();
            for ty in (fy - 10)..=(fy + 10) {
                for tx in (fx - 10)..=(fx + 10) {
                    expected.push(gs.can_see(Some(cn), fx, fy, tx, ty, radius));
                }
            }
            let reference_see = gs.see_map[cn];

            let mut see = SeeMap::default();
            let mut deferred = Deferred::default();
            let (world, _players, _see_map) = split_world(gs);
            let mut actual = Vec::new();
            for ty in (fy - 10)..=(fy + 10) {
                for tx in (fx - 10)..=(fx + 10) {
                    actual.push(can_see(
                        &world,
                        &mut see,
                        cn,
                        fx,
                        fy,
                        tx,
                        ty,
                        radius,
                        &mut deferred,
                    ));
                }
            }

            assert_eq!(actual, expected);
            assert_eq!(see.vis[..], reference_see.vis[..]);
            assert_eq!(deferred.see_miss, 1);
            assert_eq!(deferred.see_hit as usize, expected.len() - 1);
        });
    }

    #[test]
    fn ctx_xsend_records_overflow_instead_of_disconnecting() {
        with_test_gs(|gs| {
            place_player(gs, 1, 1, 300, 300);
            let cap = gs.players[1].tbuf.len();
            gs.players[1].tptr = cap - 4;

            let deferred = {
                let (world, players, see_map) = split_world(gs);
                let mut ctx = PlayerUpdateCtx {
                    world: &world,
                    nr: 1,
                    cn: 1,
                    player: &mut players[1],
                    see: &mut see_map[1],
                    deferred: Deferred::default(),
                };
                ctx.xsend(&[1, 2, 3], 3);
                assert_eq!(ctx.player.tptr, cap - 1);
                ctx.xsend(&[9], 1);
                assert!(ctx.deferred.overflow);
                // Further sends are dropped once overflowed.
                ctx.xsend(&[9], 1);
                assert_eq!(ctx.player.tptr, cap - 1);
                ctx.deferred
            };

            assert!(gs.players[1].sock.is_some());
            apply_deferred(gs, 1, deferred);
            assert!(gs.players[1].sock.is_none());
        });
    }

    #[test]
    fn apply_deferred_clears_item_update_flags() {
        with_test_gs(|gs| {
            gs.items[5].flags |= ItemFlags::IF_UPDATE.bits();
            gs.items[6].flags |= ItemFlags::IF_UPDATE.bits();
            let deferred = Deferred {
                clear_item_update: vec![5, usize::MAX],
                see_hit: 2,
                see_miss: 1,
                ..Deferred::default()
            };
            apply_deferred(gs, 1, deferred);
            assert_eq!(gs.items[5].flags & ItemFlags::IF_UPDATE.bits(), 0);
            assert_ne!(gs.items[6].flags & ItemFlags::IF_UPDATE.bits(), 0);
            assert_eq!(gs.see_hit, 2);
            assert_eq!(gs.see_miss, 1);
        });
    }

    type Snapshot = (Vec<u8>, Vec<crate::player::map::CMap>, [i32; 4], SeeMap);

    /// Snapshot of everything a player update is allowed to write.
    fn player_snapshot(gs: &GameState, nr: usize) -> Snapshot {
        let p = &gs.players[nr];
        (
            p.tbuf[..p.tptr].to_vec(),
            p.cmap.to_vec(),
            [p.vx, p.vy, p.last_dlight, p.cpl.x],
            gs.see_map[p.usnr],
        )
    }

    #[test]
    fn parallel_and_serial_updates_produce_identical_output() {
        const PLAYERS: usize = 12;

        fn setup(gs: &mut GameState) {
            gs.globals.dlight = 200;
            gs.globals.ticker = 77;
            for i in 0..PLAYERS {
                let nr = i + 1;
                let cn = i + 1;
                // Cluster players so they see each other, plus a few far away.
                let (x, y) = if i < 8 {
                    (400 + (i as i16 % 4) * 3, 400 + (i as i16 / 4) * 3)
                } else {
                    (600 + (i as i16) * 20, 600)
                };
                place_player(gs, nr, cn, x, y);
                gs.characters[cn].gold = 1000 + i as i32;
                gs.characters[cn].dir = (i % 8) as u8;
            }
            // Some scenery and a light-blocking wall in the cluster.
            for dy in 0..6 {
                gs.map[map_index(405, 398 + dy)].flags |= u64::from(MF_SIGHTBLOCK);
            }
            gs.map[map_index(403, 403)].light = 40;
        }

        fn move_char(gs: &mut GameState, cn: usize, dx: i16) {
            let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
            gs.map[map_index(i32::from(x), i32::from(y))].ch = 0;
            gs.characters[cn].x = x + dx;
            gs.map[map_index(i32::from(x + dx), i32::from(y))].ch = cn as u32;
        }

        fn run(workers: Option<usize>) -> Vec<Snapshot> {
            with_test_gs(move |gs| {
                setup(gs);
                let pool = workers.map(|n| build_tick_worker_pool(n).expect("pool"));
                // Two ticks: the first builds full views, the second exercises
                // the delta path and light/scroll batching.
                let first = run_player_updates(gs, pool.as_ref());
                assert_eq!(first.players_updated, PLAYERS);
                assert_eq!(first.parallel, pool.is_some());
                for nr in 1..=PLAYERS {
                    gs.players[nr].tptr = 0;
                }
                gs.globals.ticker += 1;
                move_char(gs, 1, 1);
                move_char(gs, 3, 1);
                gs.globals.dlight = 150;
                let second = run_player_updates(gs, pool.as_ref());
                assert_eq!(second.players_updated, PLAYERS);
                (1..=PLAYERS).map(|nr| player_snapshot(gs, nr)).collect()
            })
        }

        let serial = run(None);
        let parallel = run(Some(4));

        assert_eq!(serial.len(), parallel.len());
        for (nr, (s, p)) in serial.iter().zip(parallel.iter()).enumerate() {
            // Clustered players moved/saw movement; the far ones may be idle.
            if nr < 8 {
                assert!(!s.0.is_empty(), "player {} produced no tick bytes", nr + 1);
            }
            assert_eq!(s.0, p.0, "tbuf differs for player {}", nr + 1);
            assert_eq!(s.1, p.1, "cmap differs for player {}", nr + 1);
            assert_eq!(s.2, p.2, "scalars differ for player {}", nr + 1);
            assert_eq!(
                s.3.vis[..],
                p.3.vis[..],
                "see-map differs for player {}",
                nr + 1
            );
        }
    }
}
