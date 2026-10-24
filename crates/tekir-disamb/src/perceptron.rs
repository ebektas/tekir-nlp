//! Averaged perceptron over hashed analysis features, in the lineage of
//! Sak, Güngör & Saraçlar (2007) and Zemberek's resolver.
//!
//! Features describe the candidate analysis (root, tag chain, inflectional
//! groups) and the tag chains of the two preceding choices. Training uses
//! teacher-forced local updates (gold left context); inference decodes with
//! a small beam. Everything is deterministic: fixed iteration order, FNV
//! hashing, no randomness.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

use tekir_morph::MorphAnalysis;

pub const BEAM_WIDTH: usize = 8;
const PRUNE_THRESHOLD: f32 = 1e-4;

/// Tag chain of an analysis: the gloss without its lemma (`+Noun+A3sg+...`).
pub fn tag_of(analysis: &MorphAnalysis) -> &str {
    &analysis.lexical_form[analysis.lemma.len()..]
}

/// Boundary tag used before the sentence start and for tokens without
/// analyses (punctuation).
pub const BOUNDARY_TAG: &str = "<s>";

fn fnv(kind: u8, parts: &[&str]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325 ^ u64::from(kind);
    let mut push = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for (index, part) in parts.iter().enumerate() {
        push(0xff - index as u8);
        for byte in part.bytes() {
            push(byte);
        }
    }
    hash
}

/// Hashed features of choosing `analysis` after the two previous tag chains.
pub fn features(prev2: &str, prev1: &str, analysis: &MorphAnalysis, out: &mut Vec<u64>) {
    out.clear();
    let tag = tag_of(analysis);
    let root = &analysis.lemma;

    out.push(fnv(1, &[tag]));
    out.push(fnv(2, &[root]));
    out.push(fnv(3, &[root, tag]));
    out.push(fnv(4, &[prev1, tag]));
    out.push(fnv(5, &[prev2, prev1, tag]));
    for segment in tag.split("^DB") {
        out.push(fnv(6, &[segment]));
    }
    let ig_count = [analysis.igs.len().to_string()];
    out.push(fnv(7, &[ig_count[0].as_str(), tag]));
    out.push(fnv(8, &[prev1, root]));
}

/// Averaged perceptron weights.
#[derive(Debug, Default, Clone)]
pub struct Model {
    weights: HashMap<u64, f32>,
}

impl Model {
    pub fn len(&self) -> usize {
        self.weights.len()
    }

    pub fn is_empty(&self) -> bool {
        self.weights.is_empty()
    }

    pub fn score(&self, feature_ids: &[u64]) -> f64 {
        feature_ids
            .iter()
            .map(|id| f64::from(self.weights.get(id).copied().unwrap_or(0.0)))
            .sum()
    }

    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
        file.write_all(b"TKD1")?;
        let kept: Vec<_> = self
            .weights
            .iter()
            .filter(|(_, weight)| weight.abs() >= PRUNE_THRESHOLD)
            .collect();
        file.write_all(&(kept.len() as u64).to_le_bytes())?;
        let mut sorted = kept;
        sorted.sort_by_key(|(id, _)| **id);
        for (id, weight) in sorted {
            file.write_all(&id.to_le_bytes())?;
            file.write_all(&weight.to_le_bytes())?;
        }
        Ok(())
    }

    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Self::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))
    }

    pub fn from_bytes(bytes: &[u8]) -> std::io::Result<Self> {
        Self::from_reader(bytes)
    }

    pub fn from_reader(mut file: impl Read) -> std::io::Result<Self> {
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if &magic != b"TKD1" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "not a tekir-disamb model",
            ));
        }
        let mut count_bytes = [0u8; 8];
        file.read_exact(&mut count_bytes)?;
        let count = u64::from_le_bytes(count_bytes) as usize;
        let mut weights = HashMap::with_capacity(count);
        for _ in 0..count {
            let mut id = [0u8; 8];
            let mut weight = [0u8; 4];
            file.read_exact(&mut id)?;
            file.read_exact(&mut weight)?;
            weights.insert(u64::from_le_bytes(id), f32::from_le_bytes(weight));
        }
        Ok(Self { weights })
    }
}

/// Trainer with lazy averaging.
#[derive(Debug, Default)]
pub struct Trainer {
    weights: HashMap<u64, f64>,
    totals: HashMap<u64, f64>,
    timestamps: HashMap<u64, u64>,
    steps: u64,
}

impl Trainer {
    pub fn raw_score(&self, feature_ids: &[u64]) -> f64 {
        feature_ids
            .iter()
            .map(|id| self.weights.get(id).copied().unwrap_or(0.0))
            .sum()
    }

    pub fn step(&mut self) {
        self.steps += 1;
    }

    pub fn update(&mut self, feature_ids: &[u64], delta: f64) {
        for &id in feature_ids {
            let timestamp = self.timestamps.entry(id).or_insert(0);
            let weight = self.weights.entry(id).or_insert(0.0);
            *self.totals.entry(id).or_insert(0.0) += *weight * (self.steps - *timestamp) as f64;
            *weight += delta;
            *timestamp = self.steps;
        }
    }

    /// Finalize into the averaged model.
    pub fn into_model(mut self) -> Model {
        let steps = self.steps.max(1) as f64;
        let mut weights = HashMap::with_capacity(self.weights.len());
        for (id, weight) in &self.weights {
            let total = self.totals.remove(id).unwrap_or(0.0)
                + weight * (self.steps - self.timestamps.get(id).copied().unwrap_or(0)) as f64;
            let averaged = (total / steps) as f32;
            if averaged.abs() >= PRUNE_THRESHOLD {
                weights.insert(*id, averaged);
            }
        }
        Model { weights }
    }
}

/// Beam-decode the best analysis index per token. Tokens with no candidates
/// yield `None` and contribute the boundary tag as context.
pub fn decode(model: &Model, sentence: &[Vec<MorphAnalysis>]) -> Vec<Option<usize>> {
    #[derive(Clone)]
    struct Item {
        prev: usize,
        choice: Option<usize>,
        score: f64,
        tag1: String,
        tag2: String,
    }

    let mut beams: Vec<Vec<Item>> = Vec::with_capacity(sentence.len() + 1);
    beams.push(vec![Item {
        prev: 0,
        choice: None,
        score: 0.0,
        tag1: BOUNDARY_TAG.to_string(),
        tag2: BOUNDARY_TAG.to_string(),
    }]);

    let mut feature_ids = Vec::with_capacity(16);
    for candidates in sentence {
        let previous = beams.last().expect("beam is never empty");
        let mut next: Vec<Item> = Vec::new();

        for (prev_index, item) in previous.iter().enumerate() {
            if candidates.is_empty() {
                next.push(Item {
                    prev: prev_index,
                    choice: None,
                    score: item.score,
                    tag1: BOUNDARY_TAG.to_string(),
                    tag2: item.tag1.clone(),
                });
                continue;
            }
            for (candidate_index, analysis) in candidates.iter().enumerate() {
                features(&item.tag2, &item.tag1, analysis, &mut feature_ids);
                next.push(Item {
                    prev: prev_index,
                    choice: Some(candidate_index),
                    score: item.score + model.score(&feature_ids),
                    tag1: tag_of(analysis).to_string(),
                    tag2: item.tag1.clone(),
                });
            }
        }

        next.sort_by(|a, b| b.score.total_cmp(&a.score));
        next.truncate(BEAM_WIDTH);
        beams.push(next);
    }

    // Follow back-pointers from the best final item.
    let mut choices = vec![None; sentence.len()];
    let mut index = 0;
    for position in (1..beams.len()).rev() {
        let item = &beams[position][index];
        choices[position - 1] = item.choice;
        index = item.prev;
    }
    choices
}

#[cfg(test)]
mod tests {
    #[test]
    fn feature_hashing_is_deterministic() {
        assert_eq!(super::fnv(1, &["a", "b"]), super::fnv(1, &["a", "b"]));
        assert_ne!(super::fnv(1, &["a", "b"]), super::fnv(2, &["a", "b"]));
        assert_ne!(super::fnv(1, &["ab"]), super::fnv(1, &["a", "b"]));
    }
}
