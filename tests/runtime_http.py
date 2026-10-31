#!/usr/bin/env python3
"""Black-box tests for the local Rust/SQLite slice. Uses pinned validator jsonschema.

Run: python3 tests/runtime_http.py --binary target/debug/any-rs-local
Fixtures, credentials and databases are temporary and never contact cloud services.
"""
from __future__ import annotations

import concurrent.futures
import json
import os
from pathlib import Path
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.request

from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/debug/any-rs-local"
if "--binary" in sys.argv:
    i = sys.argv.index("--binary")
    BINARY = Path(sys.argv[i + 1]).resolve()
    del sys.argv[i:i + 2]

SCOPE = "/v2/businesses/b/projects/p/clients/c"
OTHER = "/v2/businesses/b/projects/p/clients/other"
CONTRACT = json.loads((ROOT / "spec/my-framework-spec-v0.1.0/schemas/contracts.schema.json").read_text())

def validate_contract(value, definition):
    schema = {"$ref": f"#/$defs/{definition}", "$defs": CONTRACT["$defs"]}
    Draft202012Validator(schema, format_checker=FormatChecker()).validate(value)

TOKENS = {"alice": "test-only-alice-credential", "bob": "test-only-bob-credential",
          "charlie": "test-only-charlie-credential"}


def command(note_id="n1", text="hello"):
    return {"name": "demo.note.create", "version": 1,
            "payload": {"id": note_id, "text": text}}


class Server:
    def __init__(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="any-rs-http-")
        self.root = Path(self.tmp.name)
        self.db = self.root / "state.sqlite"
        self.process = None
        self.log = None
        self.config = self.root / "runtime.toml"
        self.port = None

    def start(self, worker=True, database=None, principals=None):
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            self.port = sock.getsockname()[1]
        db = str(database if database is not None else self.db)
        self.config.write_text(
            f'bind = "127.0.0.1:{self.port}"\n'
            f'database = {json.dumps(db)}\npoll_ms = 20\n'
            + "\n".join(
                f'[[principals]]\nid = "{name}"\ntoken_env = "ANYRS_{name.upper()}_TOKEN"\n'
                f'business_id = "b"\nproject_id = "p"\nclient_id = "{client}"\n'
                for name, client in (principals if principals is not None else [("alice", "c"), ("bob", "other"), ("charlie", "c")])
            ), encoding="utf-8")
        env = os.environ.copy()
        env.update({f"ANYRS_{name.upper()}_TOKEN": token for name, token in TOKENS.items()})
        self.log = (self.root / "server.log").open("ab")
        args = [str(BINARY), "--config", str(self.config)]
        if not worker:
            args.append("--worker-disabled")
        self.process = subprocess.Popen(args, stdout=self.log, stderr=self.log, env=env)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                self.log.flush()
                raise AssertionError("server exited during startup: " +
                                     (self.root / "server.log").read_text(errors="replace"))
            try:
                if self.request("GET", "/version", token=None)[0] == 200:
                    return self
            except (urllib.error.URLError, OSError):
                pass
            time.sleep(.02)
        raise AssertionError("server did not become ready")

    def stop(self, kill=False):
        if self.process is not None:
            if self.process.poll() is None:
                (self.process.kill if kill else self.process.terminate)()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
            self.process = None
        if self.log is not None:
            self.log.close()
            self.log = None

    def close(self):
        self.stop()
        self.tmp.cleanup()

    def request(self, method, path, body=None, token="alice", key=None, raw=None):
        headers = {"Content-Type": "application/json"}
        if token is not None:
            headers["Authorization"] = "Bearer " + TOKENS.get(token, token)
        if key is not None:
            headers["Idempotency-Key"] = key
        data = raw if raw is not None else (json.dumps(body).encode() if body is not None else None)
        req = urllib.request.Request(f"http://127.0.0.1:{self.port}{path}",
                                     data=data, headers=headers, method=method)
        try:
            response = urllib.request.urlopen(req, timeout=5)
        except urllib.error.HTTPError as err:
            response = err
        with response:
            payload = response.read()
            try:
                result = json.loads(payload)
            except (ValueError, UnicodeDecodeError):
                result = payload.decode(errors="replace")
            return response.status, result, {k.lower(): v for k, v in response.headers.items()}

    def submit(self, value=None, key="k1", scope=SCOPE, token="alice"):
        return self.request("POST", scope + "/commands", command() if value is None else value,
                            token=token, key=key)

    def terminal(self, url, token="alice"):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            status, value, _ = self.request("GET", url, token=token)
            if status != 200:
                raise AssertionError((status, value))
            if value["status"] in {"succeeded", "rejected", "failed", "expired"}:
                return value
            time.sleep(.02)
        raise AssertionError("operation never reached terminal status")

    def counts(self):
        with sqlite3.connect(self.db) as db:
            tables = {r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            counts = {name: db.execute(f'SELECT COUNT(*) FROM "{name}"').fetchone()[0]
                    for name in ("commands", "operations", "objects", "outbox") if name in tables}
            # The durable command row also owns its operation state in this slice.
            counts.setdefault("operations", counts.get("commands", 0))
            return counts


class RuntimeHTTPTests(unittest.TestCase):
    def setUp(self):
        if not BINARY.is_file():
            self.fail(f"build Rust binary first: {BINARY}")
        self.server = Server()
        self.addCleanup(self.server.close)

    def test_durable_acceptance_then_kill_restart_completes_once(self):
        s = self.server.start(worker=False)
        validate_contract(command(), "CommandInput")
        status, accepted, headers = s.submit()
        self.assertEqual(status, 202, accepted)
        self.assertEqual(accepted["status"], "accepted")
        self.assertEqual(s.request("GET", accepted["operation_url"])[1]["status"], "accepted")
        counts = s.counts()
        self.assertEqual(counts.get("operations"), 1, counts)
        self.assertEqual(counts.get("objects"), 0, counts)
        self.assertEqual(counts.get("outbox"), 0, counts)
        self.assertEqual(headers.get("location"), accepted["operation_url"])
        s.stop(kill=True)
        s.start()
        operation = s.terminal(accepted["operation_url"])
        self.assertEqual(operation["status"], "succeeded", operation)
        status, obj, _ = s.request("GET", SCOPE + "/demo.note/objects/n1")
        self.assertEqual(status, 200, obj)
        self.assertEqual(obj["values"], {"text": "hello"})
        self.assertEqual(obj["revision"], 1)
        validate_contract(accepted, "Accepted")
        validate_contract(operation, "Operation")
        validate_contract(obj, "ObjectDocument")
        self.assertEqual(obj["scope"], {"business_id": "b", "project_id": "p", "client_id": "c"})
        self.assertEqual(s.counts()["objects"], 1)
        self.assertEqual(s.counts()["outbox"], 1)
        s.stop(kill=True)
        s.start()
        self.assertEqual(s.request("GET", accepted["operation_url"])[1]["status"], "succeeded")
        self.assertEqual(s.request("GET", SCOPE + "/demo.note/objects/n1")[1], obj)
        self.assertEqual(s.counts()["outbox"], 1)

    def test_semantic_replay_and_conflicting_idempotency(self):
        s = self.server.start()
        status, accepted, _ = s.submit()
        self.assertEqual(status, 202)
        self.assertEqual(s.terminal(accepted["operation_url"])["status"], "succeeded")
        reordered = {"payload": {"text": "hello", "id": "n1"}, "version": 1, "name": "demo.note.create"}
        status, replay, _ = s.submit(reordered)
        self.assertEqual(status, 202, replay)
        self.assertEqual(replay["operation_id"], accepted["operation_id"])
        self.assertEqual(s.submit(command(text="different"))[0], 409)
        self.assertEqual(s.counts()["operations"], 1)
        self.assertEqual(s.counts()["outbox"], 1)

    def test_duplicate_domain_object_rejected_as_operation(self):
        s = self.server.start()
        first = s.submit()[1]
        self.assertEqual(s.terminal(first["operation_url"])["status"], "succeeded")
        status, second, _ = s.submit(command(text="different"), key="k2")
        self.assertEqual(status, 202, second)
        rejected = s.terminal(second["operation_url"])
        self.assertEqual(rejected["status"], "rejected", rejected)
        self.assertEqual(rejected["problem"]["status"], 409)
        validate_contract(rejected, "Operation")
        validate_contract(rejected["problem"], "Problem")
        self.assertEqual(s.counts()["objects"], 1)
        self.assertEqual(s.counts()["outbox"], 1)

    def test_authentication_is_fail_closed(self):
        s = self.server.start()
        for token in [None, "wrong-token"]:
            self.assertEqual(s.submit(token=token)[0], 401)
            self.assertEqual(s.request("GET", SCOPE + "/demo.note/objects/n1", token=token)[0], 401)
        self.assertEqual(s.counts()["operations"], 0)

    def test_scope_isolation_and_principal_private_operations(self):
        s = self.server.start()
        a = s.submit()[1]
        s.terminal(a["operation_url"])
        for scope in [OTHER, SCOPE.replace("/b/", "/foreign/"), SCOPE.replace("/p/", "/foreign/")]:
            self.assertIn(s.submit(scope=scope)[0], (403, 404))
        self.assertIn(s.request("GET", a["operation_url"], token="bob")[0], (403, 404))
        self.assertIn(s.request("GET", a["operation_url"], token="charlie")[0], (403, 404))
        status, b, _ = s.submit(command(text="other tenant"), scope=OTHER, token="bob")
        self.assertEqual(status, 202, b)
        self.assertNotEqual(a["operation_id"], b["operation_id"])
        self.assertEqual(s.terminal(b["operation_url"], token="bob")["status"], "succeeded")
        self.assertEqual(s.request("GET", OTHER + "/demo.note/objects/n1", token="bob")[1]["values"]["text"], "other tenant")
        self.assertEqual(s.request("GET", SCOPE + "/demo.note/objects/n1")[1]["values"]["text"], "hello")

    def test_idempotency_is_scoped_by_principal(self):
        s = self.server.start()
        a = s.submit(command("alice-note"))[1]
        status, b, _ = s.submit(command("charlie-note"), token="charlie")
        self.assertEqual(status, 202, b)
        self.assertNotEqual(a["operation_id"], b["operation_id"])
        self.assertEqual(s.terminal(a["operation_url"])["status"], "succeeded")
        self.assertEqual(s.terminal(b["operation_url"], token="charlie")["status"], "succeeded")

    def test_concurrent_duplicate_request_accepts_one_operation(self):
        s = self.server.start()
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            results = list(pool.map(lambda _: s.submit(), range(16)))
        self.assertEqual({r[0] for r in results}, {202})
        self.assertEqual(len({r[1]["operation_id"] for r in results}), 1)
        self.assertEqual(s.terminal(results[0][1]["operation_url"])["status"], "succeeded")
        self.assertEqual(s.counts()["operations"], 1)
        self.assertEqual(s.counts()["objects"], 1)
        self.assertEqual(s.counts()["outbox"], 1)

    def test_input_validation_rejects_before_acceptance(self):
        s = self.server.start()
        cases = []
        v = command(); v["name"] = "unknown.command"; cases.append(v)
        v = command(); v["version"] = 9; cases.append(v)
        v = command(); v["payload"]["id"] = "../escape"; cases.append(v)
        v = command(); v["payload"]["text"] = 123; cases.append(v)
        v = command(); v["payload"]["extra"] = "not allowed"; cases.append(v)
        v = command(); v["principal"] = "administrator"; cases.append(v)
        v = command(); del v["payload"]; cases.append(v)
        for i, value in enumerate(cases):
            status, detail, _ = s.submit(value, key=f"invalid-{i}")
            self.assertIn(status, (400, 422), (value, status, detail))
        self.assertEqual(s.counts()["operations"], 0)

    def test_malformed_missing_key_and_oversized_request(self):
        s = self.server.start()
        self.assertEqual(s.request("POST", SCOPE + "/commands", raw=b'{', key="malformed")[0], 400)
        self.assertIn(s.submit(key=None)[0], (400, 422))
        self.assertIn(s.submit(key="")[0], (400, 422))
        self.assertIn(s.submit(key="k" * 129)[0], (400, 422))
        self.assertEqual(s.submit(command(text="x" * 1_100_000), key="oversize")[0], 413)
        self.assertEqual(s.counts()["operations"], 0)

    def test_storage_lock_does_not_return_false_acceptance(self):
        s = self.server.start(worker=False)
        with sqlite3.connect(s.db, timeout=1) as blocker:
            blocker.execute("BEGIN EXCLUSIVE")
            status, detail, _ = s.submit()
            self.assertEqual(status, 503, detail)
            blocker.rollback()
        self.assertEqual(s.counts()["operations"], 0)
        self.assertEqual(s.submit()[0], 202)

    def test_missing_resources_do_not_leak(self):
        s = self.server.start()
        self.assertEqual(s.request("GET", SCOPE + "/operations/not-found")[0], 404)
        self.assertEqual(s.request("GET", SCOPE + "/demo.note/objects/not-found")[0], 404)

    def test_revoked_principal_cannot_commit_queued_command(self):
        s = self.server.start(worker=False)
        status, accepted, _ = s.submit()
        self.assertEqual(status, 202)
        s.stop(kill=True)
        s.start(principals=[("bob", "other")])
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            with sqlite3.connect(s.db) as db:
                row = db.execute("SELECT status, problem FROM commands WHERE operation_id=?",
                                 (accepted["operation_id"],)).fetchone()
            if row[0] != "accepted":
                break
            time.sleep(.02)
        self.assertEqual(row[0], "rejected", row)
        self.assertEqual(json.loads(row[1])["status"], 403)
        self.assertEqual(s.counts()["objects"], 0)
        self.assertEqual(s.counts()["outbox"], 0)
        with sqlite3.connect(s.db) as db:
            self.assertEqual(db.execute("SELECT COUNT(*) FROM inbox").fetchone()[0], 1)
        self.assertEqual(s.request("GET", accepted["operation_url"])[0], 401)

    def test_outbox_failure_rolls_back_entire_domain_commit(self):
        s = self.server.start(worker=False)
        status, accepted, _ = s.submit()
        self.assertEqual(status, 202)
        with sqlite3.connect(s.db) as db:
            db.execute("CREATE TRIGGER fixture_outbox_failure BEFORE INSERT ON outbox "
                       "BEGIN SELECT RAISE(ABORT, 'fixture outbox unavailable'); END")
        s.stop(kill=True)
        s.start()
        # Several dispatcher polls encounter the injected failure. The command
        # must remain retryable and none of the partial state may survive.
        time.sleep(.25)
        with sqlite3.connect(s.db) as db:
            self.assertEqual(db.execute("SELECT status FROM commands WHERE operation_id=?",
                                        (accepted["operation_id"],)).fetchone()[0], "accepted")
            for table in ["objects", "inbox", "outbox"]:
                self.assertEqual(db.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0], 0)
            db.execute("DROP TRIGGER fixture_outbox_failure")
        self.assertEqual(s.terminal(accepted["operation_url"])["status"], "succeeded")
        with sqlite3.connect(s.db) as db:
            for table in ["objects", "inbox", "outbox"]:
                self.assertEqual(db.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0], 1)

    def test_openapi_is_available_and_marks_local_scope(self):
        s = self.server.start()
        status, document, _ = s.request("GET", "/openapi.json", token=None)
        self.assertEqual(status, 200)
        self.assertEqual(document["openapi"], "3.1.0")
        self.assertIn("paths", document)
        self.assertTrue(any(path.endswith("/commands") for path in document["paths"]))

    def test_database_open_failure_does_not_listen(self):
        s = self.server
        with self.assertRaisesRegex(AssertionError, "server exited"):
            s.start(database=s.root / "missing-parent" / "db.sqlite")


if __name__ == "__main__":
    unittest.main(verbosity=2)
