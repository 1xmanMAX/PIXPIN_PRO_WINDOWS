// El editor de notas de PixPin (8-oct-2026): la misma cara y las mismas
// teclas que el de antes (pixpin-notas, sobre el RichEdit), con un motor
// nuevo: CodeMirror dentro de WebView2. El usuario: «no mejores la UI
// anterior sino la funcionalidad, ya que no funcionaba bien».
//
// Con PixPin se habla por `chrome.webview` (JSON con `tipo`): ver
// `apps/pixpin/src/notas_md/lector_web.rs`.

import { EditorState, Transaction, Annotation, EditorSelection } from '@codemirror/state'
import { EditorView, keymap, drawSelection, placeholder, dropCursor } from '@codemirror/view'
import { defaultKeymap, history, historyKeymap, redo } from '@codemirror/commands'
import { markdown, markdownLanguage, insertNewlineContinueMarkup, deleteMarkupBackward } from '@codemirror/lang-markdown'
import { search, searchKeymap, openSearchPanel } from '@codemirror/search'
import { vivo, clicEnCasilla, acciones, RE_CASILLA, RE_LISTA } from './vivo.js'
import { celdas } from './tablas.js'
import { TEXTOS } from './textos.js'
import { crearComentarios, resaltados } from './comentarios.js'

const $ = id => document.getElementById(id)
const desdeFuera = Annotation.define()
let tx = TEXTOS.es
let vista = null
let nombreFichero = ''

// ------------------------------------------------------------------ puente

function mandar(msg) {
  try {
    window.chrome.webview.postMessage(JSON.stringify(msg))
  } catch (e) {
    console.warn('sin PixPin', msg)
  }
}

// ------------------------------------------------------------------ guardar

let version = 0
let guardada = 0
let enVuelo = null
let temporizador = null
let cerrando = false
const sucio = () => version !== guardada
const texto = () => vista.state.doc.toString()

function programarGuardado() {
  clearTimeout(temporizador)
  temporizador = setTimeout(guardarYa, 800)
}
function guardarYa() {
  clearTimeout(temporizador)
  if (!sucio() || enVuelo !== null) return
  enVuelo = version
  mandar({ tipo: 'guardar', texto: texto(), version })
}
function alGuardar({ version: v, ok }) {
  enVuelo = null
  if (ok) guardada = Math.max(guardada, v)
  else aviso(tx.noGuardada)
  if (sucio()) {
    if (ok) guardarYa()
    else temporizador = setTimeout(guardarYa, 3000)
  } else if (cerrando) {
    mandar({ tipo: 'cerrar_listo' })
  }
}

// ------------------------------------------------------------------ titulo

/** El titulo: el primer renglon con algo, sin marcas, 40 letras. */
export function tituloDe(t) {
  for (const l of t.split('\n')) {
    const s = l
      .replace(/^\s*(#{1,6}\s+|>\s*|[-*+]\s+(\[[ xX]\]\s+)?|\d+[.)]\s+)/, '')
      .replace(/!\[([^\]|]*)(\|\d+)?\]\([^)]*\)/g, '$1')
      .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
      .replace(/[*_~`$]/g, '')
      .trim()
    if (s) return s.length > 40 ? s.slice(0, 40).trimEnd() + '…' : s
  }
  return ''
}
let ultimoTitulo = null
function ponerTitulo() {
  const t = tituloDe(texto())
  if (t === ultimoTitulo) return
  ultimoTitulo = t
  $('titulo-texto').textContent = t || nombreFichero || tx.notaNueva
  mandar({ tipo: 'titulo', texto: t })
}

/** Cambiar el titulo (`md_vivo::con_titulo`): si el primer renglon con algo
 * ya es un titulo se cambia su texto; si no, va un `# nuevo` encima. */
function cambiarTitulo(nuevo) {
  nuevo = nuevo.trim()
  if (!nuevo) return
  const doc = vista.state.doc
  for (let k = 1; k <= doc.lines; k++) {
    const l = doc.line(k)
    if (!l.text.trim()) continue
    const m = /^(\s*#{1,6}\s+)(.*)$/.exec(l.text)
    if (m) vista.dispatch({ changes: { from: l.from + m[1].length, to: l.to, insert: nuevo } })
    else vista.dispatch({ changes: { from: l.from, insert: `# ${nuevo}\n` } })
    return
  }
  vista.dispatch({ changes: { from: 0, to: doc.length, insert: `# ${nuevo}\n` } })
}

function editarTitulo() {
  const chip = $('titulo')
  const actual = tituloDe(texto())
  const input = document.createElement('input')
  input.value = actual
  const txt = $('titulo-texto')
  txt.hidden = true
  chip.insertBefore(input, txt)
  input.select()
  input.focus()
  let hecho = false
  const fin = aplicar => {
    if (hecho) return
    hecho = true
    if (aplicar && input.value.trim() !== actual) cambiarTitulo(input.value)
    input.remove()
    txt.hidden = false
    vista.focus()
  }
  input.addEventListener('keydown', e => {
    if (e.key === 'Enter') {
      e.preventDefault()
      fin(true)
    } else if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      fin(false)
    }
  })
  input.addEventListener('blur', () => fin(true))
}

// ------------------------------------------------------------------ formato

function envolver(marca) {
  const st = vista.state
  const cambios = st.changeByRange(r => {
    let { from, to } = r
    if (from === to) {
      const p = st.wordAt(from)
      if (p) ({ from, to } = p)
    }
    const doc = st.doc
    if (doc.sliceString(from - marca.length, from) === marca && doc.sliceString(to, to + marca.length) === marca) {
      return {
        changes: [
          { from: from - marca.length, to: from, insert: '' },
          { from: to, to: to + marca.length, insert: '' },
        ],
        range: EditorSelection.range(from - marca.length, to - marca.length),
      }
    }
    const t = doc.sliceString(from, to)
    if (t.length >= marca.length * 2 && t.startsWith(marca) && t.endsWith(marca)) {
      return {
        changes: { from, to, insert: t.slice(marca.length, t.length - marca.length) },
        range: EditorSelection.range(from, to - marca.length * 2),
      }
    }
    return {
      changes: [
        { from, insert: marca },
        { from: to, insert: marca },
      ],
      range: EditorSelection.range(from + marca.length, to + marca.length),
    }
  })
  vista.dispatch(st.update(cambios, { userEvent: 'input', scrollIntoView: true }))
  vista.focus()
}

const PREFIJO = /^(\s*)(#{1,6}\s+|>\s?|[-*+]\s+\[[ xX]\]\s+|[-*+]\s+|\d+[.)]\s+)?/

function lineasElegidas(st) {
  const n = new Set()
  for (const r of st.selection.ranges) {
    for (let p = r.from; p <= r.to; ) {
      const l = st.doc.lineAt(p)
      n.add(l.number)
      p = l.to + 1
    }
  }
  return [...n]
}

/** Pone `prefijo` a los renglones elegidos; si todos lo tienen, lo quita. */
function prefijar(prefijo) {
  const st = vista.state
  const nums = lineasElegidas(st)
  const tipo = p => (p || '').replace(/\d+/, '1').replace(/\[[xX]\]/, '[ ]').replace(/\s+$/, ' ')
  const todas = prefijo !== '' && nums.every(n => tipo(PREFIJO.exec(st.doc.line(n).text)[2]) === tipo(prefijo))
  const changes = nums.map((n, i) => {
    const l = st.doc.line(n)
    const m = PREFIJO.exec(l.text)
    const desde = l.from + m[1].length
    let nuevo = todas ? '' : prefijo
    if (!todas && prefijo === '1. ') nuevo = `${i + 1}. `
    return { from: desde, to: desde + (m[2] || '').length, insert: nuevo }
  })
  vista.dispatch({ changes, userEvent: 'input', scrollIntoView: true })
  vista.focus()
}

function insertar(t, cursor = null) {
  const { from, to } = vista.state.selection.main
  vista.dispatch({
    changes: { from, to, insert: t },
    selection: { anchor: from + (cursor ?? t.length) },
    userEvent: 'input',
    scrollIntoView: true,
  })
  vista.focus()
}

/** Un bloque en un renglon suyo. */
function insertarBloque(t, cursor = null) {
  const st = vista.state
  const l = st.doc.lineAt(st.selection.main.head)
  const lleno = !!l.text.trim()
  const antes = lleno ? '\n' : ''
  const pos = lleno ? l.to : l.from
  vista.dispatch({
    changes: { from: pos, to: lleno ? pos : l.to, insert: antes + t },
    selection: { anchor: pos + antes.length + (cursor ?? t.length) },
    userEvent: 'input',
    scrollIntoView: true,
  })
  vista.focus()
}

function fechaDeHoy() {
  const d = new Date()
  return `${d.getDate()} ${tx.meses[d.getMonth()]} ${d.getFullYear()}`
}

function alternarCasilla() {
  const st = vista.state
  const changes = []
  for (const n of lineasElegidas(st)) {
    const l = st.doc.line(n)
    const m = RE_CASILLA.exec(l.text)
    if (!m) return prefijar('- [ ] ')
    const en = l.from + m[1].length + m[2].length + m[3].length + 1
    changes.push({ from: en, to: en + 1, insert: m[4] === ' ' ? 'x' : ' ' })
  }
  vista.dispatch({ changes })
}

function quitarFormato() {
  const st = vista.state
  const r = st.selection.main
  const t = st.doc.sliceString(r.from, r.to).replace(/\*\*|~~|`|\$/g, '').replace(/(^|[^*])\*([^*]+)\*/g, '$1$2')
  vista.dispatch({ changes: { from: r.from, to: r.to, insert: t }, selection: { anchor: r.from, head: r.from + t.length } })
}

const TABLA_NUEVA = '|  |  |\n| --- | --- |\n|  |  |\n|  |  |'

const ORDENES = {
  negrita: () => envolver('**'),
  cursiva: () => envolver('*'),
  tachado: () => envolver('~~'),
  codigo: () => envolver('`'),
  formula: () => envolver('$'),
  enlace: () => abrirCajaEnlace(),
  normal: () => prefijar(''),
  t1: () => prefijar('# '),
  t2: () => prefijar('## '),
  t3: () => prefijar('### '),
  lista: () => prefijar('- '),
  numerada: () => prefijar('1. '),
  casillas: () => prefijar('- [ ] '),
  marcar: () => alternarCasilla(),
  cita: () => prefijar('> '),
  bloque: () => insertarBloque('```\n\n```', 4),
  raya: () => insertarBloque('---\n'),
  tabla: () => insertarBloque(TABLA_NUEVA, 2),
  fecha: () => insertar(fechaDeHoy()),
  imagen: () => mandar({ tipo: 'elegir', clase: 'imagen' }),
  documento: () => mandar({ tipo: 'elegir', clase: 'documento' }),
  audio: () => mandar({ tipo: 'elegir', clase: 'audio' }),
  pagina: () => mandar({ tipo: 'elegir_hoja', viva: true }),
  hoja: () => mandar({ tipo: 'elegir_hoja', viva: false }),
  chat: () => mandar({ tipo: 'elegir_mensaje' }),
  quitar: () => quitarFormato(),
  guardar: () => guardarYa(),
  copia: () => mandar({ tipo: 'guardar_copia', texto: texto(), titulo: tituloDe(texto()) }),
  word: () => {
    guardarYa()
    mandar({ tipo: 'exportar_word', texto: texto(), titulo: tituloDe(texto()) })
  },
  cambiarTitulo: () => editarTitulo(),
  comentar: () => com.comentar(),
  buscar: () => openSearchPanel(vista),
  cortar: () => {
    vista.focus()
    document.execCommand('cut')
  },
  copiar: () => {
    vista.focus()
    document.execCommand('copy')
  },
  pegar: () => navigator.clipboard.readText().then(t => insertar(t)),
}

// ------------------------------------------------------------------ menus

const menu = $('menu')
let menuActual = null

/** Abre un menu en (x, y). `filas`: {icono, texto, atajo, accion, apagada,
 * muestra} o 'linea' o {pista}. */
function abrirMenu(filas, x, y, { alCerrar } = {}) {
  cerrarMenu()
  menu.textContent = ''
  const elegibles = []
  for (const f of filas) {
    if (f === 'linea') {
      const d = document.createElement('div')
      d.className = 'linea'
      menu.append(d)
      continue
    }
    if (f.pista) {
      const d = document.createElement('div')
      d.className = 'pista'
      d.textContent = f.pista
      menu.append(d)
      continue
    }
    const d = document.createElement('div')
    d.className = 'fila' + (f.apagada ? ' apagada' : '')
    const ic = document.createElement('span')
    ic.className = 'icono'
    if (f.muestra) {
      const m = document.createElement('span')
      m.className = 'muestra'
      m.style.background = f.muestra
      ic.append(m)
    } else ic.textContent = f.icono || ''
    const t = document.createElement('span')
    t.textContent = f.texto
    d.append(ic, t)
    if (f.atajo) {
      const a = document.createElement('span')
      a.className = 'atajo'
      a.textContent = f.atajo
      d.append(a)
    }
    d._accion = f.accion
    d.addEventListener('mousedown', e => e.preventDefault())
    d.addEventListener('mouseenter', () => elegir(elegibles.indexOf(d)))
    d.addEventListener('click', () => {
      cerrarMenu()
      f.accion?.()
    })
    menu.append(d)
    if (!f.apagada) elegibles.push(d)
  }
  menu.hidden = false
  const r = menu.getBoundingClientRect()
  menu.style.left = Math.max(4, Math.min(x, innerWidth - r.width - 6)) + 'px'
  menu.style.top = Math.max(4, Math.min(y, innerHeight - r.height - 6)) + 'px'
  let i = -1
  function elegir(n) {
    elegibles[i]?.classList.remove('elegida')
    i = n
    elegibles[i]?.classList.add('elegida')
  }
  menuActual = {
    elegir,
    mover(d) {
      if (elegibles.length) elegir((i + d + elegibles.length) % elegibles.length)
    },
    pulsar() {
      const d = elegibles[i]
      if (!d) return false
      cerrarMenu()
      d._accion?.()
      return true
    },
    alCerrar,
  }
}
function cerrarMenu() {
  if (menu.hidden) return
  menu.hidden = true
  const m = menuActual
  menuActual = null
  m?.alCerrar?.()
}
document.addEventListener('mousedown', e => {
  if (!menu.hidden && !menu.contains(e.target)) cerrarMenu()
})

function menuTitulo() {
  const r = $('titulo').getBoundingClientRect()
  abrirMenu(
    [
      { icono: '✎', texto: tx.cambiarTitulo, accion: ORDENES.cambiarTitulo },
      { icono: '💾', texto: tx.guardar, atajo: 'Ctrl+S', accion: ORDENES.guardar },
      { icono: '⤓', texto: tx.copiaMd, atajo: 'Ctrl+Mayús+S', accion: ORDENES.copia },
      { icono: '📄', texto: tx.word, accion: ORDENES.word },
      'linea',
      { icono: 'Aa', texto: tx.letraTexto, atajo: preferencias.cuerpo + '  ›', accion: () => menuLetras('cuerpo', r) },
      { icono: 'H1', texto: tx.letraTitulos, atajo: preferencias.titulos + '  ›', accion: () => menuLetras('titulos', r) },
      { icono: 'A+', texto: tx.tamano, atajo: preferencias.px + ' px  ›', accion: () => menuTamano(r) },
    ],
    r.left,
    r.bottom + 4,
  )
}

const LETRAS = ['Excalifont', 'Nunito', 'Lilita One', 'Comic Shanns', 'Work Sans', 'Fraunces', 'Courier New', 'Caveat']

function menuLetras(cual, r) {
  abrirMenu(
    LETRAS.map(l => ({
      icono: preferencias[cual] === l ? '✓' : '',
      texto: l,
      accion: () => {
        preferencias[cual] = l
        aplicarPreferencias()
        guardarPreferencias()
      },
    })),
    r.left,
    r.bottom + 4,
  )
}

function menuTamano(r) {
  const tams = [
    [tx.pequena, 14],
    [tx.normal, 16],
    [tx.grande, 18],
    [tx.muyGrande, 21],
  ]
  abrirMenu(
    [
      ...tams.map(([t, px]) => ({
        icono: preferencias.px === px ? '✓' : '',
        texto: t,
        atajo: px + ' px',
        accion: () => {
          preferencias.px = px
          aplicarPreferencias()
          guardarPreferencias()
        },
      })),
      'linea',
      {
        icono: preferencias.soloEsta ? '✓' : '✎',
        texto: tx.soloEsta,
        accion: () => {
          preferencias.soloEsta = !preferencias.soloEsta
          guardarPreferencias()
        },
      },
      { pista: tx.pistaTamano.replace('{px}', preferencias.px) },
    ],
    r.left,
    r.bottom + 4,
  )
}

function menuMas() {
  const r = $('b-mas').getBoundingClientRect()
  abrirMenu(
    [
      { icono: '•', texto: tx.lista, accion: ORDENES.lista },
      { icono: '☑', texto: tx.casillas, accion: ORDENES.casillas },
      { icono: '📅', texto: tx.fecha, accion: ORDENES.fecha },
      { icono: '1.', texto: tx.numerada, accion: ORDENES.numerada },
      ...(capacidades.hojas
        ? [
            { icono: '▣', texto: tx.pagina, accion: ORDENES.pagina },
            { icono: '🔗', texto: tx.enlaceHoja, accion: ORDENES.hoja },
          ]
        : []),
      ...(capacidades.medios
        ? [
            { icono: '📄', texto: tx.documento, accion: ORDENES.documento },
            { icono: '💬', texto: tx.delChat, accion: ORDENES.chat },
            { icono: '🎧', texto: tx.audio, accion: ORDENES.audio },
          ]
        : []),
      { pista: tx.pistaBarra },
    ],
    r.left,
    r.bottom + 4,
  )
}

// El menu del boton derecho.
function menuContextual(x, y) {
  const filas = []
  if (enTabla()) {
    filas.push(
      { texto: tx.filaEncima, accion: () => tablaFila(-1) },
      { texto: tx.filaDebajo, accion: () => tablaFila(1) },
      { texto: tx.colIzquierda, accion: () => tablaColumna(-1) },
      { texto: tx.colDerecha, accion: () => tablaColumna(1) },
      { texto: tx.quitarFila, accion: () => tablaQuitarFila() },
      { texto: tx.quitarColumna, accion: () => tablaQuitarColumna() },
      { texto: tx.quitarTabla, accion: () => tablaQuitar() },
      'linea',
    )
  }
  filas.push(
    { icono: '💬', texto: tx.comentar, atajo: 'Ctrl+Alt+M', accion: ORDENES.comentar },
    'linea',
    { icono: 'B', texto: tx.negrita, atajo: 'Ctrl+B', accion: ORDENES.negrita },
    { icono: 'I', texto: tx.cursiva, atajo: 'Ctrl+I', accion: ORDENES.cursiva },
    { icono: 'S', texto: tx.tachado, atajo: 'Ctrl+Mayús+X', accion: ORDENES.tachado },
    { icono: '</>', texto: tx.codigo, atajo: 'Ctrl+E', accion: ORDENES.codigo },
    { icono: '🔗', texto: tx.enlace, atajo: 'Ctrl+K', accion: ORDENES.enlace },
    { icono: '∑', texto: tx.formula, atajo: 'Ctrl+M', accion: ORDENES.formula },
    'linea',
    { icono: 'H1', texto: tx.titulo, atajo: 'Ctrl+1', accion: ORDENES.t1 },
    { icono: 'H2', texto: tx.subtitulo, atajo: 'Ctrl+2', accion: ORDENES.t2 },
    { icono: 'H3', texto: tx.apartado, atajo: 'Ctrl+3', accion: ORDENES.t3 },
    { icono: '•', texto: tx.lista, atajo: 'Ctrl+Mayús+8', accion: ORDENES.lista },
    { icono: '1.', texto: tx.numerada, atajo: 'Ctrl+Mayús+7', accion: ORDENES.numerada },
    { icono: '☑', texto: tx.casillas, atajo: 'Ctrl+Mayús+9', accion: ORDENES.casillas },
    { icono: '✓', texto: tx.marcar, atajo: 'Ctrl+Intro', accion: ORDENES.marcar },
    { icono: '❝', texto: tx.cita, atajo: 'Ctrl+Mayús+.', accion: ORDENES.cita },
    { icono: '{}', texto: tx.bloque, atajo: 'Ctrl+Mayús+C', accion: ORDENES.bloque },
    { icono: '—', texto: tx.raya, accion: ORDENES.raya },
    { icono: '⊞', texto: tx.tabla, atajo: 'Ctrl+Mayús+T', accion: ORDENES.tabla },
    { icono: '🖼', texto: tx.imagen, accion: ORDENES.imagen },
    ...(capacidades.hojas
      ? [
          { icono: '▣', texto: tx.pagina, accion: ORDENES.pagina },
          { icono: '🔗', texto: tx.enlaceHoja, accion: ORDENES.hoja },
        ]
      : []),
    { icono: '📅', texto: tx.fecha, atajo: 'Ctrl+Mayús+D', accion: ORDENES.fecha },
    'linea',
    { texto: tx.cortar, atajo: 'Ctrl+X', accion: ORDENES.cortar },
    { texto: tx.copiar, atajo: 'Ctrl+C', accion: ORDENES.copiar },
    { texto: tx.pegar, atajo: 'Ctrl+V', accion: ORDENES.pegar },
    'linea',
    { texto: tx.guardar, atajo: 'Ctrl+S', accion: ORDENES.guardar },
    { texto: tx.copiaMd, atajo: 'Ctrl+Mayús+S', accion: ORDENES.copia },
  )
  abrirMenu(filas, x, y)
}

// ------------------------------------------------------------------ menu «/»

const CATALOGO = [
  ['t1', 'titulo', '# t1 h1 titulo heading encabezado', '#', 'H1'],
  ['t2', 'subtitulo', '## t2 h2 subtitulo', '##', 'H2'],
  ['t3', 'apartado', '### t3 h3 apartado', '###', 'H3'],
  ['lista', 'lista', '- lista vinetas bullet list', '-', '•'],
  ['numerada', 'numerada', '1. numerada numbered ol', '1.', '1.'],
  ['casillas', 'casillas', '[] casillas tareas checklist todo', '[ ]', '☑'],
  ['cita', 'cita', '> cita quote', '>', '❝'],
  ['bloque', 'bloque', '``` codigo code pre', '```', '{}'],
  ['tabla', 'tabla', 'tabla table', '', '⊞'],
  ['imagen', 'imagen', 'imagen foto image picture', '', '🖼'],
  ['fecha', 'fecha', 'fecha hoy date today', '', '📅'],
  ['raya', 'raya', '--- separador raya divider rule', '---', '—'],
  ['pagina', 'pagina', 'pagina hoja lienzo proyecto page sheet canvas', '', '▣', 'hojas'],
  ['hoja', 'enlaceHoja', 'enlace vinculo link', '', '🔗', 'hojas'],
  ['documento', 'documento', 'documento archivo adjunto pdf document file', '', '📄', 'medios'],
  ['chat', 'delChat', 'chat mensaje message', '', '💬', 'medios'],
  ['audio', 'audio', 'audio voz musica sonido voice', '', '🎧', 'medios'],
]

const sinTildes = s => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()

let barra = null // { desde } mientras el menu «/» esta abierto

function abrirBarra(desde) {
  barra = { desde }
  filtrarBarra()
}

function filtrarBarra() {
  if (!barra) return
  const st = vista.state
  const pos = st.selection.main.head
  const l = st.doc.lineAt(barra.desde)
  const fuera = pos < barra.desde + 1 || pos > l.to || st.doc.sliceString(barra.desde, barra.desde + 1) !== '/'
  const q = fuera ? '' : sinTildes(st.doc.sliceString(barra.desde + 1, pos))
  const lista = fuera
    ? []
    : CATALOGO.filter(([, , , , , req]) => !req || capacidades[req]).filter(([, clave, alias]) => {
        if (!q) return true
        if (/\s/.test(q)) return false
        return (sinTildes(tx[clave] || '') + ' ' + alias).split(/\s+/).some(p => p.startsWith(q))
      })
  if (!lista.length) {
    barra = null
    cerrarMenu()
    return
  }
  const c = vista.coordsAtPos(barra.desde)
  const desde = barra.desde
  const b = barra
  abrirMenu(
    lista.map(([orden, clave, , pista, icono]) => ({
      icono,
      texto: tx[clave],
      atajo: pista,
      accion: () => {
        const fin = vista.state.selection.main.head
        vista.dispatch({ changes: { from: desde, to: fin, insert: '' }, selection: { anchor: desde } })
        barra = null
        ORDENES[orden]()
      },
    })),
    c ? c.left : 100,
    c ? c.bottom + 4 : 100,
    {
      alCerrar: () => {
        if (barra === b) barra = null
      },
    },
  )
  barra = b
  menuActual?.elegir(0)
}

// ------------------------------------------------------------------ barra flotante

const flotante = $('flotante')
function mostrarFlotante() {
  if (!vista) return
  const st = vista.state
  const r = st.selection.main
  if (r.empty || !vista.hasFocus || menuActual) {
    flotante.hidden = true
    return
  }
  const c = vista.coordsAtPos(r.from)
  if (!c) return
  flotante.hidden = false
  const fr = flotante.getBoundingClientRect()
  let y = c.top - fr.height - 8
  if (y < 86) {
    const c2 = vista.coordsAtPos(r.to)
    y = (c2 ? c2.bottom : c.bottom) + 8
  }
  flotante.style.left = Math.max(8, Math.min(c.left - 16, innerWidth - fr.width - 8)) + 'px'
  flotante.style.top = Math.min(y, innerHeight - fr.height - 8) + 'px'
  const doc = st.doc
  const rodea = m => doc.sliceString(r.from - m.length, r.from) === m && doc.sliceString(r.to, r.to + m.length) === m
  flotante.querySelector('[data-f="negrita"]').classList.toggle('puesto', rodea('**'))
  flotante.querySelector('[data-f="cursiva"]').classList.toggle('puesto', rodea('*') && !rodea('**'))
  flotante.querySelector('[data-f="tachado"]').classList.toggle('puesto', rodea('~~'))
  flotante.querySelector('[data-f="codigo"]').classList.toggle('puesto', rodea('`'))
}
flotante.addEventListener('mousedown', e => e.preventDefault())
flotante.addEventListener('click', e => {
  const b = e.target.closest('[data-f]')
  if (!b) return
  const f = b.dataset.f
  const r = b.getBoundingClientRect()
  if (f === 'aa') {
    abrirMenu(
      [
        { texto: tx.textoNormal, accion: ORDENES.normal },
        { icono: 'H1', texto: tx.titulo, atajo: 'Ctrl+1', accion: ORDENES.t1 },
        { icono: 'H2', texto: tx.subtitulo, atajo: 'Ctrl+2', accion: ORDENES.t2 },
        { icono: 'H3', texto: tx.apartado, atajo: 'Ctrl+3', accion: ORDENES.t3 },
      ],
      r.left,
      r.bottom + 4,
    )
  } else if (f === 'listas') {
    abrirMenu(
      [
        { icono: '•', texto: tx.lista, atajo: 'Ctrl+Mayús+8', accion: ORDENES.lista },
        { icono: '1.', texto: tx.numerada, atajo: 'Ctrl+Mayús+7', accion: ORDENES.numerada },
        { icono: '☑', texto: tx.casillas, atajo: 'Ctrl+Mayús+9', accion: ORDENES.casillas },
      ],
      r.left,
      r.bottom + 4,
    )
  } else if (f === 'emoji') {
    abrirMenu(
      tx.emojis.map(([e2, n]) => ({ icono: e2, texto: n, accion: () => insertar(e2) })),
      r.left,
      r.bottom + 4,
    )
  } else {
    ORDENES[f]?.()
    requestAnimationFrame(mostrarFlotante)
  }
})

// ------------------------------------------------------------------ enlace

const cajaEnlace = $('caja-enlace')
function abrirCajaEnlace() {
  const st = vista.state
  const r = st.selection.main
  const c = vista.coordsAtPos(r.from) || { left: 100, top: 120 }
  flotante.hidden = true
  cajaEnlace.hidden = false
  cajaEnlace.value = ''
  cajaEnlace.placeholder = tx.pistaEnlace
  cajaEnlace.style.left = Math.max(8, Math.min(c.left - 16, innerWidth - 370)) + 'px'
  cajaEnlace.style.top = Math.max(90, c.top - 40) + 'px'
  navigator.clipboard
    .readText()
    .then(t => {
      if (/^(https?:\/\/|www\.)\S+$/i.test(t.trim()) && !cajaEnlace.value) {
        cajaEnlace.value = t.trim()
        cajaEnlace.select()
      }
    })
    .catch(() => {})
  cajaEnlace.focus()
  cajaEnlace.onkeydown = e => {
    if (e.key === 'Enter') {
      e.preventDefault()
      let url = cajaEnlace.value.trim()
      cajaEnlace.hidden = true
      if (!url) return vista.focus()
      if (/^www\./i.test(url)) url = 'https://' + url
      else if (url.includes('@') && !url.includes('/') && !/^mailto:/i.test(url)) url = 'mailto:' + url
      const sel = vista.state.selection.main
      const txt = vista.state.doc.sliceString(sel.from, sel.to) || url
      vista.dispatch({ changes: { from: sel.from, to: sel.to, insert: `[${txt}](${url})` } })
      vista.focus()
    } else if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      cajaEnlace.hidden = true
      vista.focus()
    }
  }
  cajaEnlace.onblur = () => (cajaEnlace.hidden = true)
}

// ------------------------------------------------------------------ tablas

/** La tabla GFM donde esta el cursor: renglones, fila y columna. */
function tablaAqui() {
  if (!vista) return null
  const st = vista.state
  const pos = st.selection.main.head
  const l = st.doc.lineAt(pos)
  if (!/^\s*\|/.test(l.text)) return null
  let a = l.number
  let b = l.number
  while (a > 1 && /^\s*\|/.test(st.doc.line(a - 1).text)) a--
  while (b < st.doc.lines && /^\s*\|/.test(st.doc.line(b + 1).text)) b++
  if (b - a < 1 || !/^\s*\|?\s*:?-{2,}/.test(st.doc.line(a + 1).text)) return null
  const filas = []
  for (let k = a; k <= b; k++) filas.push(celdas(st.doc.line(k).text))
  const antes = l.text.slice(0, pos - l.from)
  const col = Math.max(0, (antes.match(/(?<!\\)\|/g) || []).length - 1)
  return { a, b, filas, fila: l.number - a, col }
}
function enTabla() {
  return !!tablaAqui()
}
function escribirTabla(t, filas, fila, col) {
  const st = vista.state
  const n = Math.max(...filas.map(f => f.length))
  const lineas = filas.map((f, i) => {
    const c = [...f]
    while (c.length < n) c.push(i === 1 ? '---' : '')
    return '| ' + c.map(x => (x || '').replace(/(?<!\\)\|/g, '\\|')).join(' | ') + ' |'
  })
  const texto2 = lineas.join('\n')
  const desde = st.doc.line(t.a).from
  const hasta = st.doc.line(t.b).to
  const f = Math.min(fila, lineas.length - 1)
  let pos = desde
  for (let i = 0; i < f; i++) pos += lineas[i].length + 1
  const r = lineas[f]
  let k = 0
  for (let j = 0; j <= col; j++) {
    const s = r.indexOf('|', k)
    if (s < 0) break
    k = s + 1
  }
  pos += k + 1
  vista.dispatch({ changes: { from: desde, to: hasta, insert: texto2 }, selection: { anchor: Math.min(pos, desde + texto2.length) } })
  vista.focus()
}
function tablaFila(d) {
  const t = tablaAqui()
  if (!t) return
  const n = t.filas[0].length
  const i = t.fila <= 1 ? 2 : t.fila + (d > 0 ? 1 : 0)
  t.filas.splice(i, 0, Array(n).fill(''))
  escribirTabla(t, t.filas, i, 0)
}
function tablaQuitarFila() {
  const t = tablaAqui()
  if (!t || t.fila <= 1 || t.filas.length <= 3) return
  t.filas.splice(t.fila, 1)
  escribirTabla(t, t.filas, Math.min(t.fila, t.filas.length - 1), t.col)
}
function tablaColumna(d) {
  const t = tablaAqui()
  if (!t) return
  const i = t.col + (d > 0 ? 1 : 0)
  t.filas.forEach((f, k) => f.splice(i, 0, k === 1 ? '---' : ''))
  escribirTabla(t, t.filas, t.fila, i)
}
function tablaQuitarColumna() {
  const t = tablaAqui()
  if (!t || t.filas[0].length <= 1) return
  t.filas.forEach(f => f.splice(t.col, 1))
  escribirTabla(t, t.filas, t.fila, Math.max(0, t.col - 1))
}
function tablaQuitar() {
  const t = tablaAqui()
  if (!t) return
  const st = vista.state
  vista.dispatch({ changes: { from: st.doc.line(t.a).from, to: Math.min(st.doc.length, st.doc.line(t.b).to + 1), insert: '' } })
}
/** Tab: a la celda siguiente (Mayus, a la anterior); en la ultima, una fila. */
function tabEnTabla(atras) {
  const t = tablaAqui()
  if (!t) return false
  const st = vista.state
  const n = t.filas[0].length
  let fila = t.fila
  let col = t.col + (atras ? -1 : 1)
  if (col >= n) {
    col = 0
    fila = fila === 0 ? 2 : fila + 1
  } else if (col < 0) {
    col = n - 1
    fila = fila === 2 ? 0 : fila - 1
  }
  if (fila > t.b - t.a) {
    t.filas.push(Array(n).fill(''))
    escribirTabla(t, t.filas, fila, 0)
    return true
  }
  if (fila < 0) return true
  const l = st.doc.line(t.a + fila)
  let k = 0
  for (let j = 0; j <= col; j++) k = l.text.indexOf('|', k) + 1
  const fin = l.text.indexOf('|', k)
  const celda = l.text.slice(k, fin < 0 ? undefined : fin)
  const ini = k + (celda.length - celda.trimStart().length)
  vista.dispatch({ selection: { anchor: l.from + ini, head: l.from + ini + celda.trim().length } })
  return true
}

// ------------------------------------------------------------------ preferencias

let preferencias = { cuerpo: 'Work Sans', titulos: 'Fraunces', px: 16, soloEsta: false }
let capacidades = { hojas: false, medios: false }

function aplicarPreferencias() {
  const r = document.documentElement.style
  r.setProperty('--letra-cuerpo', `'${preferencias.cuerpo}', 'Segoe UI', sans-serif`)
  r.setProperty('--letra-titulos', `'${preferencias.titulos}', Georgia, serif`)
  r.setProperty('--tam', preferencias.px + 'px')
  vista?.requestMeasure()
}
function guardarPreferencias() {
  mandar({ tipo: 'vista', cuerpo: preferencias.cuerpo, titulos: preferencias.titulos, px: preferencias.px, solo_esta: preferencias.soloEsta })
}

// ------------------------------------------------------------------ aviso

let quitarAviso = null
function aviso(t) {
  const a = $('aviso')
  a.textContent = t
  a.classList.add('visible')
  clearTimeout(quitarAviso)
  quitarAviso = setTimeout(() => a.classList.remove('visible'), 2800)
}

// ------------------------------------------------------------------ adjuntar

let siguienteAdjunto = 1
function pegarArchivos(archivos) {
  for (const f of archivos) {
    if (f.size > 80 * 1024 * 1024) {
      aviso(tx.demasiadoGrande)
      continue
    }
    const id = siguienteAdjunto++
    const r = new FileReader()
    r.onload = () => mandar({ tipo: 'adjuntar', id, nombre: f.name || 'Captura.png', datos: String(r.result).split(',')[1] || '' })
    r.readAsDataURL(f)
  }
}
function alAdjuntar({ md, error }) {
  if (error || !md) return aviso(tx.noAdjuntado)
  insertarBloque(md + '\n')
}

// ------------------------------------------------------------------ dibujos

acciones.abrir = ruta => mandar({ tipo: 'abrir_pixpin', ruta })
acciones.verFoto = ruta => mandar({ tipo: 'abrir_pixpin', ruta })
function ponerAncho(desde, ancho) {
  const l = vista.state.doc.lineAt(desde)
  const m = /^(\s*!\[)([^\]|]*)(\|\d+)?(\]\(.*)$/.exec(l.text)
  if (!m) return
  vista.dispatch({ changes: { from: l.from, to: l.to, insert: m[1] + m[2] + (ancho ? '|' + ancho : '') + m[4] } })
}
acciones.menuFoto = (ruta, desde, x, y) =>
  abrirMenu(
    [
      { texto: tx.verGrande, accion: () => acciones.verFoto(ruta) },
      'linea',
      { texto: tx.pequenaFoto, accion: () => ponerAncho(desde, 240) },
      { texto: tx.medianaFoto, accion: () => ponerAncho(desde, 360) },
      { texto: tx.grandeFoto, accion: () => ponerAncho(desde, 540) },
      { texto: tx.alAncho, accion: () => ponerAncho(desde, null) },
      'linea',
      { texto: tx.quitarImagen, accion: () => quitarRenglon(desde) },
    ],
    x,
    y,
  )
acciones.menuIncrustado = (ruta, desde, x, y) =>
  abrirMenu(
    [
      { texto: /pixpin:mensaje=/.test(ruta) ? tx.irAlMensaje : tx.abrir, accion: () => acciones.abrir(ruta) },
      { texto: tx.quitarDeLaNota, accion: () => quitarRenglon(desde) },
    ],
    x,
    y,
  )
function quitarRenglon(pos) {
  const l = vista.state.doc.lineAt(pos)
  vista.dispatch({ changes: { from: l.from, to: Math.min(vista.state.doc.length, l.to + 1), insert: '' } })
}

// ------------------------------------------------------------------ teclas

function atajos() {
  const de = (key, f) => ({ key, run: () => (f(), true), preventDefault: true })
  return [
    de('Mod-b', ORDENES.negrita),
    de('Mod-i', ORDENES.cursiva),
    de('Mod-e', ORDENES.codigo),
    de('Mod-m', ORDENES.formula),
    de('Mod-Shift-x', ORDENES.tachado),
    de('Mod-k', ORDENES.enlace),
    de('Mod-1', ORDENES.t1),
    de('Mod-2', ORDENES.t2),
    de('Mod-3', ORDENES.t3),
    de('Mod-Shift-8', ORDENES.lista),
    de('Mod-Shift-(', ORDENES.lista),
    de('Mod-Shift-7', ORDENES.numerada),
    de('Mod-Shift-/', ORDENES.numerada),
    de('Mod-Shift-9', ORDENES.casillas),
    de('Mod-Shift-)', ORDENES.casillas),
    de('Mod-Enter', ORDENES.marcar),
    de('Mod-Shift-.', ORDENES.cita),
    de('Mod-Shift-:', ORDENES.cita),
    de('Mod-Shift-c', ORDENES.bloque),
    de('Mod-Shift-t', ORDENES.tabla),
    de('Mod-Shift-d', ORDENES.fecha),
    de('Mod-s', ORDENES.guardar),
    de('Mod-Shift-s', ORDENES.copia),
    de('Mod-f', ORDENES.buscar),
    { key: 'Mod-=', run: () => (cambiarTam(1), true) },
    { key: 'Mod-+', run: () => (cambiarTam(1), true) },
    { key: 'Mod--', run: () => (cambiarTam(-1), true) },
    { key: 'Tab', run: () => tabEnTabla(false) },
    { key: 'Shift-Tab', run: () => tabEnTabla(true) },
    { key: 'Enter', run: enter },
    { key: 'Backspace', run: deleteMarkupBackward },
  ]
}

function cambiarTam(d) {
  preferencias.px = Math.max(10, Math.min(32, preferencias.px + d))
  aplicarPreferencias()
  guardarPreferencias()
}

/** Intro: en un menu, elige; en una tabla, a la celda de abajo; en una
 * lista, la sigue (una tarea, sin marcar), y en un renglon solo con la
 * marca, la quita. */
function enter(view) {
  if (menuActual) return menuActual.pulsar()
  const t = tablaAqui()
  if (t) {
    if (t.fila + 1 > t.b - t.a) {
      t.filas.push(Array(t.filas[0].length).fill(''))
      escribirTabla(t, t.filas, t.filas.length - 1, t.col)
    } else escribirTabla(t, t.filas, t.fila === 0 ? 2 : t.fila + 1, t.col)
    return true
  }
  const st = view.state
  const sel = st.selection.main
  const l = st.doc.lineAt(sel.head)
  const c = RE_CASILLA.exec(l.text)
  if (c && sel.head >= l.from + c[0].length) {
    if (l.text.trim() === c[0].trim()) {
      view.dispatch({ changes: { from: l.from, to: l.to, insert: '' } })
      return true
    }
    view.dispatch(st.replaceSelection('\n' + c[1] + c[2] + c[3] + '[ ] '))
    return true
  }
  const li = RE_LISTA.exec(l.text)
  if (li && sel.head >= l.from + li[0].length && l.text.trim() === li[0].trim()) {
    view.dispatch({ changes: { from: l.from, to: l.to, insert: '' } })
    return true
  }
  return insertNewlineContinueMarkup(view)
}

let pantallaCompleta = false
let repintarCom = 0
const com = crearComentarios({
  mandar,
  vistaDe: () => vista,
  tx: () => tx,
  abrirMenu,
  insignia: $('insignia'),
  boton: $('b-comentarios'),
})
document.addEventListener(
  'keydown',
  e => {
    document.body.classList.toggle('ctrl', e.ctrlKey)
    if (menuActual) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        menuActual.mover(e.key === 'ArrowDown' ? 1 : -1)
        e.preventDefault()
        e.stopPropagation()
      } else if (e.key === 'Enter' || e.key === 'Tab') {
        if (menuActual.pulsar()) {
          e.preventDefault()
          e.stopPropagation()
        }
      } else if (e.key === 'Escape') {
        cerrarMenu()
        e.preventDefault()
        e.stopPropagation()
      }
      return
    }
    if (e.key === 'F11') {
      e.preventDefault()
      mandar({ tipo: 'ventana', accion: 'pantalla' })
    } else if (e.key === 'Escape') {
      if (!$('grande').hidden) $('grande').hidden = true
      else if (pantallaCompleta) mandar({ tipo: 'ventana', accion: 'pantalla' })
      else if (!flotante.hidden) flotante.hidden = true
      e.preventDefault()
    } else if (e.ctrlKey && e.altKey && e.key.toLowerCase() === 'm') {
      e.preventDefault()
      e.stopPropagation()
      com.comentar()
    } else if (e.ctrlKey && e.key.toLowerCase() === 'w') {
      e.preventDefault()
      mandar({ tipo: 'cerrar' })
    }
  },
  true,
)
document.addEventListener('keyup', e => document.body.classList.toggle('ctrl', e.ctrlKey))

// ------------------------------------------------------------------ montar

function crear(textoInicial) {
  vista?.destroy()
  vista = new EditorView({
    parent: $('papel'),
    state: EditorState.create({
      doc: textoInicial,
      extensions: [
        history(),
        drawSelection(),
        dropCursor(),
        search({ top: true }),
        EditorView.lineWrapping,
        markdown({ base: markdownLanguage, addKeymap: false }),
        vivo,
        resaltados,
        placeholder(tx.pista),
        EditorView.contentAttributes.of({ spellcheck: 'true', lang: 'es' }),
        keymap.of([...atajos(), ...searchKeymap, ...historyKeymap, { key: 'Mod-y', run: redo }, ...defaultKeymap]),
        EditorView.updateListener.of(u => {
          if (u.docChanged) {
            ponerTitulo()
            if (!u.transactions.some(t => t.annotation(desdeFuera))) {
              version++
              programarGuardado()
            }
            // El menu «/»: una barra escrita al principio del renglon.
            for (const t of u.transactions) {
              if (!t.isUserEvent('input.type')) continue
              t.changes.iterChanges((fa, ta, fb, tb, ins) => {
                if (ins.toString() === '/') {
                  const l = u.state.doc.lineAt(fb)
                  if (!u.state.doc.sliceString(l.from, fb).trim()) abrirBarra(fb)
                }
              })
            }
          }
          if (u.selectionSet || u.docChanged) {
            document.body.classList.toggle('en-tabla', enTabla())
            if (barra) filtrarBarra()
          }
          if (u.selectionSet || u.focusChanged) requestAnimationFrame(mostrarFlotante)
          if ((u.docChanged || u.geometryChanged) && com.abierto()) {
            cancelAnimationFrame(repintarCom)
            repintarCom = requestAnimationFrame(() => com.pintar())
          }
        }),
        EditorView.domEventHandlers({
          mousedown(e, view) {
            flotante.hidden = true
            com.alClicEnTexto(e)
            if (clicEnCasilla(view, e)) return true
            const enlace = e.target.closest?.('.pp-enlace')
            if (enlace && e.ctrlKey) {
              const url = enlace.dataset.url || ''
              if (/^pixpin:/i.test(url)) mandar({ tipo: 'abrir_pixpin', ruta: url })
              else mandar({ tipo: 'abrir_enlace', url })
              e.preventDefault()
              return true
            }
            const minuto = e.target.closest?.('.pp-minuto')
            if (minuto) {
              const audios = [...document.querySelectorAll('.pp-audio audio')].filter(
                a => a.compareDocumentPosition(minuto) & Node.DOCUMENT_POSITION_FOLLOWING,
              )
              const a = audios.pop()
              if (a) {
                a.currentTime = Number(minuto.dataset.seg)
                a.play()
              }
            }
            return false
          },
          contextmenu(e) {
            e.preventDefault()
            menuContextual(e.clientX, e.clientY)
            return true
          },
          paste(e) {
            const archivos = [...(e.clipboardData?.files || [])]
            const conTexto = (e.clipboardData?.getData('text/plain') || '').length > 0
            if (archivos.length && !conTexto) {
              e.preventDefault()
              pegarArchivos(archivos)
              return true
            }
            return false
          },
          drop(e, view) {
            const archivos = [...(e.dataTransfer?.files || [])]
            if (!archivos.length) return false
            e.preventDefault()
            const pos = view.posAtCoords({ x: e.clientX, y: e.clientY })
            if (pos != null) view.dispatch({ selection: { anchor: pos } })
            pegarArchivos(archivos)
            return true
          },
          scroll() {
            flotante.hidden = true
          },
        }),
      ],
    }),
  })
}

// La cabecera: arrastrar mueve la ventana; doble clic, maximiza.
$('arrastre').addEventListener('mousedown', e => {
  if (e.button !== 0) return
  mandar({ tipo: 'ventana', accion: e.detail >= 2 ? 'maximizar' : 'arrastrar' })
})
$('titulo-texto').addEventListener('click', editarTitulo)
$('titulo-flecha').addEventListener('click', menuTitulo)
$('b-compartir').addEventListener('click', () => {
  guardarYa()
  mandar({ tipo: 'compartir' })
})
$('b-minimizar').addEventListener('click', () => mandar({ tipo: 'ventana', accion: 'minimizar' }))
$('b-maximizar').addEventListener('click', () => mandar({ tipo: 'ventana', accion: 'pantalla' }))
$('b-cerrar').addEventListener('click', () => mandar({ tipo: 'cerrar' }))
$('b-tabla').addEventListener('click', ORDENES.tabla)
$('b-imagen').addEventListener('click', ORDENES.imagen)
$('b-casillas').addEventListener('click', ORDENES.casillas)
$('b-mas').addEventListener('click', menuMas)
$('b-comentar').addEventListener('click', ORDENES.comentar)
$('b-comentarios').addEventListener('click', () => com.abrir())
document.querySelectorAll('[data-tabla]').forEach(b =>
  b.addEventListener('click', () => {
    const o = b.dataset.tabla
    if (o === 'fila+') tablaFila(1)
    else if (o === 'fila-') tablaQuitarFila()
    else if (o === 'col+') tablaColumna(1)
    else if (o === 'col-') tablaQuitarColumna()
  }),
)
document.querySelectorAll('.herramientas button, .cabecera button').forEach(b => b.addEventListener('mousedown', e => e.preventDefault()))
$('grande').addEventListener('click', () => ($('grande').hidden = true))

// Estirar la ventana por sus bordes (no tiene marco).
const BORDE = 5
const CURSORES = { n: 'ns-resize', s: 'ns-resize', e: 'ew-resize', o: 'ew-resize', ne: 'nesw-resize', so: 'nesw-resize', no: 'nwse-resize', se: 'nwse-resize' }
function bordeEn(x, y) {
  if (pantallaCompleta) return ''
  const v = y < BORDE ? 'n' : y > innerHeight - BORDE ? 's' : ''
  const h = x < BORDE ? 'o' : x > innerWidth - BORDE ? 'e' : ''
  return v + h
}
document.addEventListener('mousemove', e => {
  const b = bordeEn(e.clientX, e.clientY)
  document.documentElement.style.cursor = b ? CURSORES[b] : ''
})
document.addEventListener(
  'mousedown',
  e => {
    const b = bordeEn(e.clientX, e.clientY)
    if (b && e.button === 0) {
      e.preventDefault()
      e.stopPropagation()
      mandar({ tipo: 'ventana', accion: 'estirar', borde: b })
    }
  },
  true,
)

// ------------------------------------------------------------------ mensajes

function recibir(msg) {
  switch (msg.tipo) {
    case 'abrir': {
      tx = TEXTOS[msg.idioma] || TEXTOS.es
      traducir()
      nombreFichero = msg.nombre || ''
      preferencias = { ...preferencias, ...(msg.vista || {}) }
      capacidades = { ...capacidades, ...(msg.capacidades || {}) }
      aplicarPreferencias()
      crear(msg.texto || '')
      com.pedir()
      version = guardada = 0
      ultimoTitulo = null
      ponerTitulo()
      vista.focus()
      break
    }
    case 'guardado':
      alGuardar(msg)
      break
    case 'adjuntado':
      alAdjuntar(msg)
      break
    case 'insertar':
      if (msg.bloque) insertarBloque(msg.texto + '\n')
      else insertar(msg.texto)
      break
    case 'aviso':
      aviso(msg.texto)
      break
    case 'comentarios':
      com.recibir(msg)
      break
    case 'hojas': {
      // El selector de hojas: las del proyecto de la nota arriba, las de
      // los demas proyectos debajo, cada uno con su nombre.
      const filas = []
      for (const g of msg.grupos || []) {
        if (!g.hojas.length) continue
        if (!g.propio) filas.push({ pista: g.proyecto })
        for (const h of g.hojas) filas.push({ icono: msg.viva ? '▣' : '🔗', texto: h.nombre, accion: () => mandar({ tipo: 'insertar_hoja', clave: h.clave, viva: !!msg.viva }) })
      }
      if (!filas.length) return aviso(tx.sinHojas)
      const c = vista.coordsAtPos(vista.state.selection.main.head)
      abrirMenu(filas, c ? c.left : 120, c ? c.bottom + 4 : 120)
      break
    }
    case 'mensajes': {
      const filas = (msg.lista || []).map(m => ({ icono: '💬', texto: m.rotulo, accion: () => mandar({ tipo: 'insertar_mensaje', clave: m.clave }) }))
      if (!filas.length) return aviso(tx.sinMensajes)
      const c = vista.coordsAtPos(vista.state.selection.main.head)
      abrirMenu(filas, c ? c.left : 120, c ? c.bottom + 4 : 120)
      break
    }
    case 'pantalla':
      pantallaCompleta = !!msg.si
      break
    case 'texto_externo': {
      if (sucio() || enVuelo !== null || msg.texto === texto()) return
      const cursor = Math.min(vista.state.selection.main.head, msg.texto.length)
      vista.dispatch({
        changes: { from: 0, to: vista.state.doc.length, insert: msg.texto },
        selection: { anchor: cursor },
        annotations: [desdeFuera.of(true), Transaction.addToHistory.of(false)],
      })
      aviso(tx.actualizada)
      break
    }
    case 'cerrando':
      cerrando = true
      guardarYa()
      if (!sucio() && enVuelo === null) mandar({ tipo: 'cerrar_listo' })
      break
  }
}

function traducir() {
  document.querySelectorAll('[data-t]').forEach(el => (el.textContent = tx[el.dataset.t] ?? el.textContent))
}

window.chrome?.webview?.addEventListener('message', e => recibir(typeof e.data === 'string' ? JSON.parse(e.data) : e.data))
mandar({ tipo: 'listo' })
if (!window.chrome?.webview) {
  recibir({ tipo: 'abrir', idioma: 'es', texto: '# Visita de obra\n\nLa losa ya está **hormigonada**.\n\n- [ ] curado\n- [x] encofrado\n' })
}
