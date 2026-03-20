//! Dual-row skill bar widget.
//!
//! Renders two rows of square cells at a fixed position:
//!
//! - **Top row (8 cells):** Assignable skill slots. Each cell shows an
//!   abbreviated skill name when bound. Left-clicking a bound slot casts the
//!   skill; left-clicking an empty slot begins the skill-assignment flow.
//!   Right-clicking a bound slot clears the binding.
//!
//! - **Bottom row (6 cells, centered):** Active spell / item-effect indicators
//!   read from `ClientPlayer::spell[]` and `active[]`. These are display-only
//!   for now.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use mag_core::skills;

use super::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget, WidgetAction};
use super::RenderContext;
use crate::filepaths;
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Side length of each square cell in pixels.
const CELL: i32 = 24;

/// Number of cells in the top (skill-bind) row.
const TOP_CELLS: usize = 8;

/// Number of cells in the bottom (spell/effect) row.
const BOTTOM_CELLS: usize = 6;

/// Horizontal offset of all cells relative to the widget (background image)
/// origin.  Increase to scoot cells rightward.
const CELLS_OFFSET_X: i32 = 100;

/// Vertical offset of all cells relative to the widget (background image)
/// origin.  Increase to scoot cells downward.
const CELLS_OFFSET_Y: i32 = 56;

/// Horizontal pixel spacing between adjacent cells.
const CELL_H_SPACING: i32 = 13;

/// Vertical pixel offset between the top row and the bottom row.
const ROW_SPACING: i32 = 11;

/// Horizontal stride from one cell origin to the next (cell width + spacing).
const CELL_STRIDE: i32 = CELL + CELL_H_SPACING;

/// Horizontal pixel offset applied to the bottom row so that it is centered
/// beneath the top row.
const BOTTOM_ROW_OFFSET_X: i32 = ((TOP_CELLS - BOTTOM_CELLS) as i32) * CELL_STRIDE / 2;

/// Total widget width (determined by the wider top row).
const BAR_WIDTH_OFFSET: u32 = 200;
const BAR_W: u32 = (TOP_CELLS as i32 * CELL_STRIDE - CELL_H_SPACING) as u32 + BAR_WIDTH_OFFSET;

/// Total widget height (two rows plus gap).
const BAR_HEIGHT_OFFSET: u32 = 100;
const BAR_H: u32 = (2 * CELL + ROW_SPACING) as u32 + BAR_HEIGHT_OFFSET;

/// Background fill for each cell.
const CELL_BG: Color = Color::RGBA(15, 15, 35, 200);

/// Border / grid line color.
const CELL_BORDER: Color = Color::RGBA(80, 80, 100, 200);

/// Text color for skill abbreviations in bound slots.
const SKILL_TEXT_COLOR: Color = Color::RGB(220, 200, 140);

/// Text color for the "+" hint in empty skill slots.
const EMPTY_HINT_COLOR: Color = Color::RGBA(100, 100, 120, 180);

/// Color for active spell/effect sprite overlay (additive highlight).
const ACTIVE_HIGHLIGHT: Color = Color::RGBA(60, 180, 60, 120);

/// Hover highlight overlay color.
const HOVER_COLOR: Color = Color::RGBA(255, 255, 255, 40);

/// Bitmap font index (yellow, sprite 701).
const UI_FONT: usize = 1;

/// Maximum characters of a skill name to show in a cell.
const MAX_ABBREV: usize = 4;

// ---------------------------------------------------------------------------
// Data snapshot
// ---------------------------------------------------------------------------

/// Per-frame data pushed into the skill bar by the game scene.
pub struct SkillBarData {
    /// CTRL+1-8 skill keybinds (index 0 = slot 1). `Some(skill_nr)` if bound.
    pub keybinds: [Option<u32>; TOP_CELLS],
    /// Spell sprite IDs (first 6 of `ClientPlayer::spell[]`).
    pub spells: [i32; BOTTOM_CELLS],
    /// Whether each spell slot is active (first 6 of `ClientPlayer::active[]`).
    pub spell_active: [bool; BOTTOM_CELLS],
}

// ---------------------------------------------------------------------------
// Widget struct
// ---------------------------------------------------------------------------

/// The dual-row skill bar HUD widget.
pub struct SkillBar {
    bounds: Bounds,
    data: Option<SkillBarData>,
    mouse_x: i32,
    mouse_y: i32,
    actions: Vec<WidgetAction>,
    /// Lazily-loaded texture ID for the `skillbar.png` background image.
    bg_texture_id: Option<usize>,
}

impl SkillBar {
    /// Creates a new skill bar at the given position.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge of the widget.
    /// * `y` - Top edge of the widget.
    ///
    /// # Returns
    ///
    /// A new `SkillBar` ready for rendering.
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            bounds: Bounds::new(x, y, BAR_W, BAR_H),
            data: None,
            mouse_x: 0,
            mouse_y: 0,
            actions: Vec::new(),
            bg_texture_id: None,
        }
    }

    /// Total widget width in pixels.
    pub const fn width() -> u32 {
        BAR_W
    }

    /// Total widget height in pixels.
    pub const fn height() -> u32 {
        BAR_H
    }

    /// Push a fresh data snapshot for this frame.
    ///
    /// # Arguments
    ///
    /// * `data` - Current skill-bind and spell/effect state.
    pub fn update_data(&mut self, data: SkillBarData) {
        self.data = Some(data);
    }

    // -----------------------------------------------------------------------
    // Hit-testing helpers
    // -----------------------------------------------------------------------

    /// Returns which top-row cell index (0..7) the point is inside, if any.
    fn hit_top_cell(&self, px: i32, py: i32) -> Option<usize> {
        let lx = px - self.bounds.x - CELLS_OFFSET_X;
        let ly = py - self.bounds.y - CELLS_OFFSET_Y;
        if lx < 0 || !(0..CELL).contains(&ly) {
            return None;
        }
        let col = (lx / CELL_STRIDE) as usize;
        if col < TOP_CELLS && (lx % CELL_STRIDE) < CELL {
            Some(col)
        } else {
            None
        }
    }

    /// Returns which bottom-row cell index (0..5) the point is inside, if any.
    fn hit_bottom_cell(&self, px: i32, py: i32) -> Option<usize> {
        let row_x = self.bounds.x + CELLS_OFFSET_X + BOTTOM_ROW_OFFSET_X;
        let row_y = self.bounds.y + CELLS_OFFSET_Y + CELL + ROW_SPACING;
        let lx = px - row_x;
        let ly = py - row_y;
        if lx < 0 || !(0..CELL).contains(&ly) {
            return None;
        }
        let col = (lx / CELL_STRIDE) as usize;
        if col < BOTTOM_CELLS && (lx % CELL_STRIDE) < CELL {
            Some(col)
        } else {
            None
        }
    }

    /// Abbreviate a skill name to fit inside a cell.
    fn abbreviate(name: &str) -> String {
        if name.len() <= MAX_ABBREV {
            return name.to_string();
        }
        // Take the first MAX_ABBREV characters.
        name.chars().take(MAX_ABBREV).collect()
    }
}

// ---------------------------------------------------------------------------
// Widget trait impl
// ---------------------------------------------------------------------------

impl Widget for SkillBar {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            UiEvent::MouseMove { x, y } => {
                self.mouse_x = *x;
                self.mouse_y = *y;
                EventResponse::Ignored
            }
            UiEvent::MouseClick { x, y, button, .. } => {
                // Sync cursor for hover state.
                self.mouse_x = *x;
                self.mouse_y = *y;

                // --- Top row (skill binds) ---
                if let Some(slot) = self.hit_top_cell(*x, *y) {
                    let bound_skill = self.data.as_ref().and_then(|d| d.keybinds[slot]);

                    match button {
                        MouseButton::Left => {
                            if let Some(skill_nr) = bound_skill {
                                // Cast the bound skill.
                                self.actions.push(WidgetAction::CastSkill { skill_nr });
                            } else {
                                // Empty slot — begin skill assignment.
                                self.actions
                                    .push(WidgetAction::BeginSkillAssign { skill_id: slot });
                            }
                        }
                        MouseButton::Right => {
                            if bound_skill.is_some() {
                                // Unbind: bind slot to a sentinel that
                                // `BindSkillKey` handler will clear.
                                // We reuse BindSkillKey with skill_nr=0 as
                                // the "clear" signal handled downstream.
                                self.actions.push(WidgetAction::BindSkillKey {
                                    skill_nr: 0,
                                    key_slot: slot as u8,
                                });
                            }
                        }
                        _ => {}
                    }
                    return EventResponse::Consumed;
                }

                // --- Bottom row (spell/effects) — consume to block passthrough ---
                if self.hit_bottom_cell(*x, *y).is_some() {
                    return EventResponse::Consumed;
                }

                EventResponse::Ignored
            }
            _ => EventResponse::Ignored,
        }
    }

    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.actions)
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let data = match self.data.as_ref() {
            Some(d) => d,
            None => return Ok(()),
        };

        ctx.canvas.set_blend_mode(BlendMode::Blend);

        // ── Background image (lazy-loaded) ─────────────────────────────
        if self.bg_texture_id.is_none() {
            let path = filepaths::get_asset_directory()
                .join("gfx")
                .join("skillbar.png");
            if let Ok(id) = ctx.gfx.load_texture_from_path(&path) {
                self.bg_texture_id = Some(id);
            }
        }
        if let Some(bg_id) = self.bg_texture_id {
            let tex = ctx.gfx.get_texture(bg_id);
            let dst = Rect::new(self.bounds.x, self.bounds.y, BAR_W, BAR_H);
            ctx.canvas.copy(tex, None, Some(dst))?;
        }

        // ── Top row: skill-bind cells ──────────────────────────────────────

        for i in 0..TOP_CELLS {
            let x = self.bounds.x + CELLS_OFFSET_X + (i as i32) * CELL_STRIDE;
            let y = self.bounds.y + CELLS_OFFSET_Y;
            let rect = sdl2::rect::Rect::new(x, y, CELL as u32, CELL as u32);

            // Cell background.
            ctx.canvas.set_draw_color(CELL_BG);
            ctx.canvas.fill_rect(rect)?;
            ctx.canvas.set_draw_color(CELL_BORDER);
            ctx.canvas.draw_rect(rect)?;

            // Hover highlight.
            if self.hit_top_cell(self.mouse_x, self.mouse_y) == Some(i) {
                ctx.canvas.set_draw_color(HOVER_COLOR);
                ctx.canvas.fill_rect(rect)?;
            }

            // Content: skill name or "+" hint.
            let cx = x + CELL / 2;
            let cy = y + (CELL - 10) / 2; // 10 = glyph height
            if let Some(skill_nr) = data.keybinds[i] {
                let name = skills::get_skill_name(skill_nr as usize);
                let abbr = Self::abbreviate(name);
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    UI_FONT,
                    &abbr,
                    cx,
                    cy,
                    font_cache::TextStyle::centered()
                        .with_tint(SKILL_TEXT_COLOR)
                        .with_drop_shadow(),
                )?;
            } else {
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    UI_FONT,
                    "+",
                    cx,
                    cy,
                    font_cache::TextStyle::centered().with_tint(EMPTY_HINT_COLOR),
                )?;
            }
        }

        // ── Bottom row: spell / effect cells ───────────────────────────────

        let row_x = self.bounds.x + CELLS_OFFSET_X + BOTTOM_ROW_OFFSET_X;
        let row_y = self.bounds.y + CELLS_OFFSET_Y + CELL + ROW_SPACING;

        for i in 0..BOTTOM_CELLS {
            let x = row_x + (i as i32) * CELL_STRIDE;
            let y = row_y;
            let rect = sdl2::rect::Rect::new(x, y, CELL as u32, CELL as u32);

            // Cell background.
            ctx.canvas.set_draw_color(CELL_BG);
            ctx.canvas.fill_rect(rect)?;
            ctx.canvas.set_draw_color(CELL_BORDER);
            ctx.canvas.draw_rect(rect)?;

            // Spell sprite (if any).
            let sprite_id = data.spells[i];
            if sprite_id > 0 {
                let tex = ctx.gfx.get_texture(sprite_id as usize);
                let q = tex.query();
                // Scale sprite to fit CELL × CELL.
                ctx.canvas.copy(
                    tex,
                    None,
                    Some(sdl2::rect::Rect::new(x, y, CELL as u32, CELL as u32)),
                )?;

                // Active-spell green highlight.
                if data.spell_active[i] {
                    ctx.canvas.set_draw_color(ACTIVE_HIGHLIGHT);
                    ctx.canvas.fill_rect(rect)?;
                }

                // Hover highlight.
                let _ = q; // suppress unused warning
            }

            if self.hit_bottom_cell(self.mouse_x, self.mouse_y) == Some(i) {
                ctx.canvas.set_draw_color(HOVER_COLOR);
                ctx.canvas.fill_rect(rect)?;
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
    use crate::ui::widget::{KeyModifiers, MouseButton};

    fn test_data() -> SkillBarData {
        SkillBarData {
            keybinds: [None; TOP_CELLS],
            spells: [0; BOTTOM_CELLS],
            spell_active: [false; BOTTOM_CELLS],
        }
    }

    #[test]
    fn hit_top_cell_in_bounds() {
        let bar = SkillBar::new(10, 20);
        let ox = 10 + CELLS_OFFSET_X;
        let oy = 20 + CELLS_OFFSET_Y;
        // First cell.
        assert_eq!(bar.hit_top_cell(ox, oy), Some(0));
        assert_eq!(bar.hit_top_cell(ox + CELL - 1, oy + CELL - 1), Some(0));
        // Second cell starts at ox + CELL_STRIDE.
        assert_eq!(bar.hit_top_cell(ox + CELL_STRIDE, oy), Some(1));
        // Last cell.
        assert_eq!(bar.hit_top_cell(ox + 7 * CELL_STRIDE, oy), Some(7));
    }

    #[test]
    fn hit_top_cell_out_of_bounds() {
        let bar = SkillBar::new(10, 20);
        let ox = 10 + CELLS_OFFSET_X;
        let oy = 20 + CELLS_OFFSET_Y;
        assert_eq!(bar.hit_top_cell(ox - 1, oy), None); // left of cells
        assert_eq!(bar.hit_top_cell(ox + 8 * CELL_STRIDE, oy), None); // right of row
        assert_eq!(bar.hit_top_cell(ox, oy + CELL), None); // below top row
    }

    #[test]
    fn hit_bottom_cell_in_bounds() {
        let bar = SkillBar::new(10, 20);
        let bx = 10 + CELLS_OFFSET_X + BOTTOM_ROW_OFFSET_X;
        let by = 20 + CELLS_OFFSET_Y + CELL + ROW_SPACING;
        assert_eq!(bar.hit_bottom_cell(bx, by), Some(0));
        assert_eq!(bar.hit_bottom_cell(bx + CELL_STRIDE, by), Some(1));
        assert_eq!(bar.hit_bottom_cell(bx + 5 * CELL_STRIDE, by), Some(5));
    }

    #[test]
    fn hit_bottom_cell_out_of_bounds() {
        let bar = SkillBar::new(10, 20);
        let bx = 10 + CELLS_OFFSET_X + BOTTOM_ROW_OFFSET_X;
        let by = 20 + CELLS_OFFSET_Y + CELL + ROW_SPACING;
        assert_eq!(bar.hit_bottom_cell(bx - 1, by), None); // left
        assert_eq!(bar.hit_bottom_cell(bx + 6 * CELL_STRIDE, by), None); // right
        assert_eq!(bar.hit_bottom_cell(bx, by + CELL), None); // below
        assert_eq!(bar.hit_bottom_cell(bx, by - 1), None); // above
    }

    #[test]
    fn click_empty_slot_begins_assign() {
        let mut bar = SkillBar::new(0, 0);
        bar.update_data(test_data());
        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: CELLS_OFFSET_X + 1,
            y: CELLS_OFFSET_Y + 1,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        let actions = bar.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::BeginSkillAssign { skill_id } => assert_eq!(*skill_id, 0),
            other => panic!("Expected BeginSkillAssign, got {:?}", other),
        }
    }

    #[test]
    fn click_bound_slot_casts_skill() {
        let mut bar = SkillBar::new(0, 0);
        let mut data = test_data();
        data.keybinds[2] = Some(26); // Heal
        bar.update_data(data);

        // Click on slot 2.
        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: CELLS_OFFSET_X + 2 * CELL_STRIDE + 1,
            y: CELLS_OFFSET_Y + 1,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        let actions = bar.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::CastSkill { skill_nr } => assert_eq!(*skill_nr, 26),
            other => panic!("Expected CastSkill, got {:?}", other),
        }
    }

    #[test]
    fn right_click_bound_slot_unbinds() {
        let mut bar = SkillBar::new(0, 0);
        let mut data = test_data();
        data.keybinds[0] = Some(3); // Sword
        bar.update_data(data);

        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: CELLS_OFFSET_X + 1,
            y: CELLS_OFFSET_Y + 1,
            button: MouseButton::Right,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        let actions = bar.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::BindSkillKey { skill_nr, key_slot } => {
                assert_eq!(*skill_nr, 0);
                assert_eq!(*key_slot, 0);
            }
            other => panic!("Expected BindSkillKey, got {:?}", other),
        }
    }

    #[test]
    fn click_outside_ignored() {
        let mut bar = SkillBar::new(100, 100);
        bar.update_data(test_data());
        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
        assert!(bar.take_actions().is_empty());
    }

    #[test]
    fn abbreviate_short_name() {
        assert_eq!(SkillBar::abbreviate("Heal"), "Heal");
        assert_eq!(SkillBar::abbreviate("Axe"), "Axe");
    }

    #[test]
    fn abbreviate_long_name() {
        assert_eq!(SkillBar::abbreviate("Magic Shield"), "Magi");
        assert_eq!(SkillBar::abbreviate("Hand to Hand"), "Hand");
    }

    #[test]
    fn bottom_row_click_consumed() {
        let mut bar = SkillBar::new(0, 0);
        bar.update_data(test_data());
        let bx = CELLS_OFFSET_X + BOTTOM_ROW_OFFSET_X + 1;
        let by = CELLS_OFFSET_Y + CELL + ROW_SPACING + 1;
        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: bx,
            y: by,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        // No action emitted (display-only for now).
        assert!(bar.take_actions().is_empty());
    }

    #[test]
    fn no_render_without_data() {
        let bar = SkillBar::new(0, 0);
        assert!(bar.data.is_none());
    }

    #[test]
    fn widget_dimensions() {
        assert_eq!(SkillBar::width(), BAR_W);
        assert_eq!(SkillBar::height(), BAR_H);
    }
}
