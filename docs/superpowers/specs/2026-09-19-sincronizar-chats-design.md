# Sincronizar chats con PixPin Android (protocolo 4)

Fecha: 2026-09-19. Estado: implementado en esta tanda.

El grupo, el saludo y la sonda ya hablaban con el movil; todo lo demas se
contestaba con «PixPin para Windows todavia no sabe juntar chats». Este
documento dice como el PC habla el resto del protocolo de
`sincro/Protocolo.kt` (v0.51.0) **sin cambiar como guarda el PC**, en los dos
sentidos: el movil dirige y el PC responde (`Respondedor`), y el PC dirige y
el movil responde (`Sesion`).

## Donde vive cada cosa

| Pieza | Crate | Capa |
|---|---|---|
| `Base` con orden de llegada y `sello` como kotlinx | `pixpin-sincro::base` | 1 |
| El modelo `Mensaje`/`Proyecto` de Kotlin (normalizar, `resumenDe`) | `pixpin-sincro::kotlin` | 1 |
| `trait Disco` + lo comun (marcas, lapidas, base, objetos, cache de resumenes, `archivos`, `textoBase`, copias) | `pixpin-sincro::disco` | 1 |
| `Respondedor`, `Sesion`, `Parches`, `Hecho`, la vuelta de `SincronizarActivity` sin pantalla | `pixpin-sincro::protocolo`, `pixpin-sincro::vuelta` | 1 |
| Un `Disco` con la carpeta de Android, puerto directo de `Disco.kt` (para pruebas: el «movil») | `pixpin-sincro::disco_android` (feature `simulador`) | 1 |
| El `Disco` del PC: la vista Android sobre el almacen | `pixpin-proyecto::vista` | 2 |
| Pantalla «¿Que sincronizar?», progreso, final, «con todos» | `apps/pixpin/src/sincronizar.rs` | 4 |

`pixpin-sincro` sigue sin depender de ningun crate de PixPin: el `Disco` es un
trait cuyas primitivas (leer/escribir mensajes, proyecto y archivos) pone
cada lado; lo que en Kotlin es igual para todos (ficheros de `sincro/`) va en
metodos por defecto sobre `fn sincro(&self) -> PathBuf`.

## La vista: el almacen del PC visto como la carpeta `files` de Android

Android guarda todo junto: `guardados.jsonl` (todos los chats),
`proyectos/proyectos.json`, `pins/draw/<id>.excalidraw.gz`, `tablas/`,
`croquis3d/`, adjuntos en `guardados/`. El PC guarda un chat = un proyecto:
`proyectos/indice.json` + `proyectos/<id>/{guardados.jsonl, proyecto.json,
lienzos/, archivos/, imagenes/, documento.pdf}`. La vista traduce, sin
tocar ese formato:

**Chat ↔ ficha.** El id de chat de una ficha es: `general` si es «Mensajes
guardados» (`Disco.GENERAL`); si no, `resto["chat"]`; si no, el `id` de su
`proyecto.json` (lo que trae un `.pixpin` del movil); si no, su `id`. Un chat
que llega y no esta crea una ficha con `id = chat` si es un nombre de carpeta
valido y libre (si no, uno nuevo con `resto["chat"] = chat`).

**Rutas (riesgo 3 y 5).** Funcion determinista por chat `C` (carpeta `P`):

| Virtual (Android, relativa a `files`) | Real en el PC |
|---|---|
| `pins/draw/<d>.excalidraw.gz` | `P/lienzos/<d>.excalidraw` (texto sin comprimir) |
| `guardados/pc/<c>/<R>` | `P(c)/<R>` (lo nacido en el PC, de cualquier chat) |
| cualquier otra `X` | `P/android/X` (gz si `X` acaba en `.gz`) |

y al reves `lienzos/<d>.excalidraw → pins/draw/<d>.excalidraw.gz`,
`android/X → X`, cualquier otra `R → guardados/pc/<limpio(C)>/R`. Lo nacido
en el PC lleva el chat en la ruta virtual: dos `archivos/captura.png` de dos
proyectos no chocan en la carpeta unica del movil. `permitida` es la de
Android (sin `\`, sin `..`, carpetas de PixPin) y se aplica a la virtual.

**Texto dentro de JSON.** Android cambia su prefijo absoluto por
`pixpin:files/` al salir (`Rutas.aPortatil`) y al reves al entrar. El PC
**guarda lo que llega tal cual, con `pixpin:files/` dentro** (aLocal =
identidad), y al salir solo reescribe lo que nacio aqui con ruta relativa:
`ruta` de un mensaje (`archivos/x.png → pixpin:files/guardados/pc/<C>/archivos/x.png`)
y `files.*.path` de un lienzo que empiece por `imagenes/`. Asi
`salir(entrar(x)) == x` y el resumen de lo que vino del movil no cambia. El
chat del PC resuelve `pixpin:files/X` con la misma tabla
(`vista::ruta_real`), en las pocas lineas de `ventana_chat.rs` que abrian
`carpeta.join(ruta)`.

**Mensajes (riesgo 2).** `portatil(m)` = la linea guardada, con `proyecto`
= chat (`null` en `general`), las rutas de arriba, y **normalizada al esquema
de `Mensaje` de Kotlin** (`kotlin::normalizar_mensaje`): los 31 campos en su
orden, cada uno con su valor por omision (`encodeDefaults = true`), enums en
mayusculas, enteros como enteros y lo que no se conoce **se conserva detras**
(un movil mas nuevo manda campos que este no sabe; tirarlos cambiaria su
resumen). Es un punto fijo: normalizar lo normalizado no cambia nada, y
decodificar y volver a codificar en Kotlin tampoco. `resumenDe` =
`sha256(Canonico.de(portatil sin recuerdaEn/uid/aparato))`, en comun para los
dos `Disco`. Las lineas del movil se escriben en el `guardados.jsonl` del PC
tal como llegan (el `Mensaje` del PC tiene `resto` y las lee sin perder
nada); las que no se entienden se copian tal cual al reescribir.

**Proyecto.** `proyectoPortatil(C)` = `P/proyecto.json` (o uno hecho con la
ficha si el proyecto nacio aqui) con `id = C` y el nombre de la ficha,
normalizado al `Proyecto`/`Hoja` de Kotlin. `guardarProyecto` hace
`Proyectos.actualizada` (sin hojas repetidas, marcas de quitadas y codigos que
no se pierden), escribe `P/proyecto.json` y pone al dia la ficha (nombre,
codigos, `tocado`, hojas). `completar_hojas` no inventa mensajes en los chats
que ya se sincronizan (`resto["sincro"]`): el movil no tiene mensaje por
pagina de PDF y se los devolveria como nuevos.

**Borrar.** Quitar mensajes en el chat del PC deja marca en
`sincro/borrados.jsonl` (como `Disco.borradosEntre`); borrar proyectos deja
lapida en `sincro/chatsborrados.jsonl`. Sin eso volverian en la vuelta
siguiente. `borrarchat` (cuando lo pide el otro) hace copia, marca todos sus
mensajes, manda la carpeta a la papelera y apunta la lapida.

## Peticion por peticion (Respondedor)

| `t` | Que hace el PC |
|---|---|
| `hola` | Igual que ya hacia (unirse con letra, juntar miembros), y luego «ocupado» si ya hay una vuelta en esta carpeta. |
| `catalogo` | `chats()`: `general` + una por ficha, con `mensajes` y `tocado = max(proyecto.tocado, ultimo mensaje)`. |
| `lapidas` | `sincro/chatsborrados.jsonl`, la ultima de cada chat. |
| `borrarchat` | `borrarChat` (arriba). |
| `inventario` | `Copias.hacer`, `sellar`, `apuntes`, `proyectoPortatil`, `selloDeBase`. |
| `mensajes` | Los portatiles de esas claves, en el orden del chat. |
| `aplicar` | `aplicarMensajes` (sustituye por clave conservando `recuerdaEn`, borra dejando marca, sin registros repetidos) y, si viene proyecto, quita la lapida y lo guarda. |
| `archivos` | `alcance` sobre lo portatil + resumen (canonico si es texto), bytes, fecha, etiqueta, crudo; cache en `sincro/resumenes.txt` solo para > 4 MB. |
| `pon` / `dame` | Trozos de 1 MB y la cola con el resumen, byte a byte como `Protocolo.salida/recibirTrozos`; lo de texto sale portatil y descomprimido. |
| `parche` / `damecambios` | `Parches.poner/sacar` con `fusion::aplicar/diferencia`. |
| `base` | Guarda la base **con el orden recibido** y los objetos; apunta la vez. |
| `adios` | Respuesta vacia y fin. |

Cualquier error de disco se contesta como `error` (lo que hace Kotlin con
`Exception`), y uno de red corta.

## Paso por paso (Sesion)

`preparar` (copia, sellar, `inventario`, base valida solo si los dos sellos
cuadran, `conMarcasViejas`, `plan`) → `aplicar` (traer en tandas de 200,
fusionar con `fusion::json` sobre el objeto de la base, borrar, mezclar el
proyecto con `mezcla::proyecto` y los archivos de cada lado, mandar en
tandas) → `prepararArchivos` → `aplicarArchivos` (parche si el otro tiene lo
acordado, entero si no; los de texto cambiados en los dos se funden figura a
figura; PDF y fotos: gana el tocado despues) → `cerrar` (solo se apunta lo
que quedo igual en los dos, con `pasados` y `traducir`) → `adios`. La vuelta
completa (`vuelta::una`) hace antes `resolverBorrados` con las lapidas y
cuenta lo `Hecho`.

## Riesgos, uno por uno

1. **Sello de la base.** `base::Base` guarda sus mapas como lista de pares
   en el orden de llegada, se (de)serializa conservandolo y el sello es
   `sha256` del texto exacto de `Canonico.json.encodeToString(Base)`:
   `{"mensajes":{…},"archivos":{…},"proyecto":null|"…"}` con el escapado de
   kotlinx (`canonico::Json::a_texto`). Vector a mano en las pruebas.
2. **Resumen de mensajes.** Ver «Mensajes». Vector derivado a mano de las
   reglas (no se puede ejecutar Kotlin aqui) en las pruebas.
3. **Rutas.** Tabla de arriba; `\` nunca sale al cable.
4. **gzip.** `flate2` (ya estaba en `Cargo.lock` por `zip`) para
   `sincro/objetos/<r>.gz` y los `.gz` del espejo `android/`; los lienzos del
   PC se guardan sin comprimir porque es lo que lee su chat.
5. **Mapa de rutas.** La vista virtual de arriba; el PC no cambia como
   guarda.
6. **Escrituras atomicas.** Temporal (`.sincro`/`.tmp`, excluidos por
   `permitida`) + `rename`; en Windows se quita antes el destino si el
   `rename` falla.
7. **Capas.** El trait en capa 1, la vista del PC en `pixpin-proyecto` (2).
8. **Cerrar.** Portado tal cual, con `pasados` y `traducir`.

Lo que queda con riesgo (no se puede probar sin el movil): un campo nuevo
con valor por omision no nulo en una version futura de `Mensaje` o
`Proyecto` cambiaria el resumen de lo nacido en el PC; y un proyecto que
entro antes por `.pixpin` sin su `guardados.jsonl` tiene en el PC mensajes
propios por cada hoja que en la primera vuelta llegan al movil como nuevos.
