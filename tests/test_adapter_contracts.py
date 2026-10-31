# SPDX-License-Identifier: AGPL-3.0-only
"""Static proposal regression tests, NOT Workers, Containers or Turso integration."""
import copy
import importlib.util
import json
from pathlib import Path
import tomllib
import unittest
from jsonschema.exceptions import ValidationError, SchemaError

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('adapter_validator', ROOT / 'tools/validate_adapter_contracts.py')
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


class AdapterContractTests(unittest.TestCase):
    def setUp(self):
        base = ROOT / 'contracts/adapters'
        self.plan = tomllib.loads((base / 'plan.toml').read_text())
        self.schema = json.loads((base / 'plan.schema.json').read_text())
        self.matrix = json.loads((base / 'storage-modes.json').read_text())
        self.scenarios = json.loads((base / 'conformance.json').read_text())
        self.usage = json.loads((base / 'usage-and-experiment.json').read_text())
        r = tomllib.loads((ROOT / 'spec/my-framework-spec-v0.1.0/requirements.toml').read_text())
        self.requirements = {x['id'] for x in r['requirements']}

    def check(self):
        validator.validate_plan(self.plan, self.schema)

    def rejected(self, mutate):
        mutate(self.plan)
        with self.assertRaises((ValueError, ValidationError, SchemaError)):
            self.check()

    def test_supported_proposals_pass(self):
        self.check()
        validator.validate_modes(self.matrix)
        validator.validate_scenarios(self.scenarios, self.requirements)
        validator.validate_usage(self.usage)

    def test_worker_cannot_be_container_artifact(self):
        self.rejected(lambda p: p['runtimes'][0].update(artifact='linux/amd64'))

    def test_container_requires_verified_architecture(self):
        self.rejected(lambda p: p['runtimes'][1].update(artifact='linux/arm64'))

    def test_profiles_are_distinct(self):
        self.rejected(lambda p: p['runtimes'].__setitem__(1, copy.deepcopy(p['runtimes'][0])))

    def test_native_client_not_assumed_in_workers(self):
        self.rejected(lambda p: p['storage'].update(worker_client='@libsql/client'))

    def test_turso_engine_is_not_libsql(self):
        self.rejected(lambda p: p['storage'].update(engine_family='turso'))

    def test_literal_database_secret_rejected(self):
        self.rejected(lambda p: p['storage'].update(credential={'value':'illustrative_not_a_real_secret'}))

    def test_token_and_url_have_distinct_references(self):
        self.rejected(lambda p: p['storage'].update(endpoint=copy.deepcopy(p['storage']['credential'])))

    def test_caller_cannot_choose_endpoint(self):
        self.rejected(lambda p: p['security'].update(caller_selects_database_endpoint=True))

    def test_no_browser_database_token(self):
        self.rejected(lambda p: p['security'].update(public_sdk_contains_database_token=True))

    def test_parameterized_sql_is_required(self):
        self.rejected(lambda p: p['security'].update(parameterized_sql=False))

    def test_bearer_not_network_acl(self):
        self.rejected(lambda p: p['security'].update(network_exclusivity='workers_only'))

    def test_no_direct_application_calls(self):
        self.rejected(lambda p: p['buffer'].update(communication='direct_handler_rpc'))

    def test_wait_until_not_durable(self):
        self.rejected(lambda p: p['buffer'].update(wait_until_is_durable=True))

    def test_memory_not_durable(self):
        self.rejected(lambda p: p['buffer'].update(memory_is_durable=True))

    def test_acceptance_requires_all_records(self):
        self.rejected(lambda p: p['buffer'].update(acceptance_records=['command','operation','other']))

    def test_commit_requires_all_records(self):
        self.rejected(lambda p: p['buffer'].update(commit_records=['state','inbox','operation','other']))

    def test_full_tenant_scope(self):
        self.rejected(lambda p: p['security'].update(tenant_scope=['business_id','project_id','other']))

    def test_projection_cannot_reopen_origin(self):
        self.rejected(lambda p: p['buffer'].update(projection_reopens_origin_operation=True))

    def test_unknown_commit_requires_reconciliation(self):
        self.rejected(lambda p: p['buffer'].update(unknown_commit='retry_blindly'))

    def test_live_gates_cannot_be_removed(self):
        self.rejected(lambda p: p['activation']['requires'].remove('live_remote_transaction_tests'))

    def test_no_early_activation_or_certification(self):
        for field in ('implemented','enabled'):
            with self.subTest(field=field):
                p=copy.deepcopy(self.plan);p[field]=True
                with self.assertRaises((ValueError, ValidationError, SchemaError)): validator.validate_plan(p,self.schema)
        self.rejected(lambda p: p['storage']['certified_capabilities'].append('atomic_ingress'))

    def test_browser_memory_cannot_be_server_authority(self):
        self.matrix['modes'][2]['authoritative_server_buffer']=True
        with self.assertRaises(ValueError): validator.validate_modes(self.matrix)

    def test_no_unsupported_indexeddb_claim(self):
        self.matrix['modes'][4]['documentation_status']='documented'
        with self.assertRaises(ValueError): validator.validate_modes(self.matrix)

    def test_browser_wasm_not_worker_wasm(self):
        self.matrix['modes'][2]['runtime']='workers'
        with self.assertRaises(ValueError): validator.validate_modes(self.matrix)

    def test_local_sync_is_not_server_acceptance(self):
        self.matrix['modes'][8]['local_success_is_server_acceptance']=True
        with self.assertRaises(ValueError): validator.validate_modes(self.matrix)

    def test_nonsync_local_commit_is_not_server_acceptance(self):
        for mode in self.matrix['modes']:
            with self.subTest(mode=mode['id']):
                matrix = copy.deepcopy(self.matrix)
                next(m for m in matrix['modes'] if m['id'] == mode['id'])['local_success_is_server_acceptance'] = True
                with self.assertRaises(ValueError): validator.validate_modes(matrix)

    def test_hybrid_profile_cannot_be_omitted(self):
        self.matrix['modes'] = [m for m in self.matrix['modes'] if m['id'] != 'browser_hybrid_framework_api_future']
        with self.assertRaises(ValueError): validator.validate_modes(self.matrix)

    def test_rows_are_not_requests_or_bytes(self):
        for field in ('query_count_equals_rows_read', 'row_count_equals_payload_bytes', 'unknown_usage_is_zero'):
            with self.subTest(field=field):
                usage=copy.deepcopy(self.usage);usage['provider_profile'][field]=True
                with self.assertRaises(ValueError): validator.validate_usage(usage)

    def test_plan_not_core_constant_or_account_entitlement(self):
        for field in ('core_constants', 'billing_account_verified', 'automatic_paid_overage_authorized'):
            with self.subTest(field=field):
                usage=copy.deepcopy(self.usage);usage['provider_profile'][field]=True
                with self.assertRaises(ValueError): validator.validate_usage(usage)

    def test_provider_allowance_requires_new_dated_evidence(self):
        self.usage['provider_profile']['allowances']['storage']['amount']=50
        with self.assertRaises(ValueError): validator.validate_usage(self.usage)

    def test_blob_experiment_cannot_claim_activation_or_free_bypass(self):
        for field in ('enabled','implemented','meter_bypass_claim','unknown_limit_means_unlimited'):
            with self.subTest(field=field):
                usage=copy.deepcopy(self.usage);usage['entity_blob_experiment'][field]=True
                with self.assertRaises(ValueError): validator.validate_usage(usage)

    def test_blob_requires_durable_log_and_snapshot_checkpoint(self):
        for field in ('durable_event_log_before_batch','checkpoint_after_snapshot_commit_only','idempotent_command_ids'):
            with self.subTest(field=field):
                usage=copy.deepcopy(self.usage);usage['entity_blob_experiment'][field]=False
                with self.assertRaises(ValueError): validator.validate_usage(usage)

    def test_blob_requires_remote_fencing(self):
        self.usage['entity_blob_experiment']['single_writer']='one_javascript_thread'
        with self.assertRaises(ValueError): validator.validate_usage(self.usage)

    def test_blob_cannot_guess_hosted_limits(self):
        self.usage['entity_blob_experiment']['hosted_limits']['max_blob_bytes']=1000000000
        with self.assertRaises(ValueError): validator.validate_usage(self.usage)

    def test_blob_cannot_merge_durable_and_applied_ack(self):
        e=self.usage['entity_blob_experiment'];e['application_ack']=e['acceptance_ack']
        with self.assertRaises(ValueError): validator.validate_usage(self.usage)

    def test_blob_gates_cannot_be_removed(self):
        self.usage['entity_blob_experiment']['activation_requires'].remove('unknown_commit_tests')
        with self.assertRaises(ValueError): validator.validate_usage(self.usage)

    def test_acceptance_traceability(self):
        self.scenarios['scenarios'][0]['requirements']=['REQ-999']
        with self.assertRaises(ValueError): validator.validate_scenarios(self.scenarios,self.requirements)

    def test_scenarios_cannot_claim_live_execution(self):
        self.scenarios['execution']='live_passed'
        with self.assertRaises(ValueError): validator.validate_scenarios(self.scenarios,self.requirements)


if __name__ == '__main__':
    unittest.main()
