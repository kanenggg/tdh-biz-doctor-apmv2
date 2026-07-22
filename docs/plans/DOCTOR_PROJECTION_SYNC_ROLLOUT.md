# Doctor Projection Sync Rollout

**Status:** conservative ops mitigation / direct sync disabled
**Last updated:** 2026-07-09

## Purpose

DoctorApp has a direct-sync contract for approved doctor profile events into the APM doctor identity projection by calling APM consultation-bg. The current rollout posture is conservative: DoctorApp Cloud Run direct projection sync is disabled in dev, staging, and prod until APM Cloud Run allowlist/auth is implemented and verified. Pub/Sub doctor profile publishing remains enabled as the active fallback path.

## Runtime contract

- Current DoctorApp rollout scope is the backoffice approve flow. Deactivation is not part of this rollout until the DoctorApp deactivate path has a completed status update and event emission contract.
- DoctorApp sends `DoctorProfileEvent` JSON to APM preferred `POST /internal/v1/doctor-projection/sync` only when `consultation_bg_base_uri` / `SERVICE__CONSULTATION_BG_BASE_URI` is non-empty; legacy `POST /internal/doctor-projection/sync` remains accepted as a compatibility alias.
- The consumer baseline is committed DoctorApp `origin/main` `DoctorProfileApproved`: it requires the full identity/display payload and the top-level `doctorFee`, `doctorFeeCurrency`, `languages`, `durationMinutes`, `channels`, `approvedAt`, and `occurredAt` fields. The canonical contract is the DoctorApp-owned Backstage API `doctor-profile-events`, defined at `tdh-mordee-doctor-app/specs/provides/doctor-profile-approved.asyncapi.yaml`.
- Optional `schemaVersion`, `profileVersion`, and nested `consultationConfig` are forward-compatible extensions only. They are not required to process the committed producer event; when nested configuration is sent, it must agree with top-level values.
- DoctorApp authenticates with a Google OAuth bearer token from its runtime service account.
- APM validates the bearer token through Google tokeninfo and only accepts callers whose service-account email is in `[doctor_projection_sync].allowed_service_account_emails`.
- Empty DoctorApp `consultation_bg_base_uri` disables direct sync before any HTTP call.
- Empty APM `allowed_service_account_emails` disables the direct sync endpoint and returns `503 SERVICE_UNAVAILABLE`.
- DoctorApp Cloud Run server manifests for dev, staging, and prod intentionally set `SERVICE__CONSULTATION_BG_BASE_URI = ""`; direct projection sync is therefore disabled in all three Cloud Run environments.
- Direct sync failure is best-effort on DoctorApp; the approval flow continues and Pub/Sub fallback remains enabled.
- Rollout is blocked until APM Cloud Run allowlist/auth is implemented and verified. Do not describe P0 direct sync as deployed or enabled before that gate passes.

## URL route convention and compatibility

- Use access-before-version route shape for new/preferred endpoints: `/{access}/vN/...` for non-public access scopes and `/vN/...` for public client APIs.
- Service-to-service routes use `/internal/vN/...`.
- Admin/backoffice routes use `/admin/vN/...`.
- Public client routes use `/vN/...`; do not add a `/public/vN` prefix.
- Doctor module/domain routes use the access/version prefix followed by the doctor domain path, for example `POST /internal/v1/doctor-projection/sync` and `POST /internal/v1/pubsub/doctor-profile`.
- Version-before-access paths such as `/v2/internal/...` are legacy compatibility aliases only when already needed; they are not the preferred convention.
- Compatibility aliases remain active during migration: `POST /internal/doctor-projection/sync` and `POST /pubsub/doctor-profile`. Do not remove legacy aliases until every caller/subscription has migrated and a separate migration note is accepted.

## Configuration knobs

### DoctorApp

Current safe Cloud Run posture keeps direct sync disabled in every DoctorApp server environment:

```text
projects/doctorapp/cloudrun-server/dev/service.yaml      SERVICE__CONSULTATION_BG_BASE_URI=""
projects/doctorapp/cloudrun-server/staging/service.yaml  SERVICE__CONSULTATION_BG_BASE_URI=""
projects/doctorapp/cloudrun-server/prod/service.yaml     SERVICE__CONSULTATION_BG_BASE_URI=""
```

Local/default config also keeps direct sync disabled with:

```toml
[service]
consultation_bg_base_uri = ""
```

Only after APM Cloud Run allowlist/auth is implemented and verified may an environment set the APM consultation-bg base URL:

```text
SERVICE__CONSULTATION_BG_BASE_URI=https://<apm-consultation-bg-service>
```

### APM consultation-bg

Allow the DoctorApp runtime service account:

```toml
[doctor_projection_sync]
allowed_service_account_emails = ["<doctor-app-service-account>@<project>.iam.gserviceaccount.com"]
tokeninfo_url = "https://oauth2.googleapis.com/tokeninfo"
```

Keep `allowed_service_account_emails = []` to disable direct sync.

## Current rollout state

1. DoctorApp Cloud Run dev keeps `SERVICE__CONSULTATION_BG_BASE_URI=""`; direct projection sync is disabled.
2. DoctorApp Cloud Run staging keeps `SERVICE__CONSULTATION_BG_BASE_URI=""`; direct projection sync is disabled.
3. DoctorApp Cloud Run prod keeps `SERVICE__CONSULTATION_BG_BASE_URI=""`; direct projection sync is disabled.
4. Pub/Sub doctor profile publishing/subscription remains the active fallback path and must stay healthy.
5. APM Cloud Run allowlist/auth is not yet an accepted rollout gate; direct sync enablement remains blocked.

## Pub/Sub fallback health validation gate

Run this gate before any direct-sync enablement. The direct-sync posture remains disabled while validating fallback health: keep DoctorApp `SERVICE__CONSULTATION_BG_BASE_URI=""` and keep APM `allowed_service_account_emails = []` unless the separate allowlist/auth gate has already passed.

### Required path

- Publisher/source: DoctorApp approval flow publishes `DoctorProfileApproved` to Pub/Sub topic `doctor-profile.approved`.
- Delivery path: GCP Pub/Sub push subscription should invoke APM consultation-bg preferred `POST /internal/v1/pubsub/doctor-profile`; legacy `POST /pubsub/doctor-profile` remains accepted for existing subscriptions.
- Consumer evidence: APM logs `pubsub_doctor_profile`, `processing doctor profile event`, then `doctor identity approved/upserted`; APM DB row in `v2.doctor_identity` is upserted with `source_event_id = <event_id>`.

### Discovery commands

Use the environment project and APM consultation-bg service name for the target environment:

```bash
export PROJECT_ID="tdg-dh-truehealth-core-<nonprod|staging|prod>"
export TOPIC="doctor-profile.approved"
export APM_SERVICE="<apm-consultation-bg-cloud-run-service>"

gcloud pubsub topics describe "$TOPIC" --project "$PROJECT_ID"
gcloud pubsub subscriptions list --project "$PROJECT_ID" \
  --filter="topic:doctor-profile.approved AND (pushConfig.pushEndpoint:/internal/v1/pubsub/doctor-profile OR pushConfig.pushEndpoint:/pubsub/doctor-profile)" \
  --format="table(name,pushConfig.pushEndpoint,deadLetterPolicy.deadLetterTopic)"
```

The listed push endpoint should end with `/internal/v1/pubsub/doctor-profile`; legacy `/pubsub/doctor-profile` remains valid during migration. If no subscription is listed, fallback health is failed and direct sync must stay disabled.

### Synthetic non-prod validation

Use only a disposable non-prod doctor identity. Do not run synthetic publishes in production unless the release owner approves a real canary doctor and rollback owner.

```bash
export EVENT_ID="fallback-health-$(date +%Y%m%d%H%M%S)"
export DOCTOR_ID="00000000-0000-0000-0000-000000000000"
export DOCTOR_ACCOUNT_ID="100000001"
export DOCTOR_PROFILE_ID="200000001"

PAYLOAD=$(printf '{"__type":"DoctorProfileApproved","event_id":"%s","doctor_id":"%s","doctor_account_id":%s,"doctor_profile_id":%s,"is_active":true}' \
  "$EVENT_ID" "$DOCTOR_ID" "$DOCTOR_ACCOUNT_ID" "$DOCTOR_PROFILE_ID" \
)
gcloud pubsub topics publish "$TOPIC" --project "$PROJECT_ID" --message="$PAYLOAD" --attribute="health_check=fallback_doctor_profile"
```

### Cloud Logging query

Query the APM consultation-bg service after publishing or after a real approval canary:

```bash
gcloud logging read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name="'"$APM_SERVICE"'" AND (httpRequest.requestUrl:"/internal/v1/pubsub/doctor-profile" OR httpRequest.requestUrl:"/pubsub/doctor-profile" OR textPayload:"pubsub_doctor_profile" OR jsonPayload.message:"processing doctor profile event" OR jsonPayload.message:"doctor identity approved/upserted")' \
  --project "$PROJECT_ID" \
  --freshness=30m \
  --limit 50 \
  --format="table(timestamp,httpRequest.status,jsonPayload.message,textPayload)"
```

Expected log evidence: at least one `POST /internal/v1/pubsub/doctor-profile` or legacy `POST /pubsub/doctor-profile` with HTTP `200`, one `processing doctor profile event`, and one `doctor identity approved/upserted` for the canary `event_id`/doctor identifiers. Any `failed to decode DoctorProfileEvent Pub/Sub data` or `database error while projecting doctor identity` during the window is a failed gate.

### Pub/Sub metrics

Check the push subscription metric for the subscription discovered above:

```text
Metric: pubsub.googleapis.com/subscription/push_request_count
Filter: resource.label.subscription_id = <doctor-profile subscription> AND metric.label.response_code_class = "2xx"
Success: count increases after the canary publish/approval.

Metric: pubsub.googleapis.com/subscription/num_undelivered_messages
Filter: resource.label.subscription_id = <doctor-profile subscription>
Success: returns to 0 within 5 minutes; no sustained backlog growth.

Metric: pubsub.googleapis.com/subscription/oldest_unacked_message_age
Filter: resource.label.subscription_id = <doctor-profile subscription>
Success: remains below 60s after delivery; no increasing trend.
```

### DB success criteria

Confirm the projection row in the APM database for the canary values:

```sql
SELECT doctor_id, doctor_account_id, doctor_profile_id, is_active, source_event_id, updated_at
FROM v2.doctor_identity
WHERE source_event_id = '<EVENT_ID>'
   OR doctor_id = '<DOCTOR_ID>';
```

Success criteria for this gate: subscription exists, push endpoint is preferred `/internal/v1/pubsub/doctor-profile` or legacy `/pubsub/doctor-profile`, logs show HTTP `200` and the expected processing/upsert messages, Pub/Sub metrics show successful 2xx delivery without backlog, and the APM projection row matches the canary event. If any criterion fails, keep direct sync disabled and investigate fallback before rollout.

## Blocked enablement checklist

Do not execute these enablement steps until the APM Cloud Run allowlist/auth gate is implemented and verified:

1. Complete the Pub/Sub fallback health validation gate above and attach endpoint, log, metric, and DB evidence before enabling direct sync.
2. Deploy APM consultation-bg with `[doctor_projection_sync].allowed_service_account_emails` containing the DoctorApp caller service account, or the accepted Cloud Run auth/IAM replacement contract.
3. Deploy one DoctorApp environment with `SERVICE__CONSULTATION_BG_BASE_URI` pointing to APM consultation-bg.
4. Approve a test doctor and confirm APM projection updates through direct sync while Pub/Sub fallback remains active.
5. Watch APM direct-sync status logs for `401`, `403`, `500`, and `503` responses.
6. Watch DoctorApp logs for `alert="apm_doctor_projection_sync_failed"`; this means direct sync failed but Pub/Sub fallback remains enabled.

## Rollback

Use either switch to disable direct sync without changing the Pub/Sub fallback:

- Clear DoctorApp `SERVICE__CONSULTATION_BG_BASE_URI` / set `consultation_bg_base_uri = ""`.
- Clear APM `[doctor_projection_sync].allowed_service_account_emails`.

## Validation commands

```bash
cargo test -p consultation-bg-rs doctor_identity:: -- --nocapture
cargo test -q projection_sync # run in tdh-mordee-doctor-app/server
```

Docs smoke check:

```bash
grep -nE "SERVICE__CONSULTATION_BG_BASE_URI=\"\"|Pub/Sub fallback|blocked until APM Cloud Run allowlist/auth|direct projection sync is disabled" docs/plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md
```
