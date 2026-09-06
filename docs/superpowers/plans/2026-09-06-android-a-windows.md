# Traer PixPin Android a Windows — Plan por fases

**Objetivo:** que PixPin Max deje de ser un capturador de pantalla con
anotaciones y sea la versión de escritorio de PixPin Android, empezando por
lo que hace que las dos mitades sean un solo producto.

**Informe del que sale este plan:**
`docs/investigacion/2026-09-06-android-a-windows-paridad.md`, con sus tres
inventarios por módulo.

**Regla que manda sobre todo lo demás:** el programa pesa 2,4 MB y va
dirigido a equipos con pocos recursos. Cada fase que proponga añadir peso
nativo tiene que justificarlo, y la respuesta por defecto es mirar antes qué
trae Windows. Es lo que ya se hizo con el texto de las imágenes, con el vídeo
y con el PDF.

---

## Por qué este orden

Lo primero no es lo más grande ni lo más vistoso: es **el puente**. Razones,
en orden de peso:

1. **Es lo que convierte dos aplicaciones en una.** Capturas en el móvil,
   sigues en el escritorio. Hoy son dos programas que no se hablan.
2. **Obliga a alinear los modelos de datos ahora**, cuando cuesta poco. Si se
   porta el motor primero y luego se intenta leer los ficheros del móvil, se
   descubrirá que los elementos no encajan y habrá que rehacer parte.
3. **Sale gratis el mapa de lo que falta.** Cada campo del elemento que no
   sepamos representar es una herramienta que no tenemos, señalada por el
   propio fichero.
4. Es **mucho más pequeño** que el motor: leer un ZIP y traducir un JSON.

Después van las herramientas de medir y construir, que son las que convierten
el anotador en algo de trabajo. Y el croquis 3D al final: es el más grande
después del motor, el más independiente, y el que menos se echa en falta
mientras el resto está a medias.

---

## Fase A — El puente: abrir y guardar `.pixpin`

Lo que hace que las dos mitades sean una.

- [x] **A.1 Leer el JSON de Excalidraw.** Traducir sus elementos a nuestro
      `Elemento`. Los siete tipos que ya compartimos primero; lo que no se
      entienda se conserva tal cual para no perderlo al volver a guardar.
- [x] **A.2 Abrir un `.pixpin`.** Es un ZIP: `manifest.json`,
      `proyecto.json`, `lienzos/`, `imagenes/`, `notas/`, `documento.pdf`.
- [x] **A.3 Guardar un `.pixpin`** que el móvil pueda volver a abrir. La
      prueba que importa: ida y vuelta sin pérdida.
- [x] **A.4 Leer el cuaderno.** Es JSON Lines, una línea por mensaje,
      legible sin convertir nada.

**Lo que no se puede romper:** un elemento que Windows no entienda tiene que
sobrevivir a la ida y vuelta. Si al abrir y guardar en el escritorio se
pierden las cotas de un plano, el puente es peor que no tenerlo.

---

## Fase B — Las herramientas que faltan, por familias

Veinte herramientas. Se traen por familias, no de una en una, porque
comparten maquinaria.

- [ ] **B.1 Dibujar:** rombo, imagen, flecha libre.
- [ ] **B.2 Tapar y señalar:** mosaico, números de serie, hoja.
- [ ] **B.3 Medir:** cota, escalar, escala gráfica. Es la familia que más
      cambia lo que se puede hacer con el programa.
- [ ] **B.4 Construir:** bote, recortar, extender, punto, nudo.
- [ ] **B.5 Seleccionar:** lazo y bolita.
- [ ] **B.6 El imán.** Vértices, medios, cruces y borde, con la prioridad que
      ya está decidida en el Android. Sin él, medir es adivinar.

La lógica de todas ellas es pura y se porta. Lo que hay que rehacer es la
entrada: donde el Android usa el segundo dedo, aquí va una tecla.

---

## Fase C — El cuaderno

La pata que en Windows no existe en absoluto.

- [ ] **C.1 El chat contigo mismo:** guardar fotos, ficheros y notas.
- [ ] **C.2 Proyectos:** un PDF, sus hojas anotadas y sus notas, juntos.
- [ ] **C.3 Markdown.** `motormd` se porta con cero imports de la aplicación
      y su propia prueba de frontera.
- [ ] **C.4 Notas de voz.** *Decisión pendiente del usuario:* mirar primero
      el reconocimiento que trae Windows antes de considerar los 26 MB de
      nativo del Android.

---

## Fase D — El croquis en el espacio

Lo más grande después del motor, y lo que peor se traduce.

- [ ] **D.1 El núcleo geométrico y el visor.** La proyección ya está escrita
      dos veces en el Android (Kotlin y JavaScript) con una prueba que exige
      que coincidan: es la mejor referencia para hacerla en Rust.
- [ ] **D.2 El editor.** *Aquí hace falta decidir de nuevo:* dibujar en el
      aire con un ratón no es lo mismo que con un dedo. Sus gestos son uno,
      dos, tres y cuatro dedos; eso no se traduce, se rediseña.

---

## Lo que NO entra, y por qué

| Qué | Por qué |
|---|---|
| La bola flotante | Existe porque Android no tiene atajos globales. Windows sí, y ya los usamos. |
| La notificación permanente | Lo mismo: es el precio de vivir en Android. |
| El rechazo de palma | No hay palma. |
| Los atajos de lápiz más dedos | No hay lápiz ni dedos. |
| `onnx` como está | No es un módulo del proyecto: es `sherpa-onnx` copiado, con 1.226 de sus 1.579 líneas muertas. |

---

## Cómo se trabaja cada tarea

Lo mismo que en el plan de paridad con PixPin, que ya funcionó:

1. La lógica pura primero, en su crate, con pruebas que corran sin escritorio.
2. La ventana después.
3. Puerta completa antes de fusionar: formato, análisis estático sin avisos y
   todas las pruebas.
4. Una rama y una solicitud de cambios por tarea, con el porqué escrito.
