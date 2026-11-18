//! The clock of the system: today's day (local) and a monotonic time.

use std::time::{Duration, Instant};

use purifex_domain::date::Date;
use purifex_domain::ports::Clock;
use windows::Win32::System::SystemInformation::GetLocalTime;

pub struct SystemClock {
    start: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn today(&self) -> Date {
        // SAFETY: a plain query with no arguments.
        let t = unsafe { GetLocalTime() };
        Date::new(t.wYear, t.wMonth as u8, t.wDay as u8).unwrap_or(Date {
            year: 1970,
            month: 1,
            day: 1,
        })
    }

    fn monotonic(&self) -> Duration {
        self.start.elapsed()
    }

    fn sleep(&self, time: Duration) {
        std::thread::sleep(time);
    }
}
