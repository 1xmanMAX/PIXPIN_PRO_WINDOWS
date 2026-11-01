# PinDesk en la Microsoft Store (rama `microsoft-store`)

Este documento solo existe en la rama `microsoft-store` (ver `docs/ramas.md`).
Explica como se arma el paquete MSIX, como se sube a Partner Center, que
cambia respecto a `main` y como se une `main` sin romper nada.

## Identidad (Partner Center › PinDesk › Identidad del producto)

| Campo | Valor |
|---|---|
| Package/Identity/Name | `MingXuan.PinDesk` |
| Package/Identity/Publisher | `CN=6973A70C-69AD-469C-8C2B-2A0DC3118CCC` |
| Package/Properties/PublisherDisplayName | `Ming Xuan` |
| Package Family Name | `MingXuan.PinDesk_mtqkn9ff51s0e` |
| Store ID | `9NKVP3K59V66` |
| Nombre visible | `PinDesk` (el nombre reservado) |

No son secretos. Si alguno no coincide con `tienda/AppxManifest.xml`,
Partner Center rechaza la subida. La familia esta tambien en
`crates/pindesk-shell/src/tienda.rs` (`FAMILIA_TIENDA`).

## Que hay en la rama

| Fichero | Para que |
|---|---|
| `tienda/AppxManifest.xml` | El manifiesto del paquete (la version la pone el workflow). |
| `tienda/Assets/*.png` | Los logos, generados con `tienda/generar-logos.py` a partir de `apps/pindesk/recursos/pindesk.ico`. |
| `.github/workflows/tienda.yml` | Compila, empaqueta y publica `PinDesk_<version>_x64.msix`. |
| `apps/pindesk/src/tienda.rs` | Lo que la app hace distinto en la Store (feature `tienda`). |
| `crates/pindesk-shell/src/tienda.rs` | La tarea de inicio (`StartupTask`) y la familia del paquete. |
| `apps/pindesk/build.rs` | Con la feature `tienda`, el manifiesto del exe sin el elemento `<msix>`. |
| Claves `tienda-ayuda-*` en `crates/pindesk-store/i18n/{es-ES,en-US}/main.ftl` | Las ayudas de Ajustes que cambian en la Store. |

## Armar el paquete

1. Actions › **Tienda** › Run workflow, en la rama `microsoft-store`.
2. `version`: cuatro numeros con el ultimo `0` (`1.0.0.0`, `1.0.1.0`...). La
   Store reserva el cuarto numero y cada envio tiene que subir la version.
3. Sale el artefacto `PinDesk-msix-<version>` y, con `publicar`, la version
   previa `v<version>-store` en Releases con el `.msix`.

Lo que hace el workflow: `cargo build --release -p pindesk --features
pindesk/tienda` y los ayudantes (`-p pindesk-aligerar -p pindesk-lanzador -p
pindesk-puente -p pindesk-mcp`); copia `pindesk.exe`, `pindesk-aligerar.exe`,
`pindesk-lanzador.exe`, `pindesk-puente.exe`, `pindesk-mcp.exe`,
`onnxruntime.dll` (del ZIP oficial 1.31.0, comprobada con el sha256 de
`crates/pindesk-indra/src/catalogo.rs`), las licencias y
`Assets\`; pone la version en el manifiesto; genera `resources.pri` con
`makepri` y empaqueta con `makeappx pack` (del SDK de Windows del runner,
`C:\Program Files (x86)\Windows Kits\10\bin\<version>\x64`).

El `.msix` va **sin firmar**: la Store lo firma al publicarlo. No se instala
con doble clic. Para probarlo en un PC propio hay que firmarlo con un
certificado de prueba cuyo sujeto sea el editor (es lo que hace el paso del
WACK del workflow) y confiar en ese certificado.

**WACK**: si el runner tiene el Kit de certificacion de apps de Windows
(`appcert.exe`), el workflow firma una copia con un certificado de prueba,
instala VCLibs y lo pasa; el informe sale como artefacto
`PinDesk-wack-<version>` y cada prueba que falla, como aviso. Si no esta, o
falla, el trabajo sigue: el `.msix` bueno es el sin firmar. Partner Center
pasa sus propias pruebas de todas formas.

## Que cambia respecto a `main`

Todo pasa por `tienda::activa()`: verdadero si se compilo con la feature
`tienda` **o** si el proceso corre dentro del paquete `MingXuan.PinDesk`
(red de seguridad con `GetCurrentPackageFamilyName`, por si alguien empaqueta
un exe compilado sin la feature). El paquete de identidad local
(`PinDesk`, editor `CN=PinDesk Local`, `herramientas/paquete-identidad.ps1`;
antes `ForgePixPin.PixPinMax`) no cuenta: ese corre desde la carpeta del ZIP.

| Lo que hace `main` | En la Store |
|---|---|
| Arrancar con Windows: `HKCU\...\Run` (`pindesk-shell/arranque.rs`), encendido por defecto. | Tarea de inicio del paquete (`desktop:StartupTask`, `TaskId="PinDeskInicio"`, `Enabled="false"`). La app **no la enciende sola**: al arrancar, el interruptor de Ajustes muestra el estado real (apagado de fabrica); al encenderlo se llama a `RequestEnableAsync`. Si el usuario la apago en Configuracion › Aplicaciones › Inicio, la app ya no puede y abre esa pagina. |
| «Abrir con» y Aplicaciones predeterminadas (`asociaciones.rs`, `abrir_con.rs`) en `HKCU\Software\Classes`. | Las declara el manifiesto: `.pindesk` y `.pixpin` (el de antes, que se sigue leyendo), imagenes, videos y audios (las listas de `asociaciones.rs`) y documentos, con el verbo «Enviar al chat de PinDesk». La app no escribe nada; el interruptor «Abrir con» de Ajustes no tiene efecto y su ayuda lo explica. |
| Notificaciones: AUMID `PinDesk.App` y protocolos `pindesk:` y `pixpinmax:` en el registro (`notificacion.rs`). | La identidad es la del paquete; `pindesk:` y `pixpinmax:` son `uap3:Protocol` del manifiesto (`--url "%1"`; `Parameters` solo existe en `uap3`). `pixpin:` no se declara: es del PixPin original. |
| Compartir y verbos de Windows 11 con el paquete de identidad (`herramientas/paquete-identidad`). | Los mismos, en el manifiesto (`windows.shareTarget`). El exe se compila sin el elemento `<msix>` de su manifiesto. |
| Cortafuegos: `herramientas/cortafuegos-sincro.ps1` a mano (la app no lo ejecuta nunca). | Igual: no se ejecuta. La ayuda de «Visible en la red» dice que Windows puede preguntar si PinDesk puede usar la red. El manifiesto declara `internetClientServer` y `privateNetworkClientServer`. |
| Actualizarse | No hay actualizador propio (tampoco en `main`). La Store actualiza el paquete. |
| `onnxruntime.dll` (OCR Indra, Whisper): se baja si Windows no la trae. | Va dentro del paquete, junto al exe (`ort::buscar` y `indra::runtime_listo` la encuentran ahi), asi que no se baja ninguna DLL. Los **modelos** (pesos `.onnx`, Vosk, Qwen3) se siguen bajando: son datos, no codigo. `libvosk.dll` nunca se baja: la pone el usuario. |
| Jump List (`jump_list.rs`) | Sin cambios: funciona igual con identidad de paquete. |

### Alias de ejecucion

El manifiesto da tres alias (en `%LOCALAPPDATA%\Microsoft\WindowsApps`, que
esta en el `PATH`): `pindesk.exe`, `pindesk-mcp.exe` (para la configuracion de
Claude Desktop u otro cliente MCP; ver `docs/integraciones-mcp.md`) y
`pindesk-puente.exe` (la ruta estable para registrar el anfitrion
`com.pindesk.puente` del navegador: `herramientas\registrar-navegador.ps1 -Puente
"$env:LOCALAPPDATA\Microsoft\WindowsApps\pindesk-puente.exe"`). Los tres van
en la **unica** `<Application>` del paquete: el WACK falla con mas de una
(Application count). La ruta real
dentro de `C:\Program Files\WindowsApps\` cambia con cada version: nunca hay
que apuntar ahi.

`pindesk-lanzador.exe` (Flow Launcher) y `pindesk-aligerar.exe` van en el paquete pero sin alias: el
plugin se instala en la carpeta de Flow y, desde la Store, todavia no hay un
instalador para el. Pendiente.

## Donde quedan los datos (sin virtualizar)

La app guarda en `%APPDATA%\PinDesk` (`crates/pindesk-store/src/rutas.rs`; al
arrancar muda alli `%APPDATA%\PixPinMax`, `pindesk_sistema::nombre`) y
algunos ayudantes leen `%LOCALAPPDATA%\PinDesk`. En un MSIX, Windows
redirige por defecto lo que se escribe en `AppData` a la carpeta privada del
paquete (`%LOCALAPPDATA%\Packages\MingXuan.PinDesk_mtqkn9ff51s0e\LocalCache`),
y entonces lo que corre **fuera** del paquete (el plugin de Flow Launcher, el
MCP lanzado por Claude Desktop, el puente que lanza el navegador) no lo veria,
ni quien viene del ZIP encontraria sus datos.

Decision: **sin virtualizar**. El manifiesto declara
`<rescap:Capability Name="unvirtualizedResources"/>` y
`desktop6:FileSystemWriteVirtualization` y
`desktop6:RegistryWriteVirtualization` en `disabled`. Consecuencias:

- Los datos estan donde siempre, compartidos con los ayudantes y con una
  instalacion anterior del ZIP (y la mudanza de `PixPinMax` a `PinDesk`
  mueve las carpetas de verdad, no una copia virtual).
- Al desinstalar, los datos **se quedan** (como con el ZIP). Por eso la app,
  en la Store, no escribe nada en el registro: lo escrito tambien se quedaria.
- `unvirtualizedResources` es una capacidad restringida: en Partner Center
  hay que justificarla (texto abajo).
- Solo vale en Windows 10 1903 (18362) o mas nuevo; el `MinVersion` del
  manifiesto es `10.0.19041.0` (2004), asi que siempre aplica.

## Subir a Partner Center, paso a paso

Partner Center › Apps y juegos › **PinDesk** (ya reservado) › Iniciar envio.

1. **Precios y disponibilidad**: mercados (todos), visibilidad publica,
   gratis (o el precio), fecha de publicacion (en cuanto se certifique, o a
   mano).
2. **Propiedades**: categoria *Productividad* (subcategoria opcional);
   politica de privacidad: **URL obligatoria** porque la app usa red,
   microfono y camara (ver abajo); sitio web y contacto de soporte (el repo o
   un correo); requisitos del sistema: teclado y raton recomendados; marcar
   que la app **no** es un juego.
3. **Clasificacion por edades**: el cuestionario IARC. Respuestas habituales
   para PinDesk: sin violencia, sin contenido sexual, sin apuestas; **si**
   permite que los usuarios se comuniquen o compartan contenido (chat con
   la IA, sincronizar con el movil, compartir) y **si** accede a internet. Sale
   normalmente PEGI 3 / ESRB Everyone con la nota de interaccion de usuarios.
4. **Paquetes**: arrastrar `PinDesk_<version>_x64.msix` (del artefacto o de
   la Release `v<version>-store`). Partner Center lo valida (identidad,
   version, capacidades). Familias de dispositivos: solo *Windows 10/11
   Desktop*.
5. **Lista de la Store** (una por idioma del paquete: `es-es` y `en-us`, los
   de `<Resources>` del manifiesto): descripcion, novedades de esta version,
   funciones (hasta 20 lineas cortas), **capturas** (al menos una, 1366x768 o
   mas; hay en `docs/img/`), logo de la Store opcional (los del paquete
   valen), palabras clave, copyright (`Max Anthony Mamani Maquera`). El
   nombre de la ficha es **PinDesk**. No usar «PixPin» en el texto ni en las
   capturas: es marca de DepthPixel.
6. **Opciones de envio** › Notas para la certificacion. Pegar algo asi:

   > PinDesk es una app de escritorio de confianza total (runFullTrust). No
   > necesita cuenta para probarla: se queda en la bandeja; Ctrl+1 hace una
   > captura. `unvirtualizedResources`: la app y sus ayudantes (servidor MCP,
   > puente de la extension del navegador y plugin de Flow Launcher, que
   > corren fuera del paquete) comparten los datos del usuario en
   > %APPDATA%\PinDesk; con la escritura virtualizada no los verian. La
   > app no escribe en el registro. Microfono: notas de voz y grabar clases.
   > Camara: la camara del equipo en la grabacion. Red: sincronizar con el
   > movil en la Wi-Fi local (TCP 47474, UDP 47475) y las API de IA que el
   > usuario configura con su clave.

   (Comprobar que el atajo de la nota es el de verdad antes de pegarla.)
7. **Enviar a la Store**. La certificacion tarda de horas a unos dias; si
   rechaza algo, el informe dice que politica y en que paso.

### Politica de privacidad (GitHub Pages)

La Store exige una URL publica. Plan: `docs/privacidad.md` (lo prepara otro
agente en `main`) publicado con GitHub Pages:

1. En `main`, Settings › Pages › *Deploy from a branch* › `main` › `/docs`.
2. La URL queda `https://1xmanmax.github.io/pixpin-max/privacidad` (o la del
   repo con el nombre nuevo si se renombra). Comprobar que abre sin sesion.
3. Ponerla en Propiedades › Politica de privacidad, y tambien en la ficha.

Si el repositorio es privado, Pages necesita un plan de pago o un repo
publico aparte solo con la politica.

## Unir `main` en `microsoft-store`

```
git checkout microsoft-store
git fetch origin
git merge origin/main
```

Lo propio de la Store vive en ficheros que `main` no toca (`tienda/`,
`tienda.rs` x2, `tienda.yml`, este documento), asi que casi siempre entra
solo. Los puntos donde puede chocar, y como resolverlos:

- `apps/pindesk/src/main.rs`, paso 5 (registro de Windows): en la Store todo
  ese bloque va dentro de `if !tienda::al_arrancar(&mut config) { ... }`.
  Tomar lo nuevo de `main` y volver a meterlo dentro del `if`. Y
  `registrar_notificaciones` lleva `&& !tienda::activa()`.
- `apps/pindesk/src/ventana_ajustes.rs`: tres ayudas envueltas en
  `crate::tienda::ayuda(...)`, «Hacerla predeterminada» (`Clave::Predeterminada
  if crate::tienda::abrir_predeterminadas()`) y el principio de
  `aplicar_al_cerrar`.
- `apps/pindesk/Cargo.toml` (`[features] tienda`) y `apps/pindesk/build.rs`
  (el manifiesto sin `<msix>`).
- `crates/pindesk-shell/src/lib.rs` (`pub mod tienda;`) y las tres claves
  `tienda-ayuda-*` de los `.ftl`.

Despues de unir: `cargo clippy` con y sin `--features pindesk/tienda`, y si
`main` anadio algo que **escribe en el registro, arranca solo, se actualiza
o baja ejecutables**, pasarlo por `tienda::activa()` aqui (nunca en `main`).

**El cambio de nombre a PinDesk ya esta** (nov-2026, `docs/nombre-pindesk.md`).
Unir `main` no lo pudo traer (la deteccion de renombrados se pierde con
tantos ficheros), asi que la rama se rehizo: lo de la Store se volvio a
poner encima de `main` ya renombrada y se publico como una union cuyo
contenido es exactamente eso (ver `docs/ramas.md`). Desde ahi las uniones de
`main` vuelven a ser normales. La carpeta de datos (`%APPDATA%\PinDesk`) y la
ventana de mensajes (`PinDeskVentanaMensajes`) tienen que seguir iguales en
las dos ramas, o los ayudantes dejan de encontrar la app.

## Lo que hay que comprobar en Windows

- `makeappx pack` acepta el manifiesto (el esquema lo valida: `Parameters` en
  `uap3:Protocol` y `uap3:FileTypeAssociation`, el orden de las capacidades).
- Instalado (firmado con el certificado de prueba): arranca, sale en Inicio
  como PinDesk, el icono de la bandeja y de la barra de tareas se ve nitido.
- Ajustes › Arrancar con Windows: encender, cerrar sesion y volver; apagar
  desde Configuracion › Aplicaciones › Inicio y ver que el interruptor lo
  refleja al reabrir.
- Doble clic en un `.pindesk` y en un `.pixpin` de antes, «Abrir con» › PinDesk en una imagen y el verbo
  «Enviar al chat de PinDesk».
- Los botones de las notificaciones y los enlaces `pindesk:` y `pixpinmax:`.
- Compartir desde otra app hacia PinDesk.
- Sincronizar con el movil: la pregunta del cortafuegos y que funciona en
  red privada.
- Los datos en `%APPDATA%\PinDesk` (no en `Packages\...\LocalCache`), que
  quien venia con `%APPDATA%\PixPinMax` los encuentra mudados, y que el MCP
  por el alias `pindesk-mcp.exe` los lee.
- OCR Indra y Whisper encuentran el `onnxruntime.dll` del paquete y no
  intentan bajarlo.
- Que nada escribe en `HKCU\Software\Classes` ni en `...\Run`
  (`reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
- El WACK sin fallos (informe del workflow o `appcert.exe` a mano).
