# El formato y los nombres de PinDesk (para Android, Mac e iPad)

Desde noviembre de 2026 la app se llama **PinDesk** en todos los aparatos y su formato propio es
**`.pindesk`**. Antes había nombres de dos épocas: «PixPin» (el de Android, que choca con el PixPin
de DepthPixel) y «PixPin Max» (el del PC). Este documento es la lista exacta de lo que cambia, lo
que se sigue aceptando y cuándo se puede dejar de aceptar.

## La regla de la transición

1. **Se escribe con el nombre nuevo, se lee también el viejo.** Todo lo que uno escribe y otro
   lee: el nuevo se escribe en cuanto el otro lado ya lo entiende (o cuando escribir el viejo no
   sirve de nada); el viejo se sigue aceptando siempre que llegue.
2. **En la red, los dos a la vez.** Mientras haya aparatos sin actualizar, se anuncia y se busca
   con el nombre nuevo **y** con el viejo, y si un aparato sale con los dos se usa el nuevo.
3. **Lo que otro aparato tiene que leer y aún no sabe leer se sigue escribiendo con el nombre
   viejo** (fase B de la tabla de abajo). Se cambia cuando todos lean el nuevo.
4. **Se migra una vez** lo que vive en un solo aparato (carpetas, registro de Windows), sin
   borrar nada si algo falla.
5. **Los nombres viejos se quitan cuando todos los aparatos hablen el nuevo** (ver «Cuándo se
   pueden quitar los viejos»).

## Lo que ya escribe el PC con el nombre nuevo (fase A)

Los otros aparatos tienen que **aceptar los dos** (el nuevo y el viejo) y, cuando lo hagan,
escribir también el nuevo.

| Qué | Antes | Ahora | Se sigue aceptando |
|---|---|---|---|
| Extensión del proyecto en un fichero (exportar, «Compartir → PinDesk», enviar por Wi‑Fi, recibir del móvil) | `.pixpin` | `.pindesk` | `.pixpin` (abrir, importar, recibir, «Abrir con») |
| `manifest.json` del paquete: `formato` | `"pixpin"` | `"pindesk"` | `"pixpin"` (nadie lo comprueba al abrir; si Android/Mac lo miran, que acepten los dos) |
| `manifest.json` del paquete: `aplicacion` (Windows) | `"pixpin-max"` | `"pindesk"` | cualquiera (es informativo; Android escribe `"pixpin-android"`, puede pasar a `"pindesk-android"`) |
| `"source"` de los `.excalidraw` que escribe el PC | `"pixpin-max"`, `"pixpin-ia"` | `"pindesk"`, `"pindesk-ia"` | cualquiera (informativo) |
| `customData.origen` de lo que dibuja el asistente | `"pixpin-ia"` | `"pindesk-ia"` | cualquiera (informativo) |
| `customData` del emoji en el lienzo | `{"pixpin":"emoji"}` | `{"pindesk":"emoji","pixpin":"emoji"}` (los dos, por los móviles sin actualizar) | `pindesk` o `pixpin` |
| Enlaces de la app | `pixpin://…` (Android), `pixpinmax://…` (PC) | `pindesk://…` (mismos verbos, campos y seña `pindesk://atajo/…`) | `pixpin://` y `pixpinmax://`, con las mismas reglas (lista blanca, firma) |
| Señas de los botones de los avisos | `pixpin:pedido?json=…` / `pixpinmax:pedido?json=…` | `pindesk:pedido?json=…` | las dos de antes |
| Versión del protocolo de pedidos | `{"pixpin": 1, "accion": …}` | `{"pindesk": 1, "accion": …}` | `"pixpin": 1` (si vienen los dos, manda `pindesk`) |
| Capas de los PDF anotados | `PixPin · hoja N` | `PinDesk · hoja N` | las dos; `cocido.rs` reconoce `PinDesk` y `PixPin` en hexadecimal UTF‑16, mayúsculas y literal |
| Mazos de Anki | `PixPin::<chat>`, etiqueta `pixpin` | `PinDesk::<chat>`, etiqueta `pindesk` | — (un mazo nuevo en Anki; los de antes se quedan como están) |
| mDNS: tipos de servicio | `_pixpin._tcp`, `_pixpinenvio._tcp`, `_pixpinrecibe._tcp`, `_pixpinchat._tcp` | `_pindesk._tcp`, `_pindeskenvio._tcp`, `_pindeskrecibe._tcp`, `_pindeskchat._tcp` | **el PC anuncia y busca los dos a la vez** (`pindesk_shell::mdns`: `tipo_de_antes`, `juntar`) |
| mDNS: nombre de la instancia | `PixPin <equipo>` | `PinDesk <equipo>` | cualquiera: se enseña, no se compara (se compara el TXT `g`/`id`) |
| QR de envío y de chat compartido (lectura) | `pixpin-envio:`, `pixpin-recibe:`, `pixpin-chat:` | se aceptan también `pindesk-envio:`, `pindesk-recibe:`, `pindesk-chat:` | los de antes |
| Lectura Veloz: enlace de PinDesk a Lectura Veloz | `lecturaveloz://pixpin/abrir?v=1&…` | `lecturaveloz://pindesk/abrir?v=1&…` | **Lectura Veloz tiene que aceptar los dos anfitriones** |
| Lectura Veloz: enlace de vuelta | `pixpinmax://lectura-veloz?…` | `pindesk://lectura-veloz?…` | `pixpinmax://` y `pixpin://` |

## Lo que se sigue escribiendo con el nombre viejo (fase B)

El PC ya **lee** el nuevo de los que lo tienen (marcado «sí»), pero **escribe** el viejo porque lo
lee un aparato que puede no estar actualizado. Cuando Android, Mac e iPad lean el nuevo, se pasa a
escribir el nuevo (y se apunta aquí).

| Qué | Se escribe (por ahora) | Nombre nuevo | El PC ya lee el nuevo |
|---|---|---|---|
| QR que enseña el PC (enviar, recibir, invitar a un chat) | `pixpin-envio:1:…`, `pixpin-recibe:1:…`, `pixpin-chat:1:…` | `pindesk-envio:`, `pindesk-recibe:`, `pindesk-chat:` | sí |
| Rutas portátiles dentro de los datos (cuaderno, notas, tareas, lecciones, memoria) | `pixpin:files/<ruta>` | `pindesk:files/<ruta>` | no (hay que enseñárselo antes a todos) |
| Ficheros al lado de un documento y dentro de `archivos/` de un chat | `<doc>.pixpin-lectura` (y su cabecera `pixpin-lectura N`), `<doc>.pixpin-anotado/`, `<doc>.pixpin-marcos`, `<doc>.pixpin-vistas`, `<doc>.pixpin-marcas-texto.json` (2-nov-2026: las marcas de texto de una nota o una página web; en un adjunto van en `anot-<uid>.marcas-texto.json`, ver `docs/sincronizar-marcos-vistas-y-tinta.md` §3.10), `<lienzo>.excalidraw.pixpin-marcas`, `<lienzo>.excalidraw.pixpin-vistas` | `.pindesk-lectura`, `.pindesk-anotado/`… | no |
| Dibujos sobre una imagen y listas sueltas (formato interno) | `.pixpin2d`, `.pixpin-lista` | `.pindesk2d`, `.pindesk-lista` | no |
| Fotos y carpeta del móvil | `PixPin_<fecha>.png`, `Pictures/PixPin` | los decide Android | — (el PC los acepta con cualquier nombre) |

## Lo que no cambia (y por qué)

| Qué | Por qué |
|---|---|
| Las «magias» del cable y la baliza (`PXS1`, `PXB1`, `PXC1`, `PXI1`) y la versión del protocolo | No dicen «PixPin»: son cuatro bytes. Cambiarlas rompería todo sin ganar nada |
| La sal de la clave del grupo (`pixpin-sincro-grupo`) y los textos firmados de los chats compartidos (`pixpin-chat|entra|…`, `pixpin-chat|sale|…`) | La clave de cada grupo y cada firma salen de ellos: cambiarlos obligaría a volver a emparejar todos los aparatos. No se ven. Si algún día se cambian, será con una versión nueva del protocolo que diga las dos |
| La entropía de DPAPI de las claves guardadas en el PC (`PixPinMax/asistente/nube/v1`, `PixPinMax/typesafe/v1`) | Sin ella no se descifran las claves que ya tiene el usuario. No depende de la carpeta (la carpeta sí pasa a `PinDesk`) |
| El `ID` del plugin de Flow Launcher | Es un GUID; con otro, Flow lo tomaría por un plugin nuevo y perdería sus ajustes |
| La «key» y el id de la extensión del navegador | Con otra «key» cambiaría el id y el anfitrión tendría que volver a autorizarla |

## Solo en el PC: la migración al actualizar

La hace `pindesk.exe` al arrancar (`pindesk_sistema::nombre::migrar_al_arrancar`), antes de abrir
ningún fichero, y los instaladores (`Instalar.cmd`, `herramientas/instalar.ps1`):

| Qué | Antes | Ahora | Cómo pasa |
|---|---|---|---|
| Datos | `%APPDATA%\PixPinMax` | `%APPDATA%\PinDesk` | Si no hay carpeta nueva y sí vieja: se **renombra** (misma unidad, instantáneo). Si no se puede (un fichero abierto por otro programa), se **copia** a `PinDesk.migrando` y, entera, se renombra a `PinDesk`; la vieja se queda con sus datos. En los dos casos queda en la vieja `MOVIDO-A-PINDESK.txt`. Si la copia falla se borra solo la copia a medias y se sigue usando la vieja (se apunta en el registro) |
| Claves, modelos, clave de los enlaces | `%LOCALAPPDATA%\PixPinMax` | `%LOCALAPPDATA%\PinDesk` | Igual. Las claves cifradas con DPAPI se siguen descifrando: la entropía no cambia y no depende de la ruta |
| Ajustes | `pixpinmax.toml` | `pindesk.toml` | Se renombra dentro de la carpeta de datos (y junto al exe en modo portable). Mientras no, se lee el viejo |
| Modo portable | `pixpinmax.toml` junto al exe | `pindesk.toml` (o el viejo) junto al exe | Los dos activan el modo portable |
| Registro | `registros\pixpinmax.log` | `registros\pindesk.log` | Los viejos se quedan |
| Impresora | «PixPin», puerto `…\PixPinMax\cache\impresora\PixPin.pdf` | «PinDesk», puerto `…\PinDesk\cache\impresora\PinDesk.pdf` | Tras la mudanza, `PixPinMax\cache\impresora` es una unión a la carpeta nueva y la app recoge `PinDesk.pdf` y `PixPin.pdf`: la impresora vieja sigue funcionando. `impresora-pindesk.ps1` (administrador) pone la nueva y quita la vieja |
| Ayudantes | `pixpin-mcp.exe`, `pixpin-puente.exe`, `pixpin-lanzador.exe`, `pixpin-aligerar.exe` | `pindesk-mcp.exe`, `pindesk-puente.exe`, `pindesk-lanzador.exe`, `pindesk-aligerar.exe` | Los instaladores cierran y quitan los viejos. `pixpin-mcp.exe` se deja como copia del nuevo solo si la configuración de Claude aún lo nombra (y se avisa) |
| Notificaciones (AUMID) | `PixPinMax.App` | `PinDesk.App` | La app escribe la nueva y borra la vieja al arrancar. El usuario puede tener que volver a permitir o silenciar los avisos |
| Tipos de fichero (ProgID) y capacidades | `PixPinMax.Imagen/Video/Audio`, `Software\PixPinMax\Capabilities` | `PinDesk.*`, `Software\PinDesk\Capabilities` | La app borra los viejos al arrancar; `instalar.ps1` vuelve a ofrecer «Establecer como predeterminada» una vez |
| Protocolos | `pixpinmax:` (y `pixpin:` si estaba libre) | `pindesk:` y `pixpinmax:` | `pixpin:` ya no se registra; si lo tenía PinDesk, se suelta (para el PixPin original) |
| Menú del Explorador | `*\shell\PixPinMax` | `*\shell\PinDesk` | `integraciones.ps1` quita la clave vieja |
| Navegador | anfitrión `com.pixpinmax.puente` (`pixpin-puente.json`) | `com.pindesk.puente` (`pindesk-puente.json`) | `registrar-navegador.ps1` quita el viejo y pone el nuevo. Hay que recargar la extensión |
| Paquete de identidad | `ForgePixPin.PixPinMax`, `CN=PixPin Max Local`, aplicación `PixPinMax` | `PinDesk`, `CN=PinDesk Local`, aplicación `PinDesk` | `paquete-identidad.ps1 -Todo` quita el viejo y registra el nuevo |
| Instancia única y ventanas | mutex `Local\PixPinMax-instancia-unica`, clase `PixPinMaxVentanaMensajes`, clases `PixPin…` | `Local\PinDesk-instancia-unica`, `PinDeskVentanaMensajes`, `PinDesk…` | La app mira también el mutex viejo (un `pixpinmax.exe` que aún corre) y todos los que buscan la app (consola, MCP, puente, Flow, scripts, instalador) prueban la clase nueva y luego la vieja |
| PowerShell y AutoHotkey | `PixPin.psm1` (`New-PixPinTarea`…), `pixpin.ahk` (`PixPinTarea()`…), `$env:PIXPIN_EXE` | `PinDesk.psm1` (`New-PinDeskTarea`…), `pindesk.ahk` (`PinDeskTarea()`…), `$env:PINDESK_EXE` | Los nombres de antes siguen como alias dentro de los nuevos; los ficheros viejos se quitan |
| Flow Launcher | icono `Images\pixpin.png` | `Images\pindesk.png` | `instalar.ps1` reemplaza la carpeta del plugin |

## Para los agentes de Android, Mac e iPad

Lo mínimo, en este orden:

1. **Leer los dos**: `.pindesk` y `.pixpin`; `pindesk://` y `pixpin://`; `formato` `"pindesk"` o
   `"pixpin"`; `customData.pindesk` o `.pixpin`; las capas `PinDesk · hoja N` y `PixPin · hoja N`;
   los QR `pindesk-envio/recibe/chat` y `pixpin-…`; `"pindesk": 1` y `"pixpin": 1` en los pedidos.
2. **Anunciar y buscar por mDNS los dos tipos** (`_pindesk…` y `_pixpin…`), como hace el PC.
3. **Escribir el nuevo**: exportar `.pindesk` (con `formato: "pindesk"`), los enlaces
   `pindesk://`, el nombre de la instancia `PinDesk <aparato>`, las capas `PinDesk · hoja N`.
4. **Fase B**: leer `pindesk:files/` y los ficheros `.pindesk-*` cuando el PC los empiece a
   escribir (se avisará aquí, con fecha).
5. Lectura Veloz: aceptar `lecturaveloz://pindesk/abrir` además de `lecturaveloz://pixpin/abrir`
   y mandar de vuelta `pindesk://lectura-veloz?…` (el PC entiende también los de antes).
6. **El enlace `pindesk://emparejar`** (3-nov-2026): abrirlo desde la cámara y unirse con él
   ([`emparejar.md`](emparejar.md) §3 y su lista para Android).
7. **La nube de los chats compartidos** (1-nov-2026, `docs/nube.md`): un chat compartido puede
   ir por la red local o por la nube (`deltas1`: solo los trazos nuevos de un lienzo). El protocolo
   sigue llamándose `pixpin-nube` (llaves `pxn_`, `/nube/api/v1`, `pixpin-nube|yo|…`); el modo de
   cada chat va en `sincro/nube/chats.json`, **no** en `compartidos.json`.

## El enlace de emparejar (3-nov-2026)

El PC enseña, al tocar el código del grupo en Sincronizar, un QR con un enlace que se abre con
**cualquier cámara** y lleva a la app a unirse sola. La especificación completa (y lo que tiene que
hacer Android: intent-filter, preguntar, llamar a las direcciones del enlace y unirse también por
la baliza) está en [`emparejar.md`](emparejar.md) §3.

```
pindesk://emparejar?v=1&c=<codigo>&h=<ip>[,<ip>…]&p=<puerto>&n=<nombre>&e=<ms>
```

| Qué | Se escribe | Se acepta |
|---|---|---|
| Esquema y anfitrión | `pindesk://emparejar` | también `pixpin://emparejar`, sin distinguir mayúsculas |
| `c` (obligatorio) | el código limpio, 10 signos | con guion y en minúsculas |
| `h`, `p`, `n`, `e` | IPv4 privadas (hasta 4), puerto, nombre cifrado como URL, caducidad en ms | cualquiera puede faltar; lo desconocido se ignora |

Rust: `pindesk_sincro::emparejar::Enlace` (con sus pruebas, para copiarlas en Kotlin y Swift).

## Cuándo se pueden quitar los viejos

Los nombres viejos se pueden dejar de **escribir** (fase B) y de **aceptar** cuando **todos los
aparatos hablen el nuevo**: el PC, Android, Mac e iPad publicados con lo de arriba, y unas semanas
para que los usuarios actualicen. Entonces, en este orden:

1. Pasar a escribir los de la fase B (QR `pindesk-…`, `pindesk:files/`, `.pindesk-*`) y quitar el
   `customData.pixpin` del emoji.
2. Dejar de anunciar `_pixpin…` por mDNS (seguir buscándolo una versión más).
3. Dejar de registrar `pixpinmax:` en el PC; seguir entendiendo los enlaces viejos que lleguen.
4. Quitar las ramas de compatibilidad del PC: `CLASE_MENSAJES_VIEJA`, el mutex viejo, `AUMID_VIEJO`,
   `PROGID_VIEJOS`, los alias de `PinDesk.psm1` y `pindesk.ahk`, el `PixPinMax` de
   `pindesk_sistema::nombre` (solo cuando ya no quede nadie sin migrar), el `pixpin-mcp.exe` de
   reserva.

Lo de «Lo que no cambia» solo se toca con una versión nueva del protocolo acordada entre todos.
