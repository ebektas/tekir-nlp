# Tekir for LLM work

What tekir adds on top of the analyzer for tokenizer training and corpus
preparation, and what it measures like. All numbers below are from a
64-core machine; corpora are UD_Turkish-Kenet sentences (train: 15.4k
sentences / 143k words; held-out: dev + test, 3.3k / 35k).

## Morpheme segmentation

Every analysis can split its surface into morphemes
(`MorphAnalysis::morphemes`, CLI `morph --segments`):
`kitap|lar|ımız|dan`, `kitab|a`, `ok|uyor|lar|dı`, `Ankara|'|da|ki|ler`.
Boundaries come from regenerating the analysis with the shared suffix
realizer while recording where each overt suffix starts — exact by
construction, since every analysis regenerates its surface byte for byte
(test-enforced). Phonology lands where it happens: voicing stays on the
stem (`kitab|a`), progressive ellipsis takes the stem vowel (`ok|uyor`).
The analyzer hot path is untouched; boundaries are computed on demand.

## Corpus-scale analysis

`MorphService::analyze_batch` analyzes each distinct word once and fans
out over scoped threads, order-preserving and thread-count independent.
The pre-tokenizer keeps a cross-batch word cache (`BoundaryCache`).
On the 149k-word corpus (30% distinct words — real corpora are far more
repetitive):

| | 1 thread | 64 threads |
|---|---:|---:|
| analyze every occurrence | 0.27M words/s | 8.1M words/s |
| `pretokenize_batch`, cold cache | 0.55M words/s | 2.9M words/s |
| `pretokenize_batch`, warm cache | 4.7M words/s | 14.5M words/s |

Cold-cache runs pay a fixed thread fan-out cost that a small corpus does
not amortize; at corpus scale the cache stays warm. From Python,
`Pretokenizer.pretokenize_batch` runs at ~2.8M words/s (warm), the
difference being Python string creation. Reproduce:
`cargo run --release -p tekir-subword --example bench -- corpus.txt [threads]`.

## Morpheme-aware tokenizers

`tekir-subword` pre-tokenizes losslessly (pieces join back to the input;
GPT-style: a single leading space attaches to the next piece, other
whitespace is its own piece, digits group by three) and splits words at
the boundaries of their best analysis. Words the analyzer only guesses
stay whole (`split_unknown` to change that). Byte-level BPE trained on
these pieces can never merge across a morpheme boundary; the result is a
standard `tokenizer.json` (ByteLevel pre-tokenizer without regex, ByteLevel
decoder) that decodes exactly and loads in Hugging Face `tokenizers`.

`tokenizer eval` scores any `tokenizer.json` on held-out text. Boundary
scores compare in-word token boundaries with tekir's morpheme boundaries
over words with an analysis.

| tokenizer | tokens/word | single-token words | boundary P | boundary R | boundary F1 | aligned words |
|---|---:|---:|---:|---:|---:|---:|
| tekir morph BPE, 8k | 2.32 | 34% | 0.85 | 1.00 | **0.92** | **87%** |
| plain BPE, same data, 8k | **1.88** | 51% | 0.44 | 0.31 | 0.36 | 73% |
| Qwen2.5 (152k) | 2.65 | 25% | 0.28 | 0.41 | 0.33 | 39% |
| GPT-2 (50k) | 3.79 | 12% | 0.23 | 0.60 | 0.33 | 16% |

The honest reading: morpheme alignment costs compression — about 24% more
tokens than plain BPE on the same data at this vocabulary size (the
baseline memorizes whole frequent words). Whether better-aligned tokens
are worth more tokens is a training question this scorecard lets you
measure rather than assume. The corpus is tiny; re-run at your scale:

```bash
tekir-cli tokenizer train --corpus train.txt --out morph.json --vocab-size 32000
tekir-cli tokenizer train --corpus train.txt --out plain.json --vocab-size 32000 --baseline
tekir-cli tokenizer eval --tokenizer morph.json --corpus heldout.txt --morph
tekir-cli tokenizer eval --tokenizer plain.json --corpus heldout.txt
```

## Corpus cleaning (`tekir-prep`, `tekir prep`)

- **PII**: rule-based, one pass, no regex. TCKN (both check digits), TR
  IBAN (mod-97), cards (Luhn + grouping), Turkish phones, email, IPv4;
  checksums keep false positives down. Bare landlines without a `0`/`+90`
  prefix are not detected, by design. For names, pair with a learned model.
- **Dedup**: exact (128-bit) plus MinHash over word 5-gram shingles with
  LSH banding, deterministic clustering (first document wins).
- **Quality**: Gopher/FineWeb-style signals plus Turkish ones — share of
  Turkish letters and *analyzer coverage* (words the lexicon can analyze
  rather than guess): 0.95–0.97 on Kenet paragraphs, 0.25 on Lorem ipsum.
  Thresholds are starting points; tune them on your corpus. The
  line-ending-punctuation rule is binary on single-line documents.

## Diacritic restoration

`Restorer` (CLI `restore`, `prep --restore-diacritics`) offers every
ASCII-for-Turkish variant the analyzer accepts as candidate analyses and
lets the disambiguator choose in context. Kenet held-out text with all
Turkish letters stripped:

| | word accuracy | on words that lost letters |
|---|---:|---:|
| no restoration | 59.9% | 0% |
| per-word (first valid variant) | 89.9% | 75.0% |
| in context (`Restorer`) | **94.7%** | **88.5%** |

3.3k sentences restore in 53ms. The disambiguator was trained on Kenet's
training split, so this is in-domain. Proper-noun stems (`Istanbul'a`) are
left alone until a proper-noun lexicon is imported. Reproduce:
`cargo run --release -p tekir-spell --example restore_eval -- heldout.txt`.

## Python

`crates/tekir-py` builds the `tekir-nlp` wheel (abi3, Python ≥ 3.9) with
maturin. Batch calls release the GIL and use all cores; the lexicon and
model load once per process.
