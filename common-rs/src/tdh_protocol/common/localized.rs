use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Localized {
    pub th: String,
    pub en: String,
}

#[cfg(feature = "sqlx")]
impl sqlx::Type<sqlx::Postgres> for Localized {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("jsonb")
    }
}

#[cfg(feature = "sqlx")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for Localized {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        sqlx::types::Json(self).encode_by_ref(buf)
    }
}

#[cfg(feature = "sqlx")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Localized {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let sqlx::types::Json(v) =
            <sqlx::types::Json<Self> as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(v)
    }
}
