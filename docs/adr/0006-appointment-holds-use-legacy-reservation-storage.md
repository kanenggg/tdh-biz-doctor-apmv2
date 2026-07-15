# Appointment Hold, Appointment, and Doctor Occupancy replace legacy reservation storage before launch

## Status

Accepted

## Context

Patients can select a doctor/time before payment. This creates many possible unpaid attempts that may be abandoned or time out. Those attempts should not be treated as booked Appointments because most may never become paid clinical engagements.

Existing ADRs establish the surrounding constraints:

- [ADR 0003](0003-booking-conflicts-use-doctor-occupancy-window.md): booking conflicts use doctor occupancy time windows, not slot-id equality.
- [ADR 0004](0004-booking-state-uses-append-only-events-with-transactional-projections.md): booking state, audit events, and occupancy state are updated transactionally.
- [ADR 0005](0005-booking-intents-share-reservation-and-occupancy-after-doctor-selection.md): booking intents share the same reservation/booking/doctor occupancy model after a doctor is selected.

Current `v2.reservation` storage is legacy combined storage. It carries data for the pre-payment hold, booked appointment source data, and the doctor occupancy window used for conflict checks. The table name does not define a canonical standalone Reservation domain concept.

A previous version of this ADR chose not to split storage yet, deferring a clean separation until Doctor Occupancy needed an independent lifecycle. That constraint has changed: booking is not launched, so there is no launched booking compatibility constraint requiring preservation of the legacy `v2.reservation` persistence shape for future split work.

## Decision

Use the domain model below in documentation and module language:

- **Appointment Hold** is the pre-payment attempt created when a patient selects a doctor/time before payment, entitlement confirmation, or another booking condition.
- **Appointment** is the confirmed booked record that exists only after booking conditions succeed, such as payment or entitlement confirmation.
- **Doctor Occupancy** is the blocked doctor capacity window used for overlap/conflict checks.
- **Reservation** remains legacy wording for existing storage, APIs, and historical event names; it is not a new domain aggregate.

Re-architect now. Because booking has not launched, the accepted target is a fresh DB/module shape that splits the legacy reservation concerns instead of preserving `v2.reservation` as the long-term source of truth.

Target storage/module shape:

- `appointment_hold`: owns pre-payment hold identity, hold status, expiry/release state, selected patient, selected doctor/time, and booking-condition correlation needed before confirmation.
- `appointment`: owns confirmed booked appointment identity, booked lifecycle state, appointment-facing reads, and links needed by consultation/session workflows.
- `doctor_occupancy`: owns doctor capacity windows used for conflict detection, including occupancy state and references back to the hold or appointment that consumes capacity.

Module boundaries should follow the same split:

- Appointment Hold module: create/hold, release, expire, and payment/entitlement confirmation handoff.
- Appointment module: book/confirm, cancel booked appointments, and serve appointment reads.
- Doctor Occupancy module: reserve/release capacity and enforce overlap checks transactionally.

Lifecycle vocabulary:

- Before payment: create/hold, release, or expire an Appointment Hold.
- After booking/payment: book or cancel an Appointment.
- Capacity management: create, confirm, release, or expire Doctor Occupancy in lockstep with the hold/appointment transition unless a later ADR grants it a more independent lifecycle.
- During care delivery: complete or terminate a Consultation separately from hold, appointment, and occupancy lifecycle terms.

Prescreen and follow-up modeling:

- Prescreen is patient-created intake data for a patient-created Appointment Hold.
- A confirmed Appointment should carry or reference the Prescreen that came from its source Appointment Hold.
- Doctor follow-up should reuse prior clinical context/Prescreen when appropriate and should not require the patient to fill the same intake form again by default.
- Prefer an Appointment Purpose such as `PATIENT_BOOKING` or `DOCTOR_FOLLOW_UP` over an `is_follow_up` boolean. Purpose explains why the pending intent or Appointment exists.
- Keep lifecycle status, payment status, and patient-acceptance status conceptually separate to avoid generic status explosion.
- FHIR `Appointment.status` is an edge adapter/projection derived from the internal model; it is not the internal source of truth.

## Consequences

Positive:

- Avoids inflating Appointment counts with unpaid attempts.
- Removes the planned half-step of keeping mixed `v2.reservation` storage while only changing vocabulary.
- Aligns schema, modules, and domain language before external booking traffic depends on legacy names.
- Preserves ADR 0003 conflict semantics by making Doctor Occupancy the explicit capacity model.
- Preserves ADR 0004 transactional requirements by keeping hold, appointment, occupancy, audit event, and projection updates in one consistency boundary where required.
- Keeps patient-selected booking and campaign/matching flows aligned after doctor selection as required by ADR 0005.

Negative:

- Requires migration work now across hold creation, expiry/release, payment confirmation, appointment reads, event publishing, and doctor availability conflict checks.
- Requires compatibility choices for any existing internal development data, tests, seeds, or API/event names that still say reservation.
- Increases near-term implementation scope before launch.

Purpose and FHIR mapping consequences:

- Purpose avoids overloading lifecycle status with business origin.
- Payment and acceptance can evolve independently without adding statuses like `PENDING_PAYMENT_AND_ACCEPTANCE`.
- FHIR projections remain possible: patient booking holds can map to `pending`, doctor follow-up proposals to `proposed`, booked Appointments to `booked`, completed clinical sessions to `fulfilled`, and cancellations to `cancelled`.
- Because this mapping is lossy, integrations must not write FHIR status back as the internal lifecycle state.

## Tradeoff

Keeping `v2.reservation` would reduce immediate migration effort, but it would preserve mixed meanings in the source of truth and defer the hardest boundary decisions until later. Because booking is not launched, the safer architectural path is now the cleaner one: split the storage and modules before production consumers depend on the legacy shape.

This ADR supersedes the previous "do not split the database now" stance. The earlier rationale depended on avoiding storage churn under a presumed compatibility constraint. With no launched booking compatibility constraint, preserving legacy storage is no longer the preferred risk reduction strategy.

## Migration implication

Implement a full cutover rather than adding a parallel half-measure that duplicates conflict truth.

Required migration path:

- Define new tables/projections for Appointment Hold, Appointment, and Doctor Occupancy before moving writers.
- Backfill any existing development or pre-launch rows from `v2.reservation` into the new target shape, or explicitly discard non-production seed/test data where allowed.
- Move write paths transactionally so hold creation also creates Doctor Occupancy when doctor/time is known.
- Move confirmation paths so successful payment/entitlement books an Appointment and confirms or transfers the related Doctor Occupancy.
- Move release/expiry paths so an expired or released Appointment Hold releases the related Doctor Occupancy.
- Retire `v2.reservation` as a long-term source of truth after readers, writers, tests, seeds, and outbox publishers use the split shape.
- Keep API/event compatibility only where existing consumers require names such as booking or reservation; do not let those names define the new domain aggregates.

Integration tests are required for:

- unpaid Appointment Hold creation with Doctor Occupancy conflict protection;
- unpaid hold expiry/release releasing occupancy;
- paid booking confirmation creating a confirmed Appointment and preserving occupancy conflict protection;
- booked Appointment cancellation and occupancy release/update;
- consultation start/completion/termination still resolving the confirmed Appointment correctly;
- outbox/audit events remaining transactional across hold, appointment, and occupancy transitions.
- Prescreen reuse for doctor follow-up without requiring duplicate patient intake.
- FHIR status projection from internal purpose/lifecycle/payment/acceptance state.
