# Una sola grabación en el PC (29-oct-2026)

Lo que PixPin Android hizo el 29-oct-2026 (v0.152.0, `docs/grabacion-unica-android.md`
del móvil), traído a «Grabar clase o reunión» del PC. Las palabras del usuario (en el móvil):

> «solo debería haber una opción de grabar, no tres (presencial, pantalla, audio más vídeo);
> que en la misma barra me dé la opción de grabar la pantalla o pausarla pero se siga grabando
> el audio; tomar foto con la cámara; capturar la pantalla; pausar… una opción que funcione para
> los tres escenarios». «Los iconos de la barrita están muy aglomerados; el botón de play y pausa
> sobra: en el tiempo, donde está el puntito rojo, le damos clic y pausa (el punto pasa a pausa),
> y si lo mantengo presionado, que se termine». «Una opción de pantalla completa donde vea la
> lista de lo que voy a decir, arriba las opciones y abajo la transcripción, más grande».

## Lo que ve el usuario

1. **Preparar**: ya no hay selector de modos. Arriba **«Empezar grabando la pantalla»** (con su
   calidad: Ligero, Normal, Nítido, en una ficha a la izquierda del interruptor), el micrófono y
   el sonido del equipo, **la cámara de Foto y Clip** (un clic pasa a la siguiente), el chat, los
   temas y su duración prevista. Lo de la vez anterior se recuerda (`modo-de-grabar.txt`: con la
   pantalla al empezar, «pantalla»; en persona, sin el sonido del equipo).
2. **La barra**, de izquierda a derecha:
   - **el reloj «● 12:34»**: un clic pausa o sigue (el punto rojo pasa a ❚❚); **mantenerlo
     (0,65 s) termina** y guarda. Ya no hay botones de pausa ni de terminar;
   - el nivel del sonido;
   - **Foto** (la cámara del equipo a su resolución entera; se abre a la primera foto o clip, en
     una clase por pantalla no se enciende), **Captura** (la pantalla entera donde está la barra,
     al instante; **mantenerla o el clic derecho** pide una zona), **Pantalla** (enciende o apaga
     el vídeo de la pantalla; en rojo mientras graba; **el audio sigue siempre**) y **Clip** (un
     clip con la cámara; en rojo con lo que lleva);
   - «☑ 2/5» si hay temas, **⛶ la pantalla completa** y ✕ tirar (dos clics);
   - abajo, lo que acaba de pasar o la pista, y «📱 Cámara del móvil» si hay móviles.
3. **La pantalla completa** (⛶; ⛶ o Escape vuelve): arriba el reloj grande (igual: clic pausa,
   mantener termina), «Terminar», Foto, Captura, Pantalla y Clip en grande, **A− / A+** (lo grande
   del texto, que se recuerda en `letra-de-grabar.txt`), **Descartar** (dos clics) y ⛶; en medio
   **lo que voy a decir** (los temas, para marcarlos a mano; los de Jev con su ✓ y minuto); abajo
   **lo que se dice** (la transcripción en vivo, frase a frase con su minuto, la última abajo).
   Como la ventana no sale en ninguna foto ni vídeo (`excluir_de_capturas`), **«Captura» desde
   aquí fotografía lo de debajo sin salir** (en el móvil hay que salir).

## Cómo va por dentro

- `apps/pixpin/src/ventana_chat/grabar_clase.rs`: la barra y Preparar sin modos (`Accion::Reloj`,
  `Pantalla`, `Completa`, `LetraMenos`/`LetraMas`; mantener en `flotar`, constante `MANTENER`).
- `grabar_clase/pantalla.rs`: **el reparto del sonido** (`Reparto`). La grabadora manda siempre su
  copia del sonido mezclado (la copia solo se puede pedir al abrirla) y un hilo la da al trozo de
  vídeo que esté en marcha o la tira. Cada vez que se enciende «Pantalla» empieza un **trozo**
  (`Trozo`, el motor de siempre, `video.rs`); apagarla suelta su canal y el trozo cierra su MP4.
- `grabar_clase/completa.rs`: la pantalla completa.
- `grabar_clase/escucha.rs`: `Vuelta::Frase(ms, texto)` con cada frase ya oída (las 200 últimas en
  `Estado::frases`). **Solo se enseña**: el texto de la nota sale al terminar, entero. La escucha
  es la de los temas y lo demás de la reunión (con la clave de Jev y un reconocedor); sin ella,
  «Lo que se dice» explica por qué.
- **Al guardar**: el trozo que empezó con la grabación es **el vídeo de la nota**, como siempre
  (`video: {id}`, `enGrabacion.ms = 0`); los demás entran como **clips** en su minuto (`clips` de la
  nota y `enGrabacion {de, ms, duracionMs}`), «Clase … · pantalla N.mp4», **sin `fuente`**. Las
  fotos y clips de la cámara llevan `fuente: "camara"` como antes. El móvil usa la regla de «en los
  primeros 5 s» porque allí el permiso de la pantalla tarda; en el PC el vídeo del principio empieza
  en el acto.
- **El diario** (`pixpin_proyecto::grabacion_a_medias`): el vídeo del principio va en `video`,
  como siempre; los demás trozos, en `clips` con `pantalla: true` (campo nuevo, solo en el diario
  local, `#[serde(default)]`: los diarios de antes se leen igual). Si algo se corta, la
  recuperación los mete como clips sin `fuente`.
- **Disco casi lleno**: se cierra el trozo que vaya y «Pantalla» ya no enciende otro (el audio
  sigue).

## Toda grabación se pasa a texto, aunque PixPin se cierre (punto 47 del móvil)

Android lo hizo el 28-oct con una cola en el disco (`ColaDeTexto`). En el PC
(`grabar_clase/cola_de_texto.rs`): cada nota de una clase guardada se apunta en
`voz/textos-pendientes/<id>.json` (`{proyecto, id, intentos}`) **antes** de pasarla a texto, también
al salir por la bandeja (que termina sin pasar a texto) o sin reconocedor todavía. Se borra al
escribir el texto o con «Dejarlo sin texto». **Al arrancar**, el hilo que recupera las grabaciones
cortadas retoma lo que quedó: si la nota ya tiene texto (se pasó a mano desde el chat) o ya no está,
fuera; si aún no hay reconocedor, a la próxima; tras 3 fallos del reconocedor con ella, se deja (queda
«Pasar a texto» en la nota). Es local de cada aparato: no viaja.

## Que se note y que se pueda terminar (punto 52 del móvil)

- **Foto** brilla en verde cuando sale la foto (de la cámara o del móvil) y dice «Foto…» mientras se
  hace; **Captura** brilla con la suya; **Clip** va en rojo con lo que lleva; **Pantalla**, en rojo
  mientras graba. La línea de abajo dice el minuto («Foto guardada en el 12:34»).
- **Terminar a la vista**: la barra está siempre encima (no depende de la ventana del chat); el
  reloj se mantiene para terminar y **se va llenando de rojo** mientras se mantiene, así que se ve
  que va a terminar y cómo; la pantalla completa tiene «Terminar» con su nombre, y «Salir» de la
  bandeja termina y guarda. El móvil, además, vibra y suena: en el PC no hay sonido de obturador.

## Compatibilidad con el móvil

El formato no cambia: la misma nota `grabacion: "clase"` con `capturas`, `video` y `clips`. Lo único
nuevo es que en una misma grabación del PC hay a la vez imágenes y clips con y sin `fuente` (antes
cada modo daba unos u otros), que el móvil ya lee: es lo mismo que hace él desde v0.152.0.

## Lo que no se trajo, y por qué

- **La notificación** del móvil (Foto, Captura y Terminar a la vista): en el PC la barra está siempre
  encima, y la bandeja ya dice «Grabando…».
- **«Ver la cámara»** (la mirilla) de la pantalla completa: la cámara del PC no tiene vista previa en
  la barra.
- **El vídeo de la sala** («Más opciones» del móvil: la reunión presencial grabada entera con la
  cámara): en el PC ese papel lo hacen Clip y el móvil como cámara; un vídeo largo de la cámara del
  equipo queda para otro día.
- Las fichas de la reunión (tareas y eventos propuestos por Jev) siguen en la barra; en la pantalla
  completa no se enseñan (se ven al volver).
