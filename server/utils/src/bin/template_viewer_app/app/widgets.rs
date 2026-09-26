//! Small shared editor widgets for the template/instance detail panels.

use eframe::egui;
use egui::emath::Numeric;
use mag_core::string_operations::c_string_to_str;

/// Row labels for the five base attributes, in `attrib` index order.
pub(super) const ATTRIBUTE_NAMES: [&str; 5] =
    ["Bravery", "Willpower", "Intuition", "Agility", "Strength"];

/// Integer drag widget with unit speed; the type's own range bounds the value.
pub(super) fn drag<T: Numeric>(ui: &mut egui::Ui, value: &mut T) -> egui::Response {
    ui.add(egui::DragValue::new(value).speed(1))
}

/// Grid row: label + one drag value.
pub(super) fn drag_row<T: Numeric>(ui: &mut egui::Ui, label: &str, value: &mut T) {
    ui.label(label);
    drag(ui, value);
    ui.end_row();
}

/// Grid row: label + two drag values side by side.
pub(super) fn drag_pair_row<T: Numeric>(ui: &mut egui::Ui, label: &str, a: &mut T, b: &mut T) {
    ui.label(label);
    ui.horizontal(|ui| {
        drag(ui, a);
        drag(ui, b);
    });
    ui.end_row();
}

/// Grid row: label + drag value with an explanatory note underneath.
pub(super) fn drag_note_row<T: Numeric>(ui: &mut egui::Ui, label: &str, value: &mut T, note: &str) {
    ui.label(label);
    ui.vertical(|ui| {
        drag(ui, value);
        ui.label(note);
    });
    ui.end_row();
}

/// One drag cell per element, for table rows (attributes, skills, vitals).
pub(super) fn drag_cells<T: Numeric>(ui: &mut egui::Ui, values: &mut [T]) {
    for value in values {
        drag(ui, value);
    }
}

/// Grid row editing a NUL-terminated fixed-size string buffer.
///
/// # Arguments
///
/// * `ui` - Target grid UI.
/// * `label` - Row label.
/// * `buf` - Fixed-size C string buffer; only rewritten when the text changes.
/// * `multiline` - Use a 3-row multi-line editor instead of a single line.
pub(super) fn c_string_row(ui: &mut egui::Ui, label: &str, buf: &mut [u8], multiline: bool) {
    ui.label(label);
    let mut text = c_string_to_str(buf).to_owned();
    let editor = if multiline {
        egui::TextEdit::multiline(&mut text).desired_rows(3)
    } else {
        egui::TextEdit::singleline(&mut text)
    };
    if ui.add(editor.desired_width(240.0)).changed() {
        crate::write_c_string(buf, &text);
    }
    ui.end_row();
}

/// Checkbox grid toggling `mask` bits within `bits`.
///
/// # Arguments
///
/// * `ui` - Target UI.
/// * `id` - Unique grid id.
/// * `bits` - Flag word being edited.
/// * `entries` - `(mask, label)` pairs, one checkbox each.
/// * `columns` - Checkboxes per row.
/// * `striped` - Whether to stripe rows.
pub(super) fn flag_grid<'a>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    bits: &mut u64,
    entries: impl IntoIterator<Item = (u64, &'a str)>,
    columns: usize,
    striped: bool,
) {
    egui::Grid::new(id)
        .num_columns(columns)
        .spacing([10.0, 4.0])
        .striped(striped)
        .show(ui, |ui| {
            let mut col = 0;
            for (mask, label) in entries {
                let mut on = (*bits & mask) == mask;
                if ui.checkbox(&mut on, label).changed() {
                    if on {
                        *bits |= mask;
                    } else {
                        *bits &= !mask;
                    }
                }
                col += 1;
                if col == columns {
                    ui.end_row();
                    col = 0;
                }
            }
            if col != 0 {
                ui.end_row();
            }
        });
}

/// "Driver Data" section: non-zero `data[i]` fields, or all of them when `show_all` is set.
///
/// # Arguments
///
/// * `ui` - Target UI.
/// * `id` - Unique grid id.
/// * `data` - Driver data slots.
/// * `show_all` - Persistent "show all fields" toggle.
/// * `spacing` - Horizontal grid spacing.
pub(super) fn driver_data_section<T: Numeric + Default + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    data: &mut [T],
    show_all: &mut bool,
    spacing: f32,
) {
    ui.separator();
    crate::centered_heading(ui, "Driver Data");
    ui.horizontal(|ui| {
        ui.checkbox(show_all, "Show all possible data fields");
    });
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([spacing, 4.0])
        .striped(true)
        .show(ui, |ui| {
            let mut shown_any = false;
            for (i, value) in data.iter_mut().enumerate() {
                if !*show_all && *value == T::default() {
                    continue;
                }
                shown_any = true;
                drag_row(ui, &format!("data[{i}]:"), value);
            }
            if !shown_any && let Some(first) = data.first_mut() {
                drag_row(ui, "data[0]:", first);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{c_string_row, driver_data_section, flag_grid};
    use eframe::egui;

    /// Run `f` inside a headless egui frame.
    fn with_ui(f: impl FnOnce(&mut egui::Ui)) {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(f) = f.take() {
                    f(ui);
                }
            });
        });
    }

    #[test]
    fn widgets_leave_values_untouched_without_input() {
        let mut bits = 0b1010_u64;
        let mut name = [0u8; 8];
        crate::write_c_string(&mut name, "abc");
        name[6] = 0xFF; // Garbage past the terminator must survive a no-op render.
        let mut data = [0u32, 5, 0];
        let mut show_all = false;

        with_ui(|ui| {
            flag_grid(ui, "flags", &mut bits, [(0b10, "a"), (0b100, "b")], 2, true);
            egui::Grid::new("text").show(ui, |ui| c_string_row(ui, "Name", &mut name, false));
            driver_data_section(ui, "data", &mut data, &mut show_all, 20.0);
        });

        assert_eq!(bits, 0b1010);
        assert_eq!(name[6], 0xFF);
        assert_eq!(data, [0, 5, 0]);
        assert!(!show_all);
    }
}
