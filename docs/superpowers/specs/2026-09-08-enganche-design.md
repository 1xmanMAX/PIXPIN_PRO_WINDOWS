# El imán: enganche a los puntos notables

Fecha: 2026-09-08.
Fase **B.6** del plan `docs/superpowers/plans/2026-09-06-android-a-windows.md`.
Fuente del port: `proyectos de referencia/PIXPIN_PRO_ANDROID`, ficheros
`motor/Iman.kt` (167 líneas) y `motor/Snapping.kt` (350). Nada de ese
directorio se ha modificado.

---

## Por qué esto, y por qué ahora

Es lo que separa un esquema de un montón de rayas casi alineadas. Con el ratón
se falla la esquina exacta de un rectángulo por dos o tres píxeles, y esos dos
o tres píxeles se ven.

Va **ahora**, justo detrás de medir, por dos razones que no son de gusto:

1. **Sin él, calibrar es adivinar.** La herramienta `Escalar` pide picar los
   dos extremos de algo de medida conocida —una pared, una cota del plano—.
   Hoy eso se hace a pulso, y el error entra directo en la escala: todo lo que
   se mida después hereda el fallo, multiplicado. El imán es lo que convierte
   ese gesto en exacto.
2. **La fase B.4 entera cuelga de él.** El inventario del motor Android
   describe «puntos etiquetados: un clic imantado», «recortar y extender: un
   clic sobre el tramo», «alfileres: el radio de agarre». Construir B.4 antes
   del imán es construirla contra una pieza que no existe.

---

## Alcance: qué entra y qué no, y por qué

`Snapping.kt` no son 350 líneas aisladas: tira de media docena de piezas que en
el Rust no existen. El inventario honesto, medido contra el árbol:

| Lo que necesita el original | ¿Está en el Rust? |
|---|---|
| Caja del elemento, esquinas giradas | **Sí** — `Elemento::caja()`, `impacto::esquinas_giradas` |
| Perímetro de una figura cualquiera (`puntoEnElPerimetro`) → ancla BORDE | No |
| Cruce de dos figuras (`interseccionesCerca`, `contornosDe`) → ancla INTERSECCIÓN | No |
| Instrumentos (plano, recta, espacio) y sus marcas | No, **ni la herramienta existe en Windows** |
| Tablas de coordenadas y eje | No, **ni la herramienta existe** |
| La marca de «guía» en el elemento (`reference`) | No |

**Entra:** la arquitectura entera —tabla de faenas, prioridad, radio en
píxeles de pantalla, interruptores por clase de punto— y las anclas que las
once figuras de Windows saben ofrecer: **esquina, extremo, medio y centro**.

**No entra:** BORDE e INTERSECCIÓN, que son reales y se echarán de menos —el
propio autor del Android dice de la intersección que «es el que más se echa de
menos»— pero las dos piden un muestreador de contornos de figura arbitraria,
que es una pieza aparte del bloque de geometría y merece su propia tanda.

**No entra nunca así:** los anclajes de instrumentos y de tablas. Son de
herramientas que Windows no tiene; escribirlos hoy es código que nace muerto y
sin forma de probarlo de verdad.

*Por qué partir así y no hacerlo entero:* la parte que hay que acertar a la
primera es la **estructura**, porque de su interfaz cuelgan cinco herramientas
de B.4. Añadir un `TipoAnclaje` después es aditivo: una variante más, una fila
más en la tabla de prioridad, y nada que reestructurar. *Coste si me equivoco:*
una tanda más de la cuenta, y mientras tanto un imán que no engancha a cantos
ni a cruces —que es exactamente el imán que tuvo el Android durante su primer
año.

---

## Dónde vive

`crates/pixpin-motor2d/src/enganche.rs`. Capa 1, lógica pura, sin escritorio:
se le dan elementos y un punto, y dice a dónde habría que pegarse. Así se
comprueba sin dispositivo que engancha donde debe y —más importante— que **no**
engancha donde no debe.

**Un solo fichero, no dos.** En el Android son dos porque `Snapping.kt` nació
primero y `Iman.kt` llegó después a poner orden sobre seis llamadores sueltos
que decidían cada uno por su cuenta si enganchar, con qué radio y a qué.
Naciendo de cero esa costura no tiene razón de ser: hay una función pública que
es la puerta, y debajo lo privado.

---

## La interfaz

```rust
pub enum TipoAnclaje { Esquina, Extremo, Medio, Centro }

pub struct Anclaje { pub punto: Punto2, pub tipo: TipoAnclaje, pub id: u64 }

pub enum Faena { Trazando, AMano, Afinando, Moviendo }

pub struct Ajustes {
    pub activo: bool,
    /// Gobierna `Esquina` **y** `Extremo`: para el usuario son la misma idea
    /// —el vértice de algo—, y en el Android el mismo interruptor los apaga
    /// a los dos. Dos casillas para eso serían dos casillas que nadie
    /// entiende por separado.
    pub esquinas: bool,
    pub medios: bool,
    pub centros: bool,
    /// Radio de captura en PÍXELES DE PANTALLA, no de escena.
    pub radio_px: f32,
}

/// La única puerta. Nadie busca anclajes por su cuenta.
pub fn sitio(
    elementos: &[Elemento],
    p: Punto2,
    zoom: f32,
    faena: Faena,
    ajustes: &Ajustes,
    excluir: Option<u64>,
) -> Option<Anclaje>;
```

**El radio va en píxeles de pantalla y se divide por el zoom dentro.** Es donde
ocurre el problema: el cursor apunta con la misma precisión mires al zoom que
mires. Si el radio fuese de escena, muy acercado engancharía a medio dibujo y
muy alejado no engancharía a nada.

**No se incluye `Faena::SitioNotable`.** Es la del punto etiquetado, que es
B.4: hoy no la construiría nadie. Llega con su herramienta.

---

## La tabla de faenas

No se pregunta «qué herramienta hay puesta» sino **qué está pasando**. Afinar
la punta de una flecha ya trazada y afinar la de una recta son la misma faena
aunque sean dos herramientas; trazar a mano alzada es otra aunque el lápiz y el
marcador sean dos botones. Es lo que hace que una herramienta nueva no tenga
que tocar el imán: encaja en una faena y hereda su comportamiento entero.

| Faena | Ajustes que hereda | Por qué |
|---|---|---|
| `Trazando` | los del usuario, tal cual | Nace una figura y se arrastra |
| `Afinando` | los del usuario, tal cual | Se corrige una punta, no se traza |
| `Moviendo` | los del usuario con **radio × 1,5** | Se busca sitio, no se traza: no hay trazo en curso al que dar un tirón, así que se puede ser generoso sin que estorbe |
| `AMano` | **ninguno** | Ver abajo |

**Lo que el usuario apaga se queda apagado.** La faena puede quitar clases de
punto, nunca encender lo que se ha desactivado a mano. Al revés sería un ajuste
que no se obedece, que es peor que no tenerlo.

### `AMano` no engancha a nada, y eso es una guarda

Con este alcance, el lápiz y el marcador no enganchan. No es un descuido: a
mano alzada el Android solo permite el canto de una guía, y aquí no hay ni
canto ni guías todavía.

La variante existe igualmente y devuelve siempre `None`. **No es código muerto,
es lo que hace imposible un error concreto:** sin ella, el primero que cablee
el lápiz al imán lo cableará a `Trazando`, y se comerá el fallo del que el
Android avisa por escrito —*«un trazo que salta a un vértice en mitad del
recorrido no se corrige, se rompe: el garabato pega un tirón y sigue»*—. El día
que exista el ancla BORDE, `AMano` se enciende sola y el lápiz empieza a
resbalar por los cantos sin que nadie recuerde nada.

---

## De `Estado` a `Faena`

El mapeo vive en `gesto.rs`, y sale limpio de la máquina de estados que ya
existe:

| `Estado` de `gesto.rs` | `Faena` |
|---|---|
| `Dibujando` con Lápiz o Resaltador | `AMano` |
| `Dibujando` con figura, y el `Pulsar` que la hace nacer | `Trazando` |
| **`Calibrando`** | **`Trazando`** |
| `Moviendo` | `Moviendo` |
| `Escalando`, `Girando` | `Afinando` |
| `Marquesina` | ninguna: seleccionar no engancha |

La fila que justifica la fase es `Calibrando`. Es la que hace que picar los dos
extremos de una pared de medida conocida deje de ser puntería.

---

## Tres trampas del árbol de Windows, ya medidas

No son detalles de implementación: son sitios donde el port se rompe en
silencio si nadie las nombra.

### 1. `Elemento::caja()` NO tiene en cuenta el giro

Devuelve la caja **local sin girar** (`x, y, x+ancho, y+alto`). El giro lo
aplica `impacto::esquinas_giradas` después. El Android criba con
`getElementBounds`, que **sí** es la caja girada.

Si la criba usa `caja()` a secas, un rectángulo girado 45° cuya esquina real
cae dentro del radio se descarta antes de mirarla, porque esa esquina está
fuera de su caja local. El fallo es silencioso y solo aparece con elementos
girados: el imán «a veces no engancha».

**Invariante que lo fija, y es probable:** *la criba no puede descartar un
elemento que tuviera un ancla dentro del radio*. La prueba es un rectángulo
girado con una esquina dentro del radio y fuera de la caja local. La fórmula
concreta —caja girada, o caja local inflada— es decisión de implementación
mientras cumpla el invariante.

### 2. `esquinas_giradas` no vale para las figuras con puntos

Se apoya en `caja()`, que para lápiz, línea, flecha y cota es la caja de sus
puntos. Sus cuatro esquinas son las de un rectángulo que no existe: la caja de
una diagonal tiene dos esquinas por las que no pasa la línea.

Es lo mismo que dice el Android —*«los lineales dan sus extremos y su medio; su
caja no significa nada»*—. Así que las figuras con puntos ofrecen
`Extremo` (primero y último) y `Medio` (el punto de en medio de la lista), y
**nunca** pasan por `esquinas_giradas`.

### 3. La caja de las figuras con puntos viene inflada por el grosor

`caja()` les suma `grosor / 2` por cada lado. Para la criba es inofensivo —una
caja más grande solo es más permisiva, nunca descarta de más— pero conviene
saberlo para no perseguir un fantasma al comparar con el Android, que no lo
hace.

---

## Prioridad y desempate

Gana **el más cercano**. En empate —diferencia menor que 0,001 de escena— gana
el más prioritario:

| Tipo | Prioridad | Por qué ahí |
|---|--:|---|
| `Esquina`, `Extremo` | 1 | Un vértice es lo más intencionado que hay |
| `Medio` | 2 | |
| `Centro` | 3 | El centro de una caja grande está lejos de todo, y engancharse a él por sorpresa desconcierta más de lo que ayuda |

---

## Rendimiento: una mejora sobre el original

El Android monta una `List<Anclaje>` por elemento y luego la tira. Aquí
**`sitio()` no monta ninguna lista**: criba por caja —una resta— y recorre los
candidatos quedándose con el mejor.

Dos motivos, y el segundo manda:

1. La criba por caja es lo que hace soportable un plano importado. Sin ella,
   cada movimiento del ratón montaría los anclajes de miles de elementos para
   tirarlos. El Android la tiene y su comentario cuenta que la app «se
   atragantaba hasta morir justo al dibujar encima» sin ella.
2. Esto corre en cada movimiento del ratón, o sea **en el camino caliente**,
   donde el presupuesto de rendimiento de este proyecto se compromete a **cero
   asignaciones**. A diferencia del Android, aquí eso está vigilado por
   `crates/pixpin-motor2d/tests/asignaciones.rs`, y el imán entra en esa
   vigilancia.

---

## Los ajustes

Un imán que tira cuando no quieres es peor que no tenerlo, y cuál estorba
depende de lo que estés dibujando. Por eso cada clase de punto se apaga por
separado.

**No hay tecla que lo anule en caliente** en esta tanda —ni `Ctrl` mantenido ni
un conmutador tipo F9—. Se apaga desde los ajustes, como en el Android. *Coste
si nos equivocamos:* si al usarlo estorba, la tecla se añade después; lo que
cuesta entonces es llevar `ctrl` hasta `EventoGesto::Pulsar` y `Mover`, que hoy
solo llevan `shift` y `alt`. Es mecánico y el compilador señala cada sitio.

**Consecuencia que hay que asumir:** al no haber tecla, la ventana de ajustes
**no es opcional en esta tanda**. Un interruptor que no se puede tocar es un
interruptor que no existe.

### Dónde se guardan

`pixpin-store::Ajustes` gana un campo `enganche: motor2d::enganche::Ajustes`.

Eso es **una arista nueva en el grafo de capas**: `pixpin-store` (capa 2) pasa
a depender de `pixpin-motor2d` (capa 1). La regla del proyecto lo permite —solo
prohíbe depender de una capa igual o superior— y el test
`apps/pixpin/tests/capas.rs` la acepta sin cambios.

*La alternativa considerada y descartada:* una copia del struct dentro de
`store`, con los mismos campos, y traducir en la aplicación. Evita el
acoplamiento a cambio de dos structs que hay que mantener sincronizados a mano
—y un campo nuevo que se olvide en la copia es un ajuste que se guarda y no se
lee—. Este proyecto ha ido eliminando justo esa clase de duplicado. *Coste si
me equivoco:* deshacer la arista es mover un struct y escribir la traducción,
sin tocar la lógica.

### En la ventana

Cuatro interruptores (`activo`, `esquinas`, `medios`, `centros`) y un número
(`radio_px`), con el patrón `Clave` / `filas_de` / `aplicar_interruptor` /
`aplicar_numero` que `apps/pixpin/src/ventana_ajustes.rs` ya tiene montado. Es
mecánico.

---

## La pista visual

Cuando el imán agarra, se ve una marca en el punto, **distinta según el tipo de
ancla**: así se distingue «me pegó al centro» de «me pegó a la esquina», que es
la queja típica cuando el imán hace algo que no esperabas.

**Esto no es un port, es una invención de la versión Windows, y conviene
decirlo.** El Android calcula `anclajeActivo` en `DrawController.kt:1533` y su
propia documentación dice que es «lo que la vista señala», pero **ningún sitio
de `DrawCanvas.kt` lo lee**: es una promesa sin cumplir. Lo que el Android sí
pinta es la lupa con una cruz —porque el dedo tapa el punto— y el recuadro de
la figura a la que se va a anclar una flecha.

Con ratón no hace falta lupa: el cursor no tapa nada. Lo que sí hace falta es
saber si agarró y a qué, porque sin eso el trazo se va dos píxeles y no puedes
distinguir un enganche de un fallo de puntería.

Las marcas, reutilizando el tipo `Orden` que ya usa el pintado de medir:

| Ancla | Marca |
|---|---|
| `Esquina`, `Extremo` | un cuadrado pequeño, sin relleno |
| `Medio` | un triángulo |
| `Centro` | un círculo |

Van **en píxeles de pantalla**, no de escena, para que no crezcan con el zoom —
igual que el radio, y por el mismo motivo—. Las formas son una elección de esta
versión, no un original que copiar: si al usarlas se confunden, se cambian sin
que nada más se entere, porque solo las mira el pintado.

---

## Pruebas

Todas puras y sin escritorio, que es la regla 1 del método de este proyecto.
Las que importan no son las de «engancha», son las de «**no** engancha»:

- Engancha a una esquina, a un extremo, a un medio y a un centro, cada uno por
  separado.
- **El elemento en curso no se engancha a sí mismo** (`excluir`). Sin esto una
  figura se pegaría a su propia esquina en cuanto naciera.
- Un elemento borrado no ofrece anclajes.
- Fuera del radio no engancha nada.
- **El radio es de pantalla:** a zoom 4 el radio de escena es la cuarta parte,
  así que un punto que engancha a zoom 1 deja de enganchar a zoom 4 a la misma
  distancia de escena.
- `Faena::AMano` no devuelve nada, nunca, con cualquier escena.
- Lo que el usuario apaga sigue apagado en todas las faenas.
- Empate de distancia: gana la esquina sobre el centro.
- `Faena::Moviendo` engancha a distancia 1,5 veces mayor que `Trazando`.
- **Un rectángulo girado cuya esquina cae dentro del radio pero fuera de su
  caja local sigue enganchando** (la trampa 1).
- Una línea no ofrece las esquinas de su caja: solo sus dos extremos y su medio
  (la trampa 2).
- **Cero asignaciones** en el camino caliente, en `tests/asignaciones.rs`.

---

## Lo que no se puede romper

- Un elemento que se está dibujando **nunca** se engancha a sí mismo.
- El lápiz **no** se engancha a vértices. Si alguien lo cablea a `Trazando`, el
  garabato pega tirones y la herramienta queda inservible.
- Apagar todo en los ajustes deja el dibujo **completamente libre**: un imán
  que sigue tirando con el interruptor a cero es peor que no tener interruptor.
- Cero asignaciones en el camino caliente.
