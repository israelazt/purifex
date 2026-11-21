//! Whether a program has a file open. A file that is open cannot be moved to the
//! quarantine, and it is the one case where waiting for the next restart works: so it is
//! told apart from the other reasons not to move something (permission, another disk).

use std::fs::{self, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;

/// `ERROR_SHARING_VIOLATION`: another program has the file open.
const SHARING_VIOLATION: i32 = 32;
/// How many files of a folder are tried before giving up: a folder that big is not one
/// program's leftover worth probing, and the answer would only be a guess.
const MAX_FILES: usize = 5_000;

/// True if the file, or any file inside the folder, is open elsewhere.
pub fn is_locked(path: &Path) -> bool {
    let mut pending = vec![path.to_path_buf()];
    let mut tried = 0;
    while let Some(next) = pending.pop() {
        if next.is_dir() {
            pending.extend(
                fs::read_dir(&next)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path()),
            );
            continue;
        }
        tried += 1;
        if tried > MAX_FILES {
            return false;
        }
        // Asking for no sharing at all fails when any other handle is open on the file.
        let attempt = OpenOptions::new().read(true).share_mode(0).open(&next);
        if attempt.is_err_and(|e| e.raw_os_error() == Some(SHARING_VIOLATION)) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;

    #[test]
    fn a_file_a_program_has_open_is_locked_and_a_free_one_or_a_folder_of_free_files_is_not() {
        let dir = std::env::temp_dir().join(format!("purifex-lock-{}", std::process::id()));
        let inner = dir.join("sub");
        fs::create_dir_all(&inner).unwrap();
        let busy = inner.join("busy.dll");
        let free = dir.join("free.txt");
        fs::write(&busy, b"x").unwrap();
        fs::write(&free, b"x").unwrap();
        assert!(!is_locked(&free) && !is_locked(&dir));
        let open = File::open(&busy).unwrap();
        assert!(is_locked(&busy), "the file itself");
        assert!(is_locked(&dir), "a file inside the folder");
        assert!(!is_locked(&free), "the others are not");
        drop(open);
        assert!(!is_locked(&dir));
        assert!(!is_locked(&dir.join("no-existe")));
        let _ = fs::remove_dir_all(dir);
    }
}
