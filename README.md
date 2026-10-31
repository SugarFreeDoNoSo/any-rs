# any-rs

A declarative, event-driven framework specification for applications spanning
multiple businesses, projects, and clients.

**Status:** specification `0.1.0-draft.1`, validation tooling, and an additive
[local Rust HTTP/SQLite vertical slice](docs/runtime/LOCAL-SLICE.md).
The bounded fixture runtime is implemented; the general framework, SDK, compiler
and cloud adapters are not implemented or certified. The owner selected the public project name **any-rs** on 2026-10-23.
Historical Spanish artifacts remain unchanged.

## Declarative tasks state machine

The [bounded tasks slice](docs/runtime/TASKS.md) runs a neutral, typed FSM object
in Rust: pending → in_progress → completed, with two explicit transitions and
deny-by-default behavior. JSON/TOML parity, CAS and atomic event persistence are
tested. Authoring/client SDKs and gRPC remain planned, not implemented.

## Runtime dashboard and SAM local preparation

The authenticated [runtime dashboard](docs/runtime/DASHBOARD.md) generates HTML,
Markdown and Mermaid from the compiled OpenAPI contract.
[SAM local instructions](docs/adapters/AWS-SAM.md) prepare the same adapters for
Docker-based tests; native tests do not certify SAM/container execution.

## AWS Lambda invocation adapter

An additive [HTTP API v2 / SQS invocation profile](docs/adapters/AWS-LAMBDA.md)
can be exercised locally with synthetic events. AWS persistence, SQS transport,
production auth and deployment remain unimplemented. The original local runtime
is preserved.

## Workers, Containers and Turso increment

An additive [adapter contract and test-preparation package](docs/adapters/WORKERS-CONTAINERS-TURSO.md)
now covers both Cloudflare compute targets plus remote Turso-hosted **libSQL**.
This is **not an implemented or deployed adapter**: schemas, capability planning
checks and offline proposal-regression tests are delivered; Workers/Wasm, Container
builds and live database integration remain unexecuted.

## Start here

- [Original specification](spec/my-framework-spec-v0.1.0/SPECIFICATION.md) (Spanish)
- [Architecture and implementation status](docs/ARCHITECTURE.md) (English)
- [Original decisions](spec/my-framework-spec-v0.1.0/DECISIONS.md),
  [roadmap](spec/my-framework-spec-v0.1.0/ROADMAP.md), and
  [conformance scenarios](spec/my-framework-spec-v0.1.0/conformance/scenarios.feature)
- [HTTP contract](spec/my-framework-spec-v0.1.0/api/openapi.yaml) and
  [JSON Schema contracts](spec/my-framework-spec-v0.1.0/schemas/contracts.schema.json)
- [Provenance and licensing decisions](docs/PROVENANCE.md)
- [Validation results and limits](docs/VALIDATION.md)
- [Adapter validation results](docs/adapters/VALIDATION.md)
- [Provider usage and experimental entity snapshots](docs/adapters/USAGE-AND-ENTITY-SNAPSHOTS.md)

## Validate locally

Python 3.11 or newer is required. Validation does not need application credentials
or a `.env` file. Install the original validator's pinned direct dependencies:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r spec/my-framework-spec-v0.1.0/tools/requirements.txt
python tools/validate.py
python tools/validate_adapter_contracts.py
python -m unittest discover -s tests -v
```

The wrapper checks source preservation and the repository scaffold, then runs the
**unmodified original validator** in its own package directory. To run that
validator directly:

```sh
cd spec/my-framework-spec-v0.1.0
../../.venv/bin/python tools/validate_spec.py
```

The GitHub Actions workflow is configured to run these checks on pushes and pull requests. A
passing check establishes static specification consistency, not working runtime
behavior. The Gherkin scenarios are specifications, not executed acceptance tests.

## Design intent

- Definitions authored in TOML compile to a language-neutral representation.
- Handlers communicate through durable messages, not direct handler calls.
- Business/project/client scope is enforced throughout storage and authorization.
- Durable acceptance, inbox/outbox, optimistic revisions, and fencing are required
  for the proposed asynchronous runtime.
- SQLite is the proposed first local adapter. Cloud adapters, an ESM SDK,
  renderer integrations, and certified TOON support are future work.

These statements summarize the full proposal. Only the explicitly documented
local slice has executable coverage; other statements do not claim delivered features.
See [CONTRIBUTING.md](CONTRIBUTING.md) before changing contracts or starting a
runtime implementation.

## License

This repository is licensed under **GNU Affero General Public License version 3
only** (`AGPL-3.0-only`), as requested by the project owner. See [LICENSE](LICENSE)
for the unmodified official license text. This is a repository-level decision:
statements in the preserved draft that the license was undecided describe its
historical state, not the current repository license.
