# Star Gate Booking Doctor and Timeslot API

Biz APM exposes the following authenticated `consultation-rs` APIs for Star Gate scheduled booking.

## Search available doctors

`GET /v2/doctor-timeslot/available-doctors`

Query parameters:

| Name | Required | Description |
| --- | --- | --- |
| `date` | yes | Local calendar date, `YYYY-MM-DD`. The service interprets this as `[00:00, next day 00:00)` in `timezone`. |
| `timezone` | no | IANA timezone for interpreting `date`; defaults to `Asia/Bangkok`. |
| `consultationChannel` | no | `video`, `voice`, or `chat`; defaults to `video`. |

Example:

```http
GET /v2/doctor-timeslot/available-doctors?date=2026-06-18&timezone=Asia%2FBangkok&consultationChannel=video
```

Response:

```json
{
  "date": "2026-06-18",
  "timezone": "Asia/Bangkok",
  "doctors": [
    {
      "doctorId": "018f1414-5e0e-7c2a-b908-7b1967f2b401",
      "doctorAccountId": 42,
      "doctorProfileId": 84,
      "availableTimeslotCount": 2,
      "nextAvailableTimeslotId": "018f1414-5e0e-7c2a-b908-7b1967f2b401:1781762400:1781764200:video"
    }
  ]
}
```

`doctorId` is the stable doctor identifier Star Gate should keep for selection. `doctorAccountId` and `doctorProfileId` are included for compatibility with older booking flows.

## Retrieve scheduled availability timeslots by doctor and date

`GET /v2/doctor-timeslot/scheduled-availability`

Query parameters:

| Name | Required | Description |
| --- | --- | --- |
| `doctorId` | yes | Stable doctor UUID returned by doctor search. |
| `date` | yes | Local calendar date, `YYYY-MM-DD`. The service interprets this as `[00:00, next day 00:00)` in `timezone`. |
| `timezone` | no | IANA timezone for interpreting `date`; defaults to `Asia/Bangkok`. |
| `consultationChannel` | no | `video`, `voice`, or `chat`; defaults to `video`. |

Example:

```http
GET /v2/doctor-timeslot/scheduled-availability?doctorId=018f1414-5e0e-7c2a-b908-7b1967f2b401&date=2026-06-18&timezone=Asia%2FBangkok&consultationChannel=video
```

Response:

```json
{
  "doctorId": "018f1414-5e0e-7c2a-b908-7b1967f2b401",
  "timeslots": [
    {
      "timeslotId": "018f1414-5e0e-7c2a-b908-7b1967f2b401:1781762400:1781764200:video",
      "start": "2026-06-18T06:00:00Z",
      "end": "2026-06-18T06:30:00Z",
      "startEpoch": 1781762400,
      "endEpoch": 1781764200,
      "consultationChannel": "video"
    }
  ]
}
```

`timeslotId` is deterministic and stable for the same doctor, start epoch, end epoch, and consultation channel. Star Gate should pass this ID through its selection UI, while booking requests should continue to submit the canonical start/end times required by the booking endpoint.

## Create an appointment after Star Gate selection

Star Gate creates both instant and scheduled appointments through the internal appointment creation API:

`POST /v2/internal/create-appointment`

This endpoint is for cluster-internal callers. Star Gate must send the patient/doctor identities, the final appointment window, a Star Gate-generated `appointmentNo`, and the prescreen payload collected from the patient. For scheduled booking, Star Gate must also echo the selected timeslot returned by the scheduled availability API.

### Required request fields

| Field | Required | Description |
| --- | --- | --- |
| `bizUnitId` | yes | Biz unit that owns the appointment. |
| `bizCenterId` | yes | Biz center for the appointment. |
| `tenantId` | yes | Tenant for both patient and doctor identities. |
| `appointmentNo` | yes | Star Gate reference code. Biz APM stores it as the booking/appointment number and returns it as `appointmentNo` and `refCode`. |
| `patientId` | yes | Object with `accountId`, `userProfileId`, `tenantId`, and optional `oidcUserId`. |
| `doctorId` | yes | Object with `accountId`, `userProfileId`, `tenantId`, and optional `oidcUserId`. Use the doctor account/profile IDs returned by doctor search. |
| `bookingType` | yes | `{"__type":"Instant"}` for instant booking or `{"__type":"Schedule"}` for scheduled booking. |
| `consultationChannel` | yes | `video`, `voice`, or `chat`. |
| `appointmentStart` | yes | Unix epoch seconds for the appointment start. |
| `appointmentEnd` | yes | Unix epoch seconds for the appointment end; must be greater than `appointmentStart`. |
| `appointmentStatus` | yes | Use `BOOKED` for confirmed Star Gate bookings. |
| `prescreen` | yes | Patient prescreen object; validation rules below. |
| `selectedTimeslot` | scheduled only | Required when `bookingType.__type` is `Schedule`; must match `appointmentStart`, `appointmentEnd`, and `consultationChannel`. |
| `paymentTxId` | no | Payment transaction ID when available. Defaults to `0` if omitted. |
| `paymentTxRefId` | no | Payment reference from Star Gate/payment flow. |
| `paymentChannels` | no | Payment channel payload when available. |
| `parentAppointmentId` | no | Parent appointment for follow-up creation. |

### Prescreen validation

`prescreen` is required for Star Gate appointment creation.

| Field | Required | Validation |
| --- | --- | --- |
| `symptom` | yes | Non-blank string. |
| `duration` | yes | Integer greater than `0`. |
| `durationUnit` | yes | Non-blank string, for example `hour`, `day`, or `week`. |
| `attachments` | yes | Array of attachment references; send `[]` when none. |
| `allergies` | yes | Array of allergy strings; send `[]` when none. |

### Instant booking example

```http
POST /v2/internal/create-appointment
Content-Type: application/json
```

```json
{
  "bizUnitId": 1,
  "bizCenterId": 100,
  "tenantId": 1,
  "appointmentNo": "SG-20260710-000001",
  "patientId": {
    "accountId": 5001,
    "userProfileId": 6001,
    "tenantId": 1
  },
  "doctorId": {
    "accountId": 7001,
    "userProfileId": 8001,
    "tenantId": 1
  },
  "bookingType": { "__type": "Instant" },
  "consultationChannel": "video",
  "appointmentStart": 1783674000,
  "appointmentEnd": 1783675200,
  "appointmentStatus": "BOOKED",
  "paymentTxRefId": "PAY-20260710-000001",
  "prescreen": {
    "symptom": "headache",
    "duration": 3,
    "durationUnit": "day",
    "attachments": [],
    "allergies": []
  }
}
```

### Scheduled booking example

```json
{
  "bizUnitId": 1,
  "bizCenterId": 100,
  "tenantId": 1,
  "appointmentNo": "SG-20260710-000002",
  "patientId": {
    "accountId": 5001,
    "userProfileId": 6001,
    "tenantId": 1
  },
  "doctorId": {
    "accountId": 7001,
    "userProfileId": 8001,
    "tenantId": 1
  },
  "bookingType": { "__type": "Schedule" },
  "consultationChannel": "video",
  "appointmentStart": 1781762400,
  "appointmentEnd": 1781764200,
  "appointmentStatus": "BOOKED",
  "selectedTimeslot": {
    "timeslotId": "018f1414-5e0e-7c2a-b908-7b1967f2b401:1781762400:1781764200:video",
    "startEpoch": 1781762400,
    "endEpoch": 1781764200,
    "consultationChannel": "video"
  },
  "prescreen": {
    "symptom": "fever",
    "duration": 1,
    "durationUnit": "day",
    "attachments": [],
    "allergies": ["penicillin"]
  }
}
```

### Response

```json
{
  "bookingId": "SG-20260710-000002",
  "appointmentId": "SG-20260710-000002",
  "appointmentNo": "SG-20260710-000002",
  "refCode": "SG-20260710-000002"
}
```

`bookingId` is the value from `v2.reservation.booking_id`; `appointmentId` is the value from `v2.appointment.appointment_id`. In the current Biz APM schema, Star Gate-provided `appointmentNo` is used as the booking/appointment identifier, so all four fields normally match. `refCode` is the Star Gate-facing alias for `appointmentNo`.

Invalid contract payloads return `400 Bad Request`; persistence failures return `500 Internal Server Error`.

## Existing range-based timeslot API

The existing `GET /v2/doctor-timeslot/available-timeslots` remains available for consumers that already know `doctorAccountId`, `doctorProfileId`, `fromDatetime`, and `toDatetime`. Its timeslot items now include the same stable `timeslotId` field.
