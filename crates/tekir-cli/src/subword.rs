//! `pretokenize` and `tokenizer train|eval`: morpheme-aware subword tools.

use std::io::{self, BufRead};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tekir_subword::{Pretokenizer, TekirTokenizer, TrainConfig, evaluate, train};

#[derive(Debug, Parser)]
pub struct PretokenizeArgs {
    /// Text to split; omit to read lines from stdin.
    input: Option<String>,
    /// Also split words the analyzer only guessed.
    #[arg(long)]
    split_unknown: bool,
}

#[derive(Debug, Parser)]
pub struct TokenizerArgs {
    #[command(subcommand)]
    command: TokenizerCommand,
}

#[derive(Debug, Subcommand)]
enum TokenizerCommand {
    /// Train a byte-level BPE tokenizer.json on a line-per-document corpus.
    Train {
        #[arg(long, value_name = "PATH")]
        corpus: PathBuf,
        #[arg(long, value_name = "PATH")]
        out: PathBuf,
        #[arg(long, default_value_t = 32_000)]
        vocab_size: usize,
        #[arg(long, default_value_t = 2)]
        min_frequency: u64,
        /// Skip morpheme splitting (plain word-level baseline).
        #[arg(long)]
        baseline: bool,
        #[arg(long, default_value_t = 0)]
        threads: usize,
    },
    /// Report compression and morpheme-boundary alignment.
    Eval {
        /// Any Hugging Face tokenizer.json.
        #[arg(long, value_name = "PATH")]
        tokenizer: PathBuf,
        #[arg(long, value_name = "PATH")]
        corpus: PathBuf,
        /// Encode through tekir's morpheme pre-tokenizer (for tokenizers
        /// trained without `--baseline`).
        #[arg(long)]
        morph: bool,
        #[arg(long, default_value_t = 0)]
        threads: usize,
    },
}

pub fn run_pretokenize(args: PretokenizeArgs) -> Result<()> {
    let pretok = Pretokenizer::default().split_unknown(args.split_unknown);
    let lines: Vec<String> = match args.input {
        Some(text) => vec![text],
        None => io::stdin()
            .lock()
            .lines()
            .collect::<io::Result<_>>()
            .context("failed to read stdin")?,
    };
    let mut cache = tekir_subword::BoundaryCache::default();
    for (line, ranges) in lines
        .iter()
        .zip(pretok.pretokenize_batch(&lines, 0, &mut cache))
    {
        let pieces: Vec<&str> = ranges.into_iter().map(|range| &line[range]).collect();
        println!("{}", pieces.join("|"));
    }
    Ok(())
}

fn read_lines(path: &PathBuf) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read corpus {}", path.display()))?;
    Ok(text.lines().map(str::to_string).collect())
}

pub fn run_tokenizer(args: TokenizerArgs) -> Result<()> {
    match args.command {
        TokenizerCommand::Train {
            corpus,
            out,
            vocab_size,
            min_frequency,
            baseline,
            threads,
        } => {
            let config = TrainConfig {
                vocab_size,
                min_frequency,
                morph: !baseline,
                threads,
                ..TrainConfig::default()
            };
            let started = std::time::Instant::now();
            let tokenizer = train(&Pretokenizer::default(), read_lines(&corpus)?, &config)
                .map_err(|error| anyhow::anyhow!("training failed: {error}"))?;
            tokenizer
                .save(&out, false)
                .map_err(|error| anyhow::anyhow!("failed to save tokenizer: {error}"))?;
            eprintln!(
                "trained {} tokens in {:.2?} -> {}",
                tokenizer.get_vocab_size(true),
                started.elapsed(),
                out.display()
            );
            Ok(())
        }
        TokenizerCommand::Eval {
            tokenizer,
            corpus,
            morph,
            threads,
        } => {
            let pretok = Pretokenizer::default();
            let tokenizer = TekirTokenizer::from_file(&tokenizer, morph.then(|| pretok.clone()))
                .map_err(|error| anyhow::anyhow!("failed to load tokenizer: {error}"))?;
            let lines = read_lines(&corpus)?;
            let report = evaluate(&tokenizer, &pretok, &lines, threads)
                .map_err(|error| anyhow::anyhow!("evaluation failed: {error}"))?;
            crate::print_json("tokenizer_eval", &report)
        }
    }
}
