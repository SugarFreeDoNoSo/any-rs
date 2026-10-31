#!/usr/bin/env python3
"""Native fixture/guard tests; NOT SAM or Docker execution."""
import json,os,subprocess,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
BIN=ROOT/'target/debug/any-rs-sam-fixture'
class SamFixture(unittest.TestCase):
    def clean(self):
        env=os.environ.copy()
        for k in ['AWS_SAM_LOCAL','ANYRS_SAM_FIXTURE_ONLY','AWS_LAMBDA_FUNCTION_NAME','AWS_LAMBDA_RUNTIME_API']:
            env.pop(k,None)
        return env
    def test_native_single_invocation_scenario(self):
        p=subprocess.run([str(BIN),'--scenario-local'],env=self.clean(),capture_output=True,text=True,timeout=10)
        self.assertEqual(p.returncode,0,p.stderr)
        r=json.loads(p.stdout)
        self.assertEqual(r['status'],'passed')
        self.assertFalse(r['aws_services_contacted'])
        self.assertFalse(r['cross_container_durability_tested'])
        self.assertEqual(r['object']['values']['title'],'synthetic SAM task')
        self.assertEqual(r['object']['state'],'completed')
        self.assertEqual(r['object']['revision'],3)
    def test_fixture_refuses_accidental_cloud_runtime(self):
        for values in [{},{'AWS_LAMBDA_FUNCTION_NAME':'production'}, {'AWS_SAM_LOCAL':'true'}, {'ANYRS_SAM_FIXTURE_ONLY':'true'}]:
            env=self.clean();env.update(values)
            p=subprocess.run([str(BIN)],env=env,capture_output=True,text=True,timeout=5)
            self.assertNotEqual(p.returncode,0)
            self.assertIn('SAM fixture refuses',p.stderr)
if __name__=='__main__':unittest.main(verbosity=2)
