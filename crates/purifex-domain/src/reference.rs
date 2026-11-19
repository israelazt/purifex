//! Does a text (a registry value, a shortcut target) refer to an app's folder or
//! program? Comparison is by whole path parts: `Kdenlive2` is not `Kdenlive`, and a
//! bare `kdenlive.exe` with no path proves nothing. The registry also writes drives
//! as `\\?\C:\`, `\\?\Volume{guid}\` and `\Device\HarddiskVolume3\`; all are read.

/// `text` contains the absolute folder `folder` (as `seeds` gives it) or a path inside it.
pub fn path_refers_to(text: &str, folder: &str) -> bool {
    let folder = normalise(folder);
    let Some((drive, tail)) = folder.split_once(':') else {
        return false;
    };
    if drive.len() != 1 || !tail.starts_with('\\') || tail.len() < 2 {
        return false;
    }
    let text = normalise(text);
    let by_drive = text.match_indices(tail).any(|(at, _)| {
        before_is_drive(&text[..at], drive) && boundary_after(&text[at + tail.len()..])
    });
    by_drive || by_known_folder(&text, tail)
}

/// Windows writes some paths with the id of a known folder instead of the drive:
/// `{6D809377-...}\Kdenlive\bin\kdenlive.exe` is `C:\Program Files\Kdenlive\bin\kdenlive.exe`.
/// Only folders of the machine, whose place does not depend on the user.
const KNOWN_FOLDERS: [(&str, &str); 3] = [
    ("{6d809377-6af0-444b-8957-a3773f02200e}", r"\program files\"),
    (
        "{7c5a40ef-a0fb-4bfc-874a-c0f2e0b9fa8e}",
        r"\program files (x86)\",
    ),
    ("{62ab5d82-fdc1-4dc3-a9dd-070d1d495d97}", r"\programdata\"),
];

/// `tail` (the folder without its drive) written as a known folder and what is inside it.
fn by_known_folder(text: &str, tail: &str) -> bool {
    KNOWN_FOLDERS.iter().any(|(id, folder)| {
        let Some(inside) = tail.strip_prefix(folder) else {
            return false;
        };
        let needle = format!(r"{id}\{inside}");
        text.match_indices(&needle).any(|(at, _)| {
            !alnum_before(&text[..at], 0) && boundary_after(&text[at + needle.len()..])
        })
    })
}

/// `text` contains a path that ends in the program `exe` (`…\bin\kdenlive.exe`).
/// A bare name, with no folder before it, is not a reference.
pub fn exe_refers_to(text: &str, exe: &str) -> bool {
    let exe = normalise(exe);
    if exe.is_empty() || exe.contains('\\') {
        return false;
    }
    let text = normalise(text);
    let needle = format!("\\{exe}");
    let found = text.match_indices(&needle).any(|(at, _)| {
        boundary_after(&text[at + needle.len()..]) && at > 0 && !text[..at].ends_with('\\')
    });
    found
}

fn normalise(text: &str) -> String {
    let mut t = text.to_lowercase().replace('/', "\\");
    for prefix in ["\\\\?\\", "\\??\\", "\\\\.\\"] {
        t = t.replace(prefix, "");
    }
    t
}

/// What comes right before the folder's tail: the same drive letter, or a volume
/// written without a letter.
fn before_is_drive(before: &str, drive: &str) -> bool {
    if let Some(rest) = before.strip_suffix(':') {
        return rest.ends_with(drive) && !alnum_before(rest, 1);
    }
    if let Some(rest) = before.strip_suffix('}') {
        return rest
            .rfind("volume{")
            .is_some_and(|i| !alnum_before(rest, rest.len() - i));
    }
    let digits = before.trim_end_matches(|c: char| c.is_ascii_digit());
    digits.len() < before.len() && digits.ends_with("harddiskvolume")
}

/// A letter or digit sits right before the last `skip` bytes of `s`.
fn alnum_before(s: &str, skip: usize) -> bool {
    s[..s.len() - skip]
        .chars()
        .last()
        .is_some_and(|c| c.is_alphanumeric())
}

/// The path ends here: end of text, a separator, or a quote or delimiter. A name
/// character means the match is the start of a longer name (`Kdenlive2`).
fn boundary_after(rest: &str) -> bool {
    match rest.chars().next() {
        None => true,
        Some(c) => !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' ' | '~' | '$' | '@')),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = r"C:\Program Files\Kdenlive";

    #[test]
    fn a_path_inside_the_folder_refers_and_a_longer_name_does_not() {
        assert!(path_refers_to(
            r"C:\Program Files\Kdenlive\bin\kdenlive.exe",
            DIR
        ));
        assert!(path_refers_to(r"C:\Program Files\Kdenlive", DIR));
        assert!(path_refers_to(r"C:\Program Files\Kdenlive\", DIR));
        assert!(!path_refers_to(
            r"C:\Program Files\Kdenlive2\bin\k.exe",
            DIR
        ));
        assert!(!path_refers_to(r"C:\Program Files\Kdenlive Pro\k.exe", DIR));
        assert!(!path_refers_to(r"C:\Program Files\Kdenlive-old", DIR));
        assert!(!path_refers_to(r"C:\Program Files\MyKdenlive\k.exe", DIR));
    }

    #[test]
    fn case_and_slash_direction_do_not_matter() {
        assert!(path_refers_to("c:/program files/KDENLIVE/bin/x.exe", DIR));
    }

    #[test]
    fn quotes_commas_and_arguments_end_the_path() {
        assert!(path_refers_to(
            r#""C:\Program Files\Kdenlive\k.exe" --x"#,
            DIR
        ));
        assert!(path_refers_to(
            r"@C:\Program Files\Kdenlive\k.dll,-101",
            DIR
        ));
        assert!(path_refers_to(r"C:\Program Files\Kdenlive;C:\Other", DIR));
    }

    #[test]
    fn the_registry_forms_of_a_drive_are_read() {
        assert!(path_refers_to(r"\\?\C:\Program Files\Kdenlive\k.exe", DIR));
        assert!(path_refers_to(
            r"\\?\Volume{1b2c-3d}\Program Files\Kdenlive\k.exe",
            DIR
        ));
        assert!(path_refers_to(
            r"\Device\HarddiskVolume3\Program Files\Kdenlive\k.exe",
            DIR
        ));
    }

    #[test]
    fn a_path_written_with_the_id_of_program_files_refers_to_the_folder_inside_it() {
        let id = "{6D809377-6AF0-444B-8957-A3773F02200E}";
        assert!(path_refers_to(
            &format!(r"{id}\Kdenlive\bin\kdenlive.exe"),
            DIR
        ));
        assert!(path_refers_to(&format!(r"{id}\Kdenlive"), DIR));
        assert!(
            !path_refers_to(&format!(r"{id}\Kdenlive2\bin\k.exe"), DIR),
            "a longer name"
        );
        assert!(
            !path_refers_to(&format!(r"X{id}\Kdenlive\k.exe"), DIR),
            "a stray prefix"
        );
        let x86 = "{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}";
        assert!(
            !path_refers_to(&format!(r"{x86}\Kdenlive\k.exe"), DIR),
            "the other Program Files"
        );
        assert!(path_refers_to(
            &format!(r"{x86}\Old\o.exe"),
            r"C:\Program Files (x86)\Old"
        ));
        assert!(
            !path_refers_to(&format!(r"{id}\Kdenlive\k.exe"), r"C:\Games\Kdenlive"),
            "a folder that is not in Program Files has no such form"
        );
    }

    #[test]
    fn another_drive_or_a_stray_prefix_is_not_the_same_folder() {
        assert!(!path_refers_to(r"D:\Program Files\Kdenlive\k.exe", DIR));
        assert!(!path_refers_to(r"XC:\Program Files\Kdenlive\k.exe", DIR));
        assert!(!path_refers_to(r"Program Files\Kdenlive\k.exe", DIR));
    }

    #[test]
    fn a_bad_folder_refers_to_nothing() {
        assert!(!path_refers_to("anything", ""));
        assert!(!path_refers_to(r"C:\x", r"C:\"));
        assert!(!path_refers_to(r"C:\x", "relative"));
    }

    #[test]
    fn a_program_needs_a_path_and_a_whole_name() {
        assert!(exe_refers_to(
            r"C:\Program Files\Kdenlive\bin\kdenlive.exe",
            "kdenlive.exe"
        ));
        assert!(exe_refers_to(r#""D:\x\Kdenlive.EXE" %1"#, "kdenlive.exe"));
        assert!(!exe_refers_to("kdenlive.exe", "kdenlive.exe"));
        assert!(!exe_refers_to(r"C:\x\mykdenlive.exe", "kdenlive.exe"));
        assert!(!exe_refers_to(r"C:\x\kdenlive.exe.bak", "kdenlive.exe"));
        assert!(!exe_refers_to(r"C:\x\kdenlive.exe", r"bin\kdenlive.exe"));
    }
}
