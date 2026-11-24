//! The parts of the search for the traces of an app, as the person sees them.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchStep {
    Folders,
    Files,
    Registry,
    System,
}

impl SearchStep {
    /// Every part, in the order the search finishes them.
    pub const ALL: [SearchStep; 4] = [
        SearchStep::Folders,
        SearchStep::Files,
        SearchStep::Registry,
        SearchStep::System,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SearchStep::Folders => "Carpetas",
            SearchStep::Files => "Archivos y accesos",
            SearchStep::Registry => "Registro",
            SearchStep::System => "Sistema",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            SearchStep::Folders => "folder-open",
            SearchStep::Files => "layers",
            SearchStep::Registry => "key-round",
            SearchStep::System => "settings",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_part_of_the_search_is_listed_once() {
        let unique: HashSet<_> = SearchStep::ALL.into_iter().collect();
        assert_eq!(unique.len(), 4);
    }

    #[test]
    fn every_part_has_an_icon_that_exists() {
        for step in SearchStep::ALL {
            assert!(
                crate::icons::names().any(|n| n == step.icon()),
                "{}",
                step.icon()
            );
        }
    }
}
