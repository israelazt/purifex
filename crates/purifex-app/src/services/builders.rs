//! How the services are put together: `new` and the `with_*` that give each optional part.

use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::{AppInventory, Elevation, UrlOpener};

use super::*;

impl<I, C, S, C2, F, A> App<I, C, S, C2, F, A> {
    pub fn new(scanner: Scanner<I, C>, recorder: Recorder<S, C2, F>, archive: Archive<A>) -> Self {
        Self {
            scanner,
            recorder,
            archive,
            fixer: Fixer::disabled(),
            offering: Offering::default(),
            candidates: Candidates::default(),
            opener: None,
            folder_opener: None,
            inventory: None,
            cleaning: Cleaning::default(),
            elevation: None,
        }
    }

    /// What uninstalls apps and cleans what they leave (none until given).
    pub fn with_cleaning(mut self, cleaning: Cleaning) -> Self {
        self.cleaning = cleaning;
        self
    }

    /// What tells whether the program is an administrator and restarts it as one (none
    /// until given: it then says it is not, and cannot restart).
    pub fn with_elevation(mut self, elevation: Box<dyn Elevation>) -> Self {
        self.elevation = Some(elevation);
        self
    }

    /// Where the installed apps are read (none until given).
    pub fn with_apps(mut self, inventory: Box<dyn AppInventory>) -> Self {
        self.inventory = Some(inventory);
        self
    }

    /// What opens a web page (none until given).
    /// How a folder is opened in the file explorer (nothing until given).
    pub fn with_folder_opener(
        mut self,
        opener: Box<dyn purifex_domain::ports::FolderOpener>,
    ) -> Self {
        self.folder_opener = Some(opener);
        self
    }

    pub fn with_opener(mut self, opener: Box<dyn UrlOpener>) -> Self {
        self.opener = Some(opener);
        self
    }

    /// Where the proposals for drivers are kept (none until given).
    pub fn with_candidates(mut self, candidates: Candidates) -> Self {
        self.candidates = candidates;
        self
    }

    /// Where driver offers are searched (none until given).
    pub fn with_offers(mut self, offering: Offering) -> Self {
        self.offering = offering;
        self
    }

    /// The apps installed: the ones to show, and everything installed.
    pub(super) fn installed(&self) -> Result<(Vec<InstalledApp>, Vec<InstalledApp>), String> {
        let all = self
            .inventory
            .as_ref()
            .ok_or("esta versión no sabe leer las apps instaladas")?
            .apps()?;
        Ok((listed(all.clone()), all))
    }

    /// The fixes this application can make (none until given).
    pub fn with_fixer(mut self, fixer: Fixer) -> Self {
        self.fixer = fixer;
        self
    }
}
