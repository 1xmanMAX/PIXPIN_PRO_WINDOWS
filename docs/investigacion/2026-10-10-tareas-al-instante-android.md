# Tareas al instante — qué hacer en Android

10-oct-2026. Guía para el agente que lo haga en PixPin Android
(`1xmanMAX/PIXPIN_PRO_ANDROID`, mirado en `main` = v0.119.0, `9ae683a`). Dice **qué cambiar, en
qué archivo y cómo comprobarlo contra el PC**. Lo del PC ya está hecho (rama
`tareas-al-instante` del repositorio del PC).

## Lo que pidió el usuario

> «implementa una función para sincronizar las tareas de forma rápida a todos los dispositivos»

Hoy, con la sincronización automática de los dos lados (PC desde el 8-oct, Android desde
v0.116.0, `593dc08`, «Sincronizar sola»), una tarea tachada tarda **30 s** en salir (`QUIETO`:
se espera a que el chat deje de cambiar) y, si el otro no llama, hasta **10 min** (`PERIODO`).
Para una casilla eso es demasiado: quien tacha en el móvil mira el PC y la quiere tachada ya.

## Lo que hace el PC (para copiarlo)

1. **Quien escribe tareas avisa** (`sincronizar::automatica::avisar_tareas(ficha)`), después de
   escribir: marcar, añadir, quitar, corregir, mover y reponer una tarea (todo pasa por
   `pedidos::reescribir`), borrar o crear una lista, la casilla del panel del chat, la del pin de
   la lista y quitar la marca ⏰ de un recordatorio que sonó.
2. El hilo de la automática junta los avisos y, tras **600 ms sin más toques**
   (`CALMA_TAREAS`) o **1,5 s desde el primero** si no paran (`TOPE_TAREAS`), lanza **una vuelta
   recortada a esos chats** con todos los que contestan a la sonda.
3. Si ya hay una vuelta en marcha (la de la pantalla, la del otro que llamó, la entera de la
   automática), **espera y sale detrás** (se mira cada 250 ms); nunca dos a la par. Lo que se
   tacha mientras se suma a la cola.
4. Si no contesta nadie, lo pendiente se olvida: la vuelta entera de cuando aparezca (`Aparecio`)
   ya lo lleva. Una vuelta entera que empieza se lleva también lo que esperaba.
5. **Sin globos**: ni si sale bien ni si falla (queda en «Actividad»). La vuelta entera de
   siempre sigue detrás (30 s): si la rápida no llegó a alguno, esa lo arregla y esa sí avisa si
   falla seguido.
6. Se apaga con la automática (`[sincro] automatica = false`); no hay ajuste aparte.
7. Lo que llega de otro aparato refresca la ventana de Tareas al momento (antes, a la mirada de
   cada segundo), sin cerrar «Mover a…» ni perder lo que se está corrigiendo.

## El cable: no cambia nada

La vuelta recortada es la de siempre con menos chats. **Quien responde atiende petición a
petición** y nunca sabe ni pregunta cuántos chats se van a juntar: en Android,
`Respondedor.responder` de `sincro/Protocolo.kt` (líneas ~440-545) contesta `catalogo`,
`lapidas`, y por chat `inventario`, `mensajes`, `aplicar`, `archivos`, `pon`, `parche`,
`damecambios`, `dame`, `base`, en el orden que pida quien dirige. Comprobado: no hay estado de
«selección» en el que responde (solo el `alcance` del último `archivos`, que es por chat). Sin
subir `VERSION`.

En el PC: `pixpin_sincro::vuelta::solo_estos` (en `crates/pixpin-sincro/src/vuelta.rs`) =
la vuelta sin preguntar, recortada:

- de los chats pedidos, **solo lo que entraría en la vuelta entera**: lo elegido la última vez
  con ese aparato (`disco.elegidos(otro.id)`, o todo si nunca se eligió) y nada con lápida;
- **no borra proyectos**, y solo levanta lápidas de esos chats;
- **sin la galería de capturas** (es lo que más tarda y no tiene que ver);
- **una ronda** con cada aparato, no dos.

## Qué cambiar en Android

### 1. `sincro/SincronizacionAutomatica.kt` — `VueltaCallada`

Añadir el recorte, sin tocar lo demás:

```kotlin
fun con(entrada: InputStream, salida: OutputStream, soloEstos: Set<String>? = null): Resultado {
    ...
    val candidatos = (mios.keys + suyos.keys).filter { it !in conLapida }
    val elegidos = disco.elegidos(otro.id) ?: candidatos.toSet()
    for (chat in candidatos.filter { it in elegidos && (soloEstos == null || it in soloEstos) }) {
        ... // igual
    }
    if (soloEstos == null) sesion.galeria(hecho, ahora())
    ...
}
```

### 2. `sincro/SincronizacionAutomatica.kt` — el reloj de las tareas (puro, con prueba)

Junto a `RelojDeSincro`, la misma lógica que `Reloj::siguiente` del PC
(`apps/pixpin/src/sincronizar/automatica.rs`):

```kotlin
class ColaDeTareas {
    private val chats = LinkedHashSet<String>()
    private var desde = 0L
    private var ultimo = 0L
    @Synchronized fun avisan(chat: String, ahora: Long) {
        if (chats.isEmpty()) desde = ahora
        chats += chat; ultimo = ahora
    }
    /** ms hasta mandarlas (0 = ya), o null si no espera ninguna. */
    @Synchronized fun espera(ahora: Long): Long? =
        if (chats.isEmpty()) null else maxOf(0L, minOf(ultimo + CALMA, desde + TOPE) - ahora)
    @Synchronized fun tomar(): Set<String> = chats.toSet().also { chats.clear() }
    companion object { const val CALMA = 600L; const val TOPE = 1_500L }
}
```

Pruebas a copiar de `automatica.rs` (`mod pruebas`, las seis de tareas): sale a los 600 ms y no
antes; cuatro clics cada 400 ms salen juntos a los 1500 ms por el tope; con una vuelta en marcha
esperan y salen detrás, sumando lo tachado mientras; sin nadie que conteste se olvidan; una
vuelta entera se las lleva. Ponerlas en `app/src/test/java/com/forge/pixpin/RelojDeSincroTest.kt`.

### 3. `sincro/SincronizacionAutomatica.kt` — despertar el bucle

Hoy `bucle()` duerme `Thread.sleep(LATIDO)` (5 s) y sondea cada 15 s. Cambiarlo por una espera
que se pueda despertar, p. ej. `private val despertar = java.util.concurrent.Semaphore(0)` y
`despertar.tryAcquire(espera, TimeUnit.MILLISECONDS)` con
`espera = cola.espera(ahora)?.coerceIn(50, LATIDO) ?: LATIDO`. Y:

```kotlin
/** Cambiaron las tareas de estos chats (`Disco.chatDe(m)`): que salgan ya. */
fun avisarTareas(chats: Set<String>) {
    if (!::app.isInitialized || chats.isEmpty() || !encendida(app)) return
    val ahora = System.currentTimeMillis()
    chats.forEach { cola.avisan(it, ahora) }
    despertar.release()
    if (hilo?.isAlive != true) lanzarSuelta()   // ver «Sin PixPin a la vista»
}
```

En cada vuelta del bucle, **antes** de `reloj.toca(...)`:

```kotlin
if (cola.espera(ahora) == 0L) {
    val chats = cola.tomar()
    for ((otroId, m) in ultimosPresentes) {           // los del último sondeo, sin sondear otra vez
        val (host, puerto) = dirs[otroId] ?: continue
        vueltaDeTareas(disco, otroId, m.nombre, host, puerto, chats)
    }
}
```

- `ultimosPresentes`: guardar el `presentes` del último sondeo (hoy es local al `if`). No volver a
  sondear a todos antes de mandar: son 1-2 s por aparato apagado.
- `vueltaDeTareas` = `vuelta(...)` con `VueltaCallada(...).con(..., soloEstos = chats)`, **sin
  `decir`** (ni Toast si sale bien ni si falla; que no cuente en `fallos`: la vuelta de `QUIETO`
  sigue detrás y esa sí avisa).
- **Ocupado** (`e.message == Protocolo.OCUPADO`, otra vuelta en marcha aquí o allí): volver a
  meter los chats en la cola (`chats.forEach { cola.avisan(it, ahora - ColaDeTareas.TOPE) }`,
  para que salgan en cuanto se pueda) y esperar 250 ms. Nunca dos vueltas a la par
  (`Protocolo.ocupar` ya lo impide).
- Si al empezar una vuelta entera (`reloj.toca` da algo) había tareas en la cola, `cola.tomar()`:
  van dentro.
- **No llamar a `reloj.despuesDeSincronizar(...)`** tras la rápida: lo demás que cambiara en los
  chats tiene que seguir pendiente para la vuelta de `QUIETO`.

### 4. `guardados/MensajesStore.kt` — avisar desde un solo sitio

Todo lo que toca tareas en Android escribe por `MensajesStore`: `MiniActivity.guardar` (línea
~117, `almacen.cambiar`), `TodasLasTareasActivity` (líneas ~142-217, `almacen.cambiar`),
`TareaRapidaActivity` (88-115, `actualizar`/`anadir`), `widget/TocarTareaDelWidget.kt` (50,
`cambiar`), `pin/RecordatoriosDeTareas.kt` (42, `cambiar`) y borrar la lista en `MiniActivity`
(997). En vez de tocar los seis, **avisar en `reescribir` y en `anadir`**, comparando las listas
de tareas de antes y de después (`reescribir` ya lee lo de antes para `anotarBorrados`):

```kotlin
private fun chatsConTareasCambiadas(antes: List<Mensaje>, despues: List<Mensaje>): Set<String> {
    fun tareas(l: List<Mensaje>) = l.filter { it.miniapp == "tareas" }.associateBy { it.id }
    val a = tareas(antes); val d = tareas(despues)
    return (a.keys + d.keys)
        .filter { a[it]?.texto != d[it]?.texto || (a[it] == null) != (d[it] == null) }
        .mapNotNullTo(HashSet()) { (d[it] ?: a[it])?.let(com.forge.pixpin.sincro.Disco::chatDe) }
}
```

y al final de `reescribir` (fuera del `runCatching` de escribir, tras escribir bien):
`com.forge.pixpin.sincro.SincronizacionAutomatica.avisarTareas(chatsConTareasCambiadas(viejos, mensajes))`.
En `anadir`, si el mensaje nuevo es `miniapp == "tareas"`, lo mismo con su chat.

**Importante — sin rebotes**: lo que escribe la sincronización va por `sincro/Disco.kt`
(`aplicarMensajes` → `escribirMensajes`), **no** por `MensajesStore`, así que lo que llega del PC
no vuelve a salir. Comprobarlo antes de dar esto por bueno: si algún día la sincronización
escribiera por `MensajesStore.reescribir`, cada tarea rebotaría entre los dos aparatos. Y no
colgar el aviso del flujo `MensajesStore.cambios`: ese sube también con lo que llega.

### 5. Sin PixPin a la vista (el widget)

El bucle de la automática solo vive con alguna pantalla de PixPin delante (`Presencia` →
`alaVista`). Una casilla del widget (`TocarTareaDelWidget`) o un recordatorio que suena se
tachan sin pantalla. Para esos, `avisarTareas` lanza un hilo suelto de una vez (`lanzarSuelta`):
espera la calma, sondea las direcciones apuntadas (`disco.direcciones()`, `Red.sondear`, 1,2 s
como mucho cada una), hace la vuelta recortada con los que contesten y termina. Sin escuchar ni
anunciarse (eso es de `Presencia`).

### 6. Lo que llega del PC ya refresca solo

`TodasLasTareasActivity` relee con `MensajesStore.cambios.debounce(250)` y `Red.disco` sube
`cambios` con cada `Disco.Cambio.MENSAJES`: lo tachado en el PC sale en la lista del móvil en
cuanto se escribe. Comprobar que el widget también se repinta (`WidgetDeTareas`): si no, avisarlo
desde `Red.disco` con `Disco.Cambio.MENSAJES`.

## Números (medidos en el PC, 10-oct)

`crates/pixpin-proyecto/tests/tareas_al_instante.rs` (dos PixPin de PC con su carpeta cada uno
y un móvil de mentira —`DiscoAndroid`, el `Disco.kt` portado—, por un socket de la máquina con
el cifrado de verdad, `--release`). Tiempo de la vuelta rápida desde antes de conectar hasta
que la tarea está escrita en el otro, tres tachaduras seguidas:

| Datos | PC → PC | PC → «móvil» | Vuelta entera PC → PC |
|---|---|---|---|
| 31 chats, 1200 notas (10× los datos del usuario hoy: 12 chats, 112 mensajes) | 182-193 ms | 470-501 ms | 4,6 s |
| 31 chats, 9000 notas (`-- --ignored`) | 409-617 ms | 3,1-3,2 s | 15 s |

Con la calma de 600 ms: **0,8-1,1 s de tachar a escrito** con datos como los del usuario.

Lo que pesa con muchos mensajes es lo que hace **quien responde** en cada `inventario` (dos por
chat: uno al preparar y otro al cerrar): `Copias.hacer`, `sellar()` y, en el móvil, leer el
`guardados.jsonl` entero (que allí tiene todos los chats). Medido en el «móvil» de mentira con
9000 notas: `hacer_copia` 0,4 s, `sellar` 0,4 s, `apuntes` 0,14 s, cada vez. En el PC, `sellar`
recorría todos los cuadernos (0,22 s) y ahora **recuerda los ya sellados** por fecha y tamaño
(`vista.rs`, `ya_sellado`): la vuelta rápida bajó de 1,1 s a 0,45 s y la entera de 28 s a 15 s.
**Para Android, lo mismo** (no cambia el cable): en `Disco.sellar()` no releer si
`guardados.jsonl` tiene la misma fecha y tamaño que tras el último sellado, y en `Copias.hacer`
no rehacer la copia de un chat si su huella no cambió (la cuenta de «nada cambió» que ya hace,
pero sin leer el archivo entero primero).

## Cómo probarlo contra el PC

1. **Sin teléfono** (en el PC del repositorio de Android): `herramientas/pc-simulado` es el PC de
   verdad. `preparar <dir> <codigo>` ya deja una lista «Compras» en Mensajes guardados;
   `responder <dir> <codigo>` escucha. Desde una prueba de Robolectric/JVM, tachar en
   `MensajesStore` y comprobar con `ver <dir>` que en el PC queda `- [x]` sin esperar 30 s.
   Las pruebas de `SincronizarConElPcTest.kt` muestran cómo montar los dos lados.
2. **Con el móvil y el PC del usuario** (los dos con PixPin abierto, en la misma Wi-Fi y el mismo
   grupo, «Sincronizar sola» encendido en los dos):
   - Abrir Tareas en el PC y en el móvil. Tachar en el móvil: tiene que verse tachada en el PC en
     **menos de 2 s** (antes: 30 s o 10 min).
   - Tachar tres seguidas rápido: llegan juntas (una vuelta), no tres.
   - Tachar en el PC: se ve en el móvil en menos de 2 s (eso ya funciona con el PC de esta rama).
   - Con Sincronizar abierta en el PC, «Actividad» dice «Tareas al instante: Al día con …».
   - Caso negativo: apagar el Wi-Fi del móvil, tachar, encenderlo: llega con la vuelta de
     «apareció», sin errores en pantalla.
   - Caso negativo: un chat desmarcado con ese aparato en «¿Qué sincronizar?» no viaja.

## Riesgos

- **Copias de seguridad**: quien responde hace `Copias.hacer` del chat en cada `inventario`
  (`Protocolo.kt` ~465 y `copias.rs` del PC), y se quedan las **30 más nuevas por proyecto**.
  Si no cambió nada desde la última copia no se hace otra, pero cada tanda de casillas sí es un
  cambio: tachar 30 veces seguidas en ratos distintos empuja fuera las copias viejas de ese chat.
  Si molesta, que `Copias.hacer` no haga otra si la última de ese chat tiene menos de N minutos
  (cambio solo de Android y del PC, sin tocar el cable).
- Más vueltas cortas = más conexiones: con la calma de 600 ms y el tope de 1,5 s, tachar una
  lista de 20 casillas en 10 s son unas 7 vueltas de un chat, cada una de décimas de segundo.
