use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};

/// Application-level events abstracted from raw terminal events.
#[derive(Debug)]
#[allow(dead_code)]
pub enum Event {
    /// A key was pressed.
    Key(KeyEvent),
    /// The mouse was used.
    Mouse(MouseEvent),
    /// The terminal was resized.
    Resize(u16, u16),
    /// A periodic tick for background work / animation.
    Tick,
}

/// Polls `crossterm` for events at the given tick rate.
///
/// This is a synchronous implementation suitable for Phase 1.
/// Phase 2+ may switch to a channel-based async model.
pub struct EventHandler {
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        Self { tick_rate }
    }

    /// Block until the next event (key/mouse/resize) or a tick timeout.
    pub fn next(&self) -> Result<Event> {
        if event::poll(self.tick_rate)? {
            match event::read()? {
                CrosstermEvent::Key(key) => Ok(Event::Key(key)),
                CrosstermEvent::Mouse(mouse) => Ok(Event::Mouse(mouse)),
                CrosstermEvent::Resize(w, h) => Ok(Event::Resize(w, h)),
                // FocusGained, FocusLost, Paste — treat as ticks.
                _ => Ok(Event::Tick),
            }
        } else {
            Ok(Event::Tick)
        }
    }
}
