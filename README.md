# Tekir

> This project was started for me to learn rust and complete years of unfinished nlp projects with the help of claude, thanks AAA & Zemberek for the inspo.

Tekir is a Rust toolkit for Turkish NLP, built around a morphological
analyzer that follows Oflazer (1994).

## Crates

| crate | what it does |
|---|---|
| `tekir-morph` | morphological analyzer + generator; 26.8k roots imported from Zemberek |
| `tekir-disamb` | averaged-perceptron disambiguator, trained on UD Turkish treebanks (model included) |
| `tekir-spell` | spell checking, suggestions, deasciification |
| `tekir-normalize`, `tekir-tokenize`, `tekir-segment` | baseline text preprocessing |
| `tekir-core` | shared types |
| `tekir-cli` | CLI and optional HTTP server |

## Usage

```bash
cargo test
cargo run -p tekir-cli -- morph "evdekilerinki"
cargo run -p tekir-cli -- disamb "Hasta çocuklar evlerinde kitap okuyorlardı."
cargo run -p tekir-cli -- spell "kitaplr"
cargo run -p tekir-cli -- segment "Dr. Ahmet geldi. Merhaba dedi."
```

Everything loads the full built-in lexicon; pass `--lexicon PATH`
(repeatable) to use your own TSV files instead.

Batch mode: `--stdin --input-format jsonl --output jsonl --jobs N` reads
`{"id": ..., "text": ...}` lines and keeps output order.

Server: `cargo build --release -p tekir-cli --features server`, then
`tekir-cli serve --port 8321` and `POST /morph|/disamb|/spell|...` with
`{"text": "..."}`.

## Docs

- `docs/oflazer-1994.md` — how the paper maps onto tekir
- `docs/morphology-design.md` — architecture, performance, roadmap
- `ARCHITECTURE.md` — workspace overview

## License

MIT. Lexicon data derived from [Zemberek-NLP](https://github.com/ahmetaa/zemberek-nlp)
(Apache-2.0); disambiguation model trained on UD Turkish treebanks (CC BY-SA 4.0).
