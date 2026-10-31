#!/usr/bin/env python3
"""Exercise the actual Rust Lambda Runtime SDK against a loopback HTTP mock.
No AWS services, DynamoDB or queue transport are emulated or contacted.
Build: cargo build -p any-rs-lambda --example runtime_api_probe
"""
import json
import os
from pathlib import Path
import queue
import sqlite3
import subprocess
import tempfile
import threading
import time
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from lambda_events import ROOT, SCOPE, TOKENS, command, http, record

BINARY = ROOT / 'target/debug/examples/runtime_api_probe'

class RuntimeAPI(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='anyrs-runtime-api-')
        self.root = Path(self.temp.name)
        self.db = self.root/'fixture.sqlite'
        self.config = self.root/'principals.json'
        self.config.write_text(json.dumps([{'id': 'alice', 'token_env': 'ANYRS_ALICE_TOKEN',
            'business_id': 'b', 'project_id': 'p', 'client_id': 'c'}]))
    def tearDown(self): self.temp.cleanup()
    def environment(self):
        env = os.environ.copy()
        env.pop('AWS_LAMBDA_FUNCTION_NAME', None)
        env['ANYRS_ALICE_TOKEN'] = TOKENS['alice']
        return env
    def invoke(self, mode, payload):
        response = queue.Queue()
        stop = threading.Event()
        request_id = 'local-probe-invocation'
        delivered = threading.Event()
        paths = []
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_): pass
            def do_GET(self):
                paths.append(self.path)
                if self.path != '/2018-06-01/runtime/invocation/next':
                    self.send_error(404); return
                if delivered.is_set():
                    stop.wait(15)
                    self.send_response(503); self.end_headers(); return
                delivered.set()
                raw = json.dumps(payload).encode()
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(raw)))
                self.send_header('Lambda-Runtime-Aws-Request-Id', request_id)
                self.send_header('Lambda-Runtime-Deadline-Ms', str(int(time.time()*1000)+30000))
                self.send_header('Lambda-Runtime-Invoked-Function-Arn',
                    'arn:aws:lambda:us-east-1:000000000000:function:local-fixture')
                self.end_headers(); self.wfile.write(raw)
            def do_POST(self):
                paths.append(self.path)
                raw = self.rfile.read(int(self.headers.get('Content-Length', '0')))
                response.put((self.path, raw))
                self.send_response(202); self.send_header('Content-Length', '0'); self.end_headers()
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        env = self.environment(); env['AWS_LAMBDA_RUNTIME_API'] = f'127.0.0.1:{server.server_port}'
        proc = subprocess.Popen([str(BINARY), '--database', str(self.db), '--principals', str(self.config),
            '--mode', mode], stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
        outcome = None
        try:
            try: outcome = response.get(timeout=10)
            except queue.Empty: pass
        finally:
            proc.terminate()
            try: stdout, stderr = proc.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill(); stdout, stderr = proc.communicate()
            stop.set(); server.shutdown(); server.server_close(); thread.join(timeout=2)
        self.assertIsNotNone(outcome, 'Runtime SDK did not post a response: '+stderr.decode())
        self.assertEqual(outcome[0], f'/2018-06-01/runtime/invocation/{request_id}/response',
            (outcome, stderr.decode()))
        self.assertIn('/2018-06-01/runtime/invocation/next', paths)
        return json.loads(outcome[1])
    def test_sdk_http_sqs_and_cold_read(self):
        accepted = self.invoke('http', http('POST', SCOPE+'/commands', command(), key='runtime-api-key'))
        self.assertEqual(accepted['statusCode'], 202)
        operation = json.loads(accepted['body'])
        with sqlite3.connect(self.db) as db:
            self.assertEqual(db.execute('SELECT status FROM commands').fetchone()[0], 'accepted')
            self.assertEqual(db.execute('SELECT COUNT(*) FROM objects').fetchone()[0], 0)
        self.assertEqual(self.invoke('sqs', {'Records': [record(operation['operation_id'])]}),
            {'batchItemFailures': []})
        completed = self.invoke('http', http(path=operation['operation_url']))
        self.assertEqual(completed['statusCode'], 200)
        self.assertEqual(json.loads(completed['body'])['status'], 'succeeded')
        self.assertEqual(self.invoke('sqs', {'Records': [record(operation['operation_id'])]}),
            {'batchItemFailures': []})
        with sqlite3.connect(self.db) as db:
            self.assertEqual(db.execute('SELECT COUNT(*) FROM outbox').fetchone()[0], 1)
            self.assertEqual(db.execute('SELECT COUNT(*) FROM objects').fetchone()[0], 1)
    def test_probe_rejects_real_lambda_and_nonlocal_endpoints_before_db(self):
        for endpoint, function_name in [('169.254.170.2:9001', None), ('localhost:9001', None),
                ('127.0.0.1:9001', 'production-function')]:
            env = self.environment(); env['AWS_LAMBDA_RUNTIME_API'] = endpoint
            if function_name: env['AWS_LAMBDA_FUNCTION_NAME'] = function_name
            rejected_db = self.root/'must-not-exist.sqlite'
            result = subprocess.run([str(BINARY), '--database', str(rejected_db), '--principals',
                str(self.config), '--mode', 'http'], capture_output=True, env=env, timeout=5)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(rejected_db.exists())

if __name__ == '__main__': unittest.main(verbosity=2)
