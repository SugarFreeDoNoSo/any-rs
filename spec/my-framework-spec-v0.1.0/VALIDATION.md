# Validación del paquete

**Resultado:** satisfactorio. **Fecha:** 22 de octubre de 2026. **Versión:** `0.1.0-draft.1`.

Se ejecutó `python tools/validate_spec.py` con Python 3.12.14, PyYAML 6.0.3, jsonschema 4.25.1 y openapi-spec-validator 0.7.2. Las dependencias se instalaron de forma aislada para esta validación.

| Comprobación ejecutada | Resultado |
| --- | --- |
| Definiciones JSON Schema 2020-12 | 63, metaesquema válido. |
| Archivos de datos analizados | 18 TOML, 17 JSON y 1 YAML. |
| Instancias positivas | 31 aceptadas por su contrato. |
| Casos estructurales inválidos | 14 rechazados, incluidos principal inyectado, revisión ausente y resultado terminal incompleto. |
| Casos semánticos inválidos | 5 rechazados: referencia desconocida, cardinalidad inconsistente y 3 valores no portables. |
| Referencias locales de contratos | 321 resueltas sin depender de una URL remota. |
| OpenAPI 3.1.0 | Válido: 10 rutas y 12 operaciones. |
| Grafo del ejemplo | Referencias de tipos, handlers, mensajes, perfiles, vistas y templates comprobadas. |
| Trazabilidad | 30 requisitos con 30 escenarios especificados. |
| Enlaces locales de Markdown | 16 comprobados. |

La comparación del JSON de dinero con el TOML confirma que ambos describen el mismo árbol de datos. No se implementó JCS ni se probó su canonicalización; comparar árboles no certifica hashes interoperables.

No se ejecutaron los 30 escenarios contra un runtime, porque este paquete especifica la implementación futura. Tampoco se ejecutó un parser TOON, un build de SDK, un renderer, un driver SQLite del framework o un adaptador AWS. Los estados de certificación de motores permanecen como propuestas. La validez estructural y las comprobaciones semánticas estáticas no demuestran recuperación frente a caídas, aislamiento real ni rendimiento.

La verificación de patrones solo cubre su compilación y excluye algunas construcciones incompatibles; no es una certificación completa del subconjunto RE2. Las comprobaciones de permisos y perfiles verifican referencias del ejemplo; no ejecutan todavía el evaluador de autorización ni prueban su comportamiento.
