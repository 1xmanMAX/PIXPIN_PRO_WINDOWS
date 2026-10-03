# Documentos, mensajes del chat y audios dentro de una nota — cómo hacerlo igual en Android

1-oct-2026. Lo hecho en el PC (H12, editor de notas Markdown) para meter en una nota
documentos, mensajes del chat (con su burbuja, su hora y su código) y audios con su
transcripción, y lo que haría falta en PixPin Android para verlos igual. Como con las
páginas vivas (`2026-09-30-paginas-vivas-android.md`), **todo lo que escribe el PC ya
se ve bien en Android tal como está hoy**: se usa solo lo que `Markdown.kt` ya sabe leer.

## 1. Qué escribe el PC en la nota

| Qué | Markdown | Fichero |
|---|---|---|
| Documento de fuera (menú `+`/`/` «Documento», soltar, pegar ficheros) | `![Plano.pdf](pixpin:files/guardados/pc/<chat>/notas/<hora>-Plano.pdf)` en un renglón suyo | **copiado** a `notas/` del proyecto, como `Adjuntos.importar` (o `adjuntos/` junto a un `.md` suelto) |
| Audio de fuera («Audio», soltar) | `![nota.m4a](pixpin:files/…/notas/<hora>-nota.m4a)` | copiado igual |
| Nota de voz del chat («Del chat», «Insertar en una nota») | `![Nota de voz](pixpin:files/guardados/pc/<chat>/archivos/voz-….m4a)` y debajo su transcripción `[0:00] …` / `[0:21] …` (párrafos separados por un renglón en blanco) | **enlazado**: el fichero del mensaje del chat, no una copia |
| Cualquier otro mensaje del chat (texto, archivo, foto, lienzo) | `[Pedir la grúa para el jueves](pixpin:mensaje=<proyecto>/<mensaje>)` en un renglón suyo | — (enlazado al mensaje) |
| Hoja enlazada | `[Planta baja](pixpin:hoja=<proyecto>/<hoja>)` en un renglón suyo (lo de siempre) | — |

`<proyecto>` es el código único del proyecto (`Ficha.uid`) y `<mensaje>`/`<hoja>` el
código único del mensaje (`Mensaje.uid`, diez signos de `Codigos.SIGNOS`), los mismos en
los dos aparatos.

### Por qué así (leyendo `Markdown.kt`)

- **Documentos y audios como `![alt](ruta)`**: `Markdown.medioDe` toma un `![alt](ruta)`
  solo en su renglón como `MarkdownBlock.Medio` y **la extensión decide la clase**
  (`claseDeMedio`: imagen, vídeo, audio o archivo). `MarkdownText.MedioUi` ya pinta un
  ARCHIVO como tarjeta (icono, nombre, extensión) y un AUDIO con su botón de tocar
  (y con `LocalAudioDeLaNota`, el mismo reproductor que la letra). Un `[x](ruta)` sería
  un enlace de texto: peor en el móvil.
- **La ruta sin paréntesis ni comillas**: `MEDIO` toma todo lo de dentro del paréntesis
  como ruta y `Disco.enTexto` (sincronización) corta en `"` y `)`; por eso el nombre de
  la copia pasa por `adjuntos::nombre_limpio` (sin `()[]` ni espacios). Con
  `pixpin:files/` el fichero viaja con la nota (`alcance_de`).
- **La transcripción como `[m:ss] texto`**: es lo que escribe el propio móvil al
  transcribir (`Transcriptor.conTiempos`, `MensajesStore.apuntarTranscripcion`:
  `![audio](voz.m4a)` y el texto debajo), y su editor ya salta el audio de la nota al
  tocar un párrafo que empieza por su minuto (`minutoDelParrafo`, `AudioDeLaNota.saltar`).
- **Un mensaje como enlace `pixpin:mensaje=`**: `parseInline` lo lee como `SpanKind.LINK`
  y se ve su texto (la primera frase del mensaje, sin corchetes ni marcas). Nada se pierde
  y no hay marca nueva que el móvil no entienda. No lleva `pixpin:files/`, así que la
  sincronización no busca ningún fichero en él.

## 2. Compatibilidad hoy, sin tocar Android

- Documento copiado: tarjeta de ARCHIVO con su nombre (el `alt`) y su extensión. ✔
- Audio copiado o del chat: tarjeta de AUDIO que suena al tocarla; los párrafos con
  minuto saltan en él (el primer audio de la nota). ✔
- Nota de voz del chat: el fichero es el del mensaje (`archivos/…`), que ya viaja con el
  chat. Si el mensaje se borra, el móvil ve la tarjeta sin fichero (la del PC dice
  «No está en este equipo»). ✔
- Mensaje enlazado: se ve como enlace con su primera frase; tocarlo hoy no lleva al
  mensaje.
- Hoja enlazada: igual que antes (enlace).

## 3. Qué haría falta en Android para verlo igual que en el PC

1. **El mensaje como su burbuja** (`MarkdownText`, bloque nuevo): un párrafo que es
   *solo* un enlace `pixpin:mensaje=<p>/<m>` se pinta con la burbuja del chat
   (`BurbujaDeMensaje` de `MensajesActivity`, colores de `ColoresDelChat`): busca el
   mensaje por `uid` en el proyecto `<p>` (`ProyectosRepository` por `Ficha.uid`) y
   pinta lo que es (texto, foto, fila de archivo con `IconoDeArchivo`, onda de voz, hoja
   con su miniatura), con la chapa del código (`#47·K7Q2`) y la hora abajo a la
   izquierda. Si ya no está: «Mensaje borrado» con el texto del enlace, apagado.
   Tocarlo abre el chat del proyecto en ese mensaje (lo que hace el buscador por código).
2. **La hoja enlazada como su burbuja**: un párrafo que es solo `pixpin:hoja=…` igual,
   con la miniatura de la tira de Proyectos; tocarlo abre la hoja.
3. **La tarjeta del ARCHIVO como la fila del chat**: el `IconoDeArchivo` de su color
   (en vez del icono genérico `Description`), el tamaño («1.2 MB · PDF») y abrir con el
   lector propio al tocar (PDF, Word, EPUB), como la burbuja del chat.
4. **El AUDIO con el reproductor del chat**: el de `BarraDelReproductor` (play, barra,
   tiempo «0:12 / 1:05», velocidad) dentro de la tarjeta; los minutos de la
   transcripción pintados como chapas (`[`/`]` escondidos) y el párrafo que suena con
   fondo, como la pantalla de la letra. Sin transcripción, el botón «Pasar a texto»
   (`Transcriptor`) que escribe los `[m:ss]` debajo del audio. Hoy los minutos van al
   *primer* audio de la nota; el PC los manda al audio que tienen más cerca por encima
   (con uno solo es lo mismo).
5. **Meterlos desde el editor**: en el menú `/` ya están «Audio» y «Archivo»
   (`Bloques.kt`); falta «Del chat» (lista de mensajes del proyecto, del más nuevo al más
   viejo) y que «Insertar en una nota» del chat escriba esto mismo para cualquier
   mensaje (hoy solo las hojas como página viva en el PC).

## 4. Dónde está en el PC

- `crates/pixpin-notas/src/incrustados.rs`: qué renglón es qué, cómo se escriben, las
  marcas de tiempo y el contrato con la aplicación (`Ficha`, `Medios`); 10 pruebas.
- `crates/pixpin-notas/src/incrustados/pintar.rs`: la burbuja, la tarjeta y el
  reproductor pintados con las medidas y colores del chat.
- `crates/pixpin-notas/src/editor/incrustados.rs` (+ `pruebas.rs`): meterlos (nacen ya
  pintados, con el control congelado), el ratón, el latido del reproductor, «Pasar a
  texto» y la transcripción que entra debajo.
- `apps/pixpin/src/notas_md/incrustados.rs` (+ `pruebas.rs`): qué dice cada mensaje,
  los mensajes del proyecto, el reproductor (`audio.rs`), Whisper (`voz.rs`) y abrir.
