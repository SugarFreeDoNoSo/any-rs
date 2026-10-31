# SAM custom runtime build. Cargo Lambda uses Zig for AL2023-compatible Linux.
# No deployment target exists. Run only after installing official SAM/Docker/Cargo Lambda.
.PHONY: build-AnyRsHttpFixture build-AnyRsScenarioFixture
build-AnyRsHttpFixture build-AnyRsScenarioFixture:
	cargo lambda build --release --locked --target x86_64-unknown-linux-gnu --package any-rs-lambda --bin any-rs-sam-fixture
	mkdir -p "$(ARTIFACTS_DIR)"
	cp target/lambda/any-rs-sam-fixture/bootstrap "$(ARTIFACTS_DIR)/bootstrap"
