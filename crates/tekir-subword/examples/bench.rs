//! Pre-tokenization throughput, with and without the word cache.
//!
//! `cargo run --release -p tekir-subword --example bench -- corpus.txt [threads]`

use std::time::Instant;

use tekir_morph::batch::parallel_map;
use tekir_subword::scan::{PieceKind, scan};
use tekir_subword::{BoundaryCache, Pretokenizer};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: bench corpus.txt [threads]");
    let threads: usize = args.next().map_or(0, |t| t.parse().expect("threads"));
    let text = std::fs::read_to_string(&path).expect("read corpus");
    let lines: Vec<&str> = text.lines().collect();

    let started = Instant::now();
    let pretok = Pretokenizer::default();
    println!("lexicon load: {:.2?}", started.elapsed());

    let words: Vec<String> = lines
        .iter()
        .flat_map(|line| {
            scan(line)
                .into_iter()
                .filter(|piece| piece.kind == PieceKind::Word)
                .map(|piece| tekir_core::turkish_lowercase(&line[piece.content()]))
                .collect::<Vec<_>>()
        })
        .collect();
    let unique: std::collections::HashSet<&str> = words.iter().map(String::as_str).collect();
    println!(
        "{} lines, {} words, {} distinct ({:.1}%)",
        lines.len(),
        words.len(),
        unique.len(),
        100.0 * unique.len() as f64 / words.len() as f64
    );

    let started = Instant::now();
    let uncached = parallel_map(&words, threads, |word| pretok.word_boundaries(word).len());
    let elapsed = started.elapsed();
    println!(
        "per-occurrence analysis: {:.2?} ({:.0} words/s), {} boundaries",
        elapsed,
        words.len() as f64 / elapsed.as_secs_f64(),
        uncached.iter().sum::<usize>()
    );

    let mut cache = BoundaryCache::default();
    let started = Instant::now();
    let pieces = pretok.pretokenize_batch(&lines, threads, &mut cache);
    let elapsed = started.elapsed();
    println!(
        "pretokenize_batch (cold cache): {:.2?} ({:.0} words/s), {} pieces",
        elapsed,
        words.len() as f64 / elapsed.as_secs_f64(),
        pieces.iter().map(Vec::len).sum::<usize>()
    );

    let started = Instant::now();
    let pieces = pretok.pretokenize_batch(&lines, threads, &mut cache);
    let elapsed = started.elapsed();
    println!(
        "pretokenize_batch (warm cache): {:.2?} ({:.0} words/s), {} pieces",
        elapsed,
        words.len() as f64 / elapsed.as_secs_f64(),
        pieces.iter().map(Vec::len).sum::<usize>()
    );
}
