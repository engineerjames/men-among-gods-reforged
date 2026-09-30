//! Filterable record lists (left panel) for all four tabs.

use super::duplicate::{TemplateKind, duplicate_menu_button};
use super::{TemplateViewerApp, ViewMode};
use eframe::egui;
use mag_core::constants::{CharacterFlags, USE_EMPTY};
use mag_core::types::{Character, Item};

/// Minimal view of a list record.
trait Record {
    /// The record's `used` state.
    fn used(&self) -> u8;
    /// The record's display name.
    fn name(&self) -> &str;
}

impl Record for Item {
    fn used(&self) -> u8 {
        self.used
    }
    fn name(&self) -> &str {
        self.get_name()
    }
}

impl Record for Character {
    fn used(&self) -> u8 {
        self.used
    }
    fn name(&self) -> &str {
        self.get_name()
    }
}

/// One visible list row.
#[derive(Debug, PartialEq, Eq)]
struct ListRow {
    idx: usize,
    label: String,
    used: bool,
}

/// Rows for `records` that pass the unused toggle, `include`, and a case-insensitive label filter.
///
/// # Arguments
///
/// * `records` - Table to list.
/// * `show_unused` - Include `USE_EMPTY` slots.
/// * `filter` - Substring matched against `"[idx] name"`; empty matches everything.
/// * `include` - Extra per-record predicate (e.g. "player only").
///
/// # Returns
///
/// * Matching rows in slot order.
fn list_rows<T: Record>(
    records: &[T],
    show_unused: bool,
    filter: &str,
    include: impl Fn(&T) -> bool,
) -> Vec<ListRow> {
    let filter = filter.to_lowercase();
    records
        .iter()
        .enumerate()
        .filter(|(_, record)| (show_unused || record.used() != USE_EMPTY) && include(record))
        .filter_map(|(idx, record)| {
            let name = record.name();
            let label = if name.is_empty() {
                format!("[{idx}] <empty>")
            } else {
                format!("[{idx}] {name}")
            };
            (filter.is_empty() || label.to_lowercase().contains(&filter)).then_some(ListRow {
                idx,
                label,
                used: record.used() != USE_EMPTY,
            })
        })
        .collect()
}

/// Number of non-empty slots in a table.
fn used_count<T: Record>(records: &[T]) -> usize {
    records.iter().filter(|r| r.used() != USE_EMPTY).count()
}

impl TemplateViewerApp {
    /// Heading for the active list, e.g. `Item Templates (120/4096)`.
    pub(super) fn list_heading(&self, mode: ViewMode) -> String {
        let (used, total, show_unused) = match mode {
            ViewMode::ItemTemplates => (
                used_count(&self.item_templates),
                self.item_templates.len(),
                self.show_unused_templates,
            ),
            ViewMode::CharacterTemplates => (
                used_count(&self.character_templates),
                self.character_templates.len(),
                self.show_unused_templates,
            ),
            ViewMode::Items => (
                used_count(&self.items),
                self.items.len(),
                self.show_unused_instances,
            ),
            ViewMode::Characters => (
                used_count(&self.characters),
                self.characters.len(),
                self.show_unused_instances,
            ),
        };
        if show_unused {
            format!("{} ({used}/{total})", mode.title())
        } else {
            format!("{} ({used})", mode.title())
        }
    }

    /// Filter controls plus the scrollable, selectable record list for `mode`.
    pub(super) fn render_list(&mut self, ui: &mut egui::Ui, mode: ViewMode) {
        let filter = match mode {
            ViewMode::ItemTemplates => &mut self.item_filter,
            ViewMode::CharacterTemplates => &mut self.character_filter,
            ViewMode::Items => &mut self.item_instance_filter,
            ViewMode::Characters => &mut self.character_instance_filter,
        };
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(filter);
        });
        ui.horizontal(|ui| {
            if mode == ViewMode::Characters {
                ui.checkbox(&mut self.character_instances_player_only, "Player only");
            }
            let show_unused = if mode.is_template() {
                &mut self.show_unused_templates
            } else {
                &mut self.show_unused_instances
            };
            ui.checkbox(show_unused, "Show unused");
        });
        ui.separator();

        let player_only = self.character_instances_player_only;
        let rows = match mode {
            ViewMode::ItemTemplates => list_rows(
                &self.item_templates,
                self.show_unused_templates,
                &self.item_filter,
                |_| true,
            ),
            ViewMode::CharacterTemplates => list_rows(
                &self.character_templates,
                self.show_unused_templates,
                &self.character_filter,
                |_| true,
            ),
            ViewMode::Items => list_rows(
                &self.items,
                self.show_unused_instances,
                &self.item_instance_filter,
                |_| true,
            ),
            ViewMode::Characters => list_rows(
                &self.characters,
                self.show_unused_instances,
                &self.character_instance_filter,
                |c| !player_only || (c.flags & CharacterFlags::Player.bits()) != 0,
            ),
        };

        let selected = self.selected_index(mode);
        let mut clicked = None;
        let mut duplicate = None;
        let list_width = ui.available_width();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_min_width(list_width);
                for row in &rows {
                    let is_selected = selected == Some(row.idx);
                    let response = ui.selectable_label(is_selected, &row.label);
                    if response.clicked() {
                        clicked = Some(row.idx);
                    }
                    if is_selected && self.scroll_to_selection {
                        response.scroll_to_me(Some(egui::Align::Center));
                        self.scroll_to_selection = false;
                    }
                    if mode.is_template() {
                        response.context_menu(|ui| {
                            if duplicate_menu_button(ui, row.used) {
                                duplicate = Some(row.idx);
                            }
                        });
                    }
                }
            });

        if let Some(idx) = clicked {
            *self.selected_index_mut(mode) = Some(idx);
        }
        if let Some(idx) = duplicate {
            let kind = if mode == ViewMode::ItemTemplates {
                TemplateKind::Item
            } else {
                TemplateKind::Character
            };
            self.duplicate_template(kind, idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ListRow, list_rows, used_count};
    use mag_core::constants::{USE_ACTIVE, USE_EMPTY, USE_NONACTIVE};
    use mag_core::types::Item;

    fn items() -> Vec<Item> {
        let mut items = vec![Item::default(); 4];
        items[1].used = USE_ACTIVE;
        crate::write_c_string(&mut items[1].name, "Iron Sword");
        items[2].used = USE_NONACTIVE;
        crate::write_c_string(&mut items[2].name, "Shield");
        crate::write_c_string(&mut items[3].name, "Old Sword");
        items
    }

    #[test]
    fn list_rows_hides_unused_unless_requested() {
        let items = items();
        let visible: Vec<usize> = list_rows(&items, false, "", |_| true)
            .iter()
            .map(|r| r.idx)
            .collect();
        assert_eq!(visible, vec![1, 2]);
        assert_eq!(list_rows(&items, true, "", |_| true).len(), 4);
    }

    #[test]
    fn list_rows_filters_case_insensitively_on_label() {
        let items = items();
        let rows = list_rows(&items, true, "SWORD", |_| true);
        assert_eq!(rows.iter().map(|r| r.idx).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(
            list_rows(&items, true, "[2]", |_| true),
            vec![ListRow {
                idx: 2,
                label: "[2] Shield".to_owned(),
                used: true
            }]
        );
    }

    #[test]
    fn list_rows_labels_empty_names_and_applies_predicate() {
        let items = items();
        let rows = list_rows(&items, true, "", |item| item.used == USE_EMPTY);
        assert_eq!(rows[0].label, "[0] <empty>");
        assert!(!rows[0].used);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn used_count_counts_every_non_empty_slot() {
        assert_eq!(used_count(&items()), 2);
    }
}
