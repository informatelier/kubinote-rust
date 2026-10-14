//! `kubirag [--store <nome>] [-c "/comando args" | --script <file>]`

use std::io::{self, BufReader};
use std::process::ExitCode;
use std::sync::Arc;

use kubi_kernel::governance::OperationChannel;
use kubirag_cli::bootstrap::{read_config, Notebook, WorkspaceRoots, DEFAULT_STORE};
use kubirag_cli::commands;
use kubirag_cli::terminal::Terminal;
use kubirag_llm::OllamaGateway;

enum Mode {
    Repl,
    Once(String),
    Script(Option<String>),
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let store = args
        .iter()
        .position(|a| a == "--store")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| DEFAULT_STORE.to_owned());
    let mode = if let Some(i) = args.iter().position(|a| a == "-c") {
        Mode::Once(args[i + 1..].join(" "))
    } else if let Some(i) = args.iter().position(|a| a == "--script") {
        Mode::Script(args.get(i + 1).cloned())
    } else {
        Mode::Repl
    };
    let channel = if args.is_empty() {
        OperationChannel::Repl
    } else {
        OperationChannel::Cli
    };
    let mut out = io::stdout();
    let mut notebook = match open(&store, channel) {
        Ok(notebook) => notebook,
        Err(error) => {
            eprintln!("Error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut terminal = Terminal::new(commands::all(), "/lucene");
    let code = match mode {
        Mode::Repl => {
            let stdin = io::stdin();
            terminal.run(&mut BufReader::new(stdin.lock()), &mut notebook, &mut out);
            0
        }
        Mode::Once(line) => terminal.run_once(&line, &mut notebook, &mut out),
        Mode::Script(None) => {
            println!("Usage: --script <file>");
            1
        }
        Mode::Script(Some(path)) => match std::fs::read_to_string(&path) {
            Ok(script) => terminal.run_script(&script, &mut notebook, &mut out),
            Err(_) => {
                println!("Script not found: {path}");
                1
            }
        },
    };
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn open(store: &str, channel: OperationChannel) -> Result<Notebook, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let roots = WorkspaceRoots::resolve(&cwd, |key| std::env::var(key).ok())?;
    let config = read_config(&roots)?;
    let llm = Arc::new(OllamaGateway::from_config(&config.ollama_base_url));
    Notebook::open(roots, store, config, llm, channel)
}
