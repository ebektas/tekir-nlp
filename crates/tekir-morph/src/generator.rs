//! Surface generation: realize an analysis (root + inflectional groups) as
//! Turkish orthography.
//!
//! `realize_noun_morpheme` and `realize_derivation` are the single source of
//! truth for suffix realization; the analyzer reuses them to match suffixes
//! against input, so analysis is the exact inverse of generation by
//! construction.

use crate::{
    lexicon::{CausativeAllomorph, LexiconEntry},
    rules::{
        PhonoContext, a_harmony, affricate_consonant, i_harmony, i_harmony_after, is_vowel,
        last_vowel, starts_with_vowel, stop_consonant,
    },
    state::{
        Agreement, CopulaTense, Derivation, IgPos, InflectionGroup, NounMorpheme, NounPath, Number,
        Polarity, Possessive, VerbTense,
    },
};

/// Fixed-capacity buffer for one realized suffix. Suffix realizations are at
/// most a few characters, so the hot path never allocates.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SuffixBuf {
    bytes: [u8; 24],
    len: usize,
}

impl SuffixBuf {
    pub(crate) fn new() -> Self {
        Self {
            bytes: [0; 24],
            len: 0,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn push(&mut self, ch: char) {
        let encoded = ch.encode_utf8(&mut self.bytes[self.len..]);
        self.len += encoded.len();
    }

    /// Replace a final `k` with `ğ` (derived-stem voicing before a
    /// vowel-initial suffix, as in `kitaplık -> kitaplığa`).
    pub(crate) fn replace_final_k_with_soft_g(&mut self) {
        if self.len > 0 && self.bytes[self.len - 1] == b'k' {
            self.len -= 1;
            self.push('ğ');
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("suffix buffer holds valid UTF-8")
    }
}

/// Realize one noun inflection morpheme at the given phonological boundary.
///
/// `number` is the number already in the path (third-plural possessive
/// contracts to a single vowel after an overt plural). `after_third_possessive`
/// selects the pronominal `n` variants of the case suffixes (`evine`,
/// `evinde`, `evinin`), Oflazer's Case-2 states; the same variants follow a
/// zero-nominalized relative `-ki` (`evdekine`).
pub(crate) fn realize_noun_morpheme(
    morpheme: NounMorpheme,
    ctx: PhonoContext,
    number: Number,
    after_third_possessive: bool,
    out: &mut SuffixBuf,
) {
    out.clear();

    match morpheme {
        NounMorpheme::Plural => {
            out.push('l');
            out.push(a_harmony(ctx));
            out.push('r');
        }
        NounMorpheme::FirstSingularPossessive => {
            if !ctx.ends_with_vowel() {
                out.push(i_harmony(ctx));
            }
            out.push('m');
        }
        NounMorpheme::SecondSingularPossessive => {
            if !ctx.ends_with_vowel() {
                out.push(i_harmony(ctx));
            }
            out.push('n');
        }
        NounMorpheme::ThirdSingularPossessive => {
            if ctx.ends_with_vowel() {
                out.push('s');
            }
            out.push(i_harmony(ctx));
        }
        NounMorpheme::FirstPluralPossessive => {
            if !ctx.ends_with_vowel() {
                out.push(i_harmony(ctx));
            }
            out.push('m');
            out.push(i_harmony(ctx));
            out.push('z');
        }
        NounMorpheme::SecondPluralPossessive => {
            if !ctx.ends_with_vowel() {
                out.push(i_harmony(ctx));
            }
            out.push('n');
            out.push(i_harmony(ctx));
            out.push('z');
        }
        NounMorpheme::ThirdPluralPossessive => {
            if matches!(number, Number::Plural) {
                out.push(i_harmony(ctx));
            } else {
                let plural_vowel = a_harmony(ctx);
                out.push('l');
                out.push(plural_vowel);
                out.push('r');
                out.push(i_harmony_after(plural_vowel));
            }
        }
        NounMorpheme::Accusative => {
            if after_third_possessive {
                out.push('n');
            } else if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(i_harmony(ctx));
        }
        NounMorpheme::Dative => {
            if after_third_possessive {
                out.push('n');
            } else if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(a_harmony(ctx));
        }
        NounMorpheme::Locative => {
            if after_third_possessive {
                out.push('n');
                out.push('d');
            } else {
                out.push(stop_consonant(ctx));
            }
            out.push(a_harmony(ctx));
        }
        NounMorpheme::Ablative => {
            if after_third_possessive {
                out.push('n');
                out.push('d');
            } else {
                out.push(stop_consonant(ctx));
            }
            out.push(a_harmony(ctx));
            out.push('n');
        }
        NounMorpheme::Genitive => {
            if after_third_possessive || ctx.ends_with_vowel() {
                out.push('n');
            }
            out.push(i_harmony(ctx));
            out.push('n');
        }
        // The instrumental keeps its `y` buffer even after third-person
        // possessives (`arabasıyla`, never a pronominal `n`).
        NounMorpheme::Instrumental => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push('l');
            out.push(a_harmony(ctx));
        }
    }
}

/// Realize a derivational suffix. All derivations are consonant-initial, so
/// they always attach to the bare stem alternant. `Zero` realizes as nothing.
pub(crate) fn realize_derivation(derivation: Derivation, ctx: PhonoContext, out: &mut SuffixBuf) {
    out.clear();

    match derivation {
        Derivation::Agent => {
            out.push(affricate_consonant(ctx));
            out.push(i_harmony(ctx));
        }
        Derivation::Ness => {
            out.push('l');
            out.push(i_harmony(ctx));
            out.push('k');
        }
        Derivation::Diminutive => {
            out.push(affricate_consonant(ctx));
            out.push(i_harmony(ctx));
            out.push('k');
        }
        Derivation::Without => {
            out.push('s');
            out.push(i_harmony(ctx));
            out.push('z');
        }
        Derivation::With => {
            out.push('l');
            out.push(i_harmony(ctx));
        }
        Derivation::Relative => {
            out.push('k');
            out.push('i');
        }
        // Temporal `+ykAn` is non-harmonizing: `okuldayken`, `güzelken`.
        Derivation::While => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push('k');
            out.push('e');
            out.push('n');
        }
        // `+çAsHnA` after the narrative copula: `evdeymişçesine`.
        Derivation::AsIf => {
            let a = a_harmony(ctx);
            out.push(affricate_consonant(ctx));
            out.push(a);
            out.push('s');
            out.push(i_harmony_after(a));
            out.push('n');
            out.push(a);
        }
        Derivation::Zero => {}
        // Verbal nominalizations and converbs.
        Derivation::Infinitive1 => {
            out.push('m');
            out.push(a_harmony(ctx));
            out.push('k');
        }
        Derivation::Infinitive2 => {
            out.push('m');
            out.push(a_harmony(ctx));
        }
        Derivation::Infinitive3 => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(i_harmony(ctx));
            out.push('ş');
        }
        Derivation::PresentParticiple => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(a_harmony(ctx));
            out.push('n');
        }
        Derivation::PastParticiple => {
            out.push(stop_consonant(ctx));
            out.push(i_harmony(ctx));
            out.push('k');
        }
        Derivation::FutureParticiple => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            let a = a_harmony(ctx);
            out.push(a);
            out.push('c');
            out.push(a);
            out.push('k');
        }
        Derivation::NarrativeParticiple => {
            out.push('m');
            out.push(i_harmony(ctx));
            out.push('ş');
        }
        Derivation::AfterDoingSo => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(i_harmony(ctx));
            out.push('p');
        }
        Derivation::ByDoingSo => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            let a = a_harmony(ctx);
            out.push(a);
            out.push('r');
            out.push(a);
            out.push('k');
        }
        Derivation::WithoutDoingSo => {
            out.push('m');
            out.push(a_harmony(ctx));
            out.push('d');
            out.push(a_harmony(ctx));
            out.push('n');
        }
        Derivation::Become => {
            out.push('l');
            out.push(a_harmony(ctx));
            out.push('ş');
        }
        Derivation::Acquire => {
            out.push('l');
            out.push(a_harmony(ctx));
            out.push('n');
        }
        Derivation::Causative | Derivation::Passive => {
            unreachable!("voice suffixes are realized by realize_causative/realize_passive")
        }
    }
}

/// Realize the causative voice. `from_root` selects a lexical allomorph
/// override (`içir`, `çıkar`); otherwise the rule applies: `+t` after
/// vowels and polysyllabic `l`/`r` stems (`okut`, `oturt`, `yaptırt`),
/// `+DHr` elsewhere (`yaptır`, `koştur`, `düşündür`).
pub(crate) fn realize_causative(
    entry: &LexiconEntry,
    from_root: bool,
    stem_vowel_count: usize,
    ctx: PhonoContext,
    out: &mut SuffixBuf,
) {
    out.clear();

    if from_root {
        match entry.causative {
            CausativeAllomorph::Hr => {
                out.push(i_harmony(ctx));
                out.push('r');
                return;
            }
            CausativeAllomorph::Ht => {
                out.push(i_harmony(ctx));
                out.push('t');
                return;
            }
            CausativeAllomorph::Ar => {
                out.push(a_harmony(ctx));
                out.push('r');
                return;
            }
            CausativeAllomorph::Rule => {}
        }
    }

    if ctx.ends_with_vowel() || (matches!(ctx.last_char, 'l' | 'r') && stem_vowel_count >= 2) {
        out.push('t');
    } else {
        out.push(stop_consonant(ctx));
        out.push(i_harmony(ctx));
        out.push('r');
    }
}

/// Realize the passive voice: `+n` after vowels (`okun`), `+Hn` after `l`
/// (`alın`), `+Hl` elsewhere (`yapıl`, `görül`).
pub(crate) fn realize_passive(ctx: PhonoContext, out: &mut SuffixBuf) {
    out.clear();

    if ctx.ends_with_vowel() {
        out.push('n');
    } else if ctx.last_char == 'l' {
        out.push(i_harmony(ctx));
        out.push('n');
    } else {
        out.push(i_harmony(ctx));
        out.push('l');
    }
}

/// Realize verbal negation `+mA` (`yapma`).
pub(crate) fn realize_negation(ctx: PhonoContext, out: &mut SuffixBuf) {
    out.clear();
    out.push('m');
    out.push(a_harmony(ctx));
}

/// Realize the abilitative `+yAbil` (`yapabil`, `gelebil`).
pub(crate) fn realize_able(ctx: PhonoContext, out: &mut SuffixBuf) {
    out.clear();
    if ctx.ends_with_vowel() {
        out.push('y');
    }
    out.push(a_harmony(ctx));
    out.push('b');
    out.push('i');
    out.push('l');
}

/// Realize the impotential `+yAmA` (`yapama`, `geleme`).
pub(crate) fn realize_unable(ctx: PhonoContext, out: &mut SuffixBuf) {
    out.clear();
    if ctx.ends_with_vowel() {
        out.push('y');
    }
    out.push(a_harmony(ctx));
    out.push('m');
    out.push(a_harmony(ctx));
}

/// Whether the aorist of this stem takes `+Hr` rather than `+Ar`
/// (vowel-final stems take bare `+r`, decided by context at realization).
/// Lexical overrides win (`gelir`, `affeder`); otherwise monosyllabic
/// roots take `+Ar` and everything else `+Hr`.
pub(crate) fn aorist_is_hr(entry: &LexiconEntry, root_bare: bool) -> bool {
    if !root_bare {
        return true;
    }
    match entry.aorist {
        crate::lexicon::AoristAllomorph::Hr => true,
        crate::lexicon::AoristAllomorph::Ar => false,
        crate::lexicon::AoristAllomorph::Rule => {
            entry.bare_stem.chars().filter(|ch| is_vowel(*ch)).count() >= 2
        }
    }
}

/// Realize a positive finite tense marker. `Imperative` is zero-marked
/// (persons carry it); the negative aorist is realized by
/// [`realize_neg_aorist_person`] and [`realize_neg_aorist_z`].
pub(crate) fn realize_verb_tense(
    tense: VerbTense,
    aorist_hr: bool,
    ctx: PhonoContext,
    out: &mut SuffixBuf,
) {
    out.clear();

    match tense {
        VerbTense::Progressive => {
            out.push(i_harmony(ctx));
            out.push('y');
            out.push('o');
            out.push('r');
        }
        VerbTense::Progressive2 => {
            out.push('m');
            out.push(a_harmony(ctx));
            out.push('k');
            out.push('t');
            out.push(a_harmony(ctx));
        }
        VerbTense::Future => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            let a = a_harmony(ctx);
            out.push(a);
            out.push('c');
            out.push(a);
            out.push('k');
        }
        VerbTense::Aorist => {
            if !ctx.ends_with_vowel() {
                if aorist_hr {
                    out.push(i_harmony(ctx));
                } else {
                    out.push(a_harmony(ctx));
                }
            }
            out.push('r');
        }
        VerbTense::Past => {
            out.push(stop_consonant(ctx));
            out.push(i_harmony(ctx));
        }
        VerbTense::Narrative => {
            out.push('m');
            out.push(i_harmony(ctx));
            out.push('ş');
        }
        VerbTense::Necessitative => {
            out.push('m');
            out.push(a_harmony(ctx));
            out.push('l');
            out.push(i_harmony(ctx));
        }
        VerbTense::Optative => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(a_harmony(ctx));
        }
        VerbTense::Conditional => {
            out.push('s');
            out.push(a_harmony(ctx));
        }
        VerbTense::Imperative => {}
    }
}

/// The `+z` of the negative aorist stem (`yapmaz`), used before second
/// tenses (`yapmazdı`).
pub(crate) fn realize_neg_aorist_z(out: &mut SuffixBuf) {
    out.clear();
    out.push('z');
}

/// Fused negative-aorist person endings on the `+mA` stem: `yapmam`,
/// `yapmayız`, `yapmaz`, `yapmazsın`, `yapmazsınız`, `yapmazlar`.
pub(crate) fn realize_neg_aorist_person(person: Agreement, ctx: PhonoContext, out: &mut SuffixBuf) {
    out.clear();

    match person {
        Agreement::A1sg => out.push('m'),
        Agreement::A1pl => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(i_harmony(ctx));
            out.push('z');
        }
        Agreement::A3sg => out.push('z'),
        Agreement::A2sg => {
            out.push('z');
            out.push('s');
            out.push(i_harmony(ctx));
            out.push('n');
        }
        Agreement::A2pl => {
            out.push('z');
            out.push('s');
            out.push(i_harmony(ctx));
            out.push('n');
            out.push(i_harmony(ctx));
            out.push('z');
        }
        Agreement::A3pl => {
            out.push('z');
            out.push('l');
            out.push(a_harmony(ctx));
            out.push('r');
        }
    }
}

/// Optative persons: the present-type series except first plural `+lHm`
/// (`yapalım`).
pub(crate) fn realize_optative_person(person: Agreement, ctx: PhonoContext, out: &mut SuffixBuf) {
    if person == Agreement::A1pl {
        out.clear();
        out.push('l');
        out.push(i_harmony(ctx));
        out.push('m');
        return;
    }
    realize_copula_person(person, false, ctx, out);
}

/// Imperative persons: zero second singular (`yap`), `+yHn` (`yapın`),
/// `+sHn` (`yapsın`), `+sHnlAr` (`yapsınlar`). The long `+yHnHz` form is
/// deferred.
pub(crate) fn realize_imperative_person(person: Agreement, ctx: PhonoContext, out: &mut SuffixBuf) {
    out.clear();

    match person {
        Agreement::A2sg => {}
        Agreement::A2pl => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(i_harmony(ctx));
            out.push('n');
        }
        Agreement::A3sg => {
            out.push('s');
            out.push(i_harmony(ctx));
            out.push('n');
        }
        Agreement::A3pl => {
            out.push('s');
            out.push(i_harmony(ctx));
            out.push('n');
            out.push('l');
            out.push(a_harmony(ctx));
            out.push('r');
        }
        Agreement::A1sg | Agreement::A1pl => {}
    }
}

/// Realize a copular tense marker. The present tense is zero-marked.
pub(crate) fn realize_copula_tense(tense: CopulaTense, ctx: PhonoContext, out: &mut SuffixBuf) {
    out.clear();

    match tense {
        CopulaTense::Present => {}
        CopulaTense::Past => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(stop_consonant(ctx));
            out.push(i_harmony(ctx));
        }
        CopulaTense::Narrative => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push('m');
            out.push(i_harmony(ctx));
            out.push('ş');
        }
        CopulaTense::Conditional => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push('s');
            out.push(a_harmony(ctx));
        }
    }
}

/// Whether a copular tense takes the past-type person endings (`+m`, `+n`,
/// `+k`, `+nHz`) rather than the present-type ones (`+yHm`, `+sHn`, ...).
pub(crate) fn copula_person_is_past_type(tense: CopulaTense) -> bool {
    matches!(tense, CopulaTense::Past | CopulaTense::Conditional)
}

/// Realize a copular person ending. Third singular is zero in both series.
pub(crate) fn realize_copula_person(
    person: Agreement,
    past_type: bool,
    ctx: PhonoContext,
    out: &mut SuffixBuf,
) {
    out.clear();

    if past_type {
        match person {
            Agreement::A1sg => out.push('m'),
            Agreement::A2sg => out.push('n'),
            Agreement::A3sg => {}
            Agreement::A1pl => out.push('k'),
            Agreement::A2pl => {
                out.push('n');
                out.push(i_harmony(ctx));
                out.push('z');
            }
            Agreement::A3pl => {
                out.push('l');
                out.push(a_harmony(ctx));
                out.push('r');
            }
        }
        return;
    }

    match person {
        Agreement::A1sg => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(i_harmony(ctx));
            out.push('m');
        }
        Agreement::A2sg => {
            out.push('s');
            out.push(i_harmony(ctx));
            out.push('n');
        }
        Agreement::A3sg => {}
        Agreement::A1pl => {
            if ctx.ends_with_vowel() {
                out.push('y');
            }
            out.push(i_harmony(ctx));
            out.push('z');
        }
        Agreement::A2pl => {
            out.push('s');
            out.push(i_harmony(ctx));
            out.push('n');
            out.push(i_harmony(ctx));
            out.push('z');
        }
        Agreement::A3pl => {
            out.push('l');
            out.push(a_harmony(ctx));
            out.push('r');
        }
    }
}

/// Realize the overt copular marker `+DHr` (`evdedir`, `kitaptır`).
pub(crate) fn realize_copula_marker(ctx: PhonoContext, out: &mut SuffixBuf) {
    out.clear();
    out.push(stop_consonant(ctx));
    out.push(i_harmony(ctx));
    out.push('r');
}

/// Whether a derivational suffix ends in a `k` that voices to `ğ` before a
/// vowel-initial suffix (`kitaplık -> kitaplığı`, `yaptığı`, `yapacağı`).
pub(crate) fn derivation_has_voicing_k(derivation: Derivation) -> bool {
    matches!(
        derivation,
        Derivation::Ness
            | Derivation::Diminutive
            | Derivation::PastParticiple
            | Derivation::FutureParticiple
    )
}

pub(crate) fn is_case_morpheme(morpheme: NounMorpheme) -> bool {
    matches!(
        morpheme,
        NounMorpheme::Accusative
            | NounMorpheme::Dative
            | NounMorpheme::Locative
            | NounMorpheme::Ablative
            | NounMorpheme::Genitive
    )
}

/// Harmony seed for an entry: the override if present, else the bare stem's
/// last vowel. Ellipsis roots harmonize by the deleted vowel (`vakit` ->
/// `vakti`, not `vaktı`), so this is always taken from the bare stem.
pub(crate) fn harmony_seed(entry: &LexiconEntry) -> Option<char> {
    entry.harmony_vowel.or_else(|| last_vowel(&entry.bare_stem))
}

/// Context the first suffix would see if the vowel-stem alternant were used.
pub(crate) fn vowel_alternant_context(entry: &LexiconEntry) -> Option<PhonoContext> {
    entry.vowel_stem.as_deref().map(|stem| PhonoContext {
        last_char: stem.chars().next_back().unwrap_or('e'),
        last_vowel: harmony_seed(entry),
    })
}

/// One realizable surface segment of an analysis.
enum Segment {
    Derive(Derivation),
    Inflect {
        morpheme: NounMorpheme,
        number: Number,
        after_third: bool,
    },
    CopulaTense(CopulaTense),
    CopulaPerson {
        person: Agreement,
        past_type: bool,
    },
    CopulaMarker,
    VerbCausative {
        from_root: bool,
    },
    VerbPassive,
    VerbNegation,
    VerbAble,
    VerbUnable,
    VerbTense {
        tense: VerbTense,
        aorist_hr: bool,
    },
    NegAoristZ,
    NegAoristPerson(Agreement),
    OptativePerson(Agreement),
    ImperativePerson(Agreement),
}

/// Person series of a finite tense without a second tense.
fn verb_person_past_type(tense: VerbTense) -> bool {
    matches!(tense, VerbTense::Past | VerbTense::Conditional)
}

/// Push the second tense and person of a finite verb in surface order:
/// third plural precedes the second tense (`yapıyorlardı`), every other
/// person follows it (`yapıyorduk`).
fn push_second_and_person(
    segments: &mut Vec<Segment>,
    tense: VerbTense,
    second: Option<CopulaTense>,
    person: Option<Agreement>,
) {
    match (second, person) {
        (None, Some(person)) if person != Agreement::A3sg => {
            segments.push(Segment::CopulaPerson {
                person,
                past_type: verb_person_past_type(tense),
            });
        }
        (None, _) => {}
        (Some(second), Some(Agreement::A3pl)) => {
            segments.push(Segment::CopulaPerson {
                person: Agreement::A3pl,
                past_type: false,
            });
            segments.push(Segment::CopulaTense(second));
        }
        (Some(second), person) => {
            segments.push(Segment::CopulaTense(second));
            if let Some(person) = person
                && person != Agreement::A3sg
            {
                segments.push(Segment::CopulaPerson {
                    person,
                    past_type: copula_person_is_past_type(second),
                });
            }
        }
    }
}

/// Flatten an inflectional-group chain into surface segments, resolving the
/// pronominal-`n` contexts (third-person possessives and the zero group
/// after a relative `-ki`).
fn segments_from_igs(entry: &LexiconEntry, igs: &[InflectionGroup]) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut previous_derivation: Option<Derivation> = None;

    for (index, ig) in igs.iter().enumerate() {
        if index > 0
            && let Some(derivation) = ig.derivation
        {
            match derivation {
                Derivation::Zero => {}
                Derivation::Causative => {
                    segments.push(Segment::VerbCausative {
                        from_root: index == 1,
                    });
                }
                Derivation::Passive => segments.push(Segment::VerbPassive),
                other => segments.push(Segment::Derive(other)),
            }
        }

        // True verbal groups (root, voice, or the finite/carrier group);
        // copular groups are zero-derived.
        if ig.pos == IgPos::Verb && ig.derivation != Some(Derivation::Zero) {
            let neg = ig.polarity == Some(Polarity::Negative);
            match (ig.able, neg) {
                (false, false) => {}
                (false, true) => segments.push(Segment::VerbNegation),
                (true, false) => segments.push(Segment::VerbAble),
                (true, true) => segments.push(Segment::VerbUnable),
            }

            if let Some(tense) = ig.verb_tense {
                if tense == VerbTense::Imperative {
                    if let Some(person) = ig.person {
                        segments.push(Segment::ImperativePerson(person));
                    }
                } else if tense == VerbTense::Aorist && neg {
                    if ig.tense.is_none() {
                        segments.push(Segment::NegAoristPerson(
                            ig.person.unwrap_or(Agreement::A3sg),
                        ));
                    } else {
                        segments.push(Segment::NegAoristZ);
                        push_second_and_person(&mut segments, tense, ig.tense, ig.person);
                    }
                } else if tense == VerbTense::Optative {
                    segments.push(Segment::VerbTense {
                        tense,
                        aorist_hr: false,
                    });
                    if let Some(person) = ig.person
                        && person != Agreement::A3sg
                    {
                        segments.push(Segment::OptativePerson(person));
                    }
                } else {
                    let root_bare = index == 0 && !ig.able && !neg;
                    segments.push(Segment::VerbTense {
                        tense,
                        aorist_hr: aorist_is_hr(entry, root_bare),
                    });
                    push_second_and_person(&mut segments, tense, ig.tense, ig.person);
                }
            }

            previous_derivation = ig.derivation;
            continue;
        }

        if ig.pos == IgPos::Verb {
            let past_type = ig.tense.is_some_and(copula_person_is_past_type);
            if let Some(tense) = ig.tense
                && tense != CopulaTense::Present
            {
                segments.push(Segment::CopulaTense(tense));
            }
            // `+DHr` precedes the person in the present (`evdedirler`) and
            // follows the zero third person in the narrative (`evdeymiştir`).
            if ig.copula && ig.tense != Some(CopulaTense::Narrative) {
                segments.push(Segment::CopulaMarker);
            }
            if let Some(person) = ig.person
                && person != Agreement::A3sg
            {
                segments.push(Segment::CopulaPerson { person, past_type });
            }
            if ig.copula && ig.tense == Some(CopulaTense::Narrative) {
                segments.push(Segment::CopulaMarker);
            }
        }

        if ig.pos == IgPos::Noun {
            let third_possessive = matches!(
                ig.possessive,
                Possessive::ThirdSingular | Possessive::ThirdPlural
            );
            let zero_after_relative = ig.derivation == Some(Derivation::Zero)
                && previous_derivation == Some(Derivation::Relative)
                && ig.number == Number::Singular
                && ig.possessive == Possessive::None;
            let pronominal_n = third_possessive || zero_after_relative;

            let path = NounPath::new(ig.number, ig.possessive, ig.case);
            for morpheme in path.morphemes {
                segments.push(Segment::Inflect {
                    morpheme,
                    number: ig.number,
                    after_third: is_case_morpheme(morpheme) && pronominal_n,
                });
            }
        }

        previous_derivation = ig.derivation;
    }

    segments
}

fn realize_segment(
    entry: &LexiconEntry,
    stem_vowel_count: usize,
    segment: &Segment,
    ctx: PhonoContext,
    out: &mut SuffixBuf,
) {
    match segment {
        Segment::Derive(derivation) => realize_derivation(*derivation, ctx, out),
        Segment::Inflect {
            morpheme,
            number,
            after_third,
        } => realize_noun_morpheme(*morpheme, ctx, *number, *after_third, out),
        Segment::CopulaTense(tense) => realize_copula_tense(*tense, ctx, out),
        Segment::CopulaPerson { person, past_type } => {
            realize_copula_person(*person, *past_type, ctx, out)
        }
        Segment::CopulaMarker => realize_copula_marker(ctx, out),
        Segment::VerbCausative { from_root } => {
            realize_causative(entry, *from_root, stem_vowel_count, ctx, out)
        }
        Segment::VerbPassive => realize_passive(ctx, out),
        Segment::VerbNegation => realize_negation(ctx, out),
        Segment::VerbAble => realize_able(ctx, out),
        Segment::VerbUnable => realize_unable(ctx, out),
        Segment::VerbTense { tense, aorist_hr } => realize_verb_tense(*tense, *aorist_hr, ctx, out),
        Segment::NegAoristZ => realize_neg_aorist_z(out),
        Segment::NegAoristPerson(person) => realize_neg_aorist_person(*person, ctx, out),
        Segment::OptativePerson(person) => realize_optative_person(*person, ctx, out),
        Segment::ImperativePerson(person) => realize_imperative_person(*person, ctx, out),
    }
}

/// Generate the surface form for an analysis expressed as an
/// inflectional-group chain (as produced by the analyzer). The chain is
/// assumed morphotactically legal.
pub fn generate_from_igs(entry: &LexiconEntry, igs: &[InflectionGroup]) -> String {
    let segments = segments_from_igs(entry, igs);

    let bare_context = PhonoContext {
        last_char: entry.bare_stem.chars().next_back().unwrap_or('e'),
        last_vowel: harmony_seed(entry),
    };
    let root_vowels = entry.bare_stem.chars().filter(|ch| is_vowel(*ch)).count();

    let mut buf = SuffixBuf::new();
    let (stem, mut ctx) = match (vowel_alternant_context(entry), segments.first()) {
        (Some(vowel_ctx), Some(first)) => {
            realize_segment(entry, root_vowels, first, vowel_ctx, &mut buf);
            if starts_with_vowel(buf.as_str()) {
                (entry.stem_for_vowel_initial_suffix(), vowel_ctx)
            } else {
                (entry.bare_stem.as_str(), bare_context)
            }
        }
        _ => (entry.bare_stem.as_str(), bare_context),
    };

    let mut surface = String::with_capacity(stem.len() + 16);
    surface.push_str(stem);
    let mut pending_voicing_k = false;
    let mut vowel_count = root_vowels;

    for segment in &segments {
        // Vowel ellipsis before the progressive (`başlıyor`, `yapmıyor`).
        if matches!(
            segment,
            Segment::VerbTense {
                tense: VerbTense::Progressive,
                ..
            }
        ) && surface.chars().next_back().is_some_and(is_vowel)
        {
            // Harmony after the ellipsis follows the last remaining vowel
            // (`okumuyor`, `oynuyor`), falling back to the elided one
            // (`yiyor`).
            let elided = surface.pop().expect("surface ends with a vowel");
            ctx.last_vowel = last_vowel(&surface).or(Some(elided));
        }

        realize_segment(entry, vowel_count, segment, ctx, &mut buf);
        if pending_voicing_k && starts_with_vowel(buf.as_str()) {
            surface.pop();
            surface.push('ğ');
            ctx.last_char = 'ğ';
        }
        surface.push_str(buf.as_str());
        ctx.extend(buf.as_str());
        vowel_count += buf.as_str().chars().filter(|ch| is_vowel(*ch)).count();
        pending_voicing_k = matches!(
            segment,
            Segment::Derive(derivation) if derivation_has_voicing_k(*derivation)
        ) || matches!(
            segment,
            Segment::VerbTense {
                tense: VerbTense::Future,
                ..
            }
        );
    }

    surface
}

pub fn generate_noun_surface(entry: &LexiconEntry, path: &NounPath) -> String {
    generate_from_igs(
        entry,
        &[InflectionGroup::noun(
            None,
            path.number,
            path.possessive,
            path.case,
        )],
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        analyzer::analyze_word,
        generator::{generate_from_igs, generate_noun_surface},
        gold::GOLD_GENERATION_CASES,
        lexicon::Lexicon,
        state::{Case, Derivation, InflectionGroup, NounPath, Number, Possessive, noun_paths},
    };

    fn seed() -> Lexicon {
        Lexicon::seed()
    }

    fn surface(lexicon: &Lexicon, lemma: &str, path: &NounPath) -> String {
        let entry = lexicon.entry_by_lemma(lemma).expect("lemma should exist");
        generate_noun_surface(entry, path)
    }

    fn derived_surface(lexicon: &Lexicon, lemma: &str, igs: &[InflectionGroup]) -> String {
        let entry = lexicon.entry_by_lemma(lemma).expect("lemma should exist");
        generate_from_igs(entry, igs)
    }

    #[test]
    fn generates_plural_nouns() {
        let lexicon = seed();
        assert_eq!(
            surface(
                &lexicon,
                "ev",
                &NounPath::new(Number::Plural, Possessive::None, Case::Nominative)
            ),
            "evler"
        );
    }

    #[test]
    fn generates_voiced_dative_forms() {
        let lexicon = seed();
        assert_eq!(
            surface(
                &lexicon,
                "kitap",
                &NounPath::new(Number::Singular, Possessive::None, Case::Dative)
            ),
            "kitaba"
        );
    }

    #[test]
    fn generates_pronominal_n_after_third_possessive() {
        let lexicon = seed();
        assert_eq!(
            surface(
                &lexicon,
                "ev",
                &NounPath::new(Number::Singular, Possessive::ThirdSingular, Case::Dative)
            ),
            "evine"
        );
        assert_eq!(
            surface(
                &lexicon,
                "masa",
                &NounPath::new(Number::Singular, Possessive::ThirdSingular, Case::Genitive)
            ),
            "masasının"
        );
    }

    #[test]
    fn generates_vowel_ellipsis_and_loan_forms() {
        let lexicon = seed();
        assert_eq!(
            surface(
                &lexicon,
                "burun",
                &NounPath::new(
                    Number::Singular,
                    Possessive::FirstSingular,
                    Case::Nominative
                )
            ),
            "burnum"
        );
        assert_eq!(
            surface(
                &lexicon,
                "vakit",
                &NounPath::new(
                    Number::Singular,
                    Possessive::ThirdSingular,
                    Case::Nominative
                )
            ),
            "vakti"
        );
        assert_eq!(
            surface(
                &lexicon,
                "saat",
                &NounPath::new(Number::Singular, Possessive::None, Case::Accusative)
            ),
            "saati"
        );
        assert_eq!(
            surface(
                &lexicon,
                "su",
                &NounPath::new(
                    Number::Singular,
                    Possessive::ThirdSingular,
                    Case::Nominative
                )
            ),
            "suyu"
        );
    }

    #[test]
    fn generates_agent_and_ness_derivations() {
        let lexicon = seed();
        let root =
            InflectionGroup::noun(None, Number::Singular, Possessive::None, Case::Nominative);

        assert_eq!(
            derived_surface(
                &lexicon,
                "kitap",
                &[
                    root,
                    InflectionGroup::noun(
                        Some(Derivation::Agent),
                        Number::Singular,
                        Possessive::None,
                        Case::Nominative
                    )
                ]
            ),
            "kitapçı"
        );
        assert_eq!(
            derived_surface(
                &lexicon,
                "deniz",
                &[
                    root,
                    InflectionGroup::noun(
                        Some(Derivation::Agent),
                        Number::Singular,
                        Possessive::None,
                        Case::Nominative
                    ),
                    InflectionGroup::noun(
                        Some(Derivation::Ness),
                        Number::Singular,
                        Possessive::None,
                        Case::Nominative
                    )
                ]
            ),
            "denizcilik"
        );
    }

    #[test]
    fn generates_derived_stem_k_voicing() {
        let lexicon = seed();
        let root =
            InflectionGroup::noun(None, Number::Singular, Possessive::None, Case::Nominative);

        assert_eq!(
            derived_surface(
                &lexicon,
                "göz",
                &[
                    root,
                    InflectionGroup::noun(
                        Some(Derivation::Ness),
                        Number::Singular,
                        Possessive::None,
                        Case::Accusative
                    )
                ]
            ),
            "gözlüğü"
        );
        assert_eq!(
            derived_surface(
                &lexicon,
                "ağaç",
                &[
                    root,
                    InflectionGroup::noun(
                        Some(Derivation::Diminutive),
                        Number::Singular,
                        Possessive::None,
                        Case::Nominative
                    )
                ]
            ),
            "ağaççık"
        );
    }

    #[test]
    fn generates_relative_ki_chains() {
        let lexicon = seed();

        assert_eq!(
            derived_surface(
                &lexicon,
                "ev",
                &[
                    InflectionGroup::noun(None, Number::Singular, Possessive::None, Case::Locative),
                    InflectionGroup::adj(Derivation::Relative),
                ]
            ),
            "evdeki"
        );
        assert_eq!(
            derived_surface(
                &lexicon,
                "ev",
                &[
                    InflectionGroup::noun(None, Number::Singular, Possessive::None, Case::Locative),
                    InflectionGroup::adj(Derivation::Relative),
                    InflectionGroup::noun(
                        Some(Derivation::Zero),
                        Number::Singular,
                        Possessive::None,
                        Case::Dative
                    ),
                ]
            ),
            "evdekine"
        );
        assert_eq!(
            derived_surface(
                &lexicon,
                "masa",
                &[
                    InflectionGroup::noun(None, Number::Singular, Possessive::None, Case::Locative),
                    InflectionGroup::adj(Derivation::Relative),
                    InflectionGroup::noun(
                        Some(Derivation::Zero),
                        Number::Plural,
                        Possessive::None,
                        Case::Genitive
                    ),
                    InflectionGroup::adj(Derivation::Relative),
                ]
            ),
            "masadakilerinki"
        );
    }

    #[test]
    fn generated_surfaces_roundtrip_back_to_their_paths() {
        let lexicon = seed();
        let paths = noun_paths();

        for entry in lexicon
            .entries()
            .iter()
            .filter(|entry| entry.pos == crate::lexicon::RootPos::Noun)
        {
            for path in paths
                .iter()
                .filter(|path| !entry.compound_p3sg || path.possessive != Possessive::None)
            {
                let surface = generate_noun_surface(entry, path);
                let analyses = analyze_word(&lexicon, &surface);

                assert!(
                    analyses.iter().any(|analysis| {
                        analysis.lemma == entry.lemma
                            && analysis.igs.len() == 1
                            && analysis.final_ig().number == path.number
                            && analysis.final_ig().possessive == path.possessive
                            && analysis.final_ig().case == path.case
                    }),
                    "generated surface {} did not roundtrip for lemma={} path={:?}",
                    surface,
                    entry.lemma,
                    path
                );
            }
        }
    }

    #[test]
    fn generates_expected_gold_noun_surfaces() {
        let lexicon = seed();

        for &(lemma, number, possessive, case, expected) in GOLD_GENERATION_CASES {
            let entry = lexicon
                .entry_by_lemma(lemma)
                .expect("gold lemma should exist in seed lexicon");
            assert_eq!(
                generate_noun_surface(entry, &NounPath::new(number, possessive, case)),
                expected,
                "unexpected surface for lemma={lemma} path=({number:?},{possessive:?},{case:?})"
            );
        }
    }
}
