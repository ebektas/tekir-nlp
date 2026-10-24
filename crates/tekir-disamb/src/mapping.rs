//! Match a UD token against tekir's candidate analyses to recover the gold
//! analysis for training and evaluation.

use tekir_morph::{Case, IgPos, MorphAnalysis, PartOfSpeech};

use crate::conllu::{Token, turkish_lowercase};

/// Index of the candidate that matches the UD annotation, if any. Lemma
/// compatibility is mandatory (exact, or prefix-compatible for UD's fully
/// derived lemmas like `abajurlu` vs tekir's root `abajur`); UPOS and key
/// features refine the choice.
pub fn gold_index(token: &Token, candidates: &[MorphAnalysis]) -> Option<usize> {
    let ud_lemma = turkish_lowercase(&token.lemma);

    let mut best: Option<(usize, i32)> = None;
    for (index, analysis) in candidates.iter().enumerate() {
        let lemma = turkish_lowercase(&analysis.lemma);
        let lemma_score = if lemma == ud_lemma {
            100
        } else if ud_lemma.starts_with(&lemma) || lemma.starts_with(&ud_lemma) {
            60
        } else {
            continue;
        };

        let mut score = lemma_score;
        if upos_compatible(&token.upos, analysis) {
            score += 20;
        }
        if let Some(case) = token.feat("Case")
            && analysis.pos == PartOfSpeech::Noun
            && ud_case(case) == Some(analysis.case)
        {
            score += 8;
        }
        if let Some(number) = token.feat("Number")
            && analysis.pos == PartOfSpeech::Noun
        {
            let plural = analysis.number == tekir_morph::Number::Plural;
            if (number == "Plur") == plural {
                score += 4;
            }
        }
        if token.feat("Number[psor]").is_some()
            == (analysis.possessive != tekir_morph::Possessive::None)
        {
            score += 4;
        }
        if let Some(polarity) = token.feat("Polarity")
            && let Some(ig) = analysis
                .igs
                .iter()
                .rev()
                .find(|ig| ig.pos == IgPos::Verb && ig.polarity.is_some())
        {
            let negative = ig.polarity == Some(tekir_morph::Polarity::Negative);
            if (polarity == "Neg") == negative {
                score += 6;
            }
        }

        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((index, score));
        }
    }

    best.map(|(index, _)| index)
}

fn upos_compatible(upos: &str, analysis: &MorphAnalysis) -> bool {
    matches!(
        (upos, analysis.pos),
        ("NOUN" | "PROPN" | "PRON" | "NUM", PartOfSpeech::Noun)
            | ("ADJ", PartOfSpeech::Adj | PartOfSpeech::Noun)
            | ("VERB" | "AUX", PartOfSpeech::Verb)
            | ("ADV", PartOfSpeech::Adv | PartOfSpeech::Adj)
            | ("PART" | "AUX", PartOfSpeech::Ques)
            | ("PRON", PartOfSpeech::Pron)
            | ("ADP", PartOfSpeech::Postp)
            | ("CCONJ" | "SCONJ", PartOfSpeech::Conj)
            | ("DET", PartOfSpeech::Det | PartOfSpeech::Pron)
            | ("INTJ", PartOfSpeech::Interj)
            | ("PART", PartOfSpeech::Conj | PartOfSpeech::Adv)
    )
}

fn ud_case(case: &str) -> Option<Case> {
    Some(match case {
        "Nom" => Case::Nominative,
        "Acc" => Case::Accusative,
        "Dat" => Case::Dative,
        "Loc" => Case::Locative,
        "Abl" => Case::Ablative,
        "Gen" => Case::Genitive,
        "Ins" => Case::Instrumental,
        _ => return None,
    })
}
