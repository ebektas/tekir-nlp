//! Morpheme-aware pre-tokenization: [`scan`] pieces, with every word split
//! further at the morpheme boundaries of its best analysis
//! (`" kitaplarımızdan"` -> `" kitap"`, `"lar"`, `"ımız"`, `"dan"`).
//!
//! Subword training on these pieces can never merge across a morpheme
//! boundary. Word analyses are cached per distinct lowercased word across
//! batches; corpus word frequencies are Zipfian, so after warm-up almost
//! every word is a cache hit.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

use tekir_core::{is_turkish_lowercase, turkish_lowercase};
use tekir_morph::batch::parallel_map;
use tekir_morph::{AnalysisSource, MorphService};

use crate::scan::{Piece, PieceKind, scan};

/// Morpheme boundaries per distinct lowercased word, reused across
/// batches. Cleared wholesale once it exceeds its capacity.
#[derive(Debug, Default)]
pub struct BoundaryCache {
    map: HashMap<String, Box<[u16]>>,
    capacity: usize,
}

impl BoundaryCache {
    /// A cache holding up to `capacity` words (`0` = unbounded).
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            capacity,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Shareable (`Send + Sync`) morpheme-aware pre-tokenizer.
#[derive(Debug, Clone)]
pub struct Pretokenizer {
    service: MorphService,
    split_unknown: bool,
}

impl Default for Pretokenizer {
    fn default() -> Self {
        Self::new(MorphService::new())
    }
}

impl Pretokenizer {
    pub fn new(service: MorphService) -> Self {
        Self {
            service,
            split_unknown: false,
        }
    }

    /// Also split words the analyzer only guessed (`+Unk`). Off by default:
    /// guessed splits of foreign words and typos are mostly noise.
    pub fn split_unknown(mut self, split: bool) -> Self {
        self.split_unknown = split;
        self
    }

    pub fn service(&self) -> &MorphService {
        &self.service
    }

    /// Interior morpheme boundaries (byte offsets) of a Turkish-lowercase
    /// word, from its best analysis.
    pub fn word_boundaries(&self, word: &str) -> Vec<usize> {
        let output = self.service.analyze_word(word);
        match output.best_analysis {
            Some(best) if self.split_unknown || best.source != AnalysisSource::Guessed => {
                best.morpheme_boundaries(self.service.lexicon())
            }
            _ => Vec::new(),
        }
    }

    /// Pre-tokenize one text into contiguous byte ranges covering it.
    pub fn pretokenize(&self, text: &str) -> Vec<Range<usize>> {
        let mut cache = BoundaryCache::default();
        self.pretokenize_batch(&[text], 1, &mut cache)
            .pop()
            .expect("one text in, one out")
    }

    /// Pre-tokenize many texts on up to `threads` threads (`0` = all
    /// cores). Output order and content are independent of `threads`.
    pub fn pretokenize_batch<S: AsRef<str> + Sync>(
        &self,
        texts: &[S],
        threads: usize,
        cache: &mut BoundaryCache,
    ) -> Vec<Vec<Range<usize>>> {
        if cache.capacity > 0 && cache.map.len() > cache.capacity {
            cache.map.clear();
        }

        // Scan and find cache misses in parallel against the read-only
        // cache; only the (small, once warm) miss set is merged serially.
        let map = &cache.map;
        let scanned = parallel_map(texts, threads, |text| {
            let text = text.as_ref();
            let pieces = scan(text);
            let misses: Vec<String> = pieces
                .iter()
                .filter(|piece| piece.kind == PieceKind::Word)
                .map(|piece| lowercase(&text[piece.content()]))
                .filter(|word| !map.contains_key(word.as_ref()))
                .map(Cow::into_owned)
                .collect();
            (pieces, misses)
        });
        let mut missing: HashSet<&str> = HashSet::new();
        for (_, misses) in &scanned {
            missing.extend(misses.iter().map(String::as_str));
        }
        let missing: Vec<&str> = missing.into_iter().collect();
        let computed = parallel_map(&missing, threads, |word| {
            self.word_boundaries(word)
                .into_iter()
                .map(|offset| offset as u16)
                .collect::<Box<[u16]>>()
        });
        for (word, boundaries) in missing.into_iter().zip(computed) {
            cache.map.insert(word.to_owned(), boundaries);
        }

        let map = &cache.map;
        let jobs: Vec<(&str, &Vec<Piece>)> = texts
            .iter()
            .map(|text| text.as_ref())
            .zip(scanned.iter().map(|(pieces, _)| pieces))
            .collect();
        parallel_map(&jobs, threads, |(text, pieces)| expand(text, pieces, map))
    }
}

fn lowercase(word: &str) -> Cow<'_, str> {
    if is_turkish_lowercase(word) {
        Cow::Borrowed(word)
    } else {
        Cow::Owned(turkish_lowercase(word))
    }
}

/// Replace each word piece with its morpheme pieces.
fn expand(text: &str, pieces: &[Piece], map: &HashMap<String, Box<[u16]>>) -> Vec<Range<usize>> {
    let mut out = Vec::with_capacity(pieces.len() * 2);
    for piece in pieces {
        if piece.kind != PieceKind::Word {
            out.push(piece.range.clone());
            continue;
        }
        let content = piece.content();
        let word = &text[content.clone()];
        let lowered = lowercase(word);
        let boundaries = map.get(lowered.as_ref()).map_or(&[][..], |b| &b[..]);
        let mut start = piece.range.start;
        for &boundary in boundaries {
            let Some(offset) = original_offset(word, &lowered, boundary as usize) else {
                continue;
            };
            out.push(start..content.start + offset);
            start = content.start + offset;
        }
        out.push(start..piece.range.end);
    }
    out
}

/// Map a byte offset in the lowercased word back to the original word.
/// Lowercasing is char-for-char except for rare expansions; an offset that
/// falls inside one is dropped.
fn original_offset(word: &str, lowered: &str, offset: usize) -> Option<usize> {
    if word.len() == lowered.len() && word == lowered {
        return Some(offset);
    }
    let mut lowered_pos = 0;
    for (original_pos, ch) in word.char_indices() {
        if lowered_pos == offset {
            return Some(original_pos);
        }
        if lowered_pos > offset {
            return None;
        }
        lowered_pos += match ch {
            'I' => 'ı'.len_utf8(),
            'İ' => 'i'.len_utf8(),
            _ => ch.to_lowercase().map(char::len_utf8).sum(),
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{BoundaryCache, Pretokenizer};
    use tekir_morph::MorphService;

    fn pieces(pretok: &Pretokenizer, text: &str) -> Vec<String> {
        pretok
            .pretokenize(text)
            .into_iter()
            .map(|range| text[range].to_string())
            .collect()
    }

    #[test]
    fn splits_words_at_morpheme_boundaries() {
        let pretok = Pretokenizer::new(MorphService::seed());
        assert_eq!(
            pieces(&pretok, "Kitaplarımızdan evdekiler geldi."),
            vec![
                "Kitap", "lar", "ımız", "dan", " ev", "de", "ki", "ler", " gel", "di", "."
            ]
        );
        assert_eq!(
            pieces(&pretok, "KİTAPLAR ve Ankara'dakiler"),
            vec!["KİTAP", "LAR", " ve", " Ankara", "'", "da", "ki", "ler"]
        );
    }

    #[test]
    fn leaves_unknown_words_whole_by_default() {
        let pretok = Pretokenizer::new(MorphService::seed());
        assert_eq!(pieces(&pretok, "xyzlerden"), vec!["xyzlerden"]);
        let pretok = pretok.split_unknown(true);
        assert_eq!(pieces(&pretok, "xyzlerden"), vec!["xyz", "ler", "den"]);
    }

    #[test]
    fn batch_is_lossless_cached_and_thread_independent() {
        let pretok = Pretokenizer::new(MorphService::seed());
        let texts = [
            "Evlerimizde kitaplar var.",
            "  Kitaplar\n\nevlerde 2024'te okunuyordu!",
            "",
        ];
        let mut cache = BoundaryCache::default();
        let one = pretok.pretokenize_batch(&texts, 1, &mut cache);
        let cached = cache.len();
        let many = pretok.pretokenize_batch(&texts, 4, &mut cache);
        assert_eq!(one, many);
        assert_eq!(cache.len(), cached);
        for (text, ranges) in texts.iter().zip(&one) {
            let joined: String = ranges.iter().map(|r| &text[r.clone()]).collect();
            assert_eq!(joined, *text);
        }
    }
}
