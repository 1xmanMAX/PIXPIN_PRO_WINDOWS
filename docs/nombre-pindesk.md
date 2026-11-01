# La app se llama PinDesk (antes PixPin Max)

Desde noviembre de 2026 la app de Windows se llama **PinDesk**, y su formato propio, sus enlaces,
sus carpetas y sus registros en Windows también: `.pindesk`, `pindesk://`, `%APPDATA%\PinDesk`,
`pindesk.toml`, `PinDesk.App`… El nombre «PixPin» es de otro producto (el PixPin de DepthPixel, que
en algunos equipos ya está instalado y registra `pixpin://`).

**La lista exacta de nombres viejos y nuevos, lo que se sigue aceptando y cuándo se puede dejar de
aceptar está en [`formato-pindesk.md`](formato-pindesk.md).** Aquí, lo que hay que saber en el PC.

## La regla

Se escribe con el nombre nuevo, se lee también el viejo y se migra una vez. Quien actualiza no
pierde nada, y una versión vieja en otro aparato (el móvil, otro PC) se sigue entendiendo con esta.
Lo que otro aparato tiene que leer y aún no sabe leer (los QR que enseña el PC, las rutas
`pixpin:files/`, los ficheros `.pixpin-*` al lado de los documentos) se sigue escribiendo con el
nombre viejo hasta que todos lean el nuevo (fase B de `formato-pindesk.md`).

## Dos etapas

1. **Primera (2026-11, «PinDesk» visible)**: el exe pasó de `pixpinmax.exe` a `pindesk.exe`, la
   carpeta de instalación de `Programs\PixPinMax` a `Programs\PinDesk`, el valor de arranque, el
   acceso del menú Inicio, «Abrir con» y `RegisteredApplications` a «PinDesk», y todos los textos.
2. **Segunda (esta)**: todo lo demás. La extensión `.pindesk`, los enlaces `pindesk://`, las
   carpetas de datos, `pindesk.toml`, el AUMID `PinDesk.App`, los ProgID `PinDesk.*`, el menú del
   Explorador, el anfitrión del navegador `com.pindesk.puente`, el paquete de identidad `PinDesk`,
   la impresora «PinDesk», el mutex y las clases de ventana `PinDesk…`, los ayudantes
   `pindesk-*.exe`, `PinDesk.psm1` y `pindesk.ahk`, Sincronizar (`_pindesk._tcp`, a la vez que el
   de antes), las capas `PinDesk · hoja N` y los mazos `PinDesk::…`.

## Qué hace la app al arrancar

- Pasa `%APPDATA%\PixPinMax` y `%LOCALAPPDATA%\PixPinMax` a `PinDesk` (renombrar; si no se puede,
  copiar y dejar la vieja con sus datos y `MOVIDO-A-PINDESK.txt`; si falla, sigue con la vieja) y
  `pixpinmax.toml` a `pindesk.toml` (`pindesk_sistema::nombre`). Se apunta en `pindesk.log`.
- Escribe sus registros de Windows con los nombres nuevos y borra los viejos: `PixPinMax.App`, los
  ProgID `PixPinMax.*`, `Software\PixPinMax`, «PixPin Max» en `RegisteredApplications`, el valor
  `PixPinMax` de `Run`, `Applications\pixpinmax.exe` y, si lo tenía una versión vieja, `pixpin://`.
- Ve a un `pixpinmax.exe` que aún corre (el mutex y la ventana de antes) y le pasa lo suyo.

## Qué hacen los instaladores

`Instalar.cmd` (el del ZIP) y `herramientas\instalar.ps1`:

1. Cierran `pindesk.exe` y `pixpinmax.exe` (y los ayudantes, con los dos nombres).
2. Copian todo a `Programs\PinDesk` y quitan los `pixpin-*.exe` de ahí y de `Programs\PixPinMax`
   (salvo si allí hay un `pixpinmax.toml`: es una copia portable con sus datos y no se toca).
   `pixpin-mcp.exe` se queda como copia de `pindesk-mcp.exe` solo si la configuración de Claude
   aún lo nombra, y se avisa.
3. Pasan el valor `PixPinMax` de `Run` a `PinDesk` y el acceso «PixPin Max» del menú Inicio a
   «PinDesk».
4. Abren `pindesk.exe`, que hace lo de arriba.

`instalar.ps1` además pone el plugin de Flow (`Plugins\PinDesk`, quitando cualquier carpeta con el
mismo `ID`), registra el anfitrión del navegador `com.pindesk.puente` (y quita
`com.pixpinmax.puente`), pone `PinDesk.psm1` y `pindesk.ahk` junto al exe (y quita `PixPin.psm1` y
`pixpin.ahk`) y vuelve a ofrecer una vez «Establecer como predeterminada».

## Lo que el usuario puede notar una vez

- **La app predeterminada**: los ProgID son otros, así que hay que volver a elegir PinDesk en
  Configuración (`instalar.ps1` abre la página).
- **Las notificaciones**: la identidad es otra; si se silenciaron, hay que volver a hacerlo.
- **El cortafuegos** pregunta otra vez la primera vez que PinDesk escucha para Sincronizar
  (`cortafuegos-sincro.ps1` quita las reglas viejas y pone las nuevas).
- **El paquete de identidad** (Compartir y menú de Windows 11, opcional): repetir
  `paquete-identidad.ps1 -Todo`, que quita `ForgePixPin.PixPinMax` y registra `PinDesk`.
- **La impresora**: la de antes sigue funcionando; `impresora-pindesk.ps1` la cambia por
  «PinDesk» (pide administrador).
- **La extensión del navegador**: recargarla en `chrome://extensions` (habla con
  `com.pindesk.puente`).
- **Claude (MCP)**: cambiar en su configuración `pixpin-mcp.exe` por `pindesk-mcp.exe`.
- **Scripts propios**: `Import-Module …\PinDesk.psm1` y `#Include …\pindesk.ahk`; las funciones
  de antes (`New-PixPinTarea`, `PixPinTarea()`) siguen valiendo como alias.
- **Anki**: los mazos nuevos son `PinDesk::<chat>`; los `PixPin::…` ya exportados se quedan.

## Lo que sigue con el nombre viejo, y por qué

Ver «Lo que no cambia» y «fase B» en [`formato-pindesk.md`](formato-pindesk.md): las magias y la sal
del protocolo de Sincronizar, la entropía de DPAPI de las claves guardadas, los QR que enseña el
PC, las rutas `pixpin:files/` y los ficheros `.pixpin-*` que leen los otros aparatos.

## Por dentro

Los crates y paquetes se llaman `pindesk-*` (`crates/pindesk-*`, `apps/pindesk`, `apps/pindesk-*`,
y la copia de `apple/nucleo/`), con sus rutas `pindesk_*::`, los identificadores de Rust, los
ficheros `apps/pindesk/pindesk.rc/.manifest` y `recursos/pindesk.ico`, y las variables de
entorno `PINDESK_*` (`PINDESK_SIN_RECURSOS`, `PINDESK_RAIZ`, `PINDESK_MUESTRAS`…). Los ejecutables
aceptan las `PIXPIN_*` de antes (`pindesk_sistema::nombre::aceptar_variables_de_antes`), y el
`build.rs` de la app también `PIXPIN_SIN_RECURSOS`; las pruebas solo miran las nuevas.

Se quedan con el nombre de antes, a propósito: `apple/nucleo/pixpin-nucleo` (su biblioteca
`pixpin_nucleo` da nombre a los símbolos de UniFFI y al `pixpin_nucleo.swift` que usa la app de
Swift: lo cambia el agente de Apple cuando regenere los enlaces) y el paquete Swift `PixPinCore`.
Y, como todo lo que lee otro aparato, los nombres de la fase B.

Los planes, investigaciones y medidas viejas (`docs/superpowers/`, `docs/investigacion/`,
`medidas/`) son historia: cuentan lo que pasó con el nombre de entonces.

## Para la ficha de Microsoft Store

- **Nombre**: PinDesk (ya reservado en Partner Center como «PinDesk»; no usar «PixPin» en el
  nombre, la descripción ni las palabras clave).
- **Descripción corta**: «PinDesk: capturas, pines flotantes, lienzo y proyectos con forma de chat
  para Windows. Se sincroniza con el móvil por Wi‑Fi, sin nube ni cuentas».
- La rama `microsoft-store` lleva su propio `AppxManifest` (identidad de la Store): los ayudantes
  ahora se llaman `pindesk-*.exe` (`tienda.yml` copia el nombre que haya).
- Un MSIX virtualiza `AppData`: hay que comprobar que lee `%APPDATA%\PinDesk` (la capacidad
  `unvirtualizedResources` y sin virtualizar la escritura del sistema de ficheros) antes de
  publicar.
