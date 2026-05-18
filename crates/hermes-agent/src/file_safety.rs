use std::path::Path;

/// Sensitive paths that should never be written by the agent
pub fn build_denied_paths(home_dir: &str) -> Vec<String> {
    let hermes_home = format!("{}/.hermes", home_dir);
    let mut paths = vec![
        format!("{}/.ssh/authorized_keys", home_dir),
        format!("{}/.ssh/id_rsa", home_dir),
        format!("{}/.ssh/id_ed25519", home_dir),
        format!("{}/.ssh/config", home_dir),
        format!("{}/.env", hermes_home),
        format!("{}/.bashrc", home_dir),
        format!("{}/.zshrc", home_dir),
        format!("{}/.profile", home_dir),
        format!("{}/.bash_profile", home_dir),
        format!("{}/.netrc", home_dir),
        format!("{}/.npmrc", home_dir),
        format!("{}/.pypirc", home_dir),
        "/etc/sudoers".to_string(),
        "/etc/passwd".to_string(),
        "/etc/shadow".to_string(),
        "/etc/ssh/sshd_config".to_string(),
    ];

    // Resolve to real paths
    paths.iter().map(|p| {
        std::fs::canonicalize(Path::new(p)).unwrap_or_else(|_| p.clone())
    }).collect()
}

/// Check if writing to this path is safe
pub fn is_safe_write_path(path: &str, denied: &[String]) -> bool {
    let real = std::fs::canonicalize(Path::new(path)).unwrap_or_else(|_| path.to_string());
    !denied.iter().any(|d| real == *d || real.starts_with(d))
}

/// Check if a path looks like a system or config file
pub fn is_sensitive_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("/etc/")
        || lower.contains("/boot/")
        || lower.contains("/dev/")
        || lower.contains("/sys/")
        || lower.contains("/proc/")
        || lower.contains("/usr/lib")
        || lower.contains("/var/lib")
        || path.contains(".ssh")
        || path.contains(".gnupg")
        || path.contains(".config/git")
        || path.contains(".aws")
        || path.contains(".docker")
}

/// Danger level for file write operations
#[derive(Debug, Clone, PartialEq)]
pub enum WriteDanger {
    Safe,
    Warn,
    Deny,
}

impl WriteDanger {
    /// Check if a write operation on this path is dangerous
    pub fn assess(path: &str, denied: &[String]) -> Self {
        let in_hermes = path.contains("/.hermes/") && !path.contains("config.yaml");
        let in_workspace = path.contains("/ao-data/") || path.contains("/workspace");

        if is_safe_write_path(path, denied) {
            if is_sensitive_path(path) {
                WriteDanger::Warn
            } else if in_hermes {
                WriteDanger::Warn
            } else if path.contains("/root/") && !in_workspace {
                WriteDanger::Warn
            } else {
                WriteDanger::Safe
            }
        } else {
            WriteDanger::Deny
        }
    }
}

/// Validate a write path and return a reason if denied
pub fn validate_write(path: &str) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let denied = build_denied_paths(&home);

    match WriteDanger::assess(path, &denied) {
        WriteDanger::Deny => Err(format!("Write to '{}' is denied (sensitive path)", path)),
        WriteDanger::Warn => {
            // Allow with warning
            eprintln!("⚠ Warning: writing to '{}'", path);
            Ok(())
        }
        WriteDanger::Safe => Ok(()),
    }
}
