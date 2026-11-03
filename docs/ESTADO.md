# Dónde lo dejamos (3-nov-2026)

Guía para retomar PinDesk desde un `git clone` limpio, sin nada más que este
repositorio. Lo que hace la app está en el [README](../README.md); esto es solo
lo que hace falta para seguir trabajando.

## Estado

- **Última versión publicada:** `v0.12.0` (Releases de GitHub), instalada y
  probada en el equipo de desarrollo el 3-nov-2026.
- **Ramas:** `main` lleva todo; `microsoft-store` es `main` más lo que pide la
  Store ([ramas.md](ramas.md)). No hay otras ramas de trabajo abiertas.
- **Emparejar sin IP** (rama `emparejar-sin-ip`, 3-nov-2026): unirse con el código busca por mDNS
  y por la baliza a la vez, «Dar permiso de red» para el cortafuegos, el QR `pindesk://emparejar` y
  sincronizar al volver de la suspensión. Pendiente en Android: [emparejar.md](emparejar.md).
- **Pruebas que ya fallaban** (no son regresiones; pendientes de arreglar):
  - `compartir::pruebas::cada_mensaje_solo_ofrece_sus_formatos_y_su_original`
  - `compartir::ventana::pruebas::la_hoja_se_pinta_entera_y_dice_cuanto_pesa`

## Lo que hay en el repositorio

| Carpeta | Qué es |
|---|---|
| `apps/pindesk` | La app (`pindesk.exe`). |
| `apps/pindesk-lanzador` | El plugin `p` de Flow Launcher (`plugin/` lleva su `plugin.json` e icono). |
| `apps/pindesk-mcp`, `apps/pindesk-puente`, `apps/pindesk-aligerar` | Servidor MCP, puente con el navegador y el ayudante de aligerar. |
| `crates/` | Las bibliotecas (`pindesk-*`): motor del lienzo, documentos, CAD/BIM, sincronizar, IA… |
| `apple/` | La versión de Mac/iPad/iPhone en curso, con su propio espacio de trabajo ([apple/README.md](../apple/README.md)). |
| `herramientas/` | Scripts: `instalar.ps1`, la página del lector de notas (`lector-md/`), idiomas, etc. |
| `docs/` | Documentación de cada parte; `investigacion/` guarda los estudios previos a cada función. |
| `extensiones/` | La extensión del navegador. |
| `medidas/` | Mediciones de rendimiento y decisiones que salieron de ellas. |

## Preparar un equipo nuevo

1. **Rust:** instalar `rustup`. La versión exacta la fija `rust-toolchain.toml`
   y se descarga sola al primer `cargo`.
2. **Visual Studio Build Tools** con «Desarrollo para el escritorio con C++»
   (el enlazador de MSVC y el SDK de Windows).
3. **Node.js** (solo si se toca la página del lector de notas).
4. **WebView2 Runtime**: ya viene con Windows 11.

## Compilar

```powershell
# La app y sus ayudantes (release: unos 20-30 min desde cero)
cargo build --release -j 4 -p pindesk -p pindesk-aligerar -p pindesk-lanzador -p pindesk-puente -p pindesk-mcp
```

- **Nunca dos `cargo` a la vez** sobre el mismo `target/`: se bloquean y el
  disco se llena. `-j 4` evita quedarse sin memoria.
- **No pasar `cargo fmt` a todo `apps/pindesk`**: mueve miles de líneas sin
  cambiar nada. Formatear solo los ficheros que se tocan.
- **Página del lector de notas** (`herramientas/lector-md`, CodeMirror 6): tras
  cambiar su `src/`, `npm ci && npm run build`. Escribe
  `apps/pindesk/recursos/lector-md.html`, que va dentro del `.exe`
  (`include_bytes!`), así que después hay que recompilar la app.

## Probar

```powershell
cargo test --workspace --no-fail-fast -- --test-threads=1
```

- `--test-threads=1` es obligatorio: `crates/pindesk-motor2d/tests/asignaciones.rs`
  cuenta asignaciones de memoria y en paralelo da fallos falsos.
- `--no-fail-fast` para que un binario rojo no esconda el resto.
- Con PinDesk abierta falla la prueba de instancia única de `pindesk-shell`
  (el mutex lo tiene la app de verdad): cerrarla antes.
- Para una sola parte: `cargo test -j 4 -p pindesk --bin pindesk -- --test-threads=1 <filtro>`.
- **Lector de notas sin compilar Rust:** `npm test` en `herramientas/lector-md`;
  para probar la página entera, abrir `lector-md.html` en Edge sin interfaz
  (Playwright) con un `window.chrome.webview` falso.

## Instalar y publicar

- **En el equipo de desarrollo:** `powershell -ExecutionPolicy Bypass -File herramientas\instalar.ps1`.
  Elige el `pindesk.exe` más nuevo de `target\*\release`, cierra la app con
  «Salir», lo copia a `%LOCALAPPDATA%\Programs\PinDesk` y la abre. No abrir el
  `.exe` de `target\` a mano: el arranque con Windows quedaría atado a esa carpeta.
- **Publicar:** una etiqueta `vX.Y.Z` lanza `.github/workflows/entrega.yml`,
  que sube el ZIP con `Instalar.cmd` a Releases.
- **Cerrar la app sin perder nada:** mensaje `WM_COMMAND` 25 a la ventana de
  mensajes `PinDeskVentanaMensajes`. Es una ventana *message-only*: se busca
  con `FindWindowEx(HWND_MESSAGE, …)`, no con `FindWindow`.
- **Plugin de Flow Launcher:** `instalar.ps1` lo pone al día, pero el
  `Instalar.cmd` del ZIP de Releases no. Tras instalar un Release, copiar
  `pindesk-lanzador.exe` del ZIP más `apps/pindesk-lanzador/plugin/*` a
  `%APPDATA%\FlowLauncher\Plugins\PinDesk\`. Comprobar con
  `pindesk-lanzador.exe buscar tareas`.

## Dónde vive cada cosa en el equipo

| Ruta | Qué |
|---|---|
| `%LOCALAPPDATA%\Programs\PinDesk` | La app instalada. |
| `%APPDATA%\PinDesk` | Datos: proyectos, ajustes (`pindesk.toml`), registro `pindesk.log.<fecha>`. |
| `%APPDATA%\FlowLauncher\Plugins\PinDesk` | El plugin `p`. |
| `%LOCALAPPDATA%\Programs\PixPin` | El PixPin **original** de DepthPixel: no tocar. |

## Forma de trabajar acordada

- Todo lo nuevo copia la lógica y la interfaz de **PinDesk Android**; desviarse
  solo cuando Windows obliga.
- Probar en la app real antes de dar algo por hecho, y avisar al terminar.
- Tras cada unión: commit, push y apuntar aquí lo que cambie de este documento.
