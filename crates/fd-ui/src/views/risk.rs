use crate::theme::{money, pnl_color};
use crate::widgets::{cell, num_cell};
use egui_extras::{Column, TableBuilder};
use fd_engine::StateSnapshot;
use fd_pricing::Universe;
use fd_portfolio::aggregate::{rollup, RollupKey};
use fd_types::Tenor;

pub fn show(ui: &mut egui::Ui, f: &StateSnapshot, universe: &Universe, curve_name: &str) {
    let tenors: Vec<Tenor> = f.snapshot.as_ref().and_then(|s| s.curve(curve_name)).map(|c| c.node_tenors()).unwrap_or_default();
    if tenors.is_empty() { ui.label("no curve yet"); return; }

    let rows = f.book.open_positions().filter_map(|p| {
        let r = f.risk.get(&p.id)?.as_ref().ok()?;
        let kind = universe.get(&p.instrument)?.instrument.kind();
        Some((p, r, kind))
    });
    let agg = rollup(rows);

    TableBuilder::new(ui).striped(true)
        .column(Column::initial(200.0))
        .columns(Column::initial(78.0), tenors.len())
        .column(Column::initial(90.0)).column(Column::initial(90.0)).column(Column::remainder())
        .header(20.0, |mut h| {
            h.col(|ui| { ui.strong("Bucket"); });
            for t in &tenors { h.col(|ui| { ui.strong(t.to_string()); }); }
            h.col(|ui| { ui.strong("Total DV01"); });
            h.col(|ui| { ui.strong("Vega"); });
            h.col(|ui| { ui.strong("PV"); });
        })
        .body(|mut body| {
            for (key, a) in &agg {
                let label = match key {
                    RollupKey::Desk => "DESK".to_string(),
                    RollupKey::Book(b) => format!("  book {b}"),
                    RollupKey::Product(k) => format!("  product {k:?}"),
                    RollupKey::Instrument(i) => format!("    {i}"),
                };
                body.row(18.0, |mut row| {
                    cell(&mut row, label);
                    for (_, v) in a.ladder(&tenors) {
                        num_cell(&mut row, if v.abs() < 0.5 { "·".into() } else { money(v) }, None, Some(pnl_color(v)));
                    }
                    num_cell(&mut row, money(a.dv01), None, Some(pnl_color(a.dv01)));
                    num_cell(&mut row, money(a.vega), None, None);
                    num_cell(&mut row, money(a.pv), None, None);
                });
            }
        });
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Ladder = par-quote bucketed DV01 (Jacobian); Total = parallel zero-rate DV01. They differ slightly by construction.").color(crate::theme::MUTED));
}
