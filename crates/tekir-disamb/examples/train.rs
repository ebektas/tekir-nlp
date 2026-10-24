//! Train and evaluate the averaged-perceptron disambiguator on UD
//! treebank files.
//!
//! ```bash
//! cargo run --release -p tekir-disamb --example train -- \
//!     --train kenet-train.conllu --eval kenet-test.conllu \
//!     --model kenet.tkd --epochs 5 \
//!     --lexicon crates/tekir-morph/data/zemberek-nouns.tsv [...]
//! ```

use std::process::ExitCode;

use tekir_disamb::conllu::{self, turkish_lowercase};
use tekir_disamb::{Trainer, decode, features, mapping, perceptron::BOUNDARY_TAG, tag_of};
use tekir_morph::{Lexicon, MorphAnalysis, MorphService};

struct PreparedToken {
    candidates: Vec<MorphAnalysis>,
    gold: Option<usize>,
    baseline: Option<usize>,
}

fn main() -> ExitCode {
    let mut train_files = Vec::new();
    let mut eval_files = Vec::new();
    let mut lexicon_files = Vec::new();
    let mut model_path = String::from("kenet.tkd");
    let mut epochs = 5usize;

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().expect("flag needs a value");
        match flag.as_str() {
            "--train" => train_files.push(value()),
            "--eval" => eval_files.push(value()),
            "--lexicon" => lexicon_files.push(value()),
            "--model" => model_path = value(),
            "--epochs" => epochs = value().parse().expect("epochs must be a number"),
            other => {
                eprintln!("unknown flag {other}");
                return ExitCode::FAILURE;
            }
        }
    }
    if train_files.is_empty() {
        eprintln!("--train is required");
        return ExitCode::FAILURE;
    }

    let service = if lexicon_files.is_empty() {
        MorphService::new()
    } else {
        MorphService::with_lexicon(
            Lexicon::from_tsv_paths(&lexicon_files).expect("lexicon files must parse"),
        )
    };

    let prepare = |files: &[String]| -> Vec<Vec<PreparedToken>> {
        let mut sentences = Vec::new();
        for file in files {
            let text = std::fs::read_to_string(file).expect("treebank file must be readable");
            for sentence in conllu::parse(&text) {
                let mut prepared = Vec::with_capacity(sentence.tokens.len());
                for token in &sentence.tokens {
                    let surface = if token.form.contains('\'') || token.form.contains('’') {
                        token.form.clone()
                    } else {
                        turkish_lowercase(&token.form)
                    };
                    let response = service.analyze(surface);
                    let candidates = response.output.analyses;
                    let gold = mapping::gold_index(token, &candidates);
                    let baseline = response.output.best_analysis.as_ref().and_then(|best| {
                        candidates
                            .iter()
                            .position(|analysis| analysis.lexical_form == best.lexical_form)
                    });
                    prepared.push(PreparedToken {
                        candidates,
                        gold,
                        baseline,
                    });
                }
                sentences.push(prepared);
            }
        }
        sentences
    };

    eprintln!("analyzing training data...");
    let train = prepare(&train_files);
    eprintln!("analyzing evaluation data...");
    let eval = prepare(&eval_files);

    report_data_stats("train", &train);
    report_data_stats("eval", &eval);

    // Teacher-forced training: score candidates under the gold left
    // context; update toward gold when the argmax disagrees.
    let mut trainer = Trainer::default();
    let mut feature_ids = Vec::with_capacity(16);
    for epoch in 0..epochs {
        let mut mistakes = 0usize;
        let mut seen = 0usize;
        for sentence in &train {
            let mut tag1 = BOUNDARY_TAG.to_string();
            let mut tag2 = BOUNDARY_TAG.to_string();
            for token in sentence {
                let Some(gold) = token.gold else {
                    tag2 = std::mem::replace(&mut tag1, BOUNDARY_TAG.to_string());
                    continue;
                };
                trainer.step();
                seen += 1;

                let mut best: Option<(usize, f64)> = None;
                for (index, analysis) in token.candidates.iter().enumerate() {
                    features(&tag2, &tag1, analysis, &mut feature_ids);
                    let score = trainer.raw_score(&feature_ids);
                    if best.is_none_or(|(_, best_score)| score > best_score) {
                        best = Some((index, score));
                    }
                }
                let predicted = best.map(|(index, _)| index).unwrap_or(gold);
                if predicted != gold {
                    mistakes += 1;
                    features(&tag2, &tag1, &token.candidates[gold], &mut feature_ids);
                    trainer.update(&feature_ids, 1.0);
                    features(&tag2, &tag1, &token.candidates[predicted], &mut feature_ids);
                    trainer.update(&feature_ids, -1.0);
                }

                tag2 = std::mem::replace(&mut tag1, tag_of(&token.candidates[gold]).to_string());
            }
        }
        eprintln!(
            "epoch {}: {mistakes} mistakes / {seen} gold tokens ({:.2}%)",
            epoch + 1,
            100.0 * mistakes as f64 / seen.max(1) as f64
        );
    }

    let model = trainer.into_model();
    eprintln!("model features: {}", model.len());
    model.save(&model_path).expect("model must be writable");
    eprintln!("saved {model_path}");

    // Evaluation: beam decoding with predicted context.
    for (name, data) in [("train", &train), ("eval", &eval)] {
        if data.is_empty() {
            continue;
        }
        let mut gold_tokens = 0usize;
        let mut baseline_correct = 0usize;
        let mut model_correct = 0usize;
        for sentence in data.iter() {
            let candidate_lists: Vec<Vec<MorphAnalysis>> = sentence
                .iter()
                .map(|token| token.candidates.clone())
                .collect();
            let choices = decode(&model, &candidate_lists);
            for (token, choice) in sentence.iter().zip(choices) {
                let Some(gold) = token.gold else { continue };
                gold_tokens += 1;
                if token.baseline == Some(gold) {
                    baseline_correct += 1;
                }
                if choice == Some(gold) {
                    model_correct += 1;
                }
            }
        }
        eprintln!(
            "{name}: baseline {:.2}%  perceptron {:.2}%  ({gold_tokens} gold tokens)",
            100.0 * baseline_correct as f64 / gold_tokens.max(1) as f64,
            100.0 * model_correct as f64 / gold_tokens.max(1) as f64,
        );
    }

    ExitCode::SUCCESS
}

fn report_data_stats(name: &str, data: &[Vec<PreparedToken>]) {
    let mut tokens = 0usize;
    let mut alphabetic = 0usize;
    let mut covered = 0usize;
    let mut mapped = 0usize;
    for sentence in data {
        for token in sentence {
            tokens += 1;
            let is_word = token
                .candidates
                .first()
                .map(|analysis| &analysis.surface)
                .is_some_and(|surface| surface.chars().any(char::is_alphabetic));
            if !token.candidates.is_empty() && is_word {
                alphabetic += 1;
                covered += 1;
            }
            if token.gold.is_some() {
                mapped += 1;
            }
        }
    }
    eprintln!(
        "{name}: {} sentences, {tokens} tokens, {covered} analyzable words, {mapped} gold-mapped ({:.1}% of analyzable)",
        data.len(),
        100.0 * mapped as f64 / covered.max(1) as f64
    );
    let _ = alphabetic;
}
