//! A* pathfinding implementation
//!
//! This module provides pathfinding capabilities for characters to navigate
//! through the game world, taking into account obstacles, movement costs,
//! and directional constraints.

use std::cmp::Ordering;
use std::cmp::{max, min};
use std::collections::BinaryHeap;
use std::time::Instant;

use core::{constants::*, traits};

const MAX_NODES: usize = 4096;

/// Aggregated pathfinding measurements for one reporting interval.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PathFindingStats {
    /// Number of pathfinding requests observed.
    pub calls: u64,
    /// Number of requests that produced a movement direction.
    pub successes: u64,
    /// Number of requests that did not produce a movement direction.
    pub failures: u64,
    /// Number of requests skipped because the target was recently known bad.
    pub bad_target_skips: u64,
    /// Number of requests whose search limit was capped at `MAX_NODES`.
    pub max_step_cap_hits: u64,
    /// Total elapsed time spent in pathfinding requests, in microseconds.
    pub total_elapsed_micros: u64,
    /// Maximum elapsed time for a single pathfinding request, in microseconds.
    pub max_elapsed_micros: u64,
    /// Total node allocations observed across requests.
    pub total_nodes: u64,
    /// Maximum node allocations observed for a single request.
    pub max_nodes: usize,
    /// Total visited nodes observed across requests.
    pub total_visited: u64,
    /// Maximum visited nodes observed for a single request.
    pub max_visited: usize,
}

impl PathFindingStats {
    /// Returns whether this interval contains no pathfinding requests.
    ///
    /// # Returns
    ///
    /// * `true` when no pathfinding calls were recorded.
    pub fn is_empty(&self) -> bool {
        self.calls == 0
    }

    /// Returns mean request time for this interval, in milliseconds.
    ///
    /// # Returns
    ///
    /// * Mean elapsed request time in milliseconds, or `0.0` with no calls.
    pub fn mean_elapsed_ms(&self) -> f64 {
        if self.calls == 0 {
            0.0
        } else {
            self.total_elapsed_micros as f64 / self.calls as f64 / 1000.0
        }
    }

    /// Returns maximum request time for this interval, in milliseconds.
    ///
    /// # Returns
    ///
    /// * Maximum elapsed request time in milliseconds.
    pub fn max_elapsed_ms(&self) -> f64 {
        self.max_elapsed_micros as f64 / 1000.0
    }

    /// Returns mean allocated-node count for this interval.
    ///
    /// # Returns
    ///
    /// * Mean allocated nodes per request, or `0.0` with no calls.
    pub fn mean_nodes(&self) -> f64 {
        if self.calls == 0 {
            0.0
        } else {
            self.total_nodes as f64 / self.calls as f64
        }
    }

    /// Returns mean visited-node count for this interval.
    ///
    /// # Returns
    ///
    /// * Mean visited nodes per request, or `0.0` with no calls.
    pub fn mean_visited(&self) -> f64 {
        if self.calls == 0 {
            0.0
        } else {
            self.total_visited as f64 / self.calls as f64
        }
    }

    /// Merge another interval into this one.
    ///
    /// # Arguments
    ///
    /// * `other` - Measurements to add to this interval.
    pub fn merge(&mut self, other: Self) {
        self.calls = self.calls.saturating_add(other.calls);
        self.successes = self.successes.saturating_add(other.successes);
        self.failures = self.failures.saturating_add(other.failures);
        self.bad_target_skips = self.bad_target_skips.saturating_add(other.bad_target_skips);
        self.max_step_cap_hits = self
            .max_step_cap_hits
            .saturating_add(other.max_step_cap_hits);
        self.total_elapsed_micros = self
            .total_elapsed_micros
            .saturating_add(other.total_elapsed_micros);
        self.max_elapsed_micros = self.max_elapsed_micros.max(other.max_elapsed_micros);
        self.total_nodes = self.total_nodes.saturating_add(other.total_nodes);
        self.max_nodes = self.max_nodes.max(other.max_nodes);
        self.total_visited = self.total_visited.saturating_add(other.total_visited);
        self.max_visited = self.max_visited.max(other.max_visited);
    }
}

/// Character fields required by pathfinding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathFindingCharacter {
    /// Current X coordinate.
    pub x: i16,
    /// Current Y coordinate.
    pub y: i16,
    /// Current facing direction.
    pub dir: u8,
    /// Character kindred flags.
    pub kindred: i32,
    /// Character runtime flags.
    pub flags: u64,
    /// Current attack target, if any.
    pub attack_cn: u16,
    /// Driver-specific pathfinding hint stored in character data slot 78.
    pub data_78: i32,
    /// Template id used for legacy pathfinding special cases.
    pub temp: u16,
}

impl PathFindingCharacter {
    /// Copy pathfinding-relevant fields from a full character record.
    ///
    /// # Arguments
    ///
    /// * `character` - Full game character record to snapshot.
    ///
    /// # Returns
    ///
    /// * A compact pathfinding character snapshot.
    pub fn from_character(character: &core::types::Character) -> Self {
        Self {
            x: character.x,
            y: character.y,
            dir: character.dir,
            kindred: character.kindred,
            flags: character.flags,
            attack_cn: character.attack_cn,
            data_78: character.data[78],
            temp: character.temp,
        }
    }

    /// Return map flags that block this character's movement.
    ///
    /// # Returns
    ///
    /// * Bitmask of map flags treated as impassable.
    pub fn mapblock(self) -> u64 {
        let mapblock = if (self.kindred as u32 & traits::KIN_MONSTER) != 0
            && (self.flags & (CharacterFlags::Usurp.bits() | CharacterFlags::Thrall.bits())) == 0
        {
            u64::from(MF_NOMONST) | u64::from(MF_MOVEBLOCK)
        } else {
            u64::from(MF_MOVEBLOCK)
        };

        if (self.flags & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits())) == 0 {
            mapblock | u64::from(MF_DEATHTRAP)
        } else {
            mapblock
        }
    }
}

/// Target coordinates and mode for one pathfinding request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathFindingTarget {
    /// Primary target X coordinate.
    pub x1: i16,
    /// Primary target Y coordinate.
    pub y1: i16,
    /// Mode: 0 = exact target, 1 = adjacent to target, 2 = two targets.
    pub flag: u8,
    /// Secondary target X coordinate for mode 2.
    pub x2: i16,
    /// Secondary target Y coordinate for mode 2.
    pub y2: i16,
}

impl PathFindingTarget {
    /// Create a pathfinding target tuple.
    ///
    /// # Arguments
    ///
    /// * `x1` - Primary target X coordinate.
    /// * `y1` - Primary target Y coordinate.
    /// * `flag` - Pathfinding mode.
    /// * `x2` - Secondary target X coordinate.
    /// * `y2` - Secondary target Y coordinate.
    ///
    /// # Returns
    ///
    /// * A target tuple suitable for pathfinding requests.
    pub fn new(x1: i16, y1: i16, flag: u8, x2: i16, y2: i16) -> Self {
        Self {
            x1,
            y1,
            flag,
            x2,
            y2,
        }
    }
}

/// Owned inputs for one pathfinding request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathFindingRequest {
    /// Character fields used by pathfinding.
    pub character: PathFindingCharacter,
    /// Current world tick for bad-target suppression.
    pub current_tick: u32,
    /// Target coordinates and mode.
    pub target: PathFindingTarget,
}

impl PathFindingRequest {
    /// Build a request by copying fields from a full character record.
    ///
    /// # Arguments
    ///
    /// * `character` - Full game character record to snapshot.
    /// * `current_tick` - Current world tick for bad-target suppression.
    /// * `target` - Target coordinates and mode.
    ///
    /// # Returns
    ///
    /// * A pathfinding request suitable for synchronous or worker execution.
    pub fn from_character(
        character: &core::types::Character,
        current_tick: u32,
        target: PathFindingTarget,
    ) -> Self {
        Self {
            character: PathFindingCharacter::from_character(character),
            current_tick,
            target,
        }
    }
}

/// Source of movement passability for A* expansion.
pub(crate) trait PassabilitySource {
    /// Return whether the linear map tile index can be traversed.
    ///
    /// # Arguments
    ///
    /// * `m` - Linear map tile index.
    /// * `mapblock` - Movement-blocking map flags for the character.
    ///
    /// # Returns
    ///
    /// * `true` when the tile is passable.
    fn is_passable(&self, m: usize, mapblock: u64) -> bool;
}

struct WorldPassability<'a> {
    map: &'a [core::types::Map],
    items: &'a [core::types::Item],
}

impl PassabilitySource for WorldPassability<'_> {
    fn is_passable(&self, m: usize, mapblock: u64) -> bool {
        PathFinder::is_world_tile_passable(self.map, self.items, m, mapblock)
    }
}

/// Compact passability snapshot for worker-thread pathfinding.
pub struct SnapshotPassability {
    origin_x: usize,
    origin_y: usize,
    width: usize,
    height: usize,
    passable: Vec<bool>,
}

impl SnapshotPassability {
    /// Build a rectangular passability snapshot around a request.
    ///
    /// # Arguments
    ///
    /// * `map` - Read-only world map tiles.
    /// * `items` - Read-only item table used for move-block checks.
    /// * `mapblock` - Movement-blocking map flags for the character.
    /// * `request` - Pathfinding request used to choose the rectangle.
    /// * `margin` - Extra tiles to include around the start and target bounds.
    ///
    /// # Returns
    ///
    /// * A compact passability snapshot covering the chosen rectangle.
    pub fn from_world_window(
        map: &[core::types::Map],
        items: &[core::types::Item],
        mapblock: u64,
        request: &PathFindingRequest,
        margin: usize,
    ) -> Self {
        let character = request.character;
        let target = request.target;

        let min_target_x = target.x1.min(target.x2.max(0));
        let max_target_x = target.x1.max(target.x2.max(0));
        let min_target_y = target.y1.min(target.y2.max(0));
        let max_target_y = target.y1.max(target.y2.max(0));

        let min_x = character.x.min(min_target_x).max(1) as usize;
        let max_x = character.x.max(max_target_x).max(1) as usize;
        let min_y = character.y.min(min_target_y).max(1) as usize;
        let max_y = character.y.max(max_target_y).max(1) as usize;

        let origin_x = min_x.saturating_sub(margin).max(1);
        let origin_y = min_y.saturating_sub(margin).max(1);
        let end_x = (max_x + margin).min(SERVER_MAPX as usize - 1);
        let end_y = (max_y + margin).min(SERVER_MAPY as usize - 1);
        let width = end_x.saturating_sub(origin_x) + 1;
        let height = end_y.saturating_sub(origin_y) + 1;

        let mut passable = Vec::with_capacity(width * height);
        for y in origin_y..=end_y {
            for x in origin_x..=end_x {
                let m = x + y * SERVER_MAPX as usize;
                passable.push(PathFinder::is_world_tile_passable(map, items, m, mapblock));
            }
        }

        Self {
            origin_x,
            origin_y,
            width,
            height,
            passable,
        }
    }
}

impl PassabilitySource for SnapshotPassability {
    fn is_passable(&self, m: usize, _mapblock: u64) -> bool {
        let x = m % SERVER_MAPX as usize;
        let y = m / SERVER_MAPX as usize;
        if x < self.origin_x || y < self.origin_y {
            return false;
        }
        let rel_x = x - self.origin_x;
        let rel_y = y - self.origin_y;
        if rel_x >= self.width || rel_y >= self.height {
            return false;
        }
        self.passable
            .get(rel_x + rel_y * self.width)
            .copied()
            .unwrap_or(false)
    }
}

/// A node in the A* search graph
#[derive(Clone, Copy, Debug)]
struct Node {
    /// X coordinate
    x: i16,
    /// Y coordinate
    y: i16,
    /// Direction we originally came from (0 = none)
    dir: u8,
    /// Total estimated cost to reach goal
    tcost: i32,
    /// Cost of steps so far
    cost: i32,
    /// Current direction of travel
    cdir: u8,
    /// Index in the nodes array
    index: usize,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.tcost == other.tcost
    }
}

impl Eq for Node {}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior
        other.tcost.cmp(&self.tcost)
    }
}

/// Tracks bad target locations that have recently failed pathfinding
#[derive(Clone, Copy)]
struct BadTarget {
    tick: u32,
}

/// Pathfinder state
pub struct PathFinder {
    /// Map of coordinates to node indices (using flat array indexed by x + y * SERVER_MAPX)
    node_map: Vec<Option<usize>>,
    /// Node map slots touched by the current/previous search.
    touched_node_map: Vec<usize>,
    /// All nodes allocated for the current search
    nodes: Vec<Node>,
    /// Priority queue of nodes to visit
    open_set: BinaryHeap<Node>,
    /// Tracks which nodes have been visited
    visited: Vec<bool>,
    /// Visited indices touched by the current/previous search.
    touched_visited: Vec<usize>,
    /// Bad target tracking
    bad_targets: Vec<BadTarget>,
    /// Set when exceeding maxstep allocations.
    failed: bool,
    /// Aggregated measurements since the last report.
    interval_stats: PathFindingStats,
}

impl PathFinder {
    /// Create a new pathfinder
    ///
    /// # Returns
    ///
    /// * A new instance configured by `new`.
    pub fn new() -> Self {
        let map_size = (SERVER_MAPX * SERVER_MAPY) as usize;
        Self {
            node_map: vec![None; map_size],
            touched_node_map: Vec::with_capacity(MAX_NODES),
            nodes: Vec::with_capacity(MAX_NODES),
            open_set: BinaryHeap::with_capacity(MAX_NODES),
            visited: vec![false; MAX_NODES],
            touched_visited: Vec::with_capacity(MAX_NODES),
            bad_targets: vec![BadTarget { tick: 0 }; map_size],
            failed: false,
            interval_stats: PathFindingStats::default(),
        }
    }

    /// Return and reset pathfinding measurements for the current interval.
    ///
    /// # Returns
    ///
    /// * Aggregated measurements captured since the previous call.
    pub fn take_interval_stats(&mut self) -> PathFindingStats {
        let stats = self.interval_stats;
        self.interval_stats = PathFindingStats::default();
        stats
    }

    /// Merge externally-collected pathfinding measurements into this interval.
    ///
    /// # Arguments
    ///
    /// * `stats` - Measurements collected by pathfinding workers.
    pub fn merge_interval_stats(&mut self, stats: PathFindingStats) {
        self.interval_stats.merge(stats);
    }

    /// Record one completed pathfinding request in the interval counters.
    fn record_request_stats(
        &mut self,
        started_at: Instant,
        result: Option<u8>,
        bad_target_skip: bool,
        max_step_capped: bool,
        nodes: usize,
        visited: usize,
    ) -> Option<u8> {
        let elapsed_micros = started_at.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;

        self.interval_stats.calls += 1;
        if result.is_some() {
            self.interval_stats.successes += 1;
        } else {
            self.interval_stats.failures += 1;
        }
        if bad_target_skip {
            self.interval_stats.bad_target_skips += 1;
        }
        if max_step_capped {
            self.interval_stats.max_step_cap_hits += 1;
        }

        self.interval_stats.total_elapsed_micros = self
            .interval_stats
            .total_elapsed_micros
            .saturating_add(elapsed_micros);
        self.interval_stats.max_elapsed_micros =
            self.interval_stats.max_elapsed_micros.max(elapsed_micros);
        self.interval_stats.total_nodes =
            self.interval_stats.total_nodes.saturating_add(nodes as u64);
        self.interval_stats.max_nodes = self.interval_stats.max_nodes.max(nodes);
        self.interval_stats.total_visited = self
            .interval_stats
            .total_visited
            .saturating_add(visited as u64);
        self.interval_stats.max_visited = self.interval_stats.max_visited.max(visited);

        result
    }

    /// Reset internal search state prior to running a new A* invocation.
    ///
    /// Clears node allocations, the open set, and visited flags so the
    /// PathFinder can be reused for a subsequent search.
    fn reset(&mut self) {
        for &idx in &self.touched_node_map {
            self.node_map[idx] = None;
        }
        self.touched_node_map.clear();

        for &idx in &self.touched_visited {
            self.visited[idx] = false;
        }
        self.touched_visited.clear();

        self.nodes.clear();
        self.open_set.clear();
        self.failed = false;
    }

    /// Check if a target location is marked as bad
    fn is_bad_target(&self, x: i16, y: i16, current_tick: u32) -> bool {
        let idx = (i32::from(x) + i32::from(y) * SERVER_MAPX) as usize;
        self.bad_targets[idx].tick > current_tick
    }

    /// Mark a target location as bad
    fn add_bad_target(&mut self, x: i16, y: i16, current_tick: u32) {
        let idx = (i32::from(x) + i32::from(y) * SERVER_MAPX) as usize;
        self.bad_targets[idx].tick = current_tick + 1;
    }

    /// Calculate heuristic cost from (fx, fy) to target
    fn heuristic_cost(&self, fx: i16, fy: i16, tx: i16, ty: i16) -> i32 {
        let dx = i32::from((fx - tx).abs());
        let dy = i32::from((fy - ty).abs());

        if dx > dy {
            (dx << 1) + dy
        } else {
            (dy << 1) + dx
        }
    }

    /// Calculate cost considering direction and targets
    #[allow(clippy::too_many_arguments)]
    fn calculate_cost(
        &self,
        fx: i16,
        fy: i16,
        cdir: u8,
        mode: u8,
        tx1: i16,
        ty1: i16,
        tx2: i16,
        ty2: i16,
    ) -> i32 {
        if mode == 0 || mode == 1 {
            self.heuristic_cost(fx, fy, tx1, ty1)
        } else {
            // Mode 2: two possible targets
            let ndir1 = dcoor_to_dir(tx1 - fx, ty1 - fy);
            let dirdiff1 = turn_count(cdir, ndir1);

            let ndir2 = dcoor_to_dir(tx2 - fx, ty2 - fy);
            let dirdiff2 = turn_count(cdir, ndir2);

            let cost1 = self.heuristic_cost(fx, fy, tx1, ty1) + 12 + dirdiff1;
            let cost2 = self.heuristic_cost(fx, fy, tx2, ty2) + dirdiff2;

            min(cost1, cost2)
        }
    }

    /// Check if a map tile is passable
    pub(crate) fn is_world_tile_passable(
        map: &[core::types::Map],
        items: &[core::types::Item],
        m: usize,
        mapblock: u64,
    ) -> bool {
        if (map[m].flags & mapblock) != 0 {
            return false;
        }

        if map[m].ch != 0 || map[m].to_ch != 0 {
            return false;
        }

        let item_idx = map[m].it as usize;
        if item_idx != 0
            && item_idx < core::constants::MAXITEM
            && (items[item_idx].flags & ItemFlags::IF_MOVEBLOCK.bits()) != 0
            && items[item_idx].driver != 2
        {
            return false;
        }

        true
    }

    /// Add a node to the search
    #[allow(clippy::too_many_arguments)]
    fn add_node(
        &mut self,
        x: i16,
        y: i16,
        dir: u8,
        ccost: i32,
        cdir: u8,
        mode: u8,
        tx1: i16,
        ty1: i16,
        tx2: i16,
        ty2: i16,
        max_step: usize,
    ) -> bool {
        if x < 1 || x >= SERVER_MAPX as i16 || y < 1 || y >= SERVER_MAPY as i16 {
            debug_assert!(
                false,
                "add_node out of bounds x={}, y={} (expected this to be filtered earlier)",
                x, y
            );
            return false;
        }

        let m = x as usize + y as usize * SERVER_MAPX as usize;
        let gcost = self.calculate_cost(x, y, cdir, mode, tx1, ty1, tx2, ty2);
        let tcost = ccost + gcost;

        // Check if we already have a node at this position
        if let Some(existing_idx) = self.node_map[m] {
            let existing_node = &self.nodes[existing_idx];

            // If existing node is better or equal, skip
            if existing_node.tcost <= tcost {
                return false;
            }

            // Match C++ behavior: allow "reopening" nodes even if previously visited.
            let updated_node = Node {
                x,
                y,
                dir,
                tcost,
                cost: ccost,
                cdir,
                index: existing_idx,
            };
            self.nodes[existing_idx] = updated_node;
            self.visited[existing_idx] = false;
            self.open_set.push(updated_node);
            return true;
        }

        // Create new node
        if self.nodes.len() >= max_step {
            self.failed = true;
            return false;
        }

        let index = self.nodes.len();
        let node = Node {
            x,
            y,
            dir,
            tcost,
            cost: ccost,
            cdir,
            index,
        };

        self.nodes.push(node);
        self.node_map[m] = Some(index);
        self.touched_node_map.push(m);
        self.open_set.push(node);

        true
    }

    /// Add successor nodes for a given node
    #[allow(clippy::too_many_arguments)]
    fn add_successors(
        &mut self,
        node: &Node,
        passability: &impl PassabilitySource,
        mapblock: u64,
        mode: u8,
        tx1: i16,
        ty1: i16,
        tx2: i16,
        ty2: i16,
        max_step: usize,
    ) {
        let base_x = i32::from(node.x);
        let base_y = i32::from(node.y);

        // Cardinal directions
        let right_m = (base_x + 1 + base_y * SERVER_MAPX) as usize;
        let left_m = (base_x - 1 + base_y * SERVER_MAPX) as usize;
        let down_m = (base_x + (base_y + 1) * SERVER_MAPX) as usize;
        let up_m = (base_x + (base_y - 1) * SERVER_MAPX) as usize;

        let can_right = passability.is_passable(right_m, mapblock);
        let can_left = passability.is_passable(left_m, mapblock);
        let can_down = passability.is_passable(down_m, mapblock);
        let can_up = passability.is_passable(up_m, mapblock);

        // Right
        if can_right {
            let cost = node.cost + 2 + turn_count(node.cdir, DX_RIGHT);
            self.add_node(
                node.x + 1,
                node.y,
                if node.dir == 0 { DX_RIGHT } else { node.dir },
                cost,
                DX_RIGHT,
                mode,
                tx1,
                ty1,
                tx2,
                ty2,
                max_step,
            );
        }

        // Left
        if can_left {
            let cost = node.cost + 2 + turn_count(node.cdir, DX_LEFT);
            self.add_node(
                node.x - 1,
                node.y,
                if node.dir == 0 { DX_LEFT } else { node.dir },
                cost,
                DX_LEFT,
                mode,
                tx1,
                ty1,
                tx2,
                ty2,
                max_step,
            );
        }

        // Down
        if can_down {
            let cost = node.cost + 2 + turn_count(node.cdir, DX_DOWN);
            self.add_node(
                node.x,
                node.y + 1,
                if node.dir == 0 { DX_DOWN } else { node.dir },
                cost,
                DX_DOWN,
                mode,
                tx1,
                ty1,
                tx2,
                ty2,
                max_step,
            );
        }

        // Up
        if can_up {
            let cost = node.cost + 2 + turn_count(node.cdir, DX_UP);
            self.add_node(
                node.x,
                node.y - 1,
                if node.dir == 0 { DX_UP } else { node.dir },
                cost,
                DX_UP,
                mode,
                tx1,
                ty1,
                tx2,
                ty2,
                max_step,
            );
        }

        // Diagonal directions (only if both cardinal directions are passable)

        // Right-Down
        if can_right && can_down {
            let rd_m = (base_x + 1 + (base_y + 1) * SERVER_MAPX) as usize;
            if passability.is_passable(rd_m, mapblock) {
                let cost = node.cost + 3 + turn_count(node.cdir, DX_RIGHTDOWN);
                self.add_node(
                    node.x + 1,
                    node.y + 1,
                    if node.dir == 0 {
                        DX_RIGHTDOWN
                    } else {
                        node.dir
                    },
                    cost,
                    DX_RIGHTDOWN,
                    mode,
                    tx1,
                    ty1,
                    tx2,
                    ty2,
                    max_step,
                );
            }
        }

        // Right-Up
        if can_right && can_up {
            let ru_m = (base_x + 1 + (base_y - 1) * SERVER_MAPX) as usize;
            if passability.is_passable(ru_m, mapblock) {
                let cost = node.cost + 3 + turn_count(node.cdir, DX_RIGHTUP);
                self.add_node(
                    node.x + 1,
                    node.y - 1,
                    if node.dir == 0 { DX_RIGHTUP } else { node.dir },
                    cost,
                    DX_RIGHTUP,
                    mode,
                    tx1,
                    ty1,
                    tx2,
                    ty2,
                    max_step,
                );
            }
        }

        // Left-Down
        if can_left && can_down {
            let ld_m = (base_x - 1 + (base_y + 1) * SERVER_MAPX) as usize;
            if passability.is_passable(ld_m, mapblock) {
                let cost = node.cost + 3 + turn_count(node.cdir, DX_LEFTDOWN);
                self.add_node(
                    node.x - 1,
                    node.y + 1,
                    if node.dir == 0 { DX_LEFTDOWN } else { node.dir },
                    cost,
                    DX_LEFTDOWN,
                    mode,
                    tx1,
                    ty1,
                    tx2,
                    ty2,
                    max_step,
                );
            }
        }

        // Left-Up
        if can_left && can_up {
            let lu_m = (base_x - 1 + (base_y - 1) * SERVER_MAPX) as usize;
            if passability.is_passable(lu_m, mapblock) {
                let cost = node.cost + 3 + turn_count(node.cdir, DX_LEFTUP);
                self.add_node(
                    node.x - 1,
                    node.y - 1,
                    if node.dir == 0 { DX_LEFTUP } else { node.dir },
                    cost,
                    DX_LEFTUP,
                    mode,
                    tx1,
                    ty1,
                    tx2,
                    ty2,
                    max_step,
                );
            }
        }
    }

    /// Run A* search
    #[allow(clippy::too_many_arguments)]
    fn astar(
        &mut self,
        fx: i16,
        fy: i16,
        cdir: u8,
        passability: &impl PassabilitySource,
        mapblock: u64,
        mode: u8,
        tx1: i16,
        ty1: i16,
        tx2: i16,
        ty2: i16,
        max_step: usize,
    ) -> Option<u8> {
        // Add start node
        let start_cost = self.calculate_cost(fx, fy, cdir, mode, tx1, ty1, tx2, ty2);
        let start_node = Node {
            x: fx,
            y: fy,
            dir: 0,
            tcost: start_cost,
            cost: 0,
            cdir,
            index: 0,
        };

        let start_m = (i32::from(fx) + i32::from(fy) * SERVER_MAPX) as usize;
        self.nodes.push(start_node);
        self.node_map[start_m] = Some(0);
        self.touched_node_map.push(start_m);
        self.open_set.push(start_node);

        while let Some(current) = self.open_set.pop() {
            if self.failed {
                break;
            }
            // Skip if already visited
            if self.visited[current.index] {
                continue;
            }

            // Mark as visited
            self.visited[current.index] = true;
            self.touched_visited.push(current.index);

            // Check if we reached the goal
            if mode == 0 && current.x == tx1 && current.y == ty1 {
                return Some(self.nodes[current.index].dir);
            }

            if mode != 0 {
                let dx = (current.x - tx1).abs();
                let dy = (current.y - ty1).abs();
                if dx + dy == 1 {
                    return Some(self.nodes[current.index].dir);
                }

                if mode == 2 {
                    let dx2 = (current.x - tx2).abs();
                    let dy2 = (current.y - ty2).abs();
                    if dx2 + dy2 == 1 {
                        return Some(self.nodes[current.index].dir);
                    }
                }
            }

            // Add successors
            self.add_successors(
                &current,
                passability,
                mapblock,
                mode,
                tx1,
                ty1,
                tx2,
                ty2,
                max_step,
            );

            if self.failed {
                break;
            }
        }

        None
    }

    /// Find a path for copied request fields and a passability source.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn find_path_for_request(
        &mut self,
        request: &PathFindingRequest,
        passability: &impl PassabilitySource,
    ) -> Option<u8> {
        let started_at = Instant::now();
        let character = request.character;
        let target = request.target;

        // Bounds checking
        if character.x < 1 || character.x >= SERVER_MAPX as i16 {
            return self.record_request_stats(started_at, None, false, false, 0, 0);
        }
        if character.y < 1 || character.y >= SERVER_MAPY as i16 {
            return self.record_request_stats(started_at, None, false, false, 0, 0);
        }
        if target.x1 < 1 || target.x1 >= SERVER_MAPX as i16 {
            return self.record_request_stats(started_at, None, false, false, 0, 0);
        }
        if target.y1 < 1 || target.y1 >= SERVER_MAPY as i16 {
            return self.record_request_stats(started_at, None, false, false, 0, 0);
        }
        if target.x2 < 0 || target.x2 >= SERVER_MAPX as i16 {
            return self.record_request_stats(started_at, None, false, false, 0, 0);
        }
        if target.y2 < 0 || target.y2 >= SERVER_MAPY as i16 {
            return self.record_request_stats(started_at, None, false, false, 0, 0);
        }

        // Check if target is marked as bad
        if self.is_bad_target(target.x1, target.y1, request.current_tick) {
            return self.record_request_stats(started_at, None, true, false, 0, 0);
        }

        let mapblock = character.mapblock();

        // Check if target is passable (for exact target mode)
        if target.flag == 0 {
            let target_m = (i32::from(target.x1) + i32::from(target.y1) * SERVER_MAPX) as usize;
            if !passability.is_passable(target_m, mapblock) {
                return self.record_request_stats(started_at, None, false, false, 0, 0);
            }
        }

        // Calculate max steps
        let distance = max(
            (character.x - target.x1).abs(),
            (character.y - target.y1).abs(),
        ) as usize;
        let mut max_step = if character.attack_cn != 0
            || ((character.flags & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
                == 0
                && character.data_78 != 0)
        {
            distance * 4 + 50
        } else {
            distance * 8 + 100
        };

        // Special case for temp == 498 (hack for grolmy in stunrun.c)
        if character.temp == 498 {
            max_step += 4000;
        }

        let max_step_capped = max_step > MAX_NODES;
        if max_step > MAX_NODES {
            max_step = MAX_NODES;
        }

        // Reset state for new search
        self.reset();

        // Run A* search
        let result = self.astar(
            character.x,
            character.y,
            character.dir,
            passability,
            mapblock,
            target.flag,
            target.x1,
            target.y1,
            target.x2,
            target.y2,
            max_step,
        );

        // Mark as bad target if failed
        if result.is_none() {
            self.add_bad_target(target.x1, target.y1, request.current_tick);
        }

        self.record_request_stats(
            started_at,
            result,
            false,
            max_step_capped,
            self.nodes.len(),
            self.touched_visited.len(),
        )
    }

    /// Find path from character to target.
    ///
    /// # Arguments
    /// * `character` - The character doing the pathfinding.
    /// * `map` - Read-only world map tiles.
    /// * `items` - Read-only item table used for move-block checks.
    /// * `current_tick` - Current world tick for bad-target suppression.
    /// * `x1`, `y1` - Primary target coordinates.
    /// * `flag` - Mode: 0 = exact target, 1 = adjacent to target, 2 = two targets.
    /// * `x2`, `y2` - Secondary target coordinates (used in mode 2).
    ///
    /// # Returns
    /// Direction to move, or `None` if no path is found.
    #[allow(clippy::too_many_arguments)]
    pub fn find_path(
        &mut self,
        character: &core::types::Character,
        map: &[core::types::Map],
        items: &[core::types::Item],
        current_tick: u32,
        x1: i16,
        y1: i16,
        flag: u8,
        x2: i16,
        y2: i16,
    ) -> Option<u8> {
        let target = PathFindingTarget::new(x1, y1, flag, x2, y2);
        let request = PathFindingRequest::from_character(character, current_tick, target);
        let passability = WorldPassability { map, items };
        self.find_path_for_request(&request, &passability)
    }
}

impl Default for PathFinder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert delta coordinates to a movement direction constant.
///
/// Port-style documentation:
/// - Purpose: Translate a delta (dx, dy) into one of the eight
///   direction constants (e.g. `DX_RIGHT`, `DX_LEFTUP`, etc.).
/// - Responsibilities: Return the best-matching direction for a single
///   step towards the given delta. If there is no movement (dx == 0
///   and dy == 0) the function returns `0`.
///
/// # Arguments
/// * `dx` - Signed delta in the X axis.
/// * `dy` - Signed delta in the Y axis.
///
/// # Returns
/// Direction constant (`u8`) or `0` for no movement.
fn dcoor_to_dir(dx: i16, dy: i16) -> u8 {
    match (dx.signum(), dy.signum()) {
        (1, 1) => DX_RIGHTDOWN,
        (1, 0) => DX_RIGHT,
        (1, -1) => DX_RIGHTUP,
        (0, 1) => DX_DOWN,
        (0, -1) => DX_UP,
        (-1, 1) => DX_LEFTDOWN,
        (-1, 0) => DX_LEFT,
        (-1, -1) => DX_LEFTUP,
        _ => 0, // No movement
    }
}

/// Calculate number of turns needed to change from dir1 to dir2
/// Calculate the number of turns required to change from `dir1` to `dir2`.
///
/// Port-style documentation:
/// - Purpose: Provide a simple cost metric for changing facing direction
///   used by the pathfinder when accounting for turn penalties.
/// - Responsibilities: Return a small integer cost (commonly 0..4)
///   representing how many "turn steps" are needed. Returns `99` for
///   invalid/unrecognized directions.
///
/// # Arguments
/// * `dir1` - Current direction constant.
/// * `dir2` - Target direction constant.
///
/// # Returns
/// Integer turn cost; `99` indicates an invalid input direction.
fn turn_count(dir1: u8, dir2: u8) -> i32 {
    if dir1 == dir2 {
        return 0;
    }

    let slot1 = direction_slot(dir1);
    let slot2 = direction_slot(dir2);

    let (Some(slot1), Some(slot2)) = (slot1, slot2) else {
        return 99;
    };

    let diff = (slot1 - slot2).abs();
    min(diff, 8 - diff)
}

fn direction_slot(dir: u8) -> Option<i32> {
    match dir {
        DX_UP => Some(0),
        DX_RIGHTUP => Some(1),
        DX_RIGHT => Some(2),
        DX_RIGHTDOWN => Some(3),
        DX_DOWN => Some(4),
        DX_LEFTDOWN => Some(5),
        DX_LEFT => Some(6),
        DX_LEFTUP => Some(7),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dcoor_to_dir() {
        assert_eq!(dcoor_to_dir(1, 0), DX_RIGHT);
        assert_eq!(dcoor_to_dir(-1, 0), DX_LEFT);
        assert_eq!(dcoor_to_dir(0, 1), DX_DOWN);
        assert_eq!(dcoor_to_dir(0, -1), DX_UP);
        assert_eq!(dcoor_to_dir(1, 1), DX_RIGHTDOWN);
        assert_eq!(dcoor_to_dir(1, -1), DX_RIGHTUP);
        assert_eq!(dcoor_to_dir(-1, 1), DX_LEFTDOWN);
        assert_eq!(dcoor_to_dir(-1, -1), DX_LEFTUP);
        assert_eq!(dcoor_to_dir(0, 0), 0);
    }

    #[test]
    fn test_turn_count() {
        // Same direction
        assert_eq!(turn_count(DX_UP, DX_UP), 0);

        // Opposite directions
        assert_eq!(turn_count(DX_UP, DX_DOWN), 4);
        assert_eq!(turn_count(DX_LEFT, DX_RIGHT), 4);

        // Adjacent directions
        assert_eq!(turn_count(DX_UP, DX_RIGHTUP), 1);
        assert_eq!(turn_count(DX_UP, DX_LEFTUP), 1);

        // Perpendicular directions
        assert_eq!(turn_count(DX_UP, DX_RIGHT), 2);
        assert_eq!(turn_count(DX_UP, DX_LEFT), 2);
    }

    #[test]
    fn test_pathfinder_creation() {
        let pf = PathFinder::new();
        assert_eq!(pf.nodes.capacity(), MAX_NODES);
        assert_eq!(pf.node_map.len(), (SERVER_MAPX * SERVER_MAPY) as usize);
    }

    #[test]
    fn request_entrypoint_matches_find_path() {
        let mut character = core::types::Character {
            x: 10,
            y: 10,
            dir: DX_RIGHT,
            flags: CharacterFlags::Player.bits(),
            ..core::types::Character::default()
        };
        character.data[78] = 0;
        let map_len = SERVER_MAPX as usize * SERVER_MAPY as usize;
        let map = vec![core::types::Map::default(); map_len];
        let items = vec![core::types::Item::default(); MAXITEM];

        let mut legacy_pathfinder = PathFinder::new();
        let legacy = legacy_pathfinder.find_path(&character, &map, &items, 1, 12, 10, 0, 0, 0);

        let target = PathFindingTarget::new(12, 10, 0, 0, 0);
        let request = PathFindingRequest::from_character(&character, 1, target);
        let passability = WorldPassability {
            map: &map,
            items: &items,
        };
        let mut request_pathfinder = PathFinder::new();
        let request_result = request_pathfinder.find_path_for_request(&request, &passability);

        assert_eq!(legacy, request_result);
        assert_eq!(request_result, Some(DX_RIGHT));
    }
}
