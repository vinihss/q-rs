use q_core::{ChatRequest, Message};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AskRequest {
    pub q: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    #[serde(default = "default_model")]
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub stream: bool,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub system_prompt: Option<String>,
}
impl From<ChatCompletionRequest> for ChatRequest {
    fn from(value: ChatCompletionRequest) -> Self {
        Self {
            model: value.model,
            messages: value.messages,
            temperature: value.temperature,
            max_tokens: value.max_tokens,
            stream: value.stream,
            system_prompt: value.system_prompt,
            tools: None,
        }
    }
}
fn default_model() -> String {
    "default".into()
}
