//! Client-side weather / ambient effect overlay.
//!
//! Drives all visual weather kinds defined by [`mag_core::weather::WeatherKind`]:
//! particle clouds (rain, snow, fireflies, fire, embers, leaves, fog, haze),
//! flat tints (blood moon), pulsing flashes (lightning), top-of-screen
//! gradient sweeps (aurora), and camera shake (earthquake).
//!
//! Rendering is pure SDL2 primitives (filled rects + lines) drawn between
//! the world tile pass and the HUD layer. No shaders, no offscreen textures.

use std::time::Instant;

use sdl2::pixels::Color;
use sdl2::rect::{Point, Rect};
use sdl2::render::{BlendMode, Canvas};
use sdl2::video::Window;

use mag_core::weather::{WEATHER_FLAG_ADDITIVE, WeatherKind};

use crate::constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT};

/// Maximum simultaneously-tracked particles, regardless of intensity.
const MAX_PARTICLES: usize = 800;

/// Single particle kept in the pool. `kind` allows mixing (e.g. `Fire` plus
/// embers) but in practice every active weather kind owns the whole pool.
#[derive(Clone, Copy)]
struct Particle {
    /// Particle subtype identifier; mirrors [`WeatherKind`] discriminants.
    kind: u8,
    /// X position in logical pixels.
    x: f32,
    /// Y position in logical pixels.
    y: f32,
    /// Horizontal velocity (px/s).
    vx: f32,
    /// Vertical velocity (px/s).
    vy: f32,
    /// Per-particle phase used for sway and pulse animations.
    phase: f32,
    /// Particle lifetime remaining (s); when ≤ 0 the slot is recycled.
    life: f32,
    /// Initial lifetime (s) — used for alpha decay.
    life_max: f32,
}

impl Particle {
    fn dead() -> Self {
        Self {
            kind: 0,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            phase: 0.0,
            life: 0.0,
            life_max: 1.0,
        }
    }
}

/// Active client-side weather state.
///
/// One of these lives on the `GameScene`. Updated every frame and rendered
/// post-world / pre-HUD.
pub struct WeatherState {
    /// Current weather kind (decoded from the wire byte).
    kind: WeatherKind,
    /// 0..=255 intensity from the most recent packet.
    intensity: u8,
    /// RGBA tint from the packet; `[0;4]` means "use kind default".
    tint: [u8; 4],
    /// Wire flags from the packet (e.g. additive blending).
    flags: u8,
    /// Particle pool; dead slots have `life <= 0`.
    particles: Vec<Particle>,
    /// When the current `kind` was first applied; used for animation phase.
    started_at: Instant,
    /// When the next lightning strike should fire.
    next_lightning_at: f32,
    /// Remaining brightness of the current lightning flash (0..=1).
    lightning_flash: f32,
    /// Aurora animation phase, advanced each frame.
    aurora_phase: f32,
    /// Earthquake shake offset (px) applied to the world camera this frame.
    shake_offset: (i32, i32),
    /// Internal seeded RNG state for deterministic-ish wiggle.
    rng_state: u32,
    /// Last update timestamp; used to derive `dt` internally.
    last_update: Option<Instant>,
    /// Set after a kind change; the next `update` pre-distributes particles
    /// across the screen so steady-state effects (snow, fog, leaves) don't
    /// appear as a single in-rushing wave.
    needs_initial_fill: bool,
}

impl Default for WeatherState {
    fn default() -> Self {
        Self::new()
    }
}

impl WeatherState {
    /// Build an inactive state ([`WeatherKind::None`], no particles).
    pub fn new() -> Self {
        let mut particles = Vec::with_capacity(MAX_PARTICLES);
        particles.resize(MAX_PARTICLES, Particle::dead());
        Self {
            kind: WeatherKind::None,
            intensity: 0,
            tint: [0; 4],
            flags: 0,
            particles,
            started_at: Instant::now(),
            next_lightning_at: 4.0,
            lightning_flash: 0.0,
            aurora_phase: 0.0,
            shake_offset: (0, 0),
            rng_state: 0x9E37_79B9,
            last_update: None,
            needs_initial_fill: false,
        }
    }

    /// Returns the current shake offset; renderers may add this to their
    /// camera transform. `(0, 0)` whenever earthquake is inactive.
    pub fn shake_offset(&self) -> (i32, i32) {
        self.shake_offset
    }

    /// Hard-reset all visual state (used on scene exit).
    pub fn reset(&mut self) {
        self.kind = WeatherKind::None;
        self.intensity = 0;
        self.tint = [0; 4];
        self.flags = 0;
        self.lightning_flash = 0.0;
        self.shake_offset = (0, 0);
        self.last_update = None;
        self.needs_initial_fill = false;
        for p in self.particles.iter_mut() {
            *p = Particle::dead();
        }
    }

    /// Apply a `SV_WEATHER` packet payload.
    ///
    /// Resets the particle pool whenever the kind changes; intensity-only
    /// changes preserve existing particles for visual smoothness.
    ///
    /// # Arguments
    ///
    /// * `kind_byte` - Discriminant of the new weather kind.
    /// * `intensity` - Particle density / shake amplitude (0..=255).
    /// * `_duration_ticks` - Server-side expiration (informational; the
    ///   server stops sending the kind when it expires).
    /// * `tint` - RGBA tint; alpha 0 keeps the kind default.
    /// * `flags` - Wire flag bitmask (e.g. additive blending).
    pub fn apply_packet(
        &mut self,
        kind_byte: u8,
        intensity: u8,
        _duration_ticks: u16,
        tint: [u8; 4],
        flags: u8,
    ) {
        let new_kind = WeatherKind::from(kind_byte);
        if new_kind != self.kind {
            for p in self.particles.iter_mut() {
                *p = Particle::dead();
            }
            self.started_at = Instant::now();
            self.lightning_flash = 0.0;
            self.next_lightning_at = 4.0;
            self.needs_initial_fill = true;
        }
        self.kind = new_kind;
        self.intensity = intensity;
        self.tint = tint;
        self.flags = flags;
    }

    /// Pseudo-random `0.0..1.0` based on the internal state.
    fn rand_unit(&mut self) -> f32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        (self.rng_state >> 8) as f32 / (1u32 << 24) as f32
    }

    fn rand_range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.rand_unit()
    }

    /// Compute the *target* live particle count for the active kind. Bounded
    /// by [`MAX_PARTICLES`] and scales linearly with intensity.
    fn target_particle_count(&self) -> usize {
        let max = match self.kind {
            WeatherKind::Rain => 600,
            WeatherKind::Snow => 400,
            WeatherKind::Fireflies => 60,
            WeatherKind::Fire => 300,
            WeatherKind::Fog => 80,
            WeatherKind::Embers => 200,
            WeatherKind::Leaves => 120,
            WeatherKind::HeatHaze => 100,
            // Tint / event-only kinds: no particles.
            WeatherKind::None
            | WeatherKind::BloodMoon
            | WeatherKind::Lightning
            | WeatherKind::Aurora
            | WeatherKind::Earthquake => 0,
        };
        let scaled = (max as u32 * self.intensity as u32) / 255;
        (scaled as usize).min(MAX_PARTICLES)
    }

    /// Mean particle lifetime (seconds) for the active kind. Used to derive
    /// a steady-state spawn rate of `target / mean_lifetime` per second.
    fn mean_lifetime(&self) -> f32 {
        match self.kind {
            WeatherKind::Rain => 1.0,
            WeatherKind::Snow => 11.0,
            WeatherKind::Fireflies => 5.0,
            WeatherKind::Fire => 1.05,
            WeatherKind::Embers => 3.0,
            WeatherKind::Fog => 9.0,
            WeatherKind::Leaves => 6.0,
            WeatherKind::HeatHaze => 2.25,
            _ => 0.0,
        }
    }

    /// Spawn one particle for the currently active kind in a dead slot.
    ///
    /// # Arguments
    ///
    /// * `slot` - Pool index to overwrite.
    /// * `view_w` - Logical viewport width (px).
    /// * `view_h` - Logical viewport height (px).
    /// * `pre_distribute` - When `true`, randomize the spawn position
    ///   along the particle's normal travel path so an initial fill looks
    ///   like an in-progress steady stream rather than a wavefront. When
    ///   `false`, spawn at the kind's normal entry edge.
    fn spawn_particle(&mut self, slot: usize, view_w: i32, view_h: i32, pre_distribute: bool) {
        let kind = self.kind;
        let w = view_w as f32;
        let h = view_h as f32;
        let x = self.rand_range(0.0, w);
        let phase = self.rand_range(0.0, std::f32::consts::TAU);
        let p = match kind {
            WeatherKind::Rain => Particle {
                kind: kind as u8,
                x,
                y: if pre_distribute {
                    self.rand_range(-10.0, h)
                } else {
                    -10.0
                },
                vx: -120.0,
                vy: 700.0,
                phase,
                life: self.rand_range(0.6, 1.4),
                life_max: 1.4,
            },
            WeatherKind::Snow => Particle {
                kind: kind as u8,
                x,
                y: if pre_distribute {
                    self.rand_range(-10.0, h)
                } else {
                    -10.0
                },
                vx: 0.0,
                vy: self.rand_range(35.0, 70.0),
                phase,
                life: self.rand_range(8.0, 14.0),
                life_max: 14.0,
            },
            WeatherKind::Fireflies => Particle {
                kind: kind as u8,
                x,
                y: self.rand_range(0.0, h),
                vx: self.rand_range(-15.0, 15.0),
                vy: self.rand_range(-10.0, 10.0),
                phase,
                life: self.rand_range(3.0, 7.0),
                life_max: 7.0,
            },
            WeatherKind::Fire => Particle {
                kind: kind as u8,
                x,
                y: if pre_distribute {
                    self.rand_range(0.0, h + 5.0)
                } else {
                    h + 5.0
                },
                vx: self.rand_range(-20.0, 20.0),
                vy: self.rand_range(-200.0, -120.0),
                phase,
                life: self.rand_range(0.6, 1.5),
                life_max: 1.5,
            },
            WeatherKind::Embers => Particle {
                kind: kind as u8,
                x,
                y: if pre_distribute {
                    self.rand_range(0.0, h + 5.0)
                } else {
                    h + 5.0
                },
                vx: self.rand_range(-10.0, 10.0),
                vy: self.rand_range(-80.0, -40.0),
                phase,
                life: self.rand_range(2.0, 4.0),
                life_max: 4.0,
            },
            WeatherKind::Fog => Particle {
                kind: kind as u8,
                x,
                y: self.rand_range(0.0, h),
                vx: self.rand_range(8.0, 25.0),
                vy: 0.0,
                phase,
                life: self.rand_range(6.0, 12.0),
                life_max: 12.0,
            },
            WeatherKind::Leaves => Particle {
                kind: kind as u8,
                x,
                y: if pre_distribute {
                    self.rand_range(-10.0, h)
                } else {
                    -10.0
                },
                vx: self.rand_range(-25.0, 25.0),
                vy: self.rand_range(40.0, 70.0),
                phase,
                life: self.rand_range(4.0, 8.0),
                life_max: 8.0,
            },
            WeatherKind::HeatHaze => Particle {
                kind: kind as u8,
                x,
                y: self.rand_range(0.0, h),
                vx: 0.0,
                vy: self.rand_range(-15.0, -5.0),
                phase,
                life: self.rand_range(1.5, 3.0),
                life_max: 3.0,
            },
            _ => Particle::dead(),
        };
        self.particles[slot] = p;
    }

    /// Advance simulation using an internally-tracked frame delta. Use this
    /// when callers don't track their own `dt`. Caps `dt` at 100 ms to
    /// survive long stalls (e.g. window minimised).
    ///
    /// # Arguments
    ///
    /// * `view_w` - Logical viewport width (px).
    /// * `view_h` - Logical viewport height (px).
    pub fn update_auto(&mut self, view_w: i32, view_h: i32) {
        let now = Instant::now();
        let dt = match self.last_update {
            Some(prev) => (now - prev).as_secs_f32().min(0.1),
            None => 0.016,
        };
        self.last_update = Some(now);
        self.update(dt, view_w, view_h);
    }

    /// Advance particles, lightning timing, and earthquake jitter by `dt`
    /// seconds. Spawns up to a bounded number of new particles per frame.
    ///
    /// # Arguments
    ///
    /// * `dt` - Frame delta in seconds.
    /// * `view_w` - Logical viewport width (px).
    /// * `view_h` - Logical viewport height (px).
    pub fn update(&mut self, dt: f32, view_w: i32, view_h: i32) {
        if self.kind == WeatherKind::None {
            self.shake_offset = (0, 0);
            self.lightning_flash = 0.0;
            return;
        }

        // Advance particle physics.
        let (w, h) = (view_w as f32, view_h as f32);
        for p in self.particles.iter_mut() {
            if p.life <= 0.0 {
                continue;
            }
            p.life -= dt;
            p.phase += dt * 2.0;
            // Per-kind kinematics tweaks.
            match WeatherKind::from(p.kind) {
                WeatherKind::Snow => {
                    p.x += (p.phase * 1.7).sin() * 12.0 * dt + p.vx * dt;
                }
                WeatherKind::Fireflies => {
                    p.vx += (p.phase * 2.3).sin() * 6.0 * dt;
                    p.vy += (p.phase * 1.9).cos() * 6.0 * dt;
                    p.vx = p.vx.clamp(-25.0, 25.0);
                    p.vy = p.vy.clamp(-25.0, 25.0);
                    p.x += p.vx * dt;
                }
                WeatherKind::Leaves => {
                    p.vx += (p.phase * 1.3).sin() * 12.0 * dt;
                    p.x += p.vx * dt;
                }
                WeatherKind::HeatHaze => {
                    p.x += (p.phase * 4.0).sin() * 8.0 * dt;
                }
                _ => {
                    p.x += p.vx * dt;
                }
            }
            p.y += p.vy * dt;
            // Recycle when off-screen.
            if p.y > h + 40.0 || p.y < -40.0 || p.x < -40.0 || p.x > w + 40.0 {
                p.life = 0.0;
            }
        }

        // Spawn replacements up to the target count.
        //
        // Two-phase spawning:
        //   1. On the first frame after a kind change, pre-distribute the
        //      whole pool across the screen so steady-state effects (snow,
        //      fog, leaves) appear as an existing column instead of a wave
        //      of particles rushing in from the spawn edge.
        //   2. After that, spawn at the entry edge at the steady-state rate
        //      (`target / mean_lifetime`), so the on-screen population stays
        //      level instead of cycling through fill/drain waves.
        let target = self.target_particle_count();
        if self.needs_initial_fill {
            self.needs_initial_fill = false;
            let mut filled = 0usize;
            for slot in 0..self.particles.len() {
                if filled >= target {
                    break;
                }
                if self.particles[slot].life <= 0.0 {
                    self.spawn_particle(slot, view_w, view_h, true);
                    filled += 1;
                }
            }
        }
        let alive = self.particles.iter().filter(|p| p.life > 0.0).count();
        let mean_life = self.mean_lifetime();
        // Steady-state spawn rate keeps `alive` close to `target`. Allow a
        // small over-spawn factor (1.2) to cover variance and refill quickly
        // after intensity changes without producing visible waves.
        let spawn_rate_per_sec = if mean_life > 0.0 {
            (target as f32 / mean_life) * 1.2
        } else {
            0.0
        };
        let needed = target.saturating_sub(alive);
        let spawn_budget = ((spawn_rate_per_sec * dt).ceil() as usize).min(needed);
        let mut spawned = 0usize;
        for slot in 0..self.particles.len() {
            if spawned >= spawn_budget {
                break;
            }
            if self.particles[slot].life <= 0.0 {
                self.spawn_particle(slot, view_w, view_h, false);
                spawned += 1;
            }
        }

        // Lightning timing.
        if self.kind == WeatherKind::Lightning {
            self.next_lightning_at -= dt;
            if self.next_lightning_at <= 0.0 {
                self.lightning_flash = 1.0;
                let interval = self.rand_range(3.0, 8.0);
                self.next_lightning_at = interval;
            }
            self.lightning_flash = (self.lightning_flash - dt * 8.0).max(0.0);
        } else {
            self.lightning_flash = (self.lightning_flash - dt * 4.0).max(0.0);
        }

        // Aurora animation phase.
        if self.kind == WeatherKind::Aurora {
            self.aurora_phase += dt * 0.4;
        }

        // Earthquake camera shake amplitude scales with intensity.
        if self.kind == WeatherKind::Earthquake {
            let amp = (self.intensity as f32 / 255.0) * 6.0;
            let dx = (self.rand_range(-1.0, 1.0) * amp) as i32;
            let dy = (self.rand_range(-1.0, 1.0) * amp) as i32;
            self.shake_offset = (dx, dy);
        } else {
            self.shake_offset = (0, 0);
        }
    }

    /// Returns the effective tint for the active kind, blending the wire
    /// tint (when alpha > 0) over the kind's default.
    fn effective_tint(&self) -> Color {
        let default = match self.kind {
            WeatherKind::Rain => Color::RGBA(40, 80, 130, 50),
            WeatherKind::Snow => Color::RGBA(200, 220, 240, 30),
            WeatherKind::Fire => Color::RGBA(180, 50, 20, 70),
            WeatherKind::BloodMoon => Color::RGBA(180, 30, 30, 80),
            WeatherKind::Fog => Color::RGBA(180, 180, 190, 90),
            WeatherKind::Lightning => Color::RGBA(20, 30, 70, 60),
            WeatherKind::HeatHaze => Color::RGBA(220, 170, 60, 28),
            _ => Color::RGBA(0, 0, 0, 0),
        };
        if self.tint[3] > 0 {
            Color::RGBA(self.tint[0], self.tint[1], self.tint[2], self.tint[3])
        } else {
            default
        }
    }

    /// Render the active weather effects. Call between the world pass and
    /// the HUD pass so tints/particles overlay the world but stay under the
    /// chat box, panels, etc.
    ///
    /// # Arguments
    ///
    /// * `canvas` - SDL2 canvas to draw onto.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, `Err(String)` if SDL primitives fail.
    pub fn render_post_world(&self, canvas: &mut Canvas<Window>) -> Result<(), String> {
        if self.kind == WeatherKind::None {
            return Ok(());
        }

        let prev_blend = canvas.blend_mode();
        canvas.set_blend_mode(BlendMode::Blend);

        let view_w = TARGET_WIDTH_INT as i32;
        let view_h = TARGET_HEIGHT_INT as i32;

        // Full-screen tint.
        let tint = self.effective_tint();
        if tint.a > 0 {
            canvas.set_draw_color(tint);
            canvas.fill_rect(Rect::new(0, 0, view_w as u32, view_h as u32))?;
        }

        // Aurora gradient strip (top of screen).
        if self.kind == WeatherKind::Aurora {
            let strip_h = (view_h / 5).max(40);
            for y in 0..strip_h {
                let t = y as f32 / strip_h as f32;
                let phase = self.aurora_phase + t * 2.0;
                let r = (60.0 + 40.0 * phase.sin().abs()) as u8;
                let g = (180.0 + 60.0 * (phase * 1.3).cos().abs()) as u8;
                let b = (180.0 + 60.0 * (phase * 0.7).sin().abs()) as u8;
                let a = ((1.0 - t) * 90.0) as u8;
                canvas.set_draw_color(Color::RGBA(r, g, b, a));
                canvas.draw_line(Point::new(0, y), Point::new(view_w, y))?;
            }
        }

        // Particle pass.
        let additive = (self.flags & WEATHER_FLAG_ADDITIVE) != 0
            || matches!(self.kind, WeatherKind::Fire | WeatherKind::Embers);
        if additive {
            canvas.set_blend_mode(BlendMode::Add);
        }

        for p in self.particles.iter() {
            if p.life <= 0.0 {
                continue;
            }
            self.draw_particle(canvas, p)?;
        }

        if additive {
            canvas.set_blend_mode(BlendMode::Blend);
        }

        // Lightning flash.
        if self.lightning_flash > 0.0 {
            let a = (self.lightning_flash.clamp(0.0, 1.0) * 200.0) as u8;
            canvas.set_draw_color(Color::RGBA(255, 255, 255, a));
            canvas.fill_rect(Rect::new(0, 0, view_w as u32, view_h as u32))?;
        }

        canvas.set_blend_mode(prev_blend);
        Ok(())
    }

    /// Draw a single particle in its kind-specific style.
    fn draw_particle(&self, canvas: &mut Canvas<Window>, p: &Particle) -> Result<(), String> {
        let alpha_t = (p.life / p.life_max).clamp(0.0, 1.0);
        let kind = WeatherKind::from(p.kind);
        match kind {
            WeatherKind::Rain => {
                let a = (alpha_t * 180.0) as u8;
                canvas.set_draw_color(Color::RGBA(160, 200, 240, a));
                canvas.draw_line(
                    Point::new(p.x as i32, p.y as i32),
                    Point::new((p.x - 4.0) as i32, (p.y + 12.0) as i32),
                )?;
            }
            WeatherKind::Snow => {
                let a = (alpha_t * 220.0) as u8;
                canvas.set_draw_color(Color::RGBA(240, 245, 255, a));
                canvas.fill_rect(Rect::new(p.x as i32, p.y as i32, 2, 2))?;
            }
            WeatherKind::Fireflies => {
                // Pulsing yellow-green dot.
                let pulse = 0.5 + 0.5 * (p.phase * 2.0).sin();
                let a = (alpha_t * 255.0 * pulse) as u8;
                canvas.set_draw_color(Color::RGBA(220, 255, 120, a));
                canvas.fill_rect(Rect::new(p.x as i32 - 1, p.y as i32 - 1, 3, 3))?;
                // Soft halo
                canvas.set_draw_color(Color::RGBA(200, 240, 80, a / 4));
                canvas.fill_rect(Rect::new(p.x as i32 - 2, p.y as i32 - 2, 5, 5))?;
            }
            WeatherKind::Fire => {
                let a = (alpha_t * 220.0) as u8;
                let r = 240u8;
                let g = (60.0 + 100.0 * alpha_t) as u8;
                let b = 30u8;
                canvas.set_draw_color(Color::RGBA(r, g, b, a));
                canvas.fill_rect(Rect::new(p.x as i32, p.y as i32, 3, 3))?;
            }
            WeatherKind::Embers => {
                let a = (alpha_t * 180.0) as u8;
                canvas.set_draw_color(Color::RGBA(255, 140, 40, a));
                canvas.fill_rect(Rect::new(p.x as i32, p.y as i32, 2, 2))?;
            }
            WeatherKind::Fog => {
                let a = (alpha_t * 70.0) as u8;
                canvas.set_draw_color(Color::RGBA(220, 220, 230, a));
                canvas.fill_rect(Rect::new(p.x as i32 - 8, p.y as i32 - 3, 16, 6))?;
            }
            WeatherKind::Leaves => {
                let a = (alpha_t * 220.0) as u8;
                let g = 90 + ((p.phase.sin().abs() * 80.0) as u8);
                canvas.set_draw_color(Color::RGBA(160, g, 40, a));
                canvas.fill_rect(Rect::new(p.x as i32, p.y as i32, 4, 3))?;
            }
            WeatherKind::HeatHaze => {
                let a = (alpha_t * 60.0) as u8;
                canvas.set_draw_color(Color::RGBA(255, 220, 120, a));
                canvas.fill_rect(Rect::new(p.x as i32, p.y as i32, 3, 1))?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_none() {
        let w = WeatherState::new();
        assert_eq!(w.kind, WeatherKind::None);
        assert_eq!(w.shake_offset(), (0, 0));
    }

    #[test]
    fn apply_packet_changes_kind_and_clears_particles() {
        let mut w = WeatherState::new();
        w.particles[0].life = 5.0;
        w.particles[0].kind = WeatherKind::Snow as u8;
        w.apply_packet(WeatherKind::Rain as u8, 200, 0, [0; 4], 0);
        assert_eq!(w.kind, WeatherKind::Rain);
        assert_eq!(w.intensity, 200);
        assert!(w.particles.iter().all(|p| p.life <= 0.0));
    }

    #[test]
    fn apply_packet_same_kind_preserves_particles() {
        let mut w = WeatherState::new();
        w.apply_packet(WeatherKind::Rain as u8, 100, 0, [0; 4], 0);
        // Manually mark a particle alive.
        w.particles[0] = Particle {
            kind: WeatherKind::Rain as u8,
            x: 10.0,
            y: 10.0,
            vx: 0.0,
            vy: 100.0,
            phase: 0.0,
            life: 1.0,
            life_max: 1.0,
        };
        w.apply_packet(WeatherKind::Rain as u8, 200, 0, [0; 4], 0);
        assert!(w.particles[0].life > 0.0);
        assert_eq!(w.intensity, 200);
    }

    #[test]
    fn update_none_does_nothing() {
        let mut w = WeatherState::new();
        w.update(0.016, 800, 600);
        assert!(w.particles.iter().all(|p| p.life <= 0.0));
    }

    #[test]
    fn update_rain_spawns_particles() {
        let mut w = WeatherState::new();
        w.apply_packet(WeatherKind::Rain as u8, 255, 0, [0; 4], 0);
        // Simulate 30 frames at 60 FPS.
        for _ in 0..30 {
            w.update(0.016, 800, 600);
        }
        let alive = w.particles.iter().filter(|p| p.life > 0.0).count();
        assert!(alive > 0, "rain should produce live particles");
    }

    #[test]
    fn earthquake_produces_nonzero_shake() {
        let mut w = WeatherState::new();
        w.apply_packet(WeatherKind::Earthquake as u8, 255, 0, [0; 4], 0);
        let mut nonzero = false;
        for _ in 0..20 {
            w.update(0.016, 800, 600);
            if w.shake_offset() != (0, 0) {
                nonzero = true;
                break;
            }
        }
        assert!(nonzero);
    }

    #[test]
    fn reset_clears_all_state() {
        let mut w = WeatherState::new();
        w.apply_packet(WeatherKind::Fire as u8, 200, 0, [0; 4], 0);
        for _ in 0..10 {
            w.update(0.016, 800, 600);
        }
        w.reset();
        assert_eq!(w.kind, WeatherKind::None);
        assert!(w.particles.iter().all(|p| p.life <= 0.0));
        assert_eq!(w.shake_offset(), (0, 0));
    }
}
