use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserIdentity {
    pub account_id: u64,
    pub account_type: AccountType,
    pub user_profile_id: u64,
    pub user_main_profile_id: u64,
    pub tenant_id: u32,
    pub oidc_user_id: Option<String>,
    pub legacy_data: Option<LegacyData>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize)]
#[serde(into = "u8")]
pub enum AccountType {
    Patient = 1,
    Doctor = 2,
}

impl From<AccountType> for u8 {
    fn from(val: AccountType) -> Self {
        val as u8
    }
}

impl<'de> Deserialize<'de> for AccountType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::Number(n) => {
                let num = n
                    .as_u64()
                    .ok_or_else(|| Error::custom("Invalid number for AccountType"))?;
                match num {
                    1 => Ok(AccountType::Patient),
                    2 => Ok(AccountType::Doctor),
                    _ => Err(Error::custom(format!("Invalid AccountType value: {}", num))),
                }
            }
            serde_json::Value::String(s) => match s.as_str() {
                "Patient" | "patient" => Ok(AccountType::Patient),
                "Doctor" | "doctor" => Ok(AccountType::Doctor),
                _ => Err(Error::custom(format!("Invalid AccountType string: {}", s))),
            },
            _ => Err(Error::custom("AccountType must be a number or string")),
        }
    }
}

impl From<AccountType> for String {
    fn from(val: AccountType) -> Self {
        match val {
            AccountType::Patient => "patient".to_string(),
            AccountType::Doctor => "doctor".to_string(),
        }
    }
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            AccountType::Patient => "patient",
            AccountType::Doctor => "doctor",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyData {
    pub user_id: String,
    pub uid: i32,
    pub client_id: String,
    pub client_int_id: u64,
    pub scopes: String,
    pub role_code: String,
}

#[derive(Debug)]
pub struct PartialUserIdentity {
    pub user_account_id: i32,
    pub user_profile_id: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_identity_serialization() {
        let user = UserIdentity {
            account_id: 3001,
            account_type: AccountType::Patient,
            user_profile_id: 4001,
            user_main_profile_id: 789,
            tenant_id: 1,
            oidc_user_id: Some("test-user".to_string()),
            legacy_data: None,
        };

        let json = serde_json::to_string(&user).unwrap();
        println!("Serialized JSON: {}", json);

        let parsed: UserIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(user, parsed);
    }

    #[test]
    fn test_user_identity_deserialization_from_string() {
        let json = r#"{"accountId":3001,"accountType":"Patient","userProfileId":4001,"userMainProfileId":789,"tenantId":1,"oidcUserId":"test-user"}"#;
        println!("Parsing JSON: {}", json);
        let parsed: UserIdentity = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.account_id, 3001);
        assert_eq!(parsed.account_type, AccountType::Patient);
    }

    #[test]
    fn test_user_identity_deserialization_from_int() {
        let json = r#"{"accountId":3001,"accountType":1,"userProfileId":4001,"userMainProfileId":789,"tenantId":1,"oidcUserId":"test-user"}"#;
        println!("Parsing JSON with int accountType: {}", json);
        let parsed: UserIdentity = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.account_id, 3001);
        assert_eq!(parsed.account_type, AccountType::Patient);
    }

    #[test]
    fn test_account_type_serializes_to_int() {
        let user = UserIdentity {
            account_id: 3001,
            account_type: AccountType::Doctor,
            user_profile_id: 4001,
            user_main_profile_id: 789,
            tenant_id: 1,
            oidc_user_id: Some("test-user".to_string()),
            legacy_data: None,
        };

        let json = serde_json::to_string(&user).unwrap();
        println!("Serialized JSON with Doctor: {}", json);
        assert!(json.contains(r#""accountType":2"#));
    }
}
