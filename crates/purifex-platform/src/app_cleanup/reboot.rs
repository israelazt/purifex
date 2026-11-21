//! Deleting at the next restart what a program has open: Windows keeps the list (in
//! `PendingFileRenameOperations`) and empties it before any program starts. A folder is not
//! deleted whole: its files go first and then the folders, deepest first, because Windows
//! only deletes an empty folder. Writing that list needs an administrator.

use std::fs;
use std::path::{Path, PathBuf};

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};

/// The most entries handed to Windows for one trace: a folder with more is left alone.
const MAX_ENTRIES: usize = 20_000;

/// What to hand to Windows, in the order it must be deleted: the files of a folder, then the
/// folders from the deepest up, and the folder itself last.
pub fn deletion_order(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut order = Vec::new();
    collect(root, &mut order)?;
    Ok(order)
}

fn collect(path: &Path, order: &mut Vec<PathBuf>) -> Result<(), String> {
    if order.len() > MAX_ENTRIES {
        return Err("tiene demasiados archivos para borrarlo al reiniciar".into());
    }
    // A link is deleted as a link: what it points to is not the app's to remove.
    let is_folder = fs::symlink_metadata(path).is_ok_and(|m| m.is_dir());
    if is_folder {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())?.flatten() {
            collect(&entry.path(), order)?;
        }
    }
    order.push(path.to_path_buf());
    Ok(())
}

/// Hands each path to Windows to delete at the next restart.
pub fn schedule(root: &Path) -> Result<(), String> {
    for path in deletion_order(root)? {
        // SAFETY: the path outlives the call; a null destination means "delete".
        let result = unsafe {
            MoveFileExW(
                &HSTRING::from(path.as_os_str()),
                PCWSTR::null(),
                MOVEFILE_DELAY_UNTIL_REBOOT,
            )
        };
        if let Err(e) = result {
            return Err(if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
                "hace falta ejecutar Purifex como administrador".to_string()
            } else {
                format!("Windows no lo programó: {e}")
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_go_before_their_folder_and_the_deepest_folders_before_the_others() {
        let dir = std::env::temp_dir().join(format!("purifex-reboot-{}", std::process::id()));
        let deep = dir.join("a").join("b");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("x.dll"), b"x").unwrap();
        fs::write(dir.join("y.txt"), b"y").unwrap();
        let order = deletion_order(&dir).unwrap();
        let at = |p: &Path| order.iter().position(|o| o == p).unwrap();
        assert!(at(&deep.join("x.dll")) < at(&deep));
        assert!(at(&deep) < at(&dir.join("a")));
        assert!(at(&dir.join("a")) < at(&dir));
        assert_eq!(order.last().unwrap(), &dir, "the folder itself is last");
        assert_eq!(order.len(), 5);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn without_administrator_nothing_is_scheduled_and_the_reason_is_said() {
        use purifex_domain::ports::Elevation;
        if crate::WindowsElevation.is_elevated() {
            return; // Here it would really schedule the deletion: not something to try.
        }
        let file = std::env::temp_dir().join(format!("purifex-reboot-file-{}", std::process::id()));
        fs::write(&file, b"x").unwrap();
        let error = schedule(&file).unwrap_err();
        let _ = fs::remove_file(&file);
        assert!(error.contains("administrador"), "{error}");
    }
}
