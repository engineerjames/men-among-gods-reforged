use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;

use std::collections::HashMap;

use crate::gfx_cache::GraphicsCache;
use crate::map::{TILEX, TILEY};

use mag_core::constants::INVIS;

use super::components::{GameplayRenderEntity, GameplayUiMinimap};
use super::layout::{MINIMAP_SIZE, MINIMAP_X, MINIMAP_Y, Z_UI_MINIMAP};
use super::world_render::screen_to_world;

#[derive(Resource, Default)]
pub(crate) struct MiniMapState {
    /// Original client keeps a persistent 1024x1024 color buffer in 16-bit 5:6:5.
    /// Indexing matches the C code: idx = y + x*1024.
    pub(crate) xmap: Vec<u16>,
    pub(crate) avg_cache: HashMap<usize, u16>,
    pub(crate) image: Option<Handle<Image>>,
}

impl MiniMapState {
    pub(crate) fn ensure_initialized(&mut self, image_assets: &mut Assets<Image>) -> Handle<Image> {
        if self.xmap.len() != 1024 * 1024 {
            self.xmap = vec![0u16; 1024 * 1024];
        }

        if let Some(handle) = self.image.clone() {
            return handle;
        }

        let image = Image::new_fill(
            Extent3d {
                width: MINIMAP_SIZE,
                height: MINIMAP_SIZE,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );

        let handle = image_assets.add(image);
        self.image = Some(handle.clone());
        handle
    }

    fn avg_color_rgb565(
        &mut self,
        sprite_id: usize,
        gfx: &GraphicsCache,
        images: &Assets<Image>,
    ) -> u16 {
        if let Some(cached) = self.avg_cache.get(&sprite_id).copied() {
            return cached;
        }

        let Some(sprite) = gfx.get_sprite(sprite_id) else {
            self.avg_cache.insert(sprite_id, 0);
            return 0;
        };
        let Some(image) = images.get(&sprite.image) else {
            self.avg_cache.insert(sprite_id, 0);
            return 0;
        };

        let col = avg_color_rgb565_from_image(image);
        self.avg_cache.insert(sprite_id, col);
        col
    }
}

fn rgb565_to_rgba8(c: u16) -> [u8; 4] {
    let r5 = ((c >> 11) & 0x1f) as u32;
    let g6 = ((c >> 5) & 0x3f) as u32;
    let b5 = (c & 0x1f) as u32;

    let r = ((r5 * 255 + 15) / 31) as u8;
    let g = ((g6 * 255 + 31) / 63) as u8;
    let b = ((b5 * 255 + 15) / 31) as u8;
    [r, g, b, 255]
}

fn rgba8_to_rgb565(r: u8, g: u8, b: u8) -> u16 {
    let r5 = ((r as u32 * 31 + 127) / 255) as u16;
    let g6 = ((g as u32 * 63 + 127) / 255) as u16;
    let b5 = ((b as u32 * 31 + 127) / 255) as u16;
    (r5 << 11) | (g6 << 5) | b5
}

fn avg_color_rgb565_from_image(image: &Image) -> u16 {
    let format = image.texture_descriptor.format;
    let Some(data) = image.data.as_deref() else {
        return 0;
    };

    match format {
        TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb => {
            let mut sum_r: u64 = 0;
            let mut sum_g: u64 = 0;
            let mut sum_b: u64 = 0;
            let mut count: u64 = 0;

            for px in data.chunks_exact(4) {
                let (r, g, b, a) = match format {
                    TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
                        (px[2], px[1], px[0], px[3])
                    }
                    _ => (px[0], px[1], px[2], px[3]),
                };

                if a == 0 {
                    continue;
                }

                sum_r += r as u64;
                sum_g += g as u64;
                sum_b += b as u64;
                count += 1;
            }

            if count == 0 {
                return 0;
            }

            let r = (sum_r / count) as u8;
            let g = (sum_g / count) as u8;
            let b = (sum_b / count) as u8;
            rgba8_to_rgb565(r, g, b)
        }
        _ => 0,
    }
}

#[cfg(test)]
mod color_tests {
    use super::{rgb565_to_rgba8, rgba8_to_rgb565};

    #[test]
    fn rgb565_known_primaries_match_expected() {
        assert_eq!(rgba8_to_rgb565(255, 0, 0), 0xF800);
        assert_eq!(rgba8_to_rgb565(0, 255, 0), 0x07E0);
        assert_eq!(rgba8_to_rgb565(0, 0, 255), 0x001F);
        assert_eq!(rgba8_to_rgb565(0, 0, 0), 0x0000);
        assert_eq!(rgba8_to_rgb565(255, 255, 255), 0xFFFF);
    }

    #[test]
    fn rgb565_roundtrips_through_rgba8() {
        let colors = [
            (12u8, 34u8, 56u8),
            (200u8, 100u8, 50u8),
            (1u8, 2u8, 3u8),
            (254u8, 253u8, 252u8),
        ];

        for (r, g, b) in colors {
            let c565 = rgba8_to_rgb565(r, g, b);
            let [rr, gg, bb, aa] = rgb565_to_rgba8(c565);
            assert_eq!(aa, 255);
            let c565_2 = rgba8_to_rgb565(rr, gg, bb);
            assert_eq!(c565, c565_2);
        }
    }
}

pub(crate) fn spawn_ui_minimap(commands: &mut Commands, image: Handle<Image>) {
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiMinimap,
        Sprite { image, ..default() },
        Anchor::TOP_LEFT,
        Transform::from_translation(screen_to_world(MINIMAP_X, MINIMAP_Y, Z_UI_MINIMAP)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

pub(crate) fn update_minimap(
    minimap: &mut MiniMapState,
    gfx: &GraphicsCache,
    images: &mut Assets<Image>,
    map: &crate::map::GameMap,
) {
    let Some(center) = map.tile_at_xy(TILEX / 2, TILEY / 2) else {
        return;
    };

    let center_x = center.x as usize;
    let center_y = center.y as usize;

    // Keep persistent xmap up-to-date with what we can currently see.
    for idx in 0..map.len() {
        let Some(tile) = map.tile_at_index(idx) else {
            continue;
        };

        let gx = tile.x as usize;
        let gy = tile.y as usize;
        if gx >= 1024 || gy >= 1024 {
            continue;
        }
        if (tile.flags & INVIS) != 0 {
            continue;
        }

        let cell = gy + gx * 1024;

        // Background updates only if the cell is empty or currently the player marker.
        let back_id = tile.back.max(0) as usize;
        if back_id != 0 {
            let cur = minimap.xmap[cell];
            if cur == 0 || cur == 0xffff {
                minimap.xmap[cell] = minimap.avg_color_rgb565(back_id, gfx, images);
            }
        }

        // Objects override the background.
        if tile.obj1 > 0 {
            minimap.xmap[cell] = minimap.avg_color_rgb565(tile.obj1 as usize, gfx, images);
        }
    }

    // Mark player position.
    if center_x < 1024 && center_y < 1024 {
        minimap.xmap[center_y + center_x * 1024] = 0xffff;
    }

    // Compute the view window (matches engine.c clamps) and copy it into a 128x128 image.
    let mut mapx = center_x as i32 - 64;
    let mut mapy = center_y as i32 - 64;

    mapx = mapx.clamp(0, 1023 - MINIMAP_SIZE as i32);
    mapy = mapy.clamp(0, 1023 - MINIMAP_SIZE as i32);

    // dd_show_map reads src as if it were row-major, but xmap is indexed as y + x*1024.
    // The original call is dd_show_map(xmap, mapy, mapx), so we preserve that swap.
    let xo = mapy as usize;
    let yo = mapx as usize;

    let handle = minimap.ensure_initialized(images);
    let Some(image) = images.get_mut(&handle) else {
        return;
    };

    let expected_len = (MINIMAP_SIZE * MINIMAP_SIZE * 4) as usize;
    let data = image.data.get_or_insert_with(|| vec![0u8; expected_len]);
    if data.len() != expected_len {
        data.resize(expected_len, 0);
    }

    let mut out_i = 0usize;
    for y in 0..MINIMAP_SIZE as usize {
        let s = (y + yo) * 1024 + xo;
        for x in 0..MINIMAP_SIZE as usize {
            let c = minimap.xmap[s + x];
            let rgba = rgb565_to_rgba8(c);
            data[out_i] = rgba[0];
            data[out_i + 1] = rgba[1];
            data[out_i + 2] = rgba[2];
            data[out_i + 3] = rgba[3];
            out_i += 4;
        }
    }
}
