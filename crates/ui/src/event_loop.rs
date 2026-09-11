use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use crossterm::event;
use ratatui::{backend::Backend, Terminal};

use crate::{app::App, TickTimer};

pub(super) trait EventSource {
    fn poll(&mut self, timeout: Duration) -> Result<Option<event::Event>>;
}

pub(super) struct CrosstermEventSource;

impl EventSource for CrosstermEventSource {
    fn poll(&mut self, timeout: Duration) -> Result<Option<event::Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }
}

pub(super) fn run_event_loop<B, E>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    events: &mut E,
    tick_rate: Duration,
    max_duration: Option<Duration>,
) -> Result<()>
where
    B: Backend,
    B::Error: std::error::Error + Send + Sync + 'static,
    E: EventSource,
{
    let mut tick_timer = TickTimer::new(tick_rate);
    let started = Instant::now();

    loop {
        let desired_rate = app.ui_settings.main_tick_rate();
        if desired_rate != tick_timer.tick_rate() {
            tick_timer.set_rate(desired_rate);
        }
        terminal.draw(|frame| app.draw(frame))?;

        if let Some(event) = events.poll(tick_timer.timeout())? {
            match event {
                event::Event::Key(key) => {
                    if app.handle_key(key) {
                        break;
                    }
                }
                event::Event::Mouse(mouse) => {
                    app.handle_mouse(mouse);
                }
                event::Event::Resize(_, _)
                | event::Event::Paste(_)
                | event::Event::FocusGained
                | event::Event::FocusLost => {
                    // Next loop iteration will redraw with new constraints.
                }
            }
        }

        if tick_timer.is_elapsed() {
            app.on_tick();
            tick_timer.reset();
        }

        if let Some(limit) = max_duration {
            if started.elapsed() >= limit {
                return Err(anyhow!(
                    "event loop exceeded {limit:?} without receiving a quit action"
                ));
            }
        }
    }

    Ok(())
}
