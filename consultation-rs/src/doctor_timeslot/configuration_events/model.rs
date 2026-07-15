use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::consultation_config::model::{DoctorConfigIdentity, DoctorIdentity, EventSource};

pub const INSTANT_AVAILABILITY_CHANGED_TOPIC: &str =
    "apm.doctor-timeslot.instant-availability.changed";
pub const SCHEDULE_AVAILABILITY_CHANGED_TOPIC: &str =
    "apm.doctor-timeslot.schedule-availability.changed";
pub const TIMESLOT_CONFIGURATION_CHANGED_TOPIC: &str =
    "apm.doctor-timeslot.timeslot-configuration.changed";
pub const SERVICE_CONFIGURATION_CHANGED_TOPIC: &str =
    "apm.doctor-timeslot.service-configuration.changed";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DoctorTimeslotConfigChangedEvent {
    pub event_id: String,
    pub event_type: DoctorTimeslotConfigEventType,
    pub doctor: DoctorIdentity,
    pub changed_at: String,
    pub source: EventSource,
    pub change_type: DoctorTimeslotConfigChangeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

impl DoctorTimeslotConfigChangedEvent {
    pub fn new(
        identity: DoctorConfigIdentity,
        change_type: DoctorTimeslotConfigChangeType,
        is_active: Option<bool>,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            event_type: DoctorTimeslotConfigEventType::DoctorTimeslotConfigurationChanged,
            doctor: identity.event_doctor(),
            changed_at: jiff::Timestamp::now().to_string(),
            source: EventSource {
                service: "tdh-biz-doctor-apmv2/consultation-rs".to_string(),
                version: "v1".to_string(),
            },
            change_type,
            is_active,
        }
    }

    pub fn topic(&self) -> &'static str {
        self.change_type.topic()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum DoctorTimeslotConfigEventType {
    DoctorTimeslotConfigurationChanged,
}

impl DoctorTimeslotConfigEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DoctorTimeslotConfigurationChanged => "DoctorTimeslotConfigurationChanged",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum DoctorTimeslotConfigChangeType {
    InstantAvailability,
    ScheduleAvailability,
    TimeslotConfiguration,
    ServiceConfiguration,
}

impl DoctorTimeslotConfigChangeType {
    pub fn topic(self) -> &'static str {
        match self {
            Self::InstantAvailability => INSTANT_AVAILABILITY_CHANGED_TOPIC,
            Self::ScheduleAvailability => SCHEDULE_AVAILABILITY_CHANGED_TOPIC,
            Self::TimeslotConfiguration => TIMESLOT_CONFIGURATION_CHANGED_TOPIC,
            Self::ServiceConfiguration => SERVICE_CONFIGURATION_CHANGED_TOPIC,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_change_types_to_required_topics() {
        assert_eq!(
            DoctorTimeslotConfigChangeType::InstantAvailability.topic(),
            "apm.doctor-timeslot.instant-availability.changed"
        );
        assert_eq!(
            DoctorTimeslotConfigChangeType::ScheduleAvailability.topic(),
            "apm.doctor-timeslot.schedule-availability.changed"
        );
        assert_eq!(
            DoctorTimeslotConfigChangeType::TimeslotConfiguration.topic(),
            "apm.doctor-timeslot.timeslot-configuration.changed"
        );
        assert_eq!(
            DoctorTimeslotConfigChangeType::ServiceConfiguration.topic(),
            "apm.doctor-timeslot.service-configuration.changed"
        );
    }
}
