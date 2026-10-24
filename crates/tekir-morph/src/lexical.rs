use crate::state::{
    Agreement, Case, CopulaTense, Derivation, IgPos, InflectionGroup, Number, Polarity, Possessive,
    VerbTense,
};

/// Render an inflectional-group chain as a lexical gloss, joining groups
/// with `^DB` derivational boundaries in the METU-Sabancı convention:
/// `ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Nom`.
pub fn gloss_from_igs(lemma: &str, igs: &[InflectionGroup]) -> String {
    let mut gloss = String::with_capacity(lemma.len() + igs.len() * 24);
    gloss.push_str(lemma);

    for (index, ig) in igs.iter().enumerate() {
        if index == 0 {
            match ig.pos {
                IgPos::Noun => {
                    gloss.push_str("+Noun");
                    push_inflection_tags(&mut gloss, ig);
                }
                IgPos::Verb => {
                    gloss.push_str("+Verb");
                    push_verbal_tags(&mut gloss, ig);
                }
                IgPos::Ques => gloss.push_str("+Ques"),
                IgPos::Func => gloss.push_str("+Func"),
                IgPos::Adj => gloss.push_str("+Adj"),
                IgPos::Adv => gloss.push_str("+Adverb"),
            }
            continue;
        }

        gloss.push_str("^DB+");
        match ig.pos {
            IgPos::Adj => {
                gloss.push_str("Adj+");
                gloss.push_str(derivation_tag(
                    ig.derivation
                        .expect("derived group must carry a derivation"),
                ));
            }
            IgPos::Noun => {
                gloss.push_str("Noun+");
                gloss.push_str(derivation_tag(
                    ig.derivation
                        .expect("derived group must carry a derivation"),
                ));
                push_inflection_tags(&mut gloss, ig);
            }
            IgPos::Verb => {
                gloss.push_str("Verb");
                if let Some(derivation) = ig.derivation {
                    gloss.push('+');
                    gloss.push_str(derivation_tag(derivation));
                }
                push_verbal_tags(&mut gloss, ig);
            }
            IgPos::Adv => {
                gloss.push_str("Adverb+");
                gloss.push_str(derivation_tag(
                    ig.derivation
                        .expect("derived group must carry a derivation"),
                ));
            }
            IgPos::Ques => gloss.push_str("Ques"),
            IgPos::Func => gloss.push_str("Func"),
        }
    }

    gloss
}

/// Verbal and copular inflection tags on a `Verb` group: `+Able`,
/// polarity, primary tense, second/copular tense, person, `+Cop`.
fn push_verbal_tags(gloss: &mut String, ig: &InflectionGroup) {
    if ig.able {
        gloss.push_str("+Able");
    }
    if let Some(polarity) = ig.polarity {
        gloss.push_str(match polarity {
            Polarity::Positive => "+Pos",
            Polarity::Negative => "+Neg",
        });
    }
    if let Some(verb_tense) = ig.verb_tense {
        gloss.push('+');
        gloss.push_str(verb_tense_tag(verb_tense));
    }
    if let Some(tense) = ig.tense {
        gloss.push('+');
        gloss.push_str(tense_tag(tense));
    }
    if let Some(person) = ig.person {
        gloss.push('+');
        gloss.push_str(agreement_tag(person));
    }
    if ig.copula {
        gloss.push_str("+Cop");
    }
}

fn verb_tense_tag(tense: VerbTense) -> &'static str {
    match tense {
        VerbTense::Progressive => "Prog1",
        VerbTense::Progressive2 => "Prog2",
        VerbTense::Future => "Fut",
        VerbTense::Aorist => "Aor",
        VerbTense::Past => "Past",
        VerbTense::Narrative => "Narr",
        VerbTense::Necessitative => "Neces",
        VerbTense::Optative => "Opt",
        VerbTense::Imperative => "Imp",
        VerbTense::Conditional => "Cond",
    }
}

fn tense_tag(tense: CopulaTense) -> &'static str {
    match tense {
        CopulaTense::Present => "Pres",
        CopulaTense::Past => "Past",
        CopulaTense::Narrative => "Narr",
        CopulaTense::Conditional => "Cond",
    }
}

fn agreement_tag(person: Agreement) -> &'static str {
    match person {
        Agreement::A1sg => "A1sg",
        Agreement::A2sg => "A2sg",
        Agreement::A3sg => "A3sg",
        Agreement::A1pl => "A1pl",
        Agreement::A2pl => "A2pl",
        Agreement::A3pl => "A3pl",
    }
}

fn push_inflection_tags(gloss: &mut String, ig: &InflectionGroup) {
    gloss.push('+');
    gloss.push_str(lexical_number_tag(ig.number));
    gloss.push('+');
    gloss.push_str(lexical_possessive_tag(ig.possessive));
    gloss.push('+');
    gloss.push_str(lexical_case_tag(ig.case));
}

fn derivation_tag(derivation: Derivation) -> &'static str {
    match derivation {
        Derivation::Agent => "Agt",
        Derivation::Ness => "Ness",
        Derivation::Diminutive => "Dim",
        Derivation::Without => "Without",
        Derivation::With => "With",
        Derivation::Relative => "Rel",
        Derivation::Zero => "Zero",
        Derivation::While => "While",
        Derivation::AsIf => "AsIf",
        Derivation::Causative => "Caus",
        Derivation::Passive => "Pass",
        Derivation::Infinitive1 => "Inf1",
        Derivation::Infinitive2 => "Inf2",
        Derivation::Infinitive3 => "Inf3",
        Derivation::PresentParticiple => "PresPart",
        Derivation::PastParticiple => "PastPart",
        Derivation::FutureParticiple => "FutPart",
        Derivation::NarrativeParticiple => "NarrPart",
        Derivation::AfterDoingSo => "AfterDoingSo",
        Derivation::ByDoingSo => "ByDoingSo",
        Derivation::WithoutDoingSo => "WithoutDoingSo",
        Derivation::Become => "Become",
        Derivation::Acquire => "Acquire",
    }
}

fn lexical_number_tag(number: Number) -> &'static str {
    match number {
        Number::Singular => "A3sg",
        Number::Plural => "A3pl",
    }
}

fn lexical_possessive_tag(possessive: Possessive) -> &'static str {
    match possessive {
        Possessive::None => "Pnon",
        Possessive::FirstSingular => "P1sg",
        Possessive::SecondSingular => "P2sg",
        Possessive::ThirdSingular => "P3sg",
        Possessive::FirstPlural => "P1pl",
        Possessive::SecondPlural => "P2pl",
        Possessive::ThirdPlural => "P3pl",
    }
}

fn lexical_case_tag(case: Case) -> &'static str {
    match case {
        Case::Nominative => "Nom",
        Case::Accusative => "Acc",
        Case::Dative => "Dat",
        Case::Locative => "Loc",
        Case::Ablative => "Abl",
        Case::Genitive => "Gen",
        Case::Instrumental => "Ins",
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        lexical::gloss_from_igs,
        state::{Case, InflectionGroup, Number, Possessive},
    };

    #[test]
    fn builds_conventional_noun_glosses() {
        let ig = InflectionGroup::noun(None, Number::Plural, Possessive::None, Case::Nominative);

        assert_eq!(gloss_from_igs("ev", &[ig]), "ev+Noun+A3pl+Pnon+Nom");
    }

    #[test]
    fn gloss_keeps_default_singular_and_nominal_tags() {
        let ig = InflectionGroup::noun(None, Number::Singular, Possessive::None, Case::Dative);

        assert_eq!(gloss_from_igs("kitap", &[ig]), "kitap+Noun+A3sg+Pnon+Dat");
    }
}
