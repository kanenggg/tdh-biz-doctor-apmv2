use std::sync::Arc;

use crate::common::tdh_protocol::common::PartialUserIdentity;
use crate::common::tdh_protocol::consultation::{
    ConsultationChannel, ConsultationEvent, PostSessionMessage,
};
use crate::common::tdh_protocol::iam::user_identity::UserIdentity;
use crate::protocol::ConsultationChannel as BizApmConsultationChannel;
use crate::protocol::PatientIdentity;
use crate::protocol::follow_up::FollowUp;
use crate::protocol::summary_note::{
    SummarizationError, SummarizationRequest, SummarizationResult,
};
use base64::Engine;
use tracing::{error, info};

use crate::infra::event::EventPublisher;
use crate::repo::enums::AppointmentTypeEnum;
use crate::summarization::follow_up_repo::FollowUpRepoPsql;
use crate::summarization::repo::{CreateSummaryNoteParams, SummaryNoteRepoPsql};
use crate::sys::crypto::kms::Kms;

#[derive(Clone)]
pub struct SummaryNoteService {
    repo: Arc<SummaryNoteRepoPsql>,
    follow_up_repo: Arc<FollowUpRepoPsql>,
    event_publisher: Arc<dyn EventPublisher>,
    kms: Arc<dyn Kms>,
    kms_key_name: String,
}

impl SummaryNoteService {
    pub fn new(
        repo: Arc<SummaryNoteRepoPsql>,
        follow_up_repo: Arc<FollowUpRepoPsql>,
        event_publisher: Arc<dyn EventPublisher>,
        kms: Arc<dyn Kms>,
        kms_key_name: String,
    ) -> Self {
        Self {
            repo,
            follow_up_repo,
            event_publisher,
            kms,
            kms_key_name,
        }
    }

    pub async fn add_summary_note(
        &self,
        _user_id: Option<UserIdentity>,
        summary_note: SummarizationRequest,
    ) -> Result<SummarizationResult, SummarizationError> {
        let json_data = serde_json::to_vec(&summary_note).map_err(|e| {
            error!(
                error = %e,
                booking_id = %summary_note.booking_id,
                "Failed to serialize summary note to JSON"
            );
            SummarizationError::InternalError(e.to_string())
        })?;

        let encrypted = self
            .kms
            .encrypt(&json_data, &self.kms_key_name)
            .await
            .map_err(|e| {
                error!(
                    error = %e,
                    booking_id = %summary_note.booking_id,
                    "Failed to encrypt summary note via KMS"
                );
                SummarizationError::InternalError(e.to_string())
            })?;

        let encrypted_data = base64::engine::general_purpose::STANDARD.encode(&encrypted);

        let icd10_codes: Vec<String> = summary_note.icd10.iter().map(|c| c.code.clone()).collect();

        let params = CreateSummaryNoteParams {
            booking_id: summary_note.booking_id.clone(),
            encrypted_data,
            encrypted_data_type: "DoctorSummaryNoteV1".to_string(),
            note_to_staff: Some(summary_note.note_to_staff),
            icd10_codes,
            prescription_id: summary_note.prescription_id,
        };

        let result = self.repo.insert(params).await.map_err(|e| {
            error!(
                error = %e,
                booking_id = %summary_note.booking_id,
                "Failed to insert summary note into database"
            );
            SummarizationError::InternalError(e.to_string())
        })?;

        let patient_identity = result.patient_identity();

        let summarization_result = if result.created {
            SummarizationResult::Success {
                summary_note_id: result.summary_note_id,
                patient_identity: patient_identity.clone(),
                biz_unit_id: result.biz_unit_id,
                biz_center_id: result.biz_center_id,
            }
        } else {
            SummarizationResult::AlreadySubmitted {
                summary_note_id: result.summary_note_id,
                patient_identity: patient_identity.clone(),
                biz_unit_id: result.biz_unit_id,
                biz_center_id: result.biz_center_id,
            }
        };

        self.handle_follow_up(
            &summary_note.booking_id,
            &summary_note.follow_up,
            &patient_identity,
            result.biz_unit_id,
        )
        .await?;

        Ok(summarization_result)
    }

    async fn handle_follow_up(
        &self,
        booking_id: &str,
        follow_up: &FollowUp,
        patient_identity: &PatientIdentity,
        biz_unit_id: i64,
    ) -> Result<(), SummarizationError> {
        match follow_up {
            FollowUp::AsNeeded => {
                info!("booking_id {booking_id} has no follow-up");
            }
            FollowUp::Appointment(data) => {
                let appointment_start = jiff::Timestamp::from_second(data.appointment_start)
                    .map_err(|e| {
                        error!(
                            error = %e,
                            booking_id = %booking_id,
                            appointment_start = data.appointment_start,
                            "Failed to parse follow-up appointment_start timestamp"
                        );
                        SummarizationError::InternalError(e.to_string())
                    })?;
                let appointment_end =
                    jiff::Timestamp::from_second(data.appointment_end).map_err(|e| {
                        error!(
                            error = %e,
                            booking_id = %booking_id,
                            appointment_end = data.appointment_end,
                            "Failed to parse follow-up appointment_end timestamp"
                        );
                        SummarizationError::InternalError(e.to_string())
                    })?;
                let appointment_start_sqlx = jiff_sqlx::Timestamp::from(appointment_start);
                let duration_seconds = (data.appointment_end - data.appointment_start) as i32;

                let creation_result = self
                    .follow_up_repo
                    .create_follow_up(
                        &data.parent_booking_id,
                        appointment_start_sqlx,
                        duration_seconds,
                        AppointmentTypeEnum::Routine,
                    )
                    .await
                    .map_err(|e| {
                        error!(
                            error = %e,
                            booking_id = %booking_id,
                            parent_booking_id = %data.parent_booking_id,
                            "Failed to create follow-up appointment in database"
                        );
                        SummarizationError::FollowUpCreationFailed(e.to_string())
                    })?;

                let now = jiff::Timestamp::now();
                let consultation_channel = match data.consultation_channel {
                    BizApmConsultationChannel::Video => ConsultationChannel::Video,
                    BizApmConsultationChannel::Voice => ConsultationChannel::Voice,
                    BizApmConsultationChannel::Chat => ConsultationChannel::Chat,
                };

                let consultation_fee = data.consultation_fee;
                let follow_up_booking_id = creation_result.booking_id.clone();

                let event =
                    ConsultationEvent::PostSessionMessage(PostSessionMessage::FollowUpRequired {
                        previous_booking_id: booking_id.to_string(),
                        follow_up_id: creation_result.booking_id,
                        patient_identity: PartialUserIdentity {
                            account_id: patient_identity.account_id as u64,
                            user_profile_id: patient_identity.user_profile_id as u64,
                            tenant_id: patient_identity.tenant_id as u32,
                            oidc_user_id: patient_identity.oidc_user_id.clone(),
                        },
                        doctor_id: creation_result.doctor_id,
                        biz_unit_id: creation_result.biz_unit_id,
                        consultation_start_time: data.appointment_start,
                        consultation_duration_in_second: duration_seconds,
                        consultation_fee,
                        consultation_channel,
                        additional_patient_note: if data.additional_note_to_patient.is_empty() {
                            None
                        } else {
                            Some(data.additional_note_to_patient.clone())
                        },
                        internal_note: if data.note_to_staff.is_empty() {
                            None
                        } else {
                            Some(data.note_to_staff.clone())
                        },
                        created_at: now.as_second(),
                    });

                self.event_publisher
                    .publish_consultation_event(event)
                    .await
                    .map_err(|e| {
                        error!(
                            error = %e,
                            booking_id = %booking_id,
                            follow_up_id = %follow_up_booking_id,
                            "Failed to publish FollowUpRequired event"
                        );
                        SummarizationError::FollowUpCreationFailed(format!(
                            "Failed to publish FollowUpRequired event: {}",
                            e
                        ))
                    })?;
            }
        }
        Ok(())
    }
}
