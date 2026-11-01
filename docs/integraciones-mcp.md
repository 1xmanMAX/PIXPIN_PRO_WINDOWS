# PinDesk para Claude: el servidor MCP (`pindesk-mcp.exe`)

Con esto, **Claude Desktop** y **Claude Code** (y cualquier cliente MCP) pueden usar PinDesk
Max: apuntar y consultar tareas, escribir en un chat, crear notas, listas y lienzos, buscar
en tus documentos y chats, leer un PDF o un Office, hacer OCR, ver tu pantalla, anotar un
plano, mandar al móvil… con **las mismas herramientas que ya tiene el asistente** de PinDesk.

`pindesk-mcp.exe` es un programa aparte y pequeño: solo existe mientras el cliente lo tiene
abierto. No cuesta nada a PinDesk cuando no se usa.

## Instalar

`herramientas\instalar.ps1` lo deja junto a `pindesk.exe`:

```
%LOCALAPPDATA%\Programs\PinDesk\pindesk-mcp.exe
```

(Si no estaba compilado en la misma carpeta de compilación, lo compila ahí mismo.)

### Claude Desktop

Ajustes → Desarrollador → **Editar configuración** abre `claude_desktop_config.json`
(`%APPDATA%\Claude\claude_desktop_config.json`). Añade `pindesk` dentro de `mcpServers`
(si ya hay otros, solo la entrada `"pindesk": {...}`), cambiando `TU_USUARIO`:

```json
{
  "mcpServers": {
    "pindesk": {
      "command": "C:\\Users\\TU_USUARIO\\AppData\\Local\\Programs\\PinDesk\\pindesk-mcp.exe"
    }
  }
}
```

Reinicia Claude Desktop: en el botón de herramientas aparece «pindesk».

**Si ya lo tenías como `pixpin-mcp.exe`** (antes de PinDesk): cambia en la configuración
`pixpin-mcp.exe` por `pindesk-mcp.exe` (y, si quieres, la clave `"pixpin"` por `"pindesk"`).
Mientras la configuración nombre el viejo, el instalador deja `pixpin-mcp.exe` como copia del
nuevo para que no deje de funcionar.

### Claude Code

```powershell
claude mcp add pindesk -- "$env:LOCALAPPDATA\Programs\PinDesk\pindesk-mcp.exe"
```

Por defecto queda para el proyecto en el que estás; con `--scope user`, para todos:

```powershell
claude mcp add --scope user pindesk -- "$env:LOCALAPPDATA\Programs\PinDesk\pindesk-mcp.exe"
```

`/mcp` dentro de Claude Code dice si está conectado y qué herramientas ve.

### Modo portable

Si PinDesk va en modo portable (un `pindesk.toml` junto al exe), copia `pindesk-mcp.exe` a esa
misma carpeta: lee los datos de ahí, como la app. `PINDESK_RAIZ` (variable de entorno) fuerza
otra carpeta de datos.

## Qué herramientas hay

### De leer (funcionan con PinDesk cerrada, no escriben nada)

| Herramienta | Qué hace |
|---|---|
| `buscar_en_mis_cosas` | Busca en tus PDF, Word, Excel, PowerPoint, notas y chats con el índice del asistente (`<datos>\cache\documentos`). Devuelve trozos con su origen y cómo abrirlos. Si el índice aún no existe, busca solo en los chats |
| `consultar_tareas` | Tareas pendientes, hechas o vencidas, por periodo, lista o chat (la misma consulta del asistente) |
| `listar_chats` | Los chats (proyectos), con sus listas de tareas |
| `leer_documento` | El texto de un PDF (OCR en las páginas escaneadas), Word, PowerPoint, Excel, EPUB, HTML, Markdown o texto |
| `ocr_imagen` | OCR de Windows de una imagen o de un PDF |
| `ver_pantalla` | Captura la pantalla, un monitor o una ventana por su título y la devuelve como imagen (no se guarda) |
| `listar_ventanas` | Las ventanas que se ven, para `ver_pantalla` |

### De la app (las hace la PinDesk abierta)

Son **las filas de la tabla del asistente** (`crates/pindesk-ia/src/acciones.rs` y
`acciones_documentos.rs`), con su mismo nombre, descripción y JSON Schema: `abrir_chat`,
`abrir_fichero`, `abrir_mensaje`, `abrir_nota`, `abrir_hoja`, `escribir_en_chat`,
`nota_nueva`, `lienzo_nuevo`, `grabar_voz`, `reproducir_audio`, `lista_nueva`,
`anadir_tarea`, `anadir_tareas`, `marcar_tarea`, `mover_tarea`, `editar_tarea`,
`quitar_tarea`, `borrar_lista`, `abrir_ventana`, `capturar_pantalla`,
`pinear_ultima_captura`, `pinear_fichero`, `conservar_captura`, `borrar_captura`,
`copiar_imagen`, `caja_para_soltar`, `leccion_nueva`, `buscar_lecciones`,
`sincronizar_movil`, `enviar_al_movil`, `crear_automatizacion`, `tabla_de_imagen`,
`anotar_documento`, `buscar_en_documento`, `abrir_por_nombre`, `exportar_documento`.
Una fila nueva en esa tabla sale sola en MCP.

No están `consultar_tareas` como pedido (se contesta leyendo, arriba) ni `hoja_a_lienzo` (dibuja
en el lienzo abierto del chat del asistente).

Si PinDesk no está abierta, `pindesk-mcp.exe` arranca la que tiene al lado y espera hasta 8 s.

## Seguridad

- **Lo que borra o saca algo del PC** (`quitar_tarea`, `borrar_lista`, `borrar_captura`,
  `sincronizar_movil`, `enviar_al_movil`, `crear_automatizacion`) **lo pregunta PinDesk** antes,
  como cuando lo pide su asistente: una tarjeta «Necesito tu permiso» en el aviso flotante
  (abajo a la derecha) y en el chat «Asistente», con «Sí»/«No». Sin respuesta en 5 minutos, es
  un no y Claude recibe «el usuario dijo que no».
- **Lo que cambia algo que ya existe** (`marcar_tarea`, `mover_tarea`, `editar_tarea`,
  `anotar_documento`) también lo pregunta PinDesk cuando lo pide un cliente MCP («Claude pide: …
  ¿Lo hago?»), aunque al asistente no se lo pregunte.
- Crear (una tarea, una nota, un mensaje, una tabla, un lienzo) y abrir no preguntan: no se
  pierde nada y cada cosa sale con su aviso.
- En la lista de herramientas, las que preguntan van con `destructiveHint: true` y las de leer
  con `readOnlyHint: true`, para que el cliente también pida confirmación a su manera.
- Las de leer **nunca escriben** en la carpeta de datos (`%APPDATA%\PinDesk`).
- `ver_pantalla` ve lo que hay en pantalla en ese momento: el cliente pregunta antes de usarla
  (Claude Desktop y Claude Code piden permiso por herramienta).

## Cómo funciona por dentro

- stdio, JSON-RPC 2.0, un mensaje por línea; `initialize` (versiones 2025-11-25, 2025-06-18,
  2025-03-26 y 2024-11-05), `ping`, `tools/list`, `tools/call`. Una herramienta que no existe
  es un error `-32602`; un fallo de la herramienta es un resultado con `isError: true`.
- Las de la app van por el **pedido `herramienta`** (`docs/protocolo-pedidos.md`):
  `WM_COPYDATA` a la ventana de PinDesk con el nombre de la fila, sus argumentos y un fichero de
  respuesta en `%TEMP%\pindesk-mcp\` (o el `%TEMP%\pixpin-mcp\` de un servidor de antes). La app la hace como el asistente
  (`apps/pindesk/src/asistente/externa.rs`) y escribe ahí `{"ok": …, "texto": "…"}`.
- Código: `apps/pindesk-mcp` (`protocolo.rs`, `herramientas.rs`, `escribir.rs`, `leer.rs`,
  `documento.rs`, `pantalla.rs`). Prueba de punta a punta por stdio:
  `cargo test -p pindesk-mcp --test conversacion`.
