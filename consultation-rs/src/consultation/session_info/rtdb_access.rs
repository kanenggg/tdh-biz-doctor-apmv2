use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use common_rs::tdh_protocol::consultation::v2::session_info::RtdbAccess;
use google_cloud_auth::credentials::{AccessTokenCredentials, Builder};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::sys::config::RtdbAccessConfig;

const FIREBASE_CUSTOM_TOKEN_AUDIENCE: &str =
    "https://identitytoolkit.googleapis.com/google.identity.identitytoolkit.v1.IdentityToolkit";
const CLOUD_PLATFORM_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const MAX_FIREBASE_CUSTOM_TOKEN_TTL_SECONDS: i64 = 3600;

#[async_trait]
pub trait BlobSigner: Send + Sync {
    async fn sign(&self, payload: &str) -> Result<Vec<u8>>;
}

#[async_trait]
pub trait AccessTokenSource: Send + Sync {
    async fn token(&self) -> Result<String>;
}

#[derive(Default)]
struct AdcAccessTokenSource {
    credentials: OnceCell<AccessTokenCredentials>,
}

#[async_trait]
impl AccessTokenSource for AdcAccessTokenSource {
    async fn token(&self) -> Result<String> {
        let credentials = self
            .credentials
            .get_or_try_init(|| async {
                Builder::default()
                    .with_scopes([CLOUD_PLATFORM_SCOPE])
                    .build_access_token_credentials()
                    .context("initialize Application Default Credentials")
            })
            .await?;
        let token = credentials
            .access_token()
            .await
            .context("obtain Application Default Credentials access token")?;
        Ok(token.token)
    }
}

struct IamCredentialsBlobSigner {
    http: reqwest::Client,
    token_source: Arc<dyn AccessTokenSource>,
    service_account_email: String,
}

#[derive(Serialize)]
struct SignBlobRequest {
    payload: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignBlobResponse {
    signed_blob: String,
}

#[async_trait]
impl BlobSigner for IamCredentialsBlobSigner {
    async fn sign(&self, payload: &str) -> Result<Vec<u8>> {
        let access_token = self.token_source.token().await?;
        let email = self.service_account_email.trim();
        if email.is_empty() {
            bail!("RTDB signing service account email is not configured");
        }
        let url = format!(
            "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/{}:signBlob",
            urlencoding::encode(email)
        );
        let response = self
            .http
            .post(url)
            .bearer_auth(access_token)
            .json(&SignBlobRequest {
                payload: STANDARD.encode(payload.as_bytes()),
            })
            .send()
            .await
            .context("call IAM Credentials signBlob")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("IAM Credentials signBlob returned {status}: {body}");
        }
        let response = response
            .json::<SignBlobResponse>()
            .await
            .context("decode IAM Credentials signBlob response")?;
        STANDARD
            .decode(response.signed_blob)
            .context("decode IAM Credentials signature")
    }
}

#[derive(Clone)]
pub struct RtdbCustomTokenIssuer {
    service_account_email: String,
    path_prefix: String,
    ttl_seconds: i64,
    signer: Option<Arc<dyn BlobSigner>>,
}

impl RtdbCustomTokenIssuer {
    pub fn from_config(config: &RtdbAccessConfig) -> Self {
        let email = config.signing_service_account_email.trim().to_string();
        let path_prefix = normalize_path_prefix(&config.path_prefix);
        let ttl_seconds = validate_ttl(config.token_ttl_seconds);
        let signer = (!email.is_empty()).then(|| {
            Arc::new(IamCredentialsBlobSigner {
                http: reqwest::Client::builder()
                    .connect_timeout(Duration::from_secs(5))
                    .timeout(Duration::from_secs(10))
                    .build()
                    .expect("construct IAM Credentials HTTP client"),
                token_source: Arc::new(AdcAccessTokenSource::default()),
                service_account_email: email.clone(),
            }) as Arc<dyn BlobSigner>
        });
        Self {
            service_account_email: email,
            path_prefix,
            ttl_seconds,
            signer,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.signer.is_some()
    }

    pub async fn issue_for_patient(
        &self,
        booking_id: &str,
        tenant_id: u32,
        patient_account_id: u64,
        patient_profile_id: u64,
        appointment_end_epoch: i64,
        now_epoch: i64,
    ) -> Result<Option<RtdbAccess>> {
        let Some(signer) = &self.signer else {
            return Ok(None);
        };
        if booking_id.trim().is_empty() {
            bail!("RTDB token requires a booking ID");
        }
        if !is_valid_booking_id(booking_id) {
            bail!("invalid RTDB booking ID");
        }
        let path = format!("{}/{}", self.path_prefix, booking_id);
        let expires_at = (now_epoch + self.ttl_seconds).min(appointment_end_epoch);
        if expires_at <= now_epoch {
            bail!("RTDB token access window has ended");
        }
        let claims = FirebaseCustomTokenClaims {
            iss: self.service_account_email.clone(),
            sub: self.service_account_email.clone(),
            aud: FIREBASE_CUSTOM_TOKEN_AUDIENCE,
            iat: now_epoch,
            exp: expires_at,
            uid: format!("patient:{tenant_id}:{patient_account_id}:{patient_profile_id}"),
            claims: PatientRtdbClaims {
                tenant_id,
                account_id: patient_account_id,
                profile_id: patient_profile_id,
                booking_id: booking_id.to_string(),
                rtdb_path: path.clone(),
                role: "patient".to_string(),
            },
        };
        let header = serde_json::json!({"alg": "RS256", "typ": "JWT"});
        let signing_input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header)?),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims)?)
        );
        let signature = signer.sign(&signing_input).await?;
        Ok(Some(RtdbAccess {
            custom_token: format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(signature)),
            expires_at,
            path,
        }))
    }

    #[cfg(test)]
    fn for_test(service_account_email: &str, path_prefix: &str, ttl_seconds: i64) -> Self {
        Self {
            service_account_email: service_account_email.to_string(),
            path_prefix: normalize_path_prefix(path_prefix),
            ttl_seconds: validate_ttl(ttl_seconds),
            signer: Some(Arc::new(FixedTestSigner)),
        }
    }
}

#[derive(Serialize)]
struct FirebaseCustomTokenClaims<'a> {
    iss: String,
    sub: String,
    aud: &'a str,
    iat: i64,
    exp: i64,
    uid: String,
    claims: PatientRtdbClaims,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PatientRtdbClaims {
    tenant_id: u32,
    account_id: u64,
    profile_id: u64,
    booking_id: String,
    rtdb_path: String,
    role: String,
}

fn normalize_path_prefix(value: &str) -> String {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() {
        "consultations".to_string()
    } else {
        trimmed.to_string()
    }
}

fn is_valid_booking_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn validate_ttl(value: i64) -> i64 {
    value.clamp(1, MAX_FIREBASE_CUSTOM_TOKEN_TTL_SECONDS)
}

#[cfg(test)]
struct FixedTestSigner;

#[cfg(test)]
#[async_trait]
impl BlobSigner for FixedTestSigner {
    async fn sign(&self, _payload: &str) -> Result<Vec<u8>> {
        Ok(vec![1, 2, 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn custom_token_scopes_patient_to_one_consultation_path_and_expiry() {
        let issuer = RtdbCustomTokenIssuer::for_test(
            "rtdb-signer@example.iam.gserviceaccount.com",
            "consultations",
            300,
        );

        let token = issuer
            .issue_for_patient("booking-123", 9, 17, 42, 1_700_001_000, 1_700_000_000)
            .await
            .expect("custom token should be issued")
            .expect("test issuer is enabled");

        assert_eq!(token.path, "consultations/booking-123");
        assert_eq!(token.expires_at, 1_700_000_300);
        let payload = token.custom_token.split('.').nth(1).unwrap();
        let payload = URL_SAFE_NO_PAD.decode(payload).unwrap();
        let claims: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(claims["iss"], "rtdb-signer@example.iam.gserviceaccount.com");
        assert_eq!(claims["sub"], "rtdb-signer@example.iam.gserviceaccount.com");
        assert_eq!(
            claims["aud"],
            "https://identitytoolkit.googleapis.com/google.identity.identitytoolkit.v1.IdentityToolkit"
        );
        assert_eq!(claims["uid"], "patient:9:17:42");
        assert_eq!(claims["claims"]["tenantId"], 9);
        assert_eq!(claims["claims"]["accountId"], 17);
        assert_eq!(claims["claims"]["profileId"], 42);
        assert_eq!(claims["claims"]["bookingId"], "booking-123");
        assert_eq!(claims["claims"]["rtdbPath"], "consultations/booking-123");
        assert_eq!(claims["exp"], 1_700_000_300);
    }

    #[tokio::test]
    async fn caps_token_expiry_at_appointment_end() {
        let issuer = RtdbCustomTokenIssuer::for_test(
            "rtdb-signer@example.iam.gserviceaccount.com",
            "consultations",
            300,
        );

        let token = issuer
            .issue_for_patient("booking-123", 9, 17, 42, 1_700_000_100, 1_700_000_000)
            .await
            .expect("custom token should be issued")
            .expect("test issuer is enabled");
        assert_eq!(token.expires_at, 1_700_000_100);
    }

    #[tokio::test]
    async fn refuses_access_after_appointment_end() {
        let issuer = RtdbCustomTokenIssuer::for_test(
            "rtdb-signer@example.iam.gserviceaccount.com",
            "consultations",
            300,
        );

        let error = issuer
            .issue_for_patient("booking-123", 9, 17, 42, 1_700_000_000, 1_700_000_000)
            .await
            .expect_err("ended appointment must not receive an RTDB token");
        assert!(error.to_string().contains("access window has ended"));
    }

    #[tokio::test]
    async fn rejects_booking_ids_that_escape_the_authorized_rtdb_path() {
        let issuer = RtdbCustomTokenIssuer::for_test(
            "rtdb-signer@example.iam.gserviceaccount.com",
            "consultations",
            300,
        );

        let error = issuer
            .issue_for_patient(
                "booking-123/other-patient",
                9,
                17,
                42,
                1_700_001_000,
                1_700_000_000,
            )
            .await
            .expect_err("RTDB path traversal must be rejected");
        assert!(error.to_string().contains("invalid RTDB booking ID"));
    }

    #[test]
    fn disabled_config_never_issues_client_access() {
        let issuer = RtdbCustomTokenIssuer::from_config(&RtdbAccessConfig::default());
        assert!(!issuer.is_enabled());
    }
}
