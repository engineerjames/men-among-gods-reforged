use bevy::prelude::*;

use crate::gfx_cache::GraphicsCache;

pub fn setup_gameplay(mut commands: Commands, gfx: Res<GraphicsCache>) {
    log::debug!("setup_gameplay - start");
    if let Some(sprite) = gfx.get_sprite(0) {
        commands.spawn(sprite.clone());
    } else {
        log::error!("No sprite found at index 0 in GraphicsCache");
    }
    log::debug!("setup_gameplay - end");
}

pub fn run_gameplay() {
    // Gameplay logic would go here
}
