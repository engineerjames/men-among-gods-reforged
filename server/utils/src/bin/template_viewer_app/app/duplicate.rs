//! "Duplicate to free slot" for item and character templates.

use super::{TemplateViewerApp, ViewMode};
use eframe::egui;
use mag_core::constants::USE_EMPTY;

/// Suffix appended to a duplicated template's name.
const DUPLICATE_SUFFIX: &str = " DUPLICATE";

/// Which template table a duplicate request targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TemplateKind {
    Item,
    Character,
}

/// Context-menu entry for duplicating a template.
///
/// # Arguments
///
/// * `ui` - Context menu UI.
/// * `enabled` - Whether the source slot is in use (empty slots can't be duplicated).
///
/// # Returns
///
/// * `true` when the entry was clicked (the menu is closed).
pub(super) fn duplicate_menu_button(ui: &mut egui::Ui, enabled: bool) -> bool {
    let clicked = ui
        .add_enabled(enabled, egui::Button::new("Duplicate to free slot"))
        .clicked();
    if clicked {
        ui.close_menu();
    }
    clicked
}

/// Name for a duplicate, shortening the original so the suffix fits a NUL-terminated buffer.
///
/// # Arguments
///
/// * `original` - Source template name.
/// * `capacity` - Size of the fixed name buffer in bytes, including the NUL terminator.
///
/// # Returns
///
/// * `original` + [`DUPLICATE_SUFFIX`], truncated on a char boundary when needed.
fn duplicate_name(original: &str, capacity: usize) -> String {
    let max_base = capacity
        .saturating_sub(1)
        .saturating_sub(DUPLICATE_SUFFIX.len());
    let mut end = original.len().min(max_base);
    while !original.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{DUPLICATE_SUFFIX}", original[..end].trim_end())
}

/// Lowest unused template slot, skipping slot 0 (template id 0 means "none").
///
/// # Arguments
///
/// * `used` - The `used` byte of every slot, in slot order.
///
/// # Returns
///
/// * The free slot index, or `None` when the table is full.
fn first_free_slot(used: impl Iterator<Item = u8>) -> Option<usize> {
    used.enumerate()
        .skip(1)
        .find(|(_, used)| *used == USE_EMPTY)
        .map(|(idx, _)| idx)
}

impl TemplateViewerApp {
    /// Copy a template into the next free slot, select it, and report the new id.
    ///
    /// The copy is an unsaved edit: it is marked dirty like any other change.
    ///
    /// # Arguments
    ///
    /// * `kind` - Item or character template table.
    /// * `source` - Slot to duplicate.
    pub(super) fn duplicate_template(&mut self, kind: TemplateKind, source: usize) {
        let (label, result) = match kind {
            TemplateKind::Item => ("item", self.duplicate_item_template(source)),
            TemplateKind::Character => ("character", self.duplicate_character_template(source)),
        };
        self.save_status = Some(match result {
            Ok(target) => {
                self.scroll_to_selection = true;
                format!(
                    "Duplicated {label} template {source} to slot {target} (unsaved, now selected)"
                )
            }
            Err(e) => format!("Duplicate failed: {e}"),
        });
    }

    /// Duplicate one item template; see [`Self::duplicate_template`].
    ///
    /// # Returns
    ///
    /// * The slot the copy was written to.
    fn duplicate_item_template(&mut self, source: usize) -> Result<usize, String> {
        self.ensure_item_template_loaded(source)?;
        let template = *self
            .item_templates
            .get(source)
            .ok_or_else(|| format!("Item template {source} is out of range"))?;
        if template.used == USE_EMPTY {
            return Err(format!("Item template {source} is unused"));
        }
        let target = first_free_slot(self.item_templates.iter().map(|t| t.used))
            .ok_or_else(|| "No free item template slots".to_owned())?;

        let mut copy = template;
        let name = duplicate_name(template.get_name(), copy.name.len());
        crate::write_c_string(&mut copy.name, &name);
        copy.temp = u16::try_from(target).map_err(|_| format!("Slot {target} exceeds u16"))?;

        self.item_templates[target] = copy;
        // The copy is authoritative locally; don't let the LiveApi lazy-fetch overwrite it.
        self.fully_loaded_item_slots.insert(target);
        self.mark_slot_dirty(ViewMode::ItemTemplates, target);
        self.selected_item_index = Some(target);
        if !self.item_filter.is_empty()
            && !copy
                .get_name()
                .to_lowercase()
                .contains(&self.item_filter.to_lowercase())
        {
            self.item_filter.clear();
        }
        Ok(target)
    }

    /// Duplicate one character template; see [`Self::duplicate_template`].
    ///
    /// # Returns
    ///
    /// * The slot the copy was written to.
    fn duplicate_character_template(&mut self, source: usize) -> Result<usize, String> {
        self.ensure_character_template_loaded(source)?;
        let template = *self
            .character_templates
            .get(source)
            .ok_or_else(|| format!("Character template {source} is out of range"))?;
        if template.used == USE_EMPTY {
            return Err(format!("Character template {source} is unused"));
        }
        let target = first_free_slot(self.character_templates.iter().map(|t| t.used))
            .ok_or_else(|| "No free character template slots".to_owned())?;

        let mut copy = template;
        let name = duplicate_name(template.get_name(), copy.name.len());
        crate::write_c_string(&mut copy.name, &name);
        copy.temp = u16::try_from(target).map_err(|_| format!("Slot {target} exceeds u16"))?;

        self.character_templates[target] = copy;
        self.fully_loaded_char_slots.insert(target);
        self.mark_slot_dirty(ViewMode::CharacterTemplates, target);
        self.selected_character_index = Some(target);
        if !self.character_filter.is_empty()
            && !copy
                .get_name()
                .to_lowercase()
                .contains(&self.character_filter.to_lowercase())
        {
            self.character_filter.clear();
        }
        Ok(target)
    }

    /// In LiveApi mode, replace a summary-only item template stub with its full payload.
    ///
    /// # Returns
    ///
    /// * `Err` with a user-facing message when the fetch fails.
    pub(super) fn ensure_item_template_loaded(&mut self, idx: usize) -> Result<(), String> {
        if !self.data_source.is_live_api() || self.fully_loaded_item_slots.contains(&idx) {
            return Ok(());
        }
        let Some(client) = self.admin_client.as_ref().cloned() else {
            return Ok(());
        };
        let item = client
            .fetch_single_item_template(idx)
            .map_err(|e| format!("Failed to load item template {idx}: {e}"))?;
        if let Some(slot) = self.item_templates.get_mut(idx) {
            *slot = item;
        }
        self.fully_loaded_item_slots.insert(idx);
        Ok(())
    }

    /// In LiveApi mode, replace a summary-only character template stub with its full payload.
    ///
    /// # Returns
    ///
    /// * `Err` with a user-facing message when the fetch fails.
    pub(super) fn ensure_character_template_loaded(&mut self, idx: usize) -> Result<(), String> {
        if !self.data_source.is_live_api() || self.fully_loaded_char_slots.contains(&idx) {
            return Ok(());
        }
        let Some(client) = self.admin_client.as_ref().cloned() else {
            return Ok(());
        };
        let character = client
            .fetch_single_character_template(idx)
            .map_err(|e| format!("Failed to load character template {idx}: {e}"))?;
        if let Some(slot) = self.character_templates.get_mut(idx) {
            *slot = character;
        }
        self.fully_loaded_char_slots.insert(idx);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::TemplateViewerApp;
    use super::{DUPLICATE_SUFFIX, TemplateKind, duplicate_name, first_free_slot};
    use mag_core::constants::{USE_ACTIVE, USE_EMPTY};
    use mag_core::types::{Character, Item};

    #[test]
    fn duplicate_name_appends_suffix() {
        assert_eq!(duplicate_name("Sword", 40), "Sword DUPLICATE");
    }

    #[test]
    fn duplicate_name_truncates_long_names_to_fit() {
        let long = "x".repeat(60);
        let name = duplicate_name(&long, 40);
        assert_eq!(name.len(), 39);
        assert!(name.ends_with(DUPLICATE_SUFFIX));
    }

    #[test]
    fn duplicate_name_truncates_on_char_boundary() {
        let name = duplicate_name(&"é".repeat(30), 40);
        assert!(name.len() <= 39);
        assert!(name.ends_with(DUPLICATE_SUFFIX));
    }

    #[test]
    fn first_free_slot_skips_slot_zero_and_used_slots() {
        let used = [USE_EMPTY, USE_ACTIVE, USE_EMPTY, USE_EMPTY];
        assert_eq!(first_free_slot(used.into_iter()), Some(2));
        assert_eq!(first_free_slot([USE_EMPTY, USE_ACTIVE].into_iter()), None);
    }

    fn app_with_templates() -> TemplateViewerApp {
        let mut app = TemplateViewerApp {
            item_templates: vec![Item::default(); 5],
            character_templates: vec![Character::default(); 5],
            ..Default::default()
        };
        app.item_templates[1].used = USE_ACTIVE;
        app.item_templates[2].used = USE_ACTIVE;
        crate::write_c_string(&mut app.item_templates[2].name, "Sword");
        app.item_templates[2].value = 123;
        app.character_templates[1].used = USE_ACTIVE;
        crate::write_c_string(&mut app.character_templates[1].name, "Guard");
        app
    }

    #[test]
    fn duplicate_item_template_copies_into_first_free_slot_and_selects_it() {
        let mut app = app_with_templates();
        app.duplicate_template(TemplateKind::Item, 2);

        let copy = app.item_templates[3];
        assert_eq!(copy.get_name(), "Sword DUPLICATE");
        assert_eq!(copy.value, 123);
        assert_eq!(copy.temp, 3);
        assert_eq!(app.item_templates[2].get_name(), "Sword");
        assert_eq!(app.selected_item_index, Some(3));
        assert!(app.dirty);
        assert!(app.dirty_item_template_slots.contains(&3));
        assert!(
            app.save_status
                .as_deref()
                .is_some_and(|s| s.contains("slot 3"))
        );
    }

    #[test]
    fn duplicate_character_template_copies_into_first_free_slot() {
        let mut app = app_with_templates();
        app.duplicate_template(TemplateKind::Character, 1);

        assert_eq!(app.character_templates[2].get_name(), "Guard DUPLICATE");
        assert_eq!(app.character_templates[2].temp, 2);
        assert_eq!(app.selected_character_index, Some(2));
        assert!(app.dirty_character_template_slots.contains(&2));
    }

    #[test]
    fn duplicate_clears_filter_that_would_hide_the_copy() {
        let mut app = app_with_templates();
        app.item_filter = "sword".to_owned();
        app.duplicate_template(TemplateKind::Item, 2);
        assert_eq!(app.item_filter, "sword");

        let long = "y".repeat(39);
        crate::write_c_string(&mut app.item_templates[1].name, &long);
        app.item_filter = long;
        app.duplicate_template(TemplateKind::Item, 1);
        assert!(app.item_filter.is_empty());
    }

    #[test]
    fn duplicate_of_unused_or_full_table_reports_error() {
        let mut app = app_with_templates();
        app.duplicate_template(TemplateKind::Item, 4);
        assert!(
            app.save_status
                .as_deref()
                .is_some_and(|s| s.starts_with("Duplicate failed"))
        );
        assert!(!app.dirty);

        for slot in &mut app.character_templates {
            slot.used = USE_ACTIVE;
        }
        app.duplicate_template(TemplateKind::Character, 1);
        assert!(
            app.save_status
                .as_deref()
                .is_some_and(|s| s.contains("No free character template slots"))
        );
    }
}
