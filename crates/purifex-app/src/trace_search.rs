//! The search for the traces of an app, run off the thread of the window: it takes seconds, and
//! the window must keep drawing and answering. One search at a time; a new one replaces the old
//! one, whose result is never heard. The result is picked up by polling, without waiting.

use std::sync::mpsc::{channel, Receiver, TryRecvError};

use purifex_domain::cleanup_log::CleanReport;
use purifex_domain::search_step::SearchStep;

use crate::cleaning::SearchJob;

enum Msg {
    Step(SearchStep),
    Done(Result<CleanReport, String>),
}

struct Running {
    receive: Receiver<Msg>,
    steps: Vec<SearchStep>,
}

/// What a poll found.
pub enum Polled {
    Idle,
    /// Still looking, and these parts have finished.
    Working(Vec<SearchStep>),
    Done(Result<CleanReport, String>),
}

#[derive(Default)]
pub struct TraceSearch {
    running: Option<Running>,
}

impl TraceSearch {
    /// Starts looking. If a search was going, this one replaces it.
    pub fn start(&mut self, job: SearchJob) -> Result<(), String> {
        let (send, receive) = channel();
        std::thread::Builder::new()
            .name("trace-search".into())
            .spawn(move || {
                // The window may be closed or the search replaced: nobody listening is fine.
                let said = send.clone();
                let found = job.run(&move |step| {
                    let _ = said.send(Msg::Step(step));
                });
                let _ = send.send(Msg::Done(found));
            })
            .map_err(|e| format!("no se pudo iniciar la búsqueda: {e}"))?;
        self.running = Some(Running {
            receive,
            steps: Vec::new(),
        });
        Ok(())
    }

    /// What has happened since the last poll. It never waits.
    pub fn poll(&mut self) -> Polled {
        let Some(running) = self.running.as_mut() else {
            return Polled::Idle;
        };
        let result = loop {
            match running.receive.try_recv() {
                Ok(Msg::Step(step)) => running.steps.push(step),
                Ok(Msg::Done(found)) => break Some(found),
                Err(TryRecvError::Empty) => break None,
                Err(TryRecvError::Disconnected) => {
                    break Some(Err("la búsqueda terminó sin resultado".into()))
                }
            }
        };
        match result {
            None => Polled::Working(running.steps.clone()),
            Some(report) => {
                self.running = None;
                Polled::Done(report)
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use purifex_domain::apps::InstalledApp;
    use purifex_domain::ports::{Found, TraceFinder};

    use super::*;
    use crate::cleaning::Cleaning;
    use crate::testing::FakeCleaning;

    /// A finder that takes `.0` ms per part and reports each; `.1` tells its answers apart.
    struct Slow(u64, &'static str);

    impl TraceFinder for Slow {
        fn candidates(
            &self,
            _: &InstalledApp,
        ) -> Result<Vec<purifex_domain::app_traces::Candidate>, String> {
            Ok(vec![])
        }
        fn search_reporting(
            &self,
            app: &InstalledApp,
            step: &dyn Fn(SearchStep),
        ) -> Result<Found, String> {
            for part in SearchStep::ALL {
                std::thread::sleep(Duration::from_millis(self.0));
                step(part);
            }
            self.candidates(app).map(|_| Found {
                incomplete: self.1 == "second",
                ..Default::default()
            })
        }
    }

    fn job(finder: Slow) -> SearchJob {
        let app = InstalledApp {
            id: "a".into(),
            name: "Old Tool".into(),
            ..Default::default()
        };
        let cleaning = Cleaning::new(
            Box::new(finder),
            Box::new(FakeCleaning(Default::default())),
            Box::new(FakeCleaning(Default::default())),
        );
        cleaning
            .prepare_search(
                std::slice::from_ref(&app),
                std::slice::from_ref(&app),
                "Old Tool",
            )
            .unwrap()
    }

    fn until_done(search: &mut TraceSearch) -> (Vec<Vec<SearchStep>>, CleanReport) {
        let (mut seen, end) = (vec![], Instant::now() + Duration::from_secs(5));
        while Instant::now() < end {
            match search.poll() {
                Polled::Idle => panic!("not started"),
                Polled::Working(steps) => seen.push(steps),
                Polled::Done(report) => return (seen, report.unwrap()),
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("the search never finished")
    }

    #[test]
    fn a_search_runs_off_the_window_thread_and_reports_each_step() {
        let mut search = TraceSearch::default();
        assert!(matches!(search.poll(), Polled::Idle));
        search.start(job(Slow(30, "first"))).unwrap();
        let started = Instant::now();
        assert!(matches!(search.poll(), Polled::Working(_)));
        assert!(started.elapsed() < Duration::from_millis(50), "never waits");
        let (seen, _) = until_done(&mut search);
        let longest = seen.iter().map(Vec::len).max().unwrap_or(0);
        assert!(longest <= 4);
        assert!(
            seen.iter().any(|s| !s.is_empty()),
            "parts arrived while working"
        );
        assert!(!search.is_running());
    }

    #[test]
    fn the_parts_arrive_in_the_order_of_the_search() {
        let mut search = TraceSearch::default();
        search.start(job(Slow(10, "first"))).unwrap();
        let (seen, _) = until_done(&mut search);
        for steps in seen {
            assert_eq!(steps, SearchStep::ALL[..steps.len()].to_vec());
        }
    }

    #[test]
    fn a_second_search_replaces_the_first_and_only_its_result_arrives() {
        let mut search = TraceSearch::default();
        search.start(job(Slow(40, "first"))).unwrap();
        search.start(job(Slow(5, "second"))).unwrap();
        let (_, found) = until_done(&mut search);
        assert!(found.incomplete, "it is the second one");
        assert!(
            matches!(search.poll(), Polled::Idle),
            "the first was never heard"
        );
    }
}
