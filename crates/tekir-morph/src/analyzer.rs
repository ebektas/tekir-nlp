//! Surface-driven morphological analysis over inflectional groups.
//!
//! Analysis walks the input: candidate stems come from a sorted stem index,
//! and the suffix tail is consumed by traversing the nominal morphotactics
//! (inflection, derivation, and the relative `-ki` loop) while realizing
//! each candidate morpheme with the shared generator rules. Because suffix
//! realization is deterministic given its left context, matching realized
//! suffixes against the remaining input makes analysis the exact inverse of
//! generation — `generate_from_igs` regenerates every analysis.
//!
//! Cost per word is O(word length) index probes plus a bounded walk per
//! matching stem — independent of lexicon size. Morphotactic loops are safe
//! because every derivational cycle consumes input, and the chain is capped
//! at [`MAX_IGS`] inflectional groups (deeper words are pruned; real text
//! never reaches the cap).

use crate::{
    analysis::{AnalysisSource, MorphAnalysis, PartOfSpeech},
    generator::{
        SuffixBuf, aorist_is_hr, copula_person_is_past_type, derivation_has_voicing_k,
        harmony_seed, realize_causative, realize_copula_marker, realize_copula_person,
        realize_copula_tense, realize_derivation, realize_imperative_person,
        realize_neg_aorist_person, realize_neg_aorist_z, realize_noun_morpheme, realize_passive,
        realize_verb_tense, vowel_alternant_context,
    },
    lexical::{build_lexical_noun_form, gloss_from_igs},
    lexicon::{Lexicon, LexiconEntry, RootPos, StemAlternant},
    rules::{PhonoContext, is_vowel, starts_with_vowel},
    state::{
        Agreement, Case, CopulaTense, Derivation, IgPos, InflectionGroup, NounMorpheme, NounPath,
        Number, Polarity, Possessive, VerbTense,
    },
};

/// Maximum inflectional groups per analysis (root group included).
pub const MAX_IGS: usize = 8;

/// Allocation-light analysis result: an entry id plus a fixed-capacity
/// inflectional-group chain, no strings. Render to a full [`MorphAnalysis`]
/// on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawAnalysis {
    pub entry: u32,
    ig_count: u8,
    igs: [InflectionGroup; MAX_IGS],
}

impl RawAnalysis {
    pub fn igs(&self) -> &[InflectionGroup] {
        &self.igs[..self.ig_count as usize]
    }

    pub fn final_ig(&self) -> InflectionGroup {
        self.igs[self.ig_count as usize - 1]
    }
}

/// Read-only analyzer over a shared lexicon. Cheap to construct, `Send +
/// Sync`, and safe to use from many threads at once — batch workers should
/// each hold one and reuse a scratch `Vec` via [`analyze_raw_into`].
///
/// [`analyze_raw_into`]: Analyzer::analyze_raw_into
#[derive(Debug, Clone, Copy)]
pub struct Analyzer<'a> {
    lexicon: &'a Lexicon,
}

/// One stem candidate under evaluation, with the context the first suffix
/// would see under the entry's vowel-stem alternant (used to decide which
/// alternant generation would commit to).
struct Candidate<'a> {
    entry: u32,
    entry_ref: &'a LexiconEntry,
    alternant: StemAlternant,
    vowel_context: Option<PhonoContext>,
}

/// Constraint left behind by a derived-stem final `k`: the canonical `k`
/// must not be followed by a vowel-initial suffix, the voiced `ğ` variant
/// must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KFixup {
    None,
    NeedsVowel,
    NeedsConsonant,
    /// A vowel was elided in anticipation of `+Hyor` (`başlıyor`,
    /// `yapmıyor`); only the progressive may follow.
    ProgressiveOnly,
}

/// Immutable per-branch walk state; `Copy` so branching never touches the
/// heap.
#[derive(Clone, Copy)]
struct WalkState<'s> {
    context: PhonoContext,
    remaining: &'s str,
    committed: bool,
    k_fixup: KFixup,
}

/// Fixed-capacity chain of closed inflectional groups; `Copy` by-value
/// pushes keep branching allocation-free.
#[derive(Clone, Copy)]
struct IgChain {
    igs: [InflectionGroup; MAX_IGS],
    len: usize,
}

impl IgChain {
    fn new() -> Self {
        Self {
            igs: [InflectionGroup::noun(None, Number::Singular, Possessive::None, Case::Nominative);
                MAX_IGS],
            len: 0,
        }
    }

    fn push(mut self, ig: InflectionGroup) -> Option<Self> {
        if self.len == MAX_IGS {
            return None;
        }
        self.igs[self.len] = ig;
        self.len += 1;
        Some(self)
    }
}

const ALL_POSSESSIVES: [Possessive; 7] = [
    Possessive::None,
    Possessive::FirstSingular,
    Possessive::SecondSingular,
    Possessive::ThirdSingular,
    Possessive::FirstPlural,
    Possessive::SecondPlural,
    Possessive::ThirdPlural,
];

const ALL_CASES: [Case; 7] = [
    Case::Nominative,
    Case::Accusative,
    Case::Dative,
    Case::Locative,
    Case::Ablative,
    Case::Genitive,
    Case::Instrumental,
];

const OVERT_CASES: [Case; 6] = [
    Case::Accusative,
    Case::Dative,
    Case::Locative,
    Case::Ablative,
    Case::Genitive,
    Case::Instrumental,
];

/// Derivations that attach to a bare noun stem.
const NOUN_DERIVATIONS: [Derivation; 5] = [
    Derivation::Agent,
    Derivation::Ness,
    Derivation::Diminutive,
    Derivation::Without,
    Derivation::With,
];

/// Present-type copular persons with an overt ending.
const OVERT_PRESENT_PERSONS: [Agreement; 4] = [
    Agreement::A1sg,
    Agreement::A2sg,
    Agreement::A1pl,
    Agreement::A2pl,
];

const ALL_PERSONS: [Agreement; 6] = [
    Agreement::A1sg,
    Agreement::A2sg,
    Agreement::A3sg,
    Agreement::A1pl,
    Agreement::A2pl,
    Agreement::A3pl,
];

const OVERT_COPULA_TENSES: [CopulaTense; 3] = [
    CopulaTense::Past,
    CopulaTense::Narrative,
    CopulaTense::Conditional,
];

impl<'a> Analyzer<'a> {
    pub fn new(lexicon: &'a Lexicon) -> Self {
        Self { lexicon }
    }

    /// Analyze a single surface form, returning rendered analyses.
    pub fn analyze(&self, surface: &str) -> Vec<MorphAnalysis> {
        let mut raw = Vec::new();
        self.analyze_raw_into(surface, &mut raw);

        let mut analyses: Vec<MorphAnalysis> = Vec::with_capacity(raw.len());
        for candidate in &raw {
            let rendered = self.render(surface, *candidate);
            let duplicate = analyses
                .iter()
                .any(|existing| existing.lemma == rendered.lemma && existing.igs == rendered.igs);
            if !duplicate {
                analyses.push(rendered);
            }
        }

        analyses
    }

    /// Append raw analyses for `surface` to `out` without heap allocation in
    /// the matching path. Homograph lexicon entries may yield analyses that
    /// render identically; [`analyze`](Analyzer::analyze) deduplicates.
    pub fn analyze_raw_into(&self, surface: &str, out: &mut Vec<RawAnalysis>) {
        self.lexicon
            .for_each_stem_prefix(surface, |entry, alternant, stem_len| {
                let entry_ref = self.lexicon.entry(entry);
                let candidate = Candidate {
                    entry,
                    entry_ref,
                    alternant,
                    vowel_context: vowel_alternant_context(entry_ref),
                };

                let stem = &surface[..stem_len];
                let state = WalkState {
                    context: PhonoContext {
                        last_char: stem.chars().next_back().unwrap_or('e'),
                        // A dropped verb stem harmonizes by its last
                        // remaining vowel (`oynuyor`), falling back to the
                        // elided one (`yiyor`).
                        last_vowel: if alternant == StemAlternant::Dropped {
                            crate::rules::last_vowel(stem).or_else(|| harmony_seed(entry_ref))
                        } else {
                            harmony_seed(entry_ref)
                        },
                    },
                    remaining: &surface[stem_len..],
                    committed: false,
                    k_fixup: KFixup::None,
                };

                match entry_ref.pos {
                    RootPos::Noun if entry_ref.compound_p3sg => {
                        walk_inflection(
                            &candidate,
                            state,
                            IgChain::new(),
                            None,
                            InflectionFilter::PossessiveRequired,
                            &[Number::Singular, Number::Plural],
                            out,
                        );
                    }
                    RootPos::Noun => {
                        walk_noun_stem(&candidate, state, IgChain::new(), None, out);
                    }
                    RootPos::Adj => {
                        walk_adjective_ig(
                            &candidate,
                            state,
                            IgChain::new(),
                            InflectionGroup::adj_root(),
                            out,
                        );
                    }
                    RootPos::Verb => {
                        let mut state = state;
                        if alternant == StemAlternant::Dropped {
                            state.k_fixup = KFixup::ProgressiveOnly;
                        }
                        let stem = VerbStemState {
                            derivation: None,
                            root_bare: true,
                            vowel_count: entry_ref
                                .bare_stem
                                .chars()
                                .filter(|ch| is_vowel(*ch))
                                .count(),
                            causative_count: 0,
                            passive_done: false,
                        };
                        walk_verb_stem(&candidate, state, IgChain::new(), stem, out);
                    }
                }
            });
    }

    /// Render a raw analysis into the full string-bearing form.
    pub fn render(&self, surface: &str, raw: RawAnalysis) -> MorphAnalysis {
        let entry = self.lexicon.entry(raw.entry);
        let igs = raw.igs();
        let final_ig = raw.final_ig();
        let path = NounPath::new(final_ig.number, final_ig.possessive, final_ig.case);
        let lexical = build_lexical_noun_form(entry, &path);
        let morphemes = if final_ig.pos == IgPos::Noun {
            path.morphemes
        } else {
            Vec::new()
        };

        MorphAnalysis {
            surface: surface.to_string(),
            lemma: entry.lemma.clone(),
            lexical_form: gloss_from_igs(&entry.lemma, igs),
            lexical,
            pos: match final_ig.pos {
                IgPos::Noun => PartOfSpeech::Noun,
                IgPos::Adj => PartOfSpeech::Adj,
                IgPos::Verb => PartOfSpeech::Verb,
                IgPos::Adv => PartOfSpeech::Adv,
                IgPos::Ques => PartOfSpeech::Ques,
                IgPos::Func => PartOfSpeech::Pron,
            },
            number: final_ig.number,
            possessive: final_ig.possessive,
            case: final_ig.case,
            morphemes,
            igs: igs.to_vec(),
            source: AnalysisSource::Lexicon,
        }
    }
}

/// Analyze `surface` against `lexicon` with a throwaway analyzer.
pub fn analyze_word(lexicon: &Lexicon, surface: &str) -> Vec<MorphAnalysis> {
    Analyzer::new(lexicon).analyze(surface)
}

/// A noun stem (root or derived): try its inflection readings, then further
/// derivation off the bare stem.
fn walk_noun_stem(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    derivation: Option<Derivation>,
    out: &mut Vec<RawAnalysis>,
) {
    walk_inflection(
        candidate,
        state,
        chain,
        derivation,
        InflectionFilter::All,
        &[Number::Singular, Number::Plural],
        out,
    );

    // Derivations always consume input and start with a gated character.
    if state.remaining.is_empty() || !may_start_noun_derivation(state.remaining.as_bytes()[0]) {
        return;
    }
    let Some(closed) = chain.push(InflectionGroup::noun(
        derivation,
        Number::Singular,
        Possessive::None,
        Case::Nominative,
    )) else {
        return;
    };

    for next_derivation in NOUN_DERIVATIONS {
        for next_state in derivation_steps(candidate, next_derivation, state) {
            match next_derivation {
                Derivation::Without | Derivation::With => {
                    walk_adjective(candidate, next_state, closed, next_derivation, out);
                }
                _ => walk_noun_stem(candidate, next_state, closed, Some(next_derivation), out),
            }
        }
    }

    try_denominal_verbs(candidate, state, closed, out);
}

/// Denominal verb derivations `+lAş` (`çocuklaş`) and `+lAn` (`evlen`)
/// from a bare noun or adjective stem; the derived stem enters the full
/// verbal machinery. The `+lA` member is not derived productively —
/// lexicalized `+lA` verbs come from the dictionary — because its bare
/// aorist collides with every plural (`evler`).
fn try_denominal_verbs(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    out: &mut Vec<RawAnalysis>,
) {
    for derivation in [Derivation::Become, Derivation::Acquire] {
        if let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_derivation(derivation, ctx, buf)
        }) {
            let consumed = &state.remaining[..state.remaining.len() - next.remaining.len()];
            walk_verb_stem(
                candidate,
                next,
                chain,
                VerbStemState {
                    derivation: Some(derivation),
                    root_bare: false,
                    vowel_count: vowels_in(consumed) + 2,
                    causative_count: 0,
                    passive_done: false,
                },
                out,
            );
        }
    }
}

/// An adjective stem (`+sHz`, `+lH`): a complete word on its own, further
/// nominalizable with `+lHk` (`evsizlik`), or zero-nominalized with overt
/// inflection (`evsizler`).
fn walk_adjective(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    derivation: Derivation,
    out: &mut Vec<RawAnalysis>,
) {
    walk_adjective_ig(
        candidate,
        state,
        chain,
        InflectionGroup::adj(derivation),
        out,
    );
}

/// An adjectival group (derived or a root adjective): a complete word, a
/// copular predicate, nominalizable with `+lHk`, or zero-nominalized with
/// overt inflection.
fn walk_adjective_ig(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    adjective_ig: InflectionGroup,
    out: &mut Vec<RawAnalysis>,
) {
    emit(candidate, state, chain, adjective_ig, out);
    try_copula(candidate, state, chain, adjective_ig, out);

    // All continuations consume input.
    if state.remaining.is_empty() {
        return;
    }
    let Some(closed) = chain.push(adjective_ig) else {
        return;
    };

    for next_state in derivation_steps(candidate, Derivation::Ness, state) {
        walk_noun_stem(candidate, next_state, closed, Some(Derivation::Ness), out);
    }

    try_denominal_verbs(candidate, state, closed, out);

    walk_inflection(
        candidate,
        state,
        closed,
        Some(Derivation::Zero),
        InflectionFilter::Overt,
        &[Number::Singular, Number::Plural],
        out,
    );
}

/// The nominal inflection chain (number -> possessive -> case) for the
/// current stem. Epsilon transitions consume nothing; overt suffixes must
/// match their realized form. Locative and genitive readings additionally
/// feed the relative `-ki`.
/// Which readings of an inflection walk may be emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InflectionFilter {
    All,
    /// At least one overt suffix (zero nominalizations).
    Overt,
    /// A possessive or an overt case (`+DHk` participles: `yaptığım`,
    /// `yaptıktan`, but not bare `yaptık`).
    PossessiveOrCase,
    /// A possessive is mandatory (compound nouns with a lexicalized
    /// third-person marker: `bitpazarı`, `buzdolabım`, never `bitpazar`).
    PossessiveRequired,
}

fn walk_inflection(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    derivation: Option<Derivation>,
    filter: InflectionFilter,
    numbers: &[Number],
    out: &mut Vec<RawAnalysis>,
) {
    for &number in numbers {
        let number_state = match number {
            Number::Singular => Some(state),
            Number::Plural => {
                consume_inflection(candidate, NounMorpheme::Plural, number, false, state)
            }
        };
        let Some(state) = number_state else {
            continue;
        };

        for possessive in ALL_POSSESSIVES {
            let possessive_state = match possessive_morpheme(possessive) {
                None => Some(state),
                Some(morpheme) => consume_inflection(candidate, morpheme, number, false, state),
            };
            let Some(state) = possessive_state else {
                continue;
            };
            let after_third = matches!(
                possessive,
                Possessive::ThirdSingular | Possessive::ThirdPlural
            );

            for case in ALL_CASES {
                let case_state = match case_morpheme(case) {
                    None => Some(state),
                    Some(morpheme) => {
                        consume_inflection(candidate, morpheme, number, after_third, state)
                    }
                };
                let Some(state) = case_state else {
                    continue;
                };

                let ig = InflectionGroup::noun(derivation, number, possessive, case);
                let overt = number == Number::Plural
                    || possessive != Possessive::None
                    || case != Case::Nominative;
                let admitted = match filter {
                    InflectionFilter::All => true,
                    InflectionFilter::Overt => overt,
                    InflectionFilter::PossessiveOrCase => {
                        possessive != Possessive::None || case != Case::Nominative
                    }
                    InflectionFilter::PossessiveRequired => possessive != Possessive::None,
                };
                if admitted {
                    emit(candidate, state, chain, ig, out);
                    try_copula(candidate, state, chain, ig, out);
                }
                if matches!(case, Case::Locative | Case::Genitive) {
                    try_relative(candidate, state, chain, ig, out);
                }
            }
        }
    }
}

/// Attach copular (nominal-verb) readings to a completed nominal or
/// adjectival group: overt present persons (`evdeyim`), the `+DHr` marker
/// (`evdedir`, `evdedirler`), the overt tenses with their person series
/// (`evdeydik`, `evdeymişsiniz`, `evdeyseler`), and the adverbials
/// (`evdeyken`, `evdeymişçesine`).
///
/// The zero-marked third singular present is never emitted (it would double
/// every nominal), and the bare `+lAr` third plural attaches only where a
/// plural-noun reading is impossible (non-nominative stems: `evdeler`).
fn try_copula(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    nominal_ig: InflectionGroup,
    out: &mut Vec<RawAnalysis>,
) {
    // Every copular emission consumes at least one character, and every
    // top-level copular suffix starts with a character in the gate set.
    if state.remaining.is_empty() || !may_start_copula(state.remaining.as_bytes()[0]) {
        return;
    }
    let Some(closed) = chain.push(nominal_ig) else {
        return;
    };

    for person in OVERT_PRESENT_PERSONS {
        if let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_copula_person(person, false, ctx, buf)
        }) {
            emit(
                candidate,
                next,
                closed,
                InflectionGroup::copula(Some(CopulaTense::Present), Some(person), false),
                out,
            );
        }
    }

    if nominal_ig.pos == IgPos::Noun
        && nominal_ig.case != Case::Nominative
        && let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_copula_person(Agreement::A3pl, false, ctx, buf)
        })
    {
        emit(
            candidate,
            next,
            closed,
            InflectionGroup::copula(Some(CopulaTense::Present), Some(Agreement::A3pl), false),
            out,
        );
    }

    if let Some(marked) = consume_with(candidate, state, realize_copula_marker) {
        emit(
            candidate,
            marked,
            closed,
            InflectionGroup::copula(Some(CopulaTense::Present), Some(Agreement::A3sg), true),
            out,
        );
        if let Some(next) = consume_with(candidate, marked, |ctx, buf| {
            realize_copula_person(Agreement::A3pl, false, ctx, buf)
        }) {
            emit(
                candidate,
                next,
                closed,
                InflectionGroup::copula(Some(CopulaTense::Present), Some(Agreement::A3pl), true),
                out,
            );
        }
    }

    for tense in OVERT_COPULA_TENSES {
        let Some(tensed) = consume_with(candidate, state, |ctx, buf| {
            realize_copula_tense(tense, ctx, buf)
        }) else {
            continue;
        };
        let past_type = copula_person_is_past_type(tense);

        for person in ALL_PERSONS {
            let person_state = if person == Agreement::A3sg {
                Some(tensed)
            } else {
                consume_with(candidate, tensed, |ctx, buf| {
                    realize_copula_person(person, past_type, ctx, buf)
                })
            };
            if let Some(next) = person_state {
                emit(
                    candidate,
                    next,
                    closed,
                    InflectionGroup::copula(Some(tense), Some(person), false),
                    out,
                );
            }
        }

        if tense == CopulaTense::Narrative {
            if let Some(marked) = consume_with(candidate, tensed, realize_copula_marker) {
                emit(
                    candidate,
                    marked,
                    closed,
                    InflectionGroup::copula(
                        Some(CopulaTense::Narrative),
                        Some(Agreement::A3sg),
                        true,
                    ),
                    out,
                );
            }
            try_copula_adverbs(candidate, tensed, closed, Some(CopulaTense::Narrative), out);
        }
    }

    try_copula_adverbs(candidate, state, closed, None, out);
}

/// Adverbial exits of a copular stem: `+ykAn` (`evdeyken`, `evdeymişken`)
/// and, after the narrative, `+çAsHnA` (`evdeymişçesine`).
fn try_copula_adverbs(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    tense: Option<CopulaTense>,
    out: &mut Vec<RawAnalysis>,
) {
    if state.remaining.is_empty() {
        return;
    }
    let Some(with_verb) = chain.push(InflectionGroup::copula(tense, None, false)) else {
        return;
    };

    if let Some(next) = consume_with(candidate, state, |ctx, buf| {
        realize_derivation(Derivation::While, ctx, buf)
    }) {
        emit(
            candidate,
            next,
            with_verb,
            InflectionGroup::adverb(Derivation::While),
            out,
        );
    }

    if tense == Some(CopulaTense::Narrative)
        && let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_derivation(Derivation::AsIf, ctx, buf)
        })
    {
        emit(
            candidate,
            next,
            with_verb,
            InflectionGroup::adverb(Derivation::AsIf),
            out,
        );
    }
}

/// Conservative first-byte gate for top-level copular suffixes: `y`/`s`/`l`
/// (persons), `d`/`t` (past, `+DHr`), `m` (narrative), `k` (`+ykAn`), plus
/// harmony vowels for the consonant-stem persons (`i`, `u` in ASCII; `ı`,
/// `ü` behind the 0xC4/0xC3 UTF-8 lead bytes). Over-approximation is safe;
/// under-approximation is caught by `first_byte_gates_cover_realizations`.
fn may_start_copula(first: u8) -> bool {
    matches!(
        first,
        b'y' | b's' | b'l' | b'd' | b't' | b'm' | b'k' | b'i' | b'u' | 0xC3 | 0xC4
    )
}

/// Conservative first-byte gate for noun-stem derivations: `l` (`+lHk`,
/// `+lH`), `s` (`+sHz`), `c` (`+cH`, `+cHk`), and `ç` behind its 0xC3 lead.
fn may_start_noun_derivation(first: u8) -> bool {
    matches!(first, b'l' | b's' | b'c' | 0xC3)
}

/// Consume one suffix realized by `realize`, with the same alternant-commit
/// probe as inflection. Returns `None` for zero realizations.
fn consume_with<'s>(
    candidate: &Candidate<'_>,
    state: WalkState<'s>,
    realize: impl Fn(PhonoContext, &mut SuffixBuf),
) -> Option<WalkState<'s>> {
    let mut buf = SuffixBuf::new();
    realize(state.context, &mut buf);
    if buf.as_str().is_empty() {
        return None;
    }

    let probe_starts_vowel = if !state.committed
        && let Some(vowel_context) = candidate.vowel_context
    {
        let mut probe = SuffixBuf::new();
        realize(vowel_context, &mut probe);
        starts_with_vowel(probe.as_str())
    } else {
        false
    };

    consume_realized(candidate, buf.as_str(), probe_starts_vowel, state)
}

/// Attach the relative `-ki` to a closed locative/genitive group.
fn try_relative(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    case_ig: InflectionGroup,
    out: &mut Vec<RawAnalysis>,
) {
    if state.remaining.is_empty() {
        return;
    }
    let Some(closed) = chain.push(case_ig) else {
        return;
    };
    let mut buf = SuffixBuf::new();
    realize_derivation(Derivation::Relative, state.context, &mut buf);
    let Some(next) = consume_realized(candidate, buf.as_str(), false, state) else {
        return;
    };
    walk_relative(candidate, next, closed, out);
}

/// After a relative `-ki`: a complete adjective, or zero-nominalized either
/// with a pronominal-`n` case directly (`evdekine`, `evdekinde`) or with a
/// plural and the standard chain (`evdekiler`, `evdekilerimizin`). Genitive
/// endings loop back into `-ki` (`masadakilerinki`).
fn walk_relative(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    out: &mut Vec<RawAnalysis>,
) {
    let adjective_ig = InflectionGroup::adj(Derivation::Relative);
    emit(candidate, state, chain, adjective_ig, out);
    try_copula(candidate, state, chain, adjective_ig, out);

    let Some(closed) = chain.push(adjective_ig) else {
        return;
    };

    for case in OVERT_CASES {
        let morpheme = case_morpheme(case).expect("overt case has a morpheme");
        let Some(next) = consume_inflection(candidate, morpheme, Number::Singular, true, state)
        else {
            continue;
        };
        let ig = InflectionGroup::noun(
            Some(Derivation::Zero),
            Number::Singular,
            Possessive::None,
            case,
        );
        emit(candidate, next, closed, ig, out);
        try_copula(candidate, next, closed, ig, out);
        if matches!(case, Case::Locative | Case::Genitive) {
            try_relative(candidate, next, closed, ig, out);
        }
    }

    walk_inflection(
        candidate,
        state,
        closed,
        Some(Derivation::Zero),
        InflectionFilter::Overt,
        &[Number::Plural],
        out,
    );
}

/// Emit a complete analysis if the input is fully consumed and all pending
/// constraints hold.
fn emit(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    final_ig: InflectionGroup,
    out: &mut Vec<RawAnalysis>,
) {
    if !state.remaining.is_empty()
        || state.k_fixup == KFixup::NeedsVowel
        || state.k_fixup == KFixup::ProgressiveOnly
    {
        return;
    }
    if !(state.committed || candidate.alternant == StemAlternant::Bare) {
        return;
    }
    let Some(done) = chain.push(final_ig) else {
        return;
    };
    out.push(RawAnalysis {
        entry: candidate.entry,
        ig_count: done.len as u8,
        igs: done.igs,
    });
}

/// Consume one realized inflection morpheme from the remaining input.
fn consume_inflection<'s>(
    candidate: &Candidate<'_>,
    morpheme: NounMorpheme,
    number: Number,
    after_third_possessive: bool,
    state: WalkState<'s>,
) -> Option<WalkState<'s>> {
    let mut buf = SuffixBuf::new();
    realize_noun_morpheme(
        morpheme,
        state.context,
        number,
        after_third_possessive,
        &mut buf,
    );

    let probe_starts_vowel = if !state.committed
        && let Some(vowel_context) = candidate.vowel_context
    {
        let mut probe = SuffixBuf::new();
        realize_noun_morpheme(
            morpheme,
            vowel_context,
            number,
            after_third_possessive,
            &mut probe,
        );
        starts_with_vowel(probe.as_str())
    } else {
        false
    };

    consume_realized(candidate, buf.as_str(), probe_starts_vowel, state)
}

/// Consume a derivational suffix, yielding the canonical form and, for
/// suffixes with a voicing final `k`, the `ğ` variant with its constraint on
/// the following suffix.
fn derivation_steps<'s>(
    candidate: &Candidate<'_>,
    derivation: Derivation,
    state: WalkState<'s>,
) -> impl Iterator<Item = WalkState<'s>> {
    let mut buf = SuffixBuf::new();
    realize_derivation(derivation, state.context, &mut buf);

    let canonical = consume_realized(candidate, buf.as_str(), false, state).map(|mut next| {
        if derivation_has_voicing_k(derivation) {
            next.k_fixup = KFixup::NeedsConsonant;
        }
        next
    });

    let voiced = if derivation_has_voicing_k(derivation) {
        buf.replace_final_k_with_soft_g();
        consume_realized(candidate, buf.as_str(), false, state).map(|mut next| {
            next.k_fixup = KFixup::NeedsVowel;
            next
        })
    } else {
        None
    };

    [canonical, voiced].into_iter().flatten()
}

/// Match a realized suffix against the remaining input, enforcing the
/// derived-`k` constraint and — on the first overt suffix of an entry with a
/// vowel-stem alternant — the alternant generation would commit to (realize
/// under the vowel-stem context: vowel-initial suffix -> vowel stem).
fn consume_realized<'s>(
    candidate: &Candidate<'_>,
    realized: &str,
    probe_starts_vowel: bool,
    state: WalkState<'s>,
) -> Option<WalkState<'s>> {
    debug_assert!(!realized.is_empty());

    match state.k_fixup {
        KFixup::NeedsVowel if !starts_with_vowel(realized) => return None,
        KFixup::NeedsConsonant if starts_with_vowel(realized) => return None,
        // Cleared explicitly by the progressive consume.
        KFixup::ProgressiveOnly => return None,
        _ => {}
    }

    if !state.committed && candidate.vowel_context.is_some() {
        let expects_vowel_stem = if candidate.alternant == StemAlternant::Vowel {
            starts_with_vowel(realized)
        } else {
            probe_starts_vowel
        };
        let is_vowel_stem = candidate.alternant == StemAlternant::Vowel;
        if expects_vowel_stem != is_vowel_stem {
            return None;
        }
    }

    let rest = state.remaining.strip_prefix(realized)?;
    let mut context = state.context;
    context.extend(realized);
    Some(WalkState {
        context,
        remaining: rest,
        committed: true,
        k_fixup: KFixup::None,
    })
}

fn possessive_morpheme(possessive: Possessive) -> Option<NounMorpheme> {
    match possessive {
        Possessive::None => None,
        Possessive::FirstSingular => Some(NounMorpheme::FirstSingularPossessive),
        Possessive::SecondSingular => Some(NounMorpheme::SecondSingularPossessive),
        Possessive::ThirdSingular => Some(NounMorpheme::ThirdSingularPossessive),
        Possessive::FirstPlural => Some(NounMorpheme::FirstPluralPossessive),
        Possessive::SecondPlural => Some(NounMorpheme::SecondPluralPossessive),
        Possessive::ThirdPlural => Some(NounMorpheme::ThirdPluralPossessive),
    }
}

fn case_morpheme(case: Case) -> Option<NounMorpheme> {
    match case {
        Case::Nominative => None,
        Case::Accusative => Some(NounMorpheme::Accusative),
        Case::Dative => Some(NounMorpheme::Dative),
        Case::Locative => Some(NounMorpheme::Locative),
        Case::Ablative => Some(NounMorpheme::Ablative),
        Case::Genitive => Some(NounMorpheme::Genitive),
        Case::Instrumental => Some(NounMorpheme::Instrumental),
    }
}

/// State of a verb stem during the verbal walk.
#[derive(Clone, Copy)]
struct VerbStemState {
    derivation: Option<Derivation>,
    root_bare: bool,
    vowel_count: usize,
    causative_count: u8,
    passive_done: bool,
}

const NON_A3SG_PERSONS: [Agreement; 5] = [
    Agreement::A1sg,
    Agreement::A2sg,
    Agreement::A1pl,
    Agreement::A2pl,
    Agreement::A3pl,
];

/// Z-group finite tenses (present-type person series).
const Z_TENSES: [VerbTense; 5] = [
    VerbTense::Progressive,
    VerbTense::Progressive2,
    VerbTense::Future,
    VerbTense::Narrative,
    VerbTense::Necessitative,
];

fn vowels_in(text: &str) -> usize {
    text.chars().filter(|ch| is_vowel(*ch)).count()
}

/// A verb stem (root or voice-extended): try its inflected readings, then
/// further voice suffixes (up to two causatives, then a passive).
fn walk_verb_stem(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    stem: VerbStemState,
    out: &mut Vec<RawAnalysis>,
) {
    walk_verb_body(candidate, state, chain, stem, out);

    if state.remaining.is_empty() {
        return;
    }
    let Some(closed) = chain.push(InflectionGroup::verb(stem.derivation)) else {
        return;
    };

    if stem.causative_count < 2 && !stem.passive_done {
        let from_root = stem.root_bare;
        let vowel_count = stem.vowel_count;
        if let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_causative(candidate.entry_ref, from_root, vowel_count, ctx, buf)
        }) {
            let consumed = &state.remaining[..state.remaining.len() - next.remaining.len()];
            walk_verb_stem(
                candidate,
                next,
                closed,
                VerbStemState {
                    derivation: Some(Derivation::Causative),
                    root_bare: false,
                    vowel_count: stem.vowel_count + vowels_in(consumed),
                    causative_count: stem.causative_count + 1,
                    passive_done: false,
                },
                out,
            );
        }
    }

    if !stem.passive_done
        && let Some(next) = consume_with(candidate, state, realize_passive)
    {
        let consumed = &state.remaining[..state.remaining.len() - next.remaining.len()];
        walk_verb_stem(
            candidate,
            next,
            closed,
            VerbStemState {
                derivation: Some(Derivation::Passive),
                root_bare: false,
                vowel_count: stem.vowel_count + vowels_in(consumed),
                causative_count: stem.causative_count,
                passive_done: true,
            },
            out,
        );
    }
}

/// Polarity and ability options of a verb stem: positive, `+mA`, `+yAbil`,
/// `+yAmA` — the negation morphemes may elide their final vowel before the
/// progressive (`yapmıyor`, `yapamıyor`).
fn walk_verb_body(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    stem: VerbStemState,
    out: &mut Vec<RawAnalysis>,
) {
    walk_verb_exits(
        candidate,
        state,
        chain,
        stem,
        false,
        Polarity::Positive,
        out,
    );

    if state.remaining.is_empty() {
        return;
    }

    for next in elidable_steps(candidate, state, crate::generator::realize_negation) {
        walk_verb_exits(candidate, next, chain, stem, false, Polarity::Negative, out);
    }
    if let Some(next) = consume_with(candidate, state, crate::generator::realize_able) {
        walk_verb_exits(candidate, next, chain, stem, true, Polarity::Positive, out);
    }
    for next in elidable_steps(candidate, state, crate::generator::realize_unable) {
        walk_verb_exits(candidate, next, chain, stem, true, Polarity::Negative, out);
    }
}

/// Consume a suffix in its canonical form and, when it ends in a vowel,
/// with that vowel elided in anticipation of `+Hyor`. The elided branch
/// keeps the vowel as harmony context (`yapmıyor` harmonizes by the elided
/// `a`).
fn elidable_steps<'s>(
    candidate: &Candidate<'_>,
    state: WalkState<'s>,
    realize: impl Fn(PhonoContext, &mut SuffixBuf) + Copy,
) -> impl Iterator<Item = WalkState<'s>> {
    let canonical = consume_with(candidate, state, realize);

    let elided = {
        let mut buf = SuffixBuf::new();
        realize(state.context, &mut buf);
        let realized = buf.as_str();
        match realized.chars().next_back().filter(|ch| is_vowel(*ch)) {
            Some(vowel) if realized.len() > vowel.len_utf8() => {
                let truncated = &realized[..realized.len() - vowel.len_utf8()];
                let probe_starts_vowel = if !state.committed
                    && let Some(vowel_context) = candidate.vowel_context
                {
                    let mut probe = SuffixBuf::new();
                    realize(vowel_context, &mut probe);
                    starts_with_vowel(probe.as_str())
                } else {
                    false
                };
                consume_realized(candidate, truncated, probe_starts_vowel, state).map(|mut next| {
                    // Harmony follows the last remaining vowel
                    // (`okumuyor`); the elided vowel is only a fallback.
                    if next.context.last_vowel.is_none() {
                        next.context.last_vowel = Some(vowel);
                    }
                    next.k_fixup = KFixup::ProgressiveOnly;
                    next
                })
            }
            _ => None,
        }
    };

    canonical.into_iter().chain(elided)
}

/// All exits of a polarity-resolved verb stem: imperative, optative, the
/// finite tense groups with second tenses and adverbials, and the
/// non-finite nominalizations, participles, and converbs.
#[allow(clippy::too_many_arguments)]
fn walk_verb_exits(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    stem: VerbStemState,
    able: bool,
    polarity: Polarity,
    out: &mut Vec<RawAnalysis>,
) {
    let root_bare = stem.root_bare && polarity == Polarity::Positive && !able;
    let template = InflectionGroup::verb(stem.derivation);
    let negative = polarity == Polarity::Negative;

    // Imperative second singular is the bare stem (`yap`, `yapma`).
    emit(
        candidate,
        state,
        chain,
        template.with_verb_inflection(
            polarity,
            able,
            Some(VerbTense::Imperative),
            None,
            Some(Agreement::A2sg),
        ),
        out,
    );

    if state.remaining.is_empty() {
        return;
    }

    for person in [Agreement::A2pl, Agreement::A3sg, Agreement::A3pl] {
        if let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_imperative_person(person, ctx, buf)
        }) {
            emit(
                candidate,
                next,
                chain,
                template.with_verb_inflection(
                    polarity,
                    able,
                    Some(VerbTense::Imperative),
                    None,
                    Some(person),
                ),
                out,
            );
        }
    }

    if let Some(optative) = consume_with(candidate, state, |ctx, buf| {
        realize_verb_tense(VerbTense::Optative, false, ctx, buf)
    }) {
        emit(
            candidate,
            optative,
            chain,
            template.with_verb_inflection(
                polarity,
                able,
                Some(VerbTense::Optative),
                None,
                Some(Agreement::A3sg),
            ),
            out,
        );
        for person in NON_A3SG_PERSONS {
            if let Some(next) = consume_with(candidate, optative, |ctx, buf| {
                crate::generator::realize_optative_person(person, ctx, buf)
            }) {
                emit(
                    candidate,
                    next,
                    chain,
                    template.with_verb_inflection(
                        polarity,
                        able,
                        Some(VerbTense::Optative),
                        None,
                        Some(person),
                    ),
                    out,
                );
            }
        }
    }

    for tense in Z_TENSES {
        for tensed in verb_tense_steps(candidate, tense, state) {
            finish_finite_tense(
                candidate, tensed, chain, template, polarity, able, tense, out,
            );
        }
    }

    if !negative {
        let aorist_hr = aorist_is_hr(candidate.entry_ref, root_bare);
        if let Some(tensed) = consume_with(candidate, state, |ctx, buf| {
            realize_verb_tense(VerbTense::Aorist, aorist_hr, ctx, buf)
        }) {
            finish_finite_tense(
                candidate,
                tensed,
                chain,
                template,
                polarity,
                able,
                VerbTense::Aorist,
                out,
            );
        }
    } else {
        for person in ALL_PERSONS {
            if let Some(next) = consume_with(candidate, state, |ctx, buf| {
                realize_neg_aorist_person(person, ctx, buf)
            }) {
                emit(
                    candidate,
                    next,
                    chain,
                    template.with_verb_inflection(
                        polarity,
                        able,
                        Some(VerbTense::Aorist),
                        None,
                        Some(person),
                    ),
                    out,
                );
            }
        }
        if let Some(zed) = consume_with(candidate, state, |_, buf| realize_neg_aorist_z(buf)) {
            finish_second_tenses(
                candidate,
                zed,
                chain,
                template,
                polarity,
                able,
                VerbTense::Aorist,
                out,
            );
            finish_a3pl_before_second(
                candidate,
                zed,
                chain,
                template,
                polarity,
                able,
                VerbTense::Aorist,
                out,
            );
        }
    }

    for tense in [VerbTense::Past, VerbTense::Conditional] {
        let Some(tensed) = consume_with(candidate, state, |ctx, buf| {
            realize_verb_tense(tense, false, ctx, buf)
        }) else {
            continue;
        };
        emit(
            candidate,
            tensed,
            chain,
            template.with_verb_inflection(polarity, able, Some(tense), None, Some(Agreement::A3sg)),
            out,
        );
        for person in NON_A3SG_PERSONS {
            if let Some(next) = consume_with(candidate, tensed, |ctx, buf| {
                realize_copula_person(person, true, ctx, buf)
            }) {
                emit(
                    candidate,
                    next,
                    chain,
                    template.with_verb_inflection(polarity, able, Some(tense), None, Some(person)),
                    out,
                );
            }
        }
        if tense == VerbTense::Conditional {
            finish_second_tenses(
                candidate, tensed, chain, template, polarity, able, tense, out,
            );
            finish_a3pl_before_second(
                candidate, tensed, chain, template, polarity, able, tense, out,
            );
        }
    }

    // Non-finite exits share a polarity-carrier verbal group.
    let carrier = template.with_verb_inflection(polarity, able, None, None, None);
    let Some(closed) = chain.push(carrier) else {
        return;
    };

    for derivation in [Derivation::AfterDoingSo, Derivation::ByDoingSo] {
        if let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_derivation(derivation, ctx, buf)
        }) {
            emit(
                candidate,
                next,
                closed,
                InflectionGroup::adverb(derivation),
                out,
            );
        }
    }
    if polarity == Polarity::Positive
        && !able
        && let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_derivation(Derivation::WithoutDoingSo, ctx, buf)
        })
    {
        emit(
            candidate,
            next,
            closed,
            InflectionGroup::adverb(Derivation::WithoutDoingSo),
            out,
        );
    }

    if let Some(next) = consume_with(candidate, state, |ctx, buf| {
        realize_derivation(Derivation::Infinitive1, ctx, buf)
    }) {
        let ig = InflectionGroup::noun(
            Some(Derivation::Infinitive1),
            Number::Singular,
            Possessive::None,
            Case::Nominative,
        );
        emit(candidate, next, closed, ig, out);
        try_copula(candidate, next, closed, ig, out);
        for case in [Case::Locative, Case::Ablative] {
            let morpheme = case_morpheme(case).expect("overt case has a morpheme");
            if let Some(cased) =
                consume_inflection(candidate, morpheme, Number::Singular, false, next)
            {
                let ig = InflectionGroup::noun(
                    Some(Derivation::Infinitive1),
                    Number::Singular,
                    Possessive::None,
                    case,
                );
                emit(candidate, cased, closed, ig, out);
                try_copula(candidate, cased, closed, ig, out);
            }
        }
    }

    for derivation in [Derivation::Infinitive2, Derivation::Infinitive3] {
        if let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_derivation(derivation, ctx, buf)
        }) {
            walk_inflection(
                candidate,
                next,
                closed,
                Some(derivation),
                InflectionFilter::All,
                &[Number::Singular, Number::Plural],
                out,
            );
        }
    }

    for derivation in [
        Derivation::PresentParticiple,
        Derivation::NarrativeParticiple,
    ] {
        if let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_derivation(derivation, ctx, buf)
        }) {
            walk_participle(candidate, next, closed, derivation, out);
        }
    }
    for next in derivation_steps(candidate, Derivation::FutureParticiple, state) {
        walk_participle(candidate, next, closed, Derivation::FutureParticiple, out);
    }
    for next in derivation_steps(candidate, Derivation::PastParticiple, state) {
        walk_inflection(
            candidate,
            next,
            closed,
            Some(Derivation::PastParticiple),
            InflectionFilter::PossessiveOrCase,
            &[Number::Singular, Number::Plural],
            out,
        );
    }
}

/// A finite z-group tense after its marker: persons, second tenses, and the
/// adverbial exits.
#[allow(clippy::too_many_arguments)]
fn finish_finite_tense(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    template: InflectionGroup,
    polarity: Polarity,
    able: bool,
    tense: VerbTense,
    out: &mut Vec<RawAnalysis>,
) {
    emit(
        candidate,
        state,
        chain,
        template.with_verb_inflection(polarity, able, Some(tense), None, Some(Agreement::A3sg)),
        out,
    );

    for person in NON_A3SG_PERSONS {
        if let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_copula_person(person, false, ctx, buf)
        }) {
            emit(
                candidate,
                next,
                chain,
                template.with_verb_inflection(polarity, able, Some(tense), None, Some(person)),
                out,
            );
        }
    }

    finish_a3pl_before_second(
        candidate, state, chain, template, polarity, able, tense, out,
    );
    finish_second_tenses(
        candidate, state, chain, template, polarity, able, tense, out,
    );

    if matches!(
        tense,
        VerbTense::Progressive
            | VerbTense::Progressive2
            | VerbTense::Future
            | VerbTense::Aorist
            | VerbTense::Narrative
    ) && let Some(with_verb) =
        chain.push(template.with_verb_inflection(polarity, able, Some(tense), None, None))
        && let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_derivation(Derivation::While, ctx, buf)
        })
    {
        emit(
            candidate,
            next,
            with_verb,
            InflectionGroup::adverb(Derivation::While),
            out,
        );
    }
    if matches!(tense, VerbTense::Aorist | VerbTense::Narrative)
        && let Some(with_verb) =
            chain.push(template.with_verb_inflection(polarity, able, Some(tense), None, None))
        && let Some(next) = consume_with(candidate, state, |ctx, buf| {
            realize_derivation(Derivation::AsIf, ctx, buf)
        })
    {
        emit(
            candidate,
            next,
            with_verb,
            InflectionGroup::adverb(Derivation::AsIf),
            out,
        );
    }
}

/// The third plural before a second tense (`yapıyorlardı`, `yapmazlardı`).
#[allow(clippy::too_many_arguments)]
fn finish_a3pl_before_second(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    template: InflectionGroup,
    polarity: Polarity,
    able: bool,
    tense: VerbTense,
    out: &mut Vec<RawAnalysis>,
) {
    let Some(plural) = consume_with(candidate, state, |ctx, buf| {
        realize_copula_person(Agreement::A3pl, false, ctx, buf)
    }) else {
        return;
    };
    // Without a second tense this reading is emitted by the caller's
    // person loop (z-tenses) or fused person handling; only the
    // pre-second-tense order lives here.
    for second in OVERT_COPULA_TENSES {
        if let Some(next) = consume_with(candidate, plural, |ctx, buf| {
            realize_copula_tense(second, ctx, buf)
        }) {
            emit(
                candidate,
                next,
                chain,
                template.with_verb_inflection(
                    polarity,
                    able,
                    Some(tense),
                    Some(second),
                    Some(Agreement::A3pl),
                ),
                out,
            );
        }
    }
}

/// Second (copular) tenses on a finite stem with their person series
/// (`yapıyordu`, `yapıyorduk`, `yapmışsa`, `yapsaydı`).
#[allow(clippy::too_many_arguments)]
fn finish_second_tenses(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    template: InflectionGroup,
    polarity: Polarity,
    able: bool,
    tense: VerbTense,
    out: &mut Vec<RawAnalysis>,
) {
    for second in OVERT_COPULA_TENSES {
        let Some(tensed) = consume_with(candidate, state, |ctx, buf| {
            realize_copula_tense(second, ctx, buf)
        }) else {
            continue;
        };
        emit(
            candidate,
            tensed,
            chain,
            template.with_verb_inflection(
                polarity,
                able,
                Some(tense),
                Some(second),
                Some(Agreement::A3sg),
            ),
            out,
        );
        let past_type = copula_person_is_past_type(second);
        for person in [
            Agreement::A1sg,
            Agreement::A2sg,
            Agreement::A1pl,
            Agreement::A2pl,
        ] {
            if let Some(next) = consume_with(candidate, tensed, |ctx, buf| {
                realize_copula_person(person, past_type, ctx, buf)
            }) {
                emit(
                    candidate,
                    next,
                    chain,
                    template.with_verb_inflection(
                        polarity,
                        able,
                        Some(tense),
                        Some(second),
                        Some(person),
                    ),
                    out,
                );
            }
        }
    }
}

/// Tense-marker consume with the special cases: the progressive clears the
/// vowel-elision constraint, and the future has canonical/voiced `k`
/// variants (`yapacak` / `yapacağım`).
fn verb_tense_steps<'s>(
    candidate: &Candidate<'_>,
    tense: VerbTense,
    state: WalkState<'s>,
) -> impl Iterator<Item = WalkState<'s>> {
    let mut base = state;
    if state.k_fixup == KFixup::ProgressiveOnly {
        if tense == VerbTense::Progressive {
            base.k_fixup = KFixup::None;
        } else {
            return None.into_iter().chain(None);
        }
    }

    let canonical = consume_with(candidate, base, |ctx, buf| {
        realize_verb_tense(tense, false, ctx, buf)
    })
    .map(|mut next| {
        if tense == VerbTense::Future {
            next.k_fixup = KFixup::NeedsConsonant;
        }
        next
    });

    let voiced = if tense == VerbTense::Future {
        let mut buf = SuffixBuf::new();
        realize_verb_tense(tense, false, base.context, &mut buf);
        buf.replace_final_k_with_soft_g();
        let probe_starts_vowel = if !base.committed
            && let Some(vowel_context) = candidate.vowel_context
        {
            let mut probe = SuffixBuf::new();
            realize_verb_tense(tense, false, vowel_context, &mut probe);
            starts_with_vowel(probe.as_str())
        } else {
            false
        };
        consume_realized(candidate, buf.as_str(), probe_starts_vowel, base).map(|mut next| {
            next.k_fixup = KFixup::NeedsVowel;
            next
        })
    } else {
        None
    };

    canonical.into_iter().chain(voiced)
}

/// Participial adjective (`yapan`, `yapacak`, `yapmış`): a complete word,
/// a copular predicate, or zero-nominalized with overt inflection
/// (`yapanlar`, `yapacağı`).
fn walk_participle(
    candidate: &Candidate<'_>,
    state: WalkState<'_>,
    chain: IgChain,
    derivation: Derivation,
    out: &mut Vec<RawAnalysis>,
) {
    let adjective_ig = InflectionGroup::adj(derivation);
    emit(candidate, state, chain, adjective_ig, out);
    try_copula(candidate, state, chain, adjective_ig, out);

    if state.remaining.is_empty() {
        return;
    }
    let Some(closed) = chain.push(adjective_ig) else {
        return;
    };
    walk_inflection(
        candidate,
        state,
        closed,
        Some(Derivation::Zero),
        InflectionFilter::Overt,
        &[Number::Singular, Number::Plural],
        out,
    );
}

/// Analyze `tail` as a nominal suffix sequence on an out-of-lexicon root
/// with phonology `context` (guesser, apostrophe suffixation). Emitted
/// analyses carry [`TAIL_ENTRY`] as their entry id.
pub(crate) const TAIL_ENTRY: u32 = u32::MAX;

/// Placeholder entry for tail analyses; its verb fields are never
/// consulted (derived stems never behave as bare roots).
static TAIL_ENTRY_REF: std::sync::LazyLock<LexiconEntry> =
    std::sync::LazyLock::new(|| LexiconEntry::new("", None, None));

pub(crate) fn analyze_nominal_tail_into(
    context: PhonoContext,
    tail: &str,
    out: &mut Vec<RawAnalysis>,
) {
    let candidate = Candidate {
        entry: TAIL_ENTRY,
        entry_ref: &TAIL_ENTRY_REF,
        alternant: StemAlternant::Bare,
        vowel_context: None,
    };
    let state = WalkState {
        context,
        remaining: tail,
        committed: true,
        k_fixup: KFixup::None,
    };
    walk_noun_stem(&candidate, state, IgChain::new(), None, out);
}

/// Analyze `tail` as a relative `-ki` continuation on a locative or
/// genitive closed form (`benimki`, `ondakiler`).
pub(crate) fn analyze_ki_tail_into(
    context: PhonoContext,
    tail: &str,
    root_ig: InflectionGroup,
    out: &mut Vec<RawAnalysis>,
) {
    let candidate = Candidate {
        entry: TAIL_ENTRY,
        entry_ref: &TAIL_ENTRY_REF,
        alternant: StemAlternant::Bare,
        vowel_context: None,
    };
    let state = WalkState {
        context,
        remaining: tail,
        committed: true,
        k_fixup: KFixup::None,
    };
    try_relative(&candidate, state, IgChain::new(), root_ig, out);
}

/// Analyze `tail` as a copular suffix sequence on `root_ig` (the question
/// particle).
pub(crate) fn analyze_copular_tail_into(
    context: PhonoContext,
    tail: &str,
    root_ig: InflectionGroup,
    out: &mut Vec<RawAnalysis>,
) {
    let candidate = Candidate {
        entry: TAIL_ENTRY,
        entry_ref: &TAIL_ENTRY_REF,
        alternant: StemAlternant::Bare,
        vowel_context: None,
    };
    let state = WalkState {
        context,
        remaining: tail,
        committed: true,
        k_fixup: KFixup::None,
    };
    try_copula(&candidate, state, IgChain::new(), root_ig, out);
}

#[cfg(test)]
mod tests {
    use crate::{
        analyzer::{Analyzer, analyze_word},
        generator::{generate_from_igs, generate_noun_surface},
        gold::{GOLD_ANALYSIS_CASES, GOLD_DERIVED_SURFACES},
        lexicon::Lexicon,
        state::{Case, NounMorpheme, Number, Possessive, noun_paths},
    };

    fn seed() -> Lexicon {
        Lexicon::seed()
    }

    #[test]
    fn analyzes_simple_plural() {
        let lexicon = seed();
        let analyses = analyze_word(&lexicon, "evler");

        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0].lemma, "ev");
        assert_eq!(analyses[0].number, Number::Plural);
        assert_eq!(analyses[0].possessive, Possessive::None);
        assert_eq!(analyses[0].case, Case::Nominative);
        assert_eq!(analyses[0].morphemes, vec![NounMorpheme::Plural]);
        assert_eq!(analyses[0].lexical_form, "ev+Noun+A3pl+Pnon+Nom");
    }

    #[test]
    fn analyzes_voiced_case_form() {
        let lexicon = seed();
        let analyses = analyze_word(&lexicon, "kitaba");

        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0].lemma, "kitap");
        assert_eq!(analyses[0].case, Case::Dative);
    }

    #[test]
    fn analyzes_pronominal_n_case_forms() {
        let lexicon = seed();

        let analyses = analyze_word(&lexicon, "masasında");
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0].lexical_form, "masa+Noun+A3sg+P3sg+Loc");
    }

    #[test]
    fn analyzes_ambiguous_leri_forms() {
        let lexicon = seed();
        let analyses = analyze_word(&lexicon, "evleri");

        assert_eq!(analyses.len(), 4);
        let glosses: Vec<&str> = analyses
            .iter()
            .map(|analysis| analysis.lexical_form.as_str())
            .collect();
        assert!(glosses.contains(&"ev+Noun+A3pl+Pnon+Acc"));
        assert!(glosses.contains(&"ev+Noun+A3pl+P3sg+Nom"));
        assert!(glosses.contains(&"ev+Noun+A3sg+P3pl+Nom"));
        assert!(glosses.contains(&"ev+Noun+A3pl+P3pl+Nom"));
    }

    #[test]
    fn rejects_ill_formed_alternant_use() {
        let lexicon = seed();

        assert!(
            analyze_word(&lexicon, "kitab").is_empty(),
            "voiced stem must not stand word-finally"
        );
        assert!(
            analyze_word(&lexicon, "kitapı").is_empty(),
            "bare stem must not take a vowel-initial suffix when a vowel stem exists"
        );
        assert!(
            analyze_word(&lexicon, "kitabta").is_empty(),
            "vowel stem must not take a consonant-initial suffix"
        );
        assert!(
            analyze_word(&lexicon, "susu").is_empty(),
            "y-stem must block the s-buffer reading"
        );
        assert!(
            analyze_word(&lexicon, "kitabçı").is_empty(),
            "derivations attach to the bare stem"
        );
        assert!(
            analyze_word(&lexicon, "kitaplığ").is_empty(),
            "voiced derived k must be followed by a vowel"
        );
        assert!(
            analyze_word(&lexicon, "kitaplıka").is_empty(),
            "canonical derived k must not be followed by a vowel"
        );
    }

    #[test]
    fn analyzes_derivations() {
        let lexicon = seed();

        let glosses = |surface: &str| -> Vec<String> {
            analyze_word(&lexicon, surface)
                .into_iter()
                .map(|analysis| analysis.lexical_form)
                .collect()
        };

        assert!(
            glosses("kitapçı")
                .contains(&"kitap+Noun+A3sg+Pnon+Nom^DB+Noun+Agt+A3sg+Pnon+Nom".to_string())
        );
        assert!(glosses("evsiz").contains(&"ev+Noun+A3sg+Pnon+Nom^DB+Adj+Without".to_string()));
        assert!(glosses("evsizlere").contains(
            &"ev+Noun+A3sg+Pnon+Nom^DB+Adj+Without^DB+Noun+Zero+A3pl+Pnon+Dat".to_string()
        ));
        assert!(
            glosses("gözlüğü")
                .contains(&"göz+Noun+A3sg+Pnon+Nom^DB+Noun+Ness+A3sg+Pnon+Acc".to_string())
        );
        assert!(
            glosses("ağaççık")
                .contains(&"ağaç+Noun+A3sg+Pnon+Nom^DB+Noun+Dim+A3sg+Pnon+Nom".to_string())
        );
    }

    #[test]
    fn analyzes_relative_ki_and_its_loop() {
        let lexicon = seed();

        let glosses = |surface: &str| -> Vec<String> {
            analyze_word(&lexicon, surface)
                .into_iter()
                .map(|analysis| analysis.lexical_form)
                .collect()
        };

        assert!(glosses("evdeki").contains(&"ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel".to_string()));
        assert!(glosses("evinki").contains(&"ev+Noun+A3sg+Pnon+Gen^DB+Adj+Rel".to_string()));
        assert!(
            glosses("evdekiler").contains(
                &"ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Nom".to_string()
            )
        );
        assert!(
            glosses("evdekine").contains(
                &"ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3sg+Pnon+Dat".to_string()
            )
        );
        assert!(glosses("masadakilerinki").contains(
            &"masa+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Gen^DB+Adj+Rel".to_string()
        ));

        assert!(
            analyze_word(&lexicon, "evdekide").is_empty(),
            "zero-nominalized ki must take pronominal-n case forms"
        );
    }

    #[test]
    fn first_byte_gates_cover_realizations() {
        use crate::generator::{
            SuffixBuf, realize_copula_marker, realize_copula_person, realize_copula_tense,
            realize_derivation,
        };
        use crate::rules::PhonoContext;
        use crate::state::{Agreement, Derivation};

        let contexts = [
            PhonoContext::for_stem("masa", None),
            PhonoContext::for_stem("ev", None),
            PhonoContext::for_stem("kitap", None),
            PhonoContext::for_stem("çocuk", None),
            PhonoContext::for_stem("göz", None),
            PhonoContext::for_stem("su", None),
            PhonoContext::for_stem("saat", Some('i')),
            PhonoContext::for_stem("alkol", Some('ü')),
        ];

        let mut buf = SuffixBuf::new();
        let check = |gate: fn(u8) -> bool, buf: &SuffixBuf, label: &str| {
            let realized = buf.as_str();
            if !realized.is_empty() {
                assert!(
                    gate(realized.as_bytes()[0]),
                    "gate must admit {label} realization {realized:?}"
                );
            }
        };

        for ctx in contexts {
            for person in super::ALL_PERSONS {
                realize_copula_person(person, false, ctx, &mut buf);
                check(super::may_start_copula, &buf, "person");
            }
            for tense in super::OVERT_COPULA_TENSES {
                realize_copula_tense(tense, ctx, &mut buf);
                check(super::may_start_copula, &buf, "tense");
            }
            realize_copula_marker(ctx, &mut buf);
            check(super::may_start_copula, &buf, "marker");
            realize_derivation(Derivation::While, ctx, &mut buf);
            check(super::may_start_copula, &buf, "while");

            for derivation in super::NOUN_DERIVATIONS {
                realize_derivation(derivation, ctx, &mut buf);
                check(super::may_start_noun_derivation, &buf, "derivation");
            }
        }

        // k-type persons only appear tense-internally, but keep them gated
        // anyway so the gate stays valid if that changes.
        for ctx in contexts {
            for person in [Agreement::A2pl, Agreement::A3pl] {
                realize_copula_person(person, true, ctx, &mut buf);
                let realized = buf.as_str();
                assert!(!realized.is_empty());
            }
        }
    }

    #[test]
    fn analyzes_copular_predicates() {
        let lexicon = seed();

        let glosses = |surface: &str| -> Vec<String> {
            analyze_word(&lexicon, surface)
                .into_iter()
                .map(|analysis| analysis.lexical_form)
                .collect()
        };

        assert!(
            glosses("evdeydi")
                .contains(&"ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Past+A3sg".to_string())
        );
        assert!(
            glosses("kitaptı")
                .contains(&"kitap+Noun+A3sg+Pnon+Nom^DB+Verb+Zero+Past+A3sg".to_string()),
            "past copula must devoice after voiceless stems"
        );
        assert!(
            glosses("evdedirler")
                .contains(&"ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Pres+A3pl+Cop".to_string())
        );
        assert!(
            glosses("evdeyken")
                .contains(&"ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero^DB+Adverb+While".to_string())
        );
        assert!(
            glosses("evdeymişçesine")
                .contains(&"ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Narr^DB+Adverb+AsIf".to_string())
        );
    }

    #[test]
    fn copular_readings_do_not_explode_bare_nominals() {
        let lexicon = seed();

        assert_eq!(
            analyze_word(&lexicon, "evler").len(),
            1,
            "bare plurals must not receive a zero-copula reading"
        );
        assert_eq!(
            analyze_word(&lexicon, "ev").len(),
            1,
            "bare nominals must not receive a third-singular copula reading"
        );
        assert_eq!(
            analyze_word(&lexicon, "evdeler").len(),
            1,
            "copular third plural after a case is unambiguous"
        );
    }

    #[test]
    fn analyzes_homograph_senses_without_duplicates() {
        let lexicon = seed();

        let shared = analyze_word(&lexicon, "şıkta");
        let locative_count = shared
            .iter()
            .filter(|analysis| analysis.lexical_form == "şık+Noun+A3sg+Pnon+Loc")
            .count();
        assert_eq!(locative_count, 1, "homograph senses must deduplicate");
    }

    #[test]
    fn returns_no_analyses_for_unknown_words() {
        let lexicon = seed();
        assert!(analyze_word(&lexicon, "zzz").is_empty());
        assert!(analyze_word(&lexicon, "").is_empty());
    }

    #[test]
    fn every_analysis_regenerates_its_surface() {
        let lexicon = seed();
        let analyzer = Analyzer::new(&lexicon);
        let paths = noun_paths();

        let mut surfaces: Vec<String> = Vec::new();
        for entry in lexicon
            .entries()
            .iter()
            .filter(|entry| entry.pos == crate::lexicon::RootPos::Noun)
        {
            for path in paths
                .iter()
                .filter(|path| !entry.compound_p3sg || path.possessive != Possessive::None)
            {
                surfaces.push(generate_noun_surface(entry, path));
            }
        }
        surfaces.extend(GOLD_DERIVED_SURFACES.iter().map(|s| s.to_string()));

        let mut raw = Vec::new();
        for surface in &surfaces {
            raw.clear();
            analyzer.analyze_raw_into(surface, &mut raw);
            assert!(
                !raw.is_empty(),
                "surface {surface} should have at least one analysis"
            );
            for analysis in &raw {
                let regenerated = generate_from_igs(lexicon.entry(analysis.entry), analysis.igs());
                assert_eq!(
                    &regenerated,
                    surface,
                    "analysis {:?} of {} regenerated as {}",
                    analysis.igs(),
                    surface,
                    regenerated
                );
            }
        }
    }

    #[test]
    fn analyzes_expected_gold_cases() {
        let lexicon = seed();

        for case in GOLD_ANALYSIS_CASES {
            let analyses = analyze_word(&lexicon, case.surface);
            assert!(
                analyses
                    .iter()
                    .any(|analysis| analysis.lexical_form == case.expected_gloss),
                "surface {} should include analysis {}; got {:?}",
                case.surface,
                case.expected_gloss,
                analyses
                    .iter()
                    .map(|analysis| analysis.lexical_form.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[cfg(test)]
mod large_tests {
    use crate::{
        Possessive,
        analyzer::{Analyzer, analyze_word},
        generator::{generate_from_igs, generate_noun_surface},
        lexicon::{Lexicon, RootPos},
        state::noun_paths,
    };
    use std::time::Instant;

    fn sweep(lexicon: &Lexicon, stride: usize) -> (usize, usize) {
        let analyzer = Analyzer::new(lexicon);
        let paths = noun_paths();
        let mut raw = Vec::new();
        let (mut checked, mut failures) = (0, 0);

        for (index, entry) in lexicon.entries().iter().enumerate() {
            if entry.pos != RootPos::Noun || index % stride != 0 {
                continue;
            }
            for path in paths
                .iter()
                .filter(|path| !entry.compound_p3sg || path.possessive != Possessive::None)
            {
                checked += 1;
                let surface = generate_noun_surface(entry, path);
                raw.clear();
                analyzer.analyze_raw_into(&surface, &mut raw);
                let ok = raw.iter().any(|analysis| {
                    analysis.entry == index as u32
                        && generate_from_igs(entry, analysis.igs()) == surface
                });
                if !ok {
                    failures += 1;
                }
            }
        }
        (checked, failures)
    }

    /// A fast slice of the full-lexicon validator runs in every gate.
    #[test]
    fn full_lexicon_sample_roundtrips() {
        let lexicon = Lexicon::full();
        let (checked, failures) = sweep(&lexicon, 100);
        assert!(checked > 10_000, "sample too small: {checked}");
        assert_eq!(failures, 0, "{failures} of {checked} forms failed");
    }

    /// The complete sweep (~1.8M forms); run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "full 1.8M-form sweep; run explicitly"]
    fn full_lexicon_roundtrips() {
        let lexicon = Lexicon::full();
        let (checked, failures) = sweep(&lexicon, 1);
        assert!(checked > 1_500_000);
        assert_eq!(failures, 0, "{failures} of {checked} forms failed");
    }

    /// Pathological inputs must terminate quickly and sanely.
    #[test]
    fn pathological_inputs_terminate() {
        let lexicon = Lexicon::seed();
        let started = Instant::now();

        let huge = "ev".repeat(50_000); // 100KB token
        let _ = analyze_word(&lexicon, &huge);

        let mut deep = String::from("ev");
        for _ in 0..30 {
            deep.push_str("dekilerinki");
        }
        let _ = analyze_word(&lexicon, &deep); // beyond MAX_IGS: pruned, not stuck

        for input in ["", " ", "🐈🐈🐈", "'", "ev'"] {
            let _ = analyze_word(&lexicon, input);
        }

        assert!(
            started.elapsed().as_secs() < 10,
            "pathological inputs took {:?}",
            started.elapsed()
        );
    }
}
