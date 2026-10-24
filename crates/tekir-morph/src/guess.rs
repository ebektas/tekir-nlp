//! Robust-text analyses beyond the lexicon: the unknown-word guesser,
//! apostrophe suffixation for proper nouns/numbers/acronyms
//! (`Ankara'daki`, `TBMM'ye`, `1990'larda`), and the detached question
//! particle (`mi`, `miydi`, `musunuz`).
//!
//! All three reuse the analyzer's tail primitives, so the full nominal and
//! copular machinery (cases, `-ki`, copulas) applies to the suffix side.

use crate::{
    analysis::{AnalysisSource, MorphAnalysis, Origin},
    analyzer::{RawAnalysis, analyze_copular_tail_into, analyze_nominal_tail_into},
    generator::generate_tail_with_boundaries,
    lexical::gloss_from_igs,
    rules::{PhonoContext, last_vowel},
    state::InflectionGroup,
};

const QUESTION_PARTICLES: [&str; 4] = ["mı", "mi", "mu", "mü"];

/// Analyses for a token containing an apostrophe: the part before the last
/// apostrophe is an opaque proper stem, the part after it a nominal suffix
/// sequence. Returns `None` when the input has no apostrophe.
pub fn apostrophe_analyses(surface: &str) -> Option<Vec<MorphAnalysis>> {
    let (index, apostrophe) = surface
        .char_indices()
        .rfind(|(_, ch)| matches!(ch, '\'' | '’'))?;
    let stem = &surface[..index];
    let tail = &surface[index + apostrophe.len_utf8()..];
    if stem.is_empty() {
        return Some(Vec::new());
    }

    let mut analyses = Vec::new();
    let mut raw = Vec::new();

    if tail.is_empty() {
        let igs = [InflectionGroup::noun(
            None,
            crate::state::Number::Singular,
            crate::state::Possessive::None,
            crate::state::Case::Nominative,
        )];
        push_unique(
            &mut analyses,
            tail_analysis(surface, stem, &igs, AnalysisSource::Proper, Vec::new()),
        );
        return Some(analyses);
    }

    for context in stem_contexts(stem) {
        raw.clear();
        analyze_nominal_tail_into(context, tail, &mut raw);
        collect(
            &mut analyses,
            surface,
            stem,
            AnalysisSource::Proper,
            &raw,
            context,
            tail,
        );
    }

    Some(analyses)
}

/// Question-particle analyses (`mi`, `miydi`, `musunuz`), appended to
/// whatever the lexicon finds for the same token.
pub fn question_particle_analyses(surface: &str) -> Vec<MorphAnalysis> {
    let mut analyses = Vec::new();
    let mut raw = Vec::new();

    for particle in QUESTION_PARTICLES {
        let Some(tail) = surface.strip_prefix(particle) else {
            continue;
        };
        let context = PhonoContext::for_stem(particle, None);

        if tail.is_empty() {
            let igs = [InflectionGroup::ques()];
            push_unique(
                &mut analyses,
                tail_analysis(
                    surface,
                    particle,
                    &igs,
                    AnalysisSource::Question,
                    Vec::new(),
                ),
            );
            continue;
        }

        raw.clear();
        analyze_copular_tail_into(context, tail, InflectionGroup::ques(), &mut raw);
        collect(
            &mut analyses,
            surface,
            particle,
            AnalysisSource::Question,
            &raw,
            context,
            tail,
        );
    }

    analyses
}

/// Unknown-word fallback: treat every prefix of at least two characters as
/// a potential nominal root and parse the rest as suffixes. Only used when
/// the lexicon finds nothing.
pub fn guessed_analyses(surface: &str) -> Vec<MorphAnalysis> {
    if surface.is_empty() || !surface.chars().all(char::is_alphabetic) {
        return Vec::new();
    }

    let mut analyses = Vec::new();
    let mut raw = Vec::new();

    let boundaries = surface
        .char_indices()
        .map(|(offset, _)| offset)
        .skip(2)
        .chain(std::iter::once(surface.len()));

    for end in boundaries {
        let root = &surface[..end];
        let tail = &surface[end..];
        let context = PhonoContext::for_stem(root, None);

        raw.clear();
        analyze_nominal_tail_into(context, tail, &mut raw);
        collect(
            &mut analyses,
            surface,
            root,
            AnalysisSource::Guessed,
            &raw,
            context,
            tail,
        );
    }

    analyses
}

/// Harmony contexts to try for an opaque stem: its own last vowel when it
/// has one, otherwise every harmony class crossed with vowel-final, voiced
/// and voiceless consonant-final endings (`6'ya`, `1990'da`, `5'te`).
fn stem_contexts(stem: &str) -> Vec<PhonoContext> {
    if last_vowel(stem).is_some() {
        return vec![PhonoContext::for_stem(stem, None)];
    }

    let mut contexts = Vec::with_capacity(12);
    for seed in ['a', 'e', 'o', 'ö'] {
        for last_char in [seed, 'n', 't'] {
            contexts.push(PhonoContext {
                last_char,
                last_vowel: Some(seed),
            });
        }
    }
    contexts
}

fn collect(
    analyses: &mut Vec<MorphAnalysis>,
    surface: &str,
    lemma: &str,
    source: AnalysisSource,
    raw: &[RawAnalysis],
    context: PhonoContext,
    tail: &str,
) {
    // Everything before the tail (root, plus the apostrophe for proper
    // stems) is one piece; the apostrophe gets a piece of its own.
    let head = &surface[..surface.len() - tail.len()];
    for candidate in raw {
        let (generated, mut boundaries) =
            generate_tail_with_boundaries(head, context, candidate.igs());
        if generated != surface {
            boundaries = vec![head.len()];
        }
        if head.len() > lemma.len() {
            boundaries.insert(0, lemma.len());
        }
        push_unique(
            analyses,
            tail_analysis(surface, lemma, candidate.igs(), source, boundaries),
        );
    }
}

fn push_unique(analyses: &mut Vec<MorphAnalysis>, analysis: MorphAnalysis) {
    if !analyses
        .iter()
        .any(|existing| existing.lexical_form == analysis.lexical_form)
    {
        analyses.push(analysis);
    }
}

/// Build a full analysis for an out-of-lexicon root. `marker` (`+Prop`,
/// `+Unk`) is inserted after the root POS tag.
fn tail_analysis(
    surface: &str,
    lemma: &str,
    igs: &[InflectionGroup],
    source: AnalysisSource,
    boundaries: Vec<usize>,
) -> MorphAnalysis {
    let final_ig = *igs.last().expect("analysis has at least one group");
    let mut lexical_form = gloss_from_igs(lemma, igs);
    let marker = match source {
        AnalysisSource::Proper => Some("+Prop"),
        AnalysisSource::Guessed => Some("+Unk"),
        _ => None,
    };
    if let Some(marker) = marker {
        lexical_form.insert_str(lemma.len() + "+Noun".len(), marker);
    }

    MorphAnalysis {
        surface: surface.to_string(),
        lemma: lemma.to_string(),
        lexical_form,
        pos: final_ig.pos.into(),
        igs: igs.to_vec(),
        source,
        origin: Origin::Boundaries(boundaries),
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::AnalysisSource;
    use crate::guess::{apostrophe_analyses, guessed_analyses, question_particle_analyses};

    fn glosses(analyses: &[crate::MorphAnalysis]) -> Vec<&str> {
        analyses
            .iter()
            .map(|analysis| analysis.lexical_form.as_str())
            .collect()
    }

    #[test]
    fn analyzes_apostrophe_suffixation() {
        let analyses = apostrophe_analyses("Ankara'daki").expect("apostrophe present");
        assert!(glosses(&analyses).contains(&"Ankara+Noun+Prop+A3sg+Pnon+Loc^DB+Adj+Rel"));
        assert!(analyses.iter().all(|a| a.source == AnalysisSource::Proper));

        let analyses = apostrophe_analyses("1990'larda").expect("apostrophe present");
        assert!(glosses(&analyses).contains(&"1990+Noun+Prop+A3pl+Pnon+Loc"));

        let analyses = apostrophe_analyses("5'te").expect("apostrophe present");
        assert!(glosses(&analyses).contains(&"5+Noun+Prop+A3sg+Pnon+Loc"));

        let analyses = apostrophe_analyses("TBMM'ye").expect("apostrophe present");
        assert!(glosses(&analyses).contains(&"TBMM+Noun+Prop+A3sg+Pnon+Dat"));

        assert!(apostrophe_analyses("evler").is_none());
    }

    #[test]
    fn analyzes_question_particles() {
        assert!(glosses(&question_particle_analyses("mi")).contains(&"mi+Ques"));
        assert!(
            glosses(&question_particle_analyses("miydi"))
                .contains(&"mi+Ques^DB+Verb+Zero+Past+A3sg")
        );
        assert!(
            glosses(&question_particle_analyses("musunuz"))
                .contains(&"mu+Ques^DB+Verb+Zero+Pres+A2pl")
        );
        assert!(question_particle_analyses("milyon").is_empty());
        assert!(question_particle_analyses("masa").is_empty());
    }

    #[test]
    fn guesses_unknown_nominals() {
        let analyses = guessed_analyses("zorbuklardan");
        assert!(glosses(&analyses).contains(&"zorbuk+Noun+Unk+A3pl+Pnon+Abl"));
        assert!(analyses.iter().all(|a| a.source == AnalysisSource::Guessed));

        assert!(guessed_analyses("1234").is_empty());
    }
}
