//! Putting back everything of one app that is still in the quarantine: what the person means
//! by "undo". Each trace goes back by the same rules as one by one (`Cleaning::restore`): it
//! never overwrites, and a trace that cannot go back is reported without stopping the rest.

use purifex_domain::quarantine::{open_removals_of, RestoreReport};

use super::Cleaning;

impl Cleaning {
    pub fn restore_app(&mut self, app: &str) -> Result<RestoreReport, String> {
        let seqs: Vec<u64> = open_removals_of(&self.store()?.entries()?, app)
            .iter()
            .map(|e| e.seq)
            .collect();
        if seqs.is_empty() {
            return Err(format!("la cuarentena no guarda restos de {app}"));
        }
        let mut report = RestoreReport {
            app: app.to_string(),
            ..Default::default()
        };
        for seq in seqs {
            match self.restore(seq) {
                Ok(_) => report.restored += 1,
                Err(why) => report.failed.push((seq, why)),
            }
        }
        Ok(report)
    }
}
