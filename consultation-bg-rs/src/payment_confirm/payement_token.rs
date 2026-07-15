use crate::common::tdh_protocol::payment::payment_transaction::PaymentTransaction;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use pasetors::{
    keys::AsymmetricPublicKey,
    token::{Public, UntrustedToken},
    version2::{PublicToken, V2},
};

pub trait PaymentTokenVerifier: Send + Sync {
    fn verify(&self, token: &str) -> Result<PaymentTransaction, anyhow::Error>;
}

/// PASETO v2.public token verifier for payment transactions.
///
/// Verifies Ed25519 signature on the PASETO token, then extracts the
/// `"payload"` claim containing the serialized PaymentTransaction.
pub struct PaymentVerifierWithPaseto {
    public_key: AsymmetricPublicKey<V2>,
}

impl PaymentVerifierWithPaseto {
    /// Create a verifier from a hex-encoded or OpenSSH Ed25519 public key.
    pub fn new(public_key_input: &str) -> Result<Self, anyhow::Error> {
        let public_key_bytes = parse_public_key(public_key_input)?;

        let public_key = AsymmetricPublicKey::<V2>::from(&public_key_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid Ed25519 public key: {}", e))?;

        Ok(Self { public_key })
    }
}

fn parse_public_key(input: &str) -> Result<[u8; 32], anyhow::Error> {
    let input = input.trim();

    match hex::decode(input) {
        Ok(public_key_bytes) => public_key_array(public_key_bytes),
        Err(hex_error) => parse_openssh_public_key(input)
            .map_err(|openssh_error| anyhow::anyhow!("{hex_error}; {openssh_error}")),
    }
}

fn public_key_array(public_key_bytes: Vec<u8>) -> Result<[u8; 32], anyhow::Error> {
    let public_key_len = public_key_bytes.len();
    public_key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Ed25519 public key must be 32 bytes, got {public_key_len}"))
}

fn parse_openssh_public_key(input: &str) -> Result<[u8; 32], anyhow::Error> {
    let encoded_blob = match input.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["ssh-ed25519", blob, ..] => *blob,
        [blob] => *blob,
        _ => return Err(anyhow::anyhow!("Invalid OpenSSH Ed25519 public key format")),
    };

    let blob = STANDARD
        .decode(encoded_blob)
        .map_err(|e| anyhow::anyhow!("Invalid OpenSSH Ed25519 public key blob: {e}"))?;

    let mut cursor = 0;
    let key_type = read_ssh_string(&blob, &mut cursor)?;
    if key_type != b"ssh-ed25519" {
        let key_type = String::from_utf8_lossy(key_type);
        return Err(anyhow::anyhow!("Unsupported OpenSSH key type: {key_type}"));
    }

    let public_key_bytes = read_ssh_string(&blob, &mut cursor)?;
    public_key_array(public_key_bytes.to_vec())
}

fn read_ssh_string<'a>(blob: &'a [u8], cursor: &mut usize) -> Result<&'a [u8], anyhow::Error> {
    let len_bytes = blob
        .get(*cursor..*cursor + 4)
        .ok_or_else(|| anyhow::anyhow!("Truncated OpenSSH public key blob"))?;
    let len = u32::from_be_bytes(len_bytes.try_into()?) as usize;
    *cursor += 4;

    let value = blob
        .get(*cursor..*cursor + len)
        .ok_or_else(|| anyhow::anyhow!("Truncated OpenSSH public key blob"))?;
    *cursor += len;

    Ok(value)
}

impl PaymentTokenVerifier for PaymentVerifierWithPaseto {
    fn verify(&self, token: &str) -> Result<PaymentTransaction, anyhow::Error> {
        let untrusted_token = UntrustedToken::<Public, V2>::try_from(token)
            .map_err(|e| anyhow::anyhow!("Invalid PASETO v2.public token: {}", e))?;

        let trusted_token = PublicToken::verify(&self.public_key, &untrusted_token, None)
            .map_err(|e| anyhow::anyhow!("PASETO signature verification failed: {}", e))?;

        // The payload is a JSON object with a "payload" claim containing the PaymentTransaction JSON
        let claims: serde_json::Value = serde_json::from_str(trusted_token.payload())
            .map_err(|e| anyhow::anyhow!("Failed to parse PASETO claims JSON: {}", e))?;

        let tx_json = claims
            .get("payload")
            .ok_or_else(|| anyhow::anyhow!("PASETO claims missing 'payload' field"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("PASETO 'payload' claim is not a string"))?;

        let tx: PaymentTransaction = serde_json::from_str(tx_json).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse PaymentTransaction from PASETO payload: {}",
                e
            )
        })?;

        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::tdh_protocol::payment::extend_data::ExtendData;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::{Signer, SigningKey};

    fn pae(pieces: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(pieces.len() as u64).to_le_bytes());
        for piece in pieces {
            out.extend_from_slice(&(piece.len() as u64).to_le_bytes());
            out.extend_from_slice(piece);
        }
        out
    }

    fn sign_v2_public_token(signing_key: &SigningKey, payload: &[u8], footer: &[u8]) -> String {
        let signed_payload = pae(&[b"v2.public.", payload, footer]);
        let signature = signing_key.sign(&signed_payload);
        let mut body = payload.to_vec();
        body.extend_from_slice(&signature.to_bytes());

        format!(
            "v2.public.{}.{}",
            URL_SAFE_NO_PAD.encode(body),
            URL_SAFE_NO_PAD.encode(footer)
        )
    }

    fn payment_transaction_json() -> serde_json::Value {
        serde_json::json!({
            "paymentTransactionId": 8926,
            "paymentTransactionRefId": "982ef8c1-fae8-46d6-a5fc-a69e295a5e61",
            "ackRefKey": "SG-20260521-171823-D6IC5E",
            "paymentSummary": {
                "header": {
                    "bizUnitId": 1,
                    "bizCenterId": 1,
                    "flowId": 1
                },
                "request": {
                    "moduleId": 1,
                    "refCode": "SG-20260521-171823-D6IC5E",
                    "amount": "525",
                    "currency": "thb",
                    "pricePlanId": 1,
                    "isRequireDelivery": false,
                    "extendedData": {
                        "__type": "ExtendData.ConsultInfoV2",
                        "bookingId": "BK-TEST-20260714-000001",
                        "doctorId": "2002",
                        "departmentId": 1,
                        "clinicIds": [1],
                        "medicalSpecialtyId": 1,
                        "channel": "video",
                        "scheduleTime": 1779383911,
                        "doctorOriginalFee": 350,
                        "platformFee": {"__type": "PlatformFee.Amount", "amount": 0},
                        "onlySelfPayChannelShown": false
                    }
                }
            },
            "amount": "525",
            "orderTotal": "525",
            "orderGrandTotal": "525",
            "platformFee": "0",
            "selectedChannelResult": null,
            "deliveryInfoV2": null,
            "couponProtocol": null,
            "paymentStatus": "success",
            "createdAt": 1779383911,
            "modifiedAt": 1779383911
        })
    }

    fn public_key_hex() -> String {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        hex::encode(signing_key.verifying_key().to_bytes())
    }

    fn public_key_openssh_blob() -> String {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let public_key = signing_key.verifying_key();
        let mut blob = Vec::new();
        blob.extend_from_slice(&(b"ssh-ed25519".len() as u32).to_be_bytes());
        blob.extend_from_slice(b"ssh-ed25519");
        blob.extend_from_slice(&(public_key.to_bytes().len() as u32).to_be_bytes());
        blob.extend_from_slice(&public_key.to_bytes());
        base64::engine::general_purpose::STANDARD.encode(blob)
    }

    #[test]
    fn test_verifier_new_valid_key() {
        let hex_key = public_key_hex();
        let verifier = PaymentVerifierWithPaseto::new(&hex_key);
        assert!(verifier.is_ok());
    }

    #[test]
    fn verifier_new_uses_public_key_bytes_directly() {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let public_key = signing_key.verifying_key();
        let public_key_hex = hex::encode(public_key.to_bytes());

        let verifier = PaymentVerifierWithPaseto::new(&public_key_hex).unwrap();

        assert_eq!(verifier.public_key.as_bytes(), public_key.to_bytes());
    }

    #[test]
    fn verifier_new_accepts_openssh_public_key_blob() {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let public_key = signing_key.verifying_key();
        let public_key_blob = public_key_openssh_blob();

        let verifier = PaymentVerifierWithPaseto::new(&public_key_blob).unwrap();

        assert_eq!(verifier.public_key.as_bytes(), public_key.to_bytes());
    }

    #[test]
    fn test_verifier_new_invalid_hex() {
        let verifier = PaymentVerifierWithPaseto::new("not-hex");
        assert!(verifier.is_err());
    }

    #[test]
    fn test_verifier_new_wrong_length() {
        let hex_key = "ab";
        let verifier = PaymentVerifierWithPaseto::new(hex_key);
        assert!(verifier.is_err());
    }

    #[test]
    fn test_verify_invalid_format() {
        let hex_key = public_key_hex();
        let verifier = PaymentVerifierWithPaseto::new(&hex_key).unwrap();
        let result = verifier.verify("not.a.valid.token");
        assert!(result.is_err());
    }

    #[test]
    fn verify_accepts_paseto_v2_public_with_footer() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let tx_json = payment_transaction_json();
        let claims = serde_json::json!({ "payload": tx_json.to_string() });
        let token = sign_v2_public_token(
            &signing_key,
            claims.to_string().as_bytes(),
            br#"{"kid":"payment"}"#,
        );
        let verifier = PaymentVerifierWithPaseto::new(&public_key_hex).unwrap();

        let tx = verifier.verify(&token).unwrap();

        assert_eq!(tx.payment_transaction_id, 8926);
        assert_eq!(
            tx.payment_transaction_ref_id,
            "982ef8c1-fae8-46d6-a5fc-a69e295a5e61"
        );
        assert!(matches!(
            tx.payment_summary.request.extended_data,
            Some(ExtendData::ConsultInfoV2 { booking_id }) if booking_id == "BK-TEST-20260714-000001"
        ));
    }
}
