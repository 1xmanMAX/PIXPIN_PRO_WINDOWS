# El universo del movil, medido (19-sep-2026)

Por que existe este documento: el universo del PC se diseno **aqui** el 18-sep
(`docs/superpowers/specs/2026-09-18-universo-design.md`), cuando el movil
todavia no tenia universo. Desde entonces Android lo implemento —v0.59
«sistema solar de proyectos y tema Cosmos», v0.61 «universos», v0.61.1 el «+»,
hasta v0.66.0 (`f2cb96a`)— y ahora el del PC tiene que **lucir y comportarse
como el del movil**.

Todo lo que se dice aqui esta medido leyendo el Kotlin, no recordado. Las
fuentes, bajadas a mano:

| Fichero del movil | Lineas | Que es |
| --- | --- | --- |
| `ui/theme/Cosmos.kt` | 119 | el tema Cosmos: colores y cielo de fondo |
| `ui/Galaxia.kt` | 1287 | la pantalla del sistema solar de proyectos |
| `ui/GalaxiaModelo.kt` | 134 | `galaxia.json`: donde esta cada sol y cada nota |
| `ui/FisicaDeGalaxia.kt` | 209 | Verlet con cuerdas, varillas y choques |
| `ui/Universo.kt` | 799 | dentro de un proyecto: el organizador |
| `ui/UniversoModelo.kt` | 162 | `universos.json`: espacios y cuerpos |
| `ui/GalaxiaDeProyecto.kt` | 495 | la version vieja del «dentro», ya no se usa |

Pruebas del movil leidas: `GalaxiaTest`, `TemaCosmosTest`, `UniversosTest`,
`FisicaDeGalaxiaTest`.

---

## 0. Lo primero: NO son el mismo producto

El movil tiene **dos pantallas** y el PC **una**, y no se solapan:

1. **`PantallaDeGalaxia`** (el «sistema solar de proyectos»): un plano infinito
   donde **cada proyecto es un sol**. Alrededor orbitan **notas y emojis** que
   el usuario deja a mano (los llama «exoplanetas»), y entre soles hay
   **conexiones** que tiran con fisica de cuerdas. No hay archivos aqui.
2. **`PantallaDeUniverso`** (dentro de un sol): un **organizador vacio** que se
   llena a mano con «+»: del chat, hojas, notas, rotulos, figuras, imagenes,
   emojis. Cada cuerpo puede abrir su propio subespacio, y asi hacia dentro.
   **Sin fisica**: las cosas se quedan donde se dejan.

El PC, en cambio, tiene **un solo plano** con tres niveles anidados —galaxia
(= proyecto) ⊃ planeta ⊃ luna (= archivo del chat)— y las lunas **aparecen
solas** desde el indice del chat (`galaxias::sincronizar`,
`crates/pixpin-universo/src/galaxias.rs:50`), con nivel de detalle por zoom
(`detalle.rs`), nebulosa de lo no colocado (`nebulosa.rs`), capa de
anotaciones, minimapa, inspector y buscador.

**Conclusion de disposicion:** la pantalla del PC corresponde a la
`PantallaDeGalaxia` del movil (proyectos como soles en un plano), con lo del
movil que en escritorio si tiene sentido. Lo de `PantallaDeUniverso`
(organizador manual, subespacios) se comenta en §7.

## 1. El formato de datos: **no se porta, y es lo correcto**

El pedido decia «si el formato del movil es distinto, porta el suyo, que tiene
que poder viajar por la sincronizacion». Medido: **no viaja**. Los dos ficheros
del movil son explicitamente del aparato:

- `Galaxia.kt:621-622`: «El archivo del sistema solar, en la carpeta de la
  aplicacion. **Es de este aparato: no viaja.**» → `galaxia.json`.
- `UniversoModelo.kt:17`: «Aqui solo consta donde esta cada cosa y que senala:
  los archivos **no se copian**. […] **Es de este aparato.**» →
  `universos.json`.

Ni `galaxia.json` ni `universos.json` estan en el protocolo de
`pixpin-sincro`; lo que viaja son proyectos, hojas y mensajes. Por tanto:

- **No se cambia `pixpin-universo::formato`.** Nuestro `universo.json` (D240)
  se queda como esta, con su `resto: Map<String, Value>` aplanado que ya
  preserva lo que no entiende.
- **No hay migracion que hacer** y no se pierde nada del usuario.
- Si algun dia el movil decidiera sincronizar lo suyo, lo que habria que
  anadir es un fichero **aparte**, no cambiar el nuestro: son documentos
  distintos (el suyo guarda notas y emojis que el usuario deja; el nuestro
  guarda la colocacion de archivos reales).

Esto queda escrito aqui para no volver a plantearlo.

## 2. El tema Cosmos, color a color

Medido en `Cosmos.kt`. A la derecha, que hay hoy en el PC.

| Que | Movil (exacto) | PC hoy | Que hacer |
| --- | --- | --- | --- |
| Base del tema | `CosmosBase = #0B0F24` | `PALETA.espacio = #0B1020` (`pintar.rs:34`) | pasar a `#0B0F24` |
| Fondo del cielo | degradado radial `#151B3F` → `#05060F`, centro en `(0.5 w, 0.35 h)`, radio `max(w,h)·0.95` (`Cosmos.kt:95`) | color liso `#0B1020` (`pintar.rs:257 p.limpiar`) | **pintar el degradado** |
| Nebulosa 1 | `#5B3FD9` al alfa `0x33`, centro `(0.2 w, 0.22 h)`, radio `0.8 w` | no hay | anadir |
| Nebulosa 2 | `#1FA2C9` al alfa `0x2A`, centro `(0.9 w, 0.78 h)`, radio `0.7 w` | no hay | anadir |
| Texto | `CosmosTexto = #E8EAF6` | `#E8ECF5` | pasar a `#E8EAF6` |
| Texto suave | `CosmosTextoSuave = #B4B9D6` | `#8A93A8` | pasar a `#B4B9D6` (el nuestro es **mucho** mas apagado) |
| Acento | `CosmosDorado = #FFD27A` | `#40A7E3` (azul) | pasar a dorado |
| Secundario / terciario | cian `#6FD3FF`, rosa `#FF8FB1` | — | usar el cian donde hoy va el azul de estado |
| Panel / tarjeta | `#1C2350` al alfa `0xC2` (vidrio) | `#141A2E` al 0.92 | pasar a `#1C2350` α 0.76 |
| Borde de panel | `outlineVariant = #343C6A` | blanco al 0.08 | pasar a `#343C6A` |
| Error | `#FFB4AB` | `#E5484D` | pasar a `#FFB4AB` |

**En la pantalla del universo** (`Galaxia.kt:343`) el movil no usa el
`FondoCosmico` del tema sino un degradado radial propio `#151B3F → #05060F`
con radio 1900 px, **mas** dos nebulosas que se mueven con la camara a 0,6 de
su velocidad (`Galaxia.kt:805-812`): `#5B3FD9` α `0x33` en `(-200, -120)` dp
radio 420 dp, y `#1FA2C9` α `0x2A` en `(+260, +220)` dp radio 360 dp. Eso es
lo que hay que reproducir, no el del tema.

### Estrellas

| Que | Movil (`Galaxia.kt:903-910`) | PC hoy (`estrellas.rs:19-23`) |
| --- | --- | --- |
| Baldosa | 360 dp | 512 px |
| Cuantas por baldosa | 70, 22, 7 (tres tamanos) | 120, 60, 30 |
| Paralaje | **0,35 para las tres** | 0,2 / 0,5 / 1,0 (tres capas distintas) |
| Colores | `#66FFFFFF`, `#99DCE6FF`, `#DDFFF4D6` | blanco con alfa 0,2-0,7 |
| Grosor | 1,1 / 2,2 / 3,3 dp, punta redonda | 1 px, y 2 px las de brillo > 0,55 |

El movil tiene estrellas **de colores** (blanco, azul palido, crema calido) y
mas escasas; el PC las tiene todas blancas y mas densas. Es de las cosas que
mas se notan al comparar las dos capturas.

Lo que **no** se copia: el PC tesela cada capa una vez como triangulos
(`pixpin_render::puntos`) porque a 3000x2000 un mosaico de bitmaps costaba
2,5-3 ms por capa (`estrellas.rs:8-12`). Eso se conserva; lo que cambia son
las cuentas, los colores y los tamanos.

## 3. El sol (= nuestra galaxia)

Medido en `Galaxia.kt:942-1056` (`@Composable Sol`).

| Pieza | Movil | PC hoy (`pintar.rs:274-311`) |
| --- | --- | --- |
| Diametro | `64 + 7·√hojas`, tope **112 dp** (`GalaxiaModelo.kt:107`) | radio fijo `RADIO_GALAXIA = 2000` del mundo |
| Corona ancha | degradado radial del color α **0,40** → transparente, radio **2,1 r** | `p.brillo(centro, r, color, 0.9)` (un disco con alfa lineal) |
| Corona calida | blanco α **0,35** → color α 0,25 → transparente, radio **1,3 r** | no hay |
| Cuerpo | degradado radial `[lerp(color,blanco,0.35), color, lerp(color,negro,0.45)]` | `p.circulo(centro, r·0.25, color α0.9)`: un puntito en el centro |
| Borde | **2 dp** de `lerp(color, blanco, 0.5)` α 0,95 | no hay |
| Dentro | la **portada** del proyecto recortada en circulo sobre blanco con 5 dp de margen; si no tiene hojas, la **inicial** en blanco, negrita, a `0,38·diametro` | nada |
| Rotulo | el nombre, `labelLarge` SemiBold, blanco α **0,96**, centrado, 2 lineas, 8 dp por debajo del disco | «nombre · N» a 13 px, 4 px por debajo |
| Segundo rotulo | «`N hojas`» (o «`archivado · N`»), `labelSmall`, blanco α **0,5** | no hay: va pegado al nombre con « · » |
| Apagado | proyecto archivado: coronas a 0,10/0,05, cuerpo a 0,35/0,3, borde a 0,35, rotulo a 0,5 | no hay concepto de archivado aqui |
| Ancho de la etiqueta | `max(150 dp, diametro)` (`Galaxia.kt:939`) | el que mida el texto |

**Color del sol** (`Galaxia.kt:926-931`): ocho colores fijos, elegidos por
`floorMod(id.hashCode(), 8)` —el `hashCode` de `String` de Java:

```
#FFB84D  #FF7A6B  #7C9CFF  #5FE0C2  #B48CFF  #6FD3FF  #FF8FB1  #FFE066
```

El PC usa hoy `color_avatar(proyecto)` (`pintar.rs:83`), que son los **siete
colores de avatar de Telegram** (`ventana_chat.rs:2255`) elegidos por la suma
de bytes. Dos paletas distintas, dos hashes distintos: el mismo proyecto sale
de otro color en cada aparato.

> Decision: se porta `COLORES_DE_SOL` **con el `hashCode` de Java**, asi el
> mismo proyecto sale del mismo color en los dos aparatos. Rompe a proposito
> el comentario de `pintar.rs:76-82` («el color es el mismo que en el chat»),
> porque en el movil tampoco lo es: `colorDelSol` es solo del universo y el
> chat tiene su propia paleta. Lucir como el movil manda.

## 4. Lo que orbita

### En la pantalla de proyectos (movil)

Un **exoplaneta** es una **nota** o un **emoji** (`NotaDeGalaxia`), nunca un
archivo. Se pintan (`Galaxia.kt:1058-1096`):

- **Emoji**: el caracter a **34 sp**, con 4 dp de margen. Nada mas.
- **Nota**: el texto a `bodySmall`, color `#2B2600`, hasta 8 lineas, ancho
  maximo **180 dp**, esquinas de **14 dp**, relleno 12x8 dp, sobre uno de
  cinco fondos pastel (`Galaxia.kt:934-936`):
  `#FFF1B8 #FFD6E0 #CDEBFF #D4F5DC #E5DBFF`.

### Las orbitas (`Galaxia.kt:848-855`)

Por cada exoplaneta atado a un sol:

- Un **anillo** del color del sol al alfa **0,10**, grosor `1,2 dp` (minimo 1),
  de radio = el largo de la varilla.
- Una **raya punteada** (`dashPathEffect(10, 10)`) del centro del sol al
  exoplaneta, color del sol al alfa **0,35**.

El PC no pinta nada de esto: sus lunas van en rejilla dentro de la galaxia.

### El iman (`Galaxia.kt:881-889`)

Mientras se arrastra un exoplaneta, el sol que lo va a enganchar dibuja un
circulo punteado de radio `radio + 150 dp` (`GalaxiaModelo.kt:110`) en su color
al alfa 0,6. El PC tiene «aterrizar» (`jerarquia.rs`) pero sin este aviso.

## 5. Las conexiones

Medido en `Galaxia.kt:856-880`. El movil solo tiene **un tipo** de conexion,
pero la pinta con fisica:

- Grosor `2,6 dp · zoom` acotado a `[0,5, 1,5]`, color = la media
  (`lerp(a, b, 0.5)`) de los colores de los dos soles.
- **Tensa** (holgura < 1): una recta con **halo** —la misma recta a 3,5x de
  grosor y alfa 0,2— y encima la linea a alfa 0,9.
- **Floja**: una **curva cuadratica que cuelga** hacia abajo de la pantalla,
  con caida `min(holgura·0.6, 160) · escala`; halo a 3,5x alfa 0,16 y linea a
  0,8x alfa 0,7.

El PC (`pintar.rs:524-585`) tiene **cuatro tipos** (relacion, depende,
referencia, secuencia con numero) en rectas de 1,5-2 px sin halo, en gris
`#8A93A8` o azul. Mas capaz, pero visualmente mucho mas seco.

> Decision: se conservan los cuatro tipos (el escritorio los usa y el movil no
> los tiene) pero se les pone el **halo** y el **extremo redondo** del movil, y
> el color pasa a ser la media de los colores de los dos astros cuando la
> conexion no lleva color propio. La **cuerda que cuelga** no se copia: en el
> PC las conexiones no tienen fisica de cuerdas ni largo en reposo, asi que una
> caida seria decorativa y mentirosa (diria «esta floja» cuando no hay nada
> flojo).

## 6. Fisica, disposicion y gestos

### Disposicion

- **Espiral de Fermat con angulo aureo** en los dos. Movil: `PASO = 170 dp`
  (`GalaxiaModelo.kt:80-95`). PC: `SEPARACION = 6000` del mundo
  (`galaxias.rs:8-22`). **La formula ya es la misma**; solo cambia la escala,
  que es correcta porque los cuerpos del PC son 2000 de radio.
- Movil: el sitio guardado manda, y quien no tiene sitio va al suyo de la
  espiral por su puesto en la lista (`Galaxia.colocar`). El PC hace lo mismo en
  `galaxias::sincronizar`, salvo que una vez colocada la galaxia ya no se
  mueve. **Igual en lo que importa.**
- **Orbita de un exoplaneta nuevo**: `radio + 70 + 26·(k % 3)` dp con el angulo
  aureo menos 90° (`GalaxiaModelo.kt:98-102`).

### Fisica (`FisicaDeGalaxia.kt`)

Verlet con restricciones, 4 vueltas por paso, amortiguacion 0,86; cuerdas
sol↔sol que **solo tiran** (rigidez 0,18) y **ceden** 0,006 por paso mientras
se tira; varillas sol→exoplaneta (rigidez 0,35) que solo mueven al exoplaneta;
choques entre soles con 24 dp de holgura, solo hasta 300 soles. Se duerme sola
cuando nada se mueve mas de 0,04 dp por paso durante 20 fotogramas, y al
dormirse guarda.

> Decision: **no se porta la fisica.** En el PC los astros no son cuerpos
> sueltos que el usuario zarandea: son contenedores anidados con reglas de
> «que cabe dentro de que» (`jerarquia.rs`) y con miles de lunas que llegan
> solas del chat. Un Verlet con 5.000 cuerpos tirando unos de otros pelearia
> con la jerarquia y con el presupuesto de 8 ms de paneo. Lo que **si** se
> porta es su resultado visible: la espiral (ya estaba) y el aspecto de las
> conexiones.

### Gestos

| Movil | PC hoy | Nota |
| --- | --- | --- |
| Un dedo en el vacio: pasear | boton central / espacio + arrastrar (`mapa.rs`) | ya esta |
| Dos dedos: zoom | rueda | ya esta |
| Toque en un sol: **entrar** en su universo | doble clic enfoca | equivalente |
| Mantener en un sol: su menu | clic derecho / inspector | equivalente |
| Pellizcar un sol: **agrandarlo** (0,5x a 3x) | no hay | **imposible tal cual** en escritorio sin pellizco; el equivalente es Ctrl+rueda sobre el astro, o el tamano S/M/L que ya tiene el inspector (`RADIO_PLANETA_S/M/L`) |
| Mantener en el vacio: una nota ahi | herramienta Planeta | equivalente |
| Cuatro modos abajo (Mover, Conectar, Nota, Emoji) con una frase de pista | barra de tres (Planeta, Emoji, Conectar) arriba a la izquierda | falta **Mover** como modo explicito y falta **la frase de pista** |

## 7. Barras, menus y el «+»

### Arriba (movil, `Galaxia.kt:450-477`)

`[←]  Sistema solar / «N proyectos · N notas · N conexiones»   [buscar] [verlo todo]`

El PC tiene arriba la **barra de ruta** «Cosmos > Galaxia > …» con el buscador
a la derecha (`pixpin-ui/src/universo.rs:44-58`). Es mas util en escritorio
(el movil no tiene migas en esta pantalla, solo dentro). Se conserva, pero
**falta la segunda linea de recuento**, que es barata y dice mucho.

### Abajo (movil)

Una **pastilla de pista** («Arrastra soles y notas · pellizca un sol para
agrandarlo · toca un sol para entrar en el») sobre fondo `#77000000` con
esquinas de 12 dp, y debajo la **barra de cristal** con los cuatro modos.

El PC pone su barra de tres herramientas **arriba a la izquierda**
(`BarraUniverso::colocar`, a 16 px del lienzo). **Aqui esta la falla que vio
el usuario**: el rotulo de una galaxia se pinta en
`pintar.rs:280` (`centro.1 - r - 24·e`, encima del disco) y en `pintar.rs:302`
(debajo), y la barra se pinta despues, en `pintar_delante`
(`sesion.rs:1519`) — asi que **la barra tapa el rotulo**. Hay que apartar el
rotulo de los paneles.

### Dentro de un proyecto (movil, `Universo.kt`)

- Arriba: `[←]  migas «Proyecto › Espacio › …» / «Vacio: anade con +»  [chat] [verlo todo]`
- Abajo, sin nada elegido: el boton **«+»** redondo; pulsado, abre la fila de
  chips **Del chat · Hoja · Nota · Rotulo · Figura · Imagen · Emoji**.
- Abajo, con algo elegido: **Abrir/Editar · Espacio/Entrar · Vincular · − · + ·
  Color · Quitar**, en una fila con scroll horizontal, fondo `#CC10163A`,
  esquinas de 50 y borde blanco al 0,25.
- v0.61.1 (`de20b20a`): el «+» no abria la barra porque el gesto del fondo la
  cerraba en el mismo instante; se arreglo con `awaitFirstDown(requireUnconsumed = true)`
  (`Universo.kt:250-255`). Aviso para el PC: cualquier barra flotante nueva
  tiene que consumir su clic antes de que lo vea el lienzo.

El PC no tiene el organizador manual: sus lunas llegan solas del chat, que es
su razon de ser en escritorio (miles de archivos). **No se porta**: seria un
segundo modelo de datos para lo mismo. Lo que si se porta es la **fila de
chips de abajo** como sitio para la pista y los modos.

## 8. Que se hace y que no — resumen

**Se hace (por tandas, de lo que mas se ve a lo que menos):**

1. Paleta Cosmos entera y el cielo: degradado radial + dos nebulosas con
   paralaje + estrellas con las cuentas, los colores y los tamanos del movil.
2. El sol: corona doble, cuerpo en degradado, borde blanco, inicial dentro.
3. Los rotulos: nombre en negrita debajo y «N hojas» en su segunda linea,
   apartados de los paneles (la falla del usuario).
4. Colores de sol del movil con el `hashCode` de Java.
5. Conexiones con halo y extremo redondo, del color medio de los dos astros.
6. Recuento «N proyectos · N archivos · N conexiones» bajo el titulo.
7. Anillo de orbita y raya punteada de la luna a su planeta.

Todo eso esta hecho. Las orbitas se pintan solo de **planetas** (no de
galaxias, que tienen cientos de lunas en rejilla) y solo cuando el planeta se
ve a 40 px o mas y tiene 24 lunas o menos: mas alla de ahi deja de leerse
como un sistema solar y pasa a ser un enjambre de aros.

**No se hace, con su porque:**

- **El paralaje de las nebulosas** (0,6 en `Galaxia.kt`): van horneadas con
  el degradado en un bitmap opaco, asi que se quedan quietas en la pantalla
  —que es como las tiene el propio `FondoCosmico` del tema—. Moverlas
  obligaria a pintarlas aparte, con alfa, y son dos pasadas de pantalla
  entera: medido, el fondo pasaba de 0,5 ms a 4,7 ms.
- **El degradado del cielo en Ligero**: ahi se queda el color liso
  `CosmosBase`. Ligero existe para la grafica integrada y una pasada de
  pantalla entera es justo lo que no se le puede pedir.

- **Portar `galaxia.json`/`universos.json`**: son del aparato y no viajan (§1).
- **La fisica Verlet**: pelea con la jerarquia del PC y con los 8 ms (§6).
- **La cuerda que cuelga**: aqui no hay largo en reposo que la justifique (§5).
- **El pellizco de un sol**: no hay pellizco en escritorio; ya existe el
  tamano S/M/L en el inspector (§6).
- **El organizador manual con subespacios**: duplicaria el modelo de datos
  (§7).
