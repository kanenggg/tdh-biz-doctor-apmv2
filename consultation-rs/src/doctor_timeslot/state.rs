use crate::doctor_timeslot::get_timeslot::service::GetDoctorTimeslotService;
use crate::doctor_timeslot::reserved_timeslot::service::ReservedTimeslotsService;

#[derive(Clone)]
pub struct AppState {
    pub get_timeslot_service: GetDoctorTimeslotService,
    pub reserved_timeslots_service: ReservedTimeslotsService,
}
