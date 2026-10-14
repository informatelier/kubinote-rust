//! Retrieval: la query di default con risposta in streaming (`/lucene`), le statistiche dei
//! tre livelli della pipeline (`/stat`) e il backfill degli embedding (`/embed`).

use std::io::Write;

use kubirag_app::retrieval::Ranking;
use kubirag_domain::config::RagConfig;
use kubirag_domain::retrieval::text::{kotlin_lines, utf16_len};
use kubirag_domain::retrieval::{RerankMode, SearchPipelineStats};

use super::{emit, exception, Command, CommandResult};
use crate::bootstrap::Notebook;
use crate::format::{java_fixed, pad_end, pad_start, short_path, take, take_last};

pub struct Stat;

impl Command for Stat {
    fn name(&self) -> &'static str {
        "/stat"
    }

    fn help_line(&self) -> String {
        "Shows effectiveness statistics for each RAG pipeline level against a query.".into()
    }

    fn help_usage(&self) -> String {
        format!(
            "{} <query> [--explain] [--mode=none|embedding|hybrid|llm]  —  runs the search pipeline (no LLM generation) and reports BM25 scores, second-stage ranking outcome/deltas, and context assembly stats.",
            self.name()
        )
    }

    fn help_example(&self) -> String {
        "/stat what is the architecture of kubirag\n/stat kubernetes deployment --explain\n/stat kubernetes deployment --mode=hybrid".into()
    }

    fn execute(
        &self,
        tokens: &[String],
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> CommandResult {
        let show_explain = tokens.iter().any(|t| t == "--explain");
        let mode_value = tokens.iter().find_map(|t| t.strip_prefix("--mode="));
        let requested = match mode_value {
            None => None,
            Some(value) => {
                match RerankMode::parse(value) {
                    Some(mode) => Some(mode),
                    None => {
                        emit(
                        out,
                        &format!("Unknown mode '{value}'. Valid values: none, embedding, hybrid, llm\n"),
                    )?;
                        return Ok(String::new());
                    }
                }
            }
        };
        let query = tokens
            .iter()
            .skip(1)
            .filter(|t| !t.starts_with("--"))
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_owned();
        if query.chars().all(char::is_whitespace) {
            emit(
                out,
                &format!(
                    "Usage: {} <query> [--explain] [--mode=none|embedding|hybrid|llm]\n",
                    self.name()
                ),
            )?;
            return Ok(String::new());
        }
        let config = notebook.config.clone();
        let stats = notebook
            .runtime()
            .map_err(exception)?
            .collect_stats(&config, &query, requested);
        emit(out, &render_stats(&stats, show_explain))?;
        Ok(String::new())
    }
}

/// Il testo di `CmdStat`, riga per riga.
pub fn render_stats(stats: &SearchPipelineStats, show_explain: bool) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut line = |text: String| lines.push(text);
    line(String::new());
    line(format!(" ★ Pipeline Stats — \"{}\"", stats.query));
    line(String::new());

    let b = &stats.bm25;
    let cache_total = b.cache_stats.hits + b.cache_stats.misses;
    let cache_info = if cache_total > 0 {
        format!(
            "cache: {} hits / {} misses ({}%)",
            b.cache_stats.hits,
            b.cache_stats.misses,
            java_fixed(f64::from(b.cache_stats.hit_rate() * 100.0), 1)
        )
    } else {
        "cache: cold".to_owned()
    };
    line(format!(
        " [1/3] BM25 / Lucene  ({}ms)  {cache_info}",
        b.elapsed_ms
    ));
    if stats.expanded_query != stats.query {
        line(format!("   Query expansion: \"{}\"", stats.query));
        line(format!(
            "   {}→ \"{}\"",
            " ".repeat(20),
            stats.expanded_query
        ));
    }
    line(format!("   Candidates: {}", b.candidates_found));
    if b.results.is_empty() {
        line("   (no results)".into());
    } else {
        line("   ┌──────┬───────────┬──────────────────────────────────────────────────┐".into());
        line("   │ rank │   score   │ path                                             │".into());
        line("   ├──────┼───────────┼──────────────────────────────────────────────────┤".into());
        for doc in &b.results {
            line(format!(
                "   │  #{}  │ {} │ {} │",
                pad_start(&doc.rank.to_string(), 2),
                pad_end(&java_fixed(f64::from(doc.score), 4), 9),
                short_path(&doc.path, 48)
            ));
            if show_explain && !doc.explanation.chars().all(char::is_whitespace) {
                for explanation in kotlin_lines(&doc.explanation).into_iter().take(5) {
                    line(format!(
                        "   │      │           │   {} │",
                        pad_end(take(explanation, 46), 46)
                    ));
                }
            }
        }
        line("   └──────┴───────────┴──────────────────────────────────────────────────┘".into());
    }
    line(String::new());

    let r = &stats.rerank;
    let lazy_info = if r.embeddings_created > 0 {
        format!(" (+{} lazy)", r.embeddings_created)
    } else {
        String::new()
    };
    let embedding_info = if r.embeddings_total > 0 {
        format!(
            ", embeddings {}/{}{lazy_info}",
            r.embeddings_used, r.embeddings_total
        )
    } else {
        String::new()
    };
    line(format!(
        " [2/3] Second stage  (mode={}, {}ms, outcome={}{embedding_info})",
        r.mode, r.elapsed_ms, r.outcome
    ));
    if r.results.is_empty() {
        line("   (no results)".into());
    } else {
        line(
            "   ┌──────┬──────────┬─────────┬─────────┬──────────────────────────────────┐".into(),
        );
        line(
            "   │ rank │ bm25-was │ vector  │  fused  │ path                             │".into(),
        );
        line(
            "   ├──────┼──────────┼─────────┼─────────┼──────────────────────────────────┤".into(),
        );
        for doc in &r.results {
            let was = if doc.previous_rank == 0 {
                "new".to_owned()
            } else {
                format!("#{}", doc.previous_rank)
            };
            let delta = if doc.previous_rank == 0 {
                "(+)".to_owned()
            } else {
                let delta = doc.previous_rank as i64 - doc.new_rank as i64;
                match delta {
                    d if d > 0 => format!("(↑ +{d})"),
                    d if d < 0 => format!("(↓ {d})"),
                    _ => "(=)".to_owned(),
                }
            };
            let score = |value: Option<f32>| {
                value.map_or_else(|| "  -  ".to_owned(), |v| java_fixed(f64::from(v), 3))
            };
            let base = if utf16_len(&doc.path) > 24 {
                format!("…{}", take_last(&doc.path, 23))
            } else {
                doc.path.clone()
            };
            let labelled = format!("{base} {delta}");
            let path = if utf16_len(&labelled) > 32 {
                take(&labelled, 32).to_owned()
            } else {
                pad_end(&labelled, 32)
            };
            line(format!(
                "   │  #{}  │   {}    │  {}  │  {}  │ {path} │",
                pad_start(&doc.new_rank.to_string(), 2),
                pad_start(&was, 3),
                score(doc.vector_score),
                score(doc.fused_score)
            ));
        }
        line(
            "   └──────┴──────────┴─────────┴─────────┴──────────────────────────────────┘".into(),
        );
    }
    line(String::new());

    let c = &stats.context;
    line(" [3/3] Context".into());
    line(format!(
        "   maxDocs: {}  |  pinned: {}  |  selected: {}",
        c.max_docs, c.pinned_doc_count, c.selected_doc_count
    ));
    line(format!(
        "   Estimated context: {}k chars",
        java_fixed(c.estimated_context_chars as f64 / 1000.0, 1)
    ));
    line(String::new());
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// `searchRanking`: la tabella di `/lucene --rank` e `--no-llm`.
pub fn render_ranking(config: &RagConfig, question: &str, ranking: &Ranking) -> String {
    let mut text = String::from("\n");
    text.push_str(
        "┌─── LUCENE RANKING ─────────────────────────────────────────────────────────────┐\n",
    );
    text.push_str(&format!("│ query  : {question}\n"));
    text.push_str(&format!(
        "│ mode   : {}  outcome: {}  maxDocs: {}  candidates: {}\n",
        ranking.mode.label(),
        ranking.outcome.name(),
        config.rag_max_docs,
        ranking.candidates
    ));
    text.push_str(
        "├──────┬──────────┬──────────┬──────────────────────────────────────────────────┤\n",
    );
    text.push_str(
        "│ rank │ bm25-was │  score   │ path                                             │\n",
    );
    text.push_str(
        "├──────┼──────────┼──────────┼──────────────────────────────────────────────────┤\n",
    );
    for row in &ranking.rows {
        text.push_str(&format!(
            "│  #{}  │    #{}    │ {} │ {} │\n",
            pad_start(&row.rank.to_string(), 2),
            pad_start(&row.bm25_rank.to_string(), 2),
            pad_end(&java_fixed(f64::from(row.score), 4), 8),
            short_path(&row.path, 48)
        ));
    }
    text.push_str(
        "└──────┴──────────┴──────────┴──────────────────────────────────────────────────┘\n",
    );
    text.push('\n');
    text
}

pub struct Embed;

impl Command for Embed {
    fn name(&self) -> &'static str {
        "/embed"
    }

    fn help_line(&self) -> String {
        "Creates missing document embeddings for the current store (backfill for the embedding/hybrid ranking modes).".into()
    }

    fn help_usage(&self) -> String {
        format!(
            "{} [force] - Embeds documents without an embedding; 'force' re-embeds everything (use after changing embeddingModelName).",
            self.name()
        )
    }

    fn help_example(&self) -> String {
        "/embed\n/embed force".into()
    }

    fn execute(
        &self,
        tokens: &[String],
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> CommandResult {
        let force = tokens
            .get(1)
            .is_some_and(|t| t.eq_ignore_ascii_case("force"));
        let config = notebook.config.clone();
        if force {
            emit(
                out,
                &format!(
                    "Re-embedding all documents with model '{}'...\n",
                    config.embedding_model_name
                ),
            )?;
        }
        let (created, total) = notebook
            .runtime()
            .map_err(exception)?
            .backfill_embeddings(&config, force)
            .map_err(exception)?;
        // Il runtime Rust salva subito i vettori: non serve il `/store` della versione Kotlin.
        emit(
            out,
            &format!(
                "Embeddings: {created} created, {} already present, {total} documents total.\n",
                total - created
            ),
        )?;
        Ok(format!("{} Done.", self.name()))
    }
}

pub struct Lucene;

impl Command for Lucene {
    fn name(&self) -> &'static str {
        "/lucene"
    }

    fn help_line(&self) -> String {
        "Queries using Lucene search with streaming AI response (default command).".into()
    }

    fn help_usage(&self) -> String {
        format!(
            "{} [--rank] [--no-llm] <query>\n  --rank    print BM25 ranking table before the LLM response\n  --no-llm  print ranking table only, skip LLM call",
            self.name()
        )
    }

    fn help_example(&self) -> String {
        "/lucene --rank How does the authentication work?".into()
    }

    fn execute(
        &self,
        tokens: &[String],
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> CommandResult {
        let show_ranking = tokens.iter().any(|t| t == "--rank");
        let no_llm = tokens.iter().any(|t| t == "--no-llm");
        // Il nome del comando non fa parte della domanda (in Kotlin sì, vedi README).
        let words: Vec<&str> = tokens
            .iter()
            .enumerate()
            .filter(|(i, t)| !(*i == 0 && t.as_str() == self.name()))
            .map(|(_, t)| t.as_str())
            .filter(|t| *t != "--rank" && *t != "--no-llm")
            .collect();
        let line = words.join(" ");
        if line.chars().all(char::is_whitespace) {
            return Ok(String::new());
        }
        if let Some(mention) = line.strip_prefix('@') {
            return answer_with_role(mention, out);
        }
        let config = notebook.config.clone();
        let mut runtime = notebook.runtime().map_err(exception)?;
        if show_ranking || no_llm {
            let ranking = runtime.ranking(&config, &line, None);
            emit(out, &render_ranking(&config, &line, &ranking))?;
        }
        if no_llm {
            return Ok(String::new());
        }
        let mut write_error = None;
        let mut sink = |chunk: &str| {
            if write_error.is_none() {
                if let Err(error) = emit(out, chunk) {
                    write_error = Some(error);
                }
            }
        };
        runtime
            .answer(&config, &line, &[], None, &mut sink)
            .map_err(exception)?;
        if let Some(error) = write_error {
            return Err(error);
        }
        // La risposta è già uscita in streaming: il risultato vuoto evita di ristamparla.
        Ok(String::new())
    }
}

/// `@ruolo domanda` e `@party domanda`: i ruoli non sono ancora letti dalla CLI Rust, quindi
/// il comportamento è quello di Kotlin con nessun ruolo definito.
fn answer_with_role(mention_and_query: &str, out: &mut dyn Write) -> CommandResult {
    let (mention, query) = match mention_and_query.split_once(' ') {
        Some((mention, query)) => (mention, query.trim()),
        None => (mention_and_query, ""),
    };
    if query.chars().all(char::is_whitespace) {
        emit(out, "Usage: @<role> <query>  or  @party <query>\n")?;
    } else if mention.eq_ignore_ascii_case("party") {
        emit(out, "Nessun ruolo definito. Usa /role-add per crearne.\n")?;
    } else {
        emit(
            out,
            &format!(
                "Ruolo '@{mention}' non trovato. Usa /role-list per vedere i ruoli disponibili.\n"
            ),
        )?;
    }
    Ok(String::new())
}
