//! The target of a Windows shortcut (`.lnk`), read from its LinkInfo block: the path of
//! a file on a local disk. Shortcuts to something that is not a plain path (a shell
//! item, a network share) have no target here and are not used as evidence.

/// The most a shortcut can weigh and still be read: they are small.
pub const MAX_LNK_BYTES: u64 = 64 * 1024;

fn u16_at(b: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?) as usize)
}

fn u32_at(b: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as usize)
}

fn ansi_at(b: &[u8], at: usize) -> Option<String> {
    let rest = b.get(at..)?;
    let end = rest.iter().position(|c| *c == 0)?;
    Some(String::from_utf8_lossy(&rest[..end]).into_owned())
}

fn utf16_at(b: &[u8], at: usize) -> Option<String> {
    let rest = b.get(at..)?;
    let units: Vec<u16> = rest
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes(*p))
        .take_while(|u| *u != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

/// The local path a shortcut points to.
pub fn target(bytes: &[u8]) -> Option<String> {
    if u32_at(bytes, 0)? != 0x4C {
        return None;
    }
    let flags = u32_at(bytes, 0x14)?;
    let mut at = 0x4C;
    if flags & 1 != 0 {
        // The list of the target's shell items: skipped.
        at += 2 + u16_at(bytes, at)?;
    }
    if flags & 2 == 0 {
        return None;
    }
    let info = at;
    let header = u32_at(bytes, info + 4)?;
    let info_flags = u32_at(bytes, info + 8)?;
    if info_flags & 1 == 0 {
        return None; // not a path on a local disk
    }
    let (base, suffix) = if header >= 0x24 {
        let (b, s) = (u32_at(bytes, info + 0x1C)?, u32_at(bytes, info + 0x20)?);
        (
            utf16_at(bytes, info + b)?,
            if s == 0 {
                String::new()
            } else {
                utf16_at(bytes, info + s)?
            },
        )
    } else {
        let (b, s) = (u32_at(bytes, info + 0x10)?, u32_at(bytes, info + 0x18)?);
        (
            ansi_at(bytes, info + b)?,
            if s == 0 {
                String::new()
            } else {
                ansi_at(bytes, info + s)?
            },
        )
    };
    let path = format!("{base}{suffix}");
    (!path.is_empty()).then_some(path)
}

#[cfg(test)]
/// A shortcut as Windows writes it: header, a shell item list, and LinkInfo.
pub(super) fn shortcut(path: &str, unicode: bool, id_list: bool) -> Vec<u8> {
    let mut out = vec![0u8; 0x4C];
    out[0] = 0x4C;
    let flags: u32 = 2 | u32::from(id_list);
    out[0x14..0x18].copy_from_slice(&flags.to_le_bytes());
    if id_list {
        out.extend(6u16.to_le_bytes());
        out.extend([1, 2, 3, 4, 0, 0]);
    }
    let header: u32 = if unicode { 0x24 } else { 0x1C };
    let ansi: Vec<u8> = path.bytes().chain([0]).collect();
    let wide: Vec<u8> = path
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect();
    let base_at = header as usize;
    let suffix_at = base_at + ansi.len();
    let wide_at = suffix_at + 1;
    let size = wide_at + if unicode { wide.len() + 2 } else { 0 };
    let mut info = Vec::new();
    for v in [
        size as u32,
        header,
        1,
        0,
        base_at as u32,
        0,
        suffix_at as u32,
    ] {
        info.extend(v.to_le_bytes());
    }
    if unicode {
        info.extend((wide_at as u32).to_le_bytes());
        info.extend(0u32.to_le_bytes());
    }
    info.extend(ansi);
    info.push(0);
    if unicode {
        info.extend(wide);
        info.extend([0, 0]);
    }
    out.extend(info);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_is_read_from_both_forms_of_the_block() {
        let path = r"C:\Program Files\Kdenlive\bin\kdenlive.exe";
        for (unicode, list) in [(true, true), (false, true), (true, false), (false, false)] {
            assert_eq!(
                target(&shortcut(path, unicode, list)).as_deref(),
                Some(path),
                "unicode {unicode} list {list}"
            );
        }
        let accented = r"C:\Users\Ñandú\App.exe";
        assert_eq!(
            target(&shortcut(accented, true, true)).as_deref(),
            Some(accented)
        );
    }

    #[test]
    fn what_is_not_a_local_path_or_is_cut_short_has_no_target() {
        assert_eq!(target(&[]), None);
        assert_eq!(
            target(b"not a shortcut at all, just text of some length........"),
            None
        );
        let mut good = shortcut(r"C:\x\y.exe", true, true);
        good.truncate(good.len() - 30);
        assert_eq!(target(&good), None);
        let mut network = shortcut(r"C:\x\y.exe", true, true);
        network[0x4C + 2 + 6 + 8] = 2; // LinkInfoFlags: no local base path
        assert_eq!(target(&network), None);
        let mut no_info = shortcut(r"C:\x\y.exe", true, false);
        no_info[0x14] = 0;
        assert_eq!(target(&no_info), None);
    }
}
