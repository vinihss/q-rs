use async_trait::async_trait;
use q_core::{ChatProvider, ChatRequest, ChatResponse, QError, Result, ToolCall, Usage};
use serde_json::{json, Value};

/// Adapter for endpoints implementing the OpenAI Chat Completions JSON protocol.
/// Streaming is intentionally left to a follow-up adapter implementation; the core port already defines it.
pub struct OpenAiCompatibleProvider {
    name: &'static str,
    base_url: String,
    api_key: String,
    default_model: String,
    client: reqwest::Client,
}
impl OpenAiCompatibleProvider {
    pub fn new(
        name: &'static str,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        default_model: impl Into<String>,
    ) -> Self {
        Self {
            name,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            default_model: default_model.into(),
            client: reqwest::Client::new(),
        }
    }
    fn upstream_model(&self, requested: &str) -> String {
        if requested == "default" || requested.is_empty() {
            return self.default_model.clone();
        }
        requested
            .strip_prefix(&format!("{}/", self.name))
            .unwrap_or(requested)
            .to_string()
    }
}
#[async_trait]
impl ChatProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &'static str {
        self.name
    }
    async fn complete(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let model = self.upstream_model(&request.model);
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut value = json!({"role": m.role.as_str(), "content": m.content});
                if let Some(name) = &m.name {
                    value["name"] = json!(name);
                }
                if let Some(id) = &m.tool_call_id {
                    value["tool_call_id"] = json!(id);
                }
                if let Some(calls) = &m.tool_calls {
                    value["tool_calls"] =
                        json!(calls.iter().map(|call| json!({
                    "id": call.id,
                    "type": "function",
                    "function": {"name": call.name, "arguments": call.arguments.to_string()}
                })).collect::<Vec<_>>());
                }
                value
            })
            .collect();
        let mut body = json!({"model": model, "messages": messages, "stream": false});
        if let Some(tools) = &request.tools {
            if !tools.is_empty() {
                body["tools"] = json!(tools.iter().map(|tool| json!({
                    "type": "function",
                    "function": {"name": tool.name, "description": tool.description, "parameters": tool.parameters_schema}
                })).collect::<Vec<_>>());
                body["tool_choice"] = json!("auto");
            }
        }
        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| QError::Provider {
                provider: self.name.into(),
                message: e.to_string(),
            })?;
        let status = response.status();
        let body_text = response.text().await.map_err(|e| QError::Provider {
            provider: self.name.into(),
            message: format!("não foi possível ler resposta: {e}"),
        })?;
        if !status.is_success() {
            let excerpt: String = body_text.chars().take(500).collect();
            return Err(QError::Provider {
                provider: self.name.into(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        let value: Value = serde_json::from_str(&body_text).map_err(|e| QError::Provider {
            provider: self.name.into(),
            message: format!("JSON inválido: {e}"),
        })?;
        let content = value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .unwrap_or("");
        let tool_calls = value
            .pointer("/choices/0/message/tool_calls")
            .and_then(Value::as_array)
            .map(|calls| {
                calls
                    .iter()
                    .map(|call| {
                        let id = call
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let name = call
                            .pointer("/function/name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let raw_args = call
                            .pointer("/function/arguments")
                            .and_then(Value::as_str)
                            .unwrap_or("{}");
                        let arguments =
                            serde_json::from_str(raw_args).map_err(|e| QError::Provider {
                                provider: self.name.into(),
                                message: format!(
                                    "argumentos JSON inválidos para tool call '{name}': {e}"
                                ),
                            })?;
                        if id.is_empty() || name.is_empty() {
                            return Err(QError::Provider {
                                provider: self.name.into(),
                                message: "tool call sem id ou nome".into(),
                            });
                        }
                        Ok(ToolCall {
                            id,
                            name,
                            arguments,
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        let usage = value.get("usage").map(|u| Usage {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
            total_tokens: u["total_tokens"].as_u64().unwrap_or(0),
        });
        let finish_reason = value
            .pointer("/choices/0/finish_reason")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("chatcmpl-q-rs")
            .to_owned();
        let returned_model = value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(&model)
            .to_owned();
        Ok(ChatResponse {
            id,
            model: returned_model,
            content: content.to_owned(),
            finish_reason,
            usage,
            tool_calls,
        })
    }
}
