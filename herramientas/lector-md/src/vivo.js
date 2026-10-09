// El Markdown «vivo» del editor de notas, como el de antes (pixpin-notas,
// `md_vivo` y `wysiwyg`): las marcas NO se ven nunca, ni en el renglon del
// cursor (`#`, `**`, `*`, `~~`, `` ` ``, `$`, `[`, `](url)`, `- `, `1. `,
// `[ ]`, `> `, `---`, las vallas de codigo); el cursor las salta. Lo que
// se guarda es el texto tal cual: esto solo lo pinta.
//
// Se recorre el arbol de lang-markdown (Lezer, con GFM) y se ponen
// decoraciones: clases por renglon (titulos, citas, codigo, hechas), marcas
// escondidas y dibujos (viñeta, numero, casilla, raya, foto, tabla,
// documento, audio, mensaje del chat).

import { StateField, RangeSetBuilder, EditorState } from '@codemirror/state'
import { EditorView, Decoration, WidgetType } from '@codemirror/view'
import { syntaxTree, ensureSyntaxTree } from '@codemirror/language'
import { tabla, esTablaHtml } from './tablas.js'

export const AUDIO = ['mp3', 'ogg', 'oga', 'm4a', 'wav', 'flac', 'opus', 'aac']
export const FOTO = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'avif', 'ico']

export function extension(ruta) {
  const limpia = String(ruta).split(/[?#]/)[0]
  const i = limpia.lastIndexOf('.')
  return i < 0 ? '' : limpia.slice(i + 1).toLowerCase()
}
export function nombreDe(ruta) {
  let l = String(ruta).split(/[?#]/)[0]
  try {
    l = decodeURIComponent(l)
  } catch (e) {}
  return l.split(/[\\/]/).pop() || l
}
export function esLocal(ruta) {
  return !/^(https?:|data:|blob:|mailto:|#)/i.test(ruta) && !/^pixpin:(mensaje|hoja)=/i.test(ruta)
}
export function urlDeArchivo(ruta) {
  return 'https://pixpin.nota/archivo?r=' + encodeURIComponent(ruta)
}

/** Lo que hay que hacer con un clic en un dibujo: lo pone `main.js`. */
export const acciones = {
  abrir: ruta => {},
  verFoto: ruta => {},
  menuFoto: (ruta, desde, x, y) => {},
  menuIncrustado: (ruta, desde, x, y) => {},
  saltarAudio: (seg, pos) => {},
}

// ------------------------------------------------------------------ dibujos

class Vineta extends WidgetType {
  constructor(nivel) {
    super()
    this.nivel = nivel
  }
  eq(o) {
    return o.nivel === this.nivel
  }
  toDOM() {
    const s = document.createElement('span')
    s.className = 'pp-vineta'
    s.textContent = ['•', '◦', '▪'][this.nivel % 3]
    return s
  }
}

class Numero extends WidgetType {
  constructor(n, sep) {
    super()
    this.n = n
    this.sep = sep
  }
  eq(o) {
    return o.n === this.n && o.sep === this.sep
  }
  toDOM() {
    const s = document.createElement('span')
    s.className = 'pp-numero'
    s.textContent = `${this.n}${this.sep}`
    return s
  }
}

class Casilla extends WidgetType {
  constructor(marcada) {
    super()
    this.marcada = marcada
  }
  eq(o) {
    return o.marcada === this.marcada
  }
  toDOM() {
    const s = document.createElement('span')
    s.className = 'pp-casilla' + (this.marcada ? ' marcada' : '')
    s.setAttribute('aria-checked', String(this.marcada))
    s.innerHTML = this.marcada ? '<svg viewBox="0 0 16 16"><path d="M3.5 8.5l3 3 6-7" fill="none" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>' : ''
    return s
  }
  ignoreEvent() {
    return false
  }
}

class Raya extends WidgetType {
  eq() {
    return true
  }
  toDOM() {
    const s = document.createElement('div')
    s.className = 'pp-raya'
    return s
  }
}

class Foto extends WidgetType {
  constructor(ruta, ancho, elegida) {
    super()
    this.ruta = ruta
    this.ancho = ancho
    this.elegida = elegida
  }
  eq(o) {
    return o.ruta === this.ruta && o.ancho === this.ancho && o.elegida === this.elegida
  }
  toDOM(view) {
    const caja = document.createElement('div')
    caja.className = 'pp-foto' + (this.elegida ? ' elegida' : '')
    const img = document.createElement('img')
    img.src = esLocal(this.ruta) ? urlDeArchivo(this.ruta) : this.ruta
    img.draggable = false
    if (this.ancho) img.style.width = this.ancho + 'px'
    img.onerror = () => {
      caja.classList.add('rota')
      caja.textContent = this.ruta
    }
    caja.append(img)
    if (this.elegida) {
      const asa = document.createElement('span')
      asa.className = 'pp-asa'
      caja.append(asa)
    }
    caja.addEventListener('dblclick', e => {
      e.preventDefault()
      acciones.verFoto(this.ruta)
    })
    caja.addEventListener('contextmenu', e => {
      e.preventDefault()
      acciones.menuFoto(this.ruta, view.posAtDOM(caja), e.clientX, e.clientY)
    })
    return caja
  }
  ignoreEvent(e) {
    return e.type !== 'mousedown'
  }
}

const ICONO_DOC = ext =>
  `<svg viewBox="0 0 32 40"><path d="M3 1h18l8 8v28a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V3a2 2 0 0 1 2-2z" fill="${colorDeExt(ext)}"/><path d="M21 1v8h8" fill="rgba(255,255,255,.35)"/><text x="15" y="31" text-anchor="middle" font-size="8" font-weight="700" fill="#fff" font-family="Segoe UI">${ext.slice(0, 4).toUpperCase()}</text></svg>`

function colorDeExt(ext) {
  if (ext === 'pdf') return '#e5484d'
  if (['doc', 'docx', 'odt', 'rtf'].includes(ext)) return '#2f6fde'
  if (['xls', 'xlsx', 'csv', 'ods'].includes(ext)) return '#2e9e5b'
  if (['ppt', 'pptx', 'odp'].includes(ext)) return '#e5772f'
  if (['dwg', 'dxf'].includes(ext)) return '#c9a227'
  if (['zip', 'rar', '7z'].includes(ext)) return '#8a6d3b'
  return '#6e6d69'
}

class Documento extends WidgetType {
  constructor(ruta, nombre) {
    super()
    this.ruta = ruta
    this.nombre = nombre
  }
  eq(o) {
    return o.ruta === this.ruta && o.nombre === this.nombre
  }
  toDOM(view) {
    const ext = extension(this.ruta)
    const d = document.createElement('div')
    d.className = 'pp-burbuja pp-documento'
    d.innerHTML = `<span class="pp-doc-icono">${ICONO_DOC(ext)}</span><span class="pp-doc-texto"><b></b><small></small></span><span class="pp-doc-abrir">Abrir</span>`
    d.querySelector('b').textContent = this.nombre || nombreDe(this.ruta)
    d.querySelector('small').textContent = ext.toUpperCase()
    d.addEventListener('click', e => {
      e.preventDefault()
      acciones.abrir(this.ruta)
    })
    d.addEventListener('contextmenu', e => {
      e.preventDefault()
      acciones.menuIncrustado(this.ruta, view.posAtDOM(d), e.clientX, e.clientY)
    })
    return d
  }
  ignoreEvent() {
    return true
  }
}

class Audio extends WidgetType {
  constructor(ruta, nombre) {
    super()
    this.ruta = ruta
    this.nombre = nombre
  }
  eq(o) {
    return o.ruta === this.ruta && o.nombre === this.nombre
  }
  toDOM(view) {
    const d = document.createElement('div')
    d.className = 'pp-burbuja pp-audio'
    const t = document.createElement('div')
    t.className = 'pp-audio-nombre'
    t.textContent = this.nombre || nombreDe(this.ruta)
    const a = document.createElement('audio')
    a.controls = true
    a.preload = 'metadata'
    a.src = urlDeArchivo(this.ruta)
    a.dataset.ruta = this.ruta
    d.append(t, a)
    d.addEventListener('contextmenu', e => {
      e.preventDefault()
      acciones.menuIncrustado(this.ruta, view.posAtDOM(d), e.clientX, e.clientY)
    })
    return d
  }
  ignoreEvent() {
    return true
  }
}

class Mensaje extends WidgetType {
  constructor(ruta, texto) {
    super()
    this.ruta = ruta
    this.texto = texto
  }
  eq(o) {
    return o.ruta === this.ruta && o.texto === this.texto
  }
  toDOM(view) {
    const d = document.createElement('div')
    d.className = 'pp-burbuja pp-mensaje'
    d.textContent = this.texto
    d.addEventListener('click', e => {
      e.preventDefault()
      acciones.abrir(this.ruta)
    })
    d.addEventListener('contextmenu', e => {
      e.preventDefault()
      acciones.menuIncrustado(this.ruta, view.posAtDOM(d), e.clientX, e.clientY)
    })
    return d
  }
  ignoreEvent() {
    return true
  }
}

class Tabla extends WidgetType {
  constructor(texto, desde) {
    super()
    this.texto = texto
    this.desde = desde
  }
  eq(o) {
    return o.texto === this.texto
  }
  toDOM(view) {
    const caja = document.createElement('div')
    caja.className = 'pp-tabla-caja'
    caja.append(tabla(this.texto))
    // Un clic en una celda: el cursor entra en la tabla (se edita en
    // Markdown, con su forma a la vista).
    caja.addEventListener('mousedown', e => {
      e.preventDefault()
      const td = e.target.closest('td, th')
      let pos = view.posAtDOM(caja)
      if (td && td.dataset.desde) pos = Number(td.dataset.desde) + pos
      view.dispatch({ selection: { anchor: Math.min(pos, view.state.doc.length) } })
      view.focus()
    })
    return caja
  }
  ignoreEvent() {
    return false
  }
}

// ------------------------------------------------------------------ recorrer

const escondido = Decoration.replace({})
const clase = c => Decoration.line({ class: c })
const marca = c => Decoration.mark({ class: c })

/** El renglon entero `n` es solo `![alt](ruta)` (o un enlace pixpin). */
const RE_MEDIO = /^\s*!\[([^\]]*)\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)\s*$/
const RE_PIXPIN = /^\s*\[([^\]]*)\]\((pixpin:(?:mensaje|hoja)=[^)\s]+)\)\s*$/
const RE_MINUTO = /^\s*\[(\d+):(\d{2})(?::(\d{2}))?\]/
const RE_CASILLA = /^(\s*)([-*+]|\d+[.)])(\s+)\[( |x|X)\](\s?)/
const RE_LISTA = /^(\s*)([-*+]|\d+[.)])(\s+)/

function nivelDe(sangria) {
  let n = 0
  for (const c of sangria) n += c === '\t' ? 2 : 1
  return Math.min(6, Math.floor(n / 2))
}

function construir(state) {
  const doc = state.doc
  const arbol = ensureSyntaxTree(state, doc.length, 200) || syntaxTree(state)
  const sel = state.selection.main
  const lineaCursor = doc.lineAt(sel.head).number
  const decos = []
  const pon = (desde, hasta, d) => {
    if (hasta >= desde) decos.push(d.range(desde, hasta))
  }
  // Lo que ocupa renglones enteros (tablas, vallas) para saltarlo despues.
  const tapados = new Set()

  // Los renglones que son un medio o un enlace de PixPin solos: se dibujan
  // enteros en el paso 2 y aqui no se tocan.
  const enteros = new Set()
  for (let k = 1; k <= doc.lines; k++) {
    const tx = doc.line(k).text
    if (RE_MEDIO.test(tx) || RE_PIXPIN.test(tx)) enteros.add(k)
  }

  // 1. Bloques: tablas, codigo, citas, titulos, rayas.
  arbol.iterate({
    enter(n) {
      const t = n.name
      if ((t === 'Link' || t === 'Image') && enteros.has(doc.lineAt(n.from).number)) return false
      if (t === 'Table' || (t === 'HTMLBlock' && esTablaHtml(doc.sliceString(n.from, n.to)))) {
        const l0 = doc.lineAt(n.from).number
        const l1 = doc.lineAt(n.to).number
        const dentro = lineaCursor >= l0 && lineaCursor <= l1
        if (!dentro) {
          const desde = doc.line(l0).from
          const hasta = doc.line(l1).to
          pon(desde, hasta, Decoration.replace({ widget: new Tabla(doc.sliceString(desde, hasta), desde), block: true }))
          for (let k = l0; k <= l1; k++) tapados.add(k)
        } else {
          for (let k = l0; k <= l1; k++) pon(doc.line(k).from, doc.line(k).from, clase('pp-l-tabla-fuente'))
        }
        return false
      }
      if (t === 'FencedCode' || t === 'CodeBlock') {
        const l0 = doc.lineAt(n.from).number
        const l1 = doc.lineAt(n.to).number
        for (let k = l0; k <= l1; k++) {
          const l = doc.line(k)
          const valla = t === 'FencedCode' && (k === l0 || k === l1) && /^\s*(```|~~~)/.test(l.text)
          if (valla && k !== lineaCursor) {
            pon(l.from, l.from, clase('pp-l-valla'))
            if (l.to > l.from) pon(l.from, l.to, escondido)
          } else {
            pon(l.from, l.from, clase('pp-l-codigo' + (k === l0 ? ' primero' : '') + (k === l1 ? ' ultimo' : '')))
          }
          tapados.add(k)
        }
        return false
      }
      if (t === 'Blockquote') {
        const l0 = doc.lineAt(n.from).number
        const l1 = doc.lineAt(n.to).number
        for (let k = l0; k <= l1; k++) pon(doc.line(k).from, doc.line(k).from, clase('pp-l-cita'))
      }
      const h = /^ATXHeading(\d)$/.exec(t) || /^SetextHeading(\d)$/.exec(t)
      if (h) {
        const l = doc.lineAt(n.from)
        pon(l.from, l.from, clase('pp-l-h' + h[1]))
      }
      if (t === 'HorizontalRule') {
        const l = doc.lineAt(n.from)
        pon(l.from, l.from, clase('pp-l-raya'))
        pon(l.from, l.to, Decoration.replace({ widget: new Raya() }))
        tapados.add(l.number)
        return false
      }
      // Marcas que se esconden.
      if (t === 'HeaderMark') {
        let hasta = n.to
        if (doc.sliceString(hasta, hasta + 1) === ' ') hasta++
        pon(n.from, hasta, escondido)
      } else if (t === 'EmphasisMark' || t === 'StrikethroughMark' || t === 'CodeMark') {
        pon(n.from, n.to, escondido)
      } else if (t === 'QuoteMark') {
        let hasta = n.to
        if (doc.sliceString(hasta, hasta + 1) === ' ') hasta++
        pon(n.from, hasta, escondido)
      } else if (t === 'Emphasis') {
        pon(n.from, n.to, marca('pp-cursiva'))
      } else if (t === 'StrongEmphasis') {
        pon(n.from, n.to, marca('pp-negrita'))
      } else if (t === 'Strikethrough') {
        pon(n.from, n.to, marca('pp-tachado'))
      } else if (t === 'InlineCode') {
        pon(n.from, n.to, marca('pp-codigo'))
      } else if (t === 'Link') {
        // [texto](url): se ve el texto como enlace; lo demas se esconde.
        const texto = doc.sliceString(n.from, n.to)
        const m = /^\[([^\]]*)\]\(([^)]*)\)$/.exec(texto)
        if (m) {
          pon(n.from, n.from + 1, escondido)
          pon(n.from + 1, n.from + 1 + m[1].length, Decoration.mark({ class: 'pp-enlace', attributes: { 'data-url': m[2].trim() } }))
          pon(n.from + 1 + m[1].length, n.to, escondido)
          return false
        }
      }
      return undefined
    },
  })

  // 2. Renglon a renglon: listas, casillas, medios, minutos, vacios.
  let numeros = []
  for (let k = 1; k <= doc.lines; k++) {
    const l = doc.line(k)
    if (tapados.has(k)) continue
    const texto = l.text
    if (!texto.trim()) {
      if (k !== lineaCursor) pon(l.from, l.from, clase('pp-l-vacia'))
      numeros = []
      continue
    }
    const medio = RE_MEDIO.exec(texto)
    if (medio) {
      const [, alt, ruta] = medio
      const ext = extension(ruta)
      const [nombre, ancho] = alt.split('|')
      let w = null
      if (FOTO.includes(ext) || !ext) {
        w = new Foto(ruta, Number(ancho) || null, k === lineaCursor)
      } else if (AUDIO.includes(ext)) {
        w = new Audio(ruta, nombre)
      } else {
        w = new Documento(ruta, nombre)
      }
      pon(l.from, l.from, clase('pp-l-medio'))
      pon(l.from, l.to, Decoration.replace({ widget: w }))
      continue
    }
    const px = RE_PIXPIN.exec(texto)
    if (px) {
      pon(l.from, l.from, clase('pp-l-medio'))
      pon(l.from, l.to, Decoration.replace({ widget: new Mensaje(px[2], px[1]) }))
      continue
    }
    const minuto = RE_MINUTO.exec(texto)
    if (minuto) {
      const seg = minuto[3] ? +minuto[1] * 3600 + +minuto[2] * 60 + +minuto[3] : +minuto[1] * 60 + +minuto[2]
      const i = texto.indexOf('[')
      const j = texto.indexOf(']')
      pon(l.from + i, l.from + i + 1, escondido)
      pon(l.from + i + 1, l.from + j, Decoration.mark({ class: 'pp-minuto', attributes: { 'data-seg': String(seg) } }))
      pon(l.from + j, l.from + j + 1, escondido)
    }
    const c = RE_CASILLA.exec(texto)
    const li = c || RE_LISTA.exec(texto)
    if (li) {
      const nivel = nivelDe(li[1])
      const ordenada = /\d/.test(li[2])
      const fin = l.from + li[0].length
      pon(l.from, l.from, Decoration.line({ class: 'pp-l-lista' + (c && c[4] !== ' ' ? ' pp-l-hecha' : ''), attributes: { style: `--nivel:${nivel}` } }))
      if (c) {
        pon(l.from, fin, Decoration.replace({ widget: new Casilla(c[4] !== ' ') }))
      } else if (ordenada) {
        // Se cuentan seguidas desde el primer numero, como un lector.
        const sep = li[2].slice(-1)
        const prev = numeros[nivel]
        const n = prev !== undefined ? prev + 1 : parseInt(li[2], 10)
        numeros[nivel] = n
        numeros.length = nivel + 1
        pon(l.from, fin, Decoration.replace({ widget: new Numero(n, sep) }))
      } else {
        pon(l.from, fin, Decoration.replace({ widget: new Vineta(nivel) }))
        numeros.length = nivel
      }
    } else {
      numeros = []
    }
  }

  // 3. Formulas $…$ (las reglas del movil: sin espacio por dentro, y que no
  // sigan con un numero: «$5 y $6» no es formula).
  const texto = doc.toString()
  const RE_FORMULA = /(^|[^\\$])\$([^\s$](?:[^$\n]*?[^\s$\\])?)\$(?!\d)/g
  let m
  while ((m = RE_FORMULA.exec(texto))) {
    const a = m.index + m[1].length
    const b = a + m[2].length + 2
    if (tapados.has(doc.lineAt(a).number)) continue
    pon(a, a + 1, escondido)
    pon(a + 1, b - 1, marca('pp-formula'))
    pon(b - 1, b, escondido)
  }

  decos.sort((x, y) => x.from - y.from || x.value.startSide - y.value.startSide)
  const todas = new RangeSetBuilder()
  const atomicas = new RangeSetBuilder()
  let finReemplazo = -1
  for (const d of decos) {
    // Los reemplazos no pueden solaparse: el primero gana.
    const esReemplazo = d.value.spec?.widget !== undefined || d.value === escondido
    if (esReemplazo && d.from !== d.to) {
      if (d.from < finReemplazo) continue
      finReemplazo = d.to
    }
    try {
      todas.add(d.from, d.to, d.value)
      if (esReemplazo && d.from !== d.to) atomicas.add(d.from, d.to, d.value)
    } catch (e) {
      /* un orden imposible: se salta */
    }
  }
  return { todas: todas.finish(), atomicas: atomicas.finish() }
}

export const vivo = StateField.define({
  create: construir,
  update(v, tr) {
    if (tr.docChanged || tr.selection || tr.reconfigured) return construir(tr.state)
    return v
  },
  provide: f => [EditorView.decorations.from(f, v => v.todas), EditorView.atomicRanges.of(view => view.state.field(f).atomicas)],
})

/** El renglon de un clic en una casilla: se marca o se desmarca. */
export function clicEnCasilla(view, e) {
  const c = e.target.closest?.('.pp-casilla')
  if (!c) return false
  const pos = view.posAtDOM(c)
  const l = view.state.doc.lineAt(pos)
  const m = RE_CASILLA.exec(l.text)
  if (!m) return false
  const en = l.from + m[1].length + m[2].length + m[3].length + 1
  view.dispatch({ changes: { from: en, to: en + 1, insert: m[4] === ' ' ? 'x' : ' ' }, userEvent: 'input' })
  e.preventDefault()
  return true
}

export { RE_CASILLA, RE_LISTA }
export const _probar = { construir, EditorState }
