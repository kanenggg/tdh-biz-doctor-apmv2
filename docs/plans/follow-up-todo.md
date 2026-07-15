# Follow-Up Implementation Plan

## Team: 3 Developers (Dev A, Dev B, Dev C) + 1 QA

---

## Sprint 1: FHIR Alignment (~3-4 days)

### Day 0: Protocol Design Session (all devs + QA, ~2 hours)

All team members align on the contract before implementation starts.

- [ ] **T1.0** Protocol Design Review (all team)
  - Agree on `AppointmentStatus` FHIR values: `PROPOSED`, `PENDING`, `BOOKED`, `ARRIVED`, `FULFILLED`, `CANCELLED`, `NOSHOW`, `ENTERED_IN_ERROR`
  - Agree on `AppointmentType` HL7 v2-0276 values: `ROUTINE`, `WALK_IN`, `EMERGENCY`, `URGENT`
  - Agree on `SummaryNote.follow_up: Option<FollowUp>` field addition
  - Agree on DB enum values (screaming snake case)
  - QA takes notes on expected behavior for test writing

### Dependency Graph (parallel implementation after design session)

```
T1.0 Design Session (all)
     │
     ├── Dev A: Protocol types (T1.1-T1.5)
     ├── Dev B: DB migrations (T1.6-T1.9)
     ├── Dev C: Service layer (T1.10-T1.17) ← can draft code against agreed values, compile after Dev A done
     └── QA:   Write tests in parallel (T1.Q1-T1.Q6) ← compile after Dev A + Dev B done
```

---

### Dev A — Protocol Types (1-2 days, starts after T1.0)

- [ ] **T1.1** Update `tdh-protocol/rust/src/appointment/appointment_status.rs`
  - Replace bare enum with FHIR values + `Serialize`/`Deserialize` + `ToSchema`
  - Values: `PROPOSED`, `PENDING`, `BOOKED`, `ARRIVED`, `FULFILLED`, `CANCELLED`, `NOSHOW`, `ENTERED_IN_ERROR`
  - `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` for serialization
  - `#[serde(rename = "ENTERED_IN_ERROR")]` for the hyphenated FHIR value

- [ ] **T1.2** Create `tdh-protocol/rust/src/appointment/appointment_type.rs` (HL7 v2-0276)
  - Values: `ROUTINE`, `WALK_IN`, `EMERGENCY`, `URGENT`
  - `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` for serialization

- [ ] **T1.3** Update `tdh-protocol/rust/src/appointment/mod.rs`
  - Export new `AppointmentStatus` and `AppointmentType`

- [ ] **T1.4** Update `tdh-protocol/rust/src/biz_apm/summary_note.rs`
  - Add `follow_up: Option<FollowUp>` to `SubmitSummaryNoteRequest` (import from `biz_apm::follow_up`)
  - Prep for Sprint 2 — follow-up is part of summarization

- [ ] **T1.5** Update `tdh-protocol/rust/Cargo.toml`
  - Version bump to `0.4.0`
  - Publish or update git dependency reference

**Blockers**: None — starts immediately after T1.0.

---

### Dev B — Database Migrations (2-3 days, starts after T1.0)

- [ ] **T1.6** Create `db/biz_apm/migrations/YYYY__fhir_enums.sql`
  ```sql
  CREATE TYPE v2.fhir_appointment_status_enum AS ENUM (
      'PROPOSED', 'PENDING', 'BOOKED', 'ARRIVED',
      'FULFILLED', 'CANCELLED', 'NOSHOW', 'ENTERED_IN_ERROR'
  );

  CREATE TYPE v2.appointment_type_enum AS ENUM (
      'ROUTINE', 'WALK_IN', 'EMERGENCY', 'URGENT'
  );
  ```

- [ ] **T1.7** Create `db/biz_apm/migrations/YYYY__migrate_appointment_status.sql`
  - Add `appointment_type v2.appointment_type_enum` column to `v2.appointment`
  - Migrate existing data: `PENDING→PENDING`, `CONFIRMED→BOOKED`, `CONSULTATION_DONE→FULFILLED`, `CANCELLED→CANCELLED`
  - Replace `v2.appointment_status_enum` with FHIR enum on `v2.appointment.appointment_status`
  - Drop old enum type
  - **Recommended**: Single atomic migration since likely pre-production

- [ ] **T1.8** Create `db/biz_apm/migrations/YYYY__update_funcs_fhir.sql`
  - Update `v2.end_active_session()` — `CONSULTATION_DONE` → `FULFILLED`
  - Update `v2.cancel_appointment()` — verify `CANCELLED` value unchanged
  - Update `v2.get_consultation_session()` — status filters
  - Update `v2.upsert_payment_transaction()` — appointment creation status → `BOOKED`
  - Update `v2.create_confirmed_appointment()` — `CONFIRMED` → `BOOKED`
  - Update `v2.create_reservation()` — if it sets appointment status

- [ ] **T1.9** Update `db/biz_apm/seeds/appointment/001_seed_confirmed_appointment.sql`
  - Replace `CONFIRMED` → `BOOKED`
  - Replace any `CONSULTATION_DONE` → `FULFILLED`

**Blockers**: None — starts immediately after T1.0.

---

### Dev C — Service Layer (2-3 days, starts after T1.0)

Dev C can draft all changes using the agreed enum values from T1.0. Code will compile once Dev A publishes the protocol types.

- [ ] **T1.10** Update `consultation-rs/src/common/repo/enums.rs`
  - `AppointmentStatusEnum` → FHIR values: `Proposed`, `Pending`, `Booked`, `Arrived`, `Fulfilled`, `Cancelled`, `Noshow`, `EnteredInError`
  - Add `AppointmentTypeEnum`: `Routine`, `WalkIn`, `Emergency`, `Urgent`
  - sqlx type mapping to new PG enum types

- [ ] **T1.11** Update `consultation-rs/src/repo/models/enums.rs`
  - Same changes as T1.10 (currently duplicated)

- [ ] **T1.12** Update `consultation-rs/src/consultation/session_info/service.rs`
  - Status check: `AppointmentStatusEnum::Confirmed` → `AppointmentStatusEnum::Booked`
  - Review lines 112-147

- [ ] **T1.13** Update `consultation-rs/src/sys/config.rs`
  - `SessionConfig.required_appointment_status` default → `Booked`

- [ ] **T1.14** Update `consultation-rs/src/internal/repo.rs`
  - Update confirmed appointment creation status references

- [ ] **T1.15** Update `consultation-rs/src/consultation/end_session/service.rs`
  - Update status references if any

- [ ] **T1.16** Update `consultation-rs/src/consultation/facial_upload/`
  - Update status references if any

- [ ] **T1.17** Update `consultation-rs/src/protocol/summary_note.rs`
  - Add `follow_up: Option<FollowUp>` to `SummaryNote` struct
  - Import `FollowUp` from `tdh_protocol::biz_apm::follow_up`
  - Prep for Sprint 2

**Blockers**: Compile depends on T1.1-T1.3 (Dev A). Can draft code immediately after T1.0 using agreed values.

---

### QA — Sprint 1 Tests (write in parallel with devs, run after Dev A + Dev B complete)

QA writes tests based on the agreed contract from T1.0. Tests can be compiled and run once Dev A (protocol types) and Dev B (DB migrations) are done.

- [ ] **T1.Q1** Write integration test: appointment status lifecycle
  - Create reservation → confirm → end session → verify status is `FULFILLED`
  - Write test on Day 1, run after T1.1-T1.3 + T1.6-T1.8 complete

- [ ] **T1.Q2** Write integration test: cancel appointment
  - Cancel at various stages → verify `CANCELLED`

- [ ] **T1.Q3** Write integration test: seed data loads correctly with new FHIR status values
  - Load seed → query DB → assert status values match FHIR enum

- [ ] **T1.Q4** Run existing integration test suite — verify no regressions

- [ ] **T1.Q5** Write test: PG function returns correct FHIR status strings
  - Call `v2.get_consultation_session()` → assert status in result matches FHIR values
  - Call `v2.end_active_session()` → assert status becomes `FULFILLED`
  - Call `v2.cancel_appointment()` → assert status becomes `CANCELLED`

- [ ] **T1.Q6** Full build verification
  - `cargo check --manifest-path consultation-rs/Cargo.toml`
  - `cargo test --manifest-path consultation-rs/Cargo.toml`
  - `cargo fmt --manifest-path consultation-rs/Cargo.toml --check`
  - `cargo clippy --manifest-path consultation-rs/Cargo.toml`

**Timeline**: Start writing tests on Day 1 (after T1.0). Run tests when Dev A + Dev B complete (~Day 2-3).

---

## Sprint 2: Follow-Up Logic (~4-5 days)

### Day 0: Follow-Up Design Session (all devs + QA, ~2 hours)

- [ ] **T2.0** Follow-Up Design Review (all team)
  - Review `FollowUp` enum (`AsNeeded` | `Appointment(FollowUpAppointment)`)
  - Review `FollowUpRequiredEvent` fields
  - Agree on follow-up creation as part of summarization service
  - Agree on DB function signatures: `create_follow_up_appointment()`, `mark_appointment_has_follow_up()`, `get_appointment_chain()`
  - Agree on error handling: `ParentNotFulfilled`, `FollowUpCreationFailed`
  - QA takes notes on expected behavior for test writing

### Design Decision: Follow-Up is Part of Summarization

Follow-up is created when the doctor submits a consultation summary note — not a separate endpoint.

**Flow:**
```
Doctor submits summary note with optional follow_up field
  → POST /e2e/v1/summary-note (existing endpoint)
  → SummarizationService::add_summary_note()
    1. Encrypt + insert summary note (existing logic)
    2. If follow_up is present:
       a. FollowUp::AsNeeded → set has_follow_up = true only
       b. FollowUp::Appointment(data) → create follow-up reservation + appointment
          + publish FollowUpRequired event
    3. Return SummarizationResult
```

### Dependency Graph (parallel implementation after design session)

```
T2.0 Design Session (all)
     │
     ├── Dev A: DB functions (T2.1-T2.5)
     ├── Dev B: Repo + service (T2.6-T2.10) ← draft against agreed signatures, compile after Dev A
     ├── Dev C: Wiring + edge cases (T2.11-T2.15) ← draft in parallel, compile after Dev B
     └── QA:   Write tests in parallel (T2.Q1-T2.Q10) ← run after all devs done
```

---

### Dev A — Follow-Up DB Functions (2-3 days, starts after T2.0)

- [ ] **T2.1** Create `db/biz_apm/migrations/YYYY__follow_up_funcs.sql`
  - `v2.mark_appointment_has_follow_up(p_appointment_id)`
    - Validates parent exists and status is `FULFILLED`
    - Sets `has_follow_up = true` on parent appointment
    - Returns boolean (success/failure)

- [ ] **T2.2** Same migration: `v2.create_follow_up_appointment(...)`
  - Parameters: `p_parent_appointment_id`, `p_appointment_start`, `p_consult_duration`, `p_appointment_type`
  - Validates parent appointment is `FULFILLED`
  - Validates parent does not already have a follow-up (or allows multiple — confirm at T2.0)
  - Sets `has_follow_up = true` on parent
  - Creates reservation with `booking_type = 'FollowUp'`, same doctor + patient as parent
  - Creates appointment with `parent_appointment_id` set
  - Returns JSONB: `{ booking_id, appointment_id, appointment_start, appointment_end }`

- [ ] **T2.3** Same migration: `v2.get_appointment_chain(p_appointment_id)`
  - Returns parent + all follow-up appointments in chronological order as JSONB array

- [ ] **T2.4** Create `db/biz_apm/migrations/YYYY__update_summary_note_upsert.sql`
  - Update `v2.create_if_not_existing_summary_note()`
  - Change `ON CONFLICT DO NOTHING` to allow updates when appointment has `parent_appointment_id IS NOT NULL`
  - Regular appointments (no parent): insert-only
  - Follow-up appointments: upsert allowed

- [ ] **T2.5** Create follow-up seed data
  - New file: `db/biz_apm/seeds/appointment/002_seed_follow_up_appointment.sql`
  - Parent appointment (`FULFILLED`) + follow-up appointment linked via `parent_appointment_id`

**Blockers**: Depends on Sprint 1 FHIR enums (T1.6-T1.8).

---

### Dev B — Follow-Up Repo + Service Integration (2-3 days, starts after T2.0)

Dev B can draft the repo trait and service logic against the agreed function signatures from T2.0. Code will compile once Dev A's DB functions are ready.

- [ ] **T2.6** Create `consultation-rs/src/summarization/follow_up_repo.rs`
  - `FollowUpRepo` trait:
    - `create_follow_up(parent_appointment_id, appointment_start, consult_duration, appointment_type) -> Result<FollowUpCreationResult>`
    - `mark_has_follow_up(appointment_id) -> Result<bool>`
    - `get_appointment_chain(appointment_id) -> Result<Vec<AppointmentChainItem>>`
  - `FollowUpRepoPsql` impl — calls PG functions from T2.1-T2.3
  - `FollowUpCreationResult` and `AppointmentChainItem` structs

- [ ] **T2.7** Update `consultation-rs/src/summarization/mod.rs`
  - Add `pub mod follow_up_repo;`

- [ ] **T2.8** Update `consultation-rs/src/summarization/service.rs`
  - Add `Arc<FollowUpRepoPsql>` field to `SummaryNoteService`
  - Update constructor
  - After summary note insert, check `summary_note.follow_up`:
    - `None` → no action
    - `Some(FollowUp::AsNeeded)` → call `follow_up_repo.mark_has_follow_up(booking_id)`
    - `Some(FollowUp::Appointment(data))` → call `follow_up_repo.create_follow_up(...)` with data from `FollowUpAppointment`, then publish `ConsultationEvent::FollowUpRequired(FollowUpRequiredEvent)` via event publisher
  - Add error variants: `FollowUpCreationFailed`, `ParentNotFulfilled`

- [ ] **T2.9** Update `consultation-rs/src/summarization/state.rs`
  - Add `FollowUpRepoPsql` to AppState wiring

- [ ] **T2.10** Update `consultation-rs/src/summarization/model.rs`
  - Add `SummarizationError::FollowUpCreationFailed(String)`
  - Add `SummarizationError::ParentNotFulfilled`

**Blockers**: Compile depends on T2.1-T2.2 (Dev A's DB functions). Can draft code immediately after T2.0 using agreed signatures.

---

### Dev C — Wiring + Edge Cases (1-2 days, starts after T2.0)

Dev C can draft wiring changes in parallel. Final compile depends on Dev B's repo and service changes.

- [ ] **T2.11** Update `consultation-rs/src/main.rs`
  - Bootstrap `FollowUpRepoPsql::new(pool)`
  - Inject into `SummaryNoteService` constructor
  - Ensure `EventPublisher` is also injected into `SummaryNoteService`

- [ ] **T2.12** Update `consultation-rs/src/openapi/mod.rs`
  - Update OpenAPI schemas for `SummaryNote` with follow-up fields
  - Verify `FollowUp`, `FollowUpAppointment`, `VisitType` schemas are registered

- [ ] **T2.13** Update `consultation-rs/src/common/infrastructure.rs`
  - Ensure `FollowUpRepoPsql` can be constructed from `Infrastructure` (if needed)

- [ ] **T2.14** Update `consultation-rs/config/default.toml`
  - Add follow-up config section if needed:
    ```toml
    [follow_up]
    max_days_ahead = 30
    ```

- [ ] **T2.15** Verify event publishing
  - Confirm `FollowUpRequiredEvent` is published with correct fields:
    - `previous_booking_id`, `follow_up_id`, `patient_identity`, `doctor_id`
    - `biz_unit_id`, `consultation_start_time`, `consultation_duration_in_second`
    - `consultation_fee`, `consultation_channel`, `additional_patient_note`, `internal_note`
  - Test event structure matches `biz_apm/consultation_event.rs:FollowUpRequiredEvent`

**Blockers**: Compile depends on T2.6-T2.8 (Dev B). Can draft wiring immediately after T2.0.

---

### QA — Sprint 2 Tests (write in parallel with devs, run after all devs complete)

QA writes tests based on the agreed contract from T2.0. Tests can be compiled and run once all dev tasks are done.

- [ ] **T2.Q1** Write integration test: submit summary note with `FollowUp::AsNeeded`
  - Verify `has_follow_up = true` on parent appointment
  - Verify NO new reservation/appointment created
  - Verify summary note inserted successfully

- [ ] **T2.Q2** Write integration test: submit summary note with `FollowUp::Appointment(data)`
  - Verify new reservation created with `booking_type = FollowUp`
  - Verify new appointment created with `parent_appointment_id` set
  - Verify same doctor + patient as parent
  - Verify `has_follow_up = true` on parent
  - Verify summary note inserted successfully

- [ ] **T2.Q3** Write integration test: submit summary note without follow-up
  - Verify `has_follow_up = false`, no side effects
  - Verify summary note inserted successfully

- [ ] **T2.Q4** Write integration test: `FollowUpRequired` event published
  - Verify event has correct `__type: "FollowUpRequired"`
  - Verify all fields populated: `previous_booking_id`, `follow_up_id`, `patient_identity`, etc.

- [ ] **T2.Q5** Write integration test: summary note upsert on follow-up
  - Submit summary note for follow-up appointment → insert succeeds
  - Submit again → update succeeds (not `AlreadyExists`)
  - Submit summary note for regular appointment twice → second returns `AlreadyExists`

- [ ] **T2.Q6** Write integration test: reject follow-up on non-`FULFILLED` parent
  - Parent in `BOOKED` status → submit summary note with follow-up → expect `ParentNotFulfilled` error

- [ ] **T2.Q7** Write integration test: `get_appointment_chain()`
  - Create parent + 2 follow-ups → verify chain returns 3 items in chronological order
  - Verify parent has `parent_appointment_id = NULL`

- [ ] **T2.Q8** Write edge case test: submit follow-up twice on same parent
  - First → `Success`
  - Second → expect error or `AlreadySubmitted`

- [ ] **T2.Q9** Verify OpenAPI spec generation
  - `just openapi consultation-rs`
  - Verify follow-up fields appear in `SummaryNote` schema

- [ ] **T2.Q10** Full build verification
  - `cargo check --manifest-path consultation-rs/Cargo.toml`
  - `cargo test --manifest-path consultation-rs/Cargo.toml`
  - `cargo fmt --manifest-path consultation-rs/Cargo.toml --check`
  - `cargo clippy --manifest-path consultation-rs/Cargo.toml`

**Timeline**: Start writing tests on Day 1 (after T2.0). Run tests when all devs complete (~Day 3-4).

---

## Timeline Summary

```
Sprint 1 (FHIR Alignment)
───────────────────────────────────────────────────
Day 0:  T1.0 Design Session (all, ~2h)
Day 1:  Dev A: T1.1-T1.3 | Dev B: T1.6-T1.7 | Dev C: draft T1.10-T1.17 | QA: write T1.Q1-T1.Q6
Day 2:  Dev A: T1.4-T1.5 | Dev B: T1.8-T1.9 | Dev C: finalize + compile | QA: continue writing tests
Day 3:  Dev C: compile + fix | QA: run all tests T1.Q1-T1.Q6 | Sprint 1 done ✓

Sprint 2 (Follow-Up Logic)
───────────────────────────────────────────────────
Day 4:  T2.0 Design Session (all, ~2h)
Day 5:  Dev A: T2.1-T2.3 | Dev B: draft T2.6-T2.8 | Dev C: draft T2.11-T2.14 | QA: write T2.Q1-T2.Q10
Day 6:  Dev A: T2.4-T2.5 | Dev B: finalize + compile | Dev C: finalize | QA: continue writing tests
Day 7:  All devs compile + fix | QA: run all tests T2.Q1-T2.Q10 | Sprint 2 done ✓
Day 8:  Buffer / polish / code review
```

---

## Handoff Checklist

### Sprint 1 → Sprint 2

- [ ] `cargo check` passes with FHIR enums
- [ ] All PG functions updated with FHIR status values
- [ ] Seed data loads successfully
- [ ] Integration tests pass
- [ ] Dev A kicks off Sprint 2 DB functions immediately after T2.0

### Final Delivery

- [ ] All Sprint 2 integration tests pass
- [ ] No clippy warnings
- [ ] OpenAPI spec generated and reviewed
- [ ] Seed data includes follow-up scenario

---

## Existing Scaffolding Already in Place

| Component | Location |
|---|---|
| `booking_type_enum` has `FollowUp` variant | `20260220000002__init_types.sql:8` |
| `appointment.parent_appointment_id` column | `20260222000002__appointment.sql:12` |
| `appointment.has_follow_up` flag (always false) | `20260222000002__appointment.sql:20` |
| Index on `parent_appointment_id` | `20260222000002__appointment.sql:41` |
| `FollowUp`, `FollowUpAppointment`, `VisitType`, `SubmitFollowUpResult` types | `tdh-protocol/biz_apm/follow_up.rs` |
| `FollowUpRequired`, `FollowUpRequestExpired`, `PatientAcceptedFollowUp`, `FollowUpCancelled` events | `tdh-protocol/biz_apm/consultation_event.rs` |

## Risk Areas

- **Three different `AppointmentStatus` types** in tdh-protocol — all need alignment
  - `appointment::appointment_status` — bare enum, no derives
  - `biz_apm::appointment::AppointmentStatus` — ComingUp/Ongoing/Completed/Missed/Record
  - `appointment_prost.rs` — prost-generated (protobuf tied)
- **Duplicate enum files** in consultation-rs (`common/repo/enums.rs` and `repo/models/enums.rs`) — must stay in sync
- **Summary note ON CONFLICT DO NOTHING** — only follow-ups should update, regular appointments remain insert-only
- **Follow-up always same doctor** — enforce in service layer, inherited from parent appointment
