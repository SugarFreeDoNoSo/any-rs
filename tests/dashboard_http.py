#!/usr/bin/env python3
"""Authenticated dashboard / schema projections over real local HTTP and Lambda events.
No cloud requests. Reuses existing HTTP fixture and its installed schema validator.
"""
import json
import unittest
from html.parser import HTMLParser
from pathlib import Path
import runtime_http as local
import lambda_events as events

class Tags(HTMLParser):
    def __init__(self):
        super().__init__(); self.tags=[]
    def handle_starttag(self,tag,attrs): self.tags.append((tag,dict(attrs)))

class DashboardHTTP(unittest.TestCase):
    def setUp(self): self.server=local.Server().start()
    def tearDown(self): self.server.close()
    def get(self,path,token='alice'): return self.server.request('GET',local.SCOPE+path,token=token)
    def test_authenticated_html_and_generated_markdown(self):
        status,body,headers=self.get('/dashboard')
        self.assertEqual(status,200); self.assertIsInstance(body,str)
        self.assertIn('text/html',headers['content-type'])
        self.assertIn('demo.note',body); self.assertIn('schema.md',body)
        self.assertIn('no-store',headers.get('cache-control',''))
        self.assertEqual(headers.get('x-content-type-options'),'nosniff')
        self.assertIn("default-src 'none'",headers.get('content-security-policy',''))
        parsed=Tags(); parsed.feed(body)
        self.assertTrue(any(t=='html' for t,a in parsed.tags))
        self.assertFalse(any(t in ('iframe','object') for t,a in parsed.tags))
        self.assertFalse(any(t=='script' and a.get('src','').startswith(('http:','https:','//')) for t,a in parsed.tags))
        status,md,headers=self.get('/schema.md')
        self.assertEqual(status,200); self.assertIsInstance(md,str)
        self.assertIn('text/',headers['content-type'])
        self.assertIn('```mermaid',md); self.assertIn('demo.note',md)
        self.assertIn('demo.note.create',md)
        self.assertIn('flowchart LR',md)
        for schema in ('CommandInput','Operation','ObjectDocument'):
            self.assertIn(schema,md)
        # These views are contracts, not live AWS telemetry or application data.
        for label in ('CloudWatch CPU','SQS queue depth','Lambda invocations:'):
            self.assertNotIn(label,md)
    def test_auth_and_tenant_scope(self):
        for path in ('/dashboard','/schema.md'):
            for token,expected in ((None,401),('invalid-fixture-token',401),('bob',403),('charlie',200)):
                with self.subTest(path=path,token=token): self.assertEqual(self.get(path,token)[0],expected)
            for scope in ('/v2/businesses/other/projects/p/clients/c',
                          '/v2/businesses/b/projects/other/clients/c',
                          '/v2/businesses/b/projects/p/clients/other'):
                self.assertEqual(self.server.request('GET',scope+path)[0],403)
    def test_note_payload_not_injected_into_views(self):
        attack='<script>alert("fixture-xss")</script><img src=x onerror=alert(1)>'
        code,op,_=self.server.submit(local.command(text=attack))
        self.assertEqual(code,202); self.assertEqual(self.server.terminal(op['operation_url'])['status'],'succeeded')
        for path in ('/dashboard','/schema.md'):
            status,body,_=self.get(path)
            self.assertEqual(status,200); self.assertNotIn(attack,body)
            self.assertNotIn('<script>alert(',body); self.assertNotIn('<img src=x',body)
    def test_views_do_not_expose_fixture_secrets(self):
        for path in ('/dashboard','/schema.md'):
            status,body,_=self.get(path)
            self.assertEqual(status,200)
            for token in local.TOKENS.values(): self.assertNotIn(token,body)
            self.assertNotIn(str(self.server.db),body)

@unittest.skipUnless(events.BINARY.exists(),'Lambda fixture binary not built')
class DashboardLambda(unittest.TestCase):
    def setUp(self): self.f=events.Fixture()
    def tearDown(self): self.f.close()
    def test_html_and_markdown_through_invocation_adapter(self):
        for path,marker in (('/dashboard','demo.note'),('/schema.md','```mermaid')):
            response=self.f.invoke('http',events.http(path=events.SCOPE+path))
            self.assertEqual(response['statusCode'],200)
            self.assertFalse(response['isBase64Encoded']); self.assertIn(marker,response['body'])
            for principal,expected in ((None,401),('bob',403),('charlie',200)):
                response=self.f.invoke('http',events.http(path=events.SCOPE+path,principal=principal))
                self.assertEqual(response['statusCode'],expected)

if __name__=='__main__': unittest.main(verbosity=2)
