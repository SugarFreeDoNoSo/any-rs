#!/usr/bin/env python3
"""Real HTTP integration for the three-state task FSM; fixtures only."""
import concurrent.futures
import json
import sqlite3
import time
import unittest
from runtime_http import Server,SCOPE,command

def task(name='tasks.create',id='t1',**payload):
    return {'name':name,'version':1,'payload':{'id':id,**payload}}

class TasksRuntime(unittest.TestCase):
    def setUp(self): self.s=Server().start()
    def tearDown(self): self.s.close()
    def submit(self,value,key='k1',token='alice',scope=SCOPE):
        return self.s.request('POST',scope+'/tasks/commands',value,key=key,token=token)
    def run_command(self,value,key='k1'):
        code,op,_=self.submit(value,key);self.assertEqual(code,202,op)
        return self.s.terminal(op['operation_url'])
    def get(self):
        code,obj,_=self.s.request('GET',SCOPE+'/tasks/objects/t1');self.assertEqual(code,200,obj);return obj
    def counts(self):
        with sqlite3.connect(self.s.db) as db:
            return {t:db.execute('SELECT COUNT(*) FROM task_'+t).fetchone()[0]
                    for t in ('commands','objects','inbox','outbox')}
    def create(self):
        result=self.run_command(task(title='Fixture task'));self.assertEqual(result['status'],'succeeded');return self.get()
    def test_full_lifecycle_and_terminal_rejection(self):
        obj=self.create();self.assertEqual((obj['state'],obj['revision']),('pending',1))
        self.assertEqual(obj['type'],'tasks.task');self.assertEqual(obj['values'],{'title':'Fixture task'})
        for name,state,rev in [('tasks.start','in_progress',2),('tasks.complete','completed',3)]:
            result=self.run_command(task(name,expected_revision=rev-1),name)
            self.assertEqual(result['status'],'succeeded',result)
            self.assertEqual((self.get()['state'],self.get()['revision']),(state,rev))
        before=self.get()
        result=self.run_command(task('tasks.start',expected_revision=3),'terminal')
        self.assertEqual(result['status'],'rejected');self.assertEqual(result['problem']['status'],409)
        self.assertEqual(self.get(),before)
        self.assertEqual(self.counts(),{'commands':4,'objects':1,'inbox':4,'outbox':3})
    def test_deny_default_and_guards_before_acceptance(self):
        for value in (task('tasks.delete'),task(title=''),task('tasks.start',expected_revision=0),
                      task('tasks.start'),task('arbitrary',title='x')):
            code,body,_=self.submit(value);self.assertEqual(code,422,body)
            self.assertEqual(self.counts(),dict.fromkeys(('commands','objects','inbox','outbox'),0))
    def test_invalid_transition_stale_and_missing(self):
        missing=self.run_command(task('tasks.start',expected_revision=1),'missing')
        self.assertEqual(missing['problem']['status'],404)
        self.create();before=self.get()
        denied=self.run_command(task('tasks.complete',expected_revision=1),'skip')
        self.assertEqual(denied['problem']['status'],409);self.assertEqual(self.get(),before)
        self.assertEqual(self.run_command(task('tasks.start',expected_revision=1),'start')['status'],'succeeded')
        before=self.get()
        stale=self.run_command(task('tasks.complete',expected_revision=1),'stale')
        self.assertEqual(stale['problem']['status'],409);self.assertEqual(self.get(),before)
        self.assertEqual(self.counts()['outbox'],2)
    def test_idempotency_and_concurrent_revision(self):
        self.create()
        code,replayed,_=self.submit(task(title='Fixture task'));self.assertEqual(code,202)
        self.assertEqual(self.counts()['commands'],1)
        self.assertEqual(self.s.terminal(replayed['operation_url'])['status'],'succeeded')
        code,_,_=self.submit(task(title='different'));self.assertEqual(code,409)
        def start(i):return self.submit(task('tasks.start',expected_revision=1),'start-'+str(i))
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            responses=list(pool.map(start,range(2)))
        operations=[]
        for code,op,_ in responses:
            self.assertEqual(code,202);operations.append(self.s.terminal(op['operation_url']))
        self.assertEqual(sorted(op['status'] for op in operations),['rejected','succeeded'])
        self.assertEqual(self.get()['revision'],2);self.assertEqual(self.counts()['outbox'],2)
        for op in operations:
            if op['status']=='rejected':self.assertEqual(op['problem']['status'],409)
    def test_authorization_and_note_compatibility(self):
        self.create()
        for token,expected in ((None,401),('bad-token',401),('bob',403),('charlie',200)):
            code,_,_=self.s.request('GET',SCOPE+'/tasks/objects/t1',token=token)
            self.assertEqual(code,expected)
        _,op,_=self.submit(task('tasks.start',expected_revision=1),'private')
        self.assertEqual(self.s.request('GET',op['operation_url'],token='charlie')[0],404)
        code,note,_=self.s.submit(command());self.assertEqual(code,202)
        self.assertEqual(self.s.terminal(note['operation_url'])['status'],'succeeded')
    def test_outbox_failure_rolls_back_all_task_mutations(self):
        self.s.stop();self.s.start(worker=False)
        code,op,_=self.submit(task(title='atomic'));self.assertEqual(code,202)
        with sqlite3.connect(self.s.db) as db:
            db.execute("CREATE TRIGGER task_fixture_failure BEFORE INSERT ON task_outbox BEGIN SELECT RAISE(ABORT,'fixture'); END")
        self.s.stop(kill=True);self.s.start()
        time.sleep(.25)
        self.assertEqual(self.counts(),{'commands':1,'objects':0,'inbox':0,'outbox':0})
        self.assertEqual(self.s.request('GET',op['operation_url'])[1]['status'],'accepted')
        with sqlite3.connect(self.s.db) as db:db.execute('DROP TRIGGER task_fixture_failure')
        self.assertEqual(self.s.terminal(op['operation_url'])['status'],'succeeded')
        self.assertEqual(self.counts(),dict.fromkeys(('commands','objects','inbox','outbox'),1))

if __name__=='__main__': unittest.main(verbosity=2)
