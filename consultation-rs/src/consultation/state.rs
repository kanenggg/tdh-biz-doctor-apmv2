use crate::consultation::session_info::rtdb_access::RtdbCustomTokenIssuer;
use crate::consultation::{
    EndSessionService, FacialUploadService, GetOrCreateConsultSessionService,
    PatientVerificationService,
};

#[derive(Clone)]
pub struct AppState {
    pub session_service: GetOrCreateConsultSessionService,
    pub rtdb_token_issuer: RtdbCustomTokenIssuer,
    // pub reserve_service: ReservationService,
    // pub create_confirmed_appointment_service: CreateConfirmedAppointment,
    pub facial_upload_service: FacialUploadService,
    pub end_session_service: EndSessionService,
    pub patient_verification_service: PatientVerificationService,
}
