# Integraciones con Windows: «Imprimir a PixPin» y el paquete de identidad

15-oct-2026. Los puntos 7 y 8 de `pixpin-v2/integraciones-propuesta.md`. Las dos cosas
necesitan **un paso del usuario con permiso de administrador** (una vez); el código de la app ya
está y, sin esos pasos, PixPin funciona exactamente como antes.

---

## 1. «Imprimir a PixPin»

**Qué hace.** Aparece una impresora «PixPin». Desde cualquier programa (navegador, AutoCAD,
una factura, Word) Imprimir → PixPin, y el PDF se abre en el lector de PixPin como pin (con
buscar, anotar, OCR, leer en voz alta y mandarlo al chat). Igual que en Android, donde un
archivo recibido sale como pin (`ShareReceiverActivity`).

**Cómo es.** No instala ningún controlador: usa **«Microsoft Print To PDF»**, que ya trae
Windows, con un **puerto local que es un fichero**:

| Qué | Dónde |
|---|---|
| El puerto escribe | `%APPDATA%\PixPinMax\cache\impresora\PixPin.pdf` |
| PixPin lo deja | `%APPDATA%\PixPinMax\impresos\Impreso AAAA-MM-DD HH.MM.SS.pdf` |
| Lo que nunca acabó (trabajo cancelado, > 60 s a medias) | `…\cache\impresora\descartados\` |

(En modo portable, cambia `%APPDATA%\PixPinMax` por la carpeta de `pixpinmax.exe` y pásala con
`-Raiz`.)

**El lado de la app** (`crates/pixpin-shell/src/impresora.rs`, `apps/pixpin/src/recibir_de_windows.rs`):
- Al arrancar, si la carpeta `cache\impresora` existe, un hilo duerme en el aviso de cambios de
  Windows (`vigia_carpeta`). Sin la carpeta, no arranca nada.
- Un PDF está **terminado** cuando se puede abrir en exclusiva (el administrador de impresión ya
  lo cerró) **y** empieza por `%PDF-` y acaba en `%%EOF`. Hasta entonces se mira cada 150 ms.
- Terminado → se **mueve** (renombrar, instantáneo) a `impresos\` con nombre propio (el puerto
  siempre escribe el mismo fichero) y se manda el pedido `abrir {tipo:"fichero"}`.
- Medido en la prueba: el PDF de muestra (5,6 MB) se recoge entre 0,10 y 0,24 s después de cerrarse (3 vueltas).

### Pasos del usuario

```powershell
cd "<repo>"
# 1) Ver qué hay y qué se haría (no cambia nada, no pide nada):
powershell -ExecutionPolicy Bypass -File herramientas\impresora-pixpin.ps1 -SoloMostrar
# 2) Ponerla (crea la carpeta como tú y pide administrador UNA vez para el puerto y la impresora):
powershell -ExecutionPolicy Bypass -File herramientas\impresora-pixpin.ps1
# 3) Cerrar y abrir PixPin (empieza a vigilar la carpeta al arrancar).
# Quitarla (los PDF de impresos\ se quedan):
powershell -ExecutionPolicy Bypass -File herramientas\impresora-pixpin.ps1 -Quitar
```

Lo que hace como administrador, por si se prefiere a mano:
`Add-PrinterPort -Name '<puerto>'` y
`Add-Printer -Name 'PixPin' -DriverName 'Microsoft Print To PDF' -PortName '<puerto>'`
(y, solo si faltara el controlador,
`Enable-WindowsOptionalFeature -Online -FeatureName Printing-PrintToPDFServices-Features`).

---

## 2. Paquete de identidad (MSIX con ubicación externa)

**Qué da.**
- **PixPin en el panel Compartir** de otras apps (WhatsApp, Teams, Fotos, Edge, el Explorador…):
  texto, enlaces, imágenes y ficheros van al chat **«Mensajes guardados»** con el pedido `chat`
  (texto + enlace en un mensaje, fotos como fotos, el resto como adjuntos).
- **Menú contextual de primer nivel en Windows 11** (sin «Mostrar más opciones») para los tipos
  que PixPin abre: **«Pinear con PixPin»** y **«Enviar al chat de PixPin»** (varios ficheros a la
  vez en una sola llamada).
- Identidad de paquete para avisos (ver «AUMID» abajo).

**Cómo es.** Un `.msix` de 13 KB que **no lleva el programa**: solo el manifiesto
(`herramientas/paquete-identidad/AppxManifest.xml`) y tres logos sacados del icono. Se registra
con `Add-AppxPackage -ExternalLocation` apuntando a `%LOCALAPPDATA%\Programs\PixPinMax` (la
carpeta fija de `instalar.ps1`). Windows le da la identidad a `pixpinmax.exe` porque su
manifiesto incrustado (`apps/pixpin/pixpinmax.manifest`) lleva el elemento `<msix>` con el mismo
editor, paquete y aplicación. El acceso directo, el arranque con Windows y la Jump List no cambian.

| Dato | Valor (igual en los dos manifiestos y en el certificado) |
|---|---|
| Editor (`Publisher`, sujeto del certificado) | `CN=PixPin Max Local` |
| Paquete (`Identity Name`) | `ForgePixPin.PixPinMax` |
| Aplicación (`Application Id`) | `PixPinMax` |
| Versión | la del `Cargo.toml` del workspace + `.0` (hoy `0.1.0.0`) |

**Por qué verbos de asociación y no `desktop4:FileExplorerContextMenus`.** Esa extensión exige
un servidor COM con `IExplorerCommand`, que en la práctica es un **DLL dentro del Explorador**:
si falla, cuelga el Explorador (lo mismo que se descartó para las miniaturas). Los verbos de
`uap3:FileTypeAssociation` salen igual en el menú de primer nivel de Windows 11, lanzan
`pixpinmax.exe` sin nada cargado en el Explorador y **no cambian** qué programa abre cada tipo.
Contra: solo aparecen en los tipos de la lista (imágenes, PDF, Office, EPUB, MD/TXT, DWG/DXF/IFC,
audio, MP4), no en carpetas ni en «cualquier fichero».

**El lado de la app** (`crates/pixpin-shell/src/compartido.rs`, `apps/pixpin/src/recibir_de_windows.rs`, `main.rs`):
- Antes de la instancia única se mira si el arranque trae algo: `--al-chat <rutas>` (los verbos)
  o, **solo si el proceso tiene identidad** (`GetCurrentPackageFullName`), una activación
  `ShareTarget` (`AppInstance::GetActivatedEventArgs`). Sin identidad no se llama a nada de
  WinRT: cuesta leer los argumentos.
- Lo compartido se **copia** a `%APPDATA%\PixPinMax\cache\compartido\<ms>\` antes de contestar a
  Windows (`ReportCompleted`): la otra app puede borrar sus temporales en cuanto se le contesta.
  Si la app manda la foto como fichero y también suelta, va una sola vez.
- El pedido `chat` va a la PixPin que corre (si esta copia es la segunda) o a la propia ventana
  al arrancar (si Compartir la abrió), por el mismo camino que la Jump List.

### Herramientas del SDK

`makeappx.exe` y `signtool.exe` vienen con el **SDK de Windows**. En este equipo están en
`C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\` (el script toma la versión más
nueva). En otro equipo: instalar el «Windows SDK» (con Visual Studio Installer → «Windows 11 SDK»,
o `winget install Microsoft.WindowsSDK.10.0.26100`); basta con «Windows SDK Signing Tools for
Desktop Apps» y «Windows App Certification Kit»/«MSIX Packaging Tools».

### Pasos del usuario

Antes: instalar con `herramientas\instalar.ps1` una compilación de **este cambio o posterior**
(la que lleva `<msix>` en el manifiesto; `-SoloMostrar` dice «con `<msix>` dentro: sí/no»).

```powershell
cd "<repo>"
# 0) Ver qué hay (herramientas, exe, certificado, registro). No cambia nada:
powershell -ExecutionPolicy Bypass -File herramientas\paquete-identidad.ps1 -SoloMostrar
# 1-4) Todo seguido (pide administrador UNA vez, en el paso 3):
powershell -ExecutionPolicy Bypass -File herramientas\paquete-identidad.ps1 -Todo
# 5) Cerrar y abrir PixPin.
```

O paso a paso:

| Paso | Orden | Qué hace | ¿Admin? |
|---|---|---|---|
| 1 | `-Crear` | Copia el manifiesto con la versión, saca los logos del `.ico` y hace `makeappx pack /nv` → `herramientas\paquete-identidad\salida\PixPinMax-identidad.msix` | No |
| 2 | `-Firmar` | Si no está, crea el certificado de firma de código `CN=PixPin Max Local` (10 años) en `Cert:\CurrentUser\My`; lo exporta a `salida\PixPinMax-identidad.cer`; `signtool sign /fd SHA256` | No |
| 3 | `-Confiar` | `Import-Certificate` del `.cer` en `Cert:\LocalMachine\TrustedPeople` (Windows solo instala paquetes firmados por alguien de confianza en el **equipo**) | **Sí, una vez** |
| 4 | `-Registrar` | `Add-AppxPackage -Path <msix> -ExternalLocation %LOCALAPPDATA%\Programs\PixPinMax -ForceUpdateFromAnyVersion`; dice el nombre completo y el AUMID del paquete | No |

**Comprobar:** en Edge, Compartir una página → «PixPin Max» en la lista → el enlace aparece en
«Mensajes guardados». Clic derecho en un PDF → «Pinear con PixPin» / «Enviar al chat de PixPin»
en el primer nivel.

**Quitar:** `paquete-identidad.ps1 -Quitar` (desregistra; sin admin). Con
`-Quitar -BorrarCertificado` también quita el certificado de los dos almacenes (pide admin).

**Al poner una versión nueva de PixPin** con `instalar.ps1` no hace falta tocar el paquete:
apunta a la carpeta, no al fichero. Solo si cambia el manifiesto del paquete: `-Crear -Firmar
-Registrar` (sin admin; el certificado ya es de confianza).

Notas:
- El modo desarrollador **no** hace falta (el paquete va firmado).
- `-Registrar` es por usuario; el certificado de confianza es por equipo.
- Si `instalar.ps1` cambiara de carpeta, hay que volver a `-Registrar` con `-Instalado <nueva>`.

---

## 3. AUMID (coordinación con `flow-asistente` e `integ-consola`)

- La rama `flow-asistente` ya usa **`ForgePixPin.PixPinMax`** como AUMID de los toasts sin
  paquete (`crates/pixpin-shell/src/notificacion.rs`, clave
  `HKCU\Software\Classes\AppUserModelId\ForgePixPin.PixPinMax`). El paquete usa **ese mismo
  nombre** como `Identity Name`; no se inventó otro.
- **Con el paquete registrado**, el proceso tiene identidad y su AUMID real pasa a ser
  `ForgePixPin.PixPinMax_<hash del editor>!PixPinMax` (lo imprime `-Registrar`). Para los toasts,
  `notificacion.rs` debería usar `ToastNotificationManager::CreateToastNotifier()` (sin id) cuando
  `pixpin_shell::compartido::tiene_identidad()` sea `true`, y el `CreateToastNotifierWithId` de
  hoy si no. Sin el paquete, todo sigue igual.
- `--al-chat <rutas>` es la orden que usan los verbos del paquete. Si `integ-consola` define su
  propia orden para lo mismo (p. ej. `chat --archivo`), se puede cambiar el `Parameters` del verbo
  en `AppxManifest.xml` y quitar `compartido::de_argumentos`; el resto no cambia.
