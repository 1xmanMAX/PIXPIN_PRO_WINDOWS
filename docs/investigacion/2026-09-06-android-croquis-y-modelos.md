# Tres módulos del PixPin de Android: croquis 3D, modelos de voz y Markdown

**Fecha:** 2026-09-06 · **Material:** el clon de solo lectura
`proyectos de referencia/PIXPIN_PRO_ANDROID`, una aplicación Android en Kotlin
con Compose, 242 ficheros `.kt` y 117.228 líneas en `app/src/main/java`.
Versión declarada `0.28.3` (`versionCode` 81).

## 0. Qué se ha mirado y cómo

Se ha inventariado con `find` y `wc -l`, y la pureza se ha comprobado con `grep`
fichero a fichero, no de oído. Todos los recuentos de este informe son medidos.

| Módulo | Ficheros | Líneas | % del código de la app | Tests que lo tocan |
|---|---:|---:|---:|---:|
| `croquis3d` | 23 | 21.024 | 17,9 % | 5.138 líneas |
| `motormd` | 22 | 6.964 | 5,9 % | 3.156 líneas |
| `onnx` (`com.k2fsa.sherpa.onnx`) | 5 | 1.579 | 1,3 % | 58 líneas |

Lo que **no** se ha hecho: ejecutar la aplicación, compilar nada ni medir
rendimiento. Todo lo que sigue sale de leer el código y su documentación
incrustada, que en este proyecto es inusualmente extensa y explica el porqué de
casi cada decisión.

Un aviso de método: la documentación de este repositorio es fiable pero **no
está toda al día**. El caso concreto está en la sección 1.7.

---

## 1. `croquis3d` — dibujar en el aire y darle la vuelta

Ruta: `F:\THE FORGE\PIXPIN PC VERSION MAX\proyectos de referencia\PIXPIN_PRO_ANDROID\app\src\main\java\com\forge\pixpin\croquis3d\`

### 1.1 Qué hace

Es una aplicación de croquis a mano alzada en el espacio. La idea de la que sale
está escrita en la cabecera de `Croquis3D.kt` (líneas 9-37): dibujar en tres
dimensiones con el dedo tiene un problema que no tiene dibujar en dos, y es que
la pantalla es plana. Un toque en la pantalla no señala un punto del espacio,
señala una recta entera, y hay que decidir en qué parte de esa recta cae el
trazo. Los programas de modelar lo resuelven con menús y planos de trabajo; aquí
se resuelve con dos herramientas que se turnan. **El lápiz** dibuja sobre un
plano: el de la lámina que se toque, o el de la propia pantalla si no se toca
ninguna. **La lámina** dibuja ese plano: se traza una raya y la raya se barre
hacia dentro de la pantalla, en la dirección en la que se está mirando. Se pone
una lámina, se gira la vista, se dibuja encima, se pone otra lámina en otra
dirección, se vuelve a girar.

No hay mallas ni sólidos porque no pretende ser un programa de modelar: el
resultado es un boceto que se puede mirar desde otro lado, que es lo que a un
croquis a mano le falta siempre. Alrededor de esas dos herramientas ha crecido
bastante más: grupos que funcionan como capas, un espejo de simetría, imágenes
puestas en el espacio con sus cuatro esquinas, vistas congeladas que se guardan
con su cámara, un sol y unas luces, profundidad de campo con punto de enfoque,
un taller para dibujar la punta del pincel a mano, gráficas de funciones en 3D,
y un modo de realidad aumentada que pinta el croquis encima de lo que ve la
cámara del teléfono. El pintado no usa la tarjeta gráfica: es el algoritmo del
pintor sobre un lienzo 2D de Compose, ordenando por hondura, y su propia
documentación (`Croquis3DLienzo.kt:167-172`) admite que un motor de verdad
usaría un búfer de profundidad y que no lo hace porque eso pediría una
superficie con OpenGL.

### 1.2 Sus piezas principales

| Fichero | Líneas | Qué es |
|---|---:|---|
| `Croquis3DLienzo.kt` | 6.156 | El pintado y el gesto. La pieza más grande del módulo y de la app |
| `Croquis3DControlador.kt` | 3.510 | La ley: dónde cae el dedo, qué se crea, qué borra el borrador. ~90 funciones públicas |
| `Croquis3DActivity.kt` | 3.088 | Toda la pantalla: barras, paneles, permisos, exportación, integración con proyectos |
| `Croquis3D.kt` | 1.726 | **El modelo serializado** (17 tipos `@Serializable`) y el trazado de rayos |
| `Croquis3DTiras.kt` | 1.085 | Deslizadores, paletas y el taller de puntas |
| `Croquis3DPluma.kt` | 970 | El pintor de trazos: proyecta el esqueleto y rellena la cinta |
| `Camara3D.kt` | 635 | **La cámara de órbita** y las proyecciones |
| `Croquis3DMando.kt` | 568 | El aro de manipular la selección (girar, escalar) |
| `Croquis3DEsqueleto.kt` | 439 | El esqueleto del trazo: spline, marcos y anchos, cocidos una vez |
| `Croquis3DHoja.kt` | 361 | Pasa una vista a vectores y la guarda como lámina del proyecto |
| `Tinta.kt` | 350 | Qué es una tinta, dicho en un solo sitio |
| `ExportarCroquisHtml.kt` | 267 | El JSON de geometría para el visor web |
| `Croquis3DCamara.kt` | 270 | CameraX y los sensores: la única capa Android del modo realidad |
| `Croquis3DCubo.kt` | 230 | El cubo de vistas de la esquina |
| `Croquis3DLista.kt` | 227 | La lista de grupos |
| `ExportarObj.kt` | 225 | OBJ + MTL |
| `Croquis3DVistas.kt` | 204 | La galería de vistas congeladas |
| `Graficas3D.kt` | 190 | Fórmulas a alambrada de trazos |
| `Graficas3DUi.kt` | 181 | El diálogo de las gráficas |
| `InerciaDelTelefono.kt` | 118 | Cuánto se ha movido el teléfono, del acelerómetro |
| `LenteDelAparato.kt` | 89 | El campo de visión de la cámara del aparato |
| `PosturaDelTelefono.kt` | 76 | Matriz de giro de Android a los tres ángulos de la cámara |
| `Croquis3DAlmacen.kt` | 59 | JSON con GZIP, escritura a temporal y renombrado |

Los tipos que forman el fichero guardado, todos en `Croquis3D.kt`: `Croquis`
(línea 39) con `trazos`, `laminas`, `grupos`, `imagenes`, `vistas`, `espejo`,
`sol`, `luces`, `puntas`, `favoritos`, color de fondo, apertura y enfoque;
`Trazo3D` (552) con sus puntos, presiones, tiempos, normales por punto y su
punta; `Lamina3D` (795) con su perfil y su dirección de barrido; `Esfera3D`
(928) y `FormaDeBola` (1084: bola, cilindro, cono, anillo); `PuntaDelPincel`
(244); `Vista3D` (475); `Imagen3D` (540); `Grupo3D` (701); `Espejo3D` (1106);
`Impacto` (1149), que es el resultado de lanzar un rayo. `Camara3D`
(`Camara3D.kt:53`) guarda giro, inclinación, zoom, centro, balanceo y lente, y
lleva la proyección entera: ortográfica de fábrica y hasta ojo de pez, con
mapeo equidistante `r = f·θ` en vez de `r = f·tan θ` para que abrir el campo a
160° no mande los bordes al infinito.

### 1.3 De qué depende

- **androidx.compose**: el grueso. `foundation.Canvas`, `foundation.gestures`,
  `runtime` (`mutableStateOf`, `snapshotFlow`), `ui.graphics` (`Path`, `Paint`,
  `BlendMode`, `graphicsLayer`, `clipPath`), `material3`, `LazyColumn`.
- **Canvas nativo de Android**: se baja a `nativeCanvas` en
  `Croquis3DLienzo.kt:27` y `Croquis3DCubo.kt:15`, y usa `asAndroidPath`
  (`Croquis3DLienzo.kt:33`) para lo que Compose no expone.
- **CameraX** (`camera-camera2`, `camera-lifecycle`, `camera-view`, tres
  dependencias del `build.gradle.kts`): solo en `Croquis3DCamara.kt`.
- **Sensores de Android**: solo en `Croquis3DCamara.kt`, con
  `TYPE_ROTATION_VECTOR`, `TYPE_LINEAR_ACCELERATION` y `TYPE_STEP_DETECTOR`.
- **kotlinx.serialization** para el fichero, y `java.util.zip` para el GZIP.
- **Nada de terceros para el 3D.** No hay OpenGL, ni filament, ni sceneform.

Hay además una dependencia interna que conviene tener presente: **`croquis3d`
no se vale solo**. Importa 67 veces del paquete `com.forge.pixpin.motor` (el
motor de dibujo 2D), más una vez de `ui`, una de `pin` y una de `data`. Lo que
coge es casi todo menudo — `Pt` y `Pt3` (los puntos), `parseColor`, `randomId`,
la rueda del color, el alisado del puntero — pero también cosas con sustancia:
`Formula.compilar` (el evaluador de expresiones que vive en
`motor/Graficas.kt:258`, con derivadas, funciones por partes y condiciones),
`Element`/`Scene`/`ExcalidrawStore` para guardar una vista como lámina, y
`elToqueDeCuatroDedos` (`motor/DrawCanvas.kt:1509`).

### 1.4 Qué hay del núcleo que sea puro

Comprobado con `grep -c '^import android'` fichero a fichero. **Diez de los 23
ficheros no tienen ni un solo `import` de Android ni de androidx**, y suman
**4.115 líneas, el 19,6 % del módulo**:

| Fichero puro | Líneas | Qué contiene |
|---|---:|---|
| `Croquis3D.kt` | 1.726 | El modelo entero y el trazado de rayos (Möller-Trumbore en línea) |
| `Camara3D.kt` | 635 | La cámara de órbita y todas las proyecciones |
| `Croquis3DEsqueleto.kt` | 439 | El cocido del trazo: spline, marcos por transporte paralelo, anchos |
| `Tinta.kt` | 350 | El motor de tintas. Ni un `import` en todo el fichero |
| `ExportarCroquisHtml.kt` | 267 | La geometría a JSON |
| `ExportarObj.kt` | 225 | OBJ + MTL |
| `Graficas3D.kt` | 190 | Fórmulas a trazos |
| `InerciaDelTelefono.kt` | 118 | Odometría inercial con ZUPT |
| `LenteDelAparato.kt` | 89 | Campo de visión |
| `PosturaDelTelefono.kt` | 76 | Matriz de giro a ángulos |

A esos hay que sumar dos casos que están atados de mentira:

- **`Croquis3DControlador.kt` (3.510 líneas) importa exactamente tres cosas de
  Compose**, y las tres son la misma: `mutableStateOf`, `getValue` y `setValue`
  (líneas 3-5). Nada más. Es decir, las 3.510 líneas de la lógica de la
  aplicación son portables cambiando el sistema de estado observable. Con esto
  dentro, el trozo portable pasa de 4.115 a **7.625 líneas, el 36 %**.
- `Croquis3DPluma.kt` (970) usa de Compose solo `Color`, `Path`, `DrawScope` y
  `dp`. La lógica de rieles y tono es agnóstica; hace falta una abstracción de
  camino y relleno.

Lo verdaderamente atado son `Croquis3DLienzo.kt` (6.156),
`Croquis3DActivity.kt` (3.088) y las barras y paneles: unas **11.400 líneas de
interfaz y pintado**.

**A diferencia de `motormd`, esta frontera no está vigilada por ningún test.**
Existe `MotorMdSeparadoTest.kt` para el motor de Markdown y `MotorSeparadoTest`
para el de dibujo, pero no hay equivalente para `croquis3d`. La separación de
hecho es buena, pero es una costumbre, no una regla comprobada.

### 1.5 Qué depende de gestos táctiles

Este es el punto crítico del módulo, y no se resuelve con un mapeo mecánico.

**Todo el gesto del lienzo es un único `pointerInput` con `awaitEachGesture` y
un bucle de `awaitPointerEvent` escrito a mano** (`Croquis3DLienzo.kt:299-513`).
No usa `detectTransformGestures` en ningún sitio del módulo. La regla maestra
está escrita en el propio código (`Croquis3DLienzo.kt:331-343`): *el lápiz
dibuja y la mano navega*. De ahí sale gratis el rechazo de palma, porque la
palma es un dedo y los dedos no pintan. Y la mano navega siempre, se tenga
puesta la herramienta que se tenga.

| Gesto | Dónde | Qué hace |
|---|---|---|
| **Un dedo** | `Croquis3DLienzo.kt:465-473` | **Orbita.** 700 px de arrastre son una vuelta entera (`VUELTA_ENTERA`). Va a `Croquis3DControlador.navegar:2673` → `Camara3D.girada:356`, que suma a giro e inclinación, con la inclinación acotada a un pelo del cenit |
| **Dos dedos** | `Croquis3DLienzo.kt:474-482` | Desplazan y acercan a la vez: `calculatePan()` mueve el centro, `calculateZoom()` acerca **al punto de los dedos, no al centro de la pantalla** (`Croquis3DControlador.kt:2758-2772`) |
| **Tres dedos** | `Croquis3DControlador.kt:2691-2701` | **Ladean la cámara y le cambian la lente.** Horizontal es balanceo, vertical abre el campo de ortográfica a ojo de pez (`RECORRIDO_DE_LA_LENTE = 520`). El pellizco se ignora a propósito: con tres dedos apoyados es imposible no pellizcar un poco |
| **Cuatro dedos** | `Croquis3DActivity.kt:553`, definido en `motor/DrawCanvas.kt:1509` | Esconde y saca la interfaz. Se lee en la pasada inicial y solo consume cuando ya hay cuatro |
| **Doble toque** | `Croquis3DLienzo.kt:491-511` | Encaja la vista en la vista técnica más cercana |
| **Lápiz** | `Croquis3DLienzo.kt:390-429` | Cuando baja un stylus, los dedos se ignoran del todo. Lee los eventos **históricos** (`punta.historical`, línea 413) porque el lápiz manda ~200 muestras por segundo y la pantalla va a 60-80 |
| **Cubo de vistas** | `Croquis3DCubo.kt:78-83` y `97-112` | Tocar una cara pide alzado, perfil o planta; arrastrar orbita la vista, o gira la pieza si hay selección |
| **Aro de manipular** | `Croquis3DMando.kt:108-150` | Girar por ángulo polar, escalar por razón de radios |
| **Bóveda del sol** | `Croquis3DActivity.kt:2367-2397` | El ángulo es el azimut y la distancia al centro es la altura |

Hay además un detalle fino que revela cuánto trabajo hay aquí: el fotograma en
que cambia el número de dedos apoyados **se descarta** (`Croquis3DLienzo.kt:449-463`),
porque el centroide salta y la vista pegaría un tirón.

**Nada de esto se traduce solo a un ratón.** Un PC de escritorio no tiene dos
dedos ni tres. El mapeo natural sería botón izquierdo para dibujar, botón
central o Alt+izquierdo para orbitar, Mayús+central para desplazar, rueda para
acercar (y el «acercar al punto» ya está resuelto, solo cambia de dónde sale el
punto), Ctrl+rueda para la lente, doble clic para encajar. Pero eso es un
rediseño de la interacción, no una traducción, y hay que probarlo con la mano:
lo que en el teléfono es un gesto continuo aquí pasa a ser un modificador
mantenido, que se siente distinto.

Dos cosas que sí se conservan enteras: la matemática de `navegar`, `acercar` y
`andar` del controlador, que no sabe de dedos sino de desplazamientos y
factores; y los eventos históricos, que en Windows tienen equivalente en Windows
Ink (`PointerPoint.GetIntermediatePoints`) y en Wintab. Sin eso, los trazos de
una tableta gráfica saldrán poligonales.

### 1.6 Los sensores y la cámara: el modo realidad

Cuatro ficheros (553 líneas) más una rama del controlador (`realidad`, desde la
línea 2480) implementan **realidad aumentada sin ARCore**: el vídeo de la cámara
trasera al fondo, el croquis pintado encima con fondo transparente, y la cámara
virtual siguiendo la orientación real del aparato.

`PosturaDelTelefono.kt` pasa la matriz de `SensorManager.getRotationMatrix` a
los tres ángulos de la cámara, y aprovecha que los dos sistemas coinciden (z
arriba, a derechas) para no convertir nada. `LenteDelAparato.kt` calcula qué
campo de visión llega de verdad a la pantalla, que es un recorte de un recorte:
sensor, proporción de la vista previa, rotación de pantalla y relleno; sin eso
el croquis resbala sobre la habitación al girar. `InerciaDelTelefono.kt` integra
dos veces el acelerómetro, con fuga exponencial y ZUPT, y su propia
documentación admite en la primera línea que esto se va a metros de error en
pocos segundos y que ARCore usa la cámara precisamente por eso. En el modo
realidad se apaga el giro con un dedo (dos mandos para lo mismo hacían temblar
la escena), se apaga la lente, y el pellizco pasa a ser andar hacia delante.

**En un PC esto no tiene sentido y hay que descartarlo entero.** No hay vector
de rotación, ni acelerómetro, ni podómetro, ni cámara trasera calibrada. Se
salvan dos cosas: `Camara3D.rectilinea` como opción de proyección, que sirve
igual sin cámara, y —si algún día el objetivo incluyera tabletas Windows con
IMU— la trigonometría de `PosturaDelTelefono.kt`, que es estándar. Para el 99 %
de los PC, son 553 líneas y una rama del controlador que se tiran.

### 1.7 La documentación no describe este módulo

`docs/superpowers/specs/2026-08-02-croquis-acotado-design.md` (209 líneas) es el
diseño del **croquis acotado**, y no es esto. Describe un editor **plano**, en
metros, con `Double` para no perder milímetros en coordenadas de orden 10⁶, con
línea, polilínea, rectángulo, círculo, texto y cota; una captura de plano de
fondo que se calibra trazando una raya sobre una medida conocida; modo medir
efímero; y exportación a PDF vectorial. Su modelo es `Croquis(entidades, fondo,
decimales)` con `P(x, y)`.

En el código no hay nada de eso. El `Croquis` que existe hoy
(`Croquis3D.kt:39`) tiene trazos y láminas en tres dimensiones, y no tiene
`Cota`, ni `Fondo`, ni `metrosPorPixel`, ni las entidades geométricas del
diseño. La razón por la que el diseño se escribió —medir sobre un plano
capturado, en obra— no se ve por ningún lado en `croquis3d`. **El documento
describe un proyecto que o se abandonó o vive en otro sitio**; el módulo que
lleva ese nombre es otra cosa. Al portar, ese documento no sirve de
especificación: sirve como aviso de que hay una necesidad medida (medir sobre
una captura calibrada) que este módulo no cubre.

Lo que sí es especificación ejecutable son los **5.138 líneas de test** en
16 ficheros, con 248 pruebas: `Croquis3DTest.kt` (2.971 líneas él solo),
`BaseDeCamaraTest.kt`, `MotorPlumaEsqueletoTest.kt`,
`MotorPlumaFijezaTest.kt`, `TintaTest.kt`, `ExportarObjTest.kt`,
`Graficas3DTest.kt`, y los tres de los sensores puros. La cobertura sigue
exactamente la línea de fractura: **todo lo puro tiene prueba en la JVM**, y el
pintado y la actividad no tienen ninguna.

### 1.8 Un dato que cambia el cálculo: la proyección ya se portó una vez

`app/src/main/java/com/forge/pixpin/motor/VisorEspacio.kt` son **1.063 líneas de
JavaScript** dentro de un `String` de Kotlin: un visor del croquis para la
página web exportada, con la **misma proyección que `Camara3D.aPantalla`,
portada línea a línea**, y con una prueba que compara las dos y se cae si se
toca una y no la otra. Pinta trazos como polilíneas ordenadas de lejos a cerca,
hojas como contorno relleno con velo según el ángulo, sólidos cara a cara con
descarte por sentido de giro, e imágenes con su textura. Lleva también su propio
sistema de órbita, con una decisión de interacción documentada: se gira
alrededor del centro de lo que se ve y se recoloca la cámara para que ese punto
no se mueva de la pantalla.

Esto vale mucho para la decisión de portar. Significa que el núcleo geométrico
ya demostró que se puede sacar de Kotlin y de Compose y seguir funcionando en
otro lenguaje y otra superficie de dibujo, y que existe una segunda
implementación de referencia contra la que comparar la de Rust.

### 1.9 Valoración

Este módulo es dos proyectos metidos en una carpeta, y hay que separarlos antes
de decidir nada.

**El núcleo geométrico se porta bien y merece la pena.** Son 4.115 líneas puras
más 3.510 del controlador, con 248 pruebas que funcionan como oráculo: se
escriben en Rust las mismas pruebas, y cuando pasan, está portado. Son `struct`,
`enum` y funciones sobre `f64`; el trazado de rayos, la cámara de órbita, el
cocido del trazo y el motor de tintas no tienen nada que traducir. El
exportador OBJ sale casi copiado. Aquí no veo riesgo.

**El pintado hay que rehacerlo, y el port es la oportunidad de hacerlo mejor.**
Las 11.400 líneas de Compose no se portan: se reescriben contra `pixpin-render` /
`pixpin-gpu`. Y hay un detalle a favor: la versión de Android pinta con el
algoritmo del pintor porque no quiso pedir una superficie con OpenGL, y su
propia documentación dice que un motor de verdad usaría un búfer de
profundidad. En Windows ya hay Direct2D y una capa de GPU en el proyecto, así
que el port arranca sin esa limitación. Ojo: eso también quiere decir que el
pintado no se porta, se rediseña, y que las decisiones finas de la versión
Android (las dos capas separadas por rendimiento, el nivel de detalle que pinta
basto al mover y fino al parar) hay que volver a tomarlas.

**El gesto es el trabajo de verdad, y no está resuelto por nadie.** El reparto
uno/dos/tres/cuatro dedos es un diseño de interacción entero que no tiene
equivalente en un ratón, y el módulo es lo que es en buena medida por ese
reparto. Aquí no hay port: hay que diseñar la interacción de escritorio,
probarla y aceptar que no va a sentirse igual. Presupuestar esto como
«traducir gestos» es equivocarse por mucho.

**El modo realidad no tiene sentido en un PC y hay que decirlo sin rodeos.**
553 líneas más una rama del controlador que se tiran. No es una pérdida: es una
función de teléfono, atada a sensores que un ordenador de sobremesa no tiene, y
cuya propia documentación reconoce que la parte inercial es imprecisa por
construcción.

**Y una duda de fondo que no es técnica.** Un croquis 3D a mano alzada es una
herramienta que se hizo para dibujar con el dedo o con un lápiz sobre una
pantalla que se puede girar en la mano. Vale la pena preguntarse, antes de
gastar el esfuerzo, si alguien va a dibujar en el aire con un ratón. Con una
tableta gráfica sí; con un ratón, tengo dudas serias. Si la respuesta es
«tableta o nada», el módulo sigue teniendo sentido pero es una función de nicho,
y probablemente lo primero que hay que portar no es el editor entero sino **el
visor**: cargar un `.pixpin` con croquis, girarlo, medirlo y exportarlo. Eso son
las 4.115 líneas puras más un pintor, sin ninguno de los gestos difíciles, y ya
existe una implementación de referencia en `VisorEspacio.kt` que hace
exactamente eso.

---

## 2. `onnx` — no es un módulo, es una biblioteca ajena copiada

Ruta: `F:\THE FORGE\PIXPIN PC VERSION MAX\proyectos de referencia\PIXPIN_PRO_ANDROID\app\src\main\java\com\k2fsa\sherpa\onnx\`

### 2.1 Qué hace

Esto no es código del proyecto. El paquete lo dice: `com.k2fsa.sherpa.onnx` son
las **ligaduras Kotlin de sherpa-onnx**, del grupo k2-fsa, copiadas tal cual del
proyecto original (licencia Apache-2.0). No hacen inferencia: declaran las
estructuras de configuración y llaman por JNI a una biblioteca nativa que sí la
hace. Están en inglés, con comentarios `TODO(fangjun)` del autor original, y
contrastan con el resto del repositorio, que está en castellano.

Quien las usa es un fichero que no está en esta carpeta:
`app/src/main/java/com/forge/pixpin/guardados/MotorWhisper.kt` (246 líneas). Ese
sí es del proyecto, y es el que hace el trabajo: transcribir audio a texto en el
aparato, sin conectarse a nada. Es uno de los tres motores de voz de la
aplicación —los otros son Vosk (`com.alphacephei:vosk-android:0.3.47`, con su
propia biblioteca nativa) y el de Google— y se elige en Ajustes.

### 2.2 Sus piezas principales

| Fichero | Líneas | Qué es |
|---|---:|---|
| `OfflineRecognizer.kt` | 1.512 | 21 tipos. **Pero solo 286 líneas son útiles** |
| `OfflineStream.kt` | 42 | El flujo de audio: `acceptWaveform`, y cuatro `external fun` |
| `FeatureConfig.kt` | 11 | Frecuencia de muestreo y dimensión del vector |
| `QnnConfig.kt` | 7 | Configuración del acelerador de Qualcomm |
| `HomophoneReplacerConfig.kt` | 7 | Sustitución de homófonos |

De `OfflineRecognizer.kt`, las líneas 1-286 son las `data class` de
configuración (una por cada familia de modelo: transductor, paraformer, NeMo,
Dolphin, Zipformer, WeNet, FireRedASR, FunASR, Qwen3, Canary, Cohere,
Whisper...) y la clase `OfflineRecognizer` con sus ocho `external fun`. **Las
líneas 287-1512 son una sola función, `getOfflineModelConfig(type: Int)`, que es
código de ejemplo del proyecto original y que en esta aplicación no se llama
desde ningún sitio** — comprobado con `grep`. Son 1.226 líneas muertas, el 78 %
del módulo.

La lógica de verdad está en `MotorWhisper.kt`: descarga del modelo con barra de
avance y ficheros temporales renombrados al terminar, instalación manual de un
fichero bajado a mano con el navegador, carga perezosa del reconocedor (cargar
cuesta segundos y se reutiliza), troceado del audio en tandas de 20 segundos
—porque Whisper oye 30 de una vez y rellena con silencio, así que cuesta casi lo
mismo un trozo de dos segundos que uno de veinte—, forzado del idioma con una
lista blanca de los 99 códigos que Whisper entiende (otro tumbaría la biblioteca
nativa), y un modo de dos idiomas en el que se le deja adivinar y solo se acepta
lo que diga si es uno de los dos.

### 2.3 El modelo: cuál, para qué, cuánto y de dónde

Esta es la pregunta que importa, y la respuesta está medida.

| | |
|---|---|
| **Qué modelo** | Whisper de OpenAI, convertido a ONNX y cuantizado a int8 por el proyecto sherpa-onnx |
| **Para qué** | Reconocimiento de voz en el aparato, sin red. Y, de propina, traducción: con el idioma forzado, lo que oye en otro lo escribe en ese |
| **Tres tamaños** | `tiny` 104 MB, `base` 161 MB, `small` 375 MB (`MotorWhisper.kt:47`). Por defecto `tiny` |
| **Tres ficheros por modelo** | `<x>-encoder.int8.onnx`, `<x>-decoder.int8.onnx`, `<x>-tokens.txt` |
| **De dónde sale** | `https://huggingface.co/csukuangfj/sherpa-onnx-whisper-<modelo>/resolve/main/` |
| **Cómo llega** | **Se descarga la primera vez que se usa**, no viaja en el APK. También se puede bajar a mano con el navegador e importar el fichero |
| **Dónde se guarda** | `filesDir/whisper/<modelo>/` |

Y aquí está el coste que no se ve en el modelo: **las bibliotecas nativas sí
viajan dentro del APK**, en `app/src/main/jniLibs/arm64-v8a/`:

| Fichero | Tamaño medido |
|---|---:|
| `libonnxruntime.so` | **21.684.880 bytes (20,7 MB)** |
| `libsherpa-onnx-jni.so` | **4.761.536 bytes (4,5 MB)** |

**25,2 MB de código nativo de terceros**, solo para `arm64-v8a` — la aplicación
declara `abiFilters` con dos arquitecturas, pero la biblioteca solo está para
64 bits, y `MotorWhisper.soportado()` lo comprueba cargándola y avisa si no va.

La documentación del propio proyecto anota (`MotorWhisper.kt:23-26`) que el
usuario probó el `tiny` el 6 de septiembre de 2026 y no era «el mejor» como
esperaba: es el más pequeño que existe.

### 2.4 Qué es puro y qué está atado

De los cinco ficheros de `com.k2fsa.sherpa.onnx`, **cuatro no importan nada de
Android**. El único que sí es `OfflineRecognizer.kt`, y por un solo `import`:
`android.content.res.AssetManager` (línea 3), que es la vía alternativa de
cargar el modelo desde los recursos del APK — la que esta aplicación **no usa**,
porque carga desde fichero.

Pero medir la pureza aquí es engañoso y hay que decirlo claro: **el 100 % de
este módulo está atado a la plataforma, solo que no por Android sino por JNI**.
Los ocho `external fun` de `OfflineRecognizer` y los cuatro de `OfflineStream`
no hacen nada por sí mismos; toda la inferencia está en los 25 MB de `.so`
compilados para ARM. Es una fachada, y una fachada de una biblioteca ajena.

`MotorWhisper.kt` sí importa `android.content.Context`, pero solo para dos
cosas: saber dónde está `filesDir` y leer el ajuste de qué modelo se quiere. El
resto —descarga, troceado del PCM, conversión de bytes a muestras de -1 a 1,
gestión de idiomas— es Kotlin y JVM del montón.

### 2.5 Gestos táctiles

Ninguno. No hay interfaz aquí. La elección de motor y la descarga del modelo se
manejan desde `MainActivity.kt` (líneas 934-1106), que es una pantalla de
ajustes normal.

### 2.6 Valoración

**Este módulo no se porta: se tira y se sustituye.** Son ligaduras JNI de una
biblioteca de terceros, y en Rust no hay JNI que valga. Lo único que se lleva es
`MotorWhisper.kt`, que no está en esta carpeta y que es donde vive todo lo que
merece la pena: la política de tandas de 20 segundos, la lista blanca de
idiomas, la estrategia de dos idiomas con adivinación y repesca, y el flujo de
descarga con reanudación. Eso son unas doscientas líneas de decisiones bien
razonadas que se reescriben en Rust en un rato.

**Y la decisión de fondo es de tamaño, no de código.** El proyecto de escritorio
ocupa hoy 2,4 MB en un fichero, y su propio informe del PixPin original presume
de no distribuir ni una dependencia nativa ni un solo modelo, frente a los
150 MB y 34 MB de modelos del original. Meter reconocimiento de voz cambia eso:

- Con **ONNX Runtime** en Rust (`ort`), el ejecutable pasa de 2,4 MB a unos
  20-30 MB solo por el tiempo de ejecución. Se puede repartir la DLL aparte,
  pero entonces ya no es un fichero.
- Con **whisper.cpp** vía `whisper-rs`, el tiempo de ejecución baja a unos pocos
  MB — es un decodificador dedicado en vez de un motor genérico— y los modelos
  GGUF de Whisper `tiny` rondan lo mismo que aquí. Para transcribir Whisper y
  nada más, es la opción proporcionada, y evita traer un motor de inferencia
  general para usar el 5 % de él.
- El modelo, se elija lo que se elija, **debe seguir descargándose bajo
  demanda**. Eso ya está bien resuelto en la versión Android y hay que copiar la
  idea entera, incluida la instalación manual del fichero para quien no quiera
  que la aplicación se conecte.

**Mi recomendación es que esto no entre en la primera versión de escritorio.**
No porque no funcione, sino porque es una función que multiplica por diez el
tamaño de reparto y que no tiene nada que ver con capturar pantalla, que es lo
que la aplicación es. Si entra algún día, que entre como **complemento
opcional** —una DLL o un ejecutable auxiliar que se baja aparte—, y con
whisper.cpp antes que con ONNX Runtime. Y si lo que se quiere es que un vídeo
capturado tenga subtítulos, conviene comprobar antes qué da el reconocimiento de
voz que ya trae Windows: es gratis, pesa cero y puede que baste.

---

## 3. `motormd` — un motor de Markdown escrito a mano

Ruta: `F:\THE FORGE\PIXPIN PC VERSION MAX\proyectos de referencia\PIXPIN_PRO_ANDROID\app\src\main\java\com\forge\pixpin\motormd\`

### 3.1 Qué hace

Es un motor de Markdown propio con un editor que enseña el resultado mientras se
escribe. La tesis está en `EditorVivo.kt:59-83`: lo que se ve es el resultado,
siempre. No hay vista previa porque no hay nada que previsualizar, y nunca se ve
una almohadilla, ni un asterisco, ni una barra, ni siquiera en el bloque donde
está el cursor. El disco guarda Markdown de verdad —de él viven la exportación,
el PDF y los pines— pero el editor no lo muestra nunca.

El mecanismo que lo consigue tiene cuatro piezas. `Trozos.kt` parte el documento
en bloques **por posiciones**, sin interpretarlos, de modo que se sabe qué rango
del texto original es cada bloque. `Vivo.kt` hace de puente: por cada bloque
convierte Markdown a texto limpio con marcas aparte, se edita eso, y al escribir
hace la vuelta con `Inline.kt` — y la ida y vuelta tiene una prueba que coge un
texto con estilos, lo escribe, lo vuelve a leer y comprueba que sale lo mismo.
En pantalla los estilos se aplican con una transformación visual cuyo mapeo de
posiciones **es la identidad**, que es el truco que evita el fallo clásico del
cursor desplazado en los editores que tapan las marcas. Y los bloques que no
tienen el cursor se pintan como texto compuesto; solo el enfocado es un campo de
edición. La justificación de no usar una biblioteca (`Markdown.kt:161-170`) es
doble: todo tiene que ser función del zoom del pin, y una biblioteca no deja
gobernar los tamaños; y sin Compose ni Android dentro se prueba en la JVM.

### 3.2 Sus piezas principales

| Fichero | Líneas | Qué es | ¿Puro? |
|---|---:|---|:-:|
| `EditorVivo.kt` | 833 | El editor: bloques, tabla editable, adornos, teclas | no |
| `MarkdownText.kt` | 687 | El compositor de solo lectura, con medios y enlaces | no |
| `Markdown.kt` | 660 | **El parser entero** y el modelo (`MarkdownBlock`, `InlineText`, `SpanKind`) | **sí** |
| `RejillaDeTabla.kt` | 638 | La rejilla de tablas sobre un `Layout` a medida | no |
| `BarraDeFormatoUi.kt` | 589 | La barra de formato y sus familias | no |
| `Tablas.kt` | 577 | Modelo de tablas: leer, escribir, fusionar, anclas | **sí** |
| `MarkdownEdit.kt` | 459 | Aritmética de índices al aplicar formato | **sí** |
| `Formulas.kt` | 399 | El lector de fórmulas | **sí** |
| `Vivo.kt` | 272 | Lógica de edición de bloques y posición del cursor | casi |
| `Inline.kt` | 248 | De texto con marcas a Markdown | **sí** |
| `MarkdownHtml.kt` | 232 | A HTML, para la exportación web | **sí** |
| `Menus.kt` | 206 | El árbol de menús de dos niveles | **sí** |
| `Paginado.kt` | 192 | Reparto en páginas A4 contando renglones | **sí** |
| `Bloques.kt` | 172 | Catálogo de 23 tipos de bloque con sus atajos | **sí** |
| `Trozos.kt` | 169 | Trocear el documento cubriéndolo entero y sin pisarse | **sí** |
| `FormulaUi.kt` | 153 | Pintar la fórmula | no |
| `Historial.kt` | 112 | Deshacer y rehacer, agrupando por palabra | **sí** |
| `Formato.kt` | 103 | Los formatos y su agrupación en islas | **sí** |
| `Tramos.kt` | 89 | Aplanar marcas solapadas en una partición sin solapes | **sí** |
| `Comandos.kt` | 74 | Detección de `/comando` | **sí** |
| `Adjuntos.kt` | 62 | Importar un fichero: copia, no enlaza | no |
| `TextoStore.kt` | 38 | Buzón en memoria entre el editor y el servicio del pin | casi |

### 3.3 Qué dialecto de Markdown soporta

En línea: `**negrita**` (y `__negrita__`, que diverge del uso de Telegram),
`*cursiva*` y `_cursiva_`, `~~tachado~~`, `` `código` `` (dentro no se
interpreta nada), `||tapado||` como spoiler, `[texto](url)` y enlaces
automáticos con `https://`, `http://` y `www.`, recortando la puntuación final.
Las marcas se anidan.

En bloque: seis niveles de título, párrafo, viñetas con `-`, `*` o `+`,
numeradas con `1.` o `1)`, cita con `>` (las líneas seguidas se funden), bloque
de código con su lenguaje, regla horizontal, y **casillas de tarea** al estilo
GitHub. Añade una sintaxis propia de contenedores, `:::plegable Título` … `:::`,
con alias en castellano (`plegable`/`detalles`, `pie`/`nota`,
`destacado`/`resaltado`, `centro`, `derecha`), anidables. Y medios con
`![alt](ruta)`, clasificados por extensión en imagen, vídeo, audio o archivo.

Las **tablas** son el punto más divergente y el más trabajado. Hay dos
serializaciones: una tabla simple se guarda con las barras de Markdown, pero en
cuanto usa algo que Markdown no puede expresar —celdas fusionadas en horizontal
o vertical, alineación vertical, título, cabecera irregular— se guarda como
`<table>` de HTML, campo a campo. Lee las dos. El modelo es de anclas: las
celdas absorbidas por una fusión no existen en las filas, y `Tablas.rejilla()`
las despliega para pintar.

`Paginado.kt` no es sintaxis: es una capa de reparto en páginas A4 que cuenta
renglones estimados (46 por página, 78 caracteres por renglón), sin pintar, para
que sea determinista y comprobable en la JVM.

**Y sobre `Formulas.kt`, que era la duda: no es un motor de cálculo ni una hoja
de cálculo.** Es un lector tipográfico de un subconjunto de LaTeX, sin
evaluación ninguna. En todo el fichero no hay ni un `Double`, ni aritmética, ni
referencias a celdas, ni `=SUMA()`. La única función pública es
`Formulas.leer(formula: String): Pieza` (`Formulas.kt:55`), que devuelve un
árbol de composición —`Texto`, `Fila`, `Fraccion`, `ConIndices`, `Raiz`,
`Agrupado`— que `FormulaUi.kt` pinta con filas, columnas y un `Canvas`. Cubre
fracciones (`\frac` y también `/` infijo), super y subíndices, `\sqrt[n]{x}`,
delimitadores que crecen, unos 90 símbolos griegos y de operadores, funciones en
redonda, alias en lenguaje natural (`infinito`, `raiz`, `grados`) y digrafías de
teclado (`<=` a ≤, `!=` a ≠, `->` a →). Lo que no entiende, lo enseña tal cual.
La justificación de no traer KaTeX ni MathJax está escrita:
son megas de JavaScript que habría que meter en un navegador dentro de la app.

Quien sí evalúa fórmulas es `Formula` en `motor/Graficas.kt:258`, que está fuera
de este módulo y es lo que usa `croquis3d` para dibujar gráficas.

### 3.4 De qué depende

- **De Markdown, de nada.** No hay commonmark, ni flexmark, ni Markwon. El
  parser es propio de cabo a rabo.
- **kotlinx.serialization no se usa** en este módulo. Lo único de `kotlinx` es
  un `withTimeoutOrNull` de corrutinas en `RejillaDeTabla.kt:20`.
- **androidx.compose**, solo en los ficheros de interfaz: `foundation`
  (`BasicTextField`, `Layout`, `Canvas`, gestos), `material3` y los iconos
  (dependencia fuerte: unos 40 `import` en `BarraDeFormatoUi.kt`), `ui.text`
  (`AnnotatedString`, `SpanStyle`, `TextRange`, `VisualTransformation`) y
  `ui.input.key`.
- **Android puro**, en tres sitios contados: `BitmapFactory` en
  `MarkdownText.kt`, `Context`/`Uri` en `Adjuntos.kt`, y
  `EditorInfo`/`InputConnection` en `EditorVivo.kt` para interceptar el
  retroceso al principio de un bloque.
- **Del resto de la aplicación, nada en absoluto**: `grep` de
  `import com.forge.pixpin.` sobre los 22 ficheros devuelve **cero
  resultados**.

### 3.5 Qué hay del núcleo que sea puro

**Catorce de los 22 ficheros no tienen ni un `import` de Android ni de
androidx**, y suman **3.692 líneas, el 53 % del módulo**: `Markdown.kt`,
`Trozos.kt`, `Tramos.kt`, `Inline.kt`, `MarkdownEdit.kt`, `Tablas.kt`,
`Formulas.kt`, `Paginado.kt`, `MarkdownHtml.kt`, `Historial.kt`, `Bloques.kt`,
`Comandos.kt`, `Menus.kt` y `Formato.kt`. **El parser entero está en la parte
pura**, y con él el modelo, el serializador inverso, las tablas con sus
fusiones, el lector de fórmulas, la paginación, el deshacer y el exportador a
HTML.

Dos de los ocho «atados» lo están de mentira: `Vivo.kt` (272 líneas de lógica de
edición de bloques) importa **una sola cosa**, `TextRange` de Compose, que es un
par de enteros; y `TextoStore.kt` (38) importa solo `mutableStateMapOf`, que es
un mapa observable. Sustituyendo esos dos tipos, el trozo portable sube a
**4.002 líneas, el 57 %**, y el trabajo de interfaz se concentra en unas 2.900
líneas.

Y aquí hay algo que no tiene `croquis3d`: **la frontera está vigilada por un
test**. `app/src/test/java/com/forge/pixpin/MotorMdSeparadoTest.kt` comprueba que
la carpeta existe y tiene al menos 15 ficheros, que **ningún fichero del motor
importa nada del resto de la aplicación** (ni pines, ni capturas, ni proyectos,
ni el motor de dibujo), y mantiene a mano la lista blanca de los ocho ficheros
autorizados a tocar Android, con una nota que explica por qué se enumeran a
mano: añadir uno obliga a pararse a pensar si de verdad hace falta. Su
documentación dice literalmente que el día que haga falta usarlo en otro sitio
se lleva la carpeta y ya. **Esa promesa está comprobada mecánicamente, no
prometida.**

Los tests son 3.156 líneas en 16 ficheros —`TablasTest.kt` 437,
`MarkdownTest.kt` 350, `FormulasTest.kt` 240, `VivoTest.kt` 240,
`FormatoTest.kt` 238, `TramosTest.kt` 206, `PaginadoTest.kt` 182— y dos de ellos
comprueban invariantes de ida y vuelta que hay que replicar en Rust: que
escribir un texto con marcas y volverlo a leer da lo mismo, y que interpretar
cada trozo por separado da lo mismo que interpretar el documento entero.

### 3.6 Qué depende de gestos táctiles

Poco, y bien acotado. Solo un fichero tiene gestos de verdad.

| Gesto | Dónde | Qué hace | Qué necesita en PC |
|---|---|---|---|
| Pulsación larga y arrastre | `RejillaDeTabla.kt:161-214` | Marca un rango de celdas. Se lee en la pasada inicial sin consumir, porque el campo de texto de la celda se queda el toque para poner su cursor; espera medio segundo con el dedo quieto, con el margen de movimiento triplicado porque un dedo aguantando se mueve solo; vibra al confirmar y luego resuelve qué celda hay bajo el dedo | Botón izquierdo y arrastre, **directo**: se cae el temporizador, el margen y toda la maquinaria de la pasada inicial. Añadir Mayús+clic para extender |
| Toque y doble toque | `RejillaDeTabla.kt:246-252` | Por celda, no en la rejilla, y desactivado en la celda que se está escribiendo | Igual, con clic y doble clic |
| Menú por pulsación larga | `RejillaDeTabla.kt:522` | Abre `MenuDeTabla` | **Clic derecho**. El menú ya existe como pieza reutilizable |
| Desplazamiento horizontal | `RejillaDeTabla.kt:150` | Tablas más anchas que la ventana | Mayús+rueda y una barra visible |
| Vibración al confirmar | `RejillaDeTabla.kt:205` | Aviso háptico | Sin equivalente: un realce visual |

Dos cosas más que conviene saber. **No hay pellizco para hacer zoom en ningún
sitio del módulo**: no aparece `detectTransformGestures` ni `transformable` en
los 22 ficheros. El zoom se resuelve fuera y entra como número —`baseSizeSp`
llega ya multiplicado por el zoom del pin, y la rejilla recibe además una
`escala` que encoge márgenes y bordes junto con la letra. Eso es una noticia
excelente para el port: **el zoom ya es un parámetro, no un gesto**, y en
Windows se alimenta igual desde Ctrl+rueda.

Y no hay `SelectionContainer` en ningún fichero: la selección de texto la aporta
el campo de edición de Compose en el bloque enfocado, y el módulo la consume
como un rango de posiciones. En modo solo lectura **el texto no se puede
seleccionar**; solo hay detección de enlaces y de spoilers. En un escritorio eso
es una carencia que hay que cubrir: seleccionar y copiar de un documento en
lectura es lo mínimo. Falta también, por la misma razón, navegar entre celdas
con tabulador y flechas: en táctil se toca la celda y ya.

### 3.7 Valoración

**Este es, con diferencia, el módulo que mejor se porta de los tres, y el que
más claramente merece la pena.**

Las 4.002 líneas portables son `data class`, `sealed interface` y funciones
sobre cadenas y enteros: en Rust son `struct`, `enum` y funciones sobre `&str` y
`usize`, y la traducción es casi mecánica. No hay estado global, no hay
corrutinas en la parte pura, no hay reflexión, no hay serialización binaria.
Los 3.156 líneas de test se traducen con el mismo esfuerzo y sirven de red: si
pasan, está portado.

Lo que hay que rehacer son unas 2.900 líneas de interfaz, y una parte de eso es
trabajo que en Windows **desaparece**: el mecanismo de leer el gesto en la
pasada inicial con temporizador y margen triplicado existe solo porque el táctil
lo exige; con ratón, un `mousedown` y arrastrar bastan. La interceptación del
método de entrada para el retroceso al principio de bloque es un apaño puramente
Android que en Windows no hace falta. Lo que sí hay que **añadir** es lo que el
móvil no necesitaba: selección de texto en lectura, navegación de tabla con
teclado, y un juego de atajos de teclado de verdad.

Un aviso sobre el alcance. Este dialecto de Markdown no es Markdown estándar:
las cajas `:::`, las tablas que se van a HTML cuando hace falta, las fórmulas
propias y los medios clasificados por extensión son extensiones de la casa. Si
el objetivo es que un `.pixpin` se abra igual en el teléfono y en el PC —y el
formato existe precisamente para eso, según `docs/formato-pixpin.md`— **hay que
portar el dialecto entero, no un Markdown cualquiera**. Tirar de una biblioteca
de Rust (`pulldown-cmark`, `comrak`) para ahorrarse el parser sería un error:
leería mal los documentos que ya existen y no sabría escribirlos.

La única pregunta abierta es si un editor de notas cabe en el alcance de una
aplicación de capturas de escritorio. No es una duda sobre el módulo, que está
bien hecho; es sobre el producto. Si la respuesta es que sí, el orden razonable
es portar primero el parser y el compositor de solo lectura —con eso ya se
enseñan las notas de un `.pixpin` importado, y son la mitad de las líneas puras—
y dejar el editor para después.

---

## 4. Resumen para decidir

| | `croquis3d` | `onnx` | `motormd` |
|---|---|---|---|
| Líneas | 21.024 | 1.579 | 6.964 |
| Puro medido | 4.115 (19,6 %) | irrelevante: es JNI | 3.692 (53 %) |
| Puro contando lo casi puro | 7.625 (36 %) | — | 4.002 (57 %) |
| Frontera vigilada por un test | **no** | no | **sí** |
| Depende del resto de la app | sí, 70 `import` | no | **no, cero** |
| Tests que lo cubren | 5.138 líneas, 248 pruebas | 58 líneas | 3.156 líneas |
| Peso que añadiría al reparto | ninguno | **~25 MB de nativo + 104-375 MB de modelo** | ninguno |
| Gestos: dificultad del rediseño | **alta** (1/2/3/4 dedos) | ninguna | baja |
| Qué hay que tirar | el modo realidad, 553 líneas | todo el módulo | nada |
| Recomendación | **portar el núcleo y el visor; el editor, después y con dudas** | **no portar; si algún día, whisper.cpp aparte** | **portar, empezando por parser y lectura** |

Tres cosas que no estaban en el encargo y conviene tener a la vista:

1. **La documentación miente sobre el croquis.** El único diseño escrito
   (`2026-08-02-croquis-acotado-design.md`) describe un croquis plano acotado en
   metros que el código no implementa. Lo que hay es otra cosa. No usar ese
   documento como especificación.
2. **La proyección 3D ya está portada una vez**, a JavaScript, en
   `motor/VisorEspacio.kt` (1.063 líneas), con una prueba que ata las dos
   implementaciones. Es la mejor referencia disponible para el port a Rust, y
   demuestra que el núcleo se despega.
3. **`croquis3d` no es autónomo**: importa 70 veces del resto de la aplicación,
   sobre todo del motor de dibujo 2D. Portarlo obliga a portar antes un puñado
   de piezas de `motor` — los tipos de punto, el color, el evaluador de
   fórmulas y el formato de escena de Excalidraw.
