# Comentarios de las notas Markdown — instrucciones para Android

30-sep-2026. El PC ya comenta notas (H12): se elige un trozo, «Comentar», el trozo
queda resaltado y el comentario sale en un panel a la derecha, a la altura de su
frase; se responde, se edita, se borra y se resuelve. Este documento dice **qué
fichero escribe el PC, con qué forma exacta, cómo se ancla y cómo viaja**, para que
el móvil lo lea y lo escriba igual. Código de referencia en el PC:

- formato y anclaje: `crates/pixpin-docs/src/md_comentarios.rs` (puro, 17 pruebas);
- sitio y viaje: `crates/pixpin-sincro/src/anotado.rs` (`COMENTARIOS`,
  `lleva_anotado`, `hojas_quitadas`), `disco.rs` (`alcance_de`),
  `crates/pixpin-proyecto/src/comentarios_de_notas.rs`, `vista.rs`;
- simulador: `crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs`
  (`los_comentarios_de_una_nota_viajan_con_ella_y_se_van_con_ella`,
  `los_comentarios_de_una_hoja_nota_del_movil_viajan_y_se_van_con_la_hoja`).

## 1. Nada dentro del `.md`

El texto de la nota **no cambia**: ni `<!-- -->`, ni marcas, ni nada. `Markdown.kt`
no tiene que tocarse y una nota con comentarios se lee igual en un móvil viejo.
Los comentarios van en un fichero aparte.

## 2. Dónde está el fichero

Junto a lo anotado de los adjuntos (`AnotacionesDelAdjunto`), con el **código
único** de la nota:

```
files/pins/draw/anot-<uid>.comentarios.json
```

| La nota es…                                   | `<uid>`                                   |
|-----------------------------------------------|-------------------------------------------|
| un mensaje `NOTA` del chat                    | `Codigos.unico(m)` del mensaje            |
| una hoja `nota` de un proyecto                | `Codigos.unico(h)` de la hoja             |
| un `.md` adjunto (`ARCHIVO` con `ruta`)       | `Codigos.unico(m)` de su mensaje          |

(En el PC, un `.md` suelto abierto desde el Explorador lleva
`apuntes.comentarios.json` a su lado; eso no viaja y no le afecta al móvil.)

Cambios en `AnotacionesDelAdjunto.kt`:

```kotlin
private val TERMINACIONES = listOf(".excalidraw.gz", ".marcas", ".espacios", ".maqueta",
    ".voz", ".sitio", ".hoja", ".comentarios.json")
fun comentarios(filesDir: File, uid: String) = File(filesDir, "$CARPETA/$PREFIJO$uid.comentarios.json")
```

`porUid` ya saca el uid hasta el primer `.` o `-`: `anot-ABCDE23456.comentarios.json`
→ `ABCDE23456`. El temporal (`….comentarios.json.tmp`) no acaba en la terminación y
no viaja.

## 3. El formato (JSON, UTF-8)

```json
{
  "version": 1,
  "comentarios": [
    {
      "id": "K7Q2-19a3c2f1e80-0",
      "ancla": {
        "cita": "segundo piso",
        "antes": "# Obra\nLa losa del ",
        "despues": " ya esta hormigonada.\nFalta el c",
        "pos": 19
      },
      "autor": "MAXBOOK",
      "aparato": "K7Q2",
      "cuando": 1790795100000,
      "texto": "¿Seguro que es el segundo?",
      "resuelto": false,
      "respuestas": [
        {
          "id": "MOVI-19a3c3a0b10-0",
          "autor": "Teléfono de Max",
          "aparato": "MOVI",
          "cuando": 1790795700000,
          "editado": 1790795900000,
          "texto": "Sí, el de la escalera."
        }
      ]
    }
  ]
}
```

- `version`: 1. Una mayor se lee igual.
- Hilo: `id`, `ancla`, `autor` (nombre del aparato, para enseñarlo), `aparato`
  (código de aparato, `Codigos`), `cuando` (ms UTC), `texto`, `resuelto`,
  `respuestas`. Opcionales (no se escriben si faltan): `editado` (ms UTC),
  `resueltoPor` (nombre), `resueltoCuando` (ms UTC).
- Respuesta: `id`, `autor`, `aparato`, `cuando`, `texto`, opcional `editado`.
- `id`: `"<aparato>-<cuando en hexadecimal>-<n>"`, con `n` el primero (desde 0)
  que no esté ya en el fichero. Único entre aparatos por el aparato.
- **Lo que no se entienda se conserva**: cualquier campo desconocido (arriba, en
  un hilo, en una respuesta o en el ancla) se vuelve a escribir tal cual. En
  kotlinx: guardar el `JsonObject` original y sobrescribir solo los campos
  conocidos, o `@Serializable` con un mapa de «resto».
- Textos sin blancos al principio ni al final y con `\n` (nunca `\r\n`). Un
  comentario o respuesta vacía no se crea; editar a vacío no se hace.
- **Fichero vacío o que no existe = sin comentarios.** Un fichero que existe y
  **no se entiende** no se toca nunca (se perderían comentarios): se avisa en el
  registro y no se escribe encima.
- Se escribe por temporal + `renameTo` (como `.hoja`).
- Al borrar el último comentario **se escribe `"comentarios": []`**, no se borra
  el fichero: la sincronización no lleva borrados de ficheros, y si se borrase,
  el otro aparato lo devolvería.

## 4. El ancla: una cita, no posiciones

Todas las posiciones y longitudes en **unidades UTF-16** (índices de `String` de
Kotlin, igual que el `RichEdit` del PC), sobre **el Markdown tal como se guarda**
(`\n`).

**Crear** (`ancla_de`): de la selección `[a, b)` se quitan los blancos de los
bordes (y no se parte una pareja sustituta); si no queda nada, no hay ancla.
`cita = texto[a, b)`, `antes = texto[max(0, a-32), a)`, `despues =
texto[b, min(len, b+32))`, `pos = a`. Sin selección, se comenta **la palabra**
bajo el cursor (letras, cifras y `_`).

**Encontrar** (`ubicar`), en este orden:

1. Todas las apariciones exactas de `cita`. Para cada una, «contexto» =
   (letras comunes contando hacia atrás entre lo de delante y `antes`) + (letras
   comunes hacia delante entre lo de detrás y `despues`). Gana la de más
   contexto; empate → la más cercana a `pos`.
2. Si la cita ya no está tal cual (se escribió dentro): se prueba con el
   contexto entero de cada lado y luego con sus 16 y sus 8 letras más pegadas a
   la cita (`antes` = su final, `despues` = su principio), todas las parejas de
   mayor a menor. Para cada pareja: si suman menos de 8 letras, no vale. Cada
   aparición de `antes` marca un inicio (si `antes` está vacío, la cita empezaba
   la nota: inicio 0, y solo si `pos ≤ límite`); el fin es la primera aparición
   de `despues` dentro de los `límite` siguientes (si `despues` está vacío, el
   final del texto, si queda a menos de `límite`). `límite = 2·len(cita) + 64`.
   Lo de en medio tiene que tener algo que no sea blanco. Gana el candidato más
   cercano a `pos`. La primera pareja con candidato decide.
3. Si nada: **sin ancla**. El comentario no se pierde: se enseña arriba del
   panel, aparte, con su cita, y se guarda con el ancla vieja (si el texto vuelve,
   p. ej. con deshacer, se vuelve a encontrar).

**Poner al día** (`reanclar`): al guardar la nota, cada ancla encontrada se
reescribe con lo de ahora (`cita` = el texto encontrado, `antes`/`despues`
recortados de nuevo a 32, `pos`). Así el comentario sigue al texto aunque se
escriba dentro de él.

Casos de prueba que el PC pasa y conviene copiar (`md_comentarios::pruebas`):
cita repetida que se elige por contexto; escribir delante; escribir dentro;
escribir dentro y en el contexto a la vez; cita al principio y al final de la
nota; cita borrada entera (sin ancla y vuelve); contexto corto que **no** debe
anclar (`"a xx b a yy b"` con cita cambiada); emoji en el borde.

## 5. Guardar sin pisar lo que llegó

Con la nota abierta pueden llegar comentarios del otro aparato. Al guardar, el
PC **junta por id** tres versiones: la que leyó al abrir (base), la suya y la que
hay ahora en disco (`fusionar`):

- lo nuevo de cualquiera de los dos lados entra;
- lo que estaba en la base y uno borró se va, salvo que el otro lo cambiara;
- cambiado en un solo lado, gana ese lado; en los dos, gana la versión cuyo
  último cambio (`max(cuando, editado, resueltoCuando, respuestas)`) es más nuevo,
  y sus respuestas se juntan con la misma regla.

En la sincronización el fichero viaja entero como cualquier anotado (gana el más
nuevo, igual que `.marcas`); juntar es solo al guardar desde el editor.

## 6. Cómo viaja y cómo se borra (cambios en `Disco.kt` y compañía)

1. **`alcance`**: hoy pone lo anotado solo si `m.ruta != null`. Añadir las notas:
   ```kotlin
   if ((m.ruta != null || m.clase == Clase.NOTA) && anotado.isNotEmpty())
       anotado[Codigos.unico(m)]?.forEach { poner(it, etiqueta) }
   ```
   y dentro del bucle de hojas del proyecto:
   ```kotlin
   anotado[Codigos.unico(h)]?.forEach { poner(it, etiqueta) }
   ```
2. **Borrar un mensaje** (`aplicarMensajes`, `anotadosFuera`) y
   `MensajesStore.borrarAdjunto(m)`: borrar lo anotado también si
   `m.clase == Clase.NOTA` (hoy solo con `ruta`).
3. **Quitar una hoja** (`Proyectos.sinHoja`, quien lo llame) y **al guardar un
   proyecto que llega** (`Disco.guardarProyecto`): los `Codigos.unico(h)` de las
   hojas que estaban antes y ya no están → `AnotacionesDelAdjunto.todoDe(files,
   uid).forEach { it.delete() }`. Ojo: el móvil tiene que quitar la hoja con su
   marca `quitadas` (como ya hace `sinHoja`); sin la marca la mezcla la devuelve.
4. Nada más: `permitida` ya admite `pins/`, y el PC lee y escribe esos ficheros en
   su carpeta `android/pins/draw/` del chat.

## 7. Lo que se ve en el PC (para copiarlo si se quiere)

- Comentar: botón «globo con +» de la barra, clic derecho → «Comentar», o
  Ctrl+Alt+M. Sin selección, la palabra del cursor.
- El trozo comentado con fondo ámbar (más fuerte el del comentario elegido). Los
  resueltos no se resaltan.
- Panel a la derecha (300 px): «Comentarios», filtro de resueltos y cerrar; «No hay
  comentarios» y la pista si está vacío; los sin ancla arriba; cada tarjeta a la
  altura de su frase, sin montarse, y la elegida justo en su frase. Tarjeta:
  redondel con la inicial del autor (color fijo por aparato), autor, fecha
  («30 sept, 14:05», con año si no es este, «· editado»), la cita en una línea,
  el texto y las respuestas; en la elegida, resolver (✓), «⋯» (editar, borrar) y
  «Responder…». Intro envía, Mayús+Intro parte el renglón, Esc cancela.
- Un clic en la tarjeta lleva al texto; el cursor dentro de un texto comentado
  elige su comentario (y un clic abre el panel si estaba cerrado).
- El globo de la cabecera abre y cierra el panel, con el número de abiertos. Una
  nota con comentarios abiertos se abre con el panel abierto.
