# APM DoctorApp Contract-First Parallel Execution Plan

**Status:** implementation freeze / conservative ops mitigation active
**Last updated:** 2026-07-09
**Owner:** APM + DoctorApp phase leads
**Scope:** docs-only planning artifact; do not change runtime/source/ops files while this plan is being reviewed.

## 0. Evidence baseline

- [V] Current APM repo path: `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-biz-doctor-apmv2`.
- [V] Local sibling DoctorApp repo exists at `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-mordee-doctor-app`.
- [V] Local sibling ops repos were not present during planning: `../tdh-mordee-doctor-app-ops`, `../tdh-biz-apm-ops`.
- [V] `git status --short` is dirty; agents must verify tracked and untracked files before merge.
- [V] Existing decision docs: `docs/plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md`, `docs/plans/PATIENT_VERIFICATION_MISMATCH.md`.
- [V] Conservative ops mitigation is the current rollout posture: DoctorApp Cloud Run server manifests for dev, staging, and prod set `SERVICE__CONSULTATION_BG_BASE_URI=""`, so direct projection sync is disabled and Pub/Sub fallback remains active.
- [V] Doctor projection routes are in `consultation-bg-rs/src/main.rs` via `doctor_identity::handler::routes`: preferred `POST /internal/v1/pubsub/doctor-profile` and `POST /internal/v1/doctor-projection/sync`; legacy aliases `POST /pubsub/doctor-profile` and `POST /internal/doctor-projection/sync` remain active.
- [V] Booking routes are in `consultation-rs/src/booking/handler.rs`: preferred public `/v1/booking` and service-to-service `/internal/v1/booking` routes, with legacy `/v2/booking` and `/v2/internal/booking` aliases for existing callers.
- [V] Provider callback route is in `consultation-rs/src/provider_callback/handler.rs`: `POST /twilio/callback`.
- [V] Session routes are `/v2/consultation/session-info/{booking_id}` and `/v2/consultation/end-session/{booking_id}`.
- [V] Outbox and payment modules exist in `consultation-bg-rs/src/event_outbox` and `consultation-bg-rs/src/payment_confirm`.

## 1. Implementation freeze

- [V] This plan freezes incremental runtime implementation until the contracts below are accepted.
- [A] Runtime/source/ops files must not be changed by the contract agent unless a task card explicitly authorizes that agent.
- [V] Docs-only changes are allowed under `docs/plans/`.
- [V] This docs update must not reopen implementation or ops work; direct projection sync remains disabled in DoctorApp Cloud Run dev/staging/prod until APM Cloud Run allowlist/auth is implemented and verified.
- [V] Untracked phase artifacts already exist in this worktree; each build agent must run `git status --short` and list owned files before coding and before handoff.
- [A] Merge order is contract audit first, agent outputs second; no agent should rediscover broad scope mid-implementation.

## 2. Phase contract outlines

### 2.1 Doctor projection sync contract

- [V] Route convention is access-before-version: service-to-service routes use `/internal/vN/...`, admin/backoffice routes use `/admin/vN/...`, and public routes use `/vN/...` with no `/public` prefix. Legacy version-before-access aliases such as `/v2/internal/...` may remain during migration but must not be the preferred form for new routes.
- [V] APM endpoint: preferred `POST /internal/v1/doctor-projection/sync` in `consultation-bg-rs`; legacy `POST /internal/doctor-projection/sync` remains a compatibility alias.
- [V] Fallback endpoint: preferred `POST /internal/v1/pubsub/doctor-profile`; legacy `POST /pubsub/doctor-profile` remains a compatibility alias. Pub/Sub fallback must remain enabled.
- [V] Current auth mode: Google OAuth bearer token validated by tokeninfo URL from `[doctor_projection_sync].tokeninfo_url`.
- [V] Allowlist: `[doctor_projection_sync].allowed_service_account_emails`; empty allowlist disables direct sync with `503`.
- [U] Cloud Run OIDC vs OAuth tokeninfo is a rollout decision blocker. Do not switch auth mode without ops approval and a contract update.
- [V] Payload is `DoctorProfileEvent` with `__type` discriminator: `DoctorProfileApproved` or `DoctorProfileDeactivated`.
- [V] Approved payload fields: `eventId`, `doctorId`, `doctorAccountId`, `doctorProfileId`, `isActive`.
- [V] Deactivated payload fields: `eventId`, `doctorId`, `doctorAccountId`, `doctorProfileId`.
- [V] `DoctorProfileDeactivated` is payload-model supported for contract compatibility.
- [A] Deactivation rollout implementation is out of scope unless explicitly accepted in a follow-up contract; this includes DoctorApp emission, APM projection state rollout beyond payload handling, ops enablement, and release validation.
- [V] Failure semantics: `401` invalid/missing bearer, `403` email not allowlisted, `503` disabled, `500` token validation/database failure, `200` success.
- [A] DoctorApp direct sync remains best effort; approval must continue and Pub/Sub fallback must preserve eventual projection.
- [V] DoctorApp Cloud Run dev/staging/prod currently keep `SERVICE__CONSULTATION_BG_BASE_URI=""`; direct projection sync is disabled before any HTTP call in those environments.
- [V] Current rollout remains blocked until APM Cloud Run allowlist/auth is implemented and verified; do not imply P0 direct sync is deployed/enabled.
- [U] Doctor snapshot completeness is a blocker: confirm whether APM only needs identity IDs or a richer doctor snapshot.

### 2.2 Booking/timeslot contract

- [V] Public reserve endpoint: preferred `POST /v1/booking` (booking/Appointment Hold); legacy `POST /v2/booking` remains accepted. Internal reserve endpoint: preferred `POST /internal/v1/booking`; legacy `POST /v2/internal/booking` remains accepted.
- [V] Legacy appointment reserve endpoints (`POST /v2/appointment/reserve`, `POST /v1/appointment/reserve`) remain compatibility code only and are not mounted or advertised by active appointment routing/OpenAPI.
- [V] State endpoints: preferred `GET /v1/booking/{booking_id}/state` and `GET /internal/v1/booking/{booking_id}/state`; legacy `GET /v2/booking/{booking_id}/state` and `GET /v2/internal/booking/{booking_id}/state` remain accepted.
- [V] Cancel endpoint: preferred `POST /internal/v1/booking/{booking_id}/cancel`; legacy `POST /v2/internal/booking/{booking_id}/cancel` remains accepted. No public booking cancel endpoint is mounted or advertised.
- [V] Reserve returns `Reserved`, `DoctorNotAvailable`, or `SlotAlreadyBooked` in `BookingResponse.status`.
- [V] Cancel accepts reservation status `RESERVED` or `CANCELLED` with appointment status `None`, `PENDING`, or `CANCELLED`.
- [V] Cancel rejects active/booked/done states with `409` through `BookingError::CannotCancel`.
- [V] Lifecycle mapping includes `Reserved`, `ReserveExpired`, `Booked`, `ConsultationDone`, `Cancelled`, `Unknown`.
- [V] Timeslot validation requires `end > start`, positive duration, and `duration == end - start`.
- [V] Booking conflicts use doctor occupancy time windows, not slot-id equality; see `docs/adr/0003-booking-conflicts-use-doctor-occupancy-window.md`.
- [A] Cancel is idempotent only when the persisted row is already cancelled and still passes state guards; tests must lock this behavior.
- [U] Patient mismatch cancellation is a blocker; current mismatch endpoint records verification only and must not drive cancel until product confirms.

### 2.3 Session/provider callback contract

- [V] Twilio callback endpoint: `POST /twilio/callback`, form encoded into `TwilioStatusCallback`.
- [V] Twilio accepted fields include `StatusCallbackEvent`, `RoomName`, `RoomSid`, `ParticipantIdentity`, `ParticipantStatus`, `Timestamp`, `SequenceNumber` plus camelCase aliases.
- [V] Only participant disconnect is acted on: event `participant-disconnected` or participant status `disconnected`.
- [V] Room name format must start with `mordee_twilio_video_`; suffix is appointment/booking ID.
- [V] Participant identity must start with `patient_` or `doctor_`.
- [V] Provider callback idempotency uses `(provider, provider_event_id)` conflict handling in `v2.provider_callback_event`.
- [V] Disconnect idempotency updates only when `patient_disconnected_at` or `doctor_disconnected_at` is null.
- [V] Emitted events are `SessionMessage::PatientDisconnected` or `SessionMessage::DoctorDisconnected`.
- [V] End-session termination request supports `SuccessfulSession`, `PatientAbsent`, `DoctorAbsent`, `BothPartiesAbsent`, `TechnicalError`, `PatientVerificationMismatch`.
- [A] Provider callback handler returns `200 OK` even on unsupported or internal callback errors to avoid Twilio retry storms; audit logs must carry error detail.

### 2.4 Event/outbox contract

- [V] Payment Pub/Sub endpoint: `POST /pubsub/payment-confirm`.
- [V] Payment success can enqueue `ConsultationBooked` to `v2.event_outbox`.
- [V] Outbox claim selects `PENDING` or expired `PROCESSING`, sets `PROCESSING`, uses `FOR UPDATE SKIP LOCKED`, and locks rows for `[event_outbox_worker].lock_seconds`.
- [V] Publish success marks `PUBLISHED`; publish failure marks `PENDING`, increments `retry_count`, clears lock, and stores `last_error`.
- [V] Payment enqueue SQL uses an advisory transaction lock and returns an existing `ConsultationBooked` event for the same aggregate/event type.
- [A] Direct publish plus worker semantics must be explicit per event type: reservation/session events may publish directly from `consultation-rs`; payment confirmation must persist to outbox before publish.
- [U] `event-v2` compatibility is a blocker: confirm whether current consumers require legacy `common-rs/src/tdh_protocol/biz_apm/consultation_event.rs`, new `common-rs/src/event/biz_apm/consultation_event/v2.rs`, or dual publishing.
- [A] Idempotency guarantee is at-least-once delivery with duplicate suppression by event ID or aggregate/event type where implemented; consumers must remain idempotent.

### 2.5 Rollout contract

- [V] Direct doctor projection can be disabled by empty DoctorApp `consultation_bg_base_uri` or empty APM allowlist.
- [V] DoctorApp Cloud Run dev/staging/prod currently use the DoctorApp disable switch: `SERVICE__CONSULTATION_BG_BASE_URI=""`.
- [V] Pub/Sub fallback remains active while direct projection sync is disabled.
- [V] Event outbox worker config exists with defaults: enabled `true`, poll interval `5s`, batch size `100`, lock seconds `60`.
- [A] Feature gates should be operational config, not compile-time flags.
- [A] Dev direct-sync rollout remains blocked; first unblock APM Cloud Run allowlist/auth, then enable logging and synthetic test approvals/bookings.
- [A] Staging rollout must prove Pub/Sub fallback, direct sync, payment confirm outbox, callbacks, and booking cancel before production, but direct sync must stay disabled until the APM allowlist/auth gate passes.
- [A] Production rollout must include rollback switches for direct sync, outbox worker, and any event-v2 publisher/consumer toggles.


### 2.6 Phase status matrix

| Phase | Status | Blocker | Owner agent | Validation commands |
| --- | --- | --- | --- | --- |
| Doctor projection sync | [U] Blocked for Cloud Run enablement; DoctorApp dev/staging/prod direct sync disabled by `SERVICE__CONSULTATION_BG_BASE_URI=""` | APM Cloud Run allowlist/auth; doctor snapshot completeness; deactivation rollout explicitly out of scope unless accepted | Agent D, with Agent E for ops enablement | `cargo test -p consultation-bg-rs doctor_identity:: -- --nocapture`<br>DoctorApp server: `cargo test -q projection_sync` |
| Booking/timeslot | [A] Accepted for bounded implementation and validation | Patient mismatch must remain record-only until product/API decision; no event-v2 schema changes | Agent A | `cargo test -p consultation-rs booking:: -- --nocapture`<br>`cargo test -p consultation-rs doctor_timeslot:: -- --nocapture`<br>`cargo check -p consultation-rs` |
| Session/provider callbacks | [A] Accepted for bounded implementation and validation | Patient mismatch cancellation behavior is blocked; unsupported callback retry behavior must stay `200 OK` | Agent B | `cargo test -p consultation-rs provider_callback:: -- --nocapture`<br>`cargo test -p consultation-rs consultation::session_info:: -- --nocapture`<br>`cargo test -p consultation-rs consultation::end_session:: -- --nocapture`<br>`cargo check -p consultation-rs` |
| Payment/outbox/events | [U] Accepted for outbox/payment hardening; blocked for schema changes | Event-v2 compatibility decision before event schema/topic/dual-publish changes | Agent C | `cargo test -p consultation-bg-rs payment_confirm:: -- --nocapture`<br>`cargo test -p consultation-bg-rs event_outbox:: -- --nocapture`<br>`cargo test -p common-rs consultation_event -- --nocapture`<br>Optional DB: `TEST_DATABASE_URL='postgres://...' cargo test -p consultation-bg-rs payment_confirm::repo::tests::enqueue_consultation_booked_event_is_idempotent_under_concurrent_calls -- --ignored --exact --nocapture` |
| Rollout/ops | [U] Conservative mitigation active; direct sync disabled in DoctorApp Cloud Run dev/staging/prod | APM Cloud Run allowlist/auth; env source for allowlist and rollback toggles | Agent E | Repo-specific plan/apply dry-run commands<br>`grep -R "CONSULTATION_BG_BASE_URI\|allowed_service_account_emails\|event_outbox" .` in ops repos |
| Contracts/docs/final validation | [A] Active final gate | Dirty worktree attribution; all blockers must be resolved or explicitly release-blocking | Agent F | `grep -nE '^### 2\.6 Phase status matrix$' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`<br>`grep -nE '^- \*\*Cloud Run auth:\*\* \[U\]' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`<br>`grep -nE '^## 5\. Final merge checklist$' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`<br>All P0 agent commands above |

## 3. Parallel agent task cards

### Agent A — APM core booking/timeslot

- **Priority:** P0.
- **Repos in scope:** `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-biz-doctor-apmv2`.
- **Files in scope:** `consultation-rs/src/booking/**`, `consultation-rs/src/doctor_timeslot/**`, `consultation-rs/src/appointment/reserve_timeslot/**` compatibility shims, `db/biz_apm/migrations/*booking*`, `db/biz_apm/migrations/*timeslot*`.
- **Files out of scope:** `consultation-bg-rs/**`, `consultation-rs/src/provider_callback/**`, `consultation-rs/src/consultation/**`, ops repos, DoctorApp repo.
- **Acceptance:** reserve status mapping locked; invalid slot validation covered; cancel state/idempotency covered; conflict semantics align with ADR 0003/0004/0005; no event-v2 schema changes.
- **Validation commands:** `cargo test -p consultation-rs booking:: -- --nocapture`; `cargo test -p consultation-rs doctor_timeslot:: -- --nocapture`; `cargo check -p consultation-rs`.
- **Do not change:** doctor projection sync, payment outbox, ops manifests, generated OpenAPI unless explicitly assigned.

### Agent B — APM session/provider callbacks

- **Priority:** P0.
- **Repos in scope:** `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-biz-doctor-apmv2`.
- **Files in scope:** `consultation-rs/src/provider_callback/**`, `consultation-rs/src/consultation/session_info/**`, `consultation-rs/src/consultation/end_session/**`, `common-rs/src/twilio/callback.rs`, `db/biz_apm/migrations/*provider_callback*`, `db/biz_apm/migrations/*session_info*`.
- **Files out of scope:** booking reservation implementation, payment confirm/outbox, DoctorApp, ops repos.
- **Acceptance:** duplicate Twilio callbacks are no-ops; first disconnect per participant emits one event; unsupported callbacks return `200`; termination code mapping includes `PatientVerificationMismatch`; patient mismatch endpoint behavior remains unchanged unless product decides.
- **Validation commands:** `cargo test -p consultation-rs provider_callback:: -- --nocapture`; `cargo test -p consultation-rs consultation::session_info:: -- --nocapture`; `cargo test -p consultation-rs consultation::end_session:: -- --nocapture`; `cargo check -p consultation-rs`.
- **Do not change:** payment event models, booking cancel semantics, DoctorApp direct sync.

### Agent C — APM payment/outbox/events

- **Priority:** P0.
- **Repos in scope:** `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-biz-doctor-apmv2`.
- **Files in scope:** `consultation-bg-rs/src/payment_confirm/**`, `consultation-bg-rs/src/event_outbox/**`, `consultation-bg-rs/src/event/**`, `consultation-rs/src/infra/event/**`, `common-rs/src/tdh_protocol/biz_apm/consultation_event.rs`, `common-rs/src/event/biz_apm/consultation_event/**`, `db/biz_apm/migrations/*outbox*`, `db/biz_apm/migrations/*payment_confirm*`.
- **Files out of scope:** DoctorApp onboarding, ops manifests, booking UI/API behavior, provider callback route.
- **Acceptance:** outbox race/idempotency tests pass; payment confirm enqueues only one `ConsultationBooked` per booking/event type; worker retry semantics covered; event compatibility decision documented before changing schemas.
- **Validation commands:** `cargo test -p consultation-bg-rs payment_confirm:: -- --nocapture`; `cargo test -p consultation-bg-rs event_outbox:: -- --nocapture`; `cargo test -p common-rs consultation_event -- --nocapture`; optional disposable DB: `TEST_DATABASE_URL='postgres://...' cargo test -p consultation-bg-rs payment_confirm::repo::tests::enqueue_consultation_booked_event_is_idempotent_under_concurrent_calls -- --ignored --exact --nocapture`.
- **Do not change:** DoctorApp sync client, Cloud Run auth, ops config.

### Agent D — Doctor projection sync

- **Priority:** P0.
- **Repos in scope:** `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-biz-doctor-apmv2`, `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-mordee-doctor-app`.
- **Files in scope:** `consultation-bg-rs/src/doctor_identity/**`, `consultation-bg-rs/src/sys/config.rs`, `consultation-bg-rs/src/main.rs`, `docs/plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md`, DoctorApp `server/src/module/backoffice/onboarding/**`.
- **Files out of scope:** APM booking/payment/provider callback modules, ops repos, unrelated DoctorApp modules.
- **Acceptance:** direct sync posts approved doctor events; auth tests cover disabled/missing/invalid/forbidden/tokeninfo failure; DoctorApp direct sync is best effort; Pub/Sub fallback remains preserved; `DoctorProfileDeactivated` payload compatibility remains supported; deactivation rollout implementation remains out of scope unless explicitly accepted.
- **Validation commands:** in APM repo, `cargo test -p consultation-bg-rs doctor_identity:: -- --nocapture`; in DoctorApp server, `cargo test -q projection_sync`.
- **Do not change:** switch to Cloud Run OIDC, ops IAM, richer doctor snapshot, or deactivation rollout flow without explicit contract approval.

### Agent E — Ops

- **Priority:** P1 after Agent D contract approval.
- **Repos in scope:** `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-mordee-doctor-app-ops` and `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-biz-apm-ops` when checked out.
- **Files in scope:** DoctorApp service env for `SERVICE__CONSULTATION_BG_BASE_URI`; APM consultation-bg env/config for `[doctor_projection_sync].allowed_service_account_emails`, `[doctor_projection_sync].tokeninfo_url`, outbox worker config, IAM/deployment docs.
- **Files out of scope:** app source code in APM or DoctorApp repos.
- **Acceptance:** dev/staging/prod env matrix states `SERVICE__CONSULTATION_BG_BASE_URI=""` until APM Cloud Run allowlist/auth is implemented and verified; Pub/Sub fallback remains active; allowlist value source is documented before enablement; rollback toggles documented; no app code changes.
- **Validation commands:** repo-specific plan/apply dry-run commands plus `grep -R "CONSULTATION_BG_BASE_URI\|allowed_service_account_emails\|event_outbox" .` in ops repos.
- **Do not change:** Rust/Scala/TS app source; generated API artifacts.

### Agent F — Contracts/docs/final validation

- **Priority:** P0 and final gate.
- **Repos in scope:** `/Users/anucha.mai/Workspace/doctorapp-verse/tdh-biz-doctor-apmv2` docs, plus validation orchestration across DoctorApp and ops repos.
- **Files in scope:** `docs/plans/**`, `docs/adr/**` if a new ADR is required, release checklist docs.
- **Files out of scope:** runtime/source/ops code.
- **Acceptance:** each contract has an accepted/blocked status; all agent outputs are audited against this document; dirty/untracked file list is attached to merge review; validation matrix is complete; blockers are either resolved or release-blocking.
- **Validation commands:** `grep -nE '^### 2\.1 Doctor projection sync contract$' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`; `grep -nE '^### 2\.6 Phase status matrix$' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`; `grep -nE '^- \*\*Cloud Run auth:\*\* \[U\]' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`; `grep -nE '^- \*\*Patient mismatch:\*\* \[U\]' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`; `grep -nE '^- \*\*Doctor snapshot:\*\* \[U\]' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`; `grep -nE '^- \*\*Event-v2:\*\* \[U\]' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`; `grep -nE '^## 5\. Final merge checklist$' docs/plans/APM_DOCTORAPP_CONTRACT_FIRST_PARALLEL_PLAN.md`; plus all agent commands above.
- **Do not change:** any non-doc files.

## 4. Decisions and blockers

- **Cloud Run auth:** [U] Decide whether to keep OAuth tokeninfo or migrate to Cloud Run OIDC/IAM. Current code is tokeninfo. DoctorApp Cloud Run direct projection sync remains disabled in dev/staging/prod and all enablement is blocked until APM Cloud Run allowlist/auth is implemented and verified.
- **Patient mismatch:** [U] Decide whether mismatch endpoint only records verification or cancels appointment/session. Current safe behavior is record-only. Block any cancellation change.
- **Doctor snapshot:** [U] Decide whether APM projection needs only identity IDs or richer doctor profile fields. Current projection stores IDs/activity/source event only.
- **Event-v2:** [U] Decide legacy vs v2 event schema, topic, attributes, and dual-publish/consumer migration. Block schema changes until consumers confirm.
- **Dirty worktree:** [V] Current worktree has tracked and untracked changes. Block merge if files are not attributed to task cards.

## 5. Final merge checklist

1. [P0] [V] Run `git status --short` in each repo and attach output to review.
2. [P0] [A] Confirm no agent changed files outside its task card scope.
3. [P0] [A] Confirm contracts above have accepted status or explicit release blocker.
4. [P0] [A] Run all P0 validation commands for Agents A-D and F.
5. [P1] [A] Run ops dry-run validation for Agent E after ops repos are available.
6. [P0] [A] Run `cargo check --workspace` in APM after merging all APM agents.
7. [P1] [A] Run package tests: `cargo nextest run -p consultation-rs`, `cargo nextest run -p consultation-bg-rs`, `cargo nextest run -p common-rs` where environment allows.
8. [P0] [A] Execute disposable-DB ignored outbox idempotency test before production release.
9. [P0] [A] Verify Pub/Sub fallback, direct sync disable switch (`SERVICE__CONSULTATION_BG_BASE_URI=""` in DoctorApp Cloud Run dev/staging/prod), and outbox worker rollback switch in staging.
10. [P0] [A] Regenerate OpenAPI only if route/schema changes were intentionally merged.

## 6. Breaking-change checklist

- [A] No route removal or rename without compatibility alias and OpenAPI update.
- [A] No event payload field rename/removal without consumer sign-off and migration plan.
- [A] No auth-mode switch without ops/IAM rollout plan and rollback.
- [A] No patient mismatch cancellation behavior change without product/API decision.
- [A] No DB migration that invalidates existing reservation/session/outbox rows without backfill and rollback notes.

## 7. Architecture soundness verdict

- [I] The current architecture supports incremental parallel completion if contracts remain fixed: booking/timeslot, callbacks/session, payment/outbox, projection sync, and ops are separable bounded contexts.
- [I] A full redesign is not required now; the clean migration need is event-v2 compatibility and auth-mode decision, not module replacement.
- [A] Preserve direct-publish and outbox split for now, but document exact event-type ownership before adding new event types.
- [A] Contract-first parallel execution is safer than continued sequential discovery because each agent has isolated scope and acceptance gates.
