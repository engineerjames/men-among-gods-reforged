// Bitmap text + chat/input UI systems live here.

use bevy::ecs::message::MessageReader;
use bevy::ecs::query::Without;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::network::client_commands::ClientCommand;
use crate::network::NetworkRuntime;
use crate::player_state::PlayerState;
use crate::states::gameplay::components::{
    BitmapText, GameplayRenderEntity, GameplayUiInputText, GameplayUiLogLine,
};
use crate::states::gameplay::layout::*;
use crate::states::gameplay::resources::GameplayTextInput;

use super::super::world_render::screen_to_world;

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
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    mut input: ResMut<GameplayTextInput>,
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
            player_state.tlog(1, format!("> {line}"));

            input.history.push(line.clone());
            input.history_pos = None;
        }
        input.current.clear();
    }

    // Update log text (22 lines), oldest at top like `engine.c`.
    for (line, mut text) in &mut q_log {
        let idx_from_most_recent = LOG_LINES.saturating_sub(1).saturating_sub(line.line);
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
