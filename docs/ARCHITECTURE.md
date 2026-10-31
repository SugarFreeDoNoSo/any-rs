# Architecture and implementation status

The authoritative design is the preserved Spanish
[specification](../spec/my-framework-spec-v0.1.0/SPECIFICATION.md). This English
overview introduces it without changing its requirements or approving proposals.

## Proposed flow

```text
Renderer / SDK -> Gateway + authorization -> Durable ingress / buffer
                                              |
                                           Dispatcher
                                              |
                                            Handler
                                              |
                      Atomic state + inbox + outbox + operation commit
                                              |
                                         Outbox relay
                                              |
                              Independent durable subscriptions
                                              |
                                    Projections / integrations
```

Readers use authorized read ports; application handlers do not directly invoke
other application handlers. A handler proposes state changes and outbound
messages; the runtime owns validation and commit. A command's single logical
owner updates its operation result. Downstream projections must not rewrite the
already-terminal result of that command.

## Boundaries and invariants

- **Declarative core:** parse TOML, normalize to portable JSON-compatible data,
  validate structure and references, check semantics, plan capabilities, and
  produce an immutable release. JCS hashing remains unimplemented.
- **Identity and scope:** a data object's key includes business, project, client,
  type, and ID. A missing client is not a wildcard. Authentication, current
  membership, deny-by-default permissions, and delegation boundaries are required.
- **Durable runtime:** HTTP `202` requires persisted command, operation, and
  idempotency state. Commit atomically coordinates state, inbox, outbox, and the
  owning operation, including expected revisions and fencing checks.
- **Delivery:** at least once, with per-subscription inbox/delivery state, stable
  message identities, bounded retries, and durable dead letters. Neither global
  ordering nor exactly-once external effects is promised.
- **Infrastructure:** small ports separate transactional state, queue, blob, log,
  cache, and projections. Capability names alone do not certify interoperability.
- **Frontend:** proposed public core/client modules remain separate from server
  adapters and credentials. Renderers are optional; htmx alone is not MVVM.

## What exists today

| Area | Status | Evidence or next step |
| --- | --- | --- |
| Draft contracts, examples, decisions | Present | Original versioned package |
| Static contract validation | Passing locally | Original Python validator and repository wrapper |
| Repository CI | Workflow prepared | Remote execution must be checked after publication |
| Compiler, IR and deterministic hashing | Not implemented | M1; select language first |
| Durable runtime and SQLite adapter | Local fixture slice implemented, not full M2 | [Boundaries and executed tests](runtime/LOCAL-SLICE.md) |
| Auth provider and owner bootstrap | Not selected/implemented | Resolve before M2 acceptance |
| SDK, ViewModel, BFF and renderers | Not implemented | M3 |
| Cloud adapters and deployment | Not implemented/certified | M4; choose provider, costs, load and recovery plan |
| Workers / Containers / Turso expansion | Contract preparation only | [Additive profiles and exit gates](adapters/WORKERS-CONTAINERS-TURSO.md) |
| TOON parser and optional extensions | Not implemented/certified | M5; fix codec/version and run official fixtures |

The repository contains a narrow executable SQLite development profile, not placeholder cloud adapters.
The owner requested Workers, Containers and Turso adapter requirements; the
additive proposal distinguishes Rust/Wasm from native Container targets without
claiming either is built. Python here is **validation and HTTP integration test tooling only**; the local runtime is Rust. The original
[roadmap](../spec/my-framework-spec-v0.1.0/ROADMAP.md) defines milestone exit criteria.
