//! Removing traces one at a time (as the window does, in slices) against the real file system and
//! the real quarantine, inside a sandbox: the variables that name the places of Windows point to a
//! folder of this test, so nothing of the user is touched. One test, because it changes the
//! environment of the process.

use std::fs;

use purifex_app::{Cleaning, RemovalStart, Removals};
use purifex_domain::app_traces::{Trace, TraceKind};
use purifex_platform::{LocalCleanup, RegisteredUninstaller, WindowsTraceFinder};

fn folder(path: &std::path::Path, bytes: u64) -> Trace {
    Trace {
        kind: TraceKind::Folder,
        location: path.to_string_lossy().into_owned(),
        bytes,
        ..Default::default()
    }
}

#[test]
fn three_traces_go_one_by_one_to_the_real_quarantine_and_come_back_byte_for_byte() {
    let sandbox = std::env::temp_dir().join(format!("purifex-sliced-real-{}", std::process::id()));
    let _ = fs::remove_dir_all(&sandbox);
    let (roaming, local, data) = (sandbox.join("r"), sandbox.join("l"), sandbox.join("p"));
    for d in [&roaming, &local, &data] {
        fs::create_dir_all(d).unwrap();
    }
    std::env::set_var("APPDATA", &roaming);
    std::env::set_var("LOCALAPPDATA", &local);
    std::env::set_var("PROGRAMDATA", &data);

    let places = [
        roaming.join("Zzyzx Sandbox Tool"),
        local.join("Zzyzx Sandbox Tool"),
        data.join("Zzyzx Sandbox Tool"),
    ];
    let contents: [&[u8]; 3] = [b"one", b"two two", b"three three three"];
    for (place, content) in places.iter().zip(contents) {
        fs::create_dir_all(place).unwrap();
        fs::write(place.join("data.bin"), content).unwrap();
    }

    let mut cleaning = Cleaning::new(
        Box::new(WindowsTraceFinder),
        Box::new(RegisteredUninstaller),
        Box::new(LocalCleanup::new()),
    );
    let name = "Zzyzx Sandbox Tool";
    let mut start: RemovalStart = cleaning.begin_removal(&[], &[], name).unwrap();
    let mut done = Removals::default();
    for place in &places {
        cleaning
            .remove_one(name, &folder(place, 10), &mut start, &mut done)
            .unwrap();
        assert!(!place.exists(), "{place:?} must have moved");
    }
    assert!(done.failed.is_empty(), "{:?}", done.failed);
    assert_eq!(done.removed.len(), 3);
    assert!(
        fs::read_dir(local.join("Purifex")).is_ok(),
        "quarantine in the sandbox"
    );

    for entry in &done.removed {
        cleaning.restore(entry.seq).unwrap();
    }
    for (place, content) in places.iter().zip(contents) {
        assert_eq!(fs::read(place.join("data.bin")).unwrap(), content);
    }
    let _ = fs::remove_dir_all(&sandbox);
}
