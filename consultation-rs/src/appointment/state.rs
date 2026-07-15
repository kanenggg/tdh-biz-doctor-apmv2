use crate::appointment::consultation_summary::service::ConsultationSummaryService;
use crate::appointment::get_detail::service::GetAppointmentDetailService;
use crate::appointment::list::service::ListAppointmentsService;
use crate::doctor_timeslot::reserved_timeslot::service::ReservedTimeslotsService;

#[derive(Clone)]
pub struct AppState {
    pub get_appointment_detail_service: GetAppointmentDetailService,
    pub consultation_summary_service: ConsultationSummaryService,
    pub list_appointments_service: ListAppointmentsService,
    pub reserved_timeslots_service: ReservedTimeslotsService,
}
