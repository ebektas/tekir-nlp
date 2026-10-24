# Oflazer (1994) as Tekir's Morphology Blueprint

Reference: Kemal Oflazer, "Two-level Description of Turkish Morphology",
*Literary and Linguistic Computing*, Vol. 9, No. 2, 1994.
(Bilkent repository: <https://repository.bilkent.edu.tr/server/api/core/bitstreams/650eee79-6ab1-43c5-97ae-3199fb1f159a/content>)

This is the foundational computational description of Turkish morphology.
Nearly every later analyzer (Xerox Turkish FST, TRmorph, Zemberek, Zeyrek)
descends from its design. `tekir-morph` implements its linguistic content in
Rust with a modernized architecture.

## What the paper contains

1. **22 two-level phonological rules** (PC-KIMMO / Koskenniemi formalism):
   vowel harmony via `A`/`H` archiphonemes, vowel deletion after vowel-final
   stems, root vowel ellipsis, suffix-initial `s/y/n` dropping, dental `D`
   (d/t), final devoicing, `c -> ç`, `k -> ğ`, `nk -> ng`, `g -> ğ`, the
   `su`/`Y` special case, proper-noun apostrophe cleanup.
2. **Morphotactics as finite-state machines**: a nominal FSM (plural ->
   possessive -> case, the relative `-ki` loop, derivational suffixes, and
   the copular "nominal verb" branch), a compound-noun FSM, and a much
   larger verbal FSM (voice -> negation -> auxiliaries -> tense/aspect ->
   second tense -> person, with exits into participles and adverbs).
3. **A ~23,000-root lexicon** in 12 sub-lexicons (about 18,500 nominals,
   2,450 verbs, plus pronouns, postpositions, acronyms, special cases),
   annotated with per-root exception behavior. (Tekir's counterpart:
   26,798 roots — 18,196 nouns, 5,056 adjectives, 3,219 verbs, and 327
   compounds — imported from the Apache-2.0 Zemberek-NLP master
   dictionary; see `docs/morphology-design.md`.)

Known limits the paper states itself: the 13 irregular aorist verbs
(tekir: lexicon flags), the detached question particle `mi` (tekir:
handled as a `+Ques` root with copular tails), no disambiguation (tekir:
still ahead, `tekir-disamb`), and (section 6) a wish for FSM transitions
with conditions and bound variables (tekir: per-root flags plus
phonological predicates in the realization rules).

## Rule-by-rule status in Tekir

| Paper rule | Phenomenon | Tekir status |
|---|---|---|
| 1–2 (`A`) | Two-way vowel harmony | `rules::a_harmony` over `PhonoContext` |
| 3–6 (`H`) | Four-way vowel harmony | `rules::i_harmony` over `PhonoContext` |
| 7 | `H` deletion after vowel-final stems | realization variants in `generator` (`masam`, `masanız`) |
| 8–9 | `+Hyor` vowel ellipsis | elision variants with a next-must-be-progressive constraint (`başlıyor`, `yapmıyor`) |
| 10 (`$`) | Root vowel ellipsis (`burun -> burnu`) | lexicon `vowel_stem` alternant |
| 11 (`X`) | Suffix-initial `s/y/n` dropping | buffer logic in `realize_noun_morpheme`; pronominal `n` after 3rd possessive (`evine`, `evinde`) covers the paper's Case-2 states |
| 12 (`D`) | Dental `d`/`t` | `rules::stop_consonant` |
| 13 | Final devoicing `b,d -> p,t` | lexicon `vowel_stem` alternant (`kitap`/`kitab`) |
| 14–15 | `c -> ç` in suffixes | `rules::affricate_consonant` (`kitapçı`, `ağaççık`) |
| 16 | `k -> ğ` | lexicon `vowel_stem` alternant (`çocuk`/`çocuğ`); `erk` (paper's `K`) is simply an entry without one |
| 17 | `nk -> ng` | lexicon `vowel_stem` alternant (`renk`/`reng`) |
| 18–19 | `g -> ğ` | `vowel_stem` alternants from the Zemberek import (`diyalog`/`diyaloğ`) |
| 20–21 (`Y`) | `su` y-stems | lexicon `vowel_stem` alternant (`su`/`suy`) |
| 22 (`'`) | Proper-noun apostrophe | implemented: apostrophe suffixation over opaque stems (`Ankara'dakiler`, `TBMM'ye`, `1990'larda`) |
| — | Consonant doubling (`hak -> hakkı`), special case 4 | lexicon `vowel_stem` alternant |
| — | Non-harmonizing loans (`saat -> saati`), special case | lexicon `harmony_vowel` override |
| — | Polysemous doubling (`şık`), special case 6 | homograph lexicon entries |

A deliberate architectural trade: rules the paper encodes as global string
constraints with lexical meta-symbols (`$`, `K`, `S`, `Y`) become **per-root
stem alternants** in the lexicon. One mechanism (`vowel_stem`) covers rules
10, 13, 16, 17, 20–21 and the doubling special cases. When the lexicon is
later built from raw word lists, those rules return as *generators of
alternants* at lexicon-compile time — not as per-word runtime machinery.

## Morphotactics status

| Paper FSM | Tekir status |
|---|---|
| Nominal: plural -> possessive -> case (+ Case-2 pronominal `n`) | implemented (`state.rs` transition table + `analyzer.rs` walk) |
| Nominal derivation: `+cH` (Agt), `+lHk` (Ness), `+cHk` (Dim), `+sHz` (Without), `+lH` (With), with derived-stem `k -> ğ` voicing | implemented as inflectional groups (`^DB`) |
| Denominal verbs `+lAş`/`+lAn` (paper's NtoV suffixes) | implemented (`+Become`/`+Acquire`: `güzelleştirdi`); productive `+lA` deferred (plural/aorist collision), lexicalized `+lA` verbs come from the dictionary |
| Relative `-ki` loop after locative/genitive, incl. zero re-nominalization with pronominal-`n` case (`evdekine`, `masadakilerinki`) | implemented; direct temporal `-ki` (`bugünkü`, `dünkü` with rounding harmony) deferred |
| Nominal verb / copula: present/past/narrative/conditional with both person series, `+DHr` (`+Cop`), `+ykAn` (`+While`), `+ymHşçAsHnA` (`+AsIf`) | implemented (`evdeydi`, `evdedirler`, `evdeymişçesine`); second copular tense stacking (`evdeydiyse`) deferred |
| Compound nouns (`bitpazarı`): lexicalized `(s)H` marker, suffixes on the second component's stem (`buzdolapları`/`buzdolabım`) | implemented as possessive-required entries with stem alternants |
| Verbal FSM: causative/passive voice with root-class allomorphy, negation/abilitative/impotential, ten tense-mood exits incl. the irregular negative aorist and the 13 `+Hr` aorists, second tenses, infinitives/participles/converbs | implemented (`yaptırıldı`, `yapamayacaktı`, `yapmam`, `yaptığımda`); root-class conditions live as lexicon flags plus phonological rules — the data-driven equivalent of the conditional transitions section 6 asks for. Deferred: reflexive/reciprocal, `+yHnHz`, `mA+yAbil` stacks |

## Deltas from the paper (modernizations)

Kept from the paper: the rule inventory, the morphotactic structure, the
lexicon-with-exception-annotations design, and the invariant that analysis
is the inverse of generation.

Changed or added:

- **No two-level rule compiler.** Phonology is procedural Rust shared
  between generation and analysis (the Zemberek approach). The two-level
  formalism was PC-KIMMO's constraint; the rules' content is what matters.
- **Surface-driven analysis** instead of PC-KIMMO's parallel constraint
  machines: stem-indexed lookup plus a bounded FSM walk. See
  `docs/morphology-design.md`.
- **Modern tagset output** (`ev+Noun+A3pl+Pnon+Nom`), matching the
  METU-Sabancı treebank conventions rather than the paper's `N(ev)+PLU`
  glosses, including `^DB` derivational boundaries between inflectional
  groups (`ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Nom`).
- **Beyond the paper, implemented**: the unknown-word guesser (`+Unk`),
  number/acronym/proper-noun apostrophe suffixation (`1990'larda`,
  `TBMM'ye`), and the question particle `mi` as a `+Ques` root with
  copular tails (analyzable here because the tokenizer sees word
  sequences, which PC-KIMMO could not). Still ahead: a statistical
  disambiguator (`tekir-disamb`) and treebank-based evaluation.
- **Dropped**: the lexical meta-symbol alphabet, and the paper's
  "input is assumed legal" caveat — the alternant-commitment check makes
  Tekir reject ill-formed forms like `kitapı`, `kitabta`, `susu`.
