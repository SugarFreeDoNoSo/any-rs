#!/usr/bin/env bash
# No cloud commands, credentials, image publishing or installation.
set -euo pipefail
cd "$(dirname "$0")/.."
export SAM_CLI_TELEMETRY=0 AWS_EC2_METADATA_DISABLED=true AWS_DEFAULT_REGION=us-east-1
export AWS_SHARED_CREDENTIALS_FILE=/dev/null AWS_CONFIG_FILE=/dev/null
unset AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN AWS_SECURITY_TOKEN AWS_PROFILE
for command in docker sam cargo; do
  if ! command -v "$command" >/dev/null; then
    echo "BLOCKED: $command is unavailable; SAM/Docker execution was not performed." >&2
    exit 78
  fi
done
if ! docker info >/dev/null 2>&1; then
  echo 'BLOCKED: Docker daemon unavailable; no permission changes attempted.' >&2
  exit 78
fi
cargo lambda --version >/dev/null
sam validate --lint --template-file adapters/aws-sam/template.yaml
sam build --template-file adapters/aws-sam/template.yaml
sam local invoke AnyRsScenarioFixture --template-file .aws-sam/build/template.yaml \
  --event adapters/aws-sam/events/scenario.json
sam local invoke AnyRsHttpFixture --template-file .aws-sam/build/template.yaml \
  --event adapters/aws-sam/events/version-v2.json
