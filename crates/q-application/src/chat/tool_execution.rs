use std::sync::Arc;

use q_core::{Message, Result, Role, ToolExecutor};

use crate::ChatService;

impl ChatService {
    /// Appends the assistant tool-call turn and executes each tool call, appending
    /// the resulting tool messages to `working`. Returns Ok(()) on success.
    pub async fn append_tool_results(
        &self,
        working: &mut q_core::ChatRequest,
        response_content: String,
        tool_calls: Vec<q_core::ToolCall>,
        executor: &Arc<dyn ToolExecutor>,
    ) -> Result<()> {
        // Keep the assistant tool-call turn in the transcript before tool results.
        let mut assistant_message = q_core::Message::assistant(response_content);
        assistant_message.tool_calls = Some(tool_calls.clone());
        working.messages.push(assistant_message);

        for call in tool_calls {
            let output = match executor.execute(&call.name, call.arguments.clone()).await {
                Ok(output) => output.content,
                Err(error) => format!("Erro ao executar ferramenta '{}': {}", call.name, error),
            };
            working.messages.push(Message {
                role: Role::Tool,
                content: output,
                name: Some(call.name),
                tool_call_id: Some(call.id),
                tool_calls: None,
            });
        }
        Ok(())
    }
}
