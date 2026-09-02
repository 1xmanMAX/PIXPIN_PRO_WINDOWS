# Análisis técnico del motor de dibujo de Excalidraw (para portar a Rust + Direct2D)

Fuente analizada: `github.com/excalidraw/excalidraw`, clonado en superficial (`--depth 1`,
commit de la rama `master` al 2026-09-01) en un directorio temporal fuera del repo.
Dependencias externas también clonadas para leer su código real (Excalidraw las importa
como paquetes npm, no las vendoriza):

- `perfect-freehand` (paquete `perfect-freehand@1.2.0` en `package.json`; clonado el tag
  `v1.2.0` del repo `steveruizok/perfect-freehand`, commit `fa0b754`).
- `roughjs` (paquete `roughjs@4.6.4`; no existe tag `v4.6.4` en `rough-stuff/rough`, así
  que se usó `HEAD` de `main`, `package.json` versión `4.6.6` — API y algoritmos
  relevantes para el rango 4.x no han cambiado en lo que se cita aquí).

Todas las rutas de fichero citadas abajo son relativas a la raíz de esos clones. Cuando
cito una constante o fórmula es literal del código fuente; cuando resumo/explico lo digo
explícitamente. Todo lo que no pude verificar lo marco como "no explorado" al final de
cada sección.

Nota importante sobre terminología: Excalidraw usa la palabra "version" para dos cosas
distintas que no hay que confundir al portar:
1. `ExcalidrawElement.version` / `versionNonce`: contador de revisión **por elemento**,
   para reconciliación en colaboración (ver sección a).
2. `VERSIONS.excalidraw = 2`: versión del **esquema del fichero** `.excalidraw` completo
   (ver sección g).

---

## a) Modelo de elementos

Fichero fuente: `packages/element/src/types.ts`.

### Base común (`_ExcalidrawElementBase`, líneas 40-82)

Todo elemento (`ExcalidrawElement`, unión discriminada por `type`, línea 206) comparte:

```
id: string
x, y: number                          // esquina superior-izquierda del bounding box, ANTES de rotar
strokeColor: string
backgroundColor: string
fillStyle: "hachure" | "cross-hatch" | "solid" | "zigzag"
strokeWidth: number
strokeStyle: "solid" | "dashed" | "dotted"
roundness: null | { type: RoundnessType; value?: number }
roughness: number                     // 0 = architect, 1 = artist, 2 = cartoonist (ROUGHNESS en common/src/constants.ts:402)
opacity: number                       // 0-100 ENTERO, no 0-1
width, height: number                 // en píxeles de "espacio de escena" (no físicos), SIEMPRE positivos
angle: Radians                        // radianes, rotación alrededor del CENTRO del bounding box (x+width/2, y+height/2)
seed: number                          // entero de 31 bits; siembra el generador de roughjs (ver sección c)
version: number                       // contador de revisión, empieza en 1, +1 en cada mutación
versionNonce: number                  // entero aleatorio de 31 bits, se regenera en cada mutación (desempate cuando `version` coincide en colaboración)
index: FractionalIndex | null         // string de "fractional indexing" (ver sección g) — orden de pintado/z-order
isDeleted: boolean                    // borrado lógico (tombstone), nunca se elimina del array salvo compactación
groupIds: readonly GroupId[]          // pila de grupos, ordenada de más profundo a más superficial (types.ts:71-73)
frameId: string | null                // id del ExcalidrawFrameElement que lo contiene, o null
boundElements: readonly {id, type: "arrow"|"text"}[] | null   // elementos QUE APUNTAN a este (no al revés)
updated: number                       // epoch ms de la última mutación
link: string | null
locked: boolean
customData?: Record<string, any>
```

Puntos de diseño a preservar:
- `x,y,width,height,angle` definen la caja SIN rotar; la rotación se aplica en el
  render/hit-test rotando el punto de consulta en sentido inverso alrededor del centro,
  nunca transformando la geometría del elemento (ver sección f).
- `roundness` es un objeto, no un booleano: `type` determina el ALGORITMO de redondeo
  (`ROUNDNESS.LEGACY=1`, `PROPORTIONAL_RADIUS=2`, `ADAPTIVE_RADIUS=3`,
  `common/src/constants.ts:383-400`) y `value` es el radio fijo en px sólo para
  `ADAPTIVE_RADIUS` (por defecto `DEFAULT_ADAPTIVE_RADIUS = 32`).
- `boundElements` vive en el contenedor y apunta hacia fuera; el texto/flecha ligado
  guarda la relación inversa en su propio campo (`containerId` en texto, `startBinding`/
  `endBinding` en flechas) — es una relación bidireccional con dos campos distintos, hay
  que mantener ambos lados sincronizados manualmente en cada mutación.

### Variantes por tipo

- **Selection** (`type: "selection"`): sin campos extra; es el rectángulo de goma elástica,
  nunca se persiste como parte de la escena real.
- **Rectangle / Diamond / Ellipse** (`ExcalidrawGenericElement`, líneas 180-184): sin
  campos extra más allá de la base.
- **Text** (`ExcalidrawTextElement`, líneas 235-257):
  `fontSize, fontFamily (número, no string — ver sección d), text (texto YA envuelto/wrapped
  para render), textAlign, verticalAlign, containerId (id del contenedor o null),
  originalText (texto SIN envolver, el que el usuario realmente escribió),
  autoResize (bool: true = la caja se ajusta al texto; false = el texto se envuelve al
  ancho fijo), lineHeight (número sin unidad, se multiplica por fontSize)`.
- **Linear / Arrow / Line** (`ExcalidrawLinearElement`, líneas 333-359):
  `points: LocalPoint[]` (coordenadas RELATIVAS a `(x,y)`, el primer punto es siempre
  `[0,0]`), `startBinding`/`endBinding: FixedPointBinding | null` (líneas 284-297:
  `{elementId, fixedPoint: [number,number] (ratio 0..1 dentro del elemento ligado),
  mode: "inside"|"orbit"|"skip"}`), `startArrowhead`/`endArrowhead: Arrowhead | null`.
  `ExcalidrawLineElement` añade `polygon: boolean` (si es un polígono cerrado rellenable).
  `ExcalidrawArrowElement` añade `elbowed: boolean`; si es `true` se convierte en
  `ExcalidrawElbowArrowElement` con campos extra de enrutado ortogonal
  (`fixedSegments`, `startIsSpecial`, `endIsSpecial`) que **no exploré en profundidad**
  (el algoritmo vive en `packages/element/src/elbowArrow.ts`, no leído a fondo — es
  enrutado ortogonal tipo "orthogonal edge routing", recomiendo tratarlo como
  funcionalidad opcional de fase 2).
- **Freedraw** (líneas 394-401): `points: LocalPoint[]`, `pressures: number[]` (uno por
  punto, o vacío si `simulatePressure=true`), `simulatePressure: boolean`,
  `strokeOptions: {variability: "variable"|"constant", streamline: number}` (ver sección b).
- **Image** (líneas 146-161): `fileId: FileId | null`, `status: "pending"|"saved"|"error"`,
  `scale: [number,number]` (±1, para flip horizontal/vertical), `crop: ImageCrop | null`
  (`{x,y,width,height,naturalWidth,naturalHeight}`, en píxeles de la imagen NATURAL).
- **Frame / MagicFrame** (líneas 163-171): `name: string | null`; un frame es un
  contenedor visual — los elementos con `frameId` igual a su `id` se recortan (clip) a
  sus límites y se mueven/eliminan junto con él.
- **Iframe / Embeddable**: sin campos de interés para un motor nativo (contenido HTML
  embebido) — candidatos claros a NO portar.

### No explorado en detalle
`elbowArrow.ts` (enrutado ortogonal), `flowchart.ts` (creación automática de diagramas de
flujo), `elementLink.ts` (enlaces entre elementos). Son capas de conveniencia por encima
del modelo base, no bloquean el port del núcleo.

---

## b) Trazo a mano alzada (freedraw)

### Dos modos de trazo (excalidraw ha evolucionado desde el `getStroke` único)

Fichero: `packages/element/src/shape.ts`, función `getFreedrawOutlinePoints` (línea 1247)
despacha según `element.strokeOptions?.variability`:

- **`"variable"` (por defecto, look clásico de Excalidraw)** → `getVariableWidthFreedrawOutline`
  (línea 1201), usa `perfect-freehand`.
- **`"constant"` (grosor uniforme, estilo "laser")** → `getConstantWidthFreedrawOutline`
  (línea 1236), usa el paquete `@excalidraw/laser-pointer` (mismo tipo de algoritmo de
  streamline pero con radio fijo). **No es necesario portar el modo constante en una v1**:
  es visualmente un caso particular (pressure fijada a 1) del mismo pipeline.

### Parámetros exactos pasados a `getStroke` (variable width)

```ts
// packages/element/src/shape.ts:1213-1221
getStroke(inputPoints, {
  simulatePressure: element.simulatePressure,
  size: element.strokeWidth * 4.25,   // VARIABLE_WIDTH_FREEDRAW.SIZE_FACTOR
  thinning: 0.6,                       // VARIABLE_WIDTH_FREEDRAW.THINNING
  smoothing: 0.5,                      // VARIABLE_WIDTH_FREEDRAW.SMOOTHING
  streamline: element.strokeOptions?.streamline ?? 0.5,  // DEFAULT_STROKE_STREAMLINE
  easing: (t) => Math.sin((t * Math.PI) / 2),  // easeOutSine
  last: true,
})
```

Los propios comentarios del código (líneas 1176-1187) advierten: `SIZE_FACTOR=4.25`,
`THINNING=0.6`, `SMOOTHING=0.5` son **constantes mágicas ajustadas visualmente**, no
derivadas analíticamente — hay que copiarlas tal cual para igualar el "feel" de Excalidraw.

`streamline` depende del dispositivo de entrada (`App.tsx` ~línea 9884-9890): ratón →
`DEFAULT_STROKE_STREAMLINE = 0.5`; pluma/touch → `DEFAULT_STROKE_STREAMLINE_PRECISE = 0.2`
(`common/src/constants.ts:557-558`).

`simulatePressure` se decide así al iniciar el trazo (`App.tsx:9867`):
```ts
const simulatePressure = event.pressure === 0.5;
```
Es decir: si el `PointerEvent.pressure` reportado por el navegador es exactamente el
valor por defecto `0.5` (lo que reportan ratón y la mayoría de touch sin presión real),
Excalidraw **asume que no hay presión real** y activa la simulación por velocidad. Si el
dispositivo (pluma) reporta un valor distinto de 0.5 en el primer punto, se graba la
presión real en `pressures[]` en cada punto subsiguiente.

### Algoritmo `perfect-freehand` (repo `steveruizok/perfect-freehand`, paquete `packages/perfect-freehand/src`)

Pipeline: `getStrokePoints` → `getStrokeOutlinePoints` → (polígono relleno).

**1. `getStrokePoints` (`getStrokePoints.ts`)** — construye la polilínea central suavizada:

- Factor de interpolación: `t = 0.15 + (1 - streamline) * 0.85`.
- Si sólo hay 2 puntos de entrada, se insertan 4 puntos interpolados linealmente entre
  ellos (evita "guiones" en trazos con extremos afilados/taper).
- Si sólo hay 1 punto, se duplica con un offset de `[1,1]`.
- Recorrido: cada punto nuevo `point[i]` se calcula por LERP entre el punto anterior ya
  aceptado (`prev.point`) y el punto crudo `pts[i]`, con factor `t` — EXCEPTO el último
  punto cuando `options.last === true`, que se usa tal cual (sin suavizar), para que el
  trazo termine exactamente donde soltó el usuario.
- Si el punto interpolado es idéntico al anterior, se descarta (evita puntos duplicados).
- **Filtro de ruido al inicio**: mientras `runningLength < size`, los puntos se descartan
  (no se añaden a `strokePoints`) — evita "ruido" al empezar a dibujar antes de que el
  trazo alcance una longitud mínima igual al `size` configurado.
- Cada `StrokePoint` guarda: `point [x,y]`, `pressure` (presión cruda del punto, o `0.25`
  para el primer punto si es negativa, `0.5` para el resto), `vector` (unitario, dirección
  DESDE el punto actual HACIA el anterior: `normalize(prev.point - point)`), `distance`
  (al punto anterior), `runningLength` (acumulada desde el inicio).
- El vector del primer punto se copia del segundo punto al final (no tiene vector propio).

**2. `getStrokeRadius` (`getStrokeRadius.ts`)** — fórmula del radio en función de la presión:

```ts
radius(size, thinning, pressure, easing) =
    size * easing(0.5 - thinning * (0.5 - pressure))
```

Con `pressure=0.5` (neutra) da `radius = size * easing(0.5)`. Con `thinning=0` el radio
es constante `size/2` independientemente de la presión (short-circuit en el llamador).

**3. `getStrokeOutlinePoints` (`getStrokeOutlinePoints.ts`)** — construye el CONTORNO
(izquierdo + derecho) del trazo a partir de la polilínea central:

- Constante `RATE_OF_PRESSURE_CHANGE = 0.275`, `FIXED_PI = Math.PI + 0.0001` (offset para
  evitar artefactos de precisión en arcos completos).
- `prevPressure` inicial = promedio acumulado (recursivo) de las presiones de los primeros
  10 puntos, simulando una aceleración de presión — así los trazos no empiezan "gordos".
  Fórmula de simulación por punto (si `simulatePressure`):
  ```
  sp = min(1, distancia_al_punto_anterior / size)      // "speed"
  rp = min(1, 1 - sp)                                   // "rate"
  pressure = min(1, prevPressure + (rp - prevPressure) * (sp * RATE_OF_PRESSURE_CHANGE))
  ```
- Radio de taper (afilado) en extremos: si `start.taper`/`end.taper` son `true`, la
  distancia de taper = `max(size, longitudTotal)`; si es un número, se usa tal cual.
  Easing de taper por defecto: inicio `t*(2-t)` (ease-out cuadrático), fin `--t*t*t+1`
  (ease-out cúbico) — Excalidraw pasa `easing: sin(t*PI/2)` sólo para el mapeo
  presión→radio, no sustituye estos easings de taper (que quedan en sus valores por
  defecto porque Excalidraw no toca `start`/`end`).
- El radio final en cada punto es `radius * min(taperStart, taperEnd)`, con mínimo
  absoluto `0.01`.
- **Detección de esquina afilada**: se compara el producto escalar (`dpr`) entre el
  vector del punto actual y el del punto siguiente/anterior; si es negativo (ángulo >90°)
  se considera esquina afilada y se dibuja un arco de radio `radius` en 13 pasos
  (`step = 1/13`) en vez de proyectar puntos normales — evita que el offset izq/der se
  "cruce" en curvas muy cerradas.
- **Puntos normales**: se proyecta perpendicular al vector promediado entre el actual y
  el siguiente (`lrp(nextVector, vector, dpr(vector,nextVector))`) a distancia `radius` a
  cada lado. Un nuevo punto sólo se añade al lado si su distancia al cuadrado al último
  punto de ese lado supera `minDistance = (size * smoothing)²` (evita amontonar puntos).
- **Tapas** (caps): redonda por defecto (`cap: true`), generada rotando el primer punto
  del lado opuesto alrededor del punto extremo en pasos de `1/13` de vuelta (inicio) o
  `1/29` de 3 medias vueltas (fin, con un giro y medio para evitar tapas incorrectas en
  giros agudos). Cap plana: 4 puntos formando un pequeño rectángulo perpendicular al
  vector final.
- Caso trazo de 1 punto: se dibuja directamente un círculo (13 puntos) de radio
  `firstRadius`.
- Orden de devolución: `leftPts + endCap + rightPts.reverse() + startCap` — un anillo
  cerrado listo para rellenar como polígono (par-impar o non-zero, da igual porque no se
  auto-interseca).

**Vectores auxiliares** (`vec.ts`): operaciones estándar 2D (`add,sub,mul,div,per
(perpendicular = [y,-x]), dpr (dot), len, dist, uni (normalizar), rotAround, lrp
(interpolación lineal), prj (proyección: A + B*c)`. Triviales de portar a `glam`/`vek`
en Rust o a floats manuales.

### De contorno a polígono relleno (Excalidraw, no perfect-freehand)

`shape.ts:getSvgPathFromStroke` (línea 1285) convierte la lista de puntos del contorno en
un path SVG usando **curvas cuadráticas de Bézier entre puntos medios consecutivos**
(técnica estándar para suavizar una polilínea de puntos densos):
```
M P0 Q P0 mid(P0,P1) Q P1 mid(P1,P2) ... Q Pn-1 mid(Pn-1,P0) L P0 Z
```
Para Direct2D esto se traduce directamente a `ID2D1GeometrySink` con
`AddQuadraticBezier` en vez de construir un string SVG — mismo algoritmo, sin el paso
intermedio de texto.

### No explorado / fuera de alcance
`getFreedrawStrokeCenterPoints` (usa `getStrokePoints` solo, sin outline, para
"bucket fill" — línea de centro para clic-para-rellenar); el modo `"constant"` vía
`@excalidraw/laser-pointer` (no clonado ni leído).

---

## c) El aspecto "a mano" (roughness / rough.js)

Fichero clave de Excalidraw: `packages/element/src/shape.ts`, función
`generateRoughOptions` (línea 194) y `adjustRoughness` (línea 171).
Fuente de rough.js: repo `rough-stuff/rough`, `src/math.ts`, `src/core.ts`,
`src/renderer.ts`, `src/generator.ts`, `src/fillers/*.ts`.

### El generador pseudoaleatorio — CRÍTICO para determinismo bit a bit

`rough/src/math.ts` completo:

```ts
export function randomSeed(): number {
  return Math.floor(Math.random() * 2 ** 31);
}

export class Random {
  private seed: number;
  constructor(seed: number) { this.seed = seed; }
  next(): number {
    if (this.seed) {
      return ((2 ** 31 - 1) & (this.seed = Math.imul(48271, this.seed))) / 2 ** 31;
    } else {
      return Math.random();
    }
  }
}
```

Es el generador **Lehmer / Park-Miller "minimal standard" LCG**: multiplicador
`48271`, módulo `2^31 - 1 = 2147483647` (via máscara AND con `2^31-1`, NO módulo real —
ojo, esto es distinto de un LCG canónico porque usa AND-mask en vez de `% (2^31-1)`;
replicar EXACTAMENTE con `seed = (48271 * seed) & 0x7FFFFFFF` en aritmética de 32 bits
sin signo, usando multiplicación 32×32→32 bits truncada como hace `Math.imul` — en Rust
esto es `seed = seed.wrapping_mul(48271) & 0x7FFF_FFFF` con `u32`). El resultado se
normaliza dividiendo por `2^31` (no por `2^31-1`), así que el rango es `[0, ~1)` pero
técnicamente puede no alcanzar exactamente el máximo de un LCG puro — replicar la
fórmula literal, no "un LCG equivalente".

Caso especial: si `seed === 0`, `next()` degenera a `Math.random()` (no determinista) —
en la práctica Excalidraw siempre asigna un seed distinto de 0 (ver abajo), así que este
caso no debería darse en producción, pero hay que replicarlo para paridad total si se
lee un fichero antiguo con `seed: 0`.

En Excalidraw (no en rough.js), este MISMO generador se reutiliza para producir los
identificadores aleatorios de los propios elementos:

```ts
// packages/common/src/random.ts (completo)
import { nanoid } from "nanoid";
import { Random } from "roughjs/bin/math";
let random = new Random(Date.now());
let testIdBase = 0;
export const randomInteger = () => Math.floor(random.next() * 2 ** 31);
export const reseed = (seed: number) => { random = new Random(seed); testIdBase = 0; };
export const randomId = () => (isTestEnv() ? `id${testIdBase++}` : nanoid());
```

Es decir: hay UN generador global de proceso (`random`, sembrado con `Date.now()` al
arrancar la app), y `randomInteger()` se usa para:
- `element.seed` al crear un elemento nuevo si no se especifica
  (`packages/element/src/newElement.ts:149`: `seed: rest.seed ?? randomInteger()`).
- `element.versionNonce` en cada mutación
  (`packages/element/src/mutateElement.ts:140,175,190`:
  `versionNonce: updates.versionNonce ?? randomInteger()`).

Es decir: **el seed del "look a mano" de un elemento y el nonce de reconciliación
comparten la MISMA fuente aleatoria de proceso**, no dos generadores distintos. Al
restaurar un elemento de un fichero sin `seed` (legado), el valor por defecto es `1`
(`packages/excalidraw/data/restore.ts:457`: `seed: element.seed ?? 1`), NO un valor
aleatorio — importante para que recargar el mismo fichero dé el mismo dibujo.

**Recomendación de implementación en Rust**: implementar `struct Lcg { seed: u32 }`
con exactamente esa fórmula; usar una instancia de proceso sembrada con un timestamp
para nuevos IDs/seeds, y una instancia *por elemento* (sembrada con `element.seed`)
cada vez que se necesite regenerar su geometría "a mano" — nunca compartir estado
entre elementos al dibujar (ver siguiente apartado, `cloneOptionsAlterSeed`).

### Opciones resueltas (`ResolvedOptions`, `rough/src/core.ts:45-67`) y valores por defecto del generador (`rough/src/generator.ts:13-33`)

```ts
defaultOptions = {
  maxRandomnessOffset: 2,
  roughness: 1,
  bowing: 1,
  strokeWidth: 1,
  curveTightness: 0,
  curveFitting: 0.95,
  curveStepCount: 9,
  fillStyle: 'hachure',
  fillWeight: -1,       // si es -1, rough.js usa strokeWidth/2 como ancho de línea de relleno
  hachureAngle: -41,     // grados
  hachureGap: -1,        // si es -1, rough.js usa strokeWidth*4
  seed: 0,
  disableMultiStroke: false,
  disableMultiStrokeFill: false,
  preserveVertices: false,
  fillShapeRoughnessGain: 0.8,
}
```

Excalidraw sobrescribe explícitamente varios de estos por elemento
(`generateRoughOptions`, `shape.ts:194-259`):

```ts
options = {
  seed: element.seed,
  strokeLineDash: strokeStyle==="dashed" ? [8, 8+strokeWidth]
                : strokeStyle==="dotted" ? [1.5, 6+strokeWidth]
                : undefined,
  disableMultiStroke: strokeStyle !== "solid",
  strokeWidth: strokeStyle !== "solid" ? strokeWidth + 0.5 : strokeWidth,
  fillWeight: strokeWidth / 2,
  hachureGap: strokeWidth * 4,
  roughness: adjustRoughness(element),   // ver abajo
  stroke: strokeColor (con filtro de modo oscuro),
  preserveVertices: continuousPath || element.roughness < ROUGHNESS.cartoonist (2),
}
// + por tipo: fillStyle/fill (relleno) para rectangle/diamond/ellipse/iframe/embeddable
// siempre; para line/freedraw SOLO si el trazo forma un bucle cerrado (isPathALoop);
// ellipse añade curveFitting=1 (fuerza el óvalo a ceñirse más a los ejes reales)
```

`adjustRoughness` (`shape.ts:171-192`) REDUCE la rugosidad en formas muy pequeñas para
que no se vean como garabatos ilegibles:
```
maxSize = max(width,height); minSize = min(width,height)
si (minSize>=20 && maxSize>=50) || (minSize>=15 && tiene esquinas redondeadas) ||
   (es lineal && maxSize>=50):
   roughness sin cambios
si no:
   roughness = min(roughness / (maxSize<10 ? 3 : 2), 2.5)
```

### Generación de línea individual (el corazón del "look a mano"): `_line` en `rough/src/renderer.ts:292-361`

Para un segmento de `(x1,y1)` a `(x2,y2)`, rough.js NO dibuja una línea recta: dibuja
UNA curva de Bézier cúbica con desplazamientos aleatorios, y por defecto la dibuja DOS
VECES con offsets independientes (`disableMultiStroke=false`) para simular el trazo
irregular de una pluma real superpuesta:

```
lengthSq = (x1-x2)² + (y1-y2)²; length = sqrt(lengthSq)
roughnessGain = length<200 ? 1 : length>500 ? 0.4 : (-0.0016668*length + 1.233334)  // interpolación lineal entre 200 y 500px
offset = maxRandomnessOffset (=2 por defecto); si offset²*100 > lengthSq: offset = length/10  (líneas muy cortas usan un offset proporcional)
halfOffset = offset/2
divergePoint = 0.2 + random()*0.2      // punto de la línea (20-40%) donde más diverge la curva del ideal
midDispX = offsetOpt( bowing*maxRandomnessOffset*(y2-y1)/200, roughnessGain )
midDispY = offsetOpt( bowing*maxRandomnessOffset*(x1-x2)/200, roughnessGain )
   // "bowing": desplazamiento del punto medio PERPENDICULAR a la línea (nótese el swap x/y con signo)
```
Con `offsetOpt(x, gain) = offset(-x, x, gain)` y
`offset(min,max,gain) = roughness * gain * (random()*(max-min) + min)`.

Se emite UNA operación de path:
```
move (x1 + offset aleatorio, y1 + offset aleatorio)       // sólo en la 1ª pasada (overlay=false)
bcurveTo(
  midDispX + x1 + (x2-x1)*divergePoint + offsetAleatorio,
  midDispY + y1 + (y2-y1)*divergePoint + offsetAleatorio,
  midDispX + x1 + 2*(x2-x1)*divergePoint + offsetAleatorio,
  midDispY + y1 + 2*(y2-y1)*divergePoint + offsetAleatorio,
  x2 + offsetAleatorio, y2 + offsetAleatorio
)
```
(los dos puntos de control son múltiplos 1x y 2x de `divergePoint` a lo largo de la
línea, con el mismo desplazamiento medio `midDisp` aplicado a ambos — de ahí la curva
suave en forma de "arco" en vez de zigzag). La 2ª pasada (`overlay=true`) usa
`randomHalf()` (offset la mitad) en vez de `randomFull()` para los puntos de control,
y SIEMPRE genera su propio `move` (no reutiliza el de la 1ª pasada) — esto es lo que da
el efecto de "doble trazo" ligeramente desalineado.

`_doubleLine` simplemente llama a `_line` dos veces (excepto si `disableMultiStroke`) y
concatena las operaciones — ambas pasadas consumen números del MISMO generador
(`ops.randomizer`, perezosamente creado la primera vez que se llama `random(ops)`,
sembrado con `ops.seed`), así que el ORDEN de las llamadas a `random()` determina el
resultado — replicar el orden exacto de llamadas es tan importante como la fórmula.

**Rectángulo** = `polygon([tl,tr,br,bl])` = `linearPath(...,close=true)` = 4 llamadas a
`_doubleLine` en orden TL→TR→TR→BR→BR→BL→BL→TL, cada línea consumiendo su propia tanda
de `random()`.

**Elipse** (`generateEllipseParams` + `ellipseWithParams`, `renderer.ts:97-121`):
```
psq = sqrt(2*PI*sqrt(((w/2)² + (h/2)²)/2))
stepCount = ceil(max(curveStepCount, curveStepCount/sqrt(200) * psq))   // más pasos cuanto más grande la elipse
increment = 2*PI / stepCount
rx = |w/2| + offsetOpt(|w/2| * (1-curveFitting))     // curveFitting=1 para Excalidraw → sin offset en rx/ry
ry = |h/2| + offsetOpt(|h/2| * (1-curveFitting))
```
Luego se muestrean puntos en el perímetro cada `increment` radianes con un offset
aleatorio de posición en CADA punto (radio angular offset por `offsetOpt(0.1, ...)` al
inicio), y esos puntos se pasan a `_curve` (Catmull-Rom → Bézier, `_curve` en
`renderer.ts:391-424`, tightness=`curveTightness`) para obtener una curva suave que
pasa por ellos. Si `!disableMultiStroke`, se genera una SEGUNDA pasada con offset de
paso 0 pero radio de offset de posición `1.5x` mayor — igual patrón de doble trazo que
las líneas rectas.

**Diamante**: no tiene generador propio en rough.js — Excalidraw lo construye como un
`polygon()` de 4 vértices (`getDiamondPoints`, ver sección f) igual que el rectángulo,
sólo cambian las coordenadas de los vértices.

### Relleno (`fillStyle`)

Router: `rough/src/fillers/filler.ts` — Excalidraw sólo expone 4 de los 6 estilos que
soporta rough.js (`FillStyle` en `types.ts:19`): `hachure | cross-hatch | solid | zigzag`
(rough.js también tiene `dots`, `dashed`, no usados por Excalidraw).

- **`solid`**: `solidFillPolygon` (`renderer.ts:196-211`) — mueve+linea recta por cada
  vértice del polígono con un pequeño offset aleatorio (`maxRandomnessOffset`) por
  vértice; es un polígono relleno "sketchy" simple, sin patrón.
- **`hachure`** (por defecto): `polygonHachureLines` (`fillers/scan-line-hachure.ts`):
  ```
  angle = hachureAngle + 90     // el ángulo de RELLENO es perpendicular al hachureAngle configurado
  gap = hachureGap<0 ? strokeWidth*4 : hachureGap; gap = max(round(gap), 0.1)
  skipOffset = 1
  si roughness>=1 && random()>0.7: skipOffset = gap    // 30% de las veces, salta líneas alternas (relleno más disperso/irregular)
  ```
  y delega el cálculo de las líneas de barrido (scanline) al paquete externo
  `hachure-fill` (**no clonado ni leído** — el algoritmo estándar es: rotar el polígono
  `-angle`, barrer líneas horizontales cada `gap` px dentro del bounding box, calcular
  intersecciones con las aristas del polígono, emparejar intersecciones consecutivas y
  rotar los segmentos resultantes `+angle` de vuelta). Cada línea resultante se dibuja
  con `_doubleLine` (mismo algoritmo "a mano" que los contornos).
- **`cross-hatch`**: `HatchFiller` = hachure normal + hachure con `angle+90`, concatenados
  (`fillers/hatch-filler.ts`) — literalmente dos pasadas de hachure cruzadas.
- **`zigzag`**: `ZigZagFiller` (`fillers/zigzag-filler.ts`) — parte de las mismas líneas
  de hachure pero las convierte en un patrón de zigzag conectado: para cada línea
  `[p1,p2]` calcula un desplazamiento `dgx,dgy = gap*0.5*[cos,sin](hachureAngle en rad)`
  y genera DOS segmentos por línea: `[p1-d, p2]` y `[p1+d, p2]`, dibujados consecutivamente
  (visualmente forma el patrón de "V" repetido característico).

### No explorado en profundidad
El paquete `hachure-fill` (algoritmo scanline exacto); `path-data-parser`/`points-on-path`
(sólo relevantes para importar SVGs externos, `svgPath()` en rough.js); `arc()` (arcos
parciales, no usados por los tipos de elemento de Excalidraw salvo internamente para
nada visible desde fuera).

---

## d) Texto

Ficheros: `packages/element/src/textElement.ts`, `textMeasurements.ts`,
`textWrapping.ts`; constantes en `packages/common/src/constants.ts` y
`font-metadata.ts`; edición in-situ en `packages/excalidraw/wysiwyg/textWysiwyg.tsx`.

### Medición (`textMeasurements.ts`)

- `getFontString({fontSize, fontFamily}) = "${fontSize}px ${familyCSSString}"`
  (`common/src/utils.ts:139-147`) — se usa literalmente como el string `font` del
  contexto 2D del canvas (`CanvasTextMetricsProvider`, `textMeasurements.ts:121-150`),
  que crea un `<canvas>` oculto y llama a `context.measureText(text).width` (la
  "advance width", no el bounding box visual) — en Direct2D el equivalente es
  `IDWriteTextLayout::GetMetrics` (o `DetermineMinWidth`) usando el mismo tamaño/familia.
- `getTextWidth`: máximo de `getLineWidth` sobre cada línea (split por `\n`).
- `getTextHeight(text, fontSize, lineHeight) = getLineHeightInPx(fontSize,lineHeight) * numLíneas`,
  con `getLineHeightInPx = fontSize * lineHeight` (line-height SIN unidad, estilo CSS/W3C).
- `charWidth`: caché por (fontString, code point) del ancho de UN carácter — usado por el
  wrapping para evitar remedir textos completos carácter a carácter.
- Texto vacío / líneas vacías: antes de medir, cada línea vacía se sustituye por un
  espacio simple (`measureText`, línea 21) porque el navegador recorta el alto de líneas
  vacías en el layout — replicar este parche en la medición propia de Direct2D.

### Line-height por familia de fuente (`common/src/font-metadata.ts:175`, no reproducido
literal pero confirmado por test `packages/element/tests/textElement.test.ts:199-208`):
`getLineHeight()` por defecto (Excalifont) = **1.25**; `getLineHeight(FONT_FAMILY.Cascadia)`
(monoespaciada) = **1.2**. `DEFAULT_FONT_SIZE = 20`, fuente por defecto = `Excalifont`
(`common/src/constants.ts:130-141,213-214` — el enum `FONT_FAMILY` asigna IDs numéricos
estables: `Virgil=1, Helvetica=2, Cascadia=3, Excalifont=5, Nunito=6, "Lilita One"=7,
"Comic Shanns"=8, "Liberation Sans"=9, Assistant=10` — el 4 se dejó libre a propósito por
compatibilidad retro). Esto confirma que `fontFamily` en el elemento es un **número**, no
un string — el mapeo número→fuente real (archivo .woff2, fallbacks) vive aparte en
`packages/excalidraw/fonts/`.

### Wrapping (`textWrapping.ts`) — pipeline de 4 pasos (comentario de cabecera, líneas 7-22)

1. `parseTokens()`: tokeniza una línea "dura" (entre `\n`) en tokens rompibles, con
   reglas Unicode-aware (no sólo espacios): separa por espacio/guion, agrupa emoji
   multi-codepoint como un solo token, y aplica reglas específicas de CJK (Han, Hiragana,
   Katakana, Hangul) — en CJK cada carácter rompe antes/después salvo que esté pegado a
   un signo de apertura/cierre (p.ej. `「」`), que forma un solo token con su vecino. Usa
   regex Unicode con lookbehind (`getLineBreakRegexAdvanced`) con un *fallback* simple
   (`getLineBreakRegexSimple`) para navegadores sin soporte de lookbehind — **para Rust,
   el crate `unicode-linebreak` (implementa UAX #14) es el sustituto natural**, no hace
   falta reimplementar las regex a mano.
2. `getWrappedTextLines(text, font, maxWidth)`: por cada línea dura, si su ancho total ya
   cabe en `maxWidth` la deja tal cual; si no, llama a `wrapLine`.
3. `wrapLine`: algoritmo greedy clásico de "acumular tokens mientras quepan":
   - Recorre tokens; añade cada uno a `currentLine` mientras
     `ancho(currentLine+token) <= maxWidth` (excepción: los tokens que son SÓLO
     whitespace siempre se añaden sin comprobar ancho, para no perder espacios finales
     visibles antes de un salto).
   - Optimización: si el token es un único code point, usa el `charWidth` cacheado en vez
     de volver a medir la línea entera (evita medir el string completo repetidamente).
   - Si un token NO cabe ni siquiera en una línea vacía → `wrapWord` lo parte carácter a
     carácter (excepto si es un emoji multi-codepoint, que nunca se rompe).
   - Al cortar por longitud excedida con `currentLine` no vacía, se hace
     `trimLineEndAtSoftBreak` (recorta whitespace final) antes de empujar la línea.
4. `trimLine` / `trimLineEndAtSoftBreak`: en la ÚLTIMA línea visual de una línea dura, si
   aún excede el ancho tras el wrap, se recorta el whitespace final progresivamente
   (carácter a carácter) hasta que quepa — replica cómo los navegadores permiten que un
   espacio final "sobresalga" ligeramente del contenedor sin forzar otro salto de línea.

`wrapText(text, font, maxWidth) = getWrappedTextLines(...).map(l=>l.text).join("\n")` es
el punto de entrada usado por el resto del código (`textElement.ts:81-85` y `:172-176`).

### `originalText` vs `text` — el dato canónico es `originalText`

- `originalText`: lo que el usuario escribió literalmente, SIN saltos de línea forzados
  por el wrapping (sólo con los `\n` que el usuario introdujo).
- `text`: el resultado de `wrapText(originalText, ...)` — con `\n` INSERTADOS donde el
  wrapping partió una línea. Es el que se usa para pintar y medir el bounding box.
- Al editar o redimensionar (`redrawTextBoundingBox`, `textElement.ts:46-140`), SIEMPRE
  se recalcula `text` desde `originalText`, nunca al revés — si se guarda sólo `text` se
  pierde la posibilidad de re-wrappear correctamente al cambiar el ancho después.
- Si `autoResize === true` Y no hay contenedor, no hay `maxWidth` (undefined) → no se
  wrappea, `text = originalText`, y el ancho del elemento se ajusta al contenido
  (comportamiento de "texto libre").

### Contenedores (texto ligado a una forma)

- `BOUND_TEXT_PADDING = 5` px (`constants.ts:357`) es el margen interior en TODOS los
  contenedores.
- `getContainerCoords` (`textElement.ts:357-375`) — posición de origen del texto DENTRO
  del contenedor, antes de aplicar alineación:
  ```
  offsetX = offsetY = BOUND_TEXT_PADDING
  si es ellipse: offsetX += (w/2)*(1-√2/2); offsetY += (h/2)*(1-√2/2)   // inscribe un rect en la elipse
  si es diamond:  offsetX += w/4;            offsetY += h/4              // inscribe un rect en el rombo
  ```
- `getBoundTextMaxWidth`/`getBoundTextMaxHeight` (`textElement.ts:468-517`) — el
  rectángulo MÁXIMO inscrito dentro de la forma, restando el padding:
  ```
  rectangle:  maxW = width  - 2*PADDING;         maxH = height - 2*PADDING
  ellipse:    maxW = round((w/2)*√2) - 2*PADDING; maxH = round((h/2)*√2) - 2*PADDING
  diamond:    maxW = round(w/2)      - 2*PADDING; maxH = round(h/2)      - 2*PADDING
  arrow:      maxW = max(0.7*w, fontSize*ratio);  maxH = height (si height-16*PADDING>0, si no usa el alto actual del texto)
              // ARROW_LABEL_WIDTH_FRACTION=0.7 y ARROW_LABEL_FONT_SIZE_TO_MIN_WIDTH_RATIO no confirmados numéricamente — no releí su valor exacto, sólo el uso
  ```
- `computeContainerDimensionForBoundText` (el inverso — cuánto debe crecer el contenedor
  para que quepa un texto de cierta medida): `rectangle: dim+2*PAD`,
  `ellipse: round((dim+2*PAD)/√2 * 2)`, `diamond: 2*(dim+2*PAD)`, `arrow: dim+16*PAD`.
- Alineación vertical dentro del contenedor: TOP → `y = containerCoords.y`; BOTTOM →
  `y = containerCoords.y + (maxHeight - textHeight)`; MIDDLE (implícito, rama no
  transcrita arriba) → centrado.
- Si el contenedor es una flecha (`isArrowElement`), la posición se delega por completo a
  `LinearElementEditor.getBoundTextElementPosition` (el texto viaja sobre el punto medio
  del trazo, con `angle` forzado a 0 — un texto ligado a una flecha NUNCA rota con ella,
  invariante comprobado explícitamente en `redrawTextBoundingBox:55-60` con un `invariant()`
  en dev).
- Si el texto medido excede el contenedor, el contenedor CRECE (`scene.mutateElement`
  sobre el contenedor, `textElement.ts:107-122`) — el ajuste es "el contenedor se adapta
  al texto", no al revés, salvo que `autoResize` del texto sea `false`.

### Edición in-situ (`textWysiwyg.tsx`)

Confirmado por lectura selectiva: crea un `<textarea>` real de HTML
(`ownerDocument.createElement("textarea")`, línea 435) posicionado en absoluto encima del
canvas, con `font`, tamaño, rotación y color calcados del elemento, y sincroniza su valor
con `originalText` en cada pulsación; al perder el foco o pulsar Escape/Enter según el
modo, confirma el texto llamando al pipeline de wrap descrito arriba. **Para Direct2D no
hay equivalente directo**: hay que decidir entre (a) un control `Edit`/`RichEdit` nativo
de Win32 superpuesto (más simple, replica el patrón de Excalidraw) o (b) un editor de
texto propio dibujado en D2D con caret y selección manejados a mano (más trabajo, pero
evita problemas de superposición de HWND sobre una superficie D2D con transformaciones/
rotación — el `<textarea>` de Excalidraw de hecho NO rota visualmente el control cuando
el contenedor está rotado más que con una transformación CSS, algo que un HWND de Win32
no puede hacer de forma nativa sin capas adicionales). Recomiendo (b) si se quiere
paridad visual con texto rotado, (a) si se acepta la limitación de no rotar mientras se
edita (igual que hacen muchos editores nativos).

### No explorado
Detalle fino de alineación horizontal por línea (`textAlign` aplicado línea a línea en
el render, en `renderElement.ts`, no releído); medida de fuentes reales vía subsetting
(`packages/excalidraw/subset/`, `fonts/`) — relevante sólo si se quiere usar las mismas
fuentes .woff2 (Excalifont, Virgil) en vez de fuentes del sistema.

---

## e) Imágenes

Ficheros: `packages/element/src/image.ts`, `cropElement.ts`;
tipos en `packages/excalidraw/types.ts:115-166`.

### Modelo de datos

```ts
type BinaryFileData = {
  mimeType: ImageMimeType | "application/octet-stream",
  id: FileId,            // string opaco (branded), referenciado por element.fileId
  dataURL: string,        // "data:<mime>;base64,..." — la imagen COMPLETA embebida inline
  created: number,        // epoch ms
  lastRetrieved?: number, // para basura recolectable: última vez que se cargó desde storage
  version?: number,
}
type BinaryFiles = Record<FileId, BinaryFileData>
```
Las imágenes NO se referencian por ruta de fichero: se embeben como `dataURL`
(base64) en un diccionario aparte (`files`), y el elemento sólo guarda el `fileId`. Este
diccionario viaja junto a `elements`/`appState` en el `.excalidraw` (ver sección g). Para
un motor nativo, el equivalente directo es un blob store indexado por hash/id — se puede
seguir usando base64 en el JSON por compatibilidad de fichero, pero internamente
convendría decodificar a un buffer de píxeles cacheado (bitmap de Direct2D) en memoria,
igual que Excalidraw mantiene un `imageCache` de `HTMLImageElement` ya decodificados
(`updateImageCache`, `image.ts:36-89`) para no decodificar el dataURL en cada frame.

### Estados de carga (`ExcalidrawImageElement.status`)

`"pending"` (fileId asignado pero el archivo aún no está en `files`/no se ha decodificado)
→ `"saved"` (decodificado y listo) → `"error"` (falló la decodificación). `updateImageCache`
guarda en el caché una PROMESA inmediatamente (antes de que resuelva) para que llamadas
concurrentes no dupliquen el trabajo de decodificación, y sólo tras el `await` reemplaza
la entrada por la imagen ya cargada.

### Transformaciones: escala, flip, recorte

- `scale: [number, number]` con valores `+1`/`-1` — el flip horizontal/vertical NO cambia
  `width`/`height` ni el contenido de `crop`, sólo el signo del `scale` correspondiente;
  el render aplica un `ctx.scale(scaleX, scaleY)` (o transform equivalente) antes de
  dibujar el bitmap.
- `crop: ImageCrop | null = {x, y, width, height, naturalWidth, naturalHeight}` — `x,y`
  y `width,height` están en coordenadas de la imagen ORIGINAL sin recortar
  ("uncropped"), NO en coordenadas naturales de píxel directamente: `cropElement.ts:33-82`
  calcula un factor `naturalWidthToUncropped = naturalWidth / uncroppedWidth` (donde
  `uncroppedWidth` es el ancho que tendría el elemento sin ningún recorte aplicado,
  derivado de sus dimensiones actuales) y usa ese factor para convertir entre el espacio
  "elemento" y el espacio "píxel natural de archivo".
- El algoritmo de recorte interactivo (arrastrar un tirador con la tecla de recorte
  activa) ajusta `nextWidth`/`nextHeight` con `clamp(...)` usando como límites la
  posición actual del recorte y si la imagen está volteada (`isFlippedByX/Y`) — el flip
  invierte qué lado del recorte es el "borde libre" vs "borde fijo": lo relevante para el
  port es que el recorte se representa siempre relativo al contenido SIN voltear, y el
  flip se aplica como transformación de render por encima, nunca horneado en `crop`.
- `MINIMAL_CROP_SIZE = 10` px — tamaño mínimo del área recortada.

### SVG (`normalizeSVG`, `image.ts:104-153`)

Los SVG se tratan como un caso especial de "imagen": se parsean con `DOMParser`, se
fuerza un `viewBox` si falta (derivado de `width`/`height`, con fallback a `50x50` si
ninguno de los dos está definido o usa `%`/`auto`), y se asegura el atributo `xmlns`. Para
un motor sin DOM, esto exige un parser SVG propio (o una librería como `resvg`/`usvg` en
Rust) que rasterice a un bitmap para insertarlo como imagen — Excalidraw en cambio
mantiene el SVG vectorial y lo re-renderiza al vuelo vía `<img>`/`<svg>` del DOM.

### No explorado
Pegado desde portapapeles / arrastrar-soltar de ficheros (probablemente en
`packages/excalidraw/clipboard.ts`, no leído); compresión/redimensionado antes de
guardar (`packages/excalidraw/data/blob.ts`, no leído).

---

## f) Interacción

### Hit-testing (`packages/element/src/collision.ts`, `distance.ts`)

Estrategia de dos fases, con caché de un solo elemento (`hitElementItself`,
`collision.ts:132-210`):

1. **Fase barata**: se prueba si el punto cae dentro del bounding box ROTADO con un
   margen de tolerancia (`isPointInRotatedBounds`, líneas 212-228) — se rota el PUNTO de
   consulta `-angle` alrededor del centro del bounding box (nunca se rota la geometría
   del elemento), y se compara contra `[bounds.x0-tol, bounds.y0-tol, bounds.x1+tol,
   bounds.y1+tol]`. Si falla y tampoco cae en el área del nombre del frame (si aplica),
   se descarta inmediatamente — el comentario del código dice explícitamente que esto
   ahorra el 99% del coste en la práctica.
2. **Fase precisa**: `shouldTestInside(element)` (líneas 82-102) decide si además de
   tocar el CONTORNO hay que considerar "dentro de la forma" como acierto:
   - Siempre falso para flechas (`arrow`).
   - Verdadero si el elemento tiene relleno opaco (`hasBackground && !isTransparent`),
     o tiene texto ligado, o es iframe/embeddable, o es un elemento de texto.
   - Para `line`/`freedraw`: sólo si además el trazo forma un bucle cerrado
     (`isPathALoop`, ver abajo) Y cumple lo anterior.
   - Las imágenes siempre cuentan como "testeable por dentro" (`isImageElement`).
   Si aplica, el acierto es `isPointInElement(...) || isPointOnElementOutline(...)`;
   si no, sólo el contorno.
3. `isPointOnElementOutline(point, element, elementsMap, tolerance=1) =
    distanceToElement(element, elementsMap, point) <= tolerance`.

**`distanceToElement`** (`distance.ts`, ver extracto completo arriba en el análisis) es
la pieza más reutilizable: descompone CADA tipo de elemento en un conjunto de
**segmentos de línea + curvas de Bézier** (mismos datos que produce
`deconstructRectanguloidElement`/`deconstructDiamondElement`/
`deconstructLinearOrFreeDrawElement` en `utils.ts`, usados también para el renderizado
"ideal" sin rugosidad) y calcula el MÍNIMO de:
- `distanceToLineSegment(punto, segmento)` sobre todos los lados rectos.
- `curvePointDistance(punto, curvaBezier)` sobre todas las esquinas curvas.

Para la elipse usa una fórmula cerrada específica (`ellipseDistanceFromPoint`, en el
paquete `@excalidraw/math/ellipse`, no releída en detalle — es la distancia punto-elipse
clásica, resoluble con Newton-Raphson sobre el parámetro angular, algoritmo bien conocido
y documentado en múltiples fuentes independientes de Excalidraw).

Este enfoque de "descomponer en primitivas simples y tomar el mínimo" es EXACTAMENTE lo
recomendable para reimplementar en Rust con las primitivas de geometría de `lyon` o
similares, o a mano con `f32`.

**Umbral de tolerancia por defecto**:
```
DEFAULT_TRANSFORM_HANDLE_SPACING = 2
SIDE_RESIZING_THRESHOLD = 2 * DEFAULT_TRANSFORM_HANDLE_SPACING = 4
DEFAULT_COLLISION_THRESHOLD = 2 * SIDE_RESIZING_THRESHOLD - EPSILON = 7.99999
```
(`common/src/constants.ts:218-225`, con `EPSILON=0.00001`).

**Detección de bucle cerrado** (`isPathALoop`, `utils.ts:477-491`):
```
si points.length>=3:
    distancia(primerPunto, últimoPunto) <= LINE_CONFIRM_THRESHOLD(=8px) / zoom
si no: false
```
El umbral se divide por el zoom actual para que a mucho zoom el umbral efectivo (en
espacio de pantalla) se mantenga razonable.

### Caché de hit-test

`hitElementItself` cachea el ÚLTIMO resultado (un solo slot global, no un mapa) por
`(point, elemento+version+versionNonce, threshold, overrideShouldTestInside,
frameNameBound)`, usando una `WeakRef` al elemento para no impedir su recolección de
basura. Un HIT cacheado sigue siendo válido para cualquier `threshold` mayor; un MISS
cacheado sólo sigue siendo válido para un `threshold` igual o menor (porque un umbral
mayor podría convertir el miss en hit). Vale la pena replicar esta микро-optimización
tal cual porque el hit-test se llama en cada `pointermove` sobre potencialmente cientos
de elementos.

### Selección múltiple y grupos (`packages/element/src/groups.ts`, `selection.ts`)

- `groupIds` en cada elemento es una PILA (array), ordenada de grupo más profundo (índice
  0) a más superficial. Pertenecer a un grupo no es exclusivo: un elemento puede estar en
  varios grupos anidados simultáneamente.
- `selectGroup(groupId, ...)` (`groups.ts:24-63`): si el grupo tiene menos de 2 elementos
  (grupo "roto"/inválido), lo deselecciona en vez de seleccionarlo; si no, añade TODOS los
  elementos con ese `groupId` en su pila a `selectedElementIds`.
- Al hacer clic sobre un elemento perteneciente a un grupo (fuera de modo edición de
  grupo), se selecciona el grupo COMPLETO más externo (comportamiento estándar de editor
  vectorial: clic = grupo, doble-clic = entra a editar el grupo/subgrupo,
  `editingGroupId` en el `AppState` rastrea en qué nivel de anidación se está editando).
- `getMaximumGroups`/`getNonDeletedGroupIds` (nombres vistos, no releídos línea a línea):
  utilidades para, dado un conjunto de elementos seleccionados, encontrar el grupo común
  más alto a agrupar/desagrupar.

### Tiradores de redimensión y rotación (`packages/element/src/transformHandles.ts`)

- 8 tiradores de redimensión (`n,s,e,w,nw,ne,sw,se`) + 1 de rotación, todos representados
  como `Bounds = [x,y,w,h]` en espacio de escena, YA rotados con el mismo ángulo que el
  elemento (`generateTransformHandle` aplica `pointRotateRads` al CENTRO del tirador
  alrededor del centro del elemento, con el mismo `angle`).
- Tamaño del tirador depende del tipo de puntero: `mouse: 8px, pen: 16px, touch: 28px`
  (`transformHandleSizes`), dividido por el zoom actual para mantener tamaño constante en
  pantalla.
- El tirador de rotación se coloca a `ROTATION_RESIZE_HANDLE_GAP = 16px` (también entre
  zoom) por encima del tirador `n`.
- Los tiradores intermedios (`n,s,e,w`, no esquineros) sólo se muestran si el lado
  correspondiente del bounding box supera `5 * tamañoTiradorRatón / zoom` en pantalla —
  evita amontonar tiradores en elementos muy pequeños o muy delgados.
- Máscaras de omisión (`OMIT_SIDES_FOR_*`): selección múltiple oculta los 4 cardinales
  (sólo esquineros); un frame oculta TODO incluida la rotación (los frames no rotan);
  una línea perfectamente diagonal (`OMIT_SIDES_FOR_LINE_SLASH`/`BACKSLASH`) oculta los
  tiradores que quedarían redundantes con sus propios extremos.
- El redimensionado en sí (cálculo de nuevas `x,y,width,height,points` a partir de la
  posición del puntero y el tirador arrastrado) vive en `resizeElements.ts`
  (**no releído en profundidad** — es la lógica más "de UI" y menos "algoritmo
  reutilizable" de todo el módulo; para un motor nuevo se puede diseñar de cero siguiendo
  el mismo contrato de tiradores/umbrales de arriba sin necesidad de portar línea a línea).

### No explorado en profundidad
`resizeElements.ts` (mecánica fina de arrastre de cada tirador, mantenimiento de
proporción con Shift, redimensionado simétrico con Alt); `binding.ts` (enlace
puntero-de-flecha a formas — sólo se documentaron aquí las constantes de gap,
`BASE_BINDING_GAP=5px` (+`strokeWidth/2` del objetivo), `BASE_ARROW_MIN_LENGTH=10px`,
y la distancia máxima de "imán" `maxBindingDistance_simple` que crece hasta `2x` la
distancia base cuando el zoom es bajo); `dragElements.ts`; `eraser/index.ts` (confirmado
que el borrador NO es un borrador de píxeles: dibuja un trazo con la misma librería
`laser-pointer` que el modo freedraw "constante", y cualquier elemento cuyo contorno
intersecte ese trazo se marca completo para borrado — es borrado POR ELEMENTO, no por
píxel); `bucketFill.ts` (relleno "cubo de pintura" vectorial: construye un arreglo planar
de segmentos a partir de los contornos de los elementos cercanos al clic, localiza la
cara cerrada más pequeña que contiene el punto, y la islas interiores se empalman como
"agujeros" tipo keyhole en el polígono resultante — es un algoritmo de geometría
computacional no trivial, con tolerancias ajustadas a mano
(`BUCKET_FILL_GAP_TOLERANCE=6px`, `BUCKET_FILL_REGION_MATCH_TOLERANCE=2px`,
`BUCKET_FILL_COVER_MARGIN=2px`, `BUCKET_FILL_CURVE_MAX_DEVIATION=0.5px`); recomiendo
tratarlo como feature de fase tardía, implementado con una librería de arreglos planares
en Rust en vez de portar línea a línea).

---

## g) Formato de fichero `.excalidraw`

Ficheros: `packages/excalidraw/data/json.ts`, `packages/excalidraw/appState.ts`,
`packages/common/src/constants.ts`.

### Estructura de nivel superior

```json
{
  "type": "excalidraw",
  "version": 2,
  "source": "https://excalidraw.com"   // o window.EXCALIDRAW_EXPORT_SOURCE si está definido
  "elements": [ /* ExcalidrawElement[] completos, incluye isDeleted:false ya filtrados según contexto */ ],
  "appState": { /* subconjunto MUY reducido, ver abajo */ },
  "files": { /* BinaryFiles, sólo para guardado LOCAL, se omite en export a "database" */ }
}
```

`serializeAsJSON` (`json.ts:52-75`): `JSON.stringify(data, null, 2)` (indentado a 2
espacios). `type`/`version` vienen de `EXPORT_DATA_TYPES.excalidraw = "excalidraw"` y
`VERSIONS.excalidraw = 2` (`constants.ts:285-290,352-355`). Antes de serializar, se
descartan de `files` las entradas cuyo `fileId` no está referenciado por ningún elemento
NO borrado (`filterOutDeletedFiles`, líneas 34-50) — es decir, el `.excalidraw` en disco
nunca acarrea imágenes huérfanas ni de elementos borrados.

### Qué campos de `appState` sobreviven al guardar en fichero (`export: true` en
`APP_STATE_STORAGE_CONF`, `appState.ts:145-270`)

De las ~70 claves del `AppState` completo (usado en memoria/localStorage), **sólo 5**
tienen `export: true` y por tanto acaban en el `.excalidraw`:
```
gridSize, gridStep, gridModeEnabled, viewBackgroundColor, lockedMultiSelections
```
Todo lo demás (selección actual, herramienta activa, zoom, scroll, tema, colores
"actuales" de la paleta, estado de diálogos, etc.) es efímero de sesión y NO se persiste
en el fichero — sólo en `localStorage`/IndexedDB del navegador (claves marcadas
`browser: true`, un conjunto bastante más amplio, ver el objeto completo citado arriba).
Esto es una simplificación importante para el port: el "documento" real es
`elements + files + ese puñado de ajustes de vista`, todo lo demás es estado de UI de la
sesión de edición actual y se puede diseñar desde cero en Rust sin replicar el resto del
`AppState` de React.

### Elementos: normalización al cargar (`packages/excalidraw/data/restore.ts`)

Al leer un fichero, cada elemento pasa por `restoreElement`/`restoreElementWithProperties`
(líneas 415-501+) que rellena valores por defecto para campos ausentes o de esquemas
antiguos — los más relevantes para reproducir el "look" exacto al reabrir un fichero:
```
version:      element.version || 1
versionNonce: element.versionNonce ?? 0
id:           element.id || randomId()
seed:         element.seed ?? 1                 // NO aleatorio — determinismo al recargar
```
Es decir: un fichero legado sin `seed` reproduce el MISMO aspecto "a mano" cada vez que se
abre (seed=1 fijo), nunca algo aleatorio — replicar este valor por defecto exacto es
necesario para que reabrir ficheros antiguos coincida pixel a pixel con el original.

### Orden de pintado / z-order: fractional indexing

`element.index: FractionalIndex | null` es un STRING (alfabeto base-62,
`packages/fractional-indexing/src/*`, vendorizado de `github.com/rocicorp/fractional-indexing`,
licencia CC0) que ordena lexicográficamente. Permite insertar un elemento "entre" otros
dos sin renumerar todo el array — imprescindible en colaboración en tiempo real (dos
usuarios pueden insertar "entre A y B" simultáneamente sin conflicto de índices enteros).
**Para una app de un solo usuario sin colaboración, esto es sobre-ingeniería que no hace
falta portar**: basta con el orden real del `Vec<Element>` en memoria (mover al frente/
fondo = `Vec::remove`+`insert`, o un simple contador `u64` de z-order si se prefiere
evitar el coste de reordenar el vector). Documentado aquí para que quede explícito que
es una de las piezas "peso muerto" pensadas para colaboración (ver sección final).

### Ficheros de biblioteca (`.excalidrawlib`)

`serializeLibraryAsJSON`: mismo patrón, `type: "excalidrawlib"`,
`version: VERSIONS.excalidrawLibrary = 2`, `libraryItems` en vez de
`elements/appState/files`. No profundicé en la estructura de `LibraryItems` — es
opcional para un motor de edición base (biblioteca de formas reutilizables, feature de
fase tardía).

### No explorado
Migraciones de esquema de versiones anteriores de fichero (`restore.ts` tiene lógica de
compatibilidad retro extensa, sólo se leyeron los defaults puntuales citados arriba);
formato de portapapeles (`excalidraw/clipboard`); exportación a PNG/SVG
(`packages/utils/src`, `renderer/staticSvgScene.ts`) — relevante sólo para "exportar",
no para el modelo de datos del editor en sí.

---

## h) Herramientas y máquina de estados del puntero

Fichero: `packages/excalidraw/components/App.tsx` (14 028 líneas — sólo se leyeron
fragmentos dirigidos, no el fichero completo). Lista de herramientas
(`packages/excalidraw/types.ts:147-165`, tipo `ToolType`):

```
selection | lasso | rectangle | diamond | ellipse | arrow | line | freedraw | text |
image | eraser | hand | frame | magicframe | embeddable | laser | autoshape | bucketfill
```

### Patrón general del ciclo de puntero

`handleCanvasPointerDown` (línea 8372) es el despachador central. Tras normalizar el
evento (coordenadas de escena, snapping a rejilla, detección de doble clic, etc.), hace
un `if/else if` por `activeTool.type` (líneas ~8780-8838) que delega en un manejador
específico y luego, de forma UNIFORME para todas las herramientas, registra listeners
de `pointermove`/`pointerup`/`keydown`/`keyup` en el `window` (no en el canvas) para el
resto de la interacción hasta soltar el puntero (líneas 8854-8876) — es decir, cada
"gesto" de herramienta es una máquina de estados de 3 fases (down → move* → up) con los
manejadores de `move`/`up` creados como closures que capturan el `pointerDownState`
inicial (`onPointerMoveFromPointerDownHandler`, `onPointerUpFromPointerDownHandler`).

Reglas de despacho relevantes por herramienta (extracto literal, líneas 8780-8838):

| Herramienta | pointerDown | Notas |
|---|---|---|
| `selection` (clic en vacío) | `createGenericElementOnPointerDown("selection", ...)` → crea un `ExcalidrawSelectionElement` (rectángulo de goma elástica, nunca se inserta en la escena real) | ver detalle abajo |
| `text` | `handleTextOnPointerDown` | entra directo en modo edición (wysiwyg) |
| `arrow`, `line` | `handleLinearElementOnPointerDown(event, tipo, ...)` | multi-punto: cada clic añade un vértice, doble-clic o Escape termina |
| `freedraw` | `handleFreeDrawElementOnPointerDown` | crea el elemento inmediatamente con 1 punto, cada `pointermove` añade puntos (ver sección b) |
| `frame`, `magicframe` | `createFrameElementOnPointerDown` | |
| `laser` | `laserTrails.startPath(...)` | NO crea ningún `ExcalidrawElement` — es un trazo puramente visual efímero, se desvanece solo, nunca se persiste |
| `autoshape` | `drawShape.handlePointerDown(...)` | reconocimiento de forma a partir del gesto dibujado a mano (p.ej. dibujar un rectángulo a mano alzada y que se auto-convierta) — **no explorado** |
| `bucketfill` | `bucketFill.handlePointerDown(...)` | herramienta de "un solo clic": arma el relleno en pointerDown, pero CONFIRMA en el pointerUp compartido sólo si la interacción se mantuvo como un click de un solo puntero (comentario explícito en el código: un segundo dedo, un menú contextual o un `pointercancel` la aborta) |
| `custom` | `cursor.applyForTool()` | tipo de puntero definido por integraciones externas (hook `onPointerDown` público) — no aplica a un motor standalone |
| cualquier otra (`rectangle`, `diamond`, `ellipse`, `embeddable`) | `createGenericElementOnPointerDown(tipo, ...)` | crea el elemento con tamaño 0 en el punto de clic, el arrastre posterior lo redimensiona |
| `eraser` | (rama aparte, línea 8847) | inicia `eraserTrail.startPath(...)` DESPUÉS del despacho principal — el borrador no crea ningún `ExcalidrawElement` nuevo, sólo un trazo de detección de colisión (ver sección f) |

`createGenericElementOnPointerDown` (líneas 10394-10450), el caso más común
(rectángulo/elipse/diamante/embeddable), en detalle:
```
1. Snap del punto de clic a rejilla (getGridPoint, salvo Ctrl/Cmd presionado)
2. Detecta si el clic cae dentro de un frame existente → asigna frameId
3. Construye baseElementAttributes desde los "currentItem*" del AppState
   (colores, grosor, estilo de trazo, rugosidad, opacidad, redondeo — es decir,
   los "últimos valores usados" que el usuario configuró en la barra de propiedades)
4. Crea el elemento con newElement({type, ...baseElementAttributes}) — ancho/alto = 0
5. Si type==="selection": sólo actualiza el estado (selectionElement), NO se inserta
   en la escena de elementos reales
6. Si no: this.insertNewElement(element) (lo añade a la escena YA, con tamaño 0) y
   guarda una referencia en appState.newElement — el pointermove subsiguiente
   simplemente ajusta width/height/x/y de ESE MISMO elemento (nunca crea uno nuevo)
```
`createFrameElementOnPointerDown` sigue el mismo patrón pero con `FRAME_STYLE` fijo
(colores/grosor no configurables por el usuario para frames:
`strokeColor:"#bbb", strokeWidth:2, strokeStyle:"solid", fillStyle:"solid",
roughness:0, backgroundColor:"transparent"`, `constants.ts:196-210`).

### Vuelta a la herramienta de selección tras dibujar

No se releyó el código exacto de esta transición, pero se infiere de
`activeTool.locked` (visto en `getDefaultAppState`, línea 55: `locked:
DEFAULT_ELEMENT_PROPS.locked`) y del patrón estándar de Excalidraw conocido: si el
candado de herramienta NO está activado, tras soltar el puntero la herramienta activa
vuelve a `selection` automáticamente; si está activado (`locked: true`), la herramienta
permanece activa para dibujar varios elementos seguidos sin tener que reseleccionarla
cada vez. **No confirmado línea a línea, marcarlo para verificar contra tests antes de
depender de este detalle exacto.**

### No explorado en profundidad
El grueso de `App.tsx` (14 028 líneas) más allá de los fragmentos citados: el manejo
completo de `pointermove` por herramienta (redimensionado en vivo, snapping a otros
elementos, binding de flechas en vivo), `handleLinearElementOnPointerDown` (máquina de
estados multi-clic completa para arrow/line), `handleTextOnPointerDown`,
`drawShape` (reconocimiento de forma a mano alzada), atajos de teclado, gestos táctiles
multi-dedo (pinch-zoom, pan de dos dedos), panel de propiedades y sus acciones
(`packages/excalidraw/actions/`). Recomiendo diseñar la máquina de estados de la app
Rust desde los PRINCIPIOS extraídos aquí (despachador único por `pointerdown`, closures/
handlers de `move`/`up` que capturan el estado inicial del gesto, elemento "en
construcción" mutado in-place durante el arrastre) en vez de intentar traducir
`App.tsx` línea a línea — es una clase de UI de React de 14k líneas, no un módulo de
algoritmo aislado.

---

## Qué portar y qué no

### Traducción directa (algoritmo puro, sin dependencias de plataforma web)

- **Modelo de elementos** (sección a): estructuras de datos casi 1:1 a `struct`/`enum`
  Rust. `angle: Radians` como `f32` con newtype; `roundness` como `enum` +
  `Option<f32>`.
- **perfect-freehand** (sección b): es matemática pura (vectores 2D, sin estado
  compartido salvo el propio array de puntos de un trazo). Portar `getStrokePoints` +
  `getStrokeRadius` + `getStrokeOutlinePoints` literalmente, con los mismos nombres de
  función para poder comparar contra el original al depurar. El resultado (polígono) se
  sube a un `ID2D1PathGeometry` vía `GeometrySink` con `AddQuadraticBezier` en vez de
  generar un string SVG.
- **El generador Lehmer/LCG de rough.js y su uso para `seed`/`versionNonce`**
  (sección c): esto es EXACTAMENTE lo que hay que hacer bit-a-bit igual, porque ya existe
  un `Rand.kt` equivalente en el lado Android que hay que igualar — la fórmula
  `seed = (seed.wrapping_mul(48271)) & 0x7FFF_FFFF; result = seed as f64 / 2^31` no admite
  aproximaciones.
- **Toda la generación "a mano" de rough.js** (`_line`, `_curve`, elipse, hachure/zigzag/
  cross-hatch): es geometría + el mismo LCG, totalmente portable, y es la pieza que más
  define la identidad visual de la app. Vale la pena portarla ANTES que reescribir el
  render "limpio", porque es la única forma de que dos elementos con el mismo `seed`
  se vean IGUAL entre Android y Windows.
- **Wrapping de texto** (sección d): el algoritmo greedy de `wrapLine`/`wrapWord` es
  portable tal cual; sustituir las regex Unicode a mano por el crate `unicode-linebreak`
  (o `icu_segmenter`) para las reglas UAX #14, que es más robusto que reimplementar las
  regex de `textWrapping.ts` a mano.
- **Distancia punto-elemento / hit-testing** (sección f): el patrón "descomponer en
  segmentos + curvas de Bézier y tomar el mínimo" es agnóstico de plataforma y encaja
  bien con la geometría de Direct2D (`ID2D1PathGeometry::CompareWithGeometry` /
  `FillContainsPoint` / `StrokeContainsPoint` incluso podrían sustituir parte de esta
  lógica usando la geometría YA construida para el render, en vez de mantener un cálculo
  de distancia completamente aparte — a evaluar como simplificación).
- **Fórmulas de contenedor de texto** (offsets/máximos por tipo de forma en la sección d):
  triviales de portar, son sólo aritmética.

### Rehacer con mejor camino en Rust/Direct2D

- **Medición de texto**: en vez de emular `canvas.measureText`, usar directamente
  `IDWriteTextLayout`/`IDWriteFactory::CreateTextLayout` de DirectWrite — da métricas más
  ricas (por-glyph, no sólo advance width) y layout/wrapping propio ya optimizado; el
  wrapping manual de Excalidraw se puede reservar sólo para el caso "texto ligado a
  contenedor con `originalText` propio" donde se necesita control fino, y dejar que
  DirectWrite haga el wrapping normal de texto libre.
- **Edición de texto in-situ**: no hay equivalente a superponer un `<textarea>` HTML; hay
  que elegir entre un control Win32 nativo superpuesto (simple pero no rota con el
  elemento) o un editor de caret/selección dibujado a mano en D2D (más trabajo, mejor
  paridad visual). Recomendado: editor propio desde el principio, ya que "texto rotado
  editable" es un caso de uso plausible en una app de captura/anotación.
- **Hit-testing de geometría ya construida**: como se dijo arriba, evaluar usar los
  métodos nativos de `ID2D1Geometry` (que YA tiene que existir para pintar el elemento)
  en vez de mantener una segunda representación paralela sólo para hit-testing —
  Excalidraw lo hace por separado porque el "shape" cacheado de rough.js es específico
  del renderer roughjs (operaciones de path con desplazamientos aleatorios), no la forma
  geométrica ideal; en Rust probablemente convenga cachear AMBAS representaciones
  (la "ideal" limpia para hit-test/bindings, la "a mano" con jitter sólo para pintar) tal
  como de hecho hace el propio Excalidraw internamente (`deconstructRectanguloidElement`
  para geometría ideal vs `ShapeCache`/rough.js `Drawable` para el dibujo).
- **Persistencia de imágenes**: se puede mantener compatibilidad de fichero
  (`dataURL` base64 embebido) pero cachear en memoria como bitmaps de Direct2D
  (`ID2D1Bitmap`) ya decodificados, igual que Excalidraw cachea `HTMLImageElement` — el
  concepto es el mismo, la implementación cambia por completo.
- **Bucket fill vectorial**: si se quiere esta función, construirla desde cero con una
  librería de arreglos planares/geometría computacional de Rust (p.ej. algo basado en
  `robust`/`spade`/estructuras half-edge propias) en vez de traducir línea a línea
  `bucketFill.ts` (1918 líneas de geometría muy afinada a mano para el DOM/roughjs).

### Peso muerto de la web — NO portar

- **React / DOM / CSS**: obviamente todo el árbol de componentes de
  `packages/excalidraw/components/`; el patrón "closures capturando estado del gesto de
  puntero" (sección h) sí vale la pena como INSPIRACIÓN de diseño, pero implementado
  sobre el bucle de mensajes de Win32/D2D propio, no sobre React.
- **Fractional indexing (`element.index`)**: diseñado explícitamente para que dos
  peers puedan insertar elementos "entre" otros dos sin coordinarse (colaboración en
  tiempo real). Un editor de un solo usuario local no lo necesita — basta el orden real
  de un `Vec<Element>` (o un contador de z-order simple). Documentado en la sección g.
- **`version`/`versionNonce` como mecanismo de reconciliación multi-peer**: el CONCEPTO
  de "contador de revisión + nonce aleatorio para desempate" sigue siendo útil para
  undo/redo local (detectar si un elemento cambió desde el último snapshot), pero toda la
  lógica de fusión de estados divergentes entre dos clientes (colaboración en vivo,
  `reconcile.ts` en `packages/excalidraw/data/`, no leído — nombre indica claramente su
  propósito) es exclusiva de colaboración y no aplica a una app de escritorio de un solo
  usuario.
- **Colaboración en general**: cualquier cosa con "collab", `reconcile.ts`,
  sincronización de cursores de otros usuarios (`collaborators: Map` en `AppState`) —
  cero relevancia para PixPin Max.
- **Iframes / Embeddables / `magicframe` (IA)**: contenido HTML embebido en el lienzo y
  generación por IA de contenido dentro de un frame — no tiene sentido en un motor
  nativo de captura/anotación de escritorio salvo que se decida soportar "insertar un
  navegador embebido", que es una decisión de producto totalmente aparte.
- **`nanoid` para IDs**: cualquier generador de IDs únicos de Rust (`uuid`, o incluso un
  contador local ya que no hay colaboración) sustituye esto sin pérdida — Excalidraw sólo
  necesita IDs únicos globalmente por la colaboración; localmente basta con unicidad
  dentro del documento.
- **Persistencia en `localStorage`/IndexedDB del navegador**: la lista de campos de
  `AppState` marcados `browser: true` (sección g) es la lista de "qué recordar entre
  sesiones de la app" — útil como REFERENCIA de qué ajustes de UI conviene persistir en
  un `.ini`/registro/fichero de config propio de Windows, pero el mecanismo de
  almacenamiento en sí no aplica.
- **`hachure-fill`, `points-on-curve`, `points-on-path`, `path-data-parser`**: paquetes
  auxiliares atados al formato de datos SVG/Canvas 2D de la web; en Rust cada uno tiene
  equivalente más idiomático (crates de geometría computacional, parseo de paths SVG con
  `usvg` si hiciera falta importar SVG externos) en vez de traducir estos paquetes.
- **Exportación a PNG/SVG del propio Excalidraw** (`packages/utils`,
  `renderer/staticSvgScene.ts`): un motor sobre Direct2D ya tiene su propio pipeline
  natural de "renderizar a bitmap"/"guardar imagen" mucho más directo que emular el
  pipeline de generación de SVG/Canvas offscreen de Excalidraw.

---

## Resumen de constantes clave para referencia rápida al implementar

```
LCG rough.js:            seed = (seed * 48271) & 0x7FFFFFFF (mult. 32 bits truncada); next = seed / 2^31
Freedraw variable width:  size = strokeWidth*4.25, thinning=0.6, smoothing=0.5,
                          streamline = 0.5 (ratón) | 0.2 (pluma/touch), easing = sin(t*PI/2)
rough.js defaults:        maxRandomnessOffset=2, bowing=1, curveTightness=0, curveFitting=0.95,
                          curveStepCount=9, fillWeight=strokeWidth/2, hachureGap=strokeWidth*4,
                          hachureAngle=-41°
ROUGHNESS:                architect=0, artist=1 (por defecto), cartoonist=2
ROUNDNESS:                LEGACY=1, PROPORTIONAL_RADIUS=2 (25% del lado mayor), ADAPTIVE_RADIUS=3 (32px fijos)
STROKE_WIDTH:             thin=1, medium=2 (por defecto), bold=4
FREEDRAW_STROKE_WIDTH:    thin=0.5, medium=1, bold=2
DEFAULT_FONT_SIZE:        20px, familia por defecto = Excalifont (id numérico 5)
Line-height:              1.25 por defecto, 1.2 para fuentes monoespaciadas (Cascadia)
BOUND_TEXT_PADDING:       5px
LINE_CONFIRM_THRESHOLD:   8px / zoom (detección de bucle cerrado en freedraw/line)
DEFAULT_COLLISION_THRESHOLD: ~7.99999px (2*4 - epsilon)
BASE_BINDING_GAP:         5px (+ strokeWidth/2 del objetivo); 5px también para flechas "elbow"
Tirador de transformación: 8px (ratón) / 16px (pluma) / 28px (touch), dividido por zoom
Ficheros:                 type:"excalidraw", version: 2 (esquema de fichero, NO confundir con element.version)
appState persistido en fichero: sólo gridSize, gridStep, gridModeEnabled, viewBackgroundColor, lockedMultiSelections
```
