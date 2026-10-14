//! La CLI sopra un workspace temporaneo, con un Ollama finto: /load, /list, /pin, /stat,
//! query in streaming, e il binario in modalità `-c`.

use std::process::Command as Process;
use std::sync::Arc;

use kubi_kernel::governance::OperationChannel;
use kubirag_app::retrieval::{GenerateRequest, LlmError, LlmGateway};
use kubirag_cli::bootstrap::{Notebook, WorkspaceRoots};
use kubirag_cli::commands;
use kubirag_cli::terminal::Terminal;
use kubirag_domain::config::RagConfig;

struct EchoLlm;

impl LlmGateway for EchoLlm {
    fn generate(
        &self,
        request: &GenerateRequest,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<String, LlmError> {
        let answer = if request.prompt.contains("Ollama locale") {
            "Si avvia con make cli."
        } else {
            "Non lo so."
        };
        for word in answer.split_inclusive(' ') {
            on_chunk(word);
        }
        Ok(answer.into())
    }

    fn embed(&self, _: &str, _: &str) -> Result<Vec<f32>, LlmError> {
        Err(LlmError::Unavailable("no embeddings in this test".into()))
    }
}

struct Session {
    _dir: tempfile::TempDir,
    notebook: Notebook,
    terminal: Terminal,
    source: String,
}

impl Session {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let roots = WorkspaceRoots {
            local: dir.path().join(".qb"),
            authority: dir.path().join("_qb"),
        };
        let source = dir.path().join("guida.md");
        std::fs::write(
            &source,
            "# Guida\n\nKubiRAG usa un Ollama locale. Avvia la CLI con make cli.\n",
        )
        .unwrap();
        let notebook = Notebook::open(
            roots,
            "prova",
            RagConfig::default(),
            Arc::new(EchoLlm),
            OperationChannel::Repl,
        )
        .unwrap();
        Self {
            _dir: dir,
            notebook,
            terminal: Terminal::new(commands::all(), "/lucene"),
            // `\` è il carattere di escape del tokenizer (come in JLine): su Windows si usa `/`.
            source: source.to_string_lossy().replace('\\', "/"),
        }
    }

    fn run(&mut self, line: &str) -> (bool, String) {
        let mut out = Vec::new();
        let ok = self
            .terminal
            .execute_line(line, &mut self.notebook, &mut out);
        (ok, String::from_utf8(out).unwrap())
    }

    fn document_id(&self) -> String {
        self.notebook
            .runtime()
            .unwrap()
            .documents()
            .next()
            .unwrap()
            .id
            .clone()
    }
}

#[test]
fn load_pin_stat_and_answer_on_a_fresh_notebook() {
    let mut session = Session::new();
    let source = session.source.clone();

    let (ok, out) = session.run(&format!("/load \"{source}\""));
    assert!(ok, "{out}");
    assert_eq!(
        out,
        "/load done. sources: found=1, added=1, updated=0, unchanged=0, ignored=0\n"
    );
    let (_, again) = session.run(&format!("/load \"{source}\""));
    assert!(again.contains("unchanged=1"), "{again}");

    let id = session.document_id();
    let (_, listed) = session.run("/list");
    assert!(listed.starts_with(&format!("{id} . ")), "{listed}");

    let (ok, pinned) = session.run(&format!("/pin {id}"));
    assert!(ok, "{pinned}");
    assert!(pinned.starts_with("/pin done at revision 2. "), "{pinned}");
    assert_eq!(
        session.run("/pinned").1,
        format!("0) {id}\n/pinned done.\n")
    );

    let (_, stale) = session.run(&format!("/unpin {id} --expected-revision=0"));
    assert!(stale.starts_with("/unpin conflicted: "), "{stale}");

    let (_, stats) = session.run("/stat ollama locale --mode=none");
    assert!(
        stats.contains(" ★ Pipeline Stats — \"ollama locale\""),
        "{stats}"
    );
    assert!(stats.contains("Candidates: 1"), "{stats}");
    assert!(stats.contains("(mode=none, "), "{stats}");
    assert!(stats.contains("outcome=SKIPPED"), "{stats}");
    assert!(
        stats.contains("maxDocs: 3  |  pinned: 1  |  selected: 0"),
        "{stats}"
    );

    let (ok, answer) = session.run("come si avvia con Ollama locale?");
    assert!(ok);
    assert!(answer.starts_with("Pinned[1]:"), "{answer}");
    assert!(answer.contains("🤖 RISPOSTA:\n"), "{answer}");
    assert!(answer.ends_with("Si avvia con make cli."), "{answer}");

    let (_, ranking) = session.run("/lucene --no-llm ollama");
    assert!(ranking.contains("│ query  : ollama\n"), "{ranking}");
    assert!(!ranking.contains("RISPOSTA"), "{ranking}");
}

#[test]
fn the_terminal_reports_help_unknown_commands_and_errors_like_kotlin() {
    let mut session = Session::new();
    assert_eq!(
        session.run("/pin ?").1,
        "Pins a document to always include it in RAG context.\n"
    );
    assert_eq!(session.run("/nope").1, "Unknown command: /nope\n");
    assert!(!session.run("/nope").0);
    assert_eq!(
        session.run("/pin x --expected-revision=abc").1,
        "Error: Invalid expected revision option\n"
    );
    assert_eq!(
        session.run("/load \"open").1,
        "Error: Missing closing quote\n"
    );
    let (ok, missing) = session.run("/pin");
    assert!(ok);
    assert_eq!(missing, "required parameter: DocumentID \n/pin done.\n");
    assert_eq!(
        session.run("/stat --mode=fast x").1,
        "Unknown mode 'fast'. Valid values: none, embedding, hybrid, llm\n\n"
    );
}

#[test]
fn the_binary_runs_one_command_and_exits_with_its_status() {
    let dir = tempfile::tempdir().unwrap();
    let run = |line: &str| {
        Process::new(env!("CARGO_BIN_EXE_kubirag"))
            .args(["--store", "smoke", "-c", line])
            .current_dir(dir.path())
            .env("QB_ROOT_PATH", dir.path().join(".qb"))
            .env("QB_AUTHORITY_PATH", dir.path().join("_qb"))
            .output()
            .unwrap()
    };
    let listed = run("/pinned");
    assert!(listed.status.success(), "{listed:?}");
    assert_eq!(String::from_utf8_lossy(&listed.stdout), "/pinned done.\n");
    assert!(dir
        .path()
        .join("_qb/notes/smoke/smoke-manifest.json")
        .is_file());
    let unknown = run("/nope");
    assert_eq!(unknown.status.code(), Some(1));
}
