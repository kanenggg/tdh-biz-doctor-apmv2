use google_cloud_kms_v1::client::KeyManagementService;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KmsError {
    #[error("Google Cloud KMS error: {0}")]
    GoogleCloud(#[from] google_cloud_kms_v1::Error),
    #[error("KMS client builder error: {0}")]
    ClientBuilderError(#[from] google_cloud_gax::client_builder::Error),
    #[error("Invalid key name: {0}")]
    InvalidKeyName(String),
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Invalid key hex: {0}")]
    InvalidKeyHex(String),
}

pub type KmsResult<T> = Result<T, KmsError>;

#[async_trait::async_trait]
pub trait Kms: Send + Sync {
    async fn encrypt(&self, plaintext: &[u8], key_name: &str) -> KmsResult<Vec<u8>>;
    async fn decrypt(&self, ciphertext: &[u8], key_name: &str) -> KmsResult<Vec<u8>>;
}

pub struct GcpKmsService {
    client: KeyManagementService,
}

impl GcpKmsService {
    pub async fn new() -> KmsResult<Self> {
        let client = KeyManagementService::builder().build().await?;
        Ok(Self { client })
    }

    pub fn with_client(client: KeyManagementService) -> Self {
        Self { client }
    }

    pub async fn from_config_with_endpoint(endpoint: String) -> KmsResult<Self> {
        let client = KeyManagementService::builder()
            .with_endpoint(endpoint)
            .build()
            .await?;
        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl Kms for GcpKmsService {
    async fn encrypt(&self, plaintext: &[u8], key_name: &str) -> KmsResult<Vec<u8>> {
        let request_builder = self
            .client
            .encrypt()
            .set_name(key_name.to_string())
            .set_plaintext(bytes::Bytes::copy_from_slice(plaintext));

        let response = request_builder
            .send()
            .await
            .map_err(|e| KmsError::EncryptionFailed(e.to_string()))?;

        Ok(response.ciphertext.to_vec())
    }

    async fn decrypt(&self, ciphertext: &[u8], key_name: &str) -> KmsResult<Vec<u8>> {
        let request_builder = self
            .client
            .decrypt()
            .set_name(key_name.to_string())
            .set_ciphertext(bytes::Bytes::copy_from_slice(ciphertext));

        let response = request_builder
            .send()
            .await
            .map_err(|e| KmsError::DecryptionFailed(e.to_string()))?;

        Ok(response.plaintext.to_vec())
    }
}

#[async_trait::async_trait]
impl common_rs::encrypted_string::KmsProvider for GcpKmsService {
    type Error = KmsError;

    async fn encrypt(&self, plaintext: &[u8], key_name: &str) -> Result<Vec<u8>, Self::Error> {
        <Self as Kms>::encrypt(self, plaintext, key_name).await
    }

    async fn decrypt(&self, ciphertext: &[u8], key_name: &str) -> Result<Vec<u8>, Self::Error> {
        <Self as Kms>::decrypt(self, ciphertext, key_name).await
    }
}
