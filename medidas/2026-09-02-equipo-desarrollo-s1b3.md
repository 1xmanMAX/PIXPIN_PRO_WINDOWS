# Medición S1-B3 (scroll, cuentagotas y «Seleccionar todo») — equipo de desarrollo — 2026-09-02

**Equipo:** el de siempre (16 GB, 4 núcleos físicos, Intel UHD + MX250, monitor
3000×2000 al 150 %, nivel `Completo`). **Binario:** `target/release/pixpinmax.exe`,
rama `s1b3-scroll-cuentagotas`, modo portable. **Método:** entrada sintetizada
DPI-aware; un Bloc de notas con 400 líneas numeradas como página objetivo; tiempos
y motivo de parada del log; portapapeles leído de vuelta; capturas revisadas.

**Aviso:** el usuario estaba trabajando en Word durante las pruebas, con Word
delante del Bloc de notas y activo. La rueda va a la ventana bajo el cursor, así
que la captura con scroll recorrió **Word**, no el Bloc de notas. No invalida la
prueba (cosió el documento de Word), pero sí las cifras de «líneas seguidas».

## Lo verificado

1. **`Ctrl+Alt+S` → overlay → arrastrar → `Enter`** → el overlay se oculta, la rueda
   llega a la ventana de debajo y se cose: `captura con scroll terminada pasos=145
   alto=2254 fin=Tiempo ms=30026` en la primera vuelta (Word, 36 páginas: nunca
   se acaba, y el tope de 30 s hizo su trabajo). El pin muestra el documento
   cosido sin cortes visibles (capturas revisadas). La imagen queda **en el
   portapapeles y como pin** (D77) ✓
2. **Parada por fin de página**: con la regla de «dos capturas idénticas seguidas»
   (añadida tras la primera vuelta), `fin=FinalDePagina ms=10137` cuando la
   ventana no se mueve ✓
3. **`Ctrl+Alt+D`** sobre el Bloc de notas (tema oscuro) → `color copiado
   color=#292929`, el portapapeles contiene `#292929` y el overlay se cierra ✓
4. **Panel «Seleccionar todo»** (pedido por el usuario a mitad de la fase): con
   `Ctrl+Alt+X` aparece arriba y centrado; un clic deja la pantalla entera
   seleccionada («3000×2000 (0, 0)», tiradores en los bordes) y lista para
   `Enter`; `Ctrl+A` hace lo mismo ✓

## Rendimiento

| Métrica | Medido |
|---|---|
| Paso de scroll (captura + asentar + rueda) | ≈ 200 ms por paso (145 pasos en 30 s) |
| Parada por fin de página | 3 pasos quietos: ≈ 10 s incluidos los tiempos de asentado |
| Overlay visible (sin cambios) | 51–98 ms |
| Cosido puro | 14 pruebas en CI; una página de 1000 filas en 6 pasos en < 1 ms |

## Lo que se corrigió durante la prueba

- **La captura no terminaba nunca al final de una página**: el cosedor devuelve
  «incierto» ante una banda lisa (el blanco tras el último párrafo) y no «sin
  movimiento». Ahora dos capturas idénticas seguidas cuentan como quieto: los
  píxeles no mienten.

## Notas honestas

- Los 200 ms por paso son en su mayoría el asentado (60 ms entre capturas hasta
  dos iguales) y la captura por WGC (el duplicador está tomado por el escritorio
  remoto en este equipo). En un equipo con duplicador libre bajarán.
- «Líneas seguidas sin repetir ni saltar» no se pudo verificar sobre el Bloc de
  notas por la interferencia; la prueba pura de la cabecera y el pie fijos y el
  documento de Word cosido son la evidencia disponible.
- El suelo real (i3 3.ª gen, 4 GB) sigue pendiente.
