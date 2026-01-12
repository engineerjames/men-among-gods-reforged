use std::path::PathBuf;

use bevy::{ecs::resource::Resource, sprite::Sprite};

#[derive(Resource, Default)]
#[allow(dead_code)]
pub struct GraphicsCache {
    assets_zip: PathBuf,
    gfx: Vec<Sprite>,
}

impl GraphicsCache {
    pub fn new(assets_zip: &str) -> Self {
        Self {
            assets_zip: PathBuf::from(assets_zip),
            gfx: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn load_graphics(&mut self) {
        // Placeholder implementation
        println!("Loading graphics from {:?}", self.assets_zip);

        match std::fs::read_dir(&self.assets_zip) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if path.is_file() {
                                // Here you would load the image and create a Sprite
                                // For now, we just print the file path
                                println!("Found graphic file: {:?}", path);
                                // Placeholder: create a dummy Sprite and add to gfx
                                self.gfx.push(Sprite::default());
                            }
                        }
                        Err(e) => {
                            println!("Failed to read an entry in {:?}: {:?}", self.assets_zip, e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("Failed to read directory {:?}: {:?}", self.assets_zip, e);
            }
        }
    }
}
