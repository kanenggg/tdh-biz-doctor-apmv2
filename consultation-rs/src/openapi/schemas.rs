use utoipa::ToSchema;

#[derive(ToSchema)]
pub struct BookingIdResponse {
    pub booking_id: i64,
}
