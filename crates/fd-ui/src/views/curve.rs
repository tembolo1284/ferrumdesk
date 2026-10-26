use crate::theme::ACCENT;
use egui_plot::{Line, Plot, PlotPoints, Points};
use fd_engine::StateSnapshot;
use fd_market::curve::{add_tenor, year_frac};

pub fn show(ui: &mut egui::Ui, f: &StateSnapshot, curve_name: &str) {
    let Some(snap) = &f.snapshot else { ui.label("no snapshot"); return; };
    let Some(curve) = snap.curve(curve_name) else { ui.label(format!("no curve {curve_name}")); return; };
    let r = curve.reference_date();

    // dense zero curve + node markers
    let max_t = curve.node_tenors().iter().map(|t| year_frac(r, add_tenor(r, *t))).fold(1.0, f64::max);
    let dense: PlotPoints = (1..=200).map(|i| {
        let t = max_t * i as f64 / 200.0;
        let d = r + chrono::Duration::days((t * 365.0) as i64);
        [t, curve.zero_rate(d) * 100.0]
    }).collect();
    let nodes: PlotPoints = curve.node_tenors().iter().map(|tn| {
        let d = add_tenor(r, *tn);
        [year_frac(r, d), curve.zero_rate(d) * 100.0]
    }).collect();

    ui.label(format!("{curve_name} — zero curve (cc, Act/365), ref {r}"));
    Plot::new("zero_curve").height(320.0).x_axis_label("years").y_axis_label("%")
        .show(ui, |p| {
            p.line(Line::new(dense).color(ACCENT).name("zero"));
            p.points(Points::new(nodes).radius(3.5).name("nodes"));
        });

    egui::Grid::new("curve_nodes").striped(true).show(ui, |ui| {
        ui.strong("Tenor"); ui.strong("Date"); ui.strong("DF"); ui.strong("Zero %"); ui.end_row();
        for tn in curve.node_tenors() {
            let d = add_tenor(r, tn);
            ui.monospace(tn.to_string()); ui.monospace(d.to_string());
            ui.monospace(format!("{:.6}", curve.discount(d)));
            ui.monospace(format!("{:.4}", curve.zero_rate(d) * 100.0));
            ui.end_row();
        }
    });
}
