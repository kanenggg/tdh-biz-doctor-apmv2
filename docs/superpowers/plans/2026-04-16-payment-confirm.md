# Payment Confirm Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the payment-confirm module in `consultation-bg-rs` to receive GCP PubSub push messages, verify PASETO-signed payment transactions, and idempotently confirm appointments.

**Architecture:** Layered (handler -> service -> repo -> postgres function). Axum HTTP endpoint receives PubSub push, service handles business logic, postgres function handles idempotent upsert with status transition. Follows patterns from `consultation-rs`.

**Tech Stack:** Rust (edition 2024), axum 0.8, sqlx 0.8, paseto 2.0, tdh-protocol, tracing

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `db/biz_apm/migrations/20260416000001__payment_confirm_idempotent.sql` | Idempotent postgres function |
| Modify | `consultation-bg-rs/src/sys/config.rs` | Add DatabaseConfig, ServerConfig |
| Modify | `consultation-bg-rs/src/payment_confirm/repo.rs` | Add payment_tx_id param |
| Modify | `consultation-bg-rs/src/payment_confirm/payement_token.rs` | PASETO v2.public verification |
| Modify | `consultation-bg-rs/src/payment_confirm/service.rs` | payment_status filter, pass payment_tx_id, structured logging |
| Create | `consultation-bg-rs/src/payment_confirm/handler.rs` | Axum POST handler for PubSub push |
| Modify | `consultation-bg-rs/src/payment_confirm/mod.rs` | Export handler |
| Modify | `consultation-bg-rs/src/main.rs` | Wire router, DB pool, config |
| Modify | `consultation-bg-rs/Cargo.toml` | Add axum, clap, tokio, urlencoding deps |

---

### Task 1: Postgres migration — idempotent `v2.upsert_payment_transaction`

**Files:**
- Create: `db/biz_apm/migrations/20260416000001__payment_confirm_idempotent.sql`

- [ ] **Step 1: Write the migration SQL**

The function must:
- Accept new `p_payment_tx_id bigint` parameter
- Use `v2.fhir_appointment_status_enum` (`BOOKED` not `CONFIRMED`)
- Handle idempotent status transitions (PENDING->BOOKED, skip if BOOKED, error if other)
- Upsert payment transaction with `payment_tx_id`

```sql
-- sqlfluff:dialect:postgres

-- ============================================
-- Payment Confirm: Idempotent upsert_payment_transaction
-- ============================================
-- Updates v2.upsert_payment_transaction to:
-- 1. Accept payment_tx_id (bigint)
-- 2. Handle idempotent appointment status transitions (PENDING -> BOOKED)
-- 3. Log idempotent replays via RAISE NOTICE

DROP FUNCTION IF EXISTS v2.upsert_payment_transaction CASCADE;
CREATE OR REPLACE FUNCTION v2.upsert_payment_transaction(
    p_booking_id varchar(20),
    p_payment_tx_id bigint,
    p_payment_tx_ref_id varchar(255),
    p_payment_channels jsonb
) RETURNS varchar(20) AS $$
DECLARE
    v_appointment_id varchar(20);
    v_current_status v2.fhir_appointment_status_enum;
    v_reservation RECORD;
    v_prescreen_id integer;
BEGIN
    -- Get reservation (must exist)
    SELECT * INTO v_reservation
    FROM v2.reservation r
    WHERE r.booking_id = p_booking_id AND r.deleted_at IS NULL;

    IF v_reservation IS NULL THEN
        RAISE EXCEPTION 'Reservation not found for booking_id: %', p_booking_id;
    END IF;

    -- Check if appointment exists
    SELECT a.appointment_id, a.appointment_status
    INTO v_appointment_id, v_current_status
    FROM v2.appointment a
    WHERE a.appointment_id = p_booking_id;

    IF v_appointment_id IS NULL THEN
        -- Get prescreen_data_id from patient_prescreen
        SELECT prescreen_id INTO v_prescreen_id
        FROM v2.patient_prescreen
        WHERE booking_id = p_booking_id;

        -- Create appointment with BOOKED status
        INSERT INTO v2.appointment (
            appointment_id,
            booking_id,
            prescreen_data_id,
            appointment_status,
            appointment_start,
            consult_duration,
            appointment_end,
            has_follow_up
        ) VALUES (
            p_booking_id,
            p_booking_id,
            COALESCE(v_prescreen_id, 0),
            'BOOKED'::v2.fhir_appointment_status_enum,
            v_reservation.appointment_start,
            v_reservation.appointment_end - v_reservation.appointment_start,
            v_reservation.appointment_end,
            false
        )
        RETURNING appointment_id INTO v_appointment_id;
    ELSIF v_current_status = 'PENDING'::v2.fhir_appointment_status_enum THEN
        -- Transition PENDING -> BOOKED
        UPDATE v2.appointment
        SET appointment_status = 'BOOKED'::v2.fhir_appointment_status_enum,
            modified_at = NOW()
        WHERE appointment_id = p_booking_id;
    ELSIF v_current_status = 'BOOKED'::v2.fhir_appointment_status_enum THEN
        -- Idempotent replay — skip status update
        RAISE NOTICE 'Idempotent payment confirm for booking_id=%, already BOOKED', p_booking_id;
    ELSE
        RAISE EXCEPTION 'Cannot confirm payment for booking_id=%, appointment status is %', p_booking_id, v_current_status;
    END IF;

    -- Upsert payment transaction
    INSERT INTO v2.appointment_payment_transaction (appointment_id, payment_tx_id, payment_tx_ref_id, payment_channels)
    VALUES (p_booking_id, p_payment_tx_id, p_payment_tx_ref_id, p_payment_channels)
    ON CONFLICT (appointment_id)
    DO UPDATE SET
        payment_tx_id = EXCLUDED.payment_tx_id,
        payment_tx_ref_id = EXCLUDED.payment_tx_ref_id,
        payment_channels = EXCLUDED.payment_channels,
        modified_at = NOW();

    RETURN v_appointment_id;
END;
$$ LANGUAGE plpgsql;
```

- [ ] **Step 2: Commit**

```bash
git add db/biz_apm/migrations/20260416000001__payment_confirm_idempotent.sql
git commit -m "feat(db): idempotent v2.upsert_payment_transaction with status transitions"
```

---

### Task 2: Config — Add DatabaseConfig and ServerConfig

**Files:**
- Modify: `consultation-bg-rs/src/sys/config.rs`

- [ ] **Step 1: Rewrite config.rs**

Replace the entire file. Keep `PaymentConfig.secret_key` (hex-encoded Ed25519 seed). Add `DatabaseConfig` (copy pattern from `consultation-rs/src/sys/config.rs`) and `ServerConfig`.

```rust
use common_rs::config::loader::load_conf_from_paths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub payment: PaymentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DatabaseConfig {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub database_name: String,
}

impl DatabaseConfig {
    pub fn connection_url(&self) -> String {
        let encoded_user = urlencoding::encode(&self.user);
        let encoded_password = urlencoding::encode(&self.password);
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            encoded_user, encoded_password, self.host, self.port, self.database_name
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PaymentConfig {
    /// Ed25519 seed as hex string (64 hex chars = 32 bytes) for PASETO v2.public verification
    pub secret_key: String,
}

impl AppConfig {
    pub fn from(paths: &[std::path::PathBuf]) -> anyhow::Result<Self> {
        load_conf_from_paths(paths)
    }
}
```

- [ ] **Step 2: Add `urlencoding` to Cargo.toml**

Add to `consultation-bg-rs/Cargo.toml` under `[dependencies]`:

```toml
urlencoding = "2.1"
```

- [ ] **Step 3: Commit**

```bash
git add consultation-bg-rs/src/sys/config.rs consultation-bg-rs/Cargo.toml
git commit -m "feat(config): add DatabaseConfig and ServerConfig for payment-confirm service"
```

---

### Task 3: Repo — Add payment_tx_id parameter

**Files:**
- Modify: `consultation-bg-rs/src/payment_confirm/repo.rs`

- [ ] **Step 1: Update the repo**

Add `payment_tx_id: i64` parameter to the trait method and implementation. Update the SQL to bind 4 params instead of 3.

Replace the entire file:

```rust
use tdh_protocol::appointment::v2::payment_transaction::PaymentChannels;
use sqlx::{Pool, Postgres};
use tracing::instrument;

#[async_trait::async_trait]
pub trait PaymentConfirmRepo: Send + Sync {
    async fn upsert_payment_transaction(
        &self,
        booking_id: &str,
        payment_tx_id: i64,
        payment_tx_ref_id: &str,
        payment_channels: &PaymentChannels,
    ) -> Result<(), anyhow::Error>;
}

pub struct PaymentConfirmPsql {
    pool: Pool<Postgres>,
}

impl PaymentConfirmPsql {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl PaymentConfirmRepo for PaymentConfirmPsql {
    #[instrument(skip(self, payment_channels))]
    async fn upsert_payment_transaction(
        &self,
        booking_id: &str,
        payment_tx_id: i64,
        payment_tx_ref_id: &str,
        payment_channels: &PaymentChannels,
    ) -> Result<(), anyhow::Error> {
        let payment_channels_json = serde_json::to_value(payment_channels)?;

        sqlx::query(
            r#"
            SELECT v2.upsert_payment_transaction($1, $2, $3, $4)
            "#,
        )
        .bind(booking_id)
        .bind(payment_tx_id)
        .bind(payment_tx_ref_id)
        .bind(&payment_channels_json)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to upsert payment transaction: {}", e))?;

        Ok(())
    }
}
```

Note: The `add_patient_verification` and `cancel_appointment` methods are removed — they are not needed by the payment-confirm flow (those belong to other services in `consultation-rs`).

- [ ] **Step 2: Commit**

```bash
git add consultation-bg-rs/src/payment_confirm/repo.rs
git commit -m "feat(repo): add payment_tx_id to upsert_payment_transaction, remove unused methods"
```

---

### Task 4: Token verifier — PASETO v2.public verification

**Files:**
- Modify: `consultation-bg-rs/src/payment_confirm/payement_token.rs`

The `payment_data` field is a PASETO v2.public token (format: `v2.public.<b64-payload>.<b64-signature>`). The payload contains a JSON claims object with a `"payload"` key holding the `PaymentTransaction` JSON string.

The Ed25519 seed (hex-encoded) is configured via `PaymentConfig.secret_key`. From the seed, derive both the private key (for signing, not needed here) and the public key (for verification).

**Verification steps:**
1. Parse token format: split on `.`, verify header is `v2.public`
2. Base64-url-decode the payload and signature parts
3. Reconstruct the signed message: `PAE("v2.public") || message`
4. Verify Ed25519 signature against the public key
5. Parse the verified payload as JSON, extract the `"payload"` claim string
6. Deserialize the claim string as `PaymentTransaction`

- [ ] **Step 1: Add `ed25519-dalek` to Cargo.toml**

Add to `consultation-bg-rs/Cargo.toml` under `[dependencies]`:

```toml
ed25519-dalek = { version = "2", features = ["default"] }
```

- [ ] **Step 2: Rewrite the token verifier**

Replace the entire file:

```rust
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, SigningKey, Verifier};
use tdh_protocol::payment::payment_transaction::PaymentTransaction;

pub trait PaymentTokenVerifier: Send + Sync {
    fn verify(&self, token: &str) -> Result<PaymentTransaction, anyhow::Error>;
}

/// PASETO v2.public token verifier for payment transactions.
///
/// Verifies Ed25519 signature on the PASETO token, then extracts the
/// `"payload"` claim containing the serialized PaymentTransaction.
pub struct PaymentVerifierWithPaseto {
    public_key: ed25519_dalek::VerifyingKey,
}

impl PaymentVerifierWithPaseto {
    /// Create a verifier from a hex-encoded Ed25519 seed (64 hex chars = 32 bytes).
    /// The public key is derived from the seed.
    pub fn new(hex_secret_key: &str) -> Result<Self, anyhow::Error> {
        let seed_bytes = hex::decode(hex_secret_key)
            .map_err(|e| anyhow::anyhow!("Invalid hex secret key: {}", e))?;

        if seed_bytes.len() != 32 {
            return Err(anyhow::anyhow!(
                "Ed25519 seed must be 32 bytes, got {}",
                seed_bytes.len()
            ));
        }

        let signing_key = SigningKey::from_bytes(
            seed_bytes.as_slice().try_into().unwrap(),
        );
        let public_key = signing_key.verifying_key();

        Ok(Self { public_key })
    }
}

impl PaymentTokenVerifier for PaymentVerifierWithPaseto {
    fn verify(&self, token: &str) -> Result<PaymentTransaction, anyhow::Error> {
        // PASETO v2.public format: v2.public.<payload>.<signature>
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 4 || parts[0] != "v2" || parts[1] != "public" {
            return Err(anyhow::anyhow!(
                "Invalid PASETO v2.public token format"
            ));
        }

        let payload_b64 = parts[2];
        let signature_b64 = parts[3];

        // Decode the payload and signature
        let payload_bytes = URL_SAFE_NO_PAD
            .decode(payload_b64)
            .map_err(|e| anyhow::anyhow!("Failed to decode PASETO payload: {}", e))?;

        let signature_bytes = URL_SAFE_NO_PAD
            .decode(signature_b64)
            .map_err(|e| anyhow::anyhow!("Failed to decode PASETO signature: {}", e))?;

        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid Ed25519 signature: {}", e))?;

        // PASETO v2.public signed message = PAE(header) || payload
        // PAE(pieces) = le64(num_pieces) || le64(len(piece_0)) || piece_0 || ...
        // For v2.public with no footer: PAE("v2.public" || message)
        let header = "v2.public";
        let header_b64 = URL_SAFE_NO_PAD.encode(header.as_bytes());

        // Signed content for PASETO v2.public: header_b64 + "." + payload_b64 (no footer)
        let signed_msg = format!("{}.{}", header_b64, payload_b64);

        self.public_key
            .verify(signed_msg.as_bytes(), &signature)
            .map_err(|e| anyhow::anyhow!("PASETO signature verification failed: {}", e))?;

        // Parse the verified payload as JSON claims
        let payload_json_str = String::from_utf8(payload_bytes)
            .map_err(|e| anyhow::anyhow!("PASETO payload is not valid UTF-8: {}", e))?;

        // The payload is a JSON object with a "payload" claim containing the PaymentTransaction JSON
        let claims: serde_json::Value = serde_json::from_str(&payload_json_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse PASETO claims JSON: {}", e))?;

        let tx_json = claims
            .get("payload")
            .ok_or_else(|| anyhow::anyhow!("PASETO claims missing 'payload' field"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("PASETO 'payload' claim is not a string"))?;

        let tx: PaymentTransaction = serde_json::from_str(tx_json)
            .map_err(|e| anyhow::anyhow!("Failed to parse PaymentTransaction from PASETO payload: {}", e))?;

        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verifier_new_valid_key() {
        // 32 bytes = 64 hex chars
        let hex_key = "a".repeat(64);
        let verifier = PaymentVerifierWithPaseto::new(&hex_key);
        assert!(verifier.is_ok());
    }

    #[test]
    fn test_verifier_new_invalid_hex() {
        let verifier = PaymentVerifierWithPaseto::new("not-hex");
        assert!(verifier.is_err());
    }

    #[test]
    fn test_verifier_new_wrong_length() {
        let hex_key = "ab"; // only 1 byte
        let verifier = PaymentVerifierWithPaseto::new(hex_key);
        assert!(verifier.is_err());
    }

    #[test]
    fn test_verify_invalid_format() {
        let hex_key = "a".repeat(64);
        let verifier = PaymentVerifierWithPaseto::new(&hex_key).unwrap();
        let result = verifier.verify("not.a.valid.token");
        assert!(result.is_err());
    }
}
```

- [ ] **Step 3: Add `hex` to Cargo.toml**

Add to `consultation-bg-rs/Cargo.toml` under `[dependencies]`:

```toml
hex = "0.4"
```

- [ ] **Step 4: Commit**

```bash
git add consultation-bg-rs/src/payment_confirm/payement_token.rs consultation-bg-rs/Cargo.toml
git commit -m "feat(token): PASETO v2.public verification with Ed25519 signature"
```

---

### Task 5: Service — payment_status filter and payment_tx_id

**Files:**
- Modify: `consultation-bg-rs/src/payment_confirm/service.rs`

- [ ] **Step 1: Update the service**

Key changes:
- Add `payment_status` filter: if not `"success"`, log and return Ok
- Pass `tx.payment_transaction_id` as `payment_tx_id` to repo
- Replace string-interpolation logging with structured fields
- Log idempotent replay detection hint via tracing::info

Replace the entire file:

```rust
use std::sync::Arc;

use tdh_protocol::{
    appointment::v2::payment_transaction::{PaymentChannel, PaymentChannels},
    payment::{
        extend_data::ExtendData,
        payment_channel_result::PaymentChannelResult,
        payment_message::PaymentMessage,
        payment_transaction::PaymentTransaction,
        selected_channel_result::SelectedChannelResult,
    },
};

use crate::payment_confirm::{payement_token::PaymentTokenVerifier, repo::PaymentConfirmRepo};

#[derive(thiserror::Error, Debug)]
pub enum PaymentConfirmError {
    #[error("Unexpected extended data type")]
    UnexpectedExtendData,
    #[error("Missing extended data")]
    MissingExtendData,
    #[error("Invalid payment token: {0}")]
    InvalidToken(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] anyhow::Error),
    #[error("Appointment not found or unauthorized")]
    AppointmentNotFound,
}

pub struct PaymentConfirmService {
    payment_verifier: Arc<dyn PaymentTokenVerifier>,
    repo: Arc<dyn PaymentConfirmRepo>,
}

impl PaymentConfirmService {
    pub fn new(
        payment_verifier: Arc<dyn PaymentTokenVerifier>,
        repo: Arc<dyn PaymentConfirmRepo>,
    ) -> Self {
        Self {
            payment_verifier,
            repo,
        }
    }

    /// Handles payment transaction message from PubSub push.
    /// Only processes if payment_status == "success".
    pub async fn handle_payment_tx_v1(
        &self,
        msg: PaymentMessage,
    ) -> Result<(), PaymentConfirmError> {
        // Filter: only process successful payments
        if msg.payment_status != "success" {
            tracing::info!(
                payment_status = %msg.payment_status,
                payment_tx_ref_id = %msg.payment_transaction_ref_id,
                "Payment not successful, skipping confirm"
            );
            return Ok(());
        }

        // Verify the PASETO token and decode PaymentTransaction
        let tx = self
            .payment_verifier
            .verify(&msg.payment_data)
            .map_err(|e| PaymentConfirmError::InvalidToken(e.to_string()))?;

        let booking_id = self.extract_booking_id(&tx)?;
        let payment_channels = self.extract_payment_channels(&tx)?;

        tracing::info!(
            booking_id = %booking_id,
            payment_tx_id = tx.payment_transaction_id,
            payment_tx_ref_id = %tx.payment_transaction_ref_id,
            "Confirming payment for appointment"
        );

        // Store payment transaction in database (idempotent)
        self.repo
            .upsert_payment_transaction(
                &booking_id,
                tx.payment_transaction_id,
                &tx.payment_transaction_ref_id,
                &payment_channels,
            )
            .await?;

        tracing::info!(
            booking_id = %booking_id,
            payment_tx_ref_id = %tx.payment_transaction_ref_id,
            "Payment confirmed successfully"
        );

        Ok(())
    }

    fn extract_booking_id(
        &self,
        tx: &PaymentTransaction,
    ) -> Result<String, PaymentConfirmError> {
        let extend_data = tx
            .payment_summary
            .request
            .extended_data
            .as_ref()
            .ok_or_else(|| PaymentConfirmError::MissingExtendData)?;

        match extend_data.as_ref() {
            ExtendData::ConsultInfo { booking_id, .. } => Ok(booking_id.clone()),
            ExtendData::FollowUp {
                previous_booking_id,
                ..
            } => Ok(previous_booking_id.clone()),
            _ => Err(PaymentConfirmError::UnexpectedExtendData),
        }
    }

    fn extract_payment_channels(
        &self,
        tx: &PaymentTransaction,
    ) -> Result<PaymentChannels, PaymentConfirmError> {
        let Some(ref channel_result) = tx.selected_channel_result else {
            return Ok(vec![]);
        };

        let mut channels = vec![];

        match channel_result {
            SelectedChannelResult::CoverageChannel { channel } => {
                if let Some(c) = Self::channel_result_to_channel(channel.as_ref()) {
                    channels.push(c);
                }
            }
            SelectedChannelResult::SelfPayChannel { channel } => {
                if let Some(c) = Self::channel_result_to_channel(channel.as_ref()) {
                    channels.push(c);
                }
            }
            SelectedChannelResult::CoverageAndSelfPayChannel {
                coverage_channel,
                self_pay_channel,
            } => {
                if let Some(c) = Self::channel_result_to_channel(coverage_channel.as_ref()) {
                    channels.push(c);
                }
                if let Some(c) = Self::channel_result_to_channel(self_pay_channel.as_ref()) {
                    channels.push(c);
                }
            }
        }

        Ok(channels)
    }

    fn channel_result_to_channel(result: &PaymentChannelResult) -> Option<PaymentChannel> {
        match result {
            PaymentChannelResult::InsuranceV3 {
                binding_id,
                privilege_id,
                ..
            } => Some(PaymentChannel::Insurance {
                binding_id: *binding_id,
                privilege_id: *privilege_id,
            }),
            PaymentChannelResult::Insurance { policy_id, .. } => Some(PaymentChannel::Insurance {
                binding_id: *policy_id as i64,
                privilege_id: 0,
            }),
            PaymentChannelResult::InsuranceV2 { binding_id, .. } => {
                Some(PaymentChannel::Insurance {
                    binding_id: *binding_id as i64,
                    privilege_id: 0,
                })
            }
            PaymentChannelResult::EmployeeBenefit { .. }
            | PaymentChannelResult::EmployeeBenefitV2 { .. } => {
                Some(PaymentChannel::EmployeeBenefit {})
            }
            PaymentChannelResult::CampaignLocation { .. } => {
                Some(PaymentChannel::CampaignLocation {})
            }
            PaymentChannelResult::Campaign { .. } => Some(PaymentChannel::Campaign),
            PaymentChannelResult::Card { id, .. } => Some(PaymentChannel::Card { id: id.clone() }),
            PaymentChannelResult::PromptPay { id, .. } => {
                Some(PaymentChannel::PromptPay { id: id.clone() })
            }
            PaymentChannelResult::TrueMoney { id, .. } => {
                Some(PaymentChannel::TrueMoney { id: id.clone() })
            }
            PaymentChannelResult::Wallet { .. } => None,
        }
    }
}
```

Note: The `ExtendData` enum fields are private in tdh-protocol. The code uses `match extend_data.as_ref()` with `ExtendData::ConsultInfo { booking_id, .. }` — if the fields are private, use the pattern already in the existing code which accesses via `tdh_protocol::payment::extend_data::ExtendData::ConsultInfo`. The subagent should check field visibility and adjust accordingly.

- [ ] **Step 2: Commit**

```bash
git add consultation-bg-rs/src/payment_confirm/service.rs
git commit -m "feat(service): add payment_status filter, payment_tx_id, structured logging"
```

---

### Task 6: Handler — Axum POST endpoint for PubSub push

**Files:**
- Create: `consultation-bg-rs/src/payment_confirm/handler.rs`

- [ ] **Step 1: Create the handler**

The handler receives a `PubSubPushMessage`, extracts the `PaymentMessage` from the base64-encoded `data` field, and calls the service. It also extracts the `googclient_traceparent` attribute for GCP trace context propagation.

```rust
use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;
use tdh_protocol::gcp::pubsub_push_message::PubSubPushMessage;
use tdh_protocol::payment::payment_message::PaymentMessage;
use tracing::Instrument;

use super::service::{PaymentConfirmError, PaymentConfirmService};

pub(crate) struct AppState {
    pub service: Arc<PaymentConfirmService>,
}

pub(crate) async fn handle_pubsub_push(
    State(state): State<Arc<AppState>>,
    Json(push_msg): Json<PubSubPushMessage>,
) -> StatusCode {
    let message_id = push_msg.message.message_id.clone();

    // Extract GCP PubSub trace context for log correlation
    let traceparent = push_msg
        .message
        .attributes
        .as_ref()
        .and_then(|attrs| attrs.get("googclient_traceparent"))
        .cloned();

    // Create a span with message_id and optional trace context
    let span = tracing::info_span!(
        "pubsub_payment_confirm",
        message_id = %message_id,
        traceparent = traceparent.as_deref().unwrap_or(""),
    );

    async move {
        tracing::info!("Received PubSub push message");

        // Decode base64 data into PaymentMessage
        let payment_msg: PaymentMessage = match push_msg.read_data() {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!(error = %e, "Failed to decode PubSub message data");
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        };

        tracing::info!(
            payment_status = %payment_msg.payment_status,
            payment_tx_ref_id = %payment_msg.payment_transaction_ref_id,
            "Processing payment message"
        );

        match state.service.handle_payment_tx_v1(payment_msg).await {
            Ok(()) => {
                tracing::info!("Payment confirm processed successfully");
                StatusCode::OK
            }
            Err(PaymentConfirmError::DatabaseError(e)) => {
                tracing::error!(error = %e, "Database error during payment confirm");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Err(e) => {
                tracing::error!(error = %e, "Payment confirm failed");
                // Return 500 to trigger PubSub retry for unexpected errors
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
    .instrument(span)
    .await
}
```

- [ ] **Step 2: Update mod.rs to export handler**

Replace `consultation-bg-rs/src/payment_confirm/mod.rs`:

```rust
pub(crate) mod handler;
pub(crate) mod payement_token;
pub(crate) mod repo;
pub(crate) mod service;
```

- [ ] **Step 3: Commit**

```bash
git add consultation-bg-rs/src/payment_confirm/handler.rs consultation-bg-rs/src/payment_confirm/mod.rs
git commit -m "feat(handler): axum POST endpoint for PubSub payment-confirm push"
```

---

### Task 7: Main.rs — Wire everything together

**Files:**
- Modify: `consultation-bg-rs/src/main.rs`

- [ ] **Step 1: Add missing dependencies to Cargo.toml**

Add to `consultation-bg-rs/Cargo.toml` under `[dependencies]`:

```toml
axum = { workspace = true }
clap = { workspace = true }
tokio = { workspace = true }
tracing-subscriber = { workspace = true }
urlencoding = "2.1"
ed25519-dalek = { version = "2", features = ["default"] }
hex = "0.4"
```

- [ ] **Step 2: Rewrite main.rs**

```rust
use std::sync::Arc;

use axum::{routing::post, Router};
use clap::Parser;

mod payment_confirm;
mod sys;

use payment_confirm::{
    handler::{self, AppState},
    payement_token::PaymentVerifierWithPaseto,
    repo::PaymentConfirmPsql,
    service::PaymentConfirmService,
};
use sys::config::AppConfig;

#[derive(Parser)]
#[command(author, version, about = "Consultation Background Service")]
struct Args {
    #[arg(short, long, value_name = "FILE")]
    config_path: Vec<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env(),
        )
        .init();

    let args = Args::try_parse()?;
    let config = AppConfig::from(&args.config_path)?;

    tracing::info!(
        host = %config.server.host,
        port = config.server.port,
        "Starting consultation-bg-rs service"
    );

    // Database pool
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.connection_url())
        .await?;
    tracing::info!("Database connected");

    // Build service
    let verifier = PaymentVerifierWithPaseto::new(&config.payment.secret_key)?;
    let repo = PaymentConfirmPsql::new(pool);
    let service = Arc::new(PaymentConfirmService::new(
        Arc::new(verifier),
        Arc::new(repo),
    ));

    let state = Arc::new(AppState { service });

    // Router
    let app = Router::new()
        .route("/pubsub/payment-confirm", post(handler::handle_pubsub_push))
        .with_state(state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
```

- [ ] **Step 3: Verify compilation**

```bash
cd /Users/peelz/Workspace/doctor-apm/tdh-biz-doctor-apmv2 && cargo check -p consultation-bg-rs
```

Expected: Compiles with no errors. Fix any compilation issues before committing.

- [ ] **Step 4: Commit**

```bash
git add consultation-bg-rs/src/main.rs consultation-bg-rs/Cargo.toml
git commit -m "feat(main): wire axum router with payment-confirm service"
```

---

### Task 8: Update `sys/mod.rs` to make config public

**Files:**
- Modify: `consultation-bg-rs/src/sys/mod.rs`

- [ ] **Step 1: Update visibility**

The config types need to be accessible from `main.rs`. Replace the file:

```rust
pub(crate) mod config;
```

No change needed — it's already `pub(crate)`. The `config` module and its types are already declared `pub(crate)` in `config.rs`. Verify this compiles.

- [ ] **Step 2: Commit (only if changes were made)**

Only commit if the file was actually modified.

---

### Task 9: Final compilation check and cleanup

- [ ] **Step 1: Full compile check**

```bash
cd /Users/peelz/Workspace/doctor-apm/tdh-biz-doctor-apmv2 && cargo check -p consultation-bg-rs 2>&1
```

Expected: Compiles successfully. Fix any remaining issues.

- [ ] **Step 2: Verify the ExtendData field visibility**

Check if `ExtendData` enum fields (like `booking_id` in `ConsultInfo`) are public or private. If private, the service code needs adjustment — use the fully-qualified path pattern `tdh_protocol::payment::extend_data::ExtendData::ConsultInfo { booking_id, .. }` as in the original code.

```bash
grep -n 'pub.*booking_id\|booking_id.*pub' /Users/peelz/.cargo/git/checkouts/tdh-protocol-14d80954e7f489fd/39dd376/rust/src/payment/extend_data.rs
```

If fields are private (no `pub`), the service match arm should work because Rust allows pattern matching on private fields within the same module or through destructuring in match arms when the type is visible. But if the compiler rejects it, use getter methods or `pub` re-exports.

- [ ] **Step 3: Commit any fixes**

```bash
git add -A && git commit -m "fix: resolve compilation issues for payment-confirm module"
```

---

## Manual Postgres-backed repo test

The ignored concurrency/idempotency test
`payment_confirm::repo::tests::enqueue_consultation_booked_event_is_idempotent_under_concurrent_calls`
requires `TEST_DATABASE_URL` to point at a disposable, migrated Biz APM Postgres database
with the `v2` schema and `v2.event_outbox` available. Do not run it against shared or production data.

```bash
TEST_DATABASE_URL='postgres://user:password@localhost:5432/biz_apm_test' cargo test -p consultation-bg-rs payment_confirm::repo::tests::enqueue_consultation_booked_event_is_idempotent_under_concurrent_calls -- --ignored --exact --nocapture
```

The test creates a unique `test-payment-confirm-<uuid>` booking aggregate and cleans only
`v2.event_outbox` rows where `aggregate_id` is that booking id and `event_type = 'ConsultationBooked'`.
