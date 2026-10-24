# Banco del asistente con Haiku como cerebro (24-oct-2026)

Antes de publicar, el asistente se mide con casos reales del lienzo:

1. `volcado.yml` (un push con «[volcado]») saca de Windows lo que la app manda
   al modelo: `sistema.txt` y `herramientas.json`.
2. Un banco en Linux con el motor 2D de verdad (`pixpin-motor2d`) pone la
   plantilla del caso, ejecuta las herramientas que pide el cerebro
   (`figuras_guardadas`, `reorganizar_hoja`) y pinta el resultado a PNG.
3. Un subagente Haiku hace de modelo, con ese mismo sistema y esas mismas
   herramientas, y el resultado se califica a ojo.

Casos: «redúcelo a 10 semanas»; «10 semanas y abajo una zona gris para
anotaciones»; la misma zona «en el lateral»; agrandar a 30×30; reorganizar
los bloques del planificador mensual.

## Primera ronda (24-oct)

- Reducir a 10 semanas: bien (en su sitio, mismo diseño, sin inventar).
- Zona gris abajo / en el lateral: mal; la dibujaba suelta, fuera de la hoja
  o encima. Faltaba una forma de añadir una sección a la plantilla.
- Planificador: no supo separar Prioridades de Tareas (bloques por franjas);
  preguntó en vez de romperlo.
- Confusiones del prompt: «lienzo de 800x600» con lienzos más grandes;
  «alargar» no decía que también reduce.

Cambios: secciones configurables («personalizar», zona «notas», «lateral»),
partes con nombre en plantillas y figuras, instrucciones que enseñan a
entender el pedido y a pensar la hoja por bloques, sin quitar herramientas
por palabras.

## Segunda ronda (24-oct)

- Zona gris abajo: bien (personalizar, «Notas» a todo el ancho, dentro de la hoja).
- 10 semanas y zona lateral: bien (personalizar con «lateral»); la zona ahora
  baja hasta el pie de la tabla.
- Planificador mensual y semanal: fallaban por la app (leía las prioridades
  01–04 como filas y pedía un mes). Arreglado.
- «No me entra en una hoja»: quitó semanas sin preguntar. Nueva regla: no
  quitar contenido; ofrecer partir en hojas, apaisado o papel más grande.

Las instrucciones vuelven a ser cortas; cómo pensar una hoja por bloques va
en el resumen del lienzo solo cuando hay una plantilla.
