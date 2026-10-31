# Provenance and repository decisions

## Preserved inputs

The owner supplied two artifacts. Both are copied without modification under
`spec/originals/`; their SHA-256 digests are recorded in
[spec/SHA256SUMS](../spec/SHA256SUMS).

| Artifact | SHA-256 |
| --- | --- |
| `my-framework-spec-v0.1.0.zip` | `4cef70e600dde218154235b8394aec44360254f5ae0c91074b3805287cc86958` |
| `my-framework-spec-v0.1.0.md` | `6a43dd5f27c2e4eb99ccbba6890ec6215fa6280a7a758ec38dd6dc44d9025d01` |

The ZIP's 51 files are extracted into `spec/my-framework-spec-v0.1.0/` with every
file's bytes unchanged. The repository validator compares the extracted tree to
the preserved ZIP before executing the original validator. Do not put new files
inside this immutable imported tree.

The standalone Markdown contains the same specification plus a short closing
note pointing to the ZIP; it is not byte-identical to `SPECIFICATION.md` in the
archive. Its original relative links are preserved as evidence, not rewritten.
Use the extracted package for browsing the linked documents. This is an input
packaging difference, not a failing specification check.

The artifact name says `v0.1.0`; the draft's internal version is
`0.1.0-draft.1`. The original title, `My freamework`, is preserved. The owner subsequently selected **any-rs** as the public project name on
2026-10-23. Original filenames, titles and contents remain evidence of the earlier
draft. No final release or registry package publication is asserted.

## License

The owner requested a public AGPL repository. The repository adopts
**AGPL-3.0-only**. The original draft's references to an undecided license remain
untouched as historical evidence and are superseded for this repository by the
root README and LICENSE.

`LICENSE` was retrieved verbatim from the GNU project's official text at
<https://www.gnu.org/licenses/agpl-3.0.txt> on 2026-10-22. SHA-256:
`0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0`.
The license text itself is not edited to express “only”; the repository's explicit
SPDX identifier supplies that choice. The original license-document copyright
notice is retained.

## Validation provenance

The original `VALIDATION.md` is historical evidence supplied in the ZIP. A fresh
run is separately recorded in [docs/VALIDATION.md](VALIDATION.md). No runtime
certification is inferred from either report. New wrappers and tests live outside
the imported package. If an original validator fails, report the defect and fix
new wrapper behavior or propose a versioned revision; do not silently patch the
original evidence.

## Adapter extension, 2026-10-23

The owner requested Workers, Containers and Turso as adapters. The new contract
revision `0.1.0-contract.1` lives outside the immutable imported tree. It prepares
implementation and offline acceptance checks; it does not revise the original
contract silently, choose a production account, or certify provider behavior.
See the [extension](adapters/WORKERS-CONTAINERS-TURSO.md) and
[Codex handoff](adapters/CODEX-HANDOFF.md).

## Local Rust vertical slice increment (2026-10-30 Santiago)

Continues the same any-rs scaffold v1. Input archive SHA256:
`05ee7a1d273d3bfc0a3a420c4377e49c0ccc62214650b7fd335a2014622fd637`.
All 51 imported specification files and the two original source artifacts remain
byte-identical; repository validators were executed after the implementation.
New Rust sources, a narrow local API description, configuration and integration
tests live outside the immutable specification. Local SQLite results do not
certify the separately planned remote-libSQL / Workers / Containers profiles.
No real-user data or live cloud credentials were used. The fixture runtime is
AGPL-3.0-only, like the repository; dependencies retain their own licenses.
