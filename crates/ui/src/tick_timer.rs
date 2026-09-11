use std::time::{Duration, Instant};

pub struct TickTimer {
    last_tick: Instant,
    tick_rate: Duration,
}

impl TickTimer {
    #[must_use]
    pub fn new(tick_rate: Duration) -> Self {
        Self {
            last_tick: Instant::now(),
            tick_rate,
        }
    }

    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.tick_rate
            .checked_sub(self.last_tick.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    #[must_use]
    pub fn is_elapsed(&self) -> bool {
        self.last_tick.elapsed() >= self.tick_rate
    }

    pub fn reset(&mut self) {
        self.last_tick = Instant::now();
    }

    #[must_use]
    pub fn tick_rate(&self) -> Duration {
        self.tick_rate
    }

    pub fn set_rate(&mut self, tick_rate: Duration) {
        self.tick_rate = tick_rate;
    }
}
