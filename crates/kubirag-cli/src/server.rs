//! `kubirag server [store] [--port N] [--load path]… [--recursive] [--include md,txt]`:
//! gli stessi argomenti di `ServerOptions.parse` della versione Kotlin.

use indexmap::IndexSet;
use kubirag_app::retrieval::SourceImportOptions;
use kubirag_http::{AppState, ServerOptions};

use crate::bootstrap::{read_config, WorkspaceRoots};

/// Porta di default di `RagServer`.
pub const DEFAULT_PORT: u16 = 7070;

/// Le opzioni della riga di comando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerArgs {
    pub store: Option<String>,
    pub port: u16,
    pub load: Vec<String>,
    pub recursive: bool,
    pub include: Option<IndexSet<String>>,
}

const DEFAULT_EXTENSIONS: [&str; 3] = ["md", "txt", "pdf"];

impl ServerArgs {
    /// `ServerOptions.parse`, con gli stessi messaggi d'errore.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let start = args
            .iter()
            .position(|a| a == "server")
            .ok_or("Missing server command")?;
        let mut parsed = Self {
            store: None,
            port: DEFAULT_PORT,
            load: Vec::new(),
            recursive: false,
            include: None,
        };
        let mut index = start + 1;
        let value = |index: usize, option: &str| {
            args.get(index + 1)
                .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
                .cloned()
                .ok_or_else(|| format!("Option {option} requires a value"))
        };
        while index < args.len() {
            let argument = args[index].as_str();
            match argument {
                "--port" => {
                    let raw = value(index, argument)?;
                    parsed.port = raw
                        .parse::<u16>()
                        .ok()
                        .filter(|p| *p >= 1)
                        .ok_or_else(|| format!("Invalid server port [{raw}]"))?;
                    index += 2;
                }
                "--load" => {
                    parsed.load.push(value(index, argument)?);
                    index += 2;
                }
                "--recursive" => {
                    parsed.recursive = true;
                    index += 1;
                }
                "--include" => {
                    let raw = value(index, argument)?;
                    let extensions: IndexSet<String> = raw
                        .split(',')
                        .map(|e| e.trim().trim_start_matches('.').to_lowercase())
                        .filter(|e| !e.is_empty())
                        .collect();
                    if extensions.is_empty() {
                        return Err("Option --include requires at least one extension".into());
                    }
                    if !extensions
                        .iter()
                        .all(|e| e.chars().all(char::is_alphanumeric))
                    {
                        return Err(format!("Invalid extension list [{raw}]"));
                    }
                    parsed
                        .include
                        .get_or_insert_with(IndexSet::new)
                        .extend(extensions);
                    index += 2;
                }
                other if other.starts_with("--") => {
                    return Err(format!("Unknown server option [{other}]"));
                }
                other => {
                    if let Some(store) = &parsed.store {
                        return Err(format!(
                            "Multiple server store names: [{store}] and [{other}]"
                        ));
                    }
                    parsed.store = Some(other.to_owned());
                    index += 1;
                }
            }
        }
        Ok(parsed)
    }

    fn import_options(&self) -> Option<SourceImportOptions> {
        (!self.load.is_empty()).then(|| SourceImportOptions {
            paths: self.load.clone(),
            recursive: self.recursive,
            included_extensions: self
                .include
                .clone()
                .unwrap_or_else(|| DEFAULT_EXTENSIONS.iter().map(|e| (*e).to_owned()).collect()),
        })
    }
}

/// Avvia il server sulla porta scelta, finché il processo non riceve Ctrl-C.
pub fn run(args: &ServerArgs) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let roots = WorkspaceRoots::resolve(&cwd, |key| std::env::var(key).ok())?;
    let config = read_config(&roots)?;
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| cwd.clone(), Into::into);
    let state = AppState::open(ServerOptions {
        roots,
        cwd,
        home,
        store: args.store.clone(),
        config,
    })?;
    if let Some(options) = args.import_options() {
        println!("SOURCE IMPORT {}", state.import_sources(&options)?);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", args.port))
            .await
            .map_err(|e| format!("Cannot listen on port {}: {e}", args.port))?;
        println!("✅ KubiRAG Web UI → http://localhost:{}", args.port);
        kubirag_http::serve(listener, state, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn options_follow_the_kotlin_parser() {
        let parsed = ServerArgs::parse(&args(
            "server kubirag --port 8080 --load docs --recursive --include .MD,txt",
        ))
        .unwrap();
        assert_eq!(parsed.store.as_deref(), Some("kubirag"));
        assert_eq!(parsed.port, 8080);
        assert_eq!(parsed.load, ["docs"]);
        assert!(parsed.recursive);
        assert_eq!(
            parsed.include.unwrap().into_iter().collect::<Vec<_>>(),
            ["md", "txt"]
        );
        assert_eq!(
            ServerArgs::parse(&args("server")).unwrap().port,
            DEFAULT_PORT
        );
    }

    #[test]
    fn invalid_options_are_reported_like_kotlin() {
        assert_eq!(
            ServerArgs::parse(&args("server --port 0")).unwrap_err(),
            "Invalid server port [0]"
        );
        assert_eq!(
            ServerArgs::parse(&args("server a b")).unwrap_err(),
            "Multiple server store names: [a] and [b]"
        );
        assert_eq!(
            ServerArgs::parse(&args("server --verbose")).unwrap_err(),
            "Unknown server option [--verbose]"
        );
        assert_eq!(
            ServerArgs::parse(&args("server --load")).unwrap_err(),
            "Option --load requires a value"
        );
    }
}
