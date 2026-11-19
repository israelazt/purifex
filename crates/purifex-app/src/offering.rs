//! The search for driver offers, run off the thread of the interface: Windows
//! Update goes out to the network and can take a minute, so the window must
//! keep answering. One search at a time; the result is picked up by polling.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

use purifex_domain::driver_offers::Offer;
use purifex_domain::ports::DriverOffers;

type Found = Result<Vec<Offer>, String>;

#[derive(Default)]
pub struct Offering {
    source: Option<Arc<dyn DriverOffers>>,
    running: Option<Receiver<Found>>,
}

impl Offering {
    pub fn new(source: Arc<dyn DriverOffers>) -> Self {
        Self {
            source: Some(source),
            running: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// Starts the search, unless one is already going.
    pub fn start(&mut self) -> Result<(), String> {
        let source = self
            .source
            .clone()
            .ok_or("Esta versión no sabe buscar actualizaciones")?;
        if self.running.is_some() {
            return Ok(());
        }
        let (send, receive) = channel();
        std::thread::Builder::new()
            .name("driver-offers".into())
            .spawn(move || {
                // The window may be closed while it works: nobody listening is fine.
                let _ = send.send(source.search());
            })
            .map_err(|e| format!("no se pudo iniciar la búsqueda: {e}"))?;
        self.running = Some(receive);
        Ok(())
    }

    /// Waits up to `wait` for the result. `None` = still searching (or not started).
    pub fn poll(&mut self, wait: Duration) -> Option<Found> {
        let receive = self.running.as_ref()?;
        match receive.recv_timeout(wait) {
            Ok(found) => {
                self.running = None;
                Some(found)
            }
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                self.running = None;
                Some(Err("la búsqueda terminó sin resultado".into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Slow(u64, Found);

    impl DriverOffers for Slow {
        fn search(&self) -> Found {
            std::thread::sleep(Duration::from_millis(self.0));
            self.1.clone()
        }
    }

    #[test]
    fn without_a_source_it_says_so_instead_of_pretending() {
        let mut o = Offering::default();
        assert!(o.start().unwrap_err().contains("no sabe buscar"));
        assert!(!o.is_running());
        assert_eq!(o.poll(Duration::ZERO), None);
    }

    #[test]
    fn the_search_runs_apart_and_polling_does_not_block_past_its_wait() {
        let mut o = Offering::new(Arc::new(Slow(150, Ok(vec![]))));
        o.start().unwrap();
        assert!(o.is_running());
        let t = std::time::Instant::now();
        assert_eq!(o.poll(Duration::from_millis(10)), None, "still searching");
        assert!(t.elapsed() < Duration::from_millis(120));
        let mut result = None;
        for _ in 0..100 {
            result = o.poll(Duration::from_millis(20));
            if result.is_some() {
                break;
            }
        }
        assert_eq!(result, Some(Ok(vec![])));
        assert!(!o.is_running(), "ready for another");
    }

    #[test]
    fn starting_twice_does_not_start_two_and_an_error_is_handed_over() {
        let mut o = Offering::new(Arc::new(Slow(50, Err("sin red".into()))));
        o.start().unwrap();
        o.start().unwrap();
        let mut result = None;
        while result.is_none() {
            result = o.poll(Duration::from_millis(20));
        }
        assert_eq!(result, Some(Err("sin red".into())));
        assert_eq!(o.poll(Duration::ZERO), None, "only one result came");
    }
}
