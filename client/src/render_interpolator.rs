//! Frame-time interpolation for smooth sub-tile sprite motion.
//!
//! The server runs at a fixed tick rate ([`mag_core::constants::TICKS`]), but
//! the client may render at a higher frame rate (e.g. 60 Hz, 144 Hz). To avoid
//! visible snapping on tick boundaries, the renderer interpolates the four
//! per-tile offset fields (`obj_xoff`, `obj_yoff`, `ovl_xoff`, `ovl_yoff`)
//! between the two most recent server tick snapshots.
//!
//! This module deliberately does **not** interpolate sprite IDs, animation
//! frames, or flags; those switch instantly on tick boundaries. It also does
//! not predict local-player input or extrapolate remote entities.

use std::time::{Duration, Instant};

use mag_core::constants::{TICKS, TILEX, TILEY};

use crate::{game_map::GameMap, types::map::TileRenderOffset};

/// Expected wall-clock duration of one server tick.
///
/// Computed from [`TICKS`] as `1_000_000 / TICKS` microseconds.
const TICK_PERIOD_MICROS: u64 = 1_000_000 / TICKS as u64;

/// Holds the previous and current server-tick offset snapshots and produces
/// interpolated [`TileRenderOffset`] values for arbitrary frame times.
///
/// The interpolator keeps two pre-allocated buffers of length
/// `TILEX * TILEY`. Calling [`advance`](Self::advance) rotates them and copies
/// the latest tile offsets from the [`GameMap`] without allocating.
#[derive(Debug)]
pub struct RenderInterpolator {
    prev: Vec<TileRenderOffset>,
    current: Vec<TileRenderOffset>,
    prev_character_keys: Vec<u32>,
    current_character_keys: Vec<u32>,
    prev_time: Option<Instant>,
    current_time: Option<Instant>,
}

impl Default for RenderInterpolator {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderInterpolator {
    /// Creates a new interpolator with two zero-initialized buffers.
    ///
    /// # Returns
    ///
    /// * A new `RenderInterpolator` ready to receive tick snapshots.
    pub fn new() -> Self {
        let count = TILEX * TILEY;
        Self {
            prev: vec![TileRenderOffset::default(); count],
            current: vec![TileRenderOffset::default(); count],
            prev_character_keys: vec![0; count],
            current_character_keys: vec![0; count],
            prev_time: None,
            current_time: None,
        }
    }

    /// Rotates the internal buffers and copies the current map offsets into
    /// the new "current" snapshot.
    ///
    /// This method does not allocate; it reuses the pre-allocated buffers.
    ///
    /// # Arguments
    ///
    /// * `map` - The visible tile map after `legacy_engine::engine_tick` has
    ///   mutated it.
    /// * `received_at` - The instant the tick packet was fully read by the
    ///   network thread.
    pub fn advance(&mut self, map: &GameMap, received_at: Instant) {
        self.advance_with_invalidation(map, received_at, false);
    }

    /// Rotates snapshots and optionally invalidates interpolation for this
    /// tick transition.
    ///
    /// Use `invalidate_previous = true` when the tile grid has shifted (for
    /// example due to map scroll). Snapshot buffers are indexed by tile slot,
    /// so after a shift, blending `prev[idx]` with `current[idx]` would mix
    /// different world cells and can cause a visible camera/object jump.
    ///
    /// # Arguments
    ///
    /// * `map` - Latest visible map after tick processing.
    /// * `received_at` - Time the tick packet was received.
    /// * `invalidate_previous` - If `true`, this advance starts a fresh
    ///   interpolation window from the new snapshot (`alpha = 0` behavior).
    pub fn advance_with_invalidation(
        &mut self,
        map: &GameMap,
        received_at: Instant,
        invalidate_previous: bool,
    ) {
        std::mem::swap(&mut self.prev, &mut self.current);
        std::mem::swap(
            &mut self.prev_character_keys,
            &mut self.current_character_keys,
        );
        self.prev_time = self.current_time;
        self.current_time = Some(received_at);

        for idx in 0..map.len().min(self.current.len()) {
            if let Some(tile) = map.tile_at_index(idx) {
                let slot = &mut self.current[idx];
                slot.obj_xoff = tile.obj_xoff as f32;
                slot.obj_yoff = tile.obj_yoff as f32;
                slot.ovl_xoff = tile.ovl_xoff as f32;
                slot.ovl_yoff = tile.ovl_yoff as f32;
                self.current_character_keys[idx] = character_key(tile.ch_nr, tile.ch_id);
            }
        }

        if invalidate_previous {
            self.prev.copy_from_slice(&self.current);
            self.prev_character_keys
                .copy_from_slice(&self.current_character_keys);
            self.prev_time = None;
        }
    }

    /// Returns the interpolated offset for `tile_idx` at `now`.
    ///
    /// Before any tick has been advanced, this returns the default (zero)
    /// offset. After the first tick it returns the first snapshot unchanged
    /// (`alpha = 0.0`). After two or more ticks it linearly blends from the
    /// previous snapshot toward the current one, clamping `alpha` to
    /// `[0.0, 1.0]` so stalls never extrapolate.
    ///
    /// # Arguments
    ///
    /// * `tile_idx` - Flat index into the `TILEX * TILEY` tile grid.
    /// * `now` - Frame time used to compute the interpolation fraction.
    ///
    /// # Returns
    ///
    /// * The interpolated [`TileRenderOffset`] for `tile_idx`, or the default
    ///   zero offset if `tile_idx` is out of bounds.
    pub fn interpolated_offset(&self, tile_idx: usize, now: Instant) -> TileRenderOffset {
        let current = self
            .current
            .get(tile_idx)
            .copied()
            .unwrap_or(TileRenderOffset::default());

        if self.prev_time.is_none() {
            return current;
        }
        let Some(current_time) = self.current_time else {
            return current;
        };
        let Some(prev) = self.prev.get(tile_idx).copied() else {
            return current;
        };

        let elapsed = now.saturating_duration_since(current_time);
        let alpha = alpha_from_elapsed(elapsed);

        TileRenderOffset {
            obj_xoff: lerp(prev.obj_xoff, current.obj_xoff, alpha),
            obj_yoff: lerp(prev.obj_yoff, current.obj_yoff, alpha),
            ovl_xoff: lerp(prev.ovl_xoff, current.ovl_xoff, alpha),
            ovl_yoff: lerp(prev.ovl_yoff, current.ovl_yoff, alpha),
        }
    }

    /// Returns the interpolated offset for a tile containing character sprites.
    ///
    /// If the tile occupant changed between snapshots (including appearing or
    /// disappearing), interpolation is skipped and the current offset is
    /// returned. This avoids blending unrelated entities across the same tile
    /// slot when NPCs/players cross tile boundaries.
    ///
    /// # Arguments
    ///
    /// * `tile_idx` - Flat index into the `TILEX * TILEY` tile grid.
    /// * `now` - Frame time used to compute interpolation.
    ///
    /// # Returns
    ///
    /// * Interpolated offset when occupant is stable, otherwise current offset.
    pub fn interpolated_character_offset(&self, tile_idx: usize, now: Instant) -> TileRenderOffset {
        let current = self
            .current
            .get(tile_idx)
            .copied()
            .unwrap_or(TileRenderOffset::default());

        let prev_key = self.prev_character_keys.get(tile_idx).copied().unwrap_or(0);
        let current_key = self
            .current_character_keys
            .get(tile_idx)
            .copied()
            .unwrap_or(0);

        if prev_key != current_key {
            return current;
        }

        self.interpolated_offset(tile_idx, now)
    }

    /// Returns the interpolation alpha that would be used for `now`.
    ///
    /// Useful for debug overlays and tests.
    ///
    /// # Arguments
    ///
    /// * `now` - Frame time to evaluate.
    ///
    /// # Returns
    ///
    /// * `0.0` before the first tick, otherwise `clamp(elapsed / tick_period,
    ///   0.0, 1.0)`.
    pub fn alpha_at(&self, now: Instant) -> f32 {
        let Some(current_time) = self.current_time else {
            return 0.0;
        };
        alpha_from_elapsed(now.saturating_duration_since(current_time))
    }
}

/// Computes the interpolation fraction from elapsed time since the current
/// tick arrived.
///
/// # Arguments
///
/// * `elapsed` - Time since the current tick snapshot was captured.
///
/// # Returns
///
/// * `elapsed / tick_period`, clamped to `[0.0, 1.0]`.
fn alpha_from_elapsed(elapsed: Duration) -> f32 {
    let tick_period = Duration::from_micros(TICK_PERIOD_MICROS);
    let alpha = if tick_period.is_zero() {
        1.0
    } else {
        elapsed.as_secs_f32() / tick_period.as_secs_f32()
    };
    alpha.clamp(0.0, 1.0)
}

/// Linearly interpolates between `a` and `b` by `t`.
///
/// # Arguments
///
/// * `a` - Start value (at `t = 0`).
/// * `b` - End value (at `t = 1`).
/// * `t` - Interpolation factor in `[0.0, 1.0]`.
///
/// # Returns
///
/// * `a + (b - a) * t`.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Builds a stable per-tile character key used to decide whether offset
/// interpolation can be blended across snapshots.
///
/// # Arguments
///
/// * `ch_nr` - Character number from the tile.
/// * `ch_id` - Character unique id from the tile.
///
/// # Returns
///
/// * `0` when no character is present, otherwise a packed non-zero key.
fn character_key(ch_nr: u16, ch_id: u16) -> u32 {
    if ch_nr == 0 {
        return 0;
    }
    ((u32::from(ch_id)) << 16) | (u32::from(ch_nr))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a tiny map with one non-zero tile at the given index.
    fn map_with_offset(idx: usize, offset: TileRenderOffset) -> GameMap {
        let mut map = GameMap::new();
        if let Some(tile) = map.tile_at_index_mut(idx) {
            tile.obj_xoff = offset.obj_xoff as i32;
            tile.obj_yoff = offset.obj_yoff as i32;
            tile.ovl_xoff = offset.ovl_xoff as i32;
            tile.ovl_yoff = offset.ovl_yoff as i32;
        }
        map
    }

    #[test]
    fn new_interpolator_returns_zero_offsets() {
        let interp = RenderInterpolator::new();
        let now = Instant::now();
        let offset = interp.interpolated_offset(0, now);
        assert_eq!(offset, TileRenderOffset::default());
        assert_eq!(interp.alpha_at(now), 0.0);
    }

    #[test]
    fn first_tick_produces_alpha_zero() {
        let mut interp = RenderInterpolator::new();
        let map = map_with_offset(
            0,
            TileRenderOffset {
                obj_xoff: 10.0,
                obj_yoff: 20.0,
                ovl_xoff: 0.0,
                ovl_yoff: 0.0,
            },
        );
        let t0 = Instant::now();
        interp.advance(&map, t0);

        assert_eq!(interp.alpha_at(t0), 0.0);
        let offset = interp.interpolated_offset(0, t0);
        assert_eq!(offset.obj_xoff, 10.0);
        assert_eq!(offset.obj_yoff, 20.0);
    }

    #[test]
    fn two_ticks_blend_linearly() {
        let mut interp = RenderInterpolator::new();
        let map1 = map_with_offset(
            0,
            TileRenderOffset {
                obj_xoff: 0.0,
                obj_yoff: 0.0,
                ovl_xoff: 0.0,
                ovl_yoff: 0.0,
            },
        );
        let t0 = Instant::now();
        interp.advance(&map1, t0);

        let map2 = map_with_offset(
            0,
            TileRenderOffset {
                obj_xoff: 10.0,
                obj_yoff: -10.0,
                ovl_xoff: 4.0,
                ovl_yoff: 8.0,
            },
        );
        let t1 = t0 + Duration::from_micros(TICK_PERIOD_MICROS);
        interp.advance(&map2, t1);

        // At the moment the second tick arrives we should still show the
        // previous snapshot (1-tick delay).
        let at_t1 = interp.interpolated_offset(0, t1);
        assert_eq!(at_t1.obj_xoff, 0.0);
        assert_eq!(at_t1.obj_yoff, 0.0);
        assert_eq!(interp.alpha_at(t1), 0.0);

        // Half a tick period later we should be halfway between snapshots.
        let t_mid = t1 + Duration::from_micros(TICK_PERIOD_MICROS / 2);
        let at_mid = interp.interpolated_offset(0, t_mid);
        assert!((at_mid.obj_xoff - 5.0).abs() < 0.01);
        assert!((at_mid.obj_yoff - (-5.0)).abs() < 0.01);
        assert!((at_mid.ovl_xoff - 2.0).abs() < 0.01);
        assert!((at_mid.ovl_yoff - 4.0).abs() < 0.01);
        assert!((interp.alpha_at(t_mid) - 0.5).abs() < 0.01);

        // One full tick period later we should reach the current snapshot.
        let t_end = t1 + Duration::from_micros(TICK_PERIOD_MICROS);
        let at_end = interp.interpolated_offset(0, t_end);
        assert!((at_end.obj_xoff - 10.0).abs() < 0.01);
        assert!((at_end.obj_yoff - (-10.0)).abs() < 0.01);
        assert!((at_end.ovl_xoff - 4.0).abs() < 0.01);
        assert!((at_end.ovl_yoff - 8.0).abs() < 0.01);
        assert!((interp.alpha_at(t_end) - 1.0).abs() < 0.01);
    }

    #[test]
    fn clamping_when_elapsed_exceeds_tick_period() {
        let mut interp = RenderInterpolator::new();
        let map1 = map_with_offset(
            0,
            TileRenderOffset {
                obj_xoff: 0.0,
                obj_yoff: 0.0,
                ovl_xoff: 0.0,
                ovl_yoff: 0.0,
            },
        );
        let t0 = Instant::now();
        interp.advance(&map1, t0);

        let map2 = map_with_offset(
            0,
            TileRenderOffset {
                obj_xoff: 10.0,
                obj_yoff: 0.0,
                ovl_xoff: 0.0,
                ovl_yoff: 0.0,
            },
        );
        let t1 = t0 + Duration::from_micros(TICK_PERIOD_MICROS);
        interp.advance(&map2, t1);

        let t_late = t1 + Duration::from_micros(TICK_PERIOD_MICROS * 3);
        let offset = interp.interpolated_offset(0, t_late);
        assert!((offset.obj_xoff - 10.0).abs() < 0.01);
        assert_eq!(interp.alpha_at(t_late), 1.0);
    }

    #[test]
    fn multi_tick_advance_updates_previous_and_current() {
        let mut interp = RenderInterpolator::new();
        let t0 = Instant::now();

        for i in 1..=3 {
            let map = map_with_offset(
                0,
                TileRenderOffset {
                    obj_xoff: i as f32 * 10.0,
                    obj_yoff: 0.0,
                    ovl_xoff: 0.0,
                    ovl_yoff: 0.0,
                },
            );
            let t = t0 + Duration::from_micros(TICK_PERIOD_MICROS * i as u64);
            interp.advance(&map, t);
        }

        // After three advances, prev should hold snapshot 2 and current
        // snapshot 3. At the moment of the third tick we show prev (snapshot 2).
        let t3 = t0 + Duration::from_micros(TICK_PERIOD_MICROS * 3);
        let offset = interp.interpolated_offset(0, t3);
        assert!((offset.obj_xoff - 20.0).abs() < 0.01);
    }

    #[test]
    fn advance_does_not_reallocate() {
        let mut interp = RenderInterpolator::new();
        let map = GameMap::new();
        let initial_prev_cap = interp.prev.capacity();
        let initial_current_cap = interp.current.capacity();

        interp.advance(&map, Instant::now());
        interp.advance(&map, Instant::now());
        interp.advance(&map, Instant::now());

        assert_eq!(interp.prev.capacity(), initial_prev_cap);
        assert_eq!(interp.current.capacity(), initial_current_cap);
    }

    #[test]
    fn invalidated_advance_skips_cross_tile_blending() {
        let mut interp = RenderInterpolator::new();
        let t0 = Instant::now();

        let map1 = map_with_offset(
            0,
            TileRenderOffset {
                obj_xoff: 0.0,
                obj_yoff: 0.0,
                ovl_xoff: 0.0,
                ovl_yoff: 0.0,
            },
        );
        interp.advance(&map1, t0);

        let map2 = map_with_offset(
            0,
            TileRenderOffset {
                obj_xoff: 12.0,
                obj_yoff: 0.0,
                ovl_xoff: 0.0,
                ovl_yoff: 0.0,
            },
        );
        let t1 = t0 + Duration::from_micros(TICK_PERIOD_MICROS);
        interp.advance_with_invalidation(&map2, t1, true);

        // Even half a period later we should stay on the new snapshot because
        // previous data was invalidated for this transition.
        let t_mid = t1 + Duration::from_micros(TICK_PERIOD_MICROS / 2);
        let offset = interp.interpolated_offset(0, t_mid);
        assert!((offset.obj_xoff - 12.0).abs() < 0.01);
        assert!((interp.alpha_at(t_mid) - 0.5).abs() < 0.01);
    }

    #[test]
    fn character_offset_does_not_blend_when_occupant_changes() {
        let mut interp = RenderInterpolator::new();
        let mut map1 = GameMap::new();
        if let Some(tile) = map1.tile_at_index_mut(0) {
            tile.ch_nr = 100;
            tile.ch_id = 1;
            tile.obj_xoff = 0;
        }
        let t0 = Instant::now();
        interp.advance(&map1, t0);

        let mut map2 = GameMap::new();
        if let Some(tile) = map2.tile_at_index_mut(0) {
            tile.ch_nr = 200;
            tile.ch_id = 2;
            tile.obj_xoff = 20;
        }
        let t1 = t0 + Duration::from_micros(TICK_PERIOD_MICROS);
        interp.advance(&map2, t1);

        let t_mid = t1 + Duration::from_micros(TICK_PERIOD_MICROS / 2);
        let offset = interp.interpolated_character_offset(0, t_mid);
        assert!((offset.obj_xoff - 20.0).abs() < 0.01);
    }
}
