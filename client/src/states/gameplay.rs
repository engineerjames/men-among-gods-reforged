use bevy::prelude::*;

use crate::gfx_cache::GraphicsCache;

pub fn setup_gameplay(mut commands: Commands, gfx: Res<GraphicsCache>) {
    log::info!("Loading complete; entering gameplay");
    if let Some(sprite) = gfx.get_sprite(0) {
        commands.spawn(sprite.clone());
    } else {
        log::error!("No sprite found at index 0 in GraphicsCache");
    }
}
