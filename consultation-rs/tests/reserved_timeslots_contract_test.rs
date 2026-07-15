use consultation_rs::doctor_timeslot::reserved_timeslot::model::{
    ReserveTimeSlot, ReservedTimeslotsResponse,
};

#[test]
fn response_serializes_to_frozen_contract() {
    let response = ReservedTimeslotsResponse {
        reserved_timeslots: vec![ReserveTimeSlot {
            booking_id: "BK20260618000123".to_string(),
            start_time: 1750000000,
            end_time: 1750001500,
        }],
    };

    let actual: serde_json::Value = serde_json::to_value(&response).unwrap();
    let frozen: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/superpowers/contracts/reserved-timeslots.response.json"
    ))
    .unwrap();

    assert_eq!(actual, frozen);
}
