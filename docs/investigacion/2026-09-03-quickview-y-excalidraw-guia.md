# QuickView y el plugin Excalidraw de Obsidian como guía

**Fecha:** 2026-09-03 · **Petición del usuario:** «clona estos repositorios y
úsalos como guía porque hacen muy bien su trabajo».

## 0. Licencias: qué se puede tomar de cada uno

| Proyecto | Licencia | Qué se puede usar |
|---|---|---|
| [QuickView](https://github.com/justnullname/QuickView) (visor de imágenes, C++/Win32) | **GPL-3.0** | Solo **técnicas** y decisiones de diseño. Ni una línea de código |
| [obsidian-excalidraw-plugin](https://github.com/zsviczian/obsidian-excalidraw-plugin) (TypeScript) | **AGPL-3.0** | Solo **técnicas** y comportamiento. Ni una línea de código |

PixPin Max es MIT y `cargo-deny` rechaza GPL y AGPL en CI. Las dos son
copyleft fuerte: incorporar su código obligaría a relicenciar todo el
proyecto. Lo que sigue está descrito con nuestras palabras y se
reimplementa desde cero. El núcleo de Excalidraw (`excalidraw/excalidraw`)
sí es MIT y es el que ya se portó en `pixpin-motor2d`; **el plugin de
Obsidian no lo es**.

Copias locales para consulta (fuera del repositorio, en el scratchpad de la
sesión): `referencias/quickview` y `referencias/obsidian-excalidraw`.

## 1. QuickView: cómo se consigue que un visor no dé tirones

Lo importante no es que dibujen rápido, es que **casi nunca dibujan**.

1. **Paneo y zoom sin redibujar.** No re-renderizan al mover ni al ampliar:
   suman la traslación o la escala al visual del compositor y confirman.
   La textura es la misma. El contenido solo se vuelve a dibujar cuando la
   interacción para.
2. **La superficie es del tamaño en pantalla, no de la imagen.** Renderizan
   a (tamaño mostrado × zoom) con tope en 1:1, y solo recrean la superficie
   si el tamaño deseado se pasa del actual por más de unos píxeles: una
   histéresis contra el reasignar y liberar continuo.
3. **Bandera de interacción con antirrebote de ~150 ms.** Mientras se
   interactúa, filtro barato y sin suavizado; al parar, repintado de
   calidad.
4. **Repintado por capas sucias.** Visuales separados para imagen, interfaz
   estática e interfaz dinámica, cada uno con su marca. Mover el ratón
   repinta solo la capa dinámica.
5. **Animaciones del propio compositor**, curva cúbica de salida de unos
   90 ms: interpola el compositor en su hilo, la aplicación no genera
   fotogramas intermedios.
6. **Filtro según la escala**: vecino más cercano en pixel-art y en 1:1
   exacto (cuidan que el 100 % sea exactamente 1.0, porque una desviación
   mínima activa el filtro lineal y emborrona), lineal o bicúbico si no.
7. **Descodificación en dos carriles**: uno rápido que saca la miniatura
   embebida o descodifica escalado para tener algo en pantalla en
   milisegundos, y otro pesado con un pool que se regula según la latencia
   medida y el tipo de disco. Cada trabajo lleva un identificador de
   generación: al cambiar de imagen, todo lo viejo se descarta al vuelo.
8. **Imágenes enormes**: pirámide de tiles de 512 px con nivel de detalle
   elegido con histéresis, predicción del viewport por la velocidad del
   arrastre y descodificación por región del fichero mapeado.
9. **El arrastre de la ventana se delega al sistema** (se suelta la captura
   y se le dice que es la barra de título): lo mueve el gestor de ventanas,
   sin repintado ni retardo.
10. **Un HUD de métricas** (tiempos de descodificación y de dibujo, memoria,
    estado de los hilos). Sin medir no se sabe qué cambio sirvió.

## 2. Plugin Excalidraw: qué hace bien la anotación

1. **El texto es un elemento vinculado a un contenedor**, no algo dentro de
   una forma: se apuntan mutuamente. De ahí salen el ajuste de línea al
   ancho del contenedor, que la caja crezca y encoja con el texto, la
   alineación vertical y horizontal, y la etiqueta pegada a una flecha.
2. **Texto suelto con ancho fijado** arrastrando con la herramienta de
   texto, sin necesidad de una forma de apoyo.
3. **Modificadores con significado uniforme**: mayúsculas restringe
   (ángulos a pasos, cuadrado y círculo, y al redimensionar escala la letra
   en vez de reajustar líneas), alt duplica al arrastrar y añade puntos a
   una línea, control desactiva el imán mientras se dibuja.
4. **Bloqueo de herramienta** para dibujar varias figuras seguidas sin
   volver a la paleta, y doble toque del lápiz para alternar el borrador.
5. **Estilos recordados**: color, relleno, grosor, estilo de trazo,
   rugosidad, opacidad, fuente y tamaño quedan como valores por defecto del
   siguiente elemento. Además, «copiar los estilos de este elemento».
6. **Bloquear elementos**: una captura anotada se inserta bloqueada, para
   dibujar encima sin moverla sin querer.
7. **Guías de alineación a otros objetos** e imán a puntos medios.
8. **Selección múltiple, grupos, marcos, rotación y editor de puntos** de
   una línea, con puntos medios sugeridos que se materializan al arrastrar.
9. **Preajustes de pluma** como datos puros: adelgazamiento, suavizado,
   streamline y afilado independiente de inicio y fin.
10. **Referencia, no bytes**: el elemento guarda un identificador; los
    binarios viven fuera del documento, con caché en dos niveles e
    invalidación por fecha de modificación. El recorte de imagen es no
    destructivo y reversible.
11. **LaTeX editable**: la fórmula se renderiza a imagen, pero lo que se
    guarda junto al identificador es el **fuente**, así que reeditar
    reabre el TeX y sustituye el render.
12. **Guardado**: cola con números de revisión, autoguardado con
    antirrebote, y la copia de respaldo se escribe **después** del guardado
    bueno y nunca con una escena vacía, para que un fallo no corrompa las
    dos copias.

## 3. Qué se ha hecho ya con esto

**De QuickView, contra el lag (D89–D91):**

- **D89 · Zoom por transformada de composición.** Los fotogramas
  intermedios de un zoom (la rueda animada y `Ctrl` + arrastrar) ya no
  redibujan nada: la ventana toma su tamaño nuevo con `SWP_NOREDRAW` y el
  compositor estira la textura ya dibujada. El último fotograma sí se
  dibuja, nítido. `Superficie::estirar` / `dejar_de_estirar`.
- **D90 · La sombra solo se pinta en el anillo.** Eran seis rectángulos
  redondeados del tamaño completo del pin en cada fotograma, y la tarjeta
  los tapaba casi enteros. Ahora se recortan a cuatro bandas disjuntas
  alrededor de la tarjeta (`Pintor::con_recorte`). En un pin a pantalla
  completa eso pasa de rellenar seis veces el área a rellenar solo el
  marco.
- **D91 · La tarjeta no se pinta bajo una captura opaca.** Se comprueba una
  vez al crear el pin (`ImagenRgba::es_opaca`); ahorra otro relleno del
  tamaño del pin por fotograma.

Ya estaba hecho de antes, y coincide con lo que hace QuickView: la ventana
del pin recortada al escritorio (la superficie nunca es mayor que la
pantalla) y la histéresis al crecer y compactar la superficie.

**Del plugin Excalidraw, la anotación (D92–D95):**

- **D92 · La herramienta «mano» ya hace algo.** Estaba en la paleta desde
  S3-A devolviendo «nada»: no se podía seleccionar, mover ni borrar lo ya
  dibujado. Ahora un clic elige el elemento de más arriba bajo el cursor y
  arrastrar lo mueve. El anotador sigue siendo puro y sin acceso a la
  escena: pregunta con `SeleccionarEn` y el consumidor le contesta con
  `poner_seleccion`.
- **D93 · Modificadores con el significado de siempre.** Mayúsculas
  restringe: las líneas y flechas se enganchan cada 15°, y los rectángulos,
  elipses y focos salen cuadrados o círculos sin saltar de cuadrante. Alt
  duplica al arrastrar, una sola copia por arrastre.
- **D94 · Suprimir borra lo elegido**; sin nada elegido sigue siendo
  deshacer, como antes.
- **D95 · Deshacer de verdad.** Era «borra el último elemento visible», así
  que mover algo no se deshacía y el borrador tampoco. Ahora la escena
  lleva una pila de pasos (añadido, borrado, movido) con su rama de
  rehacer, que se corta al hacer algo nuevo. Un arrastre entero es **un**
  paso, y uno que acaba donde empezó no deja rastro. El historial **no se
  guarda en el fichero**: al abrir un dibujo de ayer no hay nada que
  deshacer, que es lo que hacen los editores y evita deshacer a ciegas.
- **D96 · Se ve qué está seleccionado**: un marco azul alrededor, con
  holgura, siempre encima del dibujo. El azul no está en la paleta a
  propósito, para que se lea como interfaz y no como algo dibujado.

## 4. Plan por delante, ordenado por valor/esfuerzo

**Barato y de valor alto**

1. ~~La herramienta «mano» no hace nada.~~ Hecho (D92).
2. ~~Modificadores: mayúsculas restringe, alt duplica.~~ Hecho (D93).
3. **Estilos recordados** entre sesiones (color, grosor, opacidad).
4. **Bloqueo de herramienta** y doble toque del lápiz para el borrador.
5. **Bloquear un elemento** para anotar encima sin moverlo.
6. ~~Deshacer de verdad.~~ Hecho (D95).
7. ~~Aviso de lo seleccionado.~~ Hecho (D96).

**Coste medio**

7. Selección múltiple con tiradores de redimensionar y rotar (`angulo` ya
   existe en el elemento y no se usa).
8. Guías de alineación a objetos e imán a puntos medios.
9. Texto vinculado a una forma y etiqueta en la flecha.
10. Grupos y orden de capas completo.
11. Preajustes de pluma con los parámetros que el motor ya soporta.
12. Recorte no destructivo de la imagen de un pin.

**Caro y selectivo**

13. Editor de puntos de línea y flecha con re-enrutado.
14. Marcos y exportación por marco.
15. LaTeX guardando el fuente junto al render.
16. Pirámide de tiles para capturas de más de 50 megapíxeles.

## 5. Reglas que se adoptan como norma del proyecto

- **No redibujar durante una interacción continua**: transformar lo ya
  dibujado y dibujar de verdad al terminar.
- **Histéresis en todo umbral** que decida reasignar memoria o cambiar de
  nivel de detalle, para que no oscile.
- **Referencia, no bytes**, en todo lo que se guarde.
- **La copia de respaldo se escribe después del guardado bueno**, y nunca
  una escena vacía.
