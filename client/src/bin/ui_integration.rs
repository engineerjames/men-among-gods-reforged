//! UI Integration Test — standalone widget gallery.
//!
//! Opens a 960×540 SDL2 window and displays one interactive instance of every
//! widget type.  This binary is a development tool for visually verifying
//! widget rendering and interaction without running the full game client.
//!
//! # Usage
//!
//! ```sh
//! cargo run -p client --bin ui-integration
//! ```
//!
//! # Keyboard shortcuts
//!
//! | Key   | Action                        |
//! |-------|-------------------------------|
//! | `1`   | Toggle Skills panel           |
//! | `2`   | Toggle Inventory panel        |
//! | `3`   | Toggle Settings panel         |
//! | `4`   | Toggle Shop panel             |
//! | `5`   | Toggle Look panel             |
//! | `Esc` | Quit                          |

use std::time::{Duration, Instant};

use sdl2::gfx::framerate::FPSManager;
use sdl2::image::InitFlag;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;

use client::constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT};
use client::filepaths;
use client::gfx_cache::GraphicsCache;
use client::types::log_message::{LogMessage, LogMessageColor};
use client::ui::button::{CircleButton, RectButton};
use client::ui::button_arc::HudButtonBar;
use client::ui::chat_box::ChatBox;
use client::ui::checkbox::Checkbox;
use client::ui::dropdown::Dropdown;
use client::ui::inventory_panel::{InventoryPanel, InventoryPanelData};
use client::ui::label::Label;
use client::ui::look_panel::LookPanel;
use client::ui::minimap_widget::MinimapWidget;
use client::ui::mode_button::ModeButton;
use client::ui::panel::Panel;
use client::ui::rank_arc::RankArc;
use client::ui::settings_panel::{SettingsPanel, SETTINGS_PANEL_H};
use client::ui::shop_panel::ShopPanel;
use client::ui::skills_panel::{SkillsPanel, SkillsPanelData};
use client::ui::slider::Slider;
use client::ui::style::{Background, Border, Padding};
use client::ui::widget::{Bounds, EventResponse, HudPanel, Widget, WidgetAction};
use client::ui::{sdl_to_ui_event, RenderContext};

// ---------------------------------------------------------------------------
// Layout constants — arranged as a gallery across the 960×540 viewport
// ---------------------------------------------------------------------------

/// Background colour shared by all HUD-style panels.
const PANEL_BG: Color = Color::RGBA(10, 10, 30, 180);

/// Dark canvas clear colour.
const CLEAR_COLOR: Color = Color::RGB(30, 30, 40);

// Column X offsets for the gallery grid.
const COL1_X: i32 = 10;
const COL2_X: i32 = 230;

// HUD arc parameters (mirrored from the game scene).
const HUD_ARC_CENTER_X: i32 = TARGET_WIDTH_INT as i32 / 2;
const HUD_ARC_CENTER_Y: i32 = TARGET_HEIGHT_INT as i32;
const HUD_ARC_RADIUS: u32 = 60;
const HUD_BUTTON_RADIUS: u32 = 16;
const HUD_SPRITE_IDS: [usize; 3] = [267, 128, 35];

const HUD_PANEL_W: u32 = 300;
const HUD_PANEL_H: u32 = 250;

/// Application entry point for the UI integration test binary.
///
/// Initialises SDL2 (video only), creates a 960×540 window, instantiates
/// every widget, and enters a 60 FPS event/render loop.
///
/// # Returns
///
/// `Ok(())` on clean exit, `Err(String)` on SDL2 initialisation failure.
fn main() -> Result<(), String> {
    // --- SDL2 initialisation (video only — no audio, no network) ----------
    let mut fps_manager = FPSManager::new();
    fps_manager.set_framerate(60)?;
    let sdl_context = sdl2::init()?;
    let _image_context = sdl2::image::init(InitFlag::PNG)?;

    let video = sdl_context.video()?;
    let window = video
        .window("UI Integration Test", TARGET_WIDTH_INT, TARGET_HEIGHT_INT)
        .position_centered()
        .allow_highdpi()
        .resizable()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())?;

    let _ = canvas.set_logical_size(TARGET_WIDTH_INT, TARGET_HEIGHT_INT);

    let mut event_pump = sdl_context.event_pump()?;
    let texture_creator = canvas.texture_creator();

    let mut gfx = GraphicsCache::new(filepaths::get_gfx_zipfile(), texture_creator);

    // --- Instantiate every widget ----------------------------------------

    // Column 1: Atomic widgets
    let mut label = Label::new("Label Widget", 0, COL1_X, 10);

    let mut rect_button = RectButton::new(
        Bounds::new(COL1_X, 30, 200, 28),
        Background::SolidColor(Color::RGB(60, 60, 90)),
    )
    .with_border(Border {
        color: Color::RGB(120, 120, 180),
        width: 1,
    })
    .with_label("RectButton", 0);

    let mut circle_button = CircleButton::new(COL1_X + 30, 90, 20, Color::RGB(80, 40, 40))
        .with_border_color(Color::RGB(200, 100, 100));

    let mut checkbox = Checkbox::new(Bounds::new(COL1_X, 125, 200, 20), "Checkbox toggle", 0);

    let mut slider = Slider::new(
        Bounds::new(COL1_X, 155, 200, 24),
        "Volume",
        0.0,
        100.0,
        50.0,
        0,
    );

    let mut dropdown = Dropdown::new(
        Bounds::new(COL1_X, 190, 200, 20),
        vec![
            "Option A".into(),
            "Option B".into(),
            "Option C".into(),
            "Option D".into(),
        ],
        0,
        0,
    );

    let mut rank_arc = RankArc::new(COL1_X + 100, 280, 30, 2);
    rank_arc.set_progress(0.65);

    let mut demo_panel = Panel::new(Bounds::new(COL1_X, 320, 200, 60))
        .with_background(Background::SolidColor(Color::RGBA(40, 40, 70, 200)))
        .with_border(Border {
            color: Color::RGB(100, 100, 140),
            width: 1,
        })
        .with_padding(Padding::uniform(6));
    demo_panel.add_child(Box::new(Label::new("Inside a Panel", 0, 0, 0)));

    // Column 2: Stateful/interactive widgets
    let mut chat_box = ChatBox::new(
        Bounds::new(COL2_X, 10, 230, 200),
        Color::RGBA(10, 10, 30, 200),
        Padding::uniform(4),
    );
    // Pre-populate with sample messages.
    chat_box.push_message(LogMessage {
        message: "Welcome to the UI test!".into(),
        color: LogMessageColor::Green,
    });
    chat_box.push_message(LogMessage {
        message: "Type here and press Enter.".into(),
        color: LogMessageColor::Yellow,
    });
    chat_box.push_message(LogMessage {
        message: "An error-styled message.".into(),
        color: LogMessageColor::Red,
    });
    chat_box.push_message(LogMessage {
        message: "A blue informational note.".into(),
        color: LogMessageColor::Blue,
    });

    let mut mode_button = ModeButton::new(COL2_X + 30, 250, 18);

    let mut minimap_widget = MinimapWidget::new(TARGET_WIDTH_INT as i32 - 30, 30, 14);

    // HUD button bar (bottom centre — same position as in-game).
    let mut hud_buttons = HudButtonBar::new(
        HUD_ARC_CENTER_X,
        HUD_ARC_CENTER_Y,
        HUD_ARC_RADIUS,
        HUD_BUTTON_RADIUS,
        HUD_SPRITE_IDS,
    );

    // Column 3 / overlays: Complex panels (toggled via keys 1-5).
    let panel_bottom = HUD_ARC_CENTER_Y - HUD_ARC_RADIUS as i32 - HUD_BUTTON_RADIUS as i32 - 20;
    let panel_x = HUD_ARC_CENTER_X - HUD_PANEL_W as i32 / 2;
    let panel_y = panel_bottom - HUD_PANEL_H as i32;

    let mut status_panel = client::ui::status_panel::StatusPanel::new(COL2_X, 230, PANEL_BG);

    let mut skills_panel = SkillsPanel::new(
        Bounds::new(panel_x, panel_y, HUD_PANEL_W, HUD_PANEL_H),
        PANEL_BG,
    );
    skills_panel.update_data(SkillsPanelData {
        attrib: [[0; 6]; 5],
        hp: [0; 6],
        end: [0; 6],
        mana: [0; 6],
        skill: [[0; 6]; 100],
        points: 42,
        sorted_skills: Vec::new(),
        keybinds: [None; 9],
    });

    let mut inventory_panel = InventoryPanel::new(
        Bounds::new(HUD_ARC_CENTER_X - 95, panel_bottom - 280, 190, 280),
        PANEL_BG,
    );
    inventory_panel.update_data(InventoryPanelData {
        items: [0; 40],
        items_p: [0; 40],
        worn: [0; 20],
        worn_p: [0; 20],
        citem: 0,
        citem_p: 0,
        gold: 12345,
        selected_char: 0,
    });

    let mut settings_panel = SettingsPanel::new(
        Bounds::new(
            HUD_ARC_CENTER_X - HUD_PANEL_W as i32 / 2,
            panel_bottom - SETTINGS_PANEL_H as i32,
            HUD_PANEL_W,
            SETTINGS_PANEL_H,
        ),
        PANEL_BG,
    );

    let look_panel_w: u32 = 180;
    let look_panel_h: u32 = 260;
    let look_panel_x = TARGET_WIDTH_INT as i32 - look_panel_w as i32 - 4;
    let look_panel_y = (TARGET_HEIGHT_INT as i32 - look_panel_h as i32) / 4;
    let mut look_panel = LookPanel::new(
        Bounds::new(look_panel_x, look_panel_y, look_panel_w, look_panel_h),
        PANEL_BG,
    );

    let shop_panel_w = client::ui::shop_panel::SHOP_PANEL_W;
    let shop_panel_h = client::ui::shop_panel::SHOP_PANEL_H;
    let shop_panel_x = (TARGET_WIDTH_INT as i32 - shop_panel_w as i32) / 2;
    let shop_panel_y = (TARGET_HEIGHT_INT as i32 - shop_panel_h as i32) / 2;
    let mut shop_panel = ShopPanel::new(
        Bounds::new(shop_panel_x, shop_panel_y, shop_panel_w, shop_panel_h),
        PANEL_BG,
    );

    // Track modifier state for UiEvent generation.
    let mut ctrl_held = false;
    let mut shift_held = false;
    let mut alt_held = false;
    let mut mouse_x: i32 = 0;
    let mut mouse_y: i32 = 0;

    let mut last_frame = Instant::now();

    println!("UI Integration Test running — press 1-5 to toggle panels, Esc to quit.");

    // --- Main loop -------------------------------------------------------
    'running: loop {
        let now = Instant::now();
        let dt = now.duration_since(last_frame);
        last_frame = now;

        // 1. Poll SDL2 events.
        for event in event_pump.poll_iter() {
            match &event {
                sdl2::event::Event::Quit { .. } => break 'running,

                // Track modifier keys.
                sdl2::event::Event::KeyDown {
                    keycode: Some(kc), ..
                } => {
                    match *kc {
                        Keycode::LCtrl | Keycode::RCtrl => ctrl_held = true,
                        Keycode::LShift | Keycode::RShift => shift_held = true,
                        Keycode::LAlt | Keycode::RAlt => alt_held = true,
                        Keycode::Escape => break 'running,
                        // Toggle overlay panels.
                        Keycode::Num1 => {
                            skills_panel.toggle();
                            println!("[Key] SkillsPanel toggled");
                        }
                        Keycode::Num2 => {
                            inventory_panel.toggle();
                            println!("[Key] InventoryPanel toggled");
                        }
                        Keycode::Num3 => {
                            settings_panel.toggle();
                            println!("[Key] SettingsPanel toggled");
                        }
                        Keycode::Num4 => {
                            shop_panel.toggle();
                            println!("[Key] ShopPanel toggled");
                        }
                        Keycode::Num5 => {
                            look_panel.toggle();
                            println!("[Key] LookPanel toggled");
                        }
                        _ => {}
                    }
                }
                sdl2::event::Event::KeyUp {
                    keycode: Some(kc), ..
                } => match *kc {
                    Keycode::LCtrl | Keycode::RCtrl => ctrl_held = false,
                    Keycode::LShift | Keycode::RShift => shift_held = false,
                    Keycode::LAlt | Keycode::RAlt => alt_held = false,
                    _ => {}
                },

                // Track mouse position.
                sdl2::event::Event::MouseMotion { x, y, .. } => {
                    mouse_x = *x;
                    mouse_y = *y;
                }
                _ => {}
            }

            // Convert to UiEvent and dispatch to all widgets.
            let modifiers = client::ui::widget::KeyModifiers {
                ctrl: ctrl_held,
                shift: shift_held,
                alt: alt_held,
            };
            if let Some(ui_event) = sdl_to_ui_event(&event, mouse_x, mouse_y, modifiers) {
                // Dispatch to overlay panels first (topmost), then base widgets.
                let widgets: Vec<&mut dyn Widget> = vec![
                    &mut shop_panel,
                    &mut look_panel,
                    &mut settings_panel,
                    &mut inventory_panel,
                    &mut skills_panel,
                    &mut dropdown,
                    &mut chat_box,
                    &mut hud_buttons,
                    &mut minimap_widget,
                    &mut mode_button,
                    &mut slider,
                    &mut checkbox,
                    &mut circle_button,
                    &mut rect_button,
                    &mut demo_panel,
                    &mut status_panel,
                    &mut rank_arc,
                    &mut label,
                ];

                for w in widgets {
                    if w.handle_event(&ui_event) == EventResponse::Consumed {
                        break;
                    }
                }
            }
        }

        // 2. Drain and log widget actions.
        drain_and_log(&mut rect_button, "RectButton");
        drain_and_log(&mut circle_button, "CircleButton");
        drain_and_log(&mut checkbox, "Checkbox");
        drain_and_log(&mut slider, "Slider");
        drain_and_log(&mut dropdown, "Dropdown");
        drain_and_log(&mut demo_panel, "Panel");
        drain_and_log(&mut chat_box, "ChatBox");
        drain_and_log(&mut mode_button, "ModeButton");
        drain_and_log(&mut minimap_widget, "MinimapWidget");
        drain_and_log(&mut status_panel, "StatusPanel");
        drain_and_log(&mut rank_arc, "RankArc");

        // HudButtonBar actions → toggle panels.
        for action in hud_buttons.take_actions() {
            println!("[Action:HudButtonBar] {:?}", action);
            if let WidgetAction::TogglePanel(panel) = action {
                match panel {
                    HudPanel::Skills => skills_panel.toggle(),
                    HudPanel::Inventory => inventory_panel.toggle(),
                    HudPanel::Settings => settings_panel.toggle(),
                    HudPanel::Minimap => minimap_widget.toggle(),
                }
            }
        }

        drain_and_log(&mut skills_panel, "SkillsPanel");
        drain_and_log(&mut inventory_panel, "InventoryPanel");
        drain_and_log(&mut settings_panel, "SettingsPanel");
        drain_and_log(&mut look_panel, "LookPanel");
        drain_and_log(&mut shop_panel, "ShopPanel");

        // 3. Update time-driven state.
        update_all(
            dt,
            &mut label,
            &mut rect_button,
            &mut circle_button,
            &mut checkbox,
            &mut slider,
            &mut dropdown,
            &mut rank_arc,
            &mut demo_panel,
            &mut chat_box,
            &mut mode_button,
            &mut minimap_widget,
            &mut hud_buttons,
            &mut status_panel,
            &mut skills_panel,
            &mut inventory_panel,
            &mut settings_panel,
            &mut look_panel,
            &mut shop_panel,
        );

        // 4. Render.
        canvas.set_draw_color(CLEAR_COLOR);
        canvas.clear();

        let mut ctx = RenderContext {
            canvas: &mut canvas,
            gfx: &mut gfx,
        };

        // Render base-layer widgets.
        let _ = label.render(&mut ctx);
        let _ = rect_button.render(&mut ctx);
        let _ = circle_button.render(&mut ctx);
        let _ = checkbox.render(&mut ctx);
        let _ = slider.render(&mut ctx);
        let _ = dropdown.render(&mut ctx);
        let _ = rank_arc.render(&mut ctx);
        let _ = demo_panel.render(&mut ctx);
        let _ = chat_box.render(&mut ctx);
        let _ = mode_button.render(&mut ctx);
        let _ = minimap_widget.render(&mut ctx);
        let _ = status_panel.render(&mut ctx);
        let _ = hud_buttons.render(&mut ctx);

        // Render overlay panels (order matches visual stacking).
        let _ = skills_panel.render(&mut ctx);
        let _ = inventory_panel.render(&mut ctx);
        let _ = settings_panel.render(&mut ctx);
        let _ = look_panel.render(&mut ctx);
        let _ = shop_panel.render(&mut ctx);

        ctx.canvas.present();

        fps_manager.delay();
    }

    println!("UI Integration Test exiting.");
    Ok(())
}

/// Drains pending [`WidgetAction`]s from a widget and prints each to stdout.
///
/// # Arguments
///
/// * `widget` - The widget to drain actions from.
/// * `name` - A display name for logging which widget produced the action.
fn drain_and_log(widget: &mut dyn Widget, name: &str) {
    for action in widget.take_actions() {
        println!("[Action:{name}] {action:?}");
    }
}

/// Calls `update(dt)` on all widgets.
///
/// # Arguments
///
/// * `dt` - Duration since the last frame.
/// * Remaining parameters are mutable references to each widget instance.
#[allow(clippy::too_many_arguments)]
fn update_all(
    dt: Duration,
    label: &mut Label,
    rect_button: &mut RectButton,
    circle_button: &mut CircleButton,
    checkbox: &mut Checkbox,
    slider: &mut Slider,
    dropdown: &mut Dropdown,
    rank_arc: &mut RankArc,
    demo_panel: &mut Panel,
    chat_box: &mut ChatBox,
    mode_button: &mut ModeButton,
    minimap_widget: &mut MinimapWidget,
    hud_buttons: &mut HudButtonBar,
    status_panel: &mut client::ui::status_panel::StatusPanel,
    skills_panel: &mut SkillsPanel,
    inventory_panel: &mut InventoryPanel,
    settings_panel: &mut SettingsPanel,
    look_panel: &mut LookPanel,
    shop_panel: &mut ShopPanel,
) {
    label.update(dt);
    rect_button.update(dt);
    circle_button.update(dt);
    checkbox.update(dt);
    slider.update(dt);
    dropdown.update(dt);
    rank_arc.update(dt);
    demo_panel.update(dt);
    chat_box.update(dt);
    mode_button.update(dt);
    minimap_widget.update(dt);
    hud_buttons.update(dt);
    status_panel.update(dt);
    skills_panel.update(dt);
    inventory_panel.update(dt);
    settings_panel.update(dt);
    look_panel.update(dt);
    shop_panel.update(dt);
}
