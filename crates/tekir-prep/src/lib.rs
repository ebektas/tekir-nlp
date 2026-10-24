//! Corpus-scale data-cleaning primitives for Turkish LLM training data:
//! PII detection and redaction ([`pii`]), near-duplicate detection
//! ([`dedup`]), and per-document quality signals with a threshold filter
//! ([`quality`]).
//!
//! Everything here is a pure function of its input (no OS randomness, no
//! global state, no locks), so results are byte-identical regardless of
//! thread count, and every public type is `Send + Sync`.

pub mod dedup;
pub mod pii;
pub mod quality;
mod text;

#[cfg(test)]
mod tests {
    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn public_types_are_send_and_sync() {
        assert_send_sync::<crate::pii::PiiSpan>();
        assert_send_sync::<crate::dedup::MinHasher>();
        assert_send_sync::<crate::dedup::Signature>();
        assert_send_sync::<crate::dedup::UnionFind>();
        assert_send_sync::<crate::quality::QualityScorer>();
        assert_send_sync::<crate::quality::QualityScratch>();
        assert_send_sync::<crate::quality::QualityConfig>();
    }
}
