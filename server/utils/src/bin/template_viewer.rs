mod template_viewer_app;

use eframe::egui;
use mag_core::ranks;

use template_viewer_app::TemplateViewerApp;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Template Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "Template Viewer",
        options,
        Box::new(|_cc| {
            let data_source = server_utils::data_source_from_args();
            Ok(Box::new(TemplateViewerApp::new(data_source)))
        }),
    )
}

fn write_c_string(dst: &mut [u8], src: &str) {
    dst.fill(0);
    if dst.is_empty() {
        return;
    }

    let bytes = src.as_bytes();
    let limit = bytes.len().min(dst.len().saturating_sub(1));
    dst[..limit].copy_from_slice(&bytes[..limit]);
}

// Helper function to format currency as gold and silver
fn format_gold_silver(value: i32) -> String {
    let gold = value / 1000;
    let silver = value % 1000;
    if gold > 0 && silver > 0 {
        format!("{} gold, {} silver", gold, silver)
    } else if gold > 0 {
        format!("{} gold", gold)
    } else {
        format!("{} silver", silver)
    }
}

fn centered_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.with_layout(
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| {
            ui.label(text);
        },
    );
}

fn centered_heading(ui: &mut egui::Ui, text: impl Into<egui::RichText>) {
    ui.horizontal_centered(|ui| {
        ui.heading(text);
    });
}

fn rank_label(min_rank: i8) -> String {
    if min_rank < 0 {
        return "-1: None".to_string();
    }

    let idx = min_rank as usize;
    format!("{}: {}", idx, ranks::rank_name_by_index(idx))
}

fn placement_options() -> &'static [(u16, &'static str)] {
    &[
        (0, "Unset"),
        (mag_core::constants::PL_HEAD, "Head"),
        (mag_core::constants::PL_NECK, "Neck"),
        (mag_core::constants::PL_BODY, "Body"),
        (mag_core::constants::PL_ARMS, "Arms"),
        (mag_core::constants::PL_BELT, "Belt"),
        (mag_core::constants::PL_LEGS, "Legs"),
        (mag_core::constants::PL_FEET, "Feet"),
        (mag_core::constants::PL_WEAPON, "Weapon"),
        (mag_core::constants::PL_SHIELD, "Shield"),
        (mag_core::constants::PL_CLOAK, "Cloak"),
        (mag_core::constants::PL_TWOHAND, "Two-Hand"),
        (0x0900, "Two-Handed"),
        (mag_core::constants::PL_RING, "Ring"),
    ]
}

fn placement_label(placement: u16) -> String {
    if placement == 0 {
        return "Unset".to_string();
    }

    for (value, name) in placement_options() {
        if *value == placement {
            return (*name).to_string();
        }
    }

    format!("Unknown (0x{:04X})", placement)
}

fn get_item_flag_info() -> Vec<(mag_core::constants::ItemFlags, &'static str)> {
    use mag_core::constants::ItemFlags;
    vec![
        (ItemFlags::IF_MOVEBLOCK, "Move Block"),
        (ItemFlags::IF_SIGHTBLOCK, "Sight Block"),
        (ItemFlags::IF_TAKE, "Take"),
        (ItemFlags::IF_MONEY, "Money"),
        (ItemFlags::IF_LOOK, "Look"),
        (ItemFlags::IF_LOOKSPECIAL, "Look Special"),
        (ItemFlags::IF_SPELL, "Spell"),
        (ItemFlags::IF_NOREPAIR, "No Repair"),
        (ItemFlags::IF_ARMOR, "Armor"),
        (ItemFlags::IF_USE, "Use"),
        (ItemFlags::IF_USESPECIAL, "Use Special"),
        (ItemFlags::IF_SINGLEAGE, "Single Age"),
        (ItemFlags::IF_SHOPDESTROY, "Shop Destroy"),
        (ItemFlags::IF_UPDATE, "Update"),
        (ItemFlags::IF_ALWAYSEXP1, "Always Expire 1"),
        (ItemFlags::IF_ALWAYSEXP2, "Always Expire 2"),
        (ItemFlags::IF_WP_SWORD, "Weapon Sword"),
        (ItemFlags::IF_WP_DAGGER, "Weapon Dagger"),
        (ItemFlags::IF_WP_AXE, "Weapon Axe"),
        (ItemFlags::IF_WP_STAFF, "Weapon Staff"),
        (ItemFlags::IF_WP_TWOHAND, "Weapon Two-Hand"),
        (ItemFlags::IF_USEDESTROY, "Use Destroy"),
        (ItemFlags::IF_USEACTIVATE, "Use Activate"),
        (ItemFlags::IF_USEDEACTIVATE, "Use Deactivate"),
        (ItemFlags::IF_MAGIC, "Magic"),
        (ItemFlags::IF_MISC, "Misc"),
        (ItemFlags::IF_REACTIVATE, "Reactivate"),
        (ItemFlags::IF_PERMSPELL, "Perm Spell"),
        (ItemFlags::IF_UNIQUE, "Unique"),
        (ItemFlags::IF_DONATE, "Donate"),
        (ItemFlags::IF_LABYDESTROY, "Laby Destroy"),
        (ItemFlags::IF_NOMARKET, "No Market"),
        (ItemFlags::IF_HIDDEN, "Hidden"),
        (ItemFlags::IF_STEPACTION, "Step Action"),
        (ItemFlags::IF_NODEPOT, "No Depot"),
        (ItemFlags::IF_LIGHTAGE, "Light Age"),
        (ItemFlags::IF_EXPIREPROC, "Expire Proc"),
        (ItemFlags::IF_IDENTIFIED, "Identified"),
        (ItemFlags::IF_NOEXPIRE, "No Expire"),
        (ItemFlags::IF_SOULSTONE, "Soulstone"),
    ]
}

fn get_character_flag_info() -> Vec<mag_core::constants::CharacterFlags> {
    use mag_core::constants::CharacterFlags;
    vec![
        CharacterFlags::Immortal,
        CharacterFlags::God,
        CharacterFlags::Creator,
        CharacterFlags::BuildMode,
        CharacterFlags::Respawn,
        CharacterFlags::Player,
        CharacterFlags::NewUser,
        CharacterFlags::NoTell,
        CharacterFlags::NoShout,
        CharacterFlags::Merchant,
        CharacterFlags::Staff,
        CharacterFlags::NoHpReg,
        CharacterFlags::NoEndReg,
        CharacterFlags::NoManaReg,
        CharacterFlags::Invisible,
        CharacterFlags::Infrared,
        CharacterFlags::Body,
        CharacterFlags::NoSleep,
        CharacterFlags::Undead,
        CharacterFlags::NoMagic,
        CharacterFlags::Stoned,
        CharacterFlags::Usurp,
        CharacterFlags::Imp,
        CharacterFlags::ShutUp,
        CharacterFlags::NoDesc,
        CharacterFlags::Profile,
        CharacterFlags::Simple,
        CharacterFlags::Kicked,
        CharacterFlags::NoList,
        CharacterFlags::NoWho,
        CharacterFlags::SpellIgnore,
        CharacterFlags::ComputerControlledPlayer,
        CharacterFlags::Safe,
        CharacterFlags::NoStaff,
        CharacterFlags::Poh,
        CharacterFlags::PohLeader,
        CharacterFlags::Thrall,
        CharacterFlags::LabKeeper,
        CharacterFlags::IsLooting,
        CharacterFlags::Golden,
        CharacterFlags::Black,
        CharacterFlags::Passwd,
        CharacterFlags::Update,
        CharacterFlags::SaveMe,
        CharacterFlags::GreaterGod,
        CharacterFlags::GreaterInv,
    ]
}
