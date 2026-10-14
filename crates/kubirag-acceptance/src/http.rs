//! Lo scenario HTTP di `contracts/golden/http/scenario.json`, eseguito contro un server in
//! ascolto e normalizzato con le stesse regole di `contracts/tools/http_golden.py`, che ha
//! registrato `kotlin-exchanges.json` contro il server Javalin:
//!
//! - l'URL dell'Ollama finto diventa `<ollama>`, i path del workspace e della home `<ws>` e
//!   `<home>` (separatori `/`);
//! - `correlationId` e `auditEventId` diventano segnaposto fissi, gli altri UUID `<uuid-N>`
//!   numerati per prima apparizione nell'intero scenario, un UUID troncato in fondo a un testo
//!   `<uuid-partial>`;
//! - `timestamp`/`updatedAt` non nulli diventano `<millis>`, gli istanti ISO `<instant>`;
//! - in `lastMessage` (troncato a lunghezza fissa) il testo dopo `<ws>`/`<home>` diventa `…`;
//! - gli asset statici sono confrontati con SHA-256 e lunghezza;
//! - la `version` di `qb-rag.json` è la build che l'ha scritto, quindi `<build>`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use regex::{Captures, Regex};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::wire;

const HEADERS: [&str; 6] = [
    "content-type",
    "etag",
    "x-qb-revision",
    "x-qb-source-reused",
    "x-qb-source-reindexed",
    "x-session-limit",
];
const FIXED_KEYS: [(&str, &str); 2] = [
    ("correlationId", "<correlation>"),
    ("auditEventId", "<audit-event>"),
];
const MILLIS_KEYS: [&str; 2] = ["timestamp", "updatedAt"];
/// Testi troncati a lunghezza fissa: il punto del taglio dipende dalla lunghezza del path del
/// workspace, quindi dopo `<ws>`/`<home>` non si confronta nulla.
const TRUNCATED_KEYS: [&str; 1] = ["lastMessage"];
const BOUNDARY: &str = "----kubirag-golden-boundary";

/// Sostituisce nei valori ciò che cambia da un'esecuzione all'altra.
pub struct Normalizer {
    roots: Vec<(String, &'static str)>,
    uuids: HashMap<String, String>,
    uuid: Regex,
    partial_uuid: Regex,
    instant: Regex,
}

impl Normalizer {
    pub fn new(ollama: &str, ws: &Path, home: &Path) -> Self {
        Self {
            roots: vec![
                (ollama.to_owned(), "<ollama>"),
                (ws.display().to_string(), "<ws>"),
                (home.display().to_string(), "<home>"),
            ],
            uuids: HashMap::new(),
            uuid: Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
                .unwrap(),
            partial_uuid: Regex::new(
                r"[0-9a-f]{8}-[0-9a-f]{4}(-[0-9a-f]{0,4}){0,2}(-[0-9a-f]{0,11})?$",
            )
            .unwrap(),
            instant: Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z").unwrap(),
        }
    }

    pub fn text(&mut self, value: &str) -> String {
        self.cut_text(value, false)
    }

    fn cut_text(&mut self, value: &str, truncated: bool) -> String {
        let mut value = value.to_owned();
        for (path, label) in &self.roots {
            value = value.replace(path.as_str(), label);
        }
        if value.contains("<ws>") || value.contains("<home>") {
            value = value.replace('\\', "/");
        }
        if truncated {
            let cut = ["<ws>", "<home>"]
                .iter()
                .filter_map(|label| value.find(label).map(|i| i + label.len()))
                .min();
            if let Some(cut) = cut {
                value = format!("{}…", &value[..cut]);
            }
        }
        let value = self.instant.replace_all(&value, "<instant>").into_owned();
        let uuids = &mut self.uuids;
        let value = self
            .uuid
            .replace_all(&value, |c: &Captures| {
                let next = format!("<uuid-{}>", uuids.len() + 1);
                uuids.entry(c[0].to_owned()).or_insert(next).clone()
            })
            .into_owned();
        self.partial_uuid
            .replace(&value, "<uuid-partial>")
            .into_owned()
    }

    pub fn value(&mut self, value: &Value, key: Option<&str>) -> Value {
        if let (Some(key), Value::String(_)) = (key, value) {
            if let Some((_, fixed)) = FIXED_KEYS.iter().find(|(k, _)| *k == key) {
                return json!(fixed);
            }
        }
        if let (Some(key), Value::Number(n)) = (key, value) {
            if MILLIS_KEYS.contains(&key) && (n.is_i64() || n.is_u64()) && n.as_f64() != Some(0.0) {
                return json!("<millis>");
            }
        }
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), self.value(v, Some(k))))
                    .collect(),
            ),
            Value::Array(items) => {
                Value::Array(items.iter().map(|v| self.value(v, None)).collect())
            }
            Value::String(s) => {
                Value::String(self.cut_text(s, key.is_some_and(|k| TRUNCATED_KEYS.contains(&k))))
            }
            other => other.clone(),
        }
    }
}

/// Il server sotto test e il suo workspace.
pub struct Target<'a> {
    /// `http://127.0.0.1:<porta>`.
    pub base: &'a str,
    pub ollama: &'a str,
    pub ws: &'a Path,
    pub home: &'a Path,
}

fn substitute(value: &Value, variables: &HashMap<String, String>) -> Value {
    match value {
        Value::String(s) => {
            let mut s = s.clone();
            for (k, v) in variables {
                s = s.replace(&format!("{{{k}}}"), v);
            }
            Value::String(s)
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), substitute(v, variables)))
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| substitute(v, variables)).collect())
        }
        other => other.clone(),
    }
}

fn multipart(fields: &Map<String, Value>, ws: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, field) in fields {
        out.extend(format!("--{BOUNDARY}\r\n").as_bytes());
        match field {
            Value::Object(file) => {
                let data = match file.get("fixture").and_then(Value::as_str) {
                    Some(fixture) => fs::read(ws.join(fixture)).expect("multipart fixture"),
                    None => file["content"].as_str().unwrap_or("").as_bytes().to_vec(),
                };
                let filename = file["filename"].as_str().unwrap_or("");
                out.extend(
                    format!(
                        "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n"
                    )
                    .as_bytes(),
                );
                out.extend(b"Content-Type: application/octet-stream\r\n\r\n");
                out.extend(data);
                out.extend(b"\r\n");
            }
            other => {
                let text = other
                    .as_str()
                    .map_or_else(|| other.to_string(), str::to_owned);
                out.extend(
                    format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n{text}\r\n")
                        .as_bytes(),
                );
            }
        }
    }
    out.extend(format!("--{BOUNDARY}--\r\n").as_bytes());
    out
}

fn decode_body(step: &Value, raw: &[u8], normalizer: &mut Normalizer) -> Map<String, Value> {
    let mut out = Map::new();
    if step["static"] == true {
        out.insert("sha256".into(), json!(format!("{:x}", Sha256::digest(raw))));
        out.insert("length".into(), json!(raw.len()));
        return out;
    }
    let text = String::from_utf8_lossy(raw);
    if text.starts_with("data: ") {
        let frames: Vec<Value> = text
            .split("\n\n")
            .filter(|f| !f.is_empty())
            .map(|f| serde_json::from_str(&f["data: ".len()..]).expect("SSE frame JSON"))
            .collect();
        out.insert("sse".into(), normalizer.value(&Value::Array(frames), None));
    } else if let Ok(value) = serde_json::from_str::<Value>(&text) {
        out.insert("json".into(), normalizer.value(&value, None));
    } else {
        out.insert("text".into(), json!(normalizer.text(&text)));
    }
    out
}

fn pointer<'v>(document: &'v Value, path: &str) -> &'v Value {
    path.trim_matches('/')
        .split('/')
        .fold(document, |d, part| match d {
            Value::Array(items) => &items[part.parse::<usize>().expect("array index")],
            other => &other[part],
        })
}

/// Una chiamata dello scenario, prima della normalizzazione: per il controllo con l'OpenAPI.
#[derive(Debug, Clone)]
pub struct Call {
    pub method: String,
    /// Il path senza query string.
    pub path: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

/// Lo scenario eseguito: il documento normalizzato `{"exchanges": [...], "files": {...}}`,
/// confrontabile con `kotlin-exchanges.json`, e le chiamate grezze.
pub struct Recording {
    pub document: Value,
    pub calls: Vec<Call>,
}

/// Esegue lo scenario contro `target`.
pub fn run(scenario: &Value, target: &Target) -> Recording {
    let mut calls = Vec::new();
    let mut normalizer = Normalizer::new(target.ollama, target.ws, target.home);
    let mut variables = HashMap::from([("ollama".to_owned(), target.ollama.to_owned())]);
    let mut exchanges = Vec::new();
    for step in scenario["steps"].as_array().expect("steps") {
        let path = substitute(&step["path"], &variables);
        let path = path.as_str().expect("path");
        let mut headers: Vec<(String, String)> = substitute(&step["headers"], &variables)
            .as_object()
            .map(|h| {
                h.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let body = if let Some(json) = step.get("json") {
            headers.push(("Content-Type".into(), "application/json".into()));
            Some(serde_json::to_vec(&substitute(json, &variables)).unwrap())
        } else if let Some(fields) = step.get("multipart").and_then(Value::as_object) {
            headers.push((
                "Content-Type".into(),
                format!("multipart/form-data; boundary={BOUNDARY}"),
            ));
            Some(multipart(fields, target.ws))
        } else {
            None
        };
        if let Some(if_match) = step.get("ifMatch").and_then(Value::as_str) {
            let value = if if_match == "current" || if_match == "W/current" {
                let from = step["revisionFrom"].as_str().unwrap_or("/governance");
                let (_, current) =
                    wire::request(target.base, "GET", from, &[], None).expect("revision request");
                let etag = current.header("etag").expect("ETag").to_owned();
                if if_match.starts_with("W/") {
                    format!("W/{etag}")
                } else {
                    etag
                }
            } else {
                if_match.to_owned()
            };
            headers.push(("If-Match".into(), value));
        }
        let method = step["method"].as_str().expect("method");
        let (status, response) =
            wire::request(target.base, method, path, &headers, body.as_deref())
                .unwrap_or_else(|e| panic!("{method} {path}: {e}"));
        let mut recorded = Map::new();
        recorded.insert("name".into(), step["name"].clone());
        recorded.insert("status".into(), json!(status));
        let kept: Map<String, Value> = HEADERS
            .iter()
            .filter_map(|h| response.header(h).map(|v| ((*h).to_owned(), json!(v))))
            .collect();
        recorded.insert("headers".into(), Value::Object(kept));
        recorded.extend(decode_body(step, &response.body, &mut normalizer));
        exchanges.push(Value::Object(recorded));
        calls.push(Call {
            method: method.to_owned(),
            path: path.split('?').next().unwrap_or(path).to_owned(),
            status,
            content_type: response.header("content-type").map(str::to_owned),
            body: response.body.clone(),
        });
        if let Some(captures) = step.get("capture").and_then(Value::as_object) {
            let document: Value = serde_json::from_slice(&response.body).expect("capture JSON");
            for (name, path) in captures {
                let value = pointer(&document, path.as_str().unwrap_or(""));
                variables.insert(name.clone(), value.as_str().unwrap_or("").to_owned());
            }
        }
    }
    let mut files = Map::new();
    for name in scenario["files"].as_array().expect("files") {
        let name = name.as_str().expect("file name");
        let Ok(text) = fs::read_to_string(target.ws.join(name)) else {
            continue;
        };
        let mut value = normalizer.value(&serde_json::from_str(&text).expect("file JSON"), None);
        if name.ends_with("qb-rag.json") && value.get("version").is_some() {
            value["version"] = json!("<build>");
        }
        files.insert(name.to_owned(), value);
    }
    Recording {
        document: json!({ "exchanges": exchanges, "files": files }),
        calls,
    }
}

/// Le differenze fra lo scenario registrato e quello atteso, una riga per scambio o file.
pub fn divergences(expected: &Value, actual: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let empty = Vec::new();
    let (want, got) = (
        expected["exchanges"].as_array().unwrap_or(&empty),
        actual["exchanges"].as_array().unwrap_or(&empty),
    );
    if want.len() != got.len() {
        out.push(format!(
            "{} scambi attesi, {} registrati",
            want.len(),
            got.len()
        ));
    }
    for (w, g) in want.iter().zip(got) {
        if w != g {
            out.push(format!(
                "scambio {}:\n  atteso   {w}\n  ottenuto {g}",
                w["name"]
            ));
        }
    }
    let no_files = Map::new();
    let (want, got) = (
        expected["files"].as_object().unwrap_or(&no_files),
        actual["files"].as_object().unwrap_or(&no_files),
    );
    for name in want
        .keys()
        .chain(got.keys().filter(|k| !want.contains_key(*k)))
    {
        if want.get(name) != got.get(name) {
            out.push(format!(
                "file {name}:\n  atteso   {}\n  ottenuto {}",
                want.get(name).unwrap_or(&Value::Null),
                got.get(name).unwrap_or(&Value::Null)
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_like_the_python_recorder() {
        let mut n = Normalizer::new("http://127.0.0.1:1/api", Path::new("/w"), Path::new("/h"));
        let id = "0f8fad5b-d9cb-469f-a165-70867728950e";
        let value = json!({
            "a": format!("/w\\docs {id}"),
            "b": [id, "2026-01-01T00:00:00.123Z"],
            "timestamp": 1_700_000_000_000_u64,
            "updatedAt": 0,
            "correlationId": "x",
            "last": "fine 0f8fad5b-d9cb-46",
            "lastMessage": format!("Pinned[1]:/w/x/{id}/ralph"),
        });
        assert_eq!(
            n.value(&value, None),
            json!({
                "a": "<ws>/docs <uuid-1>",
                "b": ["<uuid-1>", "<instant>"],
                "timestamp": "<millis>",
                "updatedAt": 0,
                "correlationId": "<correlation>",
                "last": "fine <uuid-partial>",
            "lastMessage": "Pinned[1]:<ws>…",
            })
        );
    }
}
