//! Diacritic restoration accuracy: strip Turkish letters from a corpus,
//! restore them, compare word by word with the original.
//!
//! `cargo run --release -p tekir-spell --example restore_eval -- corpus.txt`

use std::time::Instant;

use tekir_spell::{Restorer, SpellChecker};

fn asciify(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'ç' => 'c',
            'ğ' => 'g',
            'ı' => 'i',
            'ö' => 'o',
            'ş' => 's',
            'ü' => 'u',
            'Ç' => 'C',
            'Ğ' => 'G',
            'İ' => 'I',
            'Ö' => 'O',
            'Ş' => 'S',
            'Ü' => 'U',
            other => other,
        })
        .collect()
}

fn score(name: &str, gold: &[&str], ascii: &[String], output: &[String]) {
    let (mut words, mut correct, mut changed, mut changed_correct) = (0, 0, 0, 0);
    for ((g, a), o) in gold.iter().zip(ascii).zip(output) {
        for ((gw, aw), ow) in g
            .split_whitespace()
            .zip(a.split_whitespace())
            .zip(o.split_whitespace())
        {
            words += 1;
            correct += usize::from(gw == ow);
            if gw != aw {
                changed += 1;
                changed_correct += usize::from(gw == ow);
            }
        }
    }
    println!(
        "{name:<22} word accuracy {:.2}%  on words that lost diacritics {:.2}% ({changed} words)",
        100.0 * correct as f64 / words as f64,
        100.0 * changed_correct as f64 / changed as f64,
    );
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: restore_eval corpus.txt");
    let text = std::fs::read_to_string(path).expect("read corpus");
    let gold: Vec<&str> = text.lines().collect();
    let ascii: Vec<String> = gold.iter().map(|line| asciify(line)).collect();

    score("no restoration", &gold, &ascii, &ascii);

    let checker = SpellChecker::default();
    let per_word: Vec<String> = ascii
        .iter()
        .map(|line| {
            line.split_whitespace()
                .map(|word| {
                    let lower = word.to_lowercase();
                    if checker.is_correct(&lower) {
                        word.to_string()
                    } else {
                        checker
                            .deasciify(&lower)
                            .unwrap_or_else(|| word.to_string())
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    score("per-word deasciify", &gold, &ascii, &per_word);

    let restorer = Restorer::default();
    let started = Instant::now();
    let restored = restorer.restore_batch(&ascii, 0);
    let elapsed = started.elapsed();
    score("in-context Restorer", &gold, &ascii, &restored);
    println!("restored {} lines in {elapsed:.2?}", ascii.len());
}
