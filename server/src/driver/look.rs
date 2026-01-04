use crate::repository::Repository;
use crate::state::State;
use core::types::FontColor;

/// Show the description and available slots for a rat-eye (driver 17)
pub fn look_rat_eye(cn: usize, item_idx: usize) {
    // Print description
    let description = Repository::with_items(|items| items[item_idx].description);
    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("{}\n", String::from_utf8_lossy(&description)),
        );
    });

    // For the first 9 data slots, if a template id is present, show its name
    for n in 0..9 {
        let temp_id = Repository::with_items(|items| items[item_idx].data[n] as usize);
        if temp_id != 0 {
            let name = Repository::with_item_templates(|temps| {
                String::from_utf8_lossy(&temps[temp_id].name).to_string()
            });
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("The slot for a {} is free.\n", name),
                );
            });
        }
    }
}

/// Show the description and remaining charges for a spell scroll (driver 48)
pub fn look_spell_scroll(cn: usize, item_idx: usize) {
    let description = Repository::with_items(|items| items[item_idx].description);
    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("{}\n", String::from_utf8_lossy(&description)),
        );
    });

    let charges = Repository::with_items(|items| items[item_idx].data[2] as i32);
    let suffix = if charges == 1 { "" } else { "s" }; // "charge" vs "charges"
    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("There are {} charge{} left.\n", charges, suffix),
        );
    });
}

/// Dispatch based on the item's look driver
pub fn look_driver(cn: usize, item_idx: usize) {
    let driver = Repository::with_items(|items| items[item_idx].driver);
    match driver {
        17 => look_rat_eye(cn, item_idx),
        48 => look_spell_scroll(cn, item_idx),
        _ => log::warn!("Unknown look_driver {}", driver),
    }
}
