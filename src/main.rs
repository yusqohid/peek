mod analyzer;
mod app;
mod config;
mod event;
mod model;
mod tui;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;

use app::App;
use config::AppConfig;
use event::{Event, EventHandler};
use tui::Tui;

#[derive(Parser, Debug)]
#[command(
    name = "rpa",
    version,
    about = "🦀 Rust Project Analyzer — TUI dashboard for developers"
)]
struct Cli {
    /// Root directory to scan for projects (overrides config).
    #[arg(short, long)]
    path: Option<String>,

    /// Path to config file (default: ./config.toml).
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Maximum scan depth (overrides config).
    #[arg(short, long)]
    depth: Option<usize>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration.
    let config_path = PathBuf::from(&cli.config);
    let mut config = AppConfig::load(&config_path)?;

    // CLI overrides.
    if let Some(path) = &cli.path {
        config.general.scan_directory = path.clone();
    }
    if let Some(depth) = cli.depth {
        config.general.scan_depth = depth;
    }

    // Initialise application state and run the initial scan.
    let mut app = App::new(config);
    app.scan_and_analyze();

    // Set up the terminal and event loop.
    let mut tui = Tui::new()?;
    let events = EventHandler::new(Duration::from_millis(250));

    // ── Main loop ──
    while !app.should_quit {
        // Render.
        tui.terminal.draw(|frame| {
            ui::render(frame, &app);
        })?;

        // Handle next event.
        match events.next()? {
            Event::Key(key) => app.handle_key(key),
            Event::Tick => { /* future: background refresh */ }
            Event::Resize(_, _) => { /* ratatui handles this automatically */ }
            Event::Mouse(_) => { /* future: mouse support */ }
        }
    }

    // Restore terminal.
    tui.restore()?;

    Ok(())
}
