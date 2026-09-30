//! Item template / item instance detail editor.

use super::data_fields::{item_data_fields, item_driver_info};
use super::widgets::{
    ATTRIBUTE_NAMES, c_string_row, drag, drag_cells, drag_pair_row, drag_row, flag_grid,
};
use super::{TemplateViewerApp, ViewMode};
use eframe::egui;
use mag_core::constants::{SERVER_MAPX, USE_EMPTY};
use mag_core::types::{Item, Map};
use mag_core::{ranks, skills};
use std::collections::HashMap;
use std::rc::Rc;

/// Maximum rows listed in the "Where used" table.
const WHERE_USED_LIMIT: usize = 500;

/// One "Where used" row: `(item_id, x, y, area)`.
type WhereUsedRow = (u32, u16, u16, String);

/// Map placements `(item_id, x, y)` grouped by template id.
type WhereUsedIndex = HashMap<u16, Vec<(u32, u16, u16)>>;

/// Cached "Where used" lookups so the map isn't rescanned every frame.
///
/// Reset via [`TemplateViewerApp::invalidate_where_used`] whenever map tiles or
/// item instances change.
#[derive(Default)]
pub(super) struct WhereUsedCache {
    /// Map placements `(item_id, x, y)` grouped by template id, from one full-map scan.
    by_template: Option<WhereUsedIndex>,
    /// Sorted rows (with area names) for the most recently shown template.
    rows: Option<(u16, Rc<[WhereUsedRow]>)>,
}

/// Group every live map item placement by its template id.
///
/// # Arguments
///
/// * `map_tiles` - Full map, row-major with width `SERVER_MAPX`.
/// * `items` - Item instances referenced by `Map::it`.
///
/// # Returns
///
/// * `template_id -> [(item_id, x, y)]` in map scan order.
fn build_where_used_index(map_tiles: &[Map], items: &[Item]) -> WhereUsedIndex {
    let tile_w = SERVER_MAPX as usize;
    let mut index: WhereUsedIndex = HashMap::new();
    for (tile_idx, tile) in map_tiles.iter().enumerate() {
        if tile.it == 0 {
            continue;
        }
        let Some(item) = items.get(tile.it as usize) else {
            continue;
        };
        if item.used == USE_EMPTY {
            continue;
        }
        let x = (tile_idx % tile_w) as u16;
        let y = (tile_idx / tile_w) as u16;
        index.entry(item.temp).or_default().push((tile.it, x, y));
    }
    index
}

/// Placement dropdown.
fn placement_combo(ui: &mut egui::Ui, id: u16, placement: &mut u16) {
    egui::ComboBox::from_id_salt(format!("placement_combo_{id}"))
        .selected_text(crate::placement_label(*placement))
        .show_ui(ui, |ui| {
            for (value, name) in crate::placement_options() {
                ui.selectable_value(placement, *value, *name);
            }
        });
}

/// Minimum-rank dropdown (`-1` = none).
fn min_rank_combo(ui: &mut egui::Ui, id: u16, min_rank: &mut i8) {
    egui::ComboBox::from_id_salt(format!("min_rank_combo_{id}"))
        .selected_text(crate::rank_label(*min_rank))
        .show_ui(ui, |ui| {
            if ui.selectable_label(*min_rank < 0, "-1: None").clicked() {
                *min_rank = -1;
            }
            for (idx, name) in ranks::ranks().iter().enumerate() {
                let Ok(rank) = i8::try_from(idx) else {
                    break;
                };
                ui.selectable_value(min_rank, rank, format!("{idx}: {name}"));
            }
        });
}

impl TemplateViewerApp {
    /// Index of the item template for an item id (slot index first, then by `temp`).
    fn find_item_template_index(&self, item_id: u32) -> Option<usize> {
        let index = item_id as usize;
        if index < self.item_templates.len() {
            return Some(index);
        }
        let temp_id = item_id as u16;
        self.item_templates
            .iter()
            .position(|item| item.temp == temp_id)
    }

    /// Edit `table[idx]` (item templates or item instances), marking the slot dirty on change.
    ///
    /// # Arguments
    ///
    /// * `ui` - Target UI.
    /// * `table` - [`ViewMode::ItemTemplates`] or [`ViewMode::Items`].
    /// * `idx` - Slot to edit.
    pub(super) fn render_item_details_by_index(
        &mut self,
        ui: &mut egui::Ui,
        table: ViewMode,
        idx: usize,
    ) {
        // LiveApi templates start as summary-only stubs; fetch the full payload on first view.
        if table == ViewMode::ItemTemplates
            && let Err(e) = self.ensure_item_template_loaded(idx)
        {
            ui.colored_label(egui::Color32::RED, e);
            return;
        }

        let records = match table {
            ViewMode::ItemTemplates => &self.item_templates,
            ViewMode::Items => &self.items,
            ViewMode::CharacterTemplates | ViewMode::Characters => return,
        };
        let Some(before) = records.get(idx).copied() else {
            return;
        };

        let mut item = before;
        self.render_item_details(ui, &mut item, table, idx);
        if item == before {
            return;
        }
        let records = match table {
            ViewMode::ItemTemplates => &mut self.item_templates,
            _ => &mut self.items,
        };
        records[idx] = item;
        self.mark_slot_dirty(table, idx);
    }

    /// Floating window showing the item template for an item id.
    pub(super) fn render_item_popup(&mut self, ctx: &egui::Context) {
        let Some(item_id) = self.item_popup_id else {
            return;
        };

        let mut open = true;
        egui::Window::new(format!("Item {}", item_id))
            .open(&mut open)
            .show(ctx, |ui| match self.find_item_template_index(item_id) {
                Some(idx) => self.render_item_details_by_index(ui, ViewMode::ItemTemplates, idx),
                None => {
                    ui.label(format!("No item template found for ID {}", item_id));
                }
            });

        if !open {
            self.item_popup_id = None;
        }
    }

    /// Centered item-instance id that jumps to the Items tab when clicked.
    fn centered_clickable_item_instance_id(&mut self, ui: &mut egui::Ui, item_id: u32) {
        if item_id == 0 {
            crate::centered_label(ui, "0");
            return;
        }

        let response = ui
            .with_layout(
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| ui.add(egui::Label::new(format!("{}", item_id)).sense(egui::Sense::click())),
            )
            .inner;

        let idx = item_id as usize;
        if response.clicked()
            && self
                .items
                .get(idx)
                .is_some_and(|item| item.used != USE_EMPTY)
        {
            self.selected_item_instance_index = Some(idx);
            self.view_mode = ViewMode::Items;
            self.scroll_to_selection = true;
        }
    }

    /// All editable fields of one item, plus "Where used" for templates.
    fn render_item_details(
        &mut self,
        ui: &mut egui::Ui,
        item: &mut Item,
        table: ViewMode,
        idx: usize,
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(item.get_name());
            ui.separator();
            let id = item.temp;

            egui::Grid::new("item_details")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    drag_row(ui, "Index:", &mut item.temp);
                    drag_row(ui, "Used:", &mut item.used);
                    c_string_row(ui, "Name:", &mut item.name, false);
                    c_string_row(ui, "Reference:", &mut item.reference, false);
                    c_string_row(ui, "Description:", &mut item.description, true);

                    ui.label("Value:");
                    ui.horizontal(|ui| {
                        drag(ui, &mut item.value);
                        ui.label(crate::format_gold_silver(item.value as i32));
                    });
                    ui.end_row();

                    ui.label("Placement:");
                    placement_combo(ui, id, &mut item.placement);
                    ui.end_row();

                    ui.label("Flags:");
                    ui.end_row();
                });

            flag_grid(
                ui,
                format!("item_flags_grid_{id}"),
                &mut item.flags,
                crate::get_item_flag_info()
                    .into_iter()
                    .map(|(flag, name)| (flag.bits(), name)),
                3,
                true,
            );

            egui::Grid::new(format!("item_details_grid2_{id}"))
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for (i, sprite) in item.sprite.iter_mut().enumerate() {
                        ui.label(format!("Sprite[{i}]:"));
                        ui.vertical(|ui| {
                            drag(ui, sprite);
                            self.sprite_cell(ui, (*sprite).max(0) as usize);
                        });
                        ui.end_row();
                    }

                    let [status_0, status_1] = &mut item.status;
                    drag_pair_row(ui, "Status:", status_0, status_1);
                    let [armor_0, armor_1] = &mut item.armor;
                    drag_pair_row(ui, "Armor:", armor_0, armor_1);
                    let [weapon_0, weapon_1] = &mut item.weapon;
                    drag_pair_row(ui, "Weapon:", weapon_0, weapon_1);
                    let [light_0, light_1] = &mut item.light;
                    drag_pair_row(ui, "Light:", light_0, light_1);
                    drag_row(ui, "Duration:", &mut item.duration);
                    drag_row(ui, "Cost:", &mut item.cost);
                    drag_row(ui, "Power:", &mut item.power);

                    ui.label("Min Rank:");
                    min_rank_combo(ui, id, &mut item.min_rank);
                    ui.end_row();

                    ui.label("Driver:");
                    ui.horizontal(|ui| {
                        drag(ui, &mut item.driver);
                        ui.label(item_driver_info(item.driver).0);
                    });
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Attributes");
            egui::Grid::new("item_attributes")
                .num_columns(4)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for header in ["Stat", "Worn", "Active", "Min Required"] {
                        ui.label(header);
                    }
                    ui.end_row();

                    for (name, row) in ATTRIBUTE_NAMES.iter().zip(item.attrib.iter_mut()) {
                        ui.label(*name);
                        drag_cells(ui, row);
                        ui.end_row();
                    }
                    for (name, row) in [
                        ("HP", &mut item.hp),
                        ("Endurance", &mut item.end),
                        ("Mana", &mut item.mana),
                    ] {
                        ui.label(name);
                        drag_cells(ui, row);
                        ui.end_row();
                    }
                });

            ui.separator();
            crate::centered_heading(ui, "Skills");
            egui::Grid::new("item_skills")
                .num_columns(5)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for header in ["Skill #", "Skill Name", "Worn", "Active", "Min Required"] {
                        ui.label(header);
                    }
                    ui.end_row();

                    for (i, row) in item.skill.iter_mut().enumerate() {
                        crate::centered_label(ui, format!("{}", i));
                        ui.label(skills::get_skill_name(i));
                        drag_cells(ui, row);
                        ui.end_row();
                    }
                });

            let defs = item_data_fields(item);
            self.ui_data_fields(ui, "item_driver_data", &mut item.data, &defs);

            if table == ViewMode::ItemTemplates {
                ui.separator();
                // For templates the slot index is the template id; the stored `temp` isn't reliable.
                self.ui_item_where_used(ui, idx as u16);
            }
        });
    }

    /// Drop cached "Where used" results; call after map tiles or item instances change.
    pub(super) fn invalidate_where_used(&mut self) {
        self.where_used = WhereUsedCache::default();
    }

    /// Map placements of `template_id`, sorted by area then position (cached).
    fn where_used_rows(&mut self, template_id: u16) -> Rc<[WhereUsedRow]> {
        if let Some((cached_id, rows)) = &self.where_used.rows
            && *cached_id == template_id
        {
            return Rc::clone(rows);
        }

        let index = self
            .where_used
            .by_template
            .get_or_insert_with(|| build_where_used_index(&self.map_tiles, &self.items));
        let mut rows: Vec<WhereUsedRow> = index
            .get(&template_id)
            .into_iter()
            .flatten()
            .map(|&(item_id, x, y)| {
                let area = mag_core::area::get_area_m(i32::from(x), i32::from(y))
                    .unwrap_or_else(|| "Unknown".to_owned());
                (item_id, x, y, area)
            })
            .collect();
        rows.sort_by(|a, b| a.3.cmp(&b.3).then(a.2.cmp(&b.2)).then(a.1.cmp(&b.1)));

        let rows: Rc<[WhereUsedRow]> = rows.into();
        self.where_used.rows = Some((template_id, Rc::clone(&rows)));
        rows
    }

    /// Collapsible table of map placements for an item template.
    fn ui_item_where_used(&mut self, ui: &mut egui::Ui, template_id: u16) {
        let locations = self.where_used_rows(template_id);
        let total = locations.len();

        egui::CollapsingHeader::new(format!("Where used ({} on map)", total))
            .default_open(total > 0 && total <= 20)
            .show(ui, |ui| {
                if total == 0 {
                    ui.label("No instances of this template were found on the map.");
                    return;
                }
                if total > WHERE_USED_LIMIT {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!("Showing first {} of {} results", WHERE_USED_LIMIT, total),
                    );
                }

                egui::Grid::new(format!("item_where_used_{}", template_id))
                    .num_columns(4)
                    .spacing([20.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        crate::centered_label(ui, "Item");
                        crate::centered_label(ui, "X");
                        crate::centered_label(ui, "Y");
                        ui.label("Area");
                        ui.end_row();

                        for (item_id, x, y, area) in locations.iter().take(WHERE_USED_LIMIT) {
                            self.centered_clickable_item_instance_id(ui, *item_id);
                            crate::centered_label(ui, format!("{}", x));
                            crate::centered_label(ui, format!("{}", y));
                            ui.label(area);
                            ui.end_row();
                        }
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::super::{TemplateViewerApp, ViewMode};
    use mag_core::constants::{SERVER_MAPX, USE_ACTIVE};
    use mag_core::types::{Item, Map};
    use std::rc::Rc;

    #[test]
    fn find_item_template_index_prefers_slot_then_temp() {
        let mut app = TemplateViewerApp {
            item_templates: vec![Item::default(); 3],
            ..Default::default()
        };
        app.item_templates[1].temp = 900;
        assert_eq!(app.find_item_template_index(2), Some(2));
        assert_eq!(app.find_item_template_index(900), Some(1));
        assert_eq!(app.find_item_template_index(901), None);
    }

    /// App with item 1 (template 7) at (5, 1), an unused template-7 item at (6, 0),
    /// and item 3 (template 8) at (7, 0).
    fn app_with_placements() -> TemplateViewerApp {
        let mut app = TemplateViewerApp {
            items: vec![Item::default(); 4],
            map_tiles: vec![Map::default(); SERVER_MAPX as usize * 2],
            ..Default::default()
        };
        app.items[1].used = USE_ACTIVE;
        app.items[1].temp = 7;
        app.items[2].temp = 7;
        app.items[3].used = USE_ACTIVE;
        app.items[3].temp = 8;
        app.map_tiles[SERVER_MAPX as usize + 5].it = 1;
        app.map_tiles[6].it = 2;
        app.map_tiles[7].it = 3;
        app
    }

    #[test]
    fn where_used_rows_finds_live_instances_of_template() {
        let mut app = app_with_placements();
        let found = app.where_used_rows(7);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].0, found[0].1, found[0].2), (1, 5, 1));
        assert!(app.where_used_rows(9).is_empty());
    }

    #[test]
    fn where_used_rows_are_cached_until_invalidated() {
        let mut app = app_with_placements();
        let first = app.where_used_rows(7);
        assert!(Rc::ptr_eq(&first, &app.where_used_rows(7)));

        // Switching templates reuses the map index instead of rescanning.
        assert_eq!(app.where_used_rows(8).len(), 1);
        assert!(app.where_used.by_template.is_some());

        app.items[3].temp = 7;
        app.mark_slot_dirty(ViewMode::Items, 3);
        assert!(app.where_used.by_template.is_none());
        assert_eq!(app.where_used_rows(7).len(), 2);
    }

    #[test]
    fn template_edits_keep_where_used_cache() {
        let mut app = app_with_placements();
        app.where_used_rows(7);
        app.mark_slot_dirty(ViewMode::ItemTemplates, 7);
        assert!(app.where_used.rows.is_some());
    }

    #[test]
    fn edits_mark_only_the_edited_table_dirty() {
        let mut app = TemplateViewerApp {
            items: vec![Item::default(); 2],
            ..Default::default()
        };
        app.mark_slot_dirty(ViewMode::Items, 1);
        assert!(app.dirty);
        assert!(app.dirty_item_slots.contains(&1));
        assert!(app.dirty_item_template_slots.is_empty());
    }
}
