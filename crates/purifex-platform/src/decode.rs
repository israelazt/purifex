//! Reads the bytes Windows returns for a device property. Plain functions, no
//! system calls.

/// The UTF-16 units (little endian) in `bytes`; a last odd byte is dropped.
pub fn utf16_units(bytes: &[u8]) -> Vec<u16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect()
}

/// A UTF-16 (little endian) string, up to the first NUL. `None` if empty.
pub fn utf16_string(bytes: &[u8]) -> Option<String> {
    let units: Vec<u16> = utf16_units(bytes)
        .into_iter()
        .take_while(|&u| u != 0)
        .collect();
    let text = String::from_utf16_lossy(&units);
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// A list of UTF-16 strings, each ending in NUL and the list in an extra NUL
/// (`REG_MULTI_SZ`).
pub fn utf16_list(bytes: &[u8]) -> Vec<String> {
    utf16_units(bytes)
        .split(|&u| u == 0)
        .take_while(|part| !part.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

/// A little-endian `u64` (a `FILETIME`), if there are eight bytes.
pub fn u64_le(bytes: &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn a_string_stops_at_the_nul_and_empty_is_none() {
        let mut bytes = wide("NVIDIA");
        bytes.extend([0, 0, b'x', 0]);
        assert_eq!(utf16_string(&bytes).as_deref(), Some("NVIDIA"));
        assert_eq!(utf16_string(&[0, 0]), None);
        assert_eq!(utf16_string(&[]), None);
    }

    #[test]
    fn a_list_has_one_item_per_nul_and_ends_at_the_empty_one() {
        let mut bytes = wide("PCI\\VEN_10DE&DEV_2504");
        bytes.extend([0, 0]);
        bytes.extend(wide("PCI\\VEN_10DE"));
        bytes.extend([0, 0, 0, 0]);
        assert_eq!(
            utf16_list(&bytes),
            ["PCI\\VEN_10DE&DEV_2504", "PCI\\VEN_10DE"]
        );
        assert!(utf16_list(&[0, 0, 0, 0]).is_empty());
    }

    #[test]
    fn eight_bytes_make_a_number() {
        assert_eq!(u64_le(&1u64.to_le_bytes()), Some(1));
        assert_eq!(u64_le(&[1, 2, 3]), None);
    }
}
