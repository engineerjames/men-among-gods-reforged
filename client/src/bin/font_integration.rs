//! Font integration test binary.
//!
//! Renders a sample sentence with the legacy bitmap font (sheet 0) and
//! with whichever TrueType face is currently selected. The active TTF
//! cycles through every font auto-discovered from
//! `client/assets/fonts/` using the left/right arrow keys, and the active
//! font's filename stem is shown at the bottom of the window. Press
//! `Escape` (or close the window) to quit.

use sdl2::event::Event;
use sdl2::gfx::framerate::FPSManager;
use sdl2::image::InitFlag;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;

use client::constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT};
use client::filepaths;
use client::font_cache::{self, TextEngine, TextStyle};
use client::gfx_cache::GraphicsCache;

/// Sentence rendered with every font.
const SAMPLE: &str = "The quick brown fox jumps over the lazy dog. 0123456789 ?!@#";

/// TTF sample sizes (in logical points) rendered top-to-bottom.
const TTF_SIZES: &[u16] = &[6, 8, 10];

/// Application entry point for the font integration test binary.
///
/// # Returns
///
/// `Ok(())` on clean exit; `Err(String)` on SDL2 initialisation failure.
fn main() -> Result<(), String> {
    let mut fps_manager = FPSManager::new();
    fps_manager.set_framerate(60)?;
    let sdl_context = sdl2::init()?;
    let _image_context = sdl2::image::init(InitFlag::PNG)?;

    let video = sdl_context.video()?;
    let window = video
        .window("Font Integration Test", TARGET_WIDTH_INT, TARGET_HEIGHT_INT)
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

    let mut gfx = GraphicsCache::new(filepaths::get_gfx_zipfile(), &texture_creator);

    let ttf_ctx_static: &'static sdl2::ttf::Sdl2TtfContext =
        Box::leak(Box::new(sdl2::ttf::init().map_err(|e| e.to_string())?));
    let mut text_engine = TextEngine::new(
        ttf_ctx_static,
        &texture_creator,
        filepaths::get_fonts_directory(),
        1.0,
    );

    let stems: Vec<String> = text_engine.ttf_stems().to_vec();
    if stems.is_empty() {
        return Err("font-integration: no TTF fonts discovered in fonts directory".to_owned());
    }
    let mut selected: usize = 0;

    let bg = Color::RGB(20, 20, 30);
    let label_color = Color::RGB(180, 200, 255);
    let body_color = Color::RGB(230, 230, 230);
    let footer_color = Color::RGB(255, 215, 120);

    'main_loop: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'main_loop,
                Event::KeyDown {
                    keycode: Some(Keycode::Right),
                    ..
                } => {
                    selected = (selected + 1) % stems.len();
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Left),
                    ..
                } => {
                    selected = (selected + stems.len() - 1) % stems.len();
                }
                _ => {}
            }
        }

        canvas.set_draw_color(bg);
        canvas.clear();

        // --- Bitmap font (sheet 0) ----------------------------------------
        let mut y = 14;
        font_cache::draw_text(
            &mut canvas,
            &mut gfx,
            0,
            "Bitmap font 0",
            10,
            y,
            TextStyle::tinted(label_color),
        )?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 2;
        font_cache::draw_text(
            &mut canvas,
            &mut gfx,
            0,
            SAMPLE,
            10,
            y,
            TextStyle::tinted(body_color),
        )?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 14;

        // --- Selected TrueType font at several sizes ----------------------
        let stem = &stems[selected];
        let header = format!("TrueType: {} (<-- / --> to cycle)", stem);
        font_cache::draw_text(
            &mut canvas,
            &mut gfx,
            0,
            &header,
            10,
            y,
            TextStyle::tinted(label_color),
        )?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 6;

        for &size_pt in TTF_SIZES {
            let handle = text_engine.handle(stem, size_pt);
            let line_h = font_cache::line_height(&mut text_engine, &handle) as i32;
            font_cache::draw_text_handle(
                &mut canvas,
                &mut text_engine,
                &mut gfx,
                &handle,
                &format!("{size_pt}pt — {SAMPLE}"),
                10,
                y,
                TextStyle::tinted(body_color),
            )?;
            y += line_h + 4;
        }

        // --- Footer: active font name in large TTF ------------------------
        let footer_handle = text_engine.handle(stem, 32);
        let footer_h = font_cache::line_height(&mut text_engine, &footer_handle) as i32;
        let footer_y = TARGET_HEIGHT_INT as i32 - footer_h - 12;
        font_cache::draw_text_handle(
            &mut canvas,
            &mut text_engine,
            &mut gfx,
            &footer_handle,
            stem,
            TARGET_WIDTH_INT as i32 / 2,
            footer_y,
            TextStyle {
                centered: true,
                drop_shadow: true,
                ..TextStyle::tinted(footer_color)
            },
        )?;

        canvas.present();
        fps_manager.delay();
    }

    Ok(())
}
