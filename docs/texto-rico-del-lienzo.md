# Texto rico del lienzo (formato para el PC y Android)

15-oct-2026. Un texto del lienzo puede llevar **tramos con estilo** (color,
negrita, cursiva, subrayado, tachado, tamaño) y **fórmulas en línea**
compuestas como en la pizarra (fracciones, superíndices, raíces, raya y arco
encima, colores dentro de la fórmula). Lo pidió el usuario para reconstruir
láminas de geometría: «Nos piden m∢COD = **X**» con la X en rojo, «A̅B̅» con su
raya, «mAC⌒» con su arco, «45° = (2a+2b)/2» con la fracción de verdad.

No es un elemento nuevo: es el mismo `text` de Excalidraw con **una clave
más**. Código del PC: `crates/pixpin-motor2d/src/texto_rico.rs` (leer,
componer, texto plano), `formulas.rs` (el lector de LaTeX, el mismo de las
notas) y `ecuacion.rs` (el compositor de cajas, el mismo de las ecuaciones
del asistente).

## En el fichero

```json
{
  "type": "text",
  "text": "Nos piden m∢COD = X",
  "originalText": "Nos piden m∢COD = X",
  "textoRico": "Nos piden $m\\sphericalangle COD = \\color{rojo}{X}$",
  "fontSize": 20, "fontFamily": 6, "textAlign": "left", ...
}
```

- `textoRico` (cadena, opcional): la **fuente** con el formato de abajo. Si
  falta, el texto es de los de siempre.
- `text`: **el texto plano de reserva**, siempre presente y siempre
  coherente con la fuente (`texto_rico::plano`). Es lo que ve quien no
  entiende el formato (Excalidraw, un Android viejo, la búsqueda, la voz).
- **Regla de coherencia** (la aplica el PC al leer, y Android debe
  aplicarla igual): si `plano(textoRico) != text`, alguien que no entiende
  el formato editó `text`; la fuente está vieja y **se descarta** (gana
  `text`). Así un Android sin soporte puede editar el texto sin que el PC
  le devuelva lo de antes.
- Quien edite un texto rico debe reescribir **los dos**: `textoRico` con la
  fuente nueva y `text` (y `originalText`) con su plano.
- La caja (`width`/`height`) es la de lo compuesto.

## La fuente

Renglones separados por `\n`. Cada renglón se lee por su cuenta (las marcas
no cruzan renglones).

| Escrito | Sale |
|---|---|
| `$...$` | fórmula en línea (LaTeX de bolsillo, abajo); sus fracciones encogen para no abrir el renglón |
| `$$...$$` | fórmula alta: fracciones a tamaño completo |
| `\$` `\*` `\\` `\{` | el carácter tal cual |
| `**negrita**` | negrita |
| `*cursiva*` o `_cursiva_` | cursiva (no abre pegado a una letra: `2*3*4` y `nombre_de_fichero` siguen siendo eso) |
| `~~tachado~~` | tachado |
| `<u>subrayado</u>` | subrayado |
| `\color{rojo}{...}` `\textcolor{red}{...}` | color: nombre en español o inglés (rojo/red, azul, verde, amarillo, naranja, morado, rosa, gris, marrón, celeste, negro, blanco; también en femenino o plural) o `#rrggbb` / `#rgb` |
| `\textbf{}` `\textit{}` `\underline{}` `\sout{}` | negrita, cursiva, subrayado, tachado |
| `\tam{1.5}{...}` | tamaño en **veces** el del texto (0,3 a 5); `\small` 0,8, `\large` 1,2, `\Large` 1,44, `\huge` 1,73, `\tiny` 0,6 |
| al empezar un renglón: `\centro` `\derecha` `\izquierda` `\justificado` (o `\center` `\flushright` `\flushleft` `\justify`) | alineación de ese renglón; sin marca, la del elemento (`textAlign`) |

Reglas de `$` (las de Pandoc, para no confundir dinero con fórmulas): el `$`
que abre no va seguido de un blanco; el que cierra no va detrás de un blanco
ni delante de una cifra. `cuesta $5 y $10` es texto.

**Lo que no se entiende se ve tal cual**: un `$` sin cerrar es un dólar, un
mandato desconocido sale con su barra, unas llaves rotas no se comen nada.
Nunca falla.

Un texto se guarda como rico solo si lleva alguna marca
(`texto_rico::tiene_marcas`); un texto sin marcas no estrena la clave.

## Dentro de `$...$`

El lector de fórmulas de las notas (`Formulas.kt` de Android, portado en
`formulas.rs`), con lo añadido el 15-oct:

- `\frac{a}{b}` `\dfrac` `\tfrac` `\cfrac`, `a/b`, `x^2`, `a_1`, `\sqrt[3]{x}`, matrices y sistemas (`\begin{bmatrix}`, `cases`).
- **Marcas que crecen con lo que cubren**: `\overline{AB}` (y `\bar`) raya,
  `\overset{\frown}{AC}` / `\wideparen{AC}` / `\overarc` / `\arc` arco,
  `\vec{v}` / `\overrightarrow` flecha, `\hat` / `\widehat` gorro, `\tilde`
  / `\widetilde`, `\underline`.
- `\color{red}{X}`, `\textcolor`, `{\color{red} X}` (hasta el final del
  grupo). El de dentro gana al de fuera.
- `\mathbf{}` `\textbf{}` `\boldsymbol{}` negrita; `\text{}` texto derecho;
  `\boxed{}` recuadro; `\overset{a}{b}` lo pequeño encima.
- Grados: `90^\circ`, `90^{\circ}` o `90°`.
- Símbolos: `\sphericalangle` ∢ (el «m∢AOB» de geometría), `\measuredangle`
  ∡, `\angle` ∠, `\theta` θ, `\pi` π, `\therefore` ∴, `\because` ∵, `\leq`
  ≤, `\geq` ≥, `\neq` ≠, `\pm` ±, `\times` ×, `\div` ÷, `\perp` ⊥,
  `\parallel` ∥, `\cong` ≅, `\sim` ∼, `\frown` ⌢, griegas, flechas...
- Los signos de ángulo (∢ ∡ ∠), dentro o fuera de una fórmula, **se dibujan
  con rayas**: casi ninguna fuente de texto los trae.

## El texto plano de reserva

`texto_rico::plano`: las marcas desaparecen y las fórmulas se escriben como en
un cuaderno: `x²`, `a₁`, `(2a + 2b)/2`, `√(x)`, `A̅B̅` (raya combinante
U+0305 tras cada letra), `AC⌒`, `45°`, `m∢COD = X`.

## Editar (PC)

- Con la herramienta de texto se escribe la fuente tal cual; **al salir de la
  caja se compone**. Al volver a abrirlo se edita la fuente.
- Mayús+flechas/Inicio/Fin eligen; Ctrl+A todo.
- Ctrl+B o Ctrl+N negrita, Ctrl+I o Ctrl+K cursiva, Ctrl+U subrayado,
  Ctrl+Mayús+X tachado, Ctrl+M fórmula (`$...$`), Ctrl+Mayús+1..6 color del
  tramo elegido (rojo, azul, verde, naranja, morado, gris), Ctrl+Mayús+0
  negro. Sin nada elegido, ponen las marcas y dejan el cursor en medio.
- Ctrl+Z / Ctrl+Y dentro del texto; al cerrar, lo escrito en un texto que ya
  existía es **un** paso del Ctrl+Z del lienzo.
- Lo mismo en el anotador de capturas.

## Para Android

1. Leer `textoRico` en `Element` (cadena opcional, como `negrita`). Aplicar
   la regla de coherencia al leer.
2. Pintar: si hay `textoRico`, componer con `Formulas.kt` (ya existe; añadir
   `Adorno`, `Color`, `Negrita`, `Recuadro`, `Encima`, `Grande` como en
   `formulas.rs`) y `Ecuacion.kt` (añadir color por pieza, `adornada`,
   `recuadro`, `encima`, `angulo`). Los renglones se alinean por el eje de
   sus cajas (`texto_rico::componer`): texto y fórmulas a la misma altura.
3. Mientras no se pinte rico, lo ideal es **conservar** `textoRico` al
   guardar (como clave desconocida que se devuelve tal cual). Si se pierde,
   el texto vuelve al PC plano y legible; si se edita `text`, el PC descarta
   la fuente vieja.
4. Mientras tanto, Android ya enseña el `text` plano, que es legible.
