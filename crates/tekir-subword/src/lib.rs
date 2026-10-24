//! Morphology-aware subword tooling for Turkish LLM work: lossless
//! pre-tokenization at morpheme boundaries, byte-level BPE training that
//! respects them, and tokenizer evaluation against tekir's segmentation.

pub mod pretok;
pub mod scan;

pub use pretok::{BoundaryCache, Pretokenizer};
