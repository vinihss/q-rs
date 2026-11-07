use q_core::{QError, Result, Tool, ToolDefinition, ToolExecutor, ToolOutput};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

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
