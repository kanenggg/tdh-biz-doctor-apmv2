# Core service returns canonical data; BFF owns presentation

`consultation-rs` is a **core** appointment data service. Its internal read endpoints
(e.g. `GET /v2/internal/appointment/{bookingId}/past-visit`) return **canonical** data —
epoch seconds for time, canonical lowercase channel enums, the stored `FollowUp` enum,
and doctor **IDs only** — never display-shaped data.

The frontend "past visit" spec that motivated this endpoint (HH:MM clock strings +
`appointmentDate`, `followUp.__type = "ScheduleAppointment"`, specialty *names* as flat
strings, doctor `name`/`specialties`, the `HasDrugAllergies`/`NoDrugAllergies` tagged union,
`"Video"` casing) is a **BFF → frontend** contract. The BFF performs that transform; the core
does not.

## Why

- **Single source of truth, no display coupling.** Wall-clock formatting needs a timezone
  (Asia/Bangkok), which is a presentation concern. Baking it into a core data service would pin
  display rules into shared infrastructure.
- **Ownership.** A doctor's name and specialties are owned by the doctor/IAM service and are only
  cached (in Redis) for the BFF's benefit. The core returns the IDs from `v2.reservation` and
  stays off that cache — which cannot be trusted for historical (fulfilled) appointments anyway.
- **Consistency.** The sibling `get_detail` endpoint already returns canonical epoch/enum data to
  internal callers; past-visit follows the same convention.

## Consequence

A reader comparing the frontend spec to this service will see deliberate shape differences
(`Appointment` not `ScheduleAppointment`, epoch not HH:MM, doctor IDs not names). That is
intentional — the mapping lives in the BFF, not here.
