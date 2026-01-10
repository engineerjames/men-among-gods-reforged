use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::window::WindowResolution;
mod assets_pipeline;
use assets_pipeline::{DatBlob, MagAssetSourcePlugin, MagCustomAssetsPlugin};
use bevy::sprite_render::{Material2d, Material2dPlugin};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .build()
                .add_before::<bevy::asset::AssetPlugin>(MagAssetSourcePlugin)
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
        .add_plugins(Material2dPlugin::<ColorKeyMaterial>::default())
        .add_plugins(MagCustomAssetsPlugin)
        .add_systems(Startup, setup)
        .run();
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct ColorKeyMaterial {
    #[uniform(0)]
    color_keys: [Vec3; 9],
    #[texture(1)]
    #[sampler(2)]
    texture: Handle<Image>,
}

impl Material2d for ColorKeyMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/color_key.wgsl".into()
    }
}

#[derive(Resource, Default)]
struct DemoHandles {
    _dat: Handle<DatBlob>,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorKeyMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn(Camera2d);

    let material = materials.add(ColorKeyMaterial {
        texture: asset_server.load("GFX/17063.PNG"),
        color_keys: [
            Vec3::new(255.0, 0.0, 255.0), // 0xFF00FF
            Vec3::new(254.0, 0.0, 254.0), // 0xFE00FE
            Vec3::new(253.0, 0.0, 253.0), // 0xFD00FD
            Vec3::new(252.0, 0.0, 252.0), // 0xFC00FC
            Vec3::new(251.0, 0.0, 251.0), // 0xFB00FB
            Vec3::new(250.0, 0.0, 250.0), // 0xFA00FA
            Vec3::new(249.0, 0.0, 249.0), // 0xF900F9
            Vec3::new(248.0, 0.0, 248.0), // 0xF800F8
            Vec3::new(247.0, 0.0, 247.0), // 0xF700F7
        ],
    });

    commands.spawn((
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(material),
        Transform::default().with_scale(Vec3::splat(128.)),
    ));

    let dat: Handle<DatBlob> = asset_server.load("mag://gx00.dat");
    commands.insert_resource(DemoHandles { _dat: dat });
}
