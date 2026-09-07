# Decisiones tomadas al ejecutar el plan del editor avanzado

Spec: docs/superpowers/specs/2026-09-06-editor-avanzado-design.md (leída)
Rama: editor-avanzado-armazon
Base de la rama: 8059de9

## Barrido previo de conflictos

### Pares de tareas que comparten fichero o interfaz

| A | B | Fichero / interfaz | Qué produce A | Qué consume B | Hallazgo |
|---|---|---|---|---|---|
| 1 | 9 | `Escena::deshacer -> bool` | bool (antes `Option<u64>`) | ignora el valor | limpio |
| 1 | 9 | `abrir_paso`/`cerrar_paso`/`apuntar_edicion` | los tres | los tres | limpio |
| 1 | 12 | mismos | los tres | los tres | limpio |
| 1 | 3 | `elemento.rs` `bytes()` | suma sin `grupos` | le suma `grupos` | limpio (T1 lo deja preparado) |
| 2 | 3 | `Elemento` literal en `mod pruebas` | — | T3 añade `grupos` a todos | **CONFLICTO A** |
| 3 | 12 | `Elemento::grupos` | `Vec<String>` | `push`/`pop`/`last` | limpio |
| 4 | 9 | `transformar::{escalar,girar,a_saltos,angulo_hacia}` | las cuatro | las cuatro | limpio |
| 4 | 5 | `pixpin_geom::Tirador` | consume | consume | limpio (una sola verdad) |
| 5 | 9 | `Tiradores::{de_caja,de_elemento,en}`, `Agarre` | los tres | los tres | limpio |
| 6 | 9 | `impacto::{dentro_de,elementos_en,toca,elemento_en}` | los cuatro | `dentro_de`, `toca`, `elemento_en` | limpio |
| 7 | 11 | `Rejilla::{nueva,sincronizar,candidatos}` | los tres | los tres | limpio |
| 8 | 11 | `Cache::ordenes(e, zoom)` | `&[Orden]` | itera | limpio |
| 8 | 15 | `Cache::{fallos,aciertos}` | contadores | los asevera | limpio |
| 9 | 11 | `EventoGesto`, `FormaCursor`, `Region`, `Gesto` | los cuatro | los cuatro | limpio |
| 9 | 13 | `Herramienta` (se muda de `pixpin-ui` al motor) | reexport en `anotador.rs` | `de_herramienta` | limpio |
| 9 | 15 | `Gesto::evento`, `PUNTOS_RESERVADOS` | ambos | ambos | limpio |
| 10 | 11 | `EventoOverlay::Tecla{alt}`, `FormaCursorWin::Giro` | ambos | ambos | limpio |
| 11 | 13 | `ventana_editor.rs` | lo crea | le añade el panel | limpio (secuencial) |
| 11 | 14 | `ventana_editor.rs` | lo crea | le añade la capa | limpio (secuencial) |
| 9 | 10 | `pixpin-ui/anotador.rs` | T9 saca `Herramienta` | T10 añade `alt` | limpio (no se solapan) |

### Coherencia interna de cada tarea

| Tarea | ¿Las pruebas cuadran con el código que especifica? | Hallazgo |
|---|---|---|
| 1 | sí | limpio |
| 2 | el `mod pruebas` construye `Elemento` con un campo que aún no existe | **CONFLICTO A** |
| 3 | sí | limpio |
| 4 | sí | limpio |
| 5 | sí (18 órdenes = 9 × 2) | limpio |
| 6 | sí | limpio |
| 7 | sí | limpio |
| 8 | sí (10 niveles, −5..4) | limpio |
| 9 | sí | limpio |
| 10 | sí | limpio |
| 11 | usa `camara.en_mundo(Punto2)`, `caja_visible()`, `matriz()` | **CONFLICTO B** |
| 12 | sí | limpio |
| 13 | los dos `match` son exhaustivos (9 figuras, 11 herramientas) | limpio |
| 14 | crea `capa_estatica.rs`, pero la tabla de ficheros decía `lienzo.rs` | **CONFLICTO C** |
| 15 | sí | limpio |

### Rulings del barrido

**Ruling A (tarea 2 / tarea 3):** el `fn rect()` de las pruebas de `seleccion.rs`
llevaba `grupos: Vec::new()`, campo que no existe hasta la tarea 3. Quitado del
plan, con un comentario que remite al paso 5 de la tarea 3, que es el que añade
ese campo a todas las construcciones literales de una vez. — *Por qué:* reordenar
las tareas costaría más que quitar una línea, y el paso 5 de la tarea 3 existe
justo para esto. — *Coste si me equivoco:* nulo; sería un error de compilación
inmediato y evidente.

**Ruling B (tarea 11):** el plan llamaba a `camara.en_mundo(Punto2)`,
`camara.caja_visible()` y `camara.matriz()`. La API real es `a_mundo(Punto2)`,
`ventana(ancho_px, alto_px)` y no existe `matriz()`. Corregido a los nombres
reales. — *Por qué:* `en_mundo` **existe** pero convierte una LONGITUD en
píxeles, no un punto: el código habría compilado y dado coordenadas
silenciosamente equivocadas. Es el peor género de error y el barrido está
justamente para eso. — *Coste si me equivoco:* ninguno; comprobado leyendo
`camara.rs` líneas 85-97 y 167.

**Ruling C (tarea 14):** la tabla de ficheros asignaba la capa estática a
`lienzo.rs` y la tarea crea `capa_estatica.rs`. Corregida la tabla. — *Por qué:*
un fichero propio es lo correcto; `lienzo.rs` ya tiene 916 líneas. — *Coste si me
equivoco:* nulo, es documentación.

**Ruling D (tarea 11, aceptado sin cambio):** los métodos de `Pintor`
(`empezar`, `orden`, `marco`, `marquesina`, `terminar`) no existen todavía. El
plan ya lo dice y manda añadirlos en `pixpin-render` siguiendo su estilo. Se
queda así. — *Por qué:* es la única API que hay que inventar, y el plan la acota
al crate correcto. — *Coste si me equivoco:* el implementador de la tarea 11
tarda más en esa parte.

---

## Progreso

**Ruling E (herramienta, no plan):** `scripts/task-brief` de la skill busca
encabezados «## Task N» y este plan los tiene en español («## Tarea N»), asi que
falla. Los briefs se extraen con `awk` sobre el plan, uno por tarea, al mismo
directorio y con los mismos nombres que habria usado el script
(`task-N-brief.md`). Comprobado: los 15 suman 5.863 lineas, ninguno vacio.
— *Por que:* traducir los encabezados del plan a ingles solo para contentar a un
script seria peor; el plan lo leen personas. — *Coste si me equivoco:* ninguno,
el formato de salida es identico.

BASE de la tarea 1: f88e15e

Tarea 1: implementada, commit 88ca449 (BASE f88e15e). Revision despachada.

### Tarea 1 — revision 1

Veredicto A (conformidad): conforme, sin sobras ni faltas. Las seis pruebas
existen y aseveran lo que el brief dice.
Veredicto B (calidad): 1 critico, 2 importantes, 2 menores.

**Ruling F (critico, plan-mandated):** el revisor encuentra que `podar()` solo
se llama desde `cerrar_paso()`, mientras que `empujar_cambio()` —el camino que
usan `anadir`, `apuntar_movimiento` y `borrar_apuntando` fuera de un paso— y
tambien `deshacer()`/`rehacer()` escriben al historial sin podar. Y ese es
justamente el camino que usa hoy la app real (capa.rs:332, pines.rs:1060). El
hallazgo viene heredado literal de mi brief. **Decido a favor del hallazgo: se
arregla.** — *Por que:* el diseno dice, en su §5, «le pongo techo al historial en
memoria: 8 MB, y al pasarse se tira el paso mas antiguo», sin excepciones por
camino. El diseno es la autoridad y el plan es su argumento; donde el plan se
queda corto, manda el diseno. Un techo que solo se aplica en uno de los cuatro
caminos no es un techo. — *Coste si me equivoco:* podar en cada escritura es un
`while` sobre un contador ya calculado; el coste es despreciable y el riesgo de
no hacerlo es que un equipo de 4 GB se quede sin memoria.

**Ruling G (importante, plan-mandated):** la prueba
`treinta_ciclos_de_deshacer_y_rehacer_no_deforman_el_dibujo` nunca llama a
`rehacer()`: modifica y deshace en cada vuelta partiendo siempre del mismo
estado, asi que es trivialmente verde. **Se arregla: tiene que alternar deshacer
y rehacer sin volver a la partida en cada vuelta.** — *Por que:* D24 es la
decision que justifica guardar el elemento entero en vez de la operacion
inversa, y su unica prueba no la ejercita. Una prueba que no puede fallar no es
una prueba, y esta ademas promete en su nombre algo que no hace. — *Coste si me
equivoco:* ninguno; una prueba mas fuerte no rompe nada.

**Ruling H (importante):** `deshacer()` y `rehacer()` son cuerpos gemelos. Se
factoriza a un privado comun. — *Por que:* el propio comentario de
`aplicar_inverso` usa ese argumento —que dos funciones separadas acaban
discrepando— para justificar su diseno, y aqui se incumple a dos metodos de
distancia. — *Coste si me equivoco:* minimo, es una refactorizacion cubierta por
las pruebas que ya hay.

**Menores diferidos** (no entran en el bucle, van al revisor final):
Tarea 1: minor (diferido): `Cambio::bytes()` cuenta `size_of::<Cambio>()` para
  `Anadido`/`Borrado`, que sobreestima por el tamano uniforme del enum. Solo
  sobra margen en el techo, nunca falta.
Tarea 1: minor (diferido): `apuntar_edicion` toma prestado `en_curso` dos veces
  en vez de capturar el `&mut Paso` una sola.

Tarea 1: fix round 1/5 (3 arreglos aplicados, commit 88ca449..3411709). Re-revision acotada despachada.
Tarea 1: fix round 1/5 (3 resueltos, 0 abiertos; commits 88ca449..3411709)
Tarea 1: complete (commits f88e15e..3411709, revision limpia)

Observacion diferida del re-revisor (no es hallazgo): en `viajar_por_historial`,
`podar()` corre tambien al rehacer. Justificado en el comentario del codigo e
inocuo (nunca poda el unico paso ni toca la pila de rehacer).

BASE de la tarea 2: 3411709

Tarea 2: implementada, commit c839c00 (BASE 3411709). Revision despachada.

**Ruling I (tarea 3, defecto del plan que mi barrido no cazo):** las pruebas de
la tarea 3 estaban escritas contra una API inventada —`cargar`/`guardar` y
`lienzo.escena.elementos[...]`— que no existe. La real es `leer`/`escribir`, y
`Lienzo` tiene `entradas: Vec<Entrada>`, no `escena`. Ademas `Lienzo::elementos()`
devuelve clones, asi que no sirve para modificar: para eso hay que ir por
`entradas` y casar `Entrada::Nuestro { elemento, .. }`. **He reescrito las cinco
pruebas contra la API real y anadido una sexta** que comprueba que un elemento
ajeno (`pixpin-measure`) sigue viajando intacto con sus grupos y sus campos.
— *Por que:* el plan decia «ajusta a los nombres reales», que es cargarle al
implementador una traduccion no trivial —sobre todo el detalle de los clones—
donde equivocarse produce una prueba que compila y no prueba lo que dice. Mi
barrido dio la tarea 3 por «limpia» sin cotejarla con `excalidraw.rs`: fallo
mio. — *Coste si me equivoco:* bajo; si algun nombre sigue sin cuadrar es error
de compilacion inmediato, no silencioso.

Brief de la tarea 3 regenerado tras la correccion.

### Tarea 2 — revision 1
Veredicto A: conforme, 6 pruebas exactas, nada de mas. El "7 pruebas" del
informe era el filtro por subcadena, que tambien corre una preexistente de
`pintado.rs`.
Veredicto B: 1 importante, 2 menores.

**⚠️ resuelto por mi (el revisor no podia verlo desde el diff):** si
`Escena::anadir` asigna ids deterministas y sin colision entre pruebas. Si:
`anadir` hace `let id = self.siguiente_id.max(1); self.siguiente_id = id + 1;`
sobre una `Escena::nueva()` recien creada en cada prueba. No es un hueco.

**Ruling J (importante):** la rama de deduplicacion de `poner_todos` —el
`contains` que la vuelve cuadratica— no la prueba nadie. Se anade la prueba.
— *Por que:* el revisor lo plantea como «o la pruebas o documentas por que
blindas un caso que ningun llamador produce», y de las dos la prueba es mejor:
cuesta cuatro lineas y deja el metodo defendible tal cual esta, mientras que
quitar el blindaje obligaria a que todos los llamadores futuros garanticen ids
unicos. — *Coste si me equivoco:* nulo; es una prueba de mas sobre codigo que ya
funciona.

**Menores diferidos:**
Tarea 2: minor (diferido): el informe dice «7 pruebas» sin aclarar que una es
  preexistente de `pintado.rs`, atrapada por el filtro por subcadena.
Tarea 2: minor (diferido): el comentario de `capacidad()` documenta quien lo usa
  en vez de que es.

Tarea 2: fix round 1/5 despachado (1 hallazgo).
Tarea 2: puerta completa verificada por el controlador (no solo por el
implementador, que solo corrio su crate): `cargo test --workspace --
--test-threads=1` = 620 pruebas, 0 fallos, en commit 8c7b250.
Tarea 2: fix round 1/5 aplicado (commit c839c00..8c7b250). Re-revision despachada.
Tarea 2: fix round 1/5 (1 resuelto, 0 abiertos; commits c839c00..8c7b250)
Tarea 2: complete (commits 3411709..8c7b250, revision limpia)

BASE de la tarea 3: 8c7b250

**Ruling K (defecto del plan, cazado por el implementador):** los literales
`r#"..."#` que escribi en las pruebas de la tarea 3 contienen `"#000000"`, y esa
secuencia `"#` cierra el raw string: no compilaban. El implementador los paso a
`r##"..."##`, que es ademas lo que ya usaba `lienzo_mixto()` en el mismo fichero.
**Aceptado.** — *Por que:* es el arreglo correcto y consistente con el fichero;
la alternativa (escapar el color o sacarlo a una constante) seria peor y se
apartaria del resto. — *Coste si me equivoco:* ninguno, es sintaxis.

Tarea 3: implementada, commit e774dde (BASE 8c7b250). 626 pruebas, 0 fallos
(linea base 620 + 6 nuevas). Revision despachada.

### Tarea 3 — revision 1
Veredicto A: conforme, 6 pruebas exactas, nada de mas. Las pruebas antiguas de
ida y vuelta del `.pixpin` NO se tocaron: `excalidraw.rs` sale con 123 lineas
anadidas y **cero deleciones**.
Veredicto B: 0 criticos, 0 importantes, 1 menor. **Revision limpia, sin bucle.**

**⚠️ los dos resueltos por mi (el revisor no los veia desde el diff):**
1. `lienzo_mixto()` usa `r##"` (linea 422): el cambio a `r##` es consistente con
   el fichero, no una invencion.
2. El patron `contains()` de la prueba nueva de `Ajeno` es exactamente el de las
   pruebas de ida y vuelta que ya habia (lineas 462-494). El «menor» del revisor
   es la convencion establecida del fichero, no algo que esta tarea introdujera.
   Ademas la linea 488 ya aseveraba `contains("groupIds")` desde antes.

Tarea 3: minor (diferido): las aserciones de
  `un_elemento_ajeno_sigue_viajando_intacto_con_sus_grupos` son por subcadena en
  vez de reparsear el JSON. Es la convencion del fichero; si se cambia, se
  cambian todas a la vez, no solo esta.

Tarea 3: complete (commits 8c7b250..e774dde, revision limpia)

BASE de la tarea 4: e774dde

### Tarea 4 — NEEDS_CONTEXT, y el implementador tenia razon en las dos

**Ruling L (defecto del plan):** mis pruebas
`con_el_elemento_girado_la_esquina_anclada_sigue_sin_moverse` y
`escalar_y_devolver_deja_el_elemento_donde_estaba` arrastraban al **mismo punto
del mundo** (180,90) y (300,300) para todos los angulos. Eso no es el mismo
gesto: a 90 grados ese punto cae al otro lado del ancla y lo que sale es un
volteo, no un escalado. Y tras un volteo la esquina que se queda quieta ya no es
la noroeste, asi que `esquina_no` deja de reconstruir el ancla. **La
implementacion del brief es correcta; las pruebas estaban mal planteadas.**
Corregidas con un ayudante `arrastre_equivalente(local, centro, angulo)` que gira
el destino con el elemento, de modo que el gesto sea el mismo en el marco propio
para cualquier angulo. — *Por que:* lo que la prueba quiere demostrar es que la
formula del ancla vale a cualquier angulo, y para eso el gesto tiene que ser el
mismo gesto. — *Coste si me equivoco:* bajo; la prueba nueva es mas estricta que
la vieja (ahora tambien asevera ancho y alto).

**Ruling M:** anadida una prueba tercera,
`cruzar_el_ancla_con_el_elemento_girado_voltea_sin_degenerar`, que ataca de
frente el caso que las otras dos evitan: arrastrar al otro lado del ancla en un
elemento girado. Exige que el resultado siga siendo valido (sin cero, sin NaN),
no que la noroeste siga quieta. — *Por que:* el volteo es comportamiento
deseado (D30) y sin esta prueba se quedaba sin cubrir a angulos distintos de
cero, que es justo donde el implementador se atasco. — *Coste si me equivoco:*
ninguno, es cobertura de mas.

**Ruling N (defecto del plan, aritmetica mia mal hecha):**
`los_saltos_de_giro_son_de_quince_grados` afirmaba que `a_saltos(0.20) == 0.0`.
Falso: 0,20 rad son 11,46 grados, la media division son 7,5, asi que sube a 15.
Comprobado: 0.20/0.261799 = 0.764, que redondea a 1. Prueba corregida a 0,10 rad
para el caso que baja a cero, y anadido el de 0,20 que sube. — *Coste si me
equivoco:* nulo, es aritmetica verificable.

Brief de la tarea 4 regenerado. Se reanuda al mismo implementador.

**Ruling O (defecto del plan, cazado por el implementador):** el literal `0.5236`
que puse en dos pruebas dispara `clippy::approx_constant` — es una aproximacion
de `FRAC_PI_6`. El implementador lo sustituyo por la constante sin consultarlo.
**Aceptado.** — *Por que:* es el mismo angulo, no cambia que comprueba la prueba,
y con `-D warnings` el literal habria roto la puerta. Ademas la constante dice
mejor lo que es. — *Coste si me equivoco:* ninguno.

Tarea 4: implementada, commit beff43b (BASE e774dde). 641 pruebas, 0 fallos
(626 + 15). Revision despachada con el modelo mas capaz: es la tarea que sostiene
el editor entero y sus pruebas las escribi yo, con dos fallos ya encontrados.

### Tarea 4 — revision 1: DOS CRITICOS que las 641 pruebas no podian ver

Veredicto A: conforme, 15/15 pruebas exactas.
Veredicto B: 2 criticos, 1 importante, 7 menores. El revisor derivo a mano la
formula del ancla, los ocho signos, `desde_centro`, `proporcional`, `tope`,
`angulo_hacia` y `a_saltos`: **todos correctos**. El fallo esta entero en como se
tratan los `puntos` de lapiz/resaltador/linea/flecha cuando `angulo != 0`.

Motivo de que ninguna prueba lo cazara: **no hay ni una prueba con figura de
puntos y angulo distinto de cero.**

**Hallazgo mio, al verificar la premisa del revisor (mas grave que los dos
criticos):** el revisor dedujo de `impacto.rs:26-45` que los `puntos` estan en
marco local y se dibujan girados por `e.angulo`. Fui a comprobarlo y encontre
que **nada en la ruta de dibujo aplica `e.angulo`**: ni `pintado.rs` (cero
menciones), ni `pixpin-render`, ni `capa.rs`, ni `pines.rs`, ni el anotador. El
unico consumidor de `angulo` es `impacto::toca`. El unico que lo pone distinto de
cero es `excalidraw.rs` al leer `angle` del movil.

O sea: hoy un elemento girado en el telefono se abre en Windows **dibujado sin
girar**, pero su picado se comporta como si estuviera girado. Es un fallo previo
a esta tarea y a este plan.

**Ruling P (convenio, decide el contrato de las tareas 5, 9 y 11):** los `puntos`
estan en **marco local** y `angulo` se aplica a todo al dibujar. — *Por que:* es
lo que hacen Excalidraw y el Android, y el puente con el movil es la prioridad
declarada del proyecto (Fase A). Con el convenio contrario —puntos en mundo, sin
giro— un trazo girado en el telefono se veria distinto en el escritorio, que es
justo lo que el plan maestro llama inaceptable. — *Coste si me equivoco:* alto si
me equivoco, porque lo consumen tres tareas; por eso lo decido con el criterio de
interoperabilidad, que es el unico que da una respuesta no arbitraria.

**Ruling Q:** se arreglan los dos criticos y el importante en `transformar.rs`,
bajo el convenio P. Derivada y comprobada la correccion del critico 2:
`s(u) = A(u) + k` con `k = (R − I)·(c_A − c)`, donde `c` es el centro viejo y
`c_A` el centro de la caja de `A(u)`. Sale de exigir `W'(s(u)) = W(A(u))` y es
independiente de `u`.

**Ruling R:** se anade una tarea nueva, **4b**, que hace que la ruta de dibujo
honre `angulo`. Va antes de la 11 (la ventana), porque sin ella `girar` no se ve
y un plano girado del movil se dibuja mal. — *Por que:* es un fallo previo, pero
lo destapa este plan y lo necesita este plan: la tarea 4 construye `girar` y sin
esto girar no hace nada visible. — *Coste si me equivoco:* si resulta que el
dibujo debia ignorar `angulo`, se tira una tarea pequena; si no la hago, el
editor entero gira sin que se note.
Tarea 4: fix round 1/5 (3 resueltos, 0 abiertos; commits beff43b..a25e6ad)
Tarea 4: complete (commits e774dde..a25e6ad, revision limpia)
  El re-revisor derivo `k = (R-I)(c_A-c)` por su cuenta y llego a la misma
  formula, y verifico los dos contraejemplos con numeros: el ancla queda quieta
  en el mundo y el extremo acaba bajo el cursor (206,155 de largo, no 141,4).

Tarea 4: minor (diferido): `escalar_un_trazo_mueve_sus_puntos` compara `e.ancho`
  contra `e.caja()` leida justo despues, y el codigo asigna una de la otra: solo
  puede fallar si se vuelve a la formula vieja. Perdio ademas las aserciones de
  valor sobre los puntos. Sugerencia del revisor: calcular el ancho esperado a
  mano (198,27 con grosor 4).
Tarea 4: minor (diferido): con `grosor != 0`, la esquina anclada del marco se
  desplaza `grosor/2*(sx-1)`, porque `ancla_local` sale de la caja CON margen
  mientras que el escalado solo toca los puntos. Es inherente al convenio de
  `caja()` y **no lo introduce este diff** (pasaba igual con angulo 0), pero
  ninguna prueba lo fija.

BASE de la tarea 4b: HEAD tras el commit del plan

Tarea 4b: implementada, commit 3def0eb (BASE ae6e374). 649 pruebas, 0 fallos
(643 + 6). El implementador descubrio que `ordenes_a_distancia` delegaba en
`ordenes` solo PARCIALMENTE, y anadio una sexta prueba para ese camino. Revision
despachada con eso como foco.
### Tarea 4b — revision 1
Veredicto A: conforme. La sexta prueba no es material de mas: el brief la
anticipa ("comprueba si esa funcion delega en ordenes") y el revisor confirmo en
el codigo el camino real sin girar — la rama
`Lapiz | Resaltador if en_pantalla < TINTA_MINIMA_PX` de `ordenes_a_distancia`
(lineas 391-408) construye su propia `Polilinea` sin pasar por `ordenes`.
Veredicto B: 0 criticos, 0 importantes, 0 menores de peso. **Limpia, sin bucle.**
Verificado: el angulo no entro en los generadores (D38 intacta), la guarda de
angulo cero esta en los dos caminos, las seis variantes de `Orden` cubiertas, el
centro de giro coincide con `impacto::toca`, y `marco_de_seleccion` sin tocar.
Ninguna prueba anterior modificada: el hunk sobre `mod pruebas` es de lineas `+`
puras.

Tarea 4b: complete (commits ae6e374..3def0eb, revision limpia)

BASE de la tarea 5: 3def0eb

Tarea 5: implementada, commit 2c7e77b (BASE 3def0eb). 659 pruebas, 0 fallos
(649 + 10). Revision despachada.
### Tarea 5 — revision 1
Veredicto A: conforme, 10/10 pruebas exactas, API identica, `pixpin_geom::Tirador`
reutilizado sin duplicar.
Veredicto B: 0 criticos, 0 importantes, 2 menores cosmeticos. **Limpia.**
Verificado con numeros: `escala = 5.0` da tiradores de 40 unidades de mundo (no
1,6), o sea que el convenio no esta invertido; las ocho posiciones derivadas a
mano coinciden; y el centro de giro es el mismo `((x0+x1)/2,(y0+y1)/2)` que usan
`impacto::toca` e `impacto`/`pintado`, asi que los tiradores no se despegan.

Tarea 5: complete (commits 3def0eb..2c7e77b, revision limpia)

BASE de la tarea 6: 2c7e77b
### Tarea 6 — revision 1
Veredicto A: conforme, 6 pruebas (el informe decia 7 por un desglose mal hecho,
pero el total 665 cuadra). API exacta y reexportada.
Veredicto B: 0 criticos, 1 importante, 1 menor.

**Ruling S (importante, hueco de mi brief):** el codigo normaliza la caja de la
marquesina con min/max antes de comparar, pero **ninguna prueba la arrastra de
derecha a izquierda** (`x1 < x0`). Todas las cajas de prueba vienen ya ordenadas.
El paso 3 de mi brief explicaba por que hay que normalizar y el paso 1 no incluia
la prueba. **Se anade.** — *Por que:* sin normalizar, arrastrar al reves no
selecciona nada, y es la mitad de los arrastres que hace un usuario. Que el
codigo lo haga bien hoy no impide que alguien lo simplifique manana; la prueba es
lo que lo fija. — *Coste si me equivoco:* nulo, es una prueba de mas.

Tarea 6: minor (diferido): el informe desglosa mal el conteo (dice 7 pruebas y
  base 658; son 6 y 659). El total, 665, es correcto.

Tarea 6: fix round 1/5 despachado (1 hallazgo).
Tarea 6: fix round 1/5 (1 resuelto, 0 abiertos; commits 4d32f45..1d0f556)
  El re-revisor simulo la prueba sin la normalizacion: daria `[]` contra `[1]` y
  fallaria. La prueba es efectiva, no decorativa.
Tarea 6: complete (commits 2c7e77b..1d0f556, revision limpia)

BASE de la tarea 7: 1d0f556
### Tarea 7 — revision 1
Veredicto A: conforme, 6/6 pruebas, API exacta. La desviacion declarada
(`#[cfg(test)] use crate::elemento::Elemento;`) es correcta y minima.
Veredicto B: 0 criticos, 2 importantes.

**Ruling T (importante, hueco de mi brief):** `sincronizar` detecta los ids
sobrantes con `self.versiones.keys().filter(|id| !vistos.contains(id))`, donde
`vistos` es un `Vec`. `Vec::contains` es lineal, asi que ese filtro es O(n^2):
con 8.000 elementos son ~64 millones de comparaciones **en cada fotograma, aunque
no haya cambiado nada**. **Se arregla pasando `vistos` a `HashSet`.** — *Por que:*
mi «aviso de rendimiento» del brief anticipaba dos cuellos (`quitar` y el
`contains` de `candidatos`) y se dejo justo el que corre por fotograma. Una
rejilla cuya sincronizacion cuesta mas que la fuerza bruta que viene a evitar no
sirve para nada. — *Coste si me equivoco:* nulo; `HashSet` para pertenencia es
estrictamente mejor aqui y el cambio es de una linea.

**Ruling U (importante, hueco de mi brief):** no hay prueba de borrar y luego
restaurar. El revisor razona por inspeccion que funcionaria —`Escena::restaurar`
llama a `tocar()`, que sube la version— pero no esta ejercitado. **Se anade la
prueba.** — *Por que:* deshacer es borrado logico y restaurar es la mitad de esa
operacion; que el elemento vuelva a la rejilla al deshacer un borrado es
exactamente lo que un usuario nota si falla. — *Coste si me equivoco:* nulo.

Tarea 7: fix round 1/5 despachado (2 hallazgos).
Tarea 7: fix round 1/5 (2 resueltos, 0 abiertos; commits 8b2cf48..c8c1a6e)
  `vistos` pasa a `HashSet`, el filtro de sobrantes queda constante. Verificado
  que NO toco los otros dos cuellos, que quedan para medir en la tarea 15.
Tarea 7: complete (commits 1d0f556..c8c1a6e, revision limpia)

BASE de la tarea 8: c8c1a6e
### Tarea 8 — revision 1
Veredicto A: conforme, 9/9 pruebas exactas, API identica. La desviacion
declarada (`#[cfg(test)] use crate::vector::Punto2;`) es correcta: `Elemento`
importa `Punto2` con `use` privado, asi que no llega transitivamente. Error real
de mi brief.
Veredicto B: 0 criticos, 0 importantes, 1 menor. **Limpia.**
Verificado el orden de las guardas de `nivel_de_detalle`: `NaN > 0.0` es falso en
IEEE-754, asi que NaN cae a NIVEL_MINIMO y no se cuela por la rama del infinito.
Y el `expect` tras insertar no tiene tercer camino: o valia (el `get` previo lo
demostro) o se acaba de insertar.

Tarea 8: minor (diferido): `Cache` deriva `Clone`; hoy nadie la clona, pero un
  `.clone()` accidental copiaria la geometria entera.

Tarea 8: complete (commits c8c1a6e..ddb161f, revision limpia)

**Patron anotado para el informe final:** tres briefs mios (7, 8 y antes el 3)
llevaban codigo que no compilaba por un `use` que falta. Escribir codigo completo
en un plan sin compilarlo produce justo esta deuda. Los implementadores lo han
resuelto bien y lo han declarado las tres veces.

BASE de la tarea 9: ddb161f
### Tarea 9 — revision 1
Veredicto A: conforme, 14/14 pruebas exactas. La mudanza de `Herramienta` esta
completa (enum + impl al motor, reexport en `anotador.rs`, sin perdida
colateral: 34 lineas borradas por 4 anadidas).
Veredicto B: 0 criticos, 0 importantes, 1 menor. **Limpia.**
Los siete puntos delicados, verificados: orden de decision al pulsar correcto;
`apuntar_edicion` en los tres sitios que transforman (Moviendo 517, Escalando
531, Girando 554); cero `to_vec`/`collect`/`clone`/`format!` en `mover()` — el
unico `collect()` esta en `SeleccionarTodo`, fuera del camino del raton; los dos
buffers con `with_capacity`; `Suprimir` usa `borrar_apuntando`; `Region::Caja`
del tramo y nunca `Todo`; y el texto excluido del camino de dibujo.

**Ruling V (defecto del plan, cazado por el implementador):** mi brief declaraba
`FormaCursor::Escalar` de dos formas — con campos en la seccion de interfaces y
como tupla en el bloque de codigo. El implementador uso la de campos.
**Aceptado.** — *Por que:* es la unica lectura consistente: las pruebas verbatim
del propio brief desestructuran `Escalar { tirador, angulo }` y con la tupla no
compilarian. — *Coste si me equivoco:* ninguno.

Tarea 9: complete (commits ddb161f..c067813, revision limpia)

**EL MOTOR ESTA COMPLETO.** Tareas 1-9 y 4b cerradas. De aqui en adelante es
ventana y acabado.

BASE de la tarea 10: c067813
### Tarea 10 — revision 1
Veredicto A: conforme, 2/2 pruebas.
Veredicto B: 0 criticos, 0 importantes, 1 menor (el `// SAFETY:` del bloque nuevo
dice «igual que arriba», siguiendo el patron encadenado que ya usaba `ctrl`).
Verificado lo que mas podia fallar en silencio: `alt` se lee de verdad de
`GetKeyState(VK_MENU)` y no es un literal `false`. Y el `match` de cursores no
tiene comodin, asi que una variante futura la para el compilador.

**⚠️ resuelto por mi:** el revisor no podia confirmar desde el diff que los otros
cuatro ficheros ya usaran `..`. Comprobado: `capa.rs:619`, `editor.rs:205`,
`gif.rs:589,597` y `ventana_ajustes.rs:251` los cuatro con `..`. El informe era
exacto y el alcance fue de 2 ficheros, no de 4 como decia mi brief.

Tarea 10: minor (diferido): el `// SAFETY:` del `GetKeyState(VK_MENU)` remite a
  «igual que arriba» en vez de decir la precondicion.

Tarea 10: complete (commits c067813..0a1c525, revision limpia)

BASE de la tarea 11: 0a1c525
### Tarea 11 — revision 1: EL HITO
Veredicto A: conforme, 8/8 pruebas. Las cuatro desviaciones declaradas, juzgadas
todas correctas por el revisor.
Veredicto B: 0 criticos, 0 importantes. **Limpia.**
Verificado lo que mas podia fallar en silencio: usa `a_mundo` y no `en_mundo`
(linea 209), y `shift`/`alt` se leen de verdad via
`pixpin_shell::entrada::modificadores()` (lineas 241-253, invocada en la 299), no
quedan fijos en `false`.

**Ruling W (desviacion 1, aceptada):** el brief pedia `pintor.orden(&Orden)`, que
obligaria a `pixpin-render` a depender de `pixpin-motor2d`. El implementador lo
resolvio traduciendo en la ventana. **Correcto, y mejor que mi brief.** — *Por
que:* el encabezado de `pintado.rs` dice que el motor no dibuja y que «quien las
pinta es el consumidor, que ya tiene su pintor». Es lo que ya hacen `capa.rs` y
`pin/ventana.rs`. Mi pseudocodigo contradecia la arquitectura documentada del
propio proyecto. — *Coste si me equivoco:* ninguno; la alternativa era peor.

**Ruling X (deuda anotada, no bloqueante):** hay ahora tres copias del `match` que
traduce `Orden` a dibujo (`capa.rs:498`, `pin/ventana.rs:1930`,
`ventana_editor.rs:287`). El revisor recomienda extraerlas a **`pixpin-pin`**,
que ya depende de `pixpin-render` y `pixpin-motor2d`, y del que ya depende
`apps/pixpin` — o sea, sin tocar ningun manifiesto. Las tres no son identicas
(offsets y escalas distintos), asi que la funcion comun necesitaria un cierre de
transformacion. **Se anota para el revisor final, no se hace ahora.** — *Por que:*
extraer con tres copias vivas y sin prueba de equivalencia es arriesgar una
regresion en el pin y en la capa por una limpieza que no urge. — *Coste si me
equivoco:* aparece una cuarta copia antes de que alguien lo extraiga.

Tarea 11: minor (diferido): `Region::Caja` acaba invalidando la ventana entera
  porque `VentanaOverlay` no tiene invalidacion parcial. Deuda de rendimiento en
  el trazo a mano, anotada honestamente por el implementador. Relevante para la
  medicion de la tarea 15.
Tarea 11: minor (diferido): la entrada «Editor» de la bandeja usa un id propio
  (900) sin catalogo ni traduccion. Es lo que el brief pedia («lo minimo para
  probarlo a mano»), queda como pulido.

**Comprobacion a mano del controlador (paso 6 del brief), parcial:**
`cargo build --release` en verde. El binario pesa **2,8 MB** y arranca: proceso
vivo, **22,3 MB de RAM en reposo**, cierre limpio. El presupuesto del proyecto
pide < 30 MB en `Ligero` y < 40 en `Completo`: **cumple**.
**Lo que NO he podido comprobar:** la lista interactiva (dibujar tres trazos,
marquesina, tirar de una esquina con dos elegidos, girar con Shift, Escape a
mitad). Necesita a alguien delante del raton. Queda pendiente del usuario.

Tarea 11: complete (commits 0a1c525..8ad2922, revision limpia)

BASE de la tarea 12: 8ad2922
### Tarea 12 — revision 1
Veredicto A: conforme, 13/13 pruebas, API exacta. Otro `use` faltante en mi
brief (`Elemento`), el cuarto de la serie.
Veredicto B: 0 criticos, 1 importante.

**Ruling Y (importante, hueco mio del plan):** `al_frente` y `al_fondo`
reordenan `escena.elementos` sin abrir paso, y el `enum Cambio` **no tiene
variante para reordenar**, asi que subir algo al frente **no se puede deshacer en
absoluto**. Ni mi brief ni el diseno lo declaran a proposito: es un olvido.
**Se arregla anadiendo `Cambio::Reordenado(Vec<u64>)`** con el orden anterior de
ids. — *Por que:* el principio del historial, escrito en el diseno, es que un
gesto es un paso; que exactamente una accion del editor quede fuera de `Ctrl+Z`
es el genero de defecto que el usuario encuentra el primer dia y que cuesta diez
veces mas arreglar despues de publicar. El coste en memoria es 8 bytes por
elemento —64 KB con ocho mil— muy por debajo del techo de 8 MB, y `bytes()` ya
esta preparado para contabilizarlo. — *Coste si me equivoco:* toco `escena.rs`,
que es codigo de la tarea 1 ya revisado; el riesgo es una regresion en el
historial, y por eso la ronda de arreglos exige que las 719 pruebas sigan verdes.

Tarea 12: fix round 1/5 despachado (1 hallazgo).

**Incidencia (no del plan):** el subagente de la ronda de arreglos de la tarea 12
murio por limite de cuota de sesion del modelo `sonnet` (reset 3:20 Lima).
Comprobado: el arbol quedo **limpio**, sin trabajo a medias, en el commit
6312fc5. Reintentado con `haiku`, que tiene cuota aparte. Si tambien se agota,
hay que esperar al reset.
Tarea 12: fix round 1/5 (1 resuelto, 0 abiertos; commits 6312fc5..fce5d9d)
  `Cambio::Reordenado(Vec<u64>)` con inversion simetrica; usa `.find()` y no
  indexacion, asi que un id compactado se ignora en vez de reventar. Las cuatro
  pruebas aseveran la lista completa de ids, no solo los extremos.
Tarea 12: minor (diferido): el comentario de `apuntar_reordenamiento` dice «igual
  que apuntar_edicion» y no lo es del todo — el de reordenamiento deduplica y el
  otro no. El comportamiento es correcto; el comentario enganha.
Tarea 12: complete (commits 8ad2922..fce5d9d, revision limpia)

BASE de la tarea 13: fce5d9d
### Tarea 13 — revision 1
Veredicto A: conforme, 10/10 pruebas, API exacta.
Veredicto B: 0 criticos, 0 importantes, 1 menor. **Limpia.**
Verificado: los dos `match` son exhaustivos y **sin comodin** (9 figuras, 11
herramientas), asi que anadir una figura manana lo para el compilador. `Propiedad`
deriva `Ord` y `comunes` ordena por el enum, no por la seleccion. Y el filtro usa
`all`, o sea que interseca de verdad.

Tarea 13: minor (diferido): falta la prueba de que `comunes(&[un_elemento])` da lo
  mismo que `de_figura` de ese elemento. Es el caso mas frecuente en uso real
  —seleccionar una sola cosa— y la logica es correcta, pero no esta cubierto.
  El revisor deja escrita la prueba de cuatro lineas. **Candidata clara a la
  oleada de arreglos del revisor final.**

Tarea 13: complete (commits fce5d9d..14f6738, revision limpia)
  Alcance recortado por mi: el paso 3 (pintar el panel en la ventana) se dejo
  fuera a proposito, para revisar la interfaz aparte de la logica.

BASE de la tarea 14: 14f6738
### Tarea 14 — revision 1
Veredicto A: conforme, 7/7 pruebas de politica. Los dos huecos que relleno el
implementador (`pintar: impl FnOnce(&Pintor)` en `preparar`, `destino` en
`volcar`) son **necesarios y minimos**: `pixpin-render` no conoce `Escena` ni
`Cache`, asi que no puede pintar la escena por su cuenta; y el backbuffer cambia
cada fotograma. Los dos calcan patrones que ya existen en el crate.
Veredicto B: 0 criticos, 0 importantes, 0 menores. **Limpia.**
Verificado: `soltar()` hace `bitmap = None` (libera de verdad); `volcar` devuelve
`bool` y la ventana lo respeta; `preparar` solo se llama en la transicion
reposo->activo, nunca dentro del bucle de pintado; y la capa se suelta tanto al
soltar el raton como al cancelar con Escape.

**⚠️ resuelto por mi:** el revisor no pudo confirmar que no exista un camino que
devuelva el gesto a reposo sin pasar por `gesto.evento`. Comprobado: `estado` es
privado (gesto.rs:139) y sus once mutaciones estan todas dentro de `gesto.rs`,
alcanzables solo desde `evento`. No hay tal camino.

Tarea 14: complete (commits 14f6738..f60585e, revision limpia)

BASE de la tarea 15: f60585e
### Tarea 15 — revision 1
Veredicto A: conforme. Asignador + 4 pruebas, en `tests/` y no en `src/`, contando
`realloc` ademas de `alloc`.
Veredicto B: 0 criticos, 1 importante.

**Lo que esta tarea encontro, y justifica el plan entero:** tres de las cuatro
pruebas pasaron a la primera; `arrastrar_una_seleccion_tampoco_asigna` fallo con
**24 asignaciones**. Causa: `pulsar` ponia el estado en `Moviendo` sin tomar la
instantanea, asi que se tomaba perezosamente en el primer `mover` — dentro del
camino caliente. Ninguna de las 740 pruebas anteriores podia verlo, porque todas
comprueban QUE pasa y no CUANTO cuesta. Arreglado en el motor, con el numero
esperado intacto en cero.

**Ruling Z (importante, regresion del propio arreglo — la encontre yo y el
revisor la confirmo con el rastro completo):** al mover `apuntar_edicion` a
`pulsar`, hacer clic sobre un elemento **ya seleccionado** y soltar **sin
arrastrar** deja un paso en el historial. `apuntar_edicion` no comprueba si el
elemento va a cambiar, y `cerrar_paso` solo descarta si `cambios.is_empty()`. El
usuario pulsa `Ctrl+Z` y no ve nada; repite y el historial se llena de pasos
fantasma. **Se arregla en `cerrar_paso`: que descarte tambien los cambios
`Editado` cuyo `antes` es identico al elemento actual.** — *Por que:* el arreglo
en `cerrar_paso` corre una vez al soltar, no en cada aviso del raton, asi que no
pone en riesgo las pruebas de cero asignaciones; y ademas arregla el mismo caso
en `alinear` cuando los elementos ya estaban alineados. La alternativa —volver a
tomar la instantanea perezosamente— reintroduciria las 24 asignaciones.
— *Coste si me equivoco:* toco `escena.rs` otra vez; el riesgo es el historial, y
por eso la ronda exige que las 744 pruebas sigan verdes.

Hueco de cobertura que el revisor senala y que existia desde la tarea 9: ninguna
prueba ejercita «clic sobre lo ya seleccionado + soltar sin arrastrar + comprobar
el historial». Las 14 de la tarea 9 nunca cerraron ese camino.

Tarea 15: fix round 1/5 despachado (1 hallazgo).
Tarea 15: fix round 1/5 (1 resuelto, 0 abiertos; commits 490823d..4e48caf)
  El filtro compara el `Elemento` entero (deriva `PartialEq`), no solo x/y; deja
  intactos `Anadido`/`Borrado`/`Reordenado`; cuenta bytes sobre el paso YA
  filtrado; y un id compactado se conserva en vez de reventar, que es lo
  conservador. Las cuatro pruebas de asignaciones siguen en cero.
Tarea 15: minor (diferido): la prueba `alinear_lo_ya_alineado_no_deja_paso`
  documenta un camino de skip que ya existia en `alinear()`, no ejercita el
  filtro nuevo de `cerrar_paso`. No es defecto, pero no es cobertura del arreglo.
Tarea 15: complete (commits f60585e..4e48caf, revision limpia)

**LAS DIECISEIS TAREAS ESTAN CERRADAS.** 747 pruebas, 0 fallos.

**Incidencia:** la revision final con `opus` murio por limite de cuota (reset
8:20 Lima) tras leer solo el mapa. Arbol limpio, sin efectos. Reintentada con
`sonnet`.

## REVISION FINAL DE LA RAMA

Triaje de los quince: 5 descartar, 5 arreglar despues, 5 arreglar antes de
fusionar. **Y dos fallos reales que ninguna revision por tarea podia ver.**

**FALLO A (real, de usuario):** los tiradores se **pintan sin girar** y se
**pican girados**. `gesto.rs:244` calcula el angulo real del elemento para decidir
que tirador agarra el clic; `ventana_editor.rs:329` pinta con el angulo fijo en
`0.0`. Un elemento girado ensena el marco y los ocho tiradores rectos, pero su
zona de picado esta girada: el usuario pincha donde ve el tirador y no agarra
nada. Es exactamente el «se ve en un sitio y se toca en otro» que esta rama vino
a arreglar, reaparecido en la ventana. Ninguna de las 747 pruebas podia verlo:
todas prueban `Gesto` sin pintar.

**FALLO B (real):** `indice.rs:93` indexa la rejilla con `e.caja()`, la caja
**sin girar**. El comentario de `candidatos()` promete «puede devolver de mas,
nunca de menos», y con un elemento girado —un cuadrado de 100 a 45 grados mide
141 en diagonal— la extension real supera la caja indexada. Cerca del borde de la
vista, ese elemento puede no pintarse aunque este en pantalla.

**Ruling AA (contradiccion entre revisores, resuelta por mi):** el revisor final
afirma que no hay prueba del camino «clic sobre lo ya seleccionado, soltar sin
arrastrar, comprobar el historial». **Es falso:** existe en `gesto.rs:952`
(`un_clic_sobre_lo_ya_seleccionado_sin_arrastrar_no_deja_paso_fantasma`),
anadida en la ronda de arreglos de la tarea 15 y verificada por el re-revisor. Lo
que si es cierto es el minor original: la prueba de `alinear` documenta un camino
de skip preexistente y no ejercita el filtro nuevo. **Lo bajo a «arreglar
despues».** — *Por que:* comprobado a mano. — *Coste si me equivoco:* ninguno, la
cobertura existe.

**Oleada unica de arreglos antes de fusionar (seis puntos):**
1. Fallo A — angulo de los tiradores al pintar, y `Pintor::marco` sin angulo.
2. Fallo B — indexar la rejilla con la caja girada.
3. Minor #6 (tarea 4) — la prueba `escalar_un_trazo_mueve_sus_puntos` es
   tautologica: compara `e.ancho` contra `e.caja()` leida justo despues, y el
   codigo asigna una de la otra.
4. Minor #14 (tarea 13) — falta la prueba de `comunes` con un solo elemento.
5. Minor #9 (tarea 8) — quitar el `Clone` de `Cache`.
6. Minor #10 (tarea 10) — el `// SAFETY:` vago incumple la regla del crate.

**Ruling AB (aritmetica mia mal copiada, cazada por el implementador):** el
ancho esperado de `escalar_un_trazo_mueve_sus_puntos` es **198,23**, no 198,27
como decia yo (numero que venia del revisor de la tarea 4). Verificado a mano:
caja (-2,-2,102,52), sx = 202/104 = 1,9423077; el punto (100,50) va a x =
196,1154 y el (0,0) a x = 1,8846; con el margen de grosor, la caja va de -0,1154
a 198,1154, o sea **198,2308**. El implementador uso 198,23 y lo declaro.
**Aceptado.** — *Coste si me equivoco:* ninguno, es aritmetica cerrada.

Oleada final aplicada: seis commits, 4e48caf..459cb2c. 751 pruebas, 0 fallos.
Las cuatro de asignaciones siguen en cero.
