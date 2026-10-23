use serde::{Deserialize, Serialize};

/// Provider-independent events. HTTP/SSE adapters translate these to wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Started {
        id: String,
        model: String,
    },
    Delta {
        content: String,
    },
    Finished {
        finish_reason: Option<String>,
    },
    Usage {
        prompt_tokens: u64,
        completion_tokens: u64,
        total_tokens: u64,
    },
}
