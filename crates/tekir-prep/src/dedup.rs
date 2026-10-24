//! Exact and near-duplicate detection.
//!
//! Near duplicates: MinHash signatures over word n-gram shingles, LSH
//! banding for candidate pairs, and union-find clustering. Shingles are
//! Turkish-lowercased alphanumeric words, so case and punctuation changes
//! do not hide a duplicate. Hashing is fixed (FNV-1a + murmur3 finalizer,
//! permutations `(a*x + b) mod (2^61 - 1)` drawn from a seeded splitmix64),
//! so signatures are a pure function of `(text, config)`: compute them on
//! any number of threads or machines and they compare equal.
//!
//! Typical pipeline: drop exact duplicates with [`exact_hash`] +
//! [`exact_duplicates`] first (it is cheap and keeps LSH buckets small),
//! then [`MinHasher::signatures`] and [`near_duplicates`].

use tekir_morph::batch::parallel_map;

use serde::Serialize;

use crate::text::{combine, fmix64, hash_lower_word};

const MERSENNE_61: u64 = (1 << 61) - 1;
/// Largest supported shingle size (the window lives on the stack).
pub const MAX_NGRAM: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MinHashConfig {
    /// Words per shingle.
    pub ngram: usize,
    /// Signature length (number of hash permutations).
    pub num_perm: usize,
    /// Seed for the permutation parameters.
    pub seed: u64,
}

impl Default for MinHashConfig {
    fn default() -> Self {
        Self {
            ngram: 5,
            num_perm: 128,
            seed: 0x7e41_6b69_725f_7072,
        }
    }
}

/// A MinHash signature: one minimum per permutation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Signature(pub Vec<u64>);

impl AsRef<[u64]> for Signature {
    fn as_ref(&self) -> &[u64] {
        &self.0
    }
}

/// Immutable MinHash signer; build once and share across threads.
#[derive(Debug, Clone)]
pub struct MinHasher {
    config: MinHashConfig,
    /// `(a, b)` per permutation, `1 <= a < p`, `0 <= b < p`.
    params: Vec<(u64, u64)>,
}

impl MinHasher {
    /// # Panics
    ///
    /// If `ngram` is 0 or above [`MAX_NGRAM`], or `num_perm` is 0.
    pub fn new(config: MinHashConfig) -> Self {
        assert!(
            (1..=MAX_NGRAM).contains(&config.ngram),
            "ngram must be in 1..={MAX_NGRAM}"
        );
        assert!(config.num_perm > 0, "num_perm must be positive");
        let mut state = config.seed;
        let params = (0..config.num_perm)
            .map(|_| {
                let a = splitmix64(&mut state) % (MERSENNE_61 - 1) + 1;
                let b = splitmix64(&mut state) % MERSENNE_61;
                (a, b)
            })
            .collect();
        Self { config, params }
    }

    pub fn config(&self) -> &MinHashConfig {
        &self.config
    }

    pub fn signature(&self, text: &str) -> Signature {
        let mut values = vec![0; self.config.num_perm];
        self.signature_into(text, &mut values);
        Signature(values)
    }

    /// Writes the signature of `text` into `out` (`num_perm` long). A text
    /// with fewer than `ngram` words is one shingle; a text with no words
    /// gets the all-`u64::MAX` signature (so empty texts match each other).
    pub fn signature_into(&self, text: &str, out: &mut [u64]) {
        assert_eq!(out.len(), self.params.len(), "out must be num_perm long");
        out.fill(u64::MAX);
        let mut emit = |shingle: u64| {
            let x = u128::from(shingle % MERSENNE_61);
            for (slot, &(a, b)) in out.iter_mut().zip(&self.params) {
                let value = mod_mersenne(u128::from(a) * x + u128::from(b));
                *slot = (*slot).min(value);
            }
        };
        for_each_shingle(text, self.config.ngram, &mut emit);
    }

    /// Signatures for many texts, in input order, on `threads` scoped
    /// threads (`0` = available parallelism). Output is identical for every
    /// thread count.
    pub fn signatures<T: AsRef<str> + Sync>(&self, texts: &[T], threads: usize) -> Vec<Signature> {
        parallel_map(texts, threads, |text| self.signature(text.as_ref()))
    }
}

/// Calls `emit` with the hash of every word `ngram`-shingle of `text`.
fn for_each_shingle(text: &str, ngram: usize, emit: &mut impl FnMut(u64)) {
    let mut window = [0u64; MAX_NGRAM];
    let mut count = 0usize;
    let shingle = |window: &[u64; MAX_NGRAM], count: usize, len: usize| {
        let mut hash = len as u64;
        for offset in 0..len {
            hash = combine(hash, window[(count - len + offset) % ngram]);
        }
        fmix64(hash)
    };
    for word in words(text) {
        window[count % ngram] = hash_lower_word(word);
        count += 1;
        if count >= ngram {
            emit(shingle(&window, count, ngram));
        }
    }
    if (1..ngram).contains(&count) {
        emit(shingle(&window, count, count));
    }
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// `x mod (2^61 - 1)` for `x < 2^123`.
#[inline]
fn mod_mersenne(x: u128) -> u64 {
    let folded = (x & u128::from(MERSENNE_61)) + (x >> 61);
    let folded = ((folded & u128::from(MERSENNE_61)) + (folded >> 61)) as u64;
    if folded >= MERSENNE_61 {
        folded - MERSENNE_61
    } else {
        folded
    }
}

/// Fraction of agreeing positions: an unbiased estimate of the Jaccard
/// similarity of the two shingle sets.
pub fn jaccard_estimate(a: &[u64], b: &[u64]) -> f64 {
    assert_eq!(a.len(), b.len(), "signatures must have equal length");
    if a.is_empty() {
        return 0.0;
    }
    let equal = a.iter().zip(b).filter(|(x, y)| x == y).count();
    equal as f64 / a.len() as f64
}

/// LSH banding: `bands` bands of `rows` signature values each. Two
/// documents become candidates when any band matches entirely; the
/// similarity at which that happens with probability 1/2 is roughly
/// `(1/bands)^(1/rows)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LshParams {
    pub bands: usize,
    pub rows: usize,
}

impl LshParams {
    /// The `bands * rows == num_perm` split whose threshold
    /// `(1/bands)^(1/rows)` is closest to `threshold`.
    pub fn for_threshold(num_perm: usize, threshold: f64) -> Self {
        (1..=num_perm)
            .filter(|rows| num_perm % rows == 0)
            .map(|rows| Self {
                bands: num_perm / rows,
                rows,
            })
            .min_by(|x, y| {
                let distance = |p: &Self| (p.threshold() - threshold).abs();
                distance(x).total_cmp(&distance(y))
            })
            .expect("num_perm must be positive")
    }

    /// Approximate similarity threshold `(1/bands)^(1/rows)`.
    pub fn threshold(&self) -> f64 {
        (1.0 / self.bands as f64).powf(1.0 / self.rows as f64)
    }
}

/// Candidate pairs `(i, j)`, `i < j`, sharing at least one LSH band;
/// sorted and deduplicated. Buckets are found by sorting, so the result is
/// deterministic. A bucket of `k` documents yields `k*(k-1)/2` pairs, so
/// remove exact duplicates first.
///
/// # Panics
///
/// If a signature is shorter than `bands * rows`.
pub fn candidate_pairs<S: AsRef<[u64]>>(
    signatures: &[S],
    params: LshParams,
) -> Vec<(usize, usize)> {
    let width = params.bands * params.rows;
    let mut keys: Vec<(u64, usize)> = Vec::with_capacity(signatures.len());
    let mut pairs = Vec::new();
    for band in 0..params.bands {
        keys.clear();
        keys.extend(signatures.iter().enumerate().map(|(doc, signature)| {
            let signature = signature.as_ref();
            assert!(
                signature.len() >= width,
                "signature shorter than bands * rows"
            );
            let rows = &signature[band * params.rows..(band + 1) * params.rows];
            let key = rows
                .iter()
                .fold(band as u64, |hash, &value| combine(hash, value));
            (fmix64(key), doc)
        }));
        keys.sort_unstable();
        for bucket in keys.chunk_by(|x, y| x.0 == y.0) {
            for (offset, &(_, i)) in bucket.iter().enumerate() {
                pairs.extend(bucket[offset + 1..].iter().map(|&(_, j)| (i, j)));
            }
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

/// Union-find whose root is always the lowest index of its set.
#[derive(Debug, Clone)]
pub struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    pub fn new(len: usize) -> Self {
        Self {
            parent: (0..len).collect(),
        }
    }

    pub fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    pub fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        let (low, high) = if a < b { (a, b) } else { (b, a) };
        self.parent[high] = low;
    }

    /// The representative (lowest index) of every element's set.
    pub fn representatives(&mut self) -> Vec<usize> {
        (0..self.parent.len()).map(|x| self.find(x)).collect()
    }
}

/// Clusters `len` documents connected by `pairs`; returns each document's
/// representative (lowest index in its cluster). Keep documents where
/// `representatives[i] == i`.
pub fn cluster_pairs(len: usize, pairs: &[(usize, usize)]) -> Vec<usize> {
    let mut sets = UnionFind::new(len);
    for &(a, b) in pairs {
        sets.union(a, b);
    }
    sets.representatives()
}

/// LSH candidates verified by `jaccard_estimate >= threshold`, clustered.
/// Returns each document's representative.
pub fn near_duplicates<S: AsRef<[u64]>>(
    signatures: &[S],
    params: LshParams,
    threshold: f64,
) -> Vec<usize> {
    let mut pairs = candidate_pairs(signatures, params);
    pairs.retain(|&(i, j)| {
        jaccard_estimate(signatures[i].as_ref(), signatures[j].as_ref()) >= threshold
    });
    cluster_pairs(signatures.len(), &pairs)
}

/// 128-bit hash of the exact bytes of `text` (two independent 64-bit
/// lanes). Normalize first if whitespace or case differences should not
/// matter.
pub fn exact_hash(text: &str) -> u128 {
    let bytes = text.as_bytes();
    let low = hash_bytes(bytes, 0x243f_6a88_85a3_08d3);
    let high = hash_bytes(bytes, 0x1319_8a2e_0370_7344);
    (u128::from(high) << 64) | u128::from(low)
}

fn hash_bytes(bytes: &[u8], seed: u64) -> u64 {
    const K: u64 = 0x9fb2_1c65_1e98_df25;
    let mut hash = seed ^ (bytes.len() as u64).wrapping_mul(K);
    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        let word = u64::from_le_bytes(chunk.try_into().expect("8-byte chunk"));
        hash = (hash ^ fmix64(word ^ seed)).rotate_left(29).wrapping_mul(K);
    }
    let mut tail = [0u8; 8];
    tail[..chunks.remainder().len()].copy_from_slice(chunks.remainder());
    hash ^= fmix64(u64::from_le_bytes(tail) ^ seed);
    fmix64(hash)
}

/// Each document's representative among exact-hash duplicates (lowest
/// index with the same hash).
pub fn exact_duplicates(hashes: &[u128]) -> Vec<usize> {
    let mut order: Vec<(u128, usize)> = hashes.iter().copied().zip(0..).collect();
    order.sort_unstable();
    let mut representatives = vec![0; hashes.len()];
    for group in order.chunk_by(|x, y| x.0 == y.0) {
        let first = group[0].1;
        for &(_, doc) in group {
            representatives[doc] = first;
        }
    }
    representatives
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        LshParams, MinHashConfig, MinHasher, UnionFind, candidate_pairs, cluster_pairs,
        exact_duplicates, exact_hash, for_each_shingle, jaccard_estimate, mod_mersenne,
        near_duplicates, splitmix64,
    };

    /// Deterministic synthetic text: `len` words from a 5000-word
    /// vocabulary.
    fn synthetic(seed: u64, len: usize) -> Vec<String> {
        let mut state = seed;
        (0..len)
            .map(|_| format!("k{}", splitmix64(&mut state) % 5000))
            .collect()
    }

    fn shingle_set(text: &str, ngram: usize) -> HashSet<u64> {
        let mut set = HashSet::new();
        for_each_shingle(text, ngram, &mut |hash| {
            set.insert(hash);
        });
        set
    }

    #[test]
    fn mersenne_reduction_matches_modulo() {
        let p = u128::from((1u64 << 61) - 1);
        for x in [
            0u128,
            1,
            p - 1,
            p,
            p + 1,
            p * p - 1,
            (p - 1) * (p - 1) + p - 1,
        ] {
            assert_eq!(u128::from(mod_mersenne(x)), x % p, "{x}");
        }
    }

    #[test]
    fn signatures_are_deterministic_and_case_insensitive() {
        let hasher = MinHasher::new(MinHashConfig::default());
        let a = hasher.signature("İSTANBUL'DA yağmur yağıyor, IRMAK taştı ve yollar kapandı.");
        let b = hasher.signature("istanbul'da yağmur yağıyor ırmak taştı ve yollar kapandı");
        assert_eq!(a, b);
        assert_eq!(
            a,
            MinHasher::new(MinHashConfig::default())
                .signature("istanbul da yağmur yağıyor ırmak taştı ve yollar kapandı")
        );
        let other_seed = MinHasher::new(MinHashConfig {
            seed: 1,
            ..MinHashConfig::default()
        });
        assert_ne!(
            a,
            other_seed.signature("istanbul da yağmur yağıyor ırmak taştı ve yollar kapandı")
        );
    }

    #[test]
    fn short_and_empty_texts() {
        let hasher = MinHasher::new(MinHashConfig::default());
        assert!(hasher.signature("").0.iter().all(|&v| v == u64::MAX));
        assert!(
            hasher
                .signature("iki kelime")
                .0
                .iter()
                .all(|&v| v != u64::MAX)
        );
        assert_ne!(
            hasher.signature("iki kelime"),
            hasher.signature("kelime iki")
        );
    }

    #[test]
    fn estimate_tracks_exact_jaccard() {
        let config = MinHashConfig {
            ngram: 3,
            num_perm: 256,
            seed: 42,
        };
        let hasher = MinHasher::new(config);
        let base = synthetic(7, 400);
        // Replace a growing fraction of words to sweep similarity.
        for (step, replace_every) in [2usize, 3, 5, 10, 25, 100].into_iter().enumerate() {
            let noise = synthetic(1000 + step as u64, base.len());
            let edited: Vec<&str> = base
                .iter()
                .zip(&noise)
                .enumerate()
                .map(|(i, (word, other))| {
                    if i % replace_every == 0 { other } else { word }.as_str()
                })
                .collect();
            let (a, b) = (base.join(" "), edited.join(" "));

            let (sa, sb) = (shingle_set(&a, 3), shingle_set(&b, 3));
            let exact = sa.intersection(&sb).count() as f64 / sa.union(&sb).count() as f64;
            let estimate = jaccard_estimate(&hasher.signature(&a).0, &hasher.signature(&b).0);
            // Standard error at 256 permutations is at most ~0.031.
            assert!(
                (estimate - exact).abs() < 0.1,
                "every {replace_every}: exact {exact:.3}, estimate {estimate:.3}"
            );
        }
    }

    #[test]
    fn parallel_signatures_match_sequential() {
        let hasher = MinHasher::new(MinHashConfig::default());
        let texts: Vec<String> = (0..37).map(|seed| synthetic(seed, 50).join(" ")).collect();
        let sequential: Vec<_> = texts.iter().map(|text| hasher.signature(text)).collect();
        for threads in [0, 1, 4, 64] {
            assert_eq!(hasher.signatures(&texts, threads), sequential, "{threads}");
        }
        assert!(hasher.signatures::<&str>(&[], 4).is_empty());
    }

    #[test]
    fn lsh_params_for_threshold() {
        let params = LshParams::for_threshold(128, 0.8);
        assert_eq!(params.bands * params.rows, 128);
        assert!((params.threshold() - 0.8).abs() < 0.1);
    }

    #[test]
    fn finds_and_clusters_near_duplicates() {
        let hasher = MinHasher::new(MinHashConfig::default());
        let base = synthetic(3, 300);
        let mut near = base.clone();
        near[150] = "değişti".to_string();
        let mut nearer = near.clone();
        nearer[10] = "başka".to_string();
        let texts = [
            synthetic(4, 300).join(" "),
            base.join(" "),
            synthetic(5, 300).join(" "),
            near.join(" "),
            nearer.join(" "),
        ];
        let signatures = hasher.signatures(&texts, 2);
        let params = LshParams::for_threshold(128, 0.8);

        let pairs = candidate_pairs(&signatures, params);
        assert!(pairs.contains(&(1, 3)) && pairs.contains(&(3, 4)));
        assert!(pairs.iter().all(|&(i, _)| i != 0 && i != 2));

        assert_eq!(near_duplicates(&signatures, params, 0.8), [0, 1, 2, 1, 1]);
    }

    #[test]
    fn union_find_uses_lowest_index_as_representative() {
        assert_eq!(
            cluster_pairs(6, &[(4, 5), (3, 5), (1, 2)]),
            [0, 1, 1, 3, 3, 3]
        );
        let mut sets = UnionFind::new(4);
        sets.union(3, 2);
        sets.union(2, 0);
        assert_eq!(sets.representatives(), [0, 1, 0, 0]);
    }

    #[test]
    fn exact_duplicates_by_hash() {
        let texts = ["a b", "c", "a b", "a  b", "c"];
        let hashes: Vec<u128> = texts.iter().map(|text| exact_hash(text)).collect();
        assert_eq!(exact_duplicates(&hashes), [0, 1, 0, 3, 1]);
        assert_ne!(exact_hash(""), exact_hash("\0"));
        assert_ne!(exact_hash("12345678"), exact_hash("12345679"));
    }
}
