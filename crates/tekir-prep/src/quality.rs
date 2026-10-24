//! Per-document quality signals and a threshold filter.
//!
//! The signals follow the Gopher/C4/FineWeb heuristics (word counts, line
//! punctuation, duplicate lines, top n-gram repetition, symbol ratio) plus
//! two Turkish-specific ones: the fraction of Turkish letters (çğıöşü), and
//! analyzer coverage, the fraction of alphabetic words `tekir-morph`
//! derives from its lexicon rather than guessing. Coverage is the strongest
//! single "this is real Turkish prose" signal: boilerplate, mangled
//! encodings, and other languages all fall back to the guesser.
//!
//! Words are whitespace-separated tokens with leading/trailing
//! non-alphanumerics trimmed (`"(İstanbul'da)," -> İstanbul'da`).

use serde::{Deserialize, Serialize};
use tekir_morph::{AnalysisSource, MorphService};

use crate::text::{FastMap, FastSet, combine, hash_lower_word, hash_str, push_lower};

/// Quality signals of one document. Fractions are in `[0, 1]` and are `0`
/// when their denominator is empty.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QualitySignals {
    pub word_count: usize,
    /// Mean word length in characters.
    pub mean_word_length: f64,
    /// Non-empty lines ending in `.`, `!`, `?` or `…` (closing quotes and
    /// brackets ignored).
    pub terminal_punctuation_line_fraction: f64,
    /// (`#` + ellipsis occurrences) per word, as in Gopher.
    pub symbol_word_ratio: f64,
    /// Non-empty lines that repeat an earlier line.
    pub duplicate_line_fraction: f64,
    /// Fraction of word characters covered by the most frequent 2-, 3- and
    /// 4-gram (Gopher's top n-gram repetition); `0` if none repeats.
    pub top_ngram_fraction: [f64; 3],
    /// Letters that are one of `çğıöşü` / `ÇĞİÖŞÜ`.
    pub turkish_letter_fraction: f64,
    /// Uppercase letters among all letters.
    pub uppercase_ratio: f64,
    /// Fraction of alphabetic words analyzed from the lexicon (or as
    /// apostrophe/question forms) rather than by the guesser; `None` when
    /// the document has no alphabetic words.
    pub analyzer_coverage: Option<f64>,
}

/// A failed [`QualityConfig`] check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRule {
    TooFewWords,
    TooManyWords,
    MeanWordLength,
    TerminalPunctuation,
    SymbolRatio,
    DuplicateLines,
    TopNgramRepetition,
    TurkishLetters,
    Uppercase,
    AnalyzerCoverage,
}

/// Thresholds for [`QualityConfig::check`]. Defaults are Gopher/FineWeb
/// values, with the word-length range widened for agglutinative Turkish;
/// treat them as a starting point to tune on your corpus. Set a minimum to
/// `0.0` (or a maximum to `1.0`/`f64::INFINITY`) to disable a rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QualityConfig {
    pub min_words: usize,
    pub max_words: usize,
    pub min_mean_word_length: f64,
    pub max_mean_word_length: f64,
    pub min_terminal_punctuation_line_fraction: f64,
    pub max_symbol_word_ratio: f64,
    pub max_duplicate_line_fraction: f64,
    /// Limits for the top 2-, 3- and 4-gram fractions.
    pub max_top_ngram_fraction: [f64; 3],
    pub min_turkish_letter_fraction: f64,
    pub max_uppercase_ratio: f64,
    /// A document without alphabetic words fails this rule unless it is
    /// `0.0`.
    pub min_analyzer_coverage: f64,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            min_words: 50,
            max_words: 100_000,
            min_mean_word_length: 3.0,
            max_mean_word_length: 12.0,
            min_terminal_punctuation_line_fraction: 0.12,
            max_symbol_word_ratio: 0.1,
            max_duplicate_line_fraction: 0.3,
            max_top_ngram_fraction: [0.20, 0.18, 0.16],
            min_turkish_letter_fraction: 0.02,
            max_uppercase_ratio: 0.3,
            min_analyzer_coverage: 0.7,
        }
    }
}

impl QualityConfig {
    /// Every rule `signals` fails, in declaration order.
    pub fn check(&self, signals: &QualitySignals) -> Vec<QualityRule> {
        let coverage_ok = match signals.analyzer_coverage {
            Some(coverage) => coverage >= self.min_analyzer_coverage,
            None => self.min_analyzer_coverage <= 0.0,
        };
        let top_ngram_ok = signals
            .top_ngram_fraction
            .iter()
            .zip(&self.max_top_ngram_fraction)
            .all(|(value, max)| value <= max);
        [
            (
                signals.word_count >= self.min_words,
                QualityRule::TooFewWords,
            ),
            (
                signals.word_count <= self.max_words,
                QualityRule::TooManyWords,
            ),
            (
                (self.min_mean_word_length..=self.max_mean_word_length)
                    .contains(&signals.mean_word_length),
                QualityRule::MeanWordLength,
            ),
            (
                signals.terminal_punctuation_line_fraction
                    >= self.min_terminal_punctuation_line_fraction,
                QualityRule::TerminalPunctuation,
            ),
            (
                signals.symbol_word_ratio <= self.max_symbol_word_ratio,
                QualityRule::SymbolRatio,
            ),
            (
                signals.duplicate_line_fraction <= self.max_duplicate_line_fraction,
                QualityRule::DuplicateLines,
            ),
            (top_ngram_ok, QualityRule::TopNgramRepetition),
            (
                signals.turkish_letter_fraction >= self.min_turkish_letter_fraction,
                QualityRule::TurkishLetters,
            ),
            (
                signals.uppercase_ratio <= self.max_uppercase_ratio,
                QualityRule::Uppercase,
            ),
            (coverage_ok, QualityRule::AnalyzerCoverage),
        ]
        .into_iter()
        .filter_map(|(ok, rule)| (!ok).then_some(rule))
        .collect()
    }

    pub fn passes(&self, signals: &QualitySignals) -> bool {
        self.check(signals).is_empty()
    }
}

/// Reusable per-worker buffers for [`QualityScorer::signals_with`]; keep
/// one per thread so steady-state scoring reuses its capacity.
#[derive(Debug, Default)]
pub struct QualityScratch {
    /// Lowercased word -> covered, for the current document.
    coverage: FastMap<String, bool>,
    lower: String,
    lines: FastSet<u64>,
    word_hashes: Vec<u64>,
    word_chars: Vec<usize>,
    /// n-gram hash -> (count, character length).
    ngrams: FastMap<u64, (usize, usize)>,
}

/// Computes [`QualitySignals`]; holds a shared [`MorphService`] (cheap to
/// clone, `Send + Sync`, no locks).
#[derive(Debug, Clone)]
pub struct QualityScorer {
    morph: MorphService,
}

impl Default for QualityScorer {
    fn default() -> Self {
        Self::new(MorphService::new())
    }
}

impl QualityScorer {
    pub fn new(morph: MorphService) -> Self {
        Self { morph }
    }

    pub fn signals(&self, text: &str) -> QualitySignals {
        self.signals_with(text, &mut QualityScratch::default())
    }

    pub fn signals_with(&self, text: &str, scratch: &mut QualityScratch) -> QualitySignals {
        let (terminal_lines, duplicate_lines, lines) = line_stats(text, scratch);

        scratch.coverage.clear();
        scratch.word_hashes.clear();
        scratch.word_chars.clear();
        let mut symbols = 0usize;
        let (mut letters, mut uppercase, mut turkish) = (0usize, 0usize, 0usize);
        let (mut alphabetic_words, mut covered_words) = (0usize, 0usize);

        for token in text.split_whitespace() {
            symbols += token.matches('#').count()
                + token.matches("...").count()
                + token.matches('…').count();
            let word = token.trim_matches(|ch: char| !ch.is_alphanumeric());
            if word.is_empty() {
                continue;
            }

            let mut chars = 0;
            let mut alphabetic = true;
            for ch in word.chars() {
                chars += 1;
                if ch.is_alphabetic() {
                    letters += 1;
                    uppercase += usize::from(ch.is_uppercase());
                    turkish += usize::from("çğıöşüÇĞİÖŞÜ".contains(ch));
                } else if !matches!(ch, '\'' | '’') {
                    alphabetic = false;
                }
            }
            scratch.word_hashes.push(hash_lower_word(word));
            scratch.word_chars.push(chars);

            if alphabetic {
                alphabetic_words += 1;
                covered_words += usize::from(self.covered(word, scratch));
            }
        }

        let word_count = scratch.word_hashes.len();
        let word_chars: usize = scratch.word_chars.iter().sum();
        let top_ngram_fraction = [2, 3, 4].map(|n| top_ngram_fraction(scratch, n, word_chars));

        QualitySignals {
            word_count,
            mean_word_length: ratio(word_chars, word_count),
            terminal_punctuation_line_fraction: ratio(terminal_lines, lines),
            symbol_word_ratio: ratio(symbols, word_count),
            duplicate_line_fraction: ratio(duplicate_lines, lines),
            top_ngram_fraction,
            turkish_letter_fraction: ratio(turkish, letters),
            uppercase_ratio: ratio(uppercase, letters),
            analyzer_coverage: (alphabetic_words > 0)
                .then(|| ratio(covered_words, alphabetic_words)),
        }
    }

    /// Whether the lowercased `word` has a non-guessed analysis, memoized
    /// per document (word frequencies are Zipfian, so most lookups hit).
    fn covered(&self, word: &str, scratch: &mut QualityScratch) -> bool {
        scratch.lower.clear();
        push_lower(word, &mut scratch.lower);
        if let Some(&covered) = scratch.coverage.get(scratch.lower.as_str()) {
            return covered;
        }
        let response = self.morph.analyze(scratch.lower.clone());
        let covered = response
            .output
            .analyses
            .iter()
            .any(|analysis| analysis.source != AnalysisSource::Guessed);
        scratch.coverage.insert(response.original, covered);
        covered
    }
}

/// (lines ending in terminal punctuation, repeated lines, non-empty lines).
fn line_stats(text: &str, scratch: &mut QualityScratch) -> (usize, usize, usize) {
    scratch.lines.clear();
    let (mut terminal, mut duplicate, mut total) = (0, 0, 0);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        total += 1;
        let end = line.trim_end_matches(['"', '\'', '”', '’', '»', ')', ']']);
        terminal += usize::from(end.ends_with(['.', '!', '?', '…']));
        duplicate += usize::from(!scratch.lines.insert(hash_str(line)));
    }
    (terminal, duplicate, total)
}

/// Characters covered by the most frequent `n`-gram (ties broken by
/// length, so the result does not depend on map order), over all word
/// characters; `0` when no n-gram repeats. Overlapping occurrences count,
/// capped at 1.
fn top_ngram_fraction(scratch: &mut QualityScratch, n: usize, word_chars: usize) -> f64 {
    let words = scratch.word_hashes.len();
    if words < n || word_chars == 0 {
        return 0.0;
    }
    scratch.ngrams.clear();
    for start in 0..=words - n {
        let hash = scratch.word_hashes[start..start + n]
            .iter()
            .fold(n as u64, |hash, &word| combine(hash, word));
        let chars = scratch.word_chars[start..start + n].iter().sum();
        scratch.ngrams.entry(hash).or_insert((0, chars)).0 += 1;
    }
    let (count, chars) = scratch.ngrams.values().copied().max().unwrap_or_default();
    if count < 2 {
        return 0.0;
    }
    ratio(count * chars, word_chars).min(1.0)
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use tekir_morph::MorphService;

    use super::{QualityConfig, QualityRule, QualityScorer, QualityScratch};

    fn seed_scorer() -> QualityScorer {
        QualityScorer::new(MorphService::seed())
    }

    #[test]
    fn basic_signals() {
        let text = "Bir iki üç.\nDört beş #etiket...\nBir iki üç.\n\n";
        let signals = seed_scorer().signals(text);
        assert_eq!(signals.word_count, 9);
        // bir iki üç dört beş etiket bir iki üç = 3+3+2+4+3+6+3+3+2 chars.
        assert!((signals.mean_word_length - 29.0 / 9.0).abs() < 1e-9);
        // Three non-empty lines, all ending in '.'; one repeats.
        assert_eq!(signals.terminal_punctuation_line_fraction, 1.0);
        assert!((signals.duplicate_line_fraction - 1.0 / 3.0).abs() < 1e-9);
        // One '#' and one "..." over nine words.
        assert!((signals.symbol_word_ratio - 2.0 / 9.0).abs() < 1e-9);
        // Letters: 29; uppercase B, D, B; Turkish ü, ç, ö, ş, ü, ç.
        assert!((signals.uppercase_ratio - 3.0 / 29.0).abs() < 1e-9);
        assert!((signals.turkish_letter_fraction - 6.0 / 29.0).abs() < 1e-9);
    }

    #[test]
    fn empty_text() {
        let signals = seed_scorer().signals("  \n ... \n");
        assert_eq!(signals.word_count, 0);
        assert_eq!(signals.mean_word_length, 0.0);
        assert_eq!(signals.analyzer_coverage, None);
        let failures = QualityConfig::default().check(&signals);
        assert!(failures.contains(&QualityRule::TooFewWords));
        assert!(failures.contains(&QualityRule::AnalyzerCoverage));
    }

    #[test]
    fn top_ngram_repetition() {
        let scorer = seed_scorer();
        let repeated = "ev kitap ".repeat(20);
        let signals = scorer.signals(&repeated);
        // "ev kitap" and "kitap ev" alternate; the top bigram is half the
        // windows but each covers two words, so ~all characters.
        assert!(signals.top_ngram_fraction[0] > 0.9);
        assert!(
            QualityConfig::default()
                .check(&signals)
                .contains(&QualityRule::TopNgramRepetition)
        );

        let varied = scorer.signals("bir iki üç dört beş altı yedi sekiz dokuz on");
        // Every n-gram is unique: no repetition at all.
        assert_eq!(varied.top_ngram_fraction, [0.0; 3]);
    }

    #[test]
    fn analyzer_coverage_uses_lexicon_not_guesser() {
        let scorer = seed_scorer();
        let covered = scorer.signals("EVLERİ kitaplar evde.");
        assert_eq!(covered.analyzer_coverage, Some(1.0));

        let half = scorer.signals("ev kitap qzxwv bnmtk");
        assert_eq!(half.analyzer_coverage, Some(0.5));

        // Numbers and mixed tokens are not alphabetic words.
        let none = scorer.signals("123 a1b2 2024");
        assert_eq!(none.analyzer_coverage, None);
    }

    #[test]
    fn scratch_reuse_matches_fresh() {
        let scorer = seed_scorer();
        let mut scratch = QualityScratch::default();
        let a = "Evde kitap var.\nEvde kitap var.";
        let b = "Kitaplar masada. Ev büyük!";
        let first = scorer.signals_with(a, &mut scratch);
        let second = scorer.signals_with(b, &mut scratch);
        assert_eq!(first, scorer.signals(a));
        assert_eq!(second, scorer.signals(b));
    }

    #[test]
    fn config_checks() {
        let scorer = seed_scorer();
        let signals = scorer.signals("BU BİR BAŞLIK");
        let failures = QualityConfig::default().check(&signals);
        assert!(failures.contains(&QualityRule::Uppercase));
        assert!(failures.contains(&QualityRule::TerminalPunctuation));

        let lenient = QualityConfig {
            min_words: 0,
            min_terminal_punctuation_line_fraction: 0.0,
            max_uppercase_ratio: 1.0,
            min_analyzer_coverage: 0.0,
            ..QualityConfig::default()
        };
        assert!(lenient.passes(&signals), "{:?}", lenient.check(&signals));
    }

    #[test]
    fn real_turkish_prose_passes_with_full_lexicon() {
        let text = "Türkiye'nin en kalabalık şehri olan İstanbul, tarih boyunca \
            pek çok medeniyete ev sahipliği yaptı. Şehrin iki yakasını ayıran \
            boğaz, hem ticaret hem de kültür açısından büyük önem taşıyor.\n\
            Her yıl milyonlarca turist, tarihi yarımadadaki camileri, sarayları \
            ve çarşıları görmek için şehre geliyor. Belediye, son yıllarda toplu \
            taşıma ağını genişletmek için yeni metro hatları açtı.\n\
            Uzmanlar, nüfus artışının altyapı üzerinde ciddi bir baskı \
            oluşturduğunu ve planlı kentleşmenin önemli olduğunu söylüyor.";
        let signals = QualityScorer::default().signals(text);
        let coverage = signals.analyzer_coverage.expect("has words");
        assert!(coverage > 0.85, "coverage {coverage}");
        let config = QualityConfig::default();
        assert!(
            config.passes(&signals),
            "{:?} {signals:?}",
            config.check(&signals)
        );

        let garbage = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed \
            do eiusmod tempor incididunt ut labore et dolore magna aliqua.";
        let garbage = QualityScorer::default().signals(garbage);
        assert!(
            garbage.analyzer_coverage.expect("has words") < 0.5,
            "{garbage:?}"
        );
    }
}
