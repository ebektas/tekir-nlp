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
mod text;
