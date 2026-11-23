//! Finding the junk of Windows: the direct children of a fixed list of places. Each child is a
//! file or a folder that moves to the quarantine as one. Links are never followed or taken,
//! and what was touched in the last day in the Temp folders is left (a program may be using it).

use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use purifex_domain::junk::{JunkItem, JunkKind};
use purifex_domain::ports::JunkFinder;

use crate::app_cleanup::folder_size;

const REPARSE_POINT: u32 = 0x400;
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

pub struct WindowsJunk;

fn env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

/// A place whose children are junk, and how old they must be.
struct Place {
    kind: JunkKind,
    folder: PathBuf,
    /// Only children whose name starts with this (the cache files of thumbnails).
    prefix: Option<&'static str>,
    min_age: Option<Duration>,
}

/// Where junk lives: what it is, the variable of the system its folder hangs from, the rest of
/// the path, the prefix its names need, and whether it must be old (a program may use it).
const PLACES: [(JunkKind, &str, &str, Option<&str>, bool); 10] = [
    (JunkKind::Temp, "TEMP", "", None, true),
    (JunkKind::Temp, "LOCALAPPDATA", "Temp", None, true),
    (JunkKind::Temp, "WINDIR", "Temp", None, true),
    (
        JunkKind::Thumbnails,
        "LOCALAPPDATA",
        r"Microsoft\Windows\Explorer",
        Some("thumbcache_"),
        false,
    ),
    (
        JunkKind::Thumbnails,
        "LOCALAPPDATA",
        r"Microsoft\Windows\Explorer",
        Some("iconcache_"),
        false,
    ),
    (
        JunkKind::CrashDumps,
        "LOCALAPPDATA",
        "CrashDumps",
        None,
        false,
    ),
    (JunkKind::CrashDumps, "WINDIR", "Minidump", None, false),
    (
        JunkKind::UpdateCache,
        "WINDIR",
        r"SoftwareDistribution\Download",
        None,
        false,
    ),
    (
        JunkKind::ErrorReports,
        "LOCALAPPDATA",
        r"Microsoft\Windows\WER",
        None,
        false,
    ),
    (
        JunkKind::ErrorReports,
        "PROGRAMDATA",
        r"Microsoft\Windows\WER",
        None,
        false,
    ),
];

fn places() -> Vec<Place> {
    PLACES
        .iter()
        .filter_map(|&(kind, variable, rest, prefix, aged)| {
            let base = env(variable)?;
            Some(Place {
                kind,
                folder: if rest.is_empty() {
                    base
                } else {
                    base.join(rest)
                },
                prefix,
                min_age: aged.then_some(DAY),
            })
        })
        .collect()
}

/// The direct children of `folder` that may be taken: not links, old enough, with the prefix.
fn children(
    folder: &Path,
    prefix: Option<&str>,
    min_age: Option<Duration>,
) -> Vec<(PathBuf, u64, bool)> {
    let Ok(read) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let now = SystemTime::now();
    read.flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if prefix.is_some_and(|p| !name.starts_with(p)) {
                return None;
            }
            let meta = entry.metadata().ok()?;
            if meta.file_attributes() & REPARSE_POINT != 0 {
                return None;
            }
            let recent = min_age.is_some_and(|age| {
                meta.modified()
                    .ok()
                    .and_then(|m| now.duration_since(m).ok())
                    .is_none_or(|old| old < age)
            });
            if recent {
                return None;
            }
            let path = entry.path();
            let bytes = if meta.is_dir() {
                folder_size(&path)
            } else {
                meta.len()
            };
            Some((path, bytes, meta.is_dir()))
        })
        .collect()
}

impl JunkFinder for WindowsJunk {
    fn scan(&self) -> Result<Vec<JunkItem>, String> {
        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::new();
        for place in places() {
            for (path, bytes, folder) in children(&place.folder, place.prefix, place.min_age) {
                let location = path.to_string_lossy().into_owned();
                if seen.insert(location.to_lowercase()) {
                    items.push(JunkItem {
                        kind: place.kind,
                        location,
                        bytes,
                        folder,
                    });
                }
            }
        }
        // The `MEMORY.DMP` of the system is a file of the Windows folder itself.
        if let Some(dump) = env("WINDIR")
            .map(|w| w.join("MEMORY.DMP"))
            .filter(|d| d.is_file())
        {
            let bytes = fs::metadata(&dump).map_or(0, |m| m.len());
            items.push(JunkItem {
                kind: JunkKind::CrashDumps,
                location: dump.to_string_lossy().into_owned(),
                bytes,
                folder: false,
            });
        }
        items.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.location.cmp(&b.location)));
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_old_enough_non_link_children_with_the_prefix_are_taken() {
        let dir = std::env::temp_dir().join(format!("purifex-junk-{}", std::process::id()));
        fs::create_dir_all(dir.join("old-folder")).unwrap();
        fs::write(dir.join("old-folder").join("a.bin"), b"12345").unwrap();
        fs::write(dir.join("old.tmp"), b"123").unwrap();
        fs::write(dir.join("new.tmp"), b"1").unwrap();
        fs::write(dir.join("thumbcache_32.db"), b"12").unwrap();
        let long_ago = SystemTime::now() - 3 * DAY;
        for name in ["old.tmp", "thumbcache_32.db"] {
            fs::File::options()
                .write(true)
                .open(dir.join(name))
                .unwrap()
                .set_modified(long_ago)
                .unwrap();
        }
        let names = |found: Vec<(PathBuf, u64, bool)>| {
            let mut n: Vec<String> = found
                .iter()
                .map(|(p, _, _)| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect();
            n.sort();
            n
        };
        assert_eq!(
            names(children(&dir, None, None)),
            ["new.tmp", "old-folder", "old.tmp", "thumbcache_32.db"]
        );
        let aged = names(children(&dir, None, Some(DAY)));
        assert!(
            aged.contains(&"old.tmp".to_string()) && !aged.contains(&"new.tmp".to_string()),
            "{aged:?}"
        );
        assert_eq!(
            names(children(&dir, Some("thumbcache_"), None)),
            ["thumbcache_32.db"]
        );
        let sized = children(&dir, Some("old"), None);
        assert!(sized
            .iter()
            .any(|(p, b, f)| *f && *b == 5 && p.ends_with("old-folder")));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_real_scan_is_well_formed_and_never_lists_something_twice() {
        let items = WindowsJunk.scan().unwrap();
        let mut places: Vec<String> = items.iter().map(|i| i.location.to_lowercase()).collect();
        places.sort_unstable();
        places.dedup();
        assert_eq!(places.len(), items.len());
        assert!(items.iter().all(|i| Path::new(&i.location).is_absolute()));
    }
}
