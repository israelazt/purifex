//! The `updates` order: starts the search for offers and waits for it, telling the
//! user it can take a minute.

use std::time::Duration;

use crate::outcome::Outcome;
use crate::services::Services;

pub fn run(services: &mut dyn Services, progress: &mut dyn FnMut(&str)) -> Result<Outcome, String> {
    services.start_offers()?;
    progress("Buscando en Windows Update (puede tardar un minuto)…");
    loop {
        if let Some(found) = services.poll_offers(Duration::from_millis(500)) {
            return found
                .map(Outcome::Offers)
                .map_err(|e| format!("No se pudo buscar: {e}"));
        }
    }
}
