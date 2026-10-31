# Decisiones, antecedentes y compatibilidad

El estado **acordado** corresponde a instrucciones visibles del proyecto. **Propuesto** es una decisión nueva que concreta el diseño y puede revisarse. **Pendiente** requiere una elección posterior. Esta especificación no convierte las propuestas en aprobaciones del propietario.

| ID | Estado | Decisión | Motivo y consecuencia |
| --- | --- | --- | --- |
| ADR-001 | Acordado | Handlers coordinados por mensajes mediante buffer. | Un handler no llama a otro handler; utilidades puras y puertos siguen siendo llamadas normales. |
| ADR-002 | Acordado | Core separado de infraestructura y lenguaje de implementación. | Los contratos definen datos y comportamiento, sin exigir Rust o AWS. |
| ADR-003 | Acordado | TOML para configuración y `.env` para credenciales. | Los secretos se resuelven en servidor y no forman parte del bundle público. |
| ADR-004 | Acordado | Entidades, vistas y eventos declarativos en TOML o TOON. | El soporte TOON completo requiere fijar un codec y versión. |
| ADR-005 | Acordado | Motores con roles y compatibilidad explícita. | Persistir datos no basta para certificar un motor como cola. |
| ADR-006 | Acordado | Múltiples negocios, clientes, usuarios y perfiles extensibles. | Negocio es tenant; cliente y usuario son entidades distintas. |
| ADR-007 | Acordado | Sesión y MVVM con frontend opcional. | HTML/htmx y consumidores de SDK comparten contratos; no se impone un renderer. |
| ADR-008 | Propuesto | Distinguir comandos de hechos confirmados. | Evita publicar una intención como si ya hubiera sucedido. |
| ADR-009 | Propuesto | CloudEvents 1.0 para eventos y JSON para transporte. | Formato portable; `specversion` vale `1.0`, independiente de la versión del dominio. |
| ADR-010 | Propuesto | Aceptación HTTP asíncrona bajo `/v2`. | `202` garantiza aceptación durable; la operación informa éxito o rechazo posterior. |
| ADR-011 | Propuesto | Estado más inbox, outbox y operación atómicos. | Evita perder eventos después del commit y tolera reentregas. No equivale a efectos externos exactamente una vez. |
| ADR-012 | Propuesto | RBAC más condiciones de atributos, denegación por defecto. | Los perfiles crecen sin depender de niveles enteros universales. |
| ADR-013 | Propuesto | Ámbito de datos obligatorio negocio/proyecto/cliente. | Clientes sin valor no actúan como comodín; administración tiene ámbitos propios. |
| ADR-014 | Propuesto | Entidades con referencias, VOs inmutables y relaciones explícitas. | Conserva el diseño previo; no se afirma 6FN sin analizar dependencias funcionales. |
| ADR-015 | Propuesto | SQLite como primera referencia local. | Facilita probar atomicidad y recuperación con un solo motor; aún no está implementado. |
| ADR-016 | Propuesto | TOML 1.0.0 y JSON Schema 2020-12 como perfiles iniciales. | Busca compatibilidad y validación; no se afirma que sean versiones más recientes. |
| ADR-017 | Propuesto | Releases coherentes y reglas sin ejecución arbitraria. | Publicación completa, referencias resueltas y handlers de código confiable. |
| ADR-018 | Propuesto | SDK ESM con módulos públicos y de servidor separados. | Vite, Astro, Preact y React pueden consumirlo sin introducir credenciales al navegador. |

## Cambios respecto de `multitenant_core.openapi.yaml`

Se revisó el antecedente actual de 2099 líneas, fechado el 5 de octubre de 2026. No se modifica ni se reemplaza ese archivo; el paquete define una evolución nueva.

| Aspecto | Antecedente | Propuesta |
| --- | --- | --- |
| Ruta | `/v1/{type}/{request}` con ámbito por headers. | `/v2/businesses/{business}/projects/{project}/clients/{client}/...`. |
| Escritura | Respuesta final `201/200` tras publicar snapshot. | `202` con operación; el resultado del worker se consulta después. |
| Ámbito | Tenant y proyecto. | Negocio/tenant, proyecto y cliente explícitos para datos. |
| Coordinación | Tareas internas y publicación de snapshots. | Buffer, suscripciones y commits con inbox/outbox. |
| Definiciones | Tipos y reglas JSON versionadas. | Documentos TOML/TOON compilados a IR y releases coherentes. |
| Tipos | Entity, value_object, relation. | Se conserva la semántica propuesta y se añaden vistas, mensajes, perfiles y bindings. |
| Consistencia | Snapshot publicado como confirmación. | Commit autoritativo más watermark de proyección; lectura mínima explícita. |
| Frontend | Fuera del contrato central. | Sesión pública, ViewModel, SDK y renderer/BFF opcional. |

Estos cambios son incompatibles en transporte y tiempos de confirmación. Un adaptador de migración tendría que esperar una operación antes de devolver el resultado antiguo y traducir ámbitos, errores y precondiciones; no basta renombrar rutas. No se ofrece compatibilidad automática ni un adaptador implementado.

## Decisiones pendientes y defaults para avanzar

| Pregunta | Default de trabajo | Condición de revisión |
| --- | --- | --- |
| Nombre y licencia | Nombre de proyecto conservado; licencia sin asignar. | Antes de publicar código o paquetes. |
| Runtime inicial | Un proceso local; contratos independientes de lenguaje. | Elegir lenguaje antes de implementar M1. |
| Primer despliegue | Local antes de Lambda. | Después de conformidad de fallos y benchmark. |
| Identidad | Puerto OIDC/JWT o sesión verificada por adaptador. | Seleccionar proveedor y protocolo de login en M2. |
| TOON | Extensión opcional deshabilitada; ejemplo ilustrativo. | Fijar versión y superar fixtures oficiales en M5. |
| Primer renderer | Ciclo ViewModel en SDK y BFF HTML opcional. | Prioridad de producto antes de M3. |
| Adaptador cloud | Ninguno certificado. | Elegir con carga, límites y costos medidos. |
| Objetivos de rendimiento | Sin SLO numérico. | Establecer carga representativa y medir M4. |
| Blobs | Puerto opcional y referencias autorizadas. | Definir tamaño, retención y acceso antes del adaptador. |

## Consistencia de autorización

El release fija esquemas y comportamiento de un comando. La autorización usa las políticas y membresías actuales, no permisos congelados en el release antiguo. Una revocación impide las siguientes decisiones de autorización; una acción que ya obtuvo una decisión válida y confirma de forma concurrente puede terminar. Una revocación linealizable respecto de commits exigiría un perfil adicional de coordinación y no se promete aquí. La implementación inicial deshabilita caché de permisos para acotar el problema.

El despliegue de código, el bootstrap del propietario y las migraciones destructivas requieren procesos confiables específicos; su diseño no se deduce de publicar un archivo de perfil.
