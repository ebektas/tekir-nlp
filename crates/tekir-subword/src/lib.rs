//! Morphology-aware subword tooling for Turkish LLM work: lossless
//! pre-tokenization at morpheme boundaries, byte-level BPE training that
//! respects them, and tokenizer evaluation against tekir's segmentation.

pub mod bpe;
pub mod eval;
pub mod pretok;
pub mod scan;

pub use bpe::{Encoded, TekirTokenizer, TrainConfig, train};
pub use eval::{EvalReport, evaluate};
pub use pretok::{BoundaryCache, Pretokenizer};
