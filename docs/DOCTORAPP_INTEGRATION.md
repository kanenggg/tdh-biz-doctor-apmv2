# DoctorApp integration

APMv2 integrates with `tdh-mordee-doctor-app` through active HTTP and Pub/Sub
contracts. The
[canonical cross-system guide](https://github.com/kanenggg/tdh-mordee-doctor-app/blob/main/docs/contracts/doctorapp-apmv2-integration.md)
is maintained by DoctorApp.

## APMv2 responsibilities

- `consultation-rs` provides `consultation-rs-api`.
- APMv2 services provide `biz-apm-published-events`.
- `consultation-bg-rs` consumes DoctorApp's `doctor-profile-events` and stores
  a read-only doctor identity and consultation-configuration projection.
- APMv2 owns Doctor Operational Availability, Appointment Holds, Doctor
  Occupancy, booking, and consultation execution.

DoctorApp remains the only editable authority for Doctor Profile and Doctor
Consultation Configuration. APMv2 must not treat its projection as an
independent configuration source.

## Contract and rollout references

- [DoctorApp-owned `doctor-profile-events` AsyncAPI](https://github.com/kanenggg/tdh-mordee-doctor-app/blob/main/specs/provides/doctor-profile-approved.asyncapi.yaml)
- [APMv2 Consultation OpenAPI](https://github.com/kanenggg/tdh-biz-doctor-apmv2/blob/main/specs/provides/consultation-rs.yaml)
- [APMv2 runtime events AsyncAPI](https://github.com/kanenggg/tdh-biz-doctor-apmv2/blob/main/specs/provides/biz-apm-published-events.asyncapi.yaml)
- [Doctor projection rollout](plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md)
- [APMv2 Backstage catalog](https://github.com/kanenggg/tdh-biz-doctor-apmv2/blob/main/catalog-info.yaml)
