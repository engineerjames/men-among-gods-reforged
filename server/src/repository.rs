use std::fs;

// Contains the data repository for the server
pub struct Repository {
    // Add fields as necessary
    map: [core::types::Map; core::constants::MAPX as usize * core::constants::MAPY as usize],
}

impl Repository {
    pub fn load(&mut self) -> Result<(), String> {
        self.load_map()?;
        self.load_items()?;
        self.load_item_templates()?;
        self.load_characters()?;
        self.load_character_templates()?;
        self.load_effects()?;
        self.load_globals()?;
        self.load_bad_names()?;
        self.load_bad_words()?;
        Ok(())
    }

    fn load_map(&mut self) -> Result<(), String> {
        log::info!("Loading map data...");
        let map_data = fs::read(".dat/map.dat").map_err(|e| e.to_string())?;

        let expected_map_size = core::constants::MAPX as usize
            * core::constants::MAPY as usize
            * std::mem::size_of::<core::types::Map>();

        let actual_map_size = map_data.len();
        if actual_map_size != expected_map_size {
            return Err(format!(
                "Map data size mismatch: expected {}, got {}",
                expected_map_size, actual_map_size
            ));
        }

        let num_map_tiles = actual_map_size / std::mem::size_of::<core::types::Map>();

        for i in 0..num_map_tiles {
            let offset = i * std::mem::size_of::<core::types::Map>();
            let map_tile = core::types::Map::from_bytes(
                &map_data[offset..offset + std::mem::size_of::<core::types::Map>()],
            )
            .ok_or_else(|| format!("Failed to parse map tile at index {}", i))?;
            self.map[i] = map_tile;
        }

        log::info!(
            "Map data loaded successfully. Loaded {} tiles.",
            num_map_tiles
        );

        Ok(())
    }

    fn load_items(&self) -> Result<(), String> {
        log::info!("Loading items data...");
        let items_data = fs::read(".dat/items.dat").map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_item_templates(&self) -> Result<(), String> {
        log::info!("Loading item templates data...");
        let item_templates_data = fs::read(".dat/titem.dat").map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_characters(&self) -> Result<(), String> {
        log::info!("Loading characters data...");
        let characters_data = fs::read(".dat/char.dat").map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_character_templates(&self) -> Result<(), String> {
        log::info!("Loading character templates data...");
        let character_templates_data = fs::read(".dat/tchar.dat").map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_effects(&self) -> Result<(), String> {
        log::info!("Loading effects data...");
        let effects_data = fs::read(".dat/effects.dat").map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_globals(&self) -> Result<(), String> {
        log::info!("Loading globals data...");
        let globals_data = fs::read(".dat/globals.dat").map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_bad_names(&self) -> Result<(), String> {
        log::info!("Loading bad names...");
        let bad_names_data = fs::read(".dat/bad_names.txt").map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_bad_words(&self) -> Result<(), String> {
        log::info!("Loading bad words...");
        let bad_words_data = fs::read(".dat/bad_words.txt").map_err(|e| e.to_string())?;
        Ok(())
    }

    // Add methods for data access and manipulation
}
