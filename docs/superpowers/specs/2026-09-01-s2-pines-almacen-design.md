# PixPin Max — S2: Pines flotantes + Almacén

**Estado:** COMPLETO — aprobado por secciones el 2026-09-01. Pendiente de revisión final del usuario.
**Método:** skill `superpowers:brainstorming` (diseño → aprobación → spec → plan → implementación)
**Documento padre:** [`2026-08-09-pixpin-pc-master-design.md`](2026-08-09-pixpin-pc-master-design.md) — cuya ordenación **este documento modifica** (D20)
**Rendimiento:** [`2026-08-31-rendimiento-equipos-modestos-design.md`](2026-08-31-rendimiento-equipos-modestos-design.md) aplica entero; aquí se añaden las puertas propias de los pines.

---

## 1. La visión, en las palabras del producto

Mantener imágenes, capturas, notas de texto, archivos y carpetas (y más adelante vídeo) en **pines flotantes**: ventanas siempre visibles, sin bordes ni cabecera, que parecen **recortes de pantalla flotando**. Es la primera de tres piezas:

1. **S2 (esta spec):** los pines y su almacén persistente.
2. **S3:** el canvas 2D de anotación —sobre pines, sobre imágenes y sobre la pantalla— con base en **Excalidraw (MIT)**, más una **interfaz con apariencia de chat** que organiza todo lo pineado y guardado: una línea de tiempo local donde cada captura, nota o archivo es un mensaje, pensada para quien gestiona su vida desde un chat. Local y sin red, como todo.
3. El resto del mapa (salidas, OCR, visor…) se conserva detrás.

### 1.1 Política de código abierto (reafirmada)

Se aceleran fases apoyándose en proyectos abiertos **solo si su licencia es MIT/BSD/Apache** (Excalidraw sí). **QuickView queda descartado como fuente de código: es GPL-3.0** y usar una línea suya convertiría PixPin Max en GPL a perpetuidad, contra las decisiones D2/D3 que `cargo-deny` hace cumplir. De QuickView se toman técnicas e ideas, y —clave— **sus mismas librerías upstream** (libjpeg-turbo, libjxl, dav1d, LibRaw: BSD/MIT), disponibles desde Rust con idéntico rendimiento. Para el vídeo futuro, la vía sin dependencias es **Media Foundation**, incluida en Windows; la evaluación fina se hará en su fase.

## 2. Decisiones

Continúan la numeración del maestro (D1-D19).

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D20 | **Orden del roadmap** | **S2 = Pines, S3 = Anotación + canvas + chat** (invierte el S2/S3 original) | Valor visible antes: capturar y pinear ya es un producto. El pin es el contenedor natural del canvas futuro |
| D21 | **Modelo de datos** | **El pin no es dueño de nada: es una vista flotante de una entrada del almacén** | Cerrar nunca pierde; reiniciar restaura; el chat de S3 será otra vista del mismo almacén, sin migración |
| D22 | **Contenidos v1** | Imagen/captura, nota de texto, archivo/carpeta. **Vídeo después** | Lo caro (decodificación, audio, controles) no puede retrasar el alma del producto |
| D23 | **Interacción base** | Mover agarrando desde **cualquier punto**; redimensión desde esquinas **siempre proporcional**; `Esc` cierra el enfocado; clic derecho abre el menú; **cero cromo** (ni bordes ni botones, tampoco al hover) | Definido por el usuario: el pin se maneja como un objeto físico |
| D24 | **Grupos v1** | Etiqueta de **color** (paleta de 8) que tiñe la sombra + **mostrar/ocultar en bloque**. Sin mover/cerrar en bloque ni nombres libres | El color ES el grupo; lo demás llega cuando haga falta |
| D25 | **Persistencia** | **Total desde el día uno**: todo lo pineado entra al almacén al crearse; los pines abiertos se restauran al reiniciar | Es D21 llevado a disco |
| D26 | **Orígenes v1** | (a) **Recorte→pin**: un atajo abre el overlay de S1-B2 y, al confirmar, el trozo queda pineado **1:1 en su sitio**; (b) **portapapeles con atajo global** (imagen, texto o archivos copiados) | El primero es el gesto insignia («arrancar un pedazo de pantalla»); el segundo cubre todo lo demás |
| D27 | **Tecnología** | Ventana por pin con DirectComposition (`WS_EX_NOREDIRECTIONBITMAP`, el stack de S1-B2) + **almacén de ficheros reales con índice JSON** | Reutiliza lo construido, cero copias al mover, y los datos del usuario son ficheros navegables. SQLite solo si el chat lo exige algún día |
| D28 | **Archivos y carpetas** | **Por referencia** (ruta), nunca copiados al almacén; imágenes y notas sí son propiedad del almacén | Pinear una carpeta de 40 GB no puede significar copiarla. La referencia rota se muestra («no encontrado»), no se oculta |
| D29 | **Presupuesto de pines** | Ver §7: 100/60 MB, primer pin < 200 ms, regla de media resolución | La memoria es contenido, no código: se regula lo que se muestra |
| D30 | **Aspecto** | Esquinas redondeadas (8 px lógicos), **sombra difusa del color del grupo** (negra suave sin grupo; más intensa en el pin enfocado), nota sobre lienzo blanco/negro según tema | Todo pin parece un recorte de pantalla elevado, sea imagen, texto o ficha |

## 3. El aspecto

### 3.1 Anatomía (común a los tres tipos)

```
      ┌─ margen transparente (la sombra vive aqui) ─┐
      │   ╭──────────────────────────────╮  ← esquinas redondeadas (8 px logicos × escala)
      │   │                              │
      │   │         CONTENIDO            │  ← imagen 1:1 / nota / ficha de archivo
      │   │                              │
      │   ╰──────────────────────────────╯
      │        sombra difusa DEL COLOR DEL GRUPO
      └──────────────────────────────────────────────┘
```

La ventana es mayor que el contenido: el margen transparente aloja la sombra **dibujada** (D2D). Windows no pinta nada del pin. La sombra se desplaza 2 px hacia abajo; la del pin **enfocado** es algo más intensa y amplia — así se sabe a quién cerrará `Esc` sin ningún borde.

**Paleta de grupos (8):** rojo, naranja, ámbar, verde, cian, azul, violeta, rosa — elegidos para teñir sin ensuciar. El color lo da el grupo; no se elige por pin. Sin grupo: sombra negra suave.

### 3.2 Los tres tipos

- **Imagen/captura** — nace **1:1 en píxeles físicos exactamente donde se recortó**. Redimensionar escala proporcional con interpolación de calidad; doble clic alterna 100 % ↔ ajustado. Si al nacer no cabe, se ajusta al 80 % del área de trabajo.
- **Nota** — texto sobre lienzo **blanco (tema claro) / negro (tema oscuro)**, misma tarjeta y sombra: un recorte de una página. En v1 nace del portapapeles y es de **solo lectura** (se lee, mueve, agrupa, copia de vuelta); la edición con cursor llega con el canvas de S3, que trae la infraestructura de texto (IME incluido). Redimensionar escala el bloque entero, como una imagen — coherente con la metáfora.
- **Archivo/carpeta** — ficha compacta: icono real del sistema + nombre + tamaño, sobre el lienzo del tema. Doble clic abre con la app predeterminada. Alto fijo; sólo se redimensiona a lo ancho.

**Tamaños:** mínimo 48×48 px lógicos; máximo el área de trabajo del monitor.

## 4. La interacción

### 4.1 Ratón

| Gesto | Efecto |
|---|---|
| Arrastrar desde cualquier punto | Mueve el pin como un objeto. Clic sin arrastre (≤ 4 px) sólo enfoca |
| Arrastrar desde una esquina (zona de 12 px lógicos) | Redimensión **proporcional**, anclada a la esquina opuesta; el cursor diagonal es el único feedback. Ficha de archivo: sólo a lo ancho |
| Doble clic | Imagen/nota: 100 % ↔ ajustado. Archivo: abrir |
| Clic derecho | El menú (4.3) |
| Imán | A menos de 8 px del borde del área de trabajo, el pin se adhiere. (Imán entre pines: llegará con «mover en bloque») |

### 4.2 Teclado (pin enfocado)

`Esc` cierra (el contenido queda en el almacén) · `Ctrl+C` copia (imagen como CF_DIB, nota como texto, archivo como ruta) · flechas / `Shift`+flechas mueven 1 / 10 px.

### 4.3 El menú del clic derecho

```
Copiar                    Ctrl+C
Guardar como…                     ← imagen y nota; en archivo: «Abrir ubicación»
Tamaño original                   ← solo imagen/nota
──────────────────────
Grupo                  ▸  ○ Sin grupo · ● Rojo … ● Rosa (8)
Ocultar este grupo                ← si el pin tiene grupo
──────────────────────
Cerrar                    Esc
Eliminar del almacén…             ← única acción destructiva; pide confirmación
```

Menú Win32 nativo (`TrackPopupMenu`, como la bandeja de S1-A), traducido por Fluent. Los grupos ocultos se recuperan desde el **menú de bandeja**: sección «Grupos ocultos → ● Verde (3 pines)».

### 4.4 Foco y orden

- Un pin **no roba el foco al crearse** (`SW_SHOWNOACTIVATE`); pinear el portapapeles no interrumpe donde escribes. El recorte→pin es la excepción natural (ya estabas en PixPin).
- Todos `TOPMOST`; entre ellos, el último clicado queda encima.
- `Esc` sólo actúa con un pin enfocado; nunca cierra «el último» a ciegas.

### 4.5 Preparado para S3

El menú ganará «Anotar», y el doble clic sobre imagen probablemente pase a abrir el editor. Movimiento y sombra no cambiarán: el canvas se montará dentro de la tarjeta.

## 5. El almacén

### 5.1 En disco: ficheros de verdad, navegables

```
<raiz>/almacen/                      ← raiz(): junto al exe en portable; %APPDATA%\PixPinMax si no
  indice.json                        ← metadatos; UNICO fichero que se reescribe
  objetos/2026/09/000041.png         ← una captura pineada
             …/000042.txt            ← una nota (UTF-8 plano)
```

- Los objetos **nunca se reescriben** (nombre por contador; sólo «Eliminar del almacén» borra).
- **Imágenes y notas: propiedad del almacén** (copiadas dentro). **Archivos/carpetas: por referencia** (D28); la ruta rota se muestra como «no encontrado».
- **Transparencia como propiedad del diseño:** la carpeta se puede abrir con el Explorador y ver lo tuyo.

### 5.2 `indice.json`

```json
{
  "version": 1,
  "grupos": [ { "id": 1, "color": "verde", "oculto": false } ],
  "entradas": [
    { "id": 41, "tipo": "imagen", "creado": "2026-09-01T18:40:12Z", "origen": "recorte",
      "objeto": "objetos/2026/09/000041.png", "grupo": 1,
      "pin": { "x": 1240, "y": 380, "ancho": 480, "alto": 300, "escala_por_cien": 150 } },
    { "id": 42, "tipo": "archivo", "creado": "2026-09-01T18:41:03Z", "origen": "portapapeles",
      "ruta": "D:\\Proyectos\\informe.pdf", "grupo": null, "pin": null }
  ]
}
```

- `pin: null` = vive sólo en el almacén (el chat de S3 lo enseñará); `pin: {…}` = abierto ahí. Ocultar un grupo conserva los `pin` y marca `oculto`; mostrarlo restaura cada uno donde estaba.
- `escala_por_cien` recuerda el DPI del monitor de origen; si el monitor ya no existe al restaurar, el pin va al principal manteniendo tamaño visible, **sujetado al área de trabajo** con la lógica pura de S1-B1 — nunca se restaura fuera de pantalla.
- **Escritura segura:** temporal + `rename` (la disciplina que el `ExcalidrawStore` del Android aprendió a golpes), con retardo de 300 ms tras el último cambio — arrastrar no escribe 60 veces por segundo.
- `serde` con `#[serde(default)]` y claves desconocidas ignoradas: misma regla de compatibilidad que los ajustes.

### 5.3 Grupos

Un grupo **es su color**: `id` + color de la paleta + `oculto`. Crear grupo = asignar el primer pin a un color libre; un grupo sin pines desaparece del índice. Nombres libres: cuando el chat los pida.

### 5.4 Lo que el almacén NO hace (a propósito)

Ni miniaturas precalculadas, ni búsqueda, ni cifrado, ni límites con limpieza automática: todo eso es del chat de S3, que decidirá con datos reales. Cero red.

## 6. Arquitectura

| Crate | Capa | Qué gana en S2 |
|---|---|---|
| `pixpin-geom` (L0 puro) | L0 | Imán de bordes; redimensión proporcional anclada a esquina; **recolocación de pines restaurados** (monitor desaparecido → principal, dentro del área de trabajo). Tests con disposiciones inventadas |
| `pixpin-store` (L2 puro) | L2 | Módulo `almacen`: índice + objetos, temporal+rename, retardo 300 ms. Puro `std::fs`, testeable con directorios temporales |
| `pixpin-pin` (L2, `unsafe` auditado) | L2 | La ventana (`PixPinPin`, WndProc **propio** — lección S1-A), el dibujo (tarjeta + sombra vía `pixpin-render`), el menú contextual, y un módulo **puro** `estado.rs` con la máquina de interacción, probado sin escritorio. (No puede vivir en `pixpin-ui`: L2 no depende de L3) |
| `apps/pixpin` (L4) | L4 | Atajos nuevos; recorte→pin (el overlay de S1-B2 gana un tercer `ModoConfirmacion`); restauración al arrancar; bandeja con «Grupos ocultos» |

**Dos reglas de dibujo que sostienen el presupuesto:**

- **La sombra se dibuja una vez y se cachea** como bitmap por (tamaño, color, enfocado). Mover un pin no repinta nada — DirectComposition mueve el visual; redimensionar repinta como mucho una vez por fotograma.
- **La textura de un pin vive al tamaño que se muestra.** Reducir un pin re-escala y libera; el 100 % se recarga del PNG del disco. El disco es el almacén; la GPU sólo tiene lo visible.

## 7. Rendimiento — puertas de la fase

| Métrica | `Completo` | `Ligero` (suelo: i3 3.ª gen, 4 GB) |
|---|---|---|
| CPU con 10 pines quietos | 0 % | 0 % |
| RAM con 10 pines típicos (~400×300) | **< 100 MB** | **< 60 MB** |
| Mover un pin | ≤ 1 fotograma del refresco real | ídem |
| Arranque: primer pin restaurado visible | **< 200 ms** | **< 200 ms** |
| Arranque: 10 pines restaurados | **< 500 ms** | anotar medido (1 hilo de trabajo) |

- La decodificación al restaurar va **en paralelo en el pool** (núcleos físicos − 1) y cada pin aparece en cuanto su imagen está — no se espera al último. La sensación de arranque es el primero, no el décimo.
- **Regla de presupuesto de texturas:** si las texturas visibles superan el tope del `Presupuesto` de `pixpin-nivel`, los pines menos recientemente enfocados bajan a **media resolución** hasta volver a entrar, y se recargan nítidos del disco al enfocarlos. En el i3, donde cada byte de textura sale de los 4 GB, esta regla es la diferencia entre fluido y ahogado.
- Qué ata cada número (para no perseguir fantasmas): el 0 % es el suelo absoluto; mover está atado al refresco del panel; la RAM está atada a los píxeles mostrados (el código entero pesa ~1 MB); el arranque está atado a disco + decodificación PNG. **La velocidad restante sale de no hacer trabajo, no de más lenguaje.**

## 8. Pruebas

- **CI, sin escritorio:** máquina de estados del pin (gestos→efectos, con casos negativos), imán, proporcional, recolocación; almacén completo (ida y vuelta del índice, temporal+rename ante panic simulado, referencia rota).
- **`--ignored` con escritorio:** crear/destruir un pin no mata la app (la mina de S1-A, con test propio otra vez); la sombra cacheada se regenera al cambiar grupo/foco/tamaño; restauración real con reinicio del proceso.
- **Manual:** recorte→pin→mover→agrupar→ocultar→mostrar→reiniciar→reaparecer, y el flujo de portapapeles con los tres tipos.

## 9. Criterios de aceptación de v0.2

- [ ] Recorte→pin 1:1 en su sitio (atajo dedicado)
- [ ] Pinear portapapeles: imagen, texto y archivos, sin robar el foco
- [ ] Mover desde cualquier punto; esquinas proporcionales; `Esc`; doble clic; menú completo y traducido
- [ ] Grupos: sombra de color, ocultar/mostrar en bloque desde pin y bandeja
- [ ] Todo persiste; al reiniciar, los pines reaparecen donde estaban
- [ ] El almacén es navegable con el Explorador; la referencia rota se muestra, no se pierde
- [ ] Las cinco puertas de §7 medidas y anotadas en `medidas/` (las que fallen, anotadas igual)

## 10. Subfases de implementación

- **S2-A — Almacén + pin de imagen:** módulo `almacen`, ventana del pin, dibujo con sombra, interacción básica (mover/proporcional/`Esc`/doble clic), recorte→pin, restauración. *Al cerrar S2-A ya se puede capturar y dejar flotando.*
- **S2-B — Los demás tipos y los grupos:** nota, ficha de archivo, portapapeles con atajo, menú completo, grupos con ocultar/mostrar, imán, puertas de rendimiento medidas.

Cada subfase con su plan (`writing-plans`), como en S1.

## 11. Fuera de alcance de S2

Vídeo con reproducción · edición de notas · canvas/anotación (S3) · chat/feed visible (S3) · OLE drag & drop hacia otras apps (fase propia en cuanto la base esté sólida: era «función estrella» del maestro y no se abandona, se ordena) · arrastrar desde el Explorador · mover/cerrar grupos en bloque · mini-apps · mostrar el contenido de carpetas.

## 12. Preguntas abiertas

- El **atajo** del recorte→pin y el del portapapeles: se decidirán en el plan de S2-A (candidatos: `Ctrl+Alt+F` y `Ctrl+Alt+V`), reasignables en TOML como los cuatro existentes.
- Los valores exactos de la sombra (radio de desenfoque, alfa por color) se afinarán a ojo en la primera tarea de dibujo, con capturas comparadas en la revisión.
- El puerto del canvas de S3 sobre Excalidraw definirá si la nota v1 (texto plano) migra a `.excalidraw` o se queda en `.txt` con conversión al abrir en el editor.
