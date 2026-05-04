//! Spell-effect duration bars flanking the vitality chevrons.
//!
//! Positive buffs stack on the **left** of the vitality chevrons (bars grow
//! rightward from the chevron edge, collapse leftward as they expire).
//! Negative effects stack on the **right** (bars grow leftward from the
//! chevron edge, collapse rightward as they expire).
//!
//! Each bar is `BAR_H` px tall with a `BAR_GAP` px gap between bars, giving a
//! stride of `BAR_H + BAR_GAP` px per slot. A full-width bar extends
//! `MAX_BAR_W` px away from the chevron edge. The fill fraction comes from
//! `active[n] / 16.0`, matching the server-side calculation.
//!
//! Hovering a bar shows a tooltip of the form `"SkillName (75%)"`.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use crate::ui::RenderContext;
use crate::ui::widget::{EventResponse, UiEvent};
use mag_core::skills;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Half-width of the vitality chevron group; bars start exactly at this
/// distance from the chevron centre (one side left, one side right).
const HP_HALF_W: i32 = 40;

/// Height of a single duration bar in pixels.
const BAR_H: i32 = 4;

/// Gap between consecutive bars in pixels.
const BAR_GAP: i32 = 1;

/// Stride per bar slot: height + gap.
const BAR_STRIDE: i32 = BAR_H + BAR_GAP;

/// Maximum number of bars rendered on each side.
const MAX_BARS: usize = 12;

/// Maximum bar width in pixels (full fill = this many pixels from the chevron
/// edge).
const MAX_BAR_W: i32 = 80;

/// Background track color (dark, same role as the vitality chevron tracks).
const TRACK_COLOR: Color = Color::RGB(30, 30, 30);

// ---------------------------------------------------------------------------
// SpellEffectKind
// ---------------------------------------------------------------------------

/// Whether a spell effect is a buff or a debuff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellEffectKind {
    /// Beneficial effect — rendered on the left side.
    Positive,
    /// Detrimental effect — rendered on the right side.
    Negative,
}

// ---------------------------------------------------------------------------
// SpellEffectMeta
// ---------------------------------------------------------------------------

/// Display metadata for a single spell effect type.
struct SpellEffectMeta {
    /// Short display name shown in hover tooltips.
    name: &'static str,
    /// Whether the effect is a buff or debuff.
    kind: SpellEffectKind,
    /// Bar fill color.
    color: Color,
}

/// Returns display metadata for the given `skill_nr` (`SK_*` constant), or
/// `None` if the skill has no associated effect bar.
///
/// # Arguments
///
/// * `skill_nr` - Skill template number transmitted in the `SetCharSpell` packet.
///
/// # Returns
///
/// * `Some(SpellEffectMeta)` for known displayable effects, `None` otherwise.
fn spell_meta(skill_nr: i16) -> Option<SpellEffectMeta> {
    let nr = skill_nr as usize;
    match nr {
        // ------------------------------------------------------------------
        // Positive effects (buffs)
        // ------------------------------------------------------------------
        skills::SK_LIGHT => Some(SpellEffectMeta {
            name: "Light",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(240, 230, 100),
        }),
        skills::SK_PROTECT => Some(SpellEffectMeta {
            name: "Protection",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(80, 160, 240),
        }),
        skills::SK_ENHANCE => Some(SpellEffectMeta {
            name: "Enhancement",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(100, 220, 100),
        }),
        skills::SK_BLESS => Some(SpellEffectMeta {
            name: "Bless",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(200, 200, 255),
        }),
        skills::SK_REGEN => Some(SpellEffectMeta {
            name: "Regeneration",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(60, 200, 80),
        }),
        skills::SK_REST => Some(SpellEffectMeta {
            name: "Rest",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(120, 200, 160),
        }),
        skills::SK_MEDIT => Some(SpellEffectMeta {
            name: "Meditation",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(160, 100, 220),
        }),
        skills::SK_MSHIELD => Some(SpellEffectMeta {
            name: "Magic Shield",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(100, 140, 255),
        }),
        skills::SK_GHOST => Some(SpellEffectMeta {
            name: "Ghost",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(200, 200, 200),
        }),
        skills::SK_IMMUN => Some(SpellEffectMeta {
            name: "Immunity",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(80, 220, 180),
        }),
        skills::SK_CONCEN => Some(SpellEffectMeta {
            name: "Concentration",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(180, 220, 80),
        }),
        skills::SK_WARCRY => Some(SpellEffectMeta {
            name: "Warcry",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(220, 120, 60),
        }),
        skills::SK_RECALL => Some(SpellEffectMeta {
            name: "Recall",
            kind: SpellEffectKind::Positive,
            color: Color::RGB(160, 200, 255),
        }),
        // ------------------------------------------------------------------
        // Negative effects (debuffs)
        // ------------------------------------------------------------------
        skills::SK_CURSE => Some(SpellEffectMeta {
            name: "Curse",
            kind: SpellEffectKind::Negative,
            color: Color::RGB(180, 40, 200),
        }),
        skills::SK_STUN => Some(SpellEffectMeta {
            name: "Stun",
            kind: SpellEffectKind::Negative,
            color: Color::RGB(200, 60, 60),
        }),
        skills::SK_WIMPY => Some(SpellEffectMeta {
            name: "Wimpy",
            kind: SpellEffectKind::Negative,
            color: Color::RGB(160, 80, 40),
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// SpellSlotEntry
// ---------------------------------------------------------------------------

/// One active spell slot to render as a bar.
#[derive(Clone, Debug)]
struct SpellSlotEntry {
    /// Skill template number (matches `SK_*` constants).
    skill_nr: i16,
    /// Fill fraction in `[0.0, 1.0]` (`active / 16.0`).
    fill: f32,
}

// ---------------------------------------------------------------------------
// HoveredBar
// ---------------------------------------------------------------------------

/// Identifies the bar currently under the cursor.
#[derive(Clone, Debug)]
struct HoveredBar {
    /// Which side the bar is on.
    kind: SpellEffectKind,
    /// Index into the corresponding `positives` or `negatives` vector.
    index: usize,
}

// ---------------------------------------------------------------------------
// SpellEffectBars
// ---------------------------------------------------------------------------

/// HUD widget that renders spell-effect duration bars flanking the vitality
/// chevrons.
///
/// Positive buffs are drawn to the left of the chevrons; negative debuffs to
/// the right. Bars stack bottom-up, with the first entry nearest the chevron
/// feet.
///
/// Modify the public [`x`] and [`y`] fields to reposition the widget without
/// recreating it (they should match the vitality bars' `x` and `y`).
///
/// [`x`]: SpellEffectBars::x
/// [`y`]: SpellEffectBars::y
pub struct SpellEffectBars {
    /// Horizontal centre of the vitality chevron group (shared with
    /// [`VitalityBars::x`]).
    ///
    /// [`VitalityBars::x`]: crate::ui::visuals::vitality_bars::VitalityBars::x
    pub x: i32,
    /// Bottom y coordinate of the vitality chevron group (shared with
    /// [`VitalityBars::y`]).
    ///
    /// [`VitalityBars::y`]: crate::ui::visuals::vitality_bars::VitalityBars::y
    pub y: i32,
    /// Active positive effects, ordered bottom-up.
    positives: Vec<SpellSlotEntry>,
    /// Active negative effects, ordered bottom-up.
    negatives: Vec<SpellSlotEntry>,
    /// Bar currently under the cursor, if any.
    hovered: Option<HoveredBar>,
}

impl SpellEffectBars {
    /// Creates a new `SpellEffectBars` widget positioned to flank vitality
    /// chevrons centred at `x` with feet at `y`.
    ///
    /// # Arguments
    ///
    /// * `x` - Horizontal centre of the vitality chevron group.
    /// * `y` - Bottom y coordinate of the vitality chevron group (feet y).
    ///
    /// # Returns
    ///
    /// * A new `SpellEffectBars` with no active bars.
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            positives: Vec::new(),
            negatives: Vec::new(),
            hovered: None,
        }
    }

    /// Synchronises active bars from the current character spell state.
    ///
    /// Iterates all 20 spell slots and builds two sorted lists (positive /
    /// negative). Slots where `spell[n] <= 0`, `active[n] <= 0`, or whose
    /// `skill_type[n]` has no known display metadata are skipped. At most
    /// [`MAX_BARS`] entries per side are kept.
    ///
    /// # Arguments
    ///
    /// * `spell` - The character's 20-element spell item-index array.
    /// * `active` - The character's 20-element active-fraction array (range
    ///   `0..=16`).
    /// * `spell_type` - The corresponding `SK_*` skill-number for each slot.
    pub fn sync(&mut self, spell: &[i32; 20], active: &[i8; 20], spell_type: &[i16; 20]) {
        self.positives.clear();
        self.negatives.clear();

        for i in 0..20usize {
            if spell[i] <= 0 || active[i] <= 0 {
                continue;
            }
            let nr = spell_type[i];
            let Some(meta) = spell_meta(nr) else {
                continue;
            };
            let fill = (active[i] as f32 / 16.0).clamp(0.0, 1.0);
            let entry = SpellSlotEntry { skill_nr: nr, fill };
            match meta.kind {
                SpellEffectKind::Positive => {
                    if self.positives.len() < MAX_BARS {
                        self.positives.push(entry);
                    }
                }
                SpellEffectKind::Negative => {
                    if self.negatives.len() < MAX_BARS {
                        self.negatives.push(entry);
                    }
                }
            }
        }
    }

    /// Returns the hit rect for the bar at `row` on the given `kind` side.
    ///
    /// Bars are stacked bottom-up so row 0 is closest to the chevron feet.
    ///
    /// # Arguments
    ///
    /// * `kind` - Which side (`Positive` = left, `Negative` = right).
    /// * `row` - Zero-based row index (0 = bottom-most bar).
    ///
    /// # Returns
    ///
    /// * The full hit rect for that bar row.
    fn bar_rect(&self, kind: SpellEffectKind, row: usize) -> Rect {
        let bar_top = self.y - (row as i32 + 1) * BAR_STRIDE + BAR_GAP;
        match kind {
            SpellEffectKind::Positive => {
                // Right edge at chevron left boundary; extends leftward.
                let right = self.x - HP_HALF_W;
                Rect::new(right - MAX_BAR_W, bar_top, MAX_BAR_W as u32, BAR_H as u32)
            }
            SpellEffectKind::Negative => {
                // Left edge at chevron right boundary; extends rightward.
                let left = self.x + HP_HALF_W;
                Rect::new(left, bar_top, MAX_BAR_W as u32, BAR_H as u32)
            }
        }
    }

    /// Returns the hovered bar for the given cursor position, or `None`.
    ///
    /// # Arguments
    ///
    /// * `px` - Cursor x in logical pixels.
    /// * `py` - Cursor y in logical pixels.
    ///
    /// # Returns
    ///
    /// * `Some(HoveredBar)` if the point falls inside a rendered bar, otherwise
    ///   `None`.
    fn hovered_at(&self, px: i32, py: i32) -> Option<HoveredBar> {
        for (i, _) in self.positives.iter().enumerate() {
            let r = self.bar_rect(SpellEffectKind::Positive, i);
            if r.contains_point((px, py)) {
                return Some(HoveredBar {
                    kind: SpellEffectKind::Positive,
                    index: i,
                });
            }
        }
        for (i, _) in self.negatives.iter().enumerate() {
            let r = self.bar_rect(SpellEffectKind::Negative, i);
            if r.contains_point((px, py)) {
                return Some(HoveredBar {
                    kind: SpellEffectKind::Negative,
                    index: i,
                });
            }
        }
        None
    }

    /// Returns `true` when the cursor is over any rendered bar.
    ///
    /// # Arguments
    ///
    /// * `x` - Cursor x in logical pixels.
    /// * `y` - Cursor y in logical pixels.
    ///
    /// # Returns
    ///
    /// * `true` if `(x, y)` hits any active bar.
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
    /// * Always [`EventResponse::Ignored`] — the widget never consumes events.
    pub fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if let UiEvent::MouseMove { x, y } = event {
            self.hovered = self.hovered_at(*x, *y);
        }
        EventResponse::Ignored
    }

    /// Returns hover tooltip text for the bar currently under the cursor.
    ///
    /// Format: `"SkillName (75%)"`.
    ///
    /// # Returns
    ///
    /// * `Some(String)` when a bar is hovered, `None` otherwise.
    pub fn hover_text(&self) -> Option<String> {
        let hov = self.hovered.as_ref()?;
        let entry = match hov.kind {
            SpellEffectKind::Positive => self.positives.get(hov.index)?,
            SpellEffectKind::Negative => self.negatives.get(hov.index)?,
        };
        let meta = spell_meta(entry.skill_nr)?;
        let pct = (entry.fill * 100.0).round() as u32;
        Some(format!("{} ({}%)", meta.name, pct))
    }

    /// Renders all active spell-effect bars onto the canvas.
    ///
    /// Each bar is drawn as a dark track rect followed by a colored fill rect
    /// of width `fill * MAX_BAR_W`. Uses opaque (`BlendMode::None`) rendering
    /// to match the vitality chevrons.
    ///
    /// Positive bars (left side) extend leftward; negative bars (right side)
    /// extend rightward. Both sides stack bottom-up.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context containing the SDL2 canvas and gfx cache.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    pub fn render(&self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let canvas = &mut ctx.canvas;
        canvas.set_blend_mode(BlendMode::None);

        self.render_side(canvas, SpellEffectKind::Positive, &self.positives)?;
        self.render_side(canvas, SpellEffectKind::Negative, &self.negatives)?;

        Ok(())
    }

    /// Renders bars for one side.
    ///
    /// # Arguments
    ///
    /// * `canvas` - The SDL2 render canvas.
    /// * `kind` - Which side to render.
    /// * `entries` - The list of active entries for this side.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn render_side(
        &self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        kind: SpellEffectKind,
        entries: &[SpellSlotEntry],
    ) -> Result<(), String> {
        for (i, entry) in entries.iter().enumerate() {
            let Some(meta) = spell_meta(entry.skill_nr) else {
                continue;
            };
            let track = self.bar_rect(kind, i);

            // Draw track background.
            canvas.set_draw_color(TRACK_COLOR);
            canvas.fill_rect(track)?;

            // Draw fill rect.
            let fill_w = (entry.fill * MAX_BAR_W as f32).round() as i32;
            if fill_w > 0 {
                let fill_rect = match kind {
                    SpellEffectKind::Positive => {
                        // Fill grows rightward from the left edge of the track.
                        Rect::new(track.x(), track.y(), fill_w as u32, BAR_H as u32)
                    }
                    SpellEffectKind::Negative => {
                        // Fill grows rightward from the left edge of the track.
                        Rect::new(track.x(), track.y(), fill_w as u32, BAR_H as u32)
                    }
                };
                canvas.set_draw_color(meta.color);
                canvas.fill_rect(fill_rect)?;
            }
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
        let mut bars = SpellEffectBars::new(400, 500);
        let mut spell = [0i32; 20];
        let mut active = [0i8; 20];
        let mut spell_type = [0i16; 20];
        // SK_BLESS (positive), SK_CURSE (negative)
        spell[0] = 1;
        active[0] = 8;
        spell_type[0] = skills::SK_BLESS as i16;
        spell[1] = 1;
        active[1] = 4;
        spell_type[1] = skills::SK_CURSE as i16;

        bars.sync(&spell, &active, &spell_type);

        assert_eq!(bars.positives.len(), 1);
        assert_eq!(bars.negatives.len(), 1);
        assert_eq!(bars.positives[0].skill_nr, skills::SK_BLESS as i16);
        assert_eq!(bars.negatives[0].skill_nr, skills::SK_CURSE as i16);
    }

    #[test]
    fn fill_fraction() {
        let mut bars = SpellEffectBars::new(400, 500);
        let (spell, active, spell_type) = make_spell_state(0, 1, 8, skills::SK_PROTECT as i16);
        bars.sync(&spell, &active, &spell_type);
        assert!((bars.positives[0].fill - 0.5).abs() < 0.01);
    }

    #[test]
    fn hover_text_format() {
        let mut bars = SpellEffectBars::new(400, 500);
        let (spell, active, spell_type) = make_spell_state(0, 1, 12, skills::SK_BLESS as i16);
        bars.sync(&spell, &active, &spell_type);
        // Manually inject a hovered state.
        bars.hovered = Some(HoveredBar {
            kind: SpellEffectKind::Positive,
            index: 0,
        });
        let text = bars.hover_text().unwrap();
        assert_eq!(text, "Bless (75%)");
    }

    #[test]
    fn sync_caps_at_max_bars() {
        let mut bars = SpellEffectBars::new(400, 500);
        // All 20 slots set to SK_PROTECT (positive); only MAX_BARS kept.
        let spell = [1i32; 20];
        let active = [8i8; 20];
        let spell_type = [skills::SK_PROTECT as i16; 20];
        bars.sync(&spell, &active, &spell_type);
        assert_eq!(bars.positives.len(), MAX_BARS);
        assert_eq!(bars.negatives.len(), 0);
    }

    #[test]
    fn empty_slots_skipped() {
        let mut bars = SpellEffectBars::new(400, 500);
        // Slot 0: spell == 0 (no item)
        // Slot 1: active == 0 (expired)
        // Slot 2: valid
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

        bars.sync(&spell, &active, &spell_type);
        assert_eq!(bars.positives.len(), 1);
    }
}
