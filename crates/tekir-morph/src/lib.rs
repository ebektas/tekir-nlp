pub mod analysis;
pub mod analyzer;
pub mod batch;
pub mod closed;
pub mod generator;
#[cfg(test)]
mod gold;
pub mod guess;
pub mod lexical;
pub mod lexicon;
pub mod preference;
pub mod rules;
pub mod state;

pub use analysis::{
    AnalysisSource, MorphAnalysis, MorphOutput, MorphResponse, MorphService, PartOfSpeech,
};
pub use analyzer::{Analyzer, MAX_IGS, RawAnalysis, analyze_word};
pub use batch::BatchAnalysis;
pub use generator::generate_from_igs;
pub use lexicon::{Lexicon, LexiconEntry, LexiconError, StemAlternant};
pub use state::{
    Agreement, Case, CopulaTense, Derivation, IgPos, InflectionGroup, NounMorpheme, NounPath,
    Number, Polarity, Possessive, VerbTense,
};
