#!/usr/bin/env python3
"""Offline API Gateway v2 / SQS adapter contract tests; no AWS network."""
import base64
import json
import os
from pathlib import Path
import select
import sqlite3
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target/debug/any-rs-lambda-fixture'
if '--binary' in sys.argv:
    i = sys.argv.index('--binary')
    BINARY = Path(sys.argv[i+1]).resolve()
    del sys.argv[i:i+2]
SCOPE = '/v2/businesses/b/projects/p/clients/c'
TOKENS = {n: f'fixture-only-{n}-token-123456' for n in ('alice', 'bob', 'charlie')}

def command(id='n1', text='hello'):
    return {'name': 'demo.note.create', 'version': 1, 'payload': {'id': id, 'text': text}}

def http(method='GET', path='/version', body=None, principal='alice', key=None):
    headers = {'content-type': 'application/json'}
    if principal:
        headers['authorization'] = 'Bearer ' + TOKENS[principal]
    if key:
        headers['idempotency-key'] = key
    return {'version': '2.0', 'routeKey': '$default', 'rawPath': path, 'rawQueryString': '',
            'headers': headers, 'requestContext': {'accountId': '123456789012', 'apiId': 'fixture',
            'domainName': 'fixture.invalid', 'domainPrefix': 'fixture', 'http': {'method': method,
            'path': path, 'protocol': 'HTTP/1.1', 'sourceIp': '127.0.0.1', 'userAgent': 'fixture'},
            'requestId': 'fixture-request', 'routeKey': '$default', 'stage': '$default',
            'time': '31/Oct/2026:00:00:00 +0000', 'timeEpoch': 1793404800000},
            'body': json.dumps(body) if body is not None else None, 'isBase64Encoded': False}

def record(op, id='msg1', principal='alice', client='c'):
    body = {'operation_id': op, 'principal_id': principal,
            'scope': {'business_id': 'b', 'project_id': 'p', 'client_id': client}}
    return {'messageId': id, 'receiptHandle': 'fixture', 'body': json.dumps(body),
            'attributes': {'ApproximateReceiveCount': '1', 'SentTimestamp': '1793404800000',
            'SenderId': 'fixture', 'ApproximateFirstReceiveTimestamp': '1793404800000'},
            'messageAttributes': {}, 'md5OfBody': '00000000000000000000000000000000',
            'eventSource': 'aws:sqs', 'eventSourceARN': 'arn:aws:sqs:us-east-1:000000000000:any-rs-fixture',
            'awsRegion': 'us-east-1'}

class Fixture:
    def __init__(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='any-rs-lambda-')
        self.root = Path(self.tmp.name)
        self.db = self.root/'state.sqlite'
        self.config = self.root/'principals.json'
        self.proc = None
        self.log = (self.root/'stderr.log').open('ab')
        self.configure()
        self.start()
    def configure(self, names=('alice','bob','charlie')):
        self.config.write_text(json.dumps([{'id': n, 'token_env': 'ANYRS_'+n.upper()+'_TOKEN',
            'business_id': 'b', 'project_id': 'p', 'client_id': 'other' if n == 'bob' else 'c'} for n in names]))
    def start(self):
        env = os.environ.copy()
        env.pop('AWS_LAMBDA_FUNCTION_NAME', None)
        env.pop('AWS_LAMBDA_RUNTIME_API', None)
        env.update({'ANYRS_'+n.upper()+'_TOKEN': t for n,t in TOKENS.items()})
        self.proc = subprocess.Popen([str(BINARY), '--database', str(self.db), '--principals', str(self.config)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log, text=True, env=env)
    def stop(self):
        if self.proc:
            self.proc.stdin.close()
            try: self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill(); self.proc.wait()
            self.proc.stdout.close()
            self.proc = None
    def invoke(self, kind, event):
        self.proc.stdin.write(json.dumps({'kind': kind, 'event': event})+'\n'); self.proc.stdin.flush()
        if not select.select([self.proc.stdout], [], [], 5)[0]:
            raise AssertionError('fixture invocation timeout; '+(self.root/'stderr.log').read_text())
        line = self.proc.stdout.readline()
        if not line:
            raise AssertionError('fixture exited: '+(self.root/'stderr.log').read_text())
        return json.loads(line)
    def request(self, event):
        result = self.invoke('http', event)
        assert result['isBase64Encoded'] is False, result
        return result['statusCode'], json.loads(result['body'])
    def accept(self, id='n1', key='key1', principal='alice', path=SCOPE):
        status, op = self.request(http('POST', path+'/commands', command(id), principal, key))
        assert status == 202, (status,op)
        return op
    def sqs(self, *records): return self.invoke('sqs', {'Records': list(records)})
    def counts(self):
        with sqlite3.connect(self.db) as db:
            return {t: db.execute('SELECT COUNT(*) FROM '+t).fetchone()[0]
                    for t in ('commands','objects','inbox','outbox')}
    def close(self): self.stop(); self.log.close(); self.tmp.cleanup()

class LambdaEvents(unittest.TestCase):
    def setUp(self): self.f = Fixture()
    def tearDown(self): self.f.close()
    def test_warm_cold_and_exact_id_dispatch(self):
        a = self.f.accept(); b = self.f.accept('n2','key2')
        self.assertEqual(self.f.request(http(path=a['operation_url']))[1]['status'], 'accepted')
        self.assertEqual(self.f.counts()['objects'],0)
        self.f.stop(); self.f.start()
        self.assertEqual(self.f.sqs(record(a['operation_id'])), {'batchItemFailures': []})
        self.assertEqual(self.f.request(http(path=a['operation_url']))[1]['status'],'succeeded')
        self.assertEqual(self.f.request(http(path=b['operation_url']))[1]['status'],'accepted')
        self.assertEqual(self.f.sqs(record(a['operation_id'],'again')), {'batchItemFailures': []})
        self.assertEqual(self.f.counts(), {'commands':2,'objects':1,'inbox':1,'outbox':1})
        self.f.stop(); self.f.start()
        self.assertEqual(self.f.request(http(path=SCOPE+'/demo.note/objects/n1'))[1]['values'], {'text':'hello'})
    def test_partial_batch_and_identity_mismatch(self):
        a = self.f.accept()
        bad = record(a['operation_id'],'malformed'); bad['body']='{'
        wrong = record(a['operation_id'],'wrong', client='other')
        absent = record('not-an-operation','absent')
        response=self.f.sqs(bad,wrong,absent)
        self.assertEqual({x['itemIdentifier'] for x in response['batchItemFailures']}, {'malformed','wrong','absent'})
        self.assertEqual(self.f.counts()['objects'],0)
        response=self.f.sqs(bad,record(a['operation_id'],'good'))
        self.assertEqual(response, {'batchItemFailures':[{'itemIdentifier':'malformed'}]})
        self.assertEqual(self.f.counts()['objects'],1)
    def test_auth_scope_and_private_operation(self):
        a=self.f.accept()
        for principal,code in ((None,401),('bob',403),('charlie',404)):
            self.assertEqual(self.f.request(http(path=a['operation_url'],principal=principal))[0],code)
        event=http(); event['headers']['authorization']='Bearer incorrect-fixture-token'
        self.assertEqual(self.f.request(event)[0],200) # public version remains public
        event=http(path=a['operation_url']); event['headers']['authorization']='Bearer invalid-token'
        self.assertEqual(self.f.request(event)[0],401)
        self.assertEqual(self.f.sqs(record(a['operation_id'],'mismatch','charlie')),
                         {'batchItemFailures':[{'itemIdentifier':'mismatch'}]})
    def test_tenant_and_principal_idempotency_namespaces(self):
        a=self.f.accept()
        c=self.f.accept(principal='charlie')
        b=self.f.accept(principal='bob',path='/v2/businesses/b/projects/p/clients/other')
        self.assertEqual(len({a['operation_id'],b['operation_id'],c['operation_id']}),3)
        self.assertEqual(self.f.sqs(record(a['operation_id'],'a'),
            record(b['operation_id'],'b','bob','other')), {'batchItemFailures':[]})
        self.assertEqual(self.f.counts()['objects'],2)
        self.assertEqual(self.f.sqs(record(c['operation_id'],'c','charlie')), {'batchItemFailures':[]})
        self.assertEqual(self.f.request(http(path=c['operation_url'],principal='charlie'))[1]['status'],'rejected')
        self.assertEqual(self.f.counts()['objects'],2)

    def test_idempotency_replay_and_conflict(self):
        a=self.f.accept()
        event=http('POST',SCOPE+'/commands',command(),key='key1')
        event['body']='{"payload":{"text":"hello","id":"n1"},"version":1,"name":"demo.note.create"}'
        status,b=self.f.request(event)
        self.assertEqual(status,202); self.assertEqual(a['operation_id'],b['operation_id'])
        event['body']=json.dumps(command(text='changed'))
        self.assertEqual(self.f.request(event)[0],409)
    def test_base64_and_case_insensitive_headers(self):
        event=http('POST',SCOPE+'/commands',command(),key='key1')
        event['body']=base64.b64encode(event['body'].encode()).decode(); event['isBase64Encoded']=True
        event['headers']={k.title():v for k,v in event['headers'].items()}
        self.assertEqual(self.f.request(event)[0],202)
    def test_invalid_inputs_and_header_ambiguity(self):
        for body,expected in (('{',400),(json.dumps(command(text='x'*17000)),413)):
            event=http('POST',SCOPE+'/commands',key='key1'); event['body']=body
            self.assertEqual(self.f.request(event)[0],expected)
        event=http('POST',SCOPE+'/commands',command(),key='key1')
        event['headers']['authorization'] += ', Bearer '+TOKENS['charlie']
        self.assertIn(self.f.request(event)[0],(400,401))
        event=http('POST',SCOPE+'/commands',command(),key='key1,key2')
        self.assertIn(self.f.request(event)[0],(400,422))
        self.assertEqual(self.f.counts()['commands'],0)
    def test_invalid_envelope_recovers(self):
        event=http(); event['version']='1.0'
        self.assertEqual(self.f.request(event)[0],400)
        self.assertEqual(self.f.invoke('sqs',{'Records':[{'body':'{}'}]}), {'error':'invalid_event'})
        self.assertEqual(self.f.request(http())[0],200)
    def test_invalid_base64_and_missing_method(self):
        event=http('POST',SCOPE+'/commands',command(),key='k')
        event['isBase64Encoded']=True; event['body']='not valid base64!'
        self.assertEqual(self.f.request(event)[0],400)
        event=http(); del event['requestContext']['http']['method']
        self.assertEqual(self.f.request(event)[0],400)
        event=http('POST',SCOPE+'/commands',command(),key='k')
        event['headers']['Authorization']=event['headers']['authorization']
        self.assertEqual(self.f.request(event)[0],400)
        self.assertEqual(self.f.counts()['commands'],0)
    def test_unreportable_batch_cannot_partially_execute(self):
        a=self.f.accept()
        bad=record(a['operation_id']); del bad['messageId']
        self.assertEqual(self.f.sqs(record(a['operation_id'],'good'),bad), {'error':'invalid_event'})
        self.assertEqual(self.f.counts()['objects'],0)

    def test_revoked_principal_is_durable_rejection(self):
        a=self.f.accept(); self.f.stop(); self.f.configure(('bob',)); self.f.start()
        self.assertEqual(self.f.sqs(record(a['operation_id'])), {'batchItemFailures':[]})
        with sqlite3.connect(self.f.db) as db:
            status=db.execute('SELECT status FROM commands').fetchone()[0]
        self.assertEqual(status,'rejected'); self.assertEqual(self.f.counts()['objects'],0)
        self.assertEqual(self.f.sqs(record(a['operation_id'],'again')), {'batchItemFailures':[]})
    def test_transient_failure_rolls_back_and_retries(self):
        a=self.f.accept()
        with sqlite3.connect(self.f.db) as db:
            db.execute("CREATE TRIGGER fixture_failure BEFORE INSERT ON outbox BEGIN SELECT RAISE(ABORT,'fixture'); END")
        self.assertEqual(self.f.sqs(record(a['operation_id'])), {'batchItemFailures':[{'itemIdentifier':'msg1'}]})
        self.assertEqual(self.f.counts(), {'commands':1,'objects':0,'inbox':0,'outbox':0})
        self.assertEqual(self.f.request(http(path=a['operation_url']))[1]['status'],'accepted')
        with sqlite3.connect(self.f.db) as db: db.execute('DROP TRIGGER fixture_failure')
        self.assertEqual(self.f.sqs(record(a['operation_id'])), {'batchItemFailures':[]})
        self.assertEqual(self.f.counts(), {'commands':1,'objects':1,'inbox':1,'outbox':1})
    def test_untrusted_queue_and_path_mismatch(self):
        a=self.f.accept()
        for field,value in (('eventSourceARN','arn:aws:sqs:us-east-1:000000000000:other'),
                            ('eventSourceARN','arn:aws:sqs:us-east-1:000000000000:any-rs-fixture.fifo'),
                            ('eventSource','aws:sns'),('awsRegion','us-west-2')):
            r=record(a['operation_id']); r[field]=value
            self.assertEqual(self.f.sqs(r), {'batchItemFailures':[{'itemIdentifier':'msg1'}]}, (field,value))
        self.assertEqual(self.f.counts()['objects'],0)
        event=http(); event['requestContext']['http']['path']='/another-path'
        self.assertEqual(self.f.request(event)[0],400)

    def test_refuses_aws_environment(self):
        env=os.environ.copy(); env['AWS_LAMBDA_FUNCTION_NAME']='fixture-production-guard'
        env.update({'ANYRS_'+n.upper()+'_TOKEN':t for n,t in TOKENS.items()})
        result=subprocess.run([str(BINARY),'--database',str(self.f.root/'forbidden.sqlite'),
            '--principals',str(self.f.config)],input='',capture_output=True,text=True,env=env,timeout=5)
        self.assertNotEqual(result.returncode,0)
        self.assertFalse((self.f.root/'forbidden.sqlite').exists())

if __name__ == '__main__': unittest.main(verbosity=2)
