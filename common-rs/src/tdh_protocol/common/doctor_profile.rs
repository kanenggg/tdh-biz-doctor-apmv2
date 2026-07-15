use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Locale {
    pub th: String,
    pub en: String,
}
