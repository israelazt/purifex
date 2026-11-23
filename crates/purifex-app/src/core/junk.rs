//! The Junk page in the core: what Windows leaves around, which kinds are checked, and moving
//! them to the quarantine.

use purifex_domain::junk::JunkKind;
use purifex_domain::view::Confirm;

use super::Core;

impl Core {
    pub(super) fn read_junk_page(&mut self, errors: &mut Vec<String>) {
        if self.model.junk.is_some() {
            return;
        }
        match self.services.junk_scan() {
            Ok(items) => self.model.junk = Some(items),
            Err(why) => {
                errors.push(format!("basura: {why}"));
                self.model.junk = Some(Vec::new());
            }
        }
    }

    pub(super) fn junk_toggled(&mut self, kind: JunkKind) {
        let off = &mut self.model.junk_unchecked;
        match off.iter().position(|k| *k == kind) {
            Some(at) => {
                off.remove(at);
            }
            None => off.push(kind),
        }
    }

    /// The kinds that are checked and have something in them.
    fn junk_kinds(&self) -> Vec<JunkKind> {
        let items = self.model.junk.as_deref().unwrap_or_default();
        JunkKind::ALL
            .into_iter()
            .filter(|k| {
                !self.model.junk_unchecked.contains(k) && items.iter().any(|i| i.kind == *k)
            })
            .collect()
    }

    pub(super) fn junk_clean_asked(&mut self) {
        if self.junk_kinds().is_empty() {
            self.model.message = Some("No hay nada marcado que limpiar.".into());
        } else {
            self.ask(Confirm::CleanJunk);
        }
    }

    pub(super) fn junk_cleaned(&mut self) {
        let kinds = self.junk_kinds();
        match self.services.junk_clean(&kinds) {
            Ok(outcome) => {
                self.model.message = Some(outcome.to_text());
                (self.model.junk, self.model.quarantine) = (None, None);
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }
}
