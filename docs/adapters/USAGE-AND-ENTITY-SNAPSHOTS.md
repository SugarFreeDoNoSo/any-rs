# Provider usage and experimental per-entity snapshots

**Revision:** `0.1.0-contract.1` · **Evidence checked:** 2026-10-23

These are requirements, not an implemented storage engine or a billing promise.
The [versioned profile](../../contracts/adapters/usage-and-experiment.json)
is provider configuration, never an immutable constant of the any-rs core.

## Usage requirements

The public Turso **Free** plan lists **5 GB storage**, **500 million rows read per
month**, **10 million rows written per month**, and **3 GB monthly sync**.
The profile preserves the published GB unit; exact byte conversion and the
user's actual account/engine entitlements are not verified. Recheck the applicable
plan before activation. No paid overage has been authorized.
[Turso pricing](https://turso.tech/pricing).

A request is not a row: one SQL statement may scan many rows. Updates and retries
can add reads and writes; aborted transactions can still incur write usage.
Storage and synchronization are separate dimensions, not derivable from a count
of API calls. The billing guide uses some older plan names, so its row-accounting
explanation does not override today's plan table or establish new-engine pricing.
[Usage and billing](https://docs.turso.tech/help/usage-and-billing).

The adapter must expose requests, rows read/written, storage/sync bytes,
transport bytes, snapshot bytes and retry counts separately where measurable.
Unknown observations remain **unknown**, not zero; provider metering is the
billing authority. Estimates require labels, timestamp, engine/transport, plan
revision and observed coverage. Do not estimate usage solely from rows returned.
Use indexed tenant keys and bounded queries. A projected budget exhaustion may
reject or defer new work **before** durable acceptance; it cannot erase accepted
commands. A provider quota error without a confirmed commit cannot produce a
successful acceptance ACK. Unknown commit outcomes still require reconciliation.
No new payment plan or automatic overage is enabled by this contract.

## Candidate: one SQLite image per entity in a remote BLOB

The user's idea is recorded as a **disabled experiment**, not a replacement for
the initial remote-libSQL relational adapter. It could be tested as a materialized
entity snapshot: an inner SQL database lives in bounded memory while working,
then its image is serialized into one tenant-scoped remote row. No compatible
inner engine/VFS, image serialization API or Worker Wasm build is certified yet.

### Non-negotiable protocol

1. Authenticate and authorize the caller and assign stable command/idempotency IDs.
   Persist the command, operation and idempotency entry in the durable event log
   **before** acknowledging acceptance or waiting for a batch. Memory is not a log.
2. Enforce a single logical writer through a durable coordinator (for example a
   qualified Durable Object) **and** authoritative remote compare-and-swap/version
   fencing. One JavaScript thread, an entity name, or an ordinary Worker instance
   does not establish a global singleton. Await/restart and competing writers
   remain possible; stale epochs must fail.
3. Load a bounded verified snapshot and its checkpoint. Replay later **committed
   domain events** deterministically with pinned schema/handler definitions, in
   order and without new external effects. Do not discard historical facts after
   membership revocation. Separately, executing **accepted but unapplied commands**
   requires current authorization; snapshot reads require current read permission.
   Both paths retain tenant boundaries. A local image is not permission to execute
   arbitrary caller SQL. Parameterize outer SQL; use compiled allowlisted handlers
   for inner operations.
4. Commit snapshot bytes, version, checkpoint, digest and schema version together,
   plus the applicable inbox/outbox/operation records in the same authoritative
   database transaction. Check the expected version/fencing epoch and affected row
   count **before** committing. Never advance a replay checkpoint first.
5. Distinguish **accepted-durable** from **applied**. A local batch completion or
   snapshot serialization is neither remote persistence nor operation completion.
   Timeout after commit is an unknown outcome: reconcile stable IDs/version before
   replay. Recover from crash between acceptance, application, serialization,
   remote commit and delivery; independently replay each outbox subscription.
6. Snapshot publication rewrites the image as a whole in this experiment. Measure
   write amplification, serialization time, temporary copies, network transfer,
   memory pressure and retries. Cross-entity queries/transactions are not provided
   by one inner database; explicit projections or a later coordinator are needed.
   Do not compact/truncate accepted log entries until verified snapshot durability
   and the configured replay/recovery retention rules allow it.

### Feasibility gates, not assumed limits

- Current reviewed Turso Cloud documentation does **not establish** the hosted
  maximum BLOB, row, request or response size. Each remains null/unknown in the
  profile and blocks activation. SQLite's theoretical limits or a configurable
  open-source daemon default are not hosted service guarantees. Set a conservative
  application snapshot cap only after provider verification and tests.
- HTTP BLOB arguments use base64, which adds approximately one third to binary
  size before JSON overhead. Track wire bytes independently of rows and stored
  bytes. [HTTP representation](https://docs.turso.tech/sdk/http/reference).
- Workers currently allow 128 MB **per isolate**, shared across JavaScript, Wasm
  and concurrent requests, not per entity. Snapshot input, decoded bytes, engine
  pages, export buffers and base64 may coexist. CPU and transport limits also
  require measurements. [Workers limits](https://developers.cloudflare.com/workers/platform/limits/).
- Browser threaded Turso/OPFS builds are not automatically Workers-compatible;
  Workers do not support the same Wasm threads/Web Worker environment. A verified
  target-compatible memory VFS plus snapshot import/export is a prerequisite.
  [Wasm support](https://developers.cloudflare.com/workers/runtime-apis/webassembly/).
- The experiment cannot claim unlimited capacity, free operation or a billing
  bypass by packing data into one row. Event log, index, snapshot, synchronization,
  failed-attempt and byte costs all require measurement on the selected engine.

Acceptance experiments must include conflicting writers, stale fencing tokens,
duplicate commands, tenant escape attempts, corrupted/oversized snapshots,
interrupted commit, restart replay, authorization revocation and load-induced
memory/CPU failures. None has been executed in a live service by this package.
