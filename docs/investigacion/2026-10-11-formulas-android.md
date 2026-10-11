# Formulas de la hoja: portar el motor del PC a Android (11-oct-2026)

Para otro agente que trabaje en Android. **No se ha tocado Android.** El PC ya
tiene el motor ampliado; el JSON de la hoja es el mismo (`celdas`: de `B7` a
texto, la formula empieza por `=`), asi que una hoja hecha en un lado tiene
que dar el mismo resultado en el otro. Esta guia es el contrato.

- Codigo del PC: `crates/pixpin-proyecto/src/formula.rs` y `formula/`
  (`analisis.rs` trocea y arma el arbol, `motor.rs` calcula y tiene el catalogo
  de nombres, `mates.rs`, `criterio.rs`, `busqueda.rs`, `texto.rs`, `fecha.rs`,
  `formato.rs`, `info.rs`). Las pruebas estan en `formula/pruebas_funciones.rs`.
  Cada fila de la tabla del final sale de ahi y pasa en el PC.
- Antes de empezar, mira que hay hoy en Android (`motor/TablaDeCalculo.kt`,
  `Calculadora`, `CalculoFormato`): el clon local viejo miente, comprueba con
  `gh api`. Lo que Android ya tenga igual (lectura de numeros, formato
  «General», las cinco funciones de siempre) no se toca: el PC se escribio
  copiando `CalculoFormato.numero` y `.general`.

## 1. Modelo de valores

Un valor es **numero** (Double), **texto**, **logico** o **error**.

- Una celda sin nada es el **texto vacio**. No hay un cuarto tipo «vacio»: lo
  que hace falta saber de «vacio» se deduce de `texto == ""`. Consecuencia:
  una formula que devuelve `""` cuenta como celda en blanco para `ESBLANCO`,
  `CONTAR.BLANCO` y `CONTARA`. (Excel distingue; aqui no, a proposito.)
- Lo escrito a mano (no formula) se lee asi, sin cambios respecto a antes:
  empieza por `'` -> texto tal cual sin el apostrofo; vacio o solo espacios ->
  texto vacio; `numero_escrito(...)` da numero (`1.234,50`, `15%`, `(300)`,
  `S/ 120`...) -> numero; si no, texto (recortado).
- Errores, con el texto exacto que se muestra:

| Error | Texto | Cuando |
|---|---|---|
| Ciclo | `#¡CICLO!` | la formula depende de si misma, o se pasa de 128 celdas encadenadas |
| Referencia | `#¡REF!` | `A0`, `A1:A0`, `INDICE` fuera de la matriz, `BUSCARV` con columna mayor que la tabla |
| Sintaxis | `#¡VALOR!` | formula mal escrita, tipo que no vale, argumentos de menos o de mas |
| DivisionPorCero | `#¡DIV/0!` | `/0`, `RESIDUO(x;0)`, `PROMEDIO` sin numeros, `0^-1` |
| Nombre | `#¿NOMBRE?` | funcion que no existe (solo con parentesis: `NOEXISTE(1)`) |
| NoDisponible | `#N/A` | lo buscado no esta (`BUSCARV`, `COINCIDIR`, `BUSCARX`, `SI.CONJUNTO` sin ninguno cierto) |
| Numero | `#¡NUM!` | raiz de negativo, `0^0`, resultado infinito, fecha fuera de rango |

  Nota: `#¡VALOR!` es el nombre en pantalla del error «Sintaxis» (asi se
  llamaba ya): todo «no se entiende / no es de ese tipo» es ese.
- Como se muestra un valor: numero -> vista «General» (`CalculoFormato.general`,
  diez cifras, `3` y no `3.0`); texto tal cual; logico -> `VERDADERO` /
  `FALSO`; error -> la tabla de arriba. **Un numero no finito (NaN, infinito)
  nunca sale de una cuenta: es `#¡NUM!`.**

## 2. Sintaxis y prioridad

Se arma un **arbol** antes de calcular nada (hace falta para que `SI` no
calcule la rama que no toca). Prioridad, de menos a mas:

1. comparaciones `= <> < > <= >=` (de izquierda a derecha)
2. `&` (concatenar)
3. `+ -`
4. `* /`
5. `^` (**de izquierda a derecha**: `2^3^2` es 64)
6. signo de delante `- +` (**mas fuerte que `^`**: `-2^2` es 4, `2^-1` es 0.5)
7. `%` de detras (`50%` = 0.5; se pueden encadenar)
8. parentesis, numeros, texto, celdas, rangos, funciones

Detalles del troceo:

- Separador de argumentos: `;` **o** `,` (se pueden mezclar). La coma NUNCA es
  decimal en una formula; el decimal es el punto. Dentro de un texto entre
  comillas no separan.
- Texto: `"..."`, y una comilla dentro se escribe doble: `"a""b"` es `a"b`.
  Comilla sin cerrar -> `#¡VALOR!`.
- Numeros: cifras y puntos (`1.2.3` -> `#¡VALOR!`), con exponente solo si lleva
  cifras detras: `1E3` y `2.5e-4` si; `2E` o `2EA` no.
- `VERDADERO`/`TRUE` y `FALSO`/`FALSE` (sin parentesis, o con: `VERDADERO()`).
  Cualquier otro nombre sin parentesis (`=hola`, `=SUMA`) -> `#¡VALOR!`.
- Referencias: `$` se ignora (`$A$1` = `A1`). Mayusculas o minusculas da igual
  (`a1`, `max`). Algo con forma de celda pero que no lo es (`A0`) ->
  `#¡REF!`. Rango `B6:B1` = `B1:B6`. Un extremo malo de un rango (`A1:A0`,
  `A1:` , `A1:5`) -> `#¡REF!`.
- Un nombre con letras y puntos (`SI.ERROR`, `REDONDEAR.MAS`) es funcion si va
  seguido de `(` (se admiten espacios antes del parentesis). Los nombres
  admiten `Ñ` (`AÑO`) y tambien `ANO`.
- Espacios (incluidos el duro y el fino, que trae un texto pegado) se ignoran.
- Lo que sobra tras la expresion (`=1 2`), un parentesis sin cerrar, `=1+`,
  `=*3`, `=1,,2`, `=SUMA(1;)`, `=SUMA(;1)`, `={1,2}` -> `#¡VALOR!`.
  **No hay argumentos vacios**: `SI(a;;b)` es error.
- Anidado maximo 64 (parentesis, signos y funciones dentro de funciones); mas
  -> `#¡VALOR!` en vez de reventar la pila. Cadena de celdas: maximo 128
  encadenadas (`A2=A1+1`, `A3=A2+1`...) -> la que se pasa es `#¡CICLO!`.

## 3. Evaluacion

- **Memoria por pasada.** Al calcular toda la hoja, cada celda se calcula una
  vez y se guarda; se recorren por columnas, de arriba abajo. Un `#¡CICLO!`
  **no se guarda** (depende de por donde se entro).
- **Ciclo**: se lleva la lista de celdas por las que se pasa; volver a una
  que ya esta es `#¡CICLO!`. Todas las funciones que leen rangos propagan el
  ciclo si cualquier celda del rango lo da (aunque la funcion normalmente
  ignore errores: `CONTARA`, `CONTAR.SI`, `ESERROR`, `SI.ERROR`...). Un ciclo
  **nunca** se tapa.
- **Solo se miran las celdas escritas** de un rango, no el rectangulo: `A1:ZZ9999`
  es instantaneo. Lo que importa de los huecos se deduce (ver `CONTAR.BLANCO`,
  `CONTAR.SI(...;"")`).
- `INDICE`, `BUSCARV` y compania **solo leen las celdas que devuelven o
  comparan**. `=INDICE(A1:A3;2)` puesta en `A1` da el valor de `A2` sin
  ciclo (Excel diria referencia circular). `=INDICE(A1:A3;1)` si da ciclo.
- `HOY()`, `AHORA()`, `ALEATORIO()`, `ALEATORIO.ENTRE()` son volatiles: se
  calculan una vez por pasada; `HOY`/`AHORA` usan **un solo instante por
  pasada** (hora local del aparato).

### Argumentos y conversiones

- **Un rango de varias celdas donde cabe un valor** (`=A1:A3*2`,
  `SI(A1:A3;..)`, `LARGO(A1:A3)`) -> `#¡VALOR!`. Un rango de una celda (`A1:A1`)
  vale lo que esa celda. Algunas funciones devuelven un rango (`SI` puede
  devolver `A1:A3`) y otra lo consume: `SUMA(SI(VERDADERO;A1:A3;B1:B3))`.
- **A numero** (cuentas, `REDONDEAR`...): numero tal cual; logico -> 1/0; texto
  vacio -> 0; texto que `numero_escrito` lee -> ese numero (`"3"+4` = 7);
  cualquier otro texto -> `#¡VALOR!` (`=B1*2` con «Importe» en B1; `1+"abc"`).
  Error -> ese error (el de la izquierda gana).
- **A texto** (`&`, `CONCATENAR`, `LARGO`...): numero -> «General»; logico ->
  `VERDADERO`/`FALSO`; error -> ese error.
- **A logico** (`SI`, `Y`...): numero != 0; texto vacio -> falso; texto
  `VERDADERO`/`TRUE`/`FALSO`/`FALSE` (sin mayusculas ni espacios de los lados);
  otro texto -> `#¡VALOR!`.
- **A entero** (posiciones, decimales de `REDONDEAR`): se **trunca hacia
  cero** (`IZQUIERDA("hola";2.9)` = `ho`). Mayor que 9e15 -> `#¡NUM!`.
- **Un error en un argumento sube** (el primero, de izquierda a derecha),
  salvo en `ES*`, `SI.ERROR`, `SI.ND`, `CONTARA`, `CONTAR.BLANCO` y la rama
  de `SI`/`SI.CONJUNTO` que no se elige.
- **Argumentos**: de menos o de mas de los que admite la funcion (tabla de
  la seccion 4) -> `#¡VALOR!`, antes de calcular nada.

### Comparaciones

`=` `<>` `<` `>` `<=` `>=` dan logico. Orden de Excel: **numeros < textos <
logicos**. El texto no distingue mayusculas (se compara en minusculas). Dos
numeros son iguales si difieren menos de `1e-14` relativo (`0.1+0.2=0.3` es
`VERDADERO`). Una **celda vacia** (texto vacio) se parece a lo que tenga
enfrente: 0 frente a un numero, `""` frente a un texto, `FALSO` frente a un
logico; dos vacias son iguales. Un error en cualquier lado sube.

## 4. Funciones

Nombre en espanol / ingles, argumentos (`[...]` opcional) y reglas. «Numeros»
= los argumentos sueltos y las celdas de los rangos, ver abajo.

**Recoger numeros** (`SUMA`, `CONTAR`, `PROMEDIO`, `MIN`, `MAX`, `PRODUCTO`,
`MEDIANA`): de un rango o de una celda citada solo cuentan los **numeros**
(texto, logicos y vacias no aportan nada; un error del rango **sube**). De un
argumento escrito en la formula: numero tal cual, logico -> 1/0, texto que se
lee como numero -> ese numero (`SUMA("5";1)` = 6), otro texto -> se ignora.

### Las de siempre (sin cambios)
| Funcion | Args | Regla |
|---|---|---|
| `SUMA`/`SUM` | 0..255 | suma; sin numeros = 0. `SUMA()` = 0 |
| `CONTAR`/`COUNT` | 0..255 | cuantos numeros. Un error del rango sube |
| `PROMEDIO`/`AVERAGE` | 0..255 | media de los numeros; ninguno -> `#¡DIV/0!` |
| `MIN`, `MINIMO` / `MAX`, `MAXIMO` | 0..255 | parte del primero (no de 0); sin numeros = 0 |

### Logicas
| Funcion | Args | Regla |
|---|---|---|
| `SI`/`IF` | 2..3 | condicion a logico (error -> sube); solo se calcula la rama elegida. Sin tercer argumento y condicion falsa -> `FALSO`. Devuelve tal cual lo que devuelva la rama (tambien un rango) |
| `Y`/`AND`, `O`/`OR` | 1..255 | se leen **todos** (un error en cualquiera sube). Numeros != 0 son verdad. En rangos o celdas citadas el texto se ignora; escrito suelto, texto que no es VERDADERO/FALSO -> `#¡VALOR!`. Si ningun argumento aporto un logico -> `#¡VALOR!` |
| `NO`/`NOT` | 1 | niega el logico |
| `SI.ERROR`/`IFERROR` | 2 | si el primero es error devuelve el segundo (que solo se calcula entonces). **No tapa `#¡CICLO!`** |
| `SI.ND`/`IFNA` | 2 | igual pero solo tapa `#N/A` |
| `SI.CONJUNTO`/`IFS` | 2..254, par | pares condicion/valor; el primero cierto. Ninguno -> `#N/A`. Se para en el cierto (lo que va detras no se calcula). Impar -> `#¡VALOR!` |
| `VERDADERO()`/`TRUE()`, `FALSO()`/`FALSE()` | 0 | |

### Matematicas
| Funcion | Args | Regla |
|---|---|---|
| `REDONDEAR`/`ROUND` | 2 | mitad **alejandose del cero**; decimales negativos redondean a decenas, centenas... Ver «Redondeo» |
| `REDONDEAR.MAS`/`ROUNDUP` | 2 | siempre alejandose del cero |
| `REDONDEAR.MENOS`/`ROUNDDOWN` | 2 | siempre hacia el cero |
| `ENTERO`/`INT` | 1 | **suelo** (hacia menos infinito): `ENTERO(-1.5)` = -2 |
| `ABS` | 1 | |
| `RESIDUO`/`MOD` | 2 | `n - d*suelo(n/d)`: lleva el signo del divisor (`RESIDUO(-3;2)`=1, `RESIDUO(3;-2)`=-1). `d=0` -> `#¡DIV/0!` |
| `POTENCIA`/`POWER` y el operador `^` | 2 | `0^0` -> `#¡NUM!`; `0^negativo` -> `#¡DIV/0!`; negativo con exponente fraccionario -> `#¡NUM!`; resultado infinito -> `#¡NUM!` |
| `RAIZ`/`SQRT` | 1 | negativo -> `#¡NUM!` |
| `PRODUCTO`/`PRODUCT` | 1..255 | producto de los numeros; **ninguno = 0** |
| `MEDIANA`/`MEDIAN` | 1..255 | media de los dos centrales si son pares; **ninguno -> `#¡NUM!`** |
| `PI` | 0 | 3.141592653589793 |
| `ALEATORIO`/`RAND` | 0 | [0, 1) |
| `ALEATORIO.ENTRE`/`RANDBETWEEN` | 2 | enteros entre `ceil(a)` y `floor(b)`, ambos incluidos; `a > b` -> `#¡NUM!` |

**Redondeo (importante).** `2.675` es `2.67499999...` en binario; Excel da
`REDONDEAR(2.675;2)` = 2.68 y `REDONDEAR(1.005;2)` = 1.01. Algoritmo del PC:
`x = n * 10^d` (o `n / 10^-d` si `d<0`); sobre `|x|`: si esta a menos de
`1e-9` (relativo, con minimo 1) de un entero, es ese entero; para `REDONDEAR`
ademas, si `2|x|` esta a menos de `1e-9` de un entero, `|x|` es esa mitad; luego
`round` (mitad lejos del cero) / `ceil` / `floor`; se deshace la escala y se
vuelve el signo de `n`. `d` se acota a ±400. `n = 0` o no finito se devuelve tal cual.

### Recuentos
| Funcion | Args | Regla |
|---|---|---|
| `CONTARA`/`COUNTA` | 1..255 | cuantos **no vacios**: suelto cuenta siempre; celda citada, si no es `""`; rango, las celdas escritas que no son `""`. **Cuenta los errores** (no suben), salvo el ciclo |
| `CONTAR.BLANCO`/`COUNTBLANK` | 1 | celdas del rango (todas las del rectangulo) menos las no vacias. Una celda sola: 1 si esta vacia |

### Con criterio
`CONTAR.SI(rango; criterio)`, `SUMAR.SI(rango; criterio; [rango_suma])`,
`PROMEDIO.SI(rango; criterio; [rango_prom])`,
`CONTAR.SI.CONJUNTO(rango1; crit1; rango2; crit2...)`,
`SUMAR.SI.CONJUNTO(rango_suma; rango1; crit1; ...)` (ingles: `COUNTIF`,
`SUMIF`, `AVERAGEIF`, `COUNTIFS`, `SUMIFS`). Argumentos: 2, 2..3, 2..3, 2..254
(pares), 3..255 (suma + pares).

- El **primer argumento tiene que ser un rango** (una celda suelta o un valor
  -> `#¡VALOR!`). Todos los rangos de los pares miden lo mismo (si no,
  `#¡VALOR!`); en `SUMAR.SI.CONJUNTO` el de suma tambien. En `SUMAR.SI` y
  `PROMEDIO.SI` el rango de suma puede ser de otro tamano: **manda su esquina
  de arriba a la izquierda y toma el tamano del rango de criterio**.
- Un error en el argumento `criterio` sube. Una celda de error en el rango de
  criterios **no cumple nada** (no sube). Un error en una celda de suma/promedio
  que si cumple **sube**.
- Criterio: si es numero -> igual a ese numero; logico -> igual; error -> como
  vacio (no se usa). Si es **texto**: se le quita un operador de delante (en
  este orden: `<>`, `<=`, `>=`, `=`, `<`, `>`; sin operador es `=`). El resto:
  vacio -> «vacio»; si `numero_escrito` lo lee -> numero; `VERDADERO`/`TRUE`/
  `FALSO`/`FALSE` (sin mayusculas) -> logico; si no, texto (en minusculas).
- Cumplir:
  - **vacio**: con `=` o sin operador, la celda es `""`; con `<>`, no lo es.
    (`CONTAR.SI(r;"")` y `CONTAR.SI(r;"=")` cuentan lo mismo.)
  - **numero**: la celda es numero y la comparacion (con la misma tolerancia
    de la seccion 3) se cumple. Una celda que no es numero solo cumple `<>`.
    (No se compara texto que parece numero.)
  - **logico**: igual; otra cosa solo cumple `<>`.
  - **texto**: `=`/sin operador/`<>` comparan sin mayusculas **con comodines**
    `*` (cualquier cosa), `?` (una letra) y `~` delante de `*`, `?` o `~` para
    escribirlos tal cual. Solo se miran celdas de texto **no vacio**: una
    celda vacia o un numero no casa con `"*"`, y solo cumple `<>`. Con `<`,
    `>`, `<=`, `>=` se compara el texto en minusculas letra a letra (sin
    comodines) y solo con celdas de texto.
- `PROMEDIO.SI` promedia **solo los numeros** de las celdas que cumplen;
  ninguno -> `#¡DIV/0!`.
- **Huecos fuera de lo escrito**: `CONTAR.SI(A1:A1000000;"")` es 999995 si hay 5
  celdas escritas: lo no escrito es vacio y cumple si todos los criterios
  aceptan lo vacio. Implementacion del PC: se recorre solo el rectangulo hasta
  la ultima fila/columna escrita **de toda la tabla**; el resto, todo vacio,
  se suma de golpe a la cuenta si todos los criterios aceptan `""`
  (`SUMAR.SI` y `PROMEDIO.SI` no cambian: sumar vacios no suma).

### Busqueda
Todas leen solo la **primera columna (o fila)** de la tabla y las celdas que
devuelven; un ciclo en ellas es `#¡CICLO!`.

| Funcion | Args | Regla |
|---|---|---|
| `BUSCARV`/`VLOOKUP` `(buscado; tabla; columna; [aprox=VERDADERO])` | 3..4 | `columna` entero >= 1 (< 1 o no numero -> `#¡VALOR!`) y <= columnas de la tabla (si no `#¡REF!`). Exacta (`aprox` falso): primera coincidencia; **con comodines** si lo buscado es texto con `* ? ~`. Aproximada (por defecto): ver abajo. No hay -> `#N/A`. `aprox` debe ser logico (texto raro -> `#¡VALOR!`) |
| `BUSCARH`/`HLOOKUP` | 3..4 | igual por filas: busca en la primera fila, `fila` (3er argumento) |
| `INDICE`/`INDEX` `(matriz; fila; [col])` | 2..3 | 1 columna y sin `col`: `fila` es la fila. 1 fila y sin `col`: `fila` es la columna. 2D sin `col` -> `#¡VALOR!`. `0` o negativo -> `#¡VALOR!` (no hay «toda la fila»). Fuera -> `#¡REF!`. Una celda vacia da vacio (no error). Una celda suelta cuenta como matriz de 1x1 |
| `COINCIDIR`/`MATCH` `(buscado; rango; [tipo=1])` | 2..3 | rango de **una fila o una columna** (si no, `#N/A`). Devuelve la posicion **contando los huecos** (el rango `Q1:Q5` con la `z` en `Q3` da 3). `tipo` 0 exacto con comodines; > 0 aproximada ascendente; < 0 aproximada descendente (ver abajo) |
| `BUSCARX`/`XLOOKUP` `(buscado; donde; devolver; [si_no]; [modo=0]; [sentido=1])` | 3..6 | `donde` una linea (si no `#N/A`); `devolver` una linea **del mismo largo** (si no `#¡VALOR!`; una tabla -> `#¡VALOR!`), horizontal o vertical, da igual la orientacion del otro. `modo`: 0 exacto, -1 exacto o el siguiente **menor**, 1 exacto o el siguiente **mayor**, 2 exacto con comodines; otro -> `#¡VALOR!`. `sentido`: 1 de primero a ultimo, -1 de ultimo a primero (2 y -2 se leen como 1 y -1); otro -> `#¡VALOR!`. No esta: `si_no` si se dio (se calcula antes, es un argumento mas), si no `#N/A` |

**Igualdad al buscar.** Numeros: tolerancia de la seccion 3. Texto: sin
mayusculas. Logicos: iguales. **Un numero nunca es igual a un texto**
(`BUSCARV("20";...)` no encuentra el 20). Celdas de otro tipo que lo buscado se
saltan.

**Aproximada (BUSCARV, BUSCARH, COINCIDIR con tipo > 0).** Se recorre en orden
y se queda con la **ultima** celda no mayor que lo buscado, **parando en la
primera mayor** (comparando solo celdas del mismo tipo que lo buscado; las de
otro tipo se saltan sin parar). Con la columna ordenada da lo que Excel; con
una desordenada es mas previsible que la busqueda binaria de Excel (ejemplo:
columna 5, 50, 7 buscando 8 -> el 5). Los comodines **no** valen aqui (`"c*"`
es el texto `c*`, que es mayor que `c`). `COINCIDIR` con tipo < 0: lo mismo
pero la ultima celda **no menor** que lo buscado, parando en la primera menor.
Ninguna -> `#N/A`.

**BUSCARX modo ±1**: primero se busca igual (en el sentido pedido, la primera);
si no hay, entre las celdas del mismo tipo que cumplen el lado pedido se elige
la **mas cercana**, y a igualdad la primera en el sentido de la busqueda.

### Texto
Posiciones y largos **en letras** (no bytes: la `ñ` es una).

| Funcion | Args | Regla |
|---|---|---|
| `CONCATENAR`/`CONCATENATE` | 1..255 | une valores sueltos (un rango de varias celdas -> `#¡VALOR!`) |
| `CONCAT` | 1..255 | igual pero admite rangos: los recorre **por filas** (solo celdas escritas) |
| `UNIRCADENAS`/`TEXTJOIN` `(sep; ignorar_vacios; textos...)` | 3..255 | `ignorar_vacios` logico (si no `#¡VALOR!`). Un rango se recorre por filas y por columnas **hasta la ultima fila/columna escrita de toda la tabla** (mas alla seria solo separadores sobrantes); con `ignorar_vacios` falso cada hueco del rango pone su separador (`C1:C4` con 3 datos da `Ana-Anibal-Beto-`). Los argumentos sueltos vacios tambien se omiten con `ignorar_vacios` |
| `IZQUIERDA`/`LEFT`, `DERECHA`/`RIGHT` `(t; [n=1])` | 1..2 | n < 0 -> `#¡VALOR!`; n mayor que el largo = todo; n = 0 = vacio. `t` numero se usa su texto «General» |
| `EXTRAE`/`MID` `(t; inicio; n)` | 3 | inicio < 1 o n < 0 -> `#¡VALOR!`; inicio pasado el final = vacio; n mayor = hasta el final |
| `LARGO`/`LEN` | 1 | letras. Una celda vacia = 0 |
| `MAYUSC`/`UPPER`, `MINUSC`/`LOWER` | 1 | Unicode (`ñ` <-> `Ñ`) |
| `NOMPROPIO`/`PROPER` | 1 | primera letra de cada racha de letras en mayuscula, el resto en minuscula; **todo lo que no es letra corta la palabra** (`o'neil` -> `O'Neil`, `2da` -> `2Da`) |
| `ESPACIOS`/`TRIM` | 1 | quita los de los lados y deja uno entre palabras; **solo el espacio ASCII 32** (saltos de linea y duros se quedan) |
| `SUSTITUIR`/`SUBSTITUTE` `(t; viejo; nuevo; [cual])` | 3..4 | **distingue mayusculas**; `viejo` vacio -> `t` sin cambios; sin `cual` cambia todas (sin solaparse: `aaa`,`aa`->`b` da `ba`); `cual` >= 1 cambia solo esa (si no existe, nada); `cual` < 1 -> `#¡VALOR!` |
| `ENCONTRAR`/`FIND` `(buscar; en; [desde=1])` | 2..3 | distingue mayusculas, sin comodines; posicion desde 1; no esta -> `#¡VALOR!`; `buscar` vacio -> `desde`; `desde` < 1 o mayor que `max(largo,1)` -> `#¡VALOR!` |
| `HALLAR`/`SEARCH` | 2..3 | no distingue y **admite** `* ? ~`; el patron casa desde esa posicion hasta donde sea; resto igual que `ENCONTRAR` |
| `TEXTO`/`TEXT` `(valor; formato)` | 2 | ver abajo |
| `VALOR`/`VALUE` | 1 | numero -> el mismo; texto vacio o en blanco -> 0; texto -> `numero_escrito` (`12,5`, `1.234,50`, `15%`) o, si no, fecha (`2000-01-01`, `1/1/2000` -> serial); si no `#¡VALOR!`; logico -> `#¡VALOR!` |

**TEXTO.** Solo se mira la primera seccion del formato (hasta un `;`). El
resultado es un **texto** (no se localiza luego con la coma del usuario).
- Valor texto: si lee como numero o como fecha (`2000-01-01`) se formatea
  como tal; si no, pasa tal cual (y el vacio, vacio). Logico -> `VERDADERO`/`FALSO`.
- `General` o formato vacio -> vista General.
- Con `0` o `#` en el formato es un **formato de numero**; si no y tiene
  alguna de `d m y a h s` fuera de comillas, es de **fecha**.
- Numero: lo anterior al primer marcador (`0 # ? . ,`) es texto fijo de delante,
  lo posterior al ultimo es de detras; `"..."`, `\x` y `_x` (un espacio) valen;
  cualquier otro caracter es literal. `%` multiplica por 100 y pone el `%` donde
  este. Entera: ceros = minimo de cifras (`000` -> `005`); `,` en la parte
  entera = separador de miles (coma). Decimales: cada `0`/`#`/`?` tras el `.`
  es una posicion; los `#` finales se recortan si son cero. El redondeo es el
  de `REDONDEAR` (`TEXTO(1.005;"0.00")` = `1.01`). Sin enteros obligatorios y
  entera `0`, se quita (`TEXTO(0.5;"#.00")` = `.50`). Negativo: `-` al
  principio de todo. Decimal siempre `.`, miles siempre `,`.
- Fecha: `yyyy`/`aaaa` (4 cifras), `yy`/`aa` (2), `m` `mm` (numero), `mmm` (3
  letras del mes), `mmmm` (mes entero), `d` `dd`, `ddd` (3 letras), `dddd` (dia
  entero), `h` `hh`, `s` `ss`, `AM/PM`. **`m` o `mm` son minutos si van
  justo detras de una hora (`h`) o justo delante de unos segundos (`s`),
  saltando literales**; en otro caso, mes. Con `AM/PM` la hora va de 1 a 12.
  Meses y dias en **español** en minusculas (`enero`..., `domingo`, `miércoles`,
  `sábado`); abreviados = las 3 primeras letras (`mié`, `sáb`, `oct`).
- Notacion cientifica (`0.0E+00`) y condiciones entre corchetes -> `#¡VALOR!`.
- Una fecha negativa -> `#¡NUM!`.

### Fecha (serial de Excel, sistema 1900)

El 1-ene-1900 es el **1**. Excel cree que 1900 fue bisiesto: el serial **60 es
el 29-feb-1900** (que no existio); del 1-mar-1900 (**61**) en adelante el
serial es `dias desde 1899-12-30`. Del 1 al 59 es `dias desde 1899-12-31`. El 0
se ensena como `0/1/1900` (`AÑO(0)`=1900, `MES(0)`=1, `DIA(0)`=0).
Referencias: `FECHA(2000;1;1)` = 36526; `FECHA(2026;10;11)` = 46306;
`FECHA(1900;3;1)` = 61. Un serial valido va de 0 a 2958465 (31-dic-9999).

| Funcion | Args | Regla |
|---|---|---|
| `HOY`/`TODAY` | 0 | serial entero de hoy (hora local) |
| `AHORA`/`NOW` | 0 | con fraccion (25569 + ms desde 1970 locales / 86400000) |
| `FECHA`/`DATE` `(a; m; d)` | 3 | trunca los tres; `a` en 0..1899 se suma a 1900; `a` fuera de 0..9999 -> `#¡NUM!`; **el mes y el dia se reparten** (mes 14 = feb del año siguiente, mes 0 = dic del anterior, dia 0 = ultimo del mes anterior); resultado fuera de 0..2958465 -> `#¡NUM!`. `FECHA(1900;2;29)` = 60 |
| `AÑO`/`YEAR` (`ANO`), `MES`/`MONTH`, `DIA`/`DAY` | 1 | argumento: numero (se usa su parte entera), logico (1/0), texto vacio (0), texto con forma de fecha o `#¡VALOR!`. Negativo, mayor de 2958465 o no finito -> `#¡NUM!` |
| `DIAS`/`DAYS` `(fin; inicio)` | 2 | `suelo(fin) - suelo(inicio)` |
| `DIASEM`/`WEEKDAY` `(f; [tipo=1])` | 1..2 | tipo 1 (o 17): domingo=1..sabado=7; 2 (u 11): lunes=1..domingo=7; 3: lunes=0..domingo=6; otro -> `#¡NUM!`. Calculado como `(serial-1) mod 7` con 0 = domingo para todo serial (Excel incluido el 1 de enero de 1900 como domingo) |

Texto con forma de fecha: tres partes separadas por `/`, `-` o `.`, cada una de
1 a 4 cifras; si la primera tiene 4 cifras es `año-mes-dia`, si la tercera
tiene 4 es `dia/mes/año` (**dia primero**); año 1900..9999, mes 1..12, dia
1..31 **y que el dia exista en ese mes** (el 31/04 no es fecha). Cualquier otra
cosa (año de 2 cifras incluido) no es fecha.

### Informacion
| Funcion | Args | Regla |
|---|---|---|
| `ESERROR`/`ISERROR` | 1 | `VERDADERO` para cualquier error **menos el ciclo** (que sube) |
| `ESND`/`ISNA` | 1 | solo `#N/A` |
| `ESBLANCO`/`ISBLANK` | 1 | el valor es `""` (ver modelo) |
| `ESNUMERO`/`ISNUMBER` | 1 | numero de verdad (`"1"` entre comillas no; un logico no) |
| `ESTEXTO`/`ISTEXT` | 1 | texto **no vacio** |

Todas leen su argumento como un valor suelto: un rango de varias celdas ahi es
el error `#¡VALOR!` (y por tanto `ESERROR(A1:A3)` es `VERDADERO`; no se usa).

## 5. Rendimiento y memoria (lo que el PC hace y conviene copiar)

- Meta del PC: hoja de 2 000 celdas con formulas (mezcla de todas las familias,
  ver `hoja_grande()` en las pruebas) en < 50 ms; mide ~15 ms. No es un
  requisito en Android, pero **sin la memoria por pasada** (seccion 3) el
  coste se dispara.
- Se guardan: el valor de cada celda; la lectura de cada rango (celdas
  escritas con su valor; no se guarda si algun valor es ciclo); y el
  resultado de cada `CONTAR.SI`-`SUMAR.SI`-... ya hecho con los mismos
  rangos y criterios (solo si fue bueno).
- El hash y las claves: el PC indexa el contenido de las celdas por posicion
  (columna, fila) en vez de por el nombre `B7`, que evita construir el nombre
  en cada lectura.

## 6. Tabla de pruebas

Se usan dos juegos de datos. Cada fila: formula (sin el `=`), resultado
mostrado. Las que dicen «sin datos» se escriben en una hoja vacia (salvo la
propia celda).

### 6.1 Sin datos

| Formula | Resultado |
|---|---|
| `1+2*3` | `7` |
| `(1+2)*3` | `9` |
| `2^3^2` | `64` |
| `-2^2` | `4` |
| `2^-1` | `0.5` |
| `50%` | `0.5` |
| `200*10%` | `20` |
| `1E3+1` | `1001` |
| `--3` | `3` |
| `1+2&3` | `33` |
| `1+1=2` | `VERDADERO` |
| `"a"&"b"` | `ab` |
| `"n"&1.5` | `n1.5` |
| `"a""b"` | `a"b` |
| `"x"&VERDADERO` | `xVERDADERO` |
| `"abc` | `#¡VALOR!` |
| `"a"&1/0` | `#¡DIV/0!` |
| `1<2` | `VERDADERO` |
| `2<=1` | `FALSO` |
| `"A"="a"` | `VERDADERO` |
| `1<"a"` | `VERDADERO` |
| `"a"<VERDADERO` | `VERDADERO` |
| `0.1+0.2=0.3` | `VERDADERO` |
| `VERDADERO+1` | `2` |
| `B9=0` (B9 vacia) | `VERDADERO` |
| `B9=""` | `VERDADERO` |
| `B9=FALSO` | `VERDADERO` |
| `B9="a"` | `FALSO` |
| `1/0=1` | `#¡DIV/0!` |
| `"3"+4` | `7` |
| `1+"abc"` | `#¡VALOR!` |
| `0^0` | `#¡NUM!` |
| `(-8)^0.5` | `#¡NUM!` |
| `0^-1` | `#¡DIV/0!` |
| `10^400` | `#¡NUM!` |
| `1E308*10` | `#¡NUM!` |
| `(-2)^3` | `-8` |
| `SI(1>0;"si";"no")` | `si` |
| `SI(FALSO;1)` | `FALSO` |
| `SI("VERDADERO";1;2)` | `1` |
| `SI(VERDADERO;1;1/0)` | `1` |
| `SI(FALSO;1/0;2)` | `2` |
| `SI(B9;1;2)` | `2` |
| `SI(1)` | `#¡VALOR!` |
| `SI(1;2;3;4)` | `#¡VALOR!` |
| `SI("x";1;2)` | `#¡VALOR!` |
| `SI(1/0;1;2)` | `#¡DIV/0!` |
| `SI(VERDADERO;1/0;2)` | `#¡DIV/0!` |
| `Y(1;1)` | `VERDADERO` |
| `Y(1;0)` | `FALSO` |
| `O(0;0;1)` | `VERDADERO` |
| `NO(0)` | `VERDADERO` |
| `Y()` | `#¡VALOR!` |
| `NO(1;2)` | `#¡VALOR!` |
| `Y(1;1/0)` | `#¡DIV/0!` |
| `NO("x")` | `#¡VALOR!` |
| `SI.CONJUNTO(FALSO;1;VERDADERO;2;VERDADERO;3)` | `2` |
| `SI.CONJUNTO(FALSO;1;FALSO;2)` | `#N/A` |
| `SI.CONJUNTO(VERDADERO;1;FALSO)` | `#¡VALOR!` |
| `SI.CONJUNTO(VERDADERO;1;1/0;2)` | `1` |
| `SI.ERROR(1/0;"x")` | `x` |
| `SI.ERROR(5;"x")` | `5` |
| `SI.ERROR("a"+1;"malo")` | `malo` |
| `SI.ERROR(1)` | `#¡VALOR!` |
| `REDONDEAR(2.675;2)` | `2.68` |
| `REDONDEAR(1.005;2)` | `1.01` |
| `REDONDEAR(2.5;0)` | `3` |
| `REDONDEAR(-2.5;0)` | `-3` |
| `REDONDEAR(1234.567;-2)` | `1200` |
| `REDONDEAR(1250;-2)` | `1300` |
| `REDONDEAR.MAS(2.1;0)` | `3` |
| `REDONDEAR.MAS(-2.1;0)` | `-3` |
| `REDONDEAR.MAS(2.123;2)` | `2.13` |
| `REDONDEAR.MENOS(2.9;0)` | `2` |
| `REDONDEAR.MENOS(-2.9;0)` | `-2` |
| `REDONDEAR.MENOS(2.999;2)` | `2.99` |
| `REDONDEAR.MENOS(1999;-3)` | `1000` |
| `REDONDEAR(0;2)` | `0` |
| `ENTERO(2.9)` | `2` |
| `ENTERO(-1.5)` | `-2` |
| `REDONDEAR(1)` | `#¡VALOR!` |
| `REDONDEAR("x";1)` | `#¡VALOR!` |
| `REDONDEAR(1/0;1)` | `#¡DIV/0!` |
| `ABS(-3)` | `3` |
| `RESIDUO(10;3)` | `1` |
| `RESIDUO(-3;2)` | `1` |
| `RESIDUO(3;-2)` | `-1` |
| `RESIDUO(5.5;2)` | `1.5` |
| `RESIDUO(1;0)` | `#¡DIV/0!` |
| `POTENCIA(2;10)` | `1024` |
| `POTENCIA(9;0.5)` | `3` |
| `POTENCIA(0;0)` | `#¡NUM!` |
| `POTENCIA(0;-1)` | `#¡DIV/0!` |
| `RAIZ(16)` | `4` |
| `RAIZ(2)` | `1.414213562` |
| `RAIZ(-1)` | `#¡NUM!` |
| `PI()` | `3.141592654` |
| `PI(1)` | `#¡VALOR!` |
| `ABS()` | `#¡VALOR!` |
| `ABS("x")` | `#¡VALOR!` |
| `PRODUCTO(2;3;4)` | `24` |
| `PRODUCTO(Q1:Q9)` (vacio) | `0` |
| `PRODUCTO()` | `#¡VALOR!` |
| `MEDIANA(1;3;2)` | `2` |
| `MEDIANA(1;2;3;4)` | `2.5` |
| `MEDIANA(Q1:Q9)` (vacio) | `#¡NUM!` |
| `ALEATORIO.ENTRE(5;5)` | `5` |
| `ALEATORIO.ENTRE(6;1)` | `#¡NUM!` |
| `ALEATORIO.ENTRE(2.2;2.8)` | `#¡NUM!` |
| `ALEATORIO(1)` | `#¡VALOR!` |
| `CONCATENAR("a";1;"b")` | `a1b` |
| `IZQUIERDA("hola";2)` | `ho` |
| `IZQUIERDA("hola")` | `h` |
| `IZQUIERDA("hola";10)` | `hola` |
| `IZQUIERDA("hola";0)` | (vacio) |
| `IZQUIERDA(12345;2)` | `12` |
| `IZQUIERDA("añb";2)` | `añ` |
| `IZQUIERDA("hola";-1)` | `#¡VALOR!` |
| `DERECHA("hola";3)` | `ola` |
| `EXTRAE("hola mundo";6;5)` | `mundo` |
| `EXTRAE("hola";10;2)` | (vacio) |
| `EXTRAE("hola";3;99)` | `la` |
| `EXTRAE("hola";0;1)` | `#¡VALOR!` |
| `EXTRAE("hola";1;-1)` | `#¡VALOR!` |
| `LARGO("año")` | `3` |
| `LARGO(123)` | `3` |
| `MAYUSC("añb")` | `AÑB` |
| `MINUSC("ABC Ñ")` | `abc ñ` |
| `NOMPROPIO("hola mUNDO o'neil")` | `Hola Mundo O'Neil` |
| `NOMPROPIO("ana-maria 2da")` | `Ana-Maria 2Da` |
| `ESPACIOS("  a   b  ")` | `a b` |
| `ESPACIOS("   ")` | (vacio) |
| `SUSTITUIR("a-b-c";"-";"+")` | `a+b+c` |
| `SUSTITUIR("a-b-c";"-";"+";2)` | `a-b+c` |
| `SUSTITUIR("a-b-c";"-";"+";5)` | `a-b-c` |
| `SUSTITUIR("abc";"";"x")` | `abc` |
| `SUSTITUIR("ABC";"b";"x")` | `ABC` |
| `SUSTITUIR("aaa";"aa";"b")` | `ba` |
| `SUSTITUIR("a";"a";"b";0)` | `#¡VALOR!` |
| `ENCONTRAR("b";"abcb")` | `2` |
| `ENCONTRAR("b";"abcb";3)` | `4` |
| `ENCONTRAR("";"abc")` | `1` |
| `ENCONTRAR("ñ";"año")` | `2` |
| `ENCONTRAR("B";"abc")` | `#¡VALOR!` |
| `ENCONTRAR("a";"abc";0)` | `#¡VALOR!` |
| `ENCONTRAR("a";"abc";9)` | `#¡VALOR!` |
| `HALLAR("B";"abc")` | `2` |
| `HALLAR("a?c";"xxabcx")` | `3` |
| `HALLAR("a*c";"xxabcx")` | `3` |
| `HALLAR("~?";"a?b")` | `2` |
| `HALLAR("z";"abc")` | `#¡VALOR!` |
| `TEXTO(1234.5;"#,##0.00")` | `1,234.50` |
| `TEXTO(1234;"#,##0")` | `1,234` |
| `TEXTO(1234567.891;"#,##0.0")` | `1,234,567.9` |
| `TEXTO(-1234.5;"#,##0.0")` | `-1,234.5` |
| `TEXTO(0.256;"0%")` | `26%` |
| `TEXTO(0.256;"0.0%")` | `25.6%` |
| `TEXTO(5;"000")` | `005` |
| `TEXTO(3.14159;"0.00")` | `3.14` |
| `TEXTO(3;"0.00")` | `3.00` |
| `TEXTO(3.5;"0.##")` | `3.5` |
| `TEXTO(3;"0.##")` | `3` |
| `TEXTO(1.005;"0.00")` | `1.01` |
| `TEXTO(0.5;"#.00")` | `.50` |
| `TEXTO(12;"$#,##0.00")` | `$12.00` |
| `TEXTO(12;"0 ""kg""")` | `12 kg` |
| `TEXTO(2.5;"0")` | `3` |
| `TEXTO("hola";"0")` | `hola` |
| `TEXTO("";"0.00")` | (vacio) |
| `TEXTO(7;"General")` | `7` |
| `TEXTO(1)` | `#¡VALOR!` |
| `TEXTO(1;"0.0E+00")` | `#¡VALOR!` |
| `TEXTO(36526;"dd/mm/aaaa")` | `01/01/2000` |
| `TEXTO(36526;"yyyy-mm-dd")` | `2000-01-01` |
| `TEXTO(36526;"d/m/yy")` | `1/1/00` |
| `TEXTO(46306;"mmmm")` | `octubre` |
| `TEXTO(46306;"mmm")` | `oct` |
| `TEXTO(46306;"dddd")` | `domingo` |
| `TEXTO(36526;"dddd d ""de"" mmmm")` | `sábado 1 de enero` |
| `TEXTO(0.75;"hh:mm")` | `18:00` |
| `TEXTO(0.5;"h:mm AM/PM")` | `12:00 PM` |
| `TEXTO(0.25;"hh:mm:ss")` | `06:00:00` |
| `TEXTO(46306.5;"dd/mm/aaaa hh:mm")` | `11/10/2026 12:00` |
| `TEXTO(60;"dd/mm/yyyy")` | `29/02/1900` |
| `TEXTO("2000-01-01";"dd/mm/yyyy")` | `01/01/2000` |
| `TEXTO(-1;"dd/mm/yyyy")` | `#¡NUM!` |
| `TEXTO("hola";"dd/mm/yyyy")` | `hola` |
| `VALOR("12,5")` | `12.5` |
| `VALOR("1.234,50")` | `1234.5` |
| `VALOR("15%")` | `0.15` |
| `VALOR("")` | `0` |
| `VALOR("2000-01-01")` | `36526` |
| `VALOR("1/1/2000")` | `36526` |
| `VALOR(" 7 ")` | `7` |
| `VALOR("abc")` | `#¡VALOR!` |
| `VALOR(VERDADERO)` | `#¡VALOR!` |
| `FECHA(2026;10;11)` | `46306` |
| `FECHA(2000;1;1)` | `36526` |
| `FECHA(1900;1;1)` | `1` |
| `FECHA(1900;3;1)` | `61` |
| `FECHA(1900;2;29)` | `60` |
| `AÑO(FECHA(2026;14;1))` | `2027` |
| `MES(FECHA(2026;14;1))` | `2` |
| `DIA(FECHA(2026;1;0))` | `31` |
| `MES(FECHA(2026;1;0))` | `12` |
| `MES(FECHA(2026;0;15))` | `12` |
| `DIA(FECHA(2026;3;0))` | `28` |
| `AÑO(FECHA(99;1;1))` | `1999` |
| `AÑO(36526)` | `2000` |
| `MES(46306)` | `10` |
| `DIA(46306)` | `11` |
| `DIA(60)` | `29` |
| `MES(60)` | `2` |
| `DIA(59)` | `28` |
| `AÑO("2000-01-01")` | `2000` |
| `MES("11/10/2026")` | `10` |
| `DIAS(FECHA(2026;10;11);FECHA(2026;1;1))` | `283` |
| `DIAS(36526;36527)` | `-1` |
| `DIASEM(36526)` | `7` |
| `DIASEM(36526;2)` | `6` |
| `DIASEM(36526;3)` | `5` |
| `DIASEM(1)` | `1` |
| `DIASEM(46306)` | `1` |
| `FECHA(-1;1;1)` | `#¡NUM!` |
| `FECHA(10000;1;1)` | `#¡NUM!` |
| `FECHA(2026;1)` | `#¡VALOR!` |
| `FECHA("x";1;1)` | `#¡VALOR!` |
| `AÑO(-1)` | `#¡NUM!` |
| `AÑO("hola")` | `#¡VALOR!` |
| `DIA("31/04/2026")` | `#¡VALOR!` |
| `DIASEM(36526;9)` | `#¡NUM!` |
| `HOY(1)` | `#¡VALOR!` |
| `NOEXISTE(1)` | `#¿NOMBRE?` |
| `SUMA(1;;2)`, `SUMA(;1)`, `1 +* 2`, `((1)`, `1)`, `"a" "b"`, `A1:`, `A1:5`, `{1,2}`, `&1`, `TRUE FALSE`, `NOEXISTE` | todas error (`#¡VALOR!` o `#¡REF!`) |

Con el reloj fijado al 11-oct-2026 15:30: `HOY()` = 46306; `AHORA()` =
46306.645833...; `ENTERO(AHORA())=HOY()` = `VERDADERO`.

### 6.2 Con datos

Hoja de datos:

| | A | B | C | D | F | H | I | J | L | M | N | O | P | Q | R | S |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 10 | 1 | Ana | 20 | `=1/0` | 10 | a | 1 | 1 | 2 | 3 | 4 | 30 | | 1 | a |
| 2 | 20 | 2 | Anibal | | | 20 | b | 2 | w | x | y | z | 20 | | 1 | b |
| 3 | 30 | 3 | Beto | | | 30 | c | 3 | | | | | 10 | z | 2 | c |
| 4 | x | 4 | | | | 40 | d | 4 | | | | | | | | |
| 5 | 40 | 5 | | | | | | | | | | | | | | |

(`A4` es el texto `x`; todo lo no puesto esta vacio.)

| Formula | Resultado |
|---|---|
| `Y(A1:A3)` | `VERDADERO` |
| `O(B9:B10;0)` | `FALSO` |
| `Y("hola")` | `#¡VALOR!` |
| `Y(1;"verdadero")` | `VERDADERO` |
| `Y(C1:C3)` | `#¡VALOR!` |
| `SI.ERROR(F1;0)` | `0` |
| `SI.ERROR(SUMA(F1:F2);-1)` | `-1` |
| `SI.ND(BUSCARV(9;H1:J4;2;0);"no")` | `no` |
| `SI.ND(1/0;"no")` | `#¡DIV/0!` |
| `PRODUCTO(A1:A3)` | `6000` |
| `PRODUCTO(A1:A5)` | `240000` |
| `MEDIANA(A1:A5)` | `25` |
| `MEDIANA(F1:F2)` | `#¡DIV/0!` |
| `SUMA(A1;A2,A3)` | `60` |
| `MAX(A1:A3)` | `30` |
| `CONTARA(A1:A5)` | `5` |
| `CONTARA(A1:A100)` | `5` |
| `CONTARA(A1:A2;"x";9)` | `4` |
| `CONTARA(F1:F2)` | `1` |
| `CONTAR.BLANCO(A1:A10)` | `5` |
| `CONTAR.BLANCO(Q1:Q9)` | `8` |
| `CONTAR.BLANCO(A1)` | `0` |
| `CONTAR.BLANCO(A9)` | `1` |
| `SUMAR.SI(A1:A5;">15")` | `90` |
| `SUMAR.SI(A1:A5;">15";B1:B5)` | `10` |
| `SUMAR.SI(A1:A5;"x";B1:B5)` | `4` |
| `SUMAR.SI(A1:A5;20)` | `20` |
| `SUMAR.SI(A1:A1000000;">0")` | `100` |
| `CONTAR.SI(A1:A5;">15")` | `3` |
| `CONTAR.SI(A1:A5;"<>10")` | `4` |
| `CONTAR.SI(A1:A5;10)` | `1` |
| `CONTAR.SI(A1:A5;"=x")` | `1` |
| `CONTAR.SI(A1:A5;"X")` | `1` |
| `CONTAR.SI(A1:A5;D1)` | `1` |
| `CONTAR.SI(A1:A5;">=20")` | `3` |
| `CONTAR.SI(A1:A5;"<=20")` | `2` |
| `CONTAR.SI(A1:A5;"<30")` | `2` |
| `CONTAR.SI(C1:C4;"An*")` | `2` |
| `CONTAR.SI(C1:C4;"?eto")` | `1` |
| `CONTAR.SI(C1:C4;"*")` | `3` |
| `CONTAR.SI(C1:C4;"*i*")` | `1` |
| `CONTAR.SI(C1:C4;"ana")` | `1` |
| `CONTAR.SI(C1:C4;"")` | `1` |
| `CONTAR.SI(C1:C4;"<>")` | `3` |
| `CONTAR.SI(A1:A100;"")` | `95` |
| `CONTAR.SI(A1:A1000000;"")` | `999995` |
| `CONTAR.SI(A1:A100;"<>")` | `5` |
| `CONTAR.SI(A1:A5;">100")` | `0` |
| `CONTAR.SI(A1:A5)` | `#¡VALOR!` |
| `CONTAR.SI(5;1)` | `#¡VALOR!` |
| `SUMAR.SI(A1:A5;1;B1:B5;1)` | `#¡VALOR!` |
| `CONTAR.SI(A1:A5;1/0)` | `#¡DIV/0!` |
| `SUMAR.SI(A1:A5;">0";F1:F5)` | `#¡DIV/0!` |
| `CONTAR.SI(F1:F2;">0")` | `0` |
| `SUMAR.SI.CONJUNTO(B1:B5;A1:A5;">15";B1:B5;"<5")` | `5` |
| `SUMAR.SI.CONJUNTO(B1:B5;A1:A5;">15")` | `10` |
| `CONTAR.SI.CONJUNTO(A1:A5;">=10";B1:B5;"<=2")` | `2` |
| `CONTAR.SI.CONJUNTO(A1:A5;">=10")` | `4` |
| `PROMEDIO.SI(A1:A5;">15")` | `30` |
| `PROMEDIO.SI(A1:A5;">15";B1:B5)` | `3.333333333` |
| `CONTAR.SI.CONJUNTO(A1:A100;"";B1:B100;"")` | `95` |
| `CONTAR.SI.CONJUNTO(A1:A100;"";B1:B100;">0")` | `0` |
| `CONTAR.SI.CONJUNTO(A1:A5;">1";B1:B4;">1")` | `#¡VALOR!` |
| `SUMAR.SI.CONJUNTO(B1:B4;A1:A5;">1")` | `#¡VALOR!` |
| `CONTAR.SI.CONJUNTO(A1:A5;">1";B1:B5)` | `#¡VALOR!` |
| `PROMEDIO.SI(A1:A5;">100")` | `#¡DIV/0!` |
| `PROMEDIO.SI(A1:A5;"x")` | `#¡DIV/0!` |
| `PROMEDIO.SI(A1:A5)` | `#¡VALOR!` |
| `BUSCARV(20;H1:J4;2;FALSO)` | `b` |
| `BUSCARV(40;H1:J4;3;0)` | `4` |
| `BUSCARV(25;H1:J4;2;VERDADERO)` | `b` |
| `BUSCARV(25;H1:J4;2)` | `b` |
| `BUSCARV(99;H1:J4;2)` | `d` |
| `BUSCARV(10;H1:J4;2)` | `a` |
| `BUSCARV(1;H1:J4;1;1)` | `#N/A` |
| `BUSCARV(5;H1:J4;2;0)` | `#N/A` |
| `BUSCARV(25;H1:J4;2;0)` | `#N/A` |
| `BUSCARV("B";I1:J4;2;0)` | `2` |
| `BUSCARV("c*";I1:J4;2;0)` | `3` |
| `BUSCARV("c*";I1:J4;2;1)` | `3` |
| `BUSCARV("zz";I1:J4;2)` | `4` |
| `BUSCARV("20";H1:J4;2;0)` | `#N/A` |
| `BUSCARV(20;A1:B5;2;0)` | `2` |
| `BUSCARV(20;H1:J4;4;0)` | `#¡REF!` |
| `BUSCARV(20;H1:J4;0;0)` | `#¡VALOR!` |
| `BUSCARV(20;H1:J4;-1;0)` | `#¡VALOR!` |
| `BUSCARV(20;H1:J4)` | `#¡VALOR!` |
| `BUSCARV(20;H1:J4;2;0;1)` | `#¡VALOR!` |
| `BUSCARV(20;H1:J4;2;"x")` | `#¡VALOR!` |
| `BUSCARV(1/0;H1:J4;2;0)` | `#¡DIV/0!` |
| `BUSCARV(1;X1:Y5;2;0)` (rango vacio) | `#N/A` |
| `BUSCARH(3;L1:O2;2;0)` | `y` |
| `BUSCARH(9;L1:O2;2)` | `z` |
| `BUSCARH(2.5;L1:O2;1)` | `2` |
| `BUSCARH(9;L1:O2;1;0)` | `#N/A` |
| `BUSCARH(3;L1:O2;3;0)` | `#¡REF!` |
| `BUSCARH(3;L1:O2;0;0)` | `#¡VALOR!` |
| `INDICE(H1:J4;2;3)` | `2` |
| `INDICE(H1:J4;4;2)` | `d` |
| `INDICE(H1:H4;3)` | `30` |
| `INDICE(L1:O1;3)` | `3` |
| `INDICE(H1:H4;3;1)` | `30` |
| `INDICE(H1:J4;5;1)` | `#¡REF!` |
| `INDICE(H1:J4;1;4)` | `#¡REF!` |
| `INDICE(H1:J4;2)` | `#¡VALOR!` |
| `INDICE(H1:J4;0;1)` | `#¡VALOR!` |
| `INDICE(H1:J4;-1;1)` | `#¡VALOR!` |
| `INDICE(H1:J4)` | `#¡VALOR!` |
| `INDICE(Q1:Q5;1)` | (vacio) |
| `COINCIDIR(30;H1:H4;0)` | `3` |
| `COINCIDIR(35;H1:H4;1)` | `3` |
| `COINCIDIR(35;H1:H4)` | `3` |
| `COINCIDIR(25;P1:P3;-1)` | `1` |
| `COINCIDIR("C";I1:I4;0)` | `3` |
| `COINCIDIR("c?";I1:I4;0)` | `#N/A` |
| `COINCIDIR("?";I1:I4;0)` | `1` |
| `COINCIDIR(3;L1:O1;0)` | `3` |
| `COINCIDIR("z";Q1:Q5;0)` | `3` |
| `COINCIDIR(35;H1:H4;0)` | `#N/A` |
| `COINCIDIR(5;H1:H4;1)` | `#N/A` |
| `COINCIDIR(1;H1:J4;0)` | `#N/A` |
| `COINCIDIR(1;H1:H4;0;1)` | `#¡VALOR!` |
| `BUSCARX(20;H1:H4;I1:I4)` | `b` |
| `BUSCARX(25;H1:H4;I1:I4)` | `#N/A` |
| `BUSCARX(25;H1:H4;I1:I4;"nada")` | `nada` |
| `BUSCARX(20;H1:H4;I1:I4;"nada")` | `b` |
| `BUSCARX(25;H1:H4;I1:I4;"-";-1)` | `b` |
| `BUSCARX(25;H1:H4;I1:I4;"-";1)` | `c` |
| `BUSCARX(99;H1:H4;I1:I4;"-";1)` | `-` |
| `BUSCARX(1;H1:H4;I1:I4;"-";-1)` | `-` |
| `BUSCARX("c*";I1:I4;H1:H4;"-";2)` | `30` |
| `BUSCARX("c*";I1:I4;H1:H4;"-")` | `-` |
| `BUSCARX(1;R1:R3;S1:S3)` | `a` |
| `BUSCARX(1;R1:R3;S1:S3;"x";0;-1)` | `b` |
| `BUSCARX(3;L1:O1;L2:O2)` | `y` |
| `BUSCARX(3;L1:O1;H1:H4)` | `30` |
| `BUSCARX(20;H1:H4;I1:I3)` | `#¡VALOR!` |
| `BUSCARX(20;H1:H4;I1:J4)` | `#¡VALOR!` |
| `BUSCARX(20;H1:J4;I1:I4)` | `#N/A` |
| `BUSCARX(20;H1:H4;I1:I4;"-";7)` | `#¡VALOR!` |
| `BUSCARX(20;H1:H4;I1:I4;"-";0;0)` | `#¡VALOR!` |
| `BUSCARX(1/0;H1:H4;I1:I4)` | `#¡DIV/0!` |
| `CONCATENAR(C1;"-";C3)` | `Ana-Beto` |
| `CONCAT(C1:C3)` | `AnaAnibalBeto` |
| `CONCAT("a";C1:C2;1)` | `aAnaAnibal1` |
| `CONCATENAR(C1:C3)` | `#¡VALOR!` |
| `CONCAT(F1:F2)` | `#¡DIV/0!` |
| `UNIRCADENAS("-";VERDADERO;"a";"";"b")` | `a-b` |
| `UNIRCADENAS("-";FALSO;"a";"";"b")` | `a--b` |
| `UNIRCADENAS(", ";VERDADERO;C1:C3)` | `Ana, Anibal, Beto` |
| `UNIRCADENAS("-";FALSO;C1:C4)` | `Ana-Anibal-Beto-` |
| `UNIRCADENAS("-";VERDADERO;C1:C4)` | `Ana-Anibal-Beto` |
| `UNIRCADENAS("-";FALSO;A1:A5)` | `10-20-30-x-40` |
| `UNIRCADENAS("-";"x";"a")` | `#¡VALOR!` |
| `LARGO(Q9)` | `0` |
| `ESERROR(1/0)` | `VERDADERO` |
| `ESERROR(F1)` | `VERDADERO` |
| `ESERROR(BUSCARV(5;H1:J4;2;0))` | `VERDADERO` |
| `ESERROR(1)` | `FALSO` |
| `ESBLANCO(A9)` | `VERDADERO` |
| `ESBLANCO(A1)` | `FALSO` |
| `ESBLANCO("")` | `VERDADERO` |
| `ESBLANCO(F1)` | `FALSO` |
| `ESNUMERO(1)` | `VERDADERO` |
| `ESNUMERO("1")` | `FALSO` |
| `ESNUMERO(A4)` | `FALSO` |
| `ESNUMERO(VERDADERO)` | `FALSO` |
| `ESTEXTO(A4)` | `VERDADERO` |
| `ESTEXTO(A1)` | `FALSO` |
| `ESND(BUSCARV(5;H1:J4;2;0))` | `VERDADERO` |
| `ESND(1/0)` | `FALSO` |
| `ESERROR()` | `#¡VALOR!` |
| `A1:A3*2` | `#¡VALOR!` |
| `SI(A1:A3;1;2)` | `#¡VALOR!` |
| `A1:A1*2` | `20` |
| `SUMA(SI(VERDADERO;A1:A3;B1:B3))` | `60` |

### 6.3 Ciclos y casos de la hoja entera

Cada fila es una hoja pequena; la celda a mirar es la primera.

| Hoja | Resultado de `A1` |
|---|---|
| `A1 =BUSCARV(1;A1:B2;2;0)`, `A2 = 1`, `B2 = 5` | `#¡CICLO!` |
| `A1 =BUSCARV(2;B1:C3;2;0)`, `B2 = 2`, `C2 =A1` | `#¡CICLO!` |
| `A1 =INDICE(A1:A3;1)`, `A2 = 5` | `#¡CICLO!` |
| `A1 =COINCIDIR(5;A1:A3;0)`, `A2 = 5` | `#¡CICLO!` |
| `A1 =CONTAR.SI(A1:A3;">0")`, `A2 = 5`, `A3 = 6` | `#¡CICLO!` |
| `A1 =CONTARA(A1:A3)`, `A2 = 5` | `#¡CICLO!` |
| `A1 =CONTAR.BLANCO(A1:A3)`, `A2 = 5` | `#¡CICLO!` |
| `A1 =SI.ERROR(A1;0)` | `#¡CICLO!` (no se tapa) |
| `A1 =ESERROR(A1)` | `#¡CICLO!` (no se tapa) |
| `A1 =SUMA(A1:A3)`, `A2 = 2`, `A3 = 3` | `#¡CICLO!` |
| `A1 =B1`, `B1 =A1`, `C1 = 5`, `D1 =C1*2` | `A1` ciclo; **`D1` = 10** (el ciclo no envenena a quien no esta dentro) |
| el ciclo de la primera fila mas `D1 = 1`, `E1 = 7`, `F1 =BUSCARV(1;D1:E1;2;0)` | `F1` = `7` |
| `A1 = 1`, `A2 =A1+1`, ..., `A2000 =A1999+1` (calculando una sola) | `A10` = 10; **`A2000` = `#¡CICLO!`** (pasa de 128 encadenadas) |
| la misma columna calculada **entera** (con memoria) | `A2000` = 2000 (cada una encuentra hecha la de encima) |

### 6.4 Compatibilidad con lo de siempre

`A1=10`, `A2=20.5`, `A3=x`; `SUMA(A1:A3)` = `30.5`, `CONTAR` = `2`,
`PROMEDIO` = `15.25`, `MIN` = `10`, `MAX` = `20.5`, `SUMA()` = `0`, `MAX()` = `0`,
`PROMEDIO()` = `#¡DIV/0!`. En es-ES el primero se ensena `30,5`.

### 6.5 Alias

Para cada par (espanol, ingles) el resultado es el mismo: `SUMA`/`SUM`,
`SI`/`IF`, `SI.ERROR`/`IFERROR`, `CONTAR.SI`/`COUNTIF`, `SUMAR.SI`/`SUMIF`,
`BUSCARV`/`VLOOKUP`, `INDICE`/`INDEX`, `COINCIDIR`/`MATCH`, `IZQUIERDA`/`LEFT`,
`NOMPROPIO`/`PROPER`, `AÑO`/`YEAR`, `ESNUMERO`/`ISNUMBER`,
`REDONDEAR`/`ROUND`, `MAX`/`MAXIMO`; mayusculas o minusculas da igual
(`max(a1:a3)`). Lista completa de alias: `motor::CATALOGO` en el PC.

## 7. Lo que NO se hizo (y es posible que Android si tenga)

- Matrices dinamicas / derrame (`FILTRAR`, `ORDENAR`...), `LET`, `LAMBDA`.
- Referencias a otras hojas (`Hoja2!A1`) y nombres definidos.
- Formatos numericos con secciones (`pos;neg;cero`), notacion cientifica y
  condiciones en `TEXTO`; `TEXTO` con formato de fecha/numero «a la española»
  (`#.##0,00`): el formato se lee siempre con `,` de miles y `.` de decimales.
- `BUSCARX` con tablas de varias columnas, y `BUSCARV` con busqueda binaria
  real (aqui la aproximada es lineal, ver arriba).
- Los criterios de `CONTAR.SI` con fechas escritas («>=1/1/2026»): no se
  leen como fecha, el criterio quedaria como texto.

Si Android ya tiene alguna de ellas con otro comportamiento, **no la cambies sin
avisar**: apunta la diferencia aqui y decide con el usuario.
