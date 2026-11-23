//! Taking an entry of what starts with Windows out: it goes to the quarantine like any other
//! trace (with its registry copy or its definition), so it can be put back.

use purifex_domain::app_traces::Trace;
use purifex_domain::cleanup_log::CleanupEntry;

use super::Cleaning;

impl Cleaning {
    /// Moves the entry to the quarantine under the name `Inicio: <name>` and logs it. `Err` says
    /// why it could not be removed (a program has it open, it needs an administrator).
    pub fn remove_startup(&mut self, name: &str, trace: &Trace) -> Result<CleanupEntry, String> {
        let mut done = self.remove_all(&format!("Inicio: {name}"), std::slice::from_ref(trace))?;
        match (done.removed.pop(), done.failed.pop()) {
            (Some(entry), _) => Ok(entry),
            (None, Some((_, why))) => Err(why),
            (None, None) => Err("no se pudo quitar".into()),
        }
    }
}
