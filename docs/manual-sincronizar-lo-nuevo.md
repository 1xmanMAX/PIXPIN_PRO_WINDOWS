# Manualito: sincronizar lo nuevo (23 al 27-oct-2026)

Para los agentes de **PixPin Android** y de **Mac, iPad e iPhone**
(`apple/`). Dice, funcion por funcion, que ficheros y campos hay, donde
viven, con que formato, como se resuelven los choques y que tiene que hacer
el aparato que lo recibe para ensenarlo. Asi lo que se hace en un aparato
llega a todos los del grupo por la red (Wi-Fi), en los dos sentidos.

**No hay version nueva del protocolo** (sigue la 4): todo viaja como
siempre, en mensajes del chat (sus campos, conservados en `resto`) y en
ficheros que esos mensajes nombran. Cambian **tres reglas de fusion**
(seccion 7), y desde el 27-oct hay **lo negociado de la sincronizacion por
diario** (seccion 13): `novedades`, `entero`, `desdeByte` y `reanudar`, que
solo se usan si los dos dicen `capacidades: ["diario1","reanudar1"]` en el
saludo. Con quien no las dice, todo va exactamente como antes.

Lo de antes sigue igual: [novedades-2026-10-25-para-los-otros-aparatos.md](novedades-2026-10-25-para-los-otros-aparatos.md)
(resumen), [sincronizar-marcos-vistas-y-tinta.md](sincronizar-marcos-vistas-y-tinta.md)
(marcos, vistas y tinta) y [nota-rica-del-chat.md](nota-rica-del-chat.md).

## 0. Las tres reglas de siempre (repaso)

1. **Lo que no conoces, lo conservas.** Un campo de un mensaje que tu
   version no sabe se guarda tal cual y se vuelve a mandar tal cual
   (`ignoreUnknownKeys` + escribirlo de vuelta). Si se tira, el resumen del
   mensaje cambia y cada vuelta lo toma por cambiado; y el otro aparato
   pierde el dato.
2. **Un fichero viaja si un mensaje lo nombra** con su ruta portatil
   (`pixpin:files/<ruta relativa a files>`), en `ruta` o dentro de `texto`,
   o si es lo anotado de un mensaje (`pins/draw/anot-<uid>.*`) o de un
   lienzo (`pins/draw/<d>.marcas`, `<d>.vistas.json`). Solo dentro de
   `guardados/ pins/ proyectos/ tablas/ croquis3d/ notas/ voz/`.
3. **Sin lapidas de ficheros**: un fichero no se «borra» por la red; se
   vacia (`{}`) o se borra el mensaje que lo nombra.

## 1. Grabar clase o reunion

### Ficheros

| Que | Donde lo deja el PC | Ruta portatil | Formato |
|---|---|---|---|
| Nota de voz | `<proyecto>/archivos/<nombre>.m4a` | `pixpin:files/guardados/pc/<chat>/archivos/<nombre>.m4a` | AAC en MP4 (`.m4a`) |
| Video | `<proyecto>/archivos/<nombre>.mp4` | `pixpin:files/guardados/pc/<chat>/archivos/<nombre>.mp4` | MP4, H.264 + AAC |
| Cada captura | `<proyecto>/archivos/clase-<ms>-<n>.png` | `pixpin:files/guardados/pc/<chat>/archivos/clase-….png` | PNG |

En Android lo natural es la voz en `files/voz/` y el resto en
`files/guardados/`; cualquier carpeta de la lista de la regla 2 vale.

### Mensajes y campos

```json
{"id":"1761400000001","clase":"VOZ","nombre":"Clase 25-oct.m4a",
 "ruta":"pixpin:files/guardados/pc/AGJ36629KT/archivos/Clase 25-oct.m4a",
 "duracionMs":5400000,"picos":[3,9,4],
 "grabacion":"clase",
 "capturas":[{"ms":65000,"id":"1761400000003"}],
 "video":{"id":"1761400000002"},
 "marcadores":[{"ms":30000,"texto":"ojo: examen"},{"ms":60000,"texto":""}],
 "marcas":[12000],
 "transcripcion":"[0:00] Hoy vemos vigas\n\n[0:40] Y columnas",
 "estadoDelTexto":"LISTO"}
{"id":"1761400000002","clase":"ARCHIVO","nombre":"Clase 25-oct.mp4",
 "ruta":"pixpin:files/guardados/pc/AGJ36629KT/archivos/Clase 25-oct.mp4",
 "bytes":512000000,"enGrabacion":{"de":"1761400000001","ms":0}}
{"id":"1761400000003","clase":"IMAGEN","texto":"⏱ 01:05 · Clase 25-oct",
 "ruta":"pixpin:files/guardados/pc/AGJ36629KT/archivos/clase-1761400000003-1.png",
 "enGrabacion":{"de":"1761400000001","ms":65000}}
```

- `grabacion: "clase"` va **siempre** en la voz de una clase, y
  `capturas` tambien aunque sea `[]`.
- `video` solo si se grabo video. `enGrabacion.de` es el `id` de la voz.
- `marcadores`: en orden de `ms`, uno por milisegundo, `texto` de 200
  caracteres como mucho sin saltos de linea (puede ir vacio). **Sin
  marcadores la clave se quita** (no `[]`). Las cuentas estan en
  `crates/pixpin-proyecto/src/clase.rs`.
- `marcas` (`[ms]`, las banderitas) es del esquema de siempre.
- `transcripcion` es `[m:ss] texto`, un parrafo por trozo y una linea en
  blanco entre trozos. **Llega despues**: Whisper tarda minutos y el PC
  reescribe la nota al acabar (`estadoDelTexto` pasa a `LISTO`). Es un
  cambio normal del mensaje: viaja en la vuelta siguiente.

### Ficheros grandes (el video)

- **No hay tope de tamano.** Un archivo viaja a trozos de 1 MiB
  (`canal::TOPE_DE_TRAMO`) leidos del disco, nunca entero en memoria, con un
  SHA-256 al final; lo que llega se escribe a `<fichero>.sincro` y solo se
  pone en su sitio si el resumen cuadra. Por encima de 4 MiB el resumen se
  recuerda por tamano y fecha (`UMBRAL_DE_CACHE`): no se vuelve a leer el
  video entero en cada vuelta. Probado con un video de 5 MiB (varios
  tramos) en `una_clase_grabada_en_el_pc_llega_al_movil_con_voz_video_capturas_y_el_texto_de_despues`.
- **No se reanuda**: si la red se corta a mitad, el `.sincro` se tira y la
  vuelta siguiente lo manda entero otra vez. Una hora de reunion (~300-600
  MB) son 30-60 s en una Wi-Fi normal; es aceptable. Si un aparato quiere
  ahorrar datos moviles, que no sincronice archivos de mas de N MB fuera de
  la Wi-Fi (politica del aparato, no del protocolo: el otro lo vuelve a
  ofrecer en la vuelta siguiente y no se pierde nada).
- El video se mueve a `archivos/` **solo al terminar** la grabacion, ya
  cerrado: nunca viaja un MP4 a medias. Si cambia mientras se lee, la cola
  dice `saltado` y el otro no lo escribe.
- **Nombres con `(`** («Clase (1).mp4»): `Rutas.enTexto` corta en `)`,
  asi que el alcance anade ademas **la `ruta` exacta de cada mensaje**.
  Android lo hace desde el 1-oct (`Disco.alcanceCon`); el PC
  (`DiscoPc::alcance`) tambien, y desde el 27-oct el movil de las pruebas
  (`disco::alcance_de`). Probado en
  `los_archivos_van_de_pequeno_a_grande_y_si_se_corta_lo_pequeno_ya_esta`.
- **Lo pequeno primero** (seccion 11): si la vuelta se corta en el video,
  la voz y las capturas ya estan; el video se ofrece entero otra vez.

### Choques

- Un mensaje cambiado en los dos se junta **campo a campo** con la base.
- `transcripcion` se junta **por parrafos** (diff3); un mismo parrafo
  cambiado en los dos lo gana el que dirige.
- `marcadores` y `marcas`, **por milisegundo** (regla nueva, seccion 7.1).
- `capturas` (`[{ms,id}]`) es una lista con `id`: se junta por `id`.

### Lo que hace el que recibe

- Conservar todos esos campos aunque no los ensene.
- Reproducir la voz como cualquier `VOZ`; el `.mp4` es un `ARCHIVO` que se
  abre con el reproductor del sistema.
- Opcional («Estudiar la clase»): con `grabacion == "clase"`, ensenar el
  texto a un lado y, al otro, el video (si hay `video`) o la captura cuyo
  `enGrabacion.ms` es el ultimo `<=` al minuto que suena; los marcadores
  como fichas con su minuto en la letra.

### El movil como camara (27-oct-2026)

Con la capacidad `camara1` en el `hola` de los dos, el movil hace fotos y
clips para la grabacion que esta en curso en el PC y entran en la clase en
su minuto. No es sincronizacion: va por `camara_remota` y por `suelto` con
`destino: "grabacion"` (lo de «Enviar a tus aparatos»). Lo que queda en el
chat si viaja como siempre:

| Mensaje | `resto` nuevo |
|---|---|
| La nota de voz (`VOZ`) | `clips: [{ms, duracionMs, id}]` en orden de `ms` (los clips de video del movil; las fotos van en `capturas`, como siempre) |
| Cada foto del movil (`IMAGEN`) | `enGrabacion: {de, ms}`, `fuente: "movil"`, `deAparato`, `tomadaEn` |
| Cada clip del movil (`ARCHIVO`, `.mp4`) | `enGrabacion: {de, ms, duracionMs}`, `fuente: "movil"`, `deAparato` |

Y cada una en la carpeta de la grabacion, en su minuto (§5.1). Conservar
todo eso tal cual. Mensajes, flujo y tareas de Android:
[movil-como-camara.md](movil-como-camara.md).

## 2. La biblioteca de figuras

- Fichero `files/pins/draw/figuras.json` en el movil (donde lo lee
  `BibliotecaStore`); en el PC, `proyectos/<asistente>/android/pins/draw/figuras.json`.
- Lo hace viajar un mensaje fijo del chat **«Asistente»** (solo aparatos
  propios, nunca a personas de un chat compartido):

  ```json
  {"id":"auto-figuras","clase":"ARCHIVO","nombre":"figuras.json",
   "ruta":"pixpin:files/pins/draw/figuras.json","proyecto":"<chat del asistente>"}
  ```
- Formato: una lista de figuras, cada una con piezas como **elementos de
  Excalidraw**:

  ```json
  [{"id":"serie-norte","nombre":"Norte","propia":false,
    "elementos":[{"id":"serie-norte-p","type":"arrow","x":0,"y":0,
                  "version":1,"versionNonce":3,"updated":1}]},
   {"id":"1761400000000-1","nombre":"Sello de obra","propia":true,"elementos":[…]}]
  ```
- Las **figuras de serie** las pone cada aparato con ids fijos (y una marca
  local `figuras-de-serie.version` que no viaja): dos aparatos que las ponen
  a la vez dejan lo mismo.
- **Choque (regla nueva, 7.3)**: si el fichero cambio en los dos aparatos,
  se juntan **figura a figura por `id`** y, dentro de una figura cambiada en
  los dos, pieza a pieza. Lo que solo esta en un lado se queda. Borrar una
  figura en un solo aparato si viaja (no hay choque).
- **Adjuntar una figura** al chat crea un `DIBUJO` normal
  (`pins/draw/<d>.excalidraw.gz`) con la figura dentro: viaja como
  cualquier lienzo.
- Receptor: tras recibir el fichero, recargar la biblioteca.

## 3. Vistas de los marcadores de un lienzo

`pins/draw/<d>.vistas.json`, al lado de `pins/draw/<d>.marcas`:
`{"<id de la marca>": "x y ancho alto"}`. Vaciar es `{}`. Va en el alcance
de cada dibujo. Detalle y formulas: [sincronizar-marcos-vistas-y-tinta.md §3.8](sincronizar-marcos-vistas-y-tinta.md).
Choque: el fichero entero, gana el tocado mas tarde.

## 4. La nota rica

Una `NOTA` con Markdown; sus fotos en `<proyecto>/notas/<ms>-<nombre>`,
nombradas en el texto como `![alt](pixpin:files/guardados/pc/<chat>/notas/<ms>-….png)`
(el alt puede llevar `|ancho`). Viajan porque el texto las nombra (regla 2).
Choque: el `texto` se junta por parrafos. Receptor: pintar el Markdown y
las fotos (seguidas = album), nunca la ruta como texto. Ver
[nota-rica-del-chat.md](nota-rica-del-chat.md).

## 5. Las carpetas de paginas

Un mensaje `MINIAPP` con `miniapp: "carpeta"`; el nombre en `nombre` y el
documento, en una linea, en `texto`:

```json
{"id":"1761400000500","clase":"MINIAPP","miniapp":"carpeta","nombre":"Diario de obra",
 "texto":"{\"formato\":1,\"paginas\":[{\"id\":\"1761400000000\",\"titulo\":\"Día 1\"},{\"id\":\"1761486400000\",\"titulo\":\"Día 2\"}]}"}
```

- Las paginas son **los `id` de otros mensajes del mismo chat** (lienzos,
  paginas, fotos, notas): no se copian ni se mueven. Una pagina cuyo
  mensaje ya no esta se salta al leer.
- Una carpeta recien creada lleva `{"formato":1,"paginas":[]}`.
- **Choque (regla nueva, 7.2)**: pagina a pagina por `id` (anadir una en
  cada aparato deja las dos; quitar en uno quita), titulo a titulo.
- Receptor: Android ya ensena el `texto` de una mini-app que no conoce sin
  romperse. Para ensenarla de verdad: una fila «carpeta» que abre sus
  paginas en orden.
- **Lo de dentro no sale suelto** (25-oct-2026): en el PC, un mensaje que
  esta en alguna carpeta (de cualquier tipo) no se ensena como burbuja en
  el chat; lo representa la tarjeta de su carpeta. Sigue siendo un mensaje
  normal (viaja, se busca, sale en la galeria) y al quitarlo de la carpeta
  vuelve a verse. Es una regla de lo que se ensena, no del formato: un
  aparato que no la sepa lo ensena suelto, sin perder nada. Lo ideal es
  hacer lo mismo (`carpeta_de_paginas::dentro_de_carpetas`).

### 5.1 La carpeta de una grabacion (25-oct-2026)

Al terminar de grabar una clase o reunion, el PC mete la nota de voz, el
video y las capturas (los mismos mensajes y enlaces de la seccion 1, sin
cambios) **en una carpeta con el nombre de la grabacion**, que entra en el
chat detras de ellos. Es una carpeta como las de arriba con campos de mas
(todos opcionales; sin ellos es una carpeta de paginas):

```json
{"id":"1761400000004","clase":"MINIAPP","miniapp":"carpeta","nombre":"Clase 25-10 10h58",
 "texto":"{\"formato\":1,\"tipo\":\"grabacion\",\"voz\":\"1761400000001\",\"video\":\"1761400000002\",\"duracionMs\":5400000,\"paginas\":[{\"id\":\"1761400000001\",\"titulo\":\"\",\"ms\":0},{\"id\":\"1761400000002\",\"titulo\":\"\",\"ms\":0},{\"id\":\"1761400000003\",\"titulo\":\"⏱ 1:05\",\"ms\":65000},{\"id\":\"1761400000009\",\"titulo\":\"Apuntes\",\"adjunto\":true}]}"}
```

| Campo | Donde | Que es |
|---|---|---|
| `tipo` | documento | `"grabacion"`. Sin el (o vacio): carpeta de paginas. |
| `voz` | documento | `id` de la nota de voz (no va si la nota no se pudo cerrar). |
| `video` | documento | `id` del video (no va si no se grabo video). |
| `duracionMs` | documento | lo que duro, para la tarjeta sin leer la nota. |
| `ms` | pagina | minuto de la pieza en la grabacion (voz y video, `0`). |
| `adjunto` | pagina | `true`: se adjunto a la clase despues y **no es de lo grabado** (un PDF, una foto de la pizarra, una nota). No va si es `false`. |

- `formato` sigue en `1`: lo nuevo son claves de mas, que un lector de
  antes conserva (el PC las guarda en `resto`) y la fusion 7.2 junta como
  cualquier clave del objeto.
- Las paginas van en el orden de la grabacion: la voz, el video y las
  capturas por su `ms`; los adjuntos, detras. Las capturas son lo grabado
  que no es ni `voz` ni `video`.
- Los enlaces `capturas`/`video`/`enGrabacion` de la seccion 1 se siguen
  escribiendo igual: «Estudiar la clase» va por ellos, no por la carpeta.
- Una grabacion de antes (sin carpeta) sigue igual: sus mensajes sueltos.
- En el PC: la carpeta se ve como **una sola tarjeta** (la primera
  captura, «● Grabacion», lo que duro, cuantas capturas y «Estudiar»);
  pulsarla abre «Estudiar la clase». Su menu tiene «Abrir la carpeta»: lo
  grabado con su minuto y, aparte y en otro color, lo adjuntado con la
  chapa «Adjunto · no es de la grabacion». «Adjuntar a la clase» (en esa
  ventana) o «Añadir a carpeta…» de un mensaje del chat lo anaden como
  adjunto.
- Para leerla (Rust): `Carpeta::es_grabacion`, `de_la_grabacion`,
  `capturas`, `adjuntos`, `adjuntar` en
  `crates/pixpin-proyecto/src/carpeta_de_paginas.rs`.

## 6. Lo que no viaja (a proposito)

- **Exportar a Anki**: deja `<raiz>/anki/<mazo>-<ms>.txt` (y fotos en la
  carpeta de Anki). Es una salida para importar en Anki, no un dato del
  chat.
- Los temporales de la grabacion mientras dura, y `figuras-de-serie.version`.

## 7. Las reglas de fusion nuevas (para copiar en `Fusion.kt`)

Estan en `crates/pixpin-sincro/src/fusion.rs` (y copiadas en
`apple/nucleo/pixpin-sincro`). El resultado lo calcula quien dirige y lo
manda al otro, asi que los dos quedan iguales aunque solo uno sepa la regla;
pero si dirige un aparato que no la sabe, se pierde lo de uno de los lados.

### 7.1 `marcas` y `marcadores`, por milisegundo

En la fusion de un mensaje, para las claves `marcas` (`[ms]`) y
`marcadores` (`[{ms,…}]`): cada lista se pasa a un mapa `ms → elemento`
(una clave que falta cuenta como lista vacia) y cada `ms` se junta a tres
bandas con la base (anadido en uno: entra; quitado en uno sin tocar en el
otro: sale; cambiado en los dos: gana el que dirige). Sin base, se queda
todo lo de los dos. Salida en orden de `ms`. `marcadores` vacio quita la
clave; `marcas` vacio queda `[]`. Si algun elemento no tiene `ms`, regla
general (la lista entera).

### 7.2 El `texto` de una carpeta, pagina a pagina

Si los dos `texto` son un objeto JSON con `formato` y una lista `paginas`
con `id` distintos, se juntan como objeto (las `paginas` por `id`, como
las figuras de un lienzo) y se escribe en JSON compacto. Una base que no se
entiende (texto vacio) cuenta como `{"paginas":[]}`. Si no, por parrafos.
Va despues de la de la tabla (`celdas`).

### 7.3 `pins/draw/figuras.json`, figura a figura

Cuando el plan de archivos dice `Fusionar` para esa ruta: traer el suyo
entero, juntar las dos listas por `id` sin base (lo de los dos se queda),
escribirlo aqui y mandarlo de vuelta. Si alguna no se entiende, gana el
tocado mas tarde, como cualquier archivo. Para el cable sigue siendo un
archivo binario (no entra en `esTexto`).

## 8. Lista de tareas

### Android

- [x] `Disco.alcance`: anadir la `ruta` exacta de cada mensaje (hecho en
      Android desde el 1-oct, `alcanceCon`).
- [ ] Conservar `grabacion`, `capturas`, `video`, `enGrabacion`,
      `marcadores`, `estadoDelTexto` (ya viajan en lo desconocido;
      comprobar que el modelo no los tira al reescribir el mensaje).
- [x] Reproducir el `.mp4` de un `ARCHIVO`; opcional, «Estudiar la clase».
- [ ] Al llegar `transcripcion` nueva, repintar la letra de la nota.
- [x] `Fusion.kt`: reglas 7.1 y 7.2; `Protocolo.kt`: regla 7.3.
- [x] Recargar `BibliotecaStore` cuando llega `figuras.json`.
- [x] `<d>.vistas.json` en el alcance de cada dibujo (ver §3).
- [x] Burbuja de `NOTA` con Markdown y album de fotos.
- [x] Carpeta: ensenar el `MINIAPP` «carpeta» (opcional), no tocar su `texto`.
- [x] Carpeta de grabacion (§5.1, Android v0.140-v0.143): conservar `tipo`, `voz`, `video`,
      `duracionMs` y, en cada pagina, `ms` y `adjunto` (si el modelo de la
      mini-app solo guarda el `texto` tal cual, ya esta). Opcional: una
      tarjeta por grabacion y no ensenar sueltos los mensajes que estan en
      una carpeta.
- [ ] Datos moviles: si se quiere, no traer archivos grandes fuera de la
      Wi-Fi (se ofrecen otra vez en la vuelta siguiente).
- [ ] El movil como camara de la reunion del PC (`camara1`):
      [movil-como-camara.md §9](movil-como-camara.md). Conservar `clips` en
      la nota y `fuente`, `deAparato`, `tomadaEn` en lo que llega.

### Mac, iPad e iPhone (`apple/`)

- [x] `apple/nucleo/pixpin-sincro`: reglas 7.1, 7.2 y 7.3, copiadas de
      `crates/` (`cargo test -p pixpin-sincro` en `apple/`).
- [ ] `apple/nucleo/pixpin-proyecto` aun no tiene `clase.rs` ni
      `carpeta_de_paginas.rs`: copiarlos cuando la app quiera ensenar
      marcadores o carpetas (para sincronizar no hacen falta: los campos
      viajan igual). Al copiar `carpeta_de_paginas.rs`, ya trae la carpeta
      de grabacion (§5.1).
- [ ] La app: reproducir `.m4a`/`.mp4`, pintar la nota rica, recargar la
      biblioteca de figuras al llegar.

## 9. Donde esta probado

`crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs`, PC contra un
movil de mentira (`DiscoAndroid`) por un socket con el protocolo y el
cifrado de verdad:

- `una_clase_grabada_en_el_pc_llega_al_movil_con_voz_video_capturas_y_el_texto_de_despues`
- `una_clase_grabada_en_el_movil_llega_al_pc_con_sus_ficheros_y_su_texto_de_despues`
- `la_biblioteca_de_figuras_viaja_y_guardar_una_en_cada_aparato_no_pierde_ninguna`
- `una_carpeta_de_paginas_viaja_y_una_pagina_en_cada_aparato_deja_las_dos`
- `una_carpeta_de_grabacion_viaja_y_un_adjunto_en_cada_aparato_deja_los_dos`
- `una_nota_rica_del_pc_llega_al_movil_con_sus_fotos`
- `las_vistas_de_los_marcadores_de_un_lienzo_viajan_con_sus_marcas` (de antes)

Y las reglas sueltas en `crates/pixpin-sincro/src/fusion.rs` (pruebas
`un_marcador_en_cada_aparato_…`, `una_pagina_anadida_a_la_carpeta_…`,
`la_biblioteca_de_figuras_guardada_en_los_dos_…`).

## 10. La foto del chat (25-oct-2026)

El circulo de cada chat puede llevar **una imagen**, **un emoji sobre un
degradado** o **las iniciales sobre el color elegido**. Viaja como un campo
mas del `Proyecto` (no del mensaje): `avatar`. Sin version nueva del
protocolo; para un aparato que aun no lo conozca es un campo desconocido que
**tiene que conservar** (regla 1), o cada vuelta veria el proyecto cambiado.

### Campo

```json
"avatar": {"tipo": "imagen", "ruta": "pixpin:files/guardados/pc/AGJ36629KT/avatar-1761400000000.png", "cuando": 1761400000000}
"avatar": {"tipo": "emoji", "emoji": "🏗️", "color": "#FFCD6A", "color2": "#FFA85C", "cuando": 1761400000000}
"avatar": {"tipo": "iniciales", "color": "#72D5FD", "color2": "#2A9EF1", "cuando": 1761400000000}
"avatar": {"tipo": "iniciales", "cuando": 1761400000000}
```

| Clave | Que es |
|---|---|
| `tipo` | `imagen`, `emoji` o `iniciales`. Otro valor: pintar como `iniciales` y **conservarlo tal cual** (con sus claves). |
| `emoji` | Un emoji (puede llevar selector de variante, `U+FE0F`). Solo con `emoji`. |
| `color`, `color2` | `#RRGGBB`: el degradado de arriba abajo del circulo, detras del emoji o de las iniciales. Sin `color2`, liso. Sin ninguno, el color de siempre (el que sale del codigo unico). |
| `ruta` | Solo con `imagen`: el fichero, con ruta portatil (en el disco del movil, su ruta local, como `pdfOrigen`). |
| `cuando` | Milisegundos desde 1970 de cuando se puso. Informativo. |

- **«Volver a lo de siempre» se escribe**: `{"tipo":"iniciales","cuando":…}`
  sin colores. No se borra el campo: asi la fusion ve un cambio y no una
  ausencia.
- Sin campo `avatar`: lo de siempre (en el PC, ademas, el `logo.png` de antes
  si esta en la carpeta del proyecto; ese sale ya como `tipo: imagen`).

### Fichero

| Que | Donde lo deja el PC | Ruta portatil | Formato |
|---|---|---|---|
| La imagen | `<proyecto>/avatar-<ms>.png` | `pixpin:files/guardados/pc/<chat>/avatar-<ms>.png` | PNG cuadrado, 256 px de lado como mucho, ya recortado |

Viaja con el proyecto porque su `Proyecto` la nombra (`Disco.alcance` recoge
las rutas portatiles del texto del proyecto). **Un nombre nuevo cada vez**
(`avatar-<ms>.png`): una foto nueva es un fichero nuevo, nunca el mismo
cambiado. Al cambiarla, la vieja se borra de la carpeta (sin lapida: ya
nadie la nombra). En Android cualquier carpeta de la regla 2 vale (lo natural,
`files/guardados/`), con el mismo nombre.

### Choques

El `Proyecto` se junta campo a campo (`Mezcla.kt`): cambiada en un solo
aparato, pasa; cambiada en los dos a la vez, gana el proyecto tocado mas
tarde. En Android, como al archivar, cambiar la foto da el proyecto por
tocado (`tocado = ahora`). El PC no mueve `tocado` de un proyecto nacido
alli (es su fecha de creacion, a proposito): su foto pasa igual, porque
solo cambio en un lado.

### Lo que hace el que recibe

1. Guarda `avatar` con el proyecto, tal cual.
2. Pinta: con `imagen`, el fichero recortado al circulo (si aun no llego,
   lo de siempre mientras tanto); con `emoji`, el emoji grande (~52 % del
   circulo) sobre el degradado; con `iniciales`, las iniciales en blanco
   sobre el degradado. En todos los sitios del avatar: la lista de chats,
   la cabecera, la ficha del chat, la busqueda. La chapa de chat compartido
   va **encima**.
3. Para elegirla, lo mismo que el PC: imagen (de la galeria o pegada, con
   recorte cuadrado que se mueve y se acerca), una rejilla de emojis con
   buscador, doce fondos (`pixpin_ui::foto_del_chat::FONDOS`, los siete de
   Telegram y cinco mas) y «Como siempre».

### Tareas

- [x] Android: `avatar` en `Proyecto` (todos los campos opcionales, y lo
      desconocido conservado), pintarlo en la lista y en la cabecera, y la
      hoja para elegirlo desde la ficha del chat.
- [ ] Mac, iPad e iPhone: el nucleo ya lo tiene (`apple/nucleo/pixpin-proyecto/src/avatar.rs`,
      `apple/nucleo/pixpin-ui/src/foto_del_chat.rs`); falta pintarlo en Swift
      y la hoja de elegirlo.

Probado en `crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs`:
`la_foto_del_chat_viaja_con_su_imagen_y_vuelve_cambiada_desde_el_movil` y
`el_logo_de_antes_empieza_a_viajar_como_foto_del_chat`.

## 11. El orden de la vuelta y lo nacido despues de elegir (26 y 27-oct-2026)

Sin mensajes nuevos ni version nueva del protocolo: solo cambia **en que
orden** pide las cosas quien dirige y **que chats** entran en una vuelta sin
preguntar. Un aparato que no lo sepa sigue entendiendose con los demas.

### 11.1 Lo nacido despues de elegir

Al contestar «¿Que sincronizar?», ademas de `sincro/elegidos/<otro>.txt` (lo
marcado), se escribe **`sincro/elegidos/<otro>.ofrecidos`**: los ids de
todos los chats que salian en la lista (con los marcados dentro), uno por
linea. No viaja. En la vuelta sin preguntar (la automatica, «Sincronizar
con todos»), van **lo marcado y los chats que no estan en `.ofrecidos`**
(nacieron despues). Lo desmarcado a proposito sigue fuera. Sin
`.ofrecidos` (elegido con una version de antes) se hace como siempre: solo
lo marcado. Al volver a preguntar, lo nacido despues sale marcado.

| | Android | PC |
|---|---|---|
| Fichero | `files/sincro/elegidos/<otro>.ofrecidos` | `<raiz>/sincro/elegidos/<otro>.ofrecidos` |
| Escribir | `Disco.guardarElegidos(otro, chats, ofrecidos)` | `Disco::guardar_elegidos_de` |
| Leer | `Disco.elegidosAhora(otro, candidatos)` | `Disco::elegidos_ahora` |

### 11.2 El orden de los archivos

Quien dirige mueve **primero los mensajes de todos los chats**, en este
orden de chats: los que tienen una **lista de tareas** (o una tabla del
chat), los **compartidos** con otras personas y luego los demas **del que
tiene menos mensajes al que tiene mas** (a igualdad, por id). Despues **los
archivos de todos los chats juntos**, por estos niveles (los de Android,
`Prioridad.kt`, desde el 27-oct; antes el PC tenia cuatro numeros):

| Nivel | Que |
|---|---|
| Tareas | lo que nombra una lista de tareas (menos lo pesado) |
| Compartidos | lo de un chat compartido con otra persona (menos lo pesado) |
| Archivos | `pdf doc docx xls xlsx ppt pptx odt ods odp rtf epub csv txt zip png jpg jpeg webp gif heic heif bmp svg tif tiff dwg dxf` |
| Lienzos y notas | `*.excalidraw.gz`, `*.croquis.gz`, `tablas/…`, `notas/…`, `.md`, `.json` y lo demas |
| Pesado | `mp4 m4v mov mkv webm avi 3gp 3gpp m4a mp3 wav ogg oga opus aac flac amr wma`: **pesado siempre** |

**Grupos**: lo de una carpeta del chat (una clase grabada: su voz, sus
capturas, su video) va junto, y lo de un mensaje suelto (su adjunto y lo
anotado) tambien. El grupo va en el nivel **mas tardio** de sus piezas y
pesa **lo que pesan todas**. Orden: nivel del grupo → peso del grupo →
grupo → peso del archivo → ruta. Cada chat se **cierra dos veces**: tras
sus mensajes (lo acordado de los mensajes ya no se pierde si la vuelta se
corta en un video) y tras sus archivos. Un archivo grande cortado a medias
**se reanuda** con quien sabe (`reanudar1`, §13.5).

Codigo: `prioridad::{nivel_de, ordenar, ordenar_chats, grupos_de_archivos}`,
`PreparadoArchivos::{en_orden, pieza}`, `Sesion::aplicar_pasos`,
`vuelta::una` (`crates/pixpin-sincro`, copiado en `apple/nucleo/pixpin-sincro`).
Probado en `prioridad::pruebas` (los vectores de `PrioridadTest`) y en
`diario_de_verdad::cortada_a_mitad_del_video_las_tareas_y_el_chat_compartido_ya_llegaron`.

### 11.3 Versiones vinculadas en los chats compartidos (Android v0.145)

Del lado de la sincronizacion no hay nada nuevo: la version es **un
mensaje mas** con claves de mas y sus propios ficheros
([docs/versiones-en-chats-compartidos.md de Android](https://github.com/1xmanMAX/pixpin_pro_android/blob/ccr-e5daeb2c-m4d1f4/docs/versiones-en-chats-compartidos.md)).

| Que | Formato |
|---|---|
| `versionDe` | cadena: el **codigo unico** (`uid`, o el sacado del `id`) del original |
| `versionN` | entero: 1, 2… Solo una etiqueta («Version 2»); se puede repetir |
| `respondeA` | el `id` del original: un PixPin que no sepa de versiones la ensena como respuesta |
| `id`, `uid`, `aparato`, `numero`, `cuando` | nuevos, del que la hace |
| Fuera | `fijado`, `emoji`, `recuerdaEn`, `enBuzon`, `unido`, `letra`, `origen`, `recibidoDe`, `hojaDelTexto` |
| Adjunto | copia en otra ruta: `guardados/<hora>_v<n>_<nombre sin el prefijo de hora>` (en el PC, `archivos/` de su carpeta) |
| Lienzo o pagina | dibujo copiado a `dib-<id nuevo>` (`pins/draw/dib-<id>.excalidraw.gz`, en el PC `lienzos/dib-<id>.excalidraw`), con su `.marcas` y `.vistas.json` |
| Foto anotada | su dibujo a `foto-<id nuevo>` |
| Lo anotado | `pins/draw/anot-<uid original>*` copiado a `anot-<uid nuevo>*` |

Ninguna regla de fusion cambia: el original no se toca. Kotlin no escribe
las dos claves si no hay (`@EncodeDefault(NEVER)`), y el PC las conserva
(en `resto`, o tipadas en `cuaderno::Mensaje`). Probado en
`una_version_hecha_en_el_movil_llega_al_pc_con_sus_copias_y_el_original_no_cambia`
y `una_version_hecha_en_el_pc_llega_al_movil_con_los_nombres_de_android`.
Reenviar a otro chat **quita** `versionDe` y `versionN` (Android
`reenviado`): el original se queda en el chat de antes.

## 12. Compatibilidad con Android (27-oct)

Revisadas todas las ramas de `pixpin_pro_android` (`main` = v0.145.0,
`ccr-e5daeb2c-m4d1f4` con 19 commits mas, `union-24-oct`,
`ccr-4d2f0b52-3ktfkq`; lo de `union-24-oct` ya esta dentro con otros
hashes). Lo de sincronizar desde el 20-oct:

| Que | Android | PC | ¿Se entienden? |
|---|---|---|---|
| Protocolo, version, mensajes (`hola`, `catalogo`, `archivos`, `pon`, `dame`, `parche`, `base`…) | sin cambios | sin cambios | si |
| Fusion: `marcas`/`marcadores`, carpeta, `figuras.json` (§7) | `Fusion.kt` igual byte a byte (eee70717) | `fusion.rs` | si |
| Carpeta de grabacion (`tipo`, `voz`, `video`, `duracionMs`, `ms`, `adjunto`) | `CarpetaDePaginas.kt` (ea0a779e) | `carpeta_de_paginas.rs` | si |
| Lo de dentro de una carpeta, sin su tarjeta | sale suelto (`sueltosDelChat`) | se esconde solo con la carpeta en el mismo chat (`marcar_lo_de_dentro`) | si (solo pantalla) |
| Foto del chat (`avatar`, `avatar-<ms>.png`, borrar la vieja) | `FotoDelChat.kt` (a437419a) | `avatar.rs` | si |
| Ruta con `)` en el alcance | desde el 1-oct | si; y el movil de pruebas desde hoy | si |
| Chats nacidos despues de elegir (`.ofrecidos`) | `elegidosAhora` (ea0a779e) | `elegidos_ahora` (hoy) | si |
| Archivos de pequeno a grande en un chat | `aplicarArchivos` (ea0a779e) | `en_orden` (hoy) | si |
| Prioridades entre chats (checklist 16) | `Prioridad.kt` (261b67a5) | `prioridad.rs`, los mismos niveles y grupos (§11.2) | si (solo orden) |
| Sincronizar por diario, `diario1`/`reanudar1` | 261b67a5 | hecho (§13) | si, y con quien no lo dice, como siempre |
| Versiones vinculadas (`versionDe`, `versionN`, copias) | 4d57eb71 | se conservan y viajan (pruebas de hoy); la regla al editar, la ventana del chat | si |
| Boton de sincronizar que gira (`VueltasEnMarcha`) | a437419a | 0470294 | no viaja |
| Pantalla de aparatos en tarjetas (e5ae6844) | solo pantalla | — | no viaja |
| Chats compartidos como conversacion (0486b8d6, `EntrePersonas.kt`) | puerto de `conversacion.rs` | `pixpin_ui::conversacion` | no cambia nada de lo que viaja |
| «Diario de cambios» (checklist 13: `docs/sincronizacion-por-diario.md`, `docs/guia-sincronizacion-para-windows.md`) | 261b67a5 | §13 | si |

### Lo que Android tiene que hacer

- [x] **Prioridades (§11.2)**: hecho en Android (`Sesion.vuelta`,
      261b67a5) con niveles y grupos; el PC tiene ahora los mismos.
- [ ] **Reenviar** ya quita `versionDe`/`versionN`; comprobar que «Copiar a
      otro chat» y «Enviar a mis aparatos» (`suelto`) tambien.
- [x] **El diario de cambios** (checklist 13): hecho en Android
      (261b67a5) y en el PC (§13), negociado con `capacidades` en `hola`.
      Lo que un aparato no conoce se contesta «No sé qué es «…»» en los dos
      lados y la conversacion sigue (probado).
- [x] «Una grabacion cuenta por su peso total» (checklist 16): los grupos
      de §11.2, en los dos.

## 13. Sincronizar por diario (27-oct)

El usuario: «comprueba si se implementó la sincronización inteligente; es
muy importante, acelera la sincronización entre Windows, Android y los
demás». En Windows **no estaba**. Ahora si, como Android (`261b67a5`, «lo
quieto ni se mira, por prioridad, reanudar y automatica por defecto»),
siguiendo su guia para el PC (`docs/guia-sincronizacion-para-windows.md`) y
su especificacion (`docs/sincronizacion-por-diario.md`). **Nunca es la unica
verdad**: si el diario falta, esta roto o no lo explica todo, la vuelta hace
lo de siempre. Un fallo del diario cuesta tiempo, nunca datos.

### 13.1 El diario (`<raiz>/sincro/diario/`)

| Fichero | Que | Formato |
|---|---|---|
| `diario.jsonl` | una linea por cambio, con un numero `n` que solo crece | cabecera `{"libro":"<uuid>","piso":0,"v":1}`; apuntes `{"n","t","k","c","o","a","r","h"}` con `k` ∈ `m p f l x` (como Android) |
| `huellas.tsv` | lo que vio el ultimo barrido | `ruta \t bytes \t fecha_ns \t nodo \t crc32|-` |
| `indice.tsv` | de que chat es cada archivo (de cada alcance calculado) | `#<libro>`, luego `chat \t ruta␁ruta…` |
| `cursores/<id>.json` | hasta donde quedo igual cada chat con cada aparato | `{"libro","chats":{"<chat>":{"n","alli","sello"}},"verificado","vueltas"}` |
| `cerrojo` | cerrojo entre procesos (se crea solo si no existe; uno de mas de 10 s es de un proceso muerto) | — |

En el PC las rutas del diario son las de este equipo relativas a su raiz
(`proyectos/<ficha>/lienzos/d1.excalidraw`): el diario no viaja. El **nodo**
es el indice de archivo de NTFS con el numero de volumen
(`(vol=…,idx=…)`, `pixpin_sistema::archivos::nodo_de`); en Mac, el inodo.

- **Se apunta despues de escribir**: lo que escribe la sincronizacion
  (`Disco::escribir_archivo_de`, `escribir_reanudable`, marcas, lapidas,
  `vista::DiscoPc` al aplicar mensajes, guardar un proyecto o borrar un
  chat, con las huellas de su cuaderno, su `proyecto.json` y su ficha) y el
  chat del PC (`cuaderno::anadir`, `anadir_lineas`, `reemplazar`,
  `cambiar`, `quitar`).
- **El barrido** (`Disco::barrer`, al empezar cada vuelta en los dos lados,
  con el aparato ocupado): tamano, fecha en ns y nodo de cada archivo de lo
  que viaja, y CRC32 de lo escrito hace menos de 3 s (y menos de 8 MB). En
  el PC, el cuaderno de una carpeta es un **mensaje de ese chat**, su
  `proyecto.json` **su proyecto**, la ficha de cada chat en el indice
  (`@chat/<chat>`, sin `tocado` ni `resumen`) **su proyecto**; lo demas, un
  archivo que el indice (o su carpeta) dice de quien es. Solo
  `sincro/borrados.jsonl` y `sincro/chatsborrados.jsonl` son «todo» (`x`).
- **Se compacta** pasadas 4 000 lineas si hay el triple que cosas; con mas
  de 200 000 cosas se olvida la mitad mas vieja y sube el `piso`.
- **Una linea rota, el indice perdido o de otro libro** → `x`. Sin diario o
  sin cabecera → otro libro (los cursores dejan de valer).

### 13.2 Los cursores y lo quieto

Al cerrar un chat, quien dirige lee su ultimo numero `N` **antes** de mirar
y, si los dos lados quedaron **identicos** (plan de mensajes vacio con lo
acordado, plan de archivos vacio sin contar lo vedado y el mismo proyecto
canonico), guarda `{n: N, alli: huella del inventario del otro, sello: el de
lo acordado}`; si no, lo quita. Quien responde apunta el suyo con el numero
que tenia al contestar el ultimo `inventario` de ese chat, si `base` llega
con `entero`. Un chat esta **quieto aqui** si tiene cursor, el libro es el
de ahora, `piso ≤ n ≤ ultimo` y no hay apuntes de ese chat (ni `x`) despues.
**Cada 25 vueltas o cada dia** (o con el reloj hacia atras) se mira todo
(las vueltas recortadas de tareas no cuentan). Salir del grupo borra los
cursores.

### 13.3 Lo negociado

| Peticion | Respuesta | Quien |
|---|---|---|
| `hola` con `"capacidades":["diario1","reanudar1"]` | lo mismo en el suyo | los dos; se usa la **interseccion**; `version` sigue en **4** |
| `{"t":"novedades"}` | `{"quietos":[…]}` (con una persona, solo los chats compartidos) o `{"error":"No sé qué es «novedades»"}` | solo a quien dijo `diario1` |
| `{"t":"base",…,"entero":true}` | `{}` | apunta el cursor del que responde |
| `dame` con `desdeByte` | `{"bytes":<lo que falta>,"desdeByte":…}` y los trozos desde ahi | solo con `reanudar1` |
| `pon` con `reanudar:true`, `bytes` entero y `resumen` del entero | **antes de los trozos** `{"desdeByte":<lo que ya tiene>}` (o un error, y no sale ningun trozo) | solo con `reanudar1` |

Un chat quieto **en los dos** no se mira (ni una peticion). Con quien no
tiene diario (un PC o un Android de antes): `inventario` + `archivos` y, si
el `selloDeBase` es el del cursor y la huella de lo que dice tener
(§7.1 de la guia: `<sena>=<resumen>[!]`, `--`, proyecto canonico, `--`,
`<ruta>=<resumen>`, SHA-256) es la del cursor, tampoco se sigue. **Lo que un
aparato no conoce se contesta con un error en los dos lados**, sin cortar
la conversacion.

### 13.4 La copia que no se rehace

`Disco::hacer_copia_si_cambio` (`copias::hacer_si_cambio`): si el chat no
se ha tocado desde la ultima vez que se miro (`<raiz>/copias/diario.tsv`:
`chat \t libro \t n`), la ultima copia sigue valiendo y no se relee nada.

### 13.5 Fiabilidad

| Caso | Que hace ahora el PC |
|---|---|
| Se corta a mitad de un archivo de ≥ 1 MiB (no de texto) | lo llegado se queda en `<ruta>.parcial` con `<ruta>.parcial.json` `{"resumen","largo","cuando"}` y `fsync`; con `reanudar1` la vuelta siguiente pide o manda solo lo que falta y comprueba el **SHA-256 del entero** antes de renombrarlo a su sitio. Otra version: se tira y empieza de cero. Mas de 7 dias: se tira |
| Escribir lo recibido | temporal `.sincro`, `fsync` y renombrar **encima** de un paso |
| Disco lleno | antes de recibir se mira el sitio (lo que llega + 16 MB); si no cabe o se llena a mitad, lo que llega se lee y se tira (la conversacion no se descuadra), queda `saltado` y la vuelta avisa «No queda espacio en este aparato…» |
| La aplicacion murio a mitad | al empezar la vuelta siguiente, fuera los `*.sincro` |
| Trozo de mas | «Llegó más de lo prometido»: corta sin escribirlo |
| Tramo repetido o fuera de orden | solo el **primero** que no descifra es «otro codigo de grupo»; los demas, `ErrorCanal::TramoDanado` |
| Dos a la vez | «ocupado» se reintenta en **1–4 s con azar**; un fallo de verdad a los 5 s, 10 s, 20 s… hasta 5 min (±25 %), sin esperar a los 10 min (las cuentas en `pixpin_sincro::reintentos`; el reloj, `automatica::Reloj` con `reintento`, `acabo` y `no_empezo`) |
| Al volver la red | Windows avisa del cambio de direccion (`pixpin_shell::red`, `NotifyAddrChange`) y se sondea ya (`presencia::sondear_ya`, `Reloj::volvio_la_red`) |

### 13.6 Automatica por defecto

Ya lo era en el PC (`[sincro] automatica = true` de fabrica) con los
aparatos del grupo y con las personas de los chats compartidos; lo que el
usuario apaga se respeta. Ahora ademas reintenta como Android y sondea al
volver la red (§13.5).

### 13.7 Lo que gana (medido)

En las pruebas (compilacion de depuracion, dos «aparatos» en la misma
maquina; lo que cuenta es la proporcion):

| Prueba | Sin diario | Con diario | |
|---|---|---|---|
| 31 chats de un movil de mentira (30 × 120 mensajes, 6 lienzos, 6 archivos de 64 KB), sin cambios | 38 383 ms · 1 395 437 bytes | 1 819 ms · 4 158 bytes | ×21 en tiempo, ×335 en bytes |
| Lo mismo, con un mensaje y un lienzo cambiados en dos chats | 38 122 ms · 1 425 482 bytes | 4 800 ms · 128 154 bytes (29 quietos) | ×8, ×11 |
| PC con 110 chats, sin cambios | 56 515 ms · 1 320 lecturas de cuadernos | 703 ms · **0 cuadernos leidos** (dirigiendo y respondiendo) | ×80 |

### 13.8 Lista de tareas de la guia (§8 de Android)

- [x] 1. Orden por prioridad: mensajes de todos (tareas, compartidos, del
      mas ligero), archivos de todos por niveles y grupos, cerrar al final.
- [x] 2. Fiabilidad: renombrar encima tras `fsync`; mirar el sitio y
      leer-y-tirar; limpiar `*.sincro`; reintentos con azar y espera que se
      dobla; primer tramo distinto de los demas; cortar si un trozo se pasa.
- [x] 3. Diario + barrido + indice + cursores, y saltarse un chat con un
      Android sin diario por la huella de su inventario.
- [x] 4. `capacidades:["diario1"]`, `novedades` y `entero`.
- [x] 5. `reanudar1`.
- [x] 6. Automatica tambien con las personas de los chats compartidos (ya
      lo era) y al volver la red.
- [x] 7. Versiones vinculadas: ya estaban (§11.3).

Pruebas (`crates/pixpin-sincro/src/diario_de_verdad.rs`,
`diario::pruebas`, `prioridad::pruebas`, `reintentos::pruebas`,
`canal::pruebas`, `crates/pixpin-proyecto/tests/sincronizar_por_diario.rs`):
la segunda vuelta no mira nada desde los dos lados, un cambio solo mira su
chat, lo escrito sin apuntar llega, un lienzo se encuentra por el indice,
sin diario o con una linea rota no se pierde nada, apagado como antes, la
verificacion entera, con uno de antes por su inventario, reanudar en los
dos sentidos, otra version a medias, sin sitio no se descuadra, restos,
tramo repetido y trozo de mas, dos a la vez, borrado contra cambiado,
idempotencia, la copia de un chat que no cambio no se rehace, cortada a
mitad del video ya trajo las tareas y el chat
compartido, el orden por niveles, lo que no se conoce se contesta, el PC con
el Android nuevo y con uno de antes en los dos sentidos, y 110 chats sin
leer ni un cuaderno.

Mac, iPad e iPhone: el nucleo (`apple/nucleo/pixpin-sincro`,
`pixpin-proyecto`, `pixpin-sistema`) lleva lo mismo.
