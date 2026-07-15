# Consultation Summary

The internal, read-only view of a **Fulfilled Appointment**: what happened during a
consultation (summary note, follow-up plan) returned to internal services that call with a
`bookingId`. Assembled live from existing storage — it is not a separate store.

This is a **core** projection named after the domain content it returns (the consultation's
summary note), NOT after any one consumer. The first caller is the BFF "past visit" feature, but
that is a presentation feature, not the domain — other callers (doctor-side consultation history,
record export, etc.) read the same canonical projection. Keep the feature's vocabulary out of this
module's names.

## Language

**Consultation Summary**:
The post-consultation projection of an appointment — the doctor's summary note plus the appointment
envelope — served read-only to internal callers by `bookingId`. Assembled on read from existing
tables; no dedicated snapshot table.
_Avoid_: past visit, visit history, encounter record (these are caller/feature framings, not the
domain).

**Fulfilled Appointment**:
An appointment whose `appointment_status` is `FULFILLED` — the consultation happened and a
summary note exists. A Consultation Summary is only meaningful for these.
_Avoid_: completed appointment, past appointment.

**Summary Note**:
The doctor's clinical record for a consultation (present illness, chief complaint, diagnosis,
recommendations, ICD-10, illness duration, follow-up). Stored KMS-encrypted in
`v2.doctor_summary_note.encrypted_data` as a serialized `SummarizationRequest`.

## Relationships

- A **Fulfilled Appointment** has exactly one **Summary Note**
- A **Consultation Summary** is derived from one **Fulfilled Appointment** + its **Summary Note**

**Drug Allergy**:
A doctor-curated drug-allergy entry (`{ id, description }`) captured at summarization. New
field on the summary note; read back as part of the Consultation Summary. Distinct from the patient's
free-text `prescreen.allergies` declared at booking.

## Scope

This is a **bounded** projection: the summary note + appointment envelope, nothing more. It is
deliberately NOT an aggregate clinical record. `prescriptionItems`, labs, attachments, and vitals
are out of scope and belong to their own endpoints that the BFF composes. `prescription_id` is kept
as a *reference* only (the BFF fetches the prescription itself).

## Core vs BFF boundary

This service is the **core** appointment data service. It returns **canonical** data (epoch
seconds, canonical enums, structured doctor/specialty objects, the stored `FollowUp` enum). The
**BFF owns presentation shaping** — the originating spec (HH:MM clock strings, `appointmentDate`,
specialty *names* as flat strings, `followUp.__type = "ScheduleAppointment"`, the
`HasDrugAllergies`/`NoDrugAllergies` tagged union, `"Video"` casing) is the BFF→frontend contract,
NOT this service's contract. Core never formats for display.

## Resolved decisions

- Storage model: **read-side assembler** — no snapshot table; assemble live from existing tables.
- Time: **epoch seconds** via shared `AppointmentTime { start_time, end_time }`; no HH:MM / date
  strings (BFF formats; timezone is a BFF concern).
- `prescriptionItems` — OUT OF SCOPE (excluded from payload).
- `drugAllergies` — IN SCOPE, sourced by **adding the field to `SummarizationRequest`** (doctor
  captures at summarization → encrypted into `doctor_summary_note` → read back here).
- Envelope fields (appointmentDate, appointmentTime, consultationChannel, doctor block) — KEPT
  per spec.
- Response contract: 200-always tagged enum mirroring `get_detail` —
  `Success(ConsultationSummary) | NotFound | NotFulfilled`. Non-FULFILLED status → distinct
  `NotFulfilled`; missing booking or missing summary note → `NotFound`; DB/KMS errors → HTTP 500.
- `followUp` / `consultationChannel` / `appointmentTime` returned as canonical (stored `FollowUp`
  enum, canonical channel enum, epoch) — no re-shaping in core.
- `doctor` block: **IDs only** (`doctorId`, `doctorAccountId`, `doctorProfileId`) from
  `reservation`. No name/specialties, no Redis/`DoctorProfileCache` dependency. BFF resolves the
  profile.

- Route: `GET /v2/internal/appointment/{bookingId}/consultation-summary` — internal, no-auth,
  network-policy-protected, on the `internal` router beside `get_detail`.
- `prescriptionItems` out, but `prescription_id` reference is KEPT (free column on summary note;
  BFF fetches the prescription itself).

## Resolved ambiguities

- Spec `followUp.__type = "ScheduleAppointment"` → core returns canonical `FollowUp::Appointment`;
  BFF remaps the name.
- `appointmentTime` → epoch `i64` (canonical), not HH:MM strings.
- `doctor.id`/name/specialties → core returns IDs only; name/specialties are BFF concerns.
- Module/route named `consultation_summary` (domain content), not `past_visit` (the first caller's
  feature). The BFF "past visit" screen is one consumer of this canonical projection.
