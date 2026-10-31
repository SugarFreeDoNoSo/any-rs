# Lambda invocation increment evidence — 2026-10-31 UTC

Base: public any-rs commit `406b194349364d6f4a2f9b2a4353b3b09ba1401d`.
Implemented on a feature branch, not merged to main. No AWS deployment.

- 14 offline API Gateway v2/SQS event tests: passed.
- 2 actual Rust SDK Runtime API loopback protocol/guard tests: passed.
- 15 existing real HTTP/process tests: passed.
- 4 Rust unit tests: passed (2 prior domain, 2 exact-dispatch storage tests).
- 52 original static tests: passed.
- Original 51 files and two source artifacts preserved; validator passed.
- rustfmt and Clippy across all workspace targets: passed.
- Runtime SDK/library dependencies resolved and compiled from Cargo.lock.

Logs are in `evidence/lambda/`. This evidence does not certify IAM, actual SQS
transport, DynamoDB, S3, cloud scaling, RIE, production authorization or live AWS.

The only observed defect during event testing was acceptance of an inconsistent
SQS awsRegion; the adapter now checks it against its allowlisted queue ARN and
the unchanged negative test passes. Probe setup initially lacked SDK-required
environment configuration; it now creates synthetic identity before threads,
after refusing inherited AWS identity/non-loopback addresses.

The owner subsequently prioritized Docker/Kubernetes serverless exploration;
this bounded Lambda milestone is preserved, not expanded or publicly deployed.
