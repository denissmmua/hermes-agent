# Hermes Agent Rust Rewrite

NousResearch/hermes-agent rewritten in Rust.

## Architecture
- **hermes-core** — core types, traits
- **hermes-agent** — agent runtime (observe-think-act loop)
- **hermes-cli** — CLI interface (clap)
- **hermes-gateway** — LLM provider gateway
- **hermes-tools** — tool system (file, shell, search)
- **hermes-state** — memory, state, persistence

## Reference
Python original at `python-reference/` directory.
