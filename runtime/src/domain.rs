use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const COMMAND_NAME: &str = "demo.note.create";
pub const OBJECT_TYPE: &str = "demo.note";
pub const EVENT_NAME: &str = "demo.note.created";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub business_id: String,
    pub project_id: String,
    pub client_id: String,
}
impl Scope {
    pub fn valid(&self) -> bool {
        [&self.business_id, &self.project_id, &self.client_id]
            .iter()
            .all(|s| identifier(s))
    }
    pub fn base(&self) -> String {
        format!(
            "/v2/businesses/{}/projects/{}/clients/{}",
            self.business_id, self.project_id, self.client_id
        )
    }
}
pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.:-".contains(&c))
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub name: String,
    pub version: u32,
    pub payload: Note,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub id: String,
    pub text: String,
}
impl Command {
    pub fn valid(&self) -> bool {
        self.version == 1
            && crate::fsm::note_machine().is_ok_and(|machine| {
                machine
                    .evaluate(machine.initial_state(), &self.name, &self.payload)
                    .is_ok()
            })
    }
}
// Installation boundary: this slice only supports one immutable collection.
// Generic IR parsing is not a claim of generic storage/action execution.
pub fn validate_installed_machine() -> Result<(), &'static str> {
    use crate::fsm::Action;
    let machine = crate::fsm::note_machine().map_err(|_| "invalid installed machine IR")?;
    let ir = machine.ir();
    let expected_actions = vec![
        Action::CreateObject {
            object_type: OBJECT_TYPE.into(),
        },
        Action::EmitEvent {
            event_name: EVENT_NAME.into(),
        },
    ];
    if machine.collection() != OBJECT_TYPE
        || machine.initial_state() != "absent"
        || ir.states != ["absent", "created"]
        || ir.terminal_states != ["created"]
        || ir.transitions.len() != 1
    {
        return Err("unsupported machine installation profile");
    }
    let t = &ir.transitions[0];
    if t.from != "absent"
        || t.to != "created"
        || t.event != COMMAND_NAME
        || t.actions != expected_actions
    {
        return Err("unsupported installed transition/action plan");
    }
    Ok(())
}
pub fn problem(status: u16, code: &str) -> Value {
    json!({"type":format!("urn:any-rs:problem:{code}"), "title":code, "status":status, "code":code, "request_id":uuid::Uuid::new_v4().to_string()})
}
// Pure handler: no persistence, network, or calls to other application handlers.
pub fn note_document(scope: &Scope, note: &Note, commit: &str) -> Value {
    json!({"scope":scope, "type":OBJECT_TYPE, "id":note.id, "schema_version":1, "revision":1, "values":{"text":note.text}, "commit_id":commit})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dashboard_metadata_matches_registered_runtime_contract() {
        let api: Value = serde_json::from_str(include_str!("../openapi.json")).unwrap();
        let metadata = &api["x-any-rs-runtime"];
        assert_eq!(metadata["command"], COMMAND_NAME);
        assert_eq!(metadata["collection"], OBJECT_TYPE);
        assert_eq!(metadata["emitted_events"], json!([EVENT_NAME]));
        assert_eq!(
            api["components"]["schemas"]["CommandInput"]["properties"]["name"]["const"],
            COMMAND_NAME
        );
    }
    #[test]
    fn identifiers_are_bounded_and_not_paths() {
        for bad in ["", "../x", "a/b", " a", "é"] {
            assert!(!identifier(bad));
        }
        assert!(identifier("a-1:two._"));
        assert!(!identifier(&"a".repeat(129)));
    }
    #[test]
    fn commands_are_explicit_and_closed() {
        assert!(serde_json::from_value::<Command>(json!({"name":"demo.note.create","version":1,"payload":{"id":"x","text":"ok"},"principal":"admin"})).is_err());
        let mut c = Command {
            name: "demo.note.create".into(),
            version: 1,
            payload: Note {
                id: "x".into(),
                text: "ok".into(),
            },
        };
        assert!(c.valid());
        c.payload.text = "".into();
        assert!(!c.valid());
    }
}
