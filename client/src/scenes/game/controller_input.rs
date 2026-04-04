//! Controller input handling for [`GameScene`].
//!
//! All SDL2 `ControllerButtonDown`, `ControllerButtonUp`, and
//! `ControllerAxisMotion` events dispatched from `handle_event` are routed
//! here so that the main event handler stays readable.

use std::time::Instant;

use sdl2::{
    controller::{Axis, Button as Btn},
    event::Event,
    mouse::MouseButton,
};

use crate::{
    network::client_commands::ClientCommand,
    scenes::scene::SceneType,
    state::AppState,
    types::controller::ControllerButton,
    ui::widget::{UiEvent, Widget},
    ui::widgets::on_screen_keyboard::OnScreenKeyboardAction,
};

use super::GameScene;

impl GameScene {
    /// Handle all controller SDL2 events: button down/up and axis motion.
    ///
    /// This method is called exclusively for `ControllerButtonDown`,
    /// `ControllerButtonUp`, and `ControllerAxisMotion` events. All other
    /// event types must not be passed here.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    /// * `event` - The raw SDL2 event (must be a controller variant).
    ///
    /// # Returns
    ///
    /// * `Some(SceneType)` to trigger a scene transition, or `None` to stay.
    pub(super) fn handle_controller_event(
        &mut self,
        app_state: &mut AppState<'_>,
        event: &Event,
    ) -> Option<SceneType> {
        match event {
            Event::ControllerButtonDown { button, .. } => {
                log::info!("Controller button pressed: {:?}", button);
                match button {
                    Btn::LeftShoulder => self.lb_held = true,
                    Btn::RightShoulder => self.rb_held = true,
                    _ => {}
                }

                // On-screen keyboard intercept (must be checked before any
                // other controller handling so X/Start/DPad are captured).
                if self.keyboard.is_visible() {
                    match button {
                        Btn::X => {
                            self.keyboard.handle_event(&UiEvent::KeyboardToggleShift);
                            return None;
                        }
                        Btn::Start => {
                            self.keyboard.handle_event(&UiEvent::KeyboardDismiss);
                            for kb_action in self.keyboard.take_actions() {
                                if let OnScreenKeyboardAction::Dismiss = kb_action {
                                    self.keyboard.hide();
                                    self.chat_box.set_focused(false);
                                }
                            }
                            return None;
                        }
                        Btn::DPadUp => {
                            self.keyboard.handle_event(&UiEvent::KeyboardRowUp);
                            return None;
                        }
                        Btn::DPadDown => {
                            self.keyboard.handle_event(&UiEvent::KeyboardRowDown);
                            return None;
                        }
                        Btn::A => {
                            self.keyboard.handle_event(&UiEvent::NavConfirm);
                            for kb_action in self.keyboard.take_actions() {
                                match kb_action {
                                    OnScreenKeyboardAction::TypeChar(ch) => {
                                        self.chat_box.inject_char(ch);
                                    }
                                    OnScreenKeyboardAction::Backspace => {
                                        self.chat_box.inject_backspace();
                                    }
                                    OnScreenKeyboardAction::Dismiss => {
                                        self.keyboard.hide();
                                        self.chat_box.set_focused(false);
                                    }
                                }
                            }
                            return None;
                        }
                        Btn::B => {
                            self.keyboard.handle_event(&UiEvent::NavBack);
                            for kb_action in self.keyboard.take_actions() {
                                if let OnScreenKeyboardAction::Backspace = kb_action {
                                    self.chat_box.inject_backspace();
                                }
                            }
                            return None;
                        }
                        Btn::DPadLeft => {
                            self.keyboard.handle_event(&UiEvent::NavPrev);
                            return None;
                        }
                        Btn::DPadRight => {
                            self.keyboard.handle_event(&UiEvent::NavNext);
                            return None;
                        }
                        _ => {} // LB/RB/Y/etc. pass through
                    }
                }

                // Y button → open chat with on-screen keyboard (controller mode only)
                if *button == Btn::Y
                    && self.controller_mode
                    && !self.settings_panel.is_visible()
                    && !self.keyboard.is_visible()
                {
                    self.chat_box.set_focused(true);
                    self.keyboard.show();
                    return None;
                }

                // If the controller bindings panel is waiting for a button
                // press, capture it and skip the normal skill-dispatch path.
                if self.settings_panel.is_controller_listening() {
                    if let Some(cb) =
                        ControllerButton::from_sdl2(*button, self.lb_held, self.rb_held)
                    {
                        log::info!("Controller binding captured: {:?} -> {:?}", button, cb);
                        self.settings_panel.capture_controller_button(cb);
                        self.process_settings_panel_actions(app_state);
                    }
                    return None;
                }

                // Start → toggle settings panel
                if *button == Btn::Start {
                    self.settings_panel.toggle();
                    if self.settings_panel.is_visible() {
                        let data = self.build_settings_panel_data(app_state);
                        self.settings_panel.sync_state(&data);
                    }
                    return None;
                }

                // When the settings panel is open, forward nav events to it
                if self.settings_panel.is_visible() {
                    if let Some(nav_event) = self.hud_nav.process_event(event) {
                        self.settings_panel.handle_event(&nav_event);
                        if let Some(sc) = self.process_settings_panel_actions(app_state) {
                            return Some(sc);
                        }
                        // NavBack on main settings panel → close it
                        if matches!(nav_event, UiEvent::NavBack)
                            && !self.settings_panel.is_visible()
                        {
                            // Panel already closed itself via NavBack handling
                        }
                        return None;
                    }
                }

                // D-pad left/right → skill bar slot navigation (gameplay only)
                if self.controller_mode && (*button == Btn::DPadLeft || *button == Btn::DPadRight) {
                    use crate::ui::hud::skill_bar::TOP_CELLS;
                    let current = self.skill_bar.controller_selected_slot();
                    let next = if *button == Btn::DPadRight {
                        Some(current.map_or(0, |s| (s + 1) % TOP_CELLS))
                    } else {
                        Some(current.map_or(TOP_CELLS - 1, |s| {
                            if s == 0 { TOP_CELLS - 1 } else { s - 1 }
                        }))
                    };
                    self.skill_bar.set_controller_selected_slot(next);
                    return None;
                }

                // B button → clear skill bar selection (gameplay only)
                if *button == Btn::B && self.controller_mode {
                    self.skill_bar.set_controller_selected_slot(None);
                    return None;
                }

                // Right stick press (R3) → activate highlighted skill
                if *button == Btn::RightStick && self.controller_mode {
                    if let Some(slot) = self.skill_bar.controller_selected_slot() {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            if let Some(skill_nr) =
                                app_state.settings.character.skill_keybinds[slot]
                            {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    skill_nr as u32,
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    }
                    return None;
                }

                // Left stick press (L3) → start press timer for select/look
                if *button == Btn::LeftStick && self.controller_mode {
                    self.l3_pressed_at = Some(Instant::now());
                    return None;
                }

                // A button → left-click equivalent (LB = shift, RB = ctrl)
                if *button == Btn::A && self.controller_mode {
                    let orig_shift = self.shift_held;
                    let orig_ctrl = self.ctrl_held;
                    self.shift_held = self.lb_held;
                    self.ctrl_held = self.rb_held;
                    self.handle_world_click(
                        app_state,
                        MouseButton::Left,
                        self.mouse_x,
                        self.mouse_y,
                    );
                    self.shift_held = orig_shift;
                    self.ctrl_held = orig_ctrl;
                    return None;
                }

                // X button → right-click equivalent (LB = shift, RB = ctrl)
                if *button == Btn::X && self.controller_mode {
                    let orig_shift = self.shift_held;
                    let orig_ctrl = self.ctrl_held;
                    self.shift_held = self.lb_held;
                    self.ctrl_held = self.rb_held;
                    self.handle_world_click(
                        app_state,
                        MouseButton::Right,
                        self.mouse_x,
                        self.mouse_y,
                    );
                    self.shift_held = orig_shift;
                    self.ctrl_held = orig_ctrl;
                    return None;
                }

                // Mapped controller button → skill dispatch
                if let Some(cb) = ControllerButton::from_sdl2(*button, self.lb_held, self.rb_held) {
                    log::info!("Controller button mapped to {:?}", cb);
                    if let Some(slot) = app_state
                        .settings
                        .character
                        .controller_bindings
                        .slot_for_button(cb)
                    {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            if let Some(skill_nr) =
                                app_state.settings.character.skill_keybinds[slot]
                            {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    skill_nr as u32,
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    }
                }
                None
            }

            Event::ControllerButtonUp { button, .. } => {
                match button {
                    Btn::LeftShoulder => self.lb_held = false,
                    Btn::RightShoulder => self.rb_held = false,
                    _ => {}
                }

                // Left stick release (L3) → short press = select/deselect character
                if *button == Btn::LeftStick && self.controller_mode {
                    if let Some(_pressed_at) = self.l3_pressed_at.take() {
                        // Hold threshold not reached (would have been consumed
                        // in update()), so this is a short press → select.
                        if let Some(ps) = app_state.player_state.as_ref() {
                            let (cam_xoff, cam_yoff) = Self::camera_offsets(ps);
                            if let Some((mx, my)) = Self::screen_to_map_tile(
                                self.mouse_x,
                                self.mouse_y,
                                cam_xoff,
                                cam_yoff,
                            ) {
                                use mag_core::constants::ISCHAR;
                                let selected_char = ps.selected_char();
                                if let Some((sx, sy)) =
                                    Self::nearest_tile_with_flag(ps, mx, my, ISCHAR)
                                {
                                    let tile = ps.map().tile_at_xy(sx, sy);
                                    let target_cn = tile.map(|t| t.ch_nr as u32).unwrap_or(0);
                                    let target_id = tile.map(|t| t.ch_id).unwrap_or(0);
                                    if target_cn != 0 {
                                        if let Some(ps_mut) = app_state.player_state.as_mut() {
                                            if selected_char == target_cn as u16 {
                                                ps_mut.clear_selected_char();
                                            } else {
                                                ps_mut.set_selected_char_with_id(
                                                    target_cn as u16,
                                                    target_id,
                                                );
                                            }
                                        }
                                    }
                                } else if selected_char != 0 {
                                    // No character near cursor but one is selected → deselect
                                    if let Some(ps_mut) = app_state.player_state.as_mut() {
                                        ps_mut.clear_selected_char();
                                    }
                                }
                            }
                        }
                    }
                }

                None
            }

            Event::ControllerAxisMotion { axis, value, .. } => {
                // Track stick axes for virtual cursor and skill bar navigation in update()
                match axis {
                    Axis::LeftX => self.left_stick_x = *value,
                    Axis::LeftY => self.left_stick_y = *value,
                    Axis::RightX => self.right_stick_x = *value,
                    _ => {}
                }

                // When settings panel is open (and keyboard hidden), forward nav events from stick
                if self.settings_panel.is_visible() && !self.keyboard.is_visible() {
                    if let Some(nav_event) = self.hud_nav.process_event(event) {
                        self.settings_panel.handle_event(&nav_event);
                        if let Some(sc) = self.process_settings_panel_actions(app_state) {
                            return Some(sc);
                        }
                        return None;
                    }
                }

                // Trigger axis → skill dispatch
                if let Some(cb) = ControllerButton::from_trigger_axis(*axis, *value) {
                    if let Some(slot) = app_state
                        .settings
                        .character
                        .controller_bindings
                        .slot_for_button(cb)
                    {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            if let Some(skill_nr) =
                                app_state.settings.character.skill_keybinds[slot]
                            {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    skill_nr as u32,
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    }
                }
                None
            }

            _ => None,
        }
    }
}
