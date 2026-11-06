# Novedades del 6-nov-2026: un solo chat para todas las asignaturas

El usuario: «además para las asignaturas que se cree un chat en la que se
agrupen todas y ahí se vaya almacenando todo de estas, solo 1 para todos».

Desde el 1-nov cada asignatura del horario tenía su lista de tareas en el
chat «Asignaturas» (`docs/novedades-2026-11-01-asignaturas-con-tareas.md`).
Ahora ese chat es **donde va todo lo de las asignaturas**, agrupado por
asignatura. Lo que entra en PinDesk (Windows) y lo que necesitan Android y
`apple/`.

**Un campo nuevo en lo que se sincroniza**: el mensaje que es de
una asignatura lleva `"asignatura": "<nombre>"`. En el PC va en `resto`, que
guarda lo que el esquema no nombra. Un mensaje sin el campo no es de ninguna
(los de antes salen igual que antes).

> **Corregido el 6-nov** (`docs/novedades-2026-11-06-sincronizar-con-android.md`
> §3.1): aquí ponía que Android lo guardaba «en `resto`» tal cual. No es así:
> el `Mensaje` de Android no tiene dónde guardar lo que no conoce y lo tira al
> leerlo (`ignoreUnknownKeys`), y lo mismo `"asignaturas": true` de su
> `Proyecto`. Android tiene que añadir `asignatura: String? = null` a
> `Mensaje` y `asignaturas: Boolean? = null` a `Proyecto`.

## 1. El chat

- **Uno solo**, el de la marca `"asignaturas": true` en su ficha del índice
  (si no la tiene, el que se llama «Asignaturas», que se adopta). No se crea
  nunca otro.
- **Está siempre que haya una asignatura**: el vigía del horario lo asegura
  al arrancar y a cada cambio de `horario.md`, aunque las listas ya
  existieran.
- **Fijo en la lista**, el tercero: «Mensajes guardados», «Asistente» y
  «Asignaturas» (`almacen::Indice::ordenadas`).
- **Con su foto: 📘** sobre un degradado azul (`avatar` de tipo `emoji`,
  `#5B9BFF` → `#2F6FE0`). Se pone al crearlo, al adoptarlo o, a uno de antes,
  la primera vez; nunca encima de una foto que haya elegido el usuario
  (también «volver a lo de siempre» cuenta como elegida). Viaja como
  cualquier foto de chat.

## 2. La etiqueta

`"asignatura": "Geometría"` en el mensaje (`pindesk_proyecto::asignatura`).

- **El nombre, no un código**: el horario no viaja, así que el otro aparato
  no tendría con qué traducir un código; el nombre se lee solo.
- Se compara **sin mayúsculas ni tildes** («geometria» es «Geometría»).
- **La lista de tareas de una asignatura no la necesita**: es de la
  asignatura cuyo nombre lleva.
- **Renombrar la asignatura** en el horario renombra la etiqueta de lo suyo
  en el chat (igual que ya renombraba su lista).
- **Quitar la asignatura del horario no toca nada**: su material sigue en el
  chat con su etiqueta, y su chip sale en gris.

## 3. Qué entra en el chat, y por qué regla

### Las clases grabadas («Grabar clase»)

La regla (`asignatura::adonde_la_grabacion`):

1. **Desde otro chat abierto a propósito** (uno del usuario, no «Mensajes
   guardados» ni el «Asistente»): se queda en ese chat, que es el que eligió.
   Si es la hora de una clase, la grabación **lleva su asignatura**, y
   «Enviar a asignatura…» la copia al chat «Asignaturas» cuando quiera.
2. **Desde el chat «Asignaturas»**: ahí. Con el chip de una asignatura
   elegido, de esa (lo eligió el usuario, manda sobre el horario); si no, de
   la clase de ahora.
3. **Desde cualquier otro sitio** (la bandeja, el atajo, Flow, «Mensajes
   guardados», el «Asistente»): **en la hora de una clase va al chat
   «Asignaturas»** con su asignatura. Fuera de clase, como siempre (el
   último chat usado primero).

«La hora de una clase» = una clase del horario en marcha, o que empieza
dentro de 10 minutos (quien llega antes abre la grabadora antes;
`asignatura::en_clase`). En la grabadora el chat elegido sale primero y dice
de qué asignatura («Asignaturas · Geometría»); se puede cambiar antes de
empezar, y la asignatura va con la grabación al chat que se elija.

Lo que se etiqueta: la nota de voz, el vídeo, cada captura, cada foto y clip,
y la carpeta de la grabación (la tarjeta que se ve en el chat). Si PinDesk se
corta, la grabación recuperada conserva su asignatura (el diario lleva
`asignatura`, `grabacion_a_medias::Diario`).

### Lo que se manda desde otro chat: «Enviar a asignatura…»

En el menú de un mensaje de cualquier chat (salvo el de las asignaturas),
con alguna asignatura en el horario. Elegida la asignatura, **se copia** al
chat «Asignaturas» con su etiqueta; el original se queda donde estaba.

- **Una carpeta va con lo suyo**: la de una clase grabada se lleva su nota,
  su vídeo y sus capturas (`asignatura::paquete`), y los enlaces entre ellos
  (`capturas`, `video`, `clips`, `enGrabacion`, las páginas de la carpeta)
  se rehacen con los `id` de las copias (`asignatura::remapear`).
- **Mandarlo dos veces no lo duplica**: la copia lleva `origen` (el código
  único del original), y si ya está allí solo se le cambia la asignatura si
  iba a otra (`asignatura::ya_esta`).

### Lo que se pone en el chat con un chip elegido

Con «Geometría» elegida arriba, lo que se escriba, se suelte, se grabe o se
cree en el chat (una nota, un archivo, un lienzo, una lista) **es de
Geometría**: el chat se lo apunta en la siguiente vuelta, entre por donde
entre. Solo lo de este equipo y puesto desde que se eligió el chip (lo que
llega del móvil en ese rato trae lo suyo).

### Las tareas

Como desde el 1-nov: `pindesk tarea … --asignatura`, Flow `p geometría …` y
la caja de Tareas van a la lista de la asignatura, que está en este chat.

### Lo que no se mueve (y por qué)

- **Lo de antes no se migra solo.** Las clases grabadas antes del 6-nov se
  quedan donde están; «Enviar a asignatura…» las copia cuando se quiera, sin
  borrar nada. Moverlas a ciegas pondría en una asignatura lo que quizá era
  una reunión.
- **Las lecciones y Anki**: una lección vive en el chat de donde salió y no
  se ve en la conversación (va en su ventana), así que las que se hagan de
  material del chat «Asignaturas» ya quedan ahí. Los mazos de Anki no tienen
  asignatura en PinDesk; no se tocan.

## 4. Agrupado por asignatura (Windows)

- **La fila de las asignaturas**, debajo de la cabecera del chat
  «Asignaturas»: «Todas» y un chip por asignatura, con un punto de su color
  del horario; las que solo quedan en el chat, en gris. Pulsar uno deja ver
  solo lo suyo (su lista de tareas, sus clases, lo que se le mandó) y lo
  pinta de su color; pulsarlo otra vez, o «Todas», lo quita. Es una fila
  sola: con muchas asignaturas, las que no caben no salen.
- **La chapa «📘 Geometría»** debajo de lo que es de una asignatura, en
  cualquier chat. Con el chip de esa asignatura elegido no sale (ya se ve).
- **«Asignatura…»** en el menú de un mensaje del chat «Asignaturas»: ponerle,
  cambiarle o quitarle la asignatura (a una carpeta, con lo suyo).
- La lupa y sus chips de etiqueta siguen igual y se combinan con el chip de
  la asignatura.

## 5. Flow Launcher

- `p geom` → la asignatura (Intro abre su lista en Tareas, como antes) y,
  detrás de la mejor, **«Geometría en el chat «Asignaturas»»**: Intro abre el
  chat con Geometría elegida en su fila.
- Pedido nuevo `abrir_asignatura` (`asignatura?`), en
  `docs/protocolo-pedidos.md`; por enlace no pregunta.

## 6. Lo que necesitan los otros aparatos

- **Android**: **obligatorio**, los dos campos en sus clases
  (`Mensaje.asignatura: String? = null`, `Proyecto.asignaturas: Boolean? =
  null`): hoy los tira al leerlos, el resumen del mensaje le sale distinto
  del del PC y, si su versión vuelve, el PC también pierde la asignatura (ver
  `docs/novedades-2026-11-06-sincronizar-con-android.md` §3.1). Después,
  recomendado, en este orden:
  1. Conservar `asignatura` al editar o reenviar un mensaje (`copy(...)` ya
     lo hace si el campo está en la clase).
  2. Reconocer el chat por `"asignaturas": true` en su `Proyecto` (el PC lo
     manda desde el 6-nov, como `"asistente": true`; no por el nombre) y
     fijarlo arriba, detrás de «Mensajes guardados» y el «Asistente». La foto
     📘 ya llega como `avatar`. Un campo de más en `Proyecto`: hay que
     conservarlo o ignorarlo, no rechazarlo.
  3. La fila de chips arriba de ese chat: las asignaturas son los valores
     distintos de `asignatura` de sus mensajes, más los títulos de sus listas
     de tareas (el móvil no tiene horario, así que sin colores o con el de
     `color_por_nombre` si se quiere igual que el PC). Lo que se ponga con un
     chip elegido, con su `asignatura`.
  4. «Enviar a asignatura…» en el menú de un mensaje: la copia con
     `asignatura` y `origen` (el código único del original), y sin
     duplicar si ya hay una con ese `origen`.
- **apple/**: lo mismo. Los textos nuevos (`asignaturas-todas`,
  `chat-asignatura-poner`, `chat-asignatura-quitar`,
  `chat-enviar-a-asignatura`, `chat-enviado-a-asignatura`) están en los
  catálogos de PinDesk; `herramientas/idiomas.py apple` no los copia hasta
  que Apple tenga esas claves en su `es`/`en`.
