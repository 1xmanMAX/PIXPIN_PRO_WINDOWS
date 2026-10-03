# Tareas con fecha de creacion (PC y Android)

2-oct-2026. Lo pidio el usuario: «que capture la fecha de creacion y que no
te muestre esta sino que me diga directamente el numero de dias desde que se
creo».

## Lo que hay hoy en Android (leido con `gh api`, rama por defecto)

`mini/Tareas.kt` no guarda ninguna fecha: el modelo es
`data class Tarea(val texto: String, val hecha: Boolean = false)`.

- **Lee** con `^\s*[-*+]\s+\[([ xX])]\s?(.*)$` y se queda el texto **crudo**,
  recortado (`groupValues[2].trim()`).
- **Escribe** el documento entero desde el modelo (`Tareas.escribir`):
  `Cabecera.linea(titulo)` + una linea `- [ ] texto` por tarea. Lo que no sea
  una casilla (parrafos, comentarios, lineas sueltas) **se pierde** al primer
  toque. Esta escrito a proposito en su KDoc.
- **Pinta** el texto con un `Text()` a pelo (`MiniActivity.kt`, `DeTareas`):
  casilla, texto tachado si esta hecha y aspa de borrar. Sin Markdown.

Consecuencia: cualquier dato que vaya **fuera** del texto de la tarea (una
linea aparte, un comentario HTML en su propia linea, un bloque al final) lo
borra el movil al marcar una casilla. Lo unico que sobrevive el viaje
movil-PC-movil es lo que va **dentro del texto** de cada casilla.

## El formato

La fecha va al final del texto de la tarea, con la marca de «creada» del
plugin Tasks de Obsidian (`➕`, U+2795, un espacio y la fecha ISO):

```
# La compra

- [x] pan ➕ 2026-09-28
- [ ] leche ➕ 2026-10-02
- [ ] sal
```

Reglas (las de `pixpin_proyecto::mini::partir`):

1. Solo cuenta **al final** del texto: `➕`, blancos opcionales, y
   `AAAA-MM-DD` como ultima cosa de la linea.
2. Delante de `➕` tiene que haber un blanco o nada (`pan➕ 2026-10-02` no es
   la marca).
3. La fecha tiene que existir (`2026-02-30` se queda como texto).
4. Lo que no cumple eso **es texto** y se ensena tal cual.
5. Es la fecha **local** del aparato que crea la tarea, sin hora.
6. Una tarea sin marca (todas las de antes) **no tiene fecha** y no ensena
   nada. No se usa la fecha del mensaje como aproximacion: la lista se crea
   una vez y las tareas se le van anadiendo durante semanas, asi que «hace 40
   dias» para una tarea de ayer seria peor que no decir nada.

### Por que este y no otro

| Opcion | Sobrevive al movil de hoy | Lo que ve un movil viejo |
|---|---|---|
| Linea aparte / bloque al final | **No**: `escribir` la tira | — |
| `<!-- 2026-10-02 -->` dentro del texto | Si | `pan <!-- 2026-10-02 -->` (ruido) |
| `pan ➕ 2026-10-02` (Obsidian Tasks) | Si | `pan ➕ 2026-10-02` (se entiende) |

Se elige el de Obsidian porque sobrevive igual que el comentario, se lee
mejor mientras el movil no se actualice, y no es un invento nuestro: la misma
lista abierta en Obsidian con el plugin Tasks sabe que es una fecha de
creacion. **Lo que se paga:** hasta que Android se actualice, en el telefono
se ve la fecha escrita detras de cada tarea creada en el PC.

### Compatibilidad, comprobada contra el Kotlin de hoy

- `Tareas.leer`: la casilla sigue casando; la fecha queda dentro de `texto`.
- `Tareas.escribir`: la reescribe igual (`enUnaLinea` no la toca).
- `alternar`, `marcar`, `borrar`, `mover`, `sinLasHechas`: copian la `Tarea`
  entera, la fecha va con ella.
- `resumenDe` («3 de 7»): no mira el texto, cuenta igual.
- `renombrar`: el movil de hoy no lo ofrece en pantalla. Si un dia se
  ofrece sin saber de fechas, el texto nuevo la quitaria (se perderia la
  fecha, no la tarea). Por eso el punto 3 de abajo.

En el PC la ida y vuelta esta probada en
`crates/pixpin-proyecto/src/mini.rs`
(`una_tarea_nueva_lleva_su_fecha_y_el_movil_la_conserva`,
`un_documento_de_antes_se_lee_y_se_escribe_igual`).

## Lo que ensena el PC

Nunca la fecha. A la derecha de cada tarea con fecha, en gris y pequeno:

- `hoy` — creada hoy (o con fecha del futuro: el reloj del otro aparato iba
  adelantado).
- `hace 1 día`
- `hace N días` — siempre dias, sin pasar a semanas ni meses (el usuario
  pidio el numero de dias, y Android no tiene nada que copiar aqui).

Los dias se cuentan por **calendario** en el huso del que mira: creada ayer a
las 23:00, a las 01:00 de hoy es «hace 1 día». Claves `mini-tarea-edad` en
`crates/pixpin-store/i18n/*/main.ftl`.

## Lo que tiene que hacer Android

1. **Leer.** En `Tareas.kt`, una funcion pura igual que `mini::partir`:

   ```kotlin
   private val CREADA = Regex("""(?:^|\s)➕\s*(\d{4}-\d{2}-\d{2})\s*$""")

   /** El texto que se ensena y la fecha de creacion, si la lleva. */
   fun partir(texto: String): Pair<String, LocalDate?> {
       val m = CREADA.find(texto) ?: return texto.trim() to null
       val fecha = runCatching { LocalDate.parse(m.groupValues[1]) }.getOrNull()
           ?: return texto.trim() to null          // 2026-02-30: es texto
       return texto.substring(0, m.range.first).trim() to fecha
   }
   ```

   `Tarea` se queda como esta (`texto` crudo, con la fecha dentro): asi
   marcar, mover y borrar no cambian nada.

2. **Escribir al anadir.** En `Tareas.anadir`, detras del texto saneado:
   `"$limpio ➕ ${LocalDate.now()}"` (fecha local, sin hora). Si el texto
   pegado ya traia su `➕ fecha` al final, se respeta la suya.

3. **No perderla al corregir.** Si algun dia `renombrar` sale en pantalla, el
   campo ensena `partir(texto).first` y al guardar se vuelve a pegar la fecha
   vieja (`mini::renombrar` del PC hace eso).

4. **Ensenar los dias.** En `MiniActivity.DeTareas`, el `Text` de la tarea
   ensena `partir(t.texto).first`, y a su derecha (antes del aspa) un `Text`
   pequeno en `onSurfaceVariant` con:

   ```kotlin
   val dias = ChronoUnit.DAYS.between(fecha, LocalDate.now()).coerceAtLeast(0)
   ```

   y los textos nuevos (`values/strings.xml` y `values-en/`):

   ```xml
   <string name="tarea_creada_hoy">hoy</string>
   <plurals name="tarea_creada_hace">
       <item quantity="one">hace %d día</item>
       <item quantity="other">hace %d días</item>
   </plurals>
   ```

   Sin fecha, nada.

5. **Lo demas que lea el texto de una tarea** (el editor de notas si abre la
   lista, los avisos, la busqueda) deberia ensenar `partir(..).first` en vez
   del texto crudo. No es urgente: lo crudo se entiende.

6. **Una prueba** en `MiniAppsTest.kt` con el mismo documento que la del PC:
   `- [ ] pan ➕ 2026-10-02` se lee como «pan» + 2-oct, `escribir(leer(d))`
   devuelve `d` letra por letra, y `pan ➕ 2026-02-30` se queda como texto.

## Lo que el PC anadio a la pantalla de tareas (no esta en Android)

Android solo tiene casilla, texto tachado, aspa y la linea de anadir. En el
PC, ademas, por peticion del usuario («mucho mejor dentro del chat, con mas
opciones»):

- la linea de avance «3 de 7» con su barra (la misma cuenta que la burbuja);
- «Ocultar las hechas» / «Ver las hechas» (solo en pantalla, no se guarda ni
  viaja);
- lapiz para corregir, y subir/bajar en la tarea elegida (las dos cosas ya
  estaban con F2 y Alt+flechas);
- la casilla es lo unico que tacha, como el `Checkbox` del movil; tocar el
  resto de la fila la elige, y tocarla otra vez la corrige.

Si Android las quiere, son pantalla y nada mas: el documento no cambia.

## La lista sacada a la pantalla (PC)

Menu del mensaje de una lista → «Sacar a la pantalla»: sale un pin con la
misma lista, viva. Lo que se tacha o se anade en el pin se guarda en el
mensaje con el cerrojo del cuaderno (`cuaderno::cambiar`: leer, cumplir el
toque sobre lo que diga el mensaje en ese momento y reescribir, sin soltar el
cerrojo) y el chat relee. Android no tiene nada igual: su pin de lista
(`PinType.CHECKLIST`, `OverlayManager.kt`) solo nace de la palabra magica y no
sabe de mensajes.

## Otros lectores de tareas en el PC

- `pedidos.rs` (`anadir_tarea` del lanzador) sigue con `mini::anadir`, sin
  fecha; para fecharlas basta con `mini::anadir_el(.., hoy)`.
- `apps/pixpin-lanzador/src/datos.rs` tiene su propio lector de casillas: hoy
  ensenaria `pan ➕ 2026-10-02`. Deberia pasar el texto por la misma regla de
  `mini::partir` (y puede ensenar la edad igual).
