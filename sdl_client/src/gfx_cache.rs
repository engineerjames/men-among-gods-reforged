use std::{collections::HashMap, path::PathBuf};

use sdl2::{
    image::LoadTexture,
    render::{Texture, TextureCreator},
    video::WindowContext,
};
use zip::ZipArchive;

pub struct GraphicsCache {
    sprite_cache: HashMap<usize, Texture>,
    avg_color_cache: HashMap<usize, (u8, u8, u8)>,
    creator: TextureCreator<WindowContext>,
}

impl GraphicsCache {
    pub fn new(path_to_zip: PathBuf, creator: TextureCreator<WindowContext>) -> Self {
        GraphicsCache {
            sprite_cache: HashMap::new(),
            avg_color_cache: HashMap::new(),
            creator,
        }
    }

    pub fn get_texture(&mut self, id: usize) -> &Texture {
        if self.sprite_cache.contains_key(&id) {
            return &self.sprite_cache[&id];
        }

        let filename = PathBuf::from(
            "/Users/jarmes/git/men-among-gods-reforged/sdl_client/assets/gfx/00001.png",
        );
        let texture = self.creator.load_texture(filename);
        self.sprite_cache.insert(id, texture.unwrap());

        &self.sprite_cache[&id]
    }
}
