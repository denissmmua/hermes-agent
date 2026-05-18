use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    fn init_instructions(&self) -> String;
}

pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self { skills: HashMap::new() }
    }

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

    pub fn collect_init_instructions(&self) -> String {
        self.skills.values()
            .map(|s| s.init_instructions())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn is_empty(&self) -> bool { self.skills.is_empty() }

    pub fn len(&self) -> usize { self.skills.len() }
}

/// Load skills from a directory (each subdir = one skill with SKILL.md + tools/)
pub async fn load_skills_from_dir(dir: impl AsRef<Path>) -> Vec<Box<dyn Skill>> {
    let mut skills: Vec<Box<dyn Skill>> = Vec::new();
    let dir_path = dir.as_ref().to_path_buf();

    let mut entries = match tokio::fs::read_dir(&dir_path).await {
        Ok(d) => d,
        Err(_) => return skills,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_dir() { continue; }

        let skill_file = path.join("SKILL.md");
        if !skill_file.exists() { continue; }

        if let Ok(content) = tokio::fs::read_to_string(&skill_file).await {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let meta = SkillMeta {
                name,
                description: content.lines().next().unwrap_or("").trim().to_string(),
                version: "1.0.0".to_string(),
                author: None,
                dependencies: Vec::new(),
                tools: Vec::new(),
            };

            // Look for tool files in the skill directory
            let mut tool_files = Vec::new();
            if let Ok(mut tool_dir) = tokio::fs::read_dir(&path).await {
                while let Ok(Some(tool_entry)) = tool_dir.next_entry().await {
                    let tool_path = tool_entry.path();
                    if tool_path.extension().map_or(false, |e| e == "rs" || e == "py" || e == "sh") {
                        tool_files.push(tool_path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string());
                    }
                }
            }

            skills.push(Box::new(FileSystemSkill { meta, content }));
        }
    }

    skills
}

struct FileSystemSkill {
    meta: SkillMeta,
    content: String,
}

#[async_trait]
impl Skill for FileSystemSkill {
    fn meta(&self) -> &SkillMeta { &self.meta }
    fn init_instructions(&self) -> String { self.content.clone() }
}
