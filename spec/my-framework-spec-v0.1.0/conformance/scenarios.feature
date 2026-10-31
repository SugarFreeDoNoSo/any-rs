# language: es
Característica: Conformidad del framework orientado a eventos
  Estos escenarios son contratos para una implementación futura.
  No se han ejecutado contra un runtime del framework.

  @REQ-001
  Escenario: Coordinar dos handlers desacoplados
    Dado un comando aceptado que procesa el handler A
    Cuando A confirma su estado y emite un evento
    Entonces el buffer entrega el evento a B mediante su suscripción
    Y A no conoce ni invoca el entrypoint de B

  @REQ-002
  Escenario: Sustituir infraestructura conservando semántica
    Dado un proyecto compilado contra puertos del core
    Cuando se configura otro adaptador con el mismo perfil certificado
    Entonces las definiciones de dominio no requieren cambios
    Y se vuelve a ejecutar la suite de conformidad del adaptador

  @REQ-003
  Escenario: Resolver una credencial sin exponerla
    Dado un binding que referencia una variable presente en el entorno y en .env
    Cuando se resuelve el binding y se genera un bundle público
    Entonces prevalece el valor del entorno de proceso
    Y el secreto no aparece en el bundle, errores ni logs

  @REQ-004
  Escenario: Compilar definiciones declarativas sin código arbitrario
    Dado un tipo, un evento y una vista expresados como TOML
    Cuando se compilan sus definiciones
    Entonces producen documentos del modelo común
    Y no se evalúan scripts incluidos por un usuario
    Y habilitar TOON requiere un parser versionado que supere fixtures oficiales

  @REQ-005
  Escenario: Aislar datos, referencias y entregas entre clientes
    Dado un usuario autorizado solo para client_acme
    Cuando solicita un objeto, un cursor o una operación de client_other
    Entonces no recibe contenido ni confirmación de existencia del recurso ajeno
    Y no puede crear una referencia entre ambos ámbitos

  @REQ-006
  Escenario: Cambiar de negocio mediante otra membresía
    Dado un usuario con una membresía activa en el negocio A
    Cuando solicita activar el negocio B sin membresía válida
    Entonces el cambio de contexto se deniega
    Y editar el identificador público de sesión no crea una membresía

  @REQ-007
  Escenario: Una denegación prevalece sobre un permiso
    Dado un perfil que incluye object.update y una regla allow aplicable
    Y una regla deny aplicable al mismo recurso
    Cuando se evalúa object.update
    Entonces se deniega la acción aunque el perfil la incluya

  @REQ-008
  Escenario: Evitar escalada mediante perfiles y políticas dinámicas
    Dado un administrador cuya frontera de delegación no incluye definition.publish
    Cuando intenta asignarse esa acción mediante un perfil o una política nueva
    Entonces la modificación se rechaza y se audita

  @REQ-009
  Escenario: Rechazar un comando revocado mientras esperaba
    Dado un comando aceptado y aún sin evaluación del worker
    Y una revocación de membresía confirmada antes de esa evaluación
    Cuando el worker verifica la autorización actual
    Entonces la operación termina en rejected
    Y una nueva lectura o suscripción también se deniega

  @REQ-010
  Escenario: Rechazar una cola sin coordinación suficiente
    Dado un motor que solo declara lectura y escritura durable
    Cuando un binding solicita leases, fencing y fan-out independiente
    Entonces el planner rechaza el binding con las capacidades faltantes
    Y no inicia el runtime con garantías reducidas

  @REQ-011
  Escenario: Recuperar un comando después de responder aceptado
    Dado un ingreso que respondió 202 después de persistir su unidad de aceptación
    Cuando se interrumpe el proceso antes de publicar hacia la cola
    Entonces tras reiniciar la misma operación sigue consultable
    Y el relay recupera el comando pendiente sin generar otra identidad

  @REQ-012
  Escenario: Caer después del commit y antes del ACK
    Dado un handler que confirmó estado, inbox, outbox y operación
    Cuando el proceso cae antes del ACK y la entrega vuelve a aparecer
    Entonces inbox reconoce la misma entrega lógica
    Y no hay un segundo cambio de dominio ni un nuevo evento con otro ID
    Y el relay termina la publicación pendiente

  @REQ-013
  Escenario: Mantener fan-out independiente y tolerar duplicados
    Dado un evento con suscripciones A y B
    Cuando A confirma y B falla temporalmente
    Entonces B puede volver a procesar su entrega
    Y la deduplicación de A no elimina la entrega de B

  @REQ-014
  Escenario: Resolver una carrera y excluir a un worker vencido
    Dado dos workers que leyeron la misma revisión y tokens sucesivos
    Cuando el token anterior expira y ambos intentan confirmar
    Entonces se rechaza el worker con fencing vencido
    Y dos comandos con la misma expected_revision no se confirman ambos

  @REQ-015
  Escenario: Preservar una entrega al agotar sus reintentos
    Dado una entrega con fallos transitorios hasta agotar su presupuesto
    Cuando se alcanza el máximo de intentos
    Entonces la entrega queda en dead-letter durable con diagnóstico sanitizado
    Y redirigirla conserva su identidad y genera una entrada de auditoría

  @REQ-016
  Escenario: Preservar invariantes del grafo bajo concurrencia
    Dado una relación con un máximo de un cliente por pedido
    Cuando dos comandos enlazan el mismo pedido a distintos clientes a la vez
    Entonces solo un enlace puede confirmarse
    Y no se permite actualizar un VO ni enlazar un tipo incompatible

  @REQ-017
  Escenario: Crear un tipo con primera instancia de forma autorizada
    Dado un tipo desconocido y un binding registry_data_atomic
    Cuando llega core.object.create con esquema explícito y permiso de registro
    Entonces esquema e instancia se publican juntos o ninguno se publica
    Y sin esquema, permiso o perfil requerido la creación se rechaza

  @REQ-018
  Escenario: Rechazar un release parcialmente inválido
    Dado el release vigente y otro que contiene una referencia no resuelta
    Cuando se solicita publicar el nuevo release
    Entonces el vigente permanece íntegro
    Y una publicación válida conserva versiones usadas por operaciones pendientes

  @REQ-019
  Escenario: Distinguir aceptación de resultado de dominio
    Dado un comando aceptado con 202
    Cuando el worker detecta una violación de cardinalidad
    Entonces GET de la operación devuelve HTTP 200 con status rejected y problem
    Y el cliente no interpreta el 202 original como éxito de dominio

  @REQ-020
  Escenario: Reutilizar una clave de idempotencia
    Dado una solicitud aceptada con una clave aún retenida
    Cuando se repite la misma solicitud normalizada
    Entonces se devuelve la misma operación sin otro comando de dominio
    Y cambiar el contenido conservando la clave devuelve 409

  @REQ-021
  Escenario: Exigir una lectura mínima y proteger el cursor
    Dado una proyección con watermark anterior al commit solicitado
    Cuando se pide min_commit y el presupuesto de espera se agota
    Entonces la consulta devuelve 503 en lugar de datos presentados como actuales
    Y un cursor de otra consulta o ámbito es rechazado
    Y un snapshot retirado devuelve 410

  @REQ-022
  Escenario: Editar una vista sin modificar autoridad
    Dado un ViewModel editable con una revisión base
    Cuando se cambia una ruta permitida de draft
    Entonces el estado pasa a editing y conserva el dato confirmado
    Y un patch sobre principal, ámbito o perfiles se rechaza
    Y un conflicto conserva el borrador del usuario

  @REQ-023
  Escenario: Usar un consumidor headless y un build navegador
    Dado el mismo contrato de comandos consumido por backend y navegador
    Cuando se produce el bundle público y se inspeccionan sus imports
    Entonces el consumidor no necesita un renderer específico
    Y el bundle no contiene credenciales ni drivers de servidor

  @REQ-024
  Escenario: Mostrar una operación con fragmentos HTML
    Dado un formulario htmx autenticado por el BFF
    Cuando el gateway responde 202 para su comando
    Entonces el BFF responde HTML con estado pendiente y polling
    Y al terminar devuelve un fragmento sin polling periódico
    Y los valores del usuario se escapan y la mutación exige protección CSRF

  @REQ-025
  Escenario: Reconstruir una proyección sin repetir efectos
    Dado un checkpoint completo y el rango de cambios necesario aún retenido
    Cuando se crea una nueva generación de proyección y su inbox
    Entonces se procesan los eventos necesarios aunque la generación anterior los confirmara
    Y no se ejecutan handlers de cobros, correos ni otros efectos externos

  @REQ-026
  Escenario: Conciliar un timeout de efecto externo
    Dado un intento de pago a un servicio sin clave idempotente
    Cuando la respuesta se pierde después de enviar la solicitud
    Entonces el efecto queda con resultado unknown
    Y se consulta o concilia antes de repetir el intento

  @REQ-027
  Escenario: Detener un ciclo causal sin perder el diagnóstico
    Dado handlers que producen repetidamente un nuevo mensaje causal
    Cuando se alcanza max_hops o el deadline del proceso
    Entonces no se acepta otro salto y se registra un resultado durable observable
    Y retransmitir el mismo mensaje no incrementa hopcount

  @REQ-028
  Escenario: Seguir una operación sin registrar secretos
    Dado un comando que produce evento y actualización de proyección
    Cuando se inspecciona su telemetría
    Entonces se reconstruye el flujo mediante operación, correlación y causación
    Y no aparecen tokens ni valores secretos de entorno

  @REQ-029
  Escenario: Mantener dependencias de entregas retenidas
    Dado una entrega pendiente dentro de su retención declarada
    Cuando se ejecuta limpieza de inbox, releases y operaciones
    Entonces no se eliminan dependencias necesarias para resolver esa entrega
    Y el replay explícito usa otra generación con retención declarada

  @REQ-030
  Escenario: Normalizar autoría sin pérdida silenciosa
    Dado documentos con NaN, fecha TOML nativa o un entero mayor al máximo seguro
    Cuando se intenta normalizar al modelo JSON
    Entonces se rechazan esos valores con ubicación y código
    Y representaciones equivalentes con defaults producen la misma IR semántica
