# AWS SAM + Docker local — prepared, not deployed

Latest owner direction: use **SAM**, not LocalStack or Kubernetes/Knative. This
continues any-rs and its Lambda invocation adapters; no separate project.

## What is available

- `adapters/aws-sam/template.yaml`: two **local-fixture-only** functions,
  `provided.al2023`, x86_64, Handler `bootstrap`, Makefile builder.
- Root Makefile builds `any-rs-sam-fixture` through Cargo Lambda/Zig and copies
  only `bootstrap` to ARTIFACTS_DIR. This avoids assuming a Debian-built GNU
  executable is compatible with AL2023's glibc 2.34.
- `AnyRsScenarioFixture` uses one invocation to create a temporary fixture store,
  explicitly create a task in pending, then accept start/complete HTTP v2 commands,
  invoke the SQS adapter for each operation, retry the same deliveries, and read
  the completed task at revision 3 and report JSON checks. All operations
  finish before the invocation response. No background worker after response.
- `AnyRsHttpFixture` accepts HTTP API v2 and exposes the same authenticated
  routes, including `/.../dashboard` and `/.../schema.md`, plus `/openapi.json`.
  A directly injected Records envelope also exercises the SQS adapter, but
  there is no real queue or delivery source.
- Both profiles require AWS_SAM_LOCAL=true (set by SAM) and the explicit fixture
  flag. They refuse default/cloud startup. Environment flags prevent accidents,
  **are not authentication or a sandbox security boundary**.

## Persistence is deliberately a fixture

`/tmp` is used only for synthetic data and can disappear with the container.
The scenario creates its own store and removes it when successful. The HTTP
fixture can reuse state in a warm process, but a different function/container
cannot be assumed to see it. This does NOT meet production durable 202 semantics
across container replacement. The profile is unsuitable for real user data.

SAM does not emulate SQS, DynamoDB or S3. No queue polling, visibility timeout,
IAM, DLQ, cloud durability, production token or distributed storage is tested.
Those services remain separate adapter milestones. `--docker-volume-basedir`
is a code-location option, not an arbitrary shared SQLite data mount.

## Run on a machine with Docker

Install official [SAM CLI](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html),
a running Docker engine and [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html)
with its Zig compiler. Use the pinned Rust toolchain in this repository.
No AWS account is needed for these synthetic local checks.

```sh
bash tools/test_sam_local.sh
# Optional HTTP exploration; uses warm local containers, not durable storage:
SAM_CLI_TELEMETRY=0 AWS_EC2_METADATA_DISABLED=true \
  sam local start-api --template-file .aws-sam/build/template.yaml \
  --warm-containers eager --host 127.0.0.1
curl -H 'Authorization: Bearer sam-only-fixture-token-not-a-secret' \
  http://127.0.0.1:3000/v2/businesses/demo/projects/notes/clients/local/schema.md
curl -H 'Authorization: Bearer sam-only-fixture-token-not-a-secret' \
  http://127.0.0.1:3000/v2/businesses/demo/projects/notes/clients/local/dashboard
```

The token is deliberately public and fixture-only. Do not repurpose it for any
real deployment. The browser does not automatically add Bearer headers; use a
local authenticated client or header-aware development tool. Do not put tokens
in URLs or generated pages.

The script checks tools/daemon first, exits 78 on a blocker, disables telemetry
and credential discovery, and contains no deploy/sync/package command. It does
not install software or change permissions. Never run `sam deploy` for this
fixture template.

## Tests possible without Docker

```sh
cargo build --locked --workspace
python tests/sam_fixture.py
python tests/dashboard_http.py
python tests/lambda_events.py
cargo build --locked -p any-rs-lambda --example runtime_api_probe
python tests/lambda_runtime_api.py
```

These are native/loopback tests, not Docker or SAM invocations. The current
executor has no Docker CLI/daemon socket, so a container image/bootstrap run and
`sam local invoke` are blocked. See the increment evidence for actual commands.

## Primary sources

- [SAM custom runtime/Makefile contract](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/building-custom-runtimes.html)
- [SAM Rust and Cargo Lambda builds](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/building-rust.html)
- [SAM local invoke and mounts](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/sam-cli-command-reference-sam-local-invoke.html)
- [Warm local HTTP containers](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/using-sam-cli-local-start-api.html)
- [SAM's AWS_SAM_LOCAL injection](https://github.com/aws/aws-sam-cli/blob/develop/samcli/local/lambdafn/env_vars.py)
- [AL2023 runtime compatibility](https://aws.amazon.com/blogs/compute/introducing-the-amazon-linux-2023-runtime-for-aws-lambda/)
