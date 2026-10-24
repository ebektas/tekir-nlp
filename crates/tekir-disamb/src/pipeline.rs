//! The end-to-end sentence pipeline: Turkish tokenization, morphological
//! analysis over the full lexicon, and contextual disambiguation.

use serde::Serialize;
use tekir_morph::{Lexicon, MorphAnalysis, MorphMode, MorphService};
use tekir_tokenize::{TokenizeMode, TokenizeService};

use crate::Disambiguator;
use crate::conllu::turkish_lowercase;
use crate::perceptron::Model;

/// The disambiguator trained on the four CC BY-SA TABILEX treebanks,
/// embedded in the crate.
pub fn bundled_model() -> Model {
    Model::from_bytes(include_bytes!("../models/kenet.tkd")).expect("embedded model must parse")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisambToken {
    pub surface: String,
    /// The contextually chosen analysis, if the token has any.
    pub analysis: Option<MorphAnalysis>,
    pub candidate_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisambSentence {
    pub tokens: Vec<DisambToken>,
}

/// Tokenizer + analyzer + disambiguator, built once and shareable across
/// threads.
#[derive(Debug, Clone)]
pub struct Pipeline {
    service: MorphService,
    disambiguator: Disambiguator,
}

impl Pipeline {
    pub fn new(service: MorphService, disambiguator: Disambiguator) -> Self {
        Self {
            service,
            disambiguator,
        }
    }

    /// Full embedded lexicon and the bundled model.
    pub fn bundled() -> Self {
        Self::new(
            MorphService::with_lexicon(Lexicon::full()),
            Disambiguator::new(bundled_model()),
        )
    }

    pub fn service(&self) -> &MorphService {
        &self.service
    }

    /// Tokenize one sentence, analyze each token, and pick the best
    /// analysis per token in context.
    pub fn analyze_sentence(&self, text: &str) -> DisambSentence {
        let tokens = TokenizeService
            .tokenize(text, TokenizeMode::Turkish)
            .output
            .tokens;

        let candidates: Vec<Vec<MorphAnalysis>> = tokens
            .iter()
            .map(|token| {
                let surface = if token.text.contains('\'') || token.text.contains('’') {
                    token.text.clone()
                } else {
                    turkish_lowercase(&token.text)
                };
                self.service
                    .analyze(surface, MorphMode::NounBaseline)
                    .output
                    .analyses
            })
            .collect();
        let choices = self.disambiguator.disambiguate(&candidates);

        DisambSentence {
            tokens: tokens
                .into_iter()
                .zip(candidates)
                .zip(choices)
                .map(|((token, candidates), choice)| DisambToken {
                    surface: token.text,
                    candidate_count: candidates.len(),
                    analysis: choice.and_then(|index| candidates.into_iter().nth(index)),
                })
                .collect(),
        }
    }
}

/// One lemma per token, for search-analyzer use cases.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LemmaToken {
    pub surface: String,
    pub lemma: Option<String>,
}

impl Pipeline {
    /// Disambiguated lemmas for a sentence (search stemming).
    pub fn lemmas(&self, text: &str) -> Vec<LemmaToken> {
        self.analyze_sentence(text)
            .tokens
            .into_iter()
            .map(|token| LemmaToken {
                lemma: token.analysis.map(|analysis| analysis.lemma),
                surface: token.surface,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Pipeline;

    #[test]
    fn disambiguates_a_sentence_end_to_end() {
        let pipeline = Pipeline::bundled();
        let sentence = pipeline.analyze_sentence("Bana göre bu kitap güzeldi.");

        let gloss = |index: usize| {
            sentence.tokens[index]
                .analysis
                .as_ref()
                .map(|analysis| analysis.lexical_form.as_str())
                .unwrap_or("<none>")
        };
        assert_eq!(sentence.tokens.len(), 6);
        assert_eq!(gloss(0), "ben+Pron+Pers+A1sg+Dat");
        assert_eq!(gloss(1), "göre+Postp+PCDat");
        assert!(gloss(3).starts_with("kitap+Noun"));
        assert!(gloss(4).contains("+Verb+Zero+Past"));
        assert_eq!(gloss(5), "<none>");
    }
}

#[cfg(test)]
mod large_tests {
    use super::Pipeline;
    use std::time::Instant;

    /// A five-thousand-token "sentence" must decode within a sane bound.
    #[test]
    fn long_sentences_decode() {
        let pipeline = Pipeline::bundled();
        let text = "evdeki çocuklar kitap okuyorlardı ".repeat(1_250);
        let started = Instant::now();
        let sentence = pipeline.analyze_sentence(&text);
        assert_eq!(sentence.tokens.len(), 5_000);
        assert!(
            sentence.tokens[0].analysis.is_some(),
            "first token should analyze"
        );
        assert!(
            started.elapsed().as_secs() < 30,
            "long sentence took {:?}",
            started.elapsed()
        );
    }
}
