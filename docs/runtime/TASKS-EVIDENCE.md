# Tasks FSM increment — executed evidence, 2026-10-31 UTC

Continues local commit d837fc85378d6ae036bc5557babe4ef95d696354 (Library v3)
without changing the published main branch. The previous delivery remains in
Library history. No SDK/client generator, gRPC, deployment or interpreter added.

## Implemented

- Neutral typed version-1 FSM objects, JSON and TOML readers, closed vocabulary.
- Structural JSON Schema plus authoritative semantic Rust compilation.
- Explicit task creation in pending and exactly two declared task transitions:
  pending→in_progress via tasks.start; in_progress→completed via tasks.complete.
- Default denial, typed guards/actions, terminal states, ambiguity rejection.
- Rust execution in the existing HTTP/operation/worker cycle; no business callbacks.
- Scoped, collection-qualified task storage separate from legacy notes.
- Expected-revision check and state validation inside SQLite write transaction;
  domain object, operation, inbox and outbox commit/rollback together.
- OpenAPI task descriptors and dashboard state diagrams read the same installed IR.
- The native SAM fixture now runs creation and both task transitions through the
  HTTP and SQS invocation adapters, with replay and terminal state checks.

## Passed checks

| Check | Result |
| --- | --- |
| Rust workspace unit tests | 26 passed, including 15 FSM tests |
| Real task HTTP tests | 6 passed |
| Note/FSM deny-default HTTP tests | 4 passed |
| Existing HTTP/process baseline | 15 passed |
| API Gateway/SQS event adapter tests | 14 passed |
| Official Rust SDK Runtime API loopback tests | 2 passed |
| Authenticated dashboard HTTP/adapter tests | 5 passed |
| Native SAM scenario and guard tests | 2 passed |
| Original static suite | 52 passed |
| Original source preservation | 51 files + two source artifacts unchanged |
| Served OpenAPI | Valid 3.1; embedded task IR identical to source JSON |
| IR JSON/TOML | Equal models and valid/denied plans |
| Machine JSON Schema | Two valid fixtures and six negative fixtures checked |
| Build, rustfmt, Clippy workspace/all targets | Passed |

Logs and the actual served OpenAPI snapshot: `evidence/tasks/`.
Machine schema evidence: `runtime/machines/schema-validation.json`.

Tests cover unknown events, guards, invalid/skipped/terminal transitions, stale
revision, conflicting idempotency, concurrent commands expecting the same
revision, tenant isolation, same IDs in task/note namespaces and injected outbox
failure rolling back all task mutations. Rejected accepted operations retain
operation/inbox audit records; domain objects/outbox are unchanged.

One initial task HTTP request hit its unchanged five-second timeout. Four
immediate full repeats passed without test relaxation; the final recorded run
also passed. No reproducible semantic failure remained. The repository-validator
fixture initially omitted runtime files newly linked by documentation; its
fixture now copies that directory, preserving the unchanged link validator.

## Not claimed

Docker/SAM container execution remains blocked in this executor (no exposed CLI
or daemon); native SAM checks are not sam local invoke. Chromium visual QA was
previously blocked by an environment socket permission and was not bypassed or
retried. SVG/Markdown responses are HTTP-tested, not visually certified.

The engine still has bounded Note/Task payload DTOs, SQLite-specific action
execution and compiled-in registrations. It is not a general type registry,
distributed CAS implementation or arbitrary neutral-expression compiler. JSON/
TOML equivalence is semantic, not a canonical byte hash/release guarantee.
Production auth, external queue delivery, cloud persistence, SDKs and Protobuf
remain explicit future gates. No new branch/PR was published or merged.
