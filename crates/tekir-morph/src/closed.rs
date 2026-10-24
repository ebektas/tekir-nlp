//! Closed-class function words: full inflected pronoun forms (`bana`,
//! `onunla`, `bunun`) and invariant postpositions, conjunctions,
//! determiners, adverbs, and interjections — loaded from
//! `data/closed.tsv`. Each form also accepts copular tails (`kimdi`,
//! `gibiydi`) and, where marked, the relative `-ki` with its full
//! continuation (`benimki`, `benimkini`, `ondakiler`).

use std::sync::LazyLock;

use crate::{
    analysis::{AnalysisSource, MorphAnalysis, PartOfSpeech},
    analyzer::{RawAnalysis, analyze_copular_tail_into, analyze_ki_tail_into},
    lexical::{LexicalNounForm, gloss_from_igs},
    rules::PhonoContext,
    state::{Case, InflectionGroup, Number, Possessive},
};

struct ClosedForm {
    surface: &'static str,
    lemma: &'static str,
    chain: &'static str,
    relative_ki: bool,
}

const CLOSED_TSV: &str = include_str!("../data/closed.tsv");

static TABLE: LazyLock<Vec<ClosedForm>> = LazyLock::new(|| {
    CLOSED_TSV
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut columns = line.split('\t');
            let surface = columns.next().expect("closed form surface");
            let lemma = columns.next().expect("closed form lemma");
            let chain = columns.next().expect("closed form chain");
            let relative_ki = columns.next() == Some("ki");
            ClosedForm {
                surface,
                lemma,
                chain,
                relative_ki,
            }
        })
        .collect()
});

fn chain_pos(chain: &str) -> PartOfSpeech {
    if chain.starts_with("+Pron") {
        PartOfSpeech::Pron
    } else if chain.starts_with("+Postp") {
        PartOfSpeech::Postp
    } else if chain.starts_with("+Conj") {
        PartOfSpeech::Conj
    } else if chain.starts_with("+Det") {
        PartOfSpeech::Det
    } else if chain.starts_with("+Interj") {
        PartOfSpeech::Interj
    } else {
        PartOfSpeech::Adv
    }
}

/// Closed-class analyses for `surface`, appended to whatever the open
/// lexicon finds.
pub fn closed_analyses(surface: &str) -> Vec<MorphAnalysis> {
    let mut analyses = Vec::new();
    let mut raw: Vec<RawAnalysis> = Vec::new();

    for form in TABLE.iter() {
        if surface == form.surface {
            analyses.push(build(
                surface,
                form,
                form.chain.to_string(),
                chain_pos(form.chain),
                1,
            ));
            continue;
        }
        let Some(tail) = surface.strip_prefix(form.surface) else {
            continue;
        };
        let context = PhonoContext::for_stem(form.surface, None);

        raw.clear();
        analyze_copular_tail_into(context, tail, InflectionGroup::func(), &mut raw);
        if form.relative_ki {
            analyze_ki_tail_into(context, tail, InflectionGroup::func(), &mut raw);
        }
        for candidate in &raw {
            let igs = candidate.igs();
            let gloss = format!("{}{}", form.chain, suffix_gloss(igs));
            let pos = final_pos(igs);
            if !analyses.iter().any(|existing: &MorphAnalysis| {
                existing.lemma == form.lemma && existing.lexical_form.ends_with(&gloss)
            }) {
                analyses.push(build(surface, form, gloss, pos, igs.len()));
            }
        }
    }

    analyses
}

/// Render the non-root groups of a chain (`^DB+Verb+Zero+Past+A3sg`).
fn suffix_gloss(igs: &[InflectionGroup]) -> String {
    let full = gloss_from_igs("", igs);
    full["+Func".len()..].to_string()
}

fn final_pos(igs: &[InflectionGroup]) -> PartOfSpeech {
    use crate::state::IgPos;
    match igs.last().map(|ig| ig.pos) {
        Some(IgPos::Noun) => PartOfSpeech::Noun,
        Some(IgPos::Adj) => PartOfSpeech::Adj,
        Some(IgPos::Verb) => PartOfSpeech::Verb,
        Some(IgPos::Adv) => PartOfSpeech::Adv,
        _ => PartOfSpeech::Pron,
    }
}

fn build(
    surface: &str,
    form: &ClosedForm,
    chain: String,
    pos: PartOfSpeech,
    ig_count: usize,
) -> MorphAnalysis {
    let mut igs = vec![InflectionGroup::func()];
    igs.resize(ig_count.max(1), InflectionGroup::func());

    MorphAnalysis {
        surface: surface.to_string(),
        lemma: form.lemma.to_string(),
        lexical: LexicalNounForm {
            lemma: form.lemma.to_string(),
            lexical_root: form.surface.to_string(),
            number: Number::Singular,
            possessive: Possessive::None,
            case: Case::Nominative,
            morphemes: Vec::new(),
        },
        lexical_form: format!("{}{}", form.lemma, chain),
        pos,
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Nominative,
        morphemes: Vec::new(),
        igs,
        source: AnalysisSource::Lexicon,
    }
}

#[cfg(test)]
mod tests {
    use super::closed_analyses;

    fn glosses(surface: &str) -> Vec<String> {
        closed_analyses(surface)
            .into_iter()
            .map(|analysis| analysis.lexical_form)
            .collect()
    }

    #[test]
    fn analyzes_irregular_pronoun_forms() {
        assert!(glosses("bana").contains(&"ben+Pron+Pers+A1sg+Dat".to_string()));
        assert!(glosses("onunla").contains(&"o+Pron+Pers+A3sg+Ins".to_string()));
        assert!(glosses("bunların").contains(&"bu+Pron+Dem+A3pl+Gen".to_string()));
    }

    #[test]
    fn analyzes_function_words_and_copular_tails() {
        assert!(glosses("için").contains(&"için+Postp+PCGen".to_string()));
        assert!(glosses("ve").contains(&"ve+Conj".to_string()));
        assert!(
            glosses("gibiydi").contains(&"gibi+Postp+PCNom^DB+Verb+Zero+Past+A3sg".to_string())
        );
        assert!(
            glosses("kimdi").contains(&"kim+Pron+Ques+A3sg+Nom^DB+Verb+Zero+Past+A3sg".to_string())
        );
    }

    #[test]
    fn analyzes_relative_ki_continuations() {
        assert!(glosses("benimki").contains(&"ben+Pron+Pers+A1sg+Gen^DB+Adj+Rel".to_string()));
        assert!(
            glosses("benimkini").contains(
                &"ben+Pron+Pers+A1sg+Gen^DB+Adj+Rel^DB+Noun+Zero+A3sg+Pnon+Acc".to_string()
            )
        );
        assert!(
            glosses("ondakiler").contains(
                &"o+Pron+Pers+A3sg+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Nom".to_string()
            )
        );
    }
}
