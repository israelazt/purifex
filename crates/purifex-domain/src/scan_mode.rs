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
}

impl ScanMode {
    pub fn label(self) -> &'static str {
        match self {
            ScanMode::Safe => "Seguro",
            ScanMode::Moderate => "Moderado",
        }
    }

    /// The other mode (the button of the window switches between them).
    pub fn other(self) -> ScanMode {
        match self {
            ScanMode::Safe => ScanMode::Moderate,
            ScanMode::Moderate => ScanMode::Safe,
        }
    }

    /// Whether a trace with this evidence is proposed in this mode.
    pub fn allows(self, evidence: Evidence) -> bool {
        match self {
            ScanMode::Moderate => true,
            ScanMode::Safe => matches!(
                evidence,
                Evidence::RegisteredInstall
                    | Evidence::PathReference
                    | Evidence::Association
                    | Evidence::ServicePath
                    | Evidence::TaskAction
                    | Evidence::ComServer
                    | Evidence::ContextMenu
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
        assert_eq!(ScanMode::Safe.other().other(), ScanMode::Safe);
    }
}
