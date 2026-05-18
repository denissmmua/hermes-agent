use std::collections::HashMap;
use std::path::Path;
use async_trait::async_trait;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn description(&self) -> &str;
    async fn on_load(&self) -> Result<(), String>;
    async fn on_unload(&self) -> Result<(), String>;
}

pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
    search_paths: Vec<String>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugins: HashMap::new(), search_paths: Vec::new() }
    }

    pub fn add_search_path(&mut self, path: impl Into<String>) {
        self.search_paths.push(path.into());
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        let n = plugin.name().to_string();
        self.plugins.insert(n, plugin);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Plugin>> {
        self.plugins.get(name)
    }

    pub fn names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool { self.plugins.is_empty() }
    pub fn len(&self) -> usize { self.plugins.len() }

    pub async fn load_builtins(&mut self) {
        self.register(Box::new(VersionPlugin));
        self.register(Box::new(HelpPlugin));
    }

    pub async fn scan_paths(&mut self) {
        let paths = self.search_paths.clone();
        let mut found: Vec<Box<dyn Plugin>> = Vec::new();
        for ps in &paths {
            let p = Path::new(ps);
            if !p.exists() { continue; }
            let dir = match std::fs::read_dir(p) { Ok(d) => d, Err(_) => continue };
            for e in dir.flatten() {
                let f = e.file_name().to_string_lossy().to_string();
                if !f.ends_with(".rs") && !f.ends_with(".sh") && !f.ends_with(".py") { continue; }
                let n = f.trim_end_matches(".rs").trim_end_matches(".sh").trim_end_matches(".py").to_string();
                found.push(Box::new(ScriptPlugin { name: n, path: e.path().to_string_lossy().to_string() }));
            }
        }
        for p in found { self.register(p); }
    }
}

struct VersionPlugin;
#[async_trait]
impl Plugin for VersionPlugin {
    fn name(&self) -> &str { "version" }
    fn version(&self) -> &str { "1.0.0" }
    fn description(&self) -> &str { "Version information" }
    async fn on_load(&self) -> Result<(), String> { Ok(()) }
    async fn on_unload(&self) -> Result<(), String> { Ok(()) }
}

struct HelpPlugin;
#[async_trait]
impl Plugin for HelpPlugin {
    fn name(&self) -> &str { "help" }
    fn version(&self) -> &str { "1.0.0" }
    fn description(&self) -> &str { "Help system" }
    async fn on_load(&self) -> Result<(), String> { Ok(()) }
    async fn on_unload(&self) -> Result<(), String> { Ok(()) }
}

struct ScriptPlugin { name: String, path: String }
#[async_trait]
impl Plugin for ScriptPlugin {
    fn name(&self) -> &str { &self.name }
    fn version(&self) -> &str { "0.1.0" }
    fn description(&self) -> &str { "Script plugin" }
    async fn on_load(&self) -> Result<(), String> { Ok(()) }
    async fn on_unload(&self) -> Result<(), String> { Ok(()) }
}
