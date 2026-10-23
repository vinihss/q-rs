//! Tools module for q-rs.
//! Provides limited, safe tool execution for the CLI.
//! Each tool runs via std::process::Command::output() (blocking, but fine for CLI).

use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

/// Represents a tool call from the user or model.
#[derive(Deserialize, Debug)]
pub struct ToolCall {
    pub name: String,
    #[serde(default)]
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Result of executing a tool.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub name: String,
    pub output: String,
    pub error: Option<String>,
    pub success: bool,
}

/// Available tools.
#[derive(Clone, Debug)]
pub struct AvailableTools;

impl AvailableTools {
    /// List of available tools with descriptions.
    pub fn list() -> Vec<(String, String)> {
        vec![
            (
                "shell".to_string(),
                "Executes a shell command (limited - no pipes/redirection)".to_string(),
            ),
            (
                "fs_read".to_string(),
                "Reads the content of a relative file".to_string(),
            ),
        ]
    }

    /// Execute a tool and return the result.
    /// Uses std::process::Command::output() directly (blocking, suitable for CLI).
    pub fn execute(call: &ToolCall) -> ToolResult {
        let name = &call.name;

        match name.as_str() {
            "shell" => {
                let command = call
                    .arguments
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Security: block pipes, redirection, dangerous operators
                if command.contains('|')
                    || command.contains('>')
                    || command.contains('<')
                    || command.contains("&&")
                    || command.contains("||")
                    || command.chars().any(|c| ";|&<>'\"`.\\".contains(c))
                {
                    return ToolResult {
                        name: "shell".to_string(),
                        output: String::new(),
                        error: Some("Comando bloqueado: operadores perigosos não são permitidos".to_string()),
                        success: false,
                    };
                }

                // Max 100 characters
                if command.len() > 100 {
                    return ToolResult {
                        name: "shell".to_string(),
                        output: String::new(),
                        error: Some("Comando muito longo (máx. 100 chars)".to_string()),
                        success: false,
                    };
                }

                // Execute command
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output();

                match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        if !stderr.is_empty() {
                            ToolResult {
                                name: "shell".to_string(),
                                output: format!("STDOUT: {}\nSTDERR: {}", stdout, stderr),
                                error: None,
                                success: out.status.success(),
                            }
                        } else {
                            ToolResult {
                                name: "shell".to_string(),
                                output: stdout,
                                error: None,
                                success: out.status.success(),
                            }
                        }
                    }
                    Err(e) => ToolResult {
                        name: "shell".to_string(),
                        output: String::new(),
                        error: Some(format!("Erro ao executar: {e}")),
                        success: false,
                    },
                }
            }
            "fs_read" => {
                let path = call
                    .arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Security: relative paths only, no "..", no absolute, no "~"
                if path.contains("..")
                    || path.starts_with('/')
                    || path.starts_with('~')
                {
                    return ToolResult {
                        name: "fs_read".to_string(),
                        output: String::new(),
                        error: Some("Caminho inválido: use caminhos relativos ao projeto".to_string()),
                        success: false,
                    };
                }

                if path.len() > 200 {
                    return ToolResult {
                        name: "fs_read".to_string(),
                        output: String::new(),
                        error: Some("Caminho muito longo".to_string()),
                        success: false,
                    };
                }

                match fs::read_to_string(path) {
                    Ok(content) => ToolResult {
                        name: "fs_read".to_string(),
                        output: content,
                        error: None,
                        success: true,
                    },
                    Err(e) => ToolResult {
                        name: "fs_read".to_string(),
                        output: String::new(),
                        error: Some(format!("Erro ao ler arquivo: {e}")),
                        success: false,
                    },
                }
            }
            _ => ToolResult {
                name: name.clone(),
                output: String::new(),
                error: Some("Tool desconhecida".to_string()),
                success: false,
            },
        }
    }
}

/// Run a tool from CLI arguments.
/// Parses "q-rs tool <name> [args]" and executes the tool.
/// Returns the output string (or error string).
pub async fn run_tool_from_cli(args: &[String]) -> Result<String, String> {
    if args.len() < 2 {
        return Err("Uso: q-rs tool <nome> [args]".to_string());
    }

    let tool_name = &args[1];
    let mut args_map: HashMap<String, serde_json::Value> = HashMap::new();

    if tool_name == "shell" && args.len() > 2 {
        // Treat remaining args as a single command string
        let cmd_str = args[2..].join(" ");
        tool_args.insert("command".to_string(), serde_json::Value::String(cmd_str));
    }

    let tool_call = serde_json::from_str::<ToolCall>(&format!(
        r#"{{
            "name": "{}",
            "arguments": {}
        }}"#,
        tool_name,
        serde_json::to_string(&tool_args).unwrap_or_default()
    ))
    .map_err(|e| format!("Erro ao parsear tool call: {e}"))?;

    let result = Self::execute(&tool_call);
    if result.success {
        Ok(result.output)
    } else {
        Err(result.error.unwrap_or_else(|| "Tool failed".to_string()))
    }
}
