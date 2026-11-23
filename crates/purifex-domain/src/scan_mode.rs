//! How much of what the search finds is proposed. The rules that recognise a trace are the
//! same in every mode; the mode only decides which kinds of evidence are enough.

use crate::app_traces::Evidence;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScanMode {
    /// Only what points into the app's folder: a path the app registered, a program of its
    /// folder that a service, a task, a rule or a menu entry runs. Nothing is guessed from a name.
    Safe,
    /// The above, and what carries the app's name or one of the extensions it registered.
    #[default]
    Moderate,
    /// The above, and what only resembles the app's name: guesses, listed as «posible» and
    /// never checked for the person. The riskiest mode.
    Advanced,
}

impl ScanMode {
    pub fn label(self) -> &'static str {
        match self {
            ScanMode::Safe => "Seguro",
            ScanMode::Moderate => "Moderado",
            ScanMode::Advanced => "Avanzado",
        }
    }

    /// The next mode, from the safest to the widest and back (the button of the window goes
    /// round them).
    pub fn next(self) -> ScanMode {
        match self {
            ScanMode::Safe => ScanMode::Moderate,
            ScanMode::Moderate => ScanMode::Advanced,
            ScanMode::Advanced => ScanMode::Safe,
        }
    }

    /// Whether a trace with this evidence is proposed in this mode.
    pub fn allows(self, evidence: Evidence) -> bool {
        match self {
            ScanMode::Advanced => true,
            ScanMode::Moderate => !evidence.is_guess(),
            ScanMode::Safe => matches!(
                evidence,
                Evidence::RegisteredInstall
                    | Evidence::PathReference
                    | Evidence::Association
                    | Evidence::ServicePath
                    | Evidence::TaskAction
                    | Evidence::ComServer
                    | Evidence::ContextMenu
                    | Evidence::Monitored
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_keeps_only_what_points_into_the_folder_and_moderate_keeps_everything() {
        let by_name = [
            Evidence::NameMatch,
            Evidence::NamePrefix,
            Evidence::AssociatedExtension,
            Evidence::CrashDump,
        ];
        let by_path = [
            Evidence::RegisteredInstall,
            Evidence::PathReference,
            Evidence::Association,
            Evidence::ServicePath,
            Evidence::TaskAction,
            Evidence::ComServer,
            Evidence::ContextMenu,
            Evidence::Monitored,
        ];
        for e in by_name {
            assert!(
                !ScanMode::Safe.allows(e) && ScanMode::Moderate.allows(e),
                "{e:?}"
            );
        }
        for e in by_path {
            assert!(
                ScanMode::Safe.allows(e) && ScanMode::Moderate.allows(e),
                "{e:?}"
            );
        }
        assert_eq!(ScanMode::default(), ScanMode::Moderate);
        let round = ScanMode::Safe.next().next().next();
        assert_eq!(round, ScanMode::Safe, "three steps go round");
        // The guess of a resemblance is only for Advanced.
        let guess = Evidence::PartialName;
        assert!(ScanMode::Advanced.allows(guess));
        assert!(!ScanMode::Moderate.allows(guess) && !ScanMode::Safe.allows(guess));
        assert!(ScanMode::Advanced.allows(Evidence::NameMatch));
    }
}
