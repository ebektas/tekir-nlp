//! Diacritic restoration in context (`cok guzel bir gun` -> `çok güzel bir
//! gün`). Every ASCII-for-Turkish variant of a word that the analyzer
//! accepts contributes its analyses as candidates, and the disambiguator
//! picks the reading that fits the sentence; its surface is the restored
//! spelling.

use std::collections::{HashMap, HashSet};

use tekir_core::TokenKind;
use tekir_disamb::{Disambiguator, bundled_model};
use tekir_morph::batch::parallel_map;
use tekir_morph::{AnalysisSource, MorphAnalysis, MorphService};
use tekir_tokenize::{TokenizeMode, TokenizeService};

use crate::{DEASCII, deascii_variants};

/// Tokens worth restoring; all-caps words tokenize as abbreviations.
fn is_word(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::Word | TokenKind::Abbreviation)
}

/// Shareable (`Send + Sync`) diacritic restorer.
#[derive(Debug, Clone)]
pub struct Restorer {
    service: MorphService,
    disambiguator: Disambiguator,
}

impl Default for Restorer {
    fn default() -> Self {
        Self::new(MorphService::new(), Disambiguator::new(bundled_model()))
    }
}

/// Lowercase for restoration: ASCII text writes `İ` as `I`, so `I` maps to
/// `i` and the `i`/`ı` choice is left to the variants.
fn ascii_lowercase(word: &str) -> String {
    word.chars()
        .flat_map(|ch| match ch {
            'İ' | 'I' => vec!['i'],
            other => other.to_lowercase().collect(),
        })
        .collect()
}

fn turkish_upper(ch: char) -> char {
    match ch {
        'i' => 'İ',
        'ı' => 'I',
        other => other.to_uppercase().next().unwrap_or(other),
    }
}

/// Carry the original's capitalization over to the restored spelling.
fn apply_case(original: &str, restored: &str) -> Option<String> {
    if original.chars().count() != restored.chars().count() {
        return None;
    }
    Some(
        original
            .chars()
            .zip(restored.chars())
            .map(|(o, r)| {
                if o.is_uppercase() {
                    turkish_upper(r)
                } else {
                    r
                }
            })
            .collect(),
    )
}

impl Restorer {
    pub fn new(service: MorphService, disambiguator: Disambiguator) -> Self {
        Self {
            service,
            disambiguator,
        }
    }

    /// Candidate analyses for a lowercased word across its variants.
    fn candidates(&self, word: &str) -> Vec<MorphAnalysis> {
        let valid = |surface: &str| {
            self.service
                .analyze_word(surface)
                .analyses
                .into_iter()
                .filter(|analysis| analysis.source != AnalysisSource::Guessed)
        };
        let mut candidates: Vec<MorphAnalysis> = valid(word).collect();
        if word
            .chars()
            .any(|ch| DEASCII.iter().any(|(ascii, _)| *ascii == ch))
        {
            for variant in deascii_variants(word) {
                candidates.extend(valid(&variant));
            }
        }
        if candidates.is_empty() {
            candidates = self.service.analyze_word(word).analyses;
        }
        candidates
    }

    pub fn restore(&self, text: &str) -> String {
        self.restore_batch(&[text], 1).remove(0)
    }

    /// Restore many texts on up to `threads` threads (`0` = all cores),
    /// analyzing each distinct word's variants once.
    pub fn restore_batch<S: AsRef<str> + Sync>(&self, texts: &[S], threads: usize) -> Vec<String> {
        let tokenized = parallel_map(texts, threads, |text| {
            TokenizeService
                .tokenize(text.as_ref(), TokenizeMode::Turkish)
                .output
                .tokens
        });
        let mut words: HashSet<String> = HashSet::new();
        for tokens in &tokenized {
            for token in tokens.iter().filter(|t| is_word(t.kind)) {
                words.insert(ascii_lowercase(&token.text));
            }
        }
        let words: Vec<String> = words.into_iter().collect();
        let candidates = parallel_map(&words, threads, |word| self.candidates(word));
        let cache: HashMap<&str, Vec<MorphAnalysis>> =
            words.iter().map(String::as_str).zip(candidates).collect();

        let jobs: Vec<usize> = (0..texts.len()).collect();
        parallel_map(&jobs, threads, |&i| {
            let text = texts[i].as_ref();
            let tokens = &tokenized[i];
            let lowered: Vec<Option<String>> = tokens
                .iter()
                .map(|t| is_word(t.kind).then(|| ascii_lowercase(&t.text)))
                .collect();
            let sentence: Vec<Vec<MorphAnalysis>> = lowered
                .iter()
                .zip(tokens)
                .map(|(word, token)| match word {
                    Some(word) => cache[word.as_str()].clone(),
                    None => self.service.analyze_word(&token.text).analyses,
                })
                .collect();
            let choices = self.disambiguator.disambiguate(&sentence);

            let mut out = String::with_capacity(text.len() + 16);
            let mut previous = 0;
            for ((token, choice), candidates) in tokens.iter().zip(choices).zip(&sentence) {
                out.push_str(&text[previous..token.start_byte]);
                // Only lexicon-backed readings may change a word; a guessed
                // one would just recase it (`IBM` -> `İBM`).
                let restored = choice
                    .map(|c| &candidates[c])
                    .filter(|chosen| {
                        is_word(token.kind) && chosen.source != AnalysisSource::Guessed
                    })
                    .and_then(|chosen| apply_case(&token.text, &chosen.surface));
                out.push_str(restored.as_deref().unwrap_or(&token.text));
                previous = token.end_byte;
            }
            out.push_str(&text[previous..]);
            out
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Restorer;

    #[test]
    fn restores_diacritics_in_context() {
        let restorer = Restorer::default();
        assert_eq!(
            restorer.restore("Bugun cok guzel bir gun, cocuklar disarida oynuyor."),
            "Bugün çok güzel bir gün, çocuklar dışarıda oynuyor."
        );
        assert_eq!(restorer.restore("COK GUZEL"), "ÇOK GÜZEL");
        assert_eq!(restorer.restore("IBM ve TBMM"), "IBM ve TBMM");
        // Proper stems are opaque (no proper-noun lexicon yet): untouched.
        assert_eq!(restorer.restore("Istanbul'a"), "Istanbul'a");
    }

    #[test]
    fn leaves_correct_text_alone() {
        let restorer = Restorer::default();
        let text = "Kitaplarımızdan biri evdeydi.";
        assert_eq!(restorer.restore(text), text);
    }
}
