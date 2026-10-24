//! Morphological analysis throughput benchmark.
//!
//! Run with:
//!
//! ```bash
//! cargo run --release -p tekir-morph --example bench -- [rounds] [extra_roots|lexicon.tsv]
//! ```
//!
//! The workload is surface forms generated from the lexicon (entries x all
//! noun paths, sampled if very large), analyzed `rounds` times. The second
//! argument either adds that many deterministic synthetic Turkish-like
//! roots to the seed lexicon, or — when it is not a number — is treated as
//! a lexicon TSV path to benchmark against real data (phase 2 acceptance:
//! within 2x of the seed ns/word at 20k+ roots). Uses the raw
//! allocation-light analysis path with one scratch buffer per worker, and
//! sweeps thread counts up to the machine's available parallelism.

use std::{env, thread, time::Instant};

use tekir_morph::{
    Lexicon, LexiconEntry, NounAnalyzer, generator::generate_noun_surface, state::noun_paths,
};

fn main() {
    let rounds: usize = env::args()
        .nth(1)
        .map(|value| value.parse().expect("rounds must be a positive integer"))
        .unwrap_or(200);
    let lexicon = match env::args().nth(2) {
        None => Lexicon::seed(),
        Some(argument) => match argument.parse::<usize>() {
            Ok(0) => Lexicon::seed(),
            Ok(extra_roots) => {
                let mut entries = Lexicon::seed().entries().to_vec();
                entries.extend(synthesize_roots(extra_roots));
                Lexicon::from_entries(entries)
            }
            Err(_) => Lexicon::from_tsv_path(&argument)
                .unwrap_or_else(|error| panic!("failed to load lexicon {argument}: {error}")),
        },
    };
    let paths = noun_paths();

    let noun_entries: Vec<_> = lexicon
        .entries()
        .iter()
        .filter(|entry| entry.pos == tekir_morph::lexicon::RootPos::Noun)
        .collect();
    let total_forms = noun_entries.len() * paths.len();
    let stride = (total_forms / 250_000).max(1);
    let mut workload: Vec<String> = Vec::new();
    for entry in noun_entries.iter().step_by(stride) {
        for path in &paths {
            workload.push(generate_noun_surface(entry, path));
        }
    }

    let total_words = workload.len() * rounds;
    println!(
        "workload: {} surface forms x {} rounds = {} words, {} lexicon entries",
        workload.len(),
        rounds,
        total_words,
        lexicon.len()
    );

    let max_threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let mut thread_counts = vec![1];
    let mut candidate = 2;
    while candidate < max_threads {
        thread_counts.push(candidate);
        candidate *= 2;
    }
    if max_threads > 1 {
        thread_counts.push(max_threads);
    }

    println!(
        "{:>8} {:>14} {:>12} {:>10} {:>12}",
        "threads", "words/s", "ns/word", "speedup", "analyses"
    );

    let mut baseline_words_per_sec = 0.0_f64;

    for &threads in &thread_counts {
        let started = Instant::now();
        let analysis_count = run(&lexicon, &workload, rounds, threads);
        let elapsed = started.elapsed();

        let words_per_sec = total_words as f64 / elapsed.as_secs_f64();
        let ns_per_word = elapsed.as_nanos() as f64 / total_words as f64;
        if threads == 1 {
            baseline_words_per_sec = words_per_sec;
        }
        let speedup = words_per_sec / baseline_words_per_sec;

        println!(
            "{:>8} {:>14.0} {:>12.0} {:>9.2}x {:>12}",
            threads, words_per_sec, ns_per_word, speedup, analysis_count
        );
    }
}

/// Analyze the whole workload `rounds` times over `threads` workers; each
/// worker owns one reusable scratch vector. Returns the total analysis count
/// as a checksum so the work cannot be optimized away (and to verify
/// determinism across thread counts).
fn run(lexicon: &Lexicon, workload: &[String], rounds: usize, threads: usize) -> usize {
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);

        for worker in 0..threads {
            handles.push(scope.spawn(move || {
                let analyzer = NounAnalyzer::new(lexicon);
                let mut scratch = Vec::with_capacity(8);
                let mut analyses = 0usize;

                for _ in 0..rounds {
                    for surface in workload.iter().skip(worker).step_by(threads) {
                        scratch.clear();
                        analyzer.analyze_raw_into(surface, &mut scratch);
                        analyses += scratch.len();
                    }
                }

                analyses
            }));
        }

        handles
            .into_iter()
            .map(|handle| handle.join().expect("bench worker must not panic"))
            .sum()
    })
}

/// Deterministic synthetic Turkish-like roots (LCG-seeded, so every run and
/// every machine builds the identical lexicon). Roughly half of the roots
/// ending in a voiceable stop get a voiced vowel-stem alternant, mirroring
/// the shape of a real lexicon.
fn synthesize_roots(count: usize) -> Vec<LexiconEntry> {
    const ONSETS: &[&str] = &[
        "b", "c", "d", "g", "k", "l", "m", "n", "p", "r", "s", "t", "v", "y", "z", "ç", "ş",
    ];
    const VOWELS: &[&str] = &["a", "e", "ı", "i", "o", "ö", "u", "ü"];
    const CODAS: &[&str] = &["", "n", "r", "l", "k", "p", "t", "s", "z", "m", "ç"];

    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move |bound: usize| -> usize {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % bound
    };

    (0..count)
        .map(|_| {
            let syllables = 2 + next(2);
            let mut lemma = String::new();
            for _ in 0..syllables {
                lemma.push_str(ONSETS[next(ONSETS.len())]);
                lemma.push_str(VOWELS[next(VOWELS.len())]);
            }
            lemma.push_str(CODAS[next(CODAS.len())]);

            let voiced = match lemma.chars().next_back() {
                Some('p') => Some('b'),
                Some('ç') => Some('c'),
                Some('k') => Some('ğ'),
                Some('t') => Some('d'),
                _ => None,
            };
            let vowel_stem = voiced.filter(|_| next(2) == 0).map(|replacement| {
                let mut stem = lemma.clone();
                stem.pop();
                stem.push(replacement);
                stem
            });

            LexiconEntry::new(lemma, vowel_stem, None)
        })
        .collect()
}
