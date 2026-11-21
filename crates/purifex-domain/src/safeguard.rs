//! The safety net taken before an uninstaller starts: a restore point of Windows. What
//! happened is told to the person, because without it the uninstall still goes on but the
//! system cannot be taken back as a whole.

use crate::apps::InstalledApp;

/// What came of asking Windows for a restore point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestorePointOutcome {
    /// Windows accepted the request. It keeps one every 24 hours at most, so a recent one
    /// may already cover this uninstall.
    Requested,
    /// It could not be made, and why (not an administrator, System Protection off...).
    Unavailable(String),
    /// This version does not ask for one.
    NotAsked,
}

impl RestorePointOutcome {
    /// The sentence that goes with the message of the uninstall, if there is one to say.
    pub fn to_text(&self) -> Option<String> {
        match self {
            Self::Requested => Some(
                "Se pidió un punto de restauración a Windows (crea como mucho uno cada 24 horas)."
                    .into(),
            ),
            Self::Unavailable(why) => Some(format!("Sin punto de restauración: {why}.")),
            Self::NotAsked => None,
        }
    }
}

/// An uninstaller that was started, and the safety net taken before it.
#[derive(Debug, Clone)]
pub struct Started {
    pub app: InstalledApp,
    pub restore_point: RestorePointOutcome,
    /// Where the copy of the app's registration was saved, if it had one.
    pub registry_copy: Option<String>,
}

impl Started {
    /// What to add to the message of the uninstall: what was kept before it started.
    pub fn safeguards_text(&self) -> String {
        let copy = self
            .registry_copy
            .as_ref()
            .map(|path| format!("Copia del registro de la app: {path}."));
        [self.restore_point.to_text(), copy]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_outcome_says_what_happened_and_not_asking_says_nothing() {
        assert!(RestorePointOutcome::Requested
            .to_text()
            .unwrap()
            .contains("24 horas"));
        let why = RestorePointOutcome::Unavailable("hace falta ser administrador".into());
        assert_eq!(
            why.to_text().unwrap(),
            "Sin punto de restauración: hace falta ser administrador."
        );
        assert_eq!(RestorePointOutcome::NotAsked.to_text(), None);
    }
}
