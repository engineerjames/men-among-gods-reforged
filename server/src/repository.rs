use std::fs;

// Contains the data repository for the server
pub struct Repository {
    // Add fields as necessary
    map: [core::types::Map; core::constants::MAPX as usize * core::constants::MAPY as usize],
    items: [core::types::Item; core::constants::MAXITEM as usize],
    item_templates: [core::types::Item; core::constants::MAXTITEM as usize],
    characters: [core::types::Character; core::constants::MAXCHARS as usize],
    character_templates: [core::types::Character; core::constants::MAXTCHARS as usize],
    effects: [core::types::Effect; core::constants::MAXEFFECT as usize],
    pub globals: core::types::Global,
    bad_names: Vec<String>,
    bad_words: Vec<String>,
}

impl Repository {
    pub fn new() -> Self {
        Self {
            map: [core::types::Map::default();
                core::constants::MAPX as usize * core::constants::MAPY as usize],
            items: [core::types::Item::default(); core::constants::MAXITEM as usize],
            item_templates: [core::types::Item::default(); core::constants::MAXTITEM as usize],
            characters: [core::types::Character::default(); core::constants::MAXCHARS as usize],
            character_templates: [core::types::Character::default();
                core::constants::MAXTCHARS as usize],
            effects: [core::types::Effect::default(); core::constants::MAXEFFECT as usize],
            globals: core::types::Global::default(),
            bad_names: Vec::new(),
            bad_words: Vec::new(),
        }
    }
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

    fn load_items(&mut self) -> Result<(), String> {
        log::info!("Loading items data...");
        let items_data = fs::read(".dat/items.dat").map_err(|e| e.to_string())?;

        let expected_items_size =
            core::constants::MAXITEM as usize * std::mem::size_of::<core::types::Item>();

        let actual_items_size = items_data.len();
        if actual_items_size != expected_items_size {
            return Err(format!(
                "Items data size mismatch: expected {}, got {}",
                expected_items_size, actual_items_size
            ));
        }

        let num_items = actual_items_size / std::mem::size_of::<core::types::Item>();

        for i in 0..num_items {
            let offset = i * std::mem::size_of::<core::types::Item>();
            let item = core::types::Item::from_bytes(
                &items_data[offset..offset + std::mem::size_of::<core::types::Item>()],
            )
            .ok_or_else(|| format!("Failed to parse item at index {}", i))?;
            self.items[i] = item;
        }

        log::info!(
            "Items data loaded successfully. Loaded {} items.",
            num_items
        );

        Ok(())
    }

    fn load_item_templates(&mut self) -> Result<(), String> {
        log::info!("Loading item templates data...");
        let item_templates_data = fs::read(".dat/titem.dat").map_err(|e| e.to_string())?;

        let expected_item_templates_size =
            core::constants::MAXTITEM as usize * std::mem::size_of::<core::types::Item>();

        let actual_item_templates_size = item_templates_data.len();

        if actual_item_templates_size != expected_item_templates_size {
            return Err(format!(
                "Item templates data size mismatch: expected {}, got {}",
                expected_item_templates_size, actual_item_templates_size
            ));
        }
        let num_item_templates =
            actual_item_templates_size / std::mem::size_of::<core::types::Item>();

        for i in 0..num_item_templates {
            let offset = i * std::mem::size_of::<core::types::Item>();
            let item_template = core::types::Item::from_bytes(
                &item_templates_data[offset..offset + std::mem::size_of::<core::types::Item>()],
            )
            .ok_or_else(|| format!("Failed to parse item template at index {}", i))?;
            self.item_templates[i] = item_template;
        }

        log::info!(
            "Item templates data loaded successfully. Loaded {} templates.",
            num_item_templates
        );

        Ok(())
    }

    fn load_characters(&mut self) -> Result<(), String> {
        log::info!("Loading characters data...");
        let characters_data = fs::read(".dat/char.dat").map_err(|e| e.to_string())?;

        let expected_characters_size =
            core::constants::MAXCHARS as usize * std::mem::size_of::<core::types::Character>();
        let actual_characters_size = characters_data.len();

        if actual_characters_size != expected_characters_size {
            return Err(format!(
                "Characters data size mismatch: expected {}, got {}",
                expected_characters_size, actual_characters_size
            ));
        }

        let num_characters = actual_characters_size / std::mem::size_of::<core::types::Character>();

        for i in 0..num_characters {
            let offset = i * std::mem::size_of::<core::types::Character>();
            let character = core::types::Character::from_bytes(
                &characters_data[offset..offset + std::mem::size_of::<core::types::Character>()],
            )
            .ok_or_else(|| format!("Failed to parse character at index {}", i))?;
            self.characters[i] = character;
        }

        Ok(())
    }

    fn load_character_templates(&mut self) -> Result<(), String> {
        log::info!("Loading character templates data...");
        let character_templates_data = fs::read(".dat/tchar.dat").map_err(|e| e.to_string())?;
        let expected_character_templates_size =
            core::constants::MAXTCHARS as usize * std::mem::size_of::<core::types::Character>();
        let actual_character_templates_size = character_templates_data.len();
        if actual_character_templates_size != expected_character_templates_size {
            return Err(format!(
                "Character templates data size mismatch: expected {}, got {}",
                expected_character_templates_size, actual_character_templates_size
            ));
        }

        let num_character_templates =
            actual_character_templates_size / std::mem::size_of::<core::types::Character>();

        for i in 0..num_character_templates {
            let offset = i * std::mem::size_of::<core::types::Character>();
            let character_template = core::types::Character::from_bytes(
                &character_templates_data
                    [offset..offset + std::mem::size_of::<core::types::Character>()],
            )
            .ok_or_else(|| format!("Failed to parse character template at index {}", i))?;
            self.character_templates[i] = character_template;
        }

        Ok(())
    }

    fn load_effects(&mut self) -> Result<(), String> {
        log::info!("Loading effects data...");
        let effects_data = fs::read(".dat/effects.dat").map_err(|e| e.to_string())?;

        let expected_effects_size =
            core::constants::MAXEFFECT as usize * std::mem::size_of::<core::types::Effect>();
        let actual_effects_size = effects_data.len();

        if actual_effects_size != expected_effects_size {
            return Err(format!(
                "Effects data size mismatch: expected {}, got {}",
                expected_effects_size, actual_effects_size
            ));
        }

        let num_effects = actual_effects_size / std::mem::size_of::<core::types::Effect>();

        for i in 0..num_effects {
            let offset = i * std::mem::size_of::<core::types::Effect>();
            let effect = core::types::Effect::from_bytes(
                &effects_data[offset..offset + std::mem::size_of::<core::types::Effect>()],
            )
            .ok_or_else(|| format!("Failed to parse effect at index {}", i))?;
            self.effects[i] = effect;
        }

        log::info!(
            "Effects data loaded successfully. Loaded {} effects.",
            num_effects
        );

        Ok(())
    }

    fn load_globals(&mut self) -> Result<(), String> {
        log::info!("Loading globals data...");
        let globals_data = fs::read(".dat/globals.dat").map_err(|e| e.to_string())?;

        if globals_data.len() != std::mem::size_of::<core::types::Global>() {
            return Err(format!(
                "Globals data size mismatch: expected {}, got {}",
                std::mem::size_of::<core::types::Global>(),
                globals_data.len()
            ));
        }

        self.globals = core::types::Global::from_bytes(&globals_data)
            .ok_or_else(|| "Failed to parse globals data".to_string())?;

        log::info!("Globals data loaded successfully.");

        Ok(())
    }

    fn load_bad_names(&mut self) -> Result<(), String> {
        log::info!("Loading bad names...");
        let bad_names_data = fs::read_to_string(".dat/bad_names.txt").map_err(|e| e.to_string())?;

        for line in bad_names_data.lines() {
            self.bad_names.push(line.to_string());
        }

        Ok(())
    }

    fn load_bad_words(&mut self) -> Result<(), String> {
        log::info!("Loading bad words...");
        let bad_words_data = fs::read_to_string(".dat/bad_words.txt").map_err(|e| e.to_string())?;

        for line in bad_words_data.lines() {
            self.bad_words.push(line.to_string());
        }

        Ok(())
    }
}
