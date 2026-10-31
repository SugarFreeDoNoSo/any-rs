use crate::domain::{Command, EVENT_NAME, OBJECT_TYPE, Scope, note_document, problem};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
type OperationRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);

#[derive(Debug)]
pub enum StoreError {
    Conflict,
    Unavailable,
}
impl From<rusqlite::Error> for StoreError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Unavailable
    }
}
impl From<serde_json::Error> for StoreError {
    fn from(_: serde_json::Error) -> Self {
        Self::Unavailable
    }
}
// Small logical port; no cloud transport masquerading as this local adapter.
pub trait RuntimeStore: Send + Sync {
    fn accept(
        &self,
        scope: &Scope,
        principal: &str,
        key: &str,
        command: &Command,
    ) -> Result<Value, StoreError>;
    fn operation(
        &self,
        scope: &Scope,
        principal: &str,
        id: &str,
    ) -> Result<Option<Value>, StoreError>;
    fn object(&self, scope: &Scope, id: &str) -> Result<Option<Value>, StoreError>;
    fn accept_task(
        &self,
        scope: &Scope,
        principal: &str,
        key: &str,
        command: &crate::tasks::TaskCommand,
    ) -> Result<Value, StoreError>;
    fn task_operation(
        &self,
        scope: &Scope,
        principal: &str,
        id: &str,
    ) -> Result<Option<Value>, StoreError>;
    fn task_object(&self, scope: &Scope, id: &str) -> Result<Option<Value>, StoreError>;
    fn dispatch_one(&self, grants: &[(String, Scope)]) -> Result<bool, StoreError>;
    fn dispatch_operation(
        &self,
        scope: &Scope,
        principal: &str,
        operation_id: &str,
        grants: &[(String, Scope)],
    ) -> Result<(), StoreError>;
}
#[derive(Clone)]
pub struct SqliteStore {
    path: PathBuf,
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if path.as_os_str() == ":memory:" {
            return Err(StoreError::Unavailable);
        }
        crate::domain::validate_installed_machine().map_err(|_| StoreError::Unavailable)?;
        let s = Self {
            path: path.to_owned(),
        };
        let c = s.connect()?;
        c.execute_batch("PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS commands (
          operation_id TEXT PRIMARY KEY,business TEXT NOT NULL,project TEXT NOT NULL,client TEXT NOT NULL,
          principal TEXT NOT NULL,idempotency_key TEXT NOT NULL,body TEXT NOT NULL,
          status TEXT NOT NULL CHECK(status IN ('accepted','succeeded','rejected')),
          accepted_at TEXT NOT NULL,updated_at TEXT NOT NULL,result TEXT,problem TEXT,
          UNIQUE(business,project,client,principal,idempotency_key));
        CREATE TABLE IF NOT EXISTS objects (
          business TEXT NOT NULL,project TEXT NOT NULL,client TEXT NOT NULL,id TEXT NOT NULL,document TEXT NOT NULL,
          PRIMARY KEY(business,project,client,id));
        CREATE TABLE IF NOT EXISTS inbox (operation_id TEXT PRIMARY KEY REFERENCES commands(operation_id));
        CREATE TABLE IF NOT EXISTS outbox (message_id TEXT PRIMARY KEY,operation_id TEXT NOT NULL UNIQUE REFERENCES commands(operation_id),body TEXT NOT NULL);")?;
        crate::tasks::initialize(path)?;
        Ok(s)
    }
    fn connect(&self) -> Result<Connection, StoreError> {
        let c = Connection::open(&self.path)?;
        c.busy_timeout(std::time::Duration::from_secs(3))?;
        c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")?;
        Ok(c)
    }
}
impl RuntimeStore for SqliteStore {
    fn accept(&self, s: &Scope, p: &str, key: &str, cmd: &Command) -> Result<Value, StoreError> {
        let body = serde_json::to_string(cmd)?;
        let mut c = self.connect()?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing:Option<(String,String,String)>=tx.query_row("SELECT operation_id,body,status FROM commands WHERE business=?1 AND project=?2 AND client=?3 AND principal=?4 AND idempotency_key=?5",params![s.business_id,s.project_id,s.client_id,p,key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((id, old, status)) = existing {
            if old != body {
                return Err(StoreError::Conflict);
            }
            tx.commit()?;
            return Ok(
                json!({"operation_id":id,"status":status,"operation_url":format!("{}/operations/{id}",s.base())}),
            );
        }
        let id = uuid::Uuid::new_v4().to_string();
        let at = now();
        tx.execute("INSERT INTO commands(operation_id,business,project,client,principal,idempotency_key,body,status,accepted_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'accepted',?8,?8)",params![id,s.business_id,s.project_id,s.client_id,p,key,body,at])?;
        tx.commit()?; // No 202 before durable commit.
        Ok(
            json!({"operation_id":id,"status":"accepted","operation_url":format!("{}/operations/{id}",s.base())}),
        )
    }
    fn operation(&self, s: &Scope, p: &str, id: &str) -> Result<Option<Value>, StoreError> {
        let c = self.connect()?;
        let row:Option<OperationRow>=c.query_row("SELECT body,status,accepted_at,updated_at,result,problem FROM commands WHERE business=?1 AND project=?2 AND client=?3 AND principal=?4 AND operation_id=?5",params![s.business_id,s.project_id,s.client_id,p,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
        row.map(|(body,status,at,updated,result,err)| {
            let cmd:Command=serde_json::from_str(&body)?;
            let mut v=json!({"operation_id":id,"scope":s,"command_name":cmd.name,"status":status,"accepted_at":at,"updated_at":updated});
            if let Some(r)=result {v["result"]=serde_json::from_str(&r)?;}
            if let Some(e)=err {v["problem"]=serde_json::from_str(&e)?;}
            Ok(v)
        }).transpose()
    }
    fn object(&self, s: &Scope, id: &str) -> Result<Option<Value>, StoreError> {
        let c = self.connect()?;
        let row:Option<String>=c.query_row("SELECT document FROM objects WHERE business=?1 AND project=?2 AND client=?3 AND id=?4",params![s.business_id,s.project_id,s.client_id,id],|r|r.get(0)).optional()?;
        row.map(|r| serde_json::from_str(&r).map_err(Into::into))
            .transpose()
    }
    fn accept_task(
        &self,
        scope: &Scope,
        principal: &str,
        key: &str,
        command: &crate::tasks::TaskCommand,
    ) -> Result<Value, StoreError> {
        crate::tasks::accept(&self.path, scope, principal, key, command)
    }
    fn task_operation(
        &self,
        scope: &Scope,
        principal: &str,
        id: &str,
    ) -> Result<Option<Value>, StoreError> {
        crate::tasks::operation(&self.path, scope, principal, id)
    }
    fn task_object(&self, scope: &Scope, id: &str) -> Result<Option<Value>, StoreError> {
        crate::tasks::object(&self.path, scope, id)
    }
    fn dispatch_one(&self, grants: &[(String, Scope)]) -> Result<bool, StoreError> {
        if self.dispatch_selected(None, grants)? {
            return Ok(true);
        }
        crate::tasks::dispatch_one(&self.path, grants)
    }
    fn dispatch_operation(
        &self,
        scope: &Scope,
        principal: &str,
        operation_id: &str,
        grants: &[(String, Scope)],
    ) -> Result<(), StoreError> {
        if self.operation(scope, principal, operation_id)?.is_some() {
            self.dispatch_selected(Some((scope, principal, operation_id)), grants)?;
            Ok(())
        } else {
            crate::tasks::dispatch_operation(&self.path, scope, principal, operation_id, grants)
        }
    }
}
impl SqliteStore {
    fn dispatch_selected(
        &self,
        selected: Option<(&Scope, &str, &str)>,
        grants: &[(String, Scope)],
    ) -> Result<bool, StoreError> {
        let mut c = self.connect()?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Resolve identity and status within the same write transaction as the effects.
        let row = if let Some((scope, principal, id)) = selected {
            let status: Option<String> = tx.query_row(
                "SELECT status FROM commands WHERE business=?1 AND project=?2 AND client=?3 AND principal=?4 AND operation_id=?5",
                params![scope.business_id, scope.project_id, scope.client_id, principal, id], |r| r.get(0)
            ).optional()?;
            match status.as_deref() {
                None => return Err(StoreError::Unavailable),
                Some("succeeded" | "rejected") => {
                    tx.commit()?;
                    return Ok(false);
                }
                Some("accepted") => {}
                Some(_) => return Err(StoreError::Unavailable),
            }
            tx.query_row("SELECT operation_id,business,project,client,body,principal FROM commands WHERE operation_id=?1", params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?))).optional()?
        } else {
            tx.query_row("SELECT operation_id,business,project,client,body,principal FROM commands WHERE status='accepted' ORDER BY rowid LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?
        };
        let Some((id, business_id, project_id, client_id, body, principal)) = row else {
            return Ok(false);
        };
        let s = Scope {
            business_id,
            project_id,
            client_id,
        };
        let cmd: Command = serde_json::from_str(&body)?;
        let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM objects WHERE business=?1 AND project=?2 AND client=?3 AND id=?4)",params![s.business_id,s.project_id,s.client_id,cmd.payload.id],|r|r.get(0))?;
        let machine = crate::fsm::note_machine().map_err(|_| StoreError::Unavailable)?;
        // State is derived under the same SQLite write transaction. This bounded
        // create-only profile has absent/created states, not arbitrary lifecycle columns.
        let current_state = if exists {
            "created"
        } else {
            machine.initial_state()
        };
        let transition = machine.evaluate(current_state, &cmd.name, &cmd.payload);
        if !grants
            .iter()
            .any(|(p, scope)| p == &principal && scope == &s)
        {
            tx.execute("UPDATE commands SET status='rejected',updated_at=?1,problem=?2 WHERE operation_id=?3",params![now(),problem(403,"scope_revoked").to_string(),id])?;
        } else if transition.is_err() {
            // Rejected operations retain audit metadata, never domain/null writes.
            let (status, code) = if exists {
                (409, "object_exists")
            } else {
                (422, "transition_denied")
            };
            tx.execute("UPDATE commands SET status='rejected',updated_at=?1,problem=?2 WHERE operation_id=?3",params![now(),problem(status,code).to_string(),id])?;
        } else {
            let plan = transition.map_err(|_| StoreError::Unavailable)?;
            // Only the closed, installed create+emit action plan can reach effects.
            if plan.actions
                != [
                    crate::fsm::Action::CreateObject {
                        object_type: OBJECT_TYPE.into(),
                    },
                    crate::fsm::Action::EmitEvent {
                        event_name: EVENT_NAME.into(),
                    },
                ]
            {
                return Err(StoreError::Unavailable);
            }
            let commit = uuid::Uuid::new_v4().to_string();
            let document = note_document(&s, &cmd.payload, &commit);
            tx.execute(
                "INSERT INTO objects VALUES(?1,?2,?3,?4,?5)",
                params![
                    s.business_id,
                    s.project_id,
                    s.client_id,
                    cmd.payload.id,
                    document.to_string()
                ],
            )?;
            let result =
                json!({"commit_id":commit,"resources":[{"type":OBJECT_TYPE,"id":cmd.payload.id}]});
            tx.execute("UPDATE commands SET status='succeeded',updated_at=?1,result=?2 WHERE operation_id=?3",params![now(),result.to_string(),id])?;
            let event = json!({"message_id":id,"name":EVENT_NAME,"scope":s,"operation_id":id,"payload":document});
            tx.execute(
                "INSERT INTO outbox VALUES(?1,?1,?2)",
                params![id, event.to_string()],
            )?;
        }
        tx.execute("INSERT INTO inbox VALUES(?1)", params![id])?;
        tx.commit()?; // Object, inbox, outbox and terminal operation are one atomic commit.
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Note;

    struct Fixture {
        directory: PathBuf,
        store: SqliteStore,
    }
    impl Fixture {
        fn new() -> Self {
            let directory =
                std::env::temp_dir().join(format!("anyrs-dispatch-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&directory).unwrap();
            let store = SqliteStore::open(&directory.join("state.sqlite3")).unwrap();
            Self { directory, store }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
    fn scope(client: &str) -> Scope {
        Scope {
            business_id: "business".into(),
            project_id: "project".into(),
            client_id: client.into(),
        }
    }
    fn accept(store: &SqliteStore, s: &Scope, principal: &str, id: &str) -> String {
        store
            .accept(
                s,
                principal,
                id,
                &Command {
                    name: "demo.note.create".into(),
                    version: 1,
                    payload: Note {
                        id: id.into(),
                        text: "fixture".into(),
                    },
                },
            )
            .unwrap()["operation_id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    #[test]
    fn targeted_dispatch_preserves_unrelated_queue_and_replays_terminal() {
        let f = Fixture::new();
        let a = scope("a");
        let b = scope("b");
        let first = accept(&f.store, &a, "alice", "first");
        let selected = accept(&f.store, &b, "bob", "selected");
        let grants = vec![("alice".into(), a.clone()), ("bob".into(), b.clone())];
        f.store
            .dispatch_operation(&b, "bob", &selected, &grants)
            .unwrap();
        assert_eq!(
            f.store.operation(&a, "alice", &first).unwrap().unwrap()["status"],
            "accepted"
        );
        assert!(f.store.object(&a, "first").unwrap().is_none());
        let completed = f.store.operation(&b, "bob", &selected).unwrap();
        assert_eq!(completed.as_ref().unwrap()["status"], "succeeded");
        // Even revoked grants must not repeat an already committed effect.
        f.store
            .dispatch_operation(&b, "bob", &selected, &[])
            .unwrap();
        assert_eq!(completed, f.store.operation(&b, "bob", &selected).unwrap());
        let c = f.store.connect().unwrap();
        let count: i64 = c
            .query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert!(f.store.dispatch_one(&grants).unwrap());
        assert!(f.store.object(&a, "first").unwrap().is_some());
        assert!(!f.store.dispatch_one(&grants).unwrap());
    }
    #[test]
    fn targeted_dispatch_rejects_identity_mismatch_and_revocation() {
        let f = Fixture::new();
        let a = scope("a");
        let b = scope("b");
        let id = accept(&f.store, &a, "alice", "note");
        let grants = vec![("alice".into(), a.clone())];
        for (s, p, op) in [
            (&b, "alice", id.as_str()),
            (&a, "bob", id.as_str()),
            (&a, "alice", "missing"),
        ] {
            assert!(matches!(
                f.store.dispatch_operation(s, p, op, &grants),
                Err(StoreError::Unavailable)
            ));
        }
        assert_eq!(
            f.store.operation(&a, "alice", &id).unwrap().unwrap()["status"],
            "accepted"
        );
        f.store.dispatch_operation(&a, "alice", &id, &[]).unwrap();
        let result = f.store.operation(&a, "alice", &id).unwrap().unwrap();
        assert_eq!(result["status"], "rejected");
        assert_eq!(result["problem"]["code"], "scope_revoked");
        assert!(f.store.object(&a, "note").unwrap().is_none());
        f.store
            .dispatch_operation(&a, "alice", &id, &grants)
            .unwrap();
        assert_eq!(
            result,
            f.store.operation(&a, "alice", &id).unwrap().unwrap()
        );
    }
}
