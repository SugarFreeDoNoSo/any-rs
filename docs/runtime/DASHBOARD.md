# Vista de contratos del runtime

Rutas autenticadas con el mismo Bearer y ámbito del API:
- `GET /v2/businesses/{b}/projects/{p}/clients/{c}/dashboard`: HTML responsive.
- `GET /v2/businesses/{b}/projects/{p}/clients/{c}/schema.md`: Markdown con Mermaid.

Se genera desde `runtime/openapi.json` incorporado al compilar: endpoints, CommandInput,
Accepted, Operation, ObjectDocument y sus referencias transitivas. Un cambio del contrato
requiere recompilar; no existe registro dinámico. La relación demo.note.create → operación
→ demo.note / demo.note.created se identifica explícitamente como perfil local, no como
schema de evento OpenAPI ni prueba de entrega AWS. Las flechas Mermaid significan `$ref`.

El HTML renderiza un subconjunto seguro del Markdown generado. No carga JavaScript ni CDN;
el Mermaid está disponible como fuente y un listado de aristas ofrece la vista HTML
alternativa del mismo grafo. No se afirma renderizado Mermaid visual. El CSP impide scripts,
frames y recursos externos. Las respuestas son private/no-store y nosniff.

No hay consultas a notas, métricas, contadores ni telemetría: configuración/esquemas se
separan explícitamente del estado actual. Se autentica antes de generar contenido; no se
incluyen tokens ni nombres de variables de credenciales. OpenAPI conserva su ruta pública.
Los enlaces requieren que el cliente adjunte Authorization de nuevo; un navegador sin
cliente autenticado responderá 401. No se añaden sesiones, login ni tokens en URL.

Validación: unit tests de generación desde contrato mutado, grafo y escape de HTML;
verificar HTTP 401 sin token, 403 con otro tenant, 200 con principal autorizado,
content-types/no-store/CSP y ausencia de credenciales. No se despliega un Site externo.

## Grafo visual sin dependencias

La página incluye además un SVG inline accesible generado en el backend desde los mismos
nodos y aristas `$ref` que la fuente Mermaid. Las cajas y curvas usan coordenadas
deterministas; texto escapado, título/descripción y listado alternativo. No es un motor
Mermaid: el SVG es otra representación del mismo grafo. Se mantiene la fuente exportable.
Los metadatos de comando, colección y eventos se leen de `x-any-rs-runtime` en OpenAPI;
no se mantiene un catálogo manual paralelo ni se anuncian eventos realmente entregados.

## Tasks FSM increment

`GET /openapi.json` now combines the base template with the installed neutral tasks
IR. Task state enums, creation/transition event names and labels come from that
IR. The dashboard includes a separate state-machine SVG and Mermaid source,
plus the full declarative object. It never infers transition rules from observed
instances. The schema reference graph and FSM diagram are distinct.
