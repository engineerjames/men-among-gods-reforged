use bevy_egui::{egui, EguiContexts, EguiTextureHandle};
use mag_core::traits;

use crate::gfx_cache::GraphicsCache;

/// Resolves an egui `TextureId` for a character portrait shown in the selection UI.
///
/// This selects a sprite based on the character's class/sex, then either reuses an existing egui
/// texture registration (via `EguiContexts::image_id`) or registers the image as a weak handle.
///
/// # Arguments
/// * `contexts` - The egui context wrapper used to register/query images.
/// * `gfx` - Sprite cache used to fetch the underlying image for the selected sprite.
/// * `class` - Character class.
/// * `sex` - Character sex.
///
/// # Returns
/// * `Some(TextureId)` if the sprite image is available.
/// * `None` if the sprite could not be resolved from the cache.
pub fn texture_id_for_character(
    contexts: &mut EguiContexts,
    gfx: &GraphicsCache,
    class: traits::Class,
    sex: traits::Sex,
) -> Option<egui::TextureId> {
    let sprite_id = sprite_id_for_selection(class, sex);
    let image = gfx
        .get_sprite(sprite_id)
        .map(|sprite| sprite.image.clone())?;
    let asset_id = image.id();
    let texture_id = contexts
        .image_id(asset_id)
        .unwrap_or_else(|| contexts.add_image(EguiTextureHandle::Weak(asset_id)));
    Some(texture_id)
}

/// Maps a character class/sex pair to the sprite ID used in the character selection list.
///
/// This is a UI-only mapping (it does not affect server-side appearance). For any unsupported
/// combination, it falls back to the mercenary male sprite.
///
/// # Arguments
/// * `class` - Character class.
/// * `sex` - Character sex.
///
/// # Returns
/// * Sprite ID in the game's sprite sheet.
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
