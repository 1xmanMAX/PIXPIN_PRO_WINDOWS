# Inventario del módulo `motor` (PixPin Android)

Fecha: 2026-09-06.
Fuente: `proyectos de referencia/PIXPIN_PRO_ANDROID`, rama de trabajo tal cual estaba hoy.
Nada de ese directorio se ha modificado.

Todas las cifras de este informe están medidas sobre los archivos, no estimadas.
El módulo son **103 archivos `.kt` y 55.385 líneas**, todos en una sola carpeta plana:
`app/src/main/java/com/forge/pixpin/motor/`.

---

## 1. Qué hace el módulo

`motor` es el editor de dibujo de PixPin, entero. Contiene el modelo de lo que se
dibuja (un `Element`, una `Scene`), la geometría que hace falta para dibujarlo
(cajas, rotaciones, intersecciones, contornos, colisiones), la máquina de estados
que decide qué pasa entre que se toca la pantalla y se levanta el dedo, el
renderizador que lo pinta, y las cuatro o cinco maneras de sacar el dibujo de la
aplicación (imagen, SVG, PDF, página web autónoma, archivo `.excalidraw`, paquete
`.pixpin`). Es una reimplementación en Kotlin del núcleo de Excalidraw —modelo,
`rough.js`, `perfect-freehand`, historial por deltas— más un buen montón de cosas
que Excalidraw no tiene y que vienen de dos aplicaciones anteriores del mismo
autor: una de anotar capturas de pantalla (mosaico, foco, números de serie, lupa)
y otra de croquis acotados (cota, escala, escala gráfica, ángulos, tablas de
coordenadas).

El mismo motor se usa en cuatro superficies distintas de la aplicación: la
captura de pantalla, el pin flotante, la capa de anotación encima de todo, y el
editor a pantalla completa. Por eso está separado del resto: hay una prueba
(`MotorSeparadoTest`) que falla si un archivo del motor importa algo de
`pixpin.pin`, `pixpin.capture`, `pixpin.croquis` o `pixpin.annotate`. Con los
años ha ido absorbiendo cosas que ya no son «dibujar»: leer y escribir PDF a
bajo nivel (sin librerías, unas 7.000 líneas), un pequeño motor 3D isométrico
para bocetos en volumen (unas 3.300 líneas), instrumentos matemáticos (plano
cartesiano, recta numérica, gráficas de funciones, ecuaciones tipografiadas), y
dos visores JavaScript que se escriben como texto para meterlos dentro del HTML
exportado. Es la mitad del proyecto y hace bastante más que su nombre sugiere.

---

## 2. La frontera entre lo puro y lo que toca Android

**Esta es la parte más importante del informe y la he comprobado archivo por
archivo.**

### 2.1 Lo que dice la documentación

`docs/motor.md`, en su sección «Dentro», afirma:

> El núcleo no importa `android.*`. Se prueba en la JVM, sin emulador.
> `936 pruebas · todas en verde`

### 2.2 Lo que dice el código

| | archivos | líneas | % líneas |
|---|--:|--:|--:|
| Sin ningún `import android.*` ni `androidx.*` | **68** | **31.718** | 57 % |
| Con `import android.*` | 25 | — | — |
| Solo con `import androidx.*` (Compose, interfaz) | 10 | — | — |
| **Total con Android en cualquier forma** | **35** | **23.667** | **43 %** |

O sea: la frase de la documentación **es cierta pero engañosa si se lee como
«el módulo `motor` no toca Android»**. Toca Android en 35 de sus 103 archivos y
en el 43 % de sus líneas. Lo que es cierto es que existe un *núcleo* bien
definido, que ese núcleo es la mayoría en número de archivos, y que la frontera
**está vigilada por una prueba automática**, que es lo que de verdad importa
para portar.

### 2.3 La prueba que vigila la frontera

`app/src/test/java/com/forge/pixpin/MotorSeparadoTest.kt` (156 líneas) hace tres
cosas:

1. Comprueba que ningún archivo del motor importa del resto de la aplicación
   (con una excepción declarada: `ImageStore`).
2. Mantiene una **lista blanca de 36 nombres de archivo** que sí pueden tocar
   Android. Cualquier otro archivo que escriba `import android.` o
   `import androidx.` hace fallar la prueba.
3. Comprueba que esa lista no tenga fantasmas (archivos que ya no existen).

La lista blanca está comentada archivo por archivo explicando por qué cada uno
está ahí. Es documentación de portabilidad hecha por el propio autor y hay que
aprovecharla.

### 2.4 Discrepancia encontrada

La lista blanca tiene **36 nombres**; los archivos que de verdad importan algo de
Android son **35**. El que sobra es **`Theme.kt` (207 líneas)**: está declarado
como «capa de Android» pero ya no importa nada. En su interior hay dos
comentarios que explican por qué:

- línea 74: `// **Sin android.graphics.Color**, a propósito: son cuatro bytes…`
- línea 176: `// **Se lee el hexadecimal a mano y no con android.graphics.Color**…`

Es decir, `Theme.kt` se limpió en algún momento y nadie sacó su nombre de la
lista. La prueba no lo detecta porque solo comprueba fantasmas (nombres que no
existen), no sobrantes (nombres limpios). **Para el port: `Theme.kt` es núcleo
puro, aunque el proyecto lo clasifique como capa de Android.**

### 2.5 Para qué se usa Android en cada archivo que lo usa

Los 25 archivos con `import android.*` usan, en total, estas familias:

| API de Android | usos | para qué |
|---|--:|---|
| `android.graphics.*` (`Bitmap`, `Canvas`, `Paint`, `Path`, `Matrix`, `Rect(F)`, `Color`, `Shader`, `PathMeasure`, `DashPathEffect`, `Typeface`) | ~40 | pintar en pantalla, rasterizar, medir texto, sacar el perfil de las letras |
| `android.content.Context` | 12 | saber dónde escribir archivos (caché, `filesDir`, `assets`) |
| `android.graphics.pdf.PdfRenderer` / `PdfDocument` | 3 | leer y generar PDF con la API del sistema |
| `android.media.MediaCodec` / `MediaMuxer` / `MediaFormat` | 4 | recodificar audio a AAC (`AudioLigero.kt`) |
| `android.os.*` (`Build`, `Bundle`, `SystemClock`, `ParcelFileDescriptor`) | 5 | ciclo de vida, versión del sistema, descriptores de archivo |
| `android.net.Uri`, `android.content.Intent`, `android.widget.Toast` | 3 | solo en `DrawEditorActivity.kt` |
| `android.util.LruCache`, `android.util.Base64` | 2 | caché de miniaturas, base64 |

Los 10 archivos que solo importan `androidx.*` son **interfaz de Jetpack Compose
al 100 %** (`DrawToolbar`, `PanelLateral`, `MandosDelPanel`, `VentanaDeAjustes`,
`PaletaDeColores`, `DrawTablas`, `DrawTablaPegada`, `DrawFiguras`, `Mando`,
`BarraDeVentana`). No hay geometría dentro: el propio autor la fue sacando a
archivos puros (`Deslizadores.kt`, `MarcasDelDeslizador.kt`, `Barra.kt`,
`Biblioteca.kt`, `TablaDibujada.kt`) precisamente para poder probarla.

### 2.6 Qué usa el núcleo puro además de Kotlin

De los 68 archivos puros, solo 15 importan algo fuera de `kotlin.*`:

| import | archivos | equivalente en Rust |
|---|---|---|
| `kotlinx.serialization` | 7 (`Element`, `Scene`, `Medida`, `Nudos`, `Tablas`, `Biblioteca`, `Proyeccion`, `Proyectos`) | `serde` / `serde_json` |
| `java.util.zip.Deflater` / `Inflater` | 3 (`PdfEscritura`, `PdfLectura`, `PlanoWeb`) | `flate2` |
| `java.util.Locale` | 4 (`Svg`, `Medida`, `Graficas`, `PdfEscritura`) | formateo de números, trivial |
| `java.io.File` / `ByteArrayOutputStream` / `ArrayDeque` | 3 | `std` |

**Conclusión de esta sección:** las 31.718 líneas puras dependen de `serde`,
`flate2` y aritmética. No hay nada ahí que ate al port a una plataforma.

### 2.7 Las pruebas

Hay **151 archivos de prueba** en `app/src/test/java`, de los cuales 146 están
directamente en el paquete raíz. Ocho de ellos mencionan `android` (siete son
pruebas de la aplicación, no del motor). No he ejecutado la suite —no tengo
Gradle ni SDK aquí— así que **no puedo confirmar el «936 pruebas, todas en
verde»** de la documentación; solo que existe un cuerpo de pruebas grande y que
está escrito para correr en la JVM (JUnit normal, no instrumentado).

---

## 3. Los tipos de datos centrales

### 3.1 `Pt` — el punto

`data class Pt(val x: Double, val y: Double)`. En coordenadas de **escena**, no
de pantalla ni de bitmap. `Double` y no `Float` a propósito: con coordenadas
grandes un `Float` pierde precisión visible y las formas «bailan» al hacer zoom.

### 3.2 `Element` — el elemento (`Element.kt`, 1.127 líneas)

Es una **clase plana con discriminante**, no una jerarquía sellada. La decisión
está documentada: el JSON de `.excalidraw` es exactamente esa forma —un objeto
con `type` y todos los campos al mismo nivel—, así que una clase plana serializa
sin capa de traducción. Los campos específicos de un tipo son nulables.

Campos, por bloques:

| bloque | campos |
|---|---|
| Identidad | `id`, `type` |
| Geometría | `x`, `y`, `width`, `height`, `angle` (radianes, alrededor del centro) |
| Estilo | `strokeColor`, `backgroundColor`, `fillStyle`, `strokeWidth`, `strokeStyle`, `roughness`, `opacity`, `roundness`, `material` |
| **`seed: Int`** | La semilla del ruido del trazo. El propio archivo la llama «el campo más importante del modelo»: sin ella, cada redibujado sortea otro garabato y la forma tiembla al mover el dedo o al hacer zoom |
| Orden e historial | `version`, `versionNonce`, `isDeleted`, `groupIds`, `boundElements`, `updated`, `link`, `locked`, `reference` |
| Puntos (líneas, flechas, lápiz) | `points`, `pressures`, `simulatePressure`, `presionFirme`, `lastCommittedPoint`, `huecos` |
| Flechas | `startArrowhead`, `endArrowhead`, `startBinding`, `endBinding`, `elbowed` |
| Texto | `text`, `fontSize`, `fontFamily`, `textAlign`, `verticalAlign`, `containerId`, `negrita`, `cursiva`, `tachado` |
| Marco / hoja | `name`, `papel`, `pauta` |
| Arco | `arcStart`, `arcSweep`, `etiquetaAngulo`, `etiquetaRadio` |
| Instrumentos (plano, recta, espacio) | `unidad`, `pasoDeNumeros`, `pasoDeCuadros`, `azimut`, `elevacion` |
| Sólido 3D | `altura`, `cota`, `giroEnPlanta`, `formaSolida`, `planta`, `inclinacion`, `esqueleto`, `enElSuelo` |
| Cronograma | `tareas`, `periodos` |
| Mosaico / lupa / foco | `mosaicBlur`, `foco`, `aumento`, `focoAncho`, `focoAlto`, `oscurecer`, `lupaRedonda`, `lupaFlecha`, `guia`, `lupaDosLineas` |
| Región | `forma` |
| Imagen | `fileId`, `scale`, `crop` |

**`ElementType` tiene 23 valores.** Los ocho primeros son de Excalidraw; los otros
quince llevan prefijo `pixpin-` en su nombre serializado para no chocar nunca con
tipos que el original añada:

`rectangle`, `diamond`, `ellipse`, `arrow`, `line`, `freedraw`, `text`, `image`,
`frame`, `pixpin-mosaic`, `pixpin-spotlight` (obsoleto: ya no se puede crear, se
conserva para poder abrir dibujos viejos), `pixpin-lupa`, `pixpin-serial`,
`pixpin-measure`, `pixpin-arc`, `pixpin-region`, `pixpin-scalebar`,
`pixpin-point`, `pixpin-axes`, `pixpin-number-line`, `pixpin-space`,
`pixpin-solid`, `pixpin-gantt`.

Enumeraciones auxiliares en el mismo archivo: `TamanoDePapel`, `PautaDeHoja`,
`FormaDeSolido` (6 formas), `FillStyle`, `StrokeStyle`, `MaterialDeTinta`,
`Arrowhead` (8), `TextAlign`, `VerticalAlign`, `BindMode`, `Roundness`,
`BoundElement`, `Binding`, `Crop`, `TareaDelCronograma`.

### 3.3 `Scene` — la escena (`Scene.kt`, 1.150 líneas)

```
Scene(
  elements: List<Element>,            // el orden de la lista ES el orden de pintado
  files: Map<String, SceneFile>,      // imágenes referenciadas por fileId
  viewport: Viewport,                 // scrollX, scrollY, zoom
  style: ItemStyle,                   // estilo de lo PRÓXIMO que se dibuje
  backgroundColor: String,
  luces: LucesDelDibujo,
  escala: Escala?,                    // qué mide un píxel: da unidades a todas las cotas
  tablas: List<TablaDeCoordenadas>,
  origenCoordenadas: Pt?,
  referenciasVisibles: Boolean,
  alfileres: List<Alfiler>,
  vista: Vista                        // el cuarto de giro de la proyección isométrica
)
```

Puntos a retener para el port:

- **No hay campo `z`.** El orden de pintado es el orden de la lista. Todas las
  operaciones de `Organize.kt` son, en el fondo, mover trozos de lista.
- `ItemStyle` es **estado del editor, no del dibujo**: es el estilo que se
  aplicará a lo siguiente. Va aparte a propósito.
- `Viewport` tiene `toScene()` / `toScreen()` y un `clampViewportToBounds()`.
- La escena lleva dentro cosas que no son elementos: la escala métrica, las
  tablas de coordenadas, los alfileres y el punto de vista 3D.

### 3.4 Cómo se guarda un dibujo

Tres formatos, todos en el motor:

| formato | archivo | qué es |
|---|---|---|
| `.excalidraw` | `ExcalidrawStore.kt` (244 l., toca Android por el `Context`) | JSON del formato original, **comprimido**; un archivo por pin, fuera del estado de la aplicación, para que arrancar no cueste leer cientos de elementos |
| Biblioteca de figuras | `BibliotecaStore.kt` (77 l.) | archivo aparte, no dentro de cada dibujo, para que una figura guardada esté puesta al abrir el siguiente |
| `.pixpin` | `PaquetePixpin.kt` (216 l.) | un **ZIP** con el proyecto entero: lienzos, croquis, notas, fotos y PDF. Escrito explícitamente para poder pasar un proyecto «a otro aparato o a la versión de escritorio» |

El JSON lo produce `ExcalidrawJson` (una instancia de `kotlinx.serialization.Json`
configurada en `Scene.kt`). La serialización es directa desde `Element`, sin
traducción. En Rust esto es `serde` con `#[serde(rename = "...")]`.

Hay además una descripción del formato en `docs/formato-pixpin.md` (54 líneas),
que no he auditado a fondo.

### 3.5 Deshacer y rehacer (`History.kt`, 133 líneas)

Es de lo más limpio del módulo y **se porta tal cual**. No guarda copias de la
escena sino diferencias:

```
ElementsDelta(
  changes: Map<String, Pair<Element?, Element?>>,  // id -> (antes, después)
  orderBefore: List<String>?,                      // null si el orden no cambió
  orderAfter: List<String>?
)
```

- `before == null` → elemento creado; `after == null` → borrado; los dos → modificado.
- `inverted()` intercambia los pares: **la misma estructura sirve para deshacer y
  para rehacer**.
- `calculateDelta(before, after)` compara **instancias completas**, no el campo
  `version` (un elemento puede volver a un estado anterior con otra versión).
- `applyDelta` respeta el orden guardado y añade al final lo que no estuviera en
  él, para que un elemento creado después no desaparezca al deshacer otra cosa.
- `History(limit = 100)` son dos `ArrayDeque`. Anotar algo nuevo vacía la pila de
  rehacer.

En Rust: dos `VecDeque<ElementsDelta>` y un `HashMap<String, (Option<Element>, Option<Element>)>`.
Cero fricción.

---

## 4. Inventario completo

Aviso previo: **la agrupación de `docs/motor.md` (dibujar / tapar y señalar /
medir / construir) es un póster de láminas, no un inventario.** Cubre las
herramientas visibles de la barra y deja fuera más de la mitad del módulo: todo
el PDF, todo el 3D, todos los instrumentos matemáticos, todas las salidas, el
proyecto, el cuaderno, las tablas y el cronograma. He mantenido sus cuatro
grupos y añadido los que faltaban.

Otro desajuste medido: **`Tool` tiene hoy 32 valores**, pero los comentarios del
propio código dicen «el motor tiene diecinueve herramientas» (`Barra.kt`) y «las
quince herramientas» (`DrawToolbar.kt`). Los dos comentarios están anticuados.

En las tablas, «And.» = número de `import android.*` (0 = núcleo puro; «cx» =
solo Compose/`androidx`).

### 4.1 Núcleo del modelo y la escena

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| Elemento | El modelo plano con 23 tipos y ~70 campos | `Element.kt` | 1.127 | 0 |
| Escena | Estado del lienzo, `Tool` (32), `ItemStyle`, `Viewport`, constructores de elementos, edición de puntos, aplicación de estilos | `Scene.kt` | 1.150 | 0 |
| Historial | Deltas, deshacer, rehacer | `History.kt` | 133 | 0 |
| Cajas y transformaciones | `Bounds`, coordenadas absolutas, rotación de puntos, distancia a segmento, radio de esquina | `Bounds.kt` | 372 | 0 |
| Mover/escalar/rotar/voltear | Port de `resizeElements.ts`. Lo delicado: el ancla no se puede mover con el elemento girado | `Transform.kt` | 655 | 0 |
| Tiradores | 8 de redimensionar + 1 de rotación, en coordenadas de escena con tamaño de pantalla | `TransformHandles.kt` | 444 | 0 |
| Picado (hit test) | Qué elemento hay bajo el dedo. Descarte por caja rotada y luego test exacto | `Collision.kt` | 423 | 0 |
| Organizar | Orden de pintado, grupos, alinear, distribuir | `Organize.kt` | 310 | 0 |
| Máquina de estados del dedo | Todo lo que pasa entre tocar y levantar. **No importa Android** | `DrawController.kt` | **3.482** | 0 |
| Qué se puede ajustar | Qué propiedades tienen sentido para cada herramienta y cada tipo | `DrawProperties.kt` | 332 | 0 |
| Aleatorio reproducible | Lehmer (Park–Miller) portado bit a bit para que la misma `seed` dé el mismo garabato que en excalidraw.com | `Rand.kt` | 58 | 0 |

### 4.2 Dibujar

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| Trazo «a mano alzada» | Port de `rough.js`: cada línea se dibuja dos veces con ruido. El orden de llamadas al generador es parte del algoritmo | `Rough.kt` | 571 | 0 |
| Geometría de las figuras | De qué se compone cada figura (rect, rombo, elipse, redondeados) | `Shapes.kt` | 418 | 0 |
| Lápiz | Port de `perfect-freehand`: devuelve el **contorno cerrado de la mancha**, no un camino | `Freehand.kt` | 563 | 0 |
| Pulso del trazo | Cuánto engorda por presión y cuánto adelgaza por velocidad, en unidades de mundo | `PulsoDelMundo.kt` | 274 | 0 |
| Alisado del puntero | Filtro 1€ de Casiez (CHI 2012), calcado del original | `AlisadoDeUnEuro.kt` | 219 | 0 |
| Espina del trazo | Catmull-Rom **centrípeta** muestreada | `EspinaDelTrazo.kt` | 425 | 0 |
| Caminos | Reduce las tres formas de generar geometría a una sola representación (`Op`) | `Caminos.kt` | 98 | 0 |
| Flechas | Puntas (8 tipos) y anclaje a formas: una flecha anclada recalcula sus extremos al mover la forma | `Arrows.kt` | 490 | 0 |
| Flechas de codos | Trazado ortogonal a 90° con esquinas redondeadas | `Elbow.kt` | 131 | 0 |
| Arco | Trozo de circunferencia; se guarda como caja del óvalo + inicio + barrido | `Arco.kt` | 172 | 0 |
| Texto dentro de figuras | El rectángulo con su palabra; se mueven juntos | `TextoEnFiguras.kt` | 211 | 0 |
| Partir en renglones | Qué cabe en un ancho dado | `Renglones.kt` | 107 | 0 |
| Modo día / noche | Filtro al pintar, **no** un cambio en los datos | `Theme.kt` | 207 | 0 (pero listado como Android) |
| Renderizador | Port de `renderElement.ts` + `staticScene.ts`. Primero el relleno, después el trazo | `Renderer.kt` | **3.824** | 11 |
| Fuentes | Excalifont, Nunito, Comic Shanns | `DrawFonts.kt` | 116 | 3 |
| Perfil de las letras | Convierte una palabra en su silueta, para las salidas vectoriales | `Glifos.kt` | 83 | 3 |

### 4.3 Tapar y señalar

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| Mosaico | Tipo `pixpin-mosaic`: pixela o desenfoca lo de debajo | (en `Element.kt` + `Renderer.kt`) | — | — |
| Foco | Tipo `pixpin-spotlight`: oscurece todo menos su caja. **Obsoleto**: ya no se puede crear | (en `Element.kt` + `Renderer.kt`) | — | — |
| Lupa | Dos cajas: el cristal (dónde se ve) y el foco (a dónde mira, en absoluto). Con guía y flecha | `Lupa.kt` | 566 | 0 |
| Números de serie | Tipo `pixpin-serial`: círculo con 1, 2, 3… | (en `DrawController.kt`) | — | — |
| Borrador | Herramienta `ERASER` | (en `DrawController.kt`) | — | — |
| Bote de pintura | Rellena el hueco cerrado entre varias figuras. Rejilla + Douglas-Peucker. Guarda el contorno encontrado, no una referencia a las figuras | `Regiones.kt` | 717 | 0 |
| Perímetros | El contorno de **cualquier** figura reducido a tramos rectos; base de las intersecciones y del bote | `Perimetros.kt` | 520 | 0 |

### 4.4 Medir

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| Cota y escala | `Escala`, longitud, rótulo que **se calcula al pintar** y por eso nunca puede mentir | `Medida.kt` | 264 | 0 |
| Escala gráfica | La reglita a cuadros; se reparte sola en cuadros redondos | `EscalaGrafica.kt` | 129 | 0 |
| Ángulos internos | Solo mientras se mueve algo, como el nivel de burbuja | `Angulos.kt` | 145 | 0 |
| Tablas de coordenadas | Puntos metidos tecleando, no a pulso; leer una tabla pegada del portapapeles | `Tablas.kt` | 275 | 0 |
| Papel | Formatos A4/A3… en puntos de PDF, para ver el lienzo a escala de hoja | `Papel.kt` | 31 | 0 |

### 4.5 Construir (ayudas de precisión)

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| Imán | **Un solo sitio** que decide a qué se pega el dedo. Prioridad: vértice → medio → cruce → centro → borde | `Iman.kt` | 167 | 0 |
| Enganche | Los anclajes de cada figura y la búsqueda del más cercano | `Snapping.kt` | 350 | 0 |
| Cuadrícula | Papel pautado de fondo, con paso adaptado al zoom | `Cuadricula.kt` | 88 | 0 |
| Recortar y extender | Quita el tramo entre dos cruces; alarga hasta lo primero que topa | `Recorte.kt` | 475 | 0 |
| Alfileres (nudos) | Un clavo que atraviesa dos figuras: 0 clavos = libre, 1 = gira, 2+ = fija | `Nudos.kt` | 548 | 0 |
| Puntos etiquetados | A, B, C sobre el dibujo, con la letra colocada en el hueco libre | `Puntos.kt` | 313 | 0 |

### 4.6 Instrumentos matemáticos (no aparecen en `docs/motor.md`)

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| Plano cartesiano y recta numérica | Instrumentos, no dibujos: saben cuánto vale una unidad y responden dónde cae el (3, −2) | `Plano.kt` | 332 | 0 |
| Espacio de tres ejes | Tres rectas numéricas proyectadas, con punto de vista que se gira | `Espacio.kt` | 275 | 0 |
| Gráficas de funciones | Se teclea `sin(x)/x` y sale dibujada como elementos normales, agrupados. Incluye un intérprete de fórmulas (`object Formula`) | `Graficas.kt` | 655 | 0 |
| Ecuaciones tipografiadas | Raíces con su raya, fracciones, exponentes, llaves. Todo textos y líneas del lienzo | `Ecuacion.kt` | 360 | 0 |
| Biblioteca de figuras | Figuras guardadas que se estampan de un toque, y las de fábrica | `Biblioteca.kt` | 222 | 0 |
| Tabla dibujada | Una tabla de Excel pegada, convertida en rayas y textos del lienzo | `TablaDibujada.kt` | 208 | 0 |
| Cronograma | Un plan dibujado: columna de nombres, escala arriba, barra por fila | `Cronograma.kt` | 217 | 0 |

### 4.7 Boceto 3D (no aparece en `docs/motor.md`)

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| Sólidos | La pieza 3D: caja, cuña, cilindro, prisma, revolución, extrusión. Caras, sombras, aristas | `Solido.kt` | **2.234** | 0 |
| Proyección | De tres números a la pantalla plana. Isométrica con 4 cuartos de vista, y desproyectar | `Proyeccion.kt` | 292 | 0 |
| Marcos de rotación mínima | Ejes perpendiculares a lo largo de una curva 3D (para cintas y tubos) | `MarcosMinimos.kt` | 266 | 0 |

### 4.8 PDF (unas 7.000 líneas; no aparece en `docs/motor.md` salvo una lámina)

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| Analizador de sintaxis PDF | Lee números, nombres, cadenas, diccionarios, flujos. Sobre `ByteArray`, no texto | `PdfLector.kt` | 298 | 0 |
| Lectura de PDF | Tabla de referencias cruzadas (clásica y comprimida), diccionarios de página | `PdfLectura.kt` | 670 | 0 |
| Escritura de PDF | **Actualización incremental**: añadir al final sin tocar un byte de lo anterior | `PdfEscritura.kt` | 354 | 0 |
| Anotar una página ajena | La cirugía de la página: colgarle una capa más | `PdfAnotado.kt` | 506 | 0 |
| El dibujo como órdenes de PDF | Traduce los `Op` del motor a `m`, `l`, `c`, `h` | `PdfLienzo.kt` | 1.207 | 3 |
| Nota como texto de verdad | Escribe letras seleccionables y buscables, no curvas | `PdfDeNota.kt` | 406 | 0 |
| Unir PDFs | Copia las páginas de uno detrás de las de otro, sin rasterizar | `PdfUnion.kt` | 127 | 0 |
| Leer el plano como geometría | Interpreta el flujo de contenido: un plano de AutoCAD son líneas, no una imagen | `PlanoDePdf.kt` | **1.525** | 0 |
| Empaquetar el plano para la web | Empalma tramos y comprime dos millones de puntos hasta que caben | `PlanoWeb.kt` | 372 | 0 |
| Cuentas del mosaico | Qué cuadro toca rasterizar y a qué tamaño | `MosaicoDePdf.kt` | 379 | 0 |
| Mosaico en memoria | Rasteriza el plano por cuadros | `ElMosaicoDelPapel.kt` | 407 | 3 |
| Cuadros en disco | Guarda los cuadros ya rasterizados como archivos | `CuadrosEnDisco.kt` | 137 | 0 |
| Lámina de cerca | Vuelve al PDF a por lo que se está mirando, a resolución de pantalla | `LaminaDeCerca.kt` | 139 | 1 |
| Plano en pantalla | Pinta el plano leído con `Canvas`/`Paint`/`Matrix` | `PlanoEnPantalla.kt` | 763 | 7 |
| Documento con `PdfRenderer` | Lectura con la API del sistema | `PdfDoc.kt` | 344 | 4 |
| Miniaturas | Caché con `LruCache` | `PdfMiniaturas.kt` | 327 | 4 |
| Dibujo como PDF | Genera un PDF vectorial con el mismo renderizador de pantalla | `DrawPdf.kt` | 241 | 3 |
| PDF del proyecto | Rehace el PDF desde la copia limpia (no va añadiendo capas) | `PdfDelProyecto.kt` | 113 | 2 |

### 4.9 Salidas y almacenamiento

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| SVG (sintaxis) | De los `Op` a la cadena de texto. Texto puro, se comprueba comparando cadenas | `Svg.kt` | 222 | 0 |
| SVG (qué se dibuja) | La decisión de qué sale y en qué orden | `DrawSvg.kt` | **1.459** | 6 |
| Documento web | Un `.html` autónomo con todas las páginas y su visor dentro | `ExportarHtml.kt` | **1.349** | 0 |
| Visor del plano (JS) | JavaScript, escrito como texto: pinta el plano en un `<canvas>` bajo el SVG. Incluye un inflador zlib propio | `VisorPlano.kt` | 683 | 0 |
| Visor del espacio (JS) | JavaScript: el croquis 3D navegable dentro del HTML | `VisorEspacio.kt` | 1.063 | 0 |
| Rasterizado | Copiar al portapapeles y guardar en galería, con el renderizador de pantalla | `DrawExport.kt` | 138 | 2 |
| Proyecto → PDF de lo marcado | Solo las láminas elegidas | `ExportarProyecto.kt` | 160 | 2 |
| Proyecto → web | Todas las hojas del proyecto en un `.html` | `ExportarProyectoWeb.kt` | 254 | 2 |
| Audio ligero | Recodifica notas de voz a AAC mono 16 kHz para la web | `AudioLigero.kt` | 110 | 4 |
| `.excalidraw` | JSON comprimido, un archivo por pin | `ExcalidrawStore.kt` | 244 | 3 |
| `.pixpin` | ZIP con el proyecto entero | `PaquetePixpin.kt` | 216 | 1 |
| Biblioteca en disco | Archivo aparte para las figuras guardadas | `BibliotecaStore.kt` | 77 | 3 |
| Proyecto | Varias hojas que se entregan juntas | `Proyectos.kt` | 483 | 0 |
| Hojas del proyecto | Despliega cada lienzo en sus marcos | `HojasDelProyecto.kt` | 200 | 0 |
| Cuaderno | El lienzo infinito puesto en hojas (una hoja **es** un marco) | `Cuaderno.kt` | 138 | 0 |
| Qué hojas llevan algo | Criterio de «anotada de verdad» | `CapaDeAnotacion.kt` | 56 | 1 |
| Peso del proyecto | Tamaño del archivo y del proyecto | `Detalle.kt` | 98 | 1 |

### 4.10 Interfaz (Compose) y cuentas extraídas de ella

| Mecanismo | Qué hace | Archivo | Líneas | And. |
|---|---|---|--:|:--:|
| Editor a pantalla completa | La `Activity` entera | `DrawEditorActivity.kt` | **3.712** | 131 |
| Lienzo | Reparto de dedos, gestos, adornos de selección | `DrawCanvas.kt` | 1.742 | 47 |
| Barra de herramientas | | `DrawToolbar.kt` | 1.233 | 85 (cx) |
| Panel lateral de estilo | | `PanelLateral.kt` | 2.185 | 75 (cx) |
| Mandos del panel | Botón que se arrastra, rueda de color, damero | `MandosDelPanel.kt` | 943 | 61 (cx) |
| Ventana de ajustes | | `VentanaDeAjustes.kt` | 815 | 53 (cx) |
| Paleta de colores | Combinaciones que pegan entre sí | `PaletaDeColores.kt` | 442 | 45 (cx) |
| Mando de la selección | Bola con las acciones de lo elegido | `Mando.kt` | 531 | 31 (cx) |
| Panel de figuras / teclado de fórmulas | | `DrawFiguras.kt` | 512 | 39 (cx) |
| Editor de tabla / tabla pegada | | `DrawTablas.kt` / `DrawTablaPegada.kt` | 308 / 198 | 45 / 37 (cx) |
| Ventana de referencia | Imagen que se copia, flotando encima | `VentanaDeReferencia.kt` | 247 | 35 (cx) |
| Cabecera de ventana flotante | Compartida por las tres ventanitas | `BarraDeVentana.kt` | 133 | 22 (cx) |
| Botón flotante | Y el agarre compartido con el lienzo | `BotonFlotante.kt` | 290 | 40 |
| **Cuentas de los deslizadores** | De dónde está el dedo a qué valor sale. **Sacado a propósito de la interfaz para poder probarlo** | `Deslizadores.kt` | 430 | 0 |
| **Marcas del deslizador** | Los valores favoritos, con imán | `MarcasDelDeslizador.kt` | 166 | 0 |
| **Reparto de la barra en grupos** | Qué botón enseña qué | `Barra.kt` | 189 | 0 |

---

## 5. Qué depende de gestos táctiles

Lo he buscado archivo por archivo. **Toda la dependencia táctil vive en
`DrawCanvas.kt` y en dos flags de `DrawController.kt`.** El controlador en sí no
sabe de dedos: recibe puntos ya en coordenadas de escena.

### 5.1 La interfaz de entrada del controlador

```kotlin
fun pointerDown(pRaw: Pt, pressure: Double = 1.0, zoom: Double = 1.0, cuando: Long = 0L)
fun pointerMove(pRaw: Pt, pressure: Double = 1.0, zoom: Double = 1.0, cuando: Long = 0L)
fun pointerUp(p: Pt, zoom: Double = 1.0)
fun latido(cuando: Long): Boolean   // se llama cada fotograma mientras dura el trazo
fun cancel()
```

Más estos interruptores, que en Android los pone el segundo dedo y en escritorio
serían teclas modificadoras:

| flag | qué hace | en Android lo activa | en Windows sería |
|---|---|---|---|
| `keepAspectRatio` | figura perfecta (círculo redondo, cuadrado cuadrado, línea al eje) | segundo dedo quieto | `Shift` |
| `resizeFromCenter` | redimensionar desde el centro | — | `Alt` |
| `discreteAngle` | rotación a múltiplos de 15° | — | `Shift` |
| `ellipseFromCenter`, `shapesFromCenter` | de dónde nace la figura | ajuste | ajuste |
| `modoDedo` | márgenes de picado más generosos | ajuste | debería ser `false` con ratón |

### 5.2 Lo que sí es táctil de verdad

| Gesto | Dónde | Qué hace | Qué habría que decidir en escritorio |
|---|---|---|---|
| **Un dedo dibuja, dos encuadran** | `DrawCanvas.kt` | La regla base del editor | Ratón dibuja, rueda hace zoom, botón central o barra espaciadora arrastra la vista |
| **El segundo dedo** | `DrawCanvas.kt:670-690` | Quieto = figura perfecta; pellizcando = encuadrar. **No se decide al posarse, sino al ver si se mueve** | `Shift` para lo primero; rueda/arrastre para lo segundo. El truco de «esperar a ver qué hace» deja de tener sentido |
| **Rechazo de palma** | `DrawCanvas.kt:675-681` | Con el lápiz en pantalla, los dedos se filtran (`filter { type == Stylus }`) | Desaparece. Con ratón no existe el problema |
| **Presión del lápiz** | `DrawCanvas.kt` (`lapiz.pressure`) → `PulsoDelMundo.kt`, `Freehand.kt` | El trazo engorda al apoyar | Un ratón no tiene presión. Queda `simulatePressure` (que ya existe y se calcula por velocidad), o soporte de tableta Wacom vía Windows Ink / `WM_POINTER` |
| **Detección automática de lápiz** | `DrawCanvas.kt:322` | Se enciende el modo lápiz al ver el primer `PointerType.Stylus` | Detectar si el evento viene de tableta |
| **Lápiz + 1 dedo apoyado** | `DrawCanvas.kt:505-545` | Abre la tira de colores sobre la punta del lápiz y se elige arrastrando | Atajo de teclado o menú radial con clic derecho |
| **Lápiz + 2 dedos apoyados** | `DrawCanvas.kt:513-535` | Borra mientras los dedos sigan ahí; al soltarlos vuelve la herramienta anterior | Una tecla mantenida (`E`) haría lo mismo |
| **Toque de cuatro dedos / varios dedos** | `DrawCanvas.kt` (`elToqueDeCuatroDedos`, `elToqueDeVariosDedos`) | Atajos globales | Teclas |
| **El dedo parado** | `DrawController.latido()` + `quietoEn`/`quietoDesde` | Si el dedo se para más de `ESPERA_PARA_LA_RECTA` mientras traza, la raya se pone recta o se convierte en compás | Funciona igual con el ratón parado, pero es un gesto pensado para el dedo; con ratón hay que probar si molesta |
| **La bolita** (`Tool.BOLITA`) | `DrawController.kt:1310` | Se pasa por encima y va entrando en la selección; se vuelve a pasar y sale | Existe con ratón, pero con ratón el rectángulo de selección ya funciona bien: pierde su razón de ser |
| **El mando** (`Mando.kt`) | Bola flotante con las acciones de lo elegido | Nació porque los tiradores de la caja quedan bajo la mano | Con ratón, los tiradores de la caja son mejores. `TransformHandles.kt` ya los tiene calculados |
| **Botón flotante** (`BotonFlotante.kt`) | Agarre compartido entre botón y lienzo sin pasar por Compose, porque muchos teléfonos cancelan el dedo cuando se acerca el lápiz | Problema exclusivo de Android. Desaparece |
| **Márgenes de picado** (`margenDelDedo(zoom)`) | `DrawController.kt` | El dedo tapa lo que toca; el margen es generoso | Con ratón hay que estrecharlo, o la selección coge de más |

**El dato importante:** todo esto son unas ~600 líneas de `DrawCanvas.kt` más un
puñado de flags. Las 3.482 líneas de `DrawController.kt` —que es donde está la
lógica de qué pasa al arrastrar cada cosa— no cambian.

---

## 6. Valoración de portabilidad

Tres cubos, como se pidió. La columna de líneas suma solo el archivo principal
de cada fila.

### 6.1 Se porta tal cual (lógica pura, aritmética y `serde`)

Son **68 archivos y 31.718 líneas**. Traducir Kotlin puro a Rust aquí es trabajo
mecánico: `data class` → `struct` + `#[derive(Serialize, Deserialize)]`,
`List<T>` → `Vec<T>`, `Map` → `HashMap`, `Double` → `f64`. Sin `unsafe`, sin
concurrencia, sin I/O salvo `Deflater`/`Inflater` (→ `flate2`).

| Bloque | Archivos | Líneas |
|---|---|--:|
| Modelo y escena | `Element`, `Scene`, `History`, `Rand` | 2.468 |
| Geometría | `Bounds`, `Transform`, `TransformHandles`, `Collision`, `Organize`, `Perimetros`, `Caminos` | 2.822 |
| Trazo | `Rough`, `Shapes`, `Freehand`, `PulsoDelMundo`, `AlisadoDeUnEuro`, `EspinaDelTrazo` | 2.470 |
| Figuras y anotación | `Arrows`, `Elbow`, `Arco`, `Lupa`, `Regiones`, `Recorte`, `Nudos`, `Puntos`, `TextoEnFiguras`, `Renglones`, `Theme` | 3.937 |
| Medir | `Medida`, `EscalaGrafica`, `Angulos`, `Tablas`, `Papel`, `Iman`, `Snapping`, `Cuadricula` | 1.449 |
| Matemáticas | `Plano`, `Espacio`, `Graficas`, `Ecuacion`, `Biblioteca`, `TablaDibujada`, `Cronograma` | 2.269 |
| 3D | `Solido`, `Proyeccion`, `MarcosMinimos` | 2.792 |
| PDF puro | `PdfLector`, `PdfLectura`, `PdfEscritura`, `PdfAnotado`, `PdfDeNota`, `PdfUnion`, `PlanoDePdf`, `PlanoWeb`, `MosaicoDePdf`, `CuadrosEnDisco` | 4.774 |
| Salidas de texto | `Svg`, `ExportarHtml`, `VisorPlano`, `VisorEspacio` | 3.317 |
| Proyecto | `Proyectos`, `HojasDelProyecto`, `Cuaderno` | 821 |
| Cuentas extraídas de la interfaz | `Deslizadores`, `MarcasDelDeslizador`, `Barra`, `DrawProperties` | 1.117 |
| Máquina de estados | `DrawController` | 3.482 |

Dos matices honestos dentro de este cubo:

- **`DrawController.kt` es puro, pero grande (3.482 líneas) y es la pieza de la
  que cuelga todo el comportamiento.** No tiene Android dentro, pero sí una
  `sealed class Gesture` con muchos estados y mucho estado mutable. Portarlo a
  Rust obliga a decidir cómo se modela ese estado (probablemente un `enum
  Gesture` y un `&mut self`). Es la traducción más cara del cubo, aunque no
  cambie de lógica.
- **`VisorPlano.kt` y `VisorEspacio.kt` (1.746 líneas) son JavaScript escrito
  dentro de cadenas de Kotlin.** Se copian sin traducir: en Rust son `&str`
  igual que en Kotlin son `String`. Coste casi nulo.

### 6.2 Hay que rehacer la entrada (la lógica vale, el gesto no)

La lógica está en el cubo anterior; lo que hay que reescribir es cómo se
dispara. En total **el trabajo aquí es reescribir `DrawCanvas.kt`** (1.742
líneas, de las que unas 600 son gestos) contra ratón + teclado + tableta.

| Herramienta / mecanismo | Qué se conserva | Qué hay que rehacer |
|---|---|---|
| Rectángulo, rombo, elipse, línea, flecha, marco, mosaico, lupa, escala gráfica | Todo (`Shapes`, `Rough`, `Scene.newElement`) | Arrastre con ratón; `Shift` en vez de segundo dedo para la figura perfecta |
| Lápiz y marcador | `Freehand`, `PulsoDelMundo`, `AlisadoDeUnEuro` | La presión: o tableta (Windows Ink) o `simulatePressure` por velocidad, que ya está implementado |
| Flecha libre (`FLECHA_LIBRE`) | Todo | Igual que el lápiz |
| Texto | `Renglones`, `TextoEnFiguras` | Entrada de teclado nativa en vez del teclado del móvil; cursor, selección |
| Selección y lazo | `Collision.getElementsWithinSelection` / `WithinLasso` | Estrechar `margenDelDedo`; `modoDedo = false` |
| Mover, redimensionar, rotar | `Transform`, `TransformHandles` | Tiradores con ratón (ya calculados); `Alt` = desde el centro, `Shift` = 15° |
| Cota y escalar | `Medida` | Diálogo de teclado en vez del táctil |
| Imán y enganche | `Iman`, `Snapping` | Radios pensados para el dedo; hay que reducirlos |
| Bote de pintura | `Regiones` (717 l.) | Un clic. Casi nada que cambiar |
| Recortar / extender | `Recorte` | Un clic sobre el tramo. Casi nada que cambiar |
| Alfileres (nudo) | `Nudos` | Clics; el radio de agarre |
| Puntos etiquetados | `Puntos` | Un clic imantado |
| Sólido, extruir, revolución | `Solido`, `Proyeccion` | El sólido se hace **en dos fases** (huella y luego altura) porque en isométrica un arrastre vertical no se distingue de una diagonal. Con ratón sigue existiendo la ambigüedad, así que las dos fases se quedan; pero podría resolverse con una tecla |
| Cronograma | `Cronograma` | Arrastre de barras con ratón; casi igual |
| Deshacer / rehacer | `History` | `Ctrl+Z` / `Ctrl+Y`. Trivial |
| Deslizadores del panel | `Deslizadores`, `MarcasDelDeslizador` | Los `Deslizadores` ya reciben píxeles y devuelven valores; solo cambia quién los llama |
| Barra de herramientas | `Barra` (grupos, orden, guardado) | El agrupar-para-que-quepa nació porque la barra tapa media pantalla del móvil. En escritorio caben las 32 herramientas |

### 6.3 Hay que repensarlo (no tiene sentido con ratón, o hay que sustituir la tecnología)

**A) Gestos que dejan de existir**

| Qué | Por qué | Qué hacer |
|---|---|---|
| Rechazo de palma (`DrawCanvas.kt:675`) | No hay palma | Borrar |
| Lápiz + 1 dedo → tira de colores; lápiz + 2 dedos → borrador temporal (`DrawCanvas.kt:505-545`) | No hay «la otra mano ya está en la pantalla» | Sustituir por teclas mantenidas o menú radial con clic derecho |
| `elToqueDeCuatroDedos` / `elToqueDeVariosDedos` | | Atajos de teclado |
| `BotonFlotante.kt` (290 l.) y su `AgarreDelBoton` | Existe porque Android cancela el dedo al acercarse el lápiz | Borrar entero |
| `Mando.kt` (531 l.) + `MandosDelPanel.kt` (943 l.) | La bola flotante existe porque los tiradores de la caja quedan bajo la mano. Con ratón el cursor no tapa nada | Usar `TransformHandles` directamente. Se ahorran ~1.500 líneas |
| `Tool.BOLITA` (`DrawController.kt:1310-1345`) | Nació porque con el dedo un recuadro de selección coge de más y de menos | Con ratón, el recuadro basta. Se puede conservar como opción, pero deja de ser necesaria |
| «El dedo parado endereza la raya» (`latido`, `ESPERA_PARA_LA_RECTA`) | Con ratón parado se dispararía sin querer | Probarlo; probablemente ponerlo tras un ajuste, apagado por defecto |
| `modoDedo` y `margenDelDedo(zoom)` | Márgenes generosos para el dedo | Recalibrar todos los umbrales del módulo |

**B) Tecnología que hay que sustituir, no traducir**

| Qué | Líneas | Depende de | Sustituto en Windows/Rust |
|---|--:|---|---|
| `Renderer.kt` | 3.824 | `android.graphics.Canvas`, `Paint`, `Path`, `Matrix`, `Shader` | Reescritura contra `tiny-skia`, `skia-safe`, `vello` o `femtovg`. **Es el trabajo más grande del port.** La buena noticia: la geometría ya está fuera, en `Shapes.kt` y `Caminos.kt`, y `Renderer` recibe `Op` |
| `PdfDoc.kt`, `PdfMiniaturas.kt`, `ElMosaicoDelPapel.kt`, `LaminaDeCerca.kt`, `PlanoEnPantalla.kt`, `DrawPdf.kt`, `PdfDelProyecto.kt` | 2.334 | `PdfRenderer` y `PdfDocument`, que **vienen en Android** | En Windows no existe equivalente del sistema. Hace falta `pdfium` (rasterizar) y `printpdf`/generación propia (escribir). Aquí el port pierde la ventaja de «sin librerías» |
| `Glifos.kt` + `PdfLienzo.kt` (parte de texto) | 1.290 | `android.graphics.Path` + `Typeface` para sacar el perfil de las letras | `ttf-parser` / `rustybuzz` / `swash`. Las salidas vectoriales necesitan contornos de glifo sí o sí |
| `DrawFonts.kt` | 116 | `Typeface` y `assets` | Cargar los `.ttf` a mano. Las fuentes (Excalifont, Nunito, Comic Shanns) hay que incluirlas |
| `AudioLigero.kt` | 110 | `MediaCodec` / `MediaMuxer` | Codificar AAC en Windows es otro mundo. Se puede dejar fuera del port inicial: solo sirve para aligerar el HTML exportado |
| `DrawSvg.kt` | 1.459 | Solo 6 imports de Android (`Bitmap` para las imágenes incrustadas, `Base64`) | Casi portable; hay que sustituir la parte de imágenes |
| `DrawExport.kt`, `ExportarProyecto.kt`, `ExportarProyectoWeb.kt` | 552 | `Bitmap`, `Context` | Reescribir la parte de archivo; la decisión de qué entra es pura |
| Toda la interfaz Compose (13 archivos) | 11.549 | Jetpack Compose | Reescritura completa contra lo que use PixPin PC. No es «portar», es «rehacer»: son decisiones de móvil (panel lateral pegado al borde, ventanitas flotantes, mandos que se arrastran) que en escritorio se resuelven de otra manera |

### 6.4 Resumen numérico del port

| Cubo | Líneas | % |
|---|--:|--:|
| Se porta tal cual | ~31.700 | 57 % |
| Hay que rehacer la entrada (mismo código, otro disparador) | ~1.700 (`DrawCanvas.kt`) | 3 % |
| Hay que repensarlo o sustituir tecnología | ~22.000 | 40 % |

De esas ~21.900 líneas del tercer cubo, **11.549 son interfaz Compose** que en
cualquier caso había que rehacer, y **3.824 son el renderizador**, que es una
reescritura acotada porque recibe geometría ya calculada. El resto (6.552) es
PDF, fuentes, salidas y almacenamiento: ahí el problema no es la lógica sino que
Android traía gratis un rasterizador de PDF y un motor de fuentes.

---

## 7. Lo que no he podido comprobar

- **Que las pruebas pasen.** No hay Gradle ni SDK de Android en este entorno. Solo
  he podido contar los 151 archivos de prueba y leer `MotorSeparadoTest.kt`. La
  cifra «936 pruebas» de `docs/motor.md` no la he verificado.
- **Fidelidad de los ports.** Que `Rough.kt` produzca exactamente el mismo
  garabato que `rough.js`, o que `Freehand.kt` coincida con `perfect-freehand`,
  está afirmado en los comentarios y hay pruebas con esos nombres, pero no lo he
  ejecutado.
- **`DrawEditorActivity.kt` (3.712 líneas) y `PanelLateral.kt` (2.185).** Los he
  clasificado por sus imports y por su documentación de cabecera, no leídos
  enteros. Es posible que haya lógica pura enterrada ahí dentro que convendría
  rescatar antes de tirarlos; merece una segunda pasada si el port llega a
  necesitarla.
- **Cuánto de `Renderer.kt` es geometría y cuánto es `Canvas`.** He visto que la
  geometría está fuera (`Shapes`, `Caminos`), pero 3.824 líneas son muchas y no
  he medido la proporción exacta.

---

## 8. Las tres cosas que más importan de este informe

1. **La frontera puro/Android existe de verdad y está vigilada por una prueba**
   (`MotorSeparadoTest`), con una lista blanca de 36 archivos comentada uno por
   uno. Eso es un regalo para el port: dice exactamente qué se puede traducir a
   ciegas.
2. **Pero no es «el motor no toca Android»: son 35 de 103 archivos y el 43 % de
   las líneas.** La frase de `docs/motor.md` habla del núcleo, no del módulo, y
   se puede leer mal. Además la lista blanca está desfasada en un archivo:
   `Theme.kt` ya no importa nada de Android y sigue listado.
3. **`docs/motor.md` es un póster, no un inventario.** Sus cuatro grupos cubren
   la barra de herramientas y dejan fuera el PDF (7.000 líneas), el 3D (3.300),
   los instrumentos matemáticos (2.300), las salidas (5.000) y el proyecto. Si
   se planifica el port a partir de esa documentación, se subestima el módulo por
   un factor de dos largo.
