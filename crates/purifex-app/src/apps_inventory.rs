//! The apps of every source together (classic programs, Store packages). One source that
//! fails does not hide the others: the person still sees what could be read, and the
//! failure goes to the log. Only when every source fails is it an error.

use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::AppInventory;

pub struct AllApps(Vec<Box<dyn AppInventory>>);

impl AllApps {
    pub fn new(sources: Vec<Box<dyn AppInventory>>) -> Self {
        Self(sources)
    }
}

impl AppInventory for AllApps {
    /// The first source that can say, says. If none can, the list is the way to know.
    fn still_registered(&self, id: &str) -> Result<bool, String> {
        let mut why = "ninguna fuente sabe decirlo".to_string();
        for source in &self.0 {
            match source.still_registered(id) {
                Ok(found) => return Ok(found),
                Err(error) => why = error,
            }
        }
        Err(why)
    }

    fn apps(&self) -> Result<Vec<InstalledApp>, String> {
        let mut apps = Vec::new();
        let mut first_error = None;
        let mut read = 0;
        for source in &self.0 {
            match source.apps() {
                Ok(found) => {
                    read += 1;
                    apps.extend(found);
                }
                Err(error) => {
                    purifex_domain::log_error!("{error}");
                    first_error.get_or_insert(error);
                }
            }
        }
        match first_error {
            Some(error) if read == 0 => Err(error),
            _ => Ok(apps),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeApps;

    struct Broken;
    impl AppInventory for Broken {
        fn still_registered(&self, _: &str) -> Result<bool, String> {
            Err("no se pudo leer".into())
        }
        fn apps(&self) -> Result<Vec<InstalledApp>, String> {
            Err("no se pudo leer".into())
        }
    }

    fn with(name: &str) -> Box<dyn AppInventory> {
        let apps = FakeApps::default();
        apps.0.borrow_mut().push(InstalledApp {
            id: name.into(),
            name: name.into(),
            ..Default::default()
        });
        Box::new(apps)
    }

    #[test]
    fn the_apps_of_every_source_are_listed_together_in_order() {
        let all = AllApps::new(vec![with("Kdenlive"), with("Calculadora")]);
        let names: Vec<String> = all.apps().unwrap().into_iter().map(|a| a.name).collect();
        assert_eq!(names, ["Kdenlive", "Calculadora"]);
    }

    #[test]
    fn a_source_that_fails_does_not_hide_the_others() {
        let all = AllApps::new(vec![Box::new(Broken), with("Kdenlive")]);
        assert_eq!(all.apps().unwrap().len(), 1);
    }

    #[test]
    fn it_is_an_error_only_when_every_source_fails() {
        assert!(AllApps::new(vec![Box::new(Broken), Box::new(Broken)])
            .apps()
            .is_err());
        assert!(
            AllApps::new(vec![Box::new(Broken), Box::new(FakeApps::default())])
                .apps()
                .unwrap()
                .is_empty()
        );
    }
}
