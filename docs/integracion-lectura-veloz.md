# Integración con Lectura Veloz (protocolo v1)

**Lectura Veloz** es la app de lectura rápida del usuario (Tauri, escritorio). Registra el
esquema `lecturaveloz://`. PixPin y Lectura Veloz se hablan **por enlaces**, en los dos
sentidos:

| Sentido | Enlace | Cuándo |
|---|---|---|
| PixPin → Lectura Veloz | `lecturaveloz://pixpin/abrir?v=1&…` | «⚡ Lectura veloz» en el menu ⋮ de los lectores |
| Lectura Veloz → PixPin | `pixpinmax://lectura-veloz?v=1&…` | De fondo, mientras se lee en Lectura Veloz |

Lo puro (armar y mirar los enlaces, qué cambia en lo leído) está en
`crates/pixpin-docs/src/lectura_veloz.rs`; lo que toca el disco y las ventanas, en
`apps/pixpin/src/lectura_veloz.rs`; el verbo, en `crates/pixpin-shell/src/orden.rs`
(`armar_lectura_veloz`). **Los nombres de los parámetros no se cambian**: los usa Lectura
Veloz tal cual. Un parámetro nuevo opcional no sube `v`; quitar o cambiar uno sí.

Todos los valores van **codificados** (`%XX` de los bytes UTF-8; todo lo que no sea letra,
cifra o `-_.~`).

## 1. De PixPin a Lectura Veloz

En el menú ⋮ del **lector de PDF** (`lector_pdf.rs`, también Word/PowerPoint en hojas) y
del **lector de documentos** (`visor.rs`: Word, EPUB, HTML, Markdown, texto) está
**«⚡ Lectura veloz»** (`lector-lectura-veloz`). Al pulsarlo:

1. Se guarda lo leído (`.pixpin-lectura` y, si es un adjunto del chat, sus `anot-<uid>…`),
   **solo si cambió**: así `t` es la fecha del último cambio de verdad, no la de pulsar.
2. Se mira que `lecturaveloz://` esté registrado (`HKCR\lecturaveloz\shell\open\command`).
   Si no lo está, el lector avisa: «Lectura Veloz no está instalada en este equipo…»
   (`lector-lectura-veloz-no-instalada`). Sin esta comprobación Windows sacaría su cuadro de
   «buscar una aplicación en la Store».
3. Se abre con `ShellExecuteW` (`pixpin_shell::abrir`):

```
lecturaveloz://pixpin/abrir?v=1&doc=<id>&ruta=<ruta>&nombre=<nombre>&tipo=<pdf|texto>
    &pagina=<n>&frac=<0..1>&fraccion=<0..1>&paginas=<n>&t=<ms>
```

| Parámetro | Qué es |
|---|---|
| `v` | Versión del protocolo: `1` |
| `doc` | Identidad estable del documento. **El uid del adjunto** si es un adjunto de un chat o de un proyecto (`pixpin_proyecto::anotado::adjunto_de`, el mismo que usan los `anot-<uid>…` que viajan al móvil); si no, **`ruta:` + FNV-1a de 64 bits** (16 cifras hex) de la ruta canónica en minúsculas, con `\` y sin `\\?\` (`lectura_veloz::id_de_ruta`) |
| `ruta` | Ruta absoluta del fichero (sin `\\?\`) |
| `nombre` | Nombre del fichero con su extensión |
| `tipo` | `pdf` si es un `.pdf`; `texto` para lo demás (también un Word abierto en hojas) |
| `pagina` | Página desde 0 (lector de documentos: 0) |
| `frac` | Parte de esa página, de 0 a 1 (lector de documentos: 0) |
| `fraccion` | Parte del documento entero, de 0 a 1 (PDF: `(pagina+frac)/paginas`) |
| `paginas` | Cuántas hojas tiene (0 si no se sabe o es de corrido) |
| `t` | Milisegundos desde 1970 del último cambio de lo leído: lo más nuevo de `<doc>.pixpin-lectura` y, de un adjunto, de `anot-<uid>.marcas`, `.voz` y `.sitio`. `0` si no hay nada |

**Desde dónde**: el marcador verde 🟢 si lo hay; si no, por donde se va.

- PDF: el 🟢 de las marcas (`y` = página + parte de su alto); si no, la vista
  (`pagina` + fracción, la cuenta de `Ajustes::pagina`).
- Documento: el verde de la voz (`voz` = `parrafo:fraccion`, se manda su fracción); si no,
  `sitio`.

## 2. De Lectura Veloz a PixPin (progreso de fondo)

```
pixpinmax://lectura-veloz?v=1&doc=<id>&ruta=<ruta>&tipo=<pdf|texto>&pagina=<n>&frac=<0..1>&fraccion=<0..1>&t=<ms>
```

(`pixpin://` vale igual si ese esquema es de PixPin Max; `pixpinmax://` lo es siempre.)
También por consola: `pixpinmax lectura-veloz --ruta <doc> --tipo pdf --pagina 4 --frac 0.25`
(la ruta puede ir suelta; sin `--doc`, el de la ruta).

### Qué se mira (todo, antes de tocar nada)

`pixpin_docs::lectura_veloz::Progreso::de_campos`, la misma regla en el enlace, en la
consola y otra vez en la app al recibir el pedido:

- `v`, si viene, es `1`.
- `doc` obligatorio, hasta 200 caracteres, sin caracteres de control. **Es informativo**:
  dónde se escribe lo decide la `ruta` (y `anotado_del_adjunto`, que sabe si es un adjunto).
- `ruta` obligatoria: **absoluta de una unidad de este equipo** (`C:\…`), ni de red
  (`\\servidor\…`), ni de dispositivo (`\\?\`, `\\.\`), ni relativa, ni con `..`, y con la
  **extensión de un documento** que PixPin lee: `pdf epub docx docm dotx dotm doc pptx pptm
  ppsx ppsm potx potm html htm xhtml md markdown txt rtf`. Al escribir, además, tiene que
  **existir y ser un fichero normal** (una carpeta o algo que no está se rechaza y no se
  crea nada).
- `tipo` obligatorio: `pdf` o `texto`.
- `pagina`, `frac`, `fraccion`, `t`: números finitos (`NaN`, `inf` o texto: rechazado). Las
  fracciones se acotan a 0..1 y la página a 0..100 000. Si faltan, 0; **en un `.pdf`,
  `pagina` es obligatoria**.

### Qué hace

**Solo cambia lo leído de ese documento** (nunca abre, ejecuta ni sobrescribe otra cosa):

- **Por hojas** (un `.pdf`, o `tipo=pdf` en un Word/PowerPoint leído por hojas): `pagina` =
  `pagina + frac`, y en `marcas` **el 🟢 pasa a** `x = 0.5`, `y = pagina + frac` (se quita
  el que hubiera; las demás marcas no se tocan, `pixpin_motor2d::marcas::con_una_sola`).
- **De corrido** (`tipo=texto` en lo que no es PDF): `voz = (-1, fraccion)` y
  `sitio = fraccion`. **El párrafo no se calcula** al recibir (habría que abrir y medir el
  documento): se guarda como **`-1:<fraccion>`**, la misma convención que el móvil
  (`voz_alta::PARRAFO_DESCONOCIDO`). Quien lo usa (el visor al escuchar o al poner el 🟢, el
  móvil) busca el párrafo que empieza más cerca de esa fracción
  (`voz_alta::parrafo_de_la_marca`); un visor abierto lo resuelve en cuanto llega.

Se escribe con `anotado_del_adjunto::escribir`: junto al documento
(`<nombre>.pixpin-lectura`) y, si es un adjunto del chat, en sus `anot-<uid>.marcas`,
`.voz` y `.sitio` (que viajan al móvil). Lo demás (zoom, letra, marcadores, tinta) queda
como estaba.

### Sin ventanas

- La copia de paso que abre Windows por el enlace (`apps/pixpin/src/orden.rs`) **no
  arranca PixPin** ni le da permiso para pasar al frente: le manda el pedido
  `lectura_veloz` (`docs/protocolo-pedidos.md`) con `mensajero::enviar_pedido`. Ese pedido
  no abre ventanas ni globo (`pedidos.rs`).
- Si PixPin **no corre** (o es una versión que no lo entiende), la copia de paso lo escribe
  ella misma y sale, sin enseñar nada.
- Un enlace mal escrito **no saca ningún cuadro** (los demás enlaces sí): el motivo va a la
  salida de errores y el código de salida es `3` (no se entiende) o `1` (el fichero no está).
- Si **el lector de ese documento está abierto**, lo recibe también en memoria (cada vuelta
  de su bucle, `lectura_veloz::tomar`): el 🟢/verde y la vista van ahí, sin traer la ventana
  delante, y al cerrarse guarda eso y no lo de antes.

### Seguridad

- `lectura_veloz` está en la **lista blanca** de los enlaces (`orden::SEGURAS`): no pregunta.
  Lleva una `ruta`, pero es la excepción a «las rutas preguntan» porque solo escribe lo leído
  de ese documento. Una ruta de red sigue prohibida también en `pixpin://pedido?json=…`.
- Todo lo de arriba se mira dos veces: en la copia de paso y en la app al recibir el pedido.
- Lo peor que puede hacer un enlace malicioso: mover el 🟢 de un documento del usuario (y
  crear su `.pixpin-lectura` al lado si no lo tenía).
- `t` no se usa para decidir nada en PixPin: **gana lo último que llega**. Si Lectura Veloz
  quiere «gana lo más nuevo», compara su `t` con el `t` que le manda PixPin al abrir.

## 3. Cómo probarlo

Desde `cmd` (en PowerShell, comillas simples alrededor del enlace):

```bat
:: PDF: el verde a la mitad de la quinta página
start "" "pixpinmax://lectura-veloz?v=1&doc=prueba&ruta=C%3A%5CUsers%5CMax%5CDocuments%5CTesis.pdf&tipo=pdf&pagina=4&frac=0.5&fraccion=0.3&t=0"

:: EPUB: el verde al 42 %
start "" "pixpinmax://lectura-veloz?v=1&doc=prueba&ruta=C%3A%5CLibros%5Ccuento.epub&tipo=texto&fraccion=0.42&t=0"

:: Lo mismo por consola, viendo el código de salida
pixpinmax lectura-veloz "C:\Libros\cuento.epub" --tipo texto --fraccion 0.42
echo %ERRORLEVEL%
```

Comprobar:

1. Con PixPin cerrada: no se abre nada; `C:\Libros\cuento.epub.pixpin-lectura` tiene
   `sitio 0.42` y `voz -1:0.42`. Al abrir el libro y escuchar (L), empieza por ese sitio y
   el 🟢 del riel está al 42 %.
2. Con el lector del PDF abierto y otra ventana delante: el 🟢 salta a la página 5 y la
   vista va ahí, sin que el lector pase al frente.
3. Casos negativos (no cambia nada, sin cuadro, `ERRORLEVEL` 3): `ruta=%5C%5Cservidor%5Cx.pdf`,
   `ruta=C%3A%5CWindows%5Cnotepad.exe`, `pagina=NaN`, sin `doc`.
4. «⚡ Lectura veloz» en el menú ⋮ de cada lector: con Lectura Veloz instalada se abre en
   el documento y la página del 🟢; sin ella, el aviso.

Pruebas automáticas: `cargo test -p pixpin-docs lectura_veloz` y `voz_alta` (Linux y
Windows), `cargo test -p pixpin-motor2d con_una_sola`, y en Windows
`cargo test -p pixpin-shell lectura_veloz` y `cargo test -p pixpin lectura_veloz`.
