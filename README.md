# Tekir-nlp

> This project was started for me to learn rust and complete years of unfinished nlp projects with the help of claude, thanks AAA & Zemberek for the inspo.

Tekir is a Rust toolkit for Turkish NLP, built around a morphological
analyzer that follows Oflazer (1994). It is meant as a modern take on
Zemberek: useful when working on LLMs — tokenizer training, corpus
cleaning, evaluation data — and fast enough for LLM data-prep scale.

## Crates

| crate | what it does |
|---|---|
| `tekir-morph` | morphological analyzer + generator, morpheme segmentation; 26.8k roots imported from Zemberek |
| `tekir-disamb` | averaged-perceptron disambiguator, trained on UD Turkish treebanks (model included) |
| `tekir-subword` | morpheme-aware pre-tokenizer, byte-level BPE training (`tokenizer.json`), tokenizer evaluation |
| `tekir-prep` | corpus cleaning: PII redaction, MinHash dedup, quality signals |
| `tekir-spell` | spell checking, suggestions, diacritic restoration in context |
| `tekir-normalize`, `tekir-tokenize`, `tekir-segment` | baseline text preprocessing |
| `tekir-py` | Python bindings (`import tekir`) |
| `tekir-core` | shared types |
| `tekir-cli` | CLI and optional HTTP server |

## Usage

```bash
cargo test
cargo run -p tekir-cli -- morph "evdekilerinki"
cargo run -p tekir-cli -- disamb "Hasta çocuklar evlerinde kitap okuyorlardı."
cargo run -p tekir-cli -- spell "kitaplr"
cargo run -p tekir-cli -- segment "Dr. Ahmet geldi. Merhaba dedi."
cargo run -p tekir-cli -- morph --segments "kitaplarımızdan"   # kitap|lar|ımız|dan
cargo run -p tekir-cli -- restore "cok guzel bir gun"          # çok güzel bir gün
```

LLM data prep:

```bash
tekir-cli pretokenize "Kitaplarımızdan biri geldi."     # Kitap|lar|ımız|dan| biri| gel|di|.
tekir-cli tokenizer train --corpus corpus.txt --out tokenizer.json --vocab-size 32000
tekir-cli tokenizer eval --tokenizer tokenizer.json --corpus held-out.txt --morph
tekir-cli prep docs.jsonl --restore-diacritics --redact-pii --dedup --filter > clean.jsonl
```

Python (`pip install maturin && maturin develop -m crates/tekir-py/Cargo.toml`):

```python
import tekir
tekir.segment("kitaplarımızdan")              # ['kitap', 'lar', 'ımız', 'dan']
tok = tekir.train_tokenizer(lines, vocab_size=32000)
tok.encode("Evlerimizde kitaplar var.")       # morpheme-aligned ids
tekir.restore("cok guzel"), tekir.redact_pii(text), tekir.quality(text), tekir.dedup(docs)
```

The saved `tokenizer.json` is a standard Hugging Face tokenizer;
`hf.encode(tekir.Pretokenizer().pretokenize(text), is_pretokenized=True)`
reproduces tekir's ids. Measurements are in `docs/llm-toolkit.md`.

Everything loads the full built-in lexicon; pass `--lexicon PATH`
(repeatable) to use your own TSV files instead.

Batch mode: `--stdin --input-format jsonl --output jsonl --jobs N` reads
`{"id": ..., "text": ...}` lines and keeps output order.

Server: `cargo build --release -p tekir-cli --features server`, then
`tekir-cli serve --port 8321` and `POST /morph|/disamb|/spell|...` with
`{"text": "..."}`.

Docker (server only, everything embedded):

```bash
docker build -t tekir-nlp .
docker run --rm -p 8321:8321 tekir-nlp
```

## Docs

- `docs/llm-toolkit.md` — segmentation, tokenizers, corpus cleaning, with measurements
- `docs/oflazer-1994.md` — how the paper maps onto tekir
- `docs/morphology-design.md` — architecture, performance, roadmap
- `ARCHITECTURE.md` — workspace overview

## License

MIT. Lexicon data derived from [Zemberek-NLP](https://github.com/ahmetaa/zemberek-nlp)
(Apache-2.0); disambiguation model trained on UD Turkish treebanks (CC BY-SA 4.0).
