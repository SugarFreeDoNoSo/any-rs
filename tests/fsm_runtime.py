#!/usr/bin/env python3
"""Black-box deny-default FSM lifecycle checks; fixture-only local HTTP/SQLite."""
import json
import sqlite3
import unittest
from runtime_http import Server, SCOPE, command, validate_contract

class FsmRuntime(unittest.TestCase):
    def setUp(self): self.server=Server().start()
    def tearDown(self): self.server.close()
    def counts(self):
        with sqlite3.connect(self.server.db) as db:
            return {table:db.execute('SELECT COUNT(*) FROM '+table).fetchone()[0]
                    for table in ('commands','objects','inbox','outbox')}
    def assert_no_effects(self):
        self.assertEqual(self.counts(),dict.fromkeys(('commands','objects','inbox','outbox'),0))
    def test_undeclared_events_are_denied_before_acceptance(self):
        for name in ('demo.note.update','demo.note.delete','demo.other.create','demo.note.created','unknown'):
            with self.subTest(name=name):
                value=command();value['name']=name
                status,problem,_=self.server.submit(value,key=name)
                self.assertEqual(status,422,problem)
                validate_contract(problem,'Problem'); self.assert_no_effects()
    def test_guard_failures_have_no_durable_effects(self):
        for text in ('','x'*4097,'é'*2049):
            with self.subTest(bytes=len(text.encode())):
                status,problem,_=self.server.submit(command(text=text))
                self.assertEqual(status,422,problem);self.assert_no_effects()
        for id in ('../escape','a/b',''):
            value=command();value['payload']['id']=id
            self.assertEqual(self.server.submit(value)[0],422);self.assert_no_effects()
    def test_declared_transition_creates_one_object_and_event(self):
        status,op,_=self.server.submit(command(text='one transition'))
        self.assertEqual(status,202)
        terminal=self.server.terminal(op['operation_url'])
        self.assertEqual(terminal['status'],'succeeded');validate_contract(terminal,'Operation')
        self.assertEqual(self.counts(),dict.fromkeys(('commands','objects','inbox','outbox'),1))
        status,obj,_=self.server.request('GET',SCOPE+'/demo.note/objects/n1')
        self.assertEqual(status,200);self.assertEqual(obj['values'],{'text':'one transition'})
        with sqlite3.connect(self.server.db) as db:
            event=json.loads(db.execute('SELECT body FROM outbox').fetchone()[0])
        self.assertEqual(event['name'],'demo.note.created')
        self.assertEqual(event['operation_id'],op['operation_id'])
        self.assertEqual(event['payload'],obj)
    def test_terminal_state_denies_second_create_without_domain_effect(self):
        first=self.server.submit(command(text='immutable first'))[1]
        self.assertEqual(self.server.terminal(first['operation_url'])['status'],'succeeded')
        before=self.server.request('GET',SCOPE+'/demo.note/objects/n1')[1]
        with sqlite3.connect(self.server.db) as db:
            event_before=db.execute('SELECT body FROM outbox').fetchone()[0]
        status,second,_=self.server.submit(command(text='forbidden overwrite'),key='second-command')
        self.assertEqual(status,202)
        rejected=self.server.terminal(second['operation_url'])
        self.assertEqual(rejected['status'],'rejected');self.assertEqual(rejected['problem']['status'],409)
        self.assertEqual(self.server.request('GET',SCOPE+'/demo.note/objects/n1')[1],before)
        # Rejection auditing is permitted: its command + inbox are durable, no domain mutation/event.
        self.assertEqual(self.counts(),{'commands':2,'objects':1,'inbox':2,'outbox':1})
        with sqlite3.connect(self.server.db) as db:
            self.assertEqual(db.execute('SELECT body FROM outbox').fetchone()[0],event_before)

if __name__=='__main__': unittest.main(verbosity=2)
