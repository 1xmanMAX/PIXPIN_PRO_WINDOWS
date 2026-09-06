# De PixPin Android a PixPin Max: qué hay, qué falta y qué cuesta

Este informe cruza la aplicación de Android del usuario contra la de Windows
para decidir en qué orden traer las cosas. Se apoya en tres inventarios por
módulo escritos el mismo día, que están en esta carpeta:

- `2026-09-06-android-motor.md` — el motor de dibujo.
- `2026-09-06-android-croquis-y-modelos.md` — croquis 3D, ONNX y Markdown.
- `2026-09-06-android-cuaderno-y-captura.md` — cuaderno, pines y captura.

Todo lo que aquí se afirma está medido con `find` y `wc -l` sobre el clon, o
leído del código. Donde no se ha podido comprobar, se dice.

## Los dos proyectos, en números

| | Android | Windows |
|---|---:|---:|
| Líneas | 117.228 Kotlin | 41.505 Rust |
| Ficheros de código | 242 | — |
| Pruebas | 151 ficheros | 548 pruebas |
| Motor de dibujo | 55.385 | 2.882 |
| Peso instalado | APK con 26 MB de nativo | 2,4 MB |

La diferencia real no es «tres veces»: es que **el motor de dibujo de Android
es diecinueve veces el nuestro**, y que hay una pata entera —el cuaderno— que
en Windows no existe en absoluto.

## Lo que cambia la forma del plan

### 1. El formato ya está pensado para esto

El `.pixpin` es un ZIP normal, y `docs/formato-pixpin.md` del propio proyecto
dice, con estas palabras: *«Es el formato con el que un proyecto se pasa de un
aparato a otro —o a la versión de escritorio— y se sigue editando allí.»*

Dentro van los lienzos en **JSON de Excalidraw plano**. En el teléfono se
guardan comprimidos, pero al empaquetar se descomprimen a propósito, para que
un editor de escritorio los lea sin más. El código y el documento coinciden:
esto está comprobado, no prometido.

### 2. Los dos modelos de elemento son el mismo modelo

Los tipos de Android usan **los nombres de Excalidraw tal cual** —
`rectangle`, `diamond`, `ellipse`, `arrow`, `line`, `freedraw`, `text`,
`image` — y lo propio va con prefijo: `pixpin-mosaic`, `pixpin-lupa`,
`pixpin-measure`, `pixpin-solid`, `pixpin-gantt`…

Nuestro `Elemento` de Rust tiene los mismos campos con los nombres
traducidos. De nuestras nueve figuras, **siete cruzan directamente**; solo el
resaltador y el foco son invención nuestra.

Consecuencia: el puente no es traducir dos formatos ajenos, es leer un
formato que ya se comparte en sus tres cuartas partes.

### 3. La frontera de lo portable está vigilada por pruebas del propio autor

`MotorSeparadoTest.kt` mantiene una lista blanca de los ficheros del motor
que pueden tocar Android, **con 36 nombres comentados uno por uno** diciendo
por qué. `MotorMdSeparadoTest.kt` hace lo mismo con el Markdown.

Eso es documentación de portabilidad escrita sin proponérselo, y es lo que
permite afirmar con números que **31.718 de las 55.385 líneas del motor se
portan tal cual**.

Un detalle: la lista está desfasada en un fichero. `Theme.kt` (207 líneas)
figura como capa de Android y ya no importa nada de Android — lee el
hexadecimal a mano a propósito. La prueba no lo detecta porque busca
fantasmas, no sobrantes. Para el port, `Theme.kt` es núcleo puro.

## Dónde la documentación no describe el código

Esto importa porque planificar desde documentos equivocados sale caro.

| Documento | Qué dice | Qué hay |
|---|---|---|
| `docs/motor.md` | Cuatro grupos de herramientas | Deja fuera ~17.000 líneas: PDF con lector y escritor propios, 3D isométrico, instrumentos matemáticos y salidas. **Subestima el módulo a la mitad.** |
| `croquis-acotado-design.md` | Croquis plano acotado en metros, con cotas y calibración | En `croquis3d` no existe nada de eso: hay trazos y láminas en 3D. **El documento no sirve de guía.** |
| Diseño del cuaderno | Room | Ni Room, ni SQLite, ni ORM. Todo es JSON sobre ficheros. |
| Comentarios de `Barra.kt` | «diecinueve herramientas» | `Tool` tiene **32 valores** |

## Las herramientas: 31 contra 11

Compartidas: seleccionar, mano, lápiz, resaltador, línea, flecha, rectángulo,
elipse, texto, foco, lupa y borrador.

| Familia | Lo que falta en Windows |
|---|---|
| Dibujar | rombo, imagen, **flecha libre** (a pulso, acaba en punta) |
| Tapar y señalar | mosaico, números de serie, hoja |
| Medir | **cota**, escalar, escala gráfica, nudo |
| Construir | **bote** (rellena el hueco que se toque), recortar, extender, punto |
| Seleccionar | lazo, **bolita** (se pasa por encima y va entrando) |
| Espacio | sólido, extruir |
| Otros | cronograma |

Las de medir y construir son las que más se echan de menos: son las que
convierten el anotador en una herramienta de trabajo y no en un rotulador.

## Qué se porta y qué no

### Se porta tal cual (lógica pura, sin Android)

| Qué | Líneas | Nota |
|---|---:|---|
| Núcleo del motor | 31.718 | Incluye `DrawController.kt` (3.482), la máquina de estados del gesto, sin un solo `import android` |
| `motormd` (Markdown) | ~3.700 | Cero imports del resto de la app, con su propia prueba de frontera |
| Formato `.pixpin` | — | ZIP + JSON, sin nada propietario |
| Cuaderno (datos) | — | JSON Lines, una línea por mensaje, legible desde Windows sin convertir |

### Hay que rehacer la entrada (la lógica vale, el gesto no)

`DrawCanvas.kt`, unas 1.700 líneas. Son tres funciones —bajar, mover,
levantar— más unas banderas que en Windows son `Shift` y `Alt`. El segundo
dedo que convierte la figura en exacta pasa a ser una tecla.

### Hay que repensarlo

- **La interfaz de Compose**, 11.549 líneas. Había que rehacerla igual.
- **El renderizador**, 3.824 líneas. Reescritura acotada: la geometría ya está
  fuera, en ficheros aparte.
- **El croquis 3D.** Sus gestos son un dedo orbitar, dos encuadrar, tres
  balancear, cuatro esconder la interfaz. Eso no se traduce a un ratón: se
  rediseña. Y solo el 19,6 % del módulo es puro; tiene 70 imports del resto
  de la aplicación.

### Se tira

- **La bola flotante** y la notificación permanente. Existen porque Android no
  tiene atajos globales — el propio diseño lo dice. En Windows ya los hay.
- **El rechazo de palma** y los atajos de lápiz más dedos.
- **`onnx`**: no es un módulo del proyecto, es `sherpa-onnx` copiado dentro.
  De sus 1.579 líneas, **1.226 son código muerto**. El APK carga 26 MB de
  bibliotecas nativas y los modelos van de 104 a 375 MB descargados.

## El reconocimiento de voz

Hay tres motores elegibles: **Vosk** por defecto, **Whisper** sobre
sherpa-onnx, y el reconocedor de Android en modo local. Todo corre en el
aparato; la red solo baja modelos.

Los tres tienen equivalente en Windows y lo único que habría que rehacer es
descodificar el audio a PCM. Pero antes de portar nada de esto conviene mirar
lo que ya trae el sistema: es la misma decisión que se tomó con el
reconocimiento de texto (`Windows.Media.Ocr` en vez de 34 MB de modelos) y
con el vídeo (Media Foundation en vez de FFmpeg). Meter 26 MB de nativo en un
programa de 2,4 MB por una función es multiplicar por diez su peso.

## Una pieza que ya se portó una vez

`motor/VisorEspacio.kt` son 1.063 líneas de **JavaScript** con la misma
proyección 3D que la cámara de Kotlin, y hay una prueba que compara las dos
implementaciones y exige que coincidan.

Es decir: esa proyección ya se ha escrito dos veces en dos lenguajes, y hay
una prueba que demuestra que son la misma. Para hacerla en Rust, eso es la
mejor referencia posible.

## Lo que no se ha comprobado

- No se ha ejecutado nada del Android: no hay Gradle ni SDK en este equipo.
  Las «936 pruebas en verde» que declara `docs/motor.md` no se han verificado;
  solo se ha contado que hay 151 ficheros de prueba de JVM.
- Tres ficheros muy grandes (`DrawEditorActivity.kt` 3.712 líneas,
  `PanelLateral.kt` 2.185 y otros) se clasificaron por sus imports y su
  cabecera, no leídos enteros. Puede haber lógica pura rescatable dentro.
- No se encontró código que reprograme las alarmas de recordatorio tras
  reiniciar el teléfono. Puede ser intencional; no está documentado en ningún
  sentido.
