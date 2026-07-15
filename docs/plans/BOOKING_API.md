# Booking API — Implementation Plan

## Context

After a patient retrieves available timeslots via `GET /doctors/{id}/timeslots` (doctor-pool),
they call `POST /v2/appointment/book` (consultation-rs) to lock a slot and receive a signed
booking token. The token proves the reservation and carries the expiry inline so downstream
services can verify it without a DB round-trip.

---

## Decisions Log

| Question | Decision |
|----------|----------|
| Which service owns the booking API? | `consultation-rs` |
| How is the reservation created? | Reuse existing `ReserveRepoPsql` / `create_reservation()` PG function |
| What does the API return? | Paseto **v4.public** signed token (Ed25519 asymmetric) |
| Where does patient identity come from? | `tdh-sec-iam-user-identity` request header (existing auth middleware) |
| How is patient intake stored? | Encrypted in `patient_prescreen` using `Kms` trait |
| KMS for local dev? | `LocalAesKms` (AES-256-GCM, key from config) |
| KMS for production? | `GcpKmsService` (existing stub, enabled when `kms_key_name` is set) |
| Message publishing? | Real GCP PubSub via `google-cloud-pubsub`; respects `PUBSUB_EMULATOR_HOST` |
| Instant booking duration? | Configurable `instant_consultation_duration_sec` in `[booking]` config section |
| Booking types? | `Instant` (now → now+duration) and `Schedule` (explicit `slotStartUnix`/`slotEndUnix`) |

---

## API Contract

### `POST /v2/appointment/book`

**Headers**
```
tdh-sec-iam-user-identity: <UserIdentity JSON>
Content-Type: application/json
```

**Request body**
```json
{
  "doctorId": "b1f2c3d4-...",
  "bizUnitId": 1,
  "bizCenterId": null,
  "consultationChannel": { "__type": "video" },
  "bookingType": {
    "__type": "schedule",
    "slotStartUnix": 1710000000,
    "slotEndUnix":   1710001800
  },
  "intake": {
    "symptom": "headache",
    "symptomDuration": "2",
    "symptomDurationUnit": "days",
    "attachments": [],
    "allergies": ["penicillin"]
  }
}
```

For an **instant booking** the `bookingType` field is:
```json
{ "__type": "instant" }
```
`reserved_from = now()`, `reserved_until = now() + instant_consultation_duration_sec`.

**Response `200 OK`**
```json
{
  "bookingToken": "v4.public.eyJ...",
  "bookingId": 1042,
  "reservedFrom": 1710000000,
  "reservedUntil": 1710001800
}
```

**Error responses**

| Status | `error` field | Cause |
|--------|---------------|-------|
| 400 | `INVALID_SLOT_TIME` | `slotEndUnix <= slotStartUnix` |
| 401 | `UNAUTHORIZED` | Missing or unparseable identity header |
| 409 | `SLOT_ALREADY_BOOKED` | PG function detected overlap |
| 500 | `INTERNAL_ERROR` | DB / PubSub / KMS failure |

All errors return JSON: `{ "error": "...", "message": "..." }`.

---

## Paseto Token Payload (`BookingTokenClaims`)

```json
{
  "bookingId": 1042,
  "doctorId": "b1f2c3d4-...",
  "patientAccountId": 101,
  "patientProfileId": 201,
  "reservedFrom": 1710000000,
  "reservedUntil": 1710001800,
  "bookingType":          { "__type": "schedule" },
  "consultationChannel":  { "__type": "video" },
  "exp": 1710001800,
  "iat": 1709999100
}
```

`exp` = `reserved_until` so the token naturally expires when the reservation expires.

Key algorithm: **Ed25519** (Paseto v4.public).  
Secret key (seed ‖ public key) stored as 128 hex chars in `config.booking.paseto_secret_key_hex`.

---

## Booking Service Logic

```
book(UserIdentity, BookingRequest) → BookingResponse

1. Resolve times
   ├── Instant:  reserved_from = now_unix
   │             reserved_until = now_unix + instant_consultation_duration_sec
   └── Schedule: reserved_from = slot_start_unix
                 reserved_until = slot_end_unix
                 guard: reserved_from < reserved_until

2. ttl_seconds = (reserved_until - reserved_from) as i32

3. reserve_repo.create_reservation(
       patient_account_id, patient_profile_id,
       doctor_id, 0 /*doc_account*/, 0 /*doc_profile*/,
       biz_unit_id, biz_center_id, tenant_id,
       booking_type_str,           ← "INSTANT" or "SCHEDULE"  (bug fix)
       consultation_channel_str,
       reserved_from, trace_id, ttl_seconds
   ) → Reservation

4. kms.encrypt(serde_json::to_vec(&intake)?, kms_key_name)
   → encrypted_bytes (base64-encoded for storage)

5. prescreen_repo.insert_prescreen(
       reservation_id, account_id, profile_id,
       encrypted_data, encrypted_data_type
   )

6. Sign Paseto v4.public token from BookingTokenClaims

7. event_publisher.publish_consultation_event(
       PreSessionMessage::TimeslotReserved { ... }
   )

8. return BookingResponse { booking_token, booking_id, reserved_from, reserved_until }
```

---

## Files To Create / Modify

### New files

| File | Purpose |
|------|---------|
| `db/postgres/migrations/20240101000004_drop_prescreen_fk.sql` | Drop FK on `patient_prescreen.appointment_id` so intake can be stored before appointment row exists |
| `consultation-rs/src/services/booking/mod.rs` | `BookingService`, `BookingRequest`, `BookingResponse`, `BookingError`, `BookingDetails` |
| `consultation-rs/src/repo/prescreen.rs` | `PrescreenRepo` trait + `PrescreenRepoPsql` impl |
| `consultation-rs/src/handlers/v2/appointment/booking.rs` | HTTP handler for `POST /v2/appointment/book` |
| `docs/plans/BOOKING_API.md` | This document |

### Modified files

| File | Change |
|------|--------|
| `protocol-rs/src/consultation/patient_consultation_request.rs` | Fix field names (snake_case), add `Serialize`/`Deserialize`, rename to `PatientIntakeForm` |
| `consultation-rs/Cargo.toml` | Add `pasetors`, `google-cloud-pubsub`, `base64`, `aes-gcm`, `rand`, `hex` |
| `consultation-rs/config/defataul.toml` | Add `[booking]` section with all new config keys |
| `consultation-rs/config/local.toml.example` | Add `[booking]` with dev-friendly values (emulator host, dev keys) |
| `consultation-rs/src/sys/config.rs` | Add `BookingConfig` struct; add `pub booking: BookingConfig` to `AppConfig` |
| `consultation-rs/src/sys/crypto/kms.rs` | Add `LocalAesKms` implementing `Kms` trait with AES-256-GCM |
| `consultation-rs/src/services/event/mod.rs` | Replace no-op `PubSubEventPublisher` with real GCP PubSub implementation |
| `consultation-rs/src/services/reserve.rs` | Fix `"SCHEDULED"` → `"SCHEDULE"` bug (DB enum value) |
| `consultation-rs/src/repo/mod.rs` | `pub mod prescreen;` |
| `consultation-rs/src/services/mod.rs` | `pub mod booking;` |
| `consultation-rs/src/handlers/v2/appointment/mod.rs` | `pub mod booking;` |
| `consultation-rs/src/state.rs` | Add `pub booking_service: BookingService` field |
| `consultation-rs/src/main.rs` | Wire KMS, real PubSub, `PrescreenRepoPsql`, `BookingService`, register route |

---

## New Config Section

```toml
# consultation-rs/config/defataul.toml  [booking]
[booking]
reservation_ttl_seconds           = 900
instant_consultation_duration_sec = 900

# Ed25519 seed||pubkey as 128 hex chars — CHANGE IN PRODUCTION
paseto_secret_key_hex = "0000000000000000000000000000000000000000000000000000000000000000\
                         3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29"

# AES-256-GCM key as 64 hex chars (32 bytes) — CHANGE IN PRODUCTION
local_kms_key_hex = "0000000000000000000000000000000000000000000000000000000000000000"

# Leave empty to use LocalAesKms. Set to full KMS key resource name for GCP KMS.
# e.g. "projects/my-project/locations/global/keyRings/my-ring/cryptoKeys/intake-key/cryptoKeyVersions/1"
kms_key_name = ""

pubsub_project_id    = "test-project"
pubsub_topic         = "consultation-session-events"
# Set to "localhost:8085" in local.toml to use the PubSub emulator
pubsub_emulator_host = ""
```

---

## GCP PubSub Publisher Design

```
PubSubEventPublisher::new(project_id, topic, emulator_host) -> Result<Self>
  if emulator_host.is_some():
      std::env::set_var("PUBSUB_EMULATOR_HOST", emulator_host)
  client = google_cloud_pubsub::client::Client::new(project_id).await?

publish_consultation_event(event):
  json_bytes = serde_json::to_vec(&event)?
  b64_data   = base64::encode(json_bytes)
  msg = PubsubMessage { data: b64_data, ..Default::default() }
  client.topic(topic).new_publisher(None).publish(msg).await?
```

`PUBSUB_EMULATOR_HOST` is the standard env var recognized by the GCP client library — no code change needed to switch between emulator and production.

---

## KMS Design

### `LocalAesKms` (AES-256-GCM)

```
encrypt(plaintext, _key_name):
  key  = hex::decode(config.local_kms_key_hex)  // 32 bytes
  nonce = random 12 bytes
  ciphertext = AesGcm256::new(key).encrypt(nonce, plaintext)
  return nonce || ciphertext    // stored as hex string

decrypt(ciphertext_hex, _key_name):
  bytes = hex::decode(ciphertext_hex)
  nonce = bytes[..12]
  ct    = bytes[12..]
  return AesGcm256::new(key).decrypt(nonce, ct)

encrypted_data_type = "aes-256-gcm-v1"
```

### KMS selection at startup (`main.rs`)

```rust
let kms: Arc<dyn Kms> = if config.booking.kms_key_name.is_empty() {
    Arc::new(LocalAesKms::from_hex(&config.booking.local_kms_key_hex)?)
} else {
    Arc::new(GcpKmsService::new().await?)
};
```

---

## Known Bugs Fixed During Implementation

| Bug | Location | Fix |
|-----|----------|-----|
| `BookingType::Schedule → "SCHEDULED"` but DB enum is `'SCHEDULE'` | `services/reserve.rs:44` | Change to `"SCHEDULE"` |
| `doctor_account_id` / `doctor_profile_id` hardcoded `0` | `services/reserve.rs:58-59` | Keep as `0` (TODO: resolve doctor identity from UUID) |

---

## Out of Scope (deferred)

- Token verification / decode endpoint
- Doctor identity resolution (account_id / profile_id from doctor UUID)
- Payment flow integration
- Integration tests for the booking endpoint (requires PubSub emulator in testcontainers — add separately)
- Reservation expiry background job (`consultation-bg-rs` stub)

---

## Implementation Order

```
1. DB migration            — drop prescreen FK
2. protocol-rs             — fix PatientIntakeForm
3. Cargo.toml              — add dependencies
4. sys/config.rs           — BookingConfig
5. sys/crypto/kms.rs       — LocalAesKms
6. services/event/mod.rs   — real PubSubEventPublisher
7. services/reserve.rs     — bug fix "SCHEDULE"
8. repo/prescreen.rs       — PrescreenRepo + PrescreenRepoPsql
9. services/booking/mod.rs — BookingService
10. handlers/.../booking.rs — HTTP handler
11. state.rs + main.rs     — wire everything
12. config files           — add [booking] section
```

---

## Sequence Diagram

```
Client
  │  POST /v2/appointment/book
  │  (tdh-sec-iam-user-identity header)
  ▼
auth_middleware ──────────────────────► Extension<UserIdentity>
  │
  ▼
BookingHandler
  │  resolve times (Instant / Schedule)
  │
  ▼
ReserveRepoPsql
  │  create_reservation() PG function
  │  → reservation_id, reserved_from, reserved_until
  ▼
LocalAesKms / GcpKmsService
  │  encrypt(intake JSON)
  │  → encrypted_bytes
  ▼
PrescreenRepoPsql
  │  INSERT patient_prescreen (appointment_id = reservation_id)
  ▼
PasetoSigner (Ed25519)
  │  sign BookingTokenClaims { exp = reserved_until }
  │  → booking_token string
  ▼
PubSubEventPublisher
  │  publish PreSessionMessage::TimeslotReserved
  │  → GCP PubSub topic "consultation-session-events"
  ▼
BookingHandler
  │  200 OK
  └─► { bookingToken, bookingId, reservedFrom, reservedUntil }
```
