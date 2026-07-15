# CHANGELOGS

## 2026-04-02 — PaymentChannelResult → PaymentChannel migration & test fixes

### `consultation-rs/src/consultation/common/mod.rs`
- Changed `DbConsultationSession.payment_channels` type from `Option<Json<Vec<PaymentChannelResult>>>` to `Option<Json<Vec<PaymentChannel>>>` to match the actual DB-stored format (`tdh_protocol::appointment::v2::payment_transaction::PaymentChannel`).

### `consultation-rs/src/summarization/service.rs`
- Updated `handle_follow_up` to use renamed fields from updated `FollowUpAppointment`: `date_time` → `appointment_start`.
- Added `ConsultationChannel` type conversion from `biz_apm::ConsultationChannel` to `consultation::ConsultationChannel` (two different enum types).
- Added `BigDecimal` → `f64` conversion for `consultation_fee` field.

### `tdh-protocol/rust/Cargo.toml`
- Made `thiserror` a non-optional dependency (was used unconditionally by `SummarizationError` but declared `optional = true`).

### `tdh-protocol/rust/src/internal.rs`
- Changed `CreateConfirmedInstantAppointmentRequest.parent_appointment_id` from `Option<i32>` to `Option<String>` to match `varchar(20)` DB column.

### `consultation-rs/tests/it_internal_create_confirmed_apm.rs`
- Fixed all SQL query bindings to use `&str` instead of `i64` for varchar columns (`booking_id`, `appointment_id`).
- Fixed `cleanup_created_appointment` helper to accept `&str`.
- Updated appointment status assertion from `"CONFIRMED"` to `"BOOKED"` (FHIR enum migration).
- Fixed date prefix assertion to use UTC timestamp (`jiff::Timestamp`) matching DB's `CURRENT_DATE`.
- Updated `parent_appointment_id` to pass `String` instead of `i32`.

### `consultation-rs/tests/it_repo.rs`
- Added missing `reserved_until` column in reservation INSERT for `test_is_facial_verified_non_video_channel`.
- Fixed enum value casing (`'CONFIRMED'` → `'Instant'`/`'video'`) and added missing columns to match schema.

### `consultation-rs/tests/common/mod.rs`
- Added `run_seeds()` function to load seed data automatically during test setup.

### DB seed files
- Updated all seed SQL to include required columns (`reserved_until`, `prescreen_data_id`, `appointment_start`, `consult_duration`, `appointment_end`).
- Changed `booking_id` values from integer to string format.
- Changed `appointment_status` values to FHIR-compatible enum values (`BOOKED`, `FULFILLED`).
- Added `appointment_payment_transaction` seed data for test bookings 10001 and 10002.
