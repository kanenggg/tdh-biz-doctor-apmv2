# Payment Confirm Module Design

## Summary

Complete the payment-confirm module in `consultation-bg-rs` to receive GCP PubSub push messages when a payment succeeds, decode the payment transaction, and idempotently confirm the appointment by updating its status and stamping the payment transaction.

## Context

- `consultation-bg-rs` is a background service for the consultation system
- The payment processor publishes a PubSub push message when payment completes
- The appointment must transition from `PENDING` to `BOOKED` and have its payment transaction recorded
- Messages may be redelivered, so idempotency is required

## Data Flow

```
GCP PubSub push
  -> POST /pubsub/payment-confirm
  -> Parse PubSubPushMessage (base64-decode .message.data)
  -> PaymentMessage { payment_status, payment_data, payment_ref_code, payment_transaction_ref_id }
  -> Filter: only process if payment_status == "success"
  -> Verify/decode payment_data -> PaymentTransaction
  -> Extract booking_id from ExtendData (ConsultInfo.booking_id | FollowUp.previous_booking_id)
  -> Extract payment_channels from SelectedChannelResult
  -> Call v2.upsert_payment_transaction(booking_id, payment_tx_id, payment_tx_ref_id, payment_channels)
  -> Return HTTP 200
```

## Architecture

The module follows the existing layered pattern in `consultation-rs`:

| Layer | File | Responsibility |
|-------|------|----------------|
| Handler | `payment_confirm/handler.rs` | Axum route: parse PubSubPushMessage, call service, return 200 |
| Service | `payment_confirm/service.rs` | Business logic: filter success, verify token, extract channels, call repo |
| Repo | `payment_confirm/repo.rs` | DB call to `v2.upsert_payment_transaction` |
| Token | `payment_confirm/payement_token.rs` | Verify/decode payment_data (base64+JSON or PASETO) |

### Handler (`handler.rs` - new file)

Single axum POST endpoint at `/pubsub/payment-confirm`:
- Receives `PubSubPushMessage` as JSON body
- Extracts `googclient_traceparent` from attributes for trace propagation
- Logs `message_id` on entry
- Calls `PaymentConfirmService::handle_payment_tx_v1`
- Returns HTTP 200 on success (acknowledges PubSub delivery)
- Returns HTTP 500 on failure (triggers PubSub retry)
- Logs errors with booking_id/payment_tx_ref_id for debugging

### Service (`service.rs` - extend existing)

The existing `handle_payment_tx_v1` already handles:
- Token verification
- booking_id extraction from ExtendData
- Payment channel extraction
- Repo call

Changes needed:
- Add `payment_status` filtering: if not `"success"`, return Ok (acknowledge but skip)
- Add `payment_transaction_ref_id` from `PaymentMessage` to repo call (currently using the one from PaymentTransaction, which is correct)
- The service remains unchanged in its core logic - it's already well-structured

### Repo (`repo.rs` - extend existing)

The existing `upsert_payment_transaction` must be extended to pass `payment_tx_id` (bigint). The `PaymentTransaction` has both IDs:
- `payment_transaction_id: i64` (integer ID) -> maps to `payment_tx_id`
- `payment_transaction_ref_id: String` (string ref) -> maps to `payment_tx_ref_id`

Updated trait method signature:
```rust
async fn upsert_payment_transaction(
    &self,
    booking_id: &str,
    payment_tx_id: i64,
    payment_tx_ref_id: &str,
    payment_channels: &PaymentChannels,
) -> Result<(), anyhow::Error>;
```

All idempotency logic lives in the postgres function.

### Payment Channels Downstream Consumer

The `payment_channels` stored in `appointment_payment_transaction` is read by `v2.get_consultation_session`, which joins the table to return `payment_channels` as jsonb. The consultation session service (`consultation-rs`) uses this to determine if patient verification is required (insurance channels). This is a critical downstream dependency — payment_channels must be stored correctly for the consultation session feature to work.

### Token Verification (`payement_token.rs` - update)

The `payment_data` field in the PubSub message is a **PASETO v2.public signed token** (Ed25519). The payment processor signs the `PaymentTransaction` JSON with an Ed25519 private key. The verifier must:

1. Parse the PASETO v2.public token
2. Verify the Ed25519 signature using the public key (derived from the configured secret key)
3. Extract the `"payload"` claim from the verified token
4. Deserialize the payload JSON as `PaymentTransaction`

The current implementation only does base64+JSON decode without PASETO verification. This must be updated to use proper PASETO v2.public verification. The `paseto` crate (v2.0) is already in workspace dependencies.

The public key can be derived from the Ed25519 seed (configured via `PaymentConfig.secret_key`).

## Database Changes

### Modify `v2.upsert_payment_transaction`

The existing function at `db/biz_apm/migrations/20260222000009__funcs.sql` creates the appointment with status `CONFIRMED` and upserts the payment transaction.

**New behavior** (via a new migration file):

```sql
CREATE OR REPLACE FUNCTION v2.upsert_payment_transaction(
    p_booking_id varchar(20),
    p_payment_tx_id bigint,
    p_payment_tx_ref_id varchar(255),
    p_payment_channels jsonb
) RETURNS varchar(20)
```

1. Find the reservation by `booking_id` (must exist, not deleted)
2. Check if appointment exists:
   - **No appointment**: Create it with status `BOOKED`
   - **Appointment PENDING**: Transition to `BOOKED`
   - **Appointment BOOKED**: Skip status update (idempotent success, log warning for visibility)
   - **Appointment in other status** (CANCELLED, FULFILLED, etc.): Raise exception
3. Upsert payment transaction (existing logic, ON CONFLICT DO UPDATE)
4. Return `appointment_id`

The appointment status column now uses `v2.fhir_appointment_status_enum`:
- `PENDING` - initial state
- `BOOKED` - confirmed (was `CONFIRMED` in old enum)
- `FULFILLED` - consultation done
- `CANCELLED` - cancelled

The function must use `BOOKED` instead of `CONFIRMED` since the migration `20260401000002` migrated the enum.

### New Migration File

`db/biz_apm/migrations/20260416000001__payment_confirm_idempotent.sql`

Contains the updated `v2.upsert_payment_transaction` function with idempotent status transitions.

## Main.rs Wiring

Replace the stub `main.rs` with:

1. Parse CLI args for config path
2. Load `AppConfig` (extend with database config)
3. Create DB pool (PgPool)
4. Construct `PaymentConfirmPsql` repo, `PaymentVerifierWithPaseto`, `PaymentConfirmService`
5. Build axum router with `/pubsub/payment-confirm` POST route
6. Start HTTP server

Config additions needed in `sys/config.rs`:
- `DatabaseConfig` (host, port, user, password, database_name) - copy pattern from `consultation-rs`
- `ServerConfig` (host, port) for HTTP listener
- `HttpServerConfig` can be replaced with a simple inline `ServerConfig`

## Error Handling

- **PubSub push returns 200** for successful processing AND for non-success payment status (acknowledges delivery, no retry)
- **PubSub push returns 500** only for unexpected errors (DB down, malformed payload) - triggers PubSub retry
- **Service errors** (missing ExtendData, unexpected ExtendData type, invalid token) return 500 to trigger retry
- **Idempotent replay** (appointment already BOOKED) returns 200 successfully

## Tracing and Observability

### GCP PubSub Trace Context

GCP PubSub push messages carry trace context in the `message.attributes` field as `googclient_traceparent` (W3C Trace Context format). The handler must extract this and propagate it into the tracing span so all downstream logs are correlated with the PubSub delivery trace.

- Extract `googclient_traceparent` from `PubSubPushMessage.message.attributes`
- Set as parent span context in the current tracing span
- Log `message_id` as structured field on every log line within the request

### Structured Logging

All log statements use structured fields (not string interpolation) so Cloud Logging can index them:

| Field | Where | Example |
|-------|-------|---------|
| `booking_id` | service, repo | `"202604161234"` |
| `payment_tx_id` | service, repo | `12345` |
| `payment_tx_ref_id` | service, repo | `"uuid-123"` |
| `message_id` | handler | `"550e8400-e29b-..."` |
| `payment_status` | service (filter) | `"success"` |

### Idempotent Replay Logging

When the postgres function detects an already-BOOKED appointment (idempotent replay):

- **Postgres**: `RAISE NOTICE 'Idempotent payment confirm for booking_id=%, already BOOKED', p_booking_id;`
- **Rust service**: `tracing::warn!(booking_id, payment_tx_ref_id, "Payment confirm skipped: appointment already BOOKED (idempotent replay)");`

This ensures replayed messages are visible in logs without treating them as errors.

### Non-Success Payment Status

When `payment_status != "success"`:

- `tracing::info!(payment_status, payment_tx_ref_id, "Payment not successful, skipping confirm");`

## Testing Strategy

- Unit tests for service layer with mock repo and mock token verifier
- Integration tests using testcontainers (postgres) for the repo layer and postgres function
- The existing mock-gen tool can generate PubSub push payloads for manual/integration testing
