mod constants;
mod gfx_cache;
mod log;
mod sfx_cache;
mod systems;

use bevy::prelude::*;
use bevy::window::WindowResolution;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::log::Logger;
use crate::sfx_cache::SoundCache;

use crate::systems::debug::print_click_coords;
use crate::systems::display::enforce_aspect_and_pixel_coords;

fn main() {
    App::new()
        .insert_resource(GraphicsCache::new("assets/gfx/images.zip"))
        .insert_resource(SoundCache::new("assets/sfx/sounds.zip"))
        .insert_resource(Logger::default())
        .add_plugins(
            DefaultPlugins
                .build()
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Men Among Gods (Client)".to_string(),
                        resolution: WindowResolution::new(800, 600),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, setup)
        .add_systems(Update, print_click_coords)
        .add_systems(Update, enforce_aspect_and_pixel_coords)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Name::new("Camera"),
        Camera2d::default(),
        Projection::from(OrthographicProjection {
            // We can set the scaling mode to FixedVertical to keep the viewport height constant as its aspect ratio changes.
            // The viewport height is the height of the camera's view in world units when the scale is 1.
            scaling_mode: bevy::camera::ScalingMode::AutoMin {
                min_width: TARGET_WIDTH,
                min_height: TARGET_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));

    commands.spawn(Sprite::from_image(asset_server.load("gfx/00001.png")));
}
