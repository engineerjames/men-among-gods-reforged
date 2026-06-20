//! Spell-effect icons flanking the vitality chevrons.
//!
//! Positive buffs render in a row to the **left** of the vitality chevrons.
//! Negative effects render in a row to the **right**. Each icon uses the same
//! 29 px size as the skill-bind cells, with the first active effect nearest
//! the chevrons and later effects extending outward.
//!
//! Hovering an icon shows the same tooltip text previously used by the spell
//! effect bars: effect name plus estimated remaining time once enough decay
//! has been observed, e.g. `"Bless (~1m 30s)"`.

use std::collections::HashMap;
use std::time::Instant;

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use crate::ui::RenderContext;
use crate::ui::visuals::spell_icons::{
    SpellIconMeta, active_spell_effect_icon_meta, spell_icon_path,
};
use crate::ui::widget::{EventResponse, UiEvent};
use mag_core::skills;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Half-width of the vitality chevron group; icons start at this distance
/// from the chevron centre.
const HP_HALF_W: i32 = 40;

/// Side length of each square spell-effect icon in pixels.
const ICON_SIZE: i32 = 20;

/// Gap between consecutive spell-effect icons.
const ICON_GAP: i32 = 3;

/// Stride per icon slot: size + gap.
const ICON_STRIDE: i32 = ICON_SIZE + ICON_GAP;

/// Vertical nudge that places the icon rows one pixel higher than the vitality
/// chevron feet alignment.
const ICON_Y_OFFSET: i32 = 1;

/// Maximum number of icons rendered on each side.
const MAX_ICONS: usize = 12;

/// Small tolerance for comparing server-provided fill fractions.
const FILL_EPSILON: f32 = 0.0001;

/// Background fill drawn behind each icon.
const ICON_BG: Color = Color::RGBA(15, 15, 35, 220);

/// Border color for each icon slot.
const ICON_BORDER: Color = Color::RGBA(95, 95, 120, 230);

/// Translucent overlay showing the expired portion of an effect.
const EXPIRED_OVERLAY: Color = Color::RGBA(0, 0, 0, 155);

/// Hover highlight overlay color.
const HOVER_COLOR: Color = Color::RGBA(255, 255, 255, 42);

/// Brighter border color for the hovered icon.
const HOVER_BORDER: Color = Color::RGBA(255, 230, 150, 230);

// ---------------------------------------------------------------------------
// SpellEffectKind
// ---------------------------------------------------------------------------

/// Whether a spell effect is a buff or a debuff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellEffectKind {
    /// Beneficial effect rendered on the left side.
    Positive,
    /// Detrimental effect rendered on the right side.
    Negative,
}

// ---------------------------------------------------------------------------
// SpellEffectMeta
// ---------------------------------------------------------------------------

/// Display metadata for a single active spell effect type.
#[derive(Clone, Copy, Debug)]
struct SpellEffectMeta {
    /// Whether the effect is a buff or debuff.
    kind: SpellEffectKind,
    /// Shared icon display metadata.
    icon: SpellIconMeta,
}

/// Returns display metadata for the given `skill_nr` (`SK_*` constant), or
/// `None` if the skill has no active spell-effect icon.
///
/// Passive effects such as Regeneration, Rest, Meditation, Ghost, Immunity,
/// Concentration, and Warcry are intentionally not listed here because they do
/// not create active spell-effect indicators.
///
/// # Arguments
///
/// * `skill_nr` - Skill template number transmitted in the `SetCharSpell` packet.
///
/// # Returns
///
/// * `Some(SpellEffectMeta)` for known displayable effects, `None` otherwise.
fn spell_meta(skill_nr: i16, sprite: i16) -> Option<SpellEffectMeta> {
    if skill_nr < 0 {
        return None;
    }
    let nr = skill_nr as usize;
    let icon = active_spell_effect_icon_meta(nr, sprite)?;
    let kind = match nr {
        skills::SK_LIGHT
        | skills::SK_PROTECT
        | skills::SK_ENHANCE
        | skills::SK_BLESS
        | skills::SK_MSHIELD
        | skills::SK_RECALL
        | skills::SK_BLAST
        | skills::SK_REVENANT_CONDUIT2
        | skills::SK_SPECTRAL_PACT2 => SpellEffectKind::Positive,
        skills::SK_CURSE
        | skills::SK_STUN
        | skills::SK_WIMPY
        | skills::SK_ANGUISH_LAVA
        | skills::SK_ANGUISH_EARTH
        | skills::SK_ANGUISH_ICE => SpellEffectKind::Negative,
        // Eye potions and Potion of Golem are positive buffs.
        254 | 449 => SpellEffectKind::Positive,
        _ => return None,
    };
    Some(SpellEffectMeta { kind, icon })
}

/// Formats a duration in seconds as a human-readable string for hover text.
///
/// # Arguments
///
/// * `secs` - Estimated remaining seconds.
///
/// # Returns
///
/// * `"< 10s"` for values below 10 s, `"Xs"` up to 59 s, `"Xm"` or
///   `"Xm Ys"` for longer durations.
fn format_duration(secs: f32) -> String {
    let s = secs.round() as u32;
    if s < 10 {
        "< 10s".to_owned()
    } else if s < 60 {
        format!("{}s", s)
    } else {
        let m = s / 60;
        let r = s % 60;
        if r == 0 {
            format!("{}m", m)
        } else {
            format!("{}m {}s", m, r)
        }
    }
}

// ---------------------------------------------------------------------------
// SpellSlotEntry
// ---------------------------------------------------------------------------

/// One active spell slot to render as an icon.
#[derive(Clone, Debug)]
struct SpellSlotEntry {
    /// Spell slot index in the server-provided 20-element arrays.
    slot_index: usize,
    /// Sprite tile number from the `SetCharSpell` packet; used as the
    /// texture cache key and to detect slot reuse.
    sprite: i16,
    /// Remaining fill fraction in `[0.0, 1.0]` (`active / 16.0`).
    fill: f32,
    /// Pre-computed display metadata for this effect.
    meta: SpellEffectMeta,
}

// ---------------------------------------------------------------------------
// HoveredIcon
// ---------------------------------------------------------------------------

/// Identifies the icon currently under the cursor.
#[derive(Clone, Debug)]
struct HoveredIcon {
    /// Which side the icon is on.
    kind: SpellEffectKind,
    /// Index into the corresponding `positives` or `negatives` vector.
    index: usize,
}

// ---------------------------------------------------------------------------
// DurationTracker
// ---------------------------------------------------------------------------

/// Tracks observed decay rate for one active spell slot.
#[derive(Clone, Debug)]
struct DurationTracker {
    /// Sprite tile number of the effect occupying the slot; used to detect
    /// when a slot is reused by a different effect.
    sprite: i16,
    /// Fill fraction at the most recent observed fill change.
    last_fill: f32,
    /// Instant when `last_fill` was observed.
    last_change_at: Instant,
    /// Estimated fill-fraction decay rate per second.
    rate_per_sec: Option<f32>,
}

impl DurationTracker {
    /// Creates a tracker for a newly observed spell slot.
    ///
    /// # Arguments
    ///
    /// * `sprite` - Sprite tile number of the effect occupying the slot.
    /// * `fill` - Current fill fraction in `[0.0, 1.0]`.
    /// * `now` - Observation time.
    ///
    /// # Returns
    ///
    /// * A tracker without a decay-rate estimate yet.
    fn new(sprite: i16, fill: f32, now: Instant) -> Self {
        Self {
            sprite,
            last_fill: fill,
            last_change_at: now,
            rate_per_sec: None,
        }
    }

    /// Updates the tracker from a fresh server fill observation.
    ///
    /// The server reports fill as a quantized `0..=16` fraction, so unchanged
    /// observations must not be treated as new samples. Only actual downward
    /// fill changes update the decay rate; upward changes are treated as a
    /// refresh/recast and reset the estimate.
    ///
    /// # Arguments
    ///
    /// * `fill` - Current fill fraction in `[0.0, 1.0]`.
    /// * `now` - Observation time.
    fn update(&mut self, fill: f32, now: Instant) {
        if fill > self.last_fill + FILL_EPSILON {
            self.last_fill = fill;
            self.last_change_at = now;
            self.rate_per_sec = None;
            return;
        }

        if fill < self.last_fill - FILL_EPSILON {
            let elapsed = now.duration_since(self.last_change_at).as_secs_f32();
            if elapsed > 0.1 {
                self.rate_per_sec = Some((self.last_fill - fill) / elapsed);
            }
            self.last_fill = fill;
            self.last_change_at = now;
        }
    }

    /// Estimates remaining seconds from the most recent fill change.
    ///
    /// # Returns
    ///
    /// * `Some(seconds)` once a decay rate has been observed, `None` otherwise.
    fn remaining_secs(&self) -> Option<f32> {
        let rate = self.rate_per_sec?;
        if rate <= 0.0 {
            return None;
        }
        let elapsed_since_change = self.last_change_at.elapsed().as_secs_f32();
        Some((self.last_fill / rate - elapsed_since_change).max(0.0))
    }
}

// ---------------------------------------------------------------------------
// SpellEffectIcons
// ---------------------------------------------------------------------------

/// HUD visual that renders active spell-effect icons beside the vitality
/// chevrons.
///
/// Positive effects are drawn left of the chevrons and negative effects to
/// the right. Icons are bottom-aligned to the vitality chevron feet and extend
/// outward from the chevrons in active-slot order.
///
/// Modify the public [`x`] and [`y`] fields to reposition the widget without
/// recreating it.
///
/// [`x`]: SpellEffectIcons::x
/// [`y`]: SpellEffectIcons::y
pub struct SpellEffectIcons {
    /// Horizontal centre of the vitality chevron group.
    pub x: i32,
    /// Bottom y coordinate of the vitality chevron group.
    pub y: i32,
    /// Active positive effects, ordered nearest-to-chevrons outward.
    positives: Vec<SpellSlotEntry>,
    /// Active negative effects, ordered nearest-to-chevrons outward.
    negatives: Vec<SpellSlotEntry>,
    /// Icon currently under the cursor, if any.
    hovered: Option<HoveredIcon>,
    /// Observed decay trackers keyed by spell slot index.
    duration_trackers: HashMap<usize, DurationTracker>,
    /// Lazily-loaded texture IDs for spell icons, keyed by sprite tile number.
    /// `None` means loading was attempted and failed, so rendering should use the fallback tile.
    icon_texture_ids: HashMap<i16, Option<usize>>,
}

impl SpellEffectIcons {
    /// Creates a new `SpellEffectIcons` visual positioned to flank vitality
    /// chevrons centred at `x` with feet at `y`.
    ///
    /// # Arguments
    ///
    /// * `x` - Horizontal centre of the vitality chevron group.
    /// * `y` - Bottom y coordinate of the vitality chevron group.
    ///
    /// # Returns
    ///
    /// * A new `SpellEffectIcons` with no active effects.
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            positives: Vec::new(),
            negatives: Vec::new(),
            hovered: None,
            duration_trackers: HashMap::new(),
            icon_texture_ids: HashMap::new(),
        }
    }

    /// Synchronises active icons from the current character spell state.
    ///
    /// Iterates all 20 spell slots and builds two lists (positive/negative).
    /// Slots where `spell[n] <= 0`, `active[n] <= 0`, or whose `skill_type[n]`
    /// has no known display metadata are skipped. At most [`MAX_ICONS`]
    /// entries per side are kept.
    ///
    /// # Arguments
    ///
    /// * `spell` - The character's 20-element spell item-index array.
    /// * `active` - The character's 20-element active-fraction array (`0..=16`).
    /// * `spell_type` - The corresponding `SK_*` skill-number for each slot.
    pub fn sync(&mut self, spell: &[i32; 20], active: &[i8; 20], spell_type: &[i16; 20]) {
        self.positives.clear();
        self.negatives.clear();

        let now = Instant::now();
        let mut active_slots = Vec::new();
        for i in 0..20usize {
            if spell[i] <= 0 || active[i] <= 0 {
                continue;
            }
            let nr = spell_type[i];
            let sprite = spell[i] as i16;
            let Some(meta) = spell_meta(nr, sprite) else {
                continue;
            };
            let fill = (f32::from(active[i]) / 16.0).clamp(0.0, 1.0);
            self.update_duration_tracker(i, sprite, fill, now);
            active_slots.push(i);
            let entry = SpellSlotEntry {
                slot_index: i,
                sprite,
                fill,
                meta,
            };
            match meta.kind {
                SpellEffectKind::Positive => {
                    if self.positives.len() < MAX_ICONS {
                        self.positives.push(entry);
                    }
                }
                SpellEffectKind::Negative => {
                    if self.negatives.len() < MAX_ICONS {
                        self.negatives.push(entry);
                    }
                }
            }
        }
        self.duration_trackers
            .retain(|slot_index, _| active_slots.contains(slot_index));
    }

    /// Updates the duration tracker for one active spell slot.
    ///
    /// # Arguments
    ///
    /// * `slot_index` - Spell slot index in the server-provided arrays.
    /// * `sprite` - Sprite tile number of the effect occupying the slot.
    /// * `fill` - Current fill fraction in `[0.0, 1.0]`.
    /// * `now` - Observation time.
    fn update_duration_tracker(&mut self, slot_index: usize, sprite: i16, fill: f32, now: Instant) {
        match self.duration_trackers.get_mut(&slot_index) {
            Some(tracker) if tracker.sprite == sprite => tracker.update(fill, now),
            _ => {
                self.duration_trackers
                    .insert(slot_index, DurationTracker::new(sprite, fill, now));
            }
        }
    }

    /// Returns the hit rect for the icon at `index` on the given `kind` side.
    ///
    /// # Arguments
    ///
    /// * `kind` - Which side (`Positive` = left, `Negative` = right).
    /// * `index` - Zero-based icon index; index 0 is nearest the chevrons.
    ///
    /// # Returns
    ///
    /// * The full hit rect for that icon.
    fn icon_rect(&self, kind: SpellEffectKind, index: usize) -> Rect {
        let top = self.y - ICON_SIZE - ICON_Y_OFFSET;
        let offset = index as i32 * ICON_STRIDE;
        match kind {
            SpellEffectKind::Positive => {
                let right = self.x - HP_HALF_W;
                Rect::new(
                    right - ICON_SIZE - offset,
                    top,
                    ICON_SIZE as u32,
                    ICON_SIZE as u32,
                )
            }
            SpellEffectKind::Negative => {
                let left = self.x + HP_HALF_W;
                Rect::new(left + offset, top, ICON_SIZE as u32, ICON_SIZE as u32)
            }
        }
    }

    /// Returns the hovered icon for the given cursor position, or `None`.
    ///
    /// # Arguments
    ///
    /// * `px` - Cursor x in logical pixels.
    /// * `py` - Cursor y in logical pixels.
    ///
    /// # Returns
    ///
    /// * `Some(HoveredIcon)` if the point falls inside a rendered icon.
    fn hovered_at(&self, px: i32, py: i32) -> Option<HoveredIcon> {
        for (i, _) in self.positives.iter().enumerate() {
            let rect = self.icon_rect(SpellEffectKind::Positive, i);
            if rect.contains_point((px, py)) {
                return Some(HoveredIcon {
                    kind: SpellEffectKind::Positive,
                    index: i,
                });
            }
        }
        for (i, _) in self.negatives.iter().enumerate() {
            let rect = self.icon_rect(SpellEffectKind::Negative, i);
            if rect.contains_point((px, py)) {
                return Some(HoveredIcon {
                    kind: SpellEffectKind::Negative,
                    index: i,
                });
            }
        }
        None
    }

    /// Returns `true` when the cursor is over any rendered icon.
    ///
    /// # Arguments
    ///
    /// * `x` - Cursor x in logical pixels.
    /// * `y` - Cursor y in logical pixels.
    ///
    /// # Returns
    ///
    /// * `true` if `(x, y)` hits any active icon.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        self.hovered_at(x, y).is_some()
    }

    /// Updates hover state from a translated UI event.
    ///
    /// # Arguments
    ///
    /// * `event` - The translated UI event.
    ///
    /// # Returns
    ///
    /// * Always [`EventResponse::Ignored`] because the visual does not consume input.
    pub fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if let UiEvent::MouseMove { x, y } = event {
            self.hovered = self.hovered_at(*x, *y);
        }
        EventResponse::Ignored
    }

    /// Returns hover tooltip text for the icon currently under the cursor.
    ///
    /// Shows the effect name followed by an estimated time remaining once
    /// enough elapsed time has been observed to compute a decay rate.
    ///
    /// # Returns
    ///
    /// * `Some(String)` when an icon is hovered, `None` otherwise.
    pub fn hover_text(&self) -> Option<String> {
        let hov = self.hovered.as_ref()?;
        let entry = match hov.kind {
            SpellEffectKind::Positive => self.positives.get(hov.index)?,
            SpellEffectKind::Negative => self.negatives.get(hov.index)?,
        };
        let meta = entry.meta;
        if let Some(remaining_secs) = self
            .duration_trackers
            .get(&entry.slot_index)
            .and_then(DurationTracker::remaining_secs)
        {
            return Some(format!(
                "{} (~{})",
                meta.icon.name,
                format_duration(remaining_secs)
            ));
        }
        Some(meta.icon.name.to_owned())
    }

    /// Renders all active spell-effect icons onto the canvas.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context containing the SDL2 canvas and gfx cache.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    pub fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        let positives = self.positives.clone();
        let negatives = self.negatives.clone();
        self.render_side(ctx, SpellEffectKind::Positive, &positives)?;
        self.render_side(ctx, SpellEffectKind::Negative, &negatives)?;
        Ok(())
    }

    /// Lazily loads and returns the texture ID for the given spell metadata.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context containing the graphics cache.
    /// * `sprite` - Sprite tile number used as the texture cache key.
    /// * `meta` - Display metadata containing the temporary icon filename.
    ///
    /// # Returns
    ///
    /// * `Some(texture_id)` when the icon was loaded successfully, `None` otherwise.
    fn texture_id_for(
        &mut self,
        ctx: &mut RenderContext<'_, '_>,
        sprite: i16,
        meta: SpellIconMeta,
    ) -> Option<usize> {
        if let Some(id) = self.icon_texture_ids.get(&sprite) {
            return *id;
        }

        // TODO: Move spell-effect icon assets into the graphics cache/images archive
        // once the final icon set and sprite IDs are settled.
        let path = spell_icon_path(meta);
        let texture_id = match ctx.gfx.load_texture_from_path(&path) {
            Ok(id) => Some(id),
            Err(err) => {
                log::warn!(
                    "Failed to load spell-effect icon {}: {}",
                    path.display(),
                    err
                );
                None
            }
        };
        self.icon_texture_ids.insert(sprite, texture_id);
        texture_id
    }

    /// Renders icons for one side.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context containing the canvas and graphics cache.
    /// * `kind` - Which side to render.
    /// * `entries` - The active entries for this side.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn render_side(
        &mut self,
        ctx: &mut RenderContext<'_, '_>,
        kind: SpellEffectKind,
        entries: &[SpellSlotEntry],
    ) -> Result<(), String> {
        for (i, entry) in entries.iter().enumerate() {
            let meta = entry.meta;
            let rect = self.icon_rect(kind, i);
            let hovered = self
                .hovered
                .as_ref()
                .is_some_and(|h| h.kind == kind && h.index == i);

            ctx.canvas.set_draw_color(ICON_BG);
            ctx.canvas.fill_rect(rect)?;

            if let Some(texture_id) = self.texture_id_for(ctx, entry.sprite, meta.icon) {
                let texture = ctx.gfx.get_texture(texture_id);
                ctx.canvas.copy(texture, None, Some(rect))?;
            } else {
                ctx.canvas.set_draw_color(meta.icon.color);
                ctx.canvas.fill_rect(Rect::new(
                    rect.x() + 1,
                    rect.y() + 1,
                    (ICON_SIZE - 2) as u32,
                    (ICON_SIZE - 2) as u32,
                ))?;
            }

            let expired_fraction = (1.0 - entry.fill).clamp(0.0, 1.0);
            let overlay_h = (expired_fraction * (ICON_SIZE - 2) as f32).round() as i32;
            if overlay_h > 0 {
                ctx.canvas.set_draw_color(EXPIRED_OVERLAY);
                ctx.canvas.fill_rect(Rect::new(
                    rect.x() + 1,
                    rect.y() + 1,
                    (ICON_SIZE - 2) as u32,
                    overlay_h as u32,
                ))?;
            }

            if hovered {
                ctx.canvas.set_draw_color(HOVER_COLOR);
                ctx.canvas.fill_rect(rect)?;
            }

            ctx.canvas
                .set_draw_color(if hovered { HOVER_BORDER } else { ICON_BORDER });
            ctx.canvas.draw_rect(rect)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spell_state(
        idx: usize,
        spell_val: i32,
        active_val: i8,
        type_val: i16,
    ) -> ([i32; 20], [i8; 20], [i16; 20]) {
        let mut spell = [0i32; 20];
        let mut active = [0i8; 20];
        let mut spell_type = [0i16; 20];
        spell[idx] = spell_val;
        active[idx] = active_val;
        spell_type[idx] = type_val;
        (spell, active, spell_type)
    }

    #[test]
    fn sync_separates_positive_and_negative() {
        let mut icons = SpellEffectIcons::new(400, 500);
        let mut spell = [0i32; 20];
        let mut active = [0i8; 20];
        let mut spell_type = [0i16; 20];
        spell[0] = 1;
        active[0] = 8;
        spell_type[0] = skills::SK_BLESS as i16;
        spell[1] = 1;
        active[1] = 4;
        spell_type[1] = skills::SK_CURSE as i16;

        icons.sync(&spell, &active, &spell_type);

        assert_eq!(icons.positives.len(), 1);
        assert_eq!(icons.negatives.len(), 1);
        assert_eq!(icons.positives[0].meta.icon.name, "Bless");
        assert_eq!(icons.negatives[0].meta.icon.name, "Curse");
    }

    #[test]
    fn fill_fraction() {
        let mut icons = SpellEffectIcons::new(400, 500);
        let (spell, active, spell_type) = make_spell_state(0, 1, 8, skills::SK_PROTECT as i16);
        icons.sync(&spell, &active, &spell_type);
        assert!((icons.positives[0].fill - 0.5).abs() < 0.01);
    }

    #[test]
    fn hover_text_format() {
        let mut icons = SpellEffectIcons::new(400, 500);
        let (spell, active, spell_type) = make_spell_state(0, 1, 12, skills::SK_BLESS as i16);
        icons.sync(&spell, &active, &spell_type);
        icons.hovered = Some(HoveredIcon {
            kind: SpellEffectKind::Positive,
            index: 0,
        });

        let text_no_time = icons.hover_text().unwrap();
        assert_eq!(text_no_time, "Bless");

        icons.duration_trackers.insert(
            0,
            DurationTracker {
                sprite: 1,
                last_fill: 0.75,
                last_change_at: Instant::now(),
                rate_per_sec: Some(1.0 / 120.0),
            },
        );
        let text_timed = icons.hover_text().unwrap();
        assert_eq!(text_timed, "Bless (~1m 30s)");
    }

    #[test]
    fn positive_duration_appears_after_first_decay_sample() {
        let mut icons = SpellEffectIcons::new(400, 500);
        let (mut spell, mut active, spell_type) =
            make_spell_state(0, 1, 16, skills::SK_PROTECT as i16);
        icons.sync(&spell, &active, &spell_type);
        icons.hovered = Some(HoveredIcon {
            kind: SpellEffectKind::Positive,
            index: 0,
        });

        icons.duration_trackers.get_mut(&0).unwrap().last_change_at =
            Instant::now() - std::time::Duration::from_secs(30);
        active[0] = 15;
        spell[0] = 1;
        icons.sync(&spell, &active, &spell_type);

        assert_eq!(icons.hover_text().unwrap(), "Protection (~7m 30s)");
    }

    #[test]
    fn unchanged_fill_does_not_reset_decay_sample() {
        let mut icons = SpellEffectIcons::new(400, 500);
        let (spell, mut active, spell_type) = make_spell_state(0, 1, 16, skills::SK_CURSE as i16);
        icons.sync(&spell, &active, &spell_type);
        icons.duration_trackers.get_mut(&0).unwrap().last_change_at =
            Instant::now() - std::time::Duration::from_secs(30);
        active[0] = 15;
        icons.sync(&spell, &active, &spell_type);

        let before = icons.duration_trackers.get(&0).unwrap().clone();
        icons.sync(&spell, &active, &spell_type);
        let after = icons.duration_trackers.get(&0).unwrap();

        assert_eq!(after.last_fill, before.last_fill);
        assert_eq!(after.rate_per_sec, before.rate_per_sec);
        assert_eq!(after.last_change_at, before.last_change_at);
    }

    #[test]
    fn sync_caps_at_max_icons() {
        let mut icons = SpellEffectIcons::new(400, 500);
        let spell = [1i32; 20];
        let active = [8i8; 20];
        let spell_type = [skills::SK_PROTECT as i16; 20];
        icons.sync(&spell, &active, &spell_type);
        assert_eq!(icons.positives.len(), MAX_ICONS);
        assert_eq!(icons.negatives.len(), 0);
    }

    #[test]
    fn empty_slots_skipped() {
        let mut icons = SpellEffectIcons::new(400, 500);
        let mut spell = [0i32; 20];
        let mut active = [0i8; 20];
        let mut spell_type = [0i16; 20];
        spell[0] = 0;
        active[0] = 8;
        spell_type[0] = skills::SK_BLESS as i16;
        spell[1] = 1;
        active[1] = 0;
        spell_type[1] = skills::SK_BLESS as i16;
        spell[2] = 1;
        active[2] = 16;
        spell_type[2] = skills::SK_BLESS as i16;

        icons.sync(&spell, &active, &spell_type);
        assert_eq!(icons.positives.len(), 1);
    }

    #[test]
    fn icon_rect_positions_flank_chevrons() {
        let icons = SpellEffectIcons::new(400, 500);
        let positive_0 = icons.icon_rect(SpellEffectKind::Positive, 0);
        let positive_1 = icons.icon_rect(SpellEffectKind::Positive, 1);
        let negative_0 = icons.icon_rect(SpellEffectKind::Negative, 0);
        let negative_1 = icons.icon_rect(SpellEffectKind::Negative, 1);

        assert_eq!(positive_0.right(), 400 - HP_HALF_W);
        assert_eq!(positive_1.right(), positive_0.x() - ICON_GAP);
        assert_eq!(negative_0.x(), 400 + HP_HALF_W);
        assert_eq!(negative_1.x(), negative_0.right() + ICON_GAP);
        assert_eq!(positive_0.y(), 500 - ICON_SIZE - ICON_Y_OFFSET);
        assert_eq!(negative_0.y(), 500 - ICON_SIZE - ICON_Y_OFFSET);
    }

    #[test]
    fn hover_hit_tests_full_icon_bounds() {
        let mut icons = SpellEffectIcons::new(400, 500);
        let (spell, active, spell_type) = make_spell_state(0, 1, 16, skills::SK_WIMPY as i16);
        icons.sync(&spell, &active, &spell_type);
        let rect = icons.icon_rect(SpellEffectKind::Negative, 0);

        assert!(icons.contains_point(rect.x(), rect.y()));
        assert!(icons.contains_point(rect.right() - 1, rect.bottom() - 1));
        assert!(!icons.contains_point(rect.right(), rect.bottom()));
    }

    #[test]
    fn displayable_effects_have_icon_filenames() {
        let displayable = [
            skills::SK_LIGHT,
            skills::SK_PROTECT,
            skills::SK_ENHANCE,
            skills::SK_BLESS,
            skills::SK_MSHIELD,
            skills::SK_RECALL,
            skills::SK_BLAST,
            skills::SK_CURSE,
            skills::SK_STUN,
            skills::SK_WIMPY,
        ];

        for skill_nr in displayable {
            let meta = spell_meta(skill_nr as i16, 0).unwrap();
            assert!(meta.icon.icon_filename.ends_with("_icon.png"));
        }
    }

    #[test]
    fn passive_effects_do_not_have_icons() {
        let passive = [
            skills::SK_REGEN,
            skills::SK_REST,
            skills::SK_MEDIT,
            skills::SK_GHOST,
            skills::SK_IMMUN,
            skills::SK_CONCEN,
            skills::SK_WARCRY,
        ];

        for skill_nr in passive {
            assert!(spell_meta(skill_nr as i16, 0).is_none());
        }
    }
}
