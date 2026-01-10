use bevy::prelude::*;
use bevy::window::WindowResolution;

mod assets_pipeline;
use assets_pipeline::{DatBlob, MagAssetSourcePlugin, MagCustomAssetsPlugin};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .build()
                .add_before::<bevy::asset::AssetPlugin, MagAssetSourcePlugin>(MagAssetSourcePlugin)
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Men Among Gods (Client)".to_string(),
                        resolution: WindowResolution::new(800.0, 600.0),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(MagCustomAssetsPlugin)
        .add_systems(Startup, setup)
        .run();
}

#[derive(Resource, Default)]
struct DemoHandles {
    _dat: Handle<DatBlob>,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());

    let texture: Handle<Image> = asset_server.load("GFX/17063.PNG");
    commands.spawn(SpriteBundle {
        texture,
        ..default()
    });

    let dat: Handle<DatBlob> = asset_server.load("mag://gx00.dat");
    commands.insert_resource(DemoHandles { _dat: dat });
}
