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
