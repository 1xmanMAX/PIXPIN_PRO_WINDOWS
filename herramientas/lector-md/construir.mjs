// Construye el editor de notas en UN solo html, sin nada de fuera (ni
// internet ni ficheros sueltos): el programa, los estilos y las letras de
// PixPin van dentro. Sale a `apps/pixpin/recursos/lector-md.html`, que el
// ejecutable lleva puesto (`include_bytes!`). Se corre a mano al cambiar
// algo de aqui: `npm install` y `npm run build`.
import { build } from 'esbuild'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const aqui = path.dirname(fileURLToPath(import.meta.url))
const salida = path.resolve(aqui, '../../apps/pixpin/recursos/lector-md.html')
const letras = path.resolve(aqui, '../../crates/pixpin-render/letras')

const js = await build({
  entryPoints: [path.join(aqui, 'src/main.js')],
  bundle: true,
  minify: true,
  format: 'iife',
  target: 'chrome110',
  write: false,
  legalComments: 'none',
})
const programa = js.outputFiles[0].text
const css = fs.readFileSync(path.join(aqui, 'src/nota.css'), 'utf8')

// Las letras que se pueden elegir para la nota (las del lienzo de PixPin).
const LETRAS = [
  ['Work Sans', 'work-sans-400.woff2', '400'],
  ['Fraunces', 'fraunces-600.woff2', '400 900'],
  ['Excalifont', 'excalifont.woff2'],
  ['Nunito', 'nunito.woff2'],
  ['Lilita One', 'lilita-one.woff2'],
  ['Comic Shanns', 'comic-shanns.woff2'],
  ['Caveat', 'caveat-500.woff2'],
]
const fuentes = LETRAS.filter(([, f]) => fs.existsSync(path.join(letras, f)))
  .map(([familia, f, peso = '400']) => {
    const b64 = fs.readFileSync(path.join(letras, f)).toString('base64')
    return `@font-face{font-family:'${familia}';src:url(data:font/woff2;base64,${b64}) format('woff2');font-weight:${peso};font-display:block}`
  })
  .join('\n')

const svg = {
  abajo: '<svg viewBox="0 0 12 12" width="10" height="10"><path d="M3 4.5l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  compartir: '<svg viewBox="0 0 16 16"><path d="M8 10V2M5 5l3-3 3 3M3 8v5a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V8"/></svg>',
  minimizar: '<svg viewBox="0 0 16 16"><path d="M4 8h8"/></svg>',
  pantalla: '<svg viewBox="0 0 16 16"><path d="M3 6V3h3M10 3h3v3M13 10v3h-3M6 13H3v-3"/></svg>',
  cerrar: '<svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8"/></svg>',
  tabla: '<svg viewBox="0 0 16 16"><rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M2 7h12M2 10h12M8 3v10"/></svg>',
  imagen: '<svg viewBox="0 0 16 16"><rect x="2" y="3" width="12" height="10" rx="1.5"/><circle cx="6" cy="6.5" r="1.2"/><path d="M2.5 12l4-4 3 3 2-2 2.5 2.5"/></svg>',
  casillas: '<svg viewBox="0 0 16 16"><path d="M2 4l1.5 1.5L6 3M2 10l1.5 1.5L6 9M8.5 4.5H14M8.5 10.5H14"/></svg>',
  mas: '<svg viewBox="0 0 16 16"><path d="M8 3v10M3 8h10"/></svg>',
  emoji: '<svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/><path d="M5.5 9.5c.8 1.2 1.6 1.7 2.5 1.7s1.7-.5 2.5-1.7"/><path d="M6 6.2v.3M10 6.2v.3"/></svg>',
  enlace: '<svg viewBox="0 0 16 16"><path d="M7 9a3 3 0 0 0 4.2 0l2-2a3 3 0 0 0-4.2-4.2l-.8.8M9 7a3 3 0 0 0-4.2 0l-2 2A3 3 0 0 0 7 13.2l.8-.8"/></svg>',
  quitar: '<svg viewBox="0 0 16 16"><path d="M4 3h8M8 3v10M3 13l10-10"/></svg>',
  bocadillo: '<svg viewBox="0 0 16 16"><path d="M3 3h10a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H7l-3 2.5V11H3a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z"/></svg>',
  comentar: '<svg viewBox="0 0 16 16"><path d="M3 3h10a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H7l-3 2.5V11H3a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z"/><path d="M8 5v4M6 7h4"/></svg>',
  codigo: '<svg viewBox="0 0 16 16"><path d="M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5"/></svg>',
}

const html = `<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<title>Nota</title>
<!-- Editor de notas de PixPin: CodeMirror (MIT), letras de PixPin. -->
<style>${fuentes}</style>
<style>${css}</style>
</head>
<body>
<header class="cabecera">
  <div class="pastilla-titulo" id="titulo"><span class="texto" id="titulo-texto"></span><span class="flecha" id="titulo-flecha">${svg.abajo}</span></div>
  <div class="arrastre" id="arrastre"></div>
  <button class="b28 comentarios" id="b-comentarios">${svg.bocadillo}<span class="insignia" id="insignia" hidden></span></button>
  <button class="compartir" id="b-compartir">${svg.compartir}<span data-t="compartir">Compartir</span></button>
  <div class="ventana-botones">
    <button class="b28" id="b-minimizar">${svg.minimizar}</button>
    <button class="b28" id="b-maximizar">${svg.pantalla}</button>
    <button class="b28" id="b-cerrar">${svg.cerrar}</button>
  </div>
</header>
<nav class="herramientas">
  <button class="b28" id="b-tabla">${svg.tabla}</button>
  <button class="b28" id="b-imagen">${svg.imagen}</button>
  <button class="b28" id="b-casillas">${svg.casillas}</button>
  <button class="b28" id="b-comentar">${svg.comentar}</button>
  <span class="separador"></span>
  <button class="b28 mas" id="b-mas">${svg.mas}<span class="v">▾</span></button>
  <div class="de-tabla">
    <span class="separador"></span>
    <button class="pildora" data-tabla="fila+" data-t="masFila">+ Fila</button>
    <button class="pildora" data-tabla="fila-" data-t="menosFila">− Fila</button>
    <button class="pildora" data-tabla="col+" data-t="masColumna">+ Columna</button>
    <button class="pildora" data-tabla="col-" data-t="menosColumna">− Columna</button>
  </div>
</nav>
<main class="papel" id="papel"></main>
<div class="flotante" id="flotante" hidden>
  <button class="b" data-f="comentar">${svg.comentar}</button>
  <button class="b" data-f="emoji">${svg.emoji}</button>
  <span class="sep"></span>
  <button class="b" data-f="aa" style="width:46px">Aa ▾</button>
  <span class="sep"></span>
  <button class="b" data-f="negrita"><b>B</b></button>
  <button class="b" data-f="cursiva"><i style="font-family:Georgia">I</i></button>
  <button class="b" data-f="tachado"><s>S</s></button>
  <button class="b" data-f="codigo">${svg.codigo}</button>
  <button class="b" data-f="quitar">${svg.quitar}</button>
  <span class="sep"></span>
  <button class="b" data-f="listas" style="width:42px">☰ ▾</button>
  <button class="b" data-f="enlace">${svg.enlace}</button>
</div>
<input class="caja-enlace" id="caja-enlace" hidden>
<div class="menu" id="menu" hidden></div>
<div class="grande" id="grande" hidden><img alt=""></div>
<div class="aviso" id="aviso"></div>
<script>${programa.replace(/<\/script/gi, '<\\/script')}</script>
</body>
</html>
`
fs.mkdirSync(path.dirname(salida), { recursive: true })
fs.writeFileSync(salida, html)
console.log(`${salida}: ${(html.length / 1024).toFixed(0)} KB`)
