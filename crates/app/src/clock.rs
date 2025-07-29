use std::time::Duration;

const REAL_DAY_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

/// Duration of day + night in ticks
const DAYNIGHT_DURATION: u32 = 200; // 24_000;

/// Duration of one game second
const SECOND_DURATION: Duration = REAL_DAY_DURATION
    .checked_div(DAYNIGHT_DURATION)
    .expect("failed to calculate duration of the second");

pub struct Clock {
    time: Duration,
}

impl Clock {
    pub const fn default() -> Self {
        Self::new(REAL_DAY_DURATION.checked_div(2).expect("failed to divide real day duration"))
    }

    pub const fn new(time: Duration) -> Self {
        Self { time }
    }

    pub const fn time(&self) -> Duration {
        self.time
    }

    pub const fn get_progress(&self) -> f32 {
        self.time.div_duration_f32(REAL_DAY_DURATION)
    }

    pub const fn get_visual_progress(&self) -> (bool, f32) {
        let progress = self.get_progress();

        let visual_progres = if progress > 0.5 { progress - 0.5 } else { progress };

        (progress > 0.5, visual_progres * 2.0)
    }

    pub const fn tick(&mut self) {
        self.time = self.time.checked_add(SECOND_DURATION).expect("failed to add one second");

        if self.time.as_nanos() >= REAL_DAY_DURATION.as_nanos() {
            self.time = Duration::ZERO;
        }
    }
}
