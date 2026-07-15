use crate::appointment::hold::create::AppointmentHoldService;
use crate::appointment::hold::handler::PublicBookingRateLimiter;
use crate::appointment::hold::service::BookingService;

#[derive(Clone)]
pub struct AppState {
    pub appointment_hold_service: AppointmentHoldService,
    pub booking_service: BookingService,
    pub public_booking_rate_limiter: PublicBookingRateLimiter,
}
