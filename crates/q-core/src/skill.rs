use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub instructions: String,
}
#[derive(Debug, Clone, Default)]
pub struct SkillContext {
    pub user_id: Option<String>,
    pub working_directory: Option<String>,
    pub values: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    pub content: String,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

#[async_trait::async_trait]
pub trait Skill: Send + Sync {
    fn definition(&self) -> SkillDefinition;
    async fn run(&self, input: &str, context: &SkillContext) -> crate::Result<SkillResult>;
}
