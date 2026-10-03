# Páginas vivas, anchos de foto y enlaces a hojas — cómo hacerlo igual en Android

30-sep-2026. Lo hecho en el PC (H12, editor de notas Markdown) y lo que haría falta
en PixPin Android para que las notas se vean y se comporten igual en los dos lados.
Nada de esto obliga a cambiar el móvil: **todo lo que escribe el PC ya se ve bien
en Android tal como está hoy** (ver «Compatibilidad»). Lo de abajo es para que el
móvil, además, *actualice* las páginas vivas, quite el ancho del pie y abra las hojas.

## 1. Qué escribe el PC en la nota

| Qué | Markdown | Fichero |
|---|---|---|
| Foto pegada (Ctrl+V), soltada o elegida | `![Captura](pixpin:files/guardados/pc/<chat>/notas/<hora>-Captura.png)` | copiada a `notas/` del proyecto, como `Adjuntos.importar` |
| Foto con el ancho cambiado | `![Planta\|320](…)` — el ancho en **píxeles a 96 ppp** tras la última barra del texto (forma de Obsidian); la columna mide 720, y a ese ancho o más no se escribe nada | — |
| Página viva (una hoja como imagen que se actualiza) | `![Planta baja](pixpin:files/…/notas/vivo-<hoja>.png)` | `notas/vivo-<hoja>.png`, `<hoja>` = **código único** de la hoja (`uid` de su mensaje y de su hoja en `proyecto.json`, diez signos de `Codigos.SIGNOS`) |
| Enlace a una hoja | `[Planta baja](pixpin:hoja=<proyecto>/<hoja>)` — `<proyecto>` es el código único del proyecto (`Ficha.uid`), `<hoja>` el de la hoja | — |

Por qué así y no de otra forma:

- **Ni `{width=…}`, ni `<img width>`, ni título `"…"` detrás de la ruta.**
  `Markdown.MEDIO` (`^!\[([^]]*)]\(([^)]*)\)$`) se queda con todo lo de dentro del
  paréntesis como ruta: un título o un `?ancho=` dejan la ruta sin extensión de foto
  (`claseDeMedio` la toma por ARCHIVO) y `BitmapFactory.decodeFile` no la encuentra.
  Además `Rutas.enTexto` (sincronización) corta la ruta en `"` o `)`, no en el
  espacio: con un título el fichero no viajaría. HTML el móvil solo entiende `<table`.
- **La página viva se reconoce por el nombre del fichero** (`vivo-*.png`): no hace
  falta ninguna marca nueva; para todo lo demás es una foto.
- **Códigos únicos y no ids**: el id de una hoja cambia al recibirla en otro aparato
  (`Hoja.uid`, Android v0.50); el código no.

## 2. Compatibilidad hoy, sin tocar Android

- Foto pegada / soltada: igual que una de `Adjuntos.importar`. ✔
- Página viva: el móvil ve **la última copia** que pintó el PC (viaja con la nota
  como cualquier `pixpin:files/`). ✔ No se actualiza sola en el móvil.
- Foto con ancho: se ve, a todo lo ancho (como todas), y **el pie dice `Planta|320`**
  en vez de `Planta`. Es lo único que se nota.
- Enlace `pixpin:hoja=`: se ve como enlace; tocarlo hoy no lleva a ningún sitio útil.

## 3. Qué haría falta en Android (por orden de utilidad)

### 3.1 Quitar el ancho del pie y respetarlo (`motormd/Markdown.kt`, `MarkdownText.kt`)

En `medioDe`, separar `alt` en texto y ancho, igual que `md_imagen::partir_alt` del PC:

```kotlin
private val ANCHO = Regex("""^(.*?)\s*\|(\d{1,5})$""")
// en medioDe:
val (alt, ancho) = ANCHO.find(m.groupValues[1])?.let {
    it.groupValues[1] to it.groupValues[2].toIntOrNull()?.takeIf { n -> n > 0 }
} ?: (m.groupValues[1] to null)
```

`MarkdownBlock.Medio` gana `val ancho: Int? = null`; `MedioUi` usa
`Modifier.widthIn(max = ancho.dp)` (o la fracción `ancho / 720f` del ancho del pin)
en vez de `fillMaxWidth()` cuando lo hay, y el pie enseña solo el texto. Al guardar,
volver a escribir `|ancho` (el editor no debe perderlo: `EditorVivo` ya conserva el
texto del renglón tal cual).

### 3.2 Actualizar las páginas vivas en el móvil

El PC lo hace así (`apps/pixpin/src/notas_md/paginas_vivas.rs`); en Android cabe en
un `PaginasVivas.kt` dentro de `motormd/`:

1. **Al abrir la nota** (y al volver a ella, `onResume`), recorrer sus medios cuya ruta
   acabe en `vivo-<código>.png`.
2. **Buscar la hoja** por su código: primero en el proyecto de la nota y si no en todos
   (`ProyectosRepository` → `Proyecto.hojas.first { it.uid == código }`). Si no está:
   dejar la imagen y poner encima el aviso «La hoja ya no está: es la última copia».
3. **¿Está vieja?** Comparar la fecha de lo que hace la hoja con la del PNG
   (`File.lastModified()`):
   - lienzo: `lienzos/<dibujo>.excalidraw`;
   - página de PDF: su lienzo (si tiene) y el PDF;
   - foto: el fichero y su dibujo;
   - tabla / nota: viven en el cuaderno, que cambia con cada mensaje → comparar
     además un hash de su texto con el de la última vez (en memoria), para no
     repintar por un mensaje ajeno.
4. **Pintarla** con lo mismo que las miniaturas y «Compartir como imagen»:
   `DrawExport.aBitmap(scene, scale)` sobre su papel (`scene.backgroundColor`, con
   `Renderer(dark = DrawTheme.esDeNoche(...))`, igual que ya hace la vista previa);
   la página de PDF con `PdfRenderer` y lo anotado encima (`PaginaAnotada`); una tabla
   o una nota, como en `HojaDeCompartir`. **1440 px de ancho como mucho** (lo mismo que
   decodifica `cargarImagen`).
5. **Escribir de un tirón**: a `vivo-<código>.png.tmp` y `renameTo` — la nota puede
   estar leyéndolo y la sincronización no debe llevarse uno a medias.
6. **Releer** en la nota: `MedioUi` tiene `remember(medio.ruta) { cargarImagen(...) }`;
   añadir la fecha del fichero a la clave (`remember(medio.ruta, lastModified)`) para
   que se relea al cambiar.
7. Todo en un `Dispatchers.IO`, nunca en el hilo de la interfaz; el PC lo mira cada
   2 s mientras la nota está abierta, en el móvil basta con al abrir y al volver.

Si los dos aparatos repintan la misma página viva, gana el último que sincroniza:
es la misma hoja pintada igual, así que da igual cuál.

### 3.3 Meter una página viva y un enlace desde el móvil

- Menú `/` (`Comandos.kt` / `Bloques.kt`): dos bloques nuevos, «Página de un
  proyecto» (`pagina`, `hoja`, `lienzo`, `page`, `sheet`, `canvas`) y «Enlace a una
  hoja» (`enlace`, `vinculo`, `link`), como `md_comandos::CATALOGO` del PC.
- Selector: proyecto → hoja (el diálogo `paginasDe` de `MensajesActivity` sirve),
  incluyendo tablas y notas, no solo páginas y lienzos.
- La página viva: pintar el PNG ya (paso 3.2.4) en `filesDir/notas/vivo-<código>.png`
  y escribir `![Nombre](<ruta>)`. El enlace: `[Nombre](pixpin:hoja=<proyecto>/<hoja>)`.

### 3.4 Abrir

- Tocar una página viva (o su «Abrir la hoja» en la pulsación larga): `abrirHoja`
  de `ui/Proyectos.kt` con la hoja encontrada (lienzo, lector de PDF, tabla, nota).
- Tocar un enlace `pixpin:hoja=`: en el manejador de `LinkHit` de `MarkdownText`,
  antes de lanzar el `Intent.ACTION_VIEW`, si empieza por `pixpin:hoja=` buscar la
  hoja (3.2.2) y abrirla igual. Sin esto Android intenta abrir el esquema como una
  dirección y no encuentra aplicación.
- Una foto normal: ya se ve a todo lo ancho; «Ver en grande» sería el visor de fotos.

## 4. Lo que el PC no hace y por qué

- **Arrastrar una miniatura desde la tarjeta de Proyectos** a la nota: la tarjeta se
  pinta con Direct2D y no es un origen de arrastre de Windows (OLE); hacerlo sería un
  `IDataObject` + `IDropSource` nuevos para un gesto que ya cubren el selector y
  «Insertar en una nota» del menú del mensaje.
- **El ancho en el móvil**: el PC lo escribe y lo respeta; el móvil lo ignorará hasta 3.1.
