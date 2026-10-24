//! `tekir serve`: a thin HTTP JSON service over the tekir pipeline,
//! compiled in with `--features server`.
//!
//! Endpoints (JSON in, versioned envelope out, mirroring the CLI):
//! `GET /health`, `POST /morph`, `POST /disamb`, `POST /tokenize`,
//! `POST /segment`, `POST /normalize`. Body: `{"text": "...", "mode":
//! "..."}` (mode optional). Configuration comes from flags or a
//! `key = value` file (`host`, `port`, `threads`, `model`, `lexicon`,
//! repeatable), flags winning.

use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Serialize;
use tekir_morph::{Lexicon, MorphService};
use tekir_normalize::{NormalizeMode, NormalizeService};
use tekir_segment::{SegmentMode, SegmentService};
use tekir_tokenize::{TokenizeMode, TokenizeService};
use tiny_http::{Header, Method, Response, Server};

use crate::ServeArgs;

#[derive(Debug, Default, Clone)]
pub struct ServeConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub threads: Option<usize>,
    pub model: Option<std::path::PathBuf>,
    pub lexicon: Vec<std::path::PathBuf>,
}

/// Parse the thin `key = value` config format.
pub fn parse_config(text: &str) -> Result<ServeConfig> {
    let mut config = ServeConfig::default();
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .with_context(|| format!("config line {} is not key = value", index + 1))?;
        let value = value.trim();
        match key.trim() {
            "host" => config.host = Some(value.to_string()),
            "port" => config.port = Some(value.parse().context("port must be a number")?),
            "threads" => config.threads = Some(value.parse().context("threads must be a number")?),
            "model" => config.model = Some(value.into()),
            "lexicon" => config.lexicon.push(value.into()),
            other => anyhow::bail!("unknown config key {other:?}"),
        }
    }
    Ok(config)
}

struct Services {
    pipeline: tekir_disamb::Pipeline,
    spell: tekir_spell::SpellChecker,
    tokenize: TokenizeService,
    segment: SegmentService,
    normalize: NormalizeService,
}

#[derive(Debug, serde::Deserialize)]
struct RequestBody {
    text: String,
    #[serde(default)]
    mode: Option<String>,
}

pub fn run_serve(args: ServeArgs) -> Result<()> {
    let mut config = match &args.config {
        Some(path) => parse_config(
            &std::fs::read_to_string(path)
                .with_context(|| format!("failed to read config {}", path.display()))?,
        )?,
        None => ServeConfig::default(),
    };
    // Flags override the file.
    if let Some(host) = args.host {
        config.host = Some(host);
    }
    if let Some(port) = args.port {
        config.port = Some(port);
    }
    if let Some(threads) = args.threads {
        config.threads = Some(threads);
    }
    if let Some(model) = args.model {
        config.model = Some(model);
    }
    if !args.lexicon.is_empty() {
        config.lexicon = args.lexicon;
    }

    let host = config.host.as_deref().unwrap_or("127.0.0.1");
    let port = config.port.unwrap_or(8321);
    let threads = config.threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    });

    let service = if config.lexicon.is_empty() {
        MorphService::new()
    } else {
        MorphService::with_lexicon(
            Lexicon::from_tsv_paths(&config.lexicon).context("failed to load lexicon files")?,
        )
    };
    let disambiguator = match &config.model {
        Some(path) => tekir_disamb::Disambiguator::load(path)
            .with_context(|| format!("failed to load model {}", path.display()))?,
        None => tekir_disamb::Disambiguator::new(tekir_disamb::bundled_model()),
    };
    let services = Arc::new(Services {
        spell: tekir_spell::SpellChecker::new(service.clone()),
        pipeline: tekir_disamb::Pipeline::new(service, disambiguator),
        tokenize: TokenizeService,
        segment: SegmentService,
        normalize: NormalizeService,
    });

    let address = format!("{host}:{port}");
    let server = Arc::new(
        Server::http(&address)
            .map_err(|error| anyhow::anyhow!("failed to bind {address}: {error}"))?,
    );
    eprintln!("tekir serving on http://{address} with {threads} worker threads");

    let mut workers = Vec::with_capacity(threads);
    for _ in 0..threads {
        let server = Arc::clone(&server);
        let services = Arc::clone(&services);
        workers.push(std::thread::spawn(move || {
            while let Ok(request) = server.recv() {
                handle(request, &services);
            }
        }));
    }
    for worker in workers {
        let _ = worker.join();
    }
    Ok(())
}

fn handle(mut request: tiny_http::Request, services: &Services) {
    let method = request.method().clone();
    let url = request.url().to_string();

    let response = match (method, url.as_str()) {
        (Method::Get, "/health") => json_response(
            200,
            &serde_json::json!({ "status": "ok", "service": "tekir" }),
        ),
        (Method::Post, path) => {
            let mut body = String::new();
            if std::io::Read::read_to_string(request.as_reader(), &mut body).is_err() {
                error_response(400, "unreadable body")
            } else {
                match serde_json::from_str::<RequestBody>(&body) {
                    Err(error) => error_response(400, &format!("invalid JSON body: {error}")),
                    Ok(body) => dispatch(path, &body, services),
                }
            }
        }
        _ => error_response(404, "unknown endpoint"),
    };

    let _ = request.respond(response);
}

fn dispatch(
    path: &str,
    body: &RequestBody,
    services: &Services,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let mode = body.mode.as_deref();
    match path {
        "/morph" => envelope(
            "morph",
            &services.pipeline.service().analyze(body.text.clone()),
        ),
        "/disamb" => envelope("disamb", &services.pipeline.analyze_sentence(&body.text)),
        "/lemmas" => envelope("lemmas", &services.pipeline.lemmas(&body.text)),
        "/spell" => envelope("spell", &services.spell.check(&body.text, 8)),
        "/tokenize" => {
            let mode = match mode.unwrap_or("turkish") {
                "turkish" => TokenizeMode::Turkish,
                "whitespace" => TokenizeMode::Whitespace,
                "graphemes" => TokenizeMode::Graphemes,
                "lines" => TokenizeMode::Lines,
                other => return error_response(400, &format!("unknown tokenize mode {other:?}")),
            };
            envelope(
                "tokenize",
                &services.tokenize.tokenize(body.text.clone(), mode),
            )
        }
        "/segment" => match mode.unwrap_or("turkish") {
            "turkish" => envelope(
                "segment",
                &services
                    .segment
                    .split(body.text.clone(), SegmentMode::Turkish),
            ),
            other => error_response(400, &format!("unknown segment mode {other:?}")),
        },
        "/normalize" => {
            let mode = match mode.unwrap_or("turkish") {
                "turkish" => NormalizeMode::Turkish,
                "basic" => NormalizeMode::Basic,
                other => return error_response(400, &format!("unknown normalize mode {other:?}")),
            };
            envelope(
                "normalize",
                &services.normalize.normalize(body.text.clone(), mode),
            )
        }
        other => error_response(404, &format!("unknown endpoint {other:?}")),
    }
}

#[derive(Serialize)]
struct Envelope<'a, T> {
    schema_version: &'static str,
    task: &'a str,
    data: &'a T,
}

fn envelope<T: Serialize>(task: &str, data: &T) -> Response<std::io::Cursor<Vec<u8>>> {
    let payload = Envelope {
        schema_version: "1",
        task,
        data,
    };
    json_response(200, &payload)
}

fn json_response<T: Serialize>(status: u16, payload: &T) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(payload).unwrap_or_else(|_| b"{}".to_vec());
    Response::from_data(body)
        .with_status_code(status)
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                .expect("static header is valid"),
        )
}

fn error_response(status: u16, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(status, &serde_json::json!({ "error": message }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_thin_config() {
        let config = super::parse_config(
            "# comment\nhost = 0.0.0.0\nport = 9000\nthreads = 8\nlexicon = a.tsv\nlexicon = b.tsv\n",
        )
        .expect("config should parse");
        assert_eq!(config.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(config.port, Some(9000));
        assert_eq!(config.threads, Some(8));
        assert_eq!(config.lexicon.len(), 2);
        assert!(super::parse_config("bogus").is_err());
        assert!(super::parse_config("nope = 1").is_err());
    }
}
