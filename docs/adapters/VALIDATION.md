# Adapter preparation: validation and limitations

Run date: **2026-10-23**. Local interpreter: **Python 3.12.14** in an isolated
virtual environment. Original pinned direct dependencies: PyYAML 6.0.3,
jsonschema 4.25.1, openapi-spec-validator 0.7.2.

| Check | Result |
| --- | --- |
| Original artifact hashes | Both preserved exactly |
| Extracted original package | All 51 files byte-identical to original ZIP |
| Original validator | PASS: 63 definitions, 31 positive examples, 14 structural and 5 semantic negative examples, 321 references, OpenAPI 10 paths/12 operations, 30 requirements/scenarios |
| Repository licensing and local document links | PASS, official GNU license text preserved; AGPL-3.0-only |
| Static adapter proposal validator | PASS: 2 runtime profiles, 12 storage profiles, 13 specified future service scenarios |
| Offline unittest suite | **52 passed**: 13 repository-wrapper tests and 39 proposal-regression tests |
| Dependency compatibility | `uv pip check` passed for the isolated environment |
| GitHub Actions YAML | Parsed and checked for two Python versions and all validation steps; not remotely executed |
| Limited credential-pattern scan | No matching token/private-key markers; not a comprehensive secret audit |

The additional tests validate schema and proposal invariants, including artifact
mismatches, server credentials, parameterization requirements, tenant scope,
atomic-record declarations, unknown-commit handling, local-versus-server ACKs,
usage-meter distinctions and disabled/unqualified BLOB claims. **They are not
transaction simulations, engine fault-injection tests or security certification.**
The 13 scenarios in `contracts/adapters/conformance.json` are future acceptance
requirements; their live execution count is zero.

A read-only independent review caught two preparation defects: nonsync local
commit could be mislabeled as server acceptance in the static validator, and the
ingress failure scenario did not inject failure after writes. Both were corrected;
the local-ACK invariant now covers every mode and the scenario includes each
partial write plus lost response after commit. The hybrid framework-API profile
is distinct from direct database synchronization.

## Not built, run or established

- Runtime adapters, compiler, SDK, Rust/Wasm artifact, native Container image: **not implemented/built**.
- Live Workers, Containers, Turso/libSQL, D1 or browser database integration: **0 tests**.
- Snapshot engine feasibility, tenant security at runtime, crash recovery, live
  transactional atomicity, billing behavior, performance and durability: **not established**.
- Python 3.11 CI and GitHub-hosted execution: **not run**. Only Python 3.12.14 was
  locally checked. Repository publication/authentication remains separate.
- No cloud account, token, OAuth grant, deployment, paid overage or network-policy
  change was made for this increment.

Reproduce with the root README commands. The immutable historical spec report
remains in `spec/my-framework-spec-v0.1.0/`; it was not rewritten to imply new tests.
