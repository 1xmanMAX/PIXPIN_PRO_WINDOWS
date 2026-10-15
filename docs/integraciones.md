# Integraciones: pedirle cosas a PixPin desde fuera (15-oct-2026)

Todo lo de aquí es **un adaptador fino** sobre los pedidos de siempre
(`docs/protocolo-pedidos.md`): la orden convierte lo que se le pide en un pedido v1 y lo manda
por `WM_COPYDATA` a la PixPin que está abierta (y la abre si no lo está). Nada nuevo en
pantalla: lo que pasa se ve como con Flow Launcher, en el aviso de PixPin.

| Puerta | Cómo | Dónde vive |
|---|---|---|
| Línea de órdenes | `pixpinmax <verbo> …`, `pixpinmax --pedido '<json>'` | `crates/pixpin-shell/src/orden.rs` (puro), `apps/pixpin/src/orden.rs` (el proceso) |
| Enlaces | `pixpinmax://tarea?texto=…`, `pixpin://atajo/…` (Android) | `orden::de_enlace`, `orden::seguridad`, `orden::firma` |
| Menú del Explorador y «Enviar a» | clic derecho → «Mostrar más opciones» → PixPin ▸ | `herramientas/integraciones.ps1`, `pixpin_shell::juntar` |
| PowerShell | `Import-Module …\PixPin.psm1` | `herramientas/PixPin.psm1` |
| AutoHotkey | `#Include …\pixpin.ahk` | `herramientas/pixpin.ahk` |
| Stream Deck, accesos directos, Programador de tareas | la acción «Abrir» con un enlace o con `pixpinmax.exe <verbo>` | (nada que instalar) |

`herramientas/instalar.ps1` lo pone todo en cada instalación (HKCU, sin administrador), llamando a
`herramientas/integraciones.ps1 -Exe <instalado>`. Para quitarlo:
`powershell -ExecutionPolicy Bypass -File herramientas\integraciones.ps1 -Quitar`. Para ver lo que
haría sin tocar nada: `-SoloMostrar`. `instalar.ps1 -SinIntegraciones` no las pone.

## 1. La línea de órdenes

```text
pixpinmax <verbo> [argumentos] [--sin-arrancar]
pixpinmax --pedido '<json>'
pixpinmax --pedido-archivo <ruta|->        (- = la entrada estándar)
pixpinmax --url "pixpinmax://<verbo>?campo=valor"
pixpinmax ayuda
```

| Verbo (alias) | Uso | Pedido |
|---|---|---|
| `capturar` | `capturar` | `capturar` |
| `pinear` (`pin`) | `pinear <fichero>… \| --codigo <id> [--proyecto P]` | un `pinear` por fichero |
| `pinear-ultima` | | `pinear_ultima` |
| `archivo` | `archivo <id del mensaje> [--proyecto P]` | `pinear {codigo}` (el `atajo/archivo` de Android) |
| `chat` (`mensaje`) | `chat [texto] [fichero…] [--proyecto P]` | `chat` (imágenes como fotos, el resto como adjuntos); sin nada, abre ese chat |
| `mensajes` | | `abrir` Mensajes guardados |
| `proyectos` | | `ventana_principal` |
| `proyecto` | `proyecto <id\|nombre>` | `abrir {tipo:"proyecto"}` |
| `tarea` (`task`, `todo`) | `tarea "<texto>" [--proyecto P] [--lista <id>]` | `anadir_tarea` (sin nada, al Inbox); sin texto, la ventana de tareas |
| `tareas` | | `ventana {cual:"tareas"}` |
| `capturas` (`galeria`) | | `ventana {cual:"galeria"}` |
| `nota` | `nota [texto] [imagen…] [--proyecto P]` | `nota_nueva` |
| `lienzo` | `lienzo [nombre] [--proyecto P]` | `lienzo_nuevo` |
| `lista` | `lista [titulo] [--proyecto P]` | `lista_nueva` («Tareas» si no se dice) |
| `leccion` | `leccion [texto] [imagen…]` | `leccion_nueva` |
| `lecciones` | `lecciones [buscar]` | `lecciones` |
| `grabar` (`voz`) | `grabar [nombre]` | `grabar` («Nota de voz» si no se dice) |
| `soltar` | `soltar [--proyecto P]` | `soltar` |
| `sincronizar` (`sync`) | `sincronizar [todos\|<aparato>]` | sin nada, `ventana {cual:"sincronizar"}`; con aparato, `sincronizar` |
| `al-movil` (`movil`) | `al-movil <fichero>… [--aparato <id>]` | `enviar_al_movil` (a `todos` si no se dice) |
| `ocr` (`texto`) | `ocr <imagen>… [--copiar]` | **ninguno**: lee el texto aquí mismo y lo escribe en la salida (sin PixPin abierta) |
| `copiar-texto` | `copiar-texto <imagen>…` | `ocr`: PixPin lo lee, lo copia y lo dice en su globo |
| `asistente` (`ia`, `preguntar`) | `asistente ["pedido"] [fichero…] [--proyecto P]` | `asistente`; con ficheros y sin pedido, pregunta en una caja qué hacer con ellos |
| `abrir` (`open`) | `abrir <fichero>… [--pagina N] [--buscar T]` | `abrir {tipo:"fichero"}` |
| `ventana` | `ventana tareas\|galeria\|lecciones\|sincronizar` | `ventana` |
| `pedido` | `pedido '<json>'` | tal cual |
| `ayuda` (`help`, `--help`, `/?`) | | la ayuda |

- `--proyecto` acepta el **id o el nombre** (sin mirar mayúsculas; «Mensajes guardados» o
  «guardados» es ese chat). El nombre lo cambia por su id la orden, leyendo
  `proyectos/indice.json`; uno que no existe se manda tal cual y PixPin lo dice.
- Las rutas relativas son de la carpeta donde se escribe la orden.
- Una orden que no conoce una `--opción` no se adivina: sale con 3 y lo dice.
- `--sin-arrancar`: si PixPin no está abierta, no la abre (sale con 4).
- Lo que no es una orden (una ruta de «Abrir con», `--atajo` de la Jump List) sigue el arranque de
  siempre.

### Códigos de salida

| Código | Significa |
|---|---|
| 0 | Aceptado (en la cola de PixPin), o la orden local (`ocr`, `ayuda`) salió bien |
| 1 | Fallo aquí mismo: un fichero que no está, una imagen que no lo es, un OCR que no leyó nada |
| 2 | PixPin no entiende esa versión del protocolo |
| 3 | La orden, el enlace o el JSON no se entienden |
| 4 | PixPin no está abierta y no se pudo abrir (o `--sin-arrancar`) |
| 5 | PixPin no contestó (colgada) |
| 6 | Rechazado: un enlace que no puede pedir eso, o el usuario dijo que no al confirmar |

0, 2, 3 y 4 son los mismos de `pixpin-lanzador pedido`.

### La consola

`pixpinmax.exe` es una aplicación de ventanas (si no, cada doble clic abriría una ventana negra).
Por eso:

- **Con la salida redirigida** (`| Out-String`, `> fichero`, `Start-Process -Redirect…`, el
  módulo de PowerShell) escribe en ella en UTF-8, y el que llama espera y ve el código.
- **Escrito a mano en una consola**, se engancha a ella para escribir, pero la consola no le
  espera: el texto sale después del símbolo del sistema. En `cmd`, `start /wait pixpinmax ocr x.png`;
  en PowerShell, `pixpinmax ocr x.png | Out-String` (o `Get-PixPinTexto`). `$LASTEXITCODE` solo
  vale si se espera.
- **Sin consola** (un enlace, el menú del Explorador, un Stream Deck), los fallos se dicen en un
  cuadro.
- `PIXPIN_ORDEN_SIN_CUADROS=1`: ningún cuadro (scripts desatendidos): lo que se avisaría va a la
  salida de errores y lo que se preguntaría se contesta «no».
- `PIXPIN_ORDEN_TIEMPO=1`: escribe cuánto tardó PixPin en aceptar.

### Lo que tarda

El proceso de la orden no carga nada de la app: es lo primero de `main`, antes de los ajustes, el
registro a fichero, Direct2D, la GPU o el idioma (el catálogo de textos solo se lee si hay que
enseñar un cuadro). Lo que cuesta es arrancar el proceso: el exe importa estáticamente `d2d1`,
`d3d11`, `dwrite`, `dxgi`, `mfplat`… y Windows mapea esos DLL al cargarlo aunque no se llamen.

Medido (`apps/pixpin/tests/orden_de_punta_a_punta.rs`, `medir_el_tiempo_hasta_aceptado`,
compilación de depuración, 15 vueltas, de lanzar el proceso a que sale con «aceptado»):

MEDIDAS

### Para otros programas en Rust

`pixpin_shell::mensajero::enviar_pedidos_o_arrancar(&[json], Some(exe))` manda pedidos a la
PixPin que corre y, si no hay ninguna, abre `exe` y la espera (8 s); devuelve `Envio`, con
`codigo_de_salida()`. Con `None` no abre nada. Es la que usan la línea de órdenes, los enlaces y el
menú del Explorador, y la que deben usar el servidor MCP, el puente del navegador o los avisos con
botones si no son ya la propia app. `pixpin_shell::orden` arma los pedidos de los verbos y de los
enlaces (`de_argumentos`, `de_enlace`, `pedido`, `pedido_al_asistente`), dice cuánto se fía de un
enlace (`seguridad`) y firma los de la casa (`orden::firma::firmar`).

## 2. Los enlaces `pixpinmax://` y `pixpin://`

`pixpinmax://<verbo>/<trozos>?campo=valor&…` es **el mismo verbo** con sus campos con nombre:

```text
pixpinmax://capturar
pixpinmax://tarea?texto=llamar%20al%20ingeniero&proyecto=Obra
pixpinmax://proyecto/pr-1789412424738
pixpinmax://abrir?tipo=mensaje&proyecto=pr-1789412424738&codigo=m-12
pixpinmax://abrir?ruta=C:%5Cobra%5Cplano.pdf&pagina=12        (pregunta antes: es un fichero)
pixpinmax://pedido?json=%7B%22pixpin%22%3A1%2C%22accion%22%3A%22capturar%22%7D
```

- **Android**: `pixpin://atajo/<accion>/<id>?…` (`atajos/AtajoActivity.kt`) se entiende igual, así
  que un enlace sirve en los dos aparatos: `capturar`, `mensajes`, `proyectos`, `sincronizar` (la
  pantalla), `proyecto/<id>`, `archivo/<id>`, `grabar`, `tarea?texto=`, `tareas`, `capturas`,
  `soltar?proyecto=`, `chat?texto=&proyecto=`, `nota?texto=`, `lista?titulo=&proyecto=`,
  `leccion`, `lecciones`. `buscar?texto=` aún no tiene pedido en el PC (sale con 3).
- **Esquemas**: los dos dan lo mismo. `pixpinmax://` siempre es nuestro. `pixpin://` lo registra
  también el **PixPin original de DepthPixel** si está instalado (en este equipo lo está:
  `…\Programs\PixPin\PixPin.exe`); `integraciones.ps1` **no se lo quita** (rompería sus
  enlaces) y solo lo toma si está libre o ya era nuestro, o con `-TomarPixpin`. Por eso los
  enlaces que hace el PC (`orden::enlace`) son `pixpinmax://`.
- Mayúsculas en el esquema o el verbo, `%20` o `+`, una barra final: da igual. Un campo repetido:
  vale el primero. Un `%` mal escrito o UTF-8 roto: 3. Más de 16 KB: 3.
- Los verbos `ocr`, `al-movil` y `ayuda` no se pueden pedir por enlace (6).

### La lista blanca

Un enlace lo puede lanzar cualquier página web (el navegador pregunta antes, pero pregunta
«¿abrir PixPin?», no «¿borrar tu lista?»). Por eso se mira **el pedido ya armado** (también el de
`pedido?json=`), no el verbo:

| | Qué | Cómo |
|---|---|---|
| **Directa** | `abrir` (proyecto, mensaje, hoja, nota), `chat`, `nota_nueva`, `lienzo_nuevo`, `lista_nueva`, `anadir_tarea`, `ventana_principal`, `ventana`, `capturar`, `lecciones`, `leccion_nueva`, `soltar`, `pinear` de un mensaje del chat, `pinear_ultima`, `asistente` sin pedido (abre su chat) | Se hace sin preguntar |
| **Pregunta** | Todo lo que lleva una ruta del disco (`abrir` un fichero, `pinear` una ruta, imágenes…), `asistente` con un pedido (puede hacer cosas), `grabar` (el micrófono), `sincronizar` y `enviar_al_movil`, `automatizacion` (activar, apagar, correr, borrar), lo que borra o quita, y cualquier acción que no esté en la lista | Un cuadro por encima de todo con lo que se pide, por qué pregunta y «No» por defecto |
| **Nunca** | Una ruta que no es de una unidad de este equipo (`\\servidor\…`, `//…`, `\\?\…`, `file:`, relativa): Windows le daría las credenciales del usuario al servidor solo por mirarla. Crear una automatización | Un cuadro que dice que no se hizo nada (6) |

Una acción nueva del protocolo **pregunta** hasta que alguien la ponga a propósito en
`orden::SEGURAS` (lo comprueba `toda_accion_del_protocolo_tiene_su_seguridad`).

### Los enlaces firmados (para los avisos con botones)

Los botones de los avisos de la propia app («Hecha», «+10 min», «Conservar») son enlaces, y
preguntar si de verdad quieres lo que acabas de pulsar en tu propio aviso sería absurdo. La app
los **firma**: `pixpin_shell::orden::firma::firmar(&url)` añade `&firma=<hex>`, un HMAC-SHA256 con
una clave de 32 bytes al azar de `%LOCALAPPDATA%\PixPinMax\enlaces.clave` (se crea la primera
vez). Se firma la forma canónica (verbo, trozos y campos descodificados y en orden), así que
reescribir el enlace por el camino no lo invalida. Un enlace con una firma buena se hace sin
preguntar (salvo una ruta que no sea de este equipo, que nunca). Una firma inventada o de otra
clave no vale nada: el enlace pasa por la lista blanca como cualquiera.

```rust
let url = pixpin_shell::orden::enlace("pedido", &[("json", &pedido.to_string())]);
let url = pixpin_shell::orden::firma::firmar(&url)?;   // pixpinmax://pedido?json=…&firma=…
```

## 3. El menú del Explorador y «Enviar a»

`integraciones.ps1` pone, en `HKCU\Software\Classes\*\shell\PixPinMax` (para todos los ficheros;
en Windows 11, bajo «Mostrar más opciones»):

```text
PixPin ▸ Al chat               pixpinmax.exe --juntar chat "%1"
         Pinear                pixpinmax.exe --juntar pinear "%1"
         Copiar texto (OCR)    pixpinmax.exe --juntar copiar-texto "%1"   (solo imágenes: AppliesTo)
         Enviar al movil       pixpinmax.exe --juntar al-movil "%1"
         Preguntar al asistente pixpinmax.exe --juntar asistente "%1"
```

y los mismos cinco en «Enviar a» (`%APPDATA%\Microsoft\Windows\SendTo\PixPin - ….lnk`, con el
verbo de argumento: ahí Windows pasa todos los ficheros en una sola llamada).

**Varios ficheros a la vez.** Un verbo del menú clásico no recibe todos los elegidos: el
Explorador lanza **una copia por fichero**. `--juntar` (`pixpin_shell::juntar`) hace que cada copia
apunte su ruta en `%TEMP%\pixpin-juntar\<verbo>` y pida un mutex con nombre; la primera espera
200 ms de silencio, se lleva todas y las demás se van sin hacer nada. Así tres capturas con
«Copiar texto (OCR)» dan un solo texto con las tres, «Al chat» un solo mensaje y «Preguntar al
asistente» una sola caja.

## 4. PowerShell

```powershell
Import-Module "$env:LOCALAPPDATA\Programs\PixPinMax\PixPin.psm1"

New-PixPinTarea "llamar al ingeniero" -Proyecto Obra
'pan', 'leche' | New-PixPinTarea                          # una tarea por línea
Get-Clipboard | New-PixPinNota
Get-ChildItem .\fotos\*.jpg | Send-PixPinChat -Texto 'Avance de obra' -Proyecto Obra
Get-PixPinTexto .\factura.png -Copiar                       # OCR, sin abrir PixPin
Import-Csv .\gastos.csv | Out-PixPin -Tabla 'Gastos'        # una tabla del chat, abierta
Invoke-PixPinAsistente 'resume mis tareas de hoy'
Open-PixPin .\tesis.pdf -Pagina 12 -Buscar 'losa'
Send-PixPin -Accion ventana -Campos @{ cual = 'galeria' }   # cualquier pedido
Send-PixPin -Accion capturar -ComoJson                      # solo ver el JSON
```

Funciones: `Invoke-PixPin` (la orden tal cual, esperando y con la salida), `Send-PixPin`,
`New-PixPinTarea`, `New-PixPinNota`, `Send-PixPinChat`, `Add-PixPinPin`, `Open-PixPin`,
`Get-PixPinTexto`, `Invoke-PixPinAsistente`, `Start-PixPinCaptura`, `Send-PixPinAlMovil`,
`Out-PixPin`, `Open-PixPinEnlace`, `Get-PixPinExe`. Los pedidos van por la entrada estándar
(`--pedido-archivo -`): un texto con comillas, saltos de línea o acentos llega entero. El código
queda en `$LASTEXITCODE` y, si no es 0, además sale un error. El exe: `$env:PIXPIN_EXE`, el que
está junto al módulo o el instalado.

## 5. AutoHotkey (v2)

```autohotkey
#Include "C:\Users\<tu>\AppData\Local\Programs\PixPinMax\pixpin.ahk"

^!t:: PixPinTarea(InputBox("¿Qué hay que hacer?", "PixPin").Value)
^!c:: PixPinCapturar()
^!o:: MsgBox(PixPinTexto("C:\capturas\factura.png"))
^!i:: PixPinAsistente("resume mis tareas de hoy")
```

`PixPin(args*)` corre cualquier orden y espera (devuelve el código); `PixPinPedido(json)`,
`PixPinEnlace(url)`, `PixPinTarea`, `PixPinNota`, `PixPinChat`, `PixPinAsistente`,
`PixPinCapturar`, `PixPinPinear`, `PixPinPinearUltima`, `PixPinGrabar`, `PixPinAlMovil`,
`PixPinAbrir(ruta, pagina)`, `PixPinTexto(ruta)` (OCR por el portapapeles, que deja como estaba).
Cada argumento va entre comillas como lo lee Windows (`CommandLineToArgvW`).

## 6. Stream Deck, accesos directos y el Programador de tareas

Sin plugin:

- **Stream Deck → Sistema → «Sitio web»** con `pixpinmax://capturar`,
  `pixpinmax://grabar?nombre=Reunión`, `pixpinmax://tarea`, `pixpinmax://atajo/sincronizar`… (el
  Stream Deck lo abre como un enlace; desmarca «Acceder en segundo plano» si lo ofrece). Lo que
  pide confirmar (grabar, sincronizar ya) saca su cuadro.
- **Stream Deck → Sistema → «Abrir»** con
  `"%LOCALAPPDATA%\Programs\PixPinMax\pixpinmax.exe" capturar` (o cualquier verbo): no pasa por la
  lista blanca, como cualquier orden.
- **Un acceso directo** (escritorio, barra de tareas, un teclado de macros): destino
  `…\pixpinmax.exe pinear-ultima`.
- **Programador de tareas**: acción «Iniciar un programa», `pixpinmax.exe` con argumentos
  `asistente "resume mis pendientes"`; con `--sin-arrancar` si no debe abrir PixPin.

Un plugin propio (iconos que cambian, p. ej. «sincronizando») solo si se pide.
