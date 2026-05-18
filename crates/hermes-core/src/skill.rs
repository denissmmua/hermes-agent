use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[async_trait]
pub trait Skill: Send + Sync {
    fn meta(&self) -> &SkillMeta;
    fn init_instructions(&self) -> &str;
}

pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self { Self { skills: HashMap::new() } }

    pub fn register(&mut self, skill: Box<dyn Skill>) {
        let name = skill.meta().name.clone();
        self.skills.insert(name, skill);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Skill>> {
        self.skills.get(name)
    }

    pub fn names(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool { self.skills.is_empty() }
    pub fn len(&self) -> usize { self.skills.len() }

    pub fn collect_init_instructions(&self) -> String {
        self.skills.values()
            .map(|s| s.init_instructions())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }
}

pub async fn load_skills_from_dir(dir: &Path) -> Vec<Box<dyn Skill>> {
    let mut skills: Vec<Box<dyn Skill>> = Vec::new();
    let mut entries = match fs::read_dir(dir).await {
        Ok(d) => d,
        Err(_) => return skills,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_dir() { continue; }

        let skill_file = path.join("SKILL.md");
        if !skill_file.exists() { continue; }

        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let content = fs::read_to_string(&skill_file).await.unwrap_or_default();
        let description = content.lines().next().unwrap_or("").trim().to_string();

        let meta = SkillMeta {
            name, description, version: "1.0.0".to_string(),
            author: None, dependencies: Vec::new(), tools: Vec::new(),
        };

        skills.push(Box::new(FsSkill { meta, content }));
    }
    skills
}

struct FsSkill { meta: SkillMeta, content: String }

#[async_trait]
impl Skill for FsSkill {
    fn meta(&self) -> &SkillMeta { &self.meta }
    fn init_instructions(&self) -> &str { &self.content }
}
