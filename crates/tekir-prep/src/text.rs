//! Shared text helpers: Turkish-correct lowercasing and fixed,
//! seed-free hashing (FNV-1a + murmur3 finalizer) so results never depend
//! on OS randomness.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Calls `f` with the Turkish lowercase form of `ch` (`I -> ı`, `İ -> i`).
/// `tekir-normalize` only exposes whole-string normalization, so the
/// per-char rule lives here to keep hot loops allocation-free.
#[inline]
pub(crate) fn for_each_lower(ch: char, mut f: impl FnMut(char)) {
    match ch {
        'I' => f('ı'),
        'İ' => f('i'),
        _ if ch.is_ascii() => f(ch.to_ascii_lowercase()),
        _ => ch.to_lowercase().for_each(f),
    }
}

/// Appends the Turkish lowercase form of `word` to `out`.
pub(crate) fn push_lower(word: &str, out: &mut String) {
    for ch in word.chars() {
        for_each_lower(ch, |lower| out.push(lower));
    }
}

/// murmur3's 64-bit finalizer: a bijective avalanche mix.
#[inline]
pub(crate) fn fmix64(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

#[inline]
fn fnv_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Hash of the Turkish lowercase form of `word`, without allocating.
pub(crate) fn hash_lower_word(word: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    let mut buf = [0u8; 4];
    for ch in word.chars() {
        for_each_lower(ch, |lower| {
            hash = fnv_bytes(hash, lower.encode_utf8(&mut buf).as_bytes());
        });
    }
    fmix64(hash)
}

/// Order-sensitive combination of word hashes.
#[inline]
pub(crate) fn combine(hash: u64, item: u64) -> u64 {
    (hash.rotate_left(27) ^ item).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

/// Fixed-seed hasher for in-crate hash maps (deterministic and cheaper
/// than SipHash for the short keys used here).
#[derive(Debug, Clone, Copy)]
pub(crate) struct FastHasher(u64);

impl Default for FastHasher {
    fn default() -> Self {
        Self(FNV_OFFSET)
    }
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.0 = fnv_bytes(self.0, bytes);
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = combine(self.0, value);
    }

    fn finish(&self) -> u64 {
        fmix64(self.0)
    }
}

pub(crate) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;
pub(crate) type FastSet<K> = HashSet<K, BuildHasherDefault<FastHasher>>;

/// Hash of a string's bytes.
pub(crate) fn hash_str(text: &str) -> u64 {
    fmix64(fnv_bytes(FNV_OFFSET, text.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::{hash_lower_word, push_lower};

    #[test]
    fn lowercases_dotted_and_dotless_i() {
        let mut out = String::new();
        push_lower("IŞIK İzmir ÇĞÖÜ", &mut out);
        assert_eq!(out, "ışık izmir çğöü");
        assert_eq!(hash_lower_word("IŞIK"), hash_lower_word("ışık"));
        assert_ne!(hash_lower_word("ISIK"), hash_lower_word("isik"));
    }
}
