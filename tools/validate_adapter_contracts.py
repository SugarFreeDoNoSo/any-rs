#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
"""Offline validation of adapter proposals; not an adapter or runtime test."""
from __future__ import annotations

import json
from pathlib import Path
import tomllib
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_GATES = {
    'original_validation', 'contract_tests', 'workers_build', 'container_build',
    'live_remote_transaction_tests', 'authorization_conformance',
    'fault_recovery_conformance', 'cost_and_deployment_approval',
}
CANDIDATE_CAPABILITIES = {
    'atomic_ingress', 'atomic_state_inbox_outbox_operation', 'conditional_write',
    'leases_with_fencing', 'independent_subscriptions', 'dead_letter',
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def validate_plan(plan, schema):
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(plan)
    runtimes = {item['id']: item for item in plan['runtimes']}
    require(len(plan['runtimes']) == 2 and set(runtimes) == {
        'cloudflare_workers', 'cloudflare_containers'}, 'exactly two distinct runtime profiles required')
    expected = {
        'cloudflare_workers': ('isolate', 'wasm32-unknown-unknown', 'workers_rs_js_bindings', 'https_fetch', 'ephemeral'),
        'cloudflare_containers': ('container', 'linux/amd64', 'worker_durable_object_container_api', 'https_client', 'restartable'),
    }
    for name, values in expected.items():
        actual = tuple(runtimes[name][key] for key in ('model', 'artifact', 'bridge', 'network', 'invocation_lifetime'))
        require(actual == values, f'incompatible artifact/host capabilities for {name}')
    require(set(plan['storage']['candidate_capabilities']) == CANDIDATE_CAPABILITIES,
            'candidate capabilities changed without a contract revision')
    require(plan['storage']['credential']['env'] != plan['storage']['endpoint']['env'],
            'credential and endpoint must be separate environment references')
    require(set(plan['buffer']['acceptance_records']) == {'command', 'operation', 'idempotency'},
            'durable acceptance must commit all three records')
    require(set(plan['buffer']['commit_records']) == {'state', 'inbox', 'outbox', 'operation'},
            'owning command commit must include all four records')
    require(set(plan['security']['tenant_scope']) == {'business_id', 'project_id', 'client_id'},
            'full client scope is required')
    require(set(plan['activation']['requires']) == REQUIRED_GATES,
            'activation gates cannot be removed or silently changed')


def validate_modes(matrix):
    modes = matrix['modes']
    ids = [m['id'] for m in modes]
    require(len(ids) == len(set(ids)), 'duplicate storage mode')
    require(matrix['status'] == 'planning_only', 'storage matrix is not a runtime certification')
    for m in modes:
        require(m['implemented'] is False and m['enabled'] is False, 'future mode cannot be silently activated')
        require(m['certified'] is False, 'no live engine certification exists')
        require(m['database_token_in_public_client'] is False, 'browser/client database token exposure forbidden')
        require(m['documentation_status'] in {'documented', 'unverified', 'not_a_database_backend'}, 'unknown evidence status')
        require(m['authoritative_server_buffer'] is False, 'no storage mode is certified as server buffer yet')
        if m['medium'] in {'web_storage', 'indexeddb'}:
            require(m['documentation_status'] != 'documented', 'no verified native Turso backend for this medium')
        if m['runtime'] == 'workers' and m['engine_family'] == 'turso_browser_wasm':
            require(False, 'browser threaded Wasm package is not a Workers target')
        require(m['local_success_is_server_acceptance'] is False, 'local completion is not command acceptance')
    require({'browser_memory', 'browser_opfs', 'browser_indexeddb_unverified', 'browser_web_storage',
             'workers_memory', 'containers_filesystem', 'libsql_remote', 'turso_remote_future',
             'turso_sync_future', 'libsql_embedded_replica_future', 'browser_sync_wasm_future',
             'browser_hybrid_framework_api_future'} <= set(ids), 'required storage-mode coverage missing')


def validate_scenarios(scenarios, requirement_ids):
    require(scenarios['execution'] == 'specified_not_live_executed', 'no live scenario evidence exists')
    ids = [case['id'] for case in scenarios['scenarios']]
    require(len(ids) == 13 and len(ids) == len(set(ids)), 'expected 13 distinct adapter acceptance scenarios')
    for case in scenarios['scenarios']:
        require(bool(case['acceptance']), 'acceptance text is required')
        require(bool(case['requirements']) and set(case['requirements']) <= requirement_ids,
                'scenario references an unknown original requirement')


def validate_usage(proposal):
    require(proposal['status'] == 'planning_only', 'usage is a planning profile')
    p = proposal['provider_profile']
    require(p['revalidate_before_activation'] is True and p['core_constants'] is False,
            'provider plan must be versioned and revalidated')
    require(p['billing_account_verified'] is False, 'no account verification exists')
    for flag in ('query_count_equals_rows_read', 'row_count_equals_payload_bytes',
                 'automatic_paid_overage_authorized', 'unknown_usage_is_zero'):
        require(p[flag] is False, f'unsafe usage assumption: {flag}')
    require(p['provider_meter_authoritative'] is True, 'estimates do not override billing meters')
    require(p['quota_error_policy'] == 'retryable_not_accepted_without_durable_commit',
            'quota error cannot acknowledge uncommitted work')
    require(p['source'] == 'https://turso.tech/pricing' and p['observed_on'] == '2026-10-23',
            'dated plan evidence required; revise profile for new observations')
    require(p['allowances'] == {'monthly_rows_read': 500000000, 'monthly_rows_written': 10000000,
            'storage': {'amount': 5, 'unit': 'GB', 'exact_bytes': None},
            'monthly_sync': {'amount': 3, 'unit': 'GB', 'exact_bytes': None}},
            'provider allowance differs from this dated evidence snapshot')
    require(set(p['telemetry_dimensions']) == {'requests', 'rows_read', 'rows_written',
            'storage_bytes', 'sync_bytes', 'wire_bytes', 'snapshot_bytes', 'retry_count'},
            'usage dimensions must remain separate')
    e = proposal['entity_blob_experiment']
    require(e['status'] == 'unqualified_experiment', 'snapshot experiment is not qualified')
    for flag in ('implemented', 'enabled', 'replaces_initial_remote_adapter', 'meter_bypass_claim',
                 'unknown_limit_means_unlimited', 'browser_opfs_package_is_workers_compatible'):
        require(e[flag] is False, f'unqualified snapshot claim: {flag}')
    for flag in ('durable_event_log_before_batch', 'idempotent_command_ids',
                 'checkpoint_after_snapshot_commit_only', 'parameterized_sql',
                 'full_snapshot_rewrite', 'base64_expansion_accounted'):
        require(e[flag] is True, f'missing snapshot safety requirement: {flag}')
    require(set(e['scope']) == {'business_id', 'project_id', 'client_id', 'entity_id'}, 'snapshot scope incomplete')
    require(e['single_writer'] == 'durable_coordination_plus_remote_cas_fencing', 'remote fencing required')
    require(e['acceptance_ack'] == 'durable_command_operation_idempotency_commit' and
            e['application_ack'] == 'atomic_snapshot_checkpoint_inbox_outbox_operation_commit',
            'durable acceptance and applied acknowledgement differ')
    require(set(e['atomic_snapshot_fields']) == {'blob', 'version', 'checkpoint', 'digest', 'schema_version'},
            'snapshot publication metadata must be atomic')
    require(e['unknown_commit'] == 'reconcile_before_retry', 'snapshot unknown outcome needs reconciliation')
    require(e['cross_entity_atomicity'] == 'not_certified' and e['workers_database_wasm_build'] == 'not_qualified',
            'no engine/transaction certification exists')
    require(e['hosted_limits'] == {'max_blob_bytes': None, 'max_row_bytes': None,
            'max_request_bytes': None, 'max_response_bytes': None} and e['app_max_snapshot_bytes'] is None,
            'unknown hosted limits cannot become fabricated guarantees')
    require(set(e['activation_requires']) == {'hosted_limits_verified', 'account_meter_verified',
            'app_size_budget_set', 'target_wasm_or_native_build', 'concurrent_writer_fencing_tests',
            'crash_recovery_tests', 'unknown_commit_tests', 'tenant_authorization_tests',
            'memory_cpu_load_tests', 'cost_and_deployment_approval'}, 'snapshot activation gates incomplete')


def main():
    base = ROOT / 'contracts/adapters'
    plan = tomllib.loads((base / 'plan.toml').read_text())
    schema = json.loads((base / 'plan.schema.json').read_text())
    matrix = json.loads((base / 'storage-modes.json').read_text())
    scenarios = json.loads((base / 'conformance.json').read_text())
    usage = json.loads((base / 'usage-and-experiment.json').read_text())
    requirements = tomllib.loads((ROOT / 'spec/my-framework-spec-v0.1.0/requirements.toml').read_text())
    validate_plan(plan, schema)
    validate_modes(matrix)
    validate_scenarios(scenarios, {r['id'] for r in requirements['requirements']})
    validate_usage(usage)
    print(json.dumps({'status': 'pass', 'runtime_profiles': len(plan['runtimes']),
                      'storage_mode_profiles': len(matrix['modes']), 'specified_scenarios': 13,
                      'live_integration_tests_executed': 0, 'adapter_implemented': False}, indent=2))


if __name__ == '__main__':
    main()
