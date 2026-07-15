# Patient Verification Mismatch Semantics

**Status:** decision needed before runtime changes  
**Last updated:** 2026-07-08

## Current behavior

The `/v2/consultation/patient-id-verify-miss-match/{booking_id}` endpoint currently records patient verification in the same way as the match endpoint. It does **not** cancel the appointment.

Evidence:

- `consultation-rs/src/consultation/patient_verification/handler.rs` routes the mismatch endpoint to `PatientVerificationService::miss_match_handle`.
- `consultation-rs/src/consultation/patient_verification/service.rs` implements both `match_handle` and `miss_match_handle` with `repo.add_patient_verification(booking_id, doctor_profile_id)`.
- `consultation-rs/src/consultation/patient_verification/repo.rs` contains an unused `canncel_appointment(...)` method that calls `v2.cancel_appointment(...)`.
- Generated OpenAPI currently describes mismatch success as "Patient ID verification mismatch recorded successfully" in `openapi/consultation-rs.yaml` and `openapi/consultation-rs.json`.
- End-session separately supports `PatientVerificationMismatch` as a termination reason, but that belongs to the end-session flow and does not define cancellation behavior for this endpoint.

## Ambiguity

The endpoint name implies a negative verification result, while the implemented behavior only records verification and returns the same success shape as a match. The presence of an unused cancellation repo method suggests cancellation may have been considered, but there is no confirmed product/API contract saying mismatch should cancel an appointment.

## Decision needed

Before changing runtime behavior, confirm the intended contract:

1. Should patient verification mismatch only record the verification attempt/result?
2. Should mismatch cancel the appointment by calling `v2.cancel_appointment(...)`?
3. If cancellation is required, what should happen to consultation session state, events, notifications, and OpenAPI response descriptions?
4. Should the misspelled path segment `miss-match` and repo method `canncel_appointment` be preserved for compatibility or migrated with aliases?

## Safe decision for now

Preserve current runtime behavior until product/API confirmation. Do not change mismatch to cancellation based only on the endpoint name or unused repository method.
