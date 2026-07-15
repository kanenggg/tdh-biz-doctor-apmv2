# CLAUDE.md

Claude-specific repository context. Use `AGENTS.md` as the source of truth for project structure, Rust style, repository hygiene, and active package guidance.

## Test Commands

Use `cargo nextest run` for Rust tests in this repository.

```bash
cargo nextest run --workspace

cargo nextest run -p consultation-rs
cargo nextest run -p consultation-rs --test token_and_room_test --no-capture

cargo nextest run -p consultation-bg-rs
cargo nextest run -p common-rs
```

Do not use Cargo's built-in test runner in routine agent instructions unless a task explicitly requires it.
