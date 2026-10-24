//! Tokenizer evaluation for Turkish: compression, and how well token
//! boundaries line up with tekir's morpheme boundaries.
//!
//! Gold boundaries come from each word's best analysis; words without an
//! analysis (unknown roots) count toward compression but not toward the
//! boundary scores.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use tekir_morph::batch::parallel_map;

use crate::bpe::{Error, TekirTokenizer};
use crate::pretok::{Pretokenizer, lowercase};
use crate::scan::{PieceKind, scan};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvalReport {
    pub lines: usize,
    pub words: usize,
    pub tokens: usize,
    /// All tokens / words.
    pub tokens_per_word: f64,
    /// Mean number of tokens overlapping each word.
    pub fertility: f64,
    pub chars_per_token: f64,
    /// Words covered by exactly one token.
    pub single_token_words: f64,
    /// Words with an analysis to score boundaries against.
    pub analyzed_words: usize,
    /// Share of in-word token boundaries that are morpheme boundaries.
    pub boundary_precision: f64,
    /// Share of morpheme boundaries that are token boundaries.
    pub boundary_recall: f64,
    pub boundary_f1: f64,
    /// Analyzed words whose every in-word token boundary is a morpheme
    /// boundary.
    pub aligned_words: f64,
    pub vocab_size: usize,
    pub vocab_used: usize,
}

#[derive(Default)]
struct Tally {
    words: usize,
    tokens: usize,
    chars: usize,
    word_tokens: usize,
    single: usize,
    analyzed: usize,
    predicted: usize,
    gold: usize,
    correct: usize,
    aligned: usize,
    ids: HashSet<u32>,
}

impl Tally {
    fn merge(&mut self, other: Tally) {
        self.words += other.words;
        self.tokens += other.tokens;
        self.chars += other.chars;
        self.word_tokens += other.word_tokens;
        self.single += other.single;
        self.analyzed += other.analyzed;
        self.predicted += other.predicted;
        self.gold += other.gold;
        self.correct += other.correct;
        self.aligned += other.aligned;
        self.ids.extend(other.ids);
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// Evaluate `tokenizer` on `lines`, scoring boundaries against `gold`.
pub fn evaluate<S: AsRef<str> + Sync>(
    tokenizer: &TekirTokenizer,
    gold: &Pretokenizer,
    lines: &[S],
    threads: usize,
) -> Result<EvalReport, Error> {
    let encoded = tokenizer.encode_batch(lines, threads)?;

    let mut words: HashSet<String> = HashSet::new();
    for line in lines {
        let line = line.as_ref();
        for piece in scan(line)
            .iter()
            .filter(|piece| piece.kind == PieceKind::Word)
        {
            words.insert(lowercase(&line[piece.content()]).into_owned());
        }
    }
    let words: Vec<String> = words.into_iter().collect();
    let boundaries = parallel_map(&words, threads, |word| gold.analyzed_boundaries(word));
    let gold_map: HashMap<&str, Option<Vec<usize>>> =
        words.iter().map(String::as_str).zip(boundaries).collect();

    let jobs: Vec<usize> = (0..lines.len()).collect();
    let tallies = parallel_map(&jobs, threads, |&i| {
        let line = lines[i].as_ref();
        let spans = &encoded[i].spans;
        let mut tally = Tally {
            tokens: spans.len(),
            chars: line.chars().count(),
            ids: encoded[i].ids.iter().copied().collect(),
            ..Tally::default()
        };
        for piece in scan(line)
            .iter()
            .filter(|piece| piece.kind == PieceKind::Word)
        {
            let content = piece.content();
            let word = &line[content.clone()];
            tally.words += 1;
            let overlapping = spans
                .iter()
                .filter(|span| span.start < content.end && span.end > content.start)
                .count();
            tally.word_tokens += overlapping;
            tally.single += usize::from(overlapping == 1);

            let lowered = lowercase(word);
            let Some(Some(gold)) = gold_map.get(lowered.as_ref()) else {
                continue;
            };
            // Gold offsets are in the lowercased word; skip the rare words
            // whose lowercasing changed their length.
            if lowered.len() != word.len() {
                continue;
            }
            let predicted: Vec<usize> = spans
                .iter()
                .filter(|span| span.start > content.start && span.start < content.end)
                .map(|span| span.start - content.start)
                .collect();
            let correct = predicted.iter().filter(|b| gold.contains(b)).count();
            tally.analyzed += 1;
            tally.predicted += predicted.len();
            tally.gold += gold.len();
            tally.correct += correct;
            tally.aligned += usize::from(correct == predicted.len());
        }
        tally
    });

    let mut total = Tally::default();
    for tally in tallies {
        total.merge(tally);
    }
    let precision = ratio(total.correct, total.predicted);
    let recall = ratio(total.correct, total.gold);
    Ok(EvalReport {
        lines: lines.len(),
        words: total.words,
        tokens: total.tokens,
        tokens_per_word: ratio(total.tokens, total.words),
        fertility: ratio(total.word_tokens, total.words),
        chars_per_token: ratio(total.chars, total.tokens),
        single_token_words: ratio(total.single, total.words),
        analyzed_words: total.analyzed,
        boundary_precision: precision,
        boundary_recall: recall,
        boundary_f1: if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        },
        aligned_words: ratio(total.aligned, total.analyzed),
        vocab_size: tokenizer.inner().get_vocab_size(true),
        vocab_used: total.ids.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::evaluate;
    use crate::{Pretokenizer, TekirTokenizer, TrainConfig, train};
    use tekir_morph::MorphService;

    #[test]
    fn morph_tokenizer_aligns_with_morphemes() {
        let lines: Vec<String> = (0..40)
            .map(|i| {
                ["Kitaplarımızdan biri evdeydi.", "Evlerimizde kitaplar var."][i % 2].to_string()
            })
            .collect();
        let pretok = Pretokenizer::new(MorphService::seed());
        let config = TrainConfig {
            vocab_size: 300,
            threads: 2,
            ..TrainConfig::default()
        };
        let tokenizer = TekirTokenizer::new(
            train(&pretok, lines.clone(), &config).expect("trains"),
            Some(pretok.clone()),
        );
        let report = evaluate(&tokenizer, &pretok, &lines, 2).expect("evaluates");

        assert_eq!(report.words, 120);
        assert!(report.analyzed_words > 0);
        assert_eq!(report.boundary_precision, 1.0);
        assert_eq!(report.aligned_words, 1.0);
        assert!(report.fertility >= 1.0);
    }
}
