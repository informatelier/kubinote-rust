use super::ChatMessage;
use crate::config::RagConfig;
use crate::notebook::RagDocument;
use crate::retrieval::text::{take_utf16, utf16_len};

const HISTORY_MESSAGE_CHARS: usize = 500;

/// `ContextUsageEstimator.estimateTokens`: circa 4 caratteri (UTF-16) per token.
pub fn estimate_tokens(text: &str) -> i64 {
    (utf16_len(text) / 4) as i64
}

/// Occupazione stimata della finestra di contesto per la conversazione corrente: la parte
/// che si ripete a ogni turno (prompt di sistema, pinned, storico) e il massimo che i chunk
/// recuperati possono aggiungere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextUsage {
    pub system_tokens: i64,
    pub pinned_tokens: i64,
    pub history_tokens: i64,
    pub occupied_tokens: i64,
    pub context_window: i64,
    pub max_budget_tokens: i64,
    pub rag_allowance_tokens: i64,
    pub percent_of_window: i64,
    pub percent_of_budget: i64,
}

impl ContextUsage {
    /// `ContextUsageEstimator.estimate`: stesse forme del prompt reale (`PINNED <id>: …`,
    /// `[<ruolo>]: <primi 500 caratteri>`), storico limitato a `chatContextWindowSize`.
    pub fn estimate(config: &RagConfig, pinned: &[RagDocument], history: &[ChatMessage]) -> Self {
        let system = config.system_prompt.as_deref().unwrap_or("");
        let pinned_text = pinned
            .iter()
            .map(|d| {
                let meta = d.metadata.get("metaContent").map_or("", String::as_str);
                format!("PINNED {}: {meta} {}", d.id, d.content)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let window = usize::try_from(config.chat_context_window_size).unwrap_or(0);
        let history_text = history[history.len().saturating_sub(window)..]
            .iter()
            .map(|m| {
                format!(
                    "[{}]: {}",
                    m.role,
                    take_utf16(&m.content, HISTORY_MESSAGE_CHARS)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let system_tokens = estimate_tokens(system);
        let pinned_tokens = estimate_tokens(&pinned_text);
        let history_tokens = estimate_tokens(&history_text);
        let occupied = system_tokens + pinned_tokens + history_tokens;
        let window = i64::from(config.context_window);
        let budget = i64::from(config.chat_context_max_tokens);
        let percent = |total: i64| if total > 0 { occupied * 100 / total } else { 0 };
        Self {
            system_tokens,
            pinned_tokens,
            history_tokens,
            occupied_tokens: occupied,
            context_window: window,
            max_budget_tokens: budget,
            rag_allowance_tokens: i64::from(config.chunk_size) * i64::from(config.rag_max_docs),
            percent_of_window: percent(window),
            percent_of_budget: percent(budget),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimate_counts_system_pinned_and_windowed_history() {
        let config = RagConfig {
            system_prompt: Some("12345678".into()),
            chat_context_window_size: 1,
            ..RagConfig::default()
        };
        let pinned = [RagDocument::new("d", "abcd")];
        let history = [
            ChatMessage {
                content: "vecchio messaggio escluso".into(),
                timestamp: 1,
                role: "user".into(),
            },
            ChatMessage {
                content: "x".repeat(600),
                timestamp: 2,
                role: "assistant".into(),
            },
        ];
        let usage = ContextUsage::estimate(&config, &pinned, &history);
        assert_eq!(usage.system_tokens, 2);
        assert_eq!(usage.pinned_tokens, "PINNED d:  abcd".len() as i64 / 4);
        assert_eq!(usage.history_tokens, (13 + 500) / 4);
        assert_eq!(
            usage.rag_allowance_tokens,
            i64::from(config.chunk_size * config.rag_max_docs)
        );
        assert_eq!(
            usage.percent_of_window,
            usage.occupied_tokens * 100 / i64::from(config.context_window)
        );
    }
}
