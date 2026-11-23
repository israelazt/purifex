//! The junk of Windows: found by a port, moved to the quarantine like any other trace (so it can
//! be put back) under the name `Limpieza: basura`.

use purifex_domain::junk::{JunkItem, JunkKind, JunkOutcome};
use purifex_domain::ports::JunkFinder;

use super::Cleaning;

impl Cleaning {
    /// What finds the junk (none until given).
    pub fn with_junk(mut self, junk: Box<dyn JunkFinder>) -> Self {
        self.junk = Some(junk);
        self
    }

    pub fn scan_junk(&self) -> Result<Vec<JunkItem>, String> {
        self.junk
            .as_ref()
            .ok_or("esta versión no busca basura")?
            .scan()
    }

    /// Moves the junk of the chosen kinds. It looks again first: what is moved is what is there
    /// now, found by the port, never what an action says.
    pub fn clean_junk(&mut self, kinds: &[JunkKind]) -> Result<JunkOutcome, String> {
        let traces: Vec<_> = self
            .scan_junk()?
            .into_iter()
            .filter(|i| kinds.contains(&i.kind))
            .map(|i| i.trace())
            .collect();
        let done = self.remove_all("Limpieza: basura", &traces)?;
        Ok(JunkOutcome {
            removed: done.removed.len(),
            bytes: done.removed.iter().map(|e| e.trace.bytes).sum(),
            failed: done
                .failed
                .into_iter()
                .map(|(t, why)| (t.location, why))
                .collect(),
        })
    }
}
