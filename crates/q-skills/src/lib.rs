//! Skill registration boundary. Skills should be explicit, named and independently testable.
use q_core::{QError, Result, Skill, SkillContext, SkillDefinition, SkillResult};
use std::{collections::HashMap, sync::Arc};

#[derive(Default, Clone)]
pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
}
impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register<S: Skill + 'static>(&mut self, skill: S) -> Result<()> {
        let name = skill.definition().name;
        if name.trim().is_empty() {
            return Err(QError::Configuration(
                "skill name não pode ser vazio".into(),
            ));
        }
        if self.skills.contains_key(&name) {
            return Err(QError::Configuration(format!("skill duplicada: {name}")));
        }
        self.skills.insert(name, Arc::new(skill));
        Ok(())
    }
    pub fn definitions(&self) -> Vec<SkillDefinition> {
        self.skills.values().map(|s| s.definition()).collect()
    }
    pub async fn run(
        &self,
        name: &str,
        input: &str,
        context: &SkillContext,
    ) -> Result<SkillResult> {
        self.skills
            .get(name)
            .ok_or_else(|| QError::SkillNotFound(name.to_owned()))?
            .run(input, context)
            .await
    }
    pub fn contains(&self, name: &str) -> bool {
        self.skills.contains_key(name)
    }
}
