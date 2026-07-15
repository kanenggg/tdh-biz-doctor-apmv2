# Doctor service-config projection rollout

Deploy `20260713000001__doctor_service_config_projection.sql` and the additive
`20260713000002__doctor_service_config_projection_source_ordering.sql` first.
The migrations create a DoctorApp-owned snapshot table and source-ordering
columns without altering `v2.doctor_consultation_config`, which remains
operational availability state owned by APMv2. Do **not** allow a generic
migration runner to apply 0003 at this stage: 0003 is activated only after the
explicit quiescence gate below.

Deploy the database migrations, then `consultation-bg-rs`, backfill and
reconcile snapshots, and only then deploy/enable enforcement in
`consultation-rs`. Deploy both services only after that ordering is complete.
The authoritative runtime input is the committed DoctorApp `origin/main`
`DoctorProfileApproved` JSON shape: its complete top-level fields include
`doctorFee`, `doctorFeeCurrency`, `languages`, `durationMinutes`, `channels`,
and `occurredAt`. It does not require `consultationConfig`, `schemaVersion`, or
`profileVersion`. The optional future nested fields remain accepted only as an
additive extension; when nested configuration is present it must equal the
top-level configuration.

For a committed unversioned event, `source_occurred_at` is the ordering key and
the explicit `effective_source_version` is that value. For a versioned event,
positive `profileVersion` is the ordering key and effective source version.
Neither case fabricates a canonical profile version. Holds persist the effective
service-config quote coordinate and payment confirmation compares that immutable
coordinate, so quote consumers work for both producer forms.

The Doctor Profile Pub/Sub subscription must have a dead-letter topic and a
bounded maximum delivery-attempt policy. The consumer returns retryable 5xx for
undecodable or invalid events so Pub/Sub can apply that policy; logs contain
event-validation errors only and never payloads or credentials.

Backfill and reconcile before enabling canonical Hold creation. Missing
snapshots are always treated as unavailable by the canonical Hold function,
including through the deprecated legacy SQL wrapper; no rollout flag may bypass
channel or duration invariants.

Reconcile before enabling: every active doctor with either availability flag set
must have exactly one projection row with non-empty channels, valid duration,
two-decimal nonnegative fee, a source event id, and either a positive
`profile_version` or a non-null `source_occurred_at`/effective source version.
Investigate/fix mismatches and replay the canonical `DoctorProfileApproved`
snapshot before setting `require_v2_snapshot = true`.

## Cutover gate

1. Deploy 0001 and 0002 plus the projection consumer only.
2. Backfill and reconcile the projection, then verify every bookable doctor.
3. Quiesce old Hold/payment writers and confirm no old-pod lease remains.
4. Apply 0003 and deploy canonical readers/writers together.
5. Enable the expiry worker, reconcile Hold/Appointment/Occupancy state, then
   lift quiescence.

The deployment runner must expose an explicit `APMV2_HOLD_CUTOVER_READY=true`
activation gate for 0003; an automatic "apply every file" runner is unsafe.
This prevents an old-pod fail-closed interval while retaining the legacy SQL
wrapper only for the controlled compatibility window. After cutover,
`v2.reservation` is backfill/reconciliation input only and never a runtime
writer, reader, or rollout source of truth.
# Appointment Hold compatibility

APMv2 consumes this projected configuration while transactionally creating an
Appointment Hold. DoctorApp remains the only writer of doctor consultation
configuration. During the rollout, deploy the migration containing
`v2.create_appointment_hold` before pods that use it; the exact legacy
thirteen-argument `v2.create_reservation` wrapper remains for old pods. Both
write `v2.appointment_hold` and `v2.doctor_occupancy`; neither writes
`v2.reservation`. After the pre-launch backfill, switch readers and writers,
then enable `appointment_hold_expiry` in consultation-bg-rs. Rollback may
return pods to the wrapper while it remains, but must not restore the legacy
table as a source of truth.
