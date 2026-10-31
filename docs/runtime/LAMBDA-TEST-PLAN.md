# Offline Lambda adapter tests

Run `python3 tests/lambda_events.py --binary target/debug/any-rs-lambda-fixture` after building the fixture binary. Python stdlib only. No AWS requests, real tokens or cloud resources are used. Each test creates a temporary SQLite database and environment-only fixture credentials.

Coverage: API Gateway HTTP API v2 normalization; warm multi-invocation process and cold restart durability; asynchronous acceptance with no post-response worker; exact-operation SQS dispatch; duplicate terminal delivery; partial batch failures; malformed records and envelopes; principal/scope matching; operation privacy; idempotency and conflicting replay; base64 body and case-insensitive headers; oversized bodies and ambiguous comma-joined headers; revoked authorization; refusal to run the SQLite fixture inside Lambda.

Boundary: this exercises local AWS-shaped events, not the AWS Runtime API, IAM, SQS visibility/deletion, deployment, production persistence, or actual AWS delivery semantics. Passing tests is not evidence of a cloud deployment.

## Recorded result

2026-10-31 UTC: `python3 tests/lambda_events.py` passed **14/14**, exit **0**, 0.129 seconds. Tests additionally prove separate tenant/principal idempotency namespaces, same object ID in different tenant scopes, domain-conflict terminal rejection, transaction rollback on an injected outbox trigger failure and subsequent successful retry, and preflight rejection of an unreportable batch before any domain mutation.

An initial test run caught mismatched SQS `awsRegion` being acknowledged; the production adapter was corrected by its owner and the unchanged negative assertion passed on the rebuilt binary. HTTP malformed events intentionally use HTTP 400 problem responses; malformed SQS envelopes use fixture invocation errors. This distinction is asserted.
