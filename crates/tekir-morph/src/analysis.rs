use std::sync::Arc;

use serde::Serialize;
use tekir_core::{TextRequest, TextTransformResponse, TextTransformer};

use crate::{
    analyzer::NounAnalyzer,
    lexical::LexicalNounForm,
    lexicon::Lexicon,
    preference::choose_best_noun_analysis,
    state::{Case, InflectionGroup, NounMorpheme, Number, Possessive},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MorphMode {
    NounBaseline,
}

pub type MorphRequest = TextRequest<MorphMode>;

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
/// chain; `pos`, `number`, `possessive`, `case`, and `morphemes` describe
/// the final group, and `lexical_form` renders the whole chain with `^DB`
/// boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MorphAnalysis {
    pub surface: String,
    pub lemma: String,
    pub lexical: LexicalNounForm,
    pub lexical_form: String,
    pub pos: PartOfSpeech,
    pub number: Number,
    pub possessive: Possessive,
    pub case: Case,
    pub morphemes: Vec<NounMorpheme>,
    pub igs: Vec<InflectionGroup>,
    #[serde(skip_serializing_if = "AnalysisSource::is_lexicon")]
    pub source: AnalysisSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MorphOutput {
    pub best_analysis: Option<MorphAnalysis>,
    pub analyses: Vec<MorphAnalysis>,
    pub mode: MorphMode,
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
    /// Service over the built-in seed lexicon.
    pub fn new() -> Self {
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

    pub fn analyze(&self, input: impl Into<String>, mode: MorphMode) -> MorphResponse {
        self.execute(MorphRequest::new(input, mode))
    }
}

impl TextTransformer for MorphService {
    type Options = MorphMode;
    type Output = MorphOutput;

    fn execute(&self, request: MorphRequest) -> MorphResponse {
        let analyses = match request.options {
            MorphMode::NounBaseline => {
                if let Some(apostrophe) = crate::guess::apostrophe_analyses(&request.input) {
                    apostrophe
                } else {
                    let mut analyses = NounAnalyzer::new(&self.lexicon).analyze(&request.input);
                    analyses.extend(crate::closed::closed_analyses(&request.input));
                    analyses.extend(crate::guess::question_particle_analyses(&request.input));
                    if analyses.is_empty() {
                        analyses = crate::guess::guessed_analyses(&request.input);
                    }
                    analyses
                }
            }
        };
        let best_analysis = choose_best_noun_analysis(&analyses);

        TextTransformResponse::new(
            request.input,
            MorphOutput {
                best_analysis,
                analysis_count: analyses.len(),
                analyses,
                mode: request.options,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{MorphMode, MorphService};

    #[test]
    fn exposes_preferred_candidate_as_best_analysis() {
        let service = MorphService::new();
        let response = service.analyze("evleri", MorphMode::NounBaseline);

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
        let service = MorphService::new();
        let response = service.analyze("evin", MorphMode::NounBaseline);

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
        let service = MorphService::new();
        let response = service.analyze("evleri", MorphMode::NounBaseline);

        assert!(response.output.best_analysis.is_some());
        assert_eq!(
            response.output.analysis_count,
            response.output.analyses.len()
        );
    }

    #[test]
    fn service_clones_share_one_lexicon() {
        let service = MorphService::new();
        let clone = service.clone();

        assert!(std::ptr::eq(service.lexicon(), clone.lexicon()));
    }
}
