use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Number {
    Singular,
    Plural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Possessive {
    None,
    FirstSingular,
    SecondSingular,
    ThirdSingular,
    FirstPlural,
    SecondPlural,
    ThirdPlural,
}

impl Possessive {
    pub const ALL: [Possessive; 7] = [
        Possessive::None,
        Possessive::FirstSingular,
        Possessive::SecondSingular,
        Possessive::ThirdSingular,
        Possessive::FirstPlural,
        Possessive::SecondPlural,
        Possessive::ThirdPlural,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Case {
    Nominative,
    Accusative,
    Dative,
    Locative,
    Ablative,
    Genitive,
    /// `+ylA` (`kitapla`, `arabayla`, `arabasıyla`), tag `Ins`.
    Instrumental,
}

impl Case {
    pub const ALL: [Case; 7] = [
        Case::Nominative,
        Case::Accusative,
        Case::Dative,
        Case::Locative,
        Case::Ablative,
        Case::Genitive,
        Case::Instrumental,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NounMorpheme {
    Plural,
    Instrumental,
    FirstSingularPossessive,
    SecondSingularPossessive,
    ThirdSingularPossessive,
    FirstPluralPossessive,
    SecondPluralPossessive,
    ThirdPluralPossessive,
    Accusative,
    Dative,
    Locative,
    Ablative,
    Genitive,
}

/// Part of speech of one inflectional group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IgPos {
    Noun,
    Adj,
    /// Verbal group: a verb root, a voice extension, or a zero-derived
    /// copular predicate (`evdeydi`).
    Verb,
    /// Adverbial (`evdeyken`, `yaparak`, `evdeymişçesine`).
    Adv,
    /// The detached question particle `mH` (`mi`, `miydi`, `musunuz`).
    Ques,
    /// Closed-class function-word root (pronoun full forms,
    /// postpositions, conjunctions); glosses are table-driven.
    Func,
}

/// Polarity of a verbal group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Polarity {
    Positive,
    Negative,
}

/// Primary tense/aspect/mood of a finite verb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbTense {
    /// `+Hyor` (`yapıyor`), with vowel ellipsis before it (`başlıyor`).
    Progressive,
    /// `+mAktA` (`yapmakta`).
    Progressive2,
    /// `+yAcAk` (`yapacak`, `yapacağım`).
    Future,
    /// `+Ar`/`+Hr`/`+r` (`yapar`, `gelir`, `okur`); negative aorist is the
    /// irregular `+mAz` series (`yapmaz`, `yapmam`).
    Aorist,
    /// `+DH` (`yaptı`, `gitti`).
    Past,
    /// `+mHş` (`yapmış`).
    Narrative,
    /// `+mAlH` (`yapmalı`).
    Necessitative,
    /// `+yA` (`yapa`, `yapalım`).
    Optative,
    /// Zero + person series (`yap`, `yapın`, `yapsınlar`).
    Imperative,
    /// `+sA` (`yapsa`, `yapsaydı`).
    Conditional,
}

/// Tense of a copular (nominal-verb) group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CopulaTense {
    /// Zero-marked present (`evdeyim`).
    Present,
    /// `+yDH` (`evdeydi`).
    Past,
    /// `+ymHş` (`evdeymiş`).
    Narrative,
    /// `+ysA` (`evdeyse`).
    Conditional,
}

/// Subject agreement of a copular predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Agreement {
    A1sg,
    A2sg,
    A3sg,
    A1pl,
    A2pl,
    A3pl,
}

/// Derivational suffixes. Each derivation closes the current inflectional
/// group and opens a new one (`^DB` in glosses).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Derivation {
    /// `+cH`, Noun -> Noun (`kitapçı`), tag `Agt`.
    Agent,
    /// `+lHk`, Noun or Adj -> Noun (`kitaplık`, `evsizlik`), tag `Ness`.
    Ness,
    /// `+cHk`, Noun -> Noun (`evcik`, `ağaççık`), tag `Dim`.
    Diminutive,
    /// `+sHz`, Noun -> Adj (`evsiz`), tag `Without`.
    Without,
    /// `+lH`, Noun -> Adj (`evli`), tag `With`.
    With,
    /// `+ki` after locative or genitive -> Adj (`evdeki`, `evinki`), tag `Rel`.
    Relative,
    /// Zero nominalization of an adjective (`evdekiler`) or zero verb
    /// formation of a copular predicate (`evdeydi`), tag `Zero`.
    Zero,
    /// `+ykAn` temporal adverb of a copular stem (`evdeyken`), tag `While`.
    While,
    /// `+ymHşçAsHnA`'s adverbial tail (`evdeymişçesine`), tag `AsIf`.
    AsIf,
    /// Causative voice `+DHr`/`+t` with lexical `+Hr`/`+Ht`/`+Ar`
    /// (`yaptır`, `okut`, `içir`), tag `Caus`.
    Causative,
    /// Passive voice `+Hl`/`+Hn` (`yapıl`, `okun`, `alın`), tag `Pass`.
    Passive,
    /// Infinitive `+mAk` (`yapmak`), tag `Inf1`.
    Infinitive1,
    /// Verbal noun `+mA` (`yapma`, `yapması`), tag `Inf2`.
    Infinitive2,
    /// Verbal noun `+yHş` (`bekleyiş`), tag `Inf3`.
    Infinitive3,
    /// Present participle `+yAn` (`yapan`), tag `PresPart`.
    PresentParticiple,
    /// Past participle `+DHk` with possessive/case (`yaptığım`,
    /// `yaptıktan`), tag `PastPart`.
    PastParticiple,
    /// Future participle `+yAcAk` (`yapacağı`), tag `FutPart`.
    FutureParticiple,
    /// Narrative participle `+mHş` (`yapmış` as adjective), tag `NarrPart`.
    NarrativeParticiple,
    /// Converb `+yHp` (`yapıp`), tag `AfterDoingSo`.
    AfterDoingSo,
    /// Converb `+yArAk` (`yaparak`), tag `ByDoingSo`.
    ByDoingSo,
    /// Converb `+mAdAn` (`yapmadan`), tag `WithoutDoingSo`.
    WithoutDoingSo,
    /// `+lAş`, Noun or Adj -> Verb (`güzelleş`, `çocuklaş`), tag `Become`.
    Become,
    /// `+lAn`, Noun or Adj -> Verb (`evlen`, `hastalan`), tag `Acquire`.
    Acquire,
}

/// One inflectional group: a stretch of inflection between derivational
/// boundaries. A word is the root plus one or more groups; glosses join them
/// with `^DB`. Nominal groups use `number`/`possessive`/`case`; copular
/// `Verb` groups use `tense`/`person`/`copula`; `Adj` and `Adv` groups hold
/// defaults that are not rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InflectionGroup {
    pub pos: IgPos,
    pub derivation: Option<Derivation>,
    pub number: Number,
    pub possessive: Possessive,
    pub case: Case,
    /// Copular tense, and the *second* tense of a finite verb
    /// (`yapıyordu`: `verb_tense` Progressive + `tense` Past).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tense: Option<CopulaTense>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person: Option<Agreement>,
    #[serde(skip_serializing_if = "is_false")]
    pub copula: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polarity: Option<Polarity>,
    #[serde(skip_serializing_if = "is_false")]
    pub able: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb_tense: Option<VerbTense>,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl InflectionGroup {
    fn base(pos: IgPos, derivation: Option<Derivation>) -> Self {
        Self {
            pos,
            derivation,
            number: Number::Singular,
            possessive: Possessive::None,
            case: Case::Nominative,
            tense: None,
            person: None,
            copula: false,
            polarity: None,
            able: false,
            verb_tense: None,
        }
    }

    /// A bare verbal group: the root before voice/inflection, or a voice
    /// extension (`Causative`/`Passive`).
    pub fn verb(derivation: Option<Derivation>) -> Self {
        Self::base(IgPos::Verb, derivation)
    }

    /// Attach finite-verb inflection to a verbal group.
    pub fn with_verb_inflection(
        mut self,
        polarity: Polarity,
        able: bool,
        verb_tense: Option<VerbTense>,
        second_tense: Option<CopulaTense>,
        person: Option<Agreement>,
    ) -> Self {
        self.polarity = Some(polarity);
        self.able = able;
        self.verb_tense = verb_tense;
        self.tense = second_tense;
        self.person = person;
        self
    }

    /// Root group of the detached question particle.
    pub fn ques() -> Self {
        Self::base(IgPos::Ques, None)
    }

    /// Root group of a closed-class function word.
    pub fn func() -> Self {
        Self::base(IgPos::Func, None)
    }

    pub fn noun(
        derivation: Option<Derivation>,
        number: Number,
        possessive: Possessive,
        case: Case,
    ) -> Self {
        Self {
            number,
            possessive,
            case,
            ..Self::base(IgPos::Noun, derivation)
        }
    }

    pub fn adj(derivation: Derivation) -> Self {
        Self::base(IgPos::Adj, Some(derivation))
    }

    /// Root group of an adjective lexeme (`güzel+Adj`).
    pub fn adj_root() -> Self {
        Self::base(IgPos::Adj, None)
    }

    /// Zero-derived copular group. `tense: None` marks the bare copular
    /// stem that only carries an adverbial (`evdeyken`); `copula` is the
    /// overt `+DHr` marker (`evdedir`).
    pub fn copula(tense: Option<CopulaTense>, person: Option<Agreement>, copula: bool) -> Self {
        Self {
            tense,
            person,
            copula,
            ..Self::base(IgPos::Verb, Some(Derivation::Zero))
        }
    }

    pub fn adverb(derivation: Derivation) -> Self {
        Self::base(IgPos::Adv, Some(derivation))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NounPath {
    pub number: Number,
    pub possessive: Possessive,
    pub case: Case,
    pub morphemes: Vec<NounMorpheme>,
}

impl NounPath {
    pub fn new(number: Number, possessive: Possessive, case: Case) -> Self {
        let mut morphemes = Vec::new();

        if matches!(number, Number::Plural) {
            morphemes.push(NounMorpheme::Plural);
        }

        match possessive {
            Possessive::None => {}
            Possessive::FirstSingular => morphemes.push(NounMorpheme::FirstSingularPossessive),
            Possessive::SecondSingular => morphemes.push(NounMorpheme::SecondSingularPossessive),
            Possessive::ThirdSingular => morphemes.push(NounMorpheme::ThirdSingularPossessive),
            Possessive::FirstPlural => morphemes.push(NounMorpheme::FirstPluralPossessive),
            Possessive::SecondPlural => morphemes.push(NounMorpheme::SecondPluralPossessive),
            Possessive::ThirdPlural => morphemes.push(NounMorpheme::ThirdPluralPossessive),
        }

        match case {
            Case::Nominative => {}
            Case::Accusative => morphemes.push(NounMorpheme::Accusative),
            Case::Dative => morphemes.push(NounMorpheme::Dative),
            Case::Locative => morphemes.push(NounMorpheme::Locative),
            Case::Ablative => morphemes.push(NounMorpheme::Ablative),
            Case::Genitive => morphemes.push(NounMorpheme::Genitive),
            Case::Instrumental => morphemes.push(NounMorpheme::Instrumental),
        }

        Self {
            number,
            possessive,
            case,
            morphemes,
        }
    }
}

/// Every single-group noun inflection: number x possessive x case (98).
pub fn noun_paths() -> Vec<NounPath> {
    let mut paths = Vec::with_capacity(2 * Possessive::ALL.len() * Case::ALL.len());
    for number in [Number::Singular, Number::Plural] {
        for possessive in Possessive::ALL {
            for case in Case::ALL {
                paths.push(NounPath::new(number, possessive, case));
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use crate::state::{Case, NounMorpheme, Number, Possessive, noun_paths};

    #[test]
    fn builds_expected_number_of_noun_paths() {
        assert_eq!(noun_paths().len(), 98);
    }

    #[test]
    fn path_morphemes_follow_state_order() {
        let path = noun_paths()
            .into_iter()
            .find(|path| {
                matches!(path.number, Number::Plural)
                    && matches!(path.possessive, Possessive::FirstSingular)
                    && matches!(path.case, Case::Locative)
            })
            .expect("path should exist");

        assert_eq!(
            path.morphemes,
            vec![
                NounMorpheme::Plural,
                NounMorpheme::FirstSingularPossessive,
                NounMorpheme::Locative,
            ]
        );
    }
}
