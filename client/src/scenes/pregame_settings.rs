//! Gear button and reduced settings panel overlaid on every pre-game scene.

use sdl2::{
    controller::Button as Btn, event::Event, keyboard::Keycode, pixels::Color, render::Canvas,
    video::Window,
};

use crate::{
    constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT},
    filepaths, preferences,
    state::{AppState, apply_global_settings_action},
    ui::{
        self, RenderContext,
        controller_nav::ControllerNavState,
        hud::settings_panel::{PREGAME_SETTINGS_PANEL_H, SettingsPanel, SettingsPanelData},
        widget::{EventResponse, KeyModifiers, UiEvent, Widget, WidgetAction},
        widgets::button::CircularImageButton,
    },
};

/// Radius of the gear button.
const GEAR_RADIUS: u32 = 14;
/// Gap between the gear button and the screen edges.
const GEAR_MARGIN: i32 = 8;
/// Width of the settings panel.
const PANEL_W: u32 = 300;

/// Bottom-right gear button that opens a modal sound/graphics settings panel.
pub struct PregameSettings {
    gear: CircularImageButton,
    panel: SettingsPanel,
    mouse_x: i32,
    mouse_y: i32,
    controller_nav: ControllerNavState,
}

impl Default for PregameSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl PregameSettings {
    /// Creates the gear button and a hidden settings panel.
    ///
    /// # Returns
    ///
    /// * A new `PregameSettings` with the panel closed.
    pub fn new() -> Self {
        let r = GEAR_RADIUS as i32;
        Self {
            gear: CircularImageButton::new(
                TARGET_WIDTH_INT as i32 - GEAR_MARGIN - r,
                TARGET_HEIGHT_INT as i32 - GEAR_MARGIN - r,
                GEAR_RADIUS,
                filepaths::get_asset_directory()
                    .join("gfx")
                    .join("buttons")
                    .join("settings.png"),
            ),
            panel: SettingsPanel::new_pregame(
                (TARGET_WIDTH_INT - PANEL_W) as i32 / 2,
                (TARGET_HEIGHT_INT - PREGAME_SETTINGS_PANEL_H) as i32 / 2,
                PANEL_W,
                Color::RGBA(10, 10, 30, 220),
            ),
            mouse_x: 0,
            mouse_y: 0,
            controller_nav: ControllerNavState::new(),
        }
    }

    /// Handles an SDL event before the active scene sees it.
    ///
    /// The gear (mouse) or controller Back button opens the panel; while open
    /// the panel is modal and swallows all input. Escape, Back or the gear
    /// close it again.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state whose settings are edited.
    /// * `event` - Raw SDL event.
    ///
    /// # Returns
    ///
    /// * `true` if the event was consumed and must not reach the scene.
    pub fn handle_event(&mut self, app_state: &mut AppState<'_>, event: &Event) -> bool {
        if let Event::MouseMotion { x, y, .. } = event {
            self.mouse_x = *x;
            self.mouse_y = *y;
        }
        let ui_event =
            ui::sdl_to_ui_event(event, self.mouse_x, self.mouse_y, KeyModifiers::default());
        let back_pressed = matches!(
            event,
            Event::ControllerButtonDown {
                button: Btn::Back,
                ..
            }
        );
        let gear_clicked = ui_event
            .as_ref()
            .is_some_and(|e| self.gear.handle_event(e) == EventResponse::Consumed);

        if !self.panel.is_visible() {
            if back_pressed || gear_clicked {
                self.panel
                    .sync_state(&SettingsPanelData::from_settings(&app_state.settings));
                self.panel.toggle();
                return true;
            }
            // Swallow presses on the gear so the scene underneath doesn't react.
            return matches!(ui_event, Some(UiEvent::MouseDown { x, y, .. })
                if self.gear.bounds().contains_point(x, y));
        }

        let escape = matches!(
            event,
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            }
        );
        if back_pressed || gear_clicked || escape {
            self.panel.close();
        } else {
            if let Some(nav) = self.controller_nav.process_event(event) {
                self.panel.handle_event(&nav);
            }
            if let Some(e) = ui_event {
                self.panel.handle_event(&e);
            }
        }

        let mut changed = false;
        for action in self.panel.take_actions() {
            if let WidgetAction::SetMasterVolume(v) = action {
                app_state.sfx_cache.set_music_volume(v);
            }
            changed |= apply_global_settings_action(
                &mut app_state.settings,
                &mut app_state.display_command,
                &action,
            );
        }
        if changed && let Err(e) = preferences::save_global_settings(&app_state.settings) {
            log::warn!("Failed to save settings: {e}");
        }
        true
    }

    /// Draws the gear button and, when open, the settings panel on top.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (graphics caches).
    /// * `canvas` - Target canvas.
    ///
    /// # Returns
    ///
    /// * `Err` with the SDL error message if drawing fails.
    pub fn render(
        &mut self,
        app_state: &mut AppState<'_>,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        let AppState {
            gfx_cache,
            text_engine,
            ..
        } = app_state;
        let mut ctx = RenderContext {
            canvas,
            gfx: gfx_cache,
            text: text_engine,
        };
        self.gear.render(&mut ctx)?;
        self.panel.render(&mut ctx)
    }
}
