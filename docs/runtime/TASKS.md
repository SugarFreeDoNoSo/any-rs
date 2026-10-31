# Tasks: neutral objects → compiled Rust execution

This increment implements the requested three-state task example. It preserves
the previous note/SAM/dashboard delivery and its AGPL-3.0-only license.

| Operation | Current state | Result |
| --- | --- | --- |
| `tasks.create` | No object | Explicit initialization in `pending`, revision 1 |
| `tasks.start` | `pending` | `in_progress`, revision +1 |
| `tasks.complete` | `in_progress` | `completed`, revision +1 |
| Any other transition | Any state | Denied, no domain effects |

Creation is declared separately in the IR, not a third hidden transition.
`completed` is terminal. Display labels are Pendiente, En progreso, Completada.
The authoritative objects live in `runtime/machines/tasks.json`; the generated
TOML fixture has equal semantics. There are no Python/JavaScript interpreters,
arbitrary callbacks, executable strings, handler-to-handler calls or raw SQL in
business definitions. Rust compiles and executes a closed vocabulary.

## HTTP workflow

Keep the server running using `runtime/local.example.toml` and an environment-only
local development token. Paths below use its demo/notes/local scope.

```sh
base=http://127.0.0.1:8080/v2/businesses/demo/projects/notes/clients/local
curl -H "Authorization: Bearer $ANYRS_LOCAL_TOKEN" \
  -H 'Content-Type: application/json' -H 'Idempotency-Key: task-create-1' \
  -d '{"name":"tasks.create","version":1,"payload":{"id":"task-1","title":"Primera tarea"}}' \
  "$base/tasks/commands"
```

A 202 returns `operation_url`. Poll that URL with the same credential until the
operation is `succeeded` or `rejected`; 202 does not mean the task transition
already happened. Then submit `tasks.start` with `{"id":"task-1",
"expected_revision":1}` and a new idempotency key, wait for success, then submit
`tasks.complete` with expected_revision 2 and another key. Read the result at
`$base/tasks/objects/task-1`: state completed, revision 3.

- Unknown names/malformed typed inputs fail before acceptance.
- Missing object, invalid state, stale revision and revoked worker grants produce
  a rejected operation, no object mutation and no new domain outbox event.
- Rejection metadata/inbox are audit bookkeeping, not a business transition.
- Retry the same key and payload to recover the same operation. Reusing that key
  with a different payload conflicts; a new key is a new command and still must
  satisfy the state machine and expected revision.
- Two concurrent commands expecting the same revision cannot both mutate it.
  Current SQLite uses a write transaction, not an unlocked read-then-write.
- Reads do not execute transitions. Operation reads are principal-private;
  task reads require the configured tenant scope, consistent with the local profile.

## State, authorization and persistence

`tasks.rs` accepts only a bounded installed profile, evaluates transitions from
persisted state, validates every action, and commits object/revision, operation,
inbox and outbox in one transaction. Initialization, updates and outbox insertion
roll back together on storage failure. Idempotency records are durable.

Task tables are separated from legacy note tables. `task_objects` keys include
business, project, client, collection and object ID. A task and a note may have
the same ID without collision; existing note data/schema are not rewritten.
This is a safe additive namespace, not a generic multi-collection migration engine.
The dispatcher and Lambda SQS adapter select task operations by exact identity;
there is still no external outbox relay/SQS producer.

## Same IR, several views

The served OpenAPI document derives task command event names, state enums and
labels from the same compiled IR. The authenticated dashboard renders its FSM
as SVG, Markdown and Mermaid source. The OpenAPI base file alone is not the full
served task contract. Data DTO fields (title/id/expected_revision) remain a bounded
Rust adapter contract, not a completed generic schema/compiler pipeline.

See [FSM-IR.md](FSM-IR.md) for typed objects, validation, creation semantics,
JSON/TOML rules and the distinction between future **authoring SDKs** and future
**client SDKs**. No Python/TypeScript SDK or gRPC/Protobuf adapter is delivered yet.

## Reproduce

```sh
cargo build --locked --workspace
cargo test --locked --workspace
python tests/fsm_runtime.py
python tests/tasks_runtime.py
python tests/sam_fixture.py
```

The SAM fixture scenario now exercises task creation, both transitions, SQS-shaped
invocation/replay and terminal reading **natively**. Docker remains unavailable;
no claim of `sam local invoke` execution, real AWS persistence or deployment.
