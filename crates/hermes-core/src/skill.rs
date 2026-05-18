use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub dependencies: Vec<String>,
}

#[async_trait]
pub trait Skill: Send + Sync {
    fn meta(&self) -> &SkillMeta;
    fn init_instructions(&self) -> String;
}

pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    pub fn register(&mut self, skill: Box<dyn Skill>) {
        let name = skill.meta().name.clone();
        self.skills.insert(name, skill);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Skill>> {
        self.skills.get(name)
    }

    pub fn all_names(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }

    pub fn collect_init_instructions(&self) -> String {
        self.skills
            .values()
            .map(|s| s.init_instructions())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}
