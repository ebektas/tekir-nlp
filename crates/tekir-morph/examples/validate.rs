//! Roundtrip validator for a lexicon TSV: every entry × every noun path
//! must generate, analyze back to itself, and every analysis of every
//! generated surface must regenerate byte-identically.
//!
//! ```bash
//! cargo run --release -p tekir-morph --example validate -- \
//!     crates/tekir-morph/data/zemberek-nouns.tsv \
//!     crates/tekir-morph/data/zemberek-compounds.tsv
//! ```
//!
//! Accepts several lexicon files (nouns, adjectives, verbs, compounds);
//! noun and compound entries are validated over their legal paths.
//!
//! Exits non-zero on any failure and prints the first few per worker.

use std::{env, process::ExitCode, thread, time::Instant};

use tekir_morph::{
    Lexicon, NounAnalyzer,
    generator::{generate_from_igs, generate_noun_surface},
    state::noun_paths,
};

const MAX_REPORTED_FAILURES_PER_WORKER: usize = 5;

fn main() -> ExitCode {
    let paths: Vec<String> = env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: validate <lexicon.tsv>...");
        return ExitCode::FAILURE;
    }

    let lexicon = match Lexicon::from_tsv_paths(&paths) {
        Ok(lexicon) => lexicon,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let paths = noun_paths();
    let threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    println!(
        "validating {} entries x {} paths on {} threads",
        lexicon.len(),
        paths.len(),
        threads
    );
    let started = Instant::now();

    let (checked, failures): (usize, usize) = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        let lexicon = &lexicon;
        let paths = &paths;

        for worker in 0..threads {
            handles.push(scope.spawn(move || {
                let analyzer = NounAnalyzer::new(lexicon);
                let mut raw = Vec::with_capacity(8);
                let mut checked = 0usize;
                let mut failures = 0usize;

                for (index, entry) in lexicon.entries().iter().enumerate() {
                    if entry.pos != tekir_morph::lexicon::RootPos::Noun {
                        continue;
                    }
                    if index % threads != worker {
                        continue;
                    }
                    let entry_id = index as u32;

                    for path in paths.iter().filter(|path| {
                        !entry.compound_p3sg
                            || path.possessive != tekir_morph::state::Possessive::None
                    }) {
                        checked += 1;
                        let surface = generate_noun_surface(entry, path);

                        raw.clear();
                        analyzer.analyze_raw_into(&surface, &mut raw);

                        let roundtrips = raw.iter().any(|analysis| {
                            analysis.entry == entry_id && analysis.igs().len() == 1 && {
                                let ig = analysis.final_ig();
                                ig.number == path.number
                                    && ig.possessive == path.possessive
                                    && ig.case == path.case
                            }
                        });
                        if !roundtrips {
                            failures += 1;
                            if failures <= MAX_REPORTED_FAILURES_PER_WORKER {
                                eprintln!(
                                    "no roundtrip: {} ({:?}/{:?}/{:?}) -> {}",
                                    entry.lemma, path.number, path.possessive, path.case, surface
                                );
                            }
                        }

                        for analysis in &raw {
                            let regenerated =
                                generate_from_igs(lexicon.entry(analysis.entry), analysis.igs());
                            if regenerated != surface {
                                failures += 1;
                                if failures <= MAX_REPORTED_FAILURES_PER_WORKER {
                                    eprintln!(
                                        "no regeneration: {} analyzed from {} regenerated as {}",
                                        surface,
                                        lexicon.entry(analysis.entry).lemma,
                                        regenerated
                                    );
                                }
                            }
                        }
                    }
                }

                (checked, failures)
            }));
        }

        handles
            .into_iter()
            .map(|handle| handle.join().expect("validator worker must not panic"))
            .fold((0, 0), |(c, f), (checked, failures)| {
                (c + checked, f + failures)
            })
    });

    let elapsed = started.elapsed();
    println!(
        "checked {} surface forms in {:.2}s: {} failures",
        checked,
        elapsed.as_secs_f64(),
        failures
    );

    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
