//! A driver version (`31.0.101.5186`): four numbers compared from left to
//! right, never as text (`10.0` is older than `9.0` as a string, not as a
//! version).

use std::fmt;

use serde::{Serialize, Serializer};

/// Up to four numbers, `major.minor.build.revision`. Missing ones are zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DriverVersion([u16; 4]);

impl DriverVersion {
    pub fn new(parts: [u16; 4]) -> Self {
        Self(parts)
    }

    /// Reads `a`, `a.b`, `a.b.c` or `a.b.c.d`. Anything else (empty, text,
    /// more than four numbers, a number that does not fit) is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = [0u16; 4];
        for (count, piece) in text.trim().split('.').enumerate() {
            if count == 4 || piece.is_empty() || !piece.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            parts[count] = piece.parse().ok()?;
        }
        Some(Self(parts))
    }
}

impl fmt::Display for DriverVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d] = self.0;
        write!(f, "{a}.{b}.{c}.{d}")
    }
}

impl Serialize for DriverVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_not_by_text() {
        let old = DriverVersion::parse("9.0.0.1").unwrap();
        let new = DriverVersion::parse("10.0.0.0").unwrap();
        assert!(new > old);
        assert!("10.0" < "9.0", "as text it would go the other way");
    }

    #[test]
    fn missing_parts_are_zero_and_it_prints_four() {
        assert_eq!(
            DriverVersion::parse("31.0").unwrap().to_string(),
            "31.0.0.0"
        );
        assert_eq!(
            DriverVersion::parse(" 31.0.101.5186 ").unwrap().to_string(),
            "31.0.101.5186"
        );
    }

    #[test]
    fn what_is_not_a_version_is_rejected() {
        for bad in [
            "",
            "a.b",
            "1..2",
            "1.2.3.4.5",
            "70000.1",
            "-1.0",
            "1.0 beta",
        ] {
            assert_eq!(DriverVersion::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn it_serializes_as_text() {
        let json = serde_json::to_string(&DriverVersion::new([1, 2, 3, 4])).unwrap();
        assert_eq!(json, "\"1.2.3.4\"");
    }
}
