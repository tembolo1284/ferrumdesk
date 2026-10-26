use crate::theme::{DOWN, MUTED};
use fd_engine::EngineState;

pub fn show(ui: &mut egui::Ui, state: &EngineState) {
    let g = state.read();
    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
        for l in g.log.iter() {
            let c = match l.level { tracing::Level::ERROR | tracing::Level::WARN => DOWN, _ => MUTED };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(l.ts.format("%H:%M:%S%.3f").to_string()).monospace().color(MUTED));
                ui.label(egui::RichText::new(l.msg.as_str()).monospace().color(c));
            });
        }
    });
}
