# Ejemplo de proyecto de ventas

Los archivos describen una aplicación ilustrativa; todavía no son ejecutables mediante un CLI del framework. `project.toml` referencia definiciones de dominio y bindings propuestos. `engines/sqlite.toml` describe un driver que aún debe implementarse, por eso indica `certification = "proposed"`.

## Grafo de tipos

`core.display_name` y `sales.money` son objetos de valor. `sales.customer` referencia un nombre y `sales.order` referencia un monto. `sales.customer_order` enlaza clientes comerciales de la aplicación con pedidos. Ese tipo de dominio es distinto de `client_acme`, que es el ámbito de acceso de la plataforma.

Antes del comando de ejemplo `core.object.create` de un pedido debe existir `money_001`. Puede crearse con el mismo protocolo usando el tipo `sales.money` y valores `{ "amount": "15990.00", "currency": "CLP" }`. El pedido guarda una referencia tipada, no esos escalares directamente. `money_002` del ejemplo de actualización representa otro VO preexistente.

## Flujo ilustrativo

1. Compilar y publicar las definiciones con un principal autorizado para el registro.
2. Crear la membresía y perfiles por un proceso de administración confiable.
3. Crear el VO de dinero y esperar su operación confirmada.
4. Enviar `messages/create-object.request.json` al endpoint de `sales.order/objects` con una clave de idempotencia.
5. Consultar la operación; `202` por sí mismo no confirma el pedido.
6. Leer el pedido autoritativo con el `commit_id` mínimo de la operación.
7. Un suscriptor independiente actualiza la proyección; su watermark puede retrasarse.

Los archivos `messages/command.json` y `messages/object-created.event.json` son sobres internos creados por infraestructura. No deben aceptarse como credenciales o hechos confiables desde un navegador. Los archivos de sesión y membresía ilustran identidades y ámbitos; no constituyen sesiones o usuarios reales.

## TOML y TOON

La definición de dinero en `definitions/money.toml` tiene un ejemplo TOON y un JSON equivalente bajo `codecs/`. Se valida el JSON equivalente y la definición TOML; no se ha ejecutado un parser TOON. La extensión seguirá deshabilitada hasta fijar su versión y pasar sus fixtures.

## Templates y SDK

`templates/orders.html` es una plantilla de referencia para un BFF optativo. Sus marcadores `{{ ... }}` requieren un renderer con escapado y soporte de esas expresiones. No es una página desplegada. El BFF prepara IDs, URLs codificadas, token CSRF y fragmentos; el template no maneja credenciales del gateway. Los nombres de módulos del SDK en la especificación son una distribución propuesta, todavía sin paquetes publicados.

`bindings/s3-blob.toml` ilustra otro rol de motor con referencias a entorno. No se activa en el proyecto local ni certifica un driver; no convertirlo en cola sin un adaptador conforme.
