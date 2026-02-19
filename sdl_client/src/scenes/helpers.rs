use std::{cell::RefCell, collections::HashMap};

use egui_sdl2::egui;
use mag_core::traits;

use crate::gfx_cache::GraphicsCache;

thread_local! {
    static CHARACTER_TEXTURES: RefCell<HashMap<usize, egui::TextureHandle>> = RefCell::new(HashMap::new());
}

/// Resolves an egui `TextureId` for a character portrait shown in the selection UI.
///
/// This selects a sprite based on the character's class/sex, then loads the sprite image from
/// the SDL graphics cache and registers it in egui.
pub fn texture_id_for_character(
    ctx: &egui::Context,
    gfx: &mut GraphicsCache,
    class: traits::Class,
    sex: traits::Sex,
) -> Option<egui::TextureId> {
    let sprite_id = sprite_id_for_selection(class, sex);

    CHARACTER_TEXTURES.with(|textures| {
        if let Some(handle) = textures.borrow().get(&sprite_id) {
            return Some(handle.id());
        }

        let image = gfx.get_rgba_image(sprite_id)?;
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([image.width, image.height], &image.pixels);

        let handle = ctx.load_texture(
            format!("character_portrait_{sprite_id}"),
            color_image,
            egui::TextureOptions::NEAREST,
        );
        let texture_id = handle.id();
        textures.borrow_mut().insert(sprite_id, handle);

        Some(texture_id)
    })
}

/// Maps a character class/sex pair to the sprite ID used in the character selection list.
///
/// This is a UI-only mapping (it does not affect server-side appearance). For any unsupported
/// combination, it falls back to the mercenary male sprite.
pub fn sprite_id_for_selection(class: traits::Class, sex: traits::Sex) -> usize {
    match (class, sex) {
        (traits::Class::Harakim, traits::Sex::Male) => 4048,
        (traits::Class::Templar, traits::Sex::Male) => 2000,
        (traits::Class::Mercenary, traits::Sex::Male) => 5072,
        (traits::Class::Harakim, traits::Sex::Female) => 6096,
        (traits::Class::Templar, traits::Sex::Female) => 8144,
        (traits::Class::Mercenary, traits::Sex::Female) => 7120,
        _ => 5072,
    }
}
