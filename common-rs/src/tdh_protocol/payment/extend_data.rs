use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::product_cart::ProductCart;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum ExtendData {
    #[serde(rename = "ExtendData.ConsultInfo")]
    ConsultInfo {
        #[serde(rename = "bookingId")]
        booking_id: String,
        #[serde(rename = "doctorId")]
        doctor_id: i32,
        #[serde(rename = "doctorUsId")]
        doctor_us_id: String,
        #[serde(default, rename = "clinicIds")]
        clinic_ids: Vec<String>,
        #[serde(default, rename = "medicalSpecialtyIds")]
        medical_specialty_ids: Vec<String>,
        channel: String,
        #[serde(rename = "scheduleTime")]
        schedule_time: i64,
    },
    #[serde(rename = "ExtendData.ConsultInfoV2")]
    ConsultInfoV2 {
        #[serde(rename = "bookingId")]
        booking_id: String,
    },
    #[serde(rename = "ExtendData.PrescriptionInfo")]
    PrescriptionInfo {
        #[serde(rename = "bookingId")]
        booking_id: String,
        #[serde(rename = "doctorId")]
        doctor_id: i32,
        #[serde(rename = "doctorUsId")]
        doctor_us_id: String,
        #[serde(default, rename = "clinicIds")]
        clinic_ids: Vec<String>,
        #[serde(default, rename = "medicalSpecialtyIds")]
        medical_specialty_ids: Vec<String>,
        channel: String,
        #[serde(rename = "scheduleTime")]
        schedule_time: i64,
        #[serde(rename = "consultAmount")]
        consult_amount: Decimal,
    },
    #[serde(rename = "ExtendData.CommerceCart")]
    CommerceCart { list: Vec<ProductCart> },
    #[serde(rename = "ExtendData.FollowUp")]
    FollowUp {
        #[serde(rename = "previousBookingId")]
        previous_booking_id: String,
        #[serde(rename = "onlySelfPayChannelShown")]
        only_self_pay_channel_shown: bool,
        #[serde(rename = "doctorId")]
        doctor_id: i32,
        #[serde(rename = "doctorUsId")]
        doctor_us_id: String,
        #[serde(default, rename = "clinicIds")]
        clinic_ids: Vec<String>,
        #[serde(default, rename = "medicalSpecialtyIds")]
        medical_specialty_ids: Vec<String>,
        channel: String,
        #[serde(rename = "scheduleTime")]
        schedule_time: i64,
    },
    #[serde(rename = "ExtendData.HealthyMePlan")]
    HealthyMePlan {
        #[serde(rename = "planId")]
        plan_id: String,
        #[serde(rename = "itemId")]
        item_id: i32,
        price: f32,
        #[serde(rename = "promotionPrice")]
        promotion_price: f32,
    },
    #[serde(rename = "ExtendData.AktivoProgram")]
    AktivoProgram {
        #[serde(rename = "programId")]
        program_id: String,
        #[serde(rename = "programPeriodMonth")]
        program_period_month: i32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_consult_info_when_legacy_payload_omits_optional_lists() {
        let extend_data: ExtendData = serde_json::from_value(serde_json::json!({
            "__type": "ExtendData.ConsultInfo",
            "bookingId": "BK20260227810949",
            "doctorId": 300,
            "doctorUsId": "doctor-us-1",
            "channel": "video",
            "scheduleTime": 1772179200,
        }))
        .unwrap();

        match extend_data {
            ExtendData::ConsultInfo {
                booking_id,
                clinic_ids,
                medical_specialty_ids,
                ..
            } => {
                assert_eq!(booking_id, "BK20260227810949");
                assert!(clinic_ids.is_empty());
                assert!(medical_specialty_ids.is_empty());
            }
            other => panic!("expected ConsultInfo, got {other:?}"),
        }
    }
}
