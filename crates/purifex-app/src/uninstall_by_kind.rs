//! Which way an app is uninstalled depends on what it is: a classic program has the command
//! it registered, a package of the Store is removed through the package manager.

use purifex_domain::apps::{AppKind, InstalledApp};
use purifex_domain::ports::Uninstaller;

pub struct UninstallByKind {
    desktop: Box<dyn Uninstaller>,
    store: Box<dyn Uninstaller>,
}

impl UninstallByKind {
    pub fn new(desktop: Box<dyn Uninstaller>, store: Box<dyn Uninstaller>) -> Self {
        Self { desktop, store }
    }
}

impl Uninstaller for UninstallByKind {
    fn launch(&self, app: &InstalledApp) -> Result<(), String> {
        match app.kind {
            AppKind::Desktop => self.desktop.launch(app),
            AppKind::Store => self.store.launch(app),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    struct Tagged(&'static str, Rc<RefCell<Vec<String>>>);
    impl Uninstaller for Tagged {
        fn launch(&self, app: &InstalledApp) -> Result<(), String> {
            self.1.borrow_mut().push(format!("{}:{}", self.0, app.name));
            Ok(())
        }
    }

    #[test]
    fn each_app_is_uninstalled_the_way_its_kind_needs() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let both = UninstallByKind::new(
            Box::new(Tagged("desktop", calls.clone())),
            Box::new(Tagged("store", calls.clone())),
        );
        let app = |name: &str, kind| InstalledApp {
            name: name.into(),
            kind,
            ..Default::default()
        };
        both.launch(&app("Kdenlive", AppKind::Desktop)).unwrap();
        both.launch(&app("Spotify", AppKind::Store)).unwrap();
        assert_eq!(*calls.borrow(), ["desktop:Kdenlive", "store:Spotify"]);
    }
}
