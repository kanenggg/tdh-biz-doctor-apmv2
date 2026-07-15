# Appointment Hold is the canonical pre-booking domain

## Status

Superseded by [ADR 0006](0006-appointment-holds-use-legacy-reservation-storage.md), 2026-07-13.

## Decision

`appointment::hold` owns creation, state, release, and expiry of an Appointment Hold.
Booking remains the public route and correlation vocabulary, preserving `bookingId` and
the existing aliases. A booked Appointment is cancelled only after booking; an unbooked
Hold is released or expires.

This ADR originally proposed retaining `v2.reservation` as legacy physical storage.
That conflicts with ADR 0006's accepted pre-launch decision to cut over now to
`appointment_hold` + `doctor_occupancy` + `appointment`. It must not be used as
authority for implementation.

The exact thirteen-argument `v2.create_reservation` remains only as a deprecated
old-pod SQL wrapper over `v2.create_appointment_hold`. Both functions write the
canonical Hold and Occupancy tables; neither writes `v2.reservation`.
V1 `TimeslotReserved`, `ReservationCancelled`, and `ReservationExpired` remain explicit
wire adapters until a separately versioned event cutover.

## Consequences

Migrations are applied before deploying pods that call the canonical function. The
deployment is migration, backfill, canonical writers/readers, expiry worker, then
legacy-wrapper caller inventory. Rollback returns application pods to the prior release
only while the wrapper is retained; it does not restore `v2.reservation` as a writer.
Removing the wrapper, legacy table wording, routes, or V1 event discriminators requires
a later approved migration after callers and consumers are inventoried.
