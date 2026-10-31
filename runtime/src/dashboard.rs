//! Read-only contract view. No registry, metrics, credential storage or JS/CDN.
use crate::domain::Scope;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn refs(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(name) = map
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|s| s.strip_prefix("#/components/schemas/"))
            {
                found.insert(name.into());
            }
            for child in map.values() {
                refs(child, found);
            }
        }
        Value::Array(values) => {
            for child in values {
                refs(child, found);
            }
        }
        _ => {}
    }
}
struct View {
    markdown: String,
    diagram: String,
    edges: Vec<(String, String)>,
    nodes: BTreeSet<String>,
}
fn generate(api: &Value, scope: &Scope) -> View {
    let schemas = &api["components"]["schemas"];
    let mut selected = BTreeSet::new();
    let mut pending: Vec<String> = [
        "CommandInput",
        "Accepted",
        "Operation",
        "ObjectDocument",
        "TaskCommand",
        "TaskObject",
        "TaskOperation",
    ]
    .iter()
    .map(|s| (*s).into())
    .collect();
    let mut edges = Vec::new();
    while let Some(name) = pending.pop() {
        if schemas.get(&name).is_none() || !selected.insert(name.clone()) {
            continue;
        }
        let mut children = BTreeSet::new();
        refs(&schemas[&name], &mut children);
        for child in children {
            edges.push((name.clone(), child.clone()));
            pending.push(child);
        }
    }
    edges.sort();
    let ids: BTreeMap<_, _> = selected
        .iter()
        .enumerate()
        .map(|(i, n)| (n.clone(), format!("s{i}")))
        .collect();
    let mut diagram = String::from("flowchart LR\n");
    for name in &selected {
        // Entity escaping protects Mermaid syntax if the checked-in contract changes.
        let label = name
            .replace('&', "#38;")
            .replace('"', "#34;")
            .replace('<', "#60;")
            .replace('>', "#62;")
            .replace('\n', " ");
        diagram.push_str(&format!("  {}[\"{}\"]\n", ids[name], label));
    }
    for (from, to) in &edges {
        diagram.push_str(&format!("  {} --> {}\n", ids[from], ids[to]));
    }
    let command = schemas["CommandInput"]["properties"]["name"]["const"]
        .as_str()
        .unwrap_or("no definido");
    let mut markdown = format!(
        "# any-rs · Contratos del runtime\n\nÁmbito autorizado: {} / {} / {}.\n\n## Configuración y esquema\n\nFuente: OpenAPI compilado y objetos FSM instalados, versión {}. No registro dinámico ni intérprete de código.\n\nComando instalado: {}.\n\n",
        scope.business_id,
        scope.project_id,
        scope.client_id,
        api["info"]["version"].as_str().unwrap_or("no definida"),
        command
    );
    if let Some(profile) = api.get("x-any-rs-runtime") {
        markdown.push_str("Metadatos del perfil implementado (x-any-rs-runtime). No implican entrega de eventos ni servicios AWS activos.\n\n");
        for field in ["command", "collection", "emitted_events"] {
            if let Some(value) = profile.get(field) {
                markdown.push_str(&format!("{}: {}\n\n", field, value));
            }
        }
    }
    if let Some(machines) = api["x-any-rs-machines"].as_array() {
        for (index, machine) in machines.iter().enumerate() {
            markdown.push_str(&format!("## Máquina {}\n\nColección: {}. Estado inicial: {}. Transiciones no declaradas: inválidas, sin efectos de dominio.\n\n",machine["id"].as_str().unwrap_or(""),machine["collection"].as_str().unwrap_or(""),machine["initial_state"].as_str().unwrap_or("")));
            if let Some(event) = machine["creation"]["event"].as_str() {
                markdown.push_str(&format!("Inicialización explícita: {} crea el objeto en {}. No añade una tercera transición.\n\n",event,machine["initial_state"].as_str().unwrap_or("")));
            }
            markdown.push_str("```mermaid\nstateDiagram-v2\n");
            if let Some(states) = machine["states"].as_array() {
                for (i, state) in states.iter().enumerate() {
                    let name = state.as_str().unwrap_or("");
                    let label = machine["state_labels"][name]
                        .as_str()
                        .unwrap_or(name)
                        .replace('"', "#34;");
                    markdown.push_str(&format!("    state \"{}\" as m{}_s{}\n", label, index, i));
                }
            }
            if let (Some(states), Some(transitions)) = (
                machine["states"].as_array(),
                machine["transitions"].as_array(),
            ) {
                for t in transitions {
                    let from = states.iter().position(|v| v == &t["from"]).unwrap_or(0);
                    let to = states.iter().position(|v| v == &t["to"]).unwrap_or(0);
                    markdown.push_str(&format!(
                        "    m{}_s{} --> m{}_s{}: {}\n",
                        index,
                        from,
                        index,
                        to,
                        t["event"].as_str().unwrap_or("")
                    ));
                }
            }
            markdown.push_str("```\n\nObjeto IR canónico:\n\n```json\n");
            markdown.push_str(&serde_json::to_string_pretty(machine).unwrap_or_default());
            markdown.push_str("\n```\n\n");
        }
    }
    markdown.push_str("## Estado actual\n\nSin consultas de datos ni contadores en esta vista. No se muestra estado de operaciones, salud de procesos, contenido de notas ni telemetría AWS. Los estados del esquema son posibilidades del contrato, no operaciones observadas; el slice local usa accepted, succeeded y rejected.\n\n## Endpoints del contrato\n\n");
    if let Some(paths) = api["paths"].as_object() {
        for (path, methods) in paths {
            for method in ["get", "post", "put", "patch", "delete"] {
                if let Some(op) = methods.get(method) {
                    markdown.push_str(&format!(
                        "{} {} — {}\n\n",
                        method.to_uppercase(),
                        path,
                        op["summary"].as_str().unwrap_or("")
                    ));
                }
            }
        }
    }
    markdown.push_str("## Grafo de referencias de esquemas\n\nLas flechas representan $ref, no ejecución ni llamadas directas.\n\n```mermaid\n");
    markdown.push_str(&diagram);
    markdown.push_str("```\n\n");
    for name in &selected {
        markdown.push_str(&format!(
            "## {}\n\n```json\n{}\n```\n\n",
            name,
            serde_json::to_string_pretty(&schemas[name]).unwrap_or_default()
        ));
    }
    View {
        markdown,
        diagram,
        edges,
        nodes: selected,
    }
}
/// Deterministic arc diagram from the exact nodes/edges used for Mermaid.
/// No Mermaid engine is included: this is a separate SVG rendering of that graph.
fn svg(view: &View) -> String {
    let height = view.nodes.len().max(1) * 64 + 40;
    let positions: BTreeMap<_, _> = view
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n, i * 64 + 40))
        .collect();
    let mut output = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="graph-title graph-desc" viewBox="0 0 900 {height}" style="width:100%;min-width:460px;max-height:none"><title id="graph-title">Referencias entre esquemas OpenAPI</title><desc id="graph-desc">Cada caja es un esquema. Las flechas indican referencias $ref. El listado adyacente contiene las mismas relaciones.</desc><defs><marker id="ref-arrow" markerWidth="10" markerHeight="10" refX="9" refY="5" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" fill="#8ee3cb"/></marker></defs>"##
    );
    for (index, (from, to)) in view.edges.iter().enumerate() {
        let a = positions[from];
        let b = positions[to];
        let reach = 300 + a.abs_diff(b).min(400) + (index % 5) * 28;
        output.push_str(&format!(r##"<path d="M 246 {a} C {reach} {a}, {reach} {b}, 246 {b}" fill="none" stroke="#8ee3cb" stroke-opacity="0.65" stroke-width="1.5" marker-end="url(#ref-arrow)"><title>{} → {}</title></path>"##,escape(from),escape(to)));
    }
    for name in &view.nodes {
        let y = positions[name];
        let top = y - 20;
        output.push_str(&format!(r##"<g><rect x="16" y="{top}" width="228" height="40" rx="6" fill="#1b2a34" stroke="#8ee3cb"/><text x="28" y="{}" fill="#edf4f5" font-family="monospace" font-size="15">{}</text></g>"##,y+5,escape(name)));
    }
    output.push_str("</svg>");
    output
}
/// Safe rendering of this generator's small Markdown subset: headings, paragraphs
/// and fenced code. Raw HTML, scripts, links and arbitrary Markdown are not enabled.
fn render_markdown(markdown: &str) -> String {
    let mut out = String::new();
    let mut code = false;
    for line in markdown.lines() {
        if line.starts_with("```") {
            if code {
                out.push_str("</code></pre>");
            } else {
                out.push_str("<pre><code>");
            }
            code = !code;
        } else if code {
            out.push_str(&escape(line));
            out.push('\n');
        } else if let Some(text) = line.strip_prefix("# ") {
            out.push_str(&format!("<h1>{}</h1>", escape(text)));
        } else if let Some(text) = line.strip_prefix("## ") {
            out.push_str(&format!("<h2>{}</h2>", escape(text)));
        } else if !line.is_empty() {
            out.push_str(&format!("<p>{}</p>", escape(line)));
        }
    }
    if code {
        out.push_str("</code></pre>");
    }
    out
}
pub(crate) fn markdown(scope: &Scope) -> Result<String, serde_json::Error> {
    let api: Value = crate::api::openapi_document();
    Ok(generate(&api, scope).markdown)
}
fn machine_svg(api: &Value) -> String {
    let mut html = String::new();
    if let Some(machines) = api["x-any-rs-machines"].as_array() {
        for (index, machine) in machines.iter().enumerate() {
            let Some(states) = machine["states"].as_array() else {
                continue;
            };
            let width = states.len().max(1) * 280;
            html.push_str(&format!("<section><h2>Máquina {}</h2><svg role=\"img\" aria-label=\"Estados y transiciones declaradas\" viewBox=\"0 0 {} 170\" style=\"width:100%\"><defs><marker id=\"task-arrow-{}\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"5\" orient=\"auto\"><path d=\"M0 0 L10 5 L0 10z\" fill=\"#8ee3cb\"/></marker></defs>",escape(machine["id"].as_str().unwrap_or("")),width,index));
            for (i, state) in states.iter().enumerate() {
                let state = state.as_str().unwrap_or("");
                let label = machine["state_labels"][state].as_str().unwrap_or(state);
                html.push_str(&format!("<rect x=\"{}\" y=\"60\" width=\"200\" height=\"70\" rx=\"12\" fill=\"#1b2a34\" stroke=\"#8ee3cb\"/><text x=\"{}\" y=\"100\" text-anchor=\"middle\" fill=\"#edf4f5\">{}</text>",i*280+20,i*280+120,escape(label)));
            }
            if let Some(transitions) = machine["transitions"].as_array() {
                for t in transitions {
                    let from = states.iter().position(|v| v == &t["from"]).unwrap_or(0);
                    let to = states.iter().position(|v| v == &t["to"]).unwrap_or(0);
                    html.push_str(&format!("<path d=\"M{} 95 L{} 95\" stroke=\"#8ee3cb\" marker-end=\"url(#task-arrow-{})\"/><text x=\"{}\" y=\"40\" text-anchor=\"middle\" fill=\"#edf4f5\" font-size=\"13\">{}</text>",from*280+220,to*280+20,index,from*280+260,escape(t["event"].as_str().unwrap_or(""))));
                }
            }
            html.push_str("</svg><p>Solo las flechas declaradas son válidas. La creación inicial es una operación explícita separada. Este diagrama describe el contrato, no instancias observadas.</p></section>");
        }
    }
    html
}
pub(crate) fn html(scope: &Scope) -> Result<String, serde_json::Error> {
    let api: Value = crate::api::openapi_document();
    let view = generate(&api, scope);
    let relations: String = view
        .edges
        .iter()
        .map(|(a, b)| {
            format!(
                "<li><code>{}</code> → <code>{}</code></li>",
                escape(a),
                escape(b)
            )
        })
        .collect();
    Ok(format!(
        r#"<!doctype html><html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>any-rs · Contratos</title><style>body{{font:16px system-ui,sans-serif;margin:0;background:#101820;color:#edf4f5}}main{{max-width:980px;margin:auto;padding:24px}}h1,h2{{color:#8ee3cb}}pre{{background:#1b2a34;padding:16px;overflow:auto;border-radius:8px}}p,li{{line-height:1.6;overflow-wrap:anywhere}}aside{{border:1px solid #8ee3cb;padding:16px}}a{{color:#8ee3cb}}details{{margin:20px 0}}</style></head><body><main><aside>Vista privada de contratos · Solo lectura · Sin credenciales ni métricas.<br>Autenticación Bearer requerida en cada petición; los enlaces no incorporan tokens.</aside><p><a href="{base}/schema.md">Fuente Markdown + Mermaid</a> · <a href="/openapi.json">OpenAPI</a></p><section><h2>Grafo del contrato</h2><p>SVG generado en el backend desde el mismo grafo de referencias que Mermaid; no usa un motor Mermaid.</p><div style="overflow:auto">{svg}</div></section><details><summary>Listado accesible (relaciones $ref)</summary><p>Sin JavaScript ni CDN: Mermaid se entrega como fuente, no como gráfico renderizado. Esta lista es la vista HTML alternativa del mismo grafo.</p><ul>{relations}</ul></details><details><summary>Fuente Mermaid</summary><pre><code>{diagram}</code></pre></details>{machines}{body}</main></body></html>"#,
        svg = svg(&view),
        base = escape(&scope.base()),
        diagram = escape(&view.diagram),
        machines = machine_svg(&api),
        body = render_markdown(&view.markdown)
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn scope() -> Scope {
        Scope {
            business_id: "b".into(),
            project_id: "p".into(),
            client_id: "c".into(),
        }
    }
    #[test]
    fn contracts_and_refs_are_generated_not_copied() {
        let mut api: Value = serde_json::from_str(include_str!("../openapi.json")).unwrap();
        let before = generate(&api, &scope());
        api["x-any-rs-runtime"] = serde_json::json!({"command":"fixture.command","collection":"fixture.collection","emitted_events":["fixture.event"]});
        let metadata = generate(&api, &scope());
        assert!(metadata.markdown.contains("fixture.event"));
        assert!(metadata.markdown.contains("fixture.collection"));
        assert!(before.diagram.contains("flowchart LR"));
        assert!(
            before
                .edges
                .contains(&("Operation".into(), "ResourceResult".into()))
        );
        api["components"]["schemas"]["CommandInput"]["properties"]["payload"]["properties"]["text"]
            ["maxLength"] = Value::from(123);
        let after = generate(&api, &scope());
        assert_ne!(before.markdown, after.markdown);
        assert!(after.markdown.contains("123"));
    }
    #[test]
    fn html_escapes_untrusted_markup_and_has_no_external_scripts() {
        let rendered =
            render_markdown("# <script>alert(1)</script>\n```mermaid\n</code><img src=x>\n```");
        assert!(!rendered.contains("<script>"));
        assert!(!rendered.contains("<img"));
        assert!(rendered.contains("&lt;script&gt;"));
        let page = html(&scope()).unwrap();
        assert!(page.contains("<svg"));
        assert!(page.contains("aria-labelledby=\"graph-title graph-desc\""));
        assert!(!page.contains("<script"));
        assert!(!page.contains("https://"));
    }
}
