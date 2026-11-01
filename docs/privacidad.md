# Política de privacidad de PinDesk

Aplicación para Windows «PinDesk» (`pindesk.exe`). Sirve como texto de
la política para la ficha de la Microsoft Store y es la que abre
Ajustes › General › «Privacidad y datos» › «Política de privacidad».

Fecha de entrada en vigor: 1 de noviembre de 2026. Última actualización: 1 de
noviembre de 2026. La versión en inglés está al final ([English](#pindesk-privacy-policy-english)).

> **En pocas palabras**
>
> - PinDesk no tiene cuentas de usuario ni servidores propios. Lo que creas
>   se guarda en tu PC.
> - No hay publicidad, ni analítica, ni informes de fallos, ni seguimiento, ni
>   SDK de terceros que recojan datos. No vendemos datos.
> - Algo sale del PC **solo cuando tú usas una función que lo necesita**: el
>   asistente con la clave de API que tú pones (Anthropic o DeepSeek), Jev de
>   TypeSafe con tu clave, la búsqueda web del asistente, tus conexiones de
>   Google, las voces de Microsoft del lector, «compartir por enlace» o la
>   descarga de modelos de voz y de texto. Abajo se detalla cada caso.
> - La sincronización con tu móvil u otros aparatos va por tu red local,
>   cifrada, sin pasar por internet.
> - Pasar a texto, dictar y escuchar una reunión en vivo (Whisper, Qwen3-ASR,
>   Vosk o el reconocedor de Windows) y leer texto en imágenes se hacen **en
>   tu PC, sin red**.

Contenido: [1 Responsable](#1-quién-es-el-responsable) ·
[2 Lo que se queda en tu PC](#2-datos-que-se-quedan-en-tu-pc) ·
[3 Lo que sale](#3-datos-que-salen-del-pc-solo-si-usas-la-función) ·
[4 Sincronización](#4-sincronización-por-la-red-local) ·
[5 Permisos](#5-permisos-y-para-qué-sirven) ·
[6 Lo que no hacemos](#6-lo-que-no-hacemos) ·
[7 Conservación y borrado](#7-conservación-y-borrado) ·
[8 Seguridad](#8-seguridad) · [9 Menores](#9-menores) ·
[10 Derechos](#10-tus-derechos) · [11 Cambios](#11-cambios-en-esta-política) ·
[12 Contacto](#12-contacto)

## 1. Quién es el responsable

PinDesk lo desarrolla un desarrollador independiente (en adelante,
«nosotros»). Para cualquier asunto de privacidad, escribe a
**max.antony.8.5.1@gmail.com**.

Como PinDesk no tiene servidores propios ni cuentas, nosotros no recibimos
ni almacenamos tus datos. Lo que la aplicación guarda está en tu PC, bajo tu
control.

## 2. Datos que se quedan en tu PC

PinDesk guarda en su carpeta de datos (`%APPDATA%\PinDesk`, o junto al
ejecutable en la versión portátil) lo que tú creas o traes a ella:

- Chats, mensajes, notas, tareas, horarios, recordatorios, lecciones y
  documentos.
- Capturas de pantalla, pines, dibujos, fotos, vídeos, notas de voz y
  grabaciones de clases o reuniones.
- Transcripciones, los temas y lo apuntado de una reunión. **La voz se pasa a
  texto en el PC** (Whisper, Qwen3-ASR o Vosk ejecutados en el equipo, o el
  reconocedor de voz de Windows).
- El reconocimiento de texto en imágenes (OCR) y la lectura de PDF, Word,
  Excel y planos también se hacen en el PC.
- Tus ajustes (`pindesk.toml`).
- Las claves de API que pegues (Anthropic, DeepSeek, TypeSafe, Brave Search,
  Google): se guardan **cifradas con tu usuario de Windows** (DPAPI) y nunca
  en el fichero de ajustes.
- Los reportes de respuestas de la IA que hagas (`reportes-ia.jsonl`, ver la
  sección 3).

## 3. Datos que salen del PC (solo si usas la función)

Ninguna de estas funciones se activa sola. Cada una envía datos directamente
desde tu PC al servicio indicado, por conexión cifrada (HTTPS/TLS); nosotros
no hacemos de intermediarios ni vemos esos datos. Cada servicio trata los
datos según su propia política de privacidad. Ajustes › General ›
«Privacidad y datos» repite esta lista, con un botón a donde se enciende o se
apaga cada función.

| Función | Qué se envía | A quién |
|---|---|---|
| **Asistente con IA** | Lo que le escribes, lo reciente de esa conversación y lo que adjuntes (una foto, una captura, el texto de un documento). | Anthropic (Claude) o DeepSeek, según elijas, **con tu propia clave de API**. Sin clave no se envía nada. |
| **Jev (TypeSafe)** | Fragmentos de texto (frases de una reunión o clase pasadas a texto en el PC, renglones de una nota, tareas, lo que dictas o explicas en voz alta) y, en algunas funciones, una imagen, para obtener una clasificación (temas tratados, tareas, prioridades, tarjetas). **Nunca el audio.** | TypeSafe (`api.typesafe.ai`), con tu propia clave y solo con esa función encendida en Ajustes › Asistente. |
| **Búsqueda y lectura web del asistente** | Los términos que el asistente busca para responderte y la dirección de las páginas que lee. Sin cookies ni datos de tu cuenta. | DuckDuckGo, o Brave Search si pones tu clave de Brave; y la web que se lee. |
| **Conexiones de Google** | Los archivos de Drive que eliges y los eventos del calendario que pides. | Google, solo si conectas tu cuenta (Ajustes › Asistente › Conexiones); se desconecta desde ahí. |
| **Voces de Microsoft del lector** | El texto que se lee en voz alta. | Microsoft (las voces en línea de Edge), solo si eliges una de esas voces; las voces de Windows funcionan sin red. |
| **Compartir por enlace** | El archivo que eliges compartir. | Un servicio público de archivos temporales (`litterbox.catbox.moe` o `temp.sh`). Cualquiera con el enlace puede abrirlo hasta que caduque (de horas a días). No lo uses para algo privado. |
| **Descarga de modelos y diccionarios** | Solo la petición de descarga (como en cualquier descarga, el servidor ve tu dirección IP). No se envía contenido tuyo. | `huggingface.co` y `github.com` (Whisper, Qwen3-ASR, Indra, la biblioteca de Vosk), `alphacephei.com` (los modelos de Vosk), `raw.githubusercontent.com` (diccionarios). |
| **Enlaces que abres** | Al abrir un enlace o un documento con otro programa, se encarga ese programa o tu navegador. | La web o el programa de destino. |

**Contenido generado por IA.** Las respuestas del asistente las genera un
modelo de terceros y pueden contener errores. Puedes informar de una
respuesta inadecuada con **«Reportar respuesta…»** (menú del mensaje › Más):
eliges el motivo (ofensiva, peligrosa, falsa u otro), se guarda en
`reportes-ia.jsonl` en tu carpeta de datos y se abre tu programa de correo con
el reporte escrito para que lo mandes, si quieres, a la dirección de la
sección 12. Solo nos llega lo que tú decidas enviar.

## 4. Sincronización por la red local

PinDesk puede sincronizar chats, fotos y grabaciones con tu móvil o con
otros aparatos en la misma red. Los aparatos se encuentran por la red local y
se emparejan con un código o un QR. Todo lo que viaja va **cifrado con
AES-GCM** con una clave que solo conocen tus aparatos. No hay ningún servidor
intermedio en internet. Si compartes un chat con otra persona, esa persona
recibe una copia en su aparato.

## 5. Permisos y para qué sirven

- **Micrófono**: notas de voz, dictado, repasar en voz alta y grabar clases o
  reuniones; solo mientras tú lo usas. Windows lo muestra en la barra de
  tareas.
- **Capturar la pantalla**: solo cuando tú la inicias (una captura, un clip o
  la grabación de la pantalla).
- **Cámara**: solo si la eliges para una foto o una grabación.
- **Red**: las funciones de la sección 3 y la sincronización de la sección 4.
- **Arrancar con Windows**: si lo enciendes en Ajustes › General.

PinDesk no lee tus contactos, tu ubicación ni tus correos.

## 6. Lo que no hacemos

- No mostramos publicidad ni usamos identificadores publicitarios.
- No usamos analítica, informes de fallos remotos ni rastreadores.
- No vendemos, alquilamos ni cedemos datos personales.
- No usamos tus datos para entrenar modelos. (Los proveedores de IA a los que
  envías datos con tu clave tienen sus propias condiciones; revísalas.)

## 7. Conservación y borrado

- Tus datos se conservan en tu PC hasta que los borras.
- **Llevártelos o borrarlos**: Ajustes › General › «Tus datos, en tu disco»
  abre la carpeta de datos. Para llevártelos, cópiala; para borrarlo todo,
  cierra PinDesk y bórrala (también los modelos bajados, en `whisper`,
  `qwen3`, `vosk`…). Las claves se borran desde Ajustes › Asistente.
- Según cómo se instaló, desinstalar PinDesk puede dejar tu carpeta de
  datos (para no perder tus cosas por error); si quieres borrarlo todo,
  bórrala a mano.
- Lo que ya enviaste a un tercero (sección 3) se rige por ese servicio; por
  ejemplo, para borrar el historial de tu cuenta de API, hazlo en la consola
  de ese proveedor. Los enlaces temporales caducan solos.
- Las copias sincronizadas en tu móvil u otros aparatos se borran en esos
  aparatos.

## 8. Seguridad

Las claves de API van cifradas con tu usuario de Windows. Las conexiones a
internet usan HTTPS y la sincronización local va cifrada entre tus aparatos.
Ningún sistema es infalible: protege tu cuenta de Windows con contraseña.

## 9. Menores

PinDesk no está dirigida a menores de 13 años (ni de la edad mínima que
fije la ley de tu país) y no recoge conscientemente datos de niños. El
asistente usa servicios de IA de terceros cuyas condiciones exigen ser mayor
de edad. Si crees que un menor nos ha enviado datos, escríbenos y los
borraremos.

## 10. Tus derechos

Según la ley que te aplique (por ejemplo, el RGPD en la Unión Europea, la
LOPDGDD en España o la Ley 29733 de Protección de Datos Personales en Perú),
tienes derecho a acceder, rectificar, suprimir y oponerte al tratamiento de
tus datos. Como tus datos están en tu PC, puedes ejercerlos directamente. Para
cualquier consulta, escríbenos; también puedes reclamar ante tu autoridad de
protección de datos (en España, la AEPD; en Perú, la Autoridad Nacional de
Protección de Datos Personales).

## 11. Cambios en esta política

Si cambiamos esta política, publicaremos la nueva versión en esta misma
dirección con su fecha. Si el cambio es importante, también lo avisaremos
dentro de la aplicación. Si algún día hubiera cuentas o funciones en la nube,
serían opcionales y esta política se actualizaría antes de activarlas.

## 12. Contacto

Correo: **max.antony.8.5.1@gmail.com**

---

# PinDesk Privacy Policy (English)

Windows app “PinDesk” (`pindesk.exe`). Effective date: November 1, 2026.
This is the policy text for the Microsoft Store listing.

> **In short**
>
> - PinDesk has no user accounts and no servers of its own. What you
>   create is stored on your PC.
> - No ads, no analytics, no crash reports, no tracking, no third-party SDKs
>   that collect data. We do not sell data.
> - Something leaves the PC **only when you use a feature that needs it**:
>   the assistant with your own API key (Anthropic or DeepSeek), Jev from
>   TypeSafe with your key, the assistant's web search, your Google
>   connections, the reader's Microsoft voices, “share by link” or
>   downloading speech and text models. Each case is listed below.
> - Sync with your phone or other devices goes over your local network,
>   encrypted, without going through the internet.
> - Transcription, dictation and live meeting listening (Whisper, Qwen3-ASR,
>   Vosk or the Windows recognizer) and reading text in images run **on your
>   PC, offline**.

## Who we are

PinDesk is made by an independent developer. Contact:
**max.antony.8.5.1@gmail.com**. PinDesk has no accounts and no servers of
its own, so we do not receive or store your data.

## Data kept on your PC

Chats, notes, tasks, schedules, reminders, documents, screenshots, pins,
drawings, photos, videos, voice notes, class and meeting recordings,
transcripts and settings are stored in the data folder
(`%APPDATA%\PinDesk`, or next to the executable in the portable version).
Speech-to-text, OCR and document reading run on the PC. API keys you enter
are encrypted with your Windows user (DPAPI) and never written to the
settings file. Reports you make about AI responses are kept in
`reportes-ia.jsonl`.

## Data that leaves the PC (only when you use the feature)

All of it goes straight from your PC to the service named, over HTTPS; we are
not in the middle and do not see it. Each service handles data under its own
privacy policy. Settings › General › “Privacy and data” shows the same list,
each with a button to where it is turned on or off.

| Feature | What is sent | To whom |
|---|---|---|
| **AI assistant** | What you write, the recent conversation and what you attach. | Anthropic (Claude) or DeepSeek, **with your own API key**. Nothing without a key. |
| **Jev (TypeSafe)** | Text snippets (meeting or class sentences transcribed on the PC, note lines, tasks, what you dictate or explain aloud) and sometimes an image, to get a classification. **Never the audio.** | TypeSafe (`api.typesafe.ai`), with your own key and only with that feature turned on. |
| **Assistant web search** | Search terms and the addresses of the pages it reads. No cookies or account data. | DuckDuckGo, or Brave Search with your key; and the pages read. |
| **Google connections** | The Drive files you pick and the calendar events you ask for. | Google, only if you connect your account; disconnect it in Settings. |
| **Reader's Microsoft voices** | The text read aloud. | Microsoft (Edge online voices), only if you pick one; Windows voices work offline. |
| **Share by link** | The file you choose to share. | A public temporary file host (`litterbox.catbox.moe` or `temp.sh`); anyone with the link can open it until it expires. Do not use it for private things. |
| **Model and dictionary downloads** | Only the download request (the server sees your IP address). Nothing of yours. | `huggingface.co`, `github.com`, `alphacephei.com`, `raw.githubusercontent.com`. |
| **Links you open** | Handled by your browser or the program you choose. | The destination site or program. |

**AI-generated content.** Assistant responses come from third-party models
and may be inaccurate. You can report a response with **“Report
response…”** (message menu › More): pick a reason (offensive, dangerous,
false or other); it is saved in `reportes-ia.jsonl` in your data folder and
your email program opens with the report written, so you can send it to the
contact address if you wish. We only receive what you choose to send.

## Local network sync

Sync with your phone or other devices happens over your local network only,
encrypted with AES-GCM using a key known only to your paired devices. No
internet relay is used.

## Permissions

Microphone (voice notes, dictation, reviewing aloud, recording classes or
meetings), screen capture and camera are used only for features you start.
Network access is used for the features above and for sync. Start with
Windows only if you turn it on. PinDesk does not read your contacts,
location or email.

## What we don't do

No ads, no analytics, no tracking, no crash-reporting SDKs, no sale of
personal data, no use of your data to train models.

## Retention and deletion

Data stays on your PC until you delete it. Settings › General › “Your data,
on your disk” opens the data folder: copy it to take your data with you, or
close PinDesk and delete it to remove everything (downloaded models
included). Keys are removed from Settings › Assistant. Depending on how it
was installed, uninstalling may leave the data folder (so nothing is lost by
accident); delete it by hand to remove everything. Data you sent to third
parties is governed by those services.

## Children

PinDesk is not directed to children under 13 (or the minimum age in your
country) and does not knowingly collect children's data.

## Your rights

Depending on your jurisdiction (e.g. GDPR, Spain's LOPDGDD, Peru's Law
29733) you may access, correct, delete or object to processing of your data;
since the data is on your PC you can do so directly, or contact us.

## Changes and contact

We will publish any change at this address with its date, and announce
important changes in the app. Contact: **max.antony.8.5.1@gmail.com**.
