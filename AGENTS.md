# AGENTS.md

Guidelines for agentic coding agents working in this repository.

**Last refreshed:** 2026-07-06

## Project Overview

Project: **tdh-biz-doctor-apmv2 / Biz Apm**. This checkout is currently a Rust-focused service repository. Some old docs, deployment files, release notes, manifests, and generated artifacts still contain stale references to packages that have been removed or moved elsewhere.

Current stack and package manifests:

- Workspace: Cargo resolver `3`; primary crates use Rust edition 2024 (`common-rs` is still edition 2021).
- HTTP/runtime: `axum` 0.8, `tokio` 1, `clap` 4 for service args/CLI, `tracing`/`tracing-subscriber` for logs.
- Data/integration: `sqlx` 0.8 with PostgreSQL, Redis/deadpool-redis, Google Cloud Pub/Sub/KMS/Storage, Twilio helpers, Paseto/PASETO/PASERK-related crypto, OpenAPI via `utoipa`.
- Time: prefer `jiff`/`jiff-sqlx` for new code; `chrono` remains only at legacy/shared boundaries.

Active local Rust packages:

- `consultation-rs`: Axum API service for appointment booking/list/detail, consultation session info/end session, patient verification, facial upload, summary notes/follow-up, OpenAPI docs, and internal APIs.
- `consultation-bg-rs`: Axum background webhook/worker service, currently focused on payment confirmation Pub/Sub push handling.
- `common-rs`: shared config loading, Twilio helpers, text masking, protocol models, event models, and common repository helpers.
- `cli`: local tooling for Twilio token utilities and OpenAPI generation.

## Project Structure Context

Use the current filesystem as the source of truth for active code:

- `consultation-rs/`: primary API service. Feature folders usually contain `handler`, `service`, `repo`, models, and `mod.rs`.
  - `src/appointment/`: appointment list/detail/reserve-timeslot/summary-note flows and appointment state/types.
  - `src/consultation/`: session info, end session, facial upload, patient verification, shared consultation state.
  - `src/summarization/`: summary-note and follow-up logic.
  - `src/internal/`: internal API handlers/repo/service.
  - `src/protocol/` and `src/openapi/`: protocol-facing schemas and generated-doc wiring.
  - `src/common/`: service-local middleware, HTTP errors, infrastructure, and Twilio client abstraction.
- `consultation-bg-rs/`: background webhook/worker service. Current payment-confirm logic is under `src/payment_confirm/`; `src/event/` publishes consultation events.
- `common-rs/`: shared helpers used by active Rust services. `src/tdh_protocol/` contains many compatibility/protocol models; `src/twilio/` contains remaining Twilio support; `src/config/`, `src/repo/`, and `src/event/` provide shared infrastructure.
- `cli/`: developer tooling. Treat README/generated JSON or stale service references in this folder carefully; active binaries are `twilio` and `openapi`.
- `db/`: database migrations, seeds, and database helper scripts. Active Biz APM migrations live in `db/biz_apm/migrations/`; doctor schedule and MSSQL folders are supporting/manual context.
- `docs/`: requirements, plans, ADRs, diagrams, and structure review notes. Treat dated plans as historical unless the user points to one as current.
- `openapi/`: generated OpenAPI outputs. Prefer regeneration over hand edits unless the task explicitly targets checked-in generated artifacts.
- `specs/`: contract/spec artifacts when present. Save generated specifications under `specs/provides/`. The active runtime AsyncAPI (`biz-apm-published-events.asyncapi.yaml`) is V1-only and does not contain `consultation-event-v2`; the separate V2 AsyncAPI artifact is draft/model-only, not an active publication contract.
- `it/`: manual or external integration helpers, not necessarily active automated tests.
- `fixtures/`: sample JSON payloads and test data.
- `examples/`: prototypes or proof-of-concept code.

When ownership, active status, or the intended target path is unclear, ask the user before editing. Do this especially when a path is mentioned only by old docs or generated artifacts and does not exist in the current filesystem.

## Context Rule for Removed Paths

When gathering context, ignore paths that are absent from the filesystem even if old docs, manifests, generated files, or release notes still mention them. Do not treat removed paths as active source, do not restore them, and do not include them in implementation plans unless the user explicitly asks to clean stale references or restore a package.

Examples of removed/out-of-scope paths in the current tree include `doctor-pool/`, `twilio-rs/`, and `report/`. Twilio support that remains in this repo is under `common-rs/src/twilio/` and `cli/`.

## Current Cargo State

As of 2026-07-06, `cargo metadata --no-deps --format-version 1` succeeds and `cargo check --workspace` succeeds in this working tree, with warnings. Notable warning themes include unused imports/dead code and an unfinished `todo!()` path in `consultation-rs/src/appointment/reserve_timeslot/service.rs`.

Do not claim workspace Cargo/test/lint commands pass unless you have run them successfully in the current tree. The workspace may still contain stale references in old docs, deployment files, generated artifacts, or READMEs; treat those as documentation drift unless the user asks for cleanup.

## Build, Lint, and Test Commands

| Action | Command |
| --- | --- |
| Inspect workspace | `cargo metadata --no-deps --format-version 1` |
| Check workspace | `cargo check --workspace` |
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --workspace --all-targets` |
| Test workspace | `cargo nextest run --workspace` |
| Build CLI | `just build-cli` or `cargo build -p cli --release` |
| Generate OpenAPI | `just openapi consultation-rs` or `just openapi-all` |
| Build container locally | `just build-local consultation-rs` or `just build-local consultation-bg-rs` |

Package-specific checks/tests:

```bash
cargo check -p consultation-rs
cargo nextest run -p consultation-rs
cargo nextest run -p consultation-rs --test token_and_room_test --no-capture

cargo check -p consultation-bg-rs
cargo nextest run -p consultation-bg-rs

cargo check -p common-rs
cargo nextest run -p common-rs
```

Run services locally:

```bash
cargo run -p consultation-rs --bin consultation-rs -- --config-path ./consultation-rs/config/default.toml --config-path ./consultation-rs/config/local.toml
cargo run -p consultation-bg-rs --bin consultation-bg-rs -- --config-path ./consultation-bg-rs/config/default.toml
```

Hot reload and utility commands:

```bash
bacon                    # default bacon UI
bacon check              # cargo check
bacon check-all          # cargo check --all-targets
bacon clippy             # cargo clippy --all-targets
bacon test               # cargo test
bacon integration-test   # ignored consultation-rs integration tests
bacon run-consultation-rs # hot-reload consultation-rs; bacon keybinding `r`
```

Deployment/devspace helpers exist (`devspace.yaml`, `rust.Dockerfile`, `rust.cloudbuild.yaml`, `justfile`). Prefer `just --list` for the current recipe list.

## Where To Look

- Source crates: `consultation-rs/src/`, `consultation-bg-rs/src/`, `common-rs/src/`, `cli/src/`.
- Tests: `consultation-rs/tests/` and `common-rs/tests/`; many integration tests require external services or config.
- Config examples/defaults: `consultation-rs/config/`, `consultation-bg-rs/config/`, and `common-rs/src/config/loader.rs`.
- Database: `db/biz_apm/migrations/` for active Biz APM migrations; `db/biz_apm/seeds/` for seed helpers.
- Docs/decisions: `docs/`, especially `docs/adr/` and `docs/plans/`; `CLAUDE.md` mirrors test-command guidance and points back to this file.

## Rust Code Style

### Naming

Use standard Rust naming and keep domain names explicit.

```rust
pub struct PatientIdentity {
    pub account_id: i32,
}

fn create_session() { ... }

pub const MAX_RETRIES: u32 = 5;
```

### Serde Serialization

Use camelCase field names for protocol-facing JSON. Use `__type` for tagged enums that must stay compatible with jsoniter-scala conventions.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientIdentity {
    pub account_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional_field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum ConsultationStatus {
    UpComingWithoutDoctor,
    InProgress { doctor_id: i32 },
}
```

### Error Handling

Prefer typed errors and propagation with `?`. Avoid `unwrap()` in service code and shared libraries.

```rust
fn parse(input: &str) -> Result<PatientIdentity, ParseError> {
    let parsed = parse_identity(input)?;
    Ok(parsed)
}
```

### Time Handling with Jiff

For new date/time code, use `jiff`. Do not introduce new `chrono` usage unless a dependency boundary forces it. Existing `chrono` usage in shared Twilio JWT code is legacy and should not be copied into new modules.

Dependencies:

```toml
jiff = { version = "0.2", features = ["serde"] }
jiff-sqlx = { version = "0.1", features = ["postgres"] }
```

Examples:

```rust
use jiff::{Timestamp, ToSpan, civil::Date};

let now = Timestamp::now();
let ts = Timestamp::from_second(1_234_567_890)?;
let in_20_minutes = now.checked_add(20.minutes())?;
let future = now.saturating_add(30.days());
let formatted = Date::strptime("%Y-%m-%d", "2026-02-22")?
    .strftime("%Y%m%d")
    .to_string();

sqlx::query("INSERT INTO table (created_at) VALUES ($1)")
    .bind(jiff_sqlx::Timestamp::from(now))
    .execute(&pool)
    .await?;
```

Key rules:

- Use `jiff` for new date/time operations.
- Wrap timestamps with `jiff_sqlx::Timestamp::from(...)` before binding to SQLx.
- Handle fallible time operations with `?` or `map_err`; do not unwrap.
- Avoid adding `time` or `chrono` to new modules unless there is a documented boundary reason.

## Testing

Keep tests close to the package or feature they validate. Use root-level `it/` only for manual, cross-service, or external-service helpers.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patient_identity() {
        let identity = PatientIdentity { account_id: 1 };
        assert!(identity.account_id > 0);
    }
}
```

For integration tests that require Postgres, Redis, Google Cloud, Twilio, Pub/Sub, or external services, document required environment variables and external services in the test file or the owning package README.

## Protocol Conversion

Use these rules when converting between Scala/jsoniter-style protocol models and Rust:

- `case class` -> `struct` with `#[serde(rename_all = "camelCase")]`.
- `sealed trait` -> `enum` with `#[serde(tag = "__type", rename_all = "camelCase")]`.
- `case object` -> enum variant without data.
- `Option[String]` -> `Option<String>`.
- `Array[String]` -> `Vec<String>`.
- `Long` -> `i64`.
- `Int` -> `i32`.
- `java.net.URL` -> `url::Url`.

Compatibility notes:

- Enums with variants use `__type` as discriminator.
- Simple enums serialize to strings.
- Public API fields use camelCase JSON names.

## Repository Hygiene

## Appointment Hold terminology

Before changing appointment, booking, availability, or occupancy behavior, read root
`CONTEXT.md`. `appointment/hold/` is the canonical owner of Appointment Hold creation,
state, release, and expiry. Use Reservation terminology only at explicit legacy storage,
SQL-function, route, or V1 wire-event adapters. After cutover, `v2.reservation` is a
backfill-only input for reconciliation; it is never a runtime writer, reader, or rollout
source of truth.

- Prefer editing first-party service crates over generated output.
- Do not hand-edit generated OpenAPI JSON/YAML unless the task is explicitly about checked-in generated artifacts.
- Do not add local secrets or developer-local config. Prefer `.example` files for templates.
- Keep feature code organized by feature boundary: `handler`, `service`, `repo`, `model(s)`, and `mod.rs`.
- Avoid introducing new duplicate repository/model paths. In `consultation-rs`, check both `src/repo` and `src/common/repo` before adding shared persistence code.
- Ignore removed paths when gathering code context, even if stale references remain in docs or generated files.
- Do not restore removed packages unless the user explicitly asks for restoration.
- Preserve legacy deployment names only when required by existing deploy/release files, and document the mapping when adding new references.
