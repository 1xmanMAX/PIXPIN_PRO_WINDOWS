# Revisión de la tanda 1 (`dd44c21`..`797bdc7`)

**Fecha:** 2026-09-19 · **Rama:** `pin-en-vivo` · **Revisor:** agente que no escribe código de
producto. Plan: `docs/superpowers/plans/2026-09-19-plan-de-ejecucion.md`.

Unos 45 commits de cinco zonas: el protocolo de sincronizar entero, el chat como el del móvil, el
universo (rendimiento, gestos de mapa, aspecto Cosmos), mDNS, envío y recepción por Wi-Fi, y el
cambio de horas a UTC.

## Cómo se ha revisado

El árbol de trabajo cambia mientras se revisa (otros agentes commitean), así que todo se ha leído
sobre una copia limpia de `HEAD` (`git archive`) en el borrador de la sesión. Las líneas que se
citan son las de esa copia. El Kotlin de referencia es el que hay en el borrador de la sesión
(móvil v0.66.0, protocolo 4).

No se ha ejecutado la aplicación ni se ha sintetizado entrada.

## La puerta común

Mientras se revisaba, el árbol de trabajo estuvo un rato **sin compilar**, y no por esta tanda:
`crates/pixpin-tinta/` (entonces sin seguimiento) tenía `Cargo.toml` y `src/{filtro,trazo}.rs` pero
**no `src/lib.rs`**, y eso tumba `cargo metadata` para todo el espacio de trabajo —`fmt`, `clippy`
y `test` fallan antes de empezar con «no targets specified in the manifest»—. No se tocó; lo cerró
su propio autor en `66b61fc`. Queda apuntado sólo como aviso de método: un crate nuevo a medias en
`crates/*` deja a **todos** los demás agentes sin puerta común, así que conviene añadir el
`Cargo.toml` y el `lib.rs` en el mismo paso.

Sobre la copia limpia de `HEAD`, la puerta sale **verde**:

| Comprobación | Resultado |
|---|---|
| `cargo fmt --all --check` | limpio |
| `cargo clippy --workspace --all-targets -- -D warnings` | limpio |
| `cargo test --workspace --no-fail-fast -- --test-threads=1` | todo pasa salvo `la_segunda_adquisicion_falla_mientras_viva_la_primera` (la app está abierta: no cuenta) |

Las pruebas marcadas `#[ignore]` se han repasado una a una: todas están ignoradas por necesitar
GPU, sesión de escritorio, portapapeles real o un móvil; ninguna esconde trabajo sin hacer.

---

## 1. Compatibilidad con el móvil

**No se ha encontrado ningún fallo de compatibilidad.** Es la parte mejor hecha de la tanda, y
conviene decir por qué, porque es lo que hay que no romper:

- `crates/pixpin-sincro/src/mensajes.rs` respeta `encodeDefaults = false` campo a campo, con
  `skip_serializing_if` en todo lo que en Kotlin tiene valor por omisión y **sin** `skip` en lo que
  no lo tiene (`Apunte.creado`/`tocado`, `ArchivoInfo.bytes`/`tocado`), que es justo el error que
  habría tumbado la respuesta entera. Comprobado contra `sincro/Protocolo.kt` y `sincro/Disco.kt`.
- `crates/pixpin-sincro/src/canonico.rs` conserva el literal de cada número (`1.50`, `1.0E-5`) en
  vez de pasarlo por `f64`, que es lo único que deja salir byte a byte igual que kotlinx en los
  lienzos y croquis; y ordena las claves por **unidades UTF-16** (`canonico.rs:535`), no por bytes
  UTF-8, con su caso negativo (`canonico.rs:678`: `U+FFFF` va después de `😀` en Kotlin y antes en
  Rust). Ese detalle es de los que se descubren en producción seis meses después.
- `crates/pixpin-sincro/src/base.rs` calcula el sello con el orden de declaración de la clase, los
  mapas en orden de llegada y `"proyecto":null` escrito, que es lo que hace
  `Canonico.json` (`encodeDefaults = true`), y lo fija con el SHA-256 concreto, no sólo con la
  forma.
- `disco.rs:690-720` (`es_texto`, `permitida`, `limpio`) coincide carácter a carácter con el
  Kotlin, incluido rechazar `\` (que en Windows también separa carpetas).
- Los formatos de QR y los tipos de servicio mDNS (`_pixpin._tcp`, `_pixpinenvio._tcp`,
  `_pixpinrecibe._tcp`, TXT `g`/`n`) coinciden con `Envio.kt`, `Red.kt` y `Presencia.kt`.
- El cambio a UTC (`bc65d74`) está completo: no queda ningún `ahora_local_ms` en un camino de
  guardado; sólo se usa para pintar, a través de `a_local`.

### RESIDUAL — nadie ha hablado todavía con un móvil de verdad

Las pruebas de extremo a extremo (`crates/pixpin-sincro/src/de_verdad.rs`) enfrentan el `Disco` del
PC contra `DiscoAndroid` (`crates/pixpin-sincro/src/disco_android.rs`), que es una
**reimplementación en Rust** del `Disco.kt`. Prueban que las dos mitades de Rust se entienden, no
que Rust entienda a Kotlin: un malentendido compartido pasaría. Lo que sí ata el formato de verdad
son los vectores de oro con JSON copiado del móvil
(`crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs:605-606`, `kotlin.rs:345`), y son
buenos. Aun así, **antes de dar por buena la sincronización hay que sincronizar una vez contra el
teléfono real**, con un chat que tenga lienzo, tabla, PDF y emojis en los nombres.

### BAJO — el nombre en el TXT se recorta por puntos de código, no por UTF-16

`apps/pixpin/src/sincronizar/enviar_wifi.rs:496` y `recibir_wifi.rs:387` hacen
`cx.nombre.chars().take(40)`; Kotlin hace `nombre.take(40)`, que son 40 unidades UTF-16. Con emojis
en el nombre del aparato el PC anuncia un nombre más largo. Sólo afecta a lo que se ve en la lista
de aparatos. *Arreglo:* `encode_utf16().take(40)` y volver a montar la cadena.

### BAJO — tope de anidamiento propio

`canonico.rs:28` se rinde a 512 niveles y devuelve el texto tal cual; kotlinx no tiene tope. Un
JSON más hondo daría aquí un resumen distinto al del móvil y se resincronizaría en cada vuelta. No
es alcanzable con un lienzo real; queda apuntado porque el comentario no lo dice.

---

## 2. Pérdida de datos

Lo bueno primero: hay escritura atómica (`escribir_atomico`), hay copia de seguridad antes de
sincronizar (`copias.rs`, llamada desde `protocolo.rs:533` y `:983`), borrar un proyecto va a
`almacen\papelera\` con un `rename` y no a `remove_dir_all`, un `universo.json` que no se entiende
se aparta en vez de pisarse, las líneas JSONL ilegibles se conservan al reescribir, y **no hay ni
un `unwrap`/`expect` peligroso en un camino de guardado o de recepción de red** fuera de pruebas.

### CRÍTICO 1 — `reemplazar` borra el destino y puede quedarse sin poner nada

`crates/pixpin-sincro/src/disco.rs:1128-1134`:

```rust
pub fn reemplazar(tmp: &Path, destino: &Path) -> io::Result<()> {
    if std::fs::rename(tmp, destino).is_ok() { return Ok(()); }
    let _ = std::fs::remove_file(destino);   // el fichero del usuario, fuera
    std::fs::rename(tmp, destino)            // si esto falla, no queda nada
}
```

Es el camino de escritura de **todo** lo que se guarda al sincronizar: `guardados.jsonl` (los
mensajes enteros de un chat), `base/*.json`, `resumenes.txt`, `elegidos/*`. Si el segundo `rename`
falla —el antivirus con el `.tmp` recién creado abierto, el destino recreado por otro hilo, un
permiso— el original ya no existe y el nuevo no se ha puesto. Se ve como un chat que se queda sin
mensajes después de un error pasajero al sincronizar.

Matiz importante: **es un puerto fiel** del `if (!tmp.renameTo(f)) { f.delete(); tmp.renameTo(f) }`
de `Disco.kt`. Pero en el sitio que más duele —el fichero del chat— el Kotlin usa el camino
seguro, `tmp.copyTo(archivoDelChat, overwrite = true)` (`Disco.kt:119`), que no tiene ventana sin
fichero; aquí se usa el de borrar y renombrar. O sea que en este punto el PC es **menos** seguro
que el móvil.

*Arreglo:* mover el destino a `destino.viejo`, reintentar el `rename`, y borrar `.viejo` sólo
cuando haya salido bien. En Windows, `MoveFileExW` con `MOVEFILE_REPLACE_EXISTING` ya es atómico
sobre un fichero que existe. No rompe compatibilidad: es disco local, no formato del cable.

### CRÍTICO 2 — un error de lectura pasajero se lee como «no hay mensajes» y se reescribe el fichero entero

`crates/pixpin-proyecto/src/vista.rs:162-165`:

```rust
let texto = std::fs::read_to_string(...join("guardados.jsonl")).unwrap_or_default();
```

`aplicar_mensajes` (`vista.rs:467`) hace `lineas()` → `aplicar_en_lista` → `escribir_lineas()`, que
**reescribe el fichero completo**. Si `read_to_string` falla por lo que sea (`NotFound`, violación
de compartición de Windows, antivirus), sale la lista vacía y el fichero queda con **sólo los
mensajes que acaban de llegar del otro aparato**: el historial anterior desaparece.

Y los dos hallazgos se encadenan: el CRÍTICO 1 fabrica precisamente ese `NotFound` —entre el
`remove_file` y el `rename` el `guardados.jsonl` no existe— y la sincronización corre en un hilo
aparte (`apps/pixpin/src/sincronizar.rs:1426`).

*Arreglo:* distinguir `ErrorKind::NotFound` (lista vacía legítima) de cualquier otro error de E/S y,
en ese caso, salir con `Err` en vez de reescribir. El mismo patrón está en `disco.rs:1080-1088`
(`leer_lineas`), que usa `quitar_lapida` (`disco.rs:313`): una lectura fallida borra todas las
lápidas y los proyectos borrados vuelven a aparecer.

### ALTO 3 — el fichero recibido por Wi-Fi se escribe directamente con su nombre final

`crates/pixpin-sincro/src/envio.rs:536-547`. El `?` de `recibir_trozos` se lleva el error fuera de
la función, y la limpieza (`remove_file`) sólo corre en el caso `Ok(false)`. Si se corta la red o
la aplicación muere a mitad, **el fichero a medias se queda en la carpeta del usuario con el nombre
definitivo**, indistinguible de uno entero. El comentario de dos líneas más abajo promete lo
contrario («mejor nada que un fichero roto con un nombre que promete estar entero»), y el propio
repositorio ya lo hace bien en `disco.rs:350-373`, que escribe en `<destino>.sincro` y renombra al
final.

*Arreglo:* recibir en `con_sufijo(&destino, ".parcial")` y llamar a `reemplazar` sólo con
`bien == true`; borrar el parcial en cualquier salida por error.

### ALTO 4 — se promete una copia que para los ficheros grandes no existe

`crates/pixpin-sincro/src/protocolo.rs:1309-1319` dice «se queda la versión tocada más tarde (la
otra sigue en la copia)», pero `copias.rs:26` pone `TOPE_POR_ARCHIVO = 40 MB` y `copias.rs:85-88`
salta los que pasan. Un PDF o un vídeo de más de 40 MB tocado en los dos aparatos se pisa **sin
ninguna copia**, y quién gana lo decide el reloj de cada máquina
(`protocolo.rs:1313`: `if tm >= ts - self.desfase`).

Los dos comentarios se contradicen entre sí y con el código: `copias.rs:24` justifica el tope
diciendo «no lo pisa nadie», y `protocolo.rs:1309` es exactamente quien lo pisa.

*Arreglo:* si se va a pisar un binario que está en `sin_copiar`, renombrar el local a
`<nombre>.antes-de-sincro-<ts>` en vez de sobreescribirlo, o dejarlo sin tocar y listarlo en los
saltados.

### ALTO 5 — si la copia de seguridad falla, se sincroniza igual

`crates/pixpin-sincro/src/disco.rs:681-683` (`hacer_copia`) hace `unwrap_or(false)`, y los dos
llamantes (`protocolo.rs:533` y `:983`) tiran el `bool`. Con el disco lleno, sin permisos en
`copias/` o con un adjunto ilegible, la vuelta sigue y pisa mensajes y ficheros **sin red**, que es
lo contrario de lo que promete el comentario «Antes de tocar nada, como estaba».

*Arreglo:* que `hacer_copia` devuelva `io::Result<bool>` y que la vuelta se pare con un aviso claro
si la copia no se pudo hacer.

### MEDIO 6 — «escritura atómica» sin `fsync`

`disco.rs:1137-1143` hace `fs::write` + `rename` sin `sync_all()`, ni del fichero ni del
directorio. Es atómico frente a otros procesos, pero no frente a un corte de corriente: puede
quedar el nombre definitivo apuntando a contenido sin volcar. Igual en `universo.json` y en
`indice.json`, y en `escribir_archivo` (`disco.rs:356`) sólo hay `flush()` del `BufWriter`, que no
llega al disco. *Arreglo:* `sync_all()` antes del `rename`.

### MEDIO 7 — el tamaño que anuncia el otro no está acotado

`protocolo.rs:228-246`: `recibir_trozos` toma `largo` de la cabecera del par sin tope y escribe
cada trozo entero aunque pase de lo que queda. El `Canal` acota el tramo suelto, así que no es un
desbordamiento de memoria, pero sí puede llenar el disco o dejar un fichero más largo de lo
anunciado. *Arreglo:* rechazar un `largo` desproporcionado antes de crear nada y recortar cada
trozo a lo que queda.

### BAJO 8 — `ruta_libre` es TOCTOU

`envio.rs:616-627`: entre el `exists()` y el `File::create` (que trunca) el fichero puede aparecer.
Sólo con dos recepciones a la vez en la misma carpeta. *Arreglo:* `create_new(true)` y reintentar
con el número siguiente si da `AlreadyExists`.

### BAJO 9 — `recoger_objetos` borra todo si no puede leer las bases

`disco.rs:595-626`: si falla el recorrido de `sincro/base` o los `Base` no parsean, `vivos` queda
vacío y se borran **todos** los objetos. No se pierde nada del usuario (se rehacen mandando los
ficheros enteros), pero conviene no borrar cuando el recorrido no se completó.

---

## 3. Cableado

### ALTO 10 — tres de los cuatro avisos de `Cambio` no los escucha nadie

`crates/pixpin-sincro/src/disco.rs:53-58` define `Cambio { Mensajes, Proyectos, Archivos,
Identidad }`. En todo el repositorio:

- `Cambio::Archivos` se emite una sola vez (`disco.rs:380`) y **no hay ni un `match` que lo mire**.
- `Cambio::Mensajes` y `Cambio::Proyectos` sólo se emiten desde `disco_android.rs` (el simulador de
  las pruebas): el `Disco` real del PC no los emite nunca.
- El único que se maneja es `Cambio::Identidad`, en `apps/pixpin/src/sincronizar.rs:1110-1111`.

O sea: la sincronización cambia los mensajes, los proyectos y los ficheros del usuario bajo la
ventana, y **la interfaz no se entera**. Es el fallo característico de este proyecto: las dos
mitades existen y no se llaman. La consecuencia mala de verdad es con el editor abierto sobre un
`.excalidraw.gz`: la sincronización lo sustituye por la versión fusionada, el editor no lo sabe y
al guardar escribe encima su copia en memoria; y como al cerrar ya se apuntó ese resumen como «lo
acordado», la vuelta siguiente toma la diferencia por una edición local y **propaga la pérdida al
otro aparato**.

*Arreglo:* emitir `Cambio::Mensajes`/`Proyectos` también desde el `Disco` del PC, y que el chat, la
lista y el editor se suscriban y recarguen. Como mínimo, que el editor compare fecha y tamaño antes
de guardar y no pise si el fichero cambió bajo él.

### ALTO 11 — una conexión del universo se puede crear y no se puede borrar

`crates/pixpin-universo/src/operar.rs:349` define `pub fn borrar_conexion(&mut self, id: u64)` y
**no la llama nadie** en todo el repositorio, ni siquiera una prueba (el nombre aparece una sola
vez: la definición).

Desde la aplicación: el usuario marca dos astros y pulsa «Conectar entre sí»
(`apps/pixpin/src/universo/sesion.rs:1955`). Para deshacerlo, el inspector sólo ofrece **ciclar el
tipo** (`sesion.rs:1926`: Relación → Depende → Referencia → Secuencia → Relación; no hay
«ninguna»), y «Limpiar huérfanas» sólo quita las que apuntan a astros que ya no existen. Una
conexión buena es irreversible salvo deshaciendo en el acto con Ctrl+Z.

*Arreglo:* añadir `AccionInspector::BorrarConexion(i)` en `crates/pixpin-ui/src/universo.rs:223` y
despacharla a `u.borrar_conexion(c.id)` en `sesion.rs:1926`; o meter «ninguna» en el ciclo de
tipos.

### MEDIO 12 — dos métodos del protocolo portados y nunca cableados

`crates/pixpin-sincro/src/protocolo.rs:1260` (`solo_este_archivo`, puerto de
`Sesion.soloEsteArchivo`) y `protocolo.rs:1540` (`info_de`): una sola aparición cada uno, la
definición. Ni llamante ni prueba. El diálogo «¿Qué sincronizar?»
(`apps/pixpin/src/sincronizar/elegir.rs`) elige por chat, no por archivo, así que no hay síntoma
visible; es código que aparenta una función que la interfaz no ofrece. *Arreglo:* cablearlos a un
«sincronizar sólo esto», o quitarlos hasta que haya interfaz. Si se cablean, ojo con lo apuntado
más abajo: `solo_este_archivo` no hace copia de seguridad.

### Lo que sí está bien

Se ha seguido el hilo, no sólo mirado el nombre:

- **El universo desde la bandeja**: `bandeja-universo` → `ID_VENTANA_UNIVERSO` (`main.rs:117,141`)
  → `Evento::Menu` (`main.rs:828`) → `universo::lanzar(..., Pedido::Cosmos)`, con pruebas de orden.
- **Ctrl+U**: `VK_U` declarado (`ventana_chat.rs:43`) y **manejado de verdad**
  (`ventana_chat.rs:1389`), con guarda de que no haya una caja de texto activa.
- **Los menús del universo**: las nueve variantes de `AccionInspector` tienen rótulo
  (`sesion.rs:2283-2291`) **y** despacho (`sesion.rs:1869-1960`). Ninguna huérfana.
- **mDNS**: `sinc-escuchando` dice «se deja encontrar mientras esta ventana esté abierta» y eso es
  exactamente lo que hace (`sincronizar.rs:340-345`, atado al `AtomicBool` de la ventana). Sin
  promesa falsa.
- Los avisos «todavía no existe en Windows» del chat (`chat-no-hay-*`) corresponden a funciones que
  de verdad no están; `chat-no-hay-unir` hasta redirige a la alternativa real. Es honestidad, no
  promesa falsa.

No hay **ni un solo** `#[allow(dead_code)]` ni `#[allow(unused…)]` en `pixpin-universo`,
`pixpin-sincro`, `pixpin-ui` ni `apps/pixpin/src/universo`: nadie ha tapado un cabo suelto con un
`allow` «de momento». Ojo, que eso no basta por sí solo: un `pub fn` de un crate de biblioteca sin
llamantes **no** lo señala `clippy`, así que la ausencia de `allow` no demuestra que todo esté
cableado.

### Apuntado, sin contar como hallazgo

`solo_este_archivo` (`protocolo.rs:1260`) **no hace copia de seguridad** antes de pisar, al revés
que `preparar`. Hoy no lo llama nadie fuera de las pruebas; si se engancha a la interfaz, hay que
añadirle el `hacer_copia` antes.

---

## 4. Promesas al usuario y textos

Los dos catálogos están **bien**: 420 claves exactas cada uno, el mismo conjunto en los dos
(`crates/pixpin-store/i18n/{es-ES,en-US}/main.ftl`), vigilado por una prueba de paridad
(`crates/pixpin-store/src/idioma.rs:246`). Y de las 302 claves que el código pide literalmente,
**ninguna falta** del catálogo (la única sin entrada es `"clave-que-no-existe"`, el fixture de la
prueba de clave ausente). Las 12 de `chat-mes-{n}`, que se construyen a mano, también existen. No
hay ni un texto roto en pantalla.

### ALTO 13 — texto en español a pelo en la lista de aparatos de Sincronizar

`apps/pixpin/src/sincronizar.rs:870-879`: `hace_cuanto` devuelve `"ahora mismo"`,
`"hace {n} min"`, `"hace {n} h"`, `"ayer"` y `"hace {n} días"` como literales, y
`sincronizar.rs:2190` los pega con `format!(" · sincronizado {}", …)`, también literal.

Con PixPin en inglés, bandeja → Sync, cada fila queda **«Available · sincronizado hace 5 min»**:
mitad en inglés, mitad en español. Contradice la regla escrita en la cabecera del propio `.ftl`
(«Toda cadena visible al usuario vive aquí, nunca literal en el código»).

*Arreglo:* seis claves nuevas (`sinc-hace-ahora`, `-min`, `-horas`, `-ayer`, `-dias`,
`sinc-ultima-vez`) en los dos catálogos, y pasarle el `Catalogo` a `hace_cuanto`.

### BAJO 14 — la entrada «Editor» de la bandeja no está traducida

`apps/pixpin/src/main.rs:148`: `v.push((ID_VENTANA_EDITOR, "Editor".to_string()))`, sin clave. El
propio comentario lo reconoce («sin catalogo ni traduccion todavia»). Sale igual en los dos
idiomas.

### BAJO 15 — diez claves traducidas que nadie usa

Definidas en los dos catálogos y sin ninguna aparición en `.rs`. Nuevas de esta tanda:
`universo-abrir-todo` (el menú de bandeja usa `bandeja-universo`), `universo-conexiones`,
`universo-nuevas`, `chat-titulo`, `chat-adjuntar`, `menu-foto-pin`, `menu-foto-abrir` (el menú usa
`chat-abrir-con`). Ya venían de antes: `error-otra-instancia`, `info-titulo`,
`resultado-copiar/-guardar/-guardar-como/-descartar`.

No se ve nada raro en pantalla; es peso que el traductor traduce para nada.
`universo-conexiones` («Conexiones») además apunta a un rótulo de sección del inspector que se
pensó y no se llegó a pintar (`sesion.rs:2242` pinta las filas sin título). *Arreglo:* borrarlas, o
pintar esa cabecera.

### MEDIO 16 — el inventario del chat dice que falta una atribución que ya está

`docs/investigacion/2026-09-18-chat-android-inventario.md:206-207` dice que falta la línea de
Material Icons en `THIRD-PARTY-NOTICES.md`. Ya está puesta (`THIRD-PARTY-NOTICES.md:57`). Es un
documento que se quedó viejo dentro de la misma tanda; conviene quitar ese punto para que nadie lo
vuelva a «arreglar».

---

## 5. Calidad

### MEDIO 17 — el «presupuesto de rendimiento bajo prueba» no es un presupuesto

El commit `c5fe112` se titula «El universo: presupuesto de rendimiento bajo prueba», pero
`apps/pixpin/src/universo/sesion/medir.rs` **no tiene ni un `assert!`**: mide, imprime y ya. Y sus
tres pruebas van con `#[ignore]` (necesitan GPU de verdad), así que no corren en la puerta común.
Si mañana el fotograma se pone al doble, nada se entera. El arnés en sí está bien hecho —separa
CPU de GPU, calienta antes de medir, pinta a 3000 × 2000 al 150 %— pero es un banco de medida, no
una defensa.

*Arreglo:* o poner un tope con `assert!` sobre `Medida::total` con margen holgado, o cambiar el
título del trabajo para que no prometa una prueba que no existe.

---

## Lo que NO se ha podido comprobar

Para que nadie lea este informe como si fuera una revisión completa:

1. **Nada se ha ejecutado.** La lista de «sin llamante» sale de buscar en el texto, no del
   compilador. Un `pub fn` de un crate de biblioteca sin usuarios **no** lo señala `clippy`, así
   que puede quedar alguno más.
2. **No se han mirado una por una las 218 `pub fn`** de `pixpin-universo` y `pixpin-sincro`. Se
   cribaron automáticamente y sólo se investigaron a fondo las de 0-2 apariciones; alguna de las
   que tienen tres o más podría tener todos sus usos dentro de `#[cfg(test)]`.
3. **Envío y recepción por Wi-Fi es la parte menos verificada.** Se comprobó que sus claves de
   texto se usan y que el mDNS arranca, pero no se siguió cada botón de `enviar_wifi.rs` (1177
   líneas) ni `recibir_wifi.rs` (829) hasta su efecto real.
4. **El despacho de `ventana_chat.rs` (8031 líneas) no se recorrió entero.** Quedaron sin mirar los
   `_ => {}` de `ventana_chat.rs:1594`, `:1759`, `:6427`, `:6489` y `universo/sesion.rs:927`:
   puede haber ahí un mensaje que se emite y se traga.
5. **No se comparó el contenido de es-ES contra en-US palabra por palabra**, sólo que las claves
   sean las mismas. Podría haber una traducción inglesa que prometa algo distinto de la española.
6. La carrera entre el editor y la sincronización (hallazgo 10) está **deducida del código**, no
   reproducida a mano.
7. Sólo se revisó hasta `797bdc7`. Lo que entró después (`dc8d747`, `66b61fc`, teselado y motor de
   tinta) **no está revisado**.

## Veredicto

**Listo para que el usuario lo pruebe:**

- El **chat** con el aspecto del móvil, el **universo** (gestos de mapa, tema Cosmos, órbitas,
  lunas, el fotograma medido fuera de pantalla) y **mDNS**. Son cambios que se ven y que, si
  fallan, fallan a la vista. Están bien cableados: bandeja, Ctrl+U y los menús se han seguido
  hasta su efecto real. La única pega de uso es que **una conexión del universo no se puede
  deshacer** (hallazgo 11).
- El **cambio de horas a UTC** está bien hecho y completo. Aviso que ya está en el propio commit y
  conviene repetirle al usuario: **lo que guardó en el PC antes de este cambio se verá corrido por
  el huso** (unas cinco horas), y no hay forma segura de distinguirlo.

**No listo, y no conviene que lo pruebe con datos que le importen:**

- **Sincronizar con el móvil** y **recibir por Wi-Fi**, por los dos CRÍTICOS y los tres ALTOS de la
  sección 2: hay caminos en los que un error pasajero se lleva por delante el historial de un chat,
  y un corte de red deja un fichero a medias con nombre de fichero bueno. El protocolo y el formato
  están muy bien; lo que falla es cómo se escribe en disco alrededor.
- Y aunque se arreglen: la primera sincronización de verdad contra el teléfono debe hacerse
  **sobre una copia** del almacén, porque hasta ahora sólo se ha probado Rust contra Rust.

**Lo primero que hay que arreglar en la tanda 2**, por este orden: CRÍTICO 1 y 2 (van juntos, uno
dispara al otro), ALTO 3, ALTO 5, ALTO 10. Después, los baratos que se ven: ALTO 11 (borrar una
conexión) y ALTO 13 (el «sincronizado hace 5 min» en español dentro del inglés).

**Aparte de esta tanda:** `crates/pixpin-tinta/` sin `src/lib.rs` tiene el espacio de trabajo sin
compilar; quien lo esté escribiendo tiene que cerrarlo antes de que nadie más pueda pasar la puerta
común.
