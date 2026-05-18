/// Display formatting — spinner, diffs, tool preview
use std::io::Write;
use std::fmt;
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════════
// ANSI Colors
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct AnsiColors {
    pub reset: &'static str,
    pub green: &'static str,
    pub red: &'static str,
    pub yellow: &'static str,
    pub blue: &'static str,
    pub cyan: &'static str,
    pub magenta: &'static str,
    pub dim: &'static str,
    pub bold: &'static str,
}

pub const ANSI: AnsiColors = AnsiColors {
    reset: "\x1b[0m",
    green: "\x1b[32m",
    red: "\x1b[31m",
    yellow: "\x1b[33m",
    blue: "\x1b[34m",
    cyan: "\x1b[36m",
    magenta: "\x1b[35m",
    dim: "\x1b[2m",
    bold: "\x1b[1m",
};

/// Color a string
pub fn color(text: &str, code: &str) -> String {
    format!("{}{}{}", code, text, ANSI.reset)
}

pub fn green(text: &str) -> String { color(text, ANSI.green) }
pub fn red(text: &str) -> String { color(text, ANSI.red) }
pub fn yellow(text: &str) -> String { color(text, ANSI.yellow) }
pub fn blue(text: &str) -> String { color(text, ANSI.blue) }
pub fn cyan(text: &str) -> String { color(text, ANSI.cyan) }
pub fn dim(text: &str) -> String { color(text, ANSI.dim) }
pub fn bold(text: &str) -> String { color(text, ANSI.bold) }

// ═══════════════════════════════════════════════════════════════════════
// Spinner
// ═══════════════════════════════════════════════════════════════════════

const SPINNER_CHARS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SPINNER_INTERVAL: Duration = Duration::from_millis(80);

pub struct Spinner {
    pub message: String,
    frame: usize,
    last_tick: Instant,
    running: bool,
}

impl Spinner {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            frame: 0,
            last_tick: Instant::now(),
            running: false,
        }
    }

    pub fn start(&mut self) {
        self.running = true;
        self.frame = 0;
        self.last_tick = Instant::now();
        self.render();
    }

    pub fn stop(&mut self) {
        self.running = false;
        print!("\r\x1b[K");
        std::io::stdout().flush().ok();
    }

    pub fn tick(&mut self) {
        if !self.running { return; }
        if self.last_tick.elapsed() >= SPINNER_INTERVAL {
            self.frame = (self.frame + 1) % SPINNER_CHARS.len();
            self.last_tick = Instant::now();
            self.render();
        }
    }

    pub fn render(&self) {
        if !self.running { return; }
        let spinner = SPINNER_CHARS[self.frame];
        print!("\r{} {} ", cyan(spinner), dim(&self.message));
        std::io::stdout().flush().ok();
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message = msg.to_string();
        self.render();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tool preview formatting
// ═══════════════════════════════════════════════════════════════════════

/// Format tool call for preview
pub fn format_tool_call(tool_name: &str, args: &serde_json::Value) -> String {
    let args_str = match args {
        serde_json::Value::Object(obj) => {
            let parts: Vec<String> = obj.iter()
                .map(|(k, v)| {
                    let val = match v {
                        serde_json::Value::String(s) if s.len() > 50 => {
                            format!("{}...", &s[..47])
                        }
                        _ => v.to_string(),
                    };
                    format!("{}={}", k, val)
                })
                .collect();
            parts.join(", ")
        }
        _ => args.to_string(),
    };
    format!("{}({})", bold(tool_name), dim(&args_str))
}

/// Format tool result for display (truncated)
pub fn format_tool_result(result: &str, max_len: usize) -> String {
    if result.len() <= max_len {
        result.to_string()
    } else {
        format!("{}... ({} more chars)", &result[..max_len], result.len() - max_len)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Diff formatting
// ═══════════════════════════════════════════════════════════════════════

/// Simple line diff
pub fn format_diff(old: &str, new: &str, context_lines: usize) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut result = String::new();
    let mut old_idx = 0usize;
    let mut new_idx = 0usize;

    while old_idx < old_lines.len() || new_idx < new_lines.len() {
        if old_idx < old_lines.len() && new_idx < new_lines.len() && old_lines[old_idx] == new_lines[new_idx] {
            result.push_str(&format!(" {}\n", old_lines[old_idx]));
            old_idx += 1;
            new_idx += 1;
        } else {
            // Lines differ — show removed and added
            if old_idx < old_lines.len() {
                result.push_str(&format!("{}-{}\n", ANSI.red, old_lines[old_idx]));
                old_idx += 1;
            }
            if new_idx < new_lines.len() {
                result.push_str(&format!("{}+{}\n", ANSI.green, new_lines[new_idx]));
                new_idx += 1;
            }
        }
    }

    result
}

// ═══════════════════════════════════════════════════════════════════════
// Status bar components
// ═══════════════════════════════════════════════════════════════════════

pub fn format_context_bar(percent: f64, width: usize) -> String {
    let filled = ((percent * width as f64) as usize).min(width);
    let empty = width.saturating_sub(filled);

    let fill_char = if percent > 0.8 { "█" } else { "▓" };
    let bar: String = std::iter::repeat(fill_char).take(filled).collect();
    let space: String = std::iter::repeat("░").take(empty).collect();

    let bar_color = if percent > 0.9 {
        ANSI.red
    } else if percent > 0.7 {
        ANSI.yellow
    } else {
        ANSI.green
    };

    format!("{}{}{}{}{:.0}%{}", bar_color, bar, ANSI.dim, space, percent * 100.0, ANSI.reset)
}

pub fn format_elapsed(dur: Duration) -> String {
    let secs = dur.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Format token count compactly
pub fn format_tokens(count: u32) -> String {
    if count < 1000 {
        format!("{}t", count)
    } else {
        format!("{:.1}Kt", count as f64 / 1000.0)
    }
}
