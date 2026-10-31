# Local runtime integration test plan

`python3 tests/runtime_http.py --binary target/debug/any-rs-local` launches real
loopback HTTP processes against fresh temporary SQLite databases. Tests use only
fixture bearer tokens injected through environment variables; no cloud service,
real user data, or account credentials are involved.

## Acceptance coverage

- HTTP 202 only after operation and command persistence; a deliberately disabled
  worker proves that acceptance is distinct from completion.
- Forced process kill after acceptance, worker recovery on restart, durable
  object reads after a second restart, and one outbox event per success.
- Idempotent semantic JSON replay, key conflicts, key scope by principal and
  tenant, and concurrent duplicate requests.
- A duplicate domain object is an accepted command followed by a rejected
  operation, not a false successful write.
- Fail-closed authentication; business/project/client separation; operation
  visibility restricted by principal; independent same-ID objects in tenants.
- A command accepted before principal revocation is rejected at dispatch after
  restart, without object/outbox writes.
- Accepted, operation, object and problem responses validate against preserved
  original JSON Schema contracts (the pinned validator dependencies are required).
- Unknown command/version/fields, invalid payloads, malformed JSON, missing or
  oversized idempotency keys, request body limits and missing resources.
- An injected outbox INSERT failure rolls back object, inbox and operation
  state; removing the fault permits one complete commit.
- SQLite write contention returns failure rather than a false 202. A database
  open failure prevents listening.

## Limits

This is one local Rust/SQLite runtime slice. It does not certify Turso/libSQL,
Cloudflare Workers/Containers, multi-host leases/fencing, a production identity
provider, arbitrary handlers, or delivery of outbox events to external systems.
The integration suite checks SQL state only for this slice's documented tables.
`SIGKILL` tests process recovery, not physical disk/power-loss durability.

Execution results belong in the runtime validation report after the binary is
built and the suite has actually completed; this plan alone is not test evidence.
