# Revisión de la tanda 2 (`2b7ca5f`..`4b66d03`)

**Fecha:** 2026-09-20 · **Rama:** `pin-en-vivo` · **Revisor:** agente que no escribe código de
producto. Revisión anterior: `docs/investigacion/2026-09-19-revision-tanda-1.md`.

Unos 50 commits de seis zonas: las 32 herramientas del lienzo (tintas con material y grano,
mosaico, foco, rombo, arco, puntas de flecha, bote de relleno, recortar, extender, puntos,
ángulos, lazo, imán, enganche flecha-figura, copiar estilo, marco y papel, lupa, serie, goma,
biblioteca), las 7 mini-apps y sus paneles, el audio (`pixpin-audio`: sonar y grabar), los
recordatorios, aligerar PDF, el visor de documentos (`pixpin-docs`) y los enganches de todo ello.

## Cómo se ha revisado

El árbol de trabajo cambia mientras se revisa (otros dos agentes commitean en `ventana_chat.rs`,
`sincronizar*`, `pixpin-ui`, `main.rs` y en el crate nuevo `crates/pixpin-voz`), así que **todo se
ha leído sobre una copia limpia de `HEAD`** (`git archive HEAD`) en el borrador de la sesión. Las
líneas que se citan son las de esa copia.

El Kotlin de referencia se ha descargado de `1xmanMAX/PIXPIN_PRO_ANDROID` (HEAD de hoy) con
`gh api`, **no** del clon local `F:\THE FORGE\PIXPIN ANDROID`, que está viejo.

No se ha ejecutado la aplicación ni se ha sintetizado entrada.

## La puerta común

Sobre la copia limpia de `HEAD`, la puerta sale **verde**:

| Comprobación | Resultado |
|---|---|
| `cargo fmt --all --check` | limpio |
| `cargo clippy --workspace --all-targets -- -D warnings` | limpio |
| `cargo test --workspace --no-fail-fast -- --test-threads=1` | **2.310 pasan**, 87 ignoradas, 1 falla: `la_segunda_adquisicion_falla_mientras_viva_la_primera` (la app está abierta: no cuenta) |

A diferencia de la tanda 1, el árbol compiló en todo momento: ningún crate a medias tumbó
`cargo metadata`. Los dos agentes que trabajan en paralelo no han roto la puerta.

**Las 14 pruebas `#[ignore]` nuevas se han repasado una a una y las 14 son legítimas**: 9 piden GPU
y sesión de escritorio, 2 micrófono real, 2 son bancos de medida declarados como tales
(`ventana_editor/medir.rs:714,732`) y 1 captura lo que compone DWM. **Ninguna tapa trabajo sin
hacer.** Tampoco hay una sola tautología (`assert!(true)`, `assert_eq!(a, a)`) en el árbol, ni
`#[allow(dead_code)]` puesto «de momento»: sólo quedan dos, y los dos están justificados
(`tests/oraculo_tinta.rs:19`, `iconos_excalidraw.rs:8`). Las 579 claves de i18n están todas
referenciadas: no hay cadenas huérfanas.

---

## 1. Compatibilidad con el móvil

Antes de nada, **una corrección de la premisa con la que se encargó esta revisión**: el lienzo
**no** usa `encodeDefaults = false`. `Scene.kt:27-31` es `encodeDefaults = true`,
`explicitNulls = false`, `ignoreUnknownKeys = true`. Que el PC escriba un campo con su valor por
omisión no ensucia nada; lo que sí rompería el fichero entero es un `null` explícito sobre un campo
no nulable, porque no hay `coerceInputValues`. Se han repasado todos los `Value::Null` que escribe
el PC (`roundness`, `containerId`, `startBinding`, `endBinding`, `fixedPoint`, `papel`, `arcSweep`,
`startArrowhead`/`endArrowhead`, `strokeOptions`, `escala`) y **todos caen en campos nulables**. El
chat sí es `encodeDefaults = true` en `MensajesStore.kt:47`, y `kotlin.rs:84-117` lo reconstruye
campo a campo en el orden de la `data class`: correcto.

**La regla de oro está bien implementada.** `excalidraw::escribir` (`excalidraw.rs:175-179`)
devuelve el `original` tal cual si la ida y vuelta coincide, y `elemento_hacia` (`:990-1001`) parte
del mapa original en vez de construir de cero. No se encontró ninguna ruta nueva que re-serialice
un elemento ajeno perdiendo campos.

Dicho eso, hay dos roturas de compatibilidad graves y varias menores.

### CRÍTICO 1 — el arco: el móvil guarda radianes, el PC lee y escribe grados

`crates/pixpin-motor2d/src/excalidraw.rs:687-688` (lectura) y `:1193-1200` (escritura):

```rust
"pixpin-arc" => Figura::Arco {
    inicio: num_o(v, "arcStart", 0.0).to_radians(),
    barrido: num(v, "arcSweep").map(f32::to_radians),
},
…
mapa.insert("arcStart".into(), Value::from(inicio.to_degrees() as f64));
```

`Element.kt:704-712` lo dice con todas las letras —«Dónde empieza el arco y cuánto barre, **en
radianes**»— y `Arco.kt:99-105,135-138` usa los dos valores crudos contra π.

**Reproducción:** un arco de media vuelta hecho en el móvil llega con `arcSweep ≈ 3.1416`; el PC lo
lee como 3,14 **grados** y pinta casi un punto. Al revés, un arco de 120° dibujado en el PC se
escribe `120.0`, el móvil lo toma por 120 radianes (≈ 19 vueltas), `abs(barrido) >= 2π` y lo pinta
como la elipse entera.

*Arreglo:* quitar las cuatro conversiones (leer y escribir el número tal cual).
**Ojo:** la prueba consagra el error. `tests/los_veintitres_tipos_del_movil.rs:454-456` afirma
`assert!((inicio - 30f32.to_radians()).abs() < 1e-5)` con el comentario «el móvil los guarda en
grados». Hay que invertir esos dos `assert` y ese comentario, o el arreglo se revierte solo.

### CRÍTICO 2 — el punto etiquetado: mismo fallo con `etiquetaAngulo`

`excalidraw.rs:711` (lectura) y `:1230` (escritura) hacen el mismo `to_radians()`/`to_degrees()`.
`Element.kt:714-723` también dice «en radianes», y `Puntos.kt:237-239` mete el valor directo en
`cos`/`sin` y lo produce con `atan2`.

**Reproducción:** un punto con la letra arriba-izquierda en el móvil (≈ 2,36 rad) sale en el PC a
2,36°, pegado a la derecha. Al arrastrarla, el PC escribe `135.0` y el móvil calcula `cos(135 rad)`:
la letra aterriza en un sitio arbitrario.

*Arreglo:* borrar el `.to_radians()` de la línea 711 y el `.to_degrees()` de la 1230.

### CRÍTICO 3 — faltan monedas sin decimales: los importes se multiplican por cien en cada vuelta

`crates/pixpin-proyecto/src/mini/gastos.rs:76-78`. `SIN_DECIMALES` tiene 12 monedas;
`Currency.getDefaultFractionDigits()` —lo que usa `Gastos.kt:93,149,281`— devuelve 0 también para
**XOF, XAF, XPF, UGX y UYI**.

**Reproducción:** el móvil escribe `| Taxi | 2500 |` en XOF (2.500 francos). El PC lo lee con
`decimales = 2` → 250.000, enseña «2.500,00» y al tocar cualquier botón reescribe `| Taxi | 2500.00 |`.
El móvil relee eso con `d = 0`, toma el punto por separador de millares y el gasto queda en
**250.000 francos**. Se multiplica por cien en cada vuelta y nadie ve un error.

*Arreglo:* añadir `"XAF","XOF","XPF","UGX","UYI"` a `SIN_DECIMALES` (y `CLF`/`UYW` a una lista de 4
decimales, que Java también reconoce).

### ALTO 4 — a `MaterialTinta` le falta `cuadritos`: el lápiz del móvil se pinta liso

`crates/pixpin-motor2d/src/tinta/material.rs:123-132` y `:137-150`. `Element.kt:487` añadió
(fechado 20-sep-2026) `@SerialName("cuadritos") CUADRITOS` como 11.º material; el PC conoce 10, así
que `desde_palabra("cuadritos")` → `None` → `unwrap_or_default()` → `Lisa`. No se pierde en el
fichero mientras no se toque el material (lo protege `material_ajeno`, `excalidraw.rs:1036-1045`),
pero se pinta mal y desde el PC no se puede volver a elegir.

*Arreglo:* añadir `MaterialTinta::Cuadritos` a `MATERIALES`, `palabra` y `desde_palabra`; de mínimo,
pintarlo como `Trama`.

### ALTO 5 — la línea `**Total: …**` de gastos no es el mismo texto en los dos aparatos

`mini/gastos.rs:356-383` (`texto_de_importe`) compone el total a mano, siempre a la española y con
**espacio normal** antes del símbolo (`:380`). `Gastos.kt:276-288` usa
`NumberFormat.getCurrencyInstance(locale)`, que en `es-ES` (CLDR) pone **U+00A0**.

El comentario de `gastos.rs:352-355` justifica la diferencia con «da igual, porque al leer se
ignora». Vale para el parseo, **pero no para la sincronización**: `Disco.resumenDe` → `textoDeBase`
(`Disco.kt:269-272`, portado en `kotlin.rs:177-190`) hashea el mensaje entero, y el documento va en
`texto`. Esa línea sí cuenta en el resumen, así que el documento va y viene marcado como cambiado
sin que nadie lo haya cambiado.

*Arreglo:* `salida.push('\u{00A0}')` en `gastos.rs:380` cuadra el caso `es-ES`, que es el del
usuario. El caso general (un móvil en inglés pone `€1,234.56`) no tiene arreglo por este lado.
**No confirmado ejecutando Java**: conviene comprobarlo con un `NumberFormat` real antes de tocar.

### MEDIO 6 — `boundElements[].type` vacío revienta la hoja entera en Kotlin

`excalidraw.rs:818-822` lee `tipo` con `unwrap_or_default()` y `:945-958` lo escribe tal cual.
`BoundElement(val id: String, val type: ElementType)` (`Element.kt:541`) es un **enum**: sin
`coerceInputValues`, un `""` lanza `SerializationException` y `ExcalidrawStore.cargar` devuelve
`null`, o sea **se pierde la hoja entera**, no sólo ese elemento.

**Reproducción:** un `.excalidraw` con `"boundElements":[{"id":"x"}]`, abrirlo en el PC, mover algo,
guardar, abrir en el móvil → la hoja no abre.

*Arreglo:* en `extras_desde`, descartar el atado si falta el `type` (`a.get("type")?.as_str()?`),
igual que ya se hace con `id`.

### MEDIO 7 — el foco del móvil siempre entra como elipse

`excalidraw.rs:598-602` lee `forma` con `as_str()`, pero `Element.kt:951` es `val forma: List<Pt>?`.
Esa rama es muerta y todo queda en `lupaRedonda`, que Kotlin escribe en **todos** los elementos con
su valor por omisión `true` (`Element.kt:927` + `encodeDefaults=true`). Resultado: todo foco venido
del móvil se dibuja como elipse, tenga la forma que tenga. No hay pérdida de datos (el PC nunca
escribe esos campos para un `Foco`), pero se pinta mal. La muestra de prueba
(`los_veintitres_tipos_del_movil.rs:174`) consagra el error metiendo `"forma": "elipse"`.

### MEDIO 8 — falta el port de `Tareas.saneado`

`crates/pixpin-proyecto/src/mini.rs:262-273` (`anadir`) sólo llama a `en_una_linea`;
`Tareas.kt:102-106` pasa por `saneado` (`:199-209`), que quita los marcadores de lista de delante.
Pegar `- [x] comprar pan` guarda en el PC `- [ ] - [x] comprar pan` y en el móvil `- [ ] comprar pan`:
el mismo gesto da dos documentos distintos.

### MEDIO 9 — una clave de una versión futura vuelve en minúsculas

`mini.rs:312-328` baja la clave con `to_lowercase` y `:362-369` la reescribe así. Un
`- ritmoBPM: 3` escrito por un móvil más nuevo vuelve como `- ritmobpm: 3`. El móvil lo sigue
leyendo, pero el texto cambia → otro resumen → otra vuelta de sincronización. La idea de conservar
lo desconocido es buena (y mejor que Android, que lo tira); sólo falta guardar la clave original.

### MEDIO 10 — `clase` nula escribiría `"clase":null`, que kotlinx no sabe leer

`crates/pixpin-proyecto/src/cuaderno.rs:112`: `Option<Clase>` con `#[serde(default)]` y sin
`skip_serializing_if`. En Kotlin `val clase: Clase` (`Mensajes.kt:119`) no tiene valor por omisión:
un `null` lanza `MissingFieldException` y `MensajesStore.leer` (`:59-63`) hace `.getOrNull()`, o sea
**la línea entera desaparece sin avisar**. Hoy es latente (los tres constructores ponen clase), pero
rompe si alguien construye un `Mensaje` con `..Default::default()`.

### BAJO 11 — varias, en una lista

- `cuaderno.rs:123-124,145`: `duracion_ms` y `numero` son `i64` donde Kotlin tiene `Int`; fuera de
  rango, kotlinx tira la línea. `duracionMs` es inalcanzable (24 días de audio); `numero` no tanto.
- `mini.rs:132-140` (`titulo()`) acepta más de seis almohadillas y no acepta tabulador;
  `Cabecera.titulo` (`MiniApps.kt:227`) es `^#{1,6}\s+`.
- `mini/tiempos.rs:313-323` lee la alarma con más tolerancia que el móvil (`trim()` y `i64` acotado
  frente a `toIntOrNull()`): `- hora: 7 : 05` da 7:05 aquí y 8:00 allí.
- `gastos.rs:414,419,423` usa `is_ascii_digit`; `Long.parseLong` acepta dígitos árabes o devanagari.
- `excalidraw.rs:711-712`: los valores por omisión del punto son `0.0` y `14.0`; en `Puntos.kt:237-238`
  son `-PI/4` y `22.0`.
- `excalidraw.rs:660-665`: un `fontFamily` desconocido se aplana a 5 y **se reescribe** como 5
  (`:1162-1165`). Hoy da igual porque Kotlin hace lo mismo; rompería contra un móvil más nuevo.
- Tres campos de la muestra `los_veintitres_tipos_del_movil.rs` no son lo que escribe Kotlin:
  `:182 "guia": "flecha"` (Kotlin serializa `"FLECHA"`, sin `@SerialName`), `:212 "unidad": "cm"`
  (`Element.kt:735` es `Double?`) y `:174 "forma": "elipse"` (es `List<Pt>?`). De ahí salió el
  hallazgo 7.
- `crates/pixpin-proyecto/src/tabla.rs:11-14,38-55`: la hoja de cálculo viaja como JSON crudo en
  `texto`, así que en la burbuja del móvil sale `{"celdas":{…}}` y `Mensajes.buscar` encuentra todas
  las tablas al buscar «celdas». No rompe nada, pero contradice la promesa de `mini.rs:14-16`.

### Comprobado y correcto (para no revisarlo dos veces)

`recuerdaEn` en UTC, entero y fuera del hash; `duracionMs` entero; `picos` como enteros crudos
0..32767 con tope 256 y uno cada 50 ms, copiados línea por línea de `Onda.kt`; los documentos de
**las 7 mini-apps** con su texto de oro, incluidos `activa: sí` con tilde, `finEn` al escribir y
`finen` al leer, y la hora sin rellenar; los campos desconocidos conservados en tres capas
(`#[serde(flatten)] resto`, `normalizar` y `otras_claves`); los 17 strings de `type`, las 8 puntas,
las 5 de `EstiloRelleno`, `orbit`/`inside`, `TamanoPapel`, `PautaHoja` y `roundness` como
`{"type":3}`; el `.m4a` MPEG-4 + AAC-LC que abre `MediaPlayer`; los puntos del lienzo como
`{x,y}` en `Double` (una lista `[x,y]` rompería el `Pt` de Kotlin).

---

## 2. Pérdida de datos

Lo bueno primero: `cuaderno::reemplazar`, `Indice::guardar`, `motor2d/formato.rs`,
`universo/formato.rs`, `disco::escribir_atomico`, `envio.rs` (con su `.parte`) y
`guardar_hoja_dibujada` hacen temporal + rename. Borrar proyectos va a `papelera/` por rename.
`mini.rs` y `mini/*.rs` no tocan el disco. **No hay un solo `unwrap`/`expect`/`panic!` ni índice sin
comprobar en código no-test de `protocolo.rs`, `envio.rs`, `disco.rs`, `copias.rs`, `aligerar.rs`,
`mini.rs` ni `mini/*.rs`.** Lo que sigue es lo que se sale de esa disciplina.

### CRÍTICO 12 — un fallo pasajero al leer `indice.json` borra la lista entera de proyectos

`crates/pixpin-proyecto/src/almacen.rs:205-210`:

```rust
pub fn leer(raiz: &Path) -> Indice {
    std::fs::read_to_string(ruta(raiz)).ok()          // cualquier error = "no hay nada"
        .and_then(|t| serde_json::from_str(&t).ok())  // JSON roto = "no hay nada"
        .unwrap_or_default()
}
```

El comentario dice «es una lista, no los datos», pero esa lista **se reescribe entera** poco
después, y el primer camino corre al abrir el chat: `ventana_chat.rs:571` → `asegurar_guardados` →
`Indice::leer` (`almacen.rs:804`) → `indice.guardar` (`:825`). También `:396-398`, `:551+570`,
`ventana_chat.rs:5285-5288` y `recibir.rs:716-719`.

**Reproducción:** dejar `proyectos/indice.json` ilegible (antivirus, OneDrive, otra instancia, o
basura dentro) y abrir el chat → la lista sale vacía y se persiste el vacío. Las carpetas siguen en
`proyectos/`, pero desde la interfaz no hay forma de volver a verlas.

*Arreglo:* que `leer` devuelva `io::Result<Indice>` tratando sólo `NotFound` como vacío, y que
aparte un JSON ilegible a `indice.json.roto-{ms}` — que es exactamente lo que ya hace bien
`universo/formato.rs:32-60`.

### CRÍTICO 13 — recibir del móvil puede sustituir la conversación entera por lo que llega

`almacen.rs:581`, dentro de `fundir_cuaderno`:

```rust
let mias = std::fs::read_to_string(fichero).unwrap_or_default();
```

Si el `guardados.jsonl` de aquí no se puede leer (bloqueado por otro proceso en Windows, o con bytes
no-UTF8), `mias` queda vacío, la fusión sale con **sólo las líneas que llegan** y se escribe encima
(`:517`). Contradice de frente el contrato escrito tres líneas más arriba (`almacen.rs:487-490`):
«Recibir no quita nada… El usuario perdió lienzos justamente por lo contrario.»

*Arreglo:* tratar sólo `NotFound` como vacío y propagar cualquier otro error en vez de escribir.

### CRÍTICO 14 — `guardados.jsonl` se reescribe en el sitio, sin `.tmp` ni rename

`almacen.rs:515-518`: `std::fs::write(destino.join("guardados.jsonl"), fundido)?`. Es el **único**
sitio del repo donde el cuaderno del usuario se escribe directo sobre el destino; `cuaderno::reemplazar`
(`cuaderno.rs:335-337`), `quitar_del_cuaderno` (`ventana_chat.rs:10137-10139`) y
`vista::escribir_lineas` (`vista.rs:199-203`) hacen temporal + rename para ese mismo fichero.
`fs::write` trunca a cero primero: un corte a mitad deja el cuaderno truncado, y lo que se estaba
escribiendo era la fusión de todos los mensajes locales más los que llegan.

*Arreglo:* `pixpin_sincro::disco::escribir_atomico(...)` — ya existe y el crate ya depende de
`pixpin_sincro` (`almacen.rs:874`).

### ALTO 15 — `aligerar`: los objetos que el lector no entiende se tiran, y la red no lo detecta

`crates/pixpin-pdf/src/aligerar.rs:606-632`, `:953-1028`, `:349-358`.

Lo que está bien: los objetos no tocados se copian **byte a byte** (`:979`), así que el texto y los
vectores de los objetos reconocidos se conservan intactos; y si el resultado no gana al menos un 15 %
no se escribe nada (`:342`). El cifrado se rechaza (`Estorbo::Cifrado`, `:230`).

Lo que falla: `objetos()` es un escaneo de bytes, y cuando `cuerpo_del_objeto` devuelve `None`
(`:624`) el objeto **no entra en la lista** — y `reescribir` sólo escribe lo que hay en la lista.
Un objeto que el analizador no sepa parsear **desaparece del fichero de salida sin que nada lo
cuente**: un `endobj` que falta, un `/Length` que no cuadra sin `endstream` literal (`:696`), un
diccionario con una construcción que `fin_del_valor` no cubre (`:498`). Fuentes incrustadas,
anotaciones, marcadores y campos de formulario son objetos ordinarios.

Y la verificación previa a sustituir es **sólo estructural**: `:353` hace
`Documento::abrir(&temporal).is_ok_and(|d| d.paginas() == paginas_antes)`, y `Documento::abrir`
(`pixpin-pdf/src/lib.rs:131-161`) es `LoadFromFileAsync` + `PageCount()`: **no dibuja ni una
página**. Un PDF que perdió una fuente carga igual y declara las mismas páginas → pasa → sustituye.

Además, las **firmas digitales** no se comprueban: una `/ByteRange` apunta a desplazamientos del
fichero viejo, así que reescribir la invalida en silencio.

*Arreglo:* (a) que `objetos()` cuente los tramos que no supo parsear y `inspeccionar` añada un
`Estorbo` en vez de seguir; (b) que la verificación renderice al menos la primera y la última página.

### ALTO 16 — el PDF original se sustituye sin copia ninguna

`apps/pixpin/src/ventana_chat.rs:7246-7254`: el temporal se renombra encima del adjunto del usuario
y **no queda copia**. La única red es el 15, que acabamos de ver que es estructural.

*Arreglo:* antes del `rename`, mover el original a `papelera(raiz)` (ya existe, `almacen.rs:830`)
con la hora en el nombre.

### ALTO 17 — la nota de voz se graba directamente sobre el fichero final

`crates/pixpin-audio/src/entrada.rs:581`: `MFCreateSinkWriterFromURL` recibe el destino definitivo.
Media Foundation lo crea/trunca al abrirlo y sólo escribe el índice MPEG-4 en `Finalize` (`:317`).
Si el proceso muere a mitad queda un `.m4a` **ilegible con nombre bueno**; si en esa ruta había algo,
se truncó al pulsar grabar; y si `preparar` falla después de `abrir_escritor` (p. ej. `cliente.Start()`,
`:397`) no llega a existir ninguna `Grabadora`, así que el `Drop` (`:203`) no corre y queda huérfano.
Es justo lo que `envio.rs:540-541` sí hace bien con `.parte`.

Relacionado: `entrada.rs:175,187,203` hacen `let _ = std::fs::remove_file(...)` sobre la ruta que dio
quien llama, sin papelera y con el error tragado. El del `Drop` es el peligroso.

*Arreglo:* grabar a `<destino>.parte` y renombrar tras un `Finalize` correcto. Eso además deja
inofensivos los tres `remove_file`.

### ALTO 18 — `actualizar_paquete`: un `proyecto.json` ilegible parte la conversación en dos

`almacen.rs:531-545`: `if let Ok(texto) = … && let Ok(antes) = …` **sin rama else**. Si el fichero
existe pero no parsea, la rama se salta en silencio y (1) las hojas que sólo tenía este equipo se
pierden, (2) `puesto.id` se queda con el id del otro aparato —que es justo lo que el comentario de
`:527-529` dice que «partiría en dos la conversación»— y (3) `archivado` se pierde.

Además, aunque parsee, **`antes.resto` nunca se usa**: `Proyecto` tiene `#[serde(flatten)] resto`
(`lib.rs:147-149`) pero sólo se rescatan `id`, `hojas` y `archivado`. Todo lo que un móvil más nuevo
hubiera escrito ahí se borra al recibir. Y ese `proyecto.json` se escribe sin `.tmp` (`:546`).

### ALTO 19 — la copia de seguridad previa a «actualizar el que tengo» se salta sin avisar

`apps/pixpin/src/recibir.rs:537-544`: si `chat_de_ficha` devuelve `None`, `copia_antes_de_tocar`
devuelve `Ok(())` y quien llama lo toma por «copia hecha». `chat_de_ficha` (`vista.rs:313-318`) sale
de `proyecto.json`, así que **un proyecto nacido en este PC no tiene copia** y `recibir.rs:532`
escribe encima igualmente. El comentario de `recibir.rs:506` promete lo contrario: «Si la copia no se
puede hacer, tampoco se escribe.»

### ALTO 20 — la poda de copias borra objetos a partir de una lista que traga errores

`crates/pixpin-sincro/src/copias.rs:291-306` + `:62-71` + `:209-214`. `podar` construye `vivos` con
`if let Ok(l) = read_dir(...)` (si falla, queda **vacío**) y `lista()` descarta en silencio toda copia
cuyo `.json` no se pueda leer. Después borra todo objeto que no esté en `vivos` (`:303`). Y `podar`
corre al final de **cada** `hacer` (`:194`), o sea en cada recepción. Un fallo transitorio de lectura
borra esos objetos para siempre; la copia sigue en la lista, y `restaurar` se salta cada fichero cuyo
objeto falte (`:209-214`) **sin decir nada**: el usuario cree que volvió atrás y la mitad se quedó igual.

### ALTO 21 — carrera real sobre `guardados.jsonl`: tres hilos sin cerrojo

No hay ningún bloqueo de fichero en el repo. Escriben sobre el mismo cuaderno el hilo del chat
(`ventana_chat.rs:5138,5250,5679`), el de recibir (`recibir.rs:706,755`) y el de recordatorios
(`recordatorios.rs:144`). `cuaderno::reemplazar` (`cuaderno.rs:316-338`) es leer-todo → construir →
escribir temporal → rename: **cualquier mensaje añadido por otro hilo entre la lectura (`:318`) y el
rename (`:337`) se pierde**, porque el rename pone encima una copia del fichero tal como estaba antes.

*Arreglo:* un `Mutex` estático por carpeta tomado en `anadir`, `reemplazar`, `quitar_del_cuaderno` y
`vista::escribir_lineas`. Pide diseño, no parche.

### ALTO 22 — `poner_al_dia` escribe con una ruta que vino de otro aparato, sin sanear

`apps/pixpin/src/recibir.rs:737-743`: `carpeta.join(r)` + `std::fs::write(&destino, bytes)?`, donde
`r` es `viejo.ruta`, del mensaje del cuaderno. No pasa por `vista::ruta_real`, que sí tiene la guarda
(`vista.rs:334`). Una `ruta` absoluta (`C:/Users/.../tesis.docx`) hace que `Path::join` descarte la
base en Windows y se escriba **fuera de la carpeta del proyecto**, encima de un fichero cualquiera.
(Los nombres de dentro del `.pixpin` sí están protegidos: `lib.rs:244` usa `enclosed_name()`.)

### ALTO 23 — el dibujo que no se pudo guardar se confunde con «no había nada que guardar»

`apps/pixpin/src/ventana_chat.rs:8384-8395`: `guardar_hoja_dibujada` devuelve `bool`, y `false`
significa a la vez «no cambió» (`:8380`) y «falló la escritura» (`:8391-8393`, sólo `tracing::error!`).
El usuario que dibuja y cierra con el disco lleno o el fichero bloqueado **no ve nada**.

### ALTO 24 — dos mensajes con el mismo `id` se editan y se borran juntos

`cuaderno.rs:322-331` empareja por `otro.id == m.id` y **no corta al primero**;
`ventana_chat.rs:10126-10133` hace lo mismo al borrar. El `id` es la hora en milisegundos
(`cuaderno.rs:215,238,260`) y `meter_en_proyecto` lee `ahora_utc_ms()` **dentro** del bucle de
ficheros soltados (`ventana_chat.rs:5237,5263-5286`). Dos adjuntos en el mismo milisegundo nacen con
el mismo `id`: editar uno sobrescribe los dos. *Arreglo:* comparar por `codigo_unico()`.

### MEDIO — el resto, en una lista

- `almacen.rs:524`: lienzos y adjuntos con `fs::write` directo (mitigado por la copia previa, salvo
  en el 19).
- **Ningún guardado hace `fsync`**: cero `sync_all`/`sync_data` en el repo. `write` + `rename`
  protege de la caída del proceso, no de la del sistema. Riesgo por construcción, no observado.
- `cuaderno.rs:300`: `writeln!` sobre un `File` sin tampón emite texto y `\n` en dos `write`; en
  append cada uno es atómico, los dos juntos no.
- `apps/pixpin/src/universo/sesion.rs:2462`: `let _ = self.guardar(escena)` en el camino real de
  cierre (`ventana_editor.rs:2343`). Con un fallo persistente, el autoguardado lleva fallando en
  silencio toda la sesión.
- `motor2d/src/escena.rs:23-52`: `Escena` no tiene cajón de sastre. En los lienzos da igual
  (`excalidraw::con_escena` conserva lo ajeno), pero en `.pixpin2d` (`motor2d/formato.rs:50-53`) y en
  `anotaciones` dentro de `universo.json` (`universo/formato.rs:71`) lo que escriba una versión futura
  se borra al guardar.
- `copias.rs:138-141`: el resumen se calcula sobre una lectura y el objeto se guarda en otra.
  `copias.rs:204-238`: `restaurar` no es atómica.
- `disco.rs:1150-1152`: el camino de respaldo de `reemplazar` trunca y reescribe el destino real
  cuando el rename falla. Es el mismo punto que la revisión anterior marcó como CRÍTICO 1 y **sigue
  abierto**.
- `protocolo.rs:606,610-622,1429`: el tamaño del archivo que llega no está acotado.
- `almacen.rs:191-198`: carrera en `guardar_adjunto` (el `while exists()` y el `write` no son atómicos).
- `lib.rs:324`: `Paquete::guardar` escribe el `.pixpin` con `fs::write` directo.
- `entrada.rs:452,473`: errores del micrófono tragados a mitad de grabación (un USB desenchufado
  devuelve `Ok(())` y la nota se cierra «como buena» con menos audio). Y `:317-319` tira una nota que
  **sí** quedó entera si el último `WriteSample` falló.
- `aligerar.rs:351`: el temporal es un nombre fijo (`aligerando.pdf`); dos aligerados simultáneos del
  mismo fichero chocan. `:352,358` mapean cualquier error de E/S a `ErrorPdf::NoExiste`.
- `envio.rs:70-76`: `legible()` indexa `&codigo[..3]` con la única guarda `len() == CIFRAS` en bytes;
  un código de 6 bytes no-ASCII entra en pánico. Es `pub` y no llama a `valido()`. Fuera del camino
  de guardado.

---

## 3. Privacidad

El mosaico **tapa bien en el camino normal**, y está mejor hecho de lo que suele: el grano se mide en
coordenadas del documento, así que acercarse no afina los bloques; la opacidad no llega a él por
ninguno de los dos caminos de pintado; el arrastre lo repinta cada fotograma en su sitio nuevo; la
capa congelada guarda el original debajo pero la pasada de tapado corre **después del `EndDraw` y
antes del `Present`**, sobre toda la superficie incluido el colchón del desplazamiento; el recorte a
la pantalla redondea hacia fuera para que no quede media fila legible; y `tapar_rgba` calcula todos
los promedios antes de escribir, para no arrastrar el cuadro ya tapado. Todo eso está probado píxel
a píxel (`mosaico.rs:510-639`, `muestra_del_mosaico.rs`).

Pero hay tres caminos por los que **sí se ve lo tapado**, y el primero es grave porque es una
herramienta que está en la caja, al lado del propio mosaico.

### CRÍTICO 25 — la lupa deshace el mosaico con el ratón

Dos sitios, el mismo fallo:

- `apps/pixpin/src/capa.rs:445` pinta las órdenes (mosaico incluido) y `:468-514` (`pintar_lupa`)
  dibuja **encima** ampliando `self.fondo.bitmap` —la foto congelada, **sin anotaciones**— o, en vivo,
  el último fotograma de la sesión, también sin anotaciones.
- `crates/pixpin-pin/src/ventana.rs:1889` hace `p.con_recorte(caja, |p| pintar_anotaciones(p, i, m))`
  y acto seguido `:1894-1910` dibuja `i.bitmap` —el bitmap original del pin— ampliado encima.

El cristal de la lupa es, literalmente, una ventana al original por encima del mosaico.

**Reproducción:** pinear una captura → herramienta Mosaico sobre un número de cuenta → pulsar `Q`
(la lupa, `ventana_editor.rs:181`) → pasar el cursor por encima del mosaico: dentro del círculo se
lee el número. Igual en la capa de anotación sobre una captura congelada.

Es sólo en pantalla (no se guarda ni se captura, como dice el comentario de `capa.rs:466`), pero en
pantalla basta: grabando, compartiendo pantalla o con alguien detrás.

*Arreglo:* ampliar desde el destino ya compuesto (`CopyFromRenderTarget` tras las anotaciones) en vez
del bitmap de origen; o, como parche mínimo, no pintar la lupa cuando su región fuente corta la caja
de algún `Figura::Mosaico`.

### ALTO 26 — un mosaico girado no tapa lo que enseña

`crates/pixpin-motor2d/src/elemento.rs:652`: `caja()` devuelve `(x, y, x+ancho, y+alto)` e **ignora
`self.angulo`**. `mosaico.rs:94` lee esa caja y `plan_en_pantalla` (`:151-171`) sólo lleva **dos
esquinas** a pantalla, sin giro; `pintado.rs:475-486` empuja la banda maciza igual.

Y el mosaico **sí se puede girar**: `tiradores.rs:97` da tirador de giro a cualquier elemento y nada
excluye `Figura::Mosaico`. El marco de selección sí gira (`ventana_editor.rs:2698`), así que la
interfaz promete un rectángulo girado y el tapado cubre la caja sin girar: **las cuatro puntas de lo
que el usuario quiso tapar quedan fuera**.

**Reproducción:** mosaico sobre un texto, girarlo 30° con el tirador → asoman las esquinas del texto.

*Arreglo:* en `plan_en_pantalla`, si `e.angulo != 0`, tapar la caja de las cuatro esquinas **ya
giradas** (tapa de más, nunca de menos), y girar los cuatro puntos del relleno en `pintado.rs:476-481`.

### ALTO 27 — lo que se guarda y lo que se sincroniza es el original más una instrucción

`excalidraw.rs:268,1237-1239`: el mosaico se serializa como un elemento `"pixpin-mosaic"` aparte. El
`.excalidraw` de una hoja de chat guarda **los elementos originales intactos** (el texto, la imagen,
la página del PDF, las fotos de `imagenes/`) y, al lado, la instrucción de taparlos. Ningún sitio del
código destruye el original: todo `mosaico.rs` promedia píxeles **de pantalla**.

Ese fichero es exactamente lo que viaja por Wi-Fi (`disco.rs:696`, `es_texto` → `.excalidraw.gz`) y
lo que queda en disco. Cualquier visor que no conozca `pixpin-mosaic` —excalidraw.com, un editor de
terceros, un `jq`— enseña lo tapado. Borrar el elemento del mosaico en el editor también lo revela.

**Reproducción:** tapar un dato en una hoja de chat, cerrar, abrir el `.excalidraw` del proyecto en
excalidraw.com → el dato se lee.

*Arreglo:* no es de dos líneas si se quiere mantener la edición no destructiva. Lo mínimo honesto:
al exportar o compartir fuera del grupo, rasterizar y mandar el PNG (el camino `ordenes_de_escena`
ya tapa opaco), y **decirlo donde el usuario lo lea**, porque hoy el mosaico promete algo que el
fichero no cumple.

### MEDIO 28 — dos tablas de propiedades, y una ofrece opacidad al mosaico

`estilo.rs:264` dice `Figura::Mosaico { .. } => &[Grosor, Material, Opacidad]`; `propiedades.rs:56-59`
dice `&[Relleno]` con el comentario «Ni opacidad —un mosaico a medias no tapa—». Como `estilo.rs:218-221`
explica, esa tabla decide qué se escribe al **pegar un estilo**: copiar el estilo de un rectángulo al
20 % y pegarlo sobre un mosaico persiste `opacity:20` en el `.excalidraw`. Hoy no se ve a través
(`pintado.rs:485` fuerza `a: 1.0`), pero el valor viaja y cualquier consumidor que lo respete lo revela.
*Arreglo:* `estilo.rs:264` → `&[Grosor, Material]`.

### MEDIO 29 — el que responde entrega el catálogo completo, elija lo que elija el usuario

`crates/pixpin-sincro/src/protocolo.rs:492-498` contesta `"catalogo"` con `d.chats()?` —id, nombre y
fechas de **todos** los chats— y `:501-502` contesta `"lapidas"` con la lista de los **borrados**.
Además `protocolo.rs:478-714` no filtra por chat elegido: quien llama puede pedir
`"preparar"`/`"archivos"`/`"dame"` de cualquier chat.

La pantalla «¿Qué sincronizar?» hace creer que el usuario elige qué sale, pero en el lado que
**responde** no hay pantalla ninguna (`sincronizar.rs:15`: «cuando el móvil llama, este equipo
responde con el `Respondedor`»).

**Reproducción:** emparejar PC y móvil; en el PC marcar sólo un chat; desde el móvil pedir el catálogo
→ salen los nombres de todos; pedir otro chat → se entrega entero.

*Arreglo:* guardar también en el lado que responde el conjunto elegido (`disco.elegidos(&otro.id)`,
que ya existe, `vuelta.rs:160`) y filtrar con él `d.chats()?` y el `chat()?` de `"preparar"`,
`"archivos"`, `"dame"` y `"damecambios"`.

### BAJO 30 — tres cierres de una línea

- `vuelta.rs:201`: sin elección previa, lo de por defecto es **todo** (`ids.iter().cloned().collect()`),
  y `:182` saca todas las casillas marcadas la primera vez. No es un fallo, pero el camino de menos
  esfuerzo es mandarlo todo.
- `envio.rs:613-629` (`nombre_sano`): no trata los nombres reservados de Windows (`CON`, `NUL`,
  `COM1`…) ni un nombre que sea `.` o `..` a secas. En la práctica no saca nada de la carpeta, pero la
  defensa depende de dos accidentes (`ruta_libre` y que crear `CON` falle).
- `disco.rs:703-714` (`permitida`): no filtra `:`, así que un `rel` remoto tipo `guardados/foo.txt:oculto`
  escribiría un **flujo alterno NTFS** dentro de la carpeta de sincronización. No saca nada fuera, pero
  deja bytes donde ninguna pantalla los enseña. *Arreglo:* `|| rel.contains(':')`.

### Comprobado y correcto

**Mosaico:** zoom (grano en coordenadas de documento, `mosaico.rs:39-49,63-69`), zoom muy lejano
(suelo `LADO_MINIMO = 2`, no 1), opacidad en los dos caminos de pintado, elemento bloqueado, arrastre,
capa congelada, recorte al borde, mosaico sobre mosaico, y que no se inventa ni conserva detalle.
El portapapeles copia la **captura**, no la escena. La vista previa del chat, las anotaciones del pin
y las miniaturas tapan (banda maciza). `pixpin-pdf` sólo lee y dibuja: **no hay exportación de escena a
PDF hoy**, así que ese camino no existe todavía.

**Wi-Fi:** no sale ni una ruta absoluta, ni el nombre de usuario, ni el del equipo (los tres llamadores
pasan la cadena literal `"PC"`); el identificador es un UUID v4 aleatorio, no una MAC ni un número de
serie. `protocolo.rs:717-727` (`suyo`) impide pedir un fichero de otro chat o de fuera de las siete
carpetas — es el control mejor hecho de este lado. `copias/` y `sincro/` no están en `CARPETAS`: no se
pueden pedir. Un envío suelto manda exactamente las rutas elegidas, sin recorrer carpetas, con
metadatos mínimos (tipo, nombre base, bytes, mime de tabla fija) y **sin EXIF ni GPS**. El canal va
cifrado con la clave derivada del código, cada archivo lleva SHA-256 de cola, quien envía aprueba a
mano, y hay tope de 5 intentos. Lo único que se emite antes de emparejar es el anuncio mDNS mientras
el usuario tiene la puerta abierta, y **el código no se publica**.

---

## 4. Fallos de cableado

Es, otra vez, la clase de fallo más abundante de la tanda. **Hay cinco piezas escritas, probadas y
enchufadas a nada**, y en dos de ellas el botón ya está en la caja de herramientas: el usuario lo
pulsa y no pasa nada.

### ALTO 31 — la tabla de atajos nueva no la lee nadie, y contradice a la que sí funciona

`crates/pixpin-motor2d/src/seleccion.rs:139-230` define `Tecla`, `OrdenEditor` (14 órdenes) y
`atajo_de(...)`. **`OrdenEditor` tiene cero consumidores fuera de su propio fichero y sus pruebas**
(`grep -rn "OrdenEditor" --include=*.rs crates apps` excluyendo `seleccion.rs` no devuelve nada), y
es nuevo de esta tanda (`git show 2b7ca5f:…/seleccion.rs` no lo tiene).

O sea: los catorce atajos documentados —`Q` lazo, `K` cuentagotas, `Ctrl+Alt+C`/`V` el estilo,
`Mayus+H`/`V` voltear, `Ctrl+G` agrupar, `Ctrl+[`/`]` el orden de pintado, `Ctrl+'` la rejilla,
`Alt+S` el imán— **no existen para el usuario**.

Y hay una contradicción que hará daño cuando alguien enchufe la tabla: el editor usa su propia
`tecla_a_herramienta` (`apps/pixpin/src/ventana_editor.rs:174-190`), donde **`Q` es la lupa**,
mientras `atajo_de` dice que `Q` es el lazo (`seleccion.rs:185`). Dos tablas de atajos que no se
conocen. `Agrupar`/`Desagrupar` sí funcionan hoy, pero por el camino viejo y directo
(`ventana_editor.rs:1548-1554`), no por la orden.

*Arreglo:* traducir las teclas del editor a `Tecla` y despachar `atajo_de` en `ventana_editor.rs`,
y resolver el choque de `Q` en el mismo paso (o quitar la tabla si no se va a usar).

### ALTO 32 — el lazo: 495 líneas, un botón con icono, y ningún llamante

`crates/pixpin-motor2d/src/lazo.rs` implementa el lazo entero (`Lazo::empezar`, `atrapados`,
`dentro_del_poligono`, `ModoLazo`), con un módulo de cabecera que explica en qué mejora al móvil.
**Su único uso en todo el repo es la prueba `tests/muestra_de_enganchar.rs:273`**: `ModoLazo`,
`PASO_MINIMO`, `dentro_del_poligono` y `atrapados` tienen cero referencias externas.

Mientras tanto `Herramienta::Lazo` sí tiene botón (`caja_herramientas.rs:87`) e icono
(`caja_dibujo.rs:110`), y `deja_rastro()` devuelve `false` (`gesto.rs:179`), así que no crea ningún
elemento. No hay arm para `Lazo` en `ventana_editor/construir.rs`.

**Reproducción:** pulsar el botón del lazo y arrastrar en el editor. No se selecciona nada, no se
dibuja nada, no pasa nada.

*Arreglo:* instanciar `Lazo` en el gesto del editor cuando la herramienta sea `Lazo` y volcar
`atrapados(...)` en la selección al soltar; o quitar el botón hasta que se enchufe.

### ALTO 33 — copiar estilo: el botón está, el motor está, no se tocan

`crates/pixpin-motor2d/src/estilo.rs:283-343` define `EstiloCopiado`, `copiar(...)` y `pegar_a(...)`.
**Los tres tienen cero referencias fuera de `estilo.rs`.** `Herramienta::CopiarEstilo` tiene botón
(`caja_herramientas.rs:113`) e icono (`caja_dibujo.rs:119`), `deja_rastro()` es `false`
(`gesto.rs:183`) y **no hay arm en `construir.rs`** —a diferencia de `Relleno`, `Recortar`,
`Extender` y `Punto`, que sí lo tienen (`construir.rs:46-51`)—.

El commit `921328a` se titula «La flecha sigue a la caja, voltear es una orden, y el estilo se
copia». El estilo no se copia: el botón no hace nada.

### ALTO 34 — voltear: la función existe, la orden existe, nadie las ejecuta

`crates/pixpin-motor2d/src/transformar.rs:347` (`voltear`) y `EjeVolteo` tienen **cero llamantes**
fuera de su fichero y sus pruebas; las únicas otras apariciones de la palabra en el repo son dos
comentarios de `seleccion.rs:175,409` y uno ajeno en `pin_geometria.rs:338`. Mismo commit,
misma promesa.

### MEDIO 35 — `texto_en_figuras.rs`: 447 líneas y nueve funciones públicas sin llamante

`crates/pixpin-motor2d/src/texto_en_figuras.rs`. De sus once elementos públicos, **nueve tienen cero
referencias externas** (`RELLENO_DEL_TEXTO`, `admite_texto_dentro`, `esquina_del_hueco`,
`ancho_que_cabe`, `alto_que_cabe`, `figura_que_lo_contiene`, `sitio_del_texto_dentro`,
`repartir_en_lineas`, `contenedor_de`) y los otros dos también: los aparentes usos de `texto_de` son
otra función del mismo nombre en `pixpin-docs/src/lib.rs:154` y `pixpin-geom/src/seleccion_texto.rs:136`,
y los de `atar` son la palabra en comentarios de otros crates. El módulo entero está fuera del circuito:
escribir texto dentro de un rectángulo (lo que en el móvil es `containerId`) no se puede hacer desde
el PC.

### MEDIO 36 — `lupa_elemento.rs`: 613 líneas de las que se usa una constante

`crates/pixpin-motor2d/src/lupa_elemento.rs`. El único uso del módulo en todo el repo es
`OSCURECER_POR_DEFECTO` desde `pintado.rs:581`. `Cristal`, `GuiaDeLupa`, `leer`, `escribir`,
`aumento_de`, `foco_de`, `toca_el_foco`, `oscurecimiento_de`, `zona_de`, `con_zona` y
`contorno_del_cristal` tienen cero llamantes. La lupa del móvil viaja intacta por el carril ajeno
(`pixpin-lupa` está en `LOS_AJENOS`), que es lo correcto, pero el puente escrito para ella no está
enchufado. Revisado su contenido: los nombres y tipos coinciden con `Element.kt:897-951`, así que es
trabajo bueno a la espera de cable.

### MEDIO 37 — editar los puntos de una flecha: escrito y sin enchufar

`crates/pixpin-motor2d/src/tiradores.rs`: `AgarrePunta`, `TiradoresDePunta`, `mover_punta`,
`insertar_punta` y `quitar_punta` tienen **cero llamantes**. `RAZON_ANADIR` y `SEPARACION_GIRO`
también. Los tiradores de caja (`Agarre`, `Tiradores`, `LADO`) sí están enchufados en `gesto.rs:39`.

### BAJO 38 — piezas sueltas sin llamante

`regiones::{es_pared, punto_en_region, area_de, area_de_region, douglas_peucker, TRANSPARENTE}`;
`relleno::{ANGULO_RAYADO, FACTOR_SEPARACION, lineas_de_zigzag, lineas_a_tiralineas}`;
`mosaico::{Tapado, tapado_de, EnPantalla, Recuadro}`; `shell::aviso::{ErrorOverlay, ID_DE_LA_BANDEJA}`.
Ninguna es un botón muerto —son ayudantes de los módulos que sí funcionan—, pero conviene decidir
si se enchufan o se quitan antes de que se olviden.

**No se encontró** ningún hilo que nadie pare (cero `thread::spawn` en `pixpin-audio`,
`pixpin-shell`, `pixpin-docs`, `pixpin-pdf`, `recordatorios.rs`, `audio.rs` y `mini_panel.rs`), ni
ninguna entrada de menú sin acción más allá de las anteriores, ni claves de i18n huérfanas.

---

## 5. Promesas falsas

- **`gesto.rs:119-122`**: «La flecha de codos… Hasta que su grupo porte `Elbow.kt` nace como una
  flecha recta». Dos commits posteriores (`38989af`, `5f1940a`) dicen que el codo ya cruza el puente.
  O el comentario está viejo o la promesa del commit es de más: hay que decidir cuál y arreglar el
  otro. **No resuelto en esta revisión.**
- **Los tres botones/órdenes de los hallazgos 32, 33 y 34** son promesas falsas de interfaz: están en
  la caja de herramientas y no hacen nada.
- **`aligerar.rs:2`** promete «sin tocar el texto ni los vectores», y (hallazgo 15) los objetos que el
  lector no entiende se tiran sin contarlos. La promesa es más fuerte que el código.
- **`recibir.rs:506`** promete «Si la copia no se puede hacer, tampoco se escribe», y el hallazgo 19
  dice que sí se escribe.
- **`almacen.rs:487-490`** promete «Recibir no quita nada», y el hallazgo 13 dice que sí puede.
- **`gastos.rs:352-355`** dice que la diferencia del total «da igual porque al leer se ignora»;
  ignora el hash de sincronización (hallazgo 5).
- **`almacen.rs:205-207`** dice «es una lista, no los datos»; el hallazgo 12 dice que esa lista es lo
  único que el usuario ve.

No se encontró ningún aviso «todavía no» de algo que ya esté hecho.

---

## 6. Calidad

### Pruebas: lo que falta donde más duele

1. **«Aligerar sin perder texto ni vectores» no lo prueba nadie.** El único PDF que la suite
   construye es `pdf_escaneado()` (`aligerar.rs:1277-1303`): **no hay ni un `/Font`, ni un `BT … Tj`,
   ni un trazo vectorial en ningún fixture**. `un_escaneo_grande_baja_de_peso_y_se_sigue_leyendo:1477`
   sólo comprueba `doc.paginas() == 1` y el ancho de la imagen. `reescribir` reconstruye la `xref`
   entera, que es exactamente donde un fallo silencioso borra texto. Es el hueco más caro de la tanda.
   *Arreglo:* meter un `/Font /Helvetica` y un `BT /F1 12 Tf 50 700 Td (Factura 2026-0042) Tj ET` más
   una raya en el fixture, y afirmar que sobreviven a `aligerar`.
   (De regalo: el fichero de `un_pdf_sin_fotos_lo_dice_y_no_escribe_nada` (`:1542`) se llama
   `solotexto.pdf` y no contiene ni un byte de texto. Refuerza la falsa sensación de cobertura.)
2. **Las notas de voz nunca se reproducen en ninguna prueba.** `pixpin-audio/src/salida.rs:288-330`
   tiene tres pruebas y **las tres son caminos de error**. `lista()`, `duracion_ms()`, `posicion_ms()`
   avanzando, `ir_a_ms()`, `pausar()` y `poner_velocidad()` no se ejercitan jamás contra un `.m4a`
   real, ni siquiera bajo `--ignored`. Dos commits se titulan «Las notas de voz suenan al pulsarlas»
   y «por fin suenan».
3. **El visor de documentos no ha leído nunca un fichero real.** `crates/pixpin-docs/` no tiene
   fixtures: las 90 pruebas construyen el ZIP **exactamente como el parser lo espera**. Un `.docx`
   guardado por Word de verdad (con `[Content_Types].xml`, `numbering.xml`, deflate) no se ha abierto
   jamás. El commit dice «El PC ya entiende el Word, el libro y la pagina».
4. **La puerta de rendimiento del grano no corre nunca sola**: `muestra_de_tintas.rs:241` es la única
   comprobación del requisito «una tinta con textura no puede volver el lienzo lento» y está tras
   `#[ignore]` (necesita GPU, inevitable). Conviene anotarla en el guion de release.

### Pruebas que no prueban lo suficiente

- **`apps/pixpin/src/recordatorios.rs:1041`** (`soltar_el_vigia_para_el_hilo_sin_colgarse`): cero
  asserts y cero tiempo límite. Si la regresión vuelve, el `drop` bloquea hasta `i64::MAX` y la
  prueba **se cuelga para siempre** en vez de dar rojo. *Arreglo:* `Instant::now()` + `assert!(elapsed < 2s)`.
- **`crates/pixpin-shell/src/aviso.rs:44`**: `ID_DE_LA_BANDEJA: u32 = 1` está copiada a mano del `UID`
  privado de `bandeja.rs`, y el doc-comment admite el riesgo. Ninguna prueba lo ata.
- **`muestra_de_enganchar.rs`**: pese al nombre, **no comprueba nada de serialización de enganches**;
  son tres pruebas de pintado a PNG. `startBinding`/`endBinding`/`fixedPoint` sólo se cubren en
  `los_veintitres_tipos_del_movil.rs:493-501`.

### Nombres engañosos

- **`muestra_de_figuras_nuevas.rs:262`** (`las_ocho_puntas_y_las_dos_tramas_se_distinguen_de_un_vistazo`):
  lo que asierta es que cada punta mancha más que `Ninguna` y tres parejas maciza>hueca. **Nunca
  comprueba que las ocho huellas sean distintas dos a dos.** El doc-comment sí es honesto; el nombre va
  por delante del cuerpo. *Arreglo:* meter las ocho huellas en un `HashSet` y afirmar `len() == 8`, o
  renombrar.

### Lo que está bien cubierto (para no buscarlo dos veces)

Ida y vuelta **byte a byte** de los 23 tipos (`los_veintitres_tipos_del_movil.rs:313`); documento de
oro de **las 7 mini-apps** con la cadena exacta, incluidas las trampas del `sí` con tilde y el `FinEn`
con efe mayúscula, más «una clave futura sobrevive» y «una clave repetida dice lo mismo que en
Kotlin» — esto está por encima de lo normal; el mosaico tapando en CPU sin `#[ignore]`
(`mosaico.rs:509-640` y `muestra_del_mosaico.rs:227`); el enganche que sobrevive a guardar y reabrir
(`excalidraw.rs:2033`); y las puertas de cero asignaciones en el camino caliente (`asignaciones.rs`,
`puertas.rs`).

---

## Veredicto

**Listo para que el usuario lo pruebe, con cuidado:**

- Las **tintas con material y grano**, el **mosaico**, el **foco**, el **rombo**, las **ocho puntas**,
  el **imán**, el **enganche flecha-figura**, el **marco y el papel**, la **serie**, los **puntos** y
  el **bote de relleno**: se dibujan, se guardan y viajan al móvil. Buen trabajo, y la serialización
  es lo mejor hecho de la tanda.
- Las **7 mini-apps**: los documentos coinciden al carácter con el móvil y están fijados con pruebas
  de oro. **Salvo gastos en XOF/XAF/XPF/UGX** (hallazgo 3), que no hay que tocar hasta arreglarlo.
- Los **recordatorios** y el **sonar** de las notas de voz.

**No listo, no lo pruebe todavía:**

- **El arco y el punto etiquetado** (hallazgos 1 y 2): cualquier ida y vuelta con el móvil los
  destroza, y la prueba consagra el error.
- **Aligerar el PDF** (15 y 16): puede tirar objetos sin contarlos, la verificación no dibuja nada y
  **no queda copia del original**. Es la función más peligrosa de la tanda.
- **Grabar notas de voz** (17): se graba sobre el fichero final; un corte deja un `.m4a` ilegible con
  nombre bueno.
- **Recibir del móvil** (12, 13, 14, 18, 19, 22): hay tres caminos por los que un fallo transitorio
  se lleva por delante la lista de proyectos o la conversación entera.
- **El lazo, copiar estilo y voltear** (32, 33, 34): los botones están puestos y no hacen nada.

**Lo primero que arreglaría, por este orden:** 12 → 13 → 14 (tres cambios de una línea cada uno,
reusando `escribir_atomico`, y cierran el fallo que el propio comentario dice que ya le costó
lienzos al usuario una vez) · 1 y 2 (quitar cuatro conversiones e invertir la prueba) · 17 (el
`.parte`) · 3 (cinco strings) · 15 y 16 · 32/33/34 o quitar los botones.

---

## Lo que no pude comprobar

- **La sección 3, privacidad, quedó sin hacer.** El barrido de los caminos del mosaico (zoom,
  opacidad, arrastre, capa congelada, exportar a PDF, pin, chat, portapapeles, deshacer) y el de lo
  que sale por Wi-Fi no terminó a tiempo. Lo único que sí consta es que el mosaico **tapa** en CPU y
  en el editor, con siete pruebas que lo comprueban píxel a píxel (`mosaico.rs:509-640`,
  `muestra_del_mosaico.rs:227` sin `#[ignore]`), y que el camino GPU tiene su prueba tras `#[ignore]`
  (`capa_estatica.rs:594`). **Lo que no está comprobado es si el original sobrevive en alguna capa
  y se puede ver por zoom, por opacidad o en un PDF exportado.** Es la pregunta abierta más
  importante que deja esta revisión.
- **Nadie ha hablado todavía con un móvil de verdad.** Sigue siendo el residual de la tanda 1: las
  pruebas de extremo a extremo enfrentan el `Disco` del PC contra una reimplementación en Rust del
  `Disco.kt`. Los hallazgos 1, 2 y 3 son justo la clase de fallo que una sincronización real contra
  el teléfono habría cazado en diez minutos.
- **El hallazgo 5 (el U+00A0 del total)** no se confirmó ejecutando Java; se apoya en el dato de CLDR.
- **La frecuencia y los canales del AAC de salida** (`entrada.rs:360-520`) no se leyeron. Riesgo bajo.
- **Dónde acaba el fichero de audio en el móvil**: el PC lo llama `voz_<ms>.m4a` y Android guarda
  `<ms>_<nombre>` en `filesDir/guardados/`. Que el móvil lo encuentre depende de la traducción de
  rutas (`disco_android.rs`), que no se leyó entera.
- **`protocolo.rs:596-600`**: un `"pon"` sin `chat` y sin `"archivos"` previo usa `c == ""`.
  Inofensivo en Android; en el `Disco` del PC queda como sospecha sin verificar.
- **El comentario de la flecha de codos** (`gesto.rs:119`) frente a los commits que dicen que ya
  funciona: no se determinó cuál de los dos está mal.
- No se ejecutó la aplicación ni se sintetizó entrada, así que **ningún hallazgo de interfaz se ha
  visto con los ojos**: todos salen de leer el código y de contar llamantes.
