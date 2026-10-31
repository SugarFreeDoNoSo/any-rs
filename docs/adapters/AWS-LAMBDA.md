# AWS Lambda — invocation adapter milestone

Status: **local contract implementation, not an AWS deployment**. Additive profile
in the existing any-rs project, after the local runtime baseline published as
`406b194349364d6f4a2f9b2a4353b3b09ba1401d`. AGPL-3.0-only. Original specifications
remain immutable. This profile does not replace the Workers/Containers/Turso plan.

## Implemented boundary

`any-rs-lambda` is a library with explicit, independent `LambdaAdapter::run_http`
and `run_sqs` entrypoints using the official `lambda_runtime` Runtime API client.
These entrypoints compile and are exercised through the actual Rust SDK against
a loopback Runtime API mock. Live AWS execution is **not certified**.
A host supplies the `RuntimeStore` and configured principal grants. There is no
background dispatcher in either invocation adapter. Every storage operation is
awaited before returning the invocation result.

- API Gateway **HTTP API payload 2.0**, default/root mapping only. REST API v1,
  ALB, Function URLs, custom domain mapping rewrites and nonempty query strings
  are outside this milestone. Root routes and JSON schemas reuse the local API.
- Header names are normalized; ambiguous or comma-combined Authorization and
  Idempotency-Key values are rejected. Authentication is still the explicit
  development bearer allowlist, **not production IAM/JWT authorization**.
- Base64 request bodies are decoded and limited to 16 KiB. The explicit gateway
  response envelope contains statusCode, headers, body and isBase64Encoded.
- SQS **standard** batches of 1–10 references are handled serially. The queue ARN
  is allowlisted exactly; FIFO is unsupported. A reference contains only
  operation_id, principal_id and scope. Command data are loaded from persisted
  state, never substituted by a queue message. A wrong scope/principal cannot
  dispatch another operation or drain another tenant's queue.
- Retry of a terminal operation is an acknowledged no-op. Persisted business
  rejection is acknowledged; malformed references/storage failures return their
  original messageId in batchItemFailures. Missing/duplicate message IDs fail the
  invocation before processing any item. Infrastructure must later enable
  **ReportBatchItemFailures** for this response to have its intended effect.
- Grants are immutable for a process lifetime; each HTTP request authenticates
  anew, and worker dispatch rechecks the configured grants. Cold-start revocation
  is tested. Production revocation across warm environments is **not solved**.

## What is deliberately NOT implemented

| Responsibility | This milestone | Next acceptance gate |
| --- | --- | --- |
| Lambda invocation/lifecycle | Runtime entrypoints + local event harness + loopback Runtime API test | RIE and authorized AWS verification |
| Shared persistence | RuntimeStore port; existing SQLite only in local harness | DynamoDB transactional adapter with scoped keys, conditional writes, durable idempotency, atomic object/operation/inbox/outbox |
| SQS transport | Event consumption shape and partial failures | Durable delivery-intent/outbox relay + SendMessage + source mapping, retry/visibility/DLQ policy |
| S3 blobs | None; unnecessary for small note fixture | Optional large-blob adapter, explicit integrity/lifecycle and atomicity boundaries |
| Auth | Development bearer/grant allowlist | Production provider, expiry/revocation, least-privilege IAM |

There is **no end-to-end HTTP→AWS SQS delivery** yet. Local acceptance persists
an accepted command, but no producer sends it to SQS. The harness explicitly
injects a reference as a separate invocation. The existing SQLite outbox records
*completed domain events*, not a new command-delivery intent. A future DynamoDB
accept must atomically write the accepted operation/idempotency and a durable
queue-delivery intent before returning 202; a relay then sends that intent.
Do not bolt `accept; SendMessage` together and call the failure window solved.

DynamoDB's temporary ClientRequestToken deduplication is not a replacement for
application idempotency records. S3 is not a transactional replacement for these
records. No DynamoDB SDK, S3 SDK, live service, credentials, cloud resource,
permission change, paid infrastructure or deployment was used here.

## Reproduce without an AWS account

```sh
cargo build --locked --workspace
python tests/lambda_events.py
python tests/runtime_http.py
cargo test --locked --workspace
cargo build --locked -p any-rs-lambda --example runtime_api_probe
python tests/lambda_runtime_api.py
cargo clippy --locked --workspace --all-targets -- -D warnings
```

The only executable shipped here, `any-rs-lambda-fixture`, is a **local JSONL
harness**, not a Lambda `bootstrap`. It rejects both AWS_LAMBDA_FUNCTION_NAME and
AWS_LAMBDA_RUNTIME_API, preventing accidental use as an AWS function backend.

```sh
export ANYRS_FIXTURE_TOKEN='fixture-only-token-at-least-16-characters'
target/debug/any-rs-lambda-fixture \
  --database /absolute/local-fixture.sqlite3 \
  --principals adapters/aws-lambda/principals.example.json
```

Each stdin line is `{"kind":"http","event":{...}}` or
`{"kind":"sqs","event":{"Records":[...]}}`. Each stdout line is the response.
Tests construct complete fixture events; see `tests/lambda_events.py`. The fixture
queue ARN is `arn:aws:sqs:us-east-1:000000000000:any-rs-fixture` (synthetic).

Warm tests reuse this adapter object. Cold tests create a new process reading the
same **host-local fixture file**, outside invocation memory. This proves the
adapter does not depend on a warm principal/request or background worker, NOT
cross-instance Lambda durability. `/tmp` and Lambda execution memory are not a
shared durable database. Synthetic SQLite fault injection is not DynamoDB fault
injection. No Lambda performance, scaling, timeout or billing claim is made.

`/version` identifies this invocation profile. `/openapi.json` continues serving
the shared **local endpoint contract**, not an AWS deployment/OpenAPI integration
configuration. No x-amazon-apigateway integration or infrastructure template is
provided.

## Primary references checked 2026-10-31

- [AWS HTTP API payload 2.0 and response envelope](https://docs.aws.amazon.com/apigateway/latest/developerguide/http-api-develop-integrations-lambda.html)
- [AWS Lambda execution environment/freeze lifecycle](https://docs.aws.amazon.com/lambda/latest/dg/lambda-runtime-environment.html)
- [AWS execution reuse and security guidance](https://docs.aws.amazon.com/lambda/latest/dg/best-practices.html)
- [SQS partial failures, retries and FIFO distinction](https://docs.aws.amazon.com/lambda/latest/dg/services-sqs-errorhandling.html)
- [Official Rust runtime and examples](https://github.com/aws/aws-lambda-rust-runtime)
- [DynamoDB transaction semantics and token lifetime](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html)

Context7 `/aws/aws-lambda-rust-runtime` was consulted alongside these sources.
Cargo resolved and locked `lambda_runtime 1.5.0` and `aws_lambda_events 1.2.1`;
versions were verified by build rather than inferred only from upstream main.

The test-only `runtime_api_probe` example refuses inherited AWS function identity
and non-loopback endpoints before opening a database. It then synthesizes the
SDK-required fixture identity before creating runtime threads. Two tests cover
the SDK protocol and these guards; this is not an AWS service emulator.
