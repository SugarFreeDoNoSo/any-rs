# Local durable vertical slice (2026-10-30)

## Status and decision

The owner asked to resume **the same any-rs project** and accepted a first actual
request → operation → persisted result cycle. This additive Rust 2024 / Axum /
SQLite profile implements that bounded cycle. It does not replace the remote
libSQL, Workers or Containers adapter plan. Those adapters remain unimplemented.
No service was deployed and no repository was published in this increment.
The immutable specification and original source archives remain unchanged.

Rust was selected by the project context; the executable package is
`any-rs-local`, version `0.1.0`. `Cargo.lock` and `rust-toolchain.toml` fix the
resolved dependencies and compiler. SQLite is bundled by rusqlite. This is a
local development profile, **not production authentication or a full framework
compiler**. The implemented HTTP subset is described by `runtime/openapi.json`, also served
at GET `/openapi.json`. It is separate from the immutable full proposed contract.
Only the explicit fixture command `demo.note.create` at schema/command version `1` exists.

## Run

From this repository root with Rust 1.99.1 and Python 3.11+:

```sh
cargo build --locked
# Use your own local development token; do not commit it.
export ANYRS_LOCAL_TOKEN='replace-with-your-own-local-token'
cargo run --locked -- --config runtime/local.example.toml
```

The example binds only `127.0.0.1:8080`. Non-loopback binds are rejected. A local
SQLite file is created at `database` (relative to the launch working directory).
Keep its directory private to the OS user. The example grants one principal all
three implemented actions in exactly business `demo`, project `notes`, client
`local`. Authentication uses the token from the named environment variable. A
scope in a request is **not** a grant. Tokens are neither logged nor stored in
SQLite. Tokens must be distinct, 16–256 ASCII graphic bytes; no token is shipped.
Static grants are read at startup. To revoke or modify them, stop **all** processes
using this database, update configuration/environment and restart. There is no
hot reload, identity provider, owner bootstrap, fine-grained policy or expiry.
Never expose this local HTTP profile directly to the Internet.

```sh
curl -i http://127.0.0.1:8080/v2/businesses/demo/projects/notes/clients/local/commands \
  -H "Authorization: Bearer $ANYRS_LOCAL_TOKEN" \
  -H 'Idempotency-Key: first-note' -H 'Content-Type: application/json' \
  -d '{"name":"demo.note.create","version":1,"payload":{"id":"n1","text":"Hello"}}'
# GET the returned operation_url with the same bearer token.
curl http://127.0.0.1:8080/v2/businesses/demo/projects/notes/clients/local/demo.note/objects/n1 \
  -H "Authorization: Bearer $ANYRS_LOCAL_TOKEN"
```

`202` means durable acceptance, not domain success. Poll `operation_url` until
`succeeded` or `rejected`. The operation response is HTTP 200 even for a rejected
command. An object-name conflict is an asynchronous rejected operation with
problem status 409. Reusing an idempotency key for a different request is an
immediate HTTP 409. Reordering JSON object fields retains the same operation.
Only the submitting principal can read its operation; all principals explicitly
granted the same scope can read the scope's objects. Responses are `no-store`.

## Architecture and correctness boundary

- `domain.rs` defines validated identifiers, closed command input and a pure
  handler. No handler calls another handler or any infrastructure service.
- `RuntimeStore` is the small logical persistence/dispatch port.
- `SqliteStore::accept` transactionally inserts one durable command/operation and
  its idempotency identity before HTTP 202. The command row owns the operation;
  there is intentionally no separate volatile queue or separate operations table.
- A background dispatcher polls persisted accepted commands independently of HTTP.
  An SQLite `BEGIN IMMEDIATE` transaction serializes selection and commit. Because
  the tiny handler does all its work inside that transaction and has no external
  effects, there is no lease expiry window to fence in this bounded profile.
- Current startup grants are rechecked by the worker. A queued command whose
  principal/scope was removed is atomically rejected without writing an object.
- State, terminal operation, inbox marker and durable `demo.note.created` outbox
  record commit atomically. A killed process before commit rolls back; accepted
  commands remain available after restart. Parameterized SQL scopes every read
  and uniqueness constraint by business/project/client and where relevant principal.
- WAL and `synchronous=FULL` are set. Storage hardware and filesystem durability
  remain assumptions; tests verify process kill/restart, not physical power loss.

This is not the entire `durable_async` specification. There are no cloud bindings,
remote libSQL transport, WASM build, general registry/type compiler, definition
release hashing, updates/deletes, dynamic schema installation, custom handlers,
deadlines, rate limits, dead letters, outbox relay, per-subscription fan-out,
projection watermarks, distributed leases/fencing or SDK. Durable outbox rows are
**not** externally delivered events. A broken database or malformed persisted
command stalls dispatch for diagnosis; there is no poison-message remediation.
The single registered note type is an additive fixture, not generic object CRUD.

## Validate

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
python tests/runtime_http.py
python -m pip install -r spec/my-framework-spec-v0.1.0/tools/requirements.txt
python tools/validate.py
python tools/validate_adapter_contracts.py
python -m unittest discover -s tests -v
```

`--worker-disabled` is an explicit test/diagnostic startup flag: ingress still
persists commands but no worker processes them. Restart without it to resume.
See [test plan](TEST_PLAN.md) and [recorded evidence](EVIDENCE.md).

## Next gate

Retain the local profile as a reproducible reference. Extend conformance around
transaction fault injection and bounded delivery before claiming general runtime
certification. Then implement the already-planned remote libSQL composition and
Workers/Containers boundaries with their own actual builds and integration tests.
Do not report local SQLite success as Turso, Workers, Containers or deployment
success. Resolve a production identity provider before exposing remote ingress.
