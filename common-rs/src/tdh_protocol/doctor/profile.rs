use serde::{Deserialize, Serialize};

use crate::tdh_protocol::doctor::specialty::Specialty;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorProfile {
    pub doctor_id: String,
    pub iam_profile_id: i64,
    pub iam_account_id: i64,
    pub name: String,
    pub specialties: Vec<Specialty>,
}
