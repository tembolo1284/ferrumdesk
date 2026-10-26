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

/// ferrumdesk [config_dir] [--throttle-ms N] [--tick-ms N] [--no-seed]
struct Cli {
    config_dir: String,
    throttle_ms: Option<u64>,
    tick_ms: Option<u64>,
    seed: bool,
}

fn parse_cli() -> Result<Cli> {
    let mut cli = Cli { config_dir: "config".into(), throttle_ms: None, tick_ms: None, seed: true };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--throttle-ms" => cli.throttle_ms = Some(args.next().ok_or_else(|| anyhow::anyhow!("--throttle-ms needs a value"))?.parse()?),
            "--tick-ms" => cli.tick_ms = Some(args.next().ok_or_else(|| anyhow::anyhow!("--tick-ms needs a value"))?.parse()?),
            "--no-seed" => cli.seed = false,
            "-h" | "--help" => {
                println!("ferrumdesk [config_dir] [--throttle-ms N] [--tick-ms N] [--no-seed]");
                std::process::exit(0);
            }
            s if s.starts_with('-') => anyhow::bail!("unknown flag {s}"),
            s => cli.config_dir = s.to_owned(),
        }
    }
    Ok(cli)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();

    let cli = parse_cli()?;
    let mut desk = config::load_desk(format!("{}/desk.toml", cli.config_dir))?;
    if let Some(t) = cli.throttle_ms { desk.throttle_ms = t; }
    if let Some(t) = cli.tick_ms {
        if let config::FeedToml::Sim { interval_ms, .. } = &mut desk.feed { *interval_ms = t; }
    }

    let val = desk.valuation_date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    let (spec, universe) = config::load_instruments(format!("{}/instruments.toml", cli.config_dir), val)?;
    let universe = Arc::new(universe);

    let (book, marks) = match &desk.book_file {
        Some(p) if std::path::Path::new(p).exists() => fd_portfolio::load_book(p)?,
        _ => (Book::new(), Marks::default()),
    };
    tracing::info!(positions = book.len(), throttle_ms = desk.throttle_ms, "starting");

    #[cfg(feature = "fq")]
    let bootstrapper = Bootstrapper::Ferrumquant;
    #[cfg(not(feature = "fq"))]
    let bootstrapper = Bootstrapper::Simple;

    let pricer: Arc<dyn Pricer> = Arc::new(DeskPricer::new(
        PricerConfig { curve: desk.curve.clone(), ..Default::default() }, universe.clone()));

    let engine = Engine::start(
        EngineConfig {
            valuation_date: val,
            curves: vec![spec],
            bootstrapper,
            throttle: ThrottleConfig { min_interval: Duration::from_millis(desk.throttle_ms), ..Default::default() },
            risk_threads: desk.risk_threads,
            ..Default::default()
        },
        config::make_feed(&desk.feed), pricer.clone(), book, marks)?;

    if cli.seed && engine.state.read().book.is_empty() {
        for s in &desk.seed_trade {
            let side = if s.side.eq_ignore_ascii_case("sell") { fd_types::Side::Sell } else { fd_types::Side::Buy };
            engine.book_trade(fd_types::Trade {
                id: fd_types::TradeId::new(), instrument: s.instrument.as_str().into(), side,
                quantity: s.quantity, price: s.price, ts: chrono::Utc::now(),
                trader: "seed".into(), book: "RATES".into(),
            })?;
        }
        tracing::info!(n = desk.seed_trade.len(), "seed trades booked");
    }

    let book_path = desk.book_file.clone().map(Into::into);
    let curve_name = desk.curve.clone();
    let opts = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1400.0, 820.0]).with_title("ferrumdesk"),
        ..Default::default()
    };
    eframe::run_native("ferrumdesk", opts, Box::new(move |cc| {
        Ok(Box::new(fd_ui::DeskApp::new(cc, engine, pricer, universe, curve_name, book_path)))
    })).map_err(|e| anyhow::anyhow!("{e}"))
}
