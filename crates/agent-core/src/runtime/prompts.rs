use crate::domain::MemorySearchHit;

use super::ProviderSelection;

pub(crate) fn fallback_response(
    selection: Option<&ProviderSelection>,
    memory_hits: &[MemorySearchHit],
    user_content: &str,
    providers_count: usize,
) -> String {
    let retrieval_note = if memory_hits.is_empty() {
        "No long-term memory hits matched strongly enough for injection.".to_string()
    } else {
        let joined = memory_hits
            .iter()
            .map(|hit| format!("{} [{}]", hit.document_title, hit.namespace))
            .collect::<Vec<_>>()
            .join(", ");
        format!("Relevant memory hits: {joined}.")
    };

    let provider_note = selection
        .map(|selection| {
            format!(
                "Selected provider {} using model {}.",
                selection.provider_name, selection.model_name
            )
        })
        .unwrap_or_else(|| "No active provider/model pair was available.".to_string());

    format!(
        "This run was processed by the decoupled agent-core runtime. It is currently configured with {providers_count} provider account(s). {provider_note} {retrieval_note} The runtime will fall back to this local response path whenever upstream model invocation is unavailable.\n\nYou said: {user_content}"
    )
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::fallback_response;
    use crate::{
        domain::{MemorySearchHit, ProviderType},
        runtime::ProviderSelection,
    };

    #[test]
    fn fallback_response_mentions_provider_and_memory_hits() {
        let selection = ProviderSelection {
            provider_id: Uuid::new_v4(),
            provider_name: "OpenRouter".to_string(),
            provider_type: ProviderType::OpenRouter,
            model_name: "demo-model".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            api_key_env: Some("OPENROUTER_API_KEY".to_string()),
        };
        let memory_hits = vec![MemorySearchHit {
            document_id: Uuid::new_v4(),
            chunk_id: Uuid::new_v4(),
            document_title: "Architecture".to_string(),
            namespace: "global".to_string(),
            content: "Rust backend".to_string(),
            score: 0.8,
        }];

        let response = fallback_response(Some(&selection), &memory_hits, "Explain the stack", 2);

        assert!(response.contains("Selected provider OpenRouter using model demo-model."));
        assert!(response.contains("Relevant memory hits: Architecture [global]."));
        assert!(response.contains("You said: Explain the stack"));
    }

    #[test]
    fn fallback_response_handles_missing_provider_selection() {
        let response = fallback_response(None, &[], "Hello", 0);
        assert!(response.contains("No active provider/model pair was available."));
        assert!(response.contains("No long-term memory hits matched strongly enough"));
    }
}
