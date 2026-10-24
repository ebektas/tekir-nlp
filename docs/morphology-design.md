# Tekir Morphology: Design, Scalability, and Roadmap

Companion to `docs/oflazer-1994.md`, which maps the linguistic content.
This document covers how `tekir-morph` is built, why it scales, and what
comes next.

## Architecture

```
lexicon TSV ──> Lexicon (immutable: entries + sorted stem index)
                    │
        ┌───────────┴────────────┐
        ▼                        ▼
  generation                 analysis
  (path -> surface)          (surface -> paths)
        └────── realize_noun_morpheme ──────┘
                (single source of truth)
```

### One realization core, two directions

`generator::realize_noun_morpheme` and `generator::realize_derivation` are
the only places a suffix is spelled out. Generation walks an analysis and
appends realizations; analysis walks the same morphotactics and *matches*
realizations against the remaining input. Because realization is
deterministic given its left context (`PhonoContext`: last surface char +
effective harmony vowel), analysis is the exact inverse of generation by
construction. Two property tests enforce this on every run — keep both
forever:

- `generated_surfaces_roundtrip_back_to_their_paths`: every entry × every
  noun path generates, analyzes, and recovers itself;
- `every_analysis_regenerates_its_surface`: every analysis of every
  workload surface (base and derived) regenerates byte-identically via
  `generate_from_igs`.

### Inflectional groups (`^DB`)

An analysis is a chain of inflectional groups: the root group plus one
group per derivation, rendered METU-Sabancı style
(`ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Nom`).
Implemented derivations: `+cH` (Agt), `+lHk` (Ness), `+cHk` (Dim), `+sHz`
(Without), `+lH` (With), the relative `-ki` (Rel) after locative/genitive,
and zero nominalization (Zero) of adjectives. Zero-nominalized `-ki` takes
the pronominal-`n` case forms (`evdekine`, `evdekinde`); genitive endings
loop back into `-ki` (`masadakilerinki`), which is exactly the loop that
makes exhaustive enumeration impossible and surface-driven analysis
necessary. Derived stems ending in `k` voice to `ğ` before vowel-initial
suffixes (`kitaplık -> kitaplığa`), tracked as a one-step constraint
(`KFixup`) between adjacent suffixes. Chains are capped at `MAX_IGS = 8`
groups — deeper words are pruned, a bound real text does not reach.

### Stem alternants instead of runtime string rewriting

Each lexicon entry carries at most two stems:

- `bare_stem` — used word-finally and before consonant-initial suffixes;
- `vowel_stem` — used before vowel-initial suffixes.

That one field encodes final voicing (`kitab`), vowel ellipsis (`burn`),
doubling (`hakk`), `nk -> ng` (`reng`), and y-stems (`suy`). A
`harmony_vowel` override handles non-harmonizing loans (`saat -> saati`).
During analysis, a candidate that matched one alternant is pruned unless
generation would have committed to that same alternant for its first
suffix — which is what rejects `kitapı`, `kitabta`, `burunu`, and `susu`
without any extra rules.

### Surface-driven analysis

`NounAnalyzer::analyze_raw_into`:

1. For each character boundary of the input, binary-search the sorted stem
   index for stems equal to that prefix (O(word length × log stems), no
   allocation).
2. For each candidate stem, walk the nominal morphotactics (inflection,
   derivation, relative `-ki`); epsilon transitions consume nothing, overt
   suffixes must match their realized form; mismatches prune immediately.
3. Emit `RawNounAnalysis` (entry id + fixed-capacity inflectional-group
   array, no strings, `Copy`).

Cost per word is independent of lexicon size — this is what the previous
generate-everything-and-compare analyzer could not offer, and it is what
makes the `-ki` loop analyzable at all. Standing invariant: every cycle
through a morphotactic loop consumes at least one character, and the
branch state (`WalkState`, `IgChain`) stays `Copy` so branching never
allocates.

## Scalability and HPC design rules

These are the standing constraints for this crate; keep them true as
features grow.

1. **Build once, share immutably.** `Lexicon` is constructed once and never
   mutated; `MorphService` wraps it in an `Arc`. Everything on the analysis
   path is `Send + Sync` with zero locks and zero interior mutability, so
   worker threads scale without coordination and results are deterministic
   regardless of thread count or scheduling.
2. **Contiguous memory.** Entries live in a `Vec`; the stem index is one
   sorted array probed by binary search (cache-friendly, no pointer
   chasing). When root counts reach the tens of thousands, swap the index
   for a double-array trie or FST *behind the same `for_each_stem_prefix`
   API*; nothing else changes. String interning into a shared arena is the
   next step after that if entry storage itself becomes hot.
3. **No allocation in the hot path.** Suffix realization writes into a
   fixed 24-byte stack buffer (`SuffixBuf`); candidate walking allocates
   nothing; `analyze_raw_into` appends fixed-size scalar records into a
   caller-owned scratch `Vec`. String-bearing `MorphAnalysis` values are
   built only on demand (`render`), i.e. at the serialization boundary,
   not per candidate.
4. **Two API tiers.** `analyze()` for convenience/JSON; `analyze_raw_into()`
   + `render()` for throughput. Batch and benchmark code must use the raw
   tier with one reused scratch buffer per worker.
5. **Deterministic, order-preserving parallelism.** The CLI batch path
   (`--jobs N`) fans work out over scoped threads and reorders results by
   sequence number; output is byte-identical to sequential runs. The same
   share-nothing structure extends to multi-node sharding: split the input,
   run one process per shard, concatenate — no cross-shard state exists.
6. **Honest measurement.** `cargo run --release -p tekir-morph --example
   bench -- [rounds] [extra_roots|lexicon.tsv]` sweeps thread counts and
   prints words/s, ns/word, and an analysis checksum (which must not vary
   with thread count). The second argument adds deterministic synthetic
   roots or loads a lexicon TSV. Measured on a 64-core machine with
   derivation and `-ki` enabled, after the virtual-trie index descent:

   | lexicon | threads | words/s | ns/word | speedup |
   |---|---:|---:|---:|---:|
   | seed (38 nouns + 35 verbs) | 1 | 0.77M | 1303 | 1.00x |
   | Zemberek (18,196 + seed verbs) | 1 | 0.53M | 1874 | 1.00x |
   | Zemberek (18,196 + seed verbs) | 64 | 17.2M | 58 | 32.16x |

   (Measured with derivation, `-ki`, the copula, and the full verbal
   system enabled; the verbal walk added ~9% on the real lexicon since
   it only runs on verb-root prefix matches.) Near-linear
   to 16 threads; beyond that, memory bandwidth and SMT set the ceiling.
   The real lexicon costs 1.61x the seed per word, inside the 2x
   acceptance bound; part of the residual is genuine ambiguity (real
   roots like `ar`/`ara`/`araba` all legitimately match and walk). Two
   optimizations keep this in check as morphotactics grow: the
   virtual-trie index descent, and conservative first-byte gates on
   copular/derivational attachment points (their completeness is
   test-enforced). A double-array trie/FST behind `for_each_stem_prefix`
   remains the prepared move if the full multi-POS lexicon pushes cost
   further. Re-run and update this table when the analyzer changes.
7. **Text assumptions.** Input is NFC, lowercase Turkish orthography
   (`tekir-normalize` handles case). Comparisons are byte-wise on UTF-8;
   no per-word Unicode normalization is performed in the analyzer.

## Roadmap

Phases are ordered so each unlocks the next; acceptance criteria keep the
quality gate meaningful.

1. **Done — surface-driven core.** Data-driven lexicon (TSV, `--lexicon`),
   stem alternants, harmony overrides, pronominal `n` (Case-2), raw
   analysis tier, thread-scaling benchmark.
2. **Done — real lexicon (Zemberek import).** 18,196 noun roots imported
   from the Apache-2.0 Zemberek-NLP master dictionary into
   `data/zemberek-nouns.tsv` (attribution in the file header) by the
   `import_zemberek` example, which replicates Zemberek's own attribute
   inference: polysyllabic nouns ending in a stop consonant voice by
   default unless `NoVoicing`/`InverseHarmony`; `nk`/`og` endings always
   voice; monosyllables default to no voicing; `LastVowelDrop`, `Doubling`,
   and combinations (`kayıp -> kayb`, `tıp -> tıbb`) compose into
   `vowel_stem`; `InverseHarmony` maps to `harmony_vowel` (`i`/`ü` by
   roundness); `su` is special-cased to its y-stem. Circumflex vowels
   (`â`, `î`, `û`) joined the harmony rules. The `validate` example checks
   every entry x every noun path (1.53M forms): generate -> analyze ->
   regenerate, currently 0 failures in 0.08s on 64 threads. Run it after
   any lexicon or phonology change.
3. **Done — nominal derivation + relative `-ki`.** `+cH`, `+lHk`, `+cHk`,
   `+sHz`, `+lH`, the `-ki` loop with zero re-nominalization, `^DB`
   inflectional groups, derived-stem `k -> ğ` voicing, suffix `c -> ç`
   (rules 14-15), and the regeneration property test. Deferred: `+cA`,
   direct temporal `-ki` (`bugünkü` with rounding harmony), diminutive
   stem truncation (`bebek -> bebecik`).
4. **Done — copula / nominal verb.** Copular predicates are zero-derived
   `Verb` groups (`ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Past+A3sg`):
   present/past/narrative/conditional tenses, present- and past-type
   person series, the `+DHr` marker (`+Cop`: `evdedir`, `evdedirler`,
   `evdeymiştir`), and the adverbials `+ykAn` (`+While`, non-harmonizing)
   and `+ymHşçAsHnA` (`+AsIf`). They attach to inflected nominals,
   derived adjectives (`evsizdi`), and relative/zero groups
   (`evdekiydi`, `evdekilerdeydi`). Ambiguity control: the zero-marked
   third-singular present is never emitted (it would double every
   nominal), and the bare `+lAr` third plural attaches only to
   non-nominative stems (`evdeler` is copular, `evler` stays a plain
   plural). Deferred: stacked copular tenses (`evdeydiyse`), copula on
   underived adjective roots (needs the root-POS lexicon extension).
   Performance note: copular attachment points are pre-filtered by a
   conservative first-byte gate whose completeness is enforced by the
   `first_byte_gates_cover_realizations` test — extend the gate set when
   adding realizations, never narrow it.
5. **Done — verbal morphotactics.** Verb roots live in the lexicon with
   per-root allomorphy data (`data/verbs.tsv` seed: the 13 irregular
   `+Hr` aorists, `t -> d` voicers like `git`/`gid`, causative
   overrides). Implemented: causative (rule-based `+DHr`/`+t` plus
   lexical `+Hr`/`+Ar`, chains up to two: `okutturmuş`), passive
   (`+Hl`/`+Hn`), negation `+mA`, abilitative `+yAbil`, impotential
   `+yAmA`; ten tense/mood exits — progressive with vowel ellipsis
   (`başlıyor`, `yapmıyor` via a next-must-be-progressive constraint),
   `+mAktA`, future with `k -> ğ`, the three-way aorist plus the
   irregular negative aorist (`yapmam`/`yapmayız`/`yapmazsınız`), past,
   narrative, necessitative, optative (`yapalım`), imperative
   (`yapsınlar`), conditional — with second tenses reusing the copular
   realizations (`yapıyordu`, `yapsaydı`; third plural precedes the
   second tense: `yapıyorlardı`); and the verb-to-nominal exits:
   infinitives (`yapmak`/`yapma`/`bekleyiş`), four participles
   (`yapan`, `yaptığım` with possessive-or-case, `yapacağı`,
   `yapmış`), converbs (`yapıp`, `yaparak`, `gitmeden`), and
   `yaparken`/`koşarcasına`. Deferred: reflexive/reciprocal voice, the
   long imperative `+yHnHz`, `mA+yAbil` stacks (`yapmayabilir`),
   stacked pasts (`yaptıydı`), person-after-second order
   (`yapıyordular`).
6. **Done — robust text layer.** The unknown-word guesser (every prefix
   of at least two characters as a nominal root, `+Unk`-marked, lexicon
   misses only), apostrophe suffixation with the full nominal/copular
   machinery on the suffix side (`Ankara'dakiler`, `TBMM'ye`,
   `1990'larda`, `5'te` — vowel-less stems try all harmony classes), and
   the question particle (`mi`, `miydi`, `musunuz`) as a `+Ques` root
   with copular tails. All three share the analyzer's tail primitives.
7. **Done (v1) — disambiguation (`tekir-disamb`).** An averaged
   perceptron in the Sak/Zemberek lineage: FNV-hashed features over the
   candidate's root, tag chain, inflectional groups, and the two
   preceding tag chains; teacher-forced training with lazy averaging;
   beam decoding (width 8). Trained on UD_Turkish-Kenet (CC BY-SA 4.0;
   15,398 sentences) with gold analyses recovered by matching UD
   lemma/UPOS/features against tekir's candidates — 99.4% of analyzable
   words map. Results on the held-out Kenet test set (15,017 gold
   tokens): heuristic baseline 70.3%, perceptron **94.8%**. Training
   takes ~1.5s on CPU; the shipped model
   (`crates/tekir-disamb/models/kenet.tkd`, ~750KB, ~57k features) loads
   in milliseconds and decodes deterministically in microseconds per
   sentence. Reproduce with `cargo run --release -p tekir-disamb
   --example train` (see its header for flags). Closed-class
   function words now live in `data/closed.tsv` (235 hand-written full
   forms: irregular pronoun paradigms like `bana`/`onunla`/`bunun`,
   postpositions, conjunctions, determiners, core adverbs), each
   accepting copular tails (`kimdi`, `gibiydi`) and — for
   locative/genitive forms — the full relative-`-ki` continuation
   (`benimkini`, `ondakiler`). The shipped model is trained on all four
   CC BY-SA TABILEX treebanks (Kenet, FrameNet, Tourism, Penn; 321k gold
   tokens): with closed classes in play (a harder, more honest candidate
   space) it scores 94.4% on the Kenet test set and 95.0% on Penn
   against a 65.8% heuristic baseline. Known limits: accuracy is
   measured against heuristically mapped gold; next steps are richer
   features (surface shape, frequency priors), silver-data augmentation,
   and optionally a candle-based neural rescorer.

## Zemberek dictionary: coverage status

The import now covers the four open word classes of
`master-dictionary.dict`, written as one file per class (each with a
`#! pos:` directive) and merged at load time (`Lexicon::from_tsv_paths`,
CLI: repeatable `--lexicon`):

| file | entries | notes |
|---|---:|---|
| `zemberek-nouns.tsv` | 18,196 | attribute inference as documented above |
| `zemberek-adjectives.tsv` | 5,056 | same inference; root groups render `+Adj`, inflect via zero nominalization (`güzeller`), nominalize with `+lHk`, and take the copula (`yorgundu`) |
| `zemberek-verbs.tsv` | 3,219 | `-mAk` stripped; `Voicing` -> vowel stem (`affed`), `Aorist_I`/`Aorist_A` -> aorist overrides (`gelir`, `affeder`) |
| `zemberek-compounds.tsv` | 327 | `CompoundP3sg` + `Roots:`; bare stem = concatenated roots (`buzdolapları`), vowel stem = citation form minus the `(s)H` marker (`buzdolabım`, ellipsis compounds like `aslanağzı` included); inflection requires a possessive |

26,798 roots total — beyond the paper's ~23,000. The validator accepts
the file list and checks every legal path (compounds only over
possessive-bearing paths): 1.55M forms, 0 failures.

Still uncovered, in rough priority order:

- **Done: denominal verbs and the instrumental.** `+lAş` (`+Become`:
  `güzelleşti`, `çocuklaştı`) and `+lAn` (`+Acquire`: `evlendik`) derive
  verbs from noun and adjective stems productively, feeding the full
  verbal machinery (`güzelleştirdi`). `+lA` is deliberately not derived
  productively: its bare aorist collides with every plural (`evler`), and
  lexicalized `+lA` verbs (`sulamak`, `oynamak`) come from the
  dictionary. The instrumental case `+ylA` was added alongside
  (`kitapla`, `arabasıyla`, `evdekiyle`) so those forms are not
  mislabeled — noun paths are now 98 per stem. Fixing the family also
  fixed progressive-ellipsis harmony to follow the last remaining vowel
  (`oynuyor`, `okumuyor`, `suluyor`), not the elided one.
- **Pre-passive vowel drop** in 14 verbs (`çağır -> çağrıl`); imported
  without the drop for now.
- **Plural-only and restricted roots**: `ImplicitPlural` (379, imported
  as plain nouns — mild overgeneration), `NoSuffix` (10, skipped),
  `ImplicitDative` (8, imported plain).
- **Proper nouns and abbreviations** from the separate source files
  (`proper.dict`, `person-names.dict`, `abbreviations.dict`); apostrophe
  suffixation already covers unlisted proper stems.
- **Closed classes** (pronouns with irregular inflection such as
  `ben -> bana`, postpositions, conjunctions) — small hand-written
  lexicons with dedicated paradigms.
- **Refresh procedure.** Re-download the source file, re-run
  `import_zemberek <dict> <output-dir>`, then `validate` over all four
  files (must stay at 0 failures) and `bench` (update the table above).
  The importer is deterministic, so TSV diffs review cleanly.
