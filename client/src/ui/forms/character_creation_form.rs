//! Composite character-creation form widget.
//!
//! Contains text inputs for name and description, radio groups for class
//! (race) and sex selection, plus Create, Random Name, and Back buttons.
//! The owning scene reads pending [`CharacterCreationFormAction`]s via
//! [`CharacterCreationForm::take_actions`].

use std::time::Duration;

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use mag_core::types::{Class, Sex};

use crate::constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT};
use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::style::{Background, Border};
use crate::ui::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};
use crate::ui::widgets::button::RectButton;
use crate::ui::widgets::radio_group::RadioGroup;
use crate::ui::widgets::text_input::TextInput;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Panel dimensions.
const PANEL_W: u32 = 360;
const PANEL_H: u32 = 420;

/// Horizontal padding inside the panel.
const PAD_X: i32 = 20;

/// Width of text input fields.
const INPUT_W: u32 = PANEL_W - (PAD_X as u32) * 2;

/// Size of the portrait sprite preview next to the race selection.
const SPRITE_PREVIEW_SIZE: u32 = 64;

/// Gap between the race radio group and the sprite preview.
const SPRITE_PREVIEW_GAP: u32 = 8;

/// Width of the race radio group (narrowed to make room for the sprite preview).
const CLASS_GROUP_W: u32 = INPUT_W - SPRITE_PREVIEW_SIZE - SPRITE_PREVIEW_GAP;

/// Height of each text input field.
const INPUT_H: u32 = 16;

/// Vertical gap between a label and the control beneath it.
const LABEL_INPUT_GAP: i32 = 2;

/// Vertical gap between field groups.
const FIELD_GAP: i32 = 10;

/// Button height.
const BTN_H: u32 = 22;

/// Gap between buttons.
const BTN_GAP: i32 = 6;

/// Bitmap font index.
const FONT: usize = 1;

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// A side-effect produced by the character creation form.
#[derive(Clone, Debug)]
pub enum CharacterCreationFormAction {
    /// User pressed Create (or hit Enter).
    Create {
        /// Character name.
        name: String,
        /// Character description (may be empty).
        description: String,
        /// Selected character class.
        class: Class,
        /// Selected sex.
        sex: Sex,
    },
    /// User pressed the Random Name button.
    RandomName,
    /// User pressed Back.
    Back,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// The character creation form panel.
pub struct CharacterCreationForm {
    bounds: Bounds,
    /// Character name input.
    name_input: TextInput,
    /// Description input.
    description_input: TextInput,
    /// Class (race) selection.
    class_group: RadioGroup<Class>,
    /// Sex selection.
    sex_group: RadioGroup<Sex>,
    /// Create character button.
    create_button: RectButton,
    /// Random name button.
    random_name_button: RectButton,
    /// Back button.
    back_button: RectButton,
    /// Index of the currently focused text field (0 = name, 1 = description).
    focused_field: usize,
    /// Pending actions for the scene to drain.
    actions: Vec<CharacterCreationFormAction>,
    /// Whether to show the "Creating..." status.
    show_busy: bool,
    /// Optional error message text.
    error_text: Option<String>,
}

impl CharacterCreationForm {
    /// Creates a new character creation form centered on screen.
    ///
    /// # Returns
    ///
    /// A fully-initialised `CharacterCreationForm`.
    pub fn new() -> Self {
        let panel_x = (TARGET_WIDTH_INT as i32 - PANEL_W as i32) / 2;
        let panel_y = (TARGET_HEIGHT_INT as i32 - PANEL_H as i32) / 2;

        let bounds = Bounds::new(panel_x, panel_y, PANEL_W, PANEL_H);

        let border_normal = Color::RGBA(100, 100, 140, 200);
        let border_focused = Color::RGBA(180, 180, 255, 255);

        let btn_bg = Background::SolidColor(Color::RGBA(50, 50, 80, 200));
        let btn_border = Border {
            color: Color::RGBA(120, 120, 180, 200),
            width: 1,
        };

        // --- Name field ---
        let mut cursor_y = panel_y + 30; // title room
        let name_y = cursor_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let name_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, name_y, INPUT_W, INPUT_H),
            "character name",
            FONT,
            40,
            false,
            border_normal,
            border_focused,
        );

        // Random name button (right-aligned after input)
        let rand_btn_w: u32 = 100;
        let random_name_button = RectButton::new(
            Bounds::new(
                panel_x + PAD_X + INPUT_W as i32 - rand_btn_w as i32,
                name_y + INPUT_H as i32 + 2,
                rand_btn_w,
                BTN_H,
            ),
            btn_bg,
        )
        .with_border(btn_border)
        .with_label("Random name", FONT);

        cursor_y = name_y + INPUT_H as i32 + BTN_H as i32 + 4 + FIELD_GAP;

        // --- Description field ---
        let desc_y = cursor_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let description_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, desc_y, INPUT_W, INPUT_H),
            "description",
            FONT,
            200,
            false,
            border_normal,
            border_focused,
        );
        cursor_y = desc_y + INPUT_H as i32 + FIELD_GAP;

        // --- Race (class) radio group ---
        let class_label_y = cursor_y;
        cursor_y = class_label_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let class_group = RadioGroup::new(
            Bounds::new(panel_x + PAD_X, cursor_y, CLASS_GROUP_W, 60),
            &[
                (Class::Harakim, "Harakim"),
                (Class::Templar, "Templar"),
                (Class::Mercenary, "Mercenary"),
            ],
            Class::Mercenary,
        );
        cursor_y += 60 + FIELD_GAP;

        // --- Sex radio group ---
        let sex_label_y = cursor_y;
        cursor_y = sex_label_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let sex_group = RadioGroup::horizontal(
            Bounds::new(panel_x + PAD_X, cursor_y, INPUT_W, 20),
            &[(Sex::Male, "Male"), (Sex::Female, "Female")],
            Sex::Male,
        );
        cursor_y += 20 + FIELD_GAP + 4;

        // --- Create / Back buttons ---
        let total_btn_w = 2 * 150 + BTN_GAP as u32;
        let btn_start_x = panel_x + (PANEL_W as i32 - total_btn_w as i32) / 2;

        let create_button = RectButton::new(Bounds::new(btn_start_x, cursor_y, 150, BTN_H), btn_bg)
            .with_border(btn_border)
            .with_label("Create", FONT);

        let back_button = RectButton::new(
            Bounds::new(btn_start_x + 150 + BTN_GAP, cursor_y, 150, BTN_H),
            btn_bg,
        )
        .with_border(btn_border)
        .with_label("Back", FONT);

        let mut form = Self {
            bounds,
            name_input,
            description_input,
            class_group,
            sex_group,
            create_button,
            random_name_button,
            back_button,
            focused_field: 0,
            actions: Vec::new(),
            show_busy: false,
            error_text: None,
        };
        form.apply_focus();
        form
    }

    /// Returns the currently selected class.
    pub fn selected_class(&self) -> Class {
        self.class_group.selected()
    }

    /// Returns the currently selected sex.
    pub fn selected_sex(&self) -> Sex {
        self.sex_group.selected()
    }

    /// Returns a reference to the current name input value.
    pub fn name_input_value(&self) -> &str {
        self.name_input.value()
    }

    /// Returns a reference to the current description input value.
    pub fn description_input_value(&self) -> &str {
        self.description_input.value()
    }

    /// Sets the name field value (used by RandomName action handler).
    ///
    /// # Arguments
    ///
    /// * `name` - The name to set.
    pub fn set_name(&mut self, name: &str) {
        self.name_input.set_value(name);
    }

    /// Sets the busy / creating indicator.
    ///
    /// # Arguments
    ///
    /// * `busy` - `true` to show "Creating...", `false` to hide.
    pub fn set_busy(&mut self, busy: bool) {
        self.show_busy = busy;
        if busy {
            self.error_text = None;
        }
    }

    /// Sets or clears the error message.
    ///
    /// # Arguments
    ///
    /// * `msg` - Error text, or `None` to clear.
    pub fn set_error(&mut self, msg: Option<String>) {
        self.error_text = msg;
    }

    /// Drains pending [`CharacterCreationFormAction`]s.
    ///
    /// # Returns
    ///
    /// A vector of actions produced since the last call.
    pub fn take_actions(&mut self) -> Vec<CharacterCreationFormAction> {
        std::mem::take(&mut self.actions)
    }

    /// Pushes a Create action with the current field values.
    fn push_create_action(&mut self) {
        self.actions.push(CharacterCreationFormAction::Create {
            name: self.name_input.value().to_owned(),
            description: self.description_input.value().to_owned(),
            class: self.class_group.selected(),
            sex: self.sex_group.selected(),
        });
    }

    /// Advances keyboard focus to the next text field.
    fn cycle_focus_forward(&mut self) {
        self.focused_field = (self.focused_field + 1) % 2;
        self.apply_focus();
    }

    /// Moves keyboard focus to the previous text field.
    fn cycle_focus_backward(&mut self) {
        self.focused_field = if self.focused_field == 0 { 1 } else { 0 };
        self.apply_focus();
    }

    /// Synchronises `set_focused` on all text inputs.
    fn apply_focus(&mut self) {
        self.name_input.set_focused(self.focused_field == 0);
        self.description_input.set_focused(self.focused_field == 1);
    }

    /// Returns the field index (0–1) that contains the given point, if any.
    fn field_index_at(&self, x: i32, y: i32) -> Option<usize> {
        if self.name_input.bounds().contains_point(x, y) {
            Some(0)
        } else if self.description_input.bounds().contains_point(x, y) {
            Some(1)
        } else {
            None
        }
    }
}

impl Widget for CharacterCreationForm {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, _x: i32, _y: i32) {
        // Fixed layout — repositioning not supported.
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        // Tab / Enter key handling.
        if let UiEvent::KeyDown {
            keycode, modifiers, ..
        } = event
        {
            match *keycode {
                Keycode::Tab => {
                    if modifiers.shift {
                        self.cycle_focus_backward();
                    } else {
                        self.cycle_focus_forward();
                    }
                    return EventResponse::Consumed;
                }
                Keycode::Return | Keycode::KpEnter => {
                    self.push_create_action();
                    return EventResponse::Consumed;
                }
                _ => {}
            }
        }

        // Mouse click: detect field focus.
        if let UiEvent::MouseClick {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        {
            if let Some(idx) = self.field_index_at(*x, *y) {
                self.focused_field = idx;
                self.apply_focus();
            }
        }

        // Forward to radio groups.
        if self.class_group.handle_event(event) == EventResponse::Consumed {
            return EventResponse::Consumed;
        }
        if self.sex_group.handle_event(event) == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        // Forward to buttons.
        if self.create_button.handle_event(event) == EventResponse::Consumed {
            self.push_create_action();
            return EventResponse::Consumed;
        }
        if self.random_name_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(CharacterCreationFormAction::RandomName);
            return EventResponse::Consumed;
        }
        if self.back_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(CharacterCreationFormAction::Back);
            return EventResponse::Consumed;
        }

        // Forward to text inputs.
        self.name_input.handle_event(event);
        self.description_input.handle_event(event);

        // Consume if inside panel.
        if let UiEvent::MouseClick { x, y, .. } | UiEvent::MouseDown { x, y, .. } = event {
            if self.bounds.contains_point(*x, *y) {
                return EventResponse::Consumed;
            }
        }

        match event {
            UiEvent::TextInput { .. } | UiEvent::KeyDown { .. } => EventResponse::Consumed,
            _ => EventResponse::Ignored,
        }
    }

    fn update(&mut self, dt: Duration) {
        self.name_input.update(dt);
        self.description_input.update(dt);
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        // Panel background.
        let panel_rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(Color::RGBA(15, 15, 30, 210));
        ctx.canvas.fill_rect(panel_rect)?;
        ctx.canvas.set_draw_color(Color::RGBA(100, 100, 160, 200));
        ctx.canvas.draw_rect(panel_rect)?;

        // Title.
        let title = "Create Character";
        let title_cx = self.bounds.x + self.bounds.width as i32 / 2;
        let title_y = self.bounds.y + 10;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            title,
            title_cx,
            title_y,
            font_cache::TextStyle::centered(),
        )?;

        let mut cursor_y = title_y + font_cache::BITMAP_GLYPH_H as i32 + 8;

        // Name field.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Name",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        self.name_input
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.name_input.render(ctx)?;

        // Random name button (below name input, right-aligned).
        let rand_btn_y = cursor_y + INPUT_H as i32 + 2;
        let rand_btn_x =
            self.bounds.x + PAD_X + INPUT_W as i32 - self.random_name_button.bounds().width as i32;
        self.random_name_button.set_position(rand_btn_x, rand_btn_y);
        self.random_name_button.render(ctx)?;
        cursor_y = rand_btn_y + BTN_H as i32 + 2 + FIELD_GAP;

        // Description field.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Description",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        self.description_input
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.description_input.render(ctx)?;
        cursor_y += INPUT_H as i32 + FIELD_GAP;

        // Race (class) radio group.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Race",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let class_group_y = cursor_y;
        self.class_group
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.class_group.render(ctx)?;

        // Portrait sprite preview to the right of the race radio group.
        let sprite_id = mag_core::traits::get_sprite_id_for_class_and_sex(
            self.class_group.selected(),
            self.sex_group.selected(),
        );
        let sprite_x = self.bounds.x + PAD_X + CLASS_GROUP_W as i32 + SPRITE_PREVIEW_GAP as i32;
        let sprite_rect = sdl2::rect::Rect::new(
            sprite_x,
            class_group_y,
            SPRITE_PREVIEW_SIZE,
            SPRITE_PREVIEW_SIZE,
        );
        let texture = ctx.gfx.get_texture(sprite_id);
        let _ = ctx.canvas.copy(texture, None, sprite_rect);
        cursor_y += 60 + FIELD_GAP;

        // Sex radio group.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Sex",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        self.sex_group.set_position(self.bounds.x + PAD_X, cursor_y);
        self.sex_group.render(ctx)?;
        cursor_y += 20 + FIELD_GAP + 4;

        // Create / Back buttons.
        let total_btn_w: i32 = 2 * 150 + BTN_GAP;
        let btn_x = self.bounds.x + (self.bounds.width as i32 - total_btn_w) / 2;
        self.create_button.set_position(btn_x, cursor_y);
        self.back_button
            .set_position(btn_x + 150 + BTN_GAP, cursor_y);
        self.create_button.render(ctx)?;
        self.back_button.render(ctx)?;
        cursor_y += BTN_H as i32 + 8;

        // Status / error labels.
        if self.show_busy {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                "Creating character...",
                self.bounds.x + PAD_X,
                cursor_y,
                font_cache::TextStyle::tinted(Color::RGB(180, 180, 255)),
            )?;
        }

        if let Some(ref err) = self.error_text {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                err,
                self.bounds.x + PAD_X,
                cursor_y,
                font_cache::TextStyle::tinted(Color::RGB(255, 80, 80)),
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
    use crate::ui::widget::KeyModifiers;

    fn make_form() -> CharacterCreationForm {
        CharacterCreationForm::new()
    }

    #[test]
    fn default_selections() {
        let form = make_form();
        assert_eq!(form.selected_class(), Class::Mercenary);
        assert_eq!(form.selected_sex(), Sex::Male);
    }

    #[test]
    fn set_name_updates_value() {
        let mut form = make_form();
        form.set_name("Tester");
        assert_eq!(form.name_input.value(), "Tester");
    }

    #[test]
    fn tab_cycles_focus() {
        let mut form = make_form();
        assert_eq!(form.focused_field, 0);
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(form.focused_field, 1);
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(form.focused_field, 0);
    }

    #[test]
    fn shift_tab_cycles_backward() {
        let mut form = make_form();
        assert_eq!(form.focused_field, 0);
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: KeyModifiers {
                shift: true,
                ..Default::default()
            },
        });
        assert_eq!(form.focused_field, 1);
    }

    #[test]
    fn enter_pushes_create_action() {
        let mut form = make_form();
        form.set_name("Hero");
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: KeyModifiers::default(),
        });
        let actions = form.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            CharacterCreationFormAction::Create { name, .. } => {
                assert_eq!(name, "Hero");
            }
            _ => panic!("Expected Create action"),
        }
    }

    #[test]
    fn set_busy_clears_error() {
        let mut form = make_form();
        form.set_error(Some("fail".into()));
        assert!(form.error_text.is_some());
        form.set_busy(true);
        assert!(form.error_text.is_none());
    }
}
