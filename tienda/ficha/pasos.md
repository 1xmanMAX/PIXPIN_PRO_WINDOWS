# Subir PinDesk a la Microsoft Store: pasos en Partner Center

Lo que hay que hacer a mano, en orden, en
[Partner Center](https://partner.microsoft.com/dashboard) → *Apps and games*
→ **PinDesk** (`9NKVP3K59V66`) → *Start your submission*. Todo lo que se pega
está en esta carpeta (`ficha.md`, `<idioma>.md`, `clasificacion.md`).

Antes de empezar:

- [ ] `docs/privacidad.md` está en `main` (ya lo está desde `0b6642d`) y
      se abre en
      https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS/blob/main/docs/privacidad.md
- [ ] El repositorio es **público** (si no, ni la política ni el soporte se
      pueden abrir y certificación lo rechaza).
- [ ] Hay una Release `v<versión>-store` con el `.msix` (la hace el flujo de
      la rama `microsoft-store`, `.github/workflows/tienda.yml`).
- [ ] Se ejecutó *Actions → Capturas para la Store → Run workflow* y se bajó
      el artefacto `capturas-tienda` (ver el paso 6).

## 1. Pricing and availability (Precio y disponibilidad)

- **Markets**: *All markets* (todos), incluidos los que Microsoft añada en el
  futuro.
- **Discoverability**: *Make this product available and discoverable in the
  Store*.
- **Schedule**: *Release as soon as possible* (o una fecha, si se prefiere).
- **Pricing**: **Free** (gratis). Sin prueba gratuita ni ventas.
- **Free trial / Sale pricing**: nada.
- Organizaciones (Microsoft Store for Business/Education): dejar lo que venga.

## 2. Properties (Propiedades)

- **Category**: **Productivity**. **Subcategory**: no tiene (Productivity
  no tiene subcategorías; si la interfaz ofrece alguna, ninguna).
  Segunda opción si algún día se cambia: *Utilities & tools*.
- **Privacy policy URL**: https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS/blob/main/docs/privacidad.md
  (en algunas versiones de Partner Center está aquí y no en la ficha).
- **Website**: https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS
- **Support contact info**: https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS/issues
- **Game settings**: no aplica.
- **Product declarations** (casillas):
  - *This app allows users to make purchases, but does not use the Microsoft
    Store commerce system*: **sin marcar** (no hay compras).
  - *This app has been tested to meet accessibility guidelines*: **sin
    marcar** (no se ha hecho esa prueba formal).
  - *Customers can install this app to alternate drives or removable storage*:
    **marcada** (por defecto).
  - *Windows can include this app's data in automatic backups to OneDrive*:
    **marcada** (por defecto; los datos son del usuario).
  - *This product supports pen and ink input*: **marcada** (hay tinta con
    lápiz en el lienzo y los visores), opcional.
  - *This app depends on non-Microsoft drivers or NT services*: **sin
    marcar**.
- **System requirements** (Minimum / Recommended):
  - **Windows**: Windows 10 **versión 21H2 (19044)** o posterior y Windows 11,
    x64. Ver la nota: lo que manda es `MinVersion` del `AppxManifest.xml`.
  - **Memory**: mínimo 4 GB (es la máquina de referencia: un Core i3 de 3.ª
    generación con 4 GB); recomendado 8 GB.
  - **DirectX**: mínimo DirectX 11 (funciona también con WARP, sin GPU).
  - **Microphone**: marcar como **opcional/recomendado** (no «required»):
    solo para notas de voz, dictado y grabar clases.
  - **Camera**: **opcional** (fotos y clips al grabar).
  - **Touch screen / Keyboard / Mouse**: teclado y ratón *Recommended*;
    pantalla táctil, no.
  - **Graphics**: integrada vale.

> **Versión mínima: Windows 10 2004 (19041)**, la misma que el `MinVersion`
> de `tienda/AppxManifest.xml`. La app usa cosas de la 2004 (que una ventana
> no salga en las capturas, `WDA_EXCLUDEFROMCAPTURE`).

## 3. Age ratings (Clasificación por edades)

Rellenar el cuestionario IARC con las respuestas de
[`clasificacion.md`](clasificacion.md) (categoría *Utility, Productivity,
Communication, or Other*; contenido: todo «No»; interacción: usuarios
interactúan **Sí**, ubicación **No**, compras **No**, internet sin
restricciones **Sí**, comparte información **Sí**, IA generativa **Sí** si se
pregunta). Resultado esperado: 3+/Everyone con avisos.

## 4. Packages (Paquetes)

1. Bajar el `.msix` de la Release `v<versión>-store`
   (https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS/releases, la que acaba en
   `-store`).
2. Arrastrarlo a *Packages*. Partner Center comprueba que la identidad sea
   `MingXuan.PinDesk` y el editor el de la cuenta; si dice que no coinciden,
   el manifiesto está mal (lo arregla el agente del MSIX, no aquí).
3. **Device families**: solo **Windows 10/11 Desktop** (x64). Desmarcar
   Xbox, HoloLens, Team, etc. si aparecen.
4. Si Partner Center avisa de **restricted capability `runFullTrust`**: es
   normal en una app de escritorio empaquetada; la justificación va en la nota
   para certificación (paso 7) y en el cuadro que pide, la misma frase:
   *«PinDesk is a desktop (Win32) app packaged as MSIX. It needs runFullTrust
   to capture the screen, register a global hotkey, and show always-on-top
   overlay windows (floating pins and the capture overlay).»*

## 5. Store listings (Ficha de la tienda)

Primero *Manage additional languages* y añadir los 14 idiomas: Spanish
(Spain), English (United States), Chinese (Simplified), Japanese, Korean,
Hindi, Arabic, Turkish, Portuguese (Brazil), Italian, French (France),
German, Russian, Indonesian.

En **cada idioma**, abrir `tienda/ficha/<idioma>.md` y pegar cada bloque
`text` en su campo:

| Campo | Bloque del `.md` |
|---|---|
| Product name | ya sale `PinDesk` (el nombre reservado) |
| Description | `## Description` |
| What's new in this version | `## What's new in this version` |
| Product features | `## Product features`: **un renglón en cada casilla** (15) |
| Screenshots | las PNG del paso 6 |
| Store logos (opcional) | `tienda/ficha/logos/` (ver `ficha.md`) |
| Short description | `## Short description` |
| Search terms | `## Search terms`: **un término en cada casilla** (7) |
| Copyright and trademark info | `© 2026 Ming Xuan` |
| Additional license terms | **vacío** |
| Developed by | `Ming Xuan` (opcional) |
| Additional system requirements | vacío (ya está en Properties) |

Truco: rellenar primero `en-US` y `es-ES` del todo; en los demás, las
capturas se pueden copiar (*Use the same screenshots as …*) si Partner Center
lo ofrece.

## 6. Capturas (dentro de Store listings)

1. *Actions → Capturas para la Store → Run workflow* (rama `tienda-ficha` o
   la que lo tenga). Tarda lo que una compilación (~20–40 min la primera vez).
2. Bajar el artefacto `capturas-tienda`:
   - `compuestas/es/*.png` → ficha `es-ES`;
   - `compuestas/en/*.png` → ficha `en-US` y, si no hay otras, los demás
     idiomas;
   - `compuestas/informe.md` dice qué escenas salieron y cuáles faltan;
   - `crudas/` son las pantallas sin componer, por si se quiere otra.
3. Subir de 4 a 8 por idioma, en este orden (es el de los nombres
   `01-…`, `02-…`). Añadir a cada una el *caption* opcional si se quiere: el
   mismo título que lleva la imagen.

Escenas que pinta el flujo (prueba `#[ignore]` → PNG que usa):

| Escena | Prueba | PNG |
|---|---|---|
| Chat de un proyecto | `muestra_de_la_foto_con_descripcion` / `muestra_del_chat_con_la_zona` | `foto-con-descripcion.png` / `chat-con-la-zona.png` |
| Captura y anotar | `muestra_captura2_despues` | `captura2-despues.png` |
| Asistente con burbujas ricas | `muestra_del_chat_con_el_asistente` | `chat-asistente-burbujas.png` |
| Tareas y Kanban | `muestra_de_la_ventana` (tareas) | `tareas-kanban.png`, `tareas-v5-tarjetas.png` |
| Horario de clases | `horario_ventana_se_pinta_en_menos_de_medio_segundo` | `horario-semanal.png` |
| Lienzo | `muestras_del_lienzo` / `muestra_del_lienzo_en_ventana` | `grafica-ecuaciones-y-tabla.png` / `lienzo-en-ventana.png` |
| Timeline | `muestra_del_timeline` | `timeline-estado.png` |
| Buscar en todo | `muestras_del_buscador` | `buscar2-resultados.png` |
| Ajustes del asistente | `muestras_de_la_ventana` (ajustes) | `ajustes2-asistente.png` |

**No hay prueba de muestra** para la barra de «Grabar clase o reunión» ni
para «Estudiar la clase», ni para Ajustes › General con el selector de
idioma: esas capturas, si se quieren, hay que hacerlas a mano en un PC
(Win+Shift+S o la propia captura de PinDesk, a 1920×1080). La interfaz de las
muestras está en español (ver `ficha.md`).

## 7. Submission options (Opciones de la entrega)

- **Publishing hold options**: *Publish this submission as soon as it passes
  certification* (o *Don't publish until I select Publish now*, para revisar
  antes).
- **Notes for certification**: pegar este texto (en inglés, que es lo que
  leen los revisores):

```text
PinDesk is a desktop (Win32, Rust) productivity app packaged as MSIX. No account or sign-in is needed, and there is no PinDesk server: all user data stays in the local app data folder.

WHY runFullTrust
PinDesk needs runFullTrust because it is a classic desktop app that:
- captures the screen (Windows.Graphics.Capture / DXGI) when the user starts a capture;
- registers one global hotkey (Ctrl+Alt+X by default) to start a capture from anywhere;
- shows always-on-top overlay windows: the capture overlay and the floating "pins" (images, PDFs, notes, live screen regions) that stay above other apps;
- lives in the notification area (tray icon) and can start with Windows if the user enables it.
Microphone and camera are used only after an explicit user action (voice note, dictation, "Record class or meeting", taking a photo or clip). Windows shows the microphone/camera indicators while they are in use.

HOW TO TEST (no keys needed)
1. Launch PinDesk. Its icon appears in the notification area (tray); left-click it for the control center.
2. Press Ctrl+Alt+X (or tray > Capture), drag a region, then use the bottom bar: Copy, Pin (it floats on top), To chat, Text (OCR), Save. Keys 1-5 annotate before copying.
3. Open the projects window from the tray, create a chat with "+", and add notes, photos, PDFs or a canvas with the attach button.
4. Tasks: tray > Tasks. Ctrl+K shows the Kanban board, Ctrl+H the class schedule.
5. Recording: in the tray search, type "Record" and choose "Record class or meeting...". Allow the microphone when Windows asks. Hold the timer to stop; the recording, screenshots and transcript appear in the chat. The first transcription downloads the speech model (internet needed once).
6. Settings > General > "Privacy and data" lists everything that can leave the PC and where it goes.

OPTIONAL FEATURES THAT NEED THE USER'S OWN KEYS
- AI assistant (chat "Assistant"): needs the user's own Anthropic (Claude) or DeepSeek API key in Settings > Assistant. Without a key, nothing is sent to any AI service. Every AI response has "Report response..." (message menu > More), which saves the report and opens an email to the developer (max.antony.8.5.1@gmail.com).
- Jev live topics: needs the user's own TypeSafe key; without it, recording works the same without live topic ticks.
- Sync with the companion Android app works only between devices paired on the same local network (code or QR); it cannot be tested without a second device.
All other features work fully without any key or account.

No ads, no in-app purchases, no telemetry. Privacy policy: https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS/blob/main/docs/privacidad.md
Support: https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS/issues
```

  Si el revisor necesita probar el asistente, se le puede dar una clave de
  prueba en esta misma nota (*«Test API key for the assistant: …»*), con un
  límite de gasto bajo en la consola del proveedor, y borrarla después de la
  certificación. Es opcional.

- **Restricted capabilities**: si sale el cuadro, la frase del paso 4.

## 8. Submit to the Store

*Submit to the Store*. La certificación suele tardar de unas horas a 3 días
laborables. Si la rechazan, el informe dice la política (p. ej. 10.1.1 nombre,
10.5.1 privacidad, 11.x IA); arreglar en la rama `microsoft-store` y volver a
entregar.

## Después

- Cada versión nueva: Release `vX.Y.Z-store` con el `.msix` → *Update
  submission* → Packages (subir el nuevo) → *What's new in this version* de
  cada idioma → Submit.
- Comprobar las fichas antes de pegar: `python3 tienda/ficha/comprobar.py`.
