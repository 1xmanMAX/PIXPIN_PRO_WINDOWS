# Ficha de PinDesk en la Microsoft Store

Todo lo que hay que **pegar o subir** en Partner Center para la ficha de la
tienda. Nosotros no podemos entrar en la cuenta: esto se deja listo para que
el usuario solo copie y pegue. El orden de las pantallas está en
[`pasos.md`](pasos.md); la clasificación por edades, en
[`clasificacion.md`](clasificacion.md).

## Identidad (ya reservada en Partner Center)

| Dato | Valor |
|---|---|
| Nombre reservado (display name) | `PinDesk` |
| Package/Identity/Name | `MingXuan.PinDesk` |
| Publisher display name | `Ming Xuan` |
| Store ID | `9NKVP3K59V66` |
| Enlace de la tienda | https://apps.microsoft.com/detail/9NKVP3K59V66 |

El `Publisher` (`CN=…`) del manifiesto lo da Partner Center en *Product
management → Product identity*; lo pone el agente que hace el MSIX
(`tienda/AppxManifest.xml`), no esta ficha.

## Ficheros

| Fichero | Qué es |
|---|---|
| `<idioma>.md` (14) | Los campos de «Store listings» de cada idioma, en bloques `text` listos para pegar. |
| `comprobar.py` | Comprueba los límites de Partner Center en las 14 fichas: `python3 tienda/ficha/comprobar.py`. |
| `logos/` | Logos opcionales de la ficha (300×300, 1:1 y 2:3) y el script que los dibuja. |
| `capturas/componer.py` | Compone las capturas 1920×1080; lo usa `.github/workflows/capturas-tienda.yml`. |
| `clasificacion.md` | Respuestas recomendadas del cuestionario IARC. |
| `pasos.md` | El orden exacto en Partner Center y la nota para certificación. |

Idiomas (los 14 de la interfaz): `es-ES`, `en-US`, `zh-CN`, `ja-JP`, `ko-KR`,
`hi-IN`, `ar-SA`, `tr-TR`, `pt-BR`, `it-IT`, `fr-FR`, `de-DE`, `ru-RU`,
`id-ID`. En Partner Center se añaden en *Store listings → Manage additional
languages*. Si el paquete MSIX declara menos idiomas en `<Resources>`, Partner
Center deja añadir igual la ficha de los demás, pero avisará; lo limpio es que
el manifiesto declare los 14.

## Campos y límites (los de Partner Center)

| Campo (Partner Center) | En español | Límite | Qué ponemos |
|---|---|---|---|
| Product name | Nombre | el reservado | `PinDesk` en todos los idiomas |
| Description | Descripción | 10 000 caracteres, obligatoria | ~1 100–3 100 según el idioma; las 2–3 primeras líneas dicen qué es |
| What's new in this version | Novedades | 1 500 | «Primera versión en Microsoft Store» y lo principal |
| Product features | Características | hasta 20, 200 caracteres cada una | 15, una por renglón (cada renglón va en su casilla) |
| Short description | Descripción corta | 1 000 | Un párrafo; se ve en algunas superficies (Xbox, resultados) |
| Search terms | Términos de búsqueda | hasta 7, 30 caracteres cada uno, 21 palabras en total | 7 por idioma, uno por casilla |
| Copyright and trademark info | Copyright | 200 | `© 2026 Ming Xuan` |
| Additional license terms | Términos de licencia adicionales | 10 000 | **Vacío** (ver abajo) |
| Support contact info | Contacto de soporte | URL o correo | https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS/issues |
| Website | Sitio web | URL | https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS |
| Privacy policy URL | Política de privacidad | URL | https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS/blob/main/docs/privacidad.md |

`comprobar.py` cuenta con `len()` de Python (puntos de código), que es como
cuenta Partner Center. Resultado actual: los 14 idiomas pasan.

### Additional license terms: vacío

Si se deja vacío, valen los *Microsoft Standard Application License Terms*,
que es lo normal. Si se pone texto, **sustituye** a esos términos. El código
es MIT (`LICENSE`), pero eso no hace falta repetirlo aquí: el repositorio ya lo
dice. Si algún día se quiere, basta con una línea:
`This app is open source under the MIT License: https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS/blob/main/LICENSE`.

### Política de privacidad: tiene que estar en `main`

La URL apunta a `docs/privacidad.md` **de la rama `main`**. A 1-nov-2026 ya
está en `main` (commit `0b6642d`), en español e inglés, escrita a partir del
código: lo que se queda en el PC, lo que sale solo si el usuario usa la
función (Anthropic, DeepSeek, TypeSafe/Jev, búsqueda web, Google, voces de
Edge, compartir por enlace, descarga de modelos), la sincronización local
cifrada, los permisos, «Reportar respuesta…», el borrado y el contacto. Se
comprobó en el código que no hay analítica, telemetría ni envío de fallos.

Un detalle a corregir en `main`: el enlace «[English]» del principio apunta a
`#pixpin-max-privacy-policy-english`, pero el título ahora es «PinDesk
Privacy Policy (English)», así que el ancla correcta es
`#pindesk-privacy-policy-english`.

## Lo que la ficha dice y por qué (honestidad)

- **Solo funciones que existen** (sacadas del README y de `docs/`): captura y
  pines, proyectos como chats, lienzo, visores de PDF/Office/DWG/Revit/IFC/
  LandXML, grabar clases y reuniones con transcripción en el equipo, Jev,
  «Estudiar la clase», Anki, tareas/Kanban/horario, timeline y lecciones,
  asistente, sincronizar con Android y el móvil como cámara.
- **Lo que necesita claves del usuario se dice**: el asistente (Claude o
  DeepSeek) y Jev (TypeSafe) son opcionales y con la clave propia; sin clave,
  lo demás funciona igual.
- **La marca PixPin no sale** (es de DepthPixel, ver el README): la app de
  Android se nombra como «la app complementaria de Android». `comprobar.py`
  falla si aparece «PixPin».
- «14 idiomas»: los catálogos de los 12 idiomas nuevos están al ~93 % (lo que
  falta se ve en inglés, `docs/idiomas.md`). La ficha dice «interfaz en 14
  idiomas», que es verdad; no dice «traducción completa».
- «Sin anuncios y sin telemetría»: comprobado en el código (no hay SDK de
  analítica ni de fallos; los registros se quedan en la carpeta de datos).
- La transcripción baja su modelo la primera vez (Whisper/Vosk, de
  huggingface.co y alphacephei.com); la ficha dice «en tu PC», que es donde se
  ejecuta.

## Capturas de pantalla

- Partner Center pide al menos 1 (mejor 4–8, hasta 10) por idioma, PNG,
  1366×768 o más; usamos 1920×1080.
- Se hacen con el flujo **«Capturas para la Store»**
  (`.github/workflows/capturas-tienda.yml`, *Actions → Run workflow*). Deja
  el artefacto `capturas-tienda` con `compuestas/es/` y `compuestas/en/`
  (título en español o en inglés sobre la misma pantalla) y `informe.md`.
- **La interfaz de las muestras está en español** (las pruebas pintan con el
  catálogo español y datos de ejemplo en español). Para `es-ES` va perfecto;
  para `en-US` y los demás, las de `compuestas/en/` llevan el título en inglés
  sobre la interfaz en español. Si se quieren capturas con la interfaz en
  inglés, hay que hacerlas a mano con la app en inglés (Ajustes › General ›
  Idioma) o añadir el idioma a las pruebas de muestra (en `main`).
- Para los otros 12 idiomas se pueden subir las de `en`: Partner Center deja
  usar las mismas en todos.
- Escenas que hay (prueba → PNG): ver la tabla de `pasos.md`, paso 6.

## Logos (opcionales)

En `logos/` (dibujados de nuevo a partir del icono `pixpinmax.ico`, aro azul
`#2A7AE2`):

| Fichero | Dónde va en Partner Center |
|---|---|
| `logo-300x300.png` | Store logos → *1:1 app tile icon* (300×300), fondo transparente |
| `logo-1080x1080.png` / `logo-2160x2160.png` | Store logos → *1:1 box art* |
| `poster-720x1080.png` / `poster-1440x2160.png` | Store logos → *2:3 poster art* |

Ninguno es obligatorio: si no se suben, la tienda usa los logos del paquete.
Recomendado subir al menos el 1:1 box art y el 2:3 poster art, que la tienda
usa en sus listas y destacados.
