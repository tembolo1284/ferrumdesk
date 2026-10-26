use crate::theme::{bp, MUTED};
use crate::widgets::{cell, num_cell, Flash};
use egui_extras::{Column, TableBuilder};
use fd_engine::StateSnapshot;
use fd_types::QuoteKind;

pub fn show(ui: &mut egui::Ui, f: &StateSnapshot, flash: &mut Flash) {
    let Some(snap) = &f.snapshot else { ui.label("waiting for first snapshot…"); return; };
    let mut quotes: Vec<_> = snap.quotes.iter().collect();
    quotes.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    ui.horizontal(|ui| {
        ui.label(format!("snapshot #{}", snap.seq));
        ui.label(egui::RichText::new(snap.ts.format("%H:%M:%S%.3f").to_string()).color(MUTED));
        ui.label(format!("{} quotes", quotes.len()));
    });

    TableBuilder::new(ui).striped(true)
        .column(Column::initial(160.0)).column(Column::initial(90.0))
        .column(Column::initial(90.0)).column(Column::initial(90.0)).column(Column::initial(90.0))
        .column(Column::initial(70.0)).column(Column::remainder())
        .header(20.0, |mut h| {
            for t in ["Id", "Kind", "Bid", "Mid", "Ask", "Sprd(bp)", "Age"] { h.col(|ui| { ui.strong(t); }); }
        })
        .body(|mut body| {
            for q in quotes {
                let tint = flash.observe(&q.id, q.mid);
                let fmt = |x: f64| match q.kind {
                    QuoteKind::ParRate | QuoteKind::Yield | QuoteKind::Fixing | QuoteKind::Vol => format!("{:.4}", x * 100.0),
                    QuoteKind::Price | QuoteKind::FuturesPrice => format!("{x:.4}"),
                };
                let age = (snap.ts - q.ts).num_milliseconds();
                body.row(18.0, |mut row| {
                    cell(&mut row, q.id.as_str());
                    cell(&mut row, format!("{:?}", q.kind));
                    num_cell(&mut row, q.bid.map(fmt).unwrap_or_default(), None, None);
                    num_cell(&mut row, fmt(q.mid), tint, None);
                    num_cell(&mut row, q.ask.map(fmt).unwrap_or_default(), None, None);
                    num_cell(&mut row, q.spread().map(bp).unwrap_or_default(), None, Some(MUTED));
                    num_cell(&mut row, format!("{age}ms"), None, Some(MUTED));
                });
            }
        });
}
