# Plan de implementación

Estado real al crear este paquete: documentación y contratos iniciales; runtime y drivers pendientes.

| Etapa | Entrega | Condición de salida |
| --- | --- | --- |
| M0 · Especificación | Contratos, ejemplos, requisitos y decisiones. | Validación documental sin errores; revisar propuestas y pendientes. |
| M1 · Compilador y core local | Parser TOML, IR, grafo de definiciones, reglas y planner de capacidades. | Ejemplos compilan; errores y referencias inválidas se rechazan; hashing determinista. |
| M2 · Runtime durable y acceso | SQLite, ingreso, workers, inbox/outbox, operaciones, revisión y autorización. | Caídas, duplicados, fan-out, conflictos, fencing y aislamiento pasan conformidad. |
| M3 · SDK y vista | Cliente ESM, ViewModel, BFF HTML/htmx y un consumidor sin renderer impuesto. | Crear, editar, esperar y resolver conflictos; bundle público sin drivers o secretos de servidor. |
| M4 · Primer despliegue remoto | Un adaptador cloud y una estrategia de recuperación. | Misma conformidad; límites reales documentados; benchmark reproducible y restauración ensayada. |
| M5 · Extensiones | TOON certificado, otros motores, renderers, replay y blobs. | Versión/fixtures fijados y garantías probadas por cada perfil. |

M1 requiere elegir el lenguaje del runtime. M2 requiere resolver autenticación de referencia y bootstrap de propietario. M3 requiere seleccionar el primer renderer o BFF que se desee priorizar. M4 requiere elegir proveedor, región, límites de gasto y carga representativa. Estas elecciones no bloquean la revisión de contratos.

No se incluye como hecho terminado ningún hito de ejecución. Los escenarios del paquete se convierten en pruebas ejecutables cuando exista una implementación; no son evidencias de que ya se haya construido.
