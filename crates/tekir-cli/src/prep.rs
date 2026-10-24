//! `prep`: the corpus-cleaning pipeline (PII redaction, quality filter,
//! near-duplicate removal) over JSONL or line-per-document text.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tekir_morph::batch::parallel_map;
use tekir_prep::dedup::{
    LshParams, MinHashConfig, MinHasher, exact_duplicates, exact_hash, near_duplicates,
};
use tekir_prep::pii::{self, PiiKind};
use tekir_prep::quality::{QualityConfig, QualityRule, QualityScorer, QualitySignals};

use crate::InputFormat;

#[derive(Debug, Parser)]
pub struct PrepArgs {
    /// Input file; omit to read stdin.
    input: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = InputFormat::Jsonl)]
    input_format: InputFormat,
    /// Replace PII with `[TCKN]`, `[IBAN]`, `[PHONE]`, ... placeholders.
    #[arg(long)]
    redact_pii: bool,
    /// Drop documents failing the quality thresholds.
    #[arg(long)]
    filter: bool,
    /// Attach quality signals to every output record.
    #[arg(long)]
    signals: bool,
    /// JSON file overriding quality thresholds (any subset of fields).
    #[arg(long, value_name = "PATH")]
    quality_config: Option<PathBuf>,
    /// Drop exact and near duplicates, keeping the first of each cluster.
    #[arg(long)]
    dedup: bool,
    /// Estimated Jaccard similarity at which documents count as duplicates.
    #[arg(long, default_value_t = 0.8)]
    dedup_threshold: f64,
    #[arg(long, default_value_t = 0)]
    threads: usize,
}

#[derive(Debug, Deserialize)]
struct InputRecord {
    #[serde(default)]
    id: Option<serde_json::Value>,
    text: String,
}

#[derive(Debug, Serialize)]
struct OutputRecord<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a serde_json::Value>,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<&'a QualitySignals>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failed: Option<&'a [QualityRule]>,
}

fn read_records(args: &PrepArgs) -> Result<Vec<InputRecord>> {
    let reader: Box<dyn BufRead> = match &args.input {
        Some(path) => Box::new(io::BufReader::new(
            std::fs::File::open(path)
                .with_context(|| format!("failed to open {}", path.display()))?,
        )),
        None => Box::new(io::BufReader::new(io::stdin().lock())),
    };
    let mut records = Vec::new();
    for (number, line) in reader.lines().enumerate() {
        let line = line.context("failed to read input")?;
        match args.input_format {
            InputFormat::Text => records.push(InputRecord {
                id: None,
                text: line,
            }),
            InputFormat::Jsonl => {
                if line.trim().is_empty() {
                    continue;
                }
                records.push(
                    serde_json::from_str(&line)
                        .with_context(|| format!("invalid JSONL on line {}", number + 1))?,
                );
            }
        }
    }
    Ok(records)
}

pub fn run_prep(args: PrepArgs) -> Result<()> {
    let mut records = read_records(&args)?;
    let total = records.len();

    if args.redact_pii {
        let redacted = parallel_map(&records, args.threads, |record| {
            pii::redact(&record.text, PiiKind::placeholder)
        });
        for (record, text) in records.iter_mut().zip(redacted) {
            record.text = text;
        }
    }

    let mut keep = vec![true; records.len()];
    let mut duplicates = 0;
    if args.dedup {
        let hashes: Vec<u128> = parallel_map(&records, args.threads, |r| exact_hash(&r.text));
        let exact = exact_duplicates(&hashes);
        let hasher = MinHasher::new(MinHashConfig::default());
        let texts: Vec<&str> = records.iter().map(|r| r.text.as_str()).collect();
        let signatures = hasher.signatures(&texts, args.threads);
        let params = LshParams::for_threshold(hasher.config().num_perm, args.dedup_threshold);
        let near = near_duplicates(&signatures, params, args.dedup_threshold);
        for i in 0..records.len() {
            if exact[i] != i || near[i] != i {
                keep[i] = false;
                duplicates += 1;
            }
        }
    }

    let config = match &args.quality_config {
        Some(path) => serde_json::from_str(
            &std::fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?,
        )
        .context("invalid quality config")?,
        None => QualityConfig::default(),
    };
    let scored = args.filter || args.signals;
    let signals: Vec<Option<QualitySignals>> = if scored {
        let scorer = QualityScorer::default();
        parallel_map(&records, args.threads, |r| Some(scorer.signals(&r.text)))
    } else {
        records.iter().map(|_| None).collect()
    };

    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut filtered = 0;
    for ((record, signals), keep) in records.iter().zip(&signals).zip(&keep) {
        if !keep {
            continue;
        }
        let failed = signals.as_ref().map(|s| config.check(s));
        if args.filter && failed.as_ref().is_some_and(|f| !f.is_empty()) {
            filtered += 1;
            continue;
        }
        let output = OutputRecord {
            id: record.id.as_ref(),
            text: &record.text,
            quality: if args.signals { signals.as_ref() } else { None },
            failed: if args.signals {
                failed.as_deref().filter(|f| !f.is_empty())
            } else {
                None
            },
        };
        serde_json::to_writer(&mut stdout, &output).context("failed to write output")?;
        stdout.write_all(b"\n")?;
    }
    stdout.flush()?;
    eprintln!(
        "{total} documents: {duplicates} duplicates dropped, {filtered} filtered, {} kept",
        total - duplicates - filtered
    );
    Ok(())
}
