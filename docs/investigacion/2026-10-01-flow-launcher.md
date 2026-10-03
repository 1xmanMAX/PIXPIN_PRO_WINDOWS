# PixPin Max desde Flow Launcher — investigacion y diseno

Fecha: 2026-10-01. Estado: **propuesta, sin codigo**. Nada de esto esta implementado.

Lo que pidio el usuario:

1. Buscar y abrir directamente desde Flow Launcher lo de PixPin: lienzos, archivos, notas,
   proyectos y mensajes guardados.
2. Meter cosas en PixPin desde Flow: tareas ahora, «lecciones aprendidas» despues, y mas.
3. Poder ampliar el plugin el mismo, asi que hace falta un protocolo limpio, documentado y
   con version.

---

## 1. Lo que se ha medido en este equipo

- Flow Launcher **2.1.4** (la ultima, publicada el 2026-09-20), instalado en
  `%LOCALAPPDATA%\FlowLauncher\app-2.1.4`. Los datos de usuario estan en
  `%APPDATA%\FlowLauncher\` (`Plugins\`, `Settings\Settings.json`). El atajo es Shift+Space y
  la interfaz esta en ingles.
- Hay 24 plugins de la comunidad (Python y C#), entre ellos **QuickTodo** (`td`, activo),
  MarkdownTodo (`t`, apagado) y Obsidian Notes (`ob`, apagado). **`pp` esta libre.**
- Hay un Python de Flow dentro de `Environments\`, pero en el `PATH` del sistema no hay ningun
  `python`. Un plugin nuestro no debe depender de el.
- Datos de PixPin: `%APPDATA%\PixPinMax\proyectos\` tiene 10 proyectos, 58 mensajes en total
  (34 KB de `guardados.jsonl`), 82 `.excalidraw` y 4 `.md`. **Leerlo entero cuesta
  milisegundos**: no hace falta ningun indice persistente.

## 2. Flow Launcher: el sistema de plugins de hoy (2.1.x)

Fuentes: los documentos `Flow-Launcher/docs` (`plugin.json.md`, `json-rpc.md`,
`json-rpc-settings.md`, `testing.md`), el codigo de `Flow-Launcher/Flow.Launcher` en la
etiqueta `v2.1.4` (`Flow.Launcher.Core/Plugin/*.cs`, `Flow.Launcher.Plugin/AllowedLanguage.cs`,
`Result.cs`, `Query.cs`), la biblioteca `cibere/flogin` (un cliente v2 en Python que se usa en
produccion contra Flow) y `Flow-Launcher/Flow.Launcher.PluginsManifest`.

### 2.1 Tipos de plugin (`Language` en `plugin.json`)

| Language | Como corre | Encaje con PixPin |
|---|---|---|
| `CSharp` / `FSharp` | Una DLL de .NET que se carga dentro del proceso de Flow (`IPlugin`/`IAsyncPlugin`, `IContextMenu`, `ISettingProvider`) | Obliga a escribir y compilar C# aparte del workspace de Rust. No tiene sentido aqui. |
| `Python`, `JavaScript`, `TypeScript` (v1) | Un proceso por consulta, con JSON por argumento | Obliga a tener un runtime. No. |
| `Python_v2`, `JavaScript_V2`, `TypeScript_V2` | Un proceso que se queda vivo y habla JSON-RPC 2.0 | Obliga a tener un runtime. No. |
| `Executable` (v1) | **Un proceso por cada tecla**: la peticion va en `argv[1]` y la respuesta sale por stdout | Funciona, pero arranca un `.exe` por cada letra que se teclea. |
| **`Executable_V2`** | **Un proceso que se queda vivo**: JSON-RPC 2.0 por stdin/stdout, un mensaje por linea | **Este.** Un `.exe` de Rust pequeno, sin runtime. |

Las mayusculas dan igual (`AllowedLanguage.IsExecutable` compara con
`OrdinalIgnoreCase`). La documentacion web solo habla de v1; **el v2 hay que sacarlo del
codigo**, y lo que sigue sale de ahi.

### 2.2 `plugin.json`

```json
{
  "$schema": "https://www.flowlauncher.com/schemas/plugin.schema.json",
  "ID": "<GUID de 32 hex, fijo para siempre>",
  "ActionKeyword": "pp",
  "Name": "PixPin Max",
  "Description": "Buscar y abrir lienzos, notas, archivos y proyectos; anadir tareas",
  "Author": "Max Anthony Mamani Maquera",
  "Version": "0.1.0",
  "Language": "Executable_V2",
  "Website": "https://github.com/1xmanMAX/pixpin-max",
  "IcoPath": "Images\\pixpin.png",
  "ExecuteFileName": "pixpin-flow.exe"
}
```

- Tambien acepta `"ActionKeywords": ["pp", "..."]`. El usuario puede cambiar o anadir palabras
  clave desde los ajustes de Flow, y la consulta trae cual se uso.
- Si dos carpetas tienen el mismo `ID`, Flow **no carga ninguna**: al actualizar, hay que
  sustituir la carpeta, no copiar otra al lado.
- `IcoPath` (del plugin y de cada resultado) es relativo a la carpeta del plugin, aunque
  tambien admite rutas absolutas.

### 2.3 El protocolo v2 al detalle (verificado en el codigo)

- Flow lanza `ExecuteFileName` **una sola vez**, al arrancar. El directorio de trabajo es la
  carpeta del plugin, sin ventana, y con stdin, stdout y stderr redirigidos. Mete el proceso en
  un Job Object con `KillOnJobClose`: si Flow muere, el plugin muere con el.
- Variables de entorno que recibe: `FLOW_VERSION`, `FLOW_PROGRAM_DIRECTORY` y
  `FLOW_APPLICATION_DIRECTORY`.
- Transporte: `StreamJsonRpc` con `NewLineDelimitedMessageHandler` y System.Text.Json en
  **camelCase**. Cada mensaje es una linea de JSON-RPC 2.0 que termina en `\n`, y se aceptan
  `\r\n`. Los mensajes llevan `"jsonrpc":"2.0"` e `id`.
- **Todo lo que salga por stderr Flow lo trata como un error del plugin** (`ReadErrorAsync`
  lanza una excepcion con el texto). El registro de depuracion tiene que ir a un fichero,
  nunca a stderr.
- Flow puede mandar `$/cancelRequest` (una notificacion sin `id`) cuando el usuario sigue
  tecleando. Se puede ignorar, pero entonces conviene contestar rapido.

Las llamadas de Flow al plugin:

| Metodo | `params` | Que hay que devolver en `result` |
|---|---|---|
| `initialize` | `[contexto]`, con `currentPluginMetadata` (en el que van `pluginDirectory` y `pluginSettingsDirectoryPath`) | lo que sea; flogin contesta `{"hide":false}` |
| `query` | `[consulta, ajustes]`. La consulta es `{rawQuery, search, searchTerms, actionKeyword, isReQuery, ...}` y los ajustes son el diccionario de `SettingsTemplate.yaml` | `{"result":[...resultados], "settingsChange":{}, "debugMessage":""}` |
| `context_menu` | `[contextData]`, el `contextData` del resultado elegido | lo mismo que en `query` |
| *metodo de la accion* | **`[[p1, p2, ...]]`**: el array `parameters` del resultado, **envuelto en otro array** (`JsonRpc.InvokeAsync(name, argument:)` mete el argumento en `new object[]{argument}`) | `{"hide": true}` o `{"hide": false}` |
| `reload_data`, `close` | `[contexto]` / nada | opcionales; si faltan, Flow lo ignora |

Ojo: la respuesta de `query` es un **objeto** que lleva la lista dentro de su propio campo
`result`. Una lista suelta no vale, porque Flow la deserializa como
`JsonRPCQueryResponseModel`. El simulador de `flux-launcher` devuelve la lista suelta, y eso
**no** es lo que hace Flow.

Un resultado (se deserializa sin distinguir mayusculas; se escribe en camelCase):

```json
{
  "title": "Gestion de proyectos",
  "subTitle": "Proyecto - 12 mensajes - tocado hace 2 dias",
  "icoPath": "Images\\proyecto.png",
  "score": 80,
  "autoCompleteText": "pp p Gestion de proyectos",
  "copyText": "pr-1789412424738",
  "contextData": {"tipo": "proyecto", "proyecto": "pr-1789412424738"},
  "jsonRPCAction": {
    "method": "abrir",
    "parameters": [{"tipo": "proyecto", "proyecto": "pr-1789412424738"}],
    "dontHideAfterAction": false
  }
}
```

Tambien acepta `titleHighlightData`, `titleToolTip`, `subTitleToolTip`, `glyph`
(`{glyph, fontFamily}`, para iconos de Segoe Fluent), `preview` (`{previewImagePath,
description, isMedia, filePath}`), `progressBar` y `roundedIcon`.

El plugin tambien puede **llamar a Flow** por el mismo canal: peticiones con su propio `id`,
con el nombre exacto del metodo en C# (`JsonRPCPublicAPI`). Entre ellos estan `ChangeQuery
[texto, requery]`, `ShowMsg [titulo, subtitulo, icono]`, `ShowMsgError`, `CopyToClipboard`,
`OpenDirectory`, `OpenUrl`, `HideMainWindow`, `BackToQueryResults`, `StartLoadingBar`,
`StopLoadingBar`, `FuzzySearch` y `LogInfo`. Y existe `UpdateResults [consulta, respuesta]`,
para mandar resultados sin que nadie los pida.

Ejemplo de una conversacion, linea a linea (`-->` va a la app, `<--` sale de ella, vista desde
el plugin):

```text
<-- {"jsonrpc":"2.0","id":1,"method":"initialize","params":[{"currentPluginMetadata":{...}}]}
--> {"jsonrpc":"2.0","id":1,"result":{"hide":false}}
<-- {"jsonrpc":"2.0","id":2,"method":"query","params":[{"rawQuery":"pp gest","search":"gest","actionKeyword":"pp","isReQuery":false,...},{}]}
--> {"jsonrpc":"2.0","id":2,"result":{"result":[{"title":"Gestion de proyectos",...}],"settingsChange":{},"debugMessage":""}}
<-- {"jsonrpc":"2.0","id":3,"method":"abrir","params":[[{"tipo":"proyecto","proyecto":"pr-1789412424738"}]]}
--> {"jsonrpc":"2.0","id":3,"result":{"hide":true}}
```

Un riesgo conocido: en la rama `dev` de Flow (issue #4686, abierta), `Result.ContextData` lleva
`[JsonIgnore]` y el menu contextual de los plugins JSON-RPC llega con `[null]`. **La 2.1.4 no
tiene ese fallo.** Para defendernos, cada resultado repite su identidad dentro de
`jsonRPCAction.parameters`, y si `context_menu` llega con `null` se contesta una lista vacia
sin fallar.

### 2.4 Ajustes, instalacion y tienda

- **Ajustes**: un `SettingsTemplate.yaml` en la carpeta del plugin (`body:` con `textBlock`,
  `input`, `inputWithFileBtn`, `inputWithFolderBtn`, `textarea`, `checkbox`, `dropdown`). Flow
  pinta el panel solo y guarda en `Settings\Plugins\<Nombre>\Settings.json`. Los valores llegan
  como segundo parametro de cada `query`.
- **Instalar en local**: se copia la carpeta (`plugin.json` + exe + `Images\`) a
  `%APPDATA%\FlowLauncher\Plugins\<Nombre>-<version>\` y se reinicia Flow. Tambien vale
  `pm install <ruta local o URL de un zip>`, pero en este equipo el plugin **Plugins Manager
  esta apagado**.
- **Tienda**: se abre un PR en `Flow-Launcher/Flow.Launcher.PluginsManifest` con
  `plugins/<Nombre>-<uuid>.json`. Ese fichero lleva `ID`, `Name`, `Description`, `Author`,
  `Version`, `Language`, `Website`, `UrlDownload` (el zip de una Release de GitHub),
  `UrlSourceCode` e `IcoPath` (por CDN, p. ej. jsDelivr), y si se quiere `MinimumAppVersion`.
  Es **obligatorio** tener un GitHub Actions que compile y publique la Release. El CI de la
  tienda busca versiones nuevas cada 3 h y las pasa por VirusTotal. Un `.exe` de Rust sin
  firmar puede dar falsos positivos. No hace falta para el uso personal.

## 3. Lo que PixPin ya tiene y sirve

### 3.1 Recibir peticiones de otro proceso

- `pixpin_shell::instancia`: es un mutex `Local\PixPinMax-instancia-unica`. La segunda copia no
  arranca.
- `pixpin_shell::mensajero`: la ventana de mensajes (de solo mensajes) tiene la clase
  **`PixPinMaxVentanaMensajes`** y se encuentra con `FindWindowW`.
  - `WM_COPYDATA` con `dwData = ABRIR_FICHEROS (0x5049_5850)` lleva rutas en UTF-16 separadas
    por nulos. La ventana lo convierte en `Evento::AbrirFicheros` (`ventana.rs:270`) y contesta
    `LRESULT(1)`. Despues hace falta un `despertar` para que el bucle no se quede dormido.
  - `WM_COMMAND <id>` sirve para los comandos del catalogo: `AbrirChat` y `25` = Salir, que ya
    usa `instalar.ps1`.
- `main.rs:882`, `AbrirFicheros`: un `.md` va a `notas_md::abrir(Destino::Fichero)`;
  Word/libro/PDF van a `lector::abrir_en_su_lector`; un `.pixpin` se importa; el resto se
  **pinea**. Un `.excalidraw` suelto acaba de ficha pineada, no en el lienzo. Por eso abrir un
  lienzo **no puede ir por rutas**: tiene que ir por proyecto y hoja.

### 3.2 Funciones para abrir que ya existen (el vocabulario sale hecho)

- `ventana_chat::ir_a(idioma, ubicacion, opciones, proyecto, codigo: Option<String>)` abre el
  chat, o lo trae al frente, en un proyecto, y si se le da un codigo, en un mensaje concreto
  que queda resaltado 1,5 s.
- `universo::abrir::abrir_hoja(raiz, proyecto, referencia, opciones)` abre una hoja o un dibujo
  en el lienzo y la guarda al cerrar, igual que tocarla en el chat.
- `notas_md::abrir(idioma, ubicacion, Destino::Mensaje{proyecto, codigo} | Nueva{proyecto} |
  Fichero{ruta})` abre el editor de notas, y si ya esta abierto lo trae al frente.
- `lector::abrir_en_su_lector` es para PDF, Word y libros; `pixpin_shell::abrir(ruta)` abre con
  el programa de Windows.
- `universo::abrir::Apertura { Fichero, Hoja{proyecto, referencia}, Chat{proyecto, codigo},
  Carpeta, Varios, Nada }` y `que_abre_ficha` ya son **la tabla «que abre cada cosa»**. El
  plugin solo tiene que hablar ese idioma.

### 3.3 Donde vive cada cosa en el disco (lo que leeria el buscador)

La raiz es `%APPDATA%\PixPinMax`, o la carpeta del exe si junto a el hay un `pixpinmax.toml`
(`pixpin_store::rutas::resolver`). Algunos proyectos pueden estar en otra carpeta que eligio el
usuario, enlazada con una *junction* (`pixpin_proyecto::ubicacion`). Leer por la junction es
transparente.

| Que | Donde | Formato |
|---|---|---|
| Lista de proyectos | `proyectos/indice.json` | `{"proyectos":[{id, nombre, uid, creado, aparato, tocado, hojas, resumen, sin_leer, paquete, guardados?, sincro}]}`. El de `guardados:true` es «Mensajes guardados» (`almacen::NOMBRE_GUARDADOS`). |
| Mensajes del chat | `proyectos/<id>/guardados.jsonl` | Una linea JSON por mensaje (`cuaderno::Mensaje`): `id, cuando, clase (NOTA, IMAGEN, ARCHIVO, VOZ, DIBUJO, PAGINA, PROYECTO, MINIAPP), texto, nombre, ruta (relativa al proyecto), referencia, transcripcion, miniapp, numero, uid, aparato, fijado, enBuzon, ...`. Una linea rota se salta. |
| Hojas, lienzos | `proyectos/<id>/proyecto.json` (`hojas[]: id, nombre, dibujo, nota, pagina, uid...`) + `lienzos/<dibujo>.excalidraw` | El nombre visible sale del mensaje `DIBUJO`/`PAGINA` o de la hoja. Leer el `.excalidraw` **no** hace falta para buscar. |
| Notas | Mensajes `NOTA` (el texto va dentro) y `notas/<id>.md` | Markdown |
| Archivos | `proyectos/<id>/archivos/...` (los `ARCHIVO` apuntan aqui) | tal cual |
| Tareas | Mensajes `MINIAPP` con `miniapp:"tareas"`. El documento va en `texto` | `# Titulo\n\n- [ ] pan\n- [x] leche` (`pixpin_proyecto::mini`, el mismo formato que `mini/Tareas.kt` de Android) |
| Recordatorios | El campo `recuerdaEn` dentro del mensaje (`recordatorios.rs`) | ms UTC |
| Pines y capturas | `almacen/indice.json` + `objetos/` | Fuera del alcance v1 |

Se excluyen `papelera/` y los mensajes `enBuzon`. Los mensajes que vienen del movil sin el
fichero en el equipo (rutas absolutas de Android) salen como «no esta en este equipo».

### 3.4 Tareas y «lecciones aprendidas»

- **Tareas: ya existen.** Son la mini-app `tareas`, una de siete, copiada de Android. Se guarda
  como casillas de Markdown dentro del mensaje y viaja al movil al sincronizar.
- **«Lecciones aprendidas» no existe** ni en el PC ni en Android. Se comprobo con `gh api
  search/code` en `1xmanMAX/PIXPIN_PRO_ANDROID`: las unicas apariciones de «lecciones» son
  comentarios del codigo. Hay que disenarlo. Ver las preguntas.

### 3.5 Por que escribir **no** puede hacerlo un proceso externo

`cuaderno::anadir` escribe una linea al final, pero `cuaderno::reemplazar` (que hace falta para
anadir una casilla a una lista de tareas que ya existe) lee el fichero entero, lo cambia y lo
renombra encima. El cerrojo que los protege (`cuaderno::cerrojo()`) es un `Mutex` **dentro del
proceso**. Si un `.exe` de fuera escribiera en `guardados.jsonl` mientras PixPin reescribe,
**perderia mensajes sin avisar**. Ademas, el chat abierto no se enteraria (le falta
`ventana_chat::refrescar()`) y la sincronizacion con el movil tampoco. Conclusion: **leer, si;
escribir, siempre a traves de la app.**

## 4. Diseno propuesto

### 4.1 Arquitectura

```text
Flow Launcher ──JSON-RPC v2 (stdin/stdout)──► pixpin-flow.exe ──lee──► %APPDATA%\PixPinMax\proyectos\...
                                                    │
                                                    └──WM_COPYDATA «pedido» JSON v1──► pixpinmax.exe (ventana PixPinMaxVentanaMensajes)
                                                                                         abre / anade / guarda / sincroniza
```

- **Un `.exe` aparte y pequeno, `pixpin-flow.exe`, en `apps/pixpin-lanzador/`** (o el nombre
  que se elija; ver la pregunta 6). No es un subcomando de `pixpinmax.exe`, por tres razones:
  1. Flow mantiene el proceso **vivo todo el tiempo**. Un `pixpinmax.exe` residente por segunda
     vez cargaria D3D, DirectWrite y todo lo demas; en 4 GB eso se nota.
  2. La instancia unica: una segunda copia de `pixpinmax.exe` se va por el mutex. Habria que
     abrirle una excepcion, que es justo lo que el mutex existe para impedir.
  3. Un `.exe` de 300 a 600 KB que solo usa `serde_json` y unas pocas funciones de `windows`
     (`FindWindowW`, `SendMessageW`) arranca en milisegundos y ocupa unos 2 o 3 MB de RAM.
- El precedente es `apps/pixpin-aligerar`: un `.exe` hermano que se instala al lado.
- **Buscar = leer el disco directamente.** Se hace al vuelo en cada consulta, con una cache en
  memoria que se invalida por la fecha de modificacion de `indice.json` y de cada
  `guardados.jsonl`. **Funciona aunque PixPin este cerrado.** Con los datos de hoy (34 KB) una
  lectura completa cuesta menos de 5 ms; con 100 veces mas seguiria muy por debajo de lo que
  tarda Flow en pintar.
- **Abrir y anadir = pedirselo a la app** con un mensaje nuevo de `WM_COPYDATA`. Si PixPin no
  esta corriendo, `pixpin-flow` lo arranca (`%LOCALAPPDATA%\Programs\PixPinMax\pixpinmax.exe`),
  espera a que aparezca la ventana `PixPinMaxVentanaMensajes` (como mucho unos 5 s, mirando cada
  100 ms) y le manda el pedido.
- **Que crates puede usar.** `pixpin-proyecto` arrastra `pixpin-motor2d`, `pixpin-sincro`,
  `pixpin-shell` (con su crate `windows` de 40 features) y `zip`. Es demasiado para el exe.
  Las alternativas son dos: (a) el exe declara sus propios structs de solo lectura, con
  `#[serde(default)]` y solo los campos que usa; o (b) se saca un crate puro nuevo,
  **`pixpin-indice`** (con `#![forbid(unsafe_code)]`, sin Windows y solo con `serde`), con
  «listar proyectos, leer el cuaderno, que se abre con cada cosa, normalizar para buscar». Lo
  usarian `pixpin-flow` y, con el tiempo, el buscador del chat o del universo. **Recomiendo la
  (b)**, que ademas puede reutilizar `pixpin_universo::buscar::normalizar` (quita tildes y pasa a
  minusculas) moviendolo ahi.
- `crates/pixpin-flow` **ya existe** y esta vacio, pero reservado en el diseno maestro para otra
  cosa: los «flujos» post-captura (`docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md`,
  linea 157). Para no mezclar los dos «flow», el exe de Flow Launcher no deberia vivir en ese
  crate. `crates/pixpin-plugin` («API de extension + CLI») es el sitio natural para el
  **protocolo** documentado (los tipos del pedido, en Rust puro).

### 4.2 Palabra clave y consultas

Palabra clave por defecto: **`pp`** (esta libre). El usuario puede anadir otras en Flow.

| Se teclea | Resultados |
|---|---|
| `pp` (vacio) | Los ultimos 8 proyectos tocados + «Anadir tarea...», «Nueva nota...», «Abrir chat» |
| `pp <texto>` | Busqueda en todo, ordenada por puntuacion: proyectos, lienzos y hojas, notas, archivos, tareas y mensajes. Como mucho 20. |
| `pp p <texto>` | Solo proyectos |
| `pp l <texto>` | Solo lienzos y hojas (DIBUJO, PAGINA, hojas de `proyecto.json`) |
| `pp n <texto>` | Solo notas (NOTA y `.md`) |
| `pp a <texto>` | Solo archivos (ARCHIVO, IMAGEN, VOZ por transcripcion) |
| `pp m <texto>` | Solo mensajes guardados (los del proyecto «Mensajes guardados») |
| `pp t` | Las tareas **pendientes** de todas las listas |
| `pp tarea <texto>` / `pp + <texto>` | Primer resultado: «Anadir tarea: *texto*» → Intro |
| `pp tarea <texto> @<proyecto>` | Lo mismo, en la lista de ese proyecto. Al teclear `@` se sugieren proyectos con `autoCompleteText`. |
| `pp nota <texto>` | «Nueva nota: *texto*» en «Mensajes guardados» (o `@proyecto`) |
| `pp leccion <texto>` | «Anadir leccion aprendida: *texto*» (cuando se decida el formato, §4.5) |

Si la primera palabra es un verbo conocido (`tarea`, `nota`, `leccion`, `+`), el primer
resultado es la accion y debajo siguen los resultados de buscar el resto. Asi, escribir «pp
tarea» para *buscar* la palabra «tarea» sigue funcionando: se baja una fila. Los verbos son
palabras ASCII, y se aceptan con y sin tilde (`leccion`/`lección`, `anadir`/`añadir`).

Puntuacion: coincidencia al principio del nombre > en el nombre > en el texto o la
transcripcion; y entre iguales, gana lo tocado mas recientemente. Las mismas reglas que
`IndiceBusqueda` del universo, sin tildes.

### 4.3 Que hace cada resultado

| Tipo | Subtitulo | Intro | Menu contextual (Mayus+Intro o clic derecho) |
|---|---|---|---|
| Proyecto | `Proyecto · N hojas · tocado <fecha>` | `ir_a(proyecto, None)` → el chat en ese proyecto | Abrir la carpeta · Copiar el id · Nueva nota aqui · Anadir tarea aqui (`ChangeQuery "pp tarea  @Nombre"`) |
| Lienzo u hoja | `Lienzo · <proyecto>` | `abrir_hoja(proyecto, referencia)` | Ver en el chat (`ir_a` con su codigo) · Abrir la carpeta |
| Nota | primera linea · `<proyecto>` | `notas_md::abrir(Destino::Mensaje{proyecto, codigo})` | Ver en el chat · Copiar el texto |
| Archivo | `<nombre> · <proyecto> · <tamano>` | PDF/Word/libro → su lector de PixPin; `.md` → el editor de notas; el resto → `pixpin_shell::abrir` | Ver en el chat · Abrir la carpeta (`OpenDirectory` de Flow, sin pasar por PixPin) · Copiar la ruta · Pinear |
| Mensaje | el texto · `<proyecto>` | `ir_a(proyecto, Some(codigo))`, resaltado | Copiar el texto |
| Tarea pendiente | `☐ <texto>` · `<lista> · <proyecto>` | Marcarla hecha (pedido `marcar_tarea`) y quedarse en Flow (`hide:false`, despues `ChangeQuery` con requery) | Abrir la lista en el chat |
| «Anadir ...» | `Enter para guardar en <destino>` | El pedido `anadir_*`. Flow se cierra y PixPin avisa con su globo. | — |

`IcoPath`: unos pocos PNG de 32 px dentro de `Images\` (proyecto, lienzo, nota, archivo,
tarea, mensaje). Para las imagenes se puede usar `preview.previewImagePath` con la ruta real
del fichero (Flow ya lo pinta).

### 4.4 El protocolo local: «pedido PixPin v1»

Es lo que el usuario usaria para escribir sus propios plugins o scripts (AutoHotkey,
PowerShell, otro lanzador). Hay tres niveles, y cada uno se apoya en el anterior.

**Nivel 1 — el pedido** (lo entiende la app; es el contrato estable):

- Transporte: `WM_COPYDATA` a la ventana de clase `PixPinMaxVentanaMensajes`, con
  `dwData = PEDIDO_JSON = 0x5049_5851` (el siguiente de `ABRIR_FICHEROS`). `lpData` lleva el
  **JSON en UTF-8, sin nulo final**. Lo envia `SendMessageW`, y despues un `despertar`.
- Respuesta inmediata (`LRESULT`): `1` aceptado (en cola); `2` version no soportada; `3` JSON
  invalido o accion desconocida; `0` no era para nosotros o no hay nadie. **El resultado real**
  (que se creo, o que fallo al guardar) lo comunica PixPin con su globo, porque el procedimiento
  de ventana no debe trabajar (`ventana.rs`: «cuanto antes vuelva...»). La respuesta tiene que
  ser inmediata y el trabajo va al bucle principal, como `AbrirFicheros`.
- Tamano maximo: 64 KB. Un pedido mas grande se rechaza con `3`.

```json
{ "pixpin": 1, "accion": "abrir", "que": { "tipo": "proyecto", "proyecto": "pr-1789412424738" } }
{ "pixpin": 1, "accion": "abrir", "que": { "tipo": "mensaje", "proyecto": "N5T7C46GZE", "codigo": "42USR8334D" } }
{ "pixpin": 1, "accion": "abrir", "que": { "tipo": "hoja", "proyecto": "pr-1788574477233", "referencia": "dib-1790560815018" } }
{ "pixpin": 1, "accion": "abrir", "que": { "tipo": "nota", "proyecto": "...", "codigo": "..." } }
{ "pixpin": 1, "accion": "abrir", "que": { "tipo": "fichero", "ruta": "C:\\...\\archivos\\informe.pdf" } }
{ "pixpin": 1, "accion": "anadir_tarea", "texto": "comprar pan", "proyecto": null, "lista": null }
{ "pixpin": 1, "accion": "marcar_tarea", "proyecto": "...", "codigo": "...", "texto": "comprar pan", "hecha": true }
{ "pixpin": 1, "accion": "anadir_nota", "texto": "idea: ...", "proyecto": null }
{ "pixpin": 1, "accion": "ventana_principal" }
```

Reglas:

- `pixpin` es la version del protocolo. Un cambio que no rompe nada (un campo nuevo opcional)
  no la sube. Quitar o cambiar el significado de algo la sube, y la app sigue entendiendo la
  anterior.
- Los campos que no se conocen se ignoran. Es la misma regla que el resto del proyecto:
  «lo que no se entiende, se conserva».
- `proyecto: null` significa **«Mensajes guardados»** (`almacen::asegurar_guardados` lo crea si
  hace falta).
- Lo que se crea lleva el sello de siempre (`Sello`: hora, numero, aparato del equipo), asi que
  el movil lo ve como nacido en el PC.
- `anadir_tarea` sin `lista` busca la **ultima mini-app `tareas`** del proyecto (la mas
  reciente) y le anade `- [ ] texto` al final con `cuaderno::reemplazar`. Si no hay ninguna,
  crea una titulada «Tareas» (`Mensaje::miniapp`). Despues llama a `ventana_chat::refrescar()`.
- `marcar_tarea` busca la casilla por su texto exacto dentro de esa lista (no hay ids de
  tarea; Android tampoco los tiene). Si hay dos iguales, marca la primera pendiente.
- Lo de abrir se reparte con las mismas funciones de §3.2. Se puede reutilizar
  `universo::abrir::Apertura` como tipo intermedio.

**Nivel 2 — la linea de mandatos de `pixpin-flow.exe`** (para scripts, sin escribir codigo de
Win32):

```text
pixpin-flow.exe                          -> sin argumentos: modo plugin (JSON-RPC por stdin)
pixpin-flow.exe buscar "gestion" [--json]  -> resultados, una linea JSON cada uno, por stdout
pixpin-flow.exe tarea "comprar pan" [--proyecto <id>]
pixpin-flow.exe nota "texto"
pixpin-flow.exe abrir proyecto <id> | hoja <id> <ref> | mensaje <id> <codigo>
pixpin-flow.exe pedido '<json del nivel 1>'
```

Codigo de salida: `0` aceptado, `2` y `3` como el `LRESULT`, `4` PixPin no arranco.

**Nivel 3 — el plugin de Flow** (§2, §4.2, §4.3). Es solo un cliente mas de los niveles 1 y 2.
Un plugin hecho por el usuario en cualquier lenguaje puede llamar a `pixpin-flow.exe buscar
--json` para leer y a `pixpin-flow.exe pedido` para actuar, sin conocer el disco.

La documentacion para el usuario iria en `docs/protocolo-pedidos.md`, con estos ejemplos y una
tabla de acciones. El esquema JSON va en `crates/pixpin-plugin`.

### 4.5 «Lecciones aprendidas»: opciones (hay que decidir)

1. **Una nota con etiqueta**: un `NOTA` con texto `#leccion ...`, en el proyecto que se diga.
   Cero formato nuevo; el movil lo ve como una nota. Buscar `pp leccion` = notas con esa
   etiqueta.
2. **Una mini-app nueva `lecciones`**: un documento Markdown dentro del mensaje (`# Lecciones —
   Proyecto X` + una entrada por linea con su fecha). Android ensena como texto plano la
   mini-app que no conoce, asi que no se rompe nada. Pero la regla de «seguir la logica de
   Android» pide **disenarla primero en Android**, o a la vez.
3. Un `.md` por proyecto (`notas/lecciones.md`), con una linea anadida por cada leccion.

Recomendacion provisional: la **1** para empezar (se puede hacer ya y es reversible). Si se
asienta, pasar a la **2** de acuerdo con Android.

### 4.6 Instalar junto a `instalar.ps1`

- La compilacion sale de la misma carpeta de `target\...\release` que `pixpinmax.exe` y
  `pixpin-aligerar.exe`: se anade un tercer par en `$ficheros`, se compila si falta (igual que
  el compresor) y se copia a `%LOCALAPPDATA%\Programs\PixPinMax\pixpin-flow.exe`.
- Ademas se crea o actualiza `%APPDATA%\FlowLauncher\Plugins\PixPin Max\` con `plugin.json`,
  `Images\*.png`, `SettingsTemplate.yaml` (si hace falta) y una **copia** de `pixpin-flow.exe`.
  Es una copia porque Flow arranca la ruta relativa a la carpeta del plugin. Las fuentes de esos
  ficheros vivirian en el repo, en `herramientas/flow-launcher/`.
- Antes de copiar hay que **cerrar Flow** o, al menos, matar `pixpin-flow.exe`, porque Flow lo
  tiene abierto y el exe esta bloqueado. Despues se vuelve a lanzar `Flow.Launcher.exe`. Solo se
  hace si Flow esta instalado (si existe `%APPDATA%\FlowLauncher\Plugins`); si no, se salta en
  silencio.
- Cada `Version` de `plugin.json` sale de `workspace.package.version` para que Flow vea que hay
  una actualizacion.
- La tienda de Flow queda para mas adelante. Haria falta un repositorio publico, una Release con
  un zip y GitHub Actions. Ver la pregunta 5.

### 4.7 Limites (4 GB de RAM, HD 4000)

- Nada de runtime: ni Python, ni Node, ni .NET aparte del propio Flow.
- El exe residente ocupa menos de 5 MB de RAM, tiene un solo hilo, lee stdin con bloqueo y no
  gasta CPU mientras no se teclea.
- No hay indice persistente, ni vigilancia de carpetas, ni hilos de fondo. La cache en memoria
  se invalida por la fecha de los ficheros.
- Del lado de la app, solo se anade un `dwData` nuevo en `procedimiento` y un `Evento::Pedido`
  en el bucle. Ni sockets ni puertos: es el mismo argumento que ya da `mensajero.rs`.
- Los identificadores siguen el estilo del codigo: ASCII y en espanol (`anadir_tarea`,
  `PEDIDO_JSON`, `Evento::Pedido`, `pixpin_indice::buscar`).

### 4.8 Orden de trabajo sugerido

1. `pixpin-indice` (puro): leer el indice y los cuadernos y buscar, con pruebas sobre una copia
   de `proyectos/`.
2. El pedido en la app: `PEDIDO_JSON`, `Evento::Pedido`, `abrir` (los cinco tipos) y
   `anadir_tarea`/`anadir_nota`, con pruebas de desempaquetar como las de `mensajero.rs`.
3. `pixpin-flow.exe`, primero con la linea de mandatos (se prueba sin Flow).
4. El modo JSON-RPC v2 + `plugin.json` + iconos. Probarlo a mano con un guion que escriba
   lineas en stdin, y luego en Flow.
5. `instalar.ps1`.
6. `marcar_tarea`, `pp t` y las lecciones (cuando se decida §4.5).

## 5. Preguntas para el usuario

1. **¿Adonde va una tarea sin destino?** La propuesta es una lista «Tareas» dentro de «Mensajes
   guardados». ¿O prefieres elegir proyecto siempre, o tener una lista fija?
2. **«Lecciones aprendidas»**: ¿la opcion 1 (nota `#leccion`), la 2 (mini-app nueva, primero en
   Android) o la 3 (un `.md` por proyecto)? ¿Que campos tiene una leccion: solo texto, o
   tambien «que paso / que aprendi / que hare distinto»?
3. **¿`pp` te vale** como palabra clave? ¿Quieres tambien atajos directos como `pt` para «anadir
   tarea» sin escribir el verbo?
4. **Intro sobre un archivo**: ¿se abre dentro de PixPin (su lector, o el pin) o con el programa
   de Windows?
5. **¿Publicarlo en la tienda de Flow** algun dia, o solo para ti? Publicar obliga a tener un
   repositorio publico y GitHub Actions.
6. **El nombre**: `crates/pixpin-flow` ya esta reservado para los «flujos» post-captura. ¿Llamo
   al exe `pixpin-flow.exe` igualmente (en `apps/pixpin-lanzador`) o prefieres otro nombre?
7. **¿Tambien pines y capturas** (`almacen/`) en la busqueda, o solo lo del chat?
