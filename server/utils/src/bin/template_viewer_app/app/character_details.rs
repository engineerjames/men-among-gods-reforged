//! Character template / character instance detail editor.

use super::widgets::{
    ATTRIBUTE_NAMES, c_string_row, drag, drag_cells, drag_note_row, drag_pair_row, drag_row,
    driver_data_section, flag_grid,
};
use super::{TemplateViewerApp, ViewMode};
use eframe::egui;
use mag_core::constants::character_flags_name;
use mag_core::types::Character;
use mag_core::{skills, traits};

/// Kindred bits shown as checkboxes.
const KINDRED_FLAGS: [(u32, &str); 12] = [
    (traits::KIN_MERCENARY, "Mercenary"),
    (traits::KIN_SEYAN_DU, "Seyan Du"),
    (traits::KIN_PURPLE, "Purple"),
    (traits::KIN_MONSTER, "Monster"),
    (traits::KIN_TEMPLAR, "Templar"),
    (traits::KIN_ARCHTEMPLAR, "ArchTemplar"),
    (traits::KIN_HARAKIM, "Harakim"),
    (traits::KIN_MALE, "Male"),
    (traits::KIN_FEMALE, "Female"),
    (traits::KIN_ARCHHARAKIM, "ArchHarakim"),
    (traits::KIN_WARRIOR, "Warrior"),
    (traits::KIN_SORCERER, "Sorcerer"),
];

/// Column headers for the six-value stat rows (attributes, vitals, skills).
const STAT_COLUMNS: [&str; 6] = ["[0]", "[1]", "[2]", "[3]", "[4]", "[5]"];

impl TemplateViewerApp {
    /// Edit `table[idx]` (character templates or instances), marking the slot dirty on change.
    ///
    /// # Arguments
    ///
    /// * `ui` - Target UI.
    /// * `table` - [`ViewMode::CharacterTemplates`] or [`ViewMode::Characters`].
    /// * `idx` - Slot to edit.
    pub(super) fn render_character_details_by_index(
        &mut self,
        ui: &mut egui::Ui,
        table: ViewMode,
        idx: usize,
    ) {
        if table == ViewMode::CharacterTemplates
            && let Err(e) = self.ensure_character_template_loaded(idx)
        {
            ui.colored_label(egui::Color32::RED, e);
            return;
        }

        let records = match table {
            ViewMode::CharacterTemplates => &self.character_templates,
            ViewMode::Characters => &self.characters,
            ViewMode::ItemTemplates | ViewMode::Items => return,
        };
        let Some(before) = records.get(idx).copied() else {
            return;
        };

        let mut character = before;
        self.render_character_details(ui, &mut character);
        if character == before {
            return;
        }
        let records = match table {
            ViewMode::CharacterTemplates => &mut self.character_templates,
            _ => &mut self.characters,
        };
        records[idx] = character;
        self.mark_slot_dirty(table, idx);
    }

    /// All editable fields of one character.
    fn render_character_details(&mut self, ui: &mut egui::Ui, character: &mut Character) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(character.get_name());
            ui.separator();
            let id = character.temp;

            self.ui_character_identity(ui, character, id);
            flag_grid(
                ui,
                format!("character_flags_grid_{id}"),
                &mut character.flags,
                crate::get_character_flag_info()
                    .into_iter()
                    .map(|flag| (flag.bits(), character_flags_name(flag))),
                3,
                true,
            );
            ui_character_stats(ui, character, id);
            ui_character_tables(ui, character);
            self.ui_character_equipment(ui, character);
            driver_data_section(
                ui,
                "character_driver_data",
                &mut character.data,
                &mut self.show_all_data_fields,
                20.0,
            );
        });
    }

    /// Index, names, kindred, sprite, and sound.
    fn ui_character_identity(&mut self, ui: &mut egui::Ui, character: &mut Character, id: u16) {
        egui::Grid::new("character_details")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                drag_row(ui, "Index:", &mut character.temp);
                drag_row(ui, "Used:", &mut character.used);
                c_string_row(ui, "Name:", &mut character.name, false);
                c_string_row(ui, "Reference:", &mut character.reference, false);
                c_string_row(ui, "Description:", &mut character.description, true);

                ui.label("Kindred:");
                ui.vertical(|ui| {
                    let kindred = &mut character.kindred;
                    ui.label(format!("{} (0x{:08X})", kindred, *kindred as u32));
                    drag(ui, kindred);
                    *kindred = (*kindred).max(0);
                    ui.separator();

                    let mut bits = u64::from(*kindred as u32);
                    flag_grid(
                        ui,
                        format!("kindred_flags_grid_{id}"),
                        &mut bits,
                        KINDRED_FLAGS
                            .iter()
                            .map(|(bit, name)| (u64::from(*bit), *name)),
                        2,
                        false,
                    );
                    *kindred = bits as u32 as i32;
                });
                ui.end_row();

                ui.label("Sprite:");
                ui.vertical(|ui| {
                    drag(ui, &mut character.sprite);
                    self.sprite_cell(ui, usize::from(character.sprite));
                });
                ui.end_row();

                drag_row(ui, "Sound:", &mut character.sound);

                ui.label("Flags:");
                ui.end_row();
            });
    }

    /// Inventory and worn slots, each with a "View" button for the item template.
    fn ui_character_equipment(&mut self, ui: &mut egui::Ui, character: &mut Character) {
        let mut item_id_cell = |ui: &mut egui::Ui, item_id: &mut u32| {
            ui.horizontal(|ui| {
                drag(ui, item_id);
                if *item_id != 0 && ui.small_button("View").clicked() {
                    self.item_popup_id = Some(*item_id);
                }
            });
        };

        ui.separator();
        crate::centered_heading(ui, "Inventory");
        egui::Grid::new("character_inventory")
            .num_columns(4)
            .spacing([20.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for header in ["Slot", "Item ID", "Slot", "Item ID"] {
                    ui.label(header);
                }
                ui.end_row();

                for (row, pair) in character.item.chunks_mut(2).enumerate() {
                    for (col, item_id) in pair.iter_mut().enumerate() {
                        ui.label(format!("{}", row * 2 + col));
                        item_id_cell(ui, item_id);
                    }
                    ui.end_row();
                }
            });

        ui.separator();
        crate::centered_heading(ui, "Worn Equipment");
        egui::Grid::new("character_worn")
            .num_columns(2)
            .spacing([20.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Slot");
                ui.label("Item ID");
                ui.end_row();

                for (slot, item_id) in character.worn.iter_mut().enumerate() {
                    ui.label(format!("{}", slot));
                    item_id_cell(ui, item_id);
                    ui.end_row();
                }
            });
    }
}

/// Alignment, positions, gold/points, combat values, and speed.
fn ui_character_stats(ui: &mut egui::Ui, character: &mut Character, id: u16) {
    egui::Grid::new(format!("character_details_grid2_{id}"))
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            drag_row(ui, "Alignment:", &mut character.alignment);
            drag_pair_row(
                ui,
                "Temple:",
                &mut character.temple_x,
                &mut character.temple_y,
            );
            drag_pair_row(
                ui,
                "Tavern:",
                &mut character.tavern_x,
                &mut character.tavern_y,
            );
            drag_pair_row(ui, "Position:", &mut character.x, &mut character.y);

            ui.label("Area:");
            ui.label(
                mag_core::area::get_area_m(i32::from(character.x), i32::from(character.y))
                    .unwrap_or_else(|| "Unknown".to_owned()),
            );
            ui.end_row();

            ui.label("Gold:");
            ui.horizontal(|ui| {
                drag(ui, &mut character.gold);
                ui.label(crate::format_gold_silver(character.gold));
            });
            ui.end_row();

            ui.label("Points:");
            let rank_name = mag_core::ranks::rank_name(character.points_tot.max(0) as u32);
            ui.horizontal(|ui| {
                drag(ui, &mut character.points);
                drag(ui, &mut character.points_tot);
                ui.label(format!("({})", rank_name));
            });
            ui.end_row();

            drag_row(ui, "Armor:", &mut character.armor);
            drag_row(ui, "Weapon:", &mut character.weapon);
            drag_row(ui, "Light:", &mut character.light);
            drag_note_row(
                ui,
                "Armor Bonus:",
                &mut character.armor_bonus,
                "Permanent armor added on top of worn items.",
            );
            drag_note_row(
                ui,
                "Weapon Bonus:",
                &mut character.weapon_bonus,
                "Permanent weapon damage added on top of worn items.",
            );
            drag_note_row(
                ui,
                "Light Bonus:",
                &mut character.light_bonus,
                "Permanent light radius added on top of worn items.",
            );
            drag_note_row(
                ui,
                "Gethit Bonus:",
                &mut character.gethit_bonus,
                "Thorns damage dealt back to melee attackers (rand(value)+1, armor-bypassing). 0 = disabled.",
            );
            drag_row(ui, "Mode:", &mut character.mode);
            drag_row(ui, "Speed:", &mut character.speed);
            drag_note_row(
                ui,
                "Speed Mod:",
                &mut character.speed_mod,
                "Race/template speed modifier applied on top of agility/strength.",
            );
            drag_row(ui, "Monster Class:", &mut character.monster_class);
        });
}

/// Attribute, vital, active-value, and skill tables.
fn ui_character_tables(ui: &mut egui::Ui, character: &mut Character) {
    ui.separator();
    crate::centered_heading(ui, "Attributes");
    egui::Grid::new("character_attributes")
        .num_columns(7)
        .spacing([15.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            for header in [
                "Stat",
                "Base",
                "Preset",
                "Max",
                "Difficulty",
                "Dynamic",
                "Total",
            ] {
                ui.label(header);
            }
            ui.end_row();

            for (name, row) in ATTRIBUTE_NAMES.iter().zip(character.attrib.iter_mut()) {
                ui.label(*name);
                drag_cells(ui, row);
                ui.end_row();
            }
        });

    ui.separator();
    egui::Grid::new("character_vitals")
        .num_columns(7)
        .spacing([15.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Vital");
            for header in STAT_COLUMNS {
                ui.label(header);
            }
            ui.end_row();

            for (name, row) in [
                ("HP", &mut character.hp),
                ("Endurance", &mut character.end),
                ("Mana", &mut character.mana),
            ] {
                ui.label(name);
                drag_cells(ui, row);
                ui.end_row();
            }
        });

    ui.separator();
    crate::centered_heading(ui, "Active Values");
    egui::Grid::new("character_active")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            drag_row(ui, "Active HP:", &mut character.a_hp);
            drag_row(ui, "Active Endurance:", &mut character.a_end);
            drag_row(ui, "Active Mana:", &mut character.a_mana);
        });

    ui.separator();
    crate::centered_heading(ui, "Skills");
    egui::Grid::new("character_skills")
        .num_columns(8)
        .spacing([15.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Skill #");
            ui.label("Skill Name");
            for header in STAT_COLUMNS {
                ui.label(header);
            }
            ui.end_row();

            for (i, row) in character.skill.iter_mut().enumerate() {
                crate::centered_label(ui, format!("{}", i));
                ui.label(skills::get_skill_name(i));
                drag_cells(ui, row);
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::KINDRED_FLAGS;

    #[test]
    fn kindred_flags_are_distinct_single_bits() {
        let mut seen = 0u32;
        for (bit, _) in KINDRED_FLAGS {
            assert_eq!(bit.count_ones(), 1);
            assert_eq!(seen & bit, 0);
            seen |= bit;
        }
    }
}
