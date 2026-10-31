# Contributing

This repository contains a draft specification, its validation scaffold, and a
bounded local runtime slice. It is not yet a complete production framework. Contributions are licensed AGPL-3.0-only.

1. Read the original specification, decisions, and roadmap in
   [spec/my-framework-spec-v0.1.0](spec/my-framework-spec-v0.1.0/README.md).
2. Keep the imported draft and source artifacts immutable. Propose revisions in a
   new versioned specification directory, with an explicit changelog. Do not
   rewrite historical validation results to make a new check pass.
3. Distinguish established requirements, proposed choices, and implemented
   behavior. Do not label an engine certified before runtime conformance tests
   actually execute and pass.
4. Before starting M1, agree the runtime language and package naming. Start with
   the compiler/core contract, then the proposed SQLite durable runtime. Do not
   add empty packages or pretend adapters merely to suggest progress.
5. Run `python tools/validate.py` and `python -m unittest discover -s tests -v`
   with the pinned original validator dependencies installed before submitting.
6. Keep credentials and private data out of commits. `.gitignore` is a convenience,
   not a secret scanner; inspect the complete staged diff before publishing.

A future runtime must turn the specified scenarios into executable tests for
recovery, duplicate delivery, fan-out, fencing, concurrency, authorization, and
scope isolation. Passing the static validator is not a substitute.
