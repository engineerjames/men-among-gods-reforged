// Bitmap text + chat/input UI systems live here.

use bevy::ecs::message::MessageReader;
use bevy::ecs::query::Without;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::network::client_commands::ClientCommand;
use crate::network::NetworkRuntime;
use crate::player_state::PlayerState;
use crate::states::gameplay::components::{
    BitmapText, GameplayRenderEntity, GameplayUiInputText, GameplayUiLogLine,
};
use crate::states::gameplay::layout::*;
use crate::states::gameplay::resources::{GameplayLogScrollState, GameplayTextInput};
use crate::systems::magic_postprocess::MagicScreenCamera;

use super::super::world_render::screen_to_world;
use super::super::{cursor_game_pos, in_rect};

const BACKSPACE_REPEAT_DELAY_SECS: f32 = 0.5;
const BACKSPACE_REPEAT_INTERVAL_SECS: f32 = 0.05;

#[derive(Default)]
pub(crate) struct BackspaceRepeatState {
    hold_time: f32,
    repeat_time: f32,
}

/// Sends a chat input line to the server using the legacy 8x15-byte packet split.
fn send_chat_input(net: &NetworkRuntime, text: &str) {
    // Original client sends 8 packets of 15 bytes each (total 120).
    // We zero-pad and ensure a NUL terminator is present after the provided text.
    let mut buf = [0u8; 120];
    let bytes = text.as_bytes();
    let n = bytes.len().min(buf.len().saturating_sub(1));
    buf[..n].copy_from_slice(&bytes[..n]);
    buf[n] = 0;

    for cmd in ClientCommand::new_say_packets(&buf) {
        net.send(cmd.to_bytes());
    }
}

/// Spawns the gameplay log text lines (bitmap text entities).
pub(crate) fn spawn_ui_log_text(commands: &mut Commands) {
    for line in 0..LOG_LINES {
        let sx = LOG_X;
        let sy = LOG_Y + (line as f32) * LOG_LINE_H;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiLogLine { line },
            BitmapText {
                text: String::new(),
                color: Color::WHITE,
                font: UI_BITMAP_FONT,
            },
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_TEXT)),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Spawns the gameplay input text line (bitmap text entity).
pub(crate) fn spawn_ui_input_text(commands: &mut Commands) {
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiInputText,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(INPUT_X, INPUT_Y, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

/// Handles gameplay chat input and updates the on-screen log/input text.
pub(crate) fn run_gameplay_text_ui(
    keys: Res<ButtonInput<KeyCode>>,
    mut kb: MessageReader<KeyboardInput>,
    mut wheel: MessageReader<MouseWheel>,
    net: Res<NetworkRuntime>,
    player_state: Res<PlayerState>,
    mut input: ResMut<GameplayTextInput>,
    mut log_scroll: ResMut<GameplayLogScrollState>,
    time: Res<Time>,
    mut backspace_repeat: Local<BackspaceRepeatState>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<&Camera, (With<Camera2d>, With<MagicScreenCamera>)>,
    mut q_log: Query<(&GameplayUiLogLine, &mut BitmapText), Without<GameplayUiInputText>>,
    mut q_input: Query<&mut BitmapText, (With<GameplayUiInputText>, Without<GameplayUiLogLine>)>,
) {
    fn bitmap_font_for_log_color(color: crate::types::log_message::LogMessageColor) -> usize {
        match color {
            crate::types::log_message::LogMessageColor::Red => 0,
            crate::types::log_message::LogMessageColor::Yellow => 1,
            crate::types::log_message::LogMessageColor::Green => 2,
            crate::types::log_message::LogMessageColor::Blue => 3,
        }
    }

    // Basic text input. We'll treat gameplay as always having "focus" for now.
    for ev in kb.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        let Some(text) = ev.text.as_deref() else {
            continue;
        };

        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            // Keep it conservative like the original (ASCII-ish). This avoids weird IME edge cases.
            if ch as u32 >= 32 && ch as u32 <= 126 {
                if input.current.len() < 120 {
                    input.current.push(ch);
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        input.current.pop();
        backspace_repeat.hold_time = 0.0;
        backspace_repeat.repeat_time = 0.0;
    }

    if keys.just_released(KeyCode::Backspace) {
        backspace_repeat.hold_time = 0.0;
        backspace_repeat.repeat_time = 0.0;
    }

    if keys.pressed(KeyCode::Backspace) {
        backspace_repeat.hold_time += time.delta().as_secs_f32();

        if backspace_repeat.hold_time >= BACKSPACE_REPEAT_DELAY_SECS {
            backspace_repeat.repeat_time += time.delta().as_secs_f32();
            while backspace_repeat.repeat_time >= BACKSPACE_REPEAT_INTERVAL_SECS {
                input.current.pop();
                backspace_repeat.repeat_time -= BACKSPACE_REPEAT_INTERVAL_SECS;
            }
        }
    }

    if keys.just_pressed(KeyCode::ArrowUp) && !input.history.is_empty() {
        let next = match input.history_pos {
            None => input.history.len().saturating_sub(1),
            Some(pos) => pos.saturating_sub(1),
        };
        input.history_pos = Some(next);
        input.current = input.history[next].clone();
    }

    if keys.just_pressed(KeyCode::ArrowDown) && !input.history.is_empty() {
        match input.history_pos {
            None => {}
            Some(pos) => {
                let next = (pos + 1).min(input.history.len());
                if next >= input.history.len() {
                    input.history_pos = None;
                    input.current.clear();
                } else {
                    input.history_pos = Some(next);
                    input.current = input.history[next].clone();
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::Enter) {
        let line = input.current.trim().to_string();
        if !line.is_empty() {
            send_chat_input(&net, &line);
            input.history.push(line.clone());
            input.history_pos = None;
        }
        input.current.clear();
    }

    // Log scrolling.
    //
    // - `offset == 0` means show the newest LOG_LINES messages.
    // - When scrolled up (offset > 0), keep the viewport stable while new messages arrive.
    let log_len = player_state.log_len();
    let max_offset = log_len.saturating_sub(LOG_LINES);

    let page = LOG_LINES.max(1);
    if keys.just_pressed(KeyCode::PageUp) {
        log_scroll.offset = log_scroll.offset.saturating_add(page);
    }
    if keys.just_pressed(KeyCode::PageDown) {
        log_scroll.offset = log_scroll.offset.saturating_sub(page);
    }

    let over_log = cursor_game_pos(&windows, &cameras).is_some_and(|game| {
        let log_w = crate::constants::TARGET_WIDTH - LOG_X;
        let log_h = LOG_LINE_H * (LOG_LINES as f32);
        in_rect(game, LOG_X, LOG_Y, log_w, log_h)
    });

    if over_log {
        for ev in wheel.read() {
            // Bevy: positive y is typically "scroll up".
            let y = match ev.unit {
                MouseScrollUnit::Line => ev.y,
                MouseScrollUnit::Pixel => ev.y / 20.0,
            };

            let ticks = y.round() as i32;
            if ticks == 0 {
                continue;
            }

            // Make wheel movement feel more like a log scroller than a precision slider.
            let lines = ticks.saturating_mul(3);
            if lines > 0 {
                log_scroll.offset = log_scroll.offset.saturating_add(lines as usize);
            } else {
                log_scroll.offset = log_scroll.offset.saturating_sub((-lines) as usize);
            }
        }
    }

    // If we're scrolled up and new log lines have arrived since last frame, increase the
    // offset by the same amount so the viewed content stays put.
    let rev_now = player_state.log_revision();
    let rev_delta = rev_now.saturating_sub(log_scroll.last_log_revision) as usize;
    if rev_delta > 0 && log_scroll.offset > 0 {
        log_scroll.offset = log_scroll.offset.saturating_add(rev_delta);
    }
    log_scroll.last_log_revision = rev_now;

    // Clamp after applying inputs + revision adjustment.
    log_scroll.offset = log_scroll.offset.min(max_offset);

    // Update log text (22 lines), oldest at top like `engine.c`.
    for (line, mut text) in &mut q_log {
        let idx_from_most_recent = log_scroll
            .offset
            .saturating_add(LOG_LINES.saturating_sub(1).saturating_sub(line.line));
        if let Some(msg) = player_state.log_message(idx_from_most_recent) {
            let desired_font = bitmap_font_for_log_color(msg.color);
            if text.font != desired_font {
                text.font = desired_font;
            }
            if text.text != msg.message {
                text.text.clear();
                text.text.push_str(&msg.message);
            }
        } else {
            if !text.text.is_empty() {
                text.text.clear();
            }
            if text.font != UI_BITMAP_FONT {
                text.font = UI_BITMAP_FONT;
            }
        }
    }

    // Update input line text. Clamp to the last 48 characters like the original viewport.
    if let Some(mut text) = q_input.iter_mut().next() {
        let current = input.current.as_str();
        let view = if current.len() > 48 {
            &current[current.len() - 48..]
        } else {
            current
        };

        let matches = text
            .text
            .strip_suffix('|')
            .is_some_and(|prefix| prefix == view);
        if !matches {
            text.text.clear();
            text.text.push_str(view);
            text.text.push('|');
        }
        if text.font != UI_BITMAP_FONT {
            text.font = UI_BITMAP_FONT;
        }
    }
}
