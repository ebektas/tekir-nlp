//! Turkish phonology helpers shared by generation and analysis.

/// Phonological state at the boundary where the next suffix attaches.
///
/// `last_char` drives buffer-consonant and stop-voicing decisions;
/// `last_vowel` drives vowel harmony. They are tracked separately because
/// harmony can be overridden per lexicon entry (loans like `saat`) and must
/// survive all-consonant suffixes. The struct is `Copy` so speculative
/// suffix matches during analysis never touch the heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhonoContext {
    pub last_char: char,
    pub last_vowel: Option<char>,
}

impl PhonoContext {
    /// Context for a stem, honoring a per-entry harmony override.
    pub fn for_stem(stem: &str, harmony_override: Option<char>) -> Self {
        Self {
            last_char: stem.chars().next_back().unwrap_or('e'),
            last_vowel: harmony_override.or_else(|| last_vowel(stem)),
        }
    }

    /// Advance the context across an appended surface string.
    pub fn extend(&mut self, appended: &str) {
        for ch in appended.chars() {
            self.last_char = ch;
            if is_vowel(ch) {
                self.last_vowel = Some(ch);
            }
        }
    }

    pub fn ends_with_vowel(self) -> bool {
        is_vowel(self.last_char)
    }
}

/// Turkish vowels, including the circumflexed long vowels of loanwords
/// (`â`, `î`, `û`), which harmonize like their plain counterparts.
pub fn is_vowel(ch: char) -> bool {
    matches!(
        ch,
        'a' | 'e' | 'ı' | 'i' | 'o' | 'ö' | 'u' | 'ü' | 'â' | 'î' | 'û'
    )
}

pub fn last_vowel(text: &str) -> Option<char> {
    text.chars().rev().find(|ch| is_vowel(*ch))
}

pub fn starts_with_vowel(text: &str) -> bool {
    text.chars().next().is_some_and(is_vowel)
}

/// Two-way harmony: the `A` archiphoneme of Oflazer's rules 1-2.
pub fn a_harmony(ctx: PhonoContext) -> char {
    match ctx.last_vowel {
        Some('a' | 'ı' | 'o' | 'u' | 'â' | 'û') => 'a',
        _ => 'e',
    }
}

/// Four-way harmony: the `H` archiphoneme of Oflazer's rules 3-6.
pub fn i_harmony(ctx: PhonoContext) -> char {
    match ctx.last_vowel {
        Some('a' | 'ı' | 'â') => 'ı',
        Some('o' | 'u' | 'û') => 'u',
        Some('ö' | 'ü') => 'ü',
        _ => 'i',
    }
}

/// `H` harmony keyed directly off a known preceding vowel.
pub fn i_harmony_after(vowel: char) -> char {
    i_harmony(PhonoContext {
        last_char: vowel,
        last_vowel: Some(vowel),
    })
}

/// Dental stop of the locative/ablative: the `D` archiphoneme of rule 12.
/// Voiceless after the voiceless consonants (fıstıkçı şahap), voiced `d`
/// otherwise.
pub fn stop_consonant(ctx: PhonoContext) -> char {
    match ctx.last_char {
        'f' | 's' | 't' | 'k' | 'ç' | 'ş' | 'h' | 'p' => 't',
        _ => 'd',
    }
}

/// Suffix-initial affricate of `+cH`/`+cHk`: Oflazer's rules 14-15.
/// Devoiced to `ç` after voiceless consonants (`kitapçı`, `ağaççık`), `c`
/// otherwise (`evcik`, `arabacı`).
pub fn affricate_consonant(ctx: PhonoContext) -> char {
    match ctx.last_char {
        'f' | 's' | 't' | 'k' | 'ç' | 'ş' | 'h' | 'p' => 'ç',
        _ => 'c',
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::{PhonoContext, a_harmony, i_harmony, stop_consonant};

    #[test]
    fn harmony_follows_last_vowel() {
        let ctx = PhonoContext::for_stem("ev", None);
        assert_eq!(a_harmony(ctx), 'e');
        assert_eq!(i_harmony(ctx), 'i');

        let ctx = PhonoContext::for_stem("çocuk", None);
        assert_eq!(a_harmony(ctx), 'a');
        assert_eq!(i_harmony(ctx), 'u');
    }

    #[test]
    fn harmony_override_wins_over_stem_vowels() {
        let ctx = PhonoContext::for_stem("saat", Some('i'));
        assert_eq!(a_harmony(ctx), 'e');
        assert_eq!(i_harmony(ctx), 'i');

        let ctx = PhonoContext::for_stem("alkol", Some('ü'));
        assert_eq!(a_harmony(ctx), 'e');
        assert_eq!(i_harmony(ctx), 'ü');
    }

    #[test]
    fn extend_tracks_suffix_vowels_and_final_char() {
        let mut ctx = PhonoContext::for_stem("ev", None);
        ctx.extend("ler");
        assert_eq!(ctx.last_char, 'r');
        assert_eq!(ctx.last_vowel, Some('e'));
        assert!(!ctx.ends_with_vowel());
    }

    #[test]
    fn stop_devoices_after_voiceless_consonants() {
        assert_eq!(stop_consonant(PhonoContext::for_stem("kitap", None)), 't');
        assert_eq!(stop_consonant(PhonoContext::for_stem("ev", None)), 'd');
        assert_eq!(stop_consonant(PhonoContext::for_stem("ağaç", None)), 't');
    }
}
