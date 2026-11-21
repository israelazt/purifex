//! The profiles of the other people who use this computer (`C:\Users\<name>`). An app installed
//! for everyone leaves its data in each of them, but only an administrator can read another
//! profile: what can be read is searched, and what cannot is counted so the person is told that
//! only part of the computer was looked at.

use std::fs;
use std::path::{Path, PathBuf};

use super::find_files::UserPlaces;

/// Folders of `Users` that are not a person's profile.
const NOT_PROFILES: [&str; 5] = [
    "public",
    "default",
    "default user",
    "all users",
    "defaultuser0",
];

/// The other profiles: the ones whose `AppData` can be read, and how many cannot.
pub struct OtherUsers {
    pub readable: Vec<PathBuf>,
    pub unread: usize,
}

impl OtherUsers {
    /// The places of the user to look at in each readable profile.
    pub fn places(&self) -> Vec<UserPlaces> {
        self.readable.iter().map(|p| places_of(p)).collect()
    }

    /// Folders whose children are looked at, in each readable profile.
    pub fn roots(&self) -> Vec<PathBuf> {
        self.readable
            .iter()
            .flat_map(|p| {
                [
                    p.join(r"AppData\Local"),
                    p.join(r"AppData\Roaming"),
                    p.join(r"AppData\Local\Programs"),
                    p.join("Documents"),
                    p.join(r"AppData\Roaming\Microsoft\Windows\Start Menu\Programs"),
                ]
            })
            .collect()
    }
}

fn places_of(profile: &Path) -> UserPlaces {
    let roaming = profile.join(r"AppData\Roaming");
    UserPlaces {
        local: Some(profile.join(r"AppData\Local")),
        shortcut_folders: vec![
            profile.join("Desktop"),
            roaming.join(r"Microsoft\Windows\Start Menu\Programs"),
        ],
        roaming: Some(roaming),
    }
}

/// The profiles under `users` other than `mine`.
pub fn other_users_in(users: &Path, mine: Option<&Path>) -> OtherUsers {
    let mine = mine.map(|p| p.to_string_lossy().to_lowercase());
    let mut found = OtherUsers {
        readable: Vec::new(),
        unread: 0,
    };
    for entry in fs::read_dir(users).into_iter().flatten().flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let is_mine = mine.as_deref() == Some(path.to_string_lossy().to_lowercase().as_str());
        // A link is not a profile, and a file is not either.
        let is_folder = entry.file_type().is_ok_and(|t| t.is_dir());
        if is_mine || !is_folder || NOT_PROFILES.contains(&name.as_str()) {
            continue;
        }
        let app_data = path.join("AppData");
        match fs::read_dir(&app_data) {
            Ok(_) => found.readable.push(path),
            // The folder is there and it cannot be listed: it is a profile that is not ours.
            Err(_) if app_data.exists() || path.join("NTUSER.DAT").exists() => found.unread += 1,
            // No `AppData` at all: nobody has ever signed in to it.
            Err(_) => {}
        }
    }
    found.readable.sort();
    found
}

/// The other profiles of this computer.
pub fn other_users() -> OtherUsers {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let users = PathBuf::from(format!(r"{drive}\Users"));
    let mine = std::env::var_os("USERPROFILE").map(PathBuf::from);
    other_users_in(&users, mine.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_other_profiles_are_those_with_a_readable_appdata_and_the_rest_are_not_profiles() {
        let root = std::env::temp_dir().join(format!("purifex-users-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for dir in [
            r"me\AppData\Local",
            r"bob\AppData\Roaming",
            r"Public\Documents",
            r"Default\AppData",
            r"never\Documents",
        ] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(root.join("desktop.ini"), "").unwrap();
        let found = other_users_in(&root, Some(&root.join("ME")));
        let names: Vec<String> = found
            .readable
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            ["bob"],
            "not me, not Public or Default, not a file, not a folder nobody used"
        );
        assert_eq!(found.unread, 0);
        let places = found.places();
        assert_eq!(
            places[0].local,
            Some(root.join("bob").join(r"AppData\Local"))
        );
        assert!(places[0]
            .shortcut_folders
            .iter()
            .any(|f| f.ends_with("Desktop")));
        assert!(found.roots().iter().any(|r| r.ends_with("Documents")));
        let _ = fs::remove_dir_all(root);
    }
}
