use async_trait::async_trait;
use q_core::{QError, Result, Tool, ToolDefinition, ToolOutput};
use serde_json::Value;
use std::process::Command;

/// Tool to execute a shell command (with timeout and output capture).
pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell".into(),
            description: "Executa um comando shell e retorna stdout/stderr. Use com cuidado."
                .into(),
            parameters_schema: serde_json::json!({
                "type":"object",
                "properties": {
                    "command": {"type":"string", "description":"Comando a executar"},
                    "timeout_secs": {"type":"integer", "description":"Timeout em segundos (padrão 30)"}
                },
                "required":["command"],
                "additionalProperties":false
            }),
        }
    }

    async fn execute(&self, arguments: Value) -> Result<ToolOutput> {
        let command = arguments["command"]
            .as_str()
            .ok_or_else(|| QError::InvalidRequest("shell.command deve ser texto".into()))?;
        let timeout_secs = arguments
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        let output = Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| QError::InvalidRequest(format!("falha ao executar: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let content = format!(
            "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        );
        // timeout_secs currently unused; reserved for future enforcement
        let _ = timeout_secs;
        Ok(ToolOutput {
            content,
            metadata: Default::default(),
        })
    }
}
