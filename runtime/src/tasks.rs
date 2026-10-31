//! Isolated durable task collection. All business effects and terminal operations
//! commit together; no external callbacks or queue transport occur here.
use crate::{
    domain::{Note, Scope, identifier, problem},
    store::StoreError,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCommand {
    pub name: String,
    pub version: u32,
    pub payload: TaskPayload,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskPayload {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u64>,
}
pub const MAX_REVISION: u64 = 9_007_199_254_740_991;
impl TaskCommand {
    /// Event membership comes from the installed IR; payload shapes remain this
    /// bounded task profile's DTO (title for creation, revision for transitions).
    pub fn valid(&self) -> bool {
        let Ok(machine) = crate::fsm::task_machine() else {
            return false;
        };
        if self.version != 1 || !identifier(&self.payload.id) {
            return false;
        }
        if machine
            .ir()
            .creation
            .as_ref()
            .is_some_and(|c| c.event == self.name)
        {
            self.payload.expected_revision.is_none()
                && self
                    .payload
                    .title
                    .as_ref()
                    .is_some_and(|v| !v.trim().is_empty() && v.len() <= 4096)
        } else if machine
            .ir()
            .transitions
            .iter()
            .any(|t| t.event == self.name)
        {
            self.payload.title.is_none()
                && self
                    .payload
                    .expected_revision
                    .is_some_and(|v| (1..=MAX_REVISION).contains(&v))
        } else {
            false
        }
    }
}
/// The storage interpreter supports exactly these action combinations. Reject
/// otherwise-valid generic machines rather than inventing implicit state writes.
fn validate_profile(machine: &crate::fsm::CompiledMachine) -> Result<(), StoreError> {
    use crate::fsm::Action;
    if machine.collection() != "tasks.task" {
        return Err(StoreError::Unavailable);
    }
    let creation = machine
        .ir()
        .creation
        .as_ref()
        .ok_or(StoreError::Unavailable)?;
    let creates = creation
        .actions
        .iter()
        .filter(
            |a| matches!(a,Action::CreateObject{object_type} if object_type==machine.collection()),
        )
        .count();
    let emits = creation
        .actions
        .iter()
        .filter(|a| matches!(a, Action::EmitEvent { .. }))
        .count();
    if creation.actions.len() != 2 || creates != 1 || emits != 1 {
        return Err(StoreError::Unavailable);
    }
    for transition in &machine.ir().transitions {
        let sets = transition
            .actions
            .iter()
            .filter(|a| matches!(a,Action::SetState{state} if state==&transition.to))
            .count();
        let emits = transition
            .actions
            .iter()
            .filter(|a| matches!(a, Action::EmitEvent { .. }))
            .count();
        if transition.actions.len() != 2 || sets != 1 || emits != 1 {
            return Err(StoreError::Unavailable);
        }
    }
    Ok(())
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn connect(path: &Path) -> Result<Connection, StoreError> {
    if path.as_os_str() == ":memory:" {
        return Err(StoreError::Unavailable);
    }
    let c = Connection::open(path)?;
    c.busy_timeout(std::time::Duration::from_secs(3))?;
    c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")?;
    Ok(c)
}
pub fn initialize(path: &Path) -> Result<(), StoreError> {
    validate_profile(crate::fsm::task_machine().map_err(|_| StoreError::Unavailable)?)?;
    connect(path)?.execute_batch("PRAGMA journal_mode=WAL;
    CREATE TABLE IF NOT EXISTS task_commands (
      operation_id TEXT PRIMARY KEY,business TEXT NOT NULL,project TEXT NOT NULL,client TEXT NOT NULL,
      principal TEXT NOT NULL,idempotency_key TEXT NOT NULL,body TEXT NOT NULL,
      status TEXT NOT NULL CHECK(status IN ('accepted','succeeded','rejected')),
      accepted_at TEXT NOT NULL,updated_at TEXT NOT NULL,result TEXT,problem TEXT,
      UNIQUE(business,project,client,principal,idempotency_key));
    CREATE TABLE IF NOT EXISTS task_objects (
      business TEXT NOT NULL,project TEXT NOT NULL,client TEXT NOT NULL,collection TEXT NOT NULL,
      id TEXT NOT NULL,state TEXT NOT NULL,revision INTEGER NOT NULL,document TEXT NOT NULL,
      PRIMARY KEY(business,project,client,collection,id));
    CREATE TABLE IF NOT EXISTS task_inbox(operation_id TEXT PRIMARY KEY REFERENCES task_commands(operation_id));
    CREATE TABLE IF NOT EXISTS task_outbox(message_id TEXT PRIMARY KEY,operation_id TEXT NOT NULL REFERENCES task_commands(operation_id),body TEXT NOT NULL);")?;
    Ok(())
}
pub fn accept(
    path: &Path,
    s: &Scope,
    p: &str,
    key: &str,
    cmd: &TaskCommand,
) -> Result<Value, StoreError> {
    if !cmd.valid()
        || !s.valid()
        || !identifier(p)
        || key.is_empty()
        || key.len() > 128
        || !key.bytes().all(|c| c.is_ascii_graphic())
    {
        return Err(StoreError::Unavailable);
    }
    let body = serde_json::to_string(cmd)?;
    let mut c = connect(path)?;
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let existing:Option<(String,String,String)>=tx.query_row("SELECT operation_id,body,status FROM task_commands WHERE business=?1 AND project=?2 AND client=?3 AND principal=?4 AND idempotency_key=?5",params![s.business_id,s.project_id,s.client_id,p,key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let (id, status) = if let Some((id, old, status)) = existing {
        if old != body {
            return Err(StoreError::Conflict);
        }
        (id, status)
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        let at = now();
        tx.execute("INSERT INTO task_commands(operation_id,business,project,client,principal,idempotency_key,body,status,accepted_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'accepted',?8,?8)",params![id,s.business_id,s.project_id,s.client_id,p,key,body,at])?;
        (id, "accepted".into())
    };
    tx.commit()?;
    Ok(
        json!({"operation_id":id,"status":status,"operation_url":format!("{}/tasks/operations/{id}",s.base())}),
    )
}
type OperationRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);
pub fn operation(path: &Path, s: &Scope, p: &str, id: &str) -> Result<Option<Value>, StoreError> {
    let c = connect(path)?;
    let row:Option<OperationRow>=c.query_row("SELECT body,status,accepted_at,updated_at,result,problem FROM task_commands WHERE business=?1 AND project=?2 AND client=?3 AND principal=?4 AND operation_id=?5",params![s.business_id,s.project_id,s.client_id,p,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
    row.map(|(body,status,at,updated,result,err)| {
        let cmd:TaskCommand=serde_json::from_str(&body)?;
        let mut v=json!({"operation_id":id,"scope":s,"command_name":cmd.name,"status":status,"accepted_at":at,"updated_at":updated});
        if let Some(r)=result {v["result"]=serde_json::from_str(&r)?;}
        if let Some(e)=err {v["problem"]=serde_json::from_str(&e)?;}
        Ok(v)
    }).transpose()
}
pub fn object(path: &Path, s: &Scope, id: &str) -> Result<Option<Value>, StoreError> {
    let c = connect(path)?;
    let row:Option<String>=c.query_row("SELECT document FROM task_objects WHERE business=?1 AND project=?2 AND client=?3 AND collection='tasks.task' AND id=?4",params![s.business_id,s.project_id,s.client_id,id],|r|r.get(0)).optional()?;
    row.map(|v| serde_json::from_str(&v).map_err(Into::into))
        .transpose()
}
pub fn dispatch_one(path: &Path, grants: &[(String, Scope)]) -> Result<bool, StoreError> {
    dispatch(path, None, grants)
}
pub fn dispatch_operation(
    path: &Path,
    s: &Scope,
    p: &str,
    id: &str,
    grants: &[(String, Scope)],
) -> Result<(), StoreError> {
    dispatch(path, Some((s, p, id)), grants)?;
    Ok(())
}
fn reject(tx: &Transaction<'_>, id: &str, status: u16, code: &str) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE task_commands SET status='rejected',updated_at=?1,problem=?2 WHERE operation_id=?3",
        params![now(), problem(status, code).to_string(), id],
    )?;
    Ok(())
}
fn dispatch(
    path: &Path,
    selected: Option<(&Scope, &str, &str)>,
    grants: &[(String, Scope)],
) -> Result<bool, StoreError> {
    let mut c = connect(path)?;
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let id = if let Some((s, p, id)) = selected {
        let status:Option<String>=tx.query_row("SELECT status FROM task_commands WHERE business=?1 AND project=?2 AND client=?3 AND principal=?4 AND operation_id=?5",params![s.business_id,s.project_id,s.client_id,p,id],|r|r.get(0)).optional()?;
        match status.as_deref() {
            Some("accepted") => id.to_owned(),
            Some("succeeded" | "rejected") => {
                tx.commit()?;
                return Ok(false);
            }
            _ => return Err(StoreError::Unavailable),
        }
    } else {
        let id:Option<String>=tx.query_row("SELECT operation_id FROM task_commands WHERE status='accepted' ORDER BY rowid LIMIT 1",[],|r|r.get(0)).optional()?;
        let Some(id) = id else { return Ok(false) };
        id
    };
    let (business_id, project_id, client_id, principal, body): (
        String,
        String,
        String,
        String,
        String,
    ) = tx.query_row(
        "SELECT business,project,client,principal,body FROM task_commands WHERE operation_id=?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    let s = Scope {
        business_id,
        project_id,
        client_id,
    };
    let cmd: TaskCommand = serde_json::from_str(&body)?;
    if !grants
        .iter()
        .any(|(p, scope)| p == &principal && scope == &s)
    {
        reject(&tx, &id, 403, "scope_revoked")?;
    } else {
        execute(&tx, &id, &s, &cmd)?;
    }
    tx.execute("INSERT INTO task_inbox VALUES(?1)", params![id])?;
    tx.commit()?;
    Ok(true)
}
fn execute(tx: &Transaction<'_>, id: &str, s: &Scope, cmd: &TaskCommand) -> Result<(), StoreError> {
    let existing:Option<(String,i64,String)>=tx.query_row("SELECT state,revision,document FROM task_objects WHERE business=?1 AND project=?2 AND client=?3 AND collection='tasks.task' AND id=?4",params![s.business_id,s.project_id,s.client_id,cmd.payload.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let machine = crate::fsm::task_machine().map_err(|_| StoreError::Unavailable)?;
    validate_profile(machine)?;
    let creation = machine
        .ir()
        .creation
        .as_ref()
        .ok_or(StoreError::Unavailable)?;
    let (state, revision, title, events) = if cmd.name == creation.event {
        if existing.is_some() {
            return reject(tx, id, 409, "object_exists");
        }
        let machine = crate::fsm::task_machine().map_err(|_| StoreError::Unavailable)?;
        let creation = machine
            .ir()
            .creation
            .as_ref()
            .ok_or(StoreError::Unavailable)?;
        if creation.event != cmd.name {
            return Err(StoreError::Unavailable);
        }
        let mut events = Vec::new();
        let mut creates = 0;
        for action in &creation.actions {
            match action {
                crate::fsm::Action::CreateObject { object_type }
                    if object_type == machine.collection() =>
                {
                    creates += 1;
                }
                crate::fsm::Action::EmitEvent { event_name } => events.push(event_name.clone()),
                _ => return Err(StoreError::Unavailable),
            }
        }
        if creates != 1 {
            return Err(StoreError::Unavailable);
        }
        (
            creation.initial_state.clone(),
            1,
            cmd.payload.title.clone().ok_or(StoreError::Unavailable)?,
            events,
        )
    } else {
        let Some((old, rev, document)) = existing else {
            return reject(tx, id, 404, "object_not_found");
        };
        if cmd.payload.expected_revision != u64::try_from(rev).ok() {
            return reject(tx, id, 409, "revision_conflict");
        }
        if rev >= MAX_REVISION as i64 {
            return reject(tx, id, 409, "revision_exhausted");
        }
        let plan = match machine.evaluate(
            &old,
            &cmd.name,
            &Note {
                id: cmd.payload.id.clone(),
                text: String::new(),
            },
        ) {
            Ok(plan) => plan,
            Err(_) => return reject(tx, id, 409, "transition_denied"),
        };
        let mut events = Vec::new();
        for action in &plan.actions {
            match action {
                crate::fsm::Action::SetState { state } if state == &plan.to_state => {}
                crate::fsm::Action::EmitEvent { event_name } => events.push(event_name.clone()),
                _ => return Err(StoreError::Unavailable),
            }
        }
        let doc: Value = serde_json::from_str(&document)?;
        (
            plan.to_state,
            rev + 1,
            doc["values"]["title"]
                .as_str()
                .ok_or(StoreError::Unavailable)?
                .to_owned(),
            events,
        )
    };
    let commit = uuid::Uuid::new_v4().to_string();
    let document = json!({"scope":s,"type":"tasks.task","id":cmd.payload.id,"schema_version":1,"revision":revision,"values":{"title":title},"state":state,"commit_id":commit});
    tx.execute("INSERT INTO task_objects(business,project,client,collection,id,state,revision,document) VALUES(?1,?2,?3,'tasks.task',?4,?5,?6,?7) ON CONFLICT(business,project,client,collection,id) DO UPDATE SET state=excluded.state,revision=excluded.revision,document=excluded.document",params![s.business_id,s.project_id,s.client_id,cmd.payload.id,state,revision,document.to_string()])?;
    let result =
        json!({"commit_id":commit,"resources":[{"type":"tasks.task","id":cmd.payload.id}]});
    tx.execute(
        "UPDATE task_commands SET status='succeeded',updated_at=?1,result=?2 WHERE operation_id=?3",
        params![now(), result.to_string(), id],
    )?;
    for (index, event_name) in events.iter().enumerate() {
        let message_id = format!("{id}:{index}");
        let event = json!({"message_id":message_id,"name":event_name,"scope":s,"operation_id":id,"payload":document});
        tx.execute(
            "INSERT INTO task_outbox VALUES(?1,?2,?3)",
            params![message_id, id, event.to_string()],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        path: std::path::PathBuf,
        directory: std::path::PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let directory =
                std::env::temp_dir().join(format!("anyrs-tasks-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&directory).unwrap();
            let path = directory.join("state.sqlite");
            initialize(&path).unwrap();
            Self { path, directory }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
    fn scope() -> Scope {
        Scope {
            business_id: "b".into(),
            project_id: "p".into(),
            client_id: "c".into(),
        }
    }
    fn command(name: &str, rev: Option<u64>) -> TaskCommand {
        TaskCommand {
            name: name.into(),
            version: 1,
            payload: TaskPayload {
                id: "t".into(),
                title: if name == "tasks.create" {
                    Some("Task".into())
                } else {
                    None
                },
                expected_revision: rev,
            },
        }
    }
    fn run(f: &Fixture, cmd: &TaskCommand, key: &str) -> Value {
        let s = scope();
        let op = accept(&f.path, &s, "alice", key, cmd).unwrap();
        let id = op["operation_id"].as_str().unwrap();
        dispatch_operation(&f.path, &s, "alice", id, &[("alice".into(), s.clone())]).unwrap();
        operation(&f.path, &s, "alice", id).unwrap().unwrap()
    }
    #[test]
    fn lifecycle_denial_cas_and_terminal_replay() {
        let f = Fixture::new();
        let s = scope();
        let create = run(&f, &command("tasks.create", None), "create");
        assert_eq!(create["status"], "succeeded");
        let original = object(&f.path, &s, "t").unwrap().unwrap();
        assert_eq!(original["state"], "pending");
        let deny = run(&f, &command("tasks.complete", Some(1)), "illegal");
        assert_eq!(deny["problem"]["code"], "transition_denied");
        assert_eq!(Some(original), object(&f.path, &s, "t").unwrap());
        assert_eq!(
            run(&f, &command("tasks.start", Some(1)), "start")["status"],
            "succeeded"
        );
        assert_eq!(
            run(&f, &command("tasks.complete", Some(1)), "stale")["problem"]["code"],
            "revision_conflict"
        );
        let complete = run(&f, &command("tasks.complete", Some(2)), "complete");
        assert_eq!(complete["status"], "succeeded");
        let doc = object(&f.path, &s, "t").unwrap().unwrap();
        assert_eq!(doc["state"], "completed");
        assert_eq!(doc["revision"], 3);
        dispatch_operation(
            &f.path,
            &s,
            "alice",
            complete["operation_id"].as_str().unwrap(),
            &[],
        )
        .unwrap();
        let c = connect(&f.path).unwrap();
        let count: i64 = c
            .query_row("SELECT COUNT(*) FROM task_outbox", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
        assert!(
            operation(
                &f.path,
                &s,
                "bob",
                complete["operation_id"].as_str().unwrap()
            )
            .unwrap()
            .is_none()
        );
        assert!(matches!(
            accept(
                &f.path,
                &s,
                "alice",
                "start",
                &command("tasks.complete", Some(2))
            ),
            Err(StoreError::Conflict)
        ));
    }
    #[test]
    fn interpreter_rejects_generic_ir_without_explicit_profile_actions() {
        use crate::fsm::{Action, CompiledMachine};
        let base = crate::fsm::task_machine().unwrap().ir().clone();
        let mut emit_only = base.clone();
        emit_only.transitions[0]
            .actions
            .retain(|a| matches!(a, Action::EmitEvent { .. }));
        assert!(validate_profile(&CompiledMachine::compile(emit_only).unwrap()).is_err());
        let mut no_emit = base.clone();
        no_emit
            .creation
            .as_mut()
            .unwrap()
            .actions
            .retain(|a| matches!(a, Action::CreateObject { .. }));
        assert!(validate_profile(&CompiledMachine::compile(no_emit).unwrap()).is_err());
        assert!(validate_profile(&CompiledMachine::compile(base).unwrap()).is_ok());
    }
    #[test]
    fn input_shape_and_unknown_commands_are_closed() {
        assert!(command("tasks.create", None).valid());
        assert!(!command("tasks.create", Some(1)).valid());
        assert!(!command("tasks.start", None).valid());
        assert!(!command("tasks.start", Some(0)).valid());
        assert!(!command("tasks.delete", Some(1)).valid());
        assert!(serde_json::from_value::<TaskCommand>(json!({"name":"tasks.create","version":1,"payload":{"id":"t","title":"T","extra":true}})).is_err());
    }
}
