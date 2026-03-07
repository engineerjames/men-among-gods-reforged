use crate::game_state::GameState;
use core::string_operations::c_string_to_str;
use core::types::FontColor;

/// Show the description and available slots for a rat-eye (driver 17)
pub fn look_rat_eye(gs: &mut GameState, cn: usize, item_idx: usize) {
    // Print description
    let description = gs.items[item_idx].description;
    gs.do_character_log(
        cn,
        FontColor::Yellow,
        &format!("{}\n", c_string_to_str(&description)),
    );

    // For the first 9 data slots, if a template id is present, show its name
    for n in 0..9 {
        let temp_id = gs.items[item_idx].data[n] as usize;
        if temp_id != 0 {
            let name = gs.item_templates[temp_id].get_name().to_string();
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("The slot for a {} is free.\n", name),
            );
        }
    }
}

/// Show the description and remaining charges for a spell scroll (driver 48)
pub fn look_spell_scroll(gs: &mut GameState, cn: usize, item_idx: usize) {
    let description = gs.items[item_idx].description;
    gs.do_character_log(
        cn,
        FontColor::Yellow,
        &format!("{}\n", c_string_to_str(&description)),
    );

    let charges = gs.items[item_idx].data[2] as i32;
    let suffix = if charges == 1 { "" } else { "s" }; // "charge" vs "charges"
    gs.do_character_log(
        cn,
        FontColor::Yellow,
        &format!("There are {} charge{} left.\n", charges, suffix),
    );
}

/// Dispatch based on the item's look driver
pub fn look_driver(gs: &mut GameState, cn: usize, item_idx: usize) {
    let driver = gs.items[item_idx].driver;
    match driver {
        17 => look_rat_eye(gs, cn, item_idx),
        48 => look_spell_scroll(gs, cn, item_idx),
        _ => log::warn!("Unknown look_driver {}", driver),
    }
}
