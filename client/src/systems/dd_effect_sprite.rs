use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin};

#[repr(C)]
#[derive(Clone, Copy, Default, ShaderType)]
pub(crate) struct DdEffectSpriteParams {
    pub(crate) effect: u32,
    pub(crate) _pad0: Vec3,
}

#[derive(AsBindGroup, Asset, TypePath, Clone)]
pub(crate) struct DdEffectSpriteMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub(crate) image: Handle<Image>,

    #[uniform(2)]
    pub(crate) params: DdEffectSpriteParams,
}

impl Material2d for DdEffectSpriteMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/dd_effect_sprite.wgsl".into()
    }
}

#[derive(Resource, Clone)]
pub(crate) struct DdEffectUnitQuadMesh(pub(crate) Handle<Mesh>);

fn setup_dd_effect_unit_quad_mesh(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let quad = Mesh::from(bevy::math::primitives::Rectangle::new(1.0, 1.0));
    let quad_handle = meshes.add(quad);
    commands.insert_resource(DdEffectUnitQuadMesh(quad_handle));
}

pub(crate) struct DdEffectSpritePlugin;

impl Plugin for DdEffectSpritePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<DdEffectSpriteMaterial>::default())
            .add_systems(Startup, setup_dd_effect_unit_quad_mesh);
    }
}
