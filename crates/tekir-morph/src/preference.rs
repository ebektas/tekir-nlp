use crate::{
    MorphAnalysis,
    state::{Case, IgPos, Number, Possessive},
};

pub fn choose_best_noun_analysis(analyses: &[MorphAnalysis]) -> Option<MorphAnalysis> {
    analyses
        .iter()
        .min_by_key(|analysis| noun_preference_key(analysis))
        .cloned()
}

fn noun_preference_key(analysis: &MorphAnalysis) -> (usize, u32, u8, &str) {
    // Penalties are summed over every nominal group, not just the final
    // one — a copular or adverbial final group carries no inflection, and
    // marked readings buried mid-chain must still count against an
    // analysis (`denizdeyken` vs `de+P2pl+...`).
    let inflection_penalty: u32 = analysis
        .igs
        .iter()
        .filter(|ig| ig.pos == IgPos::Noun)
        .map(|ig| u32::from(possessive_penalty(ig.possessive)) + u32::from(case_penalty(ig.case)))
        .sum();

    (
        analysis.igs.len(),
        inflection_penalty,
        number_penalty(analysis),
        analysis.lexical_form.as_str(),
    )
}

fn possessive_penalty(possessive: Possessive) -> u8 {
    match possessive {
        Possessive::None => 0,
        Possessive::ThirdSingular => 1,
        Possessive::FirstSingular | Possessive::SecondSingular => 2,
        Possessive::FirstPlural | Possessive::SecondPlural => 3,
        Possessive::ThirdPlural => 4,
    }
}

fn case_penalty(case: Case) -> u8 {
    match case {
        Case::Nominative => 1,
        Case::Accusative
        | Case::Dative
        | Case::Locative
        | Case::Ablative
        | Case::Genitive
        | Case::Instrumental => 0,
    }
}

fn number_penalty(analysis: &MorphAnalysis) -> u8 {
    if looks_plural_surface(&analysis.surface) && analysis.final_ig().number == Number::Singular {
        1
    } else {
        0
    }
}

fn looks_plural_surface(surface: &str) -> bool {
    surface.contains("lar") || surface.contains("ler")
}

#[cfg(test)]
mod tests {
    use crate::{
        MorphAnalysis, analyzer::analyze_word, gold::GOLD_PREFERENCE_CASES, lexicon::Lexicon,
        preference::choose_best_noun_analysis,
    };

    #[test]
    fn prefers_non_possessive_case_reading_for_evleri() {
        let lexicon = Lexicon::seed();
        let analyses = analyze_word(&lexicon, "evleri");
        let best =
            choose_best_noun_analysis(&analyses).expect("evleri should have at least one analysis");

        assert_eq!(best.lexical_form, "ev+Noun+A3pl+Pnon+Acc");
    }

    #[test]
    fn prefers_genitive_reading_for_evin() {
        let lexicon = Lexicon::seed();
        let analyses = analyze_word(&lexicon, "evin");
        let best =
            choose_best_noun_analysis(&analyses).expect("evin should have at least one analysis");

        assert_eq!(best.lexical_form, "ev+Noun+A3sg+Pnon+Gen");
    }

    #[test]
    fn returns_none_for_empty_analysis_sets() {
        assert_eq!(choose_best_noun_analysis(&[]), None::<MorphAnalysis>);
    }

    #[test]
    fn matches_shared_preference_expectations() {
        let lexicon = Lexicon::seed();

        for &(surface, expected_best_gloss) in GOLD_PREFERENCE_CASES {
            let analyses = analyze_word(&lexicon, surface);
            let best = choose_best_noun_analysis(&analyses)
                .expect("shared preference case should have at least one analysis");

            assert_eq!(
                best.lexical_form, expected_best_gloss,
                "unexpected best analysis for {surface}"
            );
        }
    }
}
