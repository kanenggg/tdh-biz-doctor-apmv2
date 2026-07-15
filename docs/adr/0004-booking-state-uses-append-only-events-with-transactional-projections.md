# Booking transitions write audit events and operational state together

Booking and reservation state transitions should append audit events for traceability, but the event log is not the conflict-check datastore. We decided to write the booking event log, current booking state, and doctor occupancy state in the same database transaction. This keeps state transitions easy to trace while keeping reservation, expiry, payment confirmation, and consultation-start conflict checks fast and strongly consistent.
