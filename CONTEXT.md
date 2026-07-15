# APMv2 Scheduling and Appointment Context

This context defines the domain language for doctor operational availability, appointment holding, booking, and live clinical consultations. It is intentionally implementation-free.

## Language

**Appointment Hold**:
A temporary pre-booking hold on a selected doctor/time while the patient completes required conditions such as payment or entitlement confirmation. It may expire or be explicitly released without becoming an Appointment.
_Avoid_: Reservation, or calling an unpaid attempt an Appointment.

**Appointment**:
A booked patient-doctor engagement created only after the required booking conditions succeed.
_Avoid_: Using Appointment for a pre-payment attempt.

**Appointment Rescheduled**:
The canonical lifecycle event emitted after a booked Appointment and its Doctor Occupancy are atomically moved to an approved doctor/time window.
_Avoid_: Treating rescheduling as an independent timeslot edit.

**Booking action**:
The domain action that confirms an Appointment Hold into a booked Appointment.
_Avoid_: Modeling Booking as a separate business object.

**Booking ID**:
The stable public correlation identifier exposed as `bookingId` across the hold, payment, Appointment, and event contracts. It connects one booking journey.
_Avoid_: Treating the Booking ID as the aggregate identifier of either the Appointment Hold or Appointment.

**Appointment Hold ID**:
The internal identifier of an Appointment Hold. It is distinct from the public Booking ID even where legacy storage uses the same physical value.
_Avoid_: Naming an Appointment Hold ID `reservationId`.

**Appointment ID**:
The identifier of a booked Appointment. It is distinct from the source Appointment Hold ID and Booking ID.
_Avoid_: Reusing the Hold identifier as though a Hold and Appointment were one object.

**Consultation**:
The live clinical session between patient and doctor that occurs for a booked Appointment.
_Avoid_: Using Consultation for a hold, booking attempt, or scheduling record.

**Doctor ID** (`doctorId` / `doctor_id` in new contracts):
The canonical, stable UUID that identifies a Doctor. New designs, APIs, persistence, and events use this identifier for doctor selection, scheduling, Holds, Appointments, and consultations.
_Avoid_: Using an account ID, profile ID, or legacy Doctor User Identity ID as though it were the canonical Doctor ID.

**Doctor User Identity ID**:
The numeric identifier of the Doctor's legacy user identity. In old events, a field named `doctorId` or `doctor_id` means this identifier, not the canonical Doctor UUID.
_Avoid_: Reinterpreting an old event's numeric `doctorId` as a UUID, or using the legacy identifier in a new contract.

**Doctor Consultation Configuration**:
The Doctor Profile-owned service configuration that determines what consultation services a doctor is eligible to provide: supported channels, supported languages, duration, fee, and profile eligibility. DoctorApp is its sole authority. APMv2 observes a complete, versioned projection.
_Avoid_: Making APMv2 an independently editable source for commercial or profile-owned configuration.

**Doctor Operational Availability**:
The Scheduling-owned configuration and current state that determine when an eligible doctor can be held or booked: weekly and specific-date schedule, scheduled availability, instant availability, Appointment Holds, and Doctor Occupancy. APMv2 is authoritative for this operational state.
_Avoid_: Mixing profile-owned fee, channel, duration, or language editing into operational availability.

**Doctor Occupancy**:
The doctor's blocked capacity window while an Appointment Hold, booked Appointment, or active Consultation consumes doctor time. Its minimal lifecycle statuses are `Active` and `Released`; the owning Hold or Appointment explains why capacity is consumed.
_Avoid_: Treating Doctor Occupancy as the patient-facing Appointment or Hold.

**Doctor Occupancy lifecycle events**:
Scheduling emits `DoctorOccupancyMoved`, `DoctorOccupancyReleased`, and `DoctorOccupancyShortened` as canonical facts about the capacity ledger. Initial release reasons are `HoldExpired`, `HoldCancelled`, `AdministrativeRelease`, and `MigrationCorrection`.
_Avoid_: Calling an Occupancy release an Appointment cancellation.

**Reservation**:
Non-canonical legacy wording. Historically it may refer to an Appointment Hold, Doctor Occupancy, or the booking flow around them.
_Avoid_: Introducing Reservation as a standalone domain object, service, repository, event, or preferred API term.

**Pending Payment Hold**:
An Appointment Hold waiting for payment to succeed before booking is complete.
_Avoid_: Pending Payment Appointment.

**Proposed Appointment**:
A doctor-created follow-up suggestion awaiting patient acceptance.
_Avoid_: Using Proposed Appointment for patient checkout or a Pending Payment Hold.

**Prescreen**:
Patient-provided intake information collected for a patient-created Appointment Hold. A booked Appointment carries or references it after booking succeeds.
_Avoid_: Requiring a new Prescreen for doctor-created follow-up when prior clinical context is intentionally reused.

**Appointment Purpose**:
Why a pending intent or booked Appointment exists, such as patient booking or doctor follow-up. Purpose is separate from lifecycle status.
_Avoid_: Encoding purpose into lifecycle status names.

**FHIR Appointment Status**:
An interoperability projection of internal Appointment lifecycle, not the internal source of truth. The mapping may be lossy.
_Avoid_: Using FHIR status as APMv2's canonical lifecycle model.

**Payment Quote**:
The authoritative immutable price offered for a specific Appointment Hold before payment.
_Avoid_: Accepting a client-supplied amount or treating a display summary as authoritative.

**Payment Summary**:
A patient-facing presentation derived from a Payment Quote.
_Avoid_: Treating Payment Summary as the financial source of truth.

## Lifecycle Vocabulary

- **Create/hold** an **Appointment Hold** before payment or entitlement confirmation.
- **Release** an **Appointment Hold** when the patient explicitly stops before booking.
- **Expire** an **Appointment Hold** when its time limit passes before booking.
- **Book** an **Appointment** when payment, entitlement, or other required conditions succeed.
- **Cancel** a booked **Appointment** only after booking.
- **Complete** or **terminate** a **Consultation** separately from Hold release/expiry and Appointment cancellation.
- **Activate**, **move**, **shorten**, or **release** **Doctor Occupancy** as capacity changes.

## Relationships

- An Appointment Hold may become a booked Appointment through the Booking action.
- One Booking ID correlates the Hold, payment, Appointment, and event contracts.
- An Appointment Hold ID and Appointment ID are distinct internal aggregate identifiers.
- Many Appointment Holds may expire or be released without becoming Appointments.
- Doctor Occupancy may be activated for an Appointment Hold before payment when doctor/time is known.
- Booking an Appointment preserves traceability to the source Appointment Hold and Booking ID.
- A Consultation occurs only for a booked Appointment.
- A canonical Doctor ID is a UUID; a Doctor User Identity ID is a distinct legacy numeric identifier.
- The meaning of a `doctorId`/`doctor_id` wire field is determined by its contract generation: new contracts carry the canonical Doctor UUID, while old events carry the Doctor User Identity ID.
- DoctorApp owns Doctor Consultation Configuration; APMv2 owns Doctor Operational Availability and Doctor Occupancy.
- APMv2 uses projected Doctor Consultation Configuration to decide eligibility but does not edit that configuration.
- Reservation is legacy terminology only; use Appointment Hold, Doctor Occupancy, Appointment, or Booking action according to meaning.

## Resolved Ambiguities

- Unpaid patient-selected attempts are Appointment Holds, not Appointments.
- Reservation is not a canonical aggregate.
- Explicit pre-booking termination releases a Hold; timeout expires it; cancellation applies to a booked Appointment.
- Booking is an action, not an object.
- `bookingId` is public correlation, not the canonical Hold or Appointment identifier.
- Doctor Occupancy is the scheduling capacity ledger, separate from patient-facing lifecycle records.
- Consultation means only the live clinical session.
- `doctorId`/`doctor_id` is not intrinsically a UUID or an integer; its event/API contract determines whether it means the canonical Doctor ID or the legacy Doctor User Identity ID.
- Doctor Consultation Configuration and Doctor Operational Availability have distinct authorities.
- FHIR status is an integration-edge projection, not internal lifecycle authority.
