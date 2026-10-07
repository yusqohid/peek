mod analyzer;
mod app;
mod config;
mod event;
mod model;
mod tui;
mod ui;
mod worker;

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
    name = "peek",
    version,
    about = "👀 peek — A friendly TUI dashboard to take a quick peek at your projects"
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

    /// GitHub username to monitor (overrides config).
    #[arg(short = 'g', long)]
    github_user: Option<String>,
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
    if let Some(user) = &cli.github_user {
        config.github.username = user.clone();
    }

    // Initialise application state and kick off background work.
    let mut app = App::new(config);
    app.start_scan();
    app.start_github_fetch();

    // Set up the terminal and event loop.
    let mut tui = Tui::new()?;
    let events = EventHandler::new(Duration::from_millis(250));

    // ── Main loop ──
    while !app.should_quit {
        // Drain completed background work without blocking the UI.
        app.poll_background();

        // Render.
        tui.terminal.draw(|frame| {
            ui::render(frame, &app);
        })?;

        // Handle next event.
        match events.next()? {
            Event::Key(key) => app.handle_key(key),
            Event::Tick => { /* polled above; refresh timers live here */ }
            Event::Resize(_, _) => { /* ratatui handles this automatically */ }
            Event::Mouse(_) => { /* future: mouse support */ }
        }
    }

    // Restore terminal.
    tui.restore()?;

    Ok(())
}
