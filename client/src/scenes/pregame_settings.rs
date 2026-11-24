//! Settings (gear) button and reduced settings panel overlaid on every
//! pre-game scene.

use sdl2::{
    controller::Button as Btn, event::Event, keyboard::Keycode, pixels::Color, rect::Point,
    render::Canvas, video::Window,
};

use crate::{
    constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT},
    preferences,
    state::{AppState, apply_global_settings_action},
    ui::{
        self, RenderContext,
        controller_nav::ControllerNavState,
        hud::settings_panel::{PREGAME_SETTINGS_PANEL_H, SettingsPanel, SettingsPanelData},
        widget::{Bounds, EventResponse, KeyModifiers, UiEvent, Widget, WidgetAction},
        widgets::button::CircleButton,
    },
};

/// Radius of the settings button.
const BUTTON_RADIUS: u32 = 14;
/// Gap between the settings button and the edges of its anchor box.
const BUTTON_MARGIN: i32 = 6;
/// Color of the gear icon, matched to the yellow UI font.
const ICON_COLOR: Color = Color::RGB(230, 215, 150);
/// Width of the settings panel.
const PANEL_W: u32 = 300;

/// Settings button, drawn in the lower-right corner of the active scene's main UI
/// box, that opens a modal sound/graphics settings panel.
pub struct PregameSettings {
    button: CircleButton,
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
    /// Creates the settings button and a hidden settings panel.
    ///
    /// # Returns
    ///
    /// * A new `PregameSettings` with the panel closed.
    pub fn new() -> Self {
        Self {
            // Styled like the pre-game forms' rectangular buttons.
            button: CircleButton::new(0, 0, BUTTON_RADIUS, Color::RGBA(50, 50, 80, 200))
                .with_border_color(Color::RGBA(120, 120, 180, 200)),
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

    /// Moves the settings button into the lower-right corner of `anchor`.
    ///
    /// # Arguments
    ///
    /// * `anchor` - The active scene's main UI box, or `None` for the screen.
    fn place_button(&mut self, anchor: Option<Bounds>) {
        let b = anchor.unwrap_or(Bounds::new(0, 0, TARGET_WIDTH_INT, TARGET_HEIGHT_INT));
        let d = 2 * BUTTON_RADIUS as i32;
        self.button.set_position(
            b.x + b.width as i32 - BUTTON_MARGIN - d,
            b.y + b.height as i32 - BUTTON_MARGIN - d,
        );
    }

    /// Handles an SDL event before the active scene sees it.
    ///
    /// The settings button (mouse) or controller Back button opens the panel;
    /// while open the panel is modal and swallows all input. Escape, Back or
    /// the settings button close it again.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state whose settings are edited.
    /// * `event` - Raw SDL event.
    /// * `anchor` - The active scene's main UI box the settings button is drawn in.
    ///
    /// # Returns
    ///
    /// * `true` if the event was consumed and must not reach the scene.
    pub fn handle_event(
        &mut self,
        app_state: &mut AppState<'_>,
        event: &Event,
        anchor: Option<Bounds>,
    ) -> bool {
        self.place_button(anchor);
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
        let button_clicked = ui_event
            .as_ref()
            .is_some_and(|e| self.button.handle_event(e) == EventResponse::Consumed);

        if !self.panel.is_visible() {
            if back_pressed || button_clicked {
                self.panel
                    .sync_state(&SettingsPanelData::from_settings(&app_state.settings));
                self.panel.toggle();
                return true;
            }
            // Swallow presses on the button so the scene underneath doesn't react.
            return matches!(ui_event, Some(UiEvent::MouseDown { x, y, .. })
                if self.button.bounds().contains_point(x, y));
        }

        let escape = matches!(
            event,
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            }
        );
        if back_pressed || button_clicked || escape {
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

    /// Draws the settings button and, when open, the settings panel on top.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (graphics caches).
    /// * `canvas` - Target canvas.
    /// * `anchor` - The active scene's main UI box the settings button is drawn in.
    ///
    /// # Returns
    ///
    /// * `Err` with the SDL error message if drawing fails.
    pub fn render(
        &mut self,
        app_state: &mut AppState<'_>,
        canvas: &mut Canvas<Window>,
        anchor: Option<Bounds>,
    ) -> Result<(), String> {
        self.place_button(anchor);
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
        self.button.render(&mut ctx)?;
        let b = *self.button.bounds();
        let r = BUTTON_RADIUS as i32;
        draw_gear(ctx.canvas, b.x + r, b.y + r)?;
        self.panel.render(&mut ctx)
    }
}

/// Pixel mask of the gear icon (`#` = lit), drawn centered on the button:
/// eight square teeth around a solid body with a round center hole.
const GEAR_MASK: [&str; 19] = [
    "........###........",
    "........###........",
    "...##...###...##...",
    "..###..#####..###..",
    "..###############..",
    "....###########....",
    "....###########....",
    "...#####...#####...",
    "#######.....#######",
    "#######.....#######",
    "#######.....#######",
    "...#####...#####...",
    "....###########....",
    "....###########....",
    "..###############..",
    "..###..#####..###..",
    "...##...###...##...",
    "........###........",
    "........###........",
];

/// Draws the gear icon from [`GEAR_MASK`] centered on `(cx, cy)`.
///
/// # Arguments
///
/// * `canvas` - Target canvas.
/// * `cx` - Icon center X in logical pixels.
/// * `cy` - Icon center Y in logical pixels.
///
/// # Returns
///
/// * `Err` with the SDL error message if drawing fails.
fn draw_gear(canvas: &mut Canvas<Window>, cx: i32, cy: i32) -> Result<(), String> {
    let half = GEAR_MASK.len() as i32 / 2;
    let points: Vec<Point> = GEAR_MASK
        .iter()
        .enumerate()
        .flat_map(|(y, row)| {
            row.bytes()
                .enumerate()
                .filter(|&(_, c)| c == b'#')
                .map(move |(x, _)| Point::new(cx - half + x as i32, cy - half + y as i32))
        })
        .collect();
    canvas.set_draw_color(ICON_COLOR);
    canvas.draw_points(points.as_slice())
}
