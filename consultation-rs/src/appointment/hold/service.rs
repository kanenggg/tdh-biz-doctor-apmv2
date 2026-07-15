use std::sync::Arc;

use crate::appointment::hold::model::{
    BookingCancelResponse, BookingLifecycle, BookingStateResponse,
};
use crate::appointment::hold::repo::{
    BookingRepo, BookingRepoError, BookingStateRow, CancelReservedBookingRow,
};
use crate::common::tdh_protocol::iam::user_identity::{AccountType, UserIdentity};
use crate::infra::event::EventPublisher;

#[derive(Debug, thiserror::Error)]
pub enum BookingError {
    #[error("booking not found")]
    NotFound,
    #[error("unauthorized booking access")]
    Unauthorized,
    #[error("booking cannot be cancelled in its current state")]
    CannotCancel,
    #[error("repository error: {0}")]
    Repository(#[from] anyhow::Error),
}

impl From<BookingRepoError> for BookingError {
    fn from(error: BookingRepoError) -> Self {
        match error {
            BookingRepoError::CannotCancel => Self::CannotCancel,
            BookingRepoError::Unexpected(error) => Self::Repository(error),
        }
    }
}

#[derive(Clone)]
pub struct BookingService {
    repo: Arc<dyn BookingRepo>,
    _event_publisher: Arc<dyn EventPublisher>,
}

impl BookingService {
    pub fn new(repo: Arc<dyn BookingRepo>, event_publisher: Arc<dyn EventPublisher>) -> Self {
        Self {
            repo,
            _event_publisher: event_publisher,
        }
    }

    pub async fn get_state(
        &self,
        booking_id: &str,
        user_identity: Option<&UserIdentity>,
    ) -> Result<BookingStateResponse, BookingError> {
        let row = self
            .repo
            .get_booking_state(booking_id)
            .await?
            .ok_or(BookingError::NotFound)?;
        authorize_patient(
            user_identity,
            row.patient_account_id,
            row.patient_profile_id,
        )?;
        Ok(row.into())
    }

    pub async fn release_hold(
        &self,
        booking_id: &str,
        user_identity: Option<&UserIdentity>,
    ) -> Result<BookingCancelResponse, BookingError> {
        let state = self
            .repo
            .get_booking_state(booking_id)
            .await?
            .ok_or(BookingError::NotFound)?;
        authorize_patient(
            user_identity,
            state.patient_account_id,
            state.patient_profile_id,
        )?;

        if !is_cancellable_booking_state(&state) {
            return Err(BookingError::CannotCancel);
        }

        let cancelled = self
            .repo
            .cancel_reserved_booking(booking_id)
            .await?
            .ok_or(BookingError::NotFound)?;

        if !is_cancelled_booking_result(&cancelled) {
            return Err(BookingError::CannotCancel);
        }

        Ok(cancelled.into())
    }

    /// Legacy route adapter for callers that still use booking-cancellation wording.
    #[deprecated(note = "pre-booking Holds are released; use release_hold")]
    pub async fn cancel(
        &self,
        booking_id: &str,
        user_identity: Option<&UserIdentity>,
    ) -> Result<BookingCancelResponse, BookingError> {
        self.release_hold(booking_id, user_identity).await
    }
}

fn authorize_patient(
    user_identity: Option<&UserIdentity>,
    patient_account_id: i32,
    patient_profile_id: i32,
) -> Result<(), BookingError> {
    let Some(user_identity) = user_identity else {
        return Ok(());
    };

    if user_identity.account_type == AccountType::Patient
        && user_identity.account_id == patient_account_id as u64
        && user_identity.user_profile_id == patient_profile_id as u64
    {
        Ok(())
    } else {
        Err(BookingError::Unauthorized)
    }
}

impl From<BookingStateRow> for BookingStateResponse {
    fn from(row: BookingStateRow) -> Self {
        Self {
            booking_id: row.booking_id,
            state: lifecycle_from_statuses(
                &row.reservation_status,
                row.appointment_status.as_deref(),
            ),
            reservation_status: row.reservation_status,
            appointment_status: row.appointment_status,
            reserved_until: row.reserved_until,
            appointment_start: row.appointment_start,
            appointment_end: row.appointment_end,
        }
    }
}

impl From<CancelReservedBookingRow> for BookingCancelResponse {
    fn from(row: CancelReservedBookingRow) -> Self {
        let state =
            lifecycle_from_statuses(&row.reservation_status, row.appointment_status.as_deref());
        Self {
            booking_id: row.booking_id,
            state,
            cancelled_lifecycle: BookingLifecycle::Reservation,
            cancelled_at: row.cancelled_at,
        }
    }
}

fn lifecycle_from_statuses(
    reservation_status: &str,
    appointment_status: Option<&str>,
) -> BookingLifecycle {
    match appointment_status {
        Some("BOOKED") | Some("ARRIVED") => BookingLifecycle::Booked,
        Some("FULFILLED") | Some("CONSULTATION_DONE") => BookingLifecycle::ConsultationDone,
        Some("CANCELLED") => BookingLifecycle::Cancelled,
        _ => match reservation_status {
            "RESERVED" => BookingLifecycle::Reserved,
            "CONFIRMED" => BookingLifecycle::Booked,
            "RESERVE_EXPIRED" => BookingLifecycle::ReserveExpired,
            "CANCELLED" => BookingLifecycle::Cancelled,
            _ => BookingLifecycle::Unknown,
        },
    }
}

fn is_cancellable_booking_state(state: &BookingStateRow) -> bool {
    let reservation_can_cancel =
        matches!(state.reservation_status.as_str(), "RESERVED" | "CANCELLED");
    let appointment_can_cancel = matches!(
        state.appointment_status.as_deref(),
        None | Some("PENDING") | Some("CANCELLED")
    );

    reservation_can_cancel && appointment_can_cancel
}

fn is_cancelled_booking_result(row: &CancelReservedBookingRow) -> bool {
    let reservation_cancelled = row.reservation_status == "CANCELLED";
    let appointment_not_active = matches!(
        row.appointment_status.as_deref(),
        None | Some("PENDING") | Some("CANCELLED")
    );

    reservation_cancelled && appointment_not_active
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::common::tdh_protocol::consultation::ConsultationEvent;
    use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangedEvent;

    #[derive(Default)]
    struct RecordingBookingRepo {
        state: Mutex<Option<BookingStateRow>>,
        cancelled: Mutex<Option<CancelReservedBookingRow>>,
        get_calls: AtomicUsize,
        cancel_calls: AtomicUsize,
    }

    impl RecordingBookingRepo {
        fn new(
            state: Option<BookingStateRow>,
            cancelled: Option<CancelReservedBookingRow>,
        ) -> Self {
            Self {
                state: Mutex::new(state),
                cancelled: Mutex::new(cancelled),
                get_calls: AtomicUsize::new(0),
                cancel_calls: AtomicUsize::new(0),
            }
        }

        fn get_calls(&self) -> usize {
            self.get_calls.load(Ordering::SeqCst)
        }

        fn cancel_calls(&self) -> usize {
            self.cancel_calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl BookingRepo for RecordingBookingRepo {
        async fn get_booking_state(
            &self,
            _booking_id: &str,
        ) -> Result<Option<BookingStateRow>, BookingRepoError> {
            self.get_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.state.lock().expect("mutex poisoned").clone())
        }

        async fn cancel_reserved_booking(
            &self,
            _booking_id: &str,
        ) -> Result<Option<CancelReservedBookingRow>, BookingRepoError> {
            self.cancel_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.cancelled.lock().expect("mutex poisoned").clone())
        }
    }

    struct CannotCancelBookingRepo {
        state: Mutex<Option<BookingStateRow>>,
        get_calls: AtomicUsize,
        cancel_calls: AtomicUsize,
    }

    impl CannotCancelBookingRepo {
        fn new(state: Option<BookingStateRow>) -> Self {
            Self {
                state: Mutex::new(state),
                get_calls: AtomicUsize::new(0),
                cancel_calls: AtomicUsize::new(0),
            }
        }

        fn get_calls(&self) -> usize {
            self.get_calls.load(Ordering::SeqCst)
        }

        fn cancel_calls(&self) -> usize {
            self.cancel_calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl BookingRepo for CannotCancelBookingRepo {
        async fn get_booking_state(
            &self,
            _booking_id: &str,
        ) -> Result<Option<BookingStateRow>, BookingRepoError> {
            self.get_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.state.lock().expect("mutex poisoned").clone())
        }

        async fn cancel_reserved_booking(
            &self,
            _booking_id: &str,
        ) -> Result<Option<CancelReservedBookingRow>, BookingRepoError> {
            self.cancel_calls.fetch_add(1, Ordering::SeqCst);
            Err(BookingRepoError::CannotCancel)
        }
    }

    #[derive(Default)]
    struct RecordingEventPublisher {
        consultation_events: Mutex<Vec<ConsultationEvent>>,
    }

    impl RecordingEventPublisher {
        fn consultation_events(&self) -> Vec<ConsultationEvent> {
            self.consultation_events
                .lock()
                .expect("mutex poisoned")
                .clone()
        }
    }

    #[async_trait::async_trait]
    impl EventPublisher for RecordingEventPublisher {
        async fn publish_consultation_event(
            &self,
            event: ConsultationEvent,
        ) -> Result<(), anyhow::Error> {
            self.consultation_events
                .lock()
                .expect("mutex poisoned")
                .push(event);
            Ok(())
        }

        async fn publish_doctor_timeslot_config_changed_event(
            &self,
            _event: DoctorTimeslotConfigChangedEvent,
        ) -> Result<(), anyhow::Error> {
            Ok(())
        }
    }

    fn user_identity() -> UserIdentity {
        UserIdentity {
            account_id: 1,
            account_type: AccountType::Patient,
            user_profile_id: 2,
            user_main_profile_id: 2,
            tenant_id: 3,
            oidc_user_id: Some("patient".to_string()),
            legacy_data: None,
        }
    }

    fn unauthorized_patient_identity() -> UserIdentity {
        UserIdentity {
            account_id: 999,
            account_type: AccountType::Patient,
            user_profile_id: 888,
            user_main_profile_id: 888,
            tenant_id: 3,
            oidc_user_id: Some("other-patient".to_string()),
            legacy_data: None,
        }
    }

    fn booking_state(
        reservation_status: &str,
        appointment_status: Option<&str>,
    ) -> BookingStateRow {
        BookingStateRow {
            booking_id: "booking-1".to_string(),
            patient_account_id: 1,
            patient_profile_id: 2,
            tenant_id: 3,
            doctor_id: 4,
            biz_unit_id: 5,
            reservation_status: reservation_status.to_string(),
            appointment_status: appointment_status.map(ToOwned::to_owned),
            reserved_until: 1_000,
            appointment_start: 2_000,
            appointment_end: 3_000,
        }
    }

    fn cancelled_booking(state_changed: bool) -> CancelReservedBookingRow {
        CancelReservedBookingRow {
            booking_id: "booking-1".to_string(),
            patient_account_id: 1,
            patient_profile_id: 2,
            tenant_id: 3,
            doctor_id: 4,
            biz_unit_id: 5,
            reservation_status: "CANCELLED".to_string(),
            appointment_status: None,
            cancelled_at: 4_000,
            state_changed,
        }
    }

    fn service_with(
        repo: Arc<RecordingBookingRepo>,
        event_publisher: Arc<RecordingEventPublisher>,
    ) -> BookingService {
        BookingService::new(repo, event_publisher)
    }

    #[test]
    fn derives_booking_lifecycle_from_reservation_and_appointment_status() {
        assert_eq!(
            lifecycle_from_statuses("RESERVED", None),
            BookingLifecycle::Reserved
        );
        assert_eq!(
            lifecycle_from_statuses("CONFIRMED", Some("BOOKED")),
            BookingLifecycle::Booked
        );
        assert_eq!(
            lifecycle_from_statuses("RESERVE_EXPIRED", None),
            BookingLifecycle::ReserveExpired
        );
    }

    #[tokio::test]
    async fn cancel_reserved_booking_does_not_immediately_publish_when_state_changed() {
        let repo = Arc::new(RecordingBookingRepo::new(
            Some(booking_state("RESERVED", None)),
            Some(cancelled_booking(true)),
        ));
        let event_publisher = Arc::new(RecordingEventPublisher::default());
        let service = service_with(repo.clone(), event_publisher.clone());

        let response = service
            .cancel("booking-1", Some(&user_identity()))
            .await
            .expect("reserved booking should cancel");

        assert_eq!(response.booking_id, "booking-1");
        assert_eq!(response.state, BookingLifecycle::Cancelled);
        assert_eq!(repo.get_calls(), 1);
        assert_eq!(repo.cancel_calls(), 1);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn cancel_reserved_booking_publishes_nothing_when_state_is_unchanged() {
        for reservation_status in ["RESERVED", "CANCELLED"] {
            let repo = Arc::new(RecordingBookingRepo::new(
                Some(booking_state(reservation_status, None)),
                Some(cancelled_booking(false)),
            ));
            let event_publisher = Arc::new(RecordingEventPublisher::default());
            let service = service_with(repo.clone(), event_publisher.clone());

            service
                .cancel("booking-1", Some(&user_identity()))
                .await
                .expect("idempotent cancellation should succeed");

            assert_eq!(repo.get_calls(), 1);
            assert_eq!(repo.cancel_calls(), 1);
            assert!(event_publisher.consultation_events().is_empty());
        }
    }

    #[tokio::test]
    async fn cancel_rejects_booked_or_confirmed_before_calling_cancel_repo() {
        for state in [
            booking_state("RESERVED", Some("BOOKED")),
            booking_state("CONFIRMED", None),
        ] {
            let repo = Arc::new(RecordingBookingRepo::new(
                Some(state),
                Some(cancelled_booking(true)),
            ));
            let event_publisher = Arc::new(RecordingEventPublisher::default());
            let service = service_with(repo.clone(), event_publisher.clone());

            let error = service
                .cancel("booking-1", Some(&user_identity()))
                .await
                .expect_err("booked or confirmed booking should not be cancellable");

            assert!(matches!(error, BookingError::CannotCancel));
            assert_eq!(repo.get_calls(), 1);
            assert_eq!(repo.cancel_calls(), 0);
            assert!(event_publisher.consultation_events().is_empty());
        }
    }

    #[tokio::test]
    async fn cancel_rejects_expired_reservation_before_calling_cancel_repo() {
        let repo = Arc::new(RecordingBookingRepo::new(
            Some(booking_state("RESERVE_EXPIRED", None)),
            Some(cancelled_booking(true)),
        ));
        let event_publisher = Arc::new(RecordingEventPublisher::default());
        let service = service_with(repo.clone(), event_publisher.clone());

        let error = service
            .cancel("booking-1", Some(&user_identity()))
            .await
            .expect_err("expired reservation should not be cancellable");

        assert!(matches!(error, BookingError::CannotCancel));
        assert_eq!(repo.get_calls(), 1);
        assert_eq!(repo.cancel_calls(), 0);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn cancel_maps_repo_cannot_cancel_to_service_cannot_cancel_without_publish() {
        let repo = Arc::new(CannotCancelBookingRepo::new(Some(booking_state(
            "RESERVED", None,
        ))));
        let event_publisher = Arc::new(RecordingEventPublisher::default());
        let service = BookingService::new(repo.clone(), event_publisher.clone());

        let error = service
            .cancel("booking-1", Some(&user_identity()))
            .await
            .expect_err("repository cannot-cancel should surface as booking cannot-cancel");

        assert!(matches!(error, BookingError::CannotCancel));
        assert_eq!(repo.get_calls(), 1);
        assert_eq!(repo.cancel_calls(), 1);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn cancel_maps_post_db_non_cancelled_reservation_to_cannot_cancel_without_publish() {
        let mut post_db_result = cancelled_booking(true);
        post_db_result.reservation_status = "RESERVE_EXPIRED".to_string();
        let repo = Arc::new(RecordingBookingRepo::new(
            Some(booking_state("RESERVED", None)),
            Some(post_db_result),
        ));
        let event_publisher = Arc::new(RecordingEventPublisher::default());
        let service = service_with(repo.clone(), event_publisher.clone());

        let error = service
            .cancel("booking-1", Some(&user_identity()))
            .await
            .expect_err("post-db expired reservation should map to cannot-cancel");

        assert!(matches!(error, BookingError::CannotCancel));
        assert_eq!(repo.get_calls(), 1);
        assert_eq!(repo.cancel_calls(), 1);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn cancel_rejects_unauthorized_patient_before_calling_cancel_repo() {
        let repo = Arc::new(RecordingBookingRepo::new(
            Some(booking_state("RESERVED", None)),
            Some(cancelled_booking(true)),
        ));
        let event_publisher = Arc::new(RecordingEventPublisher::default());
        let service = service_with(repo.clone(), event_publisher.clone());

        let error = service
            .cancel("booking-1", Some(&unauthorized_patient_identity()))
            .await
            .expect_err("other patient should not cancel this booking");

        assert!(matches!(error, BookingError::Unauthorized));
        assert_eq!(repo.get_calls(), 1);
        assert_eq!(repo.cancel_calls(), 0);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn get_state_rejects_unauthorized_patient() {
        let repo = Arc::new(RecordingBookingRepo::new(
            Some(booking_state("RESERVED", None)),
            None,
        ));
        let event_publisher = Arc::new(RecordingEventPublisher::default());
        let service = service_with(repo.clone(), event_publisher.clone());

        let error = service
            .get_state("booking-1", Some(&unauthorized_patient_identity()))
            .await
            .expect_err("other patient should not read this booking state");

        assert!(matches!(error, BookingError::Unauthorized));
        assert_eq!(repo.get_calls(), 1);
        assert_eq!(repo.cancel_calls(), 0);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn cancel_returns_not_found_without_cancel_or_publish_when_state_is_missing() {
        let repo = Arc::new(RecordingBookingRepo::new(
            None,
            Some(cancelled_booking(true)),
        ));
        let event_publisher = Arc::new(RecordingEventPublisher::default());
        let service = service_with(repo.clone(), event_publisher.clone());

        let error = service
            .cancel("missing-booking", Some(&user_identity()))
            .await
            .expect_err("missing booking state should return not found");

        assert!(matches!(error, BookingError::NotFound));
        assert_eq!(repo.get_calls(), 1);
        assert_eq!(repo.cancel_calls(), 0);
        assert!(event_publisher.consultation_events().is_empty());
    }
}
