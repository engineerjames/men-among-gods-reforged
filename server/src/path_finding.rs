//! A* pathfinding implementation
//!
//! This module provides pathfinding capabilities for characters to navigate
//! through the game world, taking into account obstacles, movement costs,
//! and directional constraints.

use std::cmp::Ordering;
use std::cmp::{max, min};
use std::collections::BinaryHeap;

use core::constants::*;

use crate::repository::Repository;
use parking_lot::ReentrantMutex;
use std::cell::UnsafeCell;
use std::sync::OnceLock;

static PATHFINDER: OnceLock<ReentrantMutex<UnsafeCell<PathFinder>>> = OnceLock::new();

const MAX_NODES: usize = 4096;

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
    /// All nodes allocated for the current search
    nodes: Vec<Node>,
    /// Priority queue of nodes to visit
    open_set: BinaryHeap<Node>,
    /// Tracks which nodes have been visited
    visited: Vec<bool>,
    /// Bad target tracking
    bad_targets: Vec<BadTarget>,
    /// Set when exceeding maxstep allocations.
    failed: bool,
}

impl PathFinder {
    /// Create a new pathfinder
    pub fn new() -> Self {
        let map_size = (SERVER_MAPX * SERVER_MAPY) as usize;
        Self {
            node_map: vec![None; map_size],
            nodes: Vec::with_capacity(MAX_NODES),
            open_set: BinaryHeap::with_capacity(MAX_NODES),
            visited: vec![false; MAX_NODES],
            bad_targets: vec![BadTarget { tick: 0 }; map_size],
            failed: false,
        }
    }

    /// Reset internal search state prior to running a new A* invocation.
    ///
    /// Clears node allocations, the open set, and visited flags so the
    /// PathFinder can be reused for a subsequent search.
    fn reset(&mut self) {
        // Clear node map
        for slot in &mut self.node_map {
            *slot = None;
        }
        self.nodes.clear();
        self.open_set.clear();
        for v in &mut self.visited {
            *v = false;
        }
        self.failed = false;
    }

    /// Check if a target location is marked as bad
    fn is_bad_target(&self, x: i16, y: i16, current_tick: u32) -> bool {
        let idx = (x as i32 + y as i32 * SERVER_MAPX) as usize;
        self.bad_targets[idx].tick > current_tick
    }

    /// Mark a target location as bad
    fn add_bad_target(&mut self, x: i16, y: i16, current_tick: u32) {
        let idx = (x as i32 + y as i32 * SERVER_MAPX) as usize;
        self.bad_targets[idx].tick = current_tick + 1;
    }

    /// Calculate heuristic cost from (fx, fy) to target
    fn heuristic_cost(&self, fx: i16, fy: i16, tx: i16, ty: i16) -> i32 {
        let dx = (fx - tx).abs() as i32;
        let dy = (fy - ty).abs() as i32;

        if dx > dy {
            (dx << 1) + dy
        } else {
            (dy << 1) + dx
        }
    }

    /// Calculate cost considering direction and targets
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
    fn is_passable(&self, m: usize, mapblock: u64) -> bool {
        Repository::with_map(|map| {
            // Check map flags
            if (map[m].flags & mapblock) != 0 {
                return false;
            }

            // Check for characters blocking
            if map[m].ch != 0 || map[m].to_ch != 0 {
                return false;
            }

            // Check for blocking items
            let item_idx = map[m].it as usize;
            if item_idx != 0 && item_idx < core::constants::MAXITEM {
                let should_return_false = Repository::with_items(|it| {
                    if (it[item_idx].flags & ItemFlags::IF_MOVEBLOCK.bits()) != 0
                        && it[item_idx].driver != 2
                    {
                        return true;
                    }

                    false
                });

                if should_return_false {
                    return false;
                }
            }

            true
        })
    }

    /// Add a node to the search
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
            log::warn!("add_node: out of bounds x={}, y={}", x, y);
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
        self.open_set.push(node);

        true
    }

    /// Add successor nodes for a given node
    fn add_successors(
        &mut self,
        node: &Node,
        mapblock: u64,
        mode: u8,
        tx1: i16,
        ty1: i16,
        tx2: i16,
        ty2: i16,
        max_step: usize,
    ) {
        let base_x = node.x as i32;
        let base_y = node.y as i32;

        // Cardinal directions
        let right_m = (base_x + 1 + base_y * SERVER_MAPX) as usize;
        let left_m = (base_x - 1 + base_y * SERVER_MAPX) as usize;
        let down_m = (base_x + (base_y + 1) * SERVER_MAPX) as usize;
        let up_m = (base_x + (base_y - 1) * SERVER_MAPX) as usize;

        let can_right = self.is_passable(right_m, mapblock);
        let can_left = self.is_passable(left_m, mapblock);
        let can_down = self.is_passable(down_m, mapblock);
        let can_up = self.is_passable(up_m, mapblock);

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
            if self.is_passable(rd_m, mapblock) {
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
            if self.is_passable(ru_m, mapblock) {
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
            if self.is_passable(ld_m, mapblock) {
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
            if self.is_passable(lu_m, mapblock) {
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
    fn astar(
        &mut self,
        fx: i16,
        fy: i16,
        cdir: u8,
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

        let start_m = (fx as i32 + fy as i32 * SERVER_MAPX) as usize;
        self.nodes.push(start_node);
        self.node_map[start_m] = Some(0);
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
            self.add_successors(&current, mapblock, mode, tx1, ty1, tx2, ty2, max_step);

            if self.failed {
                break;
            }
        }

        None
    }

    /// Find path from character to target
    ///
    /// # Arguments
    /// * `character` - The character doing the pathfinding
    /// * `x1`, `y1` - Primary target coordinates
    /// * `flag` - Mode: 0 = exact target, 1 = adjacent to target, 2 = two targets
    /// * `x2`, `y2` - Secondary target coordinates (used in mode 2)
    ///
    /// # Returns
    /// Direction to move, or None if no path found
    pub fn find_path(
        &mut self,
        cn: usize,
        x1: i16,
        y1: i16,
        flag: u8,
        x2: i16,
        y2: i16,
    ) -> Option<u8> {
        // Bounds checking
        // TODO: Confirm this is the right tick value
        Repository::with_characters(|ch| {
            let current_tick = Repository::with_globals(|globs| globs.ticker);
            if ch[cn].x < 1 || ch[cn].x >= SERVER_MAPX as i16 {
                return None;
            }
            if ch[cn].y < 1 || ch[cn].y >= SERVER_MAPY as i16 {
                return None;
            }
            if x1 < 1 || x1 >= SERVER_MAPX as i16 {
                return None;
            }
            if y1 < 1 || y1 >= SERVER_MAPY as i16 {
                return None;
            }
            if x2 < 0 || x2 >= SERVER_MAPX as i16 {
                return None;
            }
            if y2 < 0 || y2 >= SERVER_MAPY as i16 {
                return None;
            }

            // Check if target is marked as bad
            if self.is_bad_target(x1, y1, current_tick as u32) {
                return None;
            }

            // Determine movement blocking flags
            let mapblock = if (ch[cn].kindred as u32 & KIN_MONSTER) != 0
                && (ch[cn].flags
                    & (CharacterFlags::CF_USURP.bits() | CharacterFlags::CF_THRALL.bits()))
                    == 0
            {
                MF_NOMONST as u64 | MF_MOVEBLOCK as u64
            } else {
                MF_MOVEBLOCK as u64
            };

            let mapblock = if (ch[cn].flags
                & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits()))
                == 0
            {
                mapblock | MF_DEATHTRAP as u64
            } else {
                mapblock
            };

            // Check if target is passable (for exact target mode)
            if flag == 0 {
                let target_m = (x1 as i32 + y1 as i32 * SERVER_MAPX) as usize;
                if !self.is_passable(target_m, mapblock) {
                    return None;
                }
            }

            // Calculate max steps
            let distance = max((ch[cn].x - x1).abs(), (ch[cn].y - y1).abs()) as usize;
            let mut max_step = if ch[cn].attack_cn != 0
                || ((ch[cn].flags
                    & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits()))
                    == 0
                    && ch[cn].data[78] != 0)
            {
                distance * 4 + 50
            } else {
                distance * 8 + 100
            };

            // Special case for temp == 498 (hack for grolmy in stunrun.c)
            if ch[cn].temp == 498 {
                max_step += 4000;
            }

            if max_step > MAX_NODES {
                max_step = MAX_NODES;
            }

            // Reset state for new search
            self.reset();

            // Run A* search
            let result = self.astar(
                ch[cn].x, ch[cn].y, ch[cn].dir, mapblock, flag, x1, y1, x2, y2, max_step,
            );

            // Mark as bad target if failed
            if result.is_none() {
                self.add_bad_target(x1, y1, current_tick as u32);
            }

            result
        })
    }

    /// Initialize the global PathFinder singleton with static allocations.
    ///
    /// This mirrors `Repository::initialize()` semantics and must be called
    /// during server startup prior to using `with`/`with_mut`.
    pub fn initialize() -> Result<(), String> {
        let pf = PathFinder::new();
        PATHFINDER
            .set(ReentrantMutex::new(UnsafeCell::new(pf)))
            .map_err(|_| "PathFinder already initialized".to_string())?;
        Ok(())
    }

    /// Execute `f` with a read-only reference to the global PathFinder.
    #[allow(dead_code)]
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&PathFinder) -> R,
    {
        let lock = PATHFINDER.get().expect("PathFinder not initialized");
        let guard = lock.lock();
        let inner: &UnsafeCell<PathFinder> = &guard;
        // SAFETY: We are holding the ReentrantMutex, providing exclusive
        // access for mutation or shared access for read-only usages from a
        // single thread. Returning a shared reference is safe here.
        let pf_ref: &PathFinder = unsafe { &*inner.get() };
        f(pf_ref)
    }

    /// Execute `f` with a mutable reference to the global PathFinder.
    pub fn with_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut PathFinder) -> R,
    {
        let lock = PATHFINDER.get().expect("PathFinder not initialized");
        let guard = lock.lock();
        let inner: &UnsafeCell<PathFinder> = &guard;
        // SAFETY: We have exclusive access to the PathFinder under the
        // ReentrantMutex; returning a mutable reference is safe.
        let pf_mut: &mut PathFinder = unsafe { &mut *inner.get() };
        f(pf_mut)
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

    if dir1 == DX_UP {
        if dir2 == DX_DOWN {
            return 4;
        }
        if dir2 == DX_RIGHTUP || dir2 == DX_LEFTUP {
            return 1;
        }
        if dir2 == DX_RIGHT || dir2 == DX_LEFT {
            return 2;
        }
        return 3;
    }

    if dir1 == DX_DOWN {
        if dir2 == DX_UP {
            return 4;
        }
        if dir2 == DX_RIGHTDOWN || dir2 == DX_LEFTDOWN {
            return 1;
        }
        if dir2 == DX_RIGHT || dir2 == DX_LEFT {
            return 2;
        }
        return 3;
    }

    if dir1 == DX_LEFT {
        if dir2 == DX_RIGHT {
            return 4;
        }
        if dir2 == DX_LEFTUP || dir2 == DX_LEFTDOWN {
            return 1;
        }
        if dir2 == DX_UP || dir2 == DX_DOWN {
            return 2;
        }
        return 3;
    }

    if dir1 == DX_RIGHT {
        if dir2 == DX_LEFT {
            return 4;
        }
        if dir2 == DX_RIGHTUP || dir2 == DX_RIGHTDOWN {
            return 1;
        }
        if dir2 == DX_UP || dir2 == DX_DOWN {
            return 2;
        }
        return 3;
    }

    if dir1 == DX_LEFTUP {
        if dir2 == DX_RIGHTDOWN {
            return 4;
        }
        if dir2 == DX_UP || dir2 == DX_LEFT {
            return 1;
        }
        if dir2 == DX_RIGHTUP || dir2 == DX_LEFTDOWN {
            return 2;
        }
        return 3;
    }

    if dir1 == DX_LEFTDOWN {
        if dir2 == DX_RIGHTUP {
            return 4;
        }
        if dir2 == DX_DOWN || dir2 == DX_LEFT {
            return 1;
        }
        if dir2 == DX_RIGHTDOWN || dir2 == DX_LEFTUP {
            return 2;
        }
        return 3;
    }

    if dir1 == DX_RIGHTUP {
        if dir2 == DX_LEFTDOWN {
            return 4;
        }
        if dir2 == DX_UP || dir2 == DX_RIGHT {
            return 1;
        }
        if dir2 == DX_RIGHTDOWN || dir2 == DX_LEFTUP {
            return 2;
        }
        return 3;
    }

    if dir1 == DX_RIGHTDOWN {
        if dir2 == DX_LEFTUP {
            return 4;
        }
        if dir2 == DX_DOWN || dir2 == DX_RIGHT {
            return 1;
        }
        if dir2 == DX_RIGHTUP || dir2 == DX_LEFTDOWN {
            return 2;
        }
        return 3;
    }

    99 // Invalid direction
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
        // Initialize singleton (ok if already initialized)
        let _ = PathFinder::initialize();
        PathFinder::with(|pf| {
            assert_eq!(pf.nodes.capacity(), MAX_NODES);
            assert_eq!(pf.node_map.len(), (SERVER_MAPX * SERVER_MAPY) as usize);
        });
    }
}
