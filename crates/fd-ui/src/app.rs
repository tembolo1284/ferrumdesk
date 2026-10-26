use crate::theme;
use crate::views::{self, positions::TradeEntry, pricer::PricerPanel, Tab};
use crate::widgets::Flash;
use fd_engine::Engine;
use fd_pricing::{Pricer, Universe};
use std::sync::Arc;
use std::time::Duration;

pub struct DeskApp {
    engine: Engine,
    pricer: Arc<dyn Pricer>,
    universe: Arc<Universe>,
    curve_name: String,
    tab: Tab,
    flash: Flash,
    trade_entry: TradeEntry,
    pricer_panel: PricerPanel,
    book_path: Option<std::path::PathBuf>,
}

impl DeskApp {
    pub fn new(cc: &eframe::CreationContext<'_>, engine: Engine, pricer: Arc<dyn Pricer>,
               universe: Arc<Universe>, curve_name: String, book_path: Option<std::path::PathBuf>) -> Self {
        theme::apply(&cc.egui_ctx);
        Self { engine, pricer, universe, curve_name, tab: Tab::Positions, flash: Flash::new(600),
               trade_entry: TradeEntry::default(), pricer_panel: PricerPanel::default(), book_path }
    }

    fn save_book(&self) {
        if let Some(p) = &self.book_path {
            let g = self.engine.state.read();
            if let Err(e) = fd_portfolio::save_book(p, &g.book, &g.marks) { tracing::error!(%e, "save failed"); }
        }
    }
}

impl eframe::App for DeskApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let f = self.engine.state.frame();

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("ferrumdesk");
                ui.separator();
                for t in Tab::ALL { ui.selectable_value(&mut self.tab, t, t.label()); }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Save book").clicked() { self.save_book(); }
                });
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let s = &f.stats;
            ui.horizontal(|ui| {
                let feed = if s.feed_connected { ("feed ●", theme::UP) } else { ("feed ○", theme::DOWN) };
                ui.colored_label(feed.1, feed.0);
                ui.separator();
                ui.label(format!("ticks {}/{}", s.ticks_accepted, s.ticks_received));
                ui.separator();
                ui.label(format!("snapshots {}", s.snapshots));
                ui.separator();
                ui.label(format!("reprices {} · last {} µs · {} pos", s.reprices, s.last_reprice_us, s.positions_priced));
                ui.separator();
                ui.label(egui::RichText::new(format!("val date {}", f.snapshot.as_ref().map(|s| s.valuation_date.to_string()).unwrap_or_default())).color(theme::MUTED));
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Positions => views::positions::show(ui, &f, &self.engine, &self.universe, &mut self.trade_entry),
            Tab::Risk => views::risk::show(ui, &f, &self.universe, &self.curve_name),
            Tab::Market => views::market::show(ui, &f, &mut self.flash),
            Tab::Curve => views::curve::show(ui, &f, &self.curve_name),
            Tab::Pricer => views::pricer::show(ui, &f, self.pricer.as_ref(), &self.universe, &mut self.pricer_panel),
            Tab::Log => views::log::show(ui, &self.engine.state),
        });

        // steady refresh so flashes/ages animate even without input
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_book();
        self.engine.shutdown();
    }
}
