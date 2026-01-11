use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResized, WindowResolution};

const TARGET_WIDTH: f32 = 800.0;
const TARGET_HEIGHT: f32 = 600.0;

fn main() {
    App::new()
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

fn print_click_coords(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else {
        error!("click: no primary window found");
        return;
    };

    let Some(cursor_logical) = window.cursor_position() else {
        info!("click: cursor not in window");
        return;
    };

    let scale_factor = window.resolution.scale_factor();
    let cursor_physical = cursor_logical * scale_factor;

    let mut extra = String::new();
    if let Ok(camera) = cameras.single() {
        let (vp_pos, vp_size) = if let Some(viewport) = camera.viewport.as_ref() {
            (
                Vec2::new(
                    viewport.physical_position.x as f32,
                    viewport.physical_position.y as f32,
                ),
                Vec2::new(
                    viewport.physical_size.x as f32,
                    viewport.physical_size.y as f32,
                ),
            )
        } else {
            (
                Vec2::ZERO,
                Vec2::new(
                    window.resolution.physical_width() as f32,
                    window.resolution.physical_height() as f32,
                ),
            )
        };

        if vp_size.x > 0.0 && vp_size.y > 0.0 {
            let vp_max = vp_pos + vp_size;
            let in_viewport = cursor_physical - vp_pos;

            // Only compute game coords if the click is inside the render viewport.
            if cursor_physical.x >= vp_pos.x
                && cursor_physical.x < vp_max.x
                && cursor_physical.y >= vp_pos.y
                && cursor_physical.y < vp_max.y
            {
                let game = Vec2::new(
                    in_viewport.x / vp_size.x * TARGET_WIDTH,
                    in_viewport.y / vp_size.y * TARGET_HEIGHT,
                );

                extra = format!(
                    ", viewport_px=({:.1},{:.1}), game_800x600=({:.1},{:.1})",
                    in_viewport.x, in_viewport.y, game.x, game.y
                );
            } else {
                extra = ", click in letterbox/pillarbox".to_string();
            }
        }
    }

    info!(
        "click: logical=({:.1},{:.1}), physical=({:.1},{:.1}){}",
        cursor_logical.x, cursor_logical.y, cursor_physical.x, cursor_physical.y, extra
    );
}

fn enforce_aspect_and_pixel_coords(
    mut window_resized: MessageReader<WindowResized>,
    mut initialized: Local<bool>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Camera, &mut Projection), With<Camera2d>>,
) {
    let mut should_update = !*initialized;
    for _ in window_resized.read() {
        should_update = true;
    }

    if !should_update {
        return;
    }
    *initialized = true;

    let Ok(window) = windows.single() else {
        return;
    };

    let window_w = window.resolution.physical_width();
    let window_h = window.resolution.physical_height();
    if window_w == 0 || window_h == 0 {
        return;
    }

    let target_aspect = TARGET_WIDTH / TARGET_HEIGHT;
    let window_aspect = window_w as f32 / window_h as f32;

    // Fit a 4:3 viewport inside the window (letterbox or pillarbox).
    let (viewport_w, viewport_h) = if window_aspect > target_aspect {
        // Window is wider than target: clamp by height.
        let vh = window_h;
        let vw = (window_h as f32 * target_aspect).round() as u32;
        (vw.min(window_w), vh)
    } else {
        // Window is taller than target: clamp by width.
        let vw = window_w;
        let vh = (window_w as f32 / target_aspect).round() as u32;
        (vw, vh.min(window_h))
    };

    let viewport_x = (window_w - viewport_w) / 2;
    let viewport_y = (window_h - viewport_h) / 2;

    for (mut camera, mut projection) in &mut cameras {
        camera.viewport = Some(Viewport {
            physical_position: UVec2::new(viewport_x, viewport_y),
            physical_size: UVec2::new(viewport_w, viewport_h),
            depth: 0.0..1.0,
        });

        // Keep the world view fixed at 800x600 world units (pixel coordinates).
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = bevy::camera::ScalingMode::Fixed {
                width: TARGET_WIDTH,
                height: TARGET_HEIGHT,
            };
            ortho.scale = 1.0;
        }
    }
}
