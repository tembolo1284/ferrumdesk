use crate::theme::{money, pnl_color, DOWN, MUTED};
use crate::widgets::{cell, cell_colored, num_cell};
use egui_extras::{Column, TableBuilder};
use fd_engine::{Engine, StateSnapshot};
use fd_pricing::Universe;
use fd_types::{Side, Trade, TradeId};

/// Trade-entry form state (lives in the app, not the engine).
pub struct TradeEntry {
    pub instrument: String,
    pub side: Side,
    pub quantity: String,
    pub price: String,
    pub book: String,
    pub trader: String,
    pub last_error: Option<String>,
}

impl Default for TradeEntry {
    fn default() -> Self {
        Self { instrument: String::new(), side: Side::Buy, quantity: "10000000".into(),
               price: String::new(), book: "RATES".into(), trader: "paul".into(), last_error: None }
    }
}

pub fn show(ui: &mut egui::Ui, f: &StateSnapshot, engine: &Engine, universe: &Universe, entry: &mut TradeEntry) {
    // ---- totals strip
    let p = &f.pnl;
    ui.horizontal(|ui| {
        ui.label("PV"); ui.monospace(money(p.total_pv));
        ui.separator();
        ui.label("DV01"); ui.label(egui::RichText::new(money(p.total_dv01)).monospace().color(pnl_color(p.total_dv01)));
        ui.separator();
        ui.label("Unrealized"); ui.label(egui::RichText::new(money(p.total_unrealized)).monospace().color(pnl_color(p.total_unrealized)));
        ui.separator();
        ui.label("Realized"); ui.label(egui::RichText::new(money(p.total_realized)).monospace().color(pnl_color(p.total_realized)));
        if p.errors > 0 { ui.separator(); ui.colored_label(DOWN, format!("{} pricing errors", p.errors)); }
        ui.separator();
        ui.label(egui::RichText::new(format!("risk @ snap #{}", f.risk_seq)).color(MUTED));
    });
    ui.separator();

    // ---- trade entry
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("inst").selected_text(if entry.instrument.is_empty() { "instrument" } else { &entry.instrument })
            .show_ui(ui, |ui| {
                let mut ids: Vec<_> = universe.iter().map(|d| d.id.as_str().to_owned()).collect();
                ids.sort();
                for id in ids { ui.selectable_value(&mut entry.instrument, id.clone(), id); }
            });
        ui.selectable_value(&mut entry.side, Side::Buy, "Buy");
        ui.selectable_value(&mut entry.side, Side::Sell, "Sell");
        ui.label("qty"); ui.add(egui::TextEdit::singleline(&mut entry.quantity).desired_width(100.0));
        ui.label("px"); ui.add(egui::TextEdit::singleline(&mut entry.price).desired_width(80.0));
        ui.label("book"); ui.add(egui::TextEdit::singleline(&mut entry.book).desired_width(70.0));
        if ui.button("Book").clicked() {
            entry.last_error = None;
            let parsed = (entry.quantity.trim().replace(',', "").parse::<f64>(), entry.price.trim().parse::<f64>());
            match parsed {
                (Ok(q), Ok(px)) if !entry.instrument.is_empty() => {
                    let t = Trade { id: TradeId::new(), instrument: entry.instrument.as_str().into(), side: entry.side,
                        quantity: q, price: px, ts: chrono::Utc::now(), trader: entry.trader.clone(), book: entry.book.clone() };
                    if let Err(e) = engine.book_trade(t) { entry.last_error = Some(e.to_string()); }
                }
                _ => entry.last_error = Some("need instrument, numeric qty and price".into()),
            }
        }
        if let Some(e) = &entry.last_error { ui.colored_label(DOWN, e); }
    });
    ui.separator();

    // ---- blotter
    TableBuilder::new(ui).striped(true)
        .column(Column::initial(90.0)).column(Column::initial(60.0)).column(Column::initial(120.0))
        .column(Column::initial(110.0)).column(Column::initial(90.0)).column(Column::initial(100.0))
        .column(Column::initial(110.0)).column(Column::initial(90.0)).column(Column::initial(100.0))
        .column(Column::initial(100.0)).column(Column::remainder())
        .header(20.0, |mut h| {
            for t in ["Pos", "Book", "Instrument", "Net Qty", "Avg Px", "Mark", "PV", "DV01", "Unreal", "Realized", "Status"] {
                h.col(|ui| { ui.strong(t); });
            }
        })
        .body(|mut body| {
            for l in &p.lines {
                body.row(18.0, |mut row| {
                    cell(&mut row, l.position.to_string());
                    cell(&mut row, &l.book);
                    cell(&mut row, &l.instrument);
                    num_cell(&mut row, money(l.net_quantity), None, Some(pnl_color(l.net_quantity)));
                    num_cell(&mut row, format!("{:.5}", l.avg_price), None, None);
                    num_cell(&mut row, l.mark.map(|m| format!("{m:.6}")).unwrap_or_default(), None, None);
                    num_cell(&mut row, l.pv.map(money).unwrap_or_default(), None, None);
                    num_cell(&mut row, l.dv01.map(money).unwrap_or_default(), None, l.dv01.map(pnl_color));
                    num_cell(&mut row, l.unrealized.map(money).unwrap_or_default(), None, l.unrealized.map(pnl_color));
                    num_cell(&mut row, money(l.realized), None, Some(pnl_color(l.realized)));
                    match &l.error {
                        Some(e) => cell_colored(&mut row, e.as_str(), DOWN),
                        None if l.net_quantity == 0.0 => cell_colored(&mut row, "flat", MUTED),
                        None => cell(&mut row, "ok"),
                    }
                });
            }
        });
}
