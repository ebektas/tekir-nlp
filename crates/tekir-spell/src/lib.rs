//! Spell checking, correction suggestions, and deasciification built on
//! the morphological analyzer: a Turkish word is well-formed exactly when
//! it has an analysis (Oflazer's original spelling-checker thesis).

use serde::Serialize;
use tekir_morph::{Lexicon, MorphService, analyze_nouns, closed, guess};

/// ASCII letters that may stand for a Turkish letter in deasciified text.
const DEASCII: &[(char, char)] = &[
    ('c', 'ç'),
    ('g', 'ğ'),
    ('i', 'ı'),
    ('o', 'ö'),
    ('s', 'ş'),
    ('u', 'ü'),
];

const ALPHABET: &[char] = &[
    'a', 'b', 'c', 'ç', 'd', 'e', 'f', 'g', 'ğ', 'h', 'ı', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'ö',
    'p', 'r', 's', 'ş', 't', 'u', 'ü', 'v', 'y', 'z',
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpellReport {
    pub correct: bool,
    pub suggestions: Vec<String>,
}

/// Spell checker over a shared lexicon. Correctness deliberately excludes
/// the unknown-word guesser: only lexicon-derivable words count.
#[derive(Debug, Clone)]
pub struct SpellChecker {
    service: MorphService,
}

impl Default for SpellChecker {
    fn default() -> Self {
        Self::new(MorphService::with_lexicon(Lexicon::full()))
    }
}

impl SpellChecker {
    pub fn new(service: MorphService) -> Self {
        Self { service }
    }

    /// Well-formed: analyzable from the lexicon, a closed-class form, a
    /// question particle, or a proper apostrophe form with a parsing tail.
    pub fn is_correct(&self, word: &str) -> bool {
        if word.is_empty() {
            return false;
        }
        if let Some(analyses) = guess::apostrophe_analyses(word) {
            return !analyses.is_empty();
        }
        !analyze_nouns(self.service.lexicon(), word).is_empty()
            || !closed::closed_analyses(word).is_empty()
            || !guess::question_particle_analyses(word).is_empty()
    }

    /// Ranked corrections: deasciified variants first, then edit-distance-1
    /// candidates, both filtered to well-formed words.
    pub fn suggest(&self, word: &str, limit: usize) -> Vec<String> {
        let mut suggestions = Vec::new();
        let push = |candidate: String, suggestions: &mut Vec<String>| {
            if candidate != word && !suggestions.contains(&candidate) && self.is_correct(&candidate)
            {
                suggestions.push(candidate);
            }
        };

        for candidate in deascii_variants(word) {
            push(candidate, &mut suggestions);
            if suggestions.len() >= limit {
                return suggestions;
            }
        }
        for candidate in edit_distance_one(word) {
            push(candidate, &mut suggestions);
            if suggestions.len() >= limit {
                break;
            }
        }
        suggestions
    }

    /// The best deasciified form, if the input has one (`gunes -> güneş`).
    pub fn deasciify(&self, word: &str) -> Option<String> {
        deascii_variants(word).find(|candidate| candidate != word && self.is_correct(candidate))
    }

    pub fn check(&self, word: &str, limit: usize) -> SpellReport {
        let correct = self.is_correct(word);
        SpellReport {
            correct,
            suggestions: if correct {
                Vec::new()
            } else {
                self.suggest(word, limit)
            },
        }
    }
}

/// All combinations of ASCII-for-Turkish substitutions (bounded: at most
/// 1024 variants), nearest-to-input first is not guaranteed; the checker
/// filter does the real work.
fn deascii_variants(word: &str) -> impl Iterator<Item = String> {
    let slots: Vec<(usize, char, char)> = word
        .char_indices()
        .filter_map(|(index, ch)| {
            DEASCII
                .iter()
                .find(|(ascii, _)| *ascii == ch)
                .map(|(_, turkish)| (index, ch, *turkish))
        })
        .take(10)
        .collect();
    let word = word.to_string();
    let count = 1u32 << slots.len();

    (1..count).map(move |mask| {
        let mut variant = String::with_capacity(word.len() + 4);
        let mut previous = 0;
        for (bit, (index, ascii, turkish)) in slots.iter().enumerate() {
            variant.push_str(&word[previous..*index]);
            variant.push(if mask & (1 << bit) != 0 {
                *turkish
            } else {
                *ascii
            });
            previous = index + ascii.len_utf8();
        }
        variant.push_str(&word[previous..]);
        variant
    })
}

/// Deletions, transpositions, replacements, and insertions over the
/// Turkish alphabet.
fn edit_distance_one(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut candidates = Vec::new();

    for index in 0..chars.len() {
        let mut deleted = chars.clone();
        deleted.remove(index);
        candidates.push(deleted.into_iter().collect());
    }
    for index in 0..chars.len().saturating_sub(1) {
        let mut swapped = chars.clone();
        swapped.swap(index, index + 1);
        candidates.push(swapped.into_iter().collect());
    }
    for index in 0..chars.len() {
        for &letter in ALPHABET {
            let mut replaced = chars.clone();
            replaced[index] = letter;
            candidates.push(replaced.into_iter().collect());
        }
    }
    for index in 0..=chars.len() {
        for &letter in ALPHABET {
            let mut inserted = chars.clone();
            inserted.insert(index, letter);
            candidates.push(inserted.into_iter().collect());
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::SpellChecker;

    #[test]
    fn checks_and_corrects() {
        let checker = SpellChecker::default();
        assert!(checker.is_correct("evdekilerinki"));
        assert!(checker.is_correct("bana"));
        assert!(checker.is_correct("Ankara'da"));
        assert!(!checker.is_correct("evdz"));

        assert_eq!(checker.deasciify("gunes").as_deref(), Some("güneş"));
        assert_eq!(checker.deasciify("cocuklar").as_deref(), Some("çocuklar"));
        assert!(checker.suggest("kitapp", 5).contains(&"kitap".to_string()));
    }
}
