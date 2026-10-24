//! Byte-level BPE training and encoding on top of Hugging Face
//! `tokenizers`, so the result is a standard `tokenizer.json`.
//!
//! The saved tokenizer is self-contained for byte mapping and decoding
//! (ByteLevel pre-tokenizer without its regex, ByteLevel decoder), so
//! decoding is exact. Morpheme splits happen outside it: encode through
//! [`TekirTokenizer`] (or feed [`Pretokenizer`] pieces to `tokenizers` as
//! pre-tokenized input) to get the segmentation it was trained on.

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::LazyLock;

use ahash::AHashMap;
use compact_str::CompactString;
use tekir_morph::batch::parallel_map;
use tokenizers::models::bpe::{BPE, BpeTrainer};
use tokenizers::pre_tokenizers::byte_level::ByteLevel;
use tokenizers::{AddedToken, InputSequence, Tokenizer};

use crate::pretok::{BoundaryCache, Pretokenizer};
use crate::scan::scan;

pub type Error = tokenizers::Error;

/// GPT-2's reversible byte -> printable char table.
static BYTE_CHARS: LazyLock<[char; 256]> = LazyLock::new(|| {
    let mut table = ['\0'; 256];
    let mut extra = 0;
    for byte in 0..=255u8 {
        let printable = matches!(byte, b'!'..=b'~' | 0xA1..=0xAC | 0xAE..=0xFF);
        table[byte as usize] = if printable {
            char::from(byte)
        } else {
            extra += 1;
            char::from_u32(255 + extra).expect("valid char")
        };
    }
    table
});

fn byte_level(piece: &str) -> CompactString {
    piece
        .bytes()
        .map(|byte| BYTE_CHARS[byte as usize])
        .collect()
}

/// Settings for [`train`].
#[derive(Debug, Clone)]
pub struct TrainConfig {
    pub vocab_size: usize,
    pub min_frequency: u64,
    pub special_tokens: Vec<String>,
    /// Split words at morpheme boundaries before counting (the point of
    /// this crate); `false` trains the plain word-level baseline on the
    /// same pieces otherwise, for comparison.
    pub morph: bool,
    /// Worker threads (`0` = all cores).
    pub threads: usize,
    /// Lines per pre-tokenization batch.
    pub chunk_lines: usize,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            vocab_size: 32_000,
            min_frequency: 2,
            special_tokens: vec!["<|endoftext|>".to_string()],
            morph: true,
            threads: 0,
            chunk_lines: 100_000,
        }
    }
}

/// Count pre-tokenized pieces over `lines` and train a byte-level BPE
/// tokenizer whose merges never cross a piece boundary.
pub fn train<I>(pretok: &Pretokenizer, lines: I, config: &TrainConfig) -> Result<Tokenizer, Error>
where
    I: IntoIterator<Item = String>,
{
    let mut counts: HashMap<String, u64> = HashMap::new();
    let mut cache = BoundaryCache::with_capacity(5_000_000);
    let mut chunk: Vec<String> = Vec::with_capacity(config.chunk_lines);
    let mut lines = lines.into_iter().peekable();

    while lines.peek().is_some() {
        chunk.clear();
        chunk.extend(lines.by_ref().take(config.chunk_lines.max(1)));
        let ranges = if config.morph {
            pretok.pretokenize_batch(&chunk, config.threads, &mut cache)
        } else {
            parallel_map(&chunk, config.threads, |line| {
                scan(line).into_iter().map(|piece| piece.range).collect()
            })
        };
        for (line, ranges) in chunk.iter().zip(&ranges) {
            for range in ranges {
                let piece = &line[range.clone()];
                match counts.get_mut(piece) {
                    Some(count) => *count += 1,
                    None => {
                        counts.insert(piece.to_string(), 1);
                    }
                }
            }
        }
    }

    let word_counts: AHashMap<CompactString, u64> = counts
        .iter()
        .map(|(piece, &count)| (byte_level(piece), count))
        .collect();

    let special: Vec<AddedToken> = config
        .special_tokens
        .iter()
        .map(|token| AddedToken::from(token.clone(), true))
        .collect();
    let trainer = BpeTrainer::builder()
        .vocab_size(config.vocab_size)
        .min_frequency(config.min_frequency)
        .initial_alphabet(ByteLevel::alphabet().into_iter().collect())
        .special_tokens(special.clone())
        .show_progress(false)
        .build();
    let mut model = BPE::default();
    trainer.do_train(&word_counts, &mut model)?;

    let mut tokenizer = Tokenizer::new(model);
    tokenizer
        .with_pre_tokenizer(Some(ByteLevel::new(false, false, false)))
        .with_decoder(Some(ByteLevel::default()));
    tokenizer.add_special_tokens(special)?;
    Ok(tokenizer)
}

/// A tokenizer plus the pre-tokenization it expects.
#[derive(Clone)]
pub struct TekirTokenizer {
    inner: Tokenizer,
    pretok: Option<Pretokenizer>,
}

impl TekirTokenizer {
    /// `pretok = Some(..)` for tokenizers trained with morpheme splits;
    /// `None` encodes raw text with the tokenizer's own pipeline (any
    /// Hugging Face tokenizer).
    pub fn new(inner: Tokenizer, pretok: Option<Pretokenizer>) -> Self {
        Self { inner, pretok }
    }

    pub fn from_file(path: impl AsRef<Path>, pretok: Option<Pretokenizer>) -> Result<Self, Error> {
        Ok(Self::new(Tokenizer::from_file(path)?, pretok))
    }

    pub fn inner(&self) -> &Tokenizer {
        &self.inner
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        self.inner.save(path, false)
    }

    /// Token ids and byte spans (into `text`) for each text, on up to
    /// `threads` threads.
    pub fn encode_batch<S: AsRef<str> + Sync>(
        &self,
        texts: &[S],
        threads: usize,
    ) -> Result<Vec<Encoded>, Error> {
        let pieces = match &self.pretok {
            Some(pretok) => {
                let mut cache = BoundaryCache::default();
                Some(pretok.pretokenize_batch(texts, threads, &mut cache))
            }
            None => None,
        };
        let jobs: Vec<usize> = (0..texts.len()).collect();
        parallel_map(&jobs, threads, |&i| {
            let text = texts[i].as_ref();
            match &pieces {
                Some(pieces) => self.encode_pieces(text, &pieces[i]),
                None => self.encode_raw(text),
            }
        })
        .into_iter()
        .collect()
    }

    pub fn encode(&self, text: &str) -> Result<Encoded, Error> {
        Ok(self.encode_batch(&[text], 1)?.remove(0))
    }

    pub fn decode(&self, ids: &[u32]) -> Result<String, Error> {
        self.inner.decode(ids, false)
    }

    fn encode_raw(&self, text: &str) -> Result<Encoded, Error> {
        let encoding = self.inner.encode(text, false)?;
        Ok(Encoded {
            ids: encoding.get_ids().to_vec(),
            spans: encoding
                .get_offsets()
                .iter()
                .map(|&(start, end)| start..end)
                .collect(),
        })
    }

    fn encode_pieces(&self, text: &str, ranges: &[Range<usize>]) -> Result<Encoded, Error> {
        let pieces: Vec<&str> = ranges.iter().map(|range| &text[range.clone()]).collect();
        let encoding = self.inner.encode(InputSequence::from(&pieces[..]), false)?;
        // Offsets of pre-tokenized input are relative to each piece.
        let spans = encoding
            .get_offsets()
            .iter()
            .zip(encoding.get_word_ids())
            .map(|(&(start, end), word)| {
                let base = word.map_or(0, |w| ranges[w as usize].start);
                base + start..base + end
            })
            .collect();
        Ok(Encoded {
            ids: encoding.get_ids().to_vec(),
            spans,
        })
    }
}

/// One encoded text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    pub ids: Vec<u32>,
    /// Byte span of each token in the input text.
    pub spans: Vec<Range<usize>>,
}

#[cfg(test)]
mod tests {
    use super::{TekirTokenizer, TrainConfig, train};
    use crate::Pretokenizer;
    use tekir_morph::MorphService;

    fn corpus() -> Vec<String> {
        let base = [
            "Kitaplarımızdan biri evdeydi.",
            "Evlerimizde kitaplar var, evlerde de var.",
            "Çocuklar kitapları okuyorlardı.",
            "Kitapçılar evlere kitap getirdi.",
        ];
        (0..50).map(|i| base[i % base.len()].to_string()).collect()
    }

    #[test]
    fn trains_a_lossless_tokenizer_that_respects_morphemes() {
        let pretok = Pretokenizer::new(MorphService::seed());
        let config = TrainConfig {
            vocab_size: 400,
            threads: 2,
            ..TrainConfig::default()
        };
        let tokenizer = train(&pretok, corpus(), &config).expect("training succeeds");
        let tokenizer = TekirTokenizer::new(tokenizer, Some(pretok.clone()));

        let text = "Kitaplarımızdan evlerde okuyorlardı.";
        let encoded = tokenizer.encode(text).expect("encodes");
        assert_eq!(tokenizer.decode(&encoded.ids).expect("decodes"), text);

        // No token crosses a morpheme boundary.
        let boundaries: Vec<usize> = pretok.pretokenize(text).iter().map(|r| r.start).collect();
        for span in &encoded.spans {
            assert!(
                !boundaries.iter().any(|&b| span.start < b && b < span.end),
                "token {:?} crosses a boundary",
                &text[span.clone()]
            );
        }
        let joined: String = encoded.spans.iter().map(|s| &text[s.clone()]).collect();
        assert_eq!(joined, text);
    }
}
