//! Public HTTP seam tests for the legacy booking route backed by canonical Hold services.
use axum::{
    Extension, Router,
    body::Body,
    http::{Request, StatusCode},
    routing::post,
};
use common_rs::tdh_protocol::{
    appointment::reserve::{PatientPrescreen, ReserveRequest, Timeslot},
    consultation::{BookingType, ConsultationChannel},
    doctor::profile::DoctorProfile,
    iam::user_identity::{AccountType, UserIdentity},
};
use consultation_rs::appointment::hold::{
    create::AppointmentHoldService,
    handler,
    model::{AppointmentHoldCreated, CreateAppointmentHold, PaymentQuote},
    repo::{
        AppointmentHoldRepo, AppointmentHoldRepoError, DoctorHoldAvailability,
        DoctorHoldProfileError, DoctorHoldProfileRepo, DoctorServiceConfig,
    },
    service::BookingService,
    state::AppState,
};
use consultation_rs::booking::repo::{
    BookingRepo, BookingRepoError, BookingStateRow, CancelReservedBookingRow,
};
use consultation_rs::consultation_config::model::ScheduleAvailableConfig;
use std::sync::Arc;
use tower::ServiceExt;

struct HoldRepo {
    instant: bool,
    schedule: bool,
}
#[async_trait::async_trait]
impl AppointmentHoldRepo for HoldRepo {
    async fn create_hold(
        &self,
        _patient: &UserIdentity,
        _request: &CreateAppointmentHold,
        _account: i64,
        _profile: i64,
        _ttl: i32,
        _created: i64,
    ) -> Result<AppointmentHoldCreated, AppointmentHoldRepoError> {
        Ok(AppointmentHoldCreated {
            booking_id: "BKHTTP001".into(),
            payment_quote: PaymentQuote {
                amount: "0.00".into(),
                currency: "THB".into(),
                effective_service_config_version: 1,
            },
        })
    }
    async fn availability(
        &self,
        _doctor: uuid::Uuid,
        _account: i64,
        _profile: i64,
    ) -> Result<Option<DoctorHoldAvailability>, AppointmentHoldRepoError> {
        Ok(Some(DoctorHoldAvailability {
            is_active: true,
            instant_available: self.instant,
            schedule_available: self.schedule,
            schedule_config: ScheduleAvailableConfig::default(),
            service_config: Some(DoctorServiceConfig {
                channels: vec!["video".into()],
                duration_minutes: 15,
            }),
        }))
    }
}
struct ProfileRepo;
#[async_trait::async_trait]
impl DoctorHoldProfileRepo for ProfileRepo {
    async fn doctor_profile(
        &self,
        _doctor_id: i32,
    ) -> Result<Option<DoctorProfile>, DoctorHoldProfileError> {
        Ok(Some(DoctorProfile {
            doctor_id: uuid::Uuid::nil().to_string(),
            iam_profile_id: 2,
            iam_account_id: 1,
            name: "doctor".into(),
            specialties: vec![],
        }))
    }
}
struct NoBookingRepo;
#[async_trait::async_trait]
impl BookingRepo for NoBookingRepo {
    async fn get_booking_state(
        &self,
        _: &str,
    ) -> Result<Option<BookingStateRow>, BookingRepoError> {
        Ok(None)
    }
    async fn cancel_reserved_booking(
        &self,
        _: &str,
    ) -> Result<Option<CancelReservedBookingRow>, BookingRepoError> {
        Ok(None)
    }
}
fn patient() -> UserIdentity {
    UserIdentity {
        account_id: 11,
        account_type: AccountType::Patient,
        user_profile_id: 12,
        user_main_profile_id: 12,
        tenant_id: 1,
        oidc_user_id: None,
        legacy_data: None,
    }
}
fn request(kind: BookingType) -> ReserveRequest {
    let start = jiff::Timestamp::now().as_second() + 3600;
    ReserveRequest {
        doctor_id: 7,
        biz_unit_id: 1,
        biz_center_id: 1,
        patient_intake: PatientPrescreen {
            symptom: "x".into(),
            symptom_duration: 1,
            symptom_duration_unit: "day".into(),
            attachments: vec![],
            allergies: vec![],
        },
        consultation_channel: ConsultationChannel::Video,
        timeslot: Timeslot {
            start,
            end: start + 900,
            duration: 900,
        },
        booking_type: kind,
        trace_id: None,
    }
}
fn app(instant: bool, schedule: bool) -> Router {
    let state = AppState {
        appointment_hold_service: AppointmentHoldService::new(
            Arc::new(HoldRepo { instant, schedule }),
            Arc::new(ProfileRepo),
            900,
        ),
        booking_service: BookingService::new(
            Arc::new(NoBookingRepo),
            Arc::new(consultation_rs::infra::event::NoOpEventPublisher),
        ),
        public_booking_rate_limiter: handler::PublicBookingRateLimiter::new(100, 60),
    };
    Router::new()
        .route("/v1/booking", post(handler::public_booking))
        .layer(Extension(patient()))
        .with_state(state)
}

#[tokio::test]
async fn public_booking_keeps_booking_id_shape_for_an_instant_hold() {
    let response = app(true, false)
        .oneshot(
            Request::post("/v1/booking")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&request(BookingType::Instant)).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["bookingId"], "BKHTTP001");
    assert_eq!(json["status"], "Reserved");
    assert_eq!(json["paymentQuote"]["amount"], "0.00");
    assert_eq!(json["paymentQuote"]["currency"], "THB");
}

#[tokio::test]
async fn public_booking_returns_doctor_not_available_when_scheduled_hold_is_disabled() {
    let response = app(true, false)
        .oneshot(
            Request::post("/v1/booking")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&request(BookingType::Schedule)).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["status"],
        "DoctorNotAvailable"
    );
}
