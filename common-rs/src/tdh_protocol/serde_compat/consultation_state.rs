//! Consultation state machine models for tracking consultation lifecycle.
//!
//! These types are shared across TDH services that need to track
//! consultation state transitions and appointment status.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::tdh_protocol::serde_compat::common::PatientIdentity;

/// Simple start/end time pair for appointments (formatted as "HH:MM" strings).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentTime {
    pub start_time: String,
    pub end_time: String,
}

/// Lightweight appointment card status for Firebase RTDB.
/// Matches Scala `AppointmentCardStatus` — serialises as `{"__type": "UpComing"}`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum AppointmentCardStatus {
    #[serde(rename = "UpComing")]
    UpComing,
    #[serde(rename = "Missed")]
    Missed,
    #[serde(rename = "Completed")]
    Completed,
    #[serde(rename = "PendingRecord")]
    PendingRecord,
}

/// Consultation phase (high-level lifecycle stage)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConsultationPhase {
    PreSession,
    Session,
    PostSession,
    Completed,
}

/// Detailed consultation state for state machine validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConsultationState {
    // No state (initial)
    None,

    // Pre-Session states
    TimeslotReserved,
    ConsultationBooked,
    ConsultationCancelled,
    ReservationCancelled,
    ReservationExpired,

    // Session states
    SessionCreated,
    PatientJoined,
    DoctorJoined,
    AllParticipantJoined,
    PatientDisconnected,
    DoctorDisconnected,
    SessionTerminated,

    // Post-Session states
    ConsultationSummarized,
    FollowUpRequired,
    FollowUpRequestExpired,
    PatientAcceptedFollowUp,
    FollowUpCancelled,
}

impl ConsultationState {
    /// Check if this is a terminal state (no further transitions allowed)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ConsultationState::ConsultationCancelled
                | ConsultationState::ReservationCancelled
                | ConsultationState::ReservationExpired
                | ConsultationState::FollowUpCancelled
                | ConsultationState::FollowUpRequestExpired
        )
    }

    /// Check if this is a session state
    pub fn is_session(&self) -> bool {
        matches!(
            self,
            ConsultationState::SessionCreated
                | ConsultationState::PatientJoined
                | ConsultationState::DoctorJoined
                | ConsultationState::AllParticipantJoined
                | ConsultationState::PatientDisconnected
                | ConsultationState::DoctorDisconnected
                | ConsultationState::SessionTerminated
        )
    }

    /// Get the phase for this state
    pub fn phase(&self) -> ConsultationPhase {
        match self {
            ConsultationState::None
            | ConsultationState::TimeslotReserved
            | ConsultationState::ConsultationBooked
            | ConsultationState::ConsultationCancelled
            | ConsultationState::ReservationCancelled
            | ConsultationState::ReservationExpired => ConsultationPhase::PreSession,

            ConsultationState::SessionCreated
            | ConsultationState::PatientJoined
            | ConsultationState::DoctorJoined
            | ConsultationState::AllParticipantJoined
            | ConsultationState::PatientDisconnected
            | ConsultationState::DoctorDisconnected
            | ConsultationState::SessionTerminated => ConsultationPhase::Session,

            ConsultationState::ConsultationSummarized
            | ConsultationState::FollowUpRequired
            | ConsultationState::FollowUpRequestExpired
            | ConsultationState::PatientAcceptedFollowUp
            | ConsultationState::FollowUpCancelled => ConsultationPhase::PostSession,
        }
    }

    /// Map consultation state to the RTDB appointment card status
    pub fn to_appointment_status(&self) -> AppointmentCardStatus {
        match self {
            ConsultationState::None
            | ConsultationState::TimeslotReserved
            | ConsultationState::ConsultationBooked
            | ConsultationState::SessionCreated
            | ConsultationState::PatientJoined
            | ConsultationState::DoctorJoined
            | ConsultationState::AllParticipantJoined
            | ConsultationState::PatientDisconnected
            | ConsultationState::DoctorDisconnected => AppointmentCardStatus::UpComing,

            ConsultationState::ConsultationCancelled
            | ConsultationState::ReservationCancelled
            | ConsultationState::ReservationExpired => AppointmentCardStatus::Missed,

            ConsultationState::SessionTerminated => AppointmentCardStatus::PendingRecord,

            ConsultationState::ConsultationSummarized
            | ConsultationState::FollowUpRequired
            | ConsultationState::FollowUpRequestExpired
            | ConsultationState::PatientAcceptedFollowUp
            | ConsultationState::FollowUpCancelled => AppointmentCardStatus::Completed,
        }
    }

    /// Check if a transition from this state to another is valid
    pub fn can_transition_to(&self, next: &ConsultationState) -> bool {
        // Terminal states cannot transition
        if self.is_terminal() {
            return false;
        }

        match self {
            ConsultationState::None => {
                matches!(
                    next,
                    ConsultationState::TimeslotReserved | ConsultationState::ConsultationBooked
                )
            }
            ConsultationState::TimeslotReserved => {
                matches!(
                    next,
                    ConsultationState::ReservationCancelled
                        | ConsultationState::ReservationExpired
                        | ConsultationState::ConsultationBooked
                )
            }
            ConsultationState::ConsultationBooked => {
                matches!(
                    next,
                    ConsultationState::ConsultationCancelled | ConsultationState::SessionCreated
                )
            }
            ConsultationState::SessionCreated => {
                matches!(
                    next,
                    ConsultationState::PatientJoined
                        | ConsultationState::DoctorJoined
                        | ConsultationState::SessionTerminated
                )
            }
            ConsultationState::PatientJoined => {
                matches!(
                    next,
                    ConsultationState::DoctorJoined | ConsultationState::PatientDisconnected
                )
            }
            ConsultationState::DoctorJoined => {
                matches!(
                    next,
                    ConsultationState::PatientJoined
                        | ConsultationState::AllParticipantJoined
                        | ConsultationState::DoctorDisconnected
                )
            }
            ConsultationState::AllParticipantJoined => {
                matches!(
                    next,
                    ConsultationState::PatientDisconnected
                        | ConsultationState::DoctorDisconnected
                        | ConsultationState::SessionTerminated
                )
            }
            ConsultationState::PatientDisconnected => {
                matches!(
                    next,
                    ConsultationState::DoctorDisconnected | ConsultationState::SessionTerminated
                )
            }
            ConsultationState::DoctorDisconnected => {
                matches!(
                    next,
                    ConsultationState::PatientDisconnected | ConsultationState::SessionTerminated
                )
            }
            ConsultationState::SessionTerminated => {
                matches!(next, ConsultationState::ConsultationSummarized)
            }
            ConsultationState::ConsultationSummarized => {
                matches!(
                    next,
                    ConsultationState::FollowUpRequired
                        | ConsultationState::FollowUpRequestExpired
                        | ConsultationState::PatientAcceptedFollowUp
                )
            }
            ConsultationState::FollowUpRequired => {
                matches!(
                    next,
                    ConsultationState::FollowUpRequestExpired
                        | ConsultationState::PatientAcceptedFollowUp
                        | ConsultationState::FollowUpCancelled
                )
            }
            ConsultationState::PatientAcceptedFollowUp => {
                matches!(next, ConsultationState::FollowUpCancelled)
            }
            ConsultationState::ConsultationCancelled
            | ConsultationState::ReservationCancelled
            | ConsultationState::ReservationExpired
            | ConsultationState::FollowUpRequestExpired
            | ConsultationState::FollowUpCancelled => false,
        }
    }
}

/// Information about the current session state
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionStateInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consultation_start_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consultation_duration_in_second: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_joined_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctor_joined_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_disconnected_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctor_disconnected_at: Option<i64>,
}

/// Consultation state document stored in Firestore
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationStateDoc {
    pub booking_id: String,
    pub doctor_id: i32,
    pub patient_identity: PatientIdentity,
    pub current_state: ConsultationState,
    pub current_phase: ConsultationPhase,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_info: Option<SessionStateInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        assert!(ConsultationState::None.can_transition_to(&ConsultationState::TimeslotReserved));
        assert!(ConsultationState::TimeslotReserved
            .can_transition_to(&ConsultationState::ConsultationBooked));
        assert!(ConsultationState::ConsultationBooked
            .can_transition_to(&ConsultationState::SessionCreated));
        assert!(
            ConsultationState::SessionCreated.can_transition_to(&ConsultationState::PatientJoined)
        );
        assert!(
            ConsultationState::PatientJoined.can_transition_to(&ConsultationState::DoctorJoined)
        );
        assert!(ConsultationState::AllParticipantJoined
            .can_transition_to(&ConsultationState::SessionTerminated));
        assert!(ConsultationState::SessionTerminated
            .can_transition_to(&ConsultationState::ConsultationSummarized));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!ConsultationState::SessionTerminated
            .can_transition_to(&ConsultationState::PatientJoined));
        assert!(!ConsultationState::ConsultationCancelled
            .can_transition_to(&ConsultationState::ConsultationBooked));
        assert!(
            !ConsultationState::PatientJoined.can_transition_to(&ConsultationState::SessionCreated)
        );
    }

    #[test]
    fn test_terminal_states() {
        assert!(!ConsultationState::SessionTerminated.is_terminal());
        assert!(ConsultationState::ConsultationCancelled.is_terminal());
        assert!(ConsultationState::FollowUpCancelled.is_terminal());
        assert!(!ConsultationState::SessionCreated.is_terminal());
    }

    #[test]
    fn test_phase_mapping() {
        assert_eq!(
            ConsultationState::None.phase(),
            ConsultationPhase::PreSession
        );
        assert_eq!(
            ConsultationState::SessionCreated.phase(),
            ConsultationPhase::Session
        );
        assert_eq!(
            ConsultationState::ConsultationSummarized.phase(),
            ConsultationPhase::PostSession
        );
    }

    #[test]
    fn test_appointment_status_mapping() {
        assert!(matches!(
            ConsultationState::ConsultationBooked.to_appointment_status(),
            AppointmentCardStatus::UpComing
        ));
        assert!(matches!(
            ConsultationState::ConsultationCancelled.to_appointment_status(),
            AppointmentCardStatus::Missed
        ));
        assert!(matches!(
            ConsultationState::SessionTerminated.to_appointment_status(),
            AppointmentCardStatus::PendingRecord
        ));
        assert!(matches!(
            ConsultationState::ConsultationSummarized.to_appointment_status(),
            AppointmentCardStatus::Completed
        ));
    }
}
