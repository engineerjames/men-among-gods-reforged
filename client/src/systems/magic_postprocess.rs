use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType, TextureFormat};
use bevy::render::storage::ShaderStorageBuffer;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin};
use bevy::transform::TransformSystems;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::player_state::PlayerState;
use crate::states::gameplay::GameplayRenderEntity;

use mag_core::constants::{CMAGIC, EMAGIC, GMAGIC, INVIS, TILEX, TILEY, XPOS, YPOS};

pub(crate) const WORLD_LAYER: usize = 0;
pub(crate) const UI_LAYER: usize = 1;

// Duplicated from gameplay layout (kept private there).
const MAP_X_SHIFT: f32 = -176.0;
const Z_UI: f32 = 900.0;

/// Keep this relatively small: it’s a per-fragment loop.
const MAX_MAGIC_SOURCES: usize = 128;

#[derive(Component)]
pub(crate) struct MagicWorldCamera;

#[derive(Component)]
pub(crate) struct MagicScreenCamera;

#[derive(Resource, Debug, Clone)]
pub(crate) struct MagicPostProcessSettings {
    pub enabled: bool,
    /// 1.0 = no change. Applied as exponent 1/gamma.
    pub gamma: f32,
}

impl Default for MagicPostProcessSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            gamma: 1.0,
        }
    }
}

#[derive(Resource, Clone)]
pub(crate) struct MagicPostProcessMaterialHandle(pub(crate) Handle<MagicPostProcessMaterial>);

#[derive(Resource, Clone)]
pub(crate) struct MagicSourcesBuffer(pub(crate) Handle<ShaderStorageBuffer>);

#[derive(Resource, Default, Clone)]
pub(crate) struct MagicSources {
    pub(crate) sources: Vec<MagicSourceGpu>,
}

#[repr(C)]
#[derive(Clone, Copy, Default, ShaderType)]
pub(crate) struct MagicPostProcessParams {
    pub(crate) screen_size: Vec2,
    pub(crate) source_count: u32,
    pub(crate) magic_enabled: u32,
    pub(crate) gamma: f32,
    pub(crate) _pad0: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, Default, ShaderType)]
pub(crate) struct MagicSourceGpu {
    pub(crate) pos: Vec2,
    pub(crate) strength: u32,
    pub(crate) mask: u32,
}

#[derive(AsBindGroup, Asset, TypePath, Clone)]
pub(crate) struct MagicPostProcessMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub(crate) scene: Handle<Image>,

    #[uniform(2)]
    pub(crate) params: MagicPostProcessParams,

    /// Storage buffer with one entry per active source.
    #[storage(3, read_only)]
    pub(crate) sources: Handle<ShaderStorageBuffer>,
}

impl Material2d for MagicPostProcessMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/magic_postprocess.wgsl".into()
    }
}

pub(crate) struct MagicPostProcessPlugin;

impl Plugin for MagicPostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MagicSources>()
            .init_resource::<MagicPostProcessSettings>()
            .add_plugins(Material2dPlugin::<MagicPostProcessMaterial>::default())
            .add_systems(Startup, setup_magic_cameras_and_quad)
            // Ensure gameplay entities are partitioned so UI isn’t postprocessed.
            // Run after transform propagation so children (e.g. bitmap glyph sprites) get the
            // effective Z inherited from their parents.
            .add_systems(
                PostUpdate,
                assign_gameplay_render_layers.after(TransformSystems::Propagate),
            )
            .add_systems(
                Update,
                (collect_magic_sources, push_magic_sources_to_material),
            );
    }
}

fn setup_magic_cameras_and_quad(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut materials: ResMut<Assets<MagicPostProcessMaterial>>,
) {
    // IMPORTANT: bevy_egui auto-creates the primary context for the *first* camera the app creates.
    // We want that to be the visible on-screen camera, not the offscreen world camera.
    commands.spawn((
        Name::new("Magic Screen Camera"),
        MagicScreenCamera,
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
        SpatialListener::default(),
        RenderLayers::layer(UI_LAYER),
        Projection::from(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::AutoMin {
                min_width: TARGET_WIDTH,
                min_height: TARGET_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
        Transform::default(),
        GlobalTransform::default(),
    ));

    // Render the world into a fixed 800x600 texture (matching our legacy coordinate system).
    let mut image = Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: TARGET_WIDTH as u32,
            height: TARGET_HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage = bevy::render::render_resource::TextureUsages::TEXTURE_BINDING
        | bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT;

    let target_handle = images.add(image);

    let sources_buffer = buffers.add(ShaderStorageBuffer::from(Vec::<MagicSourceGpu>::new()));
    commands.insert_resource(MagicSourcesBuffer(sources_buffer.clone()));

    // World camera: draws only WORLD_LAYER into the render target.
    commands.spawn((
        Name::new("Magic World Camera"),
        MagicWorldCamera,
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
        RenderTarget::Image(ImageRenderTarget {
            handle: target_handle.clone(),
            scale_factor: 1.0,
        }),
        RenderLayers::layer(WORLD_LAYER),
        Projection::from(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::Fixed {
                width: TARGET_WIDTH,
                height: TARGET_HEIGHT,
            },
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::default(),
        GlobalTransform::default(),
    ));

    // Postprocess quad material.
    let material_handle = materials.add(MagicPostProcessMaterial {
        scene: target_handle.clone(),
        params: MagicPostProcessParams {
            screen_size: Vec2::new(TARGET_WIDTH, TARGET_HEIGHT),
            source_count: 0,
            magic_enabled: 1,
            gamma: 1.0,
            _pad0: Vec3::ZERO,
        },
        sources: sources_buffer,
    });
    commands.insert_resource(MagicPostProcessMaterialHandle(material_handle.clone()));

    // Fullscreen quad sized to our fixed render target.
    let quad = Mesh::from(bevy::math::primitives::Rectangle::new(
        TARGET_WIDTH,
        TARGET_HEIGHT,
    ));
    let quad_handle = meshes.add(quad);

    commands.spawn((
        Name::new("Magic Postprocess Quad"),
        Mesh2d(quad_handle),
        MeshMaterial2d(material_handle),
        Transform::from_translation(Vec3::new(0.0, 0.0, -100.0)),
        RenderLayers::layer(UI_LAYER),
    ));
}

fn assign_gameplay_render_layers(
    mut commands: Commands,
    q_unlayered: Query<
        (Entity, &GlobalTransform),
        (With<GameplayRenderEntity>, Without<RenderLayers>),
    >,
) {
    for (e, global_transform) in &q_unlayered {
        let z = global_transform.translation().z;
        let layer = if z >= (Z_UI - 1.0) {
            UI_LAYER
        } else {
            WORLD_LAYER
        };
        commands.entity(e).insert(RenderLayers::layer(layer));
    }
}

fn collect_magic_sources(
    player_state: Res<PlayerState>,
    settings: Res<MagicPostProcessSettings>,
    mut sources: ResMut<MagicSources>,
) {
    sources.sources.clear();

    if !settings.enabled {
        return;
    }

    let map = player_state.map();

    // Match engine.c camera offsets.
    let (global_xoff, global_yoff) = map
        .tile_at_xy(TILEX / 2, TILEY / 2)
        .map(|center| {
            (
                (-(center.obj_xoff as f32) + MAP_X_SHIFT).round() as i32,
                (-(center.obj_yoff as f32)).round() as i32,
            )
        })
        .unwrap_or((MAP_X_SHIFT.round() as i32, 0));

    for y in (0..TILEY).rev() {
        for x in 0..TILEX {
            let Some(tile) = map.tile_at_xy(x, y) else {
                continue;
            };

            // Respect visibility: if the tile itself isn't visible (dark/fog/LOS blocked),
            // don't show magic effects originating from it.
            if (tile.flags & INVIS) != 0 {
                continue;
            }

            let mut alpha: u32 = 0;
            let mut alphastr: u32 = 0;

            if (tile.flags & EMAGIC) != 0 {
                alpha |= 1;
                alphastr = alphastr.max((tile.flags & EMAGIC) >> 22);
            }
            if (tile.flags & GMAGIC) != 0 {
                alpha |= 2;
                alphastr = alphastr.max((tile.flags & GMAGIC) >> 25);
            }
            if (tile.flags & CMAGIC) != 0 {
                alpha |= 4;
                alphastr = alphastr.max((tile.flags & CMAGIC) >> 28);
            }

            if alpha == 0 {
                continue;
            }

            let xpos = (x as i32) * 32;
            let ypos = (y as i32) * 32;
            let xoff = global_xoff + tile.obj_xoff;
            let yoff = global_yoff + tile.obj_yoff;

            let (rx, ry) = dd_magic_top_left(xpos, ypos, xoff, yoff);

            sources.sources.push(MagicSourceGpu {
                pos: Vec2::new(rx as f32, ry as f32),
                strength: alphastr.max(1),
                mask: alpha,
            });

            if sources.sources.len() >= MAX_MAGIC_SOURCES {
                return;
            }
        }
    }
}

fn push_magic_sources_to_material(
    sources: Res<MagicSources>,
    settings: Res<MagicPostProcessSettings>,
    buffer: Option<Res<MagicSourcesBuffer>>,
    handle: Option<Res<MagicPostProcessMaterialHandle>>,
    mut materials: ResMut<Assets<MagicPostProcessMaterial>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
) {
    let Some(buffer) = buffer else {
        return;
    };

    let Some(handle) = handle else {
        return;
    };

    let Some(material) = materials.get_mut(&handle.0) else {
        return;
    };

    material.params.magic_enabled = if settings.enabled { 1 } else { 0 };
    material.params.gamma = settings.gamma.clamp(0.1, 5.0);
    material.params.source_count = if settings.enabled {
        sources.sources.len().min(u32::MAX as usize) as u32
    } else {
        0
    };

    let Some(storage) = buffers.get_mut(&buffer.0) else {
        return;
    };
    storage.set_data(sources.sources.clone());
}

fn dd_magic_top_left(xpos: i32, ypos: i32, xoff: i32, yoff: i32) -> (i32, i32) {
    // Ported from dd.c::dd_alphaeffect_magic_0/_1.
    let mut rx = (xpos / 2) + (ypos / 2) - (2 * 16) + 32 + XPOS;
    if xpos < 0 && (xpos & 1) != 0 {
        rx -= 1;
    }
    if ypos < 0 && (ypos & 1) != 0 {
        rx -= 1;
    }

    let mut ry = (xpos / 4) - (ypos / 4) + YPOS - 2 * 32;
    if xpos < 0 && (xpos & 3) != 0 {
        ry -= 1;
    }
    if ypos < 0 && (ypos & 3) != 0 {
        ry += 1;
    }

    rx += xoff;
    ry += yoff;

    (rx, ry)
}
