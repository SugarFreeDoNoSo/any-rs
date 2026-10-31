# Fuentes técnicas y alcance

Fuentes primarias consultadas el 22 de octubre de 2026. Los enlaces documentan formatos y propiedades de servicios; la arquitectura y los defaults son propuestas propias del proyecto.

| Fuente | Uso en esta especificación |
| --- | --- |
| [CloudEvents 1.0.2](https://github.com/cloudevents/spec/blob/v1.0.2/cloudevents/spec.md) | Estructura de hechos y valor `specversion = "1.0"`; no define la semántica de nuestros comandos. |
| [TOML 1.0.0](https://toml.io/en/v1.0.0) | Perfil de autoría elegido y restricciones adicionales para normalizar a JSON. |
| [TOON: especificación oficial](https://github.com/toon-format/spec) | Codec opcional del modelo JSON. El repositorio consultado indica un working draft; el proyecto todavía debe fijar versión y parser. |
| [OpenAPI 3.1.0](https://spec.openapis.org/oas/v3.1.0) | Formato del contrato HTTP elegido por compatibilidad. |
| [JSON Schema 2020-12](https://json-schema.org/draft/2020-12/json-schema-core) | Formato de contratos estructurales; las restricciones entre recursos necesitan validación semántica aparte. |
| [RFC 8785: JCS](https://www.rfc-editor.org/rfc/rfc8785) | Canonicalización propuesta para hashes; no basta ordenar claves con un serializador JSON arbitrario. |
| [S3: escrituras condicionales](https://docs.aws.amazon.com/AmazonS3/latest/userguide/conditional-writes.html) | Precondiciones por objeto; no prueba un commit multiobjeto o una cola. |
| [Lambda y DynamoDB Streams](https://docs.aws.amazon.com/lambda/latest/dg/with-ddb.html) | Procesamiento al menos una vez y duplicados posibles; se requiere idempotencia. |
| [Documentación de htmx](https://htmx.org/docs/) | Flujo HTTP de fragmentos HTML; el estado de ViewModel y la autorización pertenecen al framework/BFF. |

Antecedente del proyecto: contenido actual de `multitenant_core.openapi.yaml`, fechado el 5 de octubre de 2026. Se revisaron sus contratos de tipos, operaciones, validación, snapshots y puertos. No se incorpora como una API ya desplegada ni se modifica el documento original.
