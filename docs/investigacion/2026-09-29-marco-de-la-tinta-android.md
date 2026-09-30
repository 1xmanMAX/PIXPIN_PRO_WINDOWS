# El marco de la tinta (`anot-….hoja`): instrucciones para Android

Fecha: 29-sep-2026. Fila K21 de la lista de implementación. El lado del PC ya está hecho (sin commit).

## La idea (del usuario)

> «un marco nuevo invisible que no sea ninguna de las funciones ya existentes, sino una función que sirva solo para encajar lo anotado, y al pasarlo la otra app lo escale según el marco [...] como un marco en un canvas infinito, que la tinta se ajuste con respecto al marco, se reescale y todo según el marco para mantener la fidelidad de las anotaciones».

Cada aparato escribe la tinta de un documento en las unidades de **su** capa. Hasta hoy el otro aparato tenía que adivinarlas: en un PDF por los espacios (`anot-<uid>.espacios` → `capa_del_movil`, K20), en un Word por el margen izquierdo de la maqueta (`izq`, K16). El marco dice las unidades en vez de adivinarlas: es **dónde está la hoja** (la página de un PDF, la columna de un Word o un libro) **en las mismas coordenadas en que están escritos los puntos de su fichero de tinta**. Quien lee lleva ese rectángulo al rectángulo de su propia hoja y la tinta cae encima.

## Formato

- Fichero **hermano** de la tinta, en `pins/draw/` (no es un elemento de la escena: la `Scene` del móvil tira la escena entera con un tipo que no conoce, y una versión vieja pintaría un recuadro):
  - `anot-<uid>-p<n>.hoja` — página `n` (desde 0) de un PDF suelto del chat.
  - `anot-<uid>.hoja` — un Word o un libro.
  - `anot-<uid>-texto.hoja` — un PDF leído como texto (mismo formato que Word; el PC aún no lo escribe).
  - **El PDF de un proyecto no lleva marco**: su hoja es el dibujo del editor del proyecto, que está siempre a 1400 desde el cero; el marco es fijo e implícito, `0,0,1400,1400/proporción`.
- Contenido: texto UTF-8, dos líneas:
  ```
  x0,y0,x1,y1
  v1
  ```
  Punto decimal, `Locale.ROOT` (nunca coma decimal). El PC escribe hasta 3 decimales sin ceros de cola (`-1050,0,2450,4950`) y lee también `1400.0` y la notación de Kotlin. La segunda línea es opcional al leer; si está, tiene que ser `v1`.
- **No vale** (se ignora y se usa la regla de antes): menos o más de 4 números, algo que no es número, `NaN`/`inf`, ancho o alto ≤ 0,001, versión distinta de `v1`, líneas de más.
- **Word/libro**: el marco es la banda de la columna desde lo alto del documento, **tan alta como ancha**: `izq,0,izq+columna,columna` (en píxeles CSS de la capa). No se usa el alto del documento: depende de quién maqueta (el `WebView` suma su relleno), y el texto crece a la escala de su columna, así que la escala vertical tiene que ser la misma.

## Cómo se usa (las mismas reglas en los dos aparatos)

**Leer.** Con `propia` = la hoja en las unidades de la capa de este aparato y `fichero` = el marco leído:

```
ex = fichero.ancho / propia.ancho      ey = fichero.alto / propia.alto
dx = fichero.x0 - propia.x0 * ex       dy = fichero.y0 - propia.y0 * ey
fichero = propia * e + d   ⇒   capa = (fichero - d) / e
```

Se aplica a `x`, `y`, `width` (÷ex), `height` (÷ey), los puntos, el `strokeWidth` y el `fontSize` (÷ `sqrt(ex·ey)`, la escala media). **Si las proporciones no coinciden se encaja eje por eje** (el marco manda: cada trazo queda en el mismo sitio relativo de la hoja) y se avisa en el registro si difieren más de un 1 %. Sin subir `version`: convertir al leer no es cambiar.

**Guardar.** Se escribe la tinta **en las unidades en que se leyó** y, al lado, su marco. Si el marco que ya hay dice lo mismo (con tolerancia de redondeo), **no se toca**: una ida y vuelta sin cambios deja los dos ficheros idénticos byte a byte y no provoca otro envío. Primero la tinta, luego el marco, cada uno por temporal + renombrar (`AnotacionesDelAdjunto.escribir`). Como se escribe en las unidades en que se leyó, un corte entre los dos no deja una tinta con un marco que no le corresponde.

**Sin `.hoja`** (tinta vieja): la regla de hoy, sin cambios.

## Cambios en Android (listos para pegar)

### 1. `sincro/AnotacionesDelAdjunto.kt`

```kotlin
    fun hoja(filesDir: File, base: String) = File(filesDir, "$CARPETA/$base.hoja")

    private val TERMINACIONES = listOf(".excalidraw.gz", ".marcas", ".espacios", ".maqueta", ".voz", ".sitio", ".hoja")

    /** **El marco de la tinta**: dónde está la hoja en las unidades en que están los puntos de su tinta. */
    data class Marco(val x0: Double, val y0: Double, val x1: Double, val y1: Double) {
        val ancho get() = x1 - x0
        val alto get() = y1 - y0
        fun valido() = listOf(x0, y0, x1, y1).all { it.isFinite() } && ancho > 1e-3 && alto > 1e-3
        fun aTexto() = String.format(java.util.Locale.ROOT, "%s,%s,%s,%s\nv1\n", n(x0), n(y0), n(x1), n(y1))

        /** Lleva un elemento de las unidades de [este] marco a las de [destino] (la misma hoja). */
        fun hacia(destino: Marco): (com.forge.pixpin.motor.Element) -> com.forge.pixpin.motor.Element {
            val ex = destino.ancho / ancho; val ey = destino.alto / alto
            val dx = destino.x0 - x0 * ex; val dy = destino.y0 - y0 * ey
            val k = kotlin.math.sqrt(kotlin.math.abs(ex * ey))
            return { e -> e.copy(
                x = e.x * ex + dx, y = e.y * ey + dy,
                width = e.width * ex, height = e.height * ey,
                // Los puntos del Element van relativos a (x, y): solo escala.
                points = e.points?.map { p -> com.forge.pixpin.motor.Pt(p.x * ex, p.y * ey) },
                strokeWidth = e.strokeWidth * k,
                fontSize = e.fontSize?.let { it * k }
            ) }
        }

        fun casiIgual(o: Marco) = listOf(x0 - o.x0, y0 - o.y0, x1 - o.x1, y1 - o.y1).all { kotlin.math.abs(it) < 0.01 }

        companion object {
            private fun n(v: Double): String {
                val t = String.format(java.util.Locale.ROOT, "%.3f", v).trimEnd('0').trimEnd('.')
                return if (t == "-0") "0" else t
            }
            fun deTexto(t: String?): Marco? {
                val lineas = t?.lines()?.map { it.trim() }?.filter { it.isNotEmpty() } ?: return null
                if (lineas.isEmpty() || lineas.size > 2) return null
                if (lineas.size == 2 && lineas[1] != "v1") return null
                val n = lineas[0].split(',').map { it.trim().toDoubleOrNull() ?: return null }
                if (n.size != 4) return null
                return Marco(n[0], n[1], n[2], n[3]).takeIf { it.valido() }
            }
        }
    }

    fun leerMarco(f: File): Marco? = Marco.deTexto(leer(f))

    /** Escribe [m] salvo que el que hay ya diga lo mismo (la ida y vuelta no reescribe). */
    fun escribirMarco(f: File, m: Marco) {
        if (leerMarco(f)?.casiIgual(m) == true) return
        escribir(f, m.aTexto())
    }
```

`todoDe`, `porUid`, `Disco.alcance` (`Disco.kt:503`), `Disco.aplicarMensajes` (`Disco.kt:317`), `MensajesStore.borrarAdjunto` (`MensajesStore.kt:368`) y `ChatQueViaja.preparar` usan `porUid`: con la terminación nueva el marco **viaja y se borra con su mensaje sin tocar nada más**. Comprobar en `ChatQueViaja` que `anot-<uid del proyecto>.hoja` no aparece (el PDF de un proyecto no lleva marco).

### 2. `pdf/LectorPdfActivity.kt` — el PDF suelto del chat

El marco **real** de la hoja en coordenadas de la capa, calculado desde `vistaDeLaCapa` **tal como está hoy** (no hace falta migrar nada: el marco dice justo las unidades raras de ahora). La capa mide `anchoPx·(1 + 0,75·lados)`, su vista tiene `zoom = anchoCapa / 3500` y `scrollX = 1050`, y la hoja ocupa en la capa de `izquierdaPx` a `izquierdaPx + anchoPx` y de 0 a `anchoPx / proporcion`. Con `escena = px / zoom − scrollX`, `anchoPx` se va:

```kotlin
/** Dónde cae la hoja en las unidades de su capa, con los [espacios] de ahora. Ver [vistaDeLaCapa]. */
internal fun marcoDeLaHoja(espacios: Int, proporcion: Float): com.forge.pixpin.sincro.AnotacionesDelAdjunto.Marco {
    val izq = if (espacios and ESPACIO_IZQUIERDA != 0) 1.0 else 0.0
    val der = if (espacios and ESPACIO_DERECHA != 0) 1.0 else 0.0
    val m = Lectura.MARGEN_DEL_PDF.toDouble()
    val w = PdfDoc.PAGE_WIDTH.toDouble()
    val k = (1.0 + 2.0 * m) / (1.0 + m * (izq + der))        // unidades de capa por unidad de hoja
    val x0 = w * m * izq * k - w * m
    val alto = w / proporcion.coerceAtLeast(0.01f) * k          // proporcion = ancho / alto (como en Hoja)
    return com.forge.pixpin.sincro.AnotacionesDelAdjunto.Marco(x0, 0.0, x0 + w * k, alto)
}
```

(Es la misma cuenta que `pixpin_docs::vista::capa_del_movil` del PC: sin espacios da `-1050,0,2450,1400/proporcion·2,5`; con los dos, `0,0,1400,1400/proporcion`.)

En `CapasDelPdf`:

- Un `espacios: () -> Int` en el constructor (o un campo que `ponerEspacios` actualice) y la proporción de cada hoja (`PdfDoc.medidaEnPuntos(ruta, i)`, la misma que usa `Hoja`).
- `fun marcoDe(i: Int): File? = if (proyecto() != null) null else uid?.let { AnotacionesDelAdjunto.hoja(actividad.filesDir, AnotacionesDelAdjunto.dePagina(it, i)) }`.
- **Leer** — en `controladorDe(i)`, justo tras `ExcalidrawStore.cargar(...)`:
  ```kotlin
  val ahora = marcoDeLaHoja(espacios(), proporcionDe(i))
  val delFichero = marcoDe(i)?.let { AnotacionesDelAdjunto.leerMarco(it) }
  val escena = if (cargada != null && delFichero != null && !delFichero.casiIgual(ahora))
      cargada.copy(elements = cargada.elements.map(delFichero.hacia(ahora))) else cargada
  ```
  Igual en `escenaDe(i)` (la que se exporta) cuando lee del disco.
- **Guardar** — en `guardarTodo()`, dentro de la corrutina, después de `ExcalidrawStore.guardar(contexto, ids[k], escena)` y solo si `marcoDe(i) != null`:
  `AnotacionesDelAdjunto.escribirMarco(marcoDe(i)!!, marcoDeLaHoja(espacios(), proporcionDe(i)))`. Lo mismo en `pasarAProyecto` **no**: en un proyecto no hay marco (y se recomienda que ahí la tinta pase a las unidades del editor, ver 4).
- **Poner o quitar un espacio** (`ponerEspacios`): `capas.guardarTodo()` **antes** de cambiar `espacios`, y después olvidar los controladores abiertos (`abiertas.clear()`), para que se vuelvan a leer y el marco los lleve a la capa nueva. Así la tinta **se queda pegada a la hoja** al cambiar los espacios (hoy se mueve: es el fallo de K20).

### 3. `ui/VisorHtmlActivity.kt` — Word y libro

- Marco actual: `Marco(espacioIzq.toDouble(), 0.0, (espacioIzq + columna).toDouble(), columna.toDouble())` con `columna = columnaDeAnotar` (sin columna fijada no hay marco).
- Fichero: `baseDelMensaje?.let { AnotacionesDelAdjunto.hoja(filesDir, it) }` (con la base `-texto` sale solo `anot-<uid>-texto.hoja`).
- **Leer**: tras leer la maqueta en `onCreate` (líneas ~1027-1034, donde se ponen `columnaDeAnotar` y `espacioIzq`), si hay marco en el fichero y no `casiIgual` al actual, `laCapa.load(laCapa.scene.copy(elements = laCapa.scene.elements.map(delFichero.hacia(actual))))`.
- **Guardar**: en `guardarLaCapa()` y `guardarLaCapaYa()`, después de `ExcalidrawStore.guardar`, `escribirMarco(fichero, actual)`. `anadirEspacio` ya corre la tinta con la columna y la guarda: con eso el marco sale con el `espacioIzq` nuevo.

### 4. (Recomendado, aparte) el PDF de un proyecto en el lector del móvil

Su hoja es el dibujo del editor (hoja a 1400 desde el cero), pero el lector la pinta y la escribe con `vistaDeLaCapa`, que solo coincide con los dos espacios: con otros, lo anotado en el lector cae en otro sitio en el editor. Con las mismas funciones: al leer, `Marco(0,0,1400,1400/proporcion).hacia(marcoDeLaHoja(espacios, proporcion))`; al guardar, la vuelta. Sin fichero `.hoja`: el marco es implícito.

### 5. Pruebas mínimas (`app/src/test/.../sincro/MarcoDeLaHojaTest.kt`)

1. `deTexto(aTexto(m)) == m`; `aTexto(Marco(-1050.0,0.0,2450.0,4950.0)) == "-1050,0,2450,4950\nv1\n"`; `deTexto("1400.0,0.0,2800.0,1980.0")` vale.
2. Mal formados → `null`: `""`, `"1,2,3"`, `"1,2,3,4,5"`, `"0,0,x,10"`, `"0,0,1400,1980\nv2"`, `"10,0,10,1980"`, `"0,0,NaN,1980"`, `"0;0;1400;1980"`.
3. `marcoDeLaHoja(0, 1400f/1980f) ≈ (-1050, 0, 2450, 4950)`; `marcoDeLaHoja(3, …) ≈ (0, 0, 1400, 1980)`; con uno solo, k = 2,5/1,75.
4. Un trazo escrito con marco `500,300,4700,6240` (×3, origen 500,300) con puntos en las esquinas de la hoja, llevado a `Marco(0,0,1400,1980)`, queda en `(0,0)`–`(1400,1980)`, con `strokeWidth` ÷3.
5. `porUid` cuenta `anot-<uid>-p0.hoja` y `anot-<uid>.hoja`, no `anot-<uid>-p0.hoja.tmp`; `todoDe(uid)` los borra.
6. Ida y vuelta: `escribirMarco` con un marco `casiIgual` al que hay no cambia la fecha del fichero.

Los números de 3, 4 y 5 son los mismos que usan las pruebas del PC (`crates/pixpin-sincro/src/anotado.rs`, `apps/pixpin/src/anotado_del_adjunto.rs::pruebas_del_marco`): si salen iguales en los dos, la tinta cae igual.

## Regla vieja frente a marco

| Caso | Regla vieja (`.espacios` / `.maqueta`) | Marco (`.hoja`) |
|---|---|---|
| Hoy, PDF del chat | Cubre (K20): la cuenta del PC imita el fallo de `vistaDeLaCapa` | Cubre, y dice lo mismo (el PC lo escribe con esas unidades) |
| Cambio de espacios en el móvil | La tinta se mueve de sitio en la hoja (es el fallo, y el PC lo imita) | Queda pegada a la hoja |
| Cambio futuro de unidades en cualquiera de las dos apps (arreglar `vistaDeLaCapa`, otro ancho de hoja, otra escala de capa) | Rompe: el otro sigue con la cuenta vieja; exige migración y aviso coordinados | Sigue funcionando: cada fichero dice sus unidades; se puede cambiar un lado sin tocar el otro |
| PDF de un proyecto | No aplica (k = 1, marco implícito); el lector del móvil con espacios ≠ 3 desencaja con su editor | Sin fichero; mismo marco implícito, que además sirve para arreglar el lector del móvil (punto 4) |
| Word/EPUB con otra maqueta (otro `izq`, otra escala de capa) | Solo corre `izq`; otra escala no | Encaja origen y escala de la columna. Una columna de otro ancho **reflowea** el texto: ninguna de las dos lo arregla (por eso sigue la maqueta fijada) |
| Tinta vieja sin `.hoja` | Es la única que la lee | Cae a la regla vieja; el PC escribe el marco la primera vez que guarda un cambio |
| Marco mal formado o de otra versión | — | Se ignora (registro), regla vieja |
| Conflicto: marco de un aparato con tinta del otro | Posible si `.espacios` y tinta llegan en vueltas distintas (el `.espacios` de ahora con una tinta escrita con otros) | Tinta y marco se escriben juntos y quien reescribe lo hace en las unidades en que leyó, así que el marco que hay siempre corresponde; si llegan en vueltas distintas, el par que sale de cada aparato es coherente por sí solo |
| Proporciones distintas entre aparatos | No se detecta | Se encaja eje por eje y se avisa si pasa del 1 % |
| Coste | Nada | Un fichero de ~30 bytes por hoja anotada |

**Prioridad cuando estén las dos:** 1) marco válido de `v1` → manda; 2) si no hay marco o no se entiende → la regla vieja (`capa_del_movil(espacios)` en un PDF del chat, `izq` de la maqueta en un Word); 3) PDF de proyecto y lo de junto al PDF → unidades fijas (sin marco). Quien guarda **siempre** escribe el marco, también cuando leyó con la regla vieja.

**Cuándo retirar la regla vieja:** cuando (a) las dos apps escriban el marco (PC ya, Android con esta nota) y (b) ya no queden tintas sin marco: basta con una migración de una vez en cada aparato que, al abrir un documento, escriba el marco de las tintas que no lo tengan (con la regla vieja, sin tocar la tinta). Hasta entonces se queda como respaldo de lectura; nunca debe volver a usarse para **escribir** otras unidades que las que dice el marco. Después de retirarla, Android puede arreglar `vistaDeLaCapa` (unidades = 1400·(1+0,75·lados), `scrollX` = 1050·izq) sin migrar dibujos ni avisar al PC: el marco nuevo dirá las unidades nuevas.
