//! Compatibility layer for tdh-protocol types with serde JSON support.
//!
//! This module provides backward-compatible JSON serialization wrappers around
//! the protobuf-generated types from tdh-protocol. The proto types use prost
//! which doesn't support serde, so we provide wrapper types for API compatibility.

use super::notification;

pub mod ref_data;
// pub mod consultation;
// pub mod consultation_state;
// pub mod notification;

// Re-export all types for convenience
// pub use common::{BookingType, ConsultationChannel, LocalizedString, PatientIdentity};


pub use ref_data::{
    AcademicPosition, District, MedicalSchool, PostalCode, Profession, Province, SubDistrict,
    WorkPlace,
};

pub use super::biz_apm::consultation_event::{
    AllParticipantJoinedEvent,
    ConsultationBookedEvent,
    ConsultationCancelledEvent,
    ConsultationEvent,
    ConsultationSummarizedEvent,
    DoctorDisconnectedEvent,
    DoctorJoinedEvent,
    FollowUpCancelledEvent,
    FollowUpRequestExpiredEvent,
    FollowUpRequiredEvent,
    Medicine,
    PatientAcceptedFollowUpEvent,
    PatientDisconnectedEvent,
    PatientJoinedEvent,
    PrescriptionInfo,
    ReservationCancelledEvent,
    ReservationExpiredEvent,
    SessionCreatedEvent,
    // Supporting types
    SessionParticipant,
    SessionTerminatedEvent,
    TerminationCode,
    // Event variants
    TimeslotReservedEvent,
};
