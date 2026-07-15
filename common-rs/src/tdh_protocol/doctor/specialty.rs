use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Specialty {
    id: String,
    name: String,
    description: String,
}
