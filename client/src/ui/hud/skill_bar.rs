//! Skill bar widget.
//!
//! Renders a row of square skill-bind cells at a fixed position:
//!
//! - **Top row (13 cells):** Assignable skill slots. Each cell shows an
//!   abbreviated skill name when bound. Left-clicking a bound slot casts the
//!   skill; left-clicking an empty slot begins the skill-assignment flow.
//!   Right-clicking a bound slot clears the binding.
//!
//! The widget also renders active spell/item-effect sprites in a configurable
//! horizontal row, positioned and sized via [`SkillBarConfig`].

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use mag_core::skills;

use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget, WidgetAction};
use crate::constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT};
use crate::filepaths;
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Side length of each square cell in pixels.
const CELL: i32 = 20;

/// Number of cells in the top (skill-bind) row.
const TOP_CELLS: usize = 13;

/// Vertical offset of all cells relative to the widget (background image)
/// origin.  Increase to scoot cells downward.
const CELLS_OFFSET_Y: i32 = 46;

/// Hard-coded cell origins relative to the widget background.
pub const TOP_CELL_POSITIONS: [(i32, i32); TOP_CELLS] = [
    (63, CELLS_OFFSET_Y),  // 1
    (92, CELLS_OFFSET_Y),  // 2
    (122, CELLS_OFFSET_Y), // 3
    (152, CELLS_OFFSET_Y), // 4
    (183, CELLS_OFFSET_Y), // 5
    (212, CELLS_OFFSET_Y), // 6
    (240, CELLS_OFFSET_Y), // 7
    (268, CELLS_OFFSET_Y), // 8
    (298, CELLS_OFFSET_Y), // 9
    (327, CELLS_OFFSET_Y), // 10
    (356, CELLS_OFFSET_Y), // 11
    (388, CELLS_OFFSET_Y), // 12
    (418, CELLS_OFFSET_Y), // 13
];

/// Total widget width (determined by the wider top row).
const BAR_W: u32 = 500;

/// Total widget height (two rows plus gap).
const BAR_H: u32 = 80;

/// Background fill for each cell.
const CELL_BG: Color = Color::RGBA(15, 15, 35, 200);

/// Border / grid line color.
const CELL_BORDER: Color = Color::RGBA(80, 80, 100, 200);

/// Text color for skill abbreviations in bound slots.
const SKILL_TEXT_COLOR: Color = Color::RGB(220, 200, 140);

/// Text color for the "+" hint in empty skill slots.
const EMPTY_HINT_COLOR: Color = Color::RGBA(100, 100, 120, 180);

/// Hover highlight overlay color.
const HOVER_COLOR: Color = Color::RGBA(255, 255, 255, 40);

/// Bitmap font index (yellow, sprite 701).
const UI_FONT: usize = 1;

/// Maximum characters of a skill name to show in a cell.
const MAX_ABBREV: usize = 4;

/// Number of active spell snapshot slots (matches `ClientPlayer::spell`).
const ACTIVE_SPELL_SLOTS: usize = 20;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the skill bar's active-spell sprite row.
///
/// The caller specifies the top-left origin and per-icon dimensions; the
/// widget lays out up to 20 sprites in a single horizontal row.
#[derive(Clone, Debug)]
pub struct SkillBarConfig {
    /// X position of the first spell sprite (pixels).
    pub spell_x: i32,
    /// Y position of the spell sprite row (pixels).
    pub spell_y: i32,
    /// Width of each spell sprite cell (pixels).
    pub spell_width: i32,
    /// Height of each spell sprite cell (pixels).
    pub spell_height: i32,
}

// ---------------------------------------------------------------------------
// Data snapshot
// ---------------------------------------------------------------------------

/// Per-frame data pushed into the skill bar by the game scene.
pub struct SkillBarData {
    /// Skill keybinds for keys 1–8 (index 0 = slot 1). `Some(skill_nr)` if bound.
    pub keybinds: [Option<usize>; TOP_CELLS],
    /// Active spell icon IDs from `ClientPlayer::spell[]`.
    pub spell: [i32; ACTIVE_SPELL_SLOTS],
    /// Active-timer values from `ClientPlayer::active[]`.
    pub active: [i8; ACTIVE_SPELL_SLOTS],
}

// ---------------------------------------------------------------------------
// Widget struct
// ---------------------------------------------------------------------------

/// The skill bar HUD widget.
pub struct SkillBar {
    bounds: Bounds,
    data: Option<SkillBarData>,
    mouse_x: i32,
    mouse_y: i32,
    actions: Vec<WidgetAction>,
    /// Lazily-loaded texture ID for the `skillbar.png` background image.
    bg_texture_id: Option<usize>,
    /// Active-spell sprite layout configuration.
    config: SkillBarConfig,
}

impl SkillBar {
    /// Creates a new skill bar at the given position.
    ///
    /// # Arguments
    ///
    /// * `config` - Layout configuration for the active-spell sprite row.
    ///
    /// # Returns
    ///
    /// A new `SkillBar` ready for rendering.
    pub fn new(config: SkillBarConfig) -> Self {
        let x_pos = (TARGET_WIDTH_INT - BAR_W) / 2;
        let y_pos = TARGET_HEIGHT_INT - BAR_H;

        Self {
            bounds: Bounds::new(x_pos as i32, y_pos as i32, BAR_W, BAR_H),
            data: None,
            mouse_x: 0,
            mouse_y: 0,
            actions: Vec::new(),
            bg_texture_id: None,
            config,
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

    /// Returns which top-row cell index (0..12) the point is inside, if any.
    fn hit_top_cell(&self, px: i32, py: i32) -> Option<usize> {
        TOP_CELL_POSITIONS
            .iter()
            .enumerate()
            .find_map(|(index, (x, y))| {
                let left = self.bounds.x + x;
                let top = self.bounds.y + y;
                let within_x = (left..left + CELL).contains(&px);
                let within_y = (top..top + CELL).contains(&py);
                within_x.then_some(index).filter(|_| within_y)
            })
    }

    /// Abbreviate a skill name to fit inside a cell.
    fn abbreviate(name: &str) -> String {
        if name.len() <= MAX_ABBREV {
            return name.to_string();
        }
        // Take the first MAX_ABBREV characters.
        name.chars().take(MAX_ABBREV).collect()
    }

    /// Compute the X position of the `n`-th active spell sprite.
    ///
    /// # Arguments
    ///
    /// * `n` - Spell slot index (0–19).
    ///
    /// # Returns
    ///
    /// Pixel X coordinate for the sprite's left edge.
    fn spell_pos_x(&self, n: usize) -> i32 {
        self.config.spell_x + (n as i32) * (self.config.spell_width - 8)
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
                .join("skillbar4.png");
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
            let (cell_x, cell_y) = TOP_CELL_POSITIONS[i];
            let x = self.bounds.x + cell_x;
            let y = self.bounds.y + cell_y;
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
                let name = skills::get_skill_name(skill_nr);
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

        // ── Active spell sprites (horizontal row) ──────────────────────────
        for n in 0..ACTIVE_SPELL_SLOTS {
            let sprite = data.spell[n];
            if sprite <= 0 {
                continue;
            }
            let x = self.spell_pos_x(n);
            let y = self.config.spell_y;
            let tex = ctx.gfx.get_texture(sprite as usize);

            // Attenuation matches engine.c: effect = 15 - min(15, active)
            let active = (data.active[n] as i32).clamp(0, 15);
            let effect = 15 - active;
            let atten = (255 * 120 / (effect * effect + 120)) as u8;

            tex.set_color_mod(atten, atten, atten);
            ctx.canvas.copy(
                tex,
                None,
                Some(Rect::new(
                    x,
                    y,
                    self.config.spell_width as u32,
                    self.config.spell_height as u32,
                )),
            )?;
            tex.set_color_mod(255, 255, 255);
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

    fn bar_at(x: i32, y: i32) -> SkillBar {
        let config = SkillBarConfig {
            spell_x: 0,
            spell_y: 0,
            spell_width: 24,
            spell_height: 24,
        };
        let mut bar = SkillBar::new(config);
        bar.set_position(x, y);
        bar
    }

    fn test_data() -> SkillBarData {
        SkillBarData {
            keybinds: [None; TOP_CELLS],
            spell: [0; ACTIVE_SPELL_SLOTS],
            active: [0; ACTIVE_SPELL_SLOTS],
        }
    }

    #[test]
    fn hit_top_cell_in_bounds() {
        let bar = bar_at(10, 20);
        let oy = 20 + CELLS_OFFSET_Y;
        // First cell.
        assert_eq!(bar.hit_top_cell(10 + TOP_CELL_POSITIONS[0].0, oy), Some(0));
        assert_eq!(
            bar.hit_top_cell(10 + TOP_CELL_POSITIONS[0].0 + CELL - 1, oy + CELL - 1),
            Some(0)
        );
        // Second cell starts at its hard-coded x position.
        assert_eq!(bar.hit_top_cell(10 + TOP_CELL_POSITIONS[1].0, oy), Some(1));
        // Last cell.
        assert_eq!(
            bar.hit_top_cell(10 + TOP_CELL_POSITIONS[12].0, oy),
            Some(12)
        );
    }

    #[test]
    fn hit_top_cell_out_of_bounds() {
        let bar = bar_at(10, 20);
        let ox = 10 + TOP_CELL_POSITIONS[0].0;
        let oy = 20 + CELLS_OFFSET_Y;
        assert_eq!(bar.hit_top_cell(ox - 1, oy), None); // left of cells
        assert_eq!(
            bar.hit_top_cell(10 + TOP_CELL_POSITIONS[12].0 + CELL, oy),
            None
        ); // right of row
        assert_eq!(bar.hit_top_cell(ox, oy + CELL), None); // below top row
    }

    #[test]
    fn update_data_stores_active_spell_snapshot() {
        let mut bar = SkillBar::new(SkillBarConfig {
            spell_x: 0,
            spell_y: 0,
            spell_width: 24,
            spell_height: 24,
        });
        let mut data = test_data();
        data.spell = {
            let mut s = [0i32; ACTIVE_SPELL_SLOTS];
            for (i, val) in [17, 18, 19, 20, 21, 22].iter().enumerate() {
                s[i] = *val;
            }
            s
        };
        data.active = {
            let mut a = [0i8; ACTIVE_SPELL_SLOTS];
            a[0] = 5;
            a[2] = 10;
            a[4] = 15;
            a
        };

        bar.update_data(data);

        let snapshot = bar.data.as_ref().expect("skill bar data should exist");
        assert_eq!(snapshot.spell[0], 17);
        assert_eq!(snapshot.spell[5], 22);
        assert_eq!(snapshot.active[0], 5);
        assert_eq!(snapshot.active[2], 10);
    }

    #[test]
    fn click_empty_slot_begins_assign() {
        let mut bar = bar_at(0, 0);
        bar.update_data(test_data());
        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: TOP_CELL_POSITIONS[0].0 + 1,
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
        let mut bar = bar_at(0, 0);
        let mut data = test_data();
        data.keybinds[2] = Some(26); // Heal
        bar.update_data(data);

        // Click on slot 2.
        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: TOP_CELL_POSITIONS[2].0 + 1,
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
        let mut bar = bar_at(0, 0);
        let mut data = test_data();
        data.keybinds[0] = Some(3); // Sword
        bar.update_data(data);

        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: TOP_CELL_POSITIONS[0].0 + 1,
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
        let mut bar = bar_at(100, 100);
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
    fn spell_snapshot_does_not_affect_top_row_clicks() {
        let mut bar = bar_at(0, 0);
        let mut data = test_data();
        data.spell[0] = 1;
        data.spell[1] = 2;
        data.active[0] = 5;
        bar.update_data(data);

        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: TOP_CELL_POSITIONS[0].0 + 1,
            y: CELLS_OFFSET_Y + 1,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });

        assert_eq!(resp, EventResponse::Consumed);
    }

    #[test]
    fn no_render_without_data() {
        let bar = bar_at(0, 0);
        assert!(bar.data.is_none());
    }

    #[test]
    fn widget_dimensions() {
        assert_eq!(SkillBar::width(), BAR_W);
        assert_eq!(SkillBar::height(), BAR_H);
    }

    #[test]
    fn spell_pos_x_layout() {
        let config = SkillBarConfig {
            spell_x: 100,
            spell_y: 200,
            spell_width: 24,
            spell_height: 24,
        };
        let bar = SkillBar::new(config);
        assert_eq!(bar.spell_pos_x(0), 100);
        assert_eq!(bar.spell_pos_x(1), 116); // 100 + 1*(24-8)
        assert_eq!(bar.spell_pos_x(5), 100 + 5 * 16); // 100 + 5*(24-8)
    }
}
