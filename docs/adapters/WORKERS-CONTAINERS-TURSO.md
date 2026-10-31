# any-rs: Workers, Containers and Turso adapter contracts

**Revision:** `0.1.0-contract.1` · **Reviewed:** 2026-10-23

**Delivered:** additive requirements, capability profiles, compatibility evidence,
and offline schema/planning regression tests. **Not delivered:** executable
Workers/Containers adapters, database transport, running framework, SDK, image,
Wasm build, cloud deployment, or live service certification. The owner requested
storage-mode requirements before implementing this expansion. Original
[specification](../../spec/my-framework-spec-v0.1.0/SPECIFICATION.md),
[ports](../../spec/my-framework-spec-v0.1.0/contracts/ports.md), and their bytes
remain unchanged.

## 1. Separate four decisions

An adapter composition selects **runtime**, **engine family**, **storage medium**,
and **synchronization/consistency protocol** independently. A runtime is not a
storage engine; an available SDK is not a conformance certificate. The initial
candidate is remote **libSQL** hosted by Turso, with two distinct compute targets.
New Turso-engine support and browser/local-first modes are separate candidates.

| Runtime adapter | Proposed artifact and host boundary | Persistence boundary |
| --- | --- | --- |
| Cloudflare Workers | Rust `wasm32-unknown-unknown` via workers-rs and generated JS bindings; remote SDK bridge is host-side JS/TS | Isolate memory is not authoritative; no assumption of native SQLite, native filesystem, or `std::net` compatibility |
| Cloudflare Containers | Native Rust candidate in a `linux/amd64` image, reached through Worker and Durable Object control plane | Ordinary instance disk is ephemeral; remote database is authoritative for this composition |
| Browser, later | Separate browser Turso Wasm package and its own host requirements | Local memory/OPFS cache/projection, never the authoritative server buffer |

Workers are isolates, not Docker processes. Rust support is documented via
workers-rs; generated JavaScript does not require rewriting the core in Node.
Native dependencies must be qualified for the Wasm target. The browser-Wasm
package's threads/Web Worker assumptions do not carry over to Workers.
[Workers execution model](https://developers.cloudflare.com/workers/reference/how-workers-works/),
[Rust support](https://developers.cloudflare.com/workers/languages/rust/),
[Wasm limitations](https://developers.cloudflare.com/workers/runtime-apis/webassembly/).

Cloudflare requires `linux/amd64` images. New Container integrations should assess
the current Durable Object Container API; readiness is distinct from `running`.
Stop/sleep/rollout can lose ordinary disk state. A Container's Durable Object
identity or routing key does not authenticate an application tenant.
[Container target](https://developers.cloudflare.com/containers/get-started/),
[lifecycle and storage](https://developers.cloudflare.com/containers/concepts/architecture/).

## 2. Turso engine and transport matrix

| Engine/profile | Documented SDK route | Initial any-rs status |
| --- | --- | --- |
| Remote libSQL | `@libsql/client/web` for Workers, explicit HTTPS transport | First proposed persistence profile; SDK version must be pinned and tested |
| New remote Turso engine | `@tursodatabase/serverless`, fetch-only | Separate future adapter; not silently substituted for libSQL |
| Native local Turso | `@tursodatabase/database`, file or memory | Candidate after native-runtime qualification |
| Native Turso synchronization | `@tursodatabase/sync`, local plus push/pull | Separate future synchronization profile |
| Browser Turso | `@tursodatabase/database-wasm`; separate sync-wasm package | Local-first candidate, not Worker runtime support |

These packages have different engines and protocols. The native Container Rust
HTTP client remains to be selected and qualified; the TypeScript web client is
not claimed to be a native Rust API. The initial plan deliberately rejects a
`file:` database URL in Workers and rejects new-Turso engines attached to a
libSQL transport. [Official SDK matrix](https://docs.turso.tech/sdk/ts/reference),
[libSQL web client](https://github.com/tursodatabase/libsql-client-ts/blob/main/packages/libsql-client/src/web.ts),
[Workers libSQL integration](https://developers.cloudflare.com/workers/tutorials/connect-to-turso-using-workers/).

D1 is only a future comparison candidate, not an additional implemented adapter.
Its read-replica consistency must be modeled by transport: Sessions/bookmarks
are available through the Workers binding, not the REST query API; enabling
replication with an administrative REST call does not provide Sessions there.
Writes still go to the primary. Do not translate a libSQL or sync watermark into
a D1 bookmark without a verified mapping. No latency winner or benchmark result
is asserted. [D1 replication and limitations](https://developers.cloudflare.com/d1/best-practices/read-replication/).

## 3. Memory, Web Storage, OPFS and hybrid modes

The requested browser persistence is represented precisely rather than treating
all web storage APIs as interchangeable:

- **Memory:** documented `:memory:` mode; reload or process loss destroys data.
- **OPFS:** documented file-backed browser persistence. It belongs to the origin;
  quota, clearing site storage, eviction and device loss remain relevant.
- **localStorage/sessionStorage (Web Storage):** string key/value APIs, not SQL or
  OPFS. No native Turso backend is claimed.
- **IndexedDB:** distinct browser database API; no native Turso backend verified
  in the reviewed official material. A custom VFS is not assumed.
- **Browser Wasm:** requires its documented worker/shared-memory environment,
  including COOP/COEP. Changing a production site's headers is a separate action.
- **Hybrid proposal:** bounded memory cache plus write-through OPFS projections,
  with an explicit pending-command log. Memory success, local commit, remote
  synchronization and server-accepted operation are four different states.

Browser persistence and `:memory:` are documented by the
[browser package](https://github.com/tursodatabase/turso/blob/main/bindings/javascript/packages/wasm/README.md).
OPFS has origin and quota constraints; it is not a server backup or durability
certificate. [OPFS behavior](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system).
The separate sync-wasm package must be pinned and target-tested, not inferred
from database-wasm. [Official Wasm SDK notes](https://github.com/tursodatabase/agent-skills/blob/main/skills/turso-db/sdks/wasm.md).

Turso Sync documents explicit push/pull and last-push-wins conflict handling.
That cannot substitute for any-rs optimistic concurrency, authorization,
idempotency or event ordering. [Sync semantics](https://docs.turso.tech/sync/usage).
For this framework, public browser clients submit commands through the
authenticated framework API and never receive a privileged database token.
Direct browser-to-database sync is **not enabled** by this proposal. A later
scoped-credential design would require a separate explicit security review.

The [machine-readable mode matrix](../../contracts/adapters/storage-modes.json)
records twelve proposed/documented/unverified modes, all disabled and uncertified.
None is certified for `durable_async`; documentation support is not test evidence.

## 4. Provider-agnostic event-buffer architecture

```text
Public renderer/SDK -> verified identity + current membership/policy
                    -> CommandIngress (one remote DB transaction)
                    -> command + operation + idempotency persisted
                    -> HTTP 202 only after confirmed commit

Durable delivery -> runtime adapter -> one HandlerRuntime invocation
                 -> proposal (not another handler call)
                 -> CommitCoordinator transaction
                 -> state + inbox + outbox + owning operation
                 -> recoverable relay/fan-out -> independent subscriptions
```

Workers, Containers, an HTTP bridge and a wake-up notification are execution or
transport mechanisms. They do not become the durable buffer by themselves.
Application handlers communicate only through committed messages. No direct
handler-to-handler RPC, Workers service-binding shortcut, or Container call may
bypass this rule. Pure functions and infrastructure port calls remain allowed.

The initial composition puts authoritative command, idempotency, operation,
state, inbox, outbox and delivery metadata in **one libSQL database**. Optional
Cloudflare Queue notifications may wake dispatchers, but durable database state
remains recoverable without the notification. Do not dual-write database plus
external queue and call it an atomic commit. A later independent queue needs an
outbox relay and its own qualification. `waitUntil()` is not a durable task queue.
[Workers context lifecycle](https://developers.cloudflare.com/workers/runtime-apis/context/).

### Required persistence semantics

1. Accepting a command commits its command, operation and idempotency record
   together. Key scope includes business/project/client/principal/route/key.
   Reusing the key with another normalized request conflicts; do not use raw TOML
   hashes or claim JCS implementation already exists.
2. The owner commits state, its subscription inbox, stable outbox records and the
   terminal operation atomically. Check expected object revisions and lease epoch
   before committing; zero affected conditional rows is a conflict, not success.
3. Scope every query, unique key and relationship by business/project/client.
   Administrative project-scope operations use their distinct existing contract;
   a missing client is never a wildcard.
4. Lease expiry/extension uses authoritative database time and conditional epoch
   checks. Each subscription has independent delivery/inbox state; one ACK must
   not suppress another subscriber.
5. If the response to a commit is lost, report/track `UnknownOutcome` and reconcile
   the same operation/idempotency/inbox identifiers before retrying. No blind
   replay of external effects. A downstream projection cannot reopen its source
   command's terminal operation.
6. Retain metadata for the required replay/retry windows. No automatic migration,
   destructive retention cleanup, or schema initialization on public requests.

For libSQL, separate client-level `execute()` calls are not a multi-statement
transaction. `client.batch(..., "write")` is atomic for a fixed batch. Interactive
transaction code must explicitly roll back/close on errors and inspect conditional
results before commit; `tx.batch()` errors do not themselves guarantee rollback.
[Client API](https://tursodatabase.github.io/libsql-client-ts/interfaces/Client.html),
[Transaction API](https://tursodatabase.github.io/libsql-client-ts/interfaces/Transaction.html).
Primary/replica consistency and any commit-watermark mapping must be tested;
this package does not certify read-your-writes, cross-database transactions,
network failure behavior, or exactly-once external effects.

## 5. Security and configuration

- Database endpoint and token are server environment references, not values in
  TOML, frontend bundles, URLs, logs, fixtures or error text. Ignore `.dev.vars`.
- Client request bodies may not choose a database URL, tenant, principal or
  authorization context. Use trusted identity and current membership/policy;
  reauthorize at execution and every read/subscription.
- Parameterize SQL values. Identifiers come from compiled allowlisted schema,
  never direct string interpolation from input. No arbitrary SQL endpoint.
- Bearer authentication does **not** mean only a Worker can access the database.
  Network allow rules/private endpoints are separate provider mechanisms;
  none was configured or promised. [Turso authentication](https://docs.turso.tech/sdk/authentication),
  [network controls](https://docs.turso.tech/cloud/allow-rules).
- No frontend renderer is imposed. Browser local projections are not permission
  grants and are cleared/partitioned appropriately on identity or scope changes.

## 6. Usage and experimental snapshots

[Versioned quotas and per-entity BLOB requirements](USAGE-AND-ENTITY-SNAPSHOTS.md)
record the approved additional design discussion. The snapshot experiment stays
disabled; it does not replace remote-libSQL or certify a Workers database engine.

## 7. Delivery and exit gates

The [proposal](../../contracts/adapters/plan.toml) deliberately sets
`implemented=false`, `enabled=false`, and has no certified capabilities.
[Acceptance scenarios](../../contracts/adapters/conformance.json) map to existing
requirements; static regression checks only verify the proposal and reject
unsafe combinations. They do not run those scenarios against a service.

Next Codex increments, once a suitable execution environment is available:
1. Build the isolated Worker/Wasm + host transport skeleton and a separate native
   Container target, without altering core contracts or frontend dependencies.
2. Implement remote-libSQL ingress and read ports, then the transactional
   coordinator/buffer and recovery. Pin clients/toolchains and add local tests.
3. Run target builds and ephemeral local transport mocks. Then, under separately
   authorized cloud access, run live rollback, conflict, duplicate, isolation,
   revocation, lease, disconnect, stop/restart and retention tests.
4. Only after actual conformance, enable candidate profiles. Deployment, paid
   services, accounts, credentials and security settings are outside this work.

**Current execution blocker:** no saved cloud coding environment is available;
the prior Windows transfer is incompatible with the Library helper and WSL access
is denied. This is not an additional approval gate invented for source changes.
The owner's Codex workflow is preserved; this package is the reviewable contract
handoff, not a claim that a provider adapter has been built.
