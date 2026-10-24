# Tekir Architecture

Tekir is a Rust Cargo workspace for Turkish NLP tooling. The current design is
intentionally simple and service-oriented: each NLP capability lives in its own
crate, shared types live in `tekir-core`, and the CLI is a thin integration
layer over those services.

## Goals

- Build a modern Rust-first Turkish NLP toolkit
- Favor deterministic, heuristic, and statistical methods first
- Keep APIs composable across CLI, libraries, and future services
- Make structured output stable enough for downstream automation
- Support batch processing without forcing heavyweight infrastructure

## Workspace Layout

- `crates/tekir-core`
  - Shared request/response traits and common data types
  - Core token and segment data structures
- `crates/tekir-morph`
  - Turkish morphological analysis following Oflazer (1994); noun
    inflection implemented, see [docs/oflazer-1994.md](docs/oflazer-1994.md)
    and [docs/morphology-design.md](docs/morphology-design.md)
- `crates/tekir-normalize`
  - Baseline text normalization
- `crates/tekir-tokenize`
  - Tokenization and token classification
- `crates/tekir-segment`
  - Rule-based text segmentation built on top of tokenization
- `crates/tekir-disamb`
  - Averaged-perceptron morphological disambiguation and the sentence
    pipeline (tokenize -> analyze -> pick best analysis in context);
    bundles a trained model and uses the full embedded lexicon
- `crates/tekir-cli`
  - Command-line interface and batch-processing entrypoint; with the
    optional `server` feature, also the HTTP JSON service (`tekir serve`)

## Core Design

Each service exposes plain methods (`MorphService::analyze`,
`TokenizeService::tokenize`, ...) that return a shared
`TextTransformResponse<Output>` envelope, defined in
[crates/tekir-core/src/service.rs](crates/tekir-core/src/service.rs): the
original input, its `input_len_bytes`, and the service-specific output.

## Data Model

The token and segment model is defined in [crates/tekir-core/src/token.rs](crates/tekir-core/src/token.rs).

### Tokens

Current token shape:

```rust
pub struct Token {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: TokenKind,
}
```

Current `TokenKind` values:

- `word`
- `apostrophed_word`
- `number`
- `abbreviation`
- `punctuation`
- `grapheme`
- `line`

This is the base layer for later morphology, NER, and alignment-sensitive
features.

### Segments

Current segment shape:

```rust
pub struct SegmentSpan {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: SegmentKind,
}
```

Current `SegmentKind` values:

- `sentence`
- `heading`
- `lead_in`

The segment layer is broader than sentence splitting. It is intended to capture
news-style and document-style structure, not only punctuation boundaries.

## Service Crate Rules

When adding a new NLP crate:

- Put reusable data structures in `tekir-core` only if multiple crates need them
- Expose a small service type such as `XService`
- Return a `TextTransformResponse<Output>` from its methods
- Keep deterministic logic in the crate itself, not in the CLI
- Add focused unit tests in the crate before wiring new CLI behavior

Recommended naming pattern:

- `XMode`
- `XOutput`
- `XResponse`
- `XService`

## CLI Design

The CLI is implemented in [crates/tekir-cli/src/main.rs](crates/tekir-cli/src/main.rs).

Current commands:

- `morph`
- `disamb`
- `spell`
- `serve` (with the `server` feature)
- `normalize`
- `tokenize`
- `segment`

The CLI should remain thin:

- parse arguments
- select input mode
- call a service
- render output

The CLI should not become the place where NLP logic lives.

## Output Contracts

Tekir currently supports three human/programmatic output classes:

- `text`
  - default, concise, shell-friendly
- `debug`
  - human inspection with spans and metadata
- `json`
  - structured single-result envelope
- `jsonl`
  - structured batch output, one record per line

### JSON Envelope

Single-result JSON output is wrapped as:

```json
{
  "schema_version": "1",
  "task": "segment",
  "data": { "...": "..." }
}
```

### JSONL Envelope

Batch output uses one compact JSON object per line:

```json
{
  "schema_version": "1",
  "task": "segment",
  "id": "optional",
  "data": { "...": "..." }
}
```

Conventions:

- `schema_version` is top-level and CLI-owned
- `task` identifies the producing command
- enum values serialize as `snake_case`
- service response structs remain the payload under `data`

For `morph`, human-facing output is intentionally split:

- `text`
  - fast plain output for pipelines and quick inspection
  - prints one lexical analysis per line
- `debug`
  - richer analysis-oriented output with lemma, surface, features, and morphemes

`tekir morph` also supports `--best-only` as a CLI-level rendering option. In
the current baseline it selects the first returned analysis; later it can map
to a real disambiguation or ranking layer without changing the flag.

Structured morphology responses expose both:

- `best_analysis`
  - currently the first returned analysis, if any
- `analyses`
  - the full candidate list

## Batch Processing

Batch processing currently exists for `morph`, `segment`, and `tokenize`.

Input contract for `--input-format jsonl`:

```json
{"id":"1","text":"Merhaba dunya"}
{"text":"Ankara'ya gidiyor mu?"}
```

Rules:

- `text` is required
- `id` is optional
- blank lines are ignored
- parse errors report the source line number

### Streaming and Parallelism

The batch path is stream-oriented:

- sequential mode processes one line at a time
- parallel mode uses `--jobs N`
- output order is preserved even when processing in parallel

This design keeps the batch API stable while leaving room for later internal
optimization.

## Current Heuristics

### Normalization

- quote/apostrophe normalization
- whitespace collapsing
- Turkish-aware lowercasing

### Tokenization

- punctuation-aware Turkish tokenization
- apostrophe-attached forms like `Ankara'ya`
- basic abbreviation and number classification

### Segmentation

- punctuation-based sentence boundaries
- abbreviation exceptions such as `Dr.`
- heading detection for all-caps/news headings
- `lead_in` detection for colon-ended intro lines
- paragraph-aware splitting

### Morphology

The design blueprint is Oflazer (1994); see
[docs/oflazer-1994.md](docs/oflazer-1994.md) for the rule-by-rule mapping and
[docs/morphology-design.md](docs/morphology-design.md) for the architecture,
scalability rules, and roadmap.

- noun inflection: root -> number -> possessive -> case, including the
  pronominal `n` case forms after third-person possessives (`evine`,
  `evinde`, `masasının`)
- data-driven root lexicon (TSV; embedded seed plus `--lexicon PATH`) with
  stem alternants covering final voicing (`kitab`), vowel ellipsis
  (`burnu`), consonant doubling (`hakkı`), `nk -> ng` (`rengi`), y-stems
  (`suyu`), and harmony overrides for loans (`saati`, `alkolü`)
- vowel harmony, buffer consonants, and dental-stop alternation shared
  between generation and analysis through one realization core
- surface-driven analysis: stem-index lookup plus a bounded state-chain
  walk; cost per word is independent of lexicon size, no allocation in the
  matching path, and ill-formed alternant use (`kitapı`, `kitabta`) is
  rejected
- nominal derivation as inflectional groups with `^DB` boundaries: `+cH`
  (`kitapçı`), `+lHk` (`kitaplık`), `+cHk` (`ağaççık`), `+sHz`/`+lH`
  (`evsiz`, `evli`), and the relative `-ki` loop with zero
  re-nominalization (`evdekilerinki`), including derived-stem `k -> ğ`
  voicing (`kitaplığa`) and suffix `c -> ç` devoicing
- the copula as zero-derived verb groups: four tenses, both person
  series, `+DHr`, and the `+ykAn`/`+ymHşçAsHnA` adverbials (`evdeydi`,
  `evdedirler`, `evdeymişçesine`), with ambiguity control for the
  zero-marked forms
- full verbal morphotactics: voice chains, negation and ability, ten
  tense-mood exits with second tenses, and the infinitive/participle/
  converb bridges back into nominal inflection (`yaptırılmıştı`,
  `yaptıklarımızdan`, `gitmeden`)
- robust text: apostrophe suffixation (`Ankara'dakiler`, `1990'larda`),
  the detached question particle (`miydi`), and an `+Unk`-marked
  unknown-word guesser as a lexicon-miss fallback
- immutable, lock-free shared lexicon (`Arc`) for parallel batch work, and
  a raw analysis API plus benchmark example for throughput measurement
- explicit lexical form with stable tags such as `ev+Noun+A3pl+Pnon+Nom`
  and `ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel`

## Evolution Guidelines

When extending Tekir:

- Prefer adding a new crate over overloading an existing one
- Prefer explicit structured outputs over ad hoc string parsing
- Keep JSON schema changes additive where possible
- Change `schema_version` if top-level structured output becomes incompatible
- Preserve fast plain-text output for shell workflows
- Add batch support only when the single-item API shape is already stable

## Near-Term Roadmap

Morphology follows the phased plan in
[docs/morphology-design.md](docs/morphology-design.md): lexicon growth,
nominal derivation with `^DB` inflectional groups, the copula, verbal
morphotactics with predicate-carrying transitions, an unknown-word guesser,
and apostrophe suffixation.

Logical next modules:

- `tekir-disamb`
  - morphological disambiguation
- `tekir-spell`
  - spelling correction and deasciification
- `tekir-ner`
  - rule/statistical named entity recognition

Likely supporting improvements:

- richer token kinds
- sentence/segment confidence or rule-source metadata
- JSON schema fixtures
- benchmark harnesses for batch throughput and latency

## Verification Standard

Changes should normally pass:

```bash
cargo fmt --all
cargo test
cargo clippy --workspace --all-targets -- -D warnings
```

This is the baseline quality gate for the workspace.
