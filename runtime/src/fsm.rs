//! Closed, pure state-machine IR. Evaluation returns a proposal, never writes.
use crate::domain::{Note, identifier};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::OnceLock,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MachineIr {
    pub schema_version: u32,
    pub id: String,
    pub collection: String,
    pub initial_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation: Option<Creation>,
    pub states: Vec<String>,
    #[serde(default)]
    pub state_labels: BTreeMap<String, String>,
    pub terminal_states: Vec<String>,
    pub transitions: Vec<Transition>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Creation {
    pub event: String,
    pub initial_state: String,
    pub actions: Vec<Action>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub from: String,
    pub event: String,
    pub to: String,
    pub guards: Vec<Guard>,
    pub actions: Vec<Action>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum InputField {
    #[serde(rename = "input.id")]
    Id,
    #[serde(rename = "input.text")]
    Text,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Guard {
    ValidIdentifier {
        field: InputField,
    },
    StringLength {
        field: InputField,
        min: usize,
        max: usize,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    CreateObject { object_type: String },
    EmitEvent { event_name: String },
    SetState { state: String },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionPlan {
    pub from_state: String,
    pub to_state: String,
    pub actions: Vec<Action>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError(pub String);
impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ValidationError {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionError {
    /// Unknown state/event, terminal state, or no explicitly declared edge.
    Denied,
    GuardFailed,
}
#[derive(Debug)]
pub struct CompiledMachine {
    ir: MachineIr,
    index: BTreeMap<(String, String), usize>,
}
impl CompiledMachine {
    pub fn from_json(source: &str) -> Result<Self, ValidationError> {
        if source.len() > 1024 * 1024 {
            return Err(ValidationError("IR exceeds 1 MiB".into()));
        }
        let ir = serde_json::from_str(source)
            .map_err(|e| ValidationError(format!("invalid IR JSON: {e}")))?;
        Self::compile(ir)
    }
    pub fn from_toml(source: &str) -> Result<Self, ValidationError> {
        if source.len() > 1024 * 1024 {
            return Err(ValidationError("IR exceeds 1 MiB".into()));
        }
        let ir =
            toml::from_str(source).map_err(|e| ValidationError(format!("invalid IR TOML: {e}")))?;
        Self::compile(ir)
    }
    pub fn compile(ir: MachineIr) -> Result<Self, ValidationError> {
        let invalid = |reason: &str| ValidationError(reason.into());
        if ir.schema_version != 1 || !identifier(&ir.id) || !identifier(&ir.collection) {
            return Err(invalid(
                "unsupported version or invalid machine/collection identifier",
            ));
        }
        if ir.states.is_empty() || ir.states.len() > 256 || ir.transitions.len() > 1024 {
            return Err(invalid("machine exceeds state/transition bounds"));
        }
        let states: BTreeSet<_> = ir.states.iter().cloned().collect();
        if states.len() != ir.states.len()
            || states.iter().any(|s| !identifier(s))
            || !states.contains(&ir.initial_state)
        {
            return Err(invalid(
                "states must be unique identifiers and contain initial_state",
            ));
        }
        if ir.state_labels.iter().any(|(state, label)| {
            !states.contains(state)
                || label.is_empty()
                || label.len() > 128
                || label.chars().any(char::is_control)
        }) {
            return Err(invalid(
                "state labels must name declared states and contain bounded display text",
            ));
        }
        let terminals: BTreeSet<_> = ir.terminal_states.iter().cloned().collect();
        if terminals.len() != ir.terminal_states.len() || !terminals.is_subset(&states) {
            return Err(invalid("terminal states must be unique declared states"));
        }
        if let Some(creation) = &ir.creation {
            if !identifier(&creation.event)
                || creation.initial_state != ir.initial_state
                || creation.actions.is_empty()
                || creation.actions.len() > 16
            {
                return Err(invalid(
                    "creation must declare a valid event, the machine initial state and bounded actions",
                ));
            }
            if ir.transitions.iter().any(|t| t.event == creation.event) {
                return Err(invalid("creation event cannot also be a transition event"));
            }
            let mut creates = 0;
            let mut events = BTreeSet::new();
            for action in &creation.actions {
                match action {
                    Action::CreateObject { object_type } if object_type == &ir.collection => {
                        creates += 1
                    }
                    Action::EmitEvent { event_name }
                        if identifier(event_name) && events.insert(event_name.clone()) => {}
                    _ => {
                        return Err(invalid(
                            "invalid creation action, collection or emitted event",
                        ));
                    }
                }
            }
            if creates != 1 {
                return Err(invalid(
                    "creation requires exactly one create_object action",
                ));
            }
        }
        let mut index = BTreeMap::new();
        for (i, t) in ir.transitions.iter().enumerate() {
            if !states.contains(&t.from)
                || !states.contains(&t.to)
                || !identifier(&t.event)
                || terminals.contains(&t.from)
            {
                return Err(invalid(
                    "transition endpoint/event invalid or terminal source",
                ));
            }
            // No first-match ordering and no attempt to prove disjoint predicates.
            if index.insert((t.from.clone(), t.event.clone()), i).is_some() {
                return Err(invalid("ambiguous state/event pair, even if guards differ"));
            }
            if t.actions.is_empty() || t.actions.len() > 16 || t.guards.len() > 16 {
                return Err(invalid(
                    "transition actions must be nonempty and within bounds",
                ));
            }
            for guard in &t.guards {
                match guard {
                    Guard::ValidIdentifier {
                        field: InputField::Id,
                    } => {}
                    Guard::StringLength {
                        field: InputField::Text,
                        min,
                        max,
                    } if min <= max && *max <= 4096 => {}
                    _ => {
                        return Err(invalid(
                            "guard has unsupported field/type or invalid bounds",
                        ));
                    }
                }
            }
            let mut creates = 0;
            let mut state_changes = 0;
            let mut emits = BTreeSet::new();
            for action in &t.actions {
                match action {
                    Action::CreateObject { object_type } if object_type == &ir.collection => {
                        creates += 1;
                    }
                    Action::SetState { state } if state == &t.to => {
                        state_changes += 1;
                    }
                    Action::EmitEvent { event_name }
                        if identifier(event_name) && emits.insert(event_name.clone()) => {}
                    _ => {
                        return Err(invalid(
                            "action references unsupported object type or duplicate/invalid event",
                        ));
                    }
                }
            }
            if state_changes > 1 {
                return Err(invalid("a transition cannot set state twice"));
            }
            if creates > 1 {
                return Err(invalid("a transition cannot create the same object twice"));
            }
        }
        Ok(Self { ir, index })
    }
    pub fn id(&self) -> &str {
        &self.ir.id
    }
    pub fn collection(&self) -> &str {
        &self.ir.collection
    }
    pub fn initial_state(&self) -> &str {
        &self.ir.initial_state
    }
    pub fn ir(&self) -> &MachineIr {
        &self.ir
    }
    /// Pure planner. It does not authorize a tenant, claim a delivery, perform
    /// CAS, or commit. The engine must independently enforce all those gates.
    pub fn evaluate(
        &self,
        state: &str,
        event: &str,
        input: &Note,
    ) -> Result<TransitionPlan, TransitionError> {
        let i = self
            .index
            .get(&(state.into(), event.into()))
            .ok_or(TransitionError::Denied)?;
        let transition = &self.ir.transitions[*i];
        for guard in &transition.guards {
            let valid = match guard {
                Guard::ValidIdentifier {
                    field: InputField::Id,
                } => identifier(&input.id),
                Guard::StringLength {
                    field: InputField::Text,
                    min,
                    max,
                } => input.text.len() >= *min && input.text.len() <= *max,
                _ => false,
            };
            if !valid {
                return Err(TransitionError::GuardFailed);
            }
        }
        Ok(TransitionPlan {
            from_state: state.into(),
            to_state: transition.to.clone(),
            actions: transition.actions.clone(),
        })
    }
}
static NOTE_MACHINE: OnceLock<Result<CompiledMachine, ValidationError>> = OnceLock::new();
pub fn note_machine() -> Result<&'static CompiledMachine, &'static ValidationError> {
    NOTE_MACHINE
        .get_or_init(|| CompiledMachine::from_json(include_str!("../machines/demo-note.json")))
        .as_ref()
}

static TASK_MACHINE: OnceLock<Result<CompiledMachine, ValidationError>> = OnceLock::new();
pub fn task_machine() -> Result<&'static CompiledMachine, &'static ValidationError> {
    TASK_MACHINE
        .get_or_init(|| CompiledMachine::from_json(include_str!("../machines/tasks.json")))
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Note {
        Note {
            id: "n1".into(),
            text: "hello".into(),
        }
    }
    fn ir() -> MachineIr {
        serde_json::from_str(include_str!("../machines/demo-note.json")).unwrap()
    }
    #[test]
    fn declared_transition_yields_explicit_actions() {
        let m = note_machine().unwrap();
        assert_eq!(m.initial_state(), "absent");
        let p = m.evaluate("absent", "demo.note.create", &input()).unwrap();
        assert_eq!(p.to_state, "created");
        assert_eq!(
            p.actions,
            vec![
                Action::CreateObject {
                    object_type: "demo.note".into()
                },
                Action::EmitEvent {
                    event_name: "demo.note.created".into()
                }
            ]
        );
    }
    #[test]
    fn undeclared_edges_and_terminal_events_are_denied() {
        let m = note_machine().unwrap();
        for (state, event) in [
            ("absent", "read"),
            ("created", "demo.note.create"),
            ("missing", "demo.note.create"),
            ("absent", "demo.note.delete"),
        ] {
            assert_eq!(
                m.evaluate(state, event, &input()),
                Err(TransitionError::Denied)
            );
        }
    }
    #[test]
    fn typed_guards_fail_closed() {
        let m = note_machine().unwrap();
        for note in [
            Note {
                id: "../x".into(),
                text: "ok".into(),
            },
            Note {
                id: "n".into(),
                text: "".into(),
            },
            Note {
                id: "n".into(),
                text: "x".repeat(4097),
            },
        ] {
            assert_eq!(
                m.evaluate("absent", "demo.note.create", &note),
                Err(TransitionError::GuardFailed)
            );
        }
    }
    #[test]
    fn string_length_is_utf8_bytes_not_unicode_characters() {
        let m = note_machine().unwrap();
        let at_limit = Note {
            id: "n".into(),
            text: "é".repeat(2048),
        };
        assert!(m.evaluate("absent", "demo.note.create", &at_limit).is_ok());
        let beyond = Note {
            id: "n".into(),
            text: "é".repeat(2049),
        };
        assert_eq!(
            m.evaluate("absent", "demo.note.create", &beyond),
            Err(TransitionError::GuardFailed)
        );
    }
    #[test]
    fn ambiguity_is_rejected_even_with_different_guards() {
        let mut i = ir();
        let mut t = i.transitions[0].clone();
        t.guards.clear();
        i.transitions.push(t);
        assert!(
            CompiledMachine::compile(i)
                .unwrap_err()
                .0
                .contains("ambiguous")
        );
    }
    #[test]
    fn malformed_states_guards_actions_and_terminal_edges_are_rejected() {
        type Invalidator = Box<dyn Fn(&mut MachineIr)>;
        let cases: Vec<Invalidator> = vec![
            Box::new(|i| i.schema_version = 2),
            Box::new(|i| i.states.push("absent".into())),
            Box::new(|i| i.initial_state = "unknown".into()),
            Box::new(|i| i.terminal_states.push("unknown".into())),
            Box::new(|i| i.transitions[0].from = "created".into()),
            Box::new(|i| i.transitions[0].to = "unknown".into()),
            Box::new(|i| i.transitions[0].actions.clear()),
            Box::new(|i| {
                i.transitions[0].actions[0] = Action::CreateObject {
                    object_type: "other.collection".into(),
                }
            }),
            Box::new(|i| {
                i.transitions[0].guards[0] = Guard::ValidIdentifier {
                    field: InputField::Text,
                }
            }),
            Box::new(|i| {
                i.transitions[0].guards[1] = Guard::StringLength {
                    field: InputField::Text,
                    min: 10,
                    max: 1,
                }
            }),
        ];
        for change in cases {
            let mut i = ir();
            change(&mut i);
            assert!(CompiledMachine::compile(i).is_err());
        }
    }
    #[test]
    fn unknown_fields_actions_and_null_are_not_executable() {
        let original = include_str!("../machines/demo-note.json");
        for source in [
            original.replace("\"create_object\"", "\"python_eval\""),
            original.replace(
                "\"schema_version\": 1",
                "\"schema_version\": 1, \"code\": \"print(1)\"",
            ),
            original.replace("\"input.id\"", "null"),
        ] {
            assert!(CompiledMachine::from_json(&source).is_err());
        }
    }
    #[test]
    fn invalid_input_cannot_produce_mutation_plan_and_reads_are_not_transitions() {
        let m = note_machine().unwrap();
        let writes = std::cell::Cell::new(0);
        for event in ["read", "query", "unknown"] {
            if m.evaluate("absent", event, &input()).is_ok() {
                writes.set(writes.get() + 1);
            }
        }
        assert_eq!(writes.get(), 0);
        assert_eq!(
            m.evaluate("absent", "demo.note.create", &input())
                .unwrap()
                .actions
                .len(),
            2
        );
    }
    #[test]
    fn tasks_have_exactly_three_states_two_edges_and_spanish_labels() {
        let m = task_machine().unwrap();
        assert_eq!(m.initial_state(), "pending");
        assert_eq!(m.ir().states, vec!["pending", "in_progress", "completed"]);
        assert_eq!(m.ir().transitions.len(), 2);
        assert_eq!(m.ir().state_labels["pending"], "Pendiente");
        let task_input = Note {
            id: "task-1".into(),
            text: String::new(),
        };
        let start = m.evaluate("pending", "tasks.start", &task_input).unwrap();
        assert_eq!(start.to_state, "in_progress");
        assert_eq!(
            start.actions[0],
            Action::SetState {
                state: "in_progress".into()
            }
        );
        let complete = m
            .evaluate("in_progress", "tasks.complete", &task_input)
            .unwrap();
        assert_eq!(complete.to_state, "completed");
        for (state, event) in [
            ("pending", "tasks.complete"),
            ("completed", "tasks.start"),
            ("in_progress", "tasks.start"),
            ("pending", "tasks.create"),
        ] {
            assert_eq!(
                m.evaluate(state, event, &task_input),
                Err(TransitionError::Denied)
            );
        }
    }
    #[test]
    fn json_and_toml_are_serializations_of_the_same_neutral_objects() {
        let json_ir: MachineIr =
            serde_json::from_str(include_str!("../machines/tasks.json")).unwrap();
        let toml_source = toml::to_string(&json_ir).unwrap();
        let toml_ir: MachineIr = toml::from_str(&toml_source).unwrap();
        assert_eq!(json_ir, toml_ir);
        let first = CompiledMachine::compile(json_ir).unwrap();
        let second = CompiledMachine::compile(toml_ir).unwrap();
        assert_eq!(
            first.evaluate("pending", "tasks.start", &input()),
            second.evaluate("pending", "tasks.start", &input())
        );
    }
    #[test]
    fn set_state_must_match_declared_target_and_labels_are_not_logic() {
        let mut ir = task_machine().unwrap().ir().clone();
        ir.transitions[0].actions[0] = Action::SetState {
            state: "completed".into(),
        };
        assert!(CompiledMachine::compile(ir).is_err());
        let mut ir = task_machine().unwrap().ir().clone();
        ir.state_labels
            .insert("unknown".into(), "Desconocido".into());
        assert!(CompiledMachine::compile(ir).is_err());
        let mut ir = task_machine().unwrap().ir().clone();
        ir.state_labels.insert("pending".into(), "Waiting".into());
        let translated = CompiledMachine::compile(ir).unwrap();
        assert_eq!(
            translated.evaluate("pending", "tasks.start", &input()),
            task_machine()
                .unwrap()
                .evaluate("pending", "tasks.start", &input())
        );
    }
    #[test]
    fn task_creation_is_explicit_but_not_a_transition() {
        let machine = task_machine().unwrap();
        let creation = machine.ir().creation.as_ref().unwrap();
        assert_eq!(creation.event, "tasks.create");
        assert_eq!(creation.initial_state, machine.initial_state());
        assert_eq!(
            creation.actions,
            vec![
                Action::CreateObject {
                    object_type: "tasks.task".into()
                },
                Action::EmitEvent {
                    event_name: "tasks.task.created".into()
                }
            ]
        );
        assert_eq!(
            machine.evaluate("pending", &creation.event, &input()),
            Err(TransitionError::Denied)
        );
        assert!(note_machine().unwrap().ir().creation.is_none());
    }
    #[test]
    fn invalid_creation_state_actions_and_event_collision_are_rejected() {
        type Change = Box<dyn Fn(&mut Creation)>;
        let changes: Vec<Change> = vec![
            Box::new(|c| c.initial_state = "completed".into()),
            Box::new(|c| c.event = "tasks.start".into()),
            Box::new(|c| c.actions.clear()),
            Box::new(|c| {
                c.actions[0] = Action::CreateObject {
                    object_type: "other".into(),
                }
            }),
            Box::new(|c| {
                c.actions[0] = Action::SetState {
                    state: "pending".into(),
                }
            }),
            Box::new(|c| {
                c.actions.push(Action::CreateObject {
                    object_type: "tasks.task".into(),
                })
            }),
            Box::new(|c| {
                c.actions[1] = Action::EmitEvent {
                    event_name: "invalid event".into(),
                }
            }),
        ];
        for change in changes {
            let mut ir = task_machine().unwrap().ir().clone();
            change(ir.creation.as_mut().unwrap());
            assert!(CompiledMachine::compile(ir).is_err());
        }
        let mut value = serde_json::to_value(task_machine().unwrap().ir()).unwrap();
        value["creation"]["python"] = serde_json::json!("print(1)");
        assert!(serde_json::from_value::<MachineIr>(value).is_err());
    }
    #[test]
    fn generated_toml_fixture_compiles_to_the_same_model_and_plan() {
        let json = CompiledMachine::from_json(include_str!("../machines/tasks.json")).unwrap();
        let toml =
            CompiledMachine::from_toml(include_str!("../machines/tasks.generated.toml")).unwrap();
        assert_eq!(json.ir(), toml.ir());
        for (state, event) in [
            ("pending", "tasks.start"),
            ("in_progress", "tasks.complete"),
            ("completed", "tasks.start"),
            ("pending", "tasks.unknown"),
        ] {
            assert_eq!(
                json.evaluate(state, event, &input()),
                toml.evaluate(state, event, &input())
            );
        }
    }
    #[test]
    fn toml_rejects_oversized_invalid_unknown_actions_and_targets() {
        assert!(CompiledMachine::from_toml(&" ".repeat(1024 * 1024 + 1)).is_err());
        assert!(CompiledMachine::from_toml("not valid TOML = [").is_err());
        let source = include_str!("../machines/tasks.generated.toml");
        assert!(
            CompiledMachine::from_toml(&source.replace("set_state", "execute_python")).is_err()
        );
        let mut ir = task_machine().unwrap().ir().clone();
        ir.transitions[0].actions[0] = Action::SetState {
            state: "unknown".into(),
        };
        assert!(CompiledMachine::from_toml(&toml::to_string(&ir).unwrap()).is_err());
        assert_eq!(
            task_machine()
                .unwrap()
                .evaluate("pending", "tasks.unknown", &input()),
            Err(TransitionError::Denied)
        );
    }
}
