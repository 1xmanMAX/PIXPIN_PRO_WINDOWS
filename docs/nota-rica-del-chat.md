# La nota rica del chat (25-oct-2026)

Una nota del chat puede llevar texto con formato y varias fotos en un solo
mensaje, como un mensaje de Telegram con su album. **No hay formato nuevo**:
sigue siendo un mensaje `NOTA` cuyo `texto` es Markdown, el mismo que ya
escribe el editor de notas de los dos aparatos.

## Lo que se guarda

```json
{"id":"1761400000000","clase":"NOTA","texto":"Visita a la obra\n\n**Pendiente:** la escalera\n![](pixpin:files/guardados/pc/<chat>/notas/1761400000000-pegado.png)\n![planta](pixpin:files/guardados/pc/<chat>/notas/1761400000001-planta.jpg)\n- [ ] medir hueco", ...}
```

- Las fotos van en `notas/<ms>-<nombre>` **dentro de la carpeta del
  proyecto**, y en el Markdown con su ruta portatil
  (`pixpin:files/guardados/pc/<chat>/notas/…`, `vista::portatil_de_ruta`).
  La sincronizacion lleva todo lo que un mensaje nombra con `pixpin:files/`
  (`disco::alcance_de`), asi que las fotos viajan con la nota (probado en
  `vista::pruebas::las_fotos_de_una_nota_rica_viajan_con_ella`).
- El alt puede llevar el ancho del editor detras de una barra
  (`![planta|300](…)`); el ancho no es parte del nombre. Las fotos pegadas
  en la caja del PC llevan de alt su numero (`![foto 2](…)`), que es lo que
  se lee mientras se escribe y lo que dice el resumen («🖼 foto 2»).
- La ruta va hasta el `)` que cierra el `(`, contando los que se abren
  dentro (`C:\Fotos (2)\a.png`), o entre `<…>`. Lo que va dentro de un
  `` `codigo` `` es texto aunque parezca una foto. Una ruta de red
  (`\\equipo\…`) o una relativa con `..` no se abre nunca.
- Un `![…](…)` que no es una foto (por su extension: un PDF, un audio) es
  un adjunto de la nota y se nombra con 📎, no se pinta.

## Como se ensena (Windows)

`pixpin_ui::nota_rica` parte el texto en bloques; el chat los mide y los
pinta con la misma disposicion (`ventana_chat/nota_rica.rs`):

- Un salto de linea es un salto de linea (es un chat, no un `.md`).
- Titulos `#`, listas `-`/`1.`, casillas `- [ ]`/`- [x]`, citas `>`,
  bloques de codigo, tablas (en monoespaciada), rayas `---`, y en linea
  `**negrita**`, `*cursiva*`, `` `codigo` ``, `~~tachado~~`, `[texto](url)`.
- **Las fotos seguidas son un album**: renglones `![…](…)` seguidos (o en el
  mismo renglon) van en filas de dos o tres; una sola guarda su proporcion.
  Una linea en blanco entre fotos las separa en dos albumes.
- Una nota sin nada de esto se pinta como siempre.
- La ruta de una foto nunca se ve como texto: mientras carga, o si su
  fichero no esta en el aparato, un recuadro con el icono de imagen.

## Escribirla en el chat

Con texto ya escrito en la caja, **pegar una foto (Ctrl+V) la mete en lo
escrito** como un renglon `![](notas/<ms>-pegado.png)` donde esta la rayita,
y un aviso lo dice. Al enviar, esas rutas relativas pasan a la portatil y
sale una sola nota con el texto y sus fotos. Con la caja vacia, pegar una
foto sigue creando su mensaje `IMAGEN` de siempre (el que sale en la
galeria). El editor de notas («Abrir en el editor de notas» del menu de la
nota) anade mas fotos por pegar, soltar o el boton de imagen, con la misma
ruta; al guardar, la burbuja se relee y las ensena.

Pendiente: las fotos de una nota rica no salen todavia en la galeria del
panel de informacion (`indices_de_la_seccion`), que solo cuenta mensajes
`IMAGEN`.

## Para Android

Basta con que la burbuja de una `NOTA` entienda el mismo Markdown y pinte
las `![…](pixpin:files/…)` de imagen en su sitio (ya resuelve esas rutas para
el editor de notas). Una ruta relativa `notas/…` no deberia llegar nunca:
el PC la pasa a portatil al enviar.
