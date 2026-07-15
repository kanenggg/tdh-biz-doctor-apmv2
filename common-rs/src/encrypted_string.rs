use std::marker::PhantomData;

use base64::Engine;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

/// Minimal byte-oriented KMS contract used by [`Encrypted`].
///
/// This intentionally does not depend on a concrete GCP KMS client so the
/// wrapper can live in `common-rs`. Production code can implement this trait for
/// its GCP KMS adapter, while tests can use a small fake implementation.
#[async_trait::async_trait]
pub trait KmsProvider: Send + Sync {
    type Error: std::fmt::Display + Send + Sync + 'static;

    async fn encrypt(&self, plaintext: &[u8], key_name: &str) -> Result<Vec<u8>, Self::Error>;

    async fn decrypt(&self, ciphertext: &[u8], key_name: &str) -> Result<Vec<u8>, Self::Error>;
}

#[derive(Debug, Error)]
pub enum EncryptedStringError {
    #[error("failed to serialize plaintext JSON: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("failed to deserialize decrypted JSON: {0}")]
    Deserialize(#[source] serde_json::Error),
    #[error("invalid base64 ciphertext: {0}")]
    InvalidBase64(#[source] base64::DecodeError),
    #[error("KMS encrypt failed: {0}")]
    KmsEncrypt(String),
    #[error("KMS decrypt failed: {0}")]
    KmsDecrypt(String),
}

/// Encrypted JSON payload for type `T`.
///
/// The stored value is a base64/base64url encoded ciphertext produced by KMS.
/// `T` is only a marker: serialization of this wrapper never exposes plaintext.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Encrypted<T> {
    /// Ciphertext, usually base64/base64url encoded.
    #[serde(rename = "value")]
    pub ciphertext: String,

    #[serde(skip)]
    _marker: PhantomData<T>,
}

pub type EncryptedString<T> = Encrypted<T>;

impl<T> Encrypted<T> {
    pub fn new(ciphertext: impl Into<String>) -> Self {
        Self {
            ciphertext: ciphertext.into(),
            _marker: PhantomData,
        }
    }

    pub fn ciphertext(&self) -> &str {
        &self.ciphertext
    }

    pub fn into_ciphertext(self) -> String {
        self.ciphertext
    }

    pub fn decode_ciphertext(&self) -> Result<Vec<u8>, EncryptedStringError> {
        decode_base64(&self.ciphertext).map_err(EncryptedStringError::InvalidBase64)
    }

    pub fn retag<U>(self) -> Encrypted<U> {
        Encrypted::new(self.ciphertext)
    }
}

impl<T> Encrypted<T>
where
    T: Serialize,
{
    /// Serialize `value` as JSON, encrypt bytes with KMS, and store ciphertext as
    /// base64 STANDARD text.
    pub async fn encrypt_json<K>(
        value: &T,
        kms: &K,
        key_name: &str,
    ) -> Result<Self, EncryptedStringError>
    where
        K: KmsProvider + ?Sized,
    {
        let plaintext = serde_json::to_vec(value).map_err(EncryptedStringError::Serialize)?;
        let ciphertext = kms
            .encrypt(&plaintext, key_name)
            .await
            .map_err(|e| EncryptedStringError::KmsEncrypt(e.to_string()))?;

        Ok(Self::new(
            base64::engine::general_purpose::STANDARD.encode(ciphertext),
        ))
    }
}

impl<T> Encrypted<T>
where
    T: DeserializeOwned,
{
    /// Base64-decode ciphertext, decrypt bytes with KMS, and deserialize JSON as
    /// `T`.
    pub async fn decrypt_json<K>(&self, kms: &K, key_name: &str) -> Result<T, EncryptedStringError>
    where
        K: KmsProvider + ?Sized,
    {
        let ciphertext = self.decode_ciphertext()?;
        let plaintext = kms
            .decrypt(&ciphertext, key_name)
            .await
            .map_err(|e| EncryptedStringError::KmsDecrypt(e.to_string()))?;

        serde_json::from_slice(&plaintext).map_err(EncryptedStringError::Deserialize)
    }
}

fn decode_base64(value: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(value))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Payload {
        name: String,
        count: i32,
    }

    struct IdentityKms;

    #[async_trait::async_trait]
    impl KmsProvider for IdentityKms {
        type Error = String;

        async fn encrypt(&self, plaintext: &[u8], _key_name: &str) -> Result<Vec<u8>, Self::Error> {
            Ok(plaintext.to_vec())
        }

        async fn decrypt(
            &self,
            ciphertext: &[u8],
            _key_name: &str,
        ) -> Result<Vec<u8>, Self::Error> {
            Ok(ciphertext.to_vec())
        }
    }

    #[tokio::test]
    async fn encrypt_json_stores_only_base64_ciphertext_and_decrypts_to_type() {
        let payload = Payload {
            name: "alice".to_string(),
            count: 7,
        };

        let encrypted = Encrypted::<Payload>::encrypt_json(&payload, &IdentityKms, "key")
            .await
            .expect("encrypt should succeed");
        let json = serde_json::to_string(&encrypted).expect("wrapper should serialize");

        assert!(json.contains("value"));
        assert!(!json.contains("alice"));

        let decrypted = encrypted
            .decrypt_json(&IdentityKms, "key")
            .await
            .expect("decrypt should succeed");

        assert_eq!(decrypted, payload);
    }

    #[tokio::test]
    async fn decrypt_json_accepts_url_safe_base64_without_padding() {
        let payload = Payload {
            name: "bob".to_string(),
            count: 3,
        };
        let plaintext = serde_json::to_vec(&payload).unwrap();
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(plaintext);
        let encrypted = Encrypted::<Payload>::new(encoded);

        let decrypted = encrypted.decrypt_json(&IdentityKms, "key").await.unwrap();

        assert_eq!(decrypted, payload);
    }

    #[tokio::test]
    async fn decrypt_json_reports_invalid_json_after_successful_kms_decrypt() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"not-json");
        let encrypted = Encrypted::<Payload>::new(encoded);

        let err = encrypted
            .decrypt_json(&IdentityKms, "key")
            .await
            .unwrap_err();

        assert!(matches!(err, EncryptedStringError::Deserialize(_)));
    }
}
