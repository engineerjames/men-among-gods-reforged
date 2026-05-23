//! Skill bar widget.
//!
//! Renders a row of square skill-bind cells at a fixed position:
//!
//! - **Top row (10 cells):** Assignable skill slots. Bound spell slots show
//!   their icon with the slot number drawn over it. Legacy or unmapped saved
//!   bindings fall back to abbreviated text. Left-clicking a bound slot casts
//!   the skill; left-clicking an empty slot begins the skill-assignment flow.
//!   Right-clicking a bound slot clears the binding.

use std::collections::HashMap;

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use mag_core::skills;

use crate::constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT};
use crate::filepaths;
use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::visuals::spell_icons::{SpellIconMeta, spell_icon_meta, spell_icon_path};
use crate::ui::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget, WidgetAction};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Side length of each square cell in pixels.
const CELL: i32 = 30;

/// Number of cells in the top (skill-bind) row.
pub const TOP_CELLS: usize = 10;

/// Vertical offset of all cells relative to the widget (background image)
/// origin.  Increase to scoot cells downward.
const CELLS_OFFSET_Y: i32 = 67;

/// Hard-coded cell origins relative to the widget background.
pub const TOP_CELL_POSITIONS: [(i32, i32); TOP_CELLS] = [
    (73 - 1, CELLS_OFFSET_Y),          // 1
    (73 + 29 - 1, CELLS_OFFSET_Y),     // 2
    (73 + 29 * 2 - 1, CELLS_OFFSET_Y), // 3
    (73 + 29 * 3 - 1, CELLS_OFFSET_Y), // 4
    (73 + 29 * 4 - 1, CELLS_OFFSET_Y), // 5
    (73 + 29 * 5 - 1, CELLS_OFFSET_Y), // 6
    (73 + 29 * 6 - 1, CELLS_OFFSET_Y), // 7
    (73 + 29 * 7 - 1, CELLS_OFFSET_Y), // 8
    (73 + 29 * 8 - 1, CELLS_OFFSET_Y), // 9
    (73 + 29 * 9 - 1, CELLS_OFFSET_Y), // 10
];

/// Total widget width (determined by the wider top row).
const BAR_W: u32 = 500;

/// Total widget height .
const BAR_H: u32 = 100;

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

/// Golden stroke color for the controller-selected skill slot.
const CONTROLLER_SELECT_COLOR: Color = Color::RGBA(255, 200, 50, 220);

/// Bitmap font index (yellow, sprite 701).
const UI_FONT: usize = 1;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Data snapshot
// ---------------------------------------------------------------------------

/// Per-frame data pushed into the skill bar by the game scene.
pub struct SkillBarData {
    /// Skill bindings for the primary bar slots 1-10 (index 0 = slot 1). `Some(skill_nr)` if bound.
    pub keybinds: [Option<usize>; TOP_CELLS],
    /// Skill bindings for the secondary bar slots 1-10. Active when `show_secondary` is true.
    pub secondary_keybinds: [Option<usize>; TOP_CELLS],
    /// When `true` the bar displays and operates on the secondary page (Shift / LT held).
    pub show_secondary: bool,
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
    /// Lazily-loaded texture IDs for spell icons. `None` means loading was
    /// attempted and failed, so rendering should use the fallback tile.
    icon_texture_ids: HashMap<usize, Option<usize>>,
    /// Controller-selected skill slot index (0..TOP_CELLS), or `None` if no slot is highlighted.
    controller_selected_slot: Option<usize>,
    /// Mirrors the `show_secondary` flag from the most-recent [`SkillBarData`] push.
    /// Used by event handlers which run independently of `update_data`.
    show_secondary: bool,
}

impl SkillBar {
    /// Creates a new skill bar at the given position.
    ///
    /// # Returns
    ///
    /// A new `SkillBar` ready for rendering.
    pub fn new() -> Self {
        let x_pos = (TARGET_WIDTH_INT - BAR_W) / 2;
        let y_pos = TARGET_HEIGHT_INT - BAR_H;

        Self {
            bounds: Bounds::new(x_pos as i32, y_pos as i32, BAR_W, BAR_H),
            data: None,
            mouse_x: 0,
            mouse_y: 0,
            actions: Vec::new(),
            bg_texture_id: None,
            icon_texture_ids: HashMap::new(),
            controller_selected_slot: None,
            show_secondary: false,
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
        self.show_secondary = data.show_secondary;
        self.data = Some(data);
    }

    /// Set the controller-highlighted skill slot.
    ///
    /// # Arguments
    ///
    /// * `slot` - Slot index (0..TOP_CELLS) to highlight, or `None` to clear.
    pub fn set_controller_selected_slot(&mut self, slot: Option<usize>) {
        self.controller_selected_slot = slot;
    }

    /// Get the currently controller-highlighted skill slot.
    ///
    /// # Returns
    ///
    /// * `Some(index)` if a slot is highlighted, `None` otherwise.
    pub fn controller_selected_slot(&self) -> Option<usize> {
        self.controller_selected_slot
    }

    /// Returns helper text for the currently hovered bound slot.
    ///
    /// # Returns
    ///
    /// * `Some(String)` with the bound spell name when hovering a bound slot,
    ///   `None` otherwise.
    pub fn hover_text(&self) -> Option<String> {
        let slot = self.hit_top_cell(self.mouse_x, self.mouse_y)?;
        let data = self.data.as_ref()?;
        let skill_nr = if self.show_secondary {
            data.secondary_keybinds[slot]
        } else {
            data.keybinds[slot]
        }?;
        if let Some(meta) = spell_icon_meta(skill_nr) {
            return Some(meta.name.to_owned());
        }

        let name = skills::get_skill_name(skill_nr);
        (!name.is_empty()).then(|| name.to_owned())
    }

    // -----------------------------------------------------------------------
    // Hit-testing helpers
    // -----------------------------------------------------------------------

    /// Returns which top-row cell index (0..TOP_CELLS) the point is inside, if any.
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

    /// Lazily loads and returns the texture ID for the given spell icon.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context containing the graphics cache.
    /// * `skill_nr` - Protocol skill number used as the texture cache key.
    /// * `meta` - Spell icon metadata containing the asset filename.
    ///
    /// # Returns
    ///
    /// * `Some(texture_id)` when the icon was loaded successfully, `None` otherwise.
    fn texture_id_for(
        &mut self,
        ctx: &mut RenderContext<'_, '_>,
        skill_nr: usize,
        meta: SpellIconMeta,
    ) -> Option<usize> {
        if let Some(id) = self.icon_texture_ids.get(&skill_nr) {
            return *id;
        }

        // TODO: Move spell skill-bar icon assets into the graphics cache/images archive
        // once the final icon set and sprite IDs are settled.
        let path = spell_icon_path(meta);
        let texture_id = match ctx.gfx.load_texture_from_path(&path) {
            Ok(id) => Some(id),
            Err(err) => {
                log::warn!(
                    "Failed to load skill-bar spell icon {}: {}",
                    path.display(),
                    err
                );
                None
            }
        };
        self.icon_texture_ids.insert(skill_nr, texture_id);
        texture_id
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
                    let bound_skill = self.data.as_ref().and_then(|d| {
                        if self.show_secondary {
                            d.secondary_keybinds[slot]
                        } else {
                            d.keybinds[slot]
                        }
                    });
                    // When the secondary page is active, key_slot values 10-19
                    // represent secondary slots 0-9. Action handlers route them
                    // to `skill_keybinds_secondary` instead of `skill_keybinds`.
                    let key_slot = if self.show_secondary {
                        slot + TOP_CELLS
                    } else {
                        slot
                    } as u8;

                    match button {
                        MouseButton::Left => {
                            if let Some(skill_nr) = bound_skill {
                                // Cast the bound skill.
                                self.actions.push(WidgetAction::CastSkill { skill_nr });
                            } else {
                                // Empty slot — begin skill assignment.
                                self.actions.push(WidgetAction::BeginSkillAssign {
                                    skill_id: key_slot as usize,
                                });
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
                                    key_slot,
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
        let (keybinds, secondary_keybinds) = match self.data.as_ref() {
            Some(d) => (d.keybinds, d.secondary_keybinds),
            None => return Ok(()),
        };
        let active_keybinds = if self.show_secondary {
            secondary_keybinds
        } else {
            keybinds
        };

        ctx.canvas.set_blend_mode(BlendMode::Blend);

        // ── Background image (lazy-loaded) ─────────────────────────────
        // TODO: Update to use the gfx cache once things are settled.
        if self.bg_texture_id.is_none() {
            let path = filepaths::get_asset_directory()
                .join("gfx")
                .join("skillbar_6.png");
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

            let bound_skill = active_keybinds[i];
            let bound_icon = bound_skill.and_then(spell_icon_meta);

            if let (Some(skill_nr), Some(meta)) = (bound_skill, bound_icon) {
                if let Some(icon_id) = self.texture_id_for(ctx, skill_nr, meta) {
                    let tex = ctx.gfx.get_texture(icon_id);
                    ctx.canvas.copy(tex, None, Some(rect))?;
                } else {
                    ctx.canvas.set_draw_color(meta.color);
                    ctx.canvas.fill_rect(Rect::new(
                        rect.x() + 1,
                        rect.y() + 1,
                        (CELL - 2) as u32,
                        (CELL - 2) as u32,
                    ))?;
                }
            }

            // Hover highlight.
            if self.hit_top_cell(self.mouse_x, self.mouse_y) == Some(i) {
                ctx.canvas.set_draw_color(HOVER_COLOR);
                ctx.canvas.fill_rect(rect)?;
            }

            // Controller-selected golden stroke.
            if self.controller_selected_slot == Some(i) {
                ctx.canvas.set_draw_color(CONTROLLER_SELECT_COLOR);
                ctx.canvas.draw_rect(rect)?;
                // Inner rect for a 2px-thick border effect.
                if CELL > 2 {
                    let inner = Rect::new(x + 1, y + 1, (CELL - 2) as u32, (CELL - 2) as u32);
                    ctx.canvas.draw_rect(inner)?;
                }
            }

            // Content: slot number on top row, skill name / "+" hint on bottom row.
            let mut cx = x + CELL / 4;

            // Nudge the last slot a little bit more
            if i == TOP_CELLS - 1 {
                cx += 1;
            }

            // Slot number (1-based) always shown near the top of the cell.
            let slot_label = (i + 1).to_string();
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                UI_FONT,
                &slot_label,
                cx,
                y + 1,
                if bound_skill.is_some() {
                    font_cache::TextStyle::centered()
                        .with_tint(SKILL_TEXT_COLOR)
                        .with_drop_shadow()
                } else {
                    font_cache::TextStyle::centered().with_tint(EMPTY_HINT_COLOR)
                },
            )?;
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
        let mut bar = SkillBar::new();
        bar.set_position(x, y);
        bar
    }

    fn test_data() -> SkillBarData {
        SkillBarData {
            keybinds: [None; TOP_CELLS],
            secondary_keybinds: [None; TOP_CELLS],
            show_secondary: false,
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
        assert_eq!(
            bar.hit_top_cell(10 + TOP_CELL_POSITIONS[1].0 + 1, oy),
            Some(1)
        );
        // Last cell.
        let last = TOP_CELLS - 1;
        assert_eq!(
            bar.hit_top_cell(10 + TOP_CELL_POSITIONS[last].0 + 1, oy),
            Some(last)
        );
    }

    #[test]
    fn hit_top_cell_out_of_bounds() {
        let bar = bar_at(10, 20);
        let ox = 10 + TOP_CELL_POSITIONS[0].0;
        let oy = 20 + CELLS_OFFSET_Y;
        assert_eq!(bar.hit_top_cell(ox - 1, oy), None); // left of cells
        assert_eq!(
            bar.hit_top_cell(10 + TOP_CELL_POSITIONS[TOP_CELLS - 1].0 + CELL, oy),
            None
        ); // right of row
        assert_eq!(bar.hit_top_cell(ox, oy + CELL), None); // below top row
    }

    #[test]
    fn update_data_stores_keybinds() {
        let mut bar = SkillBar::new();
        let mut data = test_data();
        data.keybinds[0] = Some(14); // SK_LIGHT
        bar.update_data(data);
        let snapshot = bar.data.as_ref().expect("skill bar data should exist");
        assert_eq!(snapshot.keybinds[0], Some(14));
    }

    #[test]
    fn hover_text_reports_bound_spell_name() {
        let mut bar = bar_at(0, 0);
        let mut data = test_data();
        data.keybinds[0] = Some(skills::SK_BLESS);
        bar.update_data(data);
        bar.handle_event(&UiEvent::MouseMove {
            x: TOP_CELL_POSITIONS[0].0 + 1,
            y: CELLS_OFFSET_Y + 1,
        });

        assert_eq!(bar.hover_text().as_deref(), Some("Bless"));
    }

    #[test]
    fn hover_text_ignores_empty_slots() {
        let mut bar = bar_at(0, 0);
        bar.update_data(test_data());
        bar.handle_event(&UiEvent::MouseMove {
            x: TOP_CELL_POSITIONS[0].0 + 1,
            y: CELLS_OFFSET_Y + 1,
        });

        assert_eq!(bar.hover_text(), None);
    }

    #[test]
    fn hover_text_uses_skill_name_for_unmapped_binding() {
        let mut bar = bar_at(0, 0);
        let mut data = test_data();
        data.keybinds[0] = Some(skills::SK_WEAPON);
        bar.update_data(data);
        bar.handle_event(&UiEvent::MouseMove {
            x: TOP_CELL_POSITIONS[0].0 + 1,
            y: CELLS_OFFSET_Y + 1,
        });

        assert_eq!(bar.hover_text().as_deref(), Some("Weapon Skill"));
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
    fn no_render_without_data() {
        let bar = bar_at(0, 0);
        assert!(bar.data.is_none());
    }

    #[test]
    fn widget_dimensions() {
        assert_eq!(SkillBar::width(), BAR_W);
        assert_eq!(SkillBar::height(), BAR_H);
    }

    // -----------------------------------------------------------------------
    // Secondary bar tests
    // -----------------------------------------------------------------------

    fn secondary_data(secondary_keybinds: [Option<usize>; TOP_CELLS]) -> SkillBarData {
        SkillBarData {
            keybinds: [None; TOP_CELLS],
            secondary_keybinds,
            show_secondary: true,
        }
    }

    #[test]
    fn secondary_empty_slot_click_emits_offset_skill_id() {
        let mut bar = bar_at(0, 0);
        bar.update_data(secondary_data([None; TOP_CELLS]));
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
            WidgetAction::BeginSkillAssign { skill_id } => {
                assert_eq!(*skill_id, TOP_CELLS); // slot 0 + TOP_CELLS = 10
            }
            other => panic!("Expected BeginSkillAssign, got {:?}", other),
        }
    }

    #[test]
    fn secondary_bound_slot_casts_skill() {
        let mut bar = bar_at(0, 0);
        let mut sec = [None; TOP_CELLS];
        sec[1] = Some(26); // Heal in secondary slot 1
        bar.update_data(secondary_data(sec));
        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: TOP_CELL_POSITIONS[1].0 + 1,
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
    fn secondary_right_click_unbinds_with_offset_key_slot() {
        let mut bar = bar_at(0, 0);
        let mut sec = [None; TOP_CELLS];
        sec[0] = Some(3);
        bar.update_data(secondary_data(sec));
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
                assert_eq!(*key_slot as usize, TOP_CELLS); // secondary slot 0 = key_slot 10
            }
            other => panic!("Expected BindSkillKey, got {:?}", other),
        }
    }

    #[test]
    fn secondary_hover_text_reads_secondary_bindings() {
        let mut bar = bar_at(0, 0);
        let mut sec = [None; TOP_CELLS];
        sec[0] = Some(skills::SK_BLESS);
        bar.update_data(secondary_data(sec));
        bar.handle_event(&UiEvent::MouseMove {
            x: TOP_CELL_POSITIONS[0].0 + 1,
            y: CELLS_OFFSET_Y + 1,
        });
        assert_eq!(bar.hover_text().as_deref(), Some("Bless"));
    }

    #[test]
    fn primary_slot_click_unaffected_when_secondary_inactive() {
        // Confirm the primary path still uses key_slot 0..9 when show_secondary is false.
        let mut bar = bar_at(0, 0);
        let mut data = test_data();
        data.keybinds[3] = Some(14);
        bar.update_data(data);
        bar.handle_event(&UiEvent::MouseClick {
            x: TOP_CELL_POSITIONS[3].0 + 1,
            y: CELLS_OFFSET_Y + 1,
            button: MouseButton::Right,
            modifiers: KeyModifiers::default(),
        });
        let actions = bar.take_actions();
        match &actions[0] {
            WidgetAction::BindSkillKey { key_slot, .. } => {
                assert_eq!(*key_slot, 3); // no offset when primary page
            }
            other => panic!("Expected BindSkillKey, got {:?}", other),
        }
    }
}
