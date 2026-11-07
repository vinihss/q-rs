use async_trait::async_trait;
use q_core::{QError, Result, Tool, ToolDefinition, ToolOutput};
use serde_json::Value;

pub struct CalculatorTool;

#[async_trait]
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
