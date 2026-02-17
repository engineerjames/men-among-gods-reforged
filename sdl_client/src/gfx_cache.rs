use std::{collections::HashMap, path::PathBuf};

use sdl2::{
    image::LoadTexture,
    render::{Texture, TextureCreator},
    video::WindowContext,
};

pub struct GraphicsCache {
    // Placeholder for future caching of textures, fonts, etc.
    cache: HashMap<usize, Texture>,
    creator: TextureCreator<WindowContext>,
}

impl GraphicsCache {
    pub fn new(creator: TextureCreator<WindowContext>) -> Self {
        GraphicsCache {
            cache: HashMap::new(),
            creator,
        }
    }

    pub fn get_texture(&mut self, id: usize) -> &Texture {
        if self.cache.contains_key(&id) {
            return &self.cache[&id];
        }

        let filename = PathBuf::from(".");
        let texture = self.creator.load_texture(filename);
        self.cache.insert(id, texture.unwrap());

        &self.cache[&id]
    }
}
