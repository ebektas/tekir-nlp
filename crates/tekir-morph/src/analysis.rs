use std::sync::Arc;

use serde::Serialize;
use tekir_core::TextTransformResponse;

use crate::{
    analyzer::Analyzer,
    generator::generate_with_boundaries,
    lexicon::Lexicon,
    preference::choose_best_noun_analysis,
    state::{IgPos, InflectionGroup},
};

/// Where an analysis came from: the lexicon, the unknown-word guesser,
/// apostrophe suffixation, or the question particle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisSource {
    Lexicon,
    Guessed,
    Proper,
    Question,
}

impl AnalysisSource {
    fn is_lexicon(&self) -> bool {
        *self == AnalysisSource::Lexicon
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PartOfSpeech {
    Noun,
    Adj,
    Verb,
    Adv,
    Ques,
    Pron,
    Postp,
    Conj,
    Det,
    Interj,
}

/// One analysis of a surface form. `igs` is the full inflectional-group
/// chain, `pos` is the part of speech of its final group, and
/// `lexical_form` renders the whole chain with `^DB` boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MorphAnalysis {
    pub surface: String,
    pub lemma: String,
    pub lexical_form: String,
    pub pos: PartOfSpeech,
    pub igs: Vec<InflectionGroup>,
    #[serde(skip_serializing_if = "AnalysisSource::is_lexicon")]
    pub source: AnalysisSource,
    #[serde(skip)]
    pub(crate) origin: Origin,
}

/// How to recover an analysis's morpheme boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Origin {
    /// Lexicon entry id; boundaries are regenerated on demand.
    Entry(u32),
    /// Boundaries computed when the analysis was built (out-of-lexicon
    /// roots, closed forms).
    Boundaries(Vec<usize>),
}

impl MorphAnalysis {
    /// The last inflectional group, which carries the word's inflection.
    pub fn final_ig(&self) -> &InflectionGroup {
        self.igs.last().expect("analysis has at least one group")
    }

    /// Byte offsets into `surface` where each overt suffix starts
    /// (`kitaplarımızdan` -> `[5, 8, 13]`). Empty for a bare root or when
    /// boundaries cannot be recovered. `lexicon` must be the one that
    /// produced the analysis.
    pub fn morpheme_boundaries(&self, lexicon: &Lexicon) -> Vec<usize> {
        match &self.origin {
            Origin::Boundaries(boundaries) => boundaries.clone(),
            Origin::Entry(id) => {
                let (surface, boundaries) = generate_with_boundaries(lexicon.entry(*id), &self.igs);
                if surface == self.surface {
                    boundaries
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// The surface split into morphemes: `["kitap", "lar", "ımız", "dan"]`.
    pub fn morphemes<'a>(&'a self, lexicon: &Lexicon) -> Vec<&'a str> {
        split_at_boundaries(&self.surface, &self.morpheme_boundaries(lexicon))
    }
}

/// Split `text` at strictly increasing interior byte offsets.
pub fn split_at_boundaries<'a>(text: &'a str, boundaries: &[usize]) -> Vec<&'a str> {
    let mut pieces = Vec::with_capacity(boundaries.len() + 1);
    let mut start = 0;
    for &boundary in boundaries {
        if boundary > start && boundary < text.len() {
            pieces.push(&text[start..boundary]);
            start = boundary;
        }
    }
    pieces.push(&text[start..]);
    pieces
}

impl From<IgPos> for PartOfSpeech {
    fn from(pos: IgPos) -> Self {
        match pos {
            IgPos::Noun => PartOfSpeech::Noun,
            IgPos::Adj => PartOfSpeech::Adj,
            IgPos::Verb => PartOfSpeech::Verb,
            IgPos::Adv => PartOfSpeech::Adv,
            IgPos::Ques => PartOfSpeech::Ques,
            IgPos::Func => PartOfSpeech::Pron,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MorphOutput {
    pub best_analysis: Option<MorphAnalysis>,
    pub analyses: Vec<MorphAnalysis>,
    pub analysis_count: usize,
}

pub type MorphResponse = TextTransformResponse<MorphOutput>;

/// Morphology service holding an immutable, shared lexicon.
///
/// The lexicon is built once and wrapped in an `Arc`, so cloning the service
/// is cheap and concurrent use from batch workers needs no locking.
#[derive(Debug, Clone)]
pub struct MorphService {
    lexicon: Arc<Lexicon>,
}

impl Default for MorphService {
    fn default() -> Self {
        Self::new()
    }
}

impl MorphService {
    /// Service over the full built-in lexicon (seed + Zemberek roots).
    pub fn new() -> Self {
        Self::with_lexicon(Lexicon::full())
    }

    /// Service over the small seed lexicon (fast to build; used by tests).
    pub fn seed() -> Self {
        Self::with_lexicon(Lexicon::seed())
    }

    /// Service over a caller-provided lexicon (for example loaded with
    /// [`Lexicon::from_tsv_path`]).
    pub fn with_lexicon(lexicon: Lexicon) -> Self {
        Self {
            lexicon: Arc::new(lexicon),
        }
    }

    pub fn lexicon(&self) -> &Lexicon {
        &self.lexicon
    }

    pub fn analyze(&self, input: impl Into<String>) -> MorphResponse {
        let input = input.into();
        let analyses = if let Some(apostrophe) = crate::guess::apostrophe_analyses(&input) {
            apostrophe
        } else {
            let mut analyses = Analyzer::new(&self.lexicon).analyze(&input);
            analyses.extend(crate::closed::closed_analyses(&input));
            analyses.extend(crate::guess::question_particle_analyses(&input));
            if analyses.is_empty() {
                analyses = crate::guess::guessed_analyses(&input);
            }
            analyses
        };
        let best_analysis = choose_best_noun_analysis(&analyses);

        TextTransformResponse::new(
            input,
            MorphOutput {
                best_analysis,
                analysis_count: analyses.len(),
                analyses,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::MorphService;

    #[test]
    fn exposes_preferred_candidate_as_best_analysis() {
        let service = MorphService::seed();
        let response = service.analyze("evleri");

        assert_eq!(response.output.analysis_count, 4);
        assert_eq!(
            response
                .output
                .best_analysis
                .as_ref()
                .map(|analysis| analysis.lexical_form.as_str()),
            Some("ev+Noun+A3pl+Pnon+Acc")
        );
    }

    #[test]
    fn exposes_preferred_candidate_for_genitive_ambiguity() {
        let service = MorphService::seed();
        let response = service.analyze("evin");

        assert_eq!(response.output.analysis_count, 2);
        assert_eq!(
            response
                .output
                .best_analysis
                .as_ref()
                .map(|analysis| analysis.lexical_form.as_str()),
            Some("ev+Noun+A3sg+Pnon+Gen")
        );
    }

    #[test]
    fn keeps_full_candidate_set_when_best_analysis_is_present() {
        let service = MorphService::seed();
        let response = service.analyze("evleri");

        assert!(response.output.best_analysis.is_some());
        assert_eq!(
            response.output.analysis_count,
            response.output.analyses.len()
        );
    }

    #[test]
    fn service_clones_share_one_lexicon() {
        let service = MorphService::seed();
        let clone = service.clone();

        assert!(std::ptr::eq(service.lexicon(), clone.lexicon()));
    }

    fn split(service: &MorphService, surface: &str, gloss: &str) -> String {
        let response = service.analyze(surface);
        let analysis = response
            .output
            .analyses
            .iter()
            .find(|analysis| analysis.lexical_form == gloss)
            .unwrap_or_else(|| panic!("{surface} should have {gloss}"));
        analysis.morphemes(service.lexicon()).join("|")
    }

    #[test]
    fn splits_surfaces_into_morphemes() {
        let service = MorphService::seed();
        let cases = [
            (
                "kitaplarımızdan",
                "kitap+Noun+A3pl+P1pl+Abl",
                "kitap|lar|ımız|dan",
            ),
            ("kitaba", "kitap+Noun+A3sg+Pnon+Dat", "kitab|a"),
            ("ev", "ev+Noun+A3sg+Pnon+Nom", "ev"),
            (
                "evdekilerinki",
                "ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Gen^DB+Adj+Rel",
                "ev|de|ki|ler|in|ki",
            ),
            ("yapmıyor", "yap+Verb+Neg+Prog1+A3sg", "yap|m|ıyor"),
            (
                "Ankara'dakiler",
                "Ankara+Noun+Prop+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Nom",
                "Ankara|'|da|ki|ler",
            ),
            (
                "benimkini",
                "ben+Pron+Pers+A1sg+Gen^DB+Adj+Rel^DB+Noun+Zero+A3sg+Pnon+Acc",
                "benim|ki|ni",
            ),
            ("miydi", "mi+Ques^DB+Verb+Zero+Past+A3sg", "mi|ydi"),
            ("xyzlerden", "xyz+Noun+Unk+A3pl+Pnon+Abl", "xyz|ler|den"),
        ];
        for (surface, gloss, expected) in cases {
            assert_eq!(split(&service, surface, gloss), expected, "{surface}");
        }
    }

    #[test]
    fn every_analysis_has_exact_boundaries() {
        use crate::{
            analysis::Origin,
            generator::{generate_noun_surface, generate_with_boundaries},
            gold::GOLD_DERIVED_SURFACES,
            state::noun_paths,
        };

        let service = MorphService::seed();
        let lexicon = service.lexicon();
        let mut surfaces: Vec<String> = GOLD_DERIVED_SURFACES
            .iter()
            .map(|s| s.to_string())
            .collect();
        for entry in lexicon.entries() {
            for path in noun_paths() {
                surfaces.push(generate_noun_surface(entry, &path));
            }
        }
        surfaces.extend(
            ["benimkini", "ondakiler", "miydi", "Ankara'ya", "xyzlerden"].map(String::from),
        );

        for surface in &surfaces {
            for analysis in service.analyze(surface.as_str()).output.analyses {
                let boundaries = analysis.morpheme_boundaries(lexicon);
                if let Origin::Entry(id) = analysis.origin {
                    let (generated, _) = generate_with_boundaries(lexicon.entry(id), &analysis.igs);
                    assert_eq!(generated, analysis.surface, "{}", analysis.lexical_form);
                }
                let mut previous = 0;
                for &boundary in &boundaries {
                    assert!(
                        boundary > previous
                            && boundary < surface.len()
                            && surface.is_char_boundary(boundary),
                        "bad boundaries {boundaries:?} for {surface} ({})",
                        analysis.lexical_form
                    );
                    previous = boundary;
                }
                assert_eq!(analysis.morphemes(lexicon).concat(), *surface);
            }
        }
    }
}
