# Appointment Detail API — Contract

## Context

The BFF needs a single internal endpoint to fetch a confirmed appointment's
core data so it can render the patient-facing appointment-detail screen
(timing, status, channel, prescreen, payment refs). The consultation service
owns booking timing/status, prescreen, and the payment transaction refs.
Everything else (full patient name, DOB, payer/insurance, body analyzer
BMI/weight, attachment URL resolution) lives in other services and is
assembled by the BFF.

This endpoint is **internal-only** — no auth middleware, mounted under
`/v2/internal/*` like the existing `create-appointment` family.

---

## Decisions Log

| Question | Decision |
|----------|----------|
| Which service owns the API? | `consultation-rs` |
| Auth model? | Internal-only (no `auth_middleware`) |
| HTTP method / path? | `GET /v2/internal/appointment/{bookingId}` |
| How is "not found" represented? | HTTP 200 with discriminated `{ "__type": "AppointmentNotFound" }` (PascalCase variant names) |
| Patient & doctor identity shape? | Local `PartialUserIdentity { accountId, profileId }` only — no tenant/oidc fields |
| Out-of-scope fields (full name, DOB, gender, payer, BMI, weight)? | Omitted entirely; BFF assembles from other services |
| Prescreen response shape? | Typed `PrescreenInfo` (parsed/decrypted), mirrors `tdh_protocol::consultation::ConsultationPreScreen` |
| Prescreen storage variants? | `RAW_JSON` (plaintext) and `ENC_GCP_KMS` (base64-encoded GCP KMS ciphertext) |
| Payment refs returned? | **Both** `paymentTxId: i64` and `paymentTxRefId: String` (payment service's `PaymentTransaction` event carries both) |
| DB strategy? | Single Postgres function `v2.get_appointment_detail(p_booking_id)` does the joins; repo trait abstracts only the row model |

---

## API Contract

### `GET /v2/internal/appointment/{bookingId}`

**Path parameters**

| Name | Type | Description |
|------|------|-------------|
| `bookingId` | `string` | Appointment booking id (same as `appointmentId` per the v2 schema 1:1 relationship) |

**Headers**

None. This is an internal endpoint with no auth middleware.

**Request body**

None.

---

### Response `200 OK`

The response is a discriminated union on `__type`. Two variants:

#### Variant: `Success`

```json
{
  "__type": "Success",
  "bookingId": "BK20260227810949",
  "appointmentTime": {
    "startTime": 1645940400,
    "endTime":   1645941300
  },
  "status": "Booked",
  "bookingType": "Schedule",
  "consultationChannel": "Video",
  "patient": {
    "accountId": 124236,
    "profileId": 200
  },
  "doctor": {
    "accountId": 300,
    "profileId": 400
  },
  "prescreen": {
    "symptom": "มีผื่นที่แขนขาปรากฏเป็นจุดแดงหรือผลเป็นลักษณะคันบางครั้งรู้สึกแสบร้อน",
    "duration": 7,
    "durationUnit": "day",
    "attachments": [
      "att-ref-001",
      "att-ref-002",
      "att-ref-003",
      "att-ref-004"
    ],
    "allergies": ["Amoxicillin"]
  },
  "paymentTxId": 1042,
  "paymentTxRefId": "tx-ref-2026-02-27-001"
}
```

#### Variant: `AppointmentNotFound`

```json
{
  "__type": "AppointmentNotFound"
}
```

Returned when no row exists for the given `bookingId`. Still HTTP 200 — the
BFF branches on `__type`, not on status code.

---

### Field reference

#### Top-level success fields

| Field | Type | Notes |
|-------|------|-------|
| `bookingId` | `string` | Same value as the path param. |
| `appointmentTime.startTime` | `i64` | Epoch seconds (UTC). |
| `appointmentTime.endTime` | `i64` | Epoch seconds (UTC). |
| `status` | `enum` | FHIR appointment status. See enum table below. |
| `bookingType` | `enum` | `Instant` \| `Schedule` \| `FollowUp`. |
| `consultationChannel` | `enum` | `Video` \| `Voice` \| `Chat`. |
| `patient` | `PartialUserIdentity` | `accountId` + `profileId` only. |
| `doctor` | `PartialUserIdentity` | `accountId` + `profileId` only. |
| `prescreen` | `PrescreenInfo` | Decoded patient intake. See below. |
| `paymentTxId` | `i64` | Integer ref from the payment service's `PaymentTransaction.paymentTransactionId`. **See sentinel note below — `0` means "not yet recorded" on the confirmed-appointment path.** |
| `paymentTxRefId` | `string` | String ref from `PaymentTransaction.paymentTransactionRefId`. |

**Note on `paymentTxId = 0`**: Until the `tdh-protocol` crate's `CreateConfirmedInstantAppointmentRequest` is extended to carry the payment transaction id, confirmed instant appointments created via `/v2/internal/create-confirmed-appointment` will have `paymentTxId: 0` — this is a sentinel value, NOT a real payment transaction id. The BFF should treat `0` as "not yet recorded" and not display or forward it as a real id. Appointments created via `/v2/internal/create-appointment` always carry a real `paymentTxId` from the client. This sentinel will be removed once the protocol type is extended.

#### `PartialUserIdentity`

| Field | Type | Notes |
|-------|------|-------|
| `accountId` | `i32` | IAM account id. |
| `profileId` | `i32` | User profile id. |

This is intentionally distinct from
`tdh_protocol::common::PartialUserIdentity`, which also carries `tenantId`
and `oidcUserId`. We do not leak those through this read endpoint.

#### `PrescreenInfo`

| Field | Type | Notes |
|-------|------|-------|
| `symptom` | `string` | Free-text primary problem. |
| `duration` | `i32` | Period of sickness, paired with `durationUnit`. |
| `durationUnit` | `string` | E.g. `"day"`, `"week"`. |
| `attachments` | `string[]` | Opaque attachment refs. BFF resolves to URLs/SecureContent. |
| `allergies` | `string[]` | Drug allergies. |

The shape mirrors
`tdh_protocol::consultation::consultation_pre_screen::ConsultationPreScreen`.
The service decodes it from one of two storage variants (see Storage section
below) and returns a single uniform shape regardless of how it was stored.

#### `status` — FHIR appointment status

Wire values are PascalCase variant names from `AppointmentStatusEnum`
(serde default — the enum has no `#[serde(rename = ...)]`). The DB stores
the same values in SCREAMING_SNAKE via `#[sqlx(rename = ...)]`:

| Wire value | DB value | Meaning |
|------------|----------|---------|
| `Proposed` | `PROPOSED` | Tentatively proposed. |
| `Pending` | `PENDING` | Awaiting confirmation. |
| `Booked` | `BOOKED` | Confirmed and scheduled. |
| `Arrived` | `ARRIVED` | Patient arrived (joined session). |
| `Fulfilled` | `FULFILLED` | Consultation completed. |
| `Cancelled` | `CANCELLED` | Cancelled by either party. |
| `Noshow` | `NOSHOW` | Patient did not show. |
| `EnteredInError` | `ENTERED_IN_ERROR` | Erroneously created. |

#### `bookingType`

| Wire value | Meaning |
|------------|---------|
| `Instant` | Created on-demand, started immediately. |
| `Schedule` | Reserved for a future timeslot. |
| `FollowUp` | Linked to a parent appointment. |

#### `consultationChannel`

PascalCase variant names from `ConsultationChannelEnum`. The DB stores
lowercase via `#[sqlx(rename = ...)]`:

| Wire value | DB value | Meaning |
|------------|----------|---------|
| `Video` | `video` | Video session. |
| `Voice` | `voice` | Voice-only. |
| `Chat` | `chat` | Text chat. |

---

## Error responses

Successful lookups (including the not-found case) always return HTTP 200 with
the discriminated response. Errors only happen for genuine failures:

| Status | Error type | Cause |
|--------|------------|-------|
| 500 | `INTERNAL_ERROR` | DB connection failure, malformed prescreen JSON, KMS decrypt failure, base64 decode failure, unsupported `prescreen_data_type`. |

Error body shape (from `consultation-rs/src/common/handlers/http_error.rs`):

```json
{
  "error": {
    "type": "INTERNAL_ERROR",
    "message": "Failed to fetch appointment detail",
    "trace_id": "...",
    "span_id": "..."
  }
}
```

Error responses also include `traceparent`, `x-trace-id`, `x-span-id`
headers when a tracing span is active.

---

## Prescreen storage & decoding

The DB column `v2.patient_prescreen.prescreen_data` is `text`. The companion
column `prescreen_data_type` discriminates how the bytes were stored:

| `prescreen_data_type` | Encoding | Read-side behaviour |
|-----------------------|----------|---------------------|
| `RAW_JSON` | UTF-8 JSON of `ConsultationPreScreen` | `serde_json::from_str` directly. |
| `ENC_GCP_KMS` | Base64(STANDARD) of GCP KMS ciphertext | base64-decode → `Kms::decrypt(key=google_cloud.kms.prescreen)` → UTF-8 → `serde_json::from_str`. |

Anything else → `500 INTERNAL_ERROR` with
`UnsupportedPrescreenDataType` in the server logs. Adding a new variant is a
service-code change, not a data-only change.

The service is read-only with respect to KMS — encryption happens in the
write path (out of scope for this contract).

---

## Sample curl

```bash
curl -sS http://localhost:8080/v2/internal/appointment/BK20260227810949 | jq
```

Success:

```json
{
  "__type": "Success",
  "bookingId": "BK20260227810949",
  "appointmentTime": { "startTime": 1645940400, "endTime": 1645941300 },
  "status": "Booked",
  "bookingType": "Schedule",
  "consultationChannel": "Video",
  "patient": { "accountId": 124236, "profileId": 200 },
  "doctor":  { "accountId": 300,    "profileId": 400 },
  "prescreen": {
    "symptom": "headache",
    "duration": 7,
    "durationUnit": "day",
    "attachments": ["att-ref-001"],
    "allergies": ["Amoxicillin"]
  },
  "paymentTxId": 1042,
  "paymentTxRefId": "tx-ref-2026-02-27-001"
}
```

Not found:

```bash
curl -sS http://localhost:8080/v2/internal/appointment/UNKNOWN | jq
# { "__type": "AppointmentNotFound" }
```

---

## Non-functional requirements

### Distributed tracing & log correlation (GCP Cloud Trace + Cloud Logging)

Every request to this endpoint MUST be observable end-to-end through GCP
Cloud Trace, and every log line emitted while handling the request MUST be
auto-correlated to the trace in Cloud Logging.

Concretely, the implementation must satisfy the following:

1. **Trace context ingress** — the HTTP layer extracts trace context from
   the incoming request in this priority order:
   1. W3C `traceparent` header (`00-<32 hex trace id>-<16 hex span id>-<flags>`)
   2. GCP `X-Cloud-Trace-Context` header
      (`TRACE_ID/SPAN_ID;o=TRACE_TRUE`)
   3. If neither present, generate a fresh root trace id locally.

   This is implemented once at the router level (e.g. via a tower middleware
   layer using `opentelemetry` + `opentelemetry-http`'s `TextMapPropagator`)
   so every endpoint benefits — not just this one.

2. **Span creation** — the handler is annotated with
   `#[tracing::instrument(skip(state), fields(booking_id = %booking_id, http.method = "GET", http.route = "/v2/internal/appointment/{bookingId}"))]`
   so the span carries the booking id and route as searchable attributes in
   Cloud Trace. The repo call and KMS decrypt call (when used) inherit this
   span as their parent.

3. **Trace id format** — the trace id used for both response headers and
   log correlation MUST be a 32-character lowercase hex W3C trace id, NOT
   the `tracing` crate's internal `u64` span id. The current
   `TraceError::trace_context()` implementation
   (`consultation-rs/src/common/handlers/http_error.rs:56-80`) uses
   `span.id().into_u64()` and is incorrect for GCP — this needs replacing
   with the OpenTelemetry span context that the propagator put on the
   tracing span.

4. **Response trace headers** — every response (success, `appointmentNotFound`,
   error) carries:
   - `traceparent: 00-<32hex trace>-<16hex span>-01` (W3C, the span the
     server actually used for the request)
   - `x-trace-id: <32hex trace>` (raw trace id, for grep convenience)
   - `x-span-id: <16hex span>` (raw span id)

5. **Log enrichment** — the `CloudRunFormatter` in
   `consultation-rs/src/main.rs` emits these JSON fields on every log line
   produced while the request span is active:
   - `logging.googleapis.com/trace`:
     `projects/<gcp_project_id>/traces/<32hex trace id>`
   - `logging.googleapis.com/spanId`: `<16hex span id>`
   - `logging.googleapis.com/trace_sampled`: `true` when the trace is
     sampled, `false` otherwise

   With these fields present, Cloud Logging automatically links the log
   entries to the corresponding span in Cloud Trace and exposes them in the
   trace waterfall view. The GCP project id is read from
   `infra.config.google_cloud.project_id`.

6. **What the handler must log** — at INFO level when the request starts
   and ends (latency in ms), at ERROR level on any failure path. The
   `booking_id` is in the span attributes so it does NOT need to be repeated
   in every log message — but the start/end logs SHOULD include it for
   readability when scanning raw logs.

7. **PII safety** — under no circumstances does the handler log
   `prescreen_data` (raw or decrypted), `payment_tx_ref_id`, or full
   patient identity. Only `booking_id`, `patient.account_id`,
   `patient.profile_id`, and the `prescreen_data_type` discriminator are
   safe to log.

#### Cross-cutting work this NFR implies

The trace propagator + log correlation pieces are not specific to this
endpoint — they belong in shared infrastructure code and benefit every
existing handler. Implementation will:

- Add an `init_tracing(project_id: &str)` helper in
  `consultation-rs/src/sys/` that wires `opentelemetry`, sets the global
  `TextMapPropagator` to W3C TraceContext + GCP propagation, and installs a
  `tracing-opentelemetry` layer alongside the existing
  `tracing_subscriber::fmt` layer.
- Add a tower layer (e.g. `tower_http::trace::TraceLayer` extended with a
  custom `MakeSpan` that calls
  `propagator.extract(&request_headers)`) onto the top-level `Router` in
  `main.rs` so every request — not just this endpoint — picks up the
  inbound trace context.
- Replace `TraceError::trace_context()` with a version that pulls the
  current span's `OpenTelemetrySpanExt::context()` and reads the
  `SpanContext` from it.
- Extend `CloudRunFormatter::format_event` to emit the
  `logging.googleapis.com/trace` and `spanId` fields when the active span
  has an OTel context.

This list is repeated in the implementation plan; it's documented here so
the API contract makes the observability guarantee explicit to consumers
(BFF can rely on `traceparent` round-tripping) and reviewers can see why
the work landed alongside this feature.

### Latency budget

| Metric | Target |
|--------|--------|
| p50 | < 30 ms |
| p95 | < 100 ms |
| p99 | < 250 ms |

These assume the `RAW_JSON` happy path (one Postgres function call, no KMS
call). The `ENC_GCP_KMS` path adds one synchronous GCP KMS `Decrypt` RPC; a
realistic p95 for that variant is `~150–250 ms` dominated by the KMS round
trip. KMS failures must not surface as slow failures — fail fast on the
KMS error rather than retrying inline.

### Availability

This endpoint is in the read path of the BFF appointment-detail screen, so
it inherits whatever SLO consultation-rs has for read endpoints. No
dedicated rate limiting is needed at the consultation-rs layer — the BFF
fronts it and is itself rate-limited.

### Caching

No response caching is added at this layer. Appointment status changes
(BOOKED → ARRIVED → FULFILLED) need to be observable immediately by the
BFF, so a cache would have to be invalidated on every status transition,
which isn't worth the complexity. If this becomes a hot path later, the
right place to add caching is the BFF, not consultation-rs.

---

## Out of scope (BFF responsibility)

The following appear in the appointment-detail UI but are **not** returned
by this endpoint. The BFF must source them elsewhere:

- Patient full name, date of birth, age, gender → IAM / patient profile service
- Payer (insurer name), insurance condition → policy / payer service
- Body Analyzer: BMI, weight → health-record service
- Consultation channel display label / icons → BFF-side i18n
- Attachment URL resolution (the `attachments` field returns opaque refs only)
- Past visit / lab results tabs (separate endpoints)
