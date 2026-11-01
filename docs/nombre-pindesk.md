# La app se llama PinDesk (antes PixPin Max)

Desde noviembre de 2026 la app de Windows se llama **PinDesk**. El nombre «PixPin» es de otro
producto (el PixPin de DepthPixel, que en algunos equipos ya está instalado y registra
`pixpin://`), así que todo lo que **ve el usuario** dice PinDesk, sin «Max».

La regla: **cambia el nombre visible; no cambia nada que guarde datos, que lea otro aparato o
que ya esté registrado en Windows**. Quien actualiza no pierde nada y una versión vieja (en el
PC o en el móvil) sigue entendiéndose con una nueva.

## Qué cambió

| Dónde | Antes | Ahora |
|---|---|---|
| Nombre en los 14 idiomas (`app-nombre` y todos los textos de `crates/pixpin-store/i18n/*/main.ftl`, y la copia de `apple/nucleo/pixpin-store/i18n/`) | PixPin Max / PixPin | PinDesk (en coreano y turco con las partículas ajustadas: «PinDesk를», «PinDesk'te») |
| Ejecutable (`[[bin]]` de `apps/pixpin/Cargo.toml`) | `pixpinmax.exe` | `pindesk.exe` (el paquete y el crate siguen siendo `pixpin`) |
| Carpeta de instalación | `%LOCALAPPDATA%\Programs\PixPinMax` | `%LOCALAPPDATA%\Programs\PinDesk` |
| ZIP de la entrega (`.github/workflows/entrega.yml`) | `PixPinMax-windows-x64-<sha>.zip`, carpeta `PixPinMax`, «PixPin Max v…» | `PinDesk-windows-x64-<sha>.zip`, carpeta `PinDesk`, «PinDesk v…» |
| Arranque con Windows (`HKCU\…\Run`) | valor `PixPinMax` | valor `PinDesk`; la app borra `PixPinMax` en cada arranque (`pixpin_shell::arranque`) |
| Acceso del menú Inicio | `PixPin Max.lnk` | `PinDesk.lnk` (`Instalar.cmd` e `instalar.ps1` quitan el viejo) |
| «Abrir con» | `Applications\pixpinmax.exe`, «PixPin Max» | `Applications\pindesk.exe`, «PinDesk»; se borra la clave vieja (`pixpin_shell::abrir_con`) |
| Apps predeterminadas (`RegisteredApplications`) | valor `PixPin Max` | valor `PinDesk`; se borra el viejo (`pixpin_shell::asociaciones`). Los tipos se llaman «Imagen (PinDesk)», «Video (PinDesk)», «Audio (PinDesk)» |
| Notificaciones, bandeja, Jump List, títulos de ventana, ayuda de la consola (`pindesk ayuda`) | PixPin Max | PinDesk |
| Exportaciones (HTML del timeline, de la clase, del lienzo y del plano; `docProps` del `.docx`) | `generator`/`Application` «PixPin» | «PinDesk» |
| Asistente (instrucciones de sistema, ayuda, herramientas) y servidor MCP (`serverInfo.title`) | «el asistente de PixPin» | «el asistente de PinDesk» |
| Plugin de Flow (`plugin.json`), extensión del navegador (`manifest.json`, menú, avisos) | PixPin Max | PinDesk (Flow: carpeta `Plugins\PinDesk`; el `ID` es el mismo) |
| Scripts (`instalar.ps1`, `integraciones.ps1`, `PixPin.psm1`, `pixpin.ahk`, `cortafuegos-sincro.ps1`, paquete de identidad) | `pixpinmax.exe`, «PixPin Max» | `pindesk.exe` (y si no está, el `pixpinmax.exe` de antes), «PinDesk» |

Los programas que buscan la app (el puente del navegador, el servidor MCP, el plugin de Flow,
`PixPin.psm1`, `pixpin.ahk`) prueban, en este orden: `pindesk.exe` y `pixpinmax.exe` junto a
ellos, `Programs\PinDesk\pindesk.exe` y `Programs\PixPinMax\pixpinmax.exe`
(`pixpin_sistema::entorno::exe_de_la_app`). La consola reconoce `pixpin://` como suyo si lo abre
cualquiera de los dos exe.

## La migración al actualizar

`Instalar.cmd` (el del ZIP) y `herramientas\instalar.ps1` hacen lo mismo:

1. Cierran `pindesk.exe` **y** `pixpinmax.exe`.
2. Copian todo a `Programs\PinDesk`.
3. Si hay un valor `PixPinMax` en `Run`, ponen `PinDesk` apuntando al exe nuevo y quitan el viejo.
4. Quitan los `.exe` de `Programs\PixPinMax` — **salvo si allí hay un `pixpinmax.toml`**: es una
   copia portable con sus datos y no se toca.
5. Cambian el acceso del menú Inicio «PixPin Max» por «PinDesk».
6. Abren `pindesk.exe`, que al arrancar vuelve a escribir arranque, «Abrir con» y apps
   predeterminadas con el nombre y la ruta nuevos, y borra los viejos.

`instalar.ps1` además pasa la marca de «predeterminada ya ofrecida», instala el plugin de Flow en
`Plugins\PinDesk` (quita cualquier otra carpeta con el mismo `ID`), vuelve a registrar el puente del
navegador con la ruta nueva y deja al día `PixPin.psm1` y `pixpin.ahk` también en la carpeta vieja,
para que un `Import-Module …\Programs\PixPinMax\PixPin.psm1` del perfil siga funcionando.

Lo que el usuario puede notar una vez:

- **El cortafuegos** vuelve a preguntar «¿Permitir?» la primera vez que PinDesk escucha para
  Sincronizar: la regla de Windows va por programa y el exe es otro. `cortafuegos-sincro.ps1` quita
  sus reglas viejas («PixPin Max - Sincronizar») y pone las nuevas.
- **El paquete de identidad** (Compartir y menú de Windows 11, opcional) se registró con
  `-ExternalLocation Programs\PixPinMax`: hay que repetir `paquete-identidad.ps1 -Registrar` para
  que apunte a `Programs\PinDesk`. El certificado sirve el mismo.
- **La app predeterminada**: la elección del usuario (`UserChoice`) va a los ProgID, que no
  cambian, así que se mantiene; en Configuración sale como «PinDesk».

## Qué se queda igual, y por qué

| Se queda | Por qué |
|---|---|
| La carpeta de datos `%APPDATA%\PixPinMax` (y `%LOCALAPPDATA%\PixPinMax` de claves y modelos), el fichero `pixpinmax.toml` y el registro `pixpinmax.log` | Son los datos de quien ya usaba la app; el modo portable se reconoce por `pixpinmax.toml` junto al exe. Moverlos sería una migración con riesgo y sin nada que ganar: el usuario no ve esas rutas |
| La extensión `.pixpin` y su formato, los `.leccion`, el cuaderno, el índice, `aplicacion: "pixpin-max"` y `"source": "pixpin-max"` en los JSON | Los leen Android, Mac e iPad y las versiones viejas |
| Todo Sincronizar: el tipo mDNS `_pixpin._tcp`, los nombres de instancia «PixPin <equipo>», la baliza, las etiquetas HMAC, la versión del protocolo, los nombres de campo | Lo tienen que entender el móvil y los demás PC; cambiarlos rompería la sincronización con quien no haya actualizado |
| Las capas de los PDF anotados («PixPin · hoja N», la marca `(PixPin` que busca `cocido.rs`) | Las escribe y las reconoce también el móvil |
| Los mazos de Anki «PixPin::…» | Son datos del usuario en Anki: un nombre nuevo partiría sus mazos en dos |
| Los protocolos `pixpinmax://` y `pixpin://` | Están en enlaces, notificaciones guardadas, Lectura Veloz y el móvil |
| El mutex de instancia única `Local\PixPinMax-instancia-unica`, la clase `PixPinMaxVentanaMensajes` y las demás clases de ventana `PixPin…` | Así un `pindesk.exe` nuevo ve al `pixpinmax.exe` viejo que aún corre (y al revés), y el puente, el MCP, Flow e `instalar.ps1` de cualquier versión encuentran la app |
| El AUMID `PixPinMax.App` y los ProgID `PixPinMax.Imagen/Video/Audio`, la clave del menú del Explorador `*\shell\PixPinMax`, `Software\PixPinMax\Capabilities` | Windows guarda con ellos el silencio de las notificaciones y la app predeterminada del usuario; con la misma clave el menú se sobrescribe en vez de duplicarse |
| La identidad del paquete MSIX (`ForgePixPin.PixPinMax`, `CN=PixPin Max Local`, aplicación `PixPinMax`) y el `assemblyIdentity` de `pixpinmax.manifest` | Son la identidad ya registrada y el certificado ya instalado; lo visible (`DisplayName`) sí dice PinDesk |
| El anfitrión del navegador `com.pixpinmax.puente` | Lo lleva escrito la extensión publicada |
| La impresora «PixPin» y su puerto `PixPin.pdf` | Ya está instalada en los equipos que la pusieron; renombrarla pide administrador. Se puede hacer más adelante con un `-Quitar` y volver a ponerla |
| Los agentes HTTP («PixPin», `PixPinMax/1.0`) y la descripción de DPAPI | Nadie los ve; la descripción de DPAPI no cuenta para descifrar |
| Los nombres de crates y paquetes (`pixpin-*`), los identificadores de Rust, los ficheros `pixpinmax.rc`, `pixpinmax.ico`, `pixpinmax.manifest`, `PixPin.psm1`, `pixpin.ahk` | Internos; el módulo y la librería se llaman desde scripts de los usuarios |
| El nombre del equipo por omisión ya guardado en `identidad.json` | Solo se usa al crearla; las nuevas dicen «PinDesk» |
| Planes, investigaciones y medidas viejas (`docs/superpowers/`, `docs/investigacion/`, `medidas/`) | Son historia: cuentan lo que pasó con el nombre de entonces |

## Para los agentes de Android, Mac e iPad

- **Cambiad solo el nombre visible a PinDesk** (título de la app, `app_name`, textos, avisos, el
  asistente). En los textos del PC ya pone «PinDesk en el móvil», «abre PinDesk en el móvil».
- **No cambiéis nada de lo de la tabla de arriba**: formato `.pixpin`, cuaderno, índice,
  `_pixpin._tcp`, el `serviceName` del anuncio, la baliza y sus etiquetas, la versión del
  protocolo, `pixpin://`, las capas «PixPin · hoja N», los nombres de las fotos `PixPin_…` y la
  carpeta `Pictures/PixPin`. El PC nuevo y el viejo siguen buscando eso mismo.
- Si el anuncio mDNS del móvil usa el nombre de la app como nombre de instancia, puede seguir
  diciendo lo que dice: el PC lo enseña pero no lo compara.
- La copia del núcleo en `apple/nucleo/` ya tiene los catálogos (`i18n`) con PinDesk; el resto de
  textos de `apple/nucleo` y `apple/por-traducir` (el pie «Exportado con PixPin Max», el `generator`
  de los HTML, las instrucciones del asistente) los cambia el agente de Apple al traerlos, con la
  misma regla.

## Para la ficha de Microsoft Store

- **Nombre**: PinDesk (ya reservado en Partner Center como «PinDesk»; no usar «PixPin» en el nombre, la
  descripción ni las palabras clave).
- **Descripción corta**: «PinDesk: capturas, pines flotantes, lienzo y proyectos con forma de chat
  para Windows. Se sincroniza con el móvil por Wi‑Fi, sin nube ni cuentas».
- Si se publica como MSIX completo, la identidad la da la Store (`Name`/`Publisher` nuevos): el
  `AUMID`, el elemento `<msix>` de `pixpinmax.manifest` y el paquete de identidad pasarían a esa
  identidad. Los datos (`%APPDATA%\PixPinMax`) no cambian, pero un MSIX virtualiza `AppData`:
  hay que comprobar que lee la carpeta de siempre (la capacidad `unvirtualizedResources`, que ya
  pide el paquete de identidad, y desactivar la virtualización de escritura del sistema de
  ficheros) antes de publicar.
- Capturas y vídeo: que el título de las ventanas y la bandeja digan PinDesk (ya lo dicen).
