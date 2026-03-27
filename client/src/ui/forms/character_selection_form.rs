//! Composite character-selection form widget.
//!
//! Displays a scrollable list of the player's characters and action buttons
//! for creating a new character, continuing into the game, deleting a
//! character, or logging out.  The owning scene reads pending
//! [`CharacterSelectionFormAction`]s via
//! [`CharacterSelectionForm::take_actions`].

use std::time::Duration;

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::ui::RenderContext;
use crate::ui::widgets::button::RectButton;
use crate::ui::widgets::scrollable_list::{ListItem, ScrollableList};
use crate::ui::style::{Background, Border};
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget};
use crate::constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT};
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Panel dimensions.
const PANEL_W: u32 = 360;
const PANEL_H: u32 = 464;

/// Horizontal padding inside the panel.
const PAD_X: i32 = 20;

/// Height of the scrollable character list (3 rows × 68 px).
const LIST_H: u32 = 204;

/// Width of action buttons.
const BTN_W: u32 = PANEL_W - (PAD_X as u32) * 2;

/// Button height.
const BTN_H: u32 = 22;

/// Gap between buttons.
const BTN_GAP: i32 = 6;

/// Bitmap font index.
const FONT: usize = 1;

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// A side-effect produced by the character selection form.
#[derive(Clone, Debug)]
pub enum CharacterSelectionFormAction {
    /// User wants to create a new character.
    CreateNew,
    /// User wants to continue to the game with the selected character.
    ContinueToGame {
        /// Selected character ID.
        character_id: u64,
    },
    /// User wants to delete the selected character.
    DeleteCharacter {
        /// Character ID to delete.
        character_id: u64,
        /// Character name (for the confirmation dialog).
        character_name: String,
    },
    /// User wants to log out.
    LogOut,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// The character selection form panel.
pub struct CharacterSelectionForm {
    bounds: Bounds,
    /// Scrollable character list.
    character_list: ScrollableList,
    /// "Create new character" button.
    create_button: RectButton,
    /// "Continue to game" button.
    continue_button: RectButton,
    /// "Delete character" button.
    delete_button: RectButton,
    /// "Log out" button.
    logout_button: RectButton,
    /// Pending actions for the scene to drain.
    actions: Vec<CharacterSelectionFormAction>,
    /// Optional error message.
    error_text: Option<String>,
    /// Status text (e.g. "Loading characters...").
    status_text: Option<String>,
    /// Username for display.
    username: Option<String>,
    /// Cached character names keyed by ID (for delete dialog).
    character_names: Vec<(u64, String)>,
}

impl CharacterSelectionForm {
    /// Creates a new character selection form centered on screen.
    ///
    /// # Returns
    ///
    /// A fully-initialised `CharacterSelectionForm`.
    pub fn new() -> Self {
        let panel_x = (TARGET_WIDTH_INT as i32 - PANEL_W as i32) / 2;
        let panel_y = (TARGET_HEIGHT_INT as i32 - PANEL_H as i32) / 2;

        let bounds = Bounds::new(panel_x, panel_y, PANEL_W, PANEL_H);

        let list_w = PANEL_W - (PAD_X as u32) * 2;

        // List starts after title + username + spacing
        let list_y = panel_y + 50;
        let character_list =
            ScrollableList::new(Bounds::new(panel_x + PAD_X, list_y, list_w, LIST_H));

        let btn_bg = Background::SolidColor(Color::RGBA(50, 50, 80, 200));
        let btn_border = Border {
            color: Color::RGBA(120, 120, 180, 200),
            width: 1,
        };
        let delete_bg = Background::SolidColor(Color::RGBA(80, 30, 30, 200));
        let delete_border = Border {
            color: Color::RGBA(200, 80, 80, 200),
            width: 1,
        };

        let mut btn_y = list_y + LIST_H as i32 + 12;

        let create_button =
            RectButton::new(Bounds::new(panel_x + PAD_X, btn_y, BTN_W, BTN_H), btn_bg)
                .with_border(btn_border)
                .with_label("Create new character", FONT);
        btn_y += BTN_H as i32 + BTN_GAP;

        let continue_button =
            RectButton::new(Bounds::new(panel_x + PAD_X, btn_y, BTN_W, BTN_H), btn_bg)
                .with_border(btn_border)
                .with_label("Continue to game", FONT);
        btn_y += BTN_H as i32 + BTN_GAP;

        let delete_button =
            RectButton::new(Bounds::new(panel_x + PAD_X, btn_y, BTN_W, BTN_H), delete_bg)
                .with_border(delete_border)
                .with_label("Delete character", FONT);
        btn_y += BTN_H as i32 + BTN_GAP;

        let logout_button =
            RectButton::new(Bounds::new(panel_x + PAD_X, btn_y, BTN_W, BTN_H), btn_bg)
                .with_border(btn_border)
                .with_label("Log out", FONT);

        Self {
            bounds,
            character_list,
            create_button,
            continue_button,
            delete_button,
            logout_button,
            actions: Vec::new(),
            error_text: None,
            status_text: None,
            username: None,
            character_names: Vec::new(),
        }
    }

    /// Sets the list of characters to display.
    ///
    /// # Arguments
    ///
    /// * `items` - Character list items.
    /// * `names` - Parallel list of `(id, name)` pairs for the delete dialog.
    pub fn set_characters(&mut self, items: Vec<ListItem>, names: Vec<(u64, String)>) {
        self.character_list.set_items(items);
        self.character_names = names;
    }

    /// Returns the currently selected character ID, if any.
    pub fn selected_character_id(&self) -> Option<u64> {
        self.character_list.selected_id()
    }

    /// Sets the selected character by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Character ID to select, or `None` to clear.
    pub fn set_selected(&mut self, id: Option<u64>) {
        self.character_list.set_selected(id);
    }

    /// Removes a character from the list by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Character ID to remove.
    pub fn remove_character(&mut self, id: u64) {
        self.character_list.remove_item(id);
        self.character_names.retain(|(cid, _)| *cid != id);
    }

    /// Returns `true` if the character list is empty.
    pub fn is_empty(&self) -> bool {
        self.character_list.is_empty()
    }

    /// Sets the username to display.
    ///
    /// # Arguments
    ///
    /// * `username` - The username, or `None` to clear.
    pub fn set_username(&mut self, username: Option<String>) {
        self.username = username;
    }

    /// Sets or clears the status text.
    ///
    /// # Arguments
    ///
    /// * `text` - Status message, or `None` to clear.
    pub fn set_status(&mut self, text: Option<String>) {
        self.status_text = text;
    }

    /// Sets or clears the error message.
    ///
    /// # Arguments
    ///
    /// * `msg` - Error text, or `None` to clear.
    pub fn set_error(&mut self, msg: Option<String>) {
        self.error_text = msg;
    }

    /// Drains pending [`CharacterSelectionFormAction`]s.
    ///
    /// # Returns
    ///
    /// A vector of actions produced since the last call.
    pub fn take_actions(&mut self) -> Vec<CharacterSelectionFormAction> {
        std::mem::take(&mut self.actions)
    }

    /// Looks up a character name by ID from the cached names.
    fn character_name_for_id(&self, id: u64) -> Option<&str> {
        self.character_names
            .iter()
            .find(|(cid, _)| *cid == id)
            .map(|(_, name)| name.as_str())
    }
}

impl Widget for CharacterSelectionForm {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, _x: i32, _y: i32) {
        // Fixed layout — repositioning not supported.
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        // Forward to character list.
        if self.character_list.handle_event(event) == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        // Forward to buttons.
        if self.create_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(CharacterSelectionFormAction::CreateNew);
            return EventResponse::Consumed;
        }
        if self.continue_button.handle_event(event) == EventResponse::Consumed {
            if let Some(id) = self.character_list.selected_id() {
                self.actions
                    .push(CharacterSelectionFormAction::ContinueToGame { character_id: id });
            }
            return EventResponse::Consumed;
        }
        if self.delete_button.handle_event(event) == EventResponse::Consumed {
            if let Some(id) = self.character_list.selected_id() {
                let name = self.character_name_for_id(id).unwrap_or("").to_owned();
                self.actions
                    .push(CharacterSelectionFormAction::DeleteCharacter {
                        character_id: id,
                        character_name: name,
                    });
            }
            return EventResponse::Consumed;
        }
        if self.logout_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(CharacterSelectionFormAction::LogOut);
            return EventResponse::Consumed;
        }

        // Consume if inside panel.
        if let UiEvent::MouseClick { x, y, .. } | UiEvent::MouseDown { x, y, .. } = event {
            if self.bounds.contains_point(*x, *y) {
                return EventResponse::Consumed;
            }
        }

        EventResponse::Ignored
    }

    fn update(&mut self, dt: Duration) {
        self.character_list.update(dt);
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
        let title = "Character Selection";
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

        let mut cursor_y = title_y + font_cache::BITMAP_GLYPH_H as i32 + 6;

        // Username label.
        if let Some(ref username) = self.username {
            let label = format!("Logged in as: {username}");
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                &label,
                self.bounds.x + PAD_X,
                cursor_y,
                font_cache::TextStyle::PLAIN,
            )?;
            cursor_y += font_cache::BITMAP_GLYPH_H as i32 + 4;
        }

        // Error.
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
            cursor_y += font_cache::BITMAP_GLYPH_H as i32 + 4;
        }

        // Status.
        if let Some(ref status) = self.status_text {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                status,
                self.bounds.x + PAD_X,
                cursor_y,
                font_cache::TextStyle::tinted(Color::RGB(180, 180, 255)),
            )?;
            cursor_y += font_cache::BITMAP_GLYPH_H as i32 + 4;
        }

        // "Characters" label.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Characters",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + 4;

        // Character list.
        self.character_list
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.character_list.render(ctx)?;
        cursor_y += LIST_H as i32 + 12;

        // Buttons.
        self.create_button
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.create_button.render(ctx)?;
        cursor_y += BTN_H as i32 + BTN_GAP;

        self.continue_button
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.continue_button.render(ctx)?;
        cursor_y += BTN_H as i32 + BTN_GAP;

        self.delete_button
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.delete_button.render(ctx)?;
        cursor_y += BTN_H as i32 + BTN_GAP;

        self.logout_button
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.logout_button.render(ctx)?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_form() -> CharacterSelectionForm {
        CharacterSelectionForm::new()
    }

    fn sample_items() -> (Vec<ListItem>, Vec<(u64, String)>) {
        let items = vec![
            ListItem {
                id: 1,
                label: "Hero (Mercenary)".into(),
                sprite_id: Some(5072),
            },
            ListItem {
                id: 2,
                label: "Mage (Harakim)".into(),
                sprite_id: Some(4048),
            },
        ];
        let names = vec![(1, "Hero".into()), (2, "Mage".into())];
        (items, names)
    }

    #[test]
    fn initially_empty() {
        let form = make_form();
        assert!(form.is_empty());
        assert!(form.selected_character_id().is_none());
    }

    #[test]
    fn set_characters_populates_list() {
        let mut form = make_form();
        let (items, names) = sample_items();
        form.set_characters(items, names);
        assert!(!form.is_empty());
    }

    #[test]
    fn remove_character_works() {
        let mut form = make_form();
        let (items, names) = sample_items();
        form.set_characters(items, names);
        form.remove_character(1);
        assert_eq!(form.character_names.len(), 1);
    }

    #[test]
    fn set_error_and_status() {
        let mut form = make_form();
        form.set_error(Some("fail".into()));
        assert!(form.error_text.is_some());
        form.set_status(Some("loading".into()));
        assert!(form.status_text.is_some());
        form.set_error(None);
        assert!(form.error_text.is_none());
    }
}
