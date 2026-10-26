mod config;

use anyhow::Result;
use fd_engine::{Engine, EngineConfig};
use fd_market::builder::Bootstrapper;
use fd_market::ThrottleConfig;
use fd_pricing::pricer::{DeskPricer, PricerConfig};
use fd_pricing::Pricer;
use fd_portfolio::{Book, Marks};
use std::sync::Arc;
use std::time::Duration;

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();

    let cfg_dir = std::env::args().nth(1).unwrap_or_else(|| "config".into());
    let desk = config::load_desk(format!("{cfg_dir}/desk.toml"))?;
    let val = desk.valuation_date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    let (spec, universe) = config::load_instruments(format!("{cfg_dir}/instruments.toml"), val)?;
    let universe = Arc::new(universe);

    let (book, marks) = match &desk.book_file {
        Some(p) if std::path::Path::new(p).exists() => fd_portfolio::load_book(p)?,
        _ => (Book::new(), Marks::default()),
    };
    tracing::info!(positions = book.len(), "book loaded");

    let bootstrapper = if cfg!(feature = "fq") {
        #[cfg(feature = "fq")] { Bootstrapper::Ferrumquant }
        #[cfg(not(feature = "fq"))] { Bootstrapper::Simple }
    } else { Bootstrapper::Simple };

    let pricer: Arc<dyn Pricer> = Arc::new(DeskPricer::new(
        PricerConfig { curve: desk.curve.clone(), ..Default::default() }, universe.clone()));

    let engine = Engine::start(
        EngineConfig {
            valuation_date: val, curves: vec![spec], bootstrapper,
            throttle: ThrottleConfig { min_interval: Duration::from_millis(desk.throttle_ms), ..Default::default() },
            risk_threads: desk.risk_threads, ..Default::default()
        },
        config::make_feed(&desk.feed), pricer.clone(), book, marks)?;

    let book_path = desk.book_file.clone().map(Into::into);
    let curve_name = desk.curve.clone();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 820.0]).with_title("ferrumdesk"),
        ..Default::default()
    };
    eframe::run_native("ferrumdesk", opts, Box::new(move |cc| {
        Ok(Box::new(fd_ui::DeskApp::new(cc, engine, pricer, universe, curve_name, book_path)))
    })).map_err(|e| anyhow::anyhow!("{e}"))
}
