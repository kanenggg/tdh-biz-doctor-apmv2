use serde::{Deserialize, Serialize};

use crate::tdh_protocol::payment::selected_channel_result::SelectedChannelResult;

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentTime {
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Deserialize, Debug, Serialize)]
pub enum AppointmentStatus {
    #[serde(rename = "ComingUp")]
    ComingUp,
    #[serde(rename = "Ongoing")]
    Ongoing,
    #[serde(rename = "Completed")]
    Completed,
    #[serde(rename = "Missed")]
    Missed,
    #[serde(rename = "Record")]
    Record,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Payer {
    pub __type: String,
    pub company: Option<String>,
    pub insurance_condition: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Patient {
    pub account_id: i32,
    pub profile_id: i32,
    pub full_name: String,
    pub date_of_birth: String,
    pub gender: String,
    pub bmi: Option<f64>,
    pub weight: Option<f64>,
}

pub type SymptomsDuration = String;

#[derive(Deserialize, Serialize, Debug)]
pub struct Symptoms {
    pub description: String,
    pub duration: SymptomsDuration,
    pub drug_allergies: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentDetail {
    pub appointment_id: String,
    pub appointment_date: String,
    pub appointment_time: AppointmentTime,
    pub status: AppointmentStatus,
    pub patient: Patient,
    pub symptom: Symptoms,
    pub attachments: Option<Vec<String>>,
    pub payment_channel: Option<SelectedChannelResult>,
}
