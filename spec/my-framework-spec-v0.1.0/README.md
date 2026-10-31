# My freamework — especificaciones del proyecto

**Versión:** `0.1.0-draft.1` · **Fecha:** 22 de octubre de 2026 · **Idioma:** español.

Este paquete especifica un framework declarativo, orientado a eventos y con múltiples negocios y clientes. Es una base de diseño para implementar y revisar; no contiene un runtime desplegado ni adaptadores certificados.

El nombre del proyecto se conserva como fue indicado. El nombre público de los paquetes sigue abierto.

## Lectura recomendada

1. [Especificación principal](SPECIFICATION.md): alcance, arquitectura, reglas e invariantes.
2. [Decisiones y compatibilidad](DECISIONS.md): acuerdos previos, propuestas y decisiones pendientes.
3. [Requisitos](requirements.toml): requisitos identificados y criterios de aceptación.
4. [Contrato HTTP](api/openapi.yaml): protocolo asíncrono propuesto bajo `/v2`.
5. [Esquemas](schemas/contracts.schema.json): contratos JSON Schema 2020-12 para documentos y mensajes.
6. [Puertos](contracts/ports.md): interfaces independientes de lenguaje e infraestructura.
7. [Catálogo de mensajes](events/catalog.toml): comandos, eventos, consumidores y permisos.
8. [Ejemplos](examples/README.md): proyecto, entidades, objetos de valor, relaciones, políticas, vistas y configuración.
9. [Conformidad](conformance/scenarios.feature): escenarios para validar una implementación futura.
10. [Plan de implementación](ROADMAP.md): entregas ordenadas y condiciones de salida.

## Convenciones

**DEBE / NO DEBE** indican obligaciones de esta propuesta. **PUEDE** indica una opción compatible. Las decisiones marcadas como propuestas todavía no representan una aprobación explícita del propietario. Los ejemplos son normativos en estructura; los valores de capacidad y rendimiento son iniciales y revisables.

Los archivos `.toml` son la fuente de autoría predeterminada. `.toon` es una representación opcional sujeta a versión y conformidad. Los contratos HTTP y los sobres internos intercambian JSON. Los secretos se resuelven en el servidor mediante variables de entorno; `.env.example` solo contiene nombres y valores ilustrativos.

## Validación del paquete

Con Python 3.11 o posterior:

```bash
python -m pip install -r tools/requirements.txt
python tools/validate_spec.py
```

La validación comprueba sintaxis TOML/JSON/YAML, esquemas, referencias del contrato HTTP, ejemplos, correspondencia entre catálogos y trazabilidad de escenarios. No ejecuta el framework, no prueba servicios AWS y no certifica el parser completo de TOON. [VALIDATION.md](VALIDATION.md) registra el resultado real y sus límites.

## Antecedente

Se revisó el documento previo `multitenant_core.openapi.yaml`, fechado el 5 de octubre de 2026. La propuesta actual conserva separación de infraestructura, tipos dinámicos, objetos de valor inmutables, referencias tipadas y publicación coherente. Introduce comunicación entre handlers mediante buffer de mensajes, confirmación asíncrona, capacidades explícitas de motores, autorización multinegocio y una capa de sesión y ViewModel independiente del renderer. [DECISIONS.md](DECISIONS.md) detalla las incompatibilidades.
