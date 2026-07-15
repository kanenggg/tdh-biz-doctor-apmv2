# Doctor Snapshot on Appointments — Plan

## Context

The appointment **list** endpoint (`GET /v2/internal/appointments`, backed by
`v2.list_fulfilled_appointments_by_patient`) currently **mocks** the doctor's
name: `service::mock_doctor` returns `first_name: "Doctor"` and
`last_name: "#{account_id}"`. The IDs are real; the names are placeholders.

The reservation row stores only doctor **IDs** (`doctor_id`,
`doctor_account_id`, `doctor_profile_id`) — no name, no image. There is no
doctor name anywhere in the `v2` schema. However, the display info *is* known
upstream at booking time: the `ConsultationBooked` event already carries
`doctor_name`, `doctor_specialties`, and `doctor_image_url`.

This plan replaces the mock with a real **snapshot-on-write**: the doctor's
display name and image are captured onto the booking when the appointment is
created, and read straight back. This makes the read path a self-contained
single-table read and preserves the doctor's name *as it was at the time of
the visit* (correct for a fulfilled / historical appointment).

This endpoint family is **internal-only**, mounted under `/v2/internal/*`.

---

## Decisions Log

| Question | Decision |
|----------|----------|
| Read-time lookup vs. snapshot-on-write? | **Snapshot-on-write** — historically accurate, self-contained reads, matches existing denormalization (`payment_tx_id`, `prescreen`) |
| What to snapshot? | `name` + `image_url` only. **Specialty dropped.** |
| Name shape? | Single `name` string — **not** `first_name`/`last_name`. All upstream sources are a single combined string; splitting is locale-fragile (Thai names, titles) |
| Where to store? | New columns on **`v2.reservation`**, co-located with the existing `doctor_*` IDs |
| Required or optional on the write request? | **Optional** (`Option<String>`), `DEFAULT NULL` in the DB function — preserves back-compat for current callers |
| Which write paths get it? | **Both** `create_appointment_internal` and `create_confirmed_appointment` |
| Reads for rows with no snapshot? | Expose `Option<String>` → JSON `null`. **No backfill** (no clean source; `reservation` has only IDs) |
| `get_detail` enriched too? | **No** — list only this slice. `get_detail` enrichment is an independent follow-up PR |
| Blank-name handling? | Normalize blank → `NULL` in the DB function (`NULLIF(BTRIM(...), '')`), so "absent" has one representation. Same for `image_url` |

---

## Schema Changes

New **nullable** columns on `v2.reservation`:

- `doctor_name text`
- `doctor_image_url text`

## Write Path

Protocol (`common-rs/src/tdh_protocol/internal.rs`) — add to **both**
`CreateAppointmentRequest` and `CreateConfirmedInstantAppointmentRequest`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub doctor_name: Option<String>,        // camelCase: doctorName
#[serde(default, skip_serializing_if = "Option::is_none")]
pub doctor_image_url: Option<String>,   // camelCase: doctorImageUrl
```

DB functions `v2.create_appointment_internal` and
`v2.create_confirmed_appointment` — add new params **appended at the end**
with `DEFAULT NULL` (preserves existing positional sqlx binds):

```sql
p_doctor_name text DEFAULT NULL,
p_doctor_image_url text DEFAULT NULL
```

Insert into `v2.reservation`, normalizing blank → NULL:

```sql
doctor_name      = NULLIF(BTRIM(p_doctor_name), ''),
doctor_image_url = NULLIF(BTRIM(p_doctor_image_url), '')
```

Plumb the two new fields through `InternalRepo` (`internal/repo.rs`) as new
binds on both create calls.

## Read Path — list only

`v2.list_fulfilled_appointments_by_patient` — DROP + recreate: add
`doctor_name` and `doctor_image_url` to the `RETURNS TABLE` signature and to
the `SELECT ... FROM v2.reservation r`.

`list/repo.rs` `AppointmentListRow` — add:

```rust
pub doctor_name: Option<String>,
pub doctor_image_url: Option<String>,
```

`list/model.rs` `AppointmentDoctor` — replace `first_name`/`last_name` with:

```rust
pub name: Option<String>,
pub image_url: Option<String>,
```

(Safe API change: endpoint is internal and the names are currently mocked, so
no real consumer depends on them.)

`list/service.rs` — **delete `mock_doctor`** and its TODO; map row → model
directly, `NULL` → JSON `null`.

## Out of Scope

- `get_detail` enrichment (independent follow-up PR)
- Historical backfill of existing rows (separate batch job if ever needed)
- Specialty snapshot

## Shape of the PR

One tight PR: migration (2 columns + 3 function rewrites) + protocol fields +
repo plumbing + list read + mock deletion.
