# Executed local vertical slice evidence

Validation ran on 2026-10-31 UTC (2026-10-30 America/Santiago), Linux x86_64,
Rust 1.99.1, in a dedicated any-rs cloud workspace. This is not the unavailable
Windows executor and not a deployment on Cloudflare or Turso.

Input scaffold v1 SHA256:
`05ee7a1d273d3bfc0a3a420c4377e49c0ccc62214650b7fd335a2014622fd637`.

- Rust build with committed Cargo.lock: passed.
- Rust unit tests: 2 passed.
- rustfmt check and Clippy all targets, warnings denied: passed.
- Original static suite: 52 passed; original preservation validator verified
  51 imported files and two source artifacts unchanged.
- Proposed adapter validator: passed, explicitly zero cloud runtime integrations.
- Narrow local OpenAPI 3.1 document: validated and served by HTTP.
- Real HTTP/process tests: **15 passed**, including induced outbox-failure rollback.
  See the complete [test log](evidence/http-tests.txt)
  and [test plan](TEST_PLAN.md). These use local synthetic principals and data.

During review, the operation result initially included an extra reference field,
revision/schema versions initially had incorrect JSON types, and dispatch did not
recheck revoked startup grants. Those issues were fixed before the final passing
suite. Early HTTP timeouts disappeared after explicitly bounding Tokio to two
worker threads; final tests retain their five-second HTTP timeout.

See evidence files in this directory for commands and results. Local CI workflow
is prepared; it has **not** run on GitHub. No repository, PR, deployment, OAuth
grant, cloud database or new external access was created.
