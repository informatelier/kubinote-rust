//! O3-KR1: il server Rust risponde allo scenario della SPA come il server Javalin, nel
//! rispetto dell'OpenAPI, e ogni route chiamata dalla SPA è servita, rimandata o statica.
//!
//! Lo scenario di `contracts/golden/http/scenario.json` (124 scambi: governance, documenti,
//! notebook, sessioni, ruoli, configurazione, asset, route rimandate) gira contro il router
//! axum in-process e un Ollama finto; risposte e file `.qb/` normalizzati devono coincidere con
//! `kotlin-exchanges.json`, registrato contro kubirag 09aba65 con lo stesso Ollama finto.
//!
//! Lo scenario gira una volta sola per i tre test. Il binario è a sé perché imposta
//! `OLLAMA_BASE_URL` per tutto il processo, come lo imposta il registratore al server Kotlin.

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use kubirag_acceptance::http::Recording;
use kubirag_acceptance::openapi::Spec;
use kubirag_acceptance::{fake_ollama, golden, http};
use kubirag_http::assets::STATIC_ASSETS;
use kubirag_http::{AppState, ServerOptions, DEFERRED_ROUTES};
use kubirag_workspace::{read_config, WorkspaceRoots};
use serde_json::json;
use serde_json::Value;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Il path reale (su macOS `/var` è un link a `/private/var`), senza il prefisso `\\?\` che
/// `canonicalize` aggiunge su Windows.
fn real(path: &Path) -> PathBuf {
    let path = fs::canonicalize(path).unwrap();
    match path.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => path,
    }
}

fn recording() -> &'static Recording {
    static RECORDING: OnceLock<Recording> = OnceLock::new();
    RECORDING.get_or_init(record)
}

fn record() -> Recording {
    let scenario = golden::json("http/scenario.json");
    let tmp = tempfile::tempdir().unwrap();
    let root = real(tmp.path());
    let (ws, home) = (root.join("ws"), root.join("home"));
    copy_dir(&golden::dir().join("http/fixture"), &ws);
    fs::create_dir_all(&home).unwrap();

    let ollama = fake_ollama::start();
    fs::create_dir_all(ws.join(".qb")).unwrap();
    fs::write(
        ws.join(".qb/qb-rag.json"),
        json!({"ollamaBaseUrl": ollama.url, "modelName": "qwen3:4b"}).to_string(),
    )
    .unwrap();
    std::env::set_var("OLLAMA_BASE_URL", &ollama.url);

    let roots = WorkspaceRoots::resolve(&ws, |_| None).unwrap();
    let config = read_config(&roots).unwrap();
    let state = AppState::open(ServerOptions {
        roots,
        cwd: ws.clone(),
        home: home.clone(),
        store: scenario["store"].as_str().map(str::to_owned),
        config,
    })
    .unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let address: SocketAddr = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = runtime.spawn(kubirag_http::serve(listener, state, async {
        let _ = stopped.await;
    }));

    let base = format!("http://{address}");
    let recorded = http::run(
        &scenario,
        &http::Target {
            base: &base,
            ollama: &ollama.url,
            ws: &ws,
            home: &home,
        },
    );
    let _ = stop.send(());
    runtime.block_on(server).unwrap().unwrap();
    recorded
}

#[test]
fn spa_scenario_matches_the_kotlin_server() {
    let expected = golden::json("http/kotlin-exchanges.json");
    golden::assert_no_divergence(
        "scenario HTTP della SPA",
        http::divergences(&expected, &recording().document),
    );
}

fn is_deferred(method: &str, path: &str) -> bool {
    DEFERRED_ROUTES
        .iter()
        .any(|r| r.method == method && same_route(r.path, path))
}

/// `pattern` usa `{nome}` (axum) o `{}` (inventario della SPA) per i segmenti variabili;
/// `{*coda}` prende il resto del path.
fn same_route(pattern: &str, path: &str) -> bool {
    let (mut p, mut a) = (pattern.split('/'), path.split('/'));
    loop {
        match (p.next(), a.next()) {
            (None, None) => return true,
            (Some(s), Some(_)) if s.starts_with("{*") => return true,
            (Some(s), Some(x))
                if (s.starts_with('{') && s.ends_with('}')) || x == "{}" || s == x => {}
            _ => return false,
        }
    }
}

/// Dove anche il server Kotlin esce dal proprio OpenAPI: lacune dello spec, non del porting.
/// Il golden Kotlin ha le stesse risposte; una violazione nuova fa fallire il test.
const KOTLIN_SPEC_GAPS: &[&str] = &[
    // Scenario negativo: una route che non esiste risponde il 404 di `RagServer`.
    "GET /no/such/route: operazione assente dall'OpenAPI",
    // `ModelInfo.maxContext` è `Int?`: null per i modelli senza `context_length` (embedding).
    "GET /models 200: $[0].maxContext: atteso integer, trovato null",
    // `isAllowedClient`: il 403 delle operazioni sul filesystem non è documentato.
    "DELETE /stores/secondo: stato 403 non documentato",
];

#[test]
fn responses_respect_the_openapi_contract() {
    let spec = golden::json("http/openapi.json");
    let spec = Spec::new(&spec);
    let mut violations = Vec::new();
    let mut checked = 0;
    for call in &recording().calls {
        let json = call
            .content_type
            .as_deref()
            .is_some_and(|t| t.starts_with("application/json"));
        let is_static = STATIC_ASSETS.iter().any(|a| a.route == call.path);
        if call.status == 501 || is_static || !json {
            continue;
        }
        // Il corpo dello stream di chat è SSE dentro `application/json`, come in Javalin.
        let body = serde_json::from_slice::<Value>(&call.body).ok();
        checked += 1;
        violations.extend(spec.check(&call.method, &call.path, call.status, body.as_ref()));
    }
    assert!(
        checked >= 100,
        "solo {checked} risposte confrontate con l'OpenAPI"
    );
    violations.retain(|v| !KOTLIN_SPEC_GAPS.contains(&v.as_str()));
    golden::assert_no_divergence("OpenAPI di kubirag", violations);
}

#[test]
fn every_spa_route_is_served_deferred_or_static() {
    let inventory = golden::json("http/spa-routes.json");
    let calls = &recording().calls;
    let (mut served, mut deferred, mut statics, mut missing) = (0, 0, 0, Vec::new());
    for route in inventory["inventory"].as_array().unwrap() {
        let path = route["route"].as_str().unwrap();
        for method in route["methods"].as_array().unwrap() {
            let method = method.as_str().unwrap();
            // `DYNAMIC`: il verbo si decide a runtime, basta che la route sia esercitata.
            let exercised = calls.iter().any(|c| {
                (method == "DYNAMIC" || c.method == method)
                    && c.status != 501
                    && same_route(path, &c.path)
            });
            if STATIC_ASSETS.iter().any(|a| same_route(path, a.route)) && method == "GET" {
                statics += 1;
            } else if is_deferred(method, path) {
                deferred += 1;
            } else if exercised {
                served += 1;
            } else {
                missing.push(format!("{method} {path}"));
            }
        }
    }
    println!("O3-KR1: {served} route servite e verificate, {deferred} rimandate (501), {statics} statiche");
    golden::assert_no_divergence("route della SPA", missing);
}
