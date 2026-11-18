//! A secret that cannot be guessed: the token of one run of the local page.
//! The standard library seeds its hash maps from the system's random source, so
//! each `RandomState` carries keys nobody outside the process can know; hashing
//! different inputs under four of them gives 256 bits.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

/// 64 hexadecimal characters, different on every call.
pub fn random_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut token = String::with_capacity(64);
    for round in 0u32..4 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u128(nanos);
        hasher.write_u32(std::process::id());
        hasher.write_u32(round);
        token.push_str(&format!("{:016x}", hasher.finish()));
    }
    token
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_64_hex_characters_and_never_repeats() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        let rounds: Vec<&str> = (0..4).map(|i| &a[i * 16..i * 16 + 16]).collect();
        assert!(
            rounds.windows(2).all(|w| w[0] != w[1]),
            "each round has its own keys"
        );
    }
}
