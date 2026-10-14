//! # kubirag-llm
//!
//! Adattatore della porta [`LlmGateway`] verso Ollama, con il comportamento di
//! `OllamaClient3.executeStreamRequest`:
//! - `POST {base}/generate` con `stream: true`, una riga JSON per frammento;
//! - si ferma al primo `done: true`, salta le righe vuote e i frammenti vuoti;
//! - interruttori: 100 000 frammenti, 5 righe illeggibili, `maxStreamChars` caratteri;
//! - `POST {base}/embeddings` con `{model, prompt}` → `{embedding}`.
//!
//! Il tempo massimo della richiesta (`GenerateRequest::timeout`) diventa [`LlmError::Timeout`],
//! che il rerank LLM riporta come esito `TIMEOUT`.

use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

use kubirag_app::retrieval::{GenerateRequest, LlmError, LlmGateway};
use serde::Deserialize;
use serde_json::{json, Map, Value};

const MAX_STREAM_CHUNKS: usize = 100_000;
const MAX_PARSE_ERRORS: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Gateway Ollama. `base_url` comprende `/api` (`http://localhost:11434/api`).
pub struct OllamaGateway {
    generate_base: String,
    embed_base: String,
    agent: ureq::Agent,
}

#[derive(Deserialize)]
struct GenerateChunk {
    response: String,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

impl OllamaGateway {
    pub fn new(base_url: &str) -> Self {
        Self::with_bases(base_url, base_url)
    }

    /// La generazione Kotlin usa `OLLAMA_BASE_URL` (o il default), gli embedding
    /// `ollamaBaseUrl` della configurazione: qui la variabile d'ambiente, se c'è, vince solo
    /// per la generazione.
    pub fn from_config(config_base_url: &str) -> Self {
        let generate = std::env::var("OLLAMA_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| config_base_url.to_owned());
        Self::with_bases(&generate, config_base_url)
    }

    fn with_bases(generate_base: &str, embed_base: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .into();
        Self {
            generate_base: generate_base.trim_end_matches('/').to_owned(),
            embed_base: embed_base.trim_end_matches('/').to_owned(),
            agent,
        }
    }

    fn post(
        &self,
        url: &str,
        body: &Value,
        timeout: Option<Duration>,
    ) -> Result<ureq::http::Response<ureq::Body>, LlmError> {
        self.agent
            .post(url)
            .header("Content-Type", "application/json")
            .config()
            .timeout_global(timeout)
            .build()
            .send(body.to_string())
            .map_err(map_error)
    }
}

fn map_error(error: ureq::Error) -> LlmError {
    match error {
        ureq::Error::Timeout(_) => LlmError::Timeout,
        ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::TimedOut => LlmError::Timeout,
        other => LlmError::Unavailable(format!("Error during Ollama request: {other}")),
    }
}

fn request_body(request: &GenerateRequest) -> Value {
    let mut body = Map::new();
    body.insert("model".into(), json!(request.model));
    body.insert("prompt".into(), json!(request.prompt));
    body.insert("stream".into(), json!(true));
    if let Some(format) = &request.format {
        body.insert("format".into(), json!(format));
    }
    if !request.options.is_empty() {
        body.insert("options".into(), Value::Object(request.options.clone()));
    }
    if let Some(system) = &request.system {
        body.insert("system".into(), json!(system));
    }
    if let Some(think) = request.think {
        body.insert("think".into(), json!(think));
    }
    Value::Object(body)
}

impl LlmGateway for OllamaGateway {
    fn generate(
        &self,
        request: &GenerateRequest,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<String, LlmError> {
        let started = Instant::now();
        let timed_out = |error: std::io::Error| {
            let expired = request.timeout.is_some_and(|t| started.elapsed() >= t);
            if expired || error.kind() == std::io::ErrorKind::TimedOut {
                LlmError::Timeout
            } else {
                LlmError::Unavailable(format!("Error during Ollama streaming request: {error}"))
            }
        };
        let url = format!("{}/generate", self.generate_base);
        let mut response = self.post(&url, &request_body(request), request.timeout)?;
        if response.status() != 200 {
            return Err(LlmError::Unavailable(format!(
                "Ollama request failed: {}",
                response.status().as_u16()
            )));
        }
        let mut reader = BufReader::new(response.body_mut().as_reader());
        let mut text = String::new();
        let (mut chunks, mut parse_errors, mut total_chars) = (0, 0, 0);
        let mut line = Vec::new();
        loop {
            line.clear();
            if reader.read_until(b'\n', &mut line).map_err(timed_out)? == 0 {
                break;
            }
            let content = String::from_utf8_lossy(&line);
            let content = content.trim_end_matches(['\n', '\r']);
            if content.is_empty() {
                continue;
            }
            if chunks >= MAX_STREAM_CHUNKS || parse_errors >= MAX_PARSE_ERRORS {
                break;
            }
            let Ok(chunk) = serde_json::from_str::<GenerateChunk>(content) else {
                parse_errors += 1;
                continue;
            };
            chunks += 1;
            if !chunk.response.is_empty() {
                on_chunk(&chunk.response);
                text.push_str(&chunk.response);
            }
            total_chars += chunk.response.encode_utf16().count();
            if total_chars > request.max_chars || chunk.done {
                break;
            }
        }
        Ok(text)
    }

    fn embed(&self, model: &str, text: &str) -> Result<Vec<f32>, LlmError> {
        let url = format!("{}/embeddings", self.embed_base);
        let mut response = self.post(&url, &json!({ "model": model, "prompt": text }), None)?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| LlmError::Unavailable(e.to_string()))?;
        if status != 200 {
            return Err(LlmError::Unavailable(format!(
                "Errore nella chiamata API: {status} {body}"
            )));
        }
        serde_json::from_str::<EmbeddingResponse>(&body)
            .map(|r| r.embedding)
            .map_err(|e| LlmError::Protocol(e.to_string()))
    }
}
