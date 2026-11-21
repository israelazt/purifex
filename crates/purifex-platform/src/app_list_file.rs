//! The choices for the list of apps on disk: `%LOCALAPPDATA%\Purifex\apps-view.json`. Small and
//! rewritten whole each time; a missing or damaged file just means the defaults.

use std::fs;
use std::path::PathBuf;

use purifex_domain::app_list::AppListSettings;
use purifex_domain::ports::AppListStore;

pub struct LocalAppList {
    path: Option<PathBuf>,
}

impl LocalAppList {
    pub fn new() -> Self {
        let path = std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("Purifex").join("apps-view.json"));
        Self { path }
    }

    pub fn in_file(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }
}

impl Default for LocalAppList {
    fn default() -> Self {
        Self::new()
    }
}

impl AppListStore for LocalAppList {
    fn load(&self) -> AppListSettings {
        let text = self.path.as_ref().and_then(|p| fs::read_to_string(p).ok());
        AppListSettings::from_text(&text.unwrap_or_default())
    }

    fn save(&self, settings: &AppListSettings) -> Result<(), String> {
        let path = self
            .path
            .as_ref()
            .ok_or("no se encuentra la carpeta de datos del usuario")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, settings.to_text()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::app_list::{AppFilter, AppSort, AppView};

    use super::*;

    #[test]
    fn what_was_chosen_is_there_next_time_and_a_damaged_file_gives_the_defaults() {
        let dir = std::env::temp_dir().join(format!("purifex-applist-{}", std::process::id()));
        let file = dir.join("deep").join("apps-view.json");
        let store = LocalAppList::in_file(file.clone());
        assert_eq!(
            store.load(),
            AppListSettings::default(),
            "nothing saved yet"
        );
        let chosen = AppListSettings {
            filter: AppFilter::Programs,
            sort: AppSort::Date,
            view: AppView::List,
        };
        store.save(&chosen).unwrap();
        assert_eq!(LocalAppList::in_file(file.clone()).load(), chosen);
        fs::write(&file, "{ roto").unwrap();
        assert_eq!(store.load(), AppListSettings::default());
        let _ = fs::remove_dir_all(dir);
    }
}
