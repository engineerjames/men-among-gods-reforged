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
    ui::widget::{KeyModifiers, MouseButton as UiMouseButton, UiEvent, Widget},
    ui::widgets::on_screen_keyboard::OnScreenKeyboardAction,
};

use super::{GameScene, net_events::UiHandleResult};

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
                                    OnScreenKeyboardAction::Submit => {
                                        self.chat_box.submit_input();
                                        self.process_chat_box_actions(app_state);
                                        self.keyboard.hide();
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

                // Skill picker popup intercept (modal for controller input).
                if self.skill_picker.is_visible() {
                    match button {
                        Btn::Back => {
                            self.skill_picker.controller_cancel();
                        }
                        Btn::RightStick => {
                            self.skill_picker.controller_confirm();
                            self.process_skill_picker_actions(app_state);
                        }
                        _ => {}
                    }
                    return None;
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

                // D-pad → panel toggles and in-panel navigation (controller mode)
                if self.controller_mode
                    && matches!(
                        button,
                        Btn::DPadUp | Btn::DPadDown | Btn::DPadLeft | Btn::DPadRight
                    )
                {
                    if self.shop_panel.is_visible() {
                        self.shop_panel.ensure_controller_selection();
                        match button {
                            Btn::DPadUp => self.shop_panel.controller_nav_up(),
                            Btn::DPadDown => self.shop_panel.controller_nav_down(),
                            Btn::DPadLeft => self.shop_panel.controller_nav_left(),
                            Btn::DPadRight => self.shop_panel.controller_nav_right(),
                            _ => {}
                        }
                    } else if self.inventory_panel.is_visible() {
                        match button {
                            Btn::DPadUp => self.inventory_panel.controller_nav_up(),
                            Btn::DPadDown => self.inventory_panel.controller_nav_down(),
                            Btn::DPadLeft => self.inventory_panel.controller_nav_left(),
                            Btn::DPadRight => self.inventory_panel.controller_nav_right(),
                            _ => {}
                        }
                    } else if self.skills_panel.is_visible() {
                        match button {
                            Btn::DPadUp => self.skills_panel.controller_nav_up(),
                            Btn::DPadDown => self.skills_panel.controller_nav_down(),
                            Btn::DPadLeft | Btn::DPadRight => {
                                self.skills_panel.controller_nav_left_right()
                            }
                            _ => {}
                        }
                    } else {
                        // No panel open → toggle inventory or skills
                        match button {
                            Btn::DPadUp => {
                                self.inventory_panel.toggle();
                                if self.inventory_panel.is_visible() {
                                    self.inventory_panel.ensure_controller_selection();
                                }
                            }
                            Btn::DPadDown => {
                                self.skills_panel.toggle();
                                if self.skills_panel.is_visible() {
                                    self.skills_panel.ensure_controller_focus();
                                }
                            }
                            _ => {}
                        }
                    }
                    return None;
                }

                // B button → close panels or clear skill bar selection
                if *button == Btn::B && self.controller_mode {
                    if self.shop_panel.is_visible() {
                        self.shop_panel.clear_controller_selection();
                        self.shop_panel.handle_event(&UiEvent::NavBack);
                        self.process_shop_panel_actions(app_state);
                        return None;
                    }
                    if self.inventory_panel.is_visible() {
                        self.inventory_panel.toggle();
                        return None;
                    }
                    if self.skills_panel.is_visible() {
                        self.skills_panel.toggle();
                        return None;
                    }
                    self.skill_bar.set_controller_selected_slot(None);
                    return None;
                }

                // Back (Select) button → clear the focused controller binding
                // if the controller bindings sub-panel is open, otherwise
                // clear the highlighted skill slot binding.
                if *button == Btn::Back && self.controller_mode {
                    if let Some(slot) = self.settings_panel.controller_focused_binding_slot() {
                        self.settings_panel.clear_controller_binding(slot);
                        self.process_settings_panel_actions(app_state);
                        return None;
                    }
                    if let Some(slot) = self.skill_bar.controller_selected_slot() {
                        if slot < app_state.settings.character.skill_keybinds.len() {
                            app_state.settings.character.skill_keybinds[slot] = None;
                            self.save_active_profile(app_state);
                        }
                    }
                    return None;
                }

                // Right stick press (R3) → activate highlighted skill or assign empty slot
                if *button == Btn::RightStick && self.controller_mode {
                    if let Some(slot) = self.skill_bar.controller_selected_slot() {
                        if let Some(skill_nr) = app_state.settings.character.skill_keybinds[slot] {
                            // Slot is bound → cast the skill.
                            if let (Some(net), Some(ps)) =
                                (app_state.network.as_ref(), app_state.player_state.as_ref())
                            {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    skill_nr as u32,
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        } else {
                            // Slot is empty → open skill picker to assign.
                            // TODO: Refactor this nonsense--controller input shouldn't be responsible
                            // for opening up UI widgets.
                            let bar = self.skill_bar.bounds();
                            let (cx, _cy) = crate::ui::hud::skill_bar::TOP_CELL_POSITIONS
                                .get(slot)
                                .copied()
                                .unwrap_or((0, 0));
                            let anchor_x = bar.x + cx;
                            let anchor_y =
                                bar.y + crate::ui::hud::skill_picker_popup::ANCHOR_Y_OFFSET;
                            let player_skills = app_state
                                .player_state
                                .as_ref()
                                .map(|ps| ps.character_info().skill.as_slice())
                                .unwrap_or(&[]);
                            self.skill_picker
                                .show(slot as u8, anchor_x, anchor_y, player_skills);
                        }
                    }
                    return None;
                }

                // Left stick press (L3) → start press timer for select/look
                if *button == Btn::LeftStick && self.controller_mode {
                    self.l3_pressed_at = Some(Instant::now());
                    return None;
                }

                // A button → controller-specific panel interaction
                // When inventory or skills is open with a focused slot, handle
                // the action directly rather than synthesizing a mouse click.
                if *button == Btn::A && self.controller_mode {
                    // Shop/grave panel: A = interact with selected slot
                    if self.shop_panel.is_visible() {
                        self.shop_panel.controller_activate();
                        self.process_shop_panel_actions(app_state);
                        return None;
                    }

                    // Inventory panel: A = interact with selected slot
                    if self.inventory_panel.is_visible() {
                        if let Some(slot) = self.inventory_panel.controller_selected() {
                            let shift = self.lb_held;
                            self.inventory_panel.controller_activate(slot, shift);
                            self.process_inventory_panel_actions(app_state);
                            return None;
                        }
                    }

                    // Skills panel: A = activate focused +/- or Update
                    if self.skills_panel.is_visible() {
                        if self.skills_panel.controller_focus().is_some() {
                            let shift = self.lb_held;
                            self.skills_panel.controller_activate(shift);
                            self.process_skills_panel_actions(app_state);
                            return None;
                        }
                    }
                }

                // A button → left-click equivalent (LB = shift, RB = ctrl)
                // Route through the UI widget stack first; only fall through
                // to the world click handler if no widget consumed the event.
                if *button == Btn::A && self.controller_mode {
                    let orig_shift = self.shift_held;
                    let orig_ctrl = self.ctrl_held;
                    self.shift_held = self.lb_held;
                    self.ctrl_held = self.rb_held;

                    let ui_event = UiEvent::MouseClick {
                        x: self.mouse_x,
                        y: self.mouse_y,
                        button: UiMouseButton::Left,
                        modifiers: KeyModifiers {
                            ctrl: self.ctrl_held,
                            shift: self.shift_held,
                            alt: self.alt_held,
                        },
                    };
                    match self.handle_ui_widget_events(app_state, &ui_event) {
                        UiHandleResult::SceneChange(sc) => {
                            self.shift_held = orig_shift;
                            self.ctrl_held = orig_ctrl;
                            return Some(sc);
                        }
                        UiHandleResult::Consumed => {}
                        UiHandleResult::NotConsumed => {
                            self.handle_world_click(
                                app_state,
                                MouseButton::Left,
                                self.mouse_x,
                                self.mouse_y,
                            );
                        }
                    }

                    self.shift_held = orig_shift;
                    self.ctrl_held = orig_ctrl;
                    return None;
                }

                // X button → right-click equivalent (LB = shift, RB = ctrl)
                // Route through the UI widget stack first; only fall through
                // to the world click handler if no widget consumed the event.
                if *button == Btn::X && self.controller_mode {
                    let orig_shift = self.shift_held;
                    let orig_ctrl = self.ctrl_held;
                    self.shift_held = self.lb_held;
                    self.ctrl_held = self.rb_held;

                    let ui_event = UiEvent::MouseClick {
                        x: self.mouse_x,
                        y: self.mouse_y,
                        button: UiMouseButton::Right,
                        modifiers: KeyModifiers {
                            ctrl: self.ctrl_held,
                            shift: self.shift_held,
                            alt: self.alt_held,
                        },
                    };
                    match self.handle_ui_widget_events(app_state, &ui_event) {
                        UiHandleResult::SceneChange(sc) => {
                            self.shift_held = orig_shift;
                            self.ctrl_held = orig_ctrl;
                            return Some(sc);
                        }
                        UiHandleResult::Consumed => {}
                        UiHandleResult::NotConsumed => {
                            self.handle_world_click(
                                app_state,
                                MouseButton::Right,
                                self.mouse_x,
                                self.mouse_y,
                            );
                        }
                    }

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

                if self.skill_picker.is_visible() {
                    return None;
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
                    Axis::RightY => self.right_stick_y = *value,
                    _ => {}
                }

                if self.skill_picker.is_visible() {
                    return None;
                }

                // When on-screen keyboard is visible, use left stick for 2D navigation.
                if self.keyboard.is_visible() {
                    const KB_DEADZONE: i16 = 16_000;
                    match axis {
                        Axis::LeftX => {
                            let v = *value;
                            if v > KB_DEADZONE && !self.kb_stick_pos_x {
                                self.kb_stick_pos_x = true;
                                self.keyboard.handle_event(&UiEvent::NavNext);
                            } else if v <= KB_DEADZONE {
                                self.kb_stick_pos_x = false;
                            }
                            if v < -KB_DEADZONE && !self.kb_stick_neg_x {
                                self.kb_stick_neg_x = true;
                                self.keyboard.handle_event(&UiEvent::NavPrev);
                            } else if v >= -KB_DEADZONE {
                                self.kb_stick_neg_x = false;
                            }
                        }
                        Axis::LeftY => {
                            let v = *value;
                            if v > KB_DEADZONE && !self.kb_stick_pos_y {
                                self.kb_stick_pos_y = true;
                                self.keyboard.handle_event(&UiEvent::KeyboardRowDown);
                            } else if v <= KB_DEADZONE {
                                self.kb_stick_pos_y = false;
                            }
                            if v < -KB_DEADZONE && !self.kb_stick_neg_y {
                                self.kb_stick_neg_y = true;
                                self.keyboard.handle_event(&UiEvent::KeyboardRowUp);
                            } else if v >= -KB_DEADZONE {
                                self.kb_stick_neg_y = false;
                            }
                        }
                        _ => {}
                    }
                    return None;
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
