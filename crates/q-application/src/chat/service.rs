use std::sync::Arc;

use q_core::{
    ChatProvider, ChatRequest, ChatResponse, ProviderRegistry, ProviderStream, QError, Result,
    Role, ToolExecutor,
};

use crate::StaticProviderRegistry;

const DEFAULT_MAX_TOOL_ROUNDS: usize = 8;

pub struct ChatService {
    pub(crate) registry: Arc<dyn ProviderRegistry>,
    pub(crate) default_provider: Option<String>,
    pub(crate) default_model: String,
    pub(crate) tools: Option<Arc<dyn ToolExecutor>>,
    pub(crate) max_tool_rounds: usize,
}

impl ChatService {
    pub fn new(providers: Vec<Arc<dyn ChatProvider>>) -> Self {
        Self {
            registry: Arc::new(StaticProviderRegistry::new(providers)),
            default_provider: None,
            default_model: "default".into(),
            tools: None,
            max_tool_rounds: DEFAULT_MAX_TOOL_ROUNDS,
        }
    }

    pub fn with_registry(registry: Arc<dyn ProviderRegistry>) -> Self {
        Self {
            registry,
            default_provider: None,
            default_model: "default".into(),
            tools: None,
            max_tool_rounds: DEFAULT_MAX_TOOL_ROUNDS,
        }
    }

    pub fn with_default_provider(mut self, name: impl Into<String>) -> Self {
        self.default_provider = Some(name.into());
        self
    }

    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    pub fn tools(&self) -> Option<Arc<dyn ToolExecutor>> {
        self.tools.clone()
    }

    /// Enables automatic tool execution. Only tools registered in this executor are exposed
    /// to the model; tool definitions supplied by an untrusted client are never trusted.
    pub fn with_tools(mut self, tools: Arc<dyn ToolExecutor>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Caps model/tool round trips to prevent runaway loops and excessive tool costs.
    pub fn with_max_tool_rounds(mut self, max_rounds: usize) -> Self {
        self.max_tool_rounds = max_rounds.max(1);
        self
    }

    pub async fn complete(&self, request: &ChatRequest) -> Result<ChatResponse> {
        request.validate()?;
        let providers = self.candidates(&request.model);
        if providers.is_empty() {
            return Err(QError::NoProvider(request.model.clone()));
        }

        // A model may request multiple tool calls in one assistant turn. Execute them and
        // append each result as a tool message, then ask the model to continue.
        let mut working = request.clone();
        if let Some(prompt) = &working.system_prompt {
            working.messages.insert(
                0,
                q_core::Message {
                    role: Role::System,
                    content: prompt.clone(),
                    name: None,
                    tool_call_id: None,
                    tool_calls: None,
                },
            );
        }
        working.system_prompt = None;
        working.tools = self.tools.as_ref().map(|executor| executor.definitions());
        let mut rounds = 0usize;

        loop {
            let response = self.complete_with_fallback(&providers, &working).await?;
            if response.tool_calls.is_empty() {
                return Ok(response);
            }

            let Some(executor) = &self.tools else {
                return Err(QError::UnsupportedOperation(
                    "o modelo solicitou ferramentas, mas nenhum ToolExecutor foi configurado"
                        .into(),
                ));
            };
            if rounds >= self.max_tool_rounds {
                return Err(QError::MaxToolRounds(self.max_tool_rounds));
            }
            const MAX_TOOL_CALLS_PER_ROUND: usize = 16;
            if response.tool_calls.len() > MAX_TOOL_CALLS_PER_ROUND {
                return Err(QError::InvalidRequest(format!(
                    "o modelo solicitou {} ferramentas numa rodada; o limite é {}",
                    response.tool_calls.len(),
                    MAX_TOOL_CALLS_PER_ROUND
                )));
            }
            rounds += 1;

            self.append_tool_results(
                &mut working,
                response.content.clone(),
                response.tool_calls.clone(),
                executor,
            )
            .await?;
        }
    }

    // complete_with_fallback moved to chat::fallback

    /// Streaming currently forwards provider events. Automatic tool execution is implemented
    /// for non-streaming completions; providers need structured tool-call stream events before
    /// this path can safely execute tools during a streamed response.
    pub async fn stream(&self, request: &ChatRequest) -> Result<ProviderStream> {
        request.validate()?;
        let providers = self.candidates(&request.model);
        if providers.is_empty() {
            return Err(QError::NoProvider(request.model.clone()));
        }
        let mut working = request.clone();
        if let Some(prompt) = &working.system_prompt {
            working.messages.insert(
                0,
                q_core::Message {
                    role: Role::System,
                    content: prompt.clone(),
                    name: None,
                    tool_call_id: None,
                    tool_calls: None,
                },
            );
        }
        working.system_prompt = None;
        let mut last_error = None;
        for provider in providers {
            match provider.stream(&working).await {
                Ok(stream) => return Ok(stream),
                Err(error) => {
                    eprintln!(
                        "q-rs: provider {} streaming failed: {}",
                        provider.name(),
                        error
                    );
                    last_error = Some(error);
                }
            }
        }
        match last_error {
            Some(error) => Err(error),
            None => Err(QError::NoProvider(request.model.clone())),
        }
    }

    // candidates moved to chat::fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures_util::stream;
    use q_core::{Message, StreamEvent, ToolCall, ToolDefinition, ToolOutput};
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeProvider {
        calls: AtomicUsize,
    }
    #[async_trait]
    impl ChatProvider for FakeProvider {
        fn name(&self) -> &'static str {
            "fake"
        }
        async fn complete(&self, request: &ChatRequest) -> Result<ChatResponse> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                assert_eq!(request.tools.as_ref().map(Vec::len), Some(1));
                Ok(ChatResponse {
                    id: "test-id".into(),
                    model: "fake-model".into(),
                    content: String::new(),
                    finish_reason: Some("tool_calls".into()),
                    usage: None,
                    tool_calls: vec![ToolCall {
                        id: "call-1".into(),
                        name: "sum".into(),
                        arguments: json!({"a":2,"b":3}),
                    }],
                })
            } else {
                assert!(request
                    .messages
                    .iter()
                    .any(|m| m.role == Role::Tool && m.content == "5"));
                Ok(ChatResponse {
                    id: "test-id-2".into(),
                    model: "fake-model".into(),
                    content: "resultado: 5".into(),
                    finish_reason: Some("stop".into()),
                    usage: None,
                    tool_calls: vec![],
                })
            }
        }
        async fn stream(&self, _request: &ChatRequest) -> Result<ProviderStream> {
            Ok(Box::pin(stream::iter(vec![Ok(StreamEvent::Delta {
                content: "ok".into(),
            })])))
        }
    }

    struct SumTool;
    #[async_trait]
    impl q_core::Tool for SumTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "sum".into(),
                description: "Soma dois números".into(),
                parameters_schema: json!({"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"}},"required":["a","b"]}),
            }
        }
        async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput> {
            let a = args["a"]
                .as_f64()
                .ok_or_else(|| QError::InvalidRequest("a inválido".into()))?;
            let b = args["b"]
                .as_f64()
                .ok_or_else(|| QError::InvalidRequest("b inválido".into()))?;
            Ok(ToolOutput {
                content: (a + b).to_string(),
                metadata: Default::default(),
            })
        }
    }

    #[tokio::test]
    async fn validates_and_completes() {
        let service = ChatService::new(vec![Arc::new(DemoForTest)]);
        let request = ChatRequest::new("default", vec![Message::user("hello")]);
        assert_eq!(service.complete(&request).await.unwrap().content, "hello");
    }
    struct DemoForTest;
    #[async_trait]
    impl ChatProvider for DemoForTest {
        fn name(&self) -> &'static str {
            "demo"
        }
        async fn complete(&self, request: &ChatRequest) -> Result<ChatResponse> {
            Ok(ChatResponse {
                id: "id".into(),
                model: "demo".into(),
                content: request.messages[0].content.clone(),
                finish_reason: Some("stop".into()),
                usage: None,
                tool_calls: vec![],
            })
        }
    }
    #[tokio::test]
    async fn rejects_empty_messages() {
        let service = ChatService::new(vec![Arc::new(DemoForTest)]);
        let request = ChatRequest::new("default", vec![]);
        assert!(matches!(
            service.complete(&request).await,
            Err(QError::InvalidRequest(_))
        ));
    }
    #[tokio::test]
    async fn executes_tool_and_continues_chat() {
        let mut registry = q_tools::ToolRegistry::new();
        registry.register(SumTool).unwrap();
        let provider = Arc::new(FakeProvider {
            calls: AtomicUsize::new(0),
        });
        let service = ChatService::new(vec![provider.clone()]).with_tools(Arc::new(registry));
        let result = service
            .complete(&ChatRequest::new(
                "default",
                vec![Message::user("quanto é 2+3?")],
            ))
            .await
            .unwrap();
        assert_eq!(result.content, "resultado: 5");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    }
}
