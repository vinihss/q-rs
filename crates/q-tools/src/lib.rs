//! Tool registration and execution boundary. No tool implementation is trusted by default.
use q_core::{QError, Result, Tool, ToolDefinition, ToolExecutor, ToolOutput};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}
impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register<T: Tool + 'static>(&mut self, tool: T) -> Result<()> {
        let name = tool.definition().name;
        if name.trim().is_empty() {
            return Err(QError::Configuration("tool name não pode ser vazio".into()));
        }
        if self.tools.contains_key(&name) {
            return Err(QError::Configuration(format!("tool duplicada: {name}")));
        }
        self.tools.insert(name, Arc::new(tool));
        Ok(())
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }
    pub async fn execute(&self, name: &str, arguments: Value) -> Result<ToolOutput> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| QError::ToolNotFound(name.to_owned()))?;
        tool.execute(arguments).await
    }
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ToolRegistry {
    fn definitions(&self) -> Vec<ToolDefinition> {
        ToolRegistry::definitions(self)
    }
    async fn execute(&self, name: &str, arguments: Value) -> Result<ToolOutput> {
        ToolRegistry::execute(self, name, arguments).await
    }
}

/// Small safe example tool, useful for validating automatic tool execution end-to-end.
pub struct CalculatorTool;

#[async_trait::async_trait]
impl Tool for CalculatorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "calculator".into(),
            description:
                "Calcula uma operação aritmética simples: add, subtract, multiply ou divide.".into(),
            parameters_schema: serde_json::json!({
                "type":"object",
                "properties": {
                    "operation": {"type":"string", "enum":["add","subtract","multiply","divide"]},
                    "a": {"type":"number"},
                    "b": {"type":"number"}
                },
                "required":["operation","a","b"],
                "additionalProperties":false
            }),
        }
    }

    async fn execute(&self, arguments: Value) -> Result<ToolOutput> {
        let operation = arguments["operation"]
            .as_str()
            .ok_or_else(|| QError::InvalidRequest("calculator.operation deve ser texto".into()))?;
        let a = arguments["a"]
            .as_f64()
            .ok_or_else(|| QError::InvalidRequest("calculator.a deve ser número".into()))?;
        let b = arguments["b"]
            .as_f64()
            .ok_or_else(|| QError::InvalidRequest("calculator.b deve ser número".into()))?;
        let value = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" if b != 0.0 => a / b,
            "divide" => return Err(QError::InvalidRequest("divisão por zero".into())),
            other => {
                return Err(QError::InvalidRequest(format!(
                    "operação não suportada: {other}"
                )))
            }
        };
        Ok(ToolOutput {
            content: value.to_string(),
            metadata: Default::default(),
        })
    }
}
