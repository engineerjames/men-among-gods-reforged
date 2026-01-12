use std::path::PathBuf;

use bevy::{ecs::resource::Resource, sprite::Sprite};

#[derive(Resource, Default)]
#[allow(dead_code)]
pub struct SoundCache {
    assets_zip: PathBuf,
    sfx: Vec<Sprite>,
}

impl SoundCache {
    pub fn new(assets_zip: &str) -> Self {
        Self {
            assets_zip: PathBuf::from(assets_zip),
            sfx: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn load_sounds(&mut self) {
        // Placeholder implementation
        println!("Loading sounds from {:?}", self.assets_zip);

        match std::fs::read_dir(&self.assets_zip) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if path.is_file() {
                                // Here you would load the sound and create a Sound object
                                // For now, we just print the file path
                                println!("Found sound file: {:?}", path);
                                // Placeholder: create a dummy Sprite and add to sfx
                                self.sfx.push(Sprite::default());
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
