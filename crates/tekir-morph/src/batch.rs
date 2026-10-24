//! Batch analysis for corpus-scale callers.
//!
//! Word frequencies in running text are Zipfian, so a batch analyzes each
//! unique word once and shares the result. Unique words are split into
//! contiguous chunks over scoped threads; results are order-preserving and
//! identical for any thread count.

use std::collections::HashMap;

use crate::analysis::{MorphOutput, MorphService};

/// Analyses for a batch of words, stored once per unique word.
#[derive(Debug, Clone)]
pub struct BatchAnalysis {
    unique: Vec<MorphOutput>,
    index: Vec<u32>,
}

impl BatchAnalysis {
    /// Number of input words.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Analyses of the `i`-th input word.
    pub fn get(&self, i: usize) -> &MorphOutput {
        &self.unique[self.index[i] as usize]
    }

    /// Position of the `i`-th input word in [`unique`](Self::unique).
    pub fn unique_index(&self, i: usize) -> usize {
        self.index[i] as usize
    }

    /// One output per distinct word, in order of first occurrence.
    pub fn unique(&self) -> &[MorphOutput] {
        &self.unique
    }

    pub fn iter(&self) -> impl Iterator<Item = &MorphOutput> {
        self.index.iter().map(|&i| &self.unique[i as usize])
    }
}

/// Resolve a requested thread count (`0` = available parallelism).
pub fn resolve_threads(threads: usize) -> usize {
    if threads > 0 {
        return threads;
    }
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Map `items` through `f` on up to `threads` scoped threads, preserving
/// order. Contiguous chunks keep results deterministic.
pub fn parallel_map<T, R, F>(items: &[T], threads: usize, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let threads = resolve_threads(threads).min(items.len()).max(1);
    if threads == 1 {
        return items.iter().map(&f).collect();
    }
    let chunk = items.len().div_ceil(threads);
    std::thread::scope(|scope| {
        let workers: Vec<_> = items
            .chunks(chunk)
            .map(|part| scope.spawn(|| part.iter().map(&f).collect::<Vec<R>>()))
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("batch worker panicked"))
            .collect()
    })
}

impl MorphService {
    /// Analyze many words at once, each distinct word once. Words are used
    /// as given (lowercase them first; see [`MorphService::analyze_word`]).
    /// `threads = 0` uses all available cores.
    pub fn analyze_batch<S: AsRef<str> + Sync>(
        &self,
        words: &[S],
        threads: usize,
    ) -> BatchAnalysis {
        let mut positions: HashMap<&str, u32> = HashMap::with_capacity(words.len() / 4 + 1);
        let mut unique_words: Vec<&str> = Vec::new();
        let index = words
            .iter()
            .map(|word| {
                let word = word.as_ref();
                *positions.entry(word).or_insert_with(|| {
                    unique_words.push(word);
                    (unique_words.len() - 1) as u32
                })
            })
            .collect();

        let unique = parallel_map(&unique_words, threads, |word| self.analyze_word(word));
        BatchAnalysis { unique, index }
    }
}

#[cfg(test)]
mod tests {
    use crate::MorphService;

    #[test]
    fn batch_matches_single_word_analysis_and_dedups() {
        let service = MorphService::seed();
        let words = ["evler", "kitaba", "evler", "gelecekmiş", "kitaba", "evler"];
        let batch = service.analyze_batch(&words, 3);

        assert_eq!(batch.len(), words.len());
        assert_eq!(batch.unique().len(), 3);
        for (i, word) in words.iter().enumerate() {
            assert_eq!(*batch.get(i), service.analyze_word(word));
        }
        assert_eq!(batch.unique_index(0), batch.unique_index(5));
    }

    #[test]
    fn batch_output_is_independent_of_thread_count() {
        let service = MorphService::seed();
        let words: Vec<String> = (0..200)
            .map(|i| ["ev", "evde", "kitaplar", "yapıyor", "xyzler"][i % 5].to_string())
            .collect();
        let one: Vec<_> = service.analyze_batch(&words, 1).iter().cloned().collect();
        let many: Vec<_> = service.analyze_batch(&words, 8).iter().cloned().collect();
        assert_eq!(one, many);
    }
}
