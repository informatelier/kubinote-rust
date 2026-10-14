//! Il gateway contro un Ollama finto su 127.0.0.1: protocollo NDJSON, interruttori, timeout.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use kubirag_app::retrieval::{GenerateRequest, LlmError, LlmGateway};
use kubirag_llm::OllamaGateway;
use serde_json::{json, Map, Value};

/// Serve una sola richiesta: restituisce `(status, body)` dopo `delay` e manda il corpo
/// della richiesta ricevuta sul canale.
fn serve(status: u16, body: &'static str, delay: Duration) -> (String, mpsc::Receiver<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/api", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut length = 0;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
        }
        let mut request = vec![0; length];
        reader.read_exact(&mut request).unwrap();
        let _ = sender.send(serde_json::from_slice(&request).unwrap_or(Value::Null));
        thread::sleep(delay);
        let mut stream = stream;
        let _ = write!(
            stream,
            "HTTP/1.1 {status} X\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    });
    (base, receiver)
}

fn request(max_chars: usize, timeout: Option<Duration>) -> GenerateRequest {
    let mut options = Map::new();
    options.insert("temperature".into(), json!(0.3));
    GenerateRequest {
        model: "qwen3:4b".into(),
        prompt: "domanda".into(),
        system: None,
        options,
        format: Some("json".into()),
        think: Some(false),
        timeout,
        max_chars,
    }
}

fn generate(base: &str, request: &GenerateRequest) -> (Result<String, LlmError>, Vec<String>) {
    let mut chunks = Vec::new();
    let result = OllamaGateway::new(base).generate(request, &mut |c| chunks.push(c.to_owned()));
    (result, chunks)
}

#[test]
fn the_stream_stops_at_done_and_skips_empty_chunks() {
    let body = "{\"response\":\"Cia\",\"done\":false}\n\n{\"response\":\"\",\"done\":false}\r\n{\"response\":\"o\",\"done\":true}\n{\"response\":\"dopo\",\"done\":false}\n";
    let (base, received) = serve(200, body, Duration::ZERO);
    let (result, chunks) = generate(&base, &request(64_000, None));
    assert_eq!(result.unwrap(), "Ciao");
    assert_eq!(chunks, ["Cia", "o"]);
    let sent = received.recv().unwrap();
    assert_eq!(
        sent,
        json!({"model":"qwen3:4b","prompt":"domanda","stream":true,"format":"json","options":{"temperature":0.3},"think":false}),
        "null omessi, come Jackson NON_NULL"
    );
}

#[test]
fn five_unreadable_lines_abort_the_stream() {
    let body = "x\nx\nx\nx\nx\n{\"response\":\"mai\",\"done\":true}\n";
    let (base, _) = serve(200, body, Duration::ZERO);
    assert_eq!(generate(&base, &request(64_000, None)).0.unwrap(), "");
    let body = "x\nx\nx\nx\n{\"response\":\"ok\",\"done\":true}\n";
    let (base, _) = serve(200, body, Duration::ZERO);
    assert_eq!(generate(&base, &request(64_000, None)).0.unwrap(), "ok");
}

#[test]
fn the_content_breaker_keeps_the_chunk_that_crossed_it() {
    let body =
        "{\"response\":\"abc\"}\n{\"response\":\"def\"}\n{\"response\":\"ghi\",\"done\":true}\n";
    let (base, _) = serve(200, body, Duration::ZERO);
    assert_eq!(generate(&base, &request(4, None)).0.unwrap(), "abcdef");
}

#[test]
fn http_errors_and_timeouts_are_distinct() {
    let (base, _) = serve(500, "boom", Duration::ZERO);
    assert!(matches!(
        generate(&base, &request(10, None)).0,
        Err(LlmError::Unavailable(_))
    ));
    let (base, _) = serve(
        200,
        "{\"response\":\"tardi\",\"done\":true}\n",
        Duration::from_millis(1500),
    );
    let result = generate(&base, &request(10, Some(Duration::from_millis(200)))).0;
    assert_eq!(result, Err(LlmError::Timeout));
}

#[test]
fn an_unreachable_server_is_unavailable() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let base = format!("http://127.0.0.1:{port}/api");
    assert!(matches!(
        generate(&base, &request(10, None)).0,
        Err(LlmError::Unavailable(_))
    ));
}

#[test]
fn embeddings_are_parsed_or_reported() {
    let (base, received) = serve(200, "{\"embedding\":[0.5,-1.0,3]}", Duration::ZERO);
    let vector = OllamaGateway::new(&base).embed("nomic", "testo").unwrap();
    assert_eq!(vector, [0.5, -1.0, 3.0]);
    assert_eq!(
        received.recv().unwrap(),
        json!({"model":"nomic","prompt":"testo"})
    );
    let (base, _) = serve(404, "model not found", Duration::ZERO);
    let error = OllamaGateway::new(&base).embed("x", "y").unwrap_err();
    assert_eq!(
        error,
        LlmError::Unavailable("Errore nella chiamata API: 404 model not found".into())
    );
}
