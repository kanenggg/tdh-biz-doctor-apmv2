use std::sync::Arc;

use super::model::{
    AppointmentDoctor, AppointmentList, AppointmentListItem, ListAppointmentsResponse,
};
use super::repo::{AppointmentListRow, ListAppointmentsRepo};
use crate::appointment::types::AppointmentTime;

#[derive(Debug, thiserror::Error)]
pub enum ListAppointmentsError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct ListAppointmentsService {
    repo: Arc<dyn ListAppointmentsRepo>,
}

impl ListAppointmentsService {
    pub fn new(repo: Arc<dyn ListAppointmentsRepo>) -> Self {
        Self { repo }
    }

    pub async fn list_appointments(
        &self,
        patient_account_id: i32,
        patient_profile_id: Option<i32>,
    ) -> Result<ListAppointmentsResponse, ListAppointmentsError> {
        let rows = self
            .repo
            .list_fulfilled_appointments(patient_account_id, patient_profile_id)
            .await?;

        let appointments = rows.into_iter().map(row_to_item).collect();
        Ok(ListAppointmentsResponse::Success(AppointmentList {
            appointments,
        }))
    }
}

fn row_to_item(row: AppointmentListRow) -> AppointmentListItem {
    AppointmentListItem {
        booking_id: row.booking_id,
        appointment_time: AppointmentTime {
            start_time: row.appointment_start.to_jiff().as_second(),
            end_time: row.appointment_end.to_jiff().as_second(),
        },
        doctor: mock_doctor(row.doctor_account_id, row.doctor_profile_id),
    }
}

// TODO: the doctor name snapshot is not yet persisted on the appointment record.
// Until the write path snapshots first/last name onto the booking, the name is
// mocked here — `last_name` is derived from `doctor_account_id` so a list with
// multiple doctors still renders distinctly downstream. The IDs are real; do NOT
// treat these names as real.
fn mock_doctor(account_id: i32, profile_id: i32) -> AppointmentDoctor {
    AppointmentDoctor {
        account_id,
        profile_id,
        first_name: "Doctor".to_string(),
        last_name: format!("#{account_id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct RowsRepo {
        rows: Vec<AppointmentListRow>,
    }

    #[async_trait::async_trait]
    impl ListAppointmentsRepo for RowsRepo {
        async fn list_fulfilled_appointments(
            &self,
            _patient_account_id: i32,
            _patient_profile_id: Option<i32>,
        ) -> Result<Vec<AppointmentListRow>, anyhow::Error> {
            Ok(self.rows.clone())
        }
    }

    struct CapturingRepo {
        calls: Mutex<Vec<(i32, Option<i32>)>>,
    }

    #[async_trait::async_trait]
    impl ListAppointmentsRepo for CapturingRepo {
        async fn list_fulfilled_appointments(
            &self,
            patient_account_id: i32,
            patient_profile_id: Option<i32>,
        ) -> Result<Vec<AppointmentListRow>, anyhow::Error> {
            self.calls
                .lock()
                .expect("test calls mutex poisoned")
                .push((patient_account_id, patient_profile_id));
            Ok(vec![])
        }
    }

    struct ErrorRepo;

    #[async_trait::async_trait]
    impl ListAppointmentsRepo for ErrorRepo {
        async fn list_fulfilled_appointments(
            &self,
            _patient_account_id: i32,
            _patient_profile_id: Option<i32>,
        ) -> Result<Vec<AppointmentListRow>, anyhow::Error> {
            Err(anyhow::anyhow!("repo unavailable"))
        }
    }

    fn ts(seconds: i64) -> jiff_sqlx::Timestamp {
        jiff_sqlx::Timestamp::from(
            jiff::Timestamp::from_second(seconds).expect("test timestamp should be valid"),
        )
    }

    fn make_service(rows: Vec<AppointmentListRow>) -> ListAppointmentsService {
        ListAppointmentsService::new(Arc::new(RowsRepo { rows }))
    }

    #[tokio::test]
    async fn list_appointments_returns_database_error_when_repo_fails() {
        let service = ListAppointmentsService::new(Arc::new(ErrorRepo));
        let err = service
            .list_appointments(1, Some(2))
            .await
            .expect_err("repo failure should surface as DatabaseError");

        assert!(matches!(err, ListAppointmentsError::DatabaseError(_)));
    }

    #[tokio::test]
    async fn list_appointments_passes_patient_filters_to_repo() {
        let repo = Arc::new(CapturingRepo {
            calls: Mutex::new(vec![]),
        });
        let service = ListAppointmentsService::new(repo.clone());

        let response = service
            .list_appointments(42, Some(99))
            .await
            .expect("capturing repo should succeed");
        let ListAppointmentsResponse::Success(list) = response;
        assert!(list.appointments.is_empty());

        let response = service
            .list_appointments(43, None)
            .await
            .expect("capturing repo should succeed");
        let ListAppointmentsResponse::Success(list) = response;
        assert!(list.appointments.is_empty());

        assert_eq!(
            repo.calls
                .lock()
                .expect("test calls mutex poisoned")
                .as_slice(),
            &[(42, Some(99)), (43, None)]
        );
    }

    #[tokio::test]
    async fn list_appointments_maps_repo_rows_to_success() {
        let service = make_service(vec![
            AppointmentListRow {
                booking_id: "booking-1".to_string(),
                appointment_start: ts(1_701_000_000),
                appointment_end: ts(1_701_003_600),
                doctor_account_id: 101,
                doctor_profile_id: 201,
            },
            AppointmentListRow {
                booking_id: "booking-2".to_string(),
                appointment_start: ts(1_702_000_000),
                appointment_end: ts(1_702_001_800),
                doctor_account_id: 102,
                doctor_profile_id: 202,
            },
        ]);

        let response = service.list_appointments(1, Some(2)).await.unwrap();

        let ListAppointmentsResponse::Success(list) = response;
        assert_eq!(list.appointments.len(), 2);

        let first = &list.appointments[0];
        assert_eq!(first.booking_id, "booking-1");
        assert_eq!(first.appointment_time.start_time, 1_701_000_000);
        assert_eq!(first.appointment_time.end_time, 1_701_003_600);
        assert_eq!(first.doctor.account_id, 101);
        assert_eq!(first.doctor.profile_id, 201);
        assert_eq!(first.doctor.first_name, "Doctor");
        assert_eq!(first.doctor.last_name, "#101");

        let second = &list.appointments[1];
        assert_eq!(second.booking_id, "booking-2");
        assert_eq!(second.appointment_time.start_time, 1_702_000_000);
        assert_eq!(second.appointment_time.end_time, 1_702_001_800);
        assert_eq!(second.doctor.account_id, 102);
        assert_eq!(second.doctor.profile_id, 202);
        assert_eq!(second.doctor.first_name, "Doctor");
        assert_eq!(second.doctor.last_name, "#102");
    }

    #[tokio::test]
    async fn list_appointments_returns_success_with_empty_list() {
        let service = make_service(vec![]);

        let response = service.list_appointments(1, None).await.unwrap();

        let ListAppointmentsResponse::Success(list) = response;
        assert!(list.appointments.is_empty());
    }
}
