//! Ollama finto e deterministico, gemello di `contracts/tools/fake_ollama.py` con cui sono
//! stati registrati i golden HTTP:
//!
//! - `GET  /api/tags`: due modelli, `qwen3:4b` e `nomic-embed-text`;
//! - `POST /api/show`: `qwen3.context_length` 40960 per qwen3, nessun context_length per gli altri;
//! - `POST /api/generate`: tre frammenti NDJSON, "Risposta ", "di prova", "." (con
//!   `format: json`, il rerank `{"ranking":[1]}`);
//! - `POST /api/embeddings`: `[1.0, lunghezza UTF-16 del testo modulo 10, 0.5]`.

use std::net::{TcpListener, TcpStream};
use std::thread;

use serde_json::{json, Value};

use crate::wire;

/// L'Ollama finto in ascolto su una porta libera di loopback, per tutta la durata del processo.
pub struct FakeOllama {
    /// `http://127.0.0.1:<porta>/api`, come `ollamaBaseUrl`.
    pub url: String,
}

pub fn start() -> FakeOllama {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake Ollama");
    let url = format!("http://{}/api", listener.local_addr().expect("local addr"));
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || {
                let _ = serve(stream);
            });
        }
    });
    FakeOllama { url }
}

fn serve(mut stream: TcpStream) -> std::io::Result<()> {
    let request = wire::read(&stream, true)?;
    let mut parts = request.start.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let body: Value = serde_json::from_slice(&request.body).unwrap_or_else(|_| json!({}));
    let (status, content_type, payload) = respond(method, path, &body);
    wire::write(
        &mut stream,
        &format!(
            "HTTP/1.1 {status} {}",
            if status == 200 { "OK" } else { "Not Found" }
        ),
        &[("Content-Type", content_type)],
        payload.as_bytes(),
    )
}

fn respond(method: &str, path: &str, request: &Value) -> (u16, &'static str, String) {
    const JSON: &str = "application/json";
    let text = |key: &str| request[key].as_str().unwrap_or("").to_owned();
    match (method, path) {
        ("GET", p) if p.ends_with("/api/tags") => (
            200,
            JSON,
            json!({"models": [
                {"name": "qwen3:4b", "model": "qwen3:4b", "size": 1},
                {"name": "nomic-embed-text", "model": "nomic-embed-text", "size": 1},
            ]})
            .to_string(),
        ),
        ("POST", p) if p.ends_with("/api/show") => {
            let name = Some(text("model"))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| text("name"));
            let info = if name.starts_with("qwen3") {
                json!({"general.architecture": "qwen3", "qwen3.context_length": 40960})
            } else {
                json!({"general.architecture": "nomic-bert"})
            };
            let body = json!({"modelfile": "", "template": "", "license": "", "parameters": "",
                "model_info": info, "details": {}, "capabilities": ["completion"],
                "modified_at": "2026-01-01T00:00:00Z"});
            (200, JSON, body.to_string())
        }
        ("POST", p) if p.ends_with("/api/embeddings") => {
            let length = text("prompt").encode_utf16().count();
            let embedding = json!({"embedding": [1.0, (length % 10) as f64, 0.5]});
            (200, JSON, embedding.to_string())
        }
        ("POST", p) if p.ends_with("/api/generate") => {
            let chunks: &[&str] = if request["format"] == "json" {
                &[r#"{"ranking":[1]}"#]
            } else {
                &["Risposta ", "di prova", "."]
            };
            let mut lines: Vec<String> = chunks
                .iter()
                .map(|c| json!({"response": c, "done": false}).to_string())
                .collect();
            lines.push(json!({"response": "", "done": true}).to_string());
            (200, "application/x-ndjson", lines.join("\n") + "\n")
        }
        _ => (404, JSON, "{}".to_owned()),
    }
}
