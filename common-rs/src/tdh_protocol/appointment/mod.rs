pub mod appointment_status;
pub mod appointment_type;
pub mod dolphin_appointment_request;
pub mod notify_doctor_action_message;
pub mod reserve;
pub mod v2;

pub use appointment_status::AppointmentStatus;
pub use appointment_type::AppointmentType;
pub use dolphin_appointment_request::DolphinAppointmentRequest;
pub use notify_doctor_action_message::NotifyDoctorActionMessage;
pub use reserve::{ReserveRequest, ReserveResponse};
