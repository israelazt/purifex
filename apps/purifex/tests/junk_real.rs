//! The junk cleaner against the real file system and the real quarantine, inside a sandbox: the
//! variables that name the places of Windows point to a folder of this test, so nothing of the
//! user is touched. This file holds one test because it changes the environment of the process.

use std::fs;
use std::time::{Duration, SystemTime};

use purifex_app::Cleaning;
use purifex_domain::junk::JunkKind;
use purifex_platform::{LocalCleanup, RegisteredUninstaller, WindowsJunk, WindowsTraceFinder};

fn age(path: &std::path::Path, days: u64) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(days * 86_400))
        .unwrap();
}

#[test]
fn old_temp_files_go_to_the_real_quarantine_and_come_back_whole() {
    let sandbox = std::env::temp_dir().join(format!("purifex-junk-real-{}", std::process::id()));
    let _ = fs::remove_dir_all(&sandbox);
    let (temp, local, win) = (sandbox.join("t"), sandbox.join("l"), sandbox.join("w"));
    for d in [&temp, &local, &win] {
        fs::create_dir_all(d).unwrap();
    }
    std::env::set_var("TEMP", &temp);
    std::env::set_var("LOCALAPPDATA", &local);
    std::env::set_var("WINDIR", &win);
    std::env::set_var("PROGRAMDATA", sandbox.join("p"));

    fs::write(temp.join("old.tmp"), b"old content").unwrap();
    age(&temp.join("old.tmp"), 3);
    fs::create_dir_all(temp.join("old-dir")).unwrap();
    fs::write(temp.join("old-dir").join("inner.bin"), b"12345").unwrap();
    fs::write(temp.join("fresh.tmp"), b"in use today").unwrap();

    let mut cleaning = Cleaning::new(
        Box::new(WindowsTraceFinder),
        Box::new(RegisteredUninstaller),
        Box::new(LocalCleanup::new()),
    )
    .with_junk(Box::new(WindowsJunk));

    let outcome = cleaning.clean_junk(&[JunkKind::Temp]).unwrap();
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    assert!(
        !temp.join("old.tmp").exists(),
        "the old file must have moved"
    );
    assert!(temp.join("fresh.tmp").exists(), "a file of today must stay");
    assert!(
        fs::read_dir(local.join("Purifex")).is_ok(),
        "the quarantine must live under the sandbox"
    );

    // Put it all back from the quarantine and compare the bytes.
    for seq in 1..=outcome.removed as u64 + 1 {
        let _ = cleaning.restore(seq);
    }
    assert_eq!(fs::read(temp.join("old.tmp")).unwrap(), b"old content");
    assert_eq!(
        fs::read(temp.join("old-dir").join("inner.bin")).unwrap(),
        b"12345"
    );
    let _ = fs::remove_dir_all(&sandbox);
}
