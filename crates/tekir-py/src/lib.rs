//! Python bindings: `import tekir`.
//!
//! Batch entry points release the GIL and fan out over all cores; shared
//! state (lexicon, disambiguation model) is built once per process.

use std::sync::OnceLock;

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::Value;

use tekir_core::{is_turkish_lowercase, turkish_lowercase};
use tekir_disamb::Pipeline;
use tekir_morph::{MorphAnalysis, MorphService};
use tekir_prep::dedup::{
    LshParams, MinHashConfig, MinHasher, exact_duplicates, exact_hash, near_duplicates,
};
use tekir_prep::pii::{self, PiiKind};
use tekir_prep::quality::{QualityConfig, QualityScorer};
use tekir_spell::Restorer;
use tekir_subword::{BoundaryCache, TekirTokenizer, TrainConfig};

fn morph() -> &'static MorphService {
    static MORPH: OnceLock<MorphService> = OnceLock::new();
    MORPH.get_or_init(MorphService::new)
}

fn pipeline() -> &'static Pipeline {
    static PIPELINE: OnceLock<Pipeline> = OnceLock::new();
    PIPELINE.get_or_init(Pipeline::bundled)
}

fn restorer() -> &'static Restorer {
    static RESTORER: OnceLock<Restorer> = OnceLock::new();
    RESTORER.get_or_init(Restorer::default)
}

fn scorer() -> &'static QualityScorer {
    static SCORER: OnceLock<QualityScorer> = OnceLock::new();
    SCORER.get_or_init(|| QualityScorer::new(morph().clone()))
}

/// JSON value -> Python object.
fn to_py<'py>(py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match value {
        Value::Null => py.None().into_bound(py),
        Value::Bool(b) => b.into_pyobject(py)?.to_owned().into_any(),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.into_pyobject(py)?.into_any(),
            (None, Some(f)) => f.into_pyobject(py)?.into_any(),
            _ => py.None().into_bound(py),
        },
        Value::String(s) => s.into_pyobject(py)?.into_any(),
        Value::Array(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(to_py(py, item)?)?;
            }
            list.into_any()
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (key, item) in map {
                dict.set_item(key, to_py(py, item)?)?;
            }
            dict.into_any()
        }
    })
}

/// Python object -> JSON value (dicts, lists, str, int, float, bool, None).
fn from_py(object: &Bound<'_, PyAny>) -> PyResult<Value> {
    if object.is_none() {
        return Ok(Value::Null);
    }
    if let Ok(b) = object.extract::<bool>() {
        return Ok(Value::Bool(b));
    }
    if let Ok(i) = object.extract::<i64>() {
        return Ok(Value::from(i));
    }
    if let Ok(f) = object.extract::<f64>() {
        return Ok(Value::from(f));
    }
    if let Ok(s) = object.extract::<String>() {
        return Ok(Value::String(s));
    }
    if let Ok(dict) = object.cast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (key, item) in dict.iter() {
            map.insert(key.extract::<String>()?, from_py(&item)?);
        }
        return Ok(Value::Object(map));
    }
    if let Ok(list) = object.cast::<PyList>() {
        return list
            .iter()
            .map(|item| from_py(&item))
            .collect::<PyResult<_>>()
            .map(Value::Array);
    }
    Err(PyValueError::new_err("unsupported value in config"))
}

fn json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("serializable")
}

/// Lowercase for analysis, keeping apostrophe forms as written.
fn analysis_form(word: &str) -> String {
    if is_turkish_lowercase(word) || word.contains(['\'', '’']) {
        word.to_string()
    } else {
        turkish_lowercase(word)
    }
}

fn analysis_json(analysis: &MorphAnalysis) -> Value {
    let mut value = json(analysis);
    value["morphemes"] = json(&analysis.morphemes(morph().lexicon()));
    value
}

/// All analyses of `word` (Turkish-lowercased first), best first.
#[pyfunction]
fn analyze<'py>(py: Python<'py>, word: &str) -> PyResult<Bound<'py, PyAny>> {
    let output = morph().analyze_word(&analysis_form(word));
    let mut analyses: Vec<Value> = Vec::with_capacity(output.analyses.len());
    if let Some(best) = &output.best_analysis {
        analyses.push(analysis_json(best));
    }
    for analysis in &output.analyses {
        if Some(analysis) != output.best_analysis.as_ref() {
            analyses.push(analysis_json(analysis));
        }
    }
    to_py(py, &Value::Array(analyses))
}

/// Best lexical form per word (or `None`), each distinct word analyzed once.
#[pyfunction]
#[pyo3(signature = (words, threads = 0))]
fn analyze_batch(py: Python<'_>, words: Vec<String>, threads: usize) -> Vec<Option<String>> {
    py.detach(|| {
        let forms: Vec<String> = words.iter().map(|w| analysis_form(w)).collect();
        let batch = morph().analyze_batch(&forms, threads);
        batch
            .iter()
            .map(|output| {
                output
                    .best_analysis
                    .as_ref()
                    .map(|a| a.lexical_form.clone())
            })
            .collect()
    })
}

/// Morphemes of the best analysis (`"kitaplardan"` -> `["kitap", "lar", "dan"]`).
#[pyfunction]
fn segment(word: &str) -> Vec<String> {
    let form = analysis_form(word);
    match morph().analyze_word(&form).best_analysis {
        Some(best) if form.len() == word.len() => {
            let boundaries = best.morpheme_boundaries(morph().lexicon());
            tekir_morph::analysis::split_at_boundaries(word, &boundaries)
                .into_iter()
                .map(str::to_string)
                .collect()
        }
        Some(best) => best
            .morphemes(morph().lexicon())
            .into_iter()
            .map(str::to_string)
            .collect(),
        None => vec![word.to_string()],
    }
}

/// Morpheme-aware, lossless pre-tokenizer with a word cache that persists
/// across calls.
#[pyclass(module = "tekir")]
struct Pretokenizer {
    inner: tekir_subword::Pretokenizer,
    cache: BoundaryCache,
}

#[pymethods]
impl Pretokenizer {
    #[new]
    #[pyo3(signature = (split_unknown = false, cache_size = 5_000_000))]
    fn new(split_unknown: bool, cache_size: usize) -> Self {
        Self {
            inner: tekir_subword::Pretokenizer::new(morph().clone()).split_unknown(split_unknown),
            cache: BoundaryCache::with_capacity(cache_size),
        }
    }

    /// Pieces of `text`; `"".join(pieces) == text`.
    fn pretokenize(&mut self, py: Python<'_>, text: String) -> Vec<String> {
        self.pretokenize_batch(py, vec![text], 1).remove(0)
    }

    #[pyo3(signature = (texts, threads = 0))]
    fn pretokenize_batch(
        &mut self,
        py: Python<'_>,
        texts: Vec<String>,
        threads: usize,
    ) -> Vec<Vec<String>> {
        let (inner, cache) = (&self.inner, &mut self.cache);
        py.detach(|| {
            let ranges = inner.pretokenize_batch(&texts, threads, cache);
            texts
                .iter()
                .zip(ranges)
                .map(|(text, ranges)| ranges.into_iter().map(|r| text[r].to_string()).collect())
                .collect()
        })
    }
}

/// A `tokenizer.json`, encoding through tekir's pre-tokenizer when `morph`.
#[pyclass(module = "tekir", frozen)]
struct Tokenizer {
    inner: TekirTokenizer,
}

#[pymethods]
impl Tokenizer {
    #[staticmethod]
    #[pyo3(signature = (path, morph = true))]
    fn from_file(path: &str, morph: bool) -> PyResult<Self> {
        let pretok = morph.then(|| tekir_subword::Pretokenizer::new(self::morph().clone()));
        TekirTokenizer::from_file(path, pretok)
            .map(|inner| Self { inner })
            .map_err(|e| PyIOError::new_err(e.to_string()))
    }

    fn encode(&self, py: Python<'_>, text: String) -> PyResult<Vec<u32>> {
        Ok(self.encode_batch(py, vec![text], 1)?.remove(0))
    }

    #[pyo3(signature = (texts, threads = 0))]
    fn encode_batch(
        &self,
        py: Python<'_>,
        texts: Vec<String>,
        threads: usize,
    ) -> PyResult<Vec<Vec<u32>>> {
        py.detach(|| self.inner.encode_batch(&texts, threads))
            .map(|encoded| encoded.into_iter().map(|e| e.ids).collect())
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn decode(&self, ids: Vec<u32>) -> PyResult<String> {
        self.inner
            .decode(&ids)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn save(&self, path: &str) -> PyResult<()> {
        self.inner
            .save(path)
            .map_err(|e| PyIOError::new_err(e.to_string()))
    }

    #[getter]
    fn vocab_size(&self) -> usize {
        self.inner.inner().get_vocab_size(true)
    }
}

/// Train a byte-level BPE tokenizer on `lines`.
#[pyfunction]
#[pyo3(signature = (lines, vocab_size = 32_000, min_frequency = 2, morph = true, special_tokens = None, threads = 0))]
fn train_tokenizer(
    py: Python<'_>,
    lines: Vec<String>,
    vocab_size: usize,
    min_frequency: u64,
    morph: bool,
    special_tokens: Option<Vec<String>>,
    threads: usize,
) -> PyResult<Tokenizer> {
    let mut config = TrainConfig {
        vocab_size,
        min_frequency,
        morph,
        threads,
        ..TrainConfig::default()
    };
    if let Some(tokens) = special_tokens {
        config.special_tokens = tokens;
    }
    let pretok = tekir_subword::Pretokenizer::new(self::morph().clone());
    let tokenizer = py
        .detach(|| tekir_subword::train(&pretok, lines, &config))
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(Tokenizer {
        inner: TekirTokenizer::new(tokenizer, morph.then_some(pretok)),
    })
}

/// Disambiguated `(surface, lexical_form | None)` per token of a sentence.
#[pyfunction]
fn disambiguate(py: Python<'_>, text: String) -> Vec<(String, Option<String>)> {
    py.detach(|| {
        pipeline()
            .analyze_sentence(&text)
            .tokens
            .into_iter()
            .map(|t| (t.surface, t.analysis.map(|a| a.lexical_form)))
            .collect()
    })
}

/// Restore Turkish letters in ASCII-typed text, in context.
#[pyfunction]
fn restore(py: Python<'_>, text: String) -> String {
    py.detach(|| restorer().restore(&text))
}

#[pyfunction]
#[pyo3(signature = (texts, threads = 0))]
fn restore_batch(py: Python<'_>, texts: Vec<String>, threads: usize) -> Vec<String> {
    py.detach(|| restorer().restore_batch(&texts, threads))
}

/// PII spans as `(kind, start, end)` with Python string (code point) offsets.
#[pyfunction]
fn detect_pii(text: &str) -> Vec<(String, usize, usize)> {
    let to_char = |byte: usize| text[..byte].chars().count();
    pii::detect(text)
        .into_iter()
        .map(|span| {
            let kind = json(&span.kind).as_str().unwrap_or_default().to_string();
            (kind, to_char(span.start_byte), to_char(span.end_byte))
        })
        .collect()
}

/// Replace PII with placeholders such as `[TCKN]` and `[PHONE]`.
#[pyfunction]
fn redact_pii(text: &str) -> String {
    pii::redact(text, PiiKind::placeholder)
}

/// Quality signals plus `failed` rules and `passes`; `config` overrides any
/// default threshold.
#[pyfunction]
#[pyo3(signature = (text, config = None))]
fn quality<'py>(
    py: Python<'py>,
    text: String,
    config: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let config: QualityConfig = match config {
        Some(dict) => serde_json::from_value(from_py(dict.as_any())?)
            .map_err(|e| PyValueError::new_err(e.to_string()))?,
        None => QualityConfig::default(),
    };
    let signals = py.detach(|| scorer().signals(&text));
    let failed = config.check(&signals);
    let mut value = json(&signals);
    value["passes"] = Value::Bool(failed.is_empty());
    value["failed"] = json(&failed);
    to_py(py, &value)
}

/// Keep-mask over `texts` dropping exact and near duplicates (first of each
/// cluster survives).
#[pyfunction]
#[pyo3(signature = (texts, threshold = 0.8, threads = 0))]
fn dedup(py: Python<'_>, texts: Vec<String>, threshold: f64, threads: usize) -> Vec<bool> {
    py.detach(|| {
        let exact = exact_duplicates(&texts.iter().map(|t| exact_hash(t)).collect::<Vec<_>>());
        let hasher = MinHasher::new(MinHashConfig::default());
        let signatures = hasher.signatures(&texts, threads);
        let params = LshParams::for_threshold(hasher.config().num_perm, threshold);
        let near = near_duplicates(&signatures, params, threshold);
        (0..texts.len())
            .map(|i| exact[i] == i && near[i] == i)
            .collect()
    })
}

/// MinHash signatures, for deduplicating across shards yourself.
#[pyfunction]
#[pyo3(signature = (texts, threads = 0))]
fn minhash(py: Python<'_>, texts: Vec<String>, threads: usize) -> Vec<Vec<u64>> {
    py.detach(|| {
        MinHasher::new(MinHashConfig::default())
            .signatures(&texts, threads)
            .into_iter()
            .map(|s| s.0)
            .collect()
    })
}

#[pymodule]
fn tekir(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(analyze, m)?)?;
    m.add_function(wrap_pyfunction!(analyze_batch, m)?)?;
    m.add_function(wrap_pyfunction!(segment, m)?)?;
    m.add_function(wrap_pyfunction!(train_tokenizer, m)?)?;
    m.add_function(wrap_pyfunction!(disambiguate, m)?)?;
    m.add_function(wrap_pyfunction!(restore, m)?)?;
    m.add_function(wrap_pyfunction!(restore_batch, m)?)?;
    m.add_function(wrap_pyfunction!(detect_pii, m)?)?;
    m.add_function(wrap_pyfunction!(redact_pii, m)?)?;
    m.add_function(wrap_pyfunction!(quality, m)?)?;
    m.add_function(wrap_pyfunction!(dedup, m)?)?;
    m.add_function(wrap_pyfunction!(minhash, m)?)?;
    m.add_class::<Pretokenizer>()?;
    m.add_class::<Tokenizer>()?;
    Ok(())
}
