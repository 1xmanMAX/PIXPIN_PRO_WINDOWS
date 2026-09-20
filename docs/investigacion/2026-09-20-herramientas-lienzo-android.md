# Las herramientas del lienzo 2D de PixPin Android, una a una

**Para qué es esto.** El usuario pidió portar «todas las herramientas del canvas 2D
que tengo en Android, que son varias» y repartirlas entre varios agentes. Este
documento es el inventario que hace falta antes de repartir: qué herramienta hay,
qué ficheros la implementan, **qué campos escribe en el `.excalidraw`** —que es lo
que viaja entre el móvil y el PC—, si el escritorio ya la tiene, cuánto valdría
portarla y con quién comparte ficheros.

**De dónde sale.** Del repositorio `1xmanMAX/PIXPIN_PRO_ANDROID` en su punta,
`0dc83acbd` («v0.72.0: llamada secreta, zoom propio de los documentos sin que la
tinta tiemble, imán del centro, texto en el editor rápido»). El encargo hablaba de
v0.66.x; la punta va ya cuatro versiones por delante y eso añade herramientas.
Del lado del PC, este árbol de trabajo en `ecdf8a2`.

**Lo que ya había.** `docs/investigacion/2026-09-06-android-motor.md` inventarió el
módulo entero y lo valoró por bloques de portabilidad. Aquí no se repite: esto va
herramienta por herramienta y, sobre todo, campo por campo, que es lo que aquel no
tenía y lo que hace falta para no romper el dibujo del otro aparato.

---

## 1. El puente: qué viaja de verdad entre los dos aparatos

Antes de las herramientas, el formato, porque de él dependen casi todas las
decisiones que vienen después.

### 1.1 El fichero

Un `.pixpin` es un ZIP y dentro lleva `lienzos/<id>.excalidraw` (documentado en
`docs/formato-pixpin.md` del Android). El nombre engaña: **ese fichero no es un
`ExcalidrawFile`, es la `Scene` de PixPin serializada tal cual**. Lo escribe
`PaquetePixpin.kt:107` con `ExcalidrawJson.encodeToString(conRutasLimpias)` sobre
una `Scene`, y lo relee `PaquetePixpin.kt:212` con
`decodeFromString<Scene>(json)`. El `ExcalidrawFile` de verdad —con `type`,
`version`, `appState` y las imágenes en `dataURL`— solo se arma cuando se exporta
a excalidraw.com, en `ExcalidrawStore.exportar` (`ExcalidrawStore.kt:152-175`).

La consecuencia práctica: el fichero que recibe el PC trae **claves de escena que
Excalidraw no tiene**, no solo campos raros dentro de los elementos.

### 1.2 Las claves de la escena (`Scene.kt:471-547`)

| Clave | Qué es | ¿De Excalidraw? |
|---|---|---|
| `elements` | la lista de elementos | sí |
| `files` | mapa `id → {id, mimeType, path, created}`; **`path`, no `dataURL`** | no, es propio |
| `viewport` | `{scrollX, scrollY, zoom}` | no |
| `style` | el `ItemStyle` activo del editor (`Scene.kt:190-245`) | no |
| `backgroundColor` | color del papel | equivalente a `appState.viewBackgroundColor` |
| `luces` | `{encendidas, fuerza}`, la llave de paso de las tintas de luz | no, propio |
| `escala` | `{unidadesPorPixel, unidad, decimales}`, la calibración de todas las cotas | no, propio |
| `tablas` | series de puntos por coordenadas (`Tablas.kt:27-39`) | no, propio |
| `origenCoordenadas` | el `(0,0)` del dibujo | no, propio |
| `referenciasVisibles` | si se ven las líneas de referencia | no, propio |
| `alfileres` | los clavos que atan figuras (`Nudos.kt:69-75`) | no, propio |
| `vista` | desde dónde se mira lo que está en volumen | no, propio |

El PC guarda todo esto **sin entenderlo**: `leer` mete el objeto entero menos
`elements` y `escala` en `Lienzo.resto` (`crates/pixpin-motor2d/src/excalidraw.rs:66-70`)
y `escribir` lo devuelve intacto (`excalidraw.rs:179`). La `escala` sí la
entiende y la traduce (`medida.rs:34-41`), y si viene en un formato que no
reconoce la guarda cruda en `escala_no_entendida` (`excalidraw.rs:142-149`) para
no destruirla. Esto está bien hecho y es la razón de que el puente funcione hoy.

### 1.3 Los campos del elemento (`Element.kt:549-1010`)

**De Excalidraw, que el PC ya entiende o al menos respeta:**
`id`, `type`, `x`, `y`, `width`, `height`, `angle`, `strokeColor`,
`backgroundColor`, `fillStyle`, `strokeWidth`, `strokeStyle`, `roughness`,
`opacity`, `roundness`, `seed`, `version`, `versionNonce`, `isDeleted`,
`groupIds`, `boundElements`, `updated`, `link`, `locked`, `points`, `pressures`,
`simulatePressure`, `lastCommittedPoint`, `startArrowhead`, `endArrowhead`,
`startBinding`, `endBinding`, `elbowed`, `text`, `fontSize`, `fontFamily`,
`textAlign`, `verticalAlign`, `containerId`, `name`, `fileId`, `scale`, `crop`.

**Propios de PixPin** (Excalidraw los ignoraría; el PC tiene que devolverlos):
`enlace` (`Element.kt:597`), `reference` (`:612`), `presionFirme` (`:628`),
`huecos` (`:644`), `negrita`/`cursiva`/`tachado` (`:686-688`),
`arcStart`/`arcSweep` (`:701-702`), `etiquetaAngulo`/`etiquetaRadio` (`:713-714`),
`unidad`/`pasoDeNumeros`/`pasoDeCuadros` (`:716-729`), `azimut`/`elevacion`
(`:731-739`), `altura` (`:756`), `cota` (`:769`), `giroEnPlanta` (`:783`),
`formaSolida` (`:793`), `planta` (`:807`), `inclinacion` (`:820`), `esqueleto`
(`:831`), `enElSuelo` (`:841`), `papel` (`:852`), `pauta` (`:855`), `tareas`
(`:859`), `periodos` (`:868`), `material` (`:872`), `mosaicBlur` (`:876`),
`foco` (`:887`), `aumento` (`:896`), `focoAncho`/`focoAlto` (`:906-907`),
`oscurecer` (`:914`), `lupaRedonda` (`:917`), `lupaFlecha` (`:919`), `guia`
(`:921`), `lupaDosLineas` (`:930`), `forma` (`:941`).

**Falsos amigos**, avisados aquí para que nadie los persiga: `MarcosMinimos.kt`
no es el marco/hoja sino marcos de rotación mínima para cintas 3D;
`CapaDeAnotacion.kt` no son capas sino un caché de qué hojas tienen algo dibujado;
`capa/CapaPantalla.kt` es la ventana que dibuja encima de toda la pantalla, no
capas de escena; y `DrawTablas.kt` no es la tabla pegada sino el editor de la tabla
de coordenadas. **No hay capas de documento en el lienzo**: el orden de pintado es
el orden de `Scene.elements` y lo único que agrupa es `groupIds`.

Y dos enumerados que son de Excalidraw **con un valor de más**: `fillStyle` gana
`pixpin-lines` (`Element.kt:383-392`) y `MaterialDeTinta` es entero propio
(`Element.kt:416-484`).

### 1.4 Los tipos de elemento

De Excalidraw: `rectangle`, `diamond`, `ellipse`, `arrow`, `line`, `freedraw`,
`text`, `image`, `frame`.

De PixPin, todos con prefijo a propósito para no chocar nunca con un tipo que el
original añada (`Element.kt:17-25`): `pixpin-mosaic`, `pixpin-spotlight`,
`pixpin-lupa`, `pixpin-serial`, `pixpin-measure`, `pixpin-arc`, `pixpin-region`,
`pixpin-scalebar`, `pixpin-point`, `pixpin-axes`, `pixpin-number-line`,
`pixpin-space`, `pixpin-solid`, `pixpin-gantt`.

El PC lee hoy diez de estos veintitrés: `rectangle`, `ellipse`, `line`, `arrow`,
`freedraw`, `text`, `image`, `frame`, `pixpin-measure`, `pixpin-scalebar`
(`excalidraw.rs:466-543`). El resto cae en `Entrada::Ajeno` y viaja intacto, que
es lo correcto mientras no se sepan dibujar. **`diamond` está entre los que caen**,
y es una de las diez figuras principales del original.

### 1.5 Las dos listas de la barra de Android (`DrawToolbar.kt`)

Fila principal (`MAIN_TOOLS`, `DrawToolbar.kt:1015-1026`):
`HAND, SELECTION, RECTANGLE, DIAMOND, ELLIPSE, ARROW, LINE, FREEDRAW, TEXT, ERASER`,
más `IMAGE` si el sitio la ofrece (`DrawToolbar.kt:366-368`).

Desplegable (`EXTRA_TOOLS`, `DrawToolbar.kt:1040-1066`):
`LASSO, ZONA, FLECHA_LIBRE, BOLITA, HIGHLIGHTER, RELLENO, RECORTAR, EXTENDER,
MOSAIC, LUPA, SPOTLIGHT, SERIAL, FRAME, SCALE, MEASURE, ESCALA_GRAFICA, NUDO,
PUNTO, SOLIDO, CRONOGRAMA, EXTRUIR, REVOLUCION`.

Treinta y dos herramientas. El PC tiene dieciséis (`gesto.rs:78-106`).

El panel de propiedades, para que el reparto sepa qué mandos hacen falta
(`DrawProperties.kt:18-88`, `enum Propiedad`): `TRAZO, FONDO, RELLENO, LINEA,
GROSOR, RUGOSIDAD, ESQUINAS, PUNTAS, FUENTE, OPACIDAD, MATERIAL,
ESTILO_DE_TEXTO, FORMA_FLECHA, VOLUMEN, MOSAICO, LUPA, OSCURECER, ZONA, PRESION`.
La tabla de qué mando aplica a qué tipo está en `propiedadesDeTipo`
(`DrawProperties.kt:169-303`) y merece portarse tal cual: es la misma tabla que
decide qué campos escribe `conEstilo` (`Scene.kt:920-959`).

---

## 2. Las herramientas, una a una

Para cada una: qué hace, con qué ficheros, qué campos guarda, cómo está el PC,
qué haría falta, valor y esfuerzo, y con quién comparte fichero.

### 2.1 Dibujar

#### Rectángulo · Elipse · Línea

Las tres de siempre: se arrastra una caja (o dos puntos) y nace la figura, con su
trazo, su relleno y su temblor. La línea se puede cerrar sobre sí misma y entonces
admite relleno (`isPathALoop`, `Element.kt:1148-1163`, umbral de 8 px de pantalla).

- **Ficheros**: `Shapes.kt` (418 l.; rectángulo `:47-59`, elipse `:75-82`, línea
  `:84-105`), `Rough.kt` (571 l.), `Scene.kt:618-678` (`newElement`).
- **Campos**: los genéricos de §1.3. `roundness` solo en rectángulo y línea
  (`Scene.kt:740-742`).
- **PC**: **sí**. `Figura::Rectangulo`/`Elipse`/`Linea` (`elemento.rs:62-73`),
  herramientas en `gesto.rs:83-86`.
- **Falta**: nada.
- Valor —, esfuerzo —.

#### Rombo (`diamond`)

La tercera figura de la fila principal de Android y de Excalidraw. Caja con
vértices en los puntos medios de los lados; admite esquinas redondeadas con radio
distinto por eje para que un rombo aplastado no se redondee mal
(`roundedDiamondPoints`, `Shapes.kt:308-351`).

- **Ficheros**: `Shapes.kt:61-73` y `:155-157`.
- **Campos**: idénticos al rectángulo. Ninguno nuevo.
- **PC**: **no**. No está en `Figura` (`elemento.rs:45-128`) ni en `Herramienta`
  (`gesto.rs:78-106`), y `elemento_desde` no lo lee (`excalidraw.rs:466-543`): un
  rombo del móvil entra como `Entrada::Ajeno` y **no se ve en pantalla** aunque
  sobreviva al guardar.
- **Falta**: una variante `Figura::Rombo`, su geometría en `formas.rs`, sus órdenes
  en `pintado.rs`, la lectura y escritura en `excalidraw.rs` y el botón. Es el
  cambio más pequeño de toda la lista.
- **Valor alto, esfuerzo bajo.** Comparte: `elemento.rs`, `formas.rs`,
  `pintado.rs`, `excalidraw.rs`, `gesto.rs`.

#### Arco

Se pone un óvalo guía translúcido —un transportador— y se repasa con el lápiz solo
el tramo que se quiere; queda guardado como «trozo de óvalo», no como puntos, así
que estirar el óvalo después reajusta el arco. El disparo está en
`DrawController.kt:288-300`: solo cuando el dedo baja sobre el borde de una guía,
con un margen mucho más ancho que el de picar.

- **Ficheros**: `Arco.kt` (172 l.): `anguloEnElOvalo` `:41-48`, `barridoAcumulado`
  `:61-67`, `puntosDelArco` `:93-111`, `cajaDelArco` `:129-163`.
- **Campos**: tipo `pixpin-arc`; `arcStart: Double?` y `arcSweep: Double?`
  (`Element.kt:701-702`), **propios**. `arcSweep == null` significa «todavía es
  solo la guía», y es un estado real que hay que respetar.
- **PC**: **no**.
- **Falta**: módulo nuevo `arco.rs` con esos cuatro cálculos, más el gesto de
  repasar la guía. Con ratón funciona igual de bien; el margen de agarre se puede
  estrechar.
- **Valor medio, esfuerzo medio.**

#### Flecha, puntas de flecha y enganche

La flecha recta con punta en uno o los dos extremos, que además **se ata a una
figura** y la sigue cuando esta se mueve.

- **Ficheros**: `Arrows.kt` (542 l.): `getHoveredElementForBinding` `:242-261`,
  `bindArrow` `:284-303`, `bindingPointOn` `:402-441`, `updateBoundPoints`
  `:371-393`, `getArrowheadPoints` `:116-174`, `direccionDelFinal` `:78-114`.
- **Campos**: `startArrowhead`/`endArrowhead` (enum de **ocho** puntas,
  `Element.kt:487-496`: `arrow, bar, circle, circle_outline, triangle,
  triangle_outline, diamond, diamond_outline`), `startBinding`/`endBinding`
  (`Binding{elementId, focus, gap, fixedPoint, mode}`, `Element.kt:990-1004`),
  `boundElements` en la figura contenedora, y `BindMode{orbit, inside}`
  (`Element.kt:964-978`). **Todos de Excalidraw**.
- **PC**: la flecha **sí** (`Figura::Flecha{puntos, punta_inicio, punta_fin}`,
  `elemento.rs:65-71`); las ocho puntas, **a medias**; el enganche, **no** —no hay
  `startBinding` ni `boundElements` en `Elemento` (`elemento.rs:142-208`).
- **Falta**: el enganche entero. Es la pieza que más se nota cuando falta, porque
  un organigrama del móvil abierto en el PC se descoloca en cuanto se mueve una
  caja. Ojo: hoy los bindings **sobreviven** al viaje (van en el JSON original,
  §5), pero quedan apuntando donde ya no toca.
- **Valor alto, esfuerzo alto.** Comparte con transformar/mover.

#### Flecha de codos

Conecta dos puntos con tramos en ángulo recto, con las esquinas redondeadas: el
conector de organigrama.

- **Ficheros**: `Elbow.kt` (131 l.): `elbowPoints` `:38-53`, `elbowRounded`
  `:73-107`, `puntosDeTrazado` `:127-131`. Constantes `ELBOW_RADIUS = 16.0` `:25`,
  `ALINEADO = 8.0` `:62`, `PASOS_CODO = 6` `:110`.
- **Campos**: `elbowed: Boolean` (`Element.kt:667`), **de Excalidraw**.
- **PC**: **no**.
- **Falta**: 131 líneas de geometría pura. De las traducciones más baratas que hay.
- **Valor medio, esfuerzo bajo.**

#### Flecha a mano alzada

Se traza a pulso como el lápiz y acaba en punta: la raya que rodea algo y lo
señala. Por dentro **es una flecha** (`ElementType.ARROW`) con todos los puntos
del trazo en vez de dos, así que se edita y se exporta por el mismo camino
(`Scene.kt:57-64`). La punta se orienta con una regresión sobre el último tramo
para que no tiemble (`Arrows.kt:78-114`).

- **Campos**: ninguno nuevo.
- **PC**: **no**.
- **Falta**: poco, una vez estén la flecha y el lápiz. Con ratón pierde gracia;
  con lápiz de tableta es igual de buena.
- **Valor medio, esfuerzo bajo.**

#### Lápiz (freedraw) con presión

El trazo a mano alzada, que no es una línea gruesa sino **el contorno cerrado de
una mancha**. Port literal de `perfect-freehand`.

- **Ficheros**: `Freehand.kt` (563 l.): `getStrokePoints` `:227-303`,
  `getStrokeOutlinePoints` `:310-444`, `strokeOptionsFor` `:152-162`.
  `AlisadoDeUnEuro.kt` (219 l.), filtro 1€ de Casiez, `UnEuro` `:57-149`,
  `AlisadoDelPuntero` `:178-219`. `EspinaDelTrazo.kt` (425 l.), spline
  Catmull-Rom centrípeta para el trazo vivo, `espinaCentripeta` `:77-128`.
- **Campos**: `points`, `pressures`, `simulatePressure`, `lastCommittedPoint` (de
  Excalidraw) y `presionFirme: Boolean` (`Element.kt:620-628`, **propio**: trazo de
  ancho constante, para escribir).
- **PC**: **sí y bien**. `Figura::Lapiz{puntos, presiones, opciones}`
  (`elemento.rs:48-56`), contorno en `tinta/freehand.rs` (453 l.) y
  `tinta/laser.rs` (287 l.), con oráculo de fidelidad en
  `crates/pixpin-motor2d/tests/oraculo_tinta.rs`.
- **Falta**: solo `presionFirme` como mando, y comprobar los valores del filtro 1€
  con ratón (los de Android están afinados en píxeles de pantalla táctil,
  `AlisadoDeUnEuro.kt:49-56`).
- **Valor bajo, esfuerzo bajo.**

#### Marcador / subrayador

El lápiz con otro ajuste: más ancho y semitransparente. No es un tipo de elemento
aparte, es un `freedraw` con `opacity = 40` (`DrawController.kt:402-404`,
`HIGHLIGHTER_OPACITY = 40` en `:3453`) y ancho multiplicado por
`ENGORDE_DEL_MARCADOR = 5.0` (`Scene.kt:284`).

- **PC**: **sí**. `Figura::Resaltador` (`elemento.rs:59-61`), y al escribir sale
  como `freedraw` con su opacidad (`excalidraw.rs:535-537`), que es exactamente lo
  que hace el móvil. Compatible sin más.
- **Valor —, esfuerzo —.**

#### Materiales de tinta

Cambia **cómo se pinta** el trazo sin tocar su geometría. Diez acabados: lisa, luz
(resplandor en tres pasadas), HDR (suma luz en vez de tapar), rayado, cruzado,
puntos, tiza, lápiz 2B, rotulador seco y trama gruesa. En un plano el color solo
no basta para decir qué es una cosa: una sección se raya, una sombra se cruza, un
terreno se puntea.

- **Ficheros**: `Element.kt:416-484` (el enum entero, con `esPorosa` `:480` y
  `alumbra` `:483`), y la llave de paso global `LucesDelDibujo`
  (`Scene.kt:425-460`).
- **Campos**: `material` en el elemento (`Element.kt:871-872`) y en `ItemStyle`
  (`Scene.kt:205`); `luces` en la escena. **Todo propio de PixPin.** Valores
  serializados: `lisa, luz, hdr, rayado, cruzado, puntos, tiza, lapiz2b, seco,
  trama`.
- **PC**: **no**. Solo existe `Variabilidad{Variable, Constante}`
  (`tinta/mod.rs:33-39`), que es otra cosa.
- **Falta**: es trabajo de pintado, no de motor: diez recetas de trazo sobre
  Direct2D. Lo que es barato es **respetar el campo** (ya se hace, §5); lo caro es
  dibujarlo. Se puede portar por tandas: primero `rayado`, `cruzado` y `puntos`,
  que son los del plano; después las porosas; las de luz al final.
- **Valor medio, esfuerzo alto.**

#### Tramas de relleno

Cinco: `hachure`, `cross-hatch`, `solid`, `zigzag` de Excalidraw, más
`pixpin-lines` —rayas a tiralíneas, rectas y sin temblor, porque en la sección de
un plano el rayado tembloroso «se lee como suciedad» (`Element.kt:383-392`).

- **Ficheros**: `Element.kt:376-393`, motor de barrido en `Rough.kt:397-450` y
  `:508-569`.
- **Campos**: `fillStyle`; el valor `pixpin-lines` es **propio**.
- **PC**: **a medias**. `EstiloRelleno{Solido, Rayado, Cruzado}`
  (`relleno.rs:50-55`). Faltan `zigzag` y `pixpin-lines`. Bien resuelto el
  respeto: un `fillStyle` desconocido se lee como el de por omisión pero **no se
  reescribe** mientras no se toque (`excalidraw.rs:635-649`).
- **Valor medio, esfuerzo bajo.**

#### El temblor (`roughness`)

Port de rough.js: cada trazo se dibuja dos veces con ruido y curvas Bézier. Tres
niveles: `0` arquitecto (sin temblor), `1` artista, `2` caricaturista
(`Element.kt:954-956`). Android cambió el de fábrica a **0** el 19-sep-2026 a
petición del usuario (`Scene.kt:201-202`).

- **Ficheros**: `Rough.kt` (571 l.). El orden de las llamadas al generador
  pseudoaleatorio **es parte del algoritmo** (`Rough.kt:21-25`): reordenar el
  código cambia el dibujo para la misma semilla.
- **Campos**: `roughness` y `seed`, de Excalidraw. `seed` es el campo más
  importante del modelo (`Element.kt:24-32`): sin él la figura tiembla en cada
  repintado.
- **PC**: **sí**, `rugosidad` en `Elemento` (`elemento.rs:172`) y `semilla`
  (`:177`); el PC incluso protege el `seed` al escribir, recortándolo a 31 bits
  porque el móvil lo lee como `Int` (`excalidraw.rs:677`).
- **Falta**: comprobar que el de fábrica del PC también es 0, para que un dibujo
  hecho aquí no salga tembloroso allí.
- **Valor bajo, esfuerzo bajo.**

#### Texto suelto y texto dentro de figura

El texto libre, y el texto **atado** a un rectángulo, elipse o rombo: se centra
solo, se parte en líneas y **hace crecer la caja** cuando no cabe.

- **Ficheros**: `TextoEnFiguras.kt` (211 l.): `anchoQueCabe`/`altoQueCabe`
  `:69-84` (rectángulo `ancho−2·relleno`, elipse `ancho/2·√2−2·relleno`, rombo
  `ancho/2−2·relleno`), `figuraQueLoContiene` `:94-102`, `sitioDelTextoDentro`
  `:115-122`, `repartirEnLineas` `:135-179`. `DrawFonts.kt` (116 l.): tres fuentes
  OFL (Excalifont, Nunito, Comic Shanns), `medirTexto` `:79-108` mide avance **y**
  caja de tinta porque en Excalifont los rabos se salen del avance.
- **Campos**: `text`, `fontSize`, `fontFamily`, `textAlign`, `verticalAlign`,
  `containerId`, `boundElements` (Excalidraw); `negrita`, `cursiva`, `tachado`
  (`Element.kt:686-688`, **propios**, y el propio código avisa de que se pierden al
  exportar a `.excalidraw`). Los números de familia no son correlativos y **van en
  el fichero**: `5` Excalifont, `6` Nunito, `8` Comic Shanns, con `1/2/3` como
  alias viejos (`Scene.kt:299-320`).
- **PC**: el texto suelto **sí** (`Figura::Texto{texto, tam, familia}`,
  `elemento.rs:80-85`, edición en `texto.rs`, 391 l.); el texto dentro de figura,
  **no** (no hay `containerId`); negrita/cursiva/tachado, **no**.
- **Falta**: el contenedor es lo valioso —una caja con su rótulo es la mitad de un
  diagrama— y son 211 líneas de geometría. Ojo con `fontFamily`: si el PC escribe
  el nombre de una fuente de Windows en vez de uno de esos números, el móvil
  reabre el texto con otra letra.
- **Valor alto, esfuerzo medio.**

#### Goma y sus modos

Se pasa por encima y borra **el elemento entero** —o el grupo entero— que toque; no
es un borrador de píxeles. Borrado blando (`isDeleted = true`), recuperable.

- **Ficheros**: `Scene.kt:1066-1110`: `eraseAt` `:1075-1092`,
  `intocableParaElBorrador` `:1104-1107`.
- **Los modos**, que son tres reglas y no un selector: nunca borra lo **bloqueado**;
  nunca borra **imágenes** (se meten para dibujar encima); nunca borra los
  **instrumentos** (plano, recta, espacio) ni lo que tiene `enlace != null`. Y hay
  un **modo guía**, que se pasa como el parámetro `alcanza`: fuera de él no se
  pueden borrar las líneas de referencia; dentro de él **solo** se borran esas.
- **Campos**: `isDeleted`, `locked`, `reference`, `enlace`.
- **PC**: **a medias**. `Herramienta::Borrador` (`gesto.rs:92`) y respeto a
  `bloqueado` (`impacto.rs:21-27`), pero sin el modo guía ni la protección de
  imágenes.
- **Valor medio, esfuerzo bajo.**

#### Láser

**No existe en Android.** `Scene.kt:33` lo dice: «quedan fuera frame, embeddable,
láser y bote». En el PC hay `tinta/laser.rs` (287 l.), pero es el port del
algoritmo de contorno de grosor constante de `@excalidraw/laser-pointer`, usado
para el lápiz de ancho fijo (`tinta/mod.rs:85`), no un puntero que se desvanece.
Si se quiere el puntero láser de verdad, es una herramienta nueva de las dos
mitades, no un port. **Valor bajo, esfuerzo bajo**, y fuera del encargo.

#### Biblioteca de figuras

Guarda una selección como figura reutilizable y la estampa centrada donde se toca,
con ids y semillas nuevos y ya agrupada.

- **Ficheros**: `Biblioteca.kt` (222 l.): `FiguraGuardada` `:30-46`,
  `normalizarFigura` `:55-59`, `estampar` `:72-77`, `figuraDeLaSeleccion` `:85-94`,
  `figurasDeFabrica` `:125-132`. Panel en `DrawFiguras.kt` (513 l.).
- **Campos**: ninguno en el elemento; `FiguraGuardada` es un contenedor aparte, no
  viaja en el `.excalidraw`.
- **PC**: **no**.
- **Valor medio, esfuerzo bajo.**

### 2.2 Seleccionar, transformar, organizar

#### Selección rectangular

Se arrastra un marco y entra lo que cae dentro (`CONTAIN`) o lo que lo toca
(`OVERLAP`).

- **Ficheros**: `Collision.kt` (423 l.): `BoxSelectionMode` `:362`,
  `getElementsWithinSelection` `:365-383`.
- **Campos**: ninguno; la selección es estado de sesión.
- **PC**: **sí**. `Estado::Marquesina` (`gesto.rs:207-210`), `seleccion.rs` (238 l.).
- **Falta**: Shift para sumar a la selección.
- **Valor bajo, esfuerzo bajo.**

#### Lazo

Se traza un contorno a mano y entra lo que queda dentro o lo que el trazo toca.

- **Ficheros**: `Collision.kt:394-409` (`getElementsWithinLasso`).
- **PC**: **no**.
- **Falta**: quince líneas de punto-en-polígono más el trazo. En escritorio es la
  tecla `Q` de Excalidraw.
- **Valor medio, esfuerzo bajo.**

#### La bolita

Se pasa por encima y las figuras van entrando en la selección; volver a pasar por
una la saca. Nació para el dedo, porque un recuadro sobre un dibujo apretado coge
de más y de menos (`Scene.kt:76-89`).

- **PC**: **no**.
- **Falta**: con ratón el recuadro ya basta. **Portarla es opcional**; si se
  porta, mejor como modificador (Alt+arrastrar) que como botón.
- **Valor bajo, esfuerzo bajo.**

#### Mano y zoom

`Viewport{scrollX, scrollY, zoom}` (`Scene.kt:334-363`), con `MIN_ZOOM = 0.1` y
`MAX_ZOOM = 400.0` (`:348`, `:361`; subido de 30 por los planos importados).
`fitToContent` en `Scene.kt:1115-1133`.

- **PC**: **sí**. `Herramienta::Mano` (`gesto.rs:80`), `Camara` (`camara.rs:46`,
  límites en `:137-142`).
- **Falta**: comprobar que el tope de zoom del PC llega a 400 y no a 30, porque un
  plano importado necesita cientos de aumentos para leer un detalle.
- **Valor bajo, esfuerzo bajo.**

#### Tiradores y giro

Ocho tiradores más el de rotación, más los de puntas de raya, más seis propios del
sólido.

- **Ficheros**: `TransformHandles.kt` (444 l.): `HandleType` `:14-66`
  (`NW, NE, SW, SE, N, E, S, W, ROTATION, POINT_START, POINT_END, POINT_MID,
  POINT_ADD, SOLIDO_*`), `HandleSize{MOUSE=8, PEN=16, TOUCH=28}` `:116-120`
  —**ya hay un tamaño pensado para ratón**—, `getTransformHandles` `:249-260`,
  `getLinearHandles` `:269-313`, `hitHandle` `:432-444`.
  `Transform.kt` (655 l.): `resizeSingleElement` `:57-202`,
  `resizeMultipleElements` `:323-384`, `rotateSingleElement` `:394-412`,
  `SHIFT_LOCKING_ANGLE = π/12` `:20`.
- **Campos**: `x`, `y`, `width`, `height`, `angle`, `points`, `huecos`, `scale`.
  Al rotar se **limpian** `startBinding`/`endBinding` (`Transform.kt:409-410`).
- **PC**: **sí**. `tiradores.rs` (324 l.), `transformar.rs` (937 l.) con
  `escalar` `:72`, `girar` `:256`, `a_saltos` `:295`.
- **Falta**: los tiradores de puntas de raya (`POINT_START/END/MID/ADD`) y los
  modificadores Shift/Alt como en el móvil.
- **Valor medio, esfuerzo bajo.**

#### Voltear

`flipHorizontal`/`flipVertical`/`flip` (`Transform.kt:448-507`): invierte `angle`,
invierte el signo de `scale` en las imágenes e **intercambia las puntas de flecha**.

- **PC**: **a medias**. Solo ocurre como efecto de cruzar el ancla al escalar
  (`transformar.rs`, prueba `cruzar_el_ancla_voltea_en_vez_de_aplastar`). No hay
  acción explícita.
- **Valor medio, esfuerzo bajo.**

#### Rejilla

Fondo pautado en líneas o puntos. **No imanta**, es andamio visual
(`Cuadricula.kt:16-21`). `PASO_DE_CUADRICULA = 20.0` (`:32`), el paso se dobla
cuando en pantalla mediría menos de 14 px (`:43-65`), tope de 400 líneas (`:88`).

- **Campos**: ninguno del lado de PixPin. **Pero** `ExcalidrawAppState` sí tiene
  `gridSize` (`ExcalidrawStore.kt:233`) y el motor no lo usa.
- **PC**: **no**. Lo que en el PC se llama `Rejilla` (`indice.rs`, 418 l.) es un
  índice espacial para el picado, nada que ver.
- **Valor medio, esfuerzo bajo.**

#### Imán

Engancha el cursor a puntos notables mientras se dibuja, se mueve o se afina.

- **Ficheros**: `Snapping.kt` (350 l.): `TipoAnclaje{ESQUINA, MEDIO, CENTRO,
  EXTREMO, EJE, INTERSECCION, BORDE}` `:25-68`, `buscarAnclaje` `:217-323`,
  prioridad en `:329-341` (intersección → esquina/extremo/eje → medio → centro →
  borde). `Iman.kt` (170 l.) es la fachada: `Faena{TRAZANDO, A_MANO, AFINANDO,
  MOVIENDO, SITIO_NOTABLE}` `:55-85`, `sitio()` `:94-120`.
- **PC**: **a medias, y muy cerca**. `enganche.rs` (876 l.) tiene
  `TipoAnclaje{Esquina, Extremo, Medio, Centro}` (`:29-39`) y
  `Faena{Trazando, AMano, Afinando, Moviendo}` (`:58-67`). **Faltan tres anclajes**
  —`EJE`, `INTERSECCION`, `BORDE`— y la faena `SITIO_NOTABLE`. La intersección es
  la de mayor prioridad en el móvil y es justo la que hace falta para recortar y
  para plantar puntos.
- **Valor alto, esfuerzo medio.**

#### Alinear y distribuir

`Organize.kt:196-310`: `AlignAxis{X,Y}`, `AlignPosition{START, CENTER, END}`,
`alignElements` `:206-240` (mueve **grupos enteros**), `distributeElements`
`:254-299` (por huecos iguales, o por centros si no caben).

- **PC**: **sí**. `organizar.rs`: `Alineacion` `:19`, `Reparto` `:29`,
  `alinear` `:152`, `repartir` `:185`.
- **Valor —, esfuerzo —.**

#### Agrupar y orden Z

`groupIds` es una **pila**: el último id es el grupo más exterior; desagrupar quita
solo el más exterior (`Organize.kt:126-159`). El orden Z **es el orden del array**,
no hay campo `z`; `toContiguousGroups` `:23-34` mueve selecciones salteadas como
bloques.

- **PC**: **sí** las dos. `organizar::agrupar`/`desagrupar` (`organizar.rs:36-60`),
  z en `escena.rs:90-112`, UI de capas en `panel_lateral.rs:60-65`.
- **Valor —, esfuerzo —.**

#### Bloquear

`toggleLock` (`Scene.kt:1057-1064`): con mezcla, bloquea todo. Lo bloqueado no se
pica, no se mueve, no se estira y el borrador no se lo lleva.

- **PC**: **sí**. `bloqueado` (`elemento.rs:196`), `escena::bloquear`
  (`escena.rs:571`), respeto en `impacto.rs:21-27`. Commit `e794b2c`.
- **Valor —, esfuerzo —.**

#### Deshacer y rehacer

`History.kt` (133 l.): pila de **deltas**, no de copias, con tope de 100
(`:92-133`).

- **PC**: **sí**, y con otro diseño: pasos abiertos y cerrados sobre la escena
  (`escena.rs:293`, `:361`, `:399`, `:404`) y presupuesto de memoria
  (`bytes_de_historial`, `:501`).
- **Valor —, esfuerzo —.**

#### Copiar estilo y cuentagotas

**No existen en Android.** Se buscó en todo el motor descargado (incluido
`PaletaDeColores.kt`, 431 l.) sin resultado. En el PC hay algo parecido: la lupa
hace de cuentagotas del catálogo de colores (`crates/pixpin-ui/src/lupa.rs:2`),
pero es cuentagotas de pantalla, no «copiar el estilo de aquel elemento».

Si se quieren, son **invención de escritorio**, no port. El molde ya está: la
tabla `propiedadesDeTipo` (`DrawProperties.kt:169-303`) dice exactamente qué
campos copiar para cada tipo. **Valor alto, esfuerzo bajo** —y es de las cosas que
en escritorio, con ratón y teclado, más se agradecen.

#### Alfileres y nudos

Un clavo que atraviesa dos o más figuras en un punto: ya no se pueden separar. Con
un clavo giran una respecto de otra; con dos, quedan fijas.

- **Ficheros**: `Nudos.kt` (548 l.): `Agarre` `:40-65` (por índice de vértice, por
  proporción de caja, o por fracción de recorrido), `Alfiler` `:69-75`,
  `clavarEn` `:159-188`, `Libertad{LIBRE, GIRA, FIJA}` `:251-258`,
  `arrastrarConAlfileres` `:289-319`, `fijarAlfileres` `:454-495`.
- **Campos**: **no van en el elemento**, van en `Scene.alfileres`
  (`Scene.kt:530`). Enteramente propios de PixPin.
- **PC**: **no**, y hoy la clave `alfileres` viaja intacta dentro de `resto`.
- **Valor bajo, esfuerzo alto.** Es la herramienta más de nicho de la lista.

### 2.3 Medir

#### Cota

Una raya que dice cuánto mide lo que cruza. Su rótulo **no se guarda**: se calcula
al pintar a partir de su largo y de la escala de la escena, así que una cota nunca
puede mentir (`Element.kt:68-79`).

- **Ficheros**: `Medida.kt` (264 l.).
- **Campos**: tipo `pixpin-measure`, `points`, `fontSize`, `fontFamily` (la cota
  lleva letra aunque no lleve texto, `Scene.kt:700-707`). La escala va **en la
  escena**, no en la cota.
- **PC**: **sí**, y es el único tipo propio de PixPin que el PC sabe leer y
  escribir (`excalidraw.rs:527-540`, commits `ecd80f9` y `4a04616`).
  `Figura::Cota{puntos}` (`elemento.rs:101-103`), `medida.rs` (503 l.),
  `apps/pixpin/src/ventana_editor/medir.rs` (808 l.).
- **Valor —, esfuerzo —.**

#### Escalar (calibrar)

Se arrastra sobre algo de medida conocida y se dicta cuánto mide; es lo que da
unidades a todas las cotas. La raya no se guarda.

- **Campos**: `escala` en la escena: `{unidadesPorPixel, unidad, decimales}`
  (`Medida.kt:37-44`). El PC tiene la misma estructura, campo por campo
  (`medida.rs:34-41`).
- **PC**: **sí**. `Herramienta::Escalar` (`gesto.rs:97`),
  `Peticion::Calibrar{largo_px}` (`gesto.rs:189`).
- **Valor —, esfuerzo —.**

#### Escala gráfica

La reglita a cuadros de los planos, que no puede mentir porque se encoge con el
dibujo. Elige un paso redondo —1, 2 o 5 por una potencia de diez— y pone los
cuadros que quepan.

- **Ficheros**: `EscalaGrafica.kt` (129 l.).
- **Campos**: tipo `pixpin-scalebar`; lo que enseña sale de su ancho y de la
  escala de la escena. Sin campos propios.
- **PC**: **sí**. `Figura::EscalaGrafica` (`elemento.rs:111`), `escalabarra.rs`
  (261 l.).
- **Valor —, esfuerzo —.**

#### Ángulos

Mientras se arrastra un vértice, enseña el ángulo interno de la esquina que se está
tocando. **No es persistente**: desaparece al soltar, no crea elemento y no se
serializa nunca.

- **Ficheros**: `Angulos.kt` (145 l.): `AnguloInterno` `:25`, `angulosInternos`
  `:45`.
- **Campos**: ninguno.
- **PC**: **no**.
- **Valor medio, esfuerzo bajo.**

#### Perímetros — *no es una herramienta, es la pieza de la que cuelgan tres*

Conviene no confundirse: `Perimetros.kt` (520 l.) no rotula áreas, **reduce
cualquier figura a tramos rectos** y con eso alimenta al imán, al bote y al
recorte.

- **Ficheros**: `contornosDe` `:63` (un `when` exhaustivo por tipo),
  `interseccionesCerca` `:285`, `puntoEnElPerimetro` `:450` (la escuadra: arrastrar
  pegado al borde de una figura). El cálculo de área vive aparte, en
  `Regiones.kt:676`.
- **PC**: **no**, y **hace falta antes que el bote, el recorte y el anclaje de
  intersección**. Por eso va en la tanda cero (§4.2), no en ningún grupo.
- **Valor alto (como cimiento), esfuerzo medio.**

### 2.4 Tapar y señalar

#### Mosaico

Tapa lo que hay debajo, pixelado o desenfocado.

- **Campos**: tipo `pixpin-mosaic`, caja normal, más `mosaicBlur: Boolean`
  (`Element.kt:876-877`, **propio**; también en `ItemStyle`, `Scene.kt:210-214`).
- **PC**: **no**. Entra como ajeno y **no se ve**, que en una captura anotada
  significa que el dato tapado **se ve en el PC**. Es el riesgo más feo de toda la
  lista: no se pierde el elemento, pero deja de cumplir su función.
- **Falta**: leer la caja y pintar encima el pixelado o el desenfoque. El pintado
  ya sabe hacer velos (`Orden::Velo`, `pintado.rs:50-55`).
- **Valor alto, esfuerzo medio.**

#### Foco (spotlight)

Oscurece todo **menos** su zona. Se saca «con varita»: se toca una figura cerrada
ya dibujada y esa figura pasa a ser el foco (`Lupa.kt:435`, `focoDesdeFigura`).

*Contradicción del código, sin resolver*: el comentario de `Element.kt:37-48` dice
que ya no se puede crear porque lo sustituyó la lupa, pero `Tool.SPOTLIGHT` sigue
vivo en la barra (`DrawToolbar.kt:1054`) y en el controlador
(`DrawController.kt:302-305`). Se porta como si se pudiera crear.

- **Ficheros**: `Lupa.kt`: `focoDesdeFigura` `:435`, `oscurecimientoDe` `:523`,
  `conZona`/`zonaDe` `:541`/`:551`.
- **Campos**: tipo `pixpin-spotlight`, más `oscurecer: Int?` (10-90 %,
  `Element.kt:914`), **propio**. Comparte con la lupa `foco`, `focoAncho`,
  `focoAlto`, `lupaRedonda` y `forma`. Ojo: **`zona` no es un campo del elemento**
  —solo vive en `ItemStyle` (`Scene.kt:228`) como ajuste de fábrica— y en el
  elemento se deriva de `focoAncho` y la caja (`zonaDe`).
- **PC**: **a medias, y mal**. Existe `Figura::Foco{elipse}` (`elemento.rs:76-79`)
  y `Herramienta::Foco` (`gesto.rs:89`), pero **al escribir sale como
  `"rectangle"`** (`excalidraw.rs:243`): un foco hecho en el PC llega al móvil
  como un rectángulo cualquiera. Y un `pixpin-spotlight` del móvil entra como
  ajeno, así que tampoco se ve aquí.
- **Falta**: mapear los dos sentidos y leer `oscurecer`.
- **Valor alto, esfuerzo bajo.** Es un fallo de compatibilidad, no una herramienta
  nueva.

#### Lupa

Un trozo del dibujo enseñado en grande **en otro sitio**. Son dos rectángulos: la
caja del elemento es el cristal, y `foco` es a dónde mira, **en coordenadas
absolutas del dibujo**, que es lo que permite apartar el resultado sin mover lo
que se está mirando (`Element.kt:50-66`).

- **Ficheros**: `Lupa.kt` (566 l.): `aumentoDe` `:62`, `focoDe` `:106`,
  `regionDeLaLupa` `:119`, `tocaElFoco` `:141`, `GuiaDeLupa{NINGUNA, FLECHA,
  DOS_LINEAS, PUNTO}` `:181-194`, `lupaDesdeFigura` `:424`.
- **Campos**, todos **propios**: `foco: Pt?` (`Element.kt:887`),
  `aumento: Double?` (`:896`), `focoAncho`/`focoAlto` (`:906-907`, se guardan, no
  se calculan), `oscurecer` (`:914`), `lupaRedonda` (`:917`), `lupaFlecha`
  (`:919`), `guia` (`:921`), `lupaDosLineas` (`:930`), `forma: List<Pt>?` (`:941`,
  el contorno del cristal en proporción de su caja).
- **PC**: **a medias, y del todo distinta**. `Herramienta::Lupa` en el PC es una
  **vista** que amplía alrededor del cursor y **no deja rastro** (`gesto.rs:90-91`,
  `deja_rastro()` la excluye en `:117-122`). La del móvil es un elemento del
  dibujo. Son dos cosas con el mismo nombre.
- **Falta**: el elemento entero, con sus nueve campos. Hay que decidir antes si el
  PC renombra su lupa-vista para no confundir.
- **Valor medio, esfuerzo alto.**

#### Números de serie

Un círculo con un número dentro: 1, 2, 3. El número va en `text`, y el siguiente
se calcula mirando el mayor que hay en la escena y sumando uno —así borrar el 2 de
tres círculos deja el 4 como siguiente (`DrawController.kt:3513-3518`).

- **Campos**: tipo `pixpin-serial`, `text`, y el radio sale de `fontSize`
  (`SERIAL_RADIUS = 0.9`, `DrawController.kt:3459`). Sin campos nuevos.
- **PC**: **no**.
- **Valor alto, esfuerzo bajo.** Para anotar una captura paso a paso es de lo más
  usado, y no necesita nada que el PC no tenga ya.

#### Marco y hoja

El marco delimita qué parte del lienzo infinito es «el dibujo». Con `papel` puesto
además se pinta como papel, con borde y sombra, y el lienzo empieza a parecer un
cuaderno. Es el mismo `frame` del original, con su nombre serializado.

- **Ficheros**: `Element.kt:129` (el tipo), `TamanoDePapel` `:269-283`,
  `PautaDeHoja` `:295-299`; la lógica está en `DrawController.kt:2886`
  (`anadirHoja`), `:2893` (`cambiarPapel`), `:2900` (`cambiarPauta`) y en
  `Cuaderno.kt` (`sitioDeLaHojaSiguiente` `:65`, `rayasDeLaPauta` `:101`).
- **Campos**: tipo `frame` (Excalidraw), `name` (Excalidraw, `:692`), más `papel:
  TamanoDePapel?` (`Element.kt:852`, valores `a4, a5, carta, cuadrada, apaisada`)
  y `pauta: PautaDeHoja` (`:855`, valores `lisa, rayada, cuadros, puntos`), los dos
  **propios**. Los hijos llevan `frameId` en Excalidraw; PixPin usa contención por
  caja (`Scene.kt:588-592`).
- **PC**: **a medias**. `Figura::Marco{nombre}` (`elemento.rs:119-122`),
  `marco.rs` (149 l.), commit `8535481`. Faltan `papel` y `pauta`.
- **Valor medio, esfuerzo bajo.**

#### Imagen

Foto en el lienzo. El `.pixpin` guarda en `files[<id>].path` una **ruta**, no un
`dataURL`, para que un plano de quince megas no se convierta en veinte de base64.

- **Campos**: `fileId` (`Element.kt:944`), `scale: List<Double>` (`:946`, el signo
  voltea), `crop: Crop{x, y, width, height, naturalWidth, naturalHeight}` (`:947`,
  definido en `:1008`) — los tres de Excalidraw; `enElSuelo: Boolean` (`:841`,
  **propio**: la imagen tumbada en el suelo isométrico). El `crop` **se pinta**
  (`Renderer.kt:2983`) pero ninguna herramienta de Android lo crea: solo llega de
  un `.excalidraw` importado de la web, y hay que devolverlo intacto.
- **Y de paso**: la *nota adhesiva* (`NotaAdhesiva.kt`, 148 l.) no es un tipo de
  elemento — se garabatea, se rasteriza a 3× (`alBitmap` `:116`) y se pega como una
  imagen normal. No hace falta portar nada del formato para que viaje.
- **PC**: **sí**. `Figura::Imagen{id_objeto}` (`elemento.rs:88-90`),
  `ficheros()` lee las rutas (`excalidraw.rs:831-843`), almacén en
  `apps/pixpin/src/imagenes_lienzo.rs` (386 l.), commits `9960dfa` y `10e09ca`.
- **Falta**: `crop` y `scale` negativo.
- **Valor medio, esfuerzo bajo.**

#### Zona

Se arrastra un rectángulo y sale una foto de lo que hay dentro, para arrastrarla a
otro sitio o mandarla al chat del proyecto como sublienzo. Lo pidió el usuario el
13-sep-2026 (`Scene.kt:43-49`). La marca que deja lleva `enlace`, se pinta con un
icono de enlace en la esquina y el borrador no se la lleva.

- **Campos**: `enlace: String?` (`Element.kt:1049` en el listado, **propio**).
- **PC**: **a medias**. El campo `enlace` sí está (`elemento.rs:202`) y el
  recuadro que lleva al sublienzo ya se pinta y se pulsa (commit `369c42b`), pero
  la herramienta de recortar la zona, no.
- **Valor medio, esfuerzo medio.** Depende del chat, que tiene otro dueño.

### 2.5 Construir

#### Bote de relleno

La única herramienta que no dibuja una figura nueva sino que mira las que ya hay:
busca hasta dónde llega el espacio tocado sin salirse y lo pinta. El hueco entre
tres líneas y media elipse no es de ninguna de las cuatro, y hasta que existió
esto había que repasarlo a mano.

- **Ficheros**: `Regiones.kt` (763 l.), `Region{contorno, huecos}` `:49-50`,
  anillos en `:239-265`. Disparo en `DrawController.kt:540`.
- **Campos**: tipo `pixpin-region`; `points` es el contorno que se **encontró** y
  `huecos: List<List<Pt>>` (`Element.kt:645`) son sus agujeros, los dos relativos a
  `(x,y)` y pintados por regla par/impar. **`huecos` es propio.** Se guarda lo
  encontrado y no una referencia a las figuras que lo encerraban, a propósito: lo
  que se rellenó se queda relleno.
- **PC**: **no**. Existe `relleno.rs` (377 l.) pero es el relleno **de una figura**,
  no de un hueco entre varias.
- **Valor alto, esfuerzo alto.** 763 líneas de barrido geométrico, pero es la
  herramienta que más distingue a PixPin de Excalidraw.

#### Recortar y extender

Recortar: se toca el trozo de raya que sobra y se va, hasta donde la cruzan las
demás; si estaba en medio, la raya se parte. Extender: se toca la punta que se
queda corta y llega hasta lo primero que topa.

- **Ficheros**: `Recorte.kt` (475 l.): `cortesDe` `:53-79`, `recortarLineal`
  `:114-137`, `recortarAnillo` `:148-173`, `recortarArco` `:183-250`,
  `extenderEn` `:330-368` (tope `ALCANCE_EXTENDER = 4000.0`, `:475`),
  `conPuntos` `:442-457`.
- **Campos**: `points`; recortar un rectángulo lo convierte en `line`
  (`Recorte.kt:165`), anula `roundness` y pone el fondo transparente (`:169-171`),
  y limpia los bindings (`:453-455`). En arcos toca `arcStart`/`arcSweep`.
- **PC**: **no**.
- **Valor alto, esfuerzo medio.** Con ratón es un clic; no pierde nada al pasar a
  escritorio. Necesita el anclaje de **intersección** del imán.

#### Puntos etiquetados

Un punto con su letra: A, B, C sobre el dibujo. Es la herramienta de las
matemáticas: un croquis de geometría se explica nombrando los puntos. Un toque,
imantado a la intersección o al vértice más cercano, y la letra sale sola
siguiendo la serie.

- **Ficheros**: `Puntos.kt` (313 l.), `SerieDePunto{MAYUSCULAS, MINUSCULAS,
  NUMEROS}` `:41-50`. Disparo en `DrawController.kt:499`.
- **Campos**: tipo `pixpin-point`; **la caja no tiene tamaño**, `x` e `y` son el
  punto; `text` es la letra; `etiquetaAngulo`/`etiquetaRadio` (`Element.kt:721-722`,
  **propios**) dicen dónde orbita la letra, en polares, para que al mover el punto
  la letra lo siga.
- **PC**: **no**.
- **Valor medio, esfuerzo bajo.** Necesita el anclaje de intersección.

#### Soldar vértices (nudo)

Se tocan los vértices que no se tienen que separar y a partir de ahí se mueven
juntos. Es la cara sencilla de los alfileres. Ver §2.2, alfileres.

### 2.6 Instrumentos matemáticos y volumen

Los agrupo porque comparten una idea: **no son dibujos, son instrumentos que saben
cuánto vale una unidad** y por eso se comportan raro al redimensionar —por un lado
aparecen más números, por una esquina los mismos más grandes.

| Instrumento | Ficheros | Tipo | Campos propios |
|---|---|---|---|
| Plano cartesiano | `Plano.kt` (332 l.) | `pixpin-axes` | `unidad`, `pasoDeNumeros`, `pasoDeCuadros` |
| Recta numérica | `Plano.kt` | `pixpin-number-line` | los mismos |
| Espacio de tres ejes | `Espacio.kt` (275 l.) | `pixpin-space` | los mismos más `azimut`, `elevacion` |
| Sólido 3D | `Solido.kt` (2.234 l.), `Proyeccion.kt` | `pixpin-solid` | `altura`, `cota`, `giroEnPlanta`, `formaSolida`, `planta`, `inclinacion`, `esqueleto` |
| Cronograma | `Cronograma.kt` (217 l.) | `pixpin-gantt` | `tareas: List<TareaDelCronograma>` (`Element.kt:243`: `nombre`, `desde`, `cuanto`, `color`), `periodos` |

`FormaDeSolido` (`Element.kt:303`) tiene seis valores: `caja`, `cuna`, `cilindro`,
`prisma`, `revolucion`, `extrusion`. Los dos botones que faltan de la barra
—`EXTRUIR` (levantar una figura cerrada) y `REVOLUCION` (torno)— trabajan sobre
`pixpin-solid`.

**La tabla pegada es otra cosa y es buena noticia**: `TablaDibujada.kt` (208 l.) y
`DrawTablaPegada.kt` (198 l.) no crean ningún tipo nuevo — convierten lo pegado del
portapapeles en `rectangle`, `line` y `text` normales de Excalidraw con un
`groupIds` común (`TablaDibujada.kt:77` y `:204`). Eso significa que **una tabla
del móvil ya se ve y se edita en el PC hoy**, y que portar «pegar una tabla» es
trabajo de portapapeles, no de formato. Ctrl+V desde Excel encajaría mejor en
escritorio que en Android. *(Lo que sí es propio y no viaja es la «tabla de
coordenadas», `Scene.tablas`, que es otra herramienta: plantar puntos por teclado.)*

- **PC**: **ninguno de los instrumentos**. Todos entran como ajenos y sobreviven.
- **Valor bajo, esfuerzo alto.** Son la parte del motor que menos se usa en un
  escritorio de anotación. **Recomiendo dejarlos fuera de esta tanda** y quedarse
  con que viajan intactos.

---

## 3. La tabla, ordenada por valor y esfuerzo

Primero lo que da más por menos.

| # | Herramienta | Valor | Esfuerzo | PC hoy | Ficheros nuevos del PC |
|--:|---|:--:|:--:|---|---|
| 1 | **Rombo** | alto | bajo | no | `formas.rs` (+arm) |
| 2 | **Foco: arreglar el puente** | alto | bajo | a medias, **escribe mal** | `excalidraw.rs` |
| 3 | **Números de serie** | alto | bajo | no | `serie.rs` |
| 4 | **Copiar estilo** (invención de escritorio) | alto | bajo | no | `estilo.rs` |
| 5 | Lazo | medio | bajo | no | `lazo.rs` |
| 6 | Flecha de codos | medio | bajo | no | `codo.rs` |
| 7 | Tramas `zigzag` y `pixpin-lines` | medio | bajo | a medias | `relleno.rs` |
| 8 | Voltear explícito | medio | bajo | a medias | `transformar.rs` |
| 9 | Rejilla visual y su imán | medio | bajo | no | `cuadricula.rs` |
| 10 | Marco: `papel` y `pauta` | medio | bajo | a medias | `marco.rs` |
| 11 | Puntos etiquetados | medio | bajo | no | `puntos.rs` |
| 12 | Biblioteca de figuras | medio | bajo | no | `biblioteca.rs` |
| 13 | Goma: modos y protecciones | medio | bajo | a medias | `gesto.rs` |
| 14 | **Mosaico** | alto | medio | no | `mosaico.rs` |
| 15 | **Imán: intersección, eje y borde** | alto | medio | a medias | `enganche.rs` |
| 16 | **Texto dentro de figura** | alto | medio | no | `texto_en_figuras.rs` |
| 17 | **Recortar y extender** | alto | medio | no | `recorte.rs` |
| 18 | Arco | medio | medio | no | `arco.rs` |
| 19 | Ángulos (rótulo en vivo) | medio | bajo | no | `angulos.rs` |
| 19b | **Perímetros** (cimiento de 15, 17 y 23) | alto | medio | no | `perimetros.rs` (tanda cero) |
| 20 | Imagen: `crop` y `scale` | medio | bajo | a medias | `imagenes_lienzo.rs` |
| 21 | Zona | medio | medio | a medias | (depende del chat) |
| 22 | **Enganche de flechas** | alto | alto | no | `enlace.rs` |
| 23 | **Bote de relleno** | alto | alto | no | `regiones.rs` |
| 24 | Materiales de tinta | medio | alto | no | `material.rs` |
| 25 | Lupa como elemento | medio | alto | a medias (otra cosa) | `lupa_elemento.rs` |
| 26 | Flecha a mano alzada | medio | bajo | no | — |
| 27 | Tiradores de punta de raya | medio | bajo | a medias | `tiradores.rs` |
| 28 | `presionFirme` | bajo | bajo | no | `tinta/mod.rs` |
| 29 | La bolita | bajo | bajo | no | — (opcional) |
| 30 | Alfileres y nudos | bajo | alto | no | `nudos.rs` |
| 31 | Plano, recta, espacio | bajo | alto | no | — (fuera) |
| 32 | Sólido, extruir, torno | bajo | alto | no | — (fuera) |
| 33 | Cronograma | bajo | alto | no | — (fuera) |
| 34 | Tablas pegadas | bajo | bajo | **ya viajan** (son figuras normales) | — (fuera) |
| 35 | Láser | bajo | bajo | no (tampoco en Android) | — (fuera) |

---

## 4. El reparto

### 4.1 Por qué hace falta una tanda cero

Un reparto en el que ningún grupo comparta fichero **no es posible tal cual**, y
conviene decirlo antes que descubrirlo con cuatro agentes pisándose. Cinco ficheros
del PC los tocaría cualquier agente que añada una herramienta:

| Fichero | Por qué lo toca todo el mundo |
|---|---|
| `crates/pixpin-motor2d/src/gesto.rs` (2.875 l.) | el `enum Herramienta` y el `match` central del gesto |
| `crates/pixpin-motor2d/src/elemento.rs` (499 l.) | el `enum Figura` |
| `crates/pixpin-motor2d/src/excalidraw.rs` (1.675 l.) | leer y escribir cada tipo nuevo |
| `crates/pixpin-motor2d/src/pintado.rs` (1.751 l.) | traducir cada figura a `Orden` |
| `apps/pixpin/src/ventana_editor.rs` (4.131 l.) | cablear cada herramienta a la ventana |

A eso se suma que **`crates/pixpin-ui/*` y `apps/pixpin/src/ventana_chat.rs` tienen
otro dueño ahora mismo** y no los toca nadie de esta tanda.

La salida es una **tanda cero**, de un solo agente y un solo commit, que abre el
hueco para los cuatro grupos. Después de ella, cada grupo trabaja en módulos que
son suyos y no vuelve a esos cinco ficheros.

### 4.2 Tanda cero — «los cimientos» (un agente, secuencial, antes que nadie)

**Qué hace**, sin implementar ninguna herramienta:

1. Añade a `Herramienta` (`gesto.rs:78`) las variantes nuevas: `Rombo`, `Arco`,
   `FlechaCodos`, `FlechaLibre`, `Lazo`, `Mosaico`, `Serie`, `Relleno`,
   `Recortar`, `Extender`, `Punto`, `CopiarEstilo`.
2. Añade a `Figura` (`elemento.rs:45`) las variantes nuevas, **con sus campos**:
   `Rombo`, `Arco{inicio, barrido}`, `Mosaico{desenfoque}`, `Serie{numero}`,
   `Region{contorno, huecos}`, `Punto{letra, angulo, radio}`.
3. Añade los campos de elemento que faltan y que hoy se ignoran:
   `material`, `presion_firme`, `negrita`, `cursiva`, `tachado`, `contenedor`,
   `enganche_inicio`, `enganche_fin`, `atados`, `papel`, `pauta`.
4. En `excalidraw.rs`, los brazos de lectura y escritura de cada tipo nuevo, más
   **el arreglo del foco** (hoy escribe `"rectangle"`, `excalidraw.rs:243`).
5. En `pintado.rs`, un brazo por figura nueva que **delega** en
   `<modulo>::ordenes(...)` de cada grupo, y en `gesto.rs` un brazo por herramienta
   nueva que delega en `<modulo>::empezar/mover/soltar`.
6. En `ventana_editor.rs`, el cableado de los submódulos nuevos, siguiendo el
   patrón de `ventana_editor/medir.rs`.
7. Porta `Perimetros.kt` (520 l.) a `crates/pixpin-motor2d/src/perimetros.rs`:
   `contornosDe`, `interseccionesCerca` y `puntoEnElPerimetro`. **No es una
   herramienta, es el cimiento de tres** —el anclaje de intersección del grupo B, y
   el bote y el recorte del grupo C— y por eso va aquí y no en un grupo: si lo
   hiciera uno, los otros dos esperarían.
8. Pruebas de ida y vuelta: que un fichero del móvil con los veintitrés tipos entre
   y salga idéntico.

**Ficheros, cerrados**: `crates/pixpin-motor2d/src/{gesto.rs, elemento.rs,
excalidraw.rs, pintado.rs, perimetros.rs, lib.rs}`,
`apps/pixpin/src/ventana_editor.rs`.

Tras este commit, **nadie más toca esos seis ficheros**.

### 4.3 Grupo A — «Formas y tinta»

Rombo · Arco · Flecha de codos · Flecha a mano alzada · Las ocho puntas de flecha ·
Tramas `zigzag` y `pixpin-lines` · Materiales de tinta · `presionFirme` ·
Texto dentro de figura · Negrita, cursiva y tachado

**Ficheros, cerrados:**
```
crates/pixpin-motor2d/src/formas.rs
crates/pixpin-motor2d/src/relleno.rs
crates/pixpin-motor2d/src/texto.rs
crates/pixpin-motor2d/src/tinta/mod.rs
crates/pixpin-motor2d/src/arco.rs              (nuevo)
crates/pixpin-motor2d/src/codo.rs              (nuevo)
crates/pixpin-motor2d/src/material.rs          (nuevo)
crates/pixpin-motor2d/src/texto_en_figuras.rs  (nuevo)
crates/pixpin-render/src/tinta.rs
apps/pixpin/src/ventana_editor/formas.rs       (nuevo)
```
Fuentes en Android: `Shapes.kt`, `Rough.kt`, `Arco.kt`, `Elbow.kt`, `Arrows.kt`
(solo la parte de puntas), `Element.kt:376-496`, `TextoEnFiguras.kt`, `DrawFonts.kt`.

### 4.4 Grupo B — «Seleccionar, transformar, enganchar»

Lazo · Voltear explícito · Tiradores de punta de raya · Imán con intersección, eje
y borde · Rejilla visual y su imán · Enganche de flechas a figuras · Copiar estilo ·
Atajos de teclado (Ctrl+G, Ctrl+[, Shift+H, Ctrl+Shift+L…)

**Ficheros, cerrados:**
```
crates/pixpin-motor2d/src/seleccion.rs
crates/pixpin-motor2d/src/transformar.rs
crates/pixpin-motor2d/src/tiradores.rs
crates/pixpin-motor2d/src/enganche.rs
crates/pixpin-motor2d/src/organizar.rs
crates/pixpin-motor2d/src/impacto.rs
crates/pixpin-motor2d/src/estilo.rs
crates/pixpin-motor2d/src/cuadricula.rs        (nuevo)
crates/pixpin-motor2d/src/lazo.rs              (nuevo)
crates/pixpin-motor2d/src/enlace.rs            (nuevo)
apps/pixpin/src/ventana_editor/seleccion.rs    (nuevo)
```
Fuentes en Android: `Collision.kt`, `Transform.kt`, `TransformHandles.kt`,
`Snapping.kt`, `Iman.kt`, `Cuadricula.kt`, `Organize.kt`, `Arrows.kt` (la parte de
enganche), `DrawProperties.kt`.

### 4.5 Grupo C — «Construir y medir»

Bote de relleno · Recortar · Extender · Puntos etiquetados · Ángulos ·
Perímetros y áreas

**Ficheros, cerrados:**
```
crates/pixpin-motor2d/src/medida.rs
crates/pixpin-motor2d/src/escalabarra.rs
crates/pixpin-motor2d/src/regiones.rs          (nuevo)
crates/pixpin-motor2d/src/recorte.rs           (nuevo)
crates/pixpin-motor2d/src/puntos.rs            (nuevo)
crates/pixpin-motor2d/src/angulos.rs           (nuevo)
apps/pixpin/src/ventana_editor/medir.rs        (ya existe, es suyo)
```
Fuentes en Android: `Regiones.kt`, `Recorte.kt`, `Puntos.kt`, `Angulos.kt`,
`Medida.kt`, `EscalaGrafica.kt`.

**Dependencias**: `perimetros.rs` lo entrega la tanda cero y este grupo solo lo
usa. El anclaje de **intersección** lo hace el grupo B sobre ese mismo `perimetros`
y lo necesitan recortar y los puntos; si B va lento, C se apaña de momento con el
anclaje de esquina.

### 4.6 Grupo D — «Tapar, señalar y la hoja»

Mosaico · Foco (arreglar el puente y leer `oscurecer`) · Números de serie ·
Lupa como elemento · Marco con `papel` y `pauta` · Imagen con `crop` y `scale` ·
Biblioteca de figuras · Goma: modos y protecciones

**Ficheros, cerrados:**
```
crates/pixpin-motor2d/src/marco.rs
crates/pixpin-motor2d/src/mosaico.rs           (nuevo)
crates/pixpin-motor2d/src/serie.rs             (nuevo)
crates/pixpin-motor2d/src/lupa_elemento.rs     (nuevo)
crates/pixpin-motor2d/src/biblioteca.rs        (nuevo)
crates/pixpin-motor2d/src/borrador.rs          (nuevo)
crates/pixpin-render/src/capa_estatica.rs
apps/pixpin/src/imagenes_lienzo.rs
apps/pixpin/src/ventana_editor/tapar.rs        (nuevo)
```
Fuentes en Android: `Lupa.kt`, `Biblioteca.kt`, `MarcosMinimos.kt` *(ojo: este
fichero **no** es de marcos de hoja, son marcos de rotación mínima para cintas 3D;
la lógica de hoja está en `Scene.kt:556-592` y `DrawController.kt:2886`)*,
`Element.kt:266-320` y `:820-900`, `Scene.kt:1066-1110`.

### 4.7 Lo que se queda fuera de esta tanda

Plano cartesiano, recta numérica, espacio de tres ejes, sólido, extruir, torno,
cronograma, tabla de coordenadas y alfileres. Entre todos son unas 4.400 líneas de
Kotlin, valor bajo en escritorio y esfuerzo alto, y **hoy ya viajan intactos**. Lo
único que hay que garantizar es eso último, que es la sección siguiente.

---

## 5. Riesgos de compatibilidad

### 5.1 La buena noticia: el puente ya conserva lo que no entiende

Hay que decirlo con precisión, porque es fácil creer lo contrario leyendo solo
`elemento.rs`:

- **El `struct Elemento` NO conserva nada.** No tiene `flatten` ni campo `extra`
  (`elemento.rs:142-208`), y una clave desconocida se descarta al deserializar
  —lo demuestra su propia prueba, `elemento.rs:462-490`.
- **Pero la conservación vive un nivel más arriba.** `Entrada::Nuestro` guarda el
  `Value` original junto al elemento parseado (`excalidraw.rs:53-58`), y
  `Entrada::Ajeno` guarda el elemento entero sin tocarlo (`:61`), **en su sitio
  dentro del orden de pintado**, que es lo que hace que un mosaico siga tapando lo
  que tapaba.
- Al escribir, un elemento **no tocado** sale byte a byte como entró
  (`excalidraw.rs:170-171`). Y un elemento **sí tocado** tampoco pierde nada: a
  `elemento_hacia` se le pasa el original y **parte de él** —`mapa` es
  `original.clone()` (`excalidraw.rs:617-620`)— y solo sobreescribe las claves que
  entiende. O sea que mover una figura del móvil en el PC conserva su `material`,
  su `boundElements` y su `frameId`.
- Al nivel del lienzo, `Lienzo.resto` (`excalidraw.rs:66-70`) guarda `luces`,
  `tablas`, `alfileres`, `origenCoordenadas`, `referenciasVisibles`, `vista`,
  `style`, `viewport` y `files`, y los devuelve intactos.
- La `escala` tiene protección propia: si viene y no se entiende, se devuelve
  cruda en vez de pisarla con `null` (`excalidraw.rs:142-149` y `:191-195`,
  commit `4a04616`).
- Un `fillStyle` que el PC no conoce no se reescribe mientras no se toque el
  estilo (`excalidraw.rs:635-649`).

Esta arquitectura es la que hay que **no romper** al portar. Cada vez que un grupo
enseñe al PC a leer un tipo nuevo, ese tipo deja de ser `Ajeno` y pasa a ser
`Nuestro`: a partir de ese momento su conservación depende de que
`elemento_hacia` parta del original. Ya lo hace; que siga haciéndolo.

### 5.2 Los campos que el PC tiene que conservar aunque no los entienda

Estos son los que hoy sobreviven **por el mecanismo de §5.1**, y que dejarían de
sobrevivir si alguien los reescribiera desde cero. Van con lo que pasaría:

| Campo | Si el PC lo pierde |
|---|---|
| `material` | la sección rayada de un plano vuelve lisa; hay que rehacer la leyenda a mano |
| `mosaicBlur` | un dato tapado con mancha pasa a taparse con bloques |
| `foco`, `aumento`, `focoAncho`, `focoAlto`, `guia`, `forma`, `lupaRedonda`, `lupaFlecha`, `lupaDosLineas` | la lupa deja de mirar a donde miraba: el detalle enseñado cambia solo |
| `oscurecer` | un foco pasa a oscurecer con el valor de fábrica (`zona` no es campo del elemento: se deriva) |
| `huecos` | el relleno de un anillo pinta también el agujero: se tapa lo que se quería dejar ver |
| `arcStart`, `arcSweep` | un arco vuelve a ser el óvalo entero |
| `etiquetaAngulo`, `etiquetaRadio` | las letras de un croquis de geometría saltan a su sitio de fábrica y se pisan |
| `altura`, `cota`, `giroEnPlanta`, `formaSolida`, `planta`, `inclinacion`, `esqueleto` | un croquis en volumen se aplasta contra el suelo |
| `unidad`, `pasoDeNumeros`, `pasoDeCuadros` | un plano cartesiano deja de saber cuánto vale una unidad: los puntos marcados en él mienten |
| `azimut`, `elevacion` | el espacio de tres ejes vuelve a la vista de partida |
| `tareas`, `periodos` | un cronograma se queda sin filas |
| `papel`, `pauta` | una hoja A4 rayada vuelve a ser un recuadro |
| `negrita`, `cursiva`, `tachado` | el formato del texto se va |
| `presionFirme` | lo escrito a mano vuelve a adelgazar en las curvas |
| `reference` | una línea de referencia pasa a ser dibujo de verdad y ya no se borra en bloque |
| `enlace` | la marca de una zona mandada al chat deja de llevar a su sublienzo |
| `enElSuelo` | un plano de planta tumbado se pone de pie delante del croquis |
| `boundElements`, `startBinding`, `endBinding`, `containerId`, `frameId` | se deshacen todas las ataduras: las flechas dejan de seguir a sus cajas y los rótulos se salen |
| `seed` | **el peor**: la figura entera cambia de forma, porque el garabato se sortea de nuevo |
| `escala` (de la escena) | **todas** las cotas del plano pasan a medir en píxeles |
| `alfileres`, `tablas`, `luces`, `origenCoordenadas`, `vista` (de la escena) | se pierden relaciones y calibraciones que no están en ningún elemento |

### 5.3 Lo que hoy ya está mal, y conviene arreglar en la tanda cero

Tres cosas medidas, no supuestas:

1. **El foco del PC se escribe como `"rectangle"`** (`excalidraw.rs:243`:
   `Figura::Rectangulo | Figura::Foco { .. } => "rectangle"`). Un foco hecho en el
   escritorio llega al móvil como un rectángulo transparente cualquiera: deja de
   oscurecer. Y al revés, un `pixpin-spotlight` del móvil entra como ajeno y **no
   se ve** en el PC. Es un fallo de ida y de vuelta.
2. **El rombo no se lee.** `diamond` es una de las diez figuras principales de
   Excalidraw y el PC lo trata como ajeno (`excalidraw.rs:541-543` lo dice
   explícitamente). Sobrevive, pero es invisible en el escritorio.
3. **El mosaico no se pinta.** Sobrevive como ajeno, pero **lo que tapaba se ve**
   en el PC. En una captura con un número de cuenta encima, eso no es una figura
   que falta: es un dato que se enseña. De los tres, este es el que hay que
   arreglar primero.

Y una cuarta, menor: cuando el PC crea un elemento **nuevo** con esquinas
redondeadas, `redondo: bool` (`elemento.rs:207`) no se traduce a un objeto
`roundness` al escribir —`roundness` se lee (`excalidraw.rs:592`) pero no se
escribe nunca—, así que un rectángulo redondeado nacido en el escritorio llega al
móvil con las esquinas en punta.

### 5.4 La regla, en una frase

**El PC solo puede escribir de cero un elemento que él mismo haya creado.** Todo lo
que viene del móvil se escribe encima de su JSON original, y toda herramienta nueva
que se porte tiene que seguir esa regla o el dibujo del otro se rompe en silencio.

---

## 6. Lo que no he podido comprobar

- **No he ejecutado nada**, ni la aplicación del PC ni las pruebas de Android. Todo
  lo de aquí sale de leer el código de los dos lados.
- **`DrawEditorActivity.kt` (3.712 l.) y `PanelLateral.kt` (2.185 l.)** no los he
  leído enteros: son la interfaz Compose, y lo que necesitaba de ellos —la lista de
  herramientas y la tabla de propiedades— está en `DrawToolbar.kt` y
  `DrawProperties.kt`, que sí.
- **Los números de valor y esfuerzo son un juicio**, no una medida. El esfuerzo lo
  he estimado por líneas de Kotlin y por cuánto del PC hay que tocar; el valor, por
  cuánto se nota la herramienta al anotar en un escritorio.
- **`ENGORDE_DEL_MARCADOR`**: la constante está en `Scene.kt:284` pero no he
  encontrado quién la multiplica; probablemente en `DrawCanvas.kt`, que no bajé.
- **Copiar estilo y cuentagotas**: los di por inexistentes tras buscarlos en todo el
  motor descargado. Si viven en `MandosDelPanel.kt` (943 l.) o en `DrawCanvas.kt`,
  se me han escapado.
- **El foco**: hay una contradicción real entre su comentario («ya no se puede
  crear») y el código vivo, que sí lo crea. No la he resuelto; la dejo apuntada.
- **`Espacio.kt`**: cuento 275 líneas con `wc -l`; una lectura parcial dio 90. La
  cifra buena es la del recuento del fichero entero.

---

## Estado de ejecución (2026-09-20, añadido por el agente coordinador)

### Hecho

| Tanda | Commits | Qué quedó |
|---|---|---|
| Tintas | `bf57fc3` | Los **diez** materiales del móvil con su fórmula, sus constantes y su azar (incluido el puerto del generador de Java). `luz` y `hdr` quedaron como pluma en esta tanda. |
| Tanda cero, 1.ª mitad | `15b1000` | El mosaico **tapa** (banda opaca de urgencia: antes se veía lo censurado), el foco viaja en los dos sentidos, el rombo se ve, `roundness` se escribe. |
| Tanda cero, 2.ª mitad | `1ad0c3b` | 12 variantes de `Herramienta`, 4 de `Figura`, los diez campos en `Elemento::extras`, el puente y el pintado de todo ello, `perimetros.rs` (cimiento de B y C), el grano de tinta en el editor, y la prueba de los 23 tipos. Destapó y arregló tres fugas: `seed` decimal, `mosaicBlur` que no se escribía y el hueco de una región sin recortar. |
| Grupo A | `38989af`, `b4cb501` | Flecha de codos, arco, las 8 puntas, tramas `zigzag` y `pixpin-lines`, texto dentro de figura, negrita/cursiva/tachado, rombo redondeado, `presionFirme`, y **el color de `luz`/`hdr` portado exacto**: el móvil dejó de pintar halo (`Renderer.kt:2136-2158`), solo cambia el color. |

### Enganches pendientes en los cinco ficheros cerrados

El grupo A paró antes de tocarlos, como se le pidió. Faltan, y son del agente coordinador:

- **`elemento.rs`**: `Figura::Flecha` necesita `TipoPunta` en vez de `bool` y un campo `codos`; `EstiloRelleno` necesita `Zigzag` y `LineasPixpin`; `caja()` del arco debe usar `arco::caja_del_arco` (hoy una uña se selecciona por el círculo entero).
- **`excalidraw.rs`**: leer y **escribir** `startArrowhead`/`endArrowhead` (hoy no se escriben), `elbowed`, las dos tramas nuevas, **`fontFamily`** (hoy ni se lee ni se escribe: el móvil reabre nuestro texto con otra letra) y la clave `luces` de la escena.
- **`pintado.rs`**: flecha por `codo::trazado_de_flecha` + las ocho puntas; las dos tramas; `presionFirme`; texto dentro de figura y tachado; rombo redondo; arco por `arco::puntos_del_arco` (hoy hay un muestreo duplicado de 64 tramos); y el color con `color_encendido`.
- **`gesto.rs`**: `Arco`, `FlechaCodos` y `FlechaLibre` nacen rectas.
- **`ventana_editor.rs`**: `mod formas;` si se quiere `ventana_editor/formas.rs`.

### Fuera de los cinco, pendiente en `pixpin-ui` (tiene otro dueño)

Botones de las 12 herramientas nuevas en `BOTONES_EDITOR`, y las filas del panel: PUNTAS (los 9 iconos ya existen en `iconos_excalidraw.rs`), RELLENO (2 botones más), ESTILO_DE_TEXTO, PRESIÓN, FORMA_FLECHA, MATERIAL y el mando de escena de las luces. **Las claves i18n ya están puestas** (`b4cb501`).
