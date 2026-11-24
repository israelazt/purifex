//! What is done before an app's uninstaller opens, as the person sees it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrepStep {
    /// What the app registered: its programs and extensions, to know its traces later.
    Registry,
    RestorePoint,
}

impl PrepStep {
    /// Every part, in the order it is done.
    pub const ALL: [PrepStep; 2] = [PrepStep::Registry, PrepStep::RestorePoint];

    pub fn label(self) -> &'static str {
        match self {
            PrepStep::Registry => "Lo que la app registró",
            PrepStep::RestorePoint => "Punto de restauración de Windows",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            PrepStep::Registry => "key-round",
            PrepStep::RestorePoint => "shield-check",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_part_has_an_icon_that_exists_and_a_name() {
        for step in PrepStep::ALL {
            assert!(crate::icons::names().any(|n| n == step.icon()));
            assert!(!step.label().is_empty());
        }
    }
}
