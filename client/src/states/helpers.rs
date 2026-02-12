use bevy_egui::{egui, EguiContexts, EguiTextureHandle};
use mag_core::traits;

use crate::gfx_cache::GraphicsCache;

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
