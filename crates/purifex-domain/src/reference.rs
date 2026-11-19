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
    let found = text.match_indices(tail).any(|(at, _)| {
        before_is_drive(&text[..at], drive) && boundary_after(&text[at + tail.len()..])
    });
    found
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
