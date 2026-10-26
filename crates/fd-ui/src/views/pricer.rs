use crate::theme::{money, DOWN, MUTED};
use fd_engine::StateSnapshot;
use fd_pricing::{Pricer, Universe};
use fd_types::InstrumentId;

#[derive(Default)]
pub struct PricerPanel { pub instrument: String, pub notional: String }

/// Ad-hoc "what would this price at" — uses the same pricer as the risk loop
/// against the current snapshot, so it's always consistent with the blotter.
pub fn show(ui: &mut egui::Ui, f: &StateSnapshot, pricer: &dyn Pricer, universe: &Universe, st: &mut PricerPanel) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("pricer_inst").selected_text(if st.instrument.is_empty() { "instrument" } else { &st.instrument })
            .show_ui(ui, |ui| {
                let mut ids: Vec<_> = universe.iter().map(|d| d.id.as_str().to_owned()).collect();
                ids.sort();
                for id in ids { ui.selectable_value(&mut st.instrument, id.clone(), id); }
            });
        ui.label("notional"); ui.add(egui::TextEdit::singleline(&mut st.notional).desired_width(110.0));
    });
    if st.notional.is_empty() { st.notional = "10000000".into(); }
    let Some(snap) = &f.snapshot else { ui.label("no snapshot"); return; };
    if st.instrument.is_empty() { return; }
    let n: f64 = st.notional.replace(',', "").parse().unwrap_or(1.0);

    match pricer.price(&InstrumentId::new(st.instrument.as_str()), snap) {
        Err(e) => { ui.colored_label(DOWN, e.to_string()); }
        Ok(r) => {
            let s = r.scaled(n);
            egui::Grid::new("pricer_out").striped(true).show(ui, |ui| {
                ui.label("PV"); ui.monospace(money(s.pv)); ui.end_row();
                ui.label("DV01 (parallel)"); ui.monospace(money(s.dv01)); ui.end_row();
                if let Some(v) = s.vega { ui.label("Vega /bp"); ui.monospace(money(v)); ui.end_row(); }
                if let Some(q) = r.fair_quote { ui.label("Fair quote"); ui.monospace(format!("{q:.6}")); ui.end_row(); }
                ui.label("Elapsed"); ui.label(egui::RichText::new(format!("{} µs", r.elapsed_us)).color(MUTED)); ui.end_row();
            });
            ui.add_space(4.0);
            ui.strong("Bucketed DV01");
            egui::Grid::new("pricer_ladder").striped(true).show(ui, |ui| {
                for (t, v) in &s.bucketed.buckets { ui.monospace(t.to_string()); ui.monospace(money(*v)); ui.end_row(); }
                ui.monospace("Σ"); ui.monospace(money(s.bucketed.total())); ui.end_row();
            });
        }
    }
}
