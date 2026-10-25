# Manualito: sincronizar lo nuevo (23 al 25-oct-2026)

Para los agentes de **PixPin Android** y de **Mac, iPad e iPhone**
(`apple/`). Dice, funcion por funcion, que ficheros y campos hay, donde
viven, con que formato, como se resuelven los choques y que tiene que hacer
el aparato que lo recibe para ensenarlo. Asi lo que se hace en un aparato
llega a todos los del grupo por la red (Wi-Fi), en los dos sentidos.

**No hay mensajes nuevos en el protocolo** ni version nueva: todo viaja
como siempre, en mensajes del chat (sus campos, conservados en `resto`) y en
ficheros que esos mensajes nombran. Lo unico que cambia son **tres reglas de
fusion** (seccion 7), que el PC ya aplica y Android tiene que copiar para
que tambien junte bien cuando dirige el.

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
- **Android: nombres con `(`**. El PC llama «Clase (1).mp4» al segundo con
  el mismo nombre. `Rutas.enTexto` corta en `)`, asi que Android tiene que
  **anadir a `Disco.alcance` la `ruta` exacta de cada mensaje** (como hace
  `DiscoPc::alcance`, ver
  [investigacion/2026-10-01-ficheros-de-proyectos-del-pc-android.md](investigacion/2026-10-01-ficheros-de-proyectos-del-pc-android.md)).
  Sin eso no rompe nada, pero el PC le vuelve a mandar ese video entero en
  **cada** vuelta. Con videos esto es lo primero que hay que arreglar.

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

- [ ] `Disco.alcance`: anadir la `ruta` exacta de cada mensaje (nombres con
      `)`; imprescindible para los videos).
- [ ] Conservar `grabacion`, `capturas`, `video`, `enGrabacion`,
      `marcadores`, `estadoDelTexto` (ya viajan en lo desconocido;
      comprobar que el modelo no los tira al reescribir el mensaje).
- [ ] Reproducir el `.mp4` de un `ARCHIVO`; opcional, «Estudiar la clase».
- [ ] Al llegar `transcripcion` nueva, repintar la letra de la nota.
- [ ] `Fusion.kt`: reglas 7.1 y 7.2; `Protocolo.kt`: regla 7.3.
- [ ] Recargar `BibliotecaStore` cuando llega `figuras.json`.
- [ ] `<d>.vistas.json` en el alcance de cada dibujo (ver §3).
- [ ] Burbuja de `NOTA` con Markdown y album de fotos.
- [ ] Carpeta: ensenar el `MINIAPP` «carpeta» (opcional), no tocar su `texto`.
- [ ] Carpeta de grabacion (§5.1): conservar `tipo`, `voz`, `video`,
      `duracionMs` y, en cada pagina, `ms` y `adjunto` (si el modelo de la
      mini-app solo guarda el `texto` tal cual, ya esta). Opcional: una
      tarjeta por grabacion y no ensenar sueltos los mensajes que estan en
      una carpeta.
- [ ] Datos moviles: si se quiere, no traer archivos grandes fuera de la
      Wi-Fi (se ofrecen otra vez en la vuelta siguiente).

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
