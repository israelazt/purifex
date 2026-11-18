//! The recorder: waits for the game, takes a sample every second while it
//! runs, writes each one as it is taken, and summarizes when the game closes.

use std::time::Duration;

use purifex_domain::log_warn;
use purifex_domain::ports::{Clock, PerformanceSampler, SessionFile};
use purifex_domain::sample::{Sample, CSV_HEADER};
use purifex_domain::session::{summarize, SessionSummary};
use purifex_domain::session_header::SessionHeader;

/// How often the game is looked for while waiting for it.
const WAIT_STEP: Duration = Duration::from_secs(2);
/// Seconds without the game, after having seen it, that end the recording.
const ABSENT_TO_STOP: u32 = 3;
/// Failed samples in a row that stop the recording.
const FAILURES_TO_STOP: u32 = 5;
/// A live line every this many samples.
const LIVE_EVERY: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchOptions {
    /// Wait for the game to start and stop when it closes. Otherwise it
    /// records straight away.
    pub wait_for_game: bool,
    /// Stop after this many seconds recorded.
    pub max_seconds: Option<u64>,
    pub interval: Duration,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            wait_for_game: true,
            max_seconds: None,
            interval: Duration::from_secs(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub summary: SessionSummary,
    pub file: String,
}

pub struct Recorder<S, C, F> {
    sampler: S,
    clock: C,
    file: F,
}

impl<S: PerformanceSampler, C: Clock, F: SessionFile> Recorder<S, C, F> {
    pub fn new(sampler: S, clock: C, file: F) -> Self {
        Self {
            sampler,
            clock,
            file,
        }
    }

    pub fn run(
        &mut self,
        options: WatchOptions,
        file_name: &str,
        progress: &mut dyn FnMut(&str),
    ) -> Result<Recording, String> {
        if options.wait_for_game {
            progress("Esperando a que empiece la partida (abre el juego; Ctrl+C para cancelar)…");
            while !self.sampler.game_running()? {
                self.clock.sleep(WAIT_STEP);
            }
            progress("Partida detectada: grabando una muestra por segundo.");
        }
        let path = self.file.create(file_name)?;
        let info = self.sampler.describe();
        let header = SessionHeader::new(&info.game, info.cores, self.clock.today());
        self.file.append(&header.to_line())?;
        self.file.append(CSV_HEADER)?;
        // The first reading only sets the baseline for the differences.
        let _ = self.sampler.sample();

        let start = self.clock.monotonic();
        let mut samples: Vec<Sample> = Vec::new();
        let (mut absent, mut failures, mut ticks) = (0u32, 0u32, 0u32);
        loop {
            ticks += 1;
            let due = start + options.interval * ticks;
            self.clock.sleep(due.saturating_sub(self.clock.monotonic()));
            let mut sample = match self.sampler.sample() {
                Ok(sample) => sample,
                Err(error) => {
                    failures += 1;
                    log_warn!("sample failed: {error}");
                    if failures >= FAILURES_TO_STOP {
                        return Err(format!("la medición falla: {error}"));
                    }
                    continue;
                }
            };
            failures = 0;
            sample.at_ms = u64::try_from(self.clock.monotonic().saturating_sub(start).as_millis())
                .unwrap_or(u64::MAX);
            if options.wait_for_game {
                if sample.game.is_none() {
                    absent += 1;
                    if absent >= ABSENT_TO_STOP {
                        break;
                    }
                    continue;
                }
                absent = 0;
            }
            if let Err(error) = self.file.append(&sample.csv_row()) {
                log_warn!("could not write the sample: {error}");
            }
            if samples.len() % LIVE_EVERY == LIVE_EVERY - 1 {
                progress(&live_line(&sample));
            }
            let finished = options
                .max_seconds
                .is_some_and(|max| sample.at_ms / 1000 >= max);
            samples.push(sample);
            if finished {
                break;
            }
        }
        if samples.is_empty() {
            return Err("no se tomó ninguna muestra".into());
        }
        Ok(Recording {
            summary: summarize(&samples),
            file: path,
        })
    }
}

fn live_line(s: &Sample) -> String {
    let gpu = s.gpu.map_or(String::new(), |g| {
        format!(" · GPU {:.0} % {} °C", g.util, g.temp_c)
    });
    format!(
        "  {:>4} s · CPU {:.0} % · núcleo máx. {:.0} %{gpu}",
        s.at_ms / 1000,
        s.cpu_total,
        s.cpu_max_core
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{FakeClock, FakeFile, FakeSampler};

    fn recorder(sampler: FakeSampler) -> Recorder<FakeSampler, FakeClock, FakeFile> {
        Recorder::new(sampler, FakeClock::ticking(0), FakeFile::default())
    }

    #[test]
    fn it_waits_for_the_game_records_while_it_runs_and_stops_when_it_closes() {
        let mut r = recorder(FakeSampler::new(2, 5, true));
        let mut said = Vec::new();
        let done = r
            .run(WatchOptions::default(), "lol", &mut |m| {
                said.push(m.to_string())
            })
            .unwrap();
        assert_eq!(done.summary.samples, 5, "only the seconds with the game");
        assert!(said[0].contains("Esperando"));
        assert!(said[1].contains("detectada"));
        assert_eq!(r.sampler.waits, 3, "asked until it appeared");
        let lines = r.file.lines.borrow();
        assert!(SessionHeader::parse(&lines[0]).is_some(), "{}", lines[0]);
        assert_eq!(lines[1], CSV_HEADER);
        assert_eq!(lines.len(), 2 + 5);
        assert_eq!(done.file, "fake/lol.csv");
    }

    #[test]
    fn the_samples_are_stamped_a_second_apart() {
        let mut r = recorder(FakeSampler::new(0, 3, true));
        r.run(WatchOptions::default(), "x", &mut |_| {}).unwrap();
        let lines = r.file.lines.borrow();
        let times: Vec<&str> = lines[2..]
            .iter()
            .map(|l| l.split(',').next().unwrap())
            .collect();
        assert_eq!(times, ["1.0", "2.0", "3.0"]);
    }

    #[test]
    fn without_waiting_it_stops_at_the_time_limit() {
        let mut r = recorder(FakeSampler::new(0, 100, false));
        let options = WatchOptions {
            wait_for_game: false,
            max_seconds: Some(4),
            ..WatchOptions::default()
        };
        let done = r.run(options, "x", &mut |_| {}).unwrap();
        assert_eq!(done.summary.samples, 4);
    }

    #[test]
    fn measuring_that_keeps_failing_ends_with_an_error() {
        let mut sampler = FakeSampler::new(0, 3, true);
        sampler.fail_after = Some(0);
        let mut r = recorder(sampler);
        let error = r
            .run(WatchOptions::default(), "x", &mut |_| {})
            .err()
            .unwrap();
        assert!(error.contains("falla"), "{error}");
    }

    #[test]
    fn a_file_that_cannot_be_written_is_an_error_up_front() {
        let mut r = recorder(FakeSampler::new(0, 3, true));
        r.file.fail_appends = true;
        let done = r.run(WatchOptions::default(), "x", &mut |_| {});
        assert!(done.is_err());
    }
}
