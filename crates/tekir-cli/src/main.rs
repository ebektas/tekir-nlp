use std::{
    collections::BTreeMap,
    io::{self, BufRead, Cursor, Read, Write},
    sync::{Arc, Mutex, mpsc},
    thread,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use tekir_core::{ReverseMode, ReverseService};
use tekir_morph::{Lexicon, MorphService};
use tekir_normalize::{NormalizeMode, NormalizeService};
use tekir_segment::{SegmentMode, SegmentService};
use tekir_tokenize::{TokenizeMode, TokenizeService};

#[derive(Debug, Parser)]
#[command(name = "tekir")]
#[command(about = "Early Tekir CLI for exercising text services")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[cfg(feature = "server")]
mod server;

#[derive(Debug, Subcommand)]
enum Command {
    Morph(MorphArgs),
    Disamb(DisambArgs),
    Spell(SpellArgs),
    #[cfg(feature = "server")]
    Serve(ServeArgs),
    Reverse(ReverseArgs),
    Normalize(NormalizeArgs),
    Segment(SegmentArgs),
    Tokenize(TokenizeArgs),
}

#[derive(Debug, Parser)]
struct ReverseArgs {
    input: Option<String>,
    #[arg(long)]
    stdin: bool,
    #[arg(long, value_enum, default_value_t = ModeArg::Graphemes)]
    mode: ModeArg,
    #[arg(long, default_value_t = OutputFormat::Text, value_enum)]
    output: OutputFormat,
}

#[derive(Debug, Parser)]
struct MorphArgs {
    input: Option<String>,
    #[arg(long)]
    stdin: bool,
    #[arg(long, value_enum, default_value_t = InputFormat::Text)]
    input_format: InputFormat,
    #[arg(long, default_value_t = 1)]
    jobs: usize,
    #[arg(long)]
    best_only: bool,
    /// Load root lexicon TSV files instead of the built-in full lexicon
    /// (repeatable; files may hold nouns, adjectives, verbs, or compounds).
    #[arg(long, value_name = "PATH")]
    lexicon: Vec<std::path::PathBuf>,
    #[arg(long, default_value_t = OutputFormat::Text, value_enum)]
    output: OutputFormat,
}

#[derive(Debug, Parser)]
struct DisambArgs {
    /// Sentence to disambiguate (Turkish-tokenized internally).
    input: Option<String>,
    #[arg(long)]
    stdin: bool,
    #[arg(long, value_enum, default_value_t = InputFormat::Text)]
    input_format: InputFormat,
    #[arg(long, default_value_t = 1)]
    jobs: usize,
    /// Trained perceptron model; defaults to the bundled one.
    #[arg(long, value_name = "PATH")]
    model: Option<std::path::PathBuf>,
    /// Root lexicon TSV files (repeatable); defaults to the full embedded
    /// lexicon.
    #[arg(long, value_name = "PATH")]
    lexicon: Vec<std::path::PathBuf>,
    #[arg(long, default_value_t = OutputFormat::Text, value_enum)]
    output: OutputFormat,
}

/// HTTP service configuration; every flag overrides the config file.
#[cfg(feature = "server")]
#[derive(Debug, Parser)]
pub struct ServeArgs {
    /// Thin `key = value` config file (host, port, threads, model, lexicon).
    #[arg(long, value_name = "PATH")]
    pub config: Option<std::path::PathBuf>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    /// Worker threads (default: available parallelism).
    #[arg(long)]
    pub threads: Option<usize>,
    /// Trained disambiguation model; defaults to the bundled one.
    #[arg(long, value_name = "PATH")]
    pub model: Option<std::path::PathBuf>,
    /// Root lexicon TSV files (repeatable); defaults to the full embedded
    /// lexicon.
    #[arg(long, value_name = "PATH")]
    pub lexicon: Vec<std::path::PathBuf>,
}

#[derive(Debug, Parser)]
struct SpellArgs {
    /// Word to check.
    input: String,
    #[arg(long, default_value_t = 8)]
    limit: usize,
}

#[derive(Debug, Parser)]
struct TokenizeArgs {
    input: Option<String>,
    #[arg(long)]
    stdin: bool,
    #[arg(long, value_enum, default_value_t = InputFormat::Text)]
    input_format: InputFormat,
    #[arg(long, default_value_t = 1)]
    jobs: usize,
    #[arg(long, value_enum, default_value_t = TokenizeModeArg::Whitespace)]
    mode: TokenizeModeArg,
    #[arg(long, default_value_t = OutputFormat::Text, value_enum)]
    output: OutputFormat,
}

#[derive(Debug, Parser)]
struct NormalizeArgs {
    input: Option<String>,
    #[arg(long)]
    stdin: bool,
    #[arg(long, value_enum, default_value_t = NormalizeModeArg::Turkish)]
    mode: NormalizeModeArg,
    #[arg(long, default_value_t = OutputFormat::Text, value_enum)]
    output: OutputFormat,
}

#[derive(Debug, Parser)]
struct SegmentArgs {
    input: Option<String>,
    #[arg(long)]
    stdin: bool,
    #[arg(long, value_enum, default_value_t = InputFormat::Text)]
    input_format: InputFormat,
    #[arg(long, default_value_t = 1)]
    jobs: usize,
    #[arg(long, value_enum, default_value_t = SegmentModeArg::Turkish)]
    mode: SegmentModeArg,
    #[arg(long, default_value_t = OutputFormat::Text, value_enum)]
    output: OutputFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ModeArg {
    Graphemes,
    Chars,
    Words,
    Lines,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Debug,
    Json,
    Jsonl,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum InputFormat {
    Text,
    Jsonl,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TokenizeModeArg {
    Turkish,
    Whitespace,
    Graphemes,
    Lines,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum NormalizeModeArg {
    Basic,
    Turkish,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SegmentModeArg {
    Turkish,
}

impl From<ModeArg> for ReverseMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Graphemes => ReverseMode::Graphemes,
            ModeArg::Chars => ReverseMode::Chars,
            ModeArg::Words => ReverseMode::Words,
            ModeArg::Lines => ReverseMode::Lines,
        }
    }
}

impl From<TokenizeModeArg> for TokenizeMode {
    fn from(value: TokenizeModeArg) -> Self {
        match value {
            TokenizeModeArg::Turkish => TokenizeMode::Turkish,
            TokenizeModeArg::Whitespace => TokenizeMode::Whitespace,
            TokenizeModeArg::Graphemes => TokenizeMode::Graphemes,
            TokenizeModeArg::Lines => TokenizeMode::Lines,
        }
    }
}

impl From<NormalizeModeArg> for NormalizeMode {
    fn from(value: NormalizeModeArg) -> Self {
        match value {
            NormalizeModeArg::Basic => NormalizeMode::Basic,
            NormalizeModeArg::Turkish => NormalizeMode::Turkish,
        }
    }
}

impl From<SegmentModeArg> for SegmentMode {
    fn from(value: SegmentModeArg) -> Self {
        match value {
            SegmentModeArg::Turkish => SegmentMode::Turkish,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Morph(args) => run_morph(args),
        Command::Disamb(args) => run_disamb(args),
        Command::Spell(args) => {
            let checker = tekir_spell::SpellChecker::default();
            let report = checker.check(&args.input, args.limit);
            if report.correct {
                println!("ok");
            } else {
                println!("misspelled; suggestions: {}", report.suggestions.join(", "));
            }
            Ok(())
        }
        #[cfg(feature = "server")]
        Command::Serve(args) => server::run_serve(args),
        Command::Reverse(args) => run_reverse(args),
        Command::Normalize(args) => run_normalize(args),
        Command::Segment(args) => run_segment(args),
        Command::Tokenize(args) => run_tokenize(args),
    }
}

fn run_morph(args: MorphArgs) -> Result<()> {
    let service = if args.lexicon.is_empty() {
        MorphService::new()
    } else {
        let lexicon =
            Lexicon::from_tsv_paths(&args.lexicon).context("failed to load lexicon files")?;
        MorphService::with_lexicon(lexicon)
    };

    if args.input_format == InputFormat::Jsonl {
        let output = args.output;
        return run_morph_jsonl(&service, args, output);
    }

    let input = resolve_input(args.input, args.stdin)?;
    let response = service.analyze(input);

    match args.output {
        OutputFormat::Text => print_morph_plain(&response, args.best_only),
        OutputFormat::Debug => print_morph_debug(&response, args.best_only),
        OutputFormat::Json => print_json("morph", &response)?,
        OutputFormat::Jsonl => print_jsonl_record("morph", None, &response)?,
    }

    Ok(())
}

fn print_morph_plain(response: &tekir_morph::MorphResponse, best_only: bool) {
    for analysis in morph_analyses_for_output(response, best_only) {
        println!("{}", analysis.lexical_form);
    }
}

fn print_morph_debug(response: &tekir_morph::MorphResponse, best_only: bool) {
    println!("input_len_bytes: {}", response.input_len_bytes);
    println!("analysis_count: {}", response.output.analysis_count);
    println!("best_only: {}", best_only);
    println!(
        "best_analysis: {}",
        response
            .output
            .best_analysis
            .as_ref()
            .map(|analysis| analysis.lexical_form.as_str())
            .unwrap_or("<none>")
    );
    println!("original: {}", response.original);
    println!();
    let analyses = morph_analyses_for_output(response, best_only);
    for (index, analysis) in analyses.iter().enumerate() {
        println!("analysis {}:", index + 1);
        println!("  lexical: {}", analysis.lexical_form);
        println!("  lemma: {}", analysis.lemma);
        println!("  surface: {}", analysis.surface);
        println!("  pos: {:?}", analysis.pos);
        println!("  igs: {}", analysis.igs.len());
        if index + 1 < analyses.len() {
            println!();
        }
    }
}

fn morph_analyses_for_output(
    response: &tekir_morph::MorphResponse,
    best_only: bool,
) -> &[tekir_morph::MorphAnalysis] {
    if best_only {
        response.output.best_analysis.as_slice()
    } else {
        &response.output.analyses
    }
}

fn run_disamb(args: DisambArgs) -> Result<()> {
    let service = if args.lexicon.is_empty() {
        MorphService::new()
    } else {
        let lexicon =
            Lexicon::from_tsv_paths(&args.lexicon).context("failed to load lexicon files")?;
        MorphService::with_lexicon(lexicon)
    };
    let disambiguator = match &args.model {
        Some(path) => tekir_disamb::Disambiguator::load(path)
            .with_context(|| format!("failed to load model {}", path.display()))?,
        None => tekir_disamb::Disambiguator::new(tekir_disamb::bundled_model()),
    };
    let pipeline = tekir_disamb::Pipeline::new(service, disambiguator);

    if args.input_format == InputFormat::Jsonl {
        match args.output {
            OutputFormat::Text | OutputFormat::Debug => {
                anyhow::bail!("--input-format jsonl requires --output json or --output jsonl")
            }
            OutputFormat::Json | OutputFormat::Jsonl => {}
        }
        return with_jsonl_reader(args.input, args.stdin, |reader| {
            process_jsonl_stream("disamb", reader, args.jobs, |text| {
                pipeline.analyze_sentence(&text)
            })
        });
    }

    let input = resolve_input(args.input, args.stdin)?;
    let sentence = pipeline.analyze_sentence(&input);

    match args.output {
        OutputFormat::Text | OutputFormat::Debug => {
            for token in &sentence.tokens {
                match &token.analysis {
                    Some(analysis) => println!("{}\t{}", token.surface, analysis.lexical_form),
                    None => println!("{}\t<none>", token.surface),
                }
            }
        }
        OutputFormat::Json => print_json("disamb", &sentence)?,
        OutputFormat::Jsonl => print_jsonl_record("disamb", None, &sentence)?,
    }
    Ok(())
}

fn run_reverse(args: ReverseArgs) -> Result<()> {
    let input = resolve_input(args.input, args.stdin)?;
    let service = ReverseService;
    let response = service.reverse(input, args.mode.into());

    match args.output {
        OutputFormat::Text => println!("{}", response.output.transformed),
        OutputFormat::Debug => {
            println!("mode: {:?}", response.output.mode);
            println!("input_len_bytes: {}", response.input_len_bytes);
            println!("output_len_bytes: {}", response.output.output_len_bytes);
            println!("original: {}", response.original);
            println!("transformed: {}", response.output.transformed);
        }
        OutputFormat::Json => print_json("reverse", &response)?,
        OutputFormat::Jsonl => anyhow::bail!("reverse does not support --output jsonl"),
    }

    Ok(())
}

fn run_tokenize(args: TokenizeArgs) -> Result<()> {
    let service = TokenizeService;

    if args.input_format == InputFormat::Jsonl {
        let mode = args.mode.into();
        let output = args.output;
        return run_tokenize_jsonl(&service, args, mode, output);
    }

    let input = resolve_input(args.input, args.stdin)?;
    let response = service.tokenize(input, args.mode.into());

    match args.output {
        OutputFormat::Text => println!(
            "{}",
            response
                .output
                .tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        ),
        OutputFormat::Debug => {
            println!("mode: {:?}", response.output.mode);
            println!("input_len_bytes: {}", response.input_len_bytes);
            println!("token_count: {}", response.output.token_count);
            println!("original: {}", response.original);
            for token in &response.output.tokens {
                println!(
                    "token\tkind={:?}\tspan={}..{}\ttext={}",
                    token.kind, token.start_byte, token.end_byte, token.text
                );
            }
        }
        OutputFormat::Json => print_json("tokenize", &response)?,
        OutputFormat::Jsonl => print_jsonl_record("tokenize", None, &response)?,
    }

    Ok(())
}

fn run_normalize(args: NormalizeArgs) -> Result<()> {
    let input = resolve_input(args.input, args.stdin)?;
    let service = NormalizeService;
    let response = service.normalize(input, args.mode.into());

    match args.output {
        OutputFormat::Text => println!("{}", response.output.normalized),
        OutputFormat::Debug => {
            println!("mode: {:?}", response.output.mode);
            println!("input_len_bytes: {}", response.input_len_bytes);
            println!("output_len_bytes: {}", response.output.output_len_bytes);
            println!("original: {}", response.original);
            println!("normalized: {}", response.output.normalized);
        }
        OutputFormat::Json => print_json("normalize", &response)?,
        OutputFormat::Jsonl => anyhow::bail!("normalize does not support --output jsonl"),
    }

    Ok(())
}

fn run_segment(args: SegmentArgs) -> Result<()> {
    let service = SegmentService;

    if args.input_format == InputFormat::Jsonl {
        let mode = args.mode.into();
        let output = args.output;
        return run_segment_jsonl(&service, args, mode, output);
    }

    let input = resolve_input(args.input, args.stdin)?;
    let response = service.split(input, args.mode.into());

    match args.output {
        OutputFormat::Text => println!(
            "{}",
            response
                .output
                .segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        ),
        OutputFormat::Debug => {
            println!("mode: {:?}", response.output.mode);
            println!("input_len_bytes: {}", response.input_len_bytes);
            println!("segment_count: {}", response.output.segment_count);
            println!("original: {}", response.original);
            for segment in &response.output.segments {
                println!(
                    "segment\tkind={:?}\tspan={}..{}\ttext={}",
                    segment.kind, segment.start_byte, segment.end_byte, segment.text
                );
            }
        }
        OutputFormat::Json => print_json("segment", &response)?,
        OutputFormat::Jsonl => print_jsonl_record("segment", None, &response)?,
    }

    Ok(())
}

fn resolve_input(input: Option<String>, stdin: bool) -> Result<String> {
    match (input, stdin) {
        (Some(value), false) => Ok(value),
        (None, true) => read_stdin(),
        (Some(_), true) => anyhow::bail!("provide either a positional input or --stdin, not both"),
        (None, false) => anyhow::bail!("missing input: provide text or pass --stdin"),
    }
}

fn read_stdin() -> Result<String> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .context("failed to read stdin")?;
    Ok(buffer)
}

fn print_json<T: Serialize>(task: &'static str, value: &T) -> Result<()> {
    let payload = JsonEnvelope {
        schema_version: "1",
        task,
        data: value,
    };
    let json = serde_json::to_string_pretty(&payload).context("failed to serialize JSON output")?;
    println!("{json}");
    Ok(())
}

fn print_jsonl_record<T: Serialize>(task: &'static str, id: Option<&str>, value: &T) -> Result<()> {
    let json = serialize_jsonl_record(task, id, value)?;
    println!("{json}");
    Ok(())
}

fn serialize_jsonl_record<T: Serialize>(
    task: &'static str,
    id: Option<&str>,
    value: &T,
) -> Result<String> {
    let payload = JsonlEnvelope {
        schema_version: "1",
        task,
        id,
        data: value,
    };
    serde_json::to_string(&payload).context("failed to serialize JSONL output")
}

fn run_tokenize_jsonl(
    service: &TokenizeService,
    args: TokenizeArgs,
    mode: TokenizeMode,
    output: OutputFormat,
) -> Result<()> {
    match output {
        OutputFormat::Text | OutputFormat::Debug => {
            anyhow::bail!("--input-format jsonl requires --output json or --output jsonl")
        }
        OutputFormat::Json | OutputFormat::Jsonl => {}
    }

    with_jsonl_reader(args.input, args.stdin, |reader| {
        process_jsonl_stream("tokenize", reader, args.jobs, |text| {
            service.tokenize(text, mode)
        })
    })
}

fn run_morph_jsonl(service: &MorphService, args: MorphArgs, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Text | OutputFormat::Debug => {
            anyhow::bail!("--input-format jsonl requires --output json or --output jsonl")
        }
        OutputFormat::Json | OutputFormat::Jsonl => {}
    }

    with_jsonl_reader(args.input, args.stdin, |reader| {
        process_jsonl_stream("morph", reader, args.jobs, |text| service.analyze(text))
    })
}

fn run_segment_jsonl(
    service: &SegmentService,
    args: SegmentArgs,
    mode: SegmentMode,
    output: OutputFormat,
) -> Result<()> {
    match output {
        OutputFormat::Text | OutputFormat::Debug => {
            anyhow::bail!("--input-format jsonl requires --output json or --output jsonl")
        }
        OutputFormat::Json | OutputFormat::Jsonl => {}
    }

    with_jsonl_reader(args.input, args.stdin, |reader| {
        process_jsonl_stream("segment", reader, args.jobs, |text| {
            service.split(text, mode)
        })
    })
}

fn with_jsonl_reader<F>(input: Option<String>, stdin: bool, f: F) -> Result<()>
where
    F: FnOnce(Box<dyn BufRead>) -> Result<()>,
{
    match (input, stdin) {
        (Some(value), false) => f(Box::new(Cursor::new(value))),
        (None, true) => f(Box::new(io::BufReader::new(io::stdin().lock()))),
        (Some(_), true) => anyhow::bail!("provide either a positional input or --stdin, not both"),
        (None, false) => anyhow::bail!("missing input: provide JSONL text or pass --stdin"),
    }
}

fn process_jsonl_stream<T, F>(
    task: &'static str,
    reader: Box<dyn BufRead>,
    jobs: usize,
    process: F,
) -> Result<()>
where
    T: Serialize + Send + 'static,
    F: Fn(String) -> T + Send + Sync,
{
    let jobs = jobs.max(1);
    if jobs == 1 {
        return process_jsonl_stream_sequential(task, reader, process);
    }

    process_jsonl_stream_parallel(task, reader, jobs, process)
}

fn process_jsonl_stream_sequential<T, F>(
    task: &'static str,
    reader: Box<dyn BufRead>,
    process: F,
) -> Result<()>
where
    T: Serialize,
    F: Fn(String) -> T,
{
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for (line_idx, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("failed to read JSONL line {}", line_idx + 1))?;
        if line.trim().is_empty() {
            continue;
        }

        let record = serde_json::from_str::<JsonlInputRecord>(&line)
            .with_context(|| format!("failed to parse JSONL record on line {}", line_idx + 1))?;
        let response = process(record.text);
        let json = serialize_jsonl_record(task, record.id.as_deref(), &response)?;
        writeln!(out, "{json}").context("failed to write JSONL output")?;
    }

    Ok(())
}

fn process_jsonl_stream_parallel<T, F>(
    task: &'static str,
    reader: Box<dyn BufRead>,
    jobs: usize,
    process: F,
) -> Result<()>
where
    T: Serialize + Send + 'static,
    F: Fn(String) -> T + Send + Sync,
{
    let (task_tx, task_rx) = mpsc::channel::<BatchTask>();
    let task_rx = Arc::new(Mutex::new(task_rx));
    let (result_tx, result_rx) = mpsc::channel::<BatchResult>();
    let process = Arc::new(process);

    thread::scope(|scope| -> Result<()> {
        for _ in 0..jobs {
            let task_rx = Arc::clone(&task_rx);
            let result_tx = result_tx.clone();
            let process = Arc::clone(&process);

            scope.spawn(move || {
                loop {
                    let batch_task = match task_rx.lock().expect("task lock poisoned").recv() {
                        Ok(task) => task,
                        Err(_) => break,
                    };

                    let response = process(batch_task.text);
                    let json = serialize_jsonl_record(task, batch_task.id.as_deref(), &response)
                        .map_err(|err| err.to_string());

                    if result_tx
                        .send(BatchResult {
                            sequence: batch_task.sequence,
                            json,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }
        drop(result_tx);

        let mut task_count = 0usize;
        for (line_idx, line) in reader.lines().enumerate() {
            let line =
                line.with_context(|| format!("failed to read JSONL line {}", line_idx + 1))?;
            if line.trim().is_empty() {
                continue;
            }

            let record = serde_json::from_str::<JsonlInputRecord>(&line).with_context(|| {
                format!("failed to parse JSONL record on line {}", line_idx + 1)
            })?;

            task_tx
                .send(BatchTask {
                    sequence: task_count,
                    id: record.id,
                    text: record.text,
                })
                .context("failed to queue JSONL batch task")?;
            task_count += 1;
        }
        drop(task_tx);

        let stdout = io::stdout();
        let mut out = stdout.lock();
        let mut expected = 0usize;
        let mut pending = BTreeMap::new();

        for _ in 0..task_count {
            let result = result_rx
                .recv()
                .context("JSONL worker channel closed unexpectedly")?;
            pending.insert(result.sequence, result.json);

            while let Some(json) = pending.remove(&expected) {
                let json = json.map_err(anyhow::Error::msg)?;
                writeln!(out, "{json}").context("failed to write JSONL output")?;
                expected += 1;
            }
        }

        Ok(())
    })
}

#[derive(Serialize)]
struct JsonEnvelope<'a, T> {
    schema_version: &'static str,
    task: &'static str,
    data: &'a T,
}

#[derive(Serialize)]
struct JsonlEnvelope<'a, T> {
    schema_version: &'static str,
    task: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    data: &'a T,
}

#[derive(Debug, Deserialize)]
struct JsonlInputRecord {
    #[serde(default)]
    id: Option<String>,
    text: String,
}

#[derive(Debug)]
struct BatchTask {
    sequence: usize,
    id: Option<String>,
    text: String,
}

#[derive(Debug)]
struct BatchResult {
    sequence: usize,
    json: std::result::Result<String, String>,
}

#[cfg(test)]
mod tests {
    use super::{
        InputFormat, ModeArg, MorphArgs, NormalizeArgs, NormalizeModeArg, OutputFormat,
        ReverseArgs, SegmentArgs, SegmentModeArg, TokenizeArgs, TokenizeModeArg,
    };

    #[test]
    fn morph_args_support_best_only() {
        let args = MorphArgs {
            input: Some("evler".to_string()),
            stdin: false,
            input_format: InputFormat::Text,
            jobs: 1,
            best_only: true,
            lexicon: Vec::new(),
            output: OutputFormat::Debug,
        };

        assert_eq!(args.input.as_deref(), Some("evler"));
        assert!(!args.stdin);
        assert!(args.best_only);
    }

    #[test]
    fn reverse_args_support_non_stdin_text_output() {
        let args = ReverseArgs {
            input: Some("tekir".to_string()),
            stdin: false,
            mode: ModeArg::Chars,
            output: OutputFormat::Text,
        };

        assert_eq!(args.input.as_deref(), Some("tekir"));
        assert!(!args.stdin);
    }

    #[test]
    fn tokenize_args_support_whitespace_mode() {
        let args = TokenizeArgs {
            input: Some("tekir bir iki".to_string()),
            stdin: false,
            input_format: InputFormat::Text,
            jobs: 1,
            mode: TokenizeModeArg::Whitespace,
            output: OutputFormat::Debug,
        };

        assert_eq!(args.input.as_deref(), Some("tekir bir iki"));
        assert!(!args.stdin);
    }

    #[test]
    fn normalize_args_default_to_text_output() {
        let args = NormalizeArgs {
            input: Some(" İSTANBUL ".to_string()),
            stdin: false,
            mode: NormalizeModeArg::Turkish,
            output: OutputFormat::Text,
        };

        assert_eq!(args.input.as_deref(), Some(" İSTANBUL "));
        assert!(!args.stdin);
    }

    #[test]
    fn segment_args_support_turkish_mode() {
        let args = SegmentArgs {
            input: Some("Merhaba. Nasılsın?".to_string()),
            stdin: false,
            input_format: InputFormat::Text,
            jobs: 1,
            mode: SegmentModeArg::Turkish,
            output: OutputFormat::Debug,
        };

        assert_eq!(args.input.as_deref(), Some("Merhaba. Nasılsın?"));
        assert!(!args.stdin);
    }

    #[test]
    fn serializes_jsonl_envelope_with_optional_id() {
        let json = super::serialize_jsonl_record("segment", Some("a1"), &"Merhaba")
            .expect("jsonl should serialize");

        assert!(json.contains("\"schema_version\":\"1\""));
        assert!(json.contains("\"task\":\"segment\""));
        assert!(json.contains("\"id\":\"a1\""));
        assert!(json.contains("\"data\":\"Merhaba\""));
    }
}
