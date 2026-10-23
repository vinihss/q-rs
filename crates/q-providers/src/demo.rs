use async_trait::async_trait;
use futures_util::stream;
use q_core::{ChatProvider, ChatRequest, ChatResponse, ProviderStream, Result, StreamEvent};

/// Deterministic local provider for development and tests; it does not call an LLM.
pub struct DemoProvider;
#[async_trait]
impl ChatProvider for DemoProvider {
    fn name(&self) -> &'static str {
        "demo"
    }
    async fn complete(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let content = request
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, q_core::Role::User))
            .map(|m| format!("[demo] Você perguntou: {}", m.content))
            .unwrap_or_else(|| "[demo] Nenhuma mensagem de usuário recebida.".into());
        Ok(ChatResponse {
            id: "chatcmpl-demo".into(),
            model: "demo".into(),
            content,
            finish_reason: Some("stop".into()),
            usage: None,
            tool_calls: vec![],
        })
    }
    async fn stream(&self, request: &ChatRequest) -> Result<ProviderStream> {
        let response = self.complete(request).await?;
        let events: Vec<q_core::Result<StreamEvent>> = vec![
            Ok(StreamEvent::Started {
                id: response.id.clone(),
                model: response.model.clone(),
            }),
            Ok(StreamEvent::Delta {
                content: response.content,
            }),
            Ok(StreamEvent::Finished {
                finish_reason: response.finish_reason,
            }),
        ];
        Ok(Box::pin(stream::iter(events)))
    }
}
