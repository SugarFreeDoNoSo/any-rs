# Puertos del core

Los nombres expresan contratos lógicos. No fijan sintaxis Rust/TypeScript ni un ABI. Los parámetros siempre incluyen contexto de ámbito, identidad de servicio, deadline y presupuesto. Ningún valor opaco recibido del cliente concede permisos.

| Puerto | Operaciones mínimas | Contrato observable |
| --- | --- | --- |
| DefinitionCompiler | `parse(codec,bytes)`, `normalize(document)`, `validate(graph)`, `plan(bindings)` | Sin efectos de dominio; errores con código y ubicación; release determinista. |
| DefinitionRegistry | `get(release,id,version)`, `publish(expected_revision,release)` | Releases inmutables y publicación coherente; no evolución parcial. |
| IdentityProvider | `verify(credential)`, `resolve_membership(principal,scope)` | Verifica emisor, audiencia, expiración y pertenencia; no confía en IDs del body. |
| PolicyEngine | `decide(principal,action,resource,scope,current_revisions)` | Denegar por defecto; deny prevalece; decisión con versión y explicación sanitizada. |
| CommandIngress | `accept(request,principal,scope,idempotency_key)` | Persiste comando, operación e idempotencia antes de devolver aceptación; mismos bytes semánticos devuelven la misma operación. |
| StateReader | `get(scope,type,id,min_commit?)`, `query(scope,plan,cursor?)` | Estado coherente, ámbito verificado, revisión y commit identificables. |
| CommitCoordinator | `commit(read_set,changes,inbox,outbox,operation,fencing)` | Todo o nada dentro de una unidad soportada; revalida revisiones, unicidad, referencias y cardinalidad. |
| MessageBuffer | `publish(record)`, `receive(subscription,lease)`, `extend(token)`, `ack(token)`, `nack(token,delay)` | Durabilidad declarada; ACK y extensión condicionados al token de entrega; lease y fencing verificables. |
| SubscriptionRegistry | `bind(definition)`, `resolve(event,release)` | Fan-out independiente; persistencia o reconstrucción idempotente de todas las entregas. |
| OutboxRelay | `scan_pending`, `publish_stable`, `mark_sent` | Recuperable; identidad estable; marca solo después de aceptación durable aguas abajo. |
| OperationStore | `get(scope,operation_id)`, actualización mediante commit | Monotonía de estados terminales; resultado y problemas consultables por el actor autorizado. |
| ProjectionStore | `apply(event,expected_revision)`, `watermark`, `rebuild(generation,range)` | No omite huecos de revisión; replay sin efectos externos. |
| BlobStore | `put(bytes,metadata)`, `get(ref,authorization)`, `delete(ref,precondition)` | Bytes durables y checksum independiente del ETag del proveedor; retención y tamaño declarados. |
| ResultCache | `get(auth_scope,query,revision)`, `put`, `invalidate` | No evita autorización; separa tenants, clientes y vistas por permisos. |
| SessionRuntime | `public_context`, `create_view`, `patch_view`, `dispatch`, `observe_operation` | Contexto autenticado inmutable para el cliente; estado de edición separado de datos confirmados. |
| HandlerRuntime | `invoke(handler,message,read_snapshot,context)` | Devuelve propuesta; no llama a otros handlers ni confirma por fuera del coordinator. |
| IntegrationEffect | `prepare(intent)`, `execute(stable_key)`, `reconcile(key)` | Resultado `confirmed`, `failed` o `unknown`; no transforma un timeout en fallo seguro. |
| Clock / IdGenerator | `now()`, `new_id(namespace)` | Dependencias explícitas para reproducir reglas y probar fallos. |
| Telemetry | `emit(metric_or_span)` | Sin secretos, con correlación; fallo de métricas no revierte un commit confirmado. |

## Resultados y errores comunes

Los puertos devuelven resultado tipado o `ValidationError`, `Unauthorized`, `Forbidden`, `NotFound`, `Conflict`, `CapabilityMismatch`, `TransientFailure`, `DeadlineExceeded` o `UnknownOutcome`. El gateway traduce errores antes de la aceptación; el worker registra resultados después de ella. No se usa una excepción genérica para decidir que cualquier operación debe reintentarse.

`commit` devuelve `commit_id` opaco, revisiones resultantes y referencias a objetos/definiciones. Un fallo de red tras enviar el commit puede dejar su resultado desconocido: el runtime consulta inbox y operación antes de ejecutarlo otra vez.

El parámetro de operación en `commit` es obligatorio para el dueño de un comando y está ausente para un suscriptor de eventos que solo mantiene su proyección o efecto. Un suscriptor conserva la correlación, pero no reabre una operación de origen ya terminal.

## Bindings y perfiles

Los bindings pertenecen al despliegue, no al dominio. La semántica de los perfiles está en SPECIFICATION §9. Una implementación de `MessageBuffer` que no dispone de fan-out no puede presentarse como pub/sub completo sin un adaptador adicional. Un motor con `conditional_write` por objeto no demuestra por sí solo atomicidad de estado, inbox y outbox.

Un driver declara alcance transaccional, límites de payload, retención, aislamiento, operaciones condicionales, recuperación y comportamiento de leases. El planner valida un binding de extremo a extremo, incluidas capacidades aportadas por un coordinador. No acepta listas de capacidades autodeclaradas sin las pruebas de conformidad correspondientes.
