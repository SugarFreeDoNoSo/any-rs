# Codex handoff: any-rs runtime and persistence adapters

Use this same project, not a new implementation/repository. The public repository
intent remains `SugarFreeDoNoSo/any-rs`, AGPL-3.0-only. Do not infer a successful
publication from that target string; verify remote existence before writing.

## Inputs and integrity

The original specification stays immutable under `spec/my-framework-spec-v0.1.0`
and `spec/originals`; the repository validator checks both source hashes and all
51 imported files. New decisions, fixtures and implementation belong outside it.
The owner selected **any-rs** and requested Workers **and** Containers plus Turso,
including memory/browser/hybrid modes. Requirements precede implementation for
this extension. Read the [adapter contract](WORKERS-CONTAINERS-TURSO.md).

## First reviewable implementation slice

Implement one remote-libSQL composition behind existing logical ports, with two
runtime capability adapters. Keep Workers Wasm/JS glue separate from native
`linux/amd64` Container execution. Do not rewrite the core in Node merely because
a host SDK uses TypeScript. Decide and document the bridge ABI and pin dependencies
before implementation. The native Rust HTTP libSQL transport is not yet qualified.

Start with authenticated `CommandIngress` and scoped `OperationStore`; add the
transactional commit/outbox path rather than treating HTTP invocation as completion.
Tests must prove 202 only after durable acceptance, idempotency replay/conflict,
parameterized SQL, scope isolation and fail-closed authorization. Then implement
leases/fencing, inbox/outbox atomicity, fan-out and recovery. No direct
application-handler calls or cloud deployment in the first source-only increment.
Do not expose a privileged Turso token to a browser or accept arbitrary SQL/URLs.

Browser OPFS/memory, sync-wasm, native embedded replicas, new Turso engine and D1
are separate future profiles. Do not silently substitute one for remote libSQL.
Document whether an observed success means local commit, synchronized state,
server command acceptance or domain completion.

## Local validation commands

From the repository root, Python 3.11+:

```sh
python -m pip install -r spec/my-framework-spec-v0.1.0/tools/requirements.txt
python tools/validate.py
python tools/validate_adapter_contracts.py
python -m unittest discover -s tests -v
```

These current checks are static proposal/provenance regressions. They do not build
Rust/Wasm, Docker images or Workers, and do not contact Turso or execute runtime
acceptance scenarios. Add actual implementation-specific commands and report
passed/failed/not-run separately.

## Environment and publication boundaries

The existing Windows Library helper failed because `os.setxattr` is unavailable;
no supported transfer completed there. Do not patch away required metadata,
manually replay signed transfer URLs, change executor to evade denied WSL access,
or install WSL/security/authentication tooling without authorization. Use the
supported Library transfer on the chosen consumer, or a user-supplied local ZIP
with recorded provenance and verified hash. A parent's cloud-local path is not
proof of a file on the consumer.

Use the existing Codex task when its environment is suitable; otherwise obtain a
supported saved cloud environment from the catalog. No implicit desktop/WSL work.
No new OAuth grant, credential, cloud account, deployment, billing change or
network restriction is authorized by this handoff. Existing publication approval
still requires an authenticated supported route, secret scan, exact remote commit
verification and CI results. Preserve the source-only package if that remains blocked.
