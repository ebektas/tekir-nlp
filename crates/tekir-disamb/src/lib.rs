//! Morphological disambiguation for tekir: an averaged perceptron over
//! analysis features with beam decoding, plus the UD treebank tooling used
//! to train and evaluate it. See `examples/train.rs`.

pub mod conllu;
pub mod mapping;
pub mod perceptron;
pub mod pipeline;

use tekir_morph::MorphAnalysis;

pub use perceptron::{BOUNDARY_TAG, Model, Trainer, decode, features, tag_of};
pub use pipeline::{DisambSentence, DisambToken, LemmaToken, Pipeline, bundled_model};

/// A trained disambiguator choosing one analysis per token in context.
#[derive(Debug, Clone, Default)]
pub struct Disambiguator {
    model: Model,
}

impl Disambiguator {
    pub fn new(model: Model) -> Self {
        Self { model }
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        Ok(Self {
            model: Model::load(path)?,
        })
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Pick the best analysis index for each token of a sentence.
    pub fn disambiguate(&self, sentence: &[Vec<MorphAnalysis>]) -> Vec<Option<usize>> {
        perceptron::decode(&self.model, sentence)
    }
}
