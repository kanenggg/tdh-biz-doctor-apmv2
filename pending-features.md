# Pending features

## Consultation event V2 runtime publication

The V2 Rust model, serde contract tests, canonical AsyncAPI, and generated
OpenAPI remain active. Production publication remains V1-only. This is a model
and contract remediation, not a rollout.

Previous decisions: V2 events must be explicitly constructed (no lossy V1
conversion), use the durable outbox, retain a stable `event_id` Pub/Sub
attribute across retries, and use the canonical `__type` wire discriminator.

Blockers:

- Current V1 producer paths do not supply all required V2 source fields.
- The existing booking-lifecycle outbox unique-index migration can be skipped
  when historical duplicates exist; V2 needs a deliberate idempotency key and
  a validated index/backfill plan before reusing that mechanism.

Reactivation questions and acceptance tests:

1. Which service owns every missing V2 field for each event variant?
   Acceptance: each active producer constructs a complete V2 payload from
   authoritative inputs without defaults invented by the publisher.
2. What V2 idempotency key is safe across retries and replay?
   Acceptance: a duplicate delivery yields one durable outbox row and repeated
   dispatches preserve the same payload and `event_id` metadata.
3. What is the consumer cutover and rollback contract?
   Acceptance: dual-consumer shadow validation succeeds for the agreed period,
   followed by an independently tested rollback that leaves V1 unaffected.
# Canonical Appointment Hold events

`TimeslotReserved`, `ReservationCancelled`, and `ReservationExpired` remain V1
wire adapters during the Appointment Hold migration. Before adding canonical
Hold events, inventory every consumer, define a versioned topic/discriminator,
and plan dual publication plus a consumer cutover. Do not rename the existing
wire discriminators in place.
