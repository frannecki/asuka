use crate::{
    domain::{MemorySearchHit, MessageRecord},
    runtime::ProviderSelection,
};

use super::types::MoonshotMessage;

pub(super) fn build_moonshot_messages(
    selection: &ProviderSelection,
    recent_messages: &[MessageRecord],
    memory_hits: &[MemorySearchHit],
    user_content: &str,
) -> Vec<MoonshotMessage> {
    let memory_context = if memory_hits.is_empty() {
        "No retrieved long-term memory hits.".to_string()
    } else {
        memory_hits
            .iter()
            .map(|hit| {
                format!(
                    "{} [{}]: {}",
                    hit.document_title, hit.namespace, hit.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut messages = vec![MoonshotMessage {
        role: "system".to_string(),
        content: "You are the core reasoning model for the Asuka agent runtime. Answer directly, stay grounded in the supplied memory context, and do not invent tool outputs.".to_string(),
    }];

    if !recent_messages.is_empty() {
        let conversation_window = recent_messages
            .iter()
            .map(|message| format!("{:?}: {}", message.role, message.content))
            .collect::<Vec<_>>()
            .join("\n");
        messages.push(MoonshotMessage {
            role: "system".to_string(),
            content: format!("Recent conversation window:\n{conversation_window}"),
        });
    }

    messages.push(MoonshotMessage {
        role: "system".to_string(),
        content: format!("Retrieved memory context:\n{memory_context}"),
    });
    messages.push(MoonshotMessage {
        role: "system".to_string(),
        content: format!(
            "Runtime selection metadata: provider={}, model={}. If the user asks which model/provider is active, answer from this metadata.",
            selection.provider_name, selection.model_name
        ),
    });
    messages.push(MoonshotMessage {
        role: "user".to_string(),
        content: user_content.to_string(),
    });

    messages
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::build_moonshot_messages;
    use crate::{
        domain::{MemorySearchHit, MessageRecord, MessageRole, ProviderType},
        runtime::ProviderSelection,
    };

    #[test]
    fn build_moonshot_messages_includes_context_metadata_and_user_message() {
        let selection = ProviderSelection {
            provider_id: Uuid::new_v4(),
            provider_name: "Moonshot".to_string(),
            provider_type: ProviderType::Moonshot,
            model_name: "kimi-k2.5".to_string(),
            base_url: "https://api.moonshot.ai/v1".to_string(),
            api_key_env: Some("MOONSHOT_API_KEY".to_string()),
        };
        let recent_messages = vec![MessageRecord {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "Summarize the architecture".to_string(),
            created_at: Utc::now(),
            run_id: None,
        }];
        let memory_hits = vec![MemorySearchHit {
            document_id: Uuid::new_v4(),
            chunk_id: Uuid::new_v4(),
            document_title: "Platform Overview".to_string(),
            namespace: "global".to_string(),
            content: "Rust backend and Next.js frontend".to_string(),
            score: 0.9,
        }];

        let messages = build_moonshot_messages(
            &selection,
            &recent_messages,
            &memory_hits,
            "What model are you using?",
        );

        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].role, "system");
        assert!(messages[1].content.contains("Recent conversation window"));
        assert!(messages[2].content.contains("Platform Overview [global]"));
        assert!(messages[3]
            .content
            .contains("provider=Moonshot, model=kimi-k2.5"));
        assert_eq!(messages[4].role, "user");
        assert_eq!(messages[4].content, "What model are you using?");
    }
}
