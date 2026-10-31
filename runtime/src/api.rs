//! OpenAPI and introspection views derived from the installed neutral FSM object.
use serde_json::{Value, json};
fn parameter(name: &str) -> Value {
    json!({"name":name,"in":"path","required":true,"schema":{"$ref":"#/components/schemas/Identifier"}})
}
pub fn openapi_document() -> Value {
    let mut api: Value =
        serde_json::from_str(include_str!("../openapi.json")).expect("checked-in OpenAPI");
    let machine = crate::fsm::task_machine().expect("validated task IR");
    let ir = serde_json::to_value(machine.ir()).expect("serializable IR");
    api["x-any-rs-machines"] = json!([ir]);
    let states: Vec<_> = machine.ir().states.iter().map(|s| json!(s)).collect();
    let mut commands = Vec::new();
    // Creation is explicit and separate from the two state transitions.
    let create_name = ir["creation"]["event"]
        .as_str()
        .expect("validated creation descriptor");
    commands.push(json!({"type":"object","additionalProperties":false,"required":["name","version","payload"],"properties":{"name":{"const":create_name},"version":{"const":1},"payload":{"type":"object","additionalProperties":false,"required":["id","title"],"properties":{"id":{"$ref":"#/components/schemas/Identifier"},"title":{"type":"string","minLength":1,"maxLength":4096,"pattern":r"\S","x-max-utf8-bytes":4096,"description":"Nonblank; also limited to 4096 UTF-8 bytes by the runtime"}}}}}));
    for t in &machine.ir().transitions {
        commands.push(json!({"type":"object","additionalProperties":false,"required":["name","version","payload"],"properties":{"name":{"const":t.event},"version":{"const":1},"payload":{"type":"object","additionalProperties":false,"required":["id","expected_revision"],"properties":{"id":{"$ref":"#/components/schemas/Identifier"},"expected_revision":{"type":"integer","minimum":1,"maximum":crate::tasks::MAX_REVISION}}}}}));
    }
    api["components"]["schemas"]["TaskCommand"] = json!({"oneOf":commands});
    api["components"]["schemas"]["TaskState"] =
        json!({"type":"string","enum":states,"x-state-labels":machine.ir().state_labels});
    api["components"]["schemas"]["TaskObject"] = json!({"type":"object","required":["scope","type","id","state","revision","values","commit_id"],"properties":{"scope":{"$ref":"#/components/schemas/Scope"},"type":{"const":machine.collection()},"id":{"$ref":"#/components/schemas/Identifier"},"state":{"$ref":"#/components/schemas/TaskState"},"revision":{"type":"integer","minimum":1},"schema_version":{"const":1},"commit_id":{"type":"string"},"values":{"type":"object","required":["title"],"properties":{"title":{"type":"string"}}}}});
    api["components"]["schemas"]["TaskOperation"] = json!({"type":"object","required":["operation_id","scope","command_name","status"],"properties":{"operation_id":{"type":"string"},"scope":{"$ref":"#/components/schemas/Scope"},"command_name":{"type":"string"},"status":{"enum":["accepted","succeeded","rejected"]},"result":{"type":"object"},"problem":{"$ref":"#/components/schemas/Problem"}}});
    let base = "/businesses/{business_id}/projects/{project_id}/clients/{client_id}/tasks";
    for (suffix, method, output, summary) in [
        (
            "/commands",
            "post",
            "Accepted",
            "Accept task creation or an explicitly declared FSM transition",
        ),
        (
            "/operations/{id}",
            "get",
            "TaskOperation",
            "Read private task operation",
        ),
        (
            "/objects/{id}",
            "get",
            "TaskObject",
            "Read scoped task state; reading is not a transition",
        ),
    ] {
        let mut parameters = vec![
            parameter("business_id"),
            parameter("project_id"),
            parameter("client_id"),
        ];
        if suffix.contains("{id}") {
            parameters.push(parameter("id"));
        }
        let status = if method == "post" { "202" } else { "200" };
        let mut operation = json!({"summary":summary,"responses":{status:{"description":"Success","content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{output}")}}}},"400":{"description":"Malformed input"},"401":{"description":"Missing or invalid credential"},"403":{"description":"Scope denied"},"404":{"description":"Not found"},"409":{"description":"Idempotency conflict"},"422":{"description":"Unsupported command or payload"},"503":{"description":"Store unavailable"}}});
        if method == "post" {
            parameters.push(json!({"name":"Idempotency-Key","in":"header","required":true,"schema":{"type":"string","minLength":1,"maxLength":128}}));
            operation["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/TaskCommand"}}}});
        }
        api["paths"][format!("{base}{suffix}")] = json!({"parameters":parameters,method:operation});
    }
    api
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn states_events_and_labels_come_from_the_installed_machine() {
        let api = openapi_document();
        let machine = crate::fsm::task_machine().unwrap();
        assert_eq!(
            api["components"]["schemas"]["TaskState"]["enum"],
            json!(machine.ir().states)
        );
        assert_eq!(
            api["x-any-rs-machines"][0],
            serde_json::to_value(machine.ir()).unwrap()
        );
        assert_eq!(
            api["components"]["schemas"]["TaskCommand"]["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
}
