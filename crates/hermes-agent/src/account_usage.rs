#![allow(dead_code)]
use hermes_core::Conversation;
use std::time::{Duration, Instant};

/// Token usage tracking per turn
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cache_creation_tokens: u32,
    pub cache_read_tokens: u32,
}

impl TokenUsage {
    pub fn new(prompt: u32, completion: u32) -> Self {
        Self {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
            ..Default::default()
        }
    }

    pub fn add(&mut self, other: &TokenUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
        self.cache_creation_tokens += other.cache_creation_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
    }
}

/// Per-provider cost rates (USD per 1M tokens)
const COST_RATES: &[(&str, f64, f64, f64, f64)] = &[
    // (name, input_1m, output_1m, cache_hit_1m, cache_miss_1m)
    ("deepseek-chat",      0.27,  1.10,  0.07,  0.27),
    ("deepseek-v4-flash",  0.27,  1.10,  0.07,  0.27),
    ("gpt-4o",            2.50, 10.00,  1.25,  2.50),
    ("gpt-4o-mini",       0.15,  0.60,  0.075, 0.15),
    ("claude-sonnet-4",   3.00, 15.00,  0.30,  3.00),
    ("claude-opus-4",    15.00, 75.00,  1.50, 15.00),
];

/// Estimate cost from token usage
pub fn estimate_cost(usage: &TokenUsage, model: &str) -> String {
    let model = model.to_lowercase();
    let mut input_rate = 0.27;
    let mut output_rate = 1.10;

    for (name, inp, outp, _, _) in COST_RATES {
        if model.contains(*name) {
            input_rate = *inp;
            output_rate = *outp;
            break;
        }
    }

    let input_cost = usage.prompt_tokens as f64 * input_rate / 1_000_000.0;
    let output_cost = usage.completion_tokens as f64 * output_rate / 1_000_000.0;
    let total = input_cost + output_cost;

    if total < 0.01 {
        format!("${:.4}", total)
    } else {
        format!("${:.2}", total)
    }
}

/// Session-level usage tracking
#[derive(Debug, Clone)]
pub struct UsageTracker {
    pub model: String,
    pub turns: u32,
    pub total_usage: TokenUsage,
    pub turn_usage: Vec<TokenUsage>,
    pub session_start: Instant,
    pub turn_times: Vec<Duration>,
}

impl UsageTracker {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            turns: 0,
            total_usage: TokenUsage::default(),
            turn_usage: Vec::new(),
            session_start: Instant::now(),
            turn_times: Vec::new(),
        }
    }

    pub fn record_turn(&mut self, usage: TokenUsage, duration: Duration) {
        self.turns += 1;
        self.total_usage.add(&usage);
        self.turn_usage.push(usage);
        self.turn_times.push(duration);
    }

    pub fn total_cost_estimate(&self) -> String {
        estimate_cost(&self.total_usage, &self.model)
    }

    pub fn avg_turn_time(&self) -> Duration {
        if self.turn_times.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.turn_times.iter().sum();
        total / self.turn_times.len() as u32
    }

    pub fn summary(&self) -> String {
        format!(
            "{}: {} turns, {} tokens ({}), avg {}",
            self.model,
            self.turns,
            self.total_usage.total_tokens,
            self.total_cost_estimate(),
            format_duration(&self.avg_turn_time())
        )
    }
}

fn format_duration(dur: &Duration) -> String {
    let secs = dur.as_secs_f64();
    if secs < 1.0 {
        format!("{:.0}ms", dur.as_millis())
    } else if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        format!("{:.0}m", secs / 60.0)
    }
}

/// Rough token counting for conversation
pub fn estimate_conversation_tokens(conv: &Conversation) -> u32 {
    conv.messages.iter().map(|m| {
        let base = (m.content.len() as f64 / 3.5) as u32 + 4;
        base + match &m.tool_calls {
            Some(calls) => calls.len() as u32 * 20,
            None => 0,
        }
    }).sum()
}
