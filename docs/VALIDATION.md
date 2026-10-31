# Fresh validation and limits

This is the 2026-10-22 scaffold report. See the separate
[2026-10-23 adapter-preparation validation](adapters/VALIDATION.md) for the latest
additive package checks; originals below remain unchanged.

Run date: **2026-10-22**. Environment: Python **3.12.14**, isolated virtual
environment, with the original pinned direct dependencies:
PyYAML **6.0.3**, jsonschema **4.25.1**, openapi-spec-validator **0.7.2**.

The unmodified original command was run from its extracted package directory:

```sh
python tools/validate_spec.py
```

## Recovery verification

After the temporary workspace was recycled, both source artifacts were restored
from their existing Library records and verified against the original hashes.
The repository scaffold was reconstructed from the retained authored content.
All checks below were rerun on the recovered files before packaging for durable
Library storage. No GitHub repository was created or published in this step.

| Original check | Fresh result |
| --- | --- |
| JSON Schema definitions | 63 valid |
| Parsed files | 18 TOML, 17 JSON, 1 YAML |
| Positive examples | 31 accepted |
| Invalid structural examples | 14 rejected |
| Invalid semantic examples | 5 rejected |
| Local contract references | 321 resolved |
| OpenAPI | Passed; 10 paths and 12 operations |
| Requirement/scenario traceability | 30 requirements / 30 specified scenarios |
| Imported Markdown file links | 16 checked |

No original specification defect was found by these checks. The standalone
Markdown's closing ZIP note and the historical pending-license language are
explained in [PROVENANCE.md](PROVENANCE.md); the original files remain unchanged.

## Additional repository checks

**Result: passed.** All 51 imported files and both source artifact hashes were
verified; 17 new-document file links were checked; all **13 wrapper regression
tests passed**. No credential-like token or private-key marker was found by a
limited pattern scan; this is not a comprehensive secret audit.

`python tools/validate.py` verifies the two original artifact hashes, all 51
extracted files against the ZIP, official LICENSE bytes, and local links in the
new English documentation, then runs the original validator without modification.
`python -m unittest discover -s tests -v` exercises the wrapper, including changed,
missing, extra, and symlinked imported files, altered checksums/license, broken
links, and original-validator failure propagation. These are repository-tooling
tests, **not** framework runtime tests.

## Not executed / not established

- Runtime conformance scenarios executed: **0**.
- TOON parser, JCS implementation, SDK/BFF/renderer, SQLite runtime, and AWS
  adapter tests: **not implemented or executed**.
- Crash recovery, tenant isolation at runtime, security of an implementation,
  performance, and certification: **not established**.
- The workflow is prepared for Python 3.11 and 3.12. Only Python 3.12.14 was run
  locally in this preparation environment. GitHub-hosted runs must be verified
  for the actual published commit; writing a workflow is not evidence it ran.

This report is separate from the historical report in the preserved package.
