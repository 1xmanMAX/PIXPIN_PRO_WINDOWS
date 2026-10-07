# Cualquier archivo del PC al chat abierto del móvil — qué hacer en Android

7-oct-2026. Es la continuación de
[`2026-10-06-foto-al-lienzo-android.md`](2026-10-06-foto-al-lienzo-android.md), que ya está
hecha en Android v0.105.0. Esta guía es para el agente que lo haga en Android: dice **qué
cambiar, en qué archivo y cómo comprobarlo en el móvil del usuario**.

## Lo que pidió el usuario

> «pasar archivos de todo tipo de formato rápidamente a cualquier chat que esté abierto en ese
> momento; de la misma forma el celular o computadora que lo recibe, si tiene un chat o canvas
> abierto, lo recibe directamente; en el caso del canvas solo recibe fotos, pero el chat
> cualquier cosa»
>
> «directamente que se niegue a enviar ya que está un lienzo abierto y este solo acepta fotos,
> pero si está el chat abierto pues se pueda enviar directamente a este, sea foto o cualquier
> cosa»

Es decir, al recibir un `suelto`, según lo que haya delante en el móvil:

| Delante en el móvil | Llega una foto | Llega otro archivo (PDF, video, Word…) |
|---|---|---|
| **Un chat** (`MensajesActivity` de un proyecto, o la Conversación general) | a **ese** chat | a **ese** chat |
| **Un lienzo** (`DrawEditorActivity`) | al lienzo (como hoy) | **se niega**: no se recibe nada |
| Nada de eso (otra pantalla, o PixPin sin pantalla a la vista) | Conversación general (como hoy) | Conversación general (como hoy) |

Todo lo del PC ya está hecho (rama `plugin-sincronizar`): el plugin manda archivos de cualquier
tipo desde `p s`, el PC que recibe hace esta misma tabla y el globo del que manda dice dónde quedó
o por qué se negó.

## 1. Lo que cambia en el cable (sin subir el protocolo: sigue en `VERSION = 4`)

```text
PC  → {"t":"suelto","nombre":"informe.pdf","mime":"application/pdf","bytes":N,"destino":"abierto"}
mov → {}                                         (vale: mándalo)
    o {"error":"MaxPhone 40 tiene un lienzo abierto: solo acepta fotos"}   (y NO llegan trozos)
PC  → TROZO… TROZO (N bytes) + {"resumen":"<sha256>"}
mov → {"t":"listo","donde":"chat_abierto","chat":"Tesis"}
    o {"t":"listo","donde":"lienzo"}
    o {"t":"listo","donde":"chat"}               (la Conversación general: nada abierto)
```

Cuatro cosas nuevas, nada más:

1. **`destino` puede venir como `"abierto"`** (el PC del 7-oct) o como `"lienzo"` (el del 6-oct).
   Se tratan igual: «lo que esté abierto». No hace falta mirarlo.
2. **Negarse ANTES del «vale»**: si hay un lienzo delante y `mime` no empieza por `image/`, se
   contesta con `error` y **no se manda el `{}`**. El texto tiene que **acabar** exactamente en
   `tiene un lienzo abierto: solo acepta fotos` (el PC lo reconoce por ese final; delante va el
   nombre del móvil, `Identidad.yo.nombre`). La conexión sigue sana: el PC puede mandar detrás
   una foto que sí entra.
3. **`donde` tiene un valor más, `"chat_abierto"`**, y la respuesta lleva **`chat`** con el
   nombre del chat donde quedó (el que se ve arriba en la pantalla del chat). `"chat"` sigue
   queriendo decir «la Conversación general» (lo que ya devuelve v0.105.0).
4. **El tope sube de 64 MB a 2 GB** (`TOPE_DE_SUELTO`): ahora pasan videos. El fichero ya se
   escribe a disco por trozos (`recibirTrozos` a un `outputStream`), así que no cuesta memoria.

El PC del 6-oct (sin esto) y el móvil de hoy siguen hablando bien entre sí: un móvil v0.105.0
que reciba un PDF con un lienzo delante lo manda a la Conversación general, como hoy.

## 2. Qué tocar en Android (v0.105.0)

### 2.1 `sincro/Protocolo.kt`

- `TOPE_DE_SUELTO = 64L shl 20` → `2L shl 30`.
- Constantes nuevas junto a `SUELTO_EN_EL_LIENZO` / `SUELTO_EN_EL_CHAT`:
  ```kotlin
  const val SUELTO_EN_EL_CHAT_ABIERTO = "chat_abierto"
  const val SOLO_FOTOS = "tiene un lienzo abierto: solo acepta fotos"
  ```
- `data class Respuesta`: añadir `val chat: String? = null` (el nombre del chat donde quedó). Va
  al final, con valor por omisión: el resto de respuestas no cambia.
- `recibirSuelto(canal, p, otro)`: hoy solo tiene un `alRecibirSuelto(fichero, nombre, mime,
  otro): String`. Hay que partirlo en dos pasos, porque **negarse tiene que ir antes del `{}`**:
  ```kotlin
  /** Antes del «vale»: null = se acepta; un texto = se niega con ese error. */
  private val alAceptarSuelto: ((nombre: String, mime: String) -> String?)? = null,
  /** Ya llegado: dónde quedó y, si fue un chat abierto, su nombre. */
  private val alRecibirSuelto: ((File, String, String, Aparato) -> Pair<String, String?>)? = null,
  ```
  y en `recibirSuelto`, después de comprobar el tope y **antes** de
  `Protocolo.enviar(canal, Protocolo.Respuesta())`:
  ```kotlin
  alAceptarSuelto?.invoke(nombre, p.mime ?: "application/octet-stream")?.let { e ->
      Protocolo.enviar(canal, Protocolo.Respuesta(error = e)); return
  }
  ```
  y al final `Protocolo.Respuesta(t = "listo", donde = donde.first, chat = donde.second)`.
- Cambiar los textos «La imagen es demasiado grande» / «La imagen cambió…» por «El archivo…».

### 2.2 Un «chat al frente», como `LienzoAlFrente`

Nuevo `guardados/ChatAlFrente.kt`, igual que `motor/LienzoAlFrente.kt`:

```kotlin
/** El chat que el usuario tiene delante ahora mismo: su proyecto (null = la Conversación general) y su nombre. */
object ChatAlFrente {
    data class Chat(val proyecto: String?, val nombre: String)
    @Volatile private var dueño: WeakReference<Any>? = null
    @Volatile private var chat: Chat? = null
    fun poner(quien: Any, c: Chat) { dueño = WeakReference(quien); chat = c }
    fun quitar(quien: Any) { if (dueño?.get() === quien) { dueño = null; chat = null } }
    fun actual(): Chat? = chat?.takeIf { dueño?.get() != null }
}
```

En **`guardados/MensajesActivity.kt`**:

- `onResume()`: `ChatAlFrente.poner(this, ChatAlFrente.Chat(chatDe, nombre))`, donde `nombre`
  es `nombreDelChat` o, con `chatDe == null`, el título de la Conversación general tal como sale
  en pantalla.
- `onPause()`: `ChatAlFrente.quitar(this)`.
- Como `chatDe` cambia sin salir de la pantalla (al entrar en un chat desde la general, línea
  ~455 `chatDe = null`, y al abrir otro), volver a llamar a `poner` cada vez que cambie `chatDe`
  (un `LaunchedEffect(chatDe)` o en el mismo sitio donde se asigna).
- Lo que entra en el chat ya se ve solo: la pantalla escucha `MensajesStore.cambios` y recarga
  (`recargar++`), así que no hay que avisarla.

`LienzoAlFrente` ya existe y ya lo ponen `DrawEditorActivity.onResume/onPause`. No hay que tocarlo.

**Si hay los dos a la vez** (pantalla partida): gana el que tuvo el `onResume` más reciente.
Basta con guardar un contador o la hora en `poner` de cada uno y comparar.

### 2.3 `sincro/Presencia.kt` (líneas ~80-142)

Pasar `alAceptarSuelto` al `Respondedor` y cambiar `alLienzoOAlChat`:

```kotlin
private fun aceptar(nombre: String, mime: String): String? {
    val lienzo = LienzoAlFrente.actual()
    val chat = ChatAlFrente.actual()
    // Un lienzo delante (y no un chat más reciente) solo acepta fotos.
    if (lienzo != null && lo_mas_reciente_es_el_lienzo && !mime.startsWith("image/"))
        return "${identidad.yo.nombre} ${Protocolo.SOLO_FOTOS}"
    return null
}

private fun alLoAbierto(fichero: File, nombre: String, mime: String, otro: Aparato): Pair<String, String?> {
    // 1) Lienzo delante + foto → recibirImagen (lo de hoy) → ("lienzo", null)
    // 2) Chat delante → AlChat.meterArchivo(app, copia, nombre, mime, chat.proyecto)
    //                 → ("chat_abierto", chat.nombre)
    // 3) Si no → lo de hoy: Recepcion.guardar a la Conversación general → ("chat", null)
}
```

- Para el chat usar **`AlChat.meterArchivo(context, archivo, nombre, tipo, proyecto)`**
  (`guardados/AlChat.kt`): ya copia al almacén y da de alta IMAGEN, VOZ o ARCHIVO según el tipo,
  igual que «Compartir → Mensajes guardados». `proyecto = null` es la Conversación general.
- **Hacer una copia antes** (como hace hoy `alLienzoOAlChat` con `chat_` + nombre): el
  `Respondedor` borra el fichero recibido al volver.
- Volver a mirar lo abierto en `alLoAbierto` (no fiarse de lo de `aceptar`): entre el «vale» y el
  último trozo de un video pueden pasar segundos y el usuario puede haber cambiado de pantalla. Si
  ahora hay un lienzo delante y no es una foto, lanzar una excepción con el texto de `SOLO_FOTOS`:
  `recibirSuelto` ya la convierte en `error`.
- `apuntar(...)`: «Archivo de PC puesto en el chat Tesis», «Foto de PC puesta en el lienzo»…

### 2.4 Pruebas JVM (en `app/src/test/`)

Junto a las del 6-oct (las de `Protocolo`/`Presencia` con el PC simulado):

1. Chat delante + PDF → `{}` (vale), llega entero, respuesta `donde = "chat_abierto"` y
   `chat = "<nombre>"`; y el mensaje está en ese proyecto con clase ARCHIVO.
2. Chat delante + foto → también al chat (no al lienzo) con clase IMAGEN.
3. **Caso negativo:** lienzo delante + PDF → `error` que acaba en `SOLO_FOTOS`, **sin `{}`
   previo y sin leer ningún TROZO**; la conversación sigue y un PNG detrás sí entra al lienzo.
4. Nada delante + PDF → `donde = "chat"` y está en la Conversación general.
5. `destino = "lienzo"` (el PC del 6-oct) se comporta igual que `"abierto"`.
6. `bytes = 2L shl 30` se acepta; `(2L shl 30) + 1` se niega con «demasiado grande».

`herramientas/pc-simulado` (en el repo de Android) ya habla `suelto`: basta con darle un PDF.

## 3. Cómo comprobarlo en el móvil del usuario (con el PC de verdad)

Antes: en el PC tiene que estar instalada la versión de la rama `plugin-sincronizar` (ya lo
está), y el móvil en el mismo grupo y en la misma Wi-Fi. **PixPin tiene que estar a la vista en
el móvil**: solo escucha con una pantalla delante.

1. **Instalar** el APK nuevo en el móvil (`adb install -r PixPin-<versión>.apk`, sin desinstalar:
   así no se pierden los datos).
2. **Que el PC vea al móvil:** en el PC, Flow Launcher → escribir `p s`. El móvil tiene que salir
   con 🟢 Conectado (puede tardar hasta un minuto; con la ventana Sincronizar del PC abierta, unos
   segundos).
3. **Chat abierto + PDF:** en el móvil abrir el chat de un proyecto (por ejemplo «Tesis»). En el
   PC copiar un PDF en el Explorador, en Flow `p s` → Ctrl+V (sale `[archivo 01]`) → elegir el
   móvil. Esperado: el PDF aparece **en ese chat** del móvil en unos segundos, y en el PC sale el
   globo «Enviado al chat «Tesis» de <móvil>».
4. **Chat abierto + foto:** igual con una imagen (`[img 01]`). Esperado: la foto queda en **el
   chat** (no en un lienzo).
5. **Lienzo abierto + PDF (se niega):** en el móvil abrir un lienzo. Mandar el PDF. Esperado: en
   el móvil **no entra nada**, y en el PC el globo dice «<móvil> tiene un lienzo abierto: solo
   acepta fotos. Abre un chat para mandarle archivos.»
6. **Lienzo abierto + foto:** mandar una imagen. Esperado: aparece en el centro del lienzo, como
   hasta ahora; globo «Enviada al lienzo de <móvil>».
7. **Lienzo abierto + foto y PDF juntos:** pegar las dos cosas y mandar. Esperado: la foto entra
   al lienzo y el globo añade «1 archivo no se envió: no era una foto y tiene un lienzo abierto».
8. **Nada abierto:** dejar el móvil en otra pantalla de PixPin (por ejemplo la de Proyectos) y
   mandar un PDF. Esperado: queda en la Conversación general; globo «… lo guardó en la
   conversación general (no tenía un chat ni un lienzo abierto)».
9. **Un archivo grande:** mandar un video de 100-300 MB al chat abierto. Esperado: llega entero y
   se puede reproducir en el chat.
10. **Al revés, del móvil al PC** no entra en esta guía (el móvil aún no tiene botón para mandar
    `suelto`); el PC ya sabe recibirlo con la misma tabla.

Si en el paso 2 el móvil sale ⚪, abrir PixPin en el móvil y esperar; si en el 3 el globo dice
«Actualiza PixPin en …», el APK instalado no es el nuevo.

## 4. Dónde está lo del PC, para comparar

- Cable y lado que recibe de referencia: `crates/pixpin-sincro/src/al_lienzo.rs` (`Suelto`,
  `Recibe`, `responder`, `SOLO_FOTOS`, `Donde::ChatAbierto`, `TOPE_DE_SUELTO`). Sus pruebas
  (`con_un_chat_abierto_llega_cualquier_cosa_y_dice_a_que_chat`,
  `caso_negativo_con_un_lienzo_delante_lo_que_no_es_foto_se_niega_sin_mandarlo`) son el modelo de
  las de Android.
- Qué hay abierto en el PC: `apps/pixpin/src/al_frente.rs` (la tabla de arriba es su `decidir`).
- Quien manda y los avisos: `apps/pixpin/src/sincronizar/al_movil.rs` y las claves `al-movil-*`
  de `crates/pixpin-store/i18n/*/main.ftl`.
