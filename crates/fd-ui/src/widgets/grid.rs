use egui::{Color32, RichText, Ui};
use egui_extras::TableRow;

pub fn cell(row: &mut TableRow, text: impl Into<String>) {
    row.col(|ui| { ui.monospace(text.into()); });
}

pub fn cell_colored(row: &mut TableRow, text: impl Into<String>, color: Color32) {
    row.col(|ui| { ui.label(RichText::new(text.into()).monospace().color(color)); });
}

/// Right-aligned numeric cell with optional background tint (flash).
pub fn num_cell(row: &mut TableRow, text: impl Into<String>, tint: Option<Color32>, fg: Option<Color32>) {
    row.col(|ui| {
        if let Some(t) = tint {
            let r = ui.available_rect_before_wrap();
            ui.painter().rect_filled(r, 2.0, t);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut Ui| {
            let mut rt = RichText::new(text.into()).monospace();
            if let Some(c) = fg { rt = rt.color(c); }
            ui.label(rt);
        });
    });
}
