// **Las marcas sobre el texto de una pagina** (2-nov-2026): resaltar,
// raya ondulada, subrayar, tachar y tapar lo elegido leyendo, como en el
// lector de PDF y en los pines (`pindesk_motor2d::marcas_texto`), pero en las
// paginas que se reajustan: una nota Markdown (el editor de notas) y una
// pagina web (el visor de HTML). Aqui solo las cuentas, sin pagina: se
// prueban con `node --test` (`marcas-texto.test.mjs`).
//
// Una cajita en la pantalla no sirve de ancla: el texto se recoloca con el
// ancho de la ventana (y del movil). El ancla es **la cita**, como las de la
// W3C (Web Annotation): `TextQuoteSelector` (lo marcado con un poco de antes
// y de despues) y `TextPositionSelector` (donde estaba, en unidades UTF-16
// del texto, las de JavaScript y Kotlin). Al abrir se busca la cita; si sale
// mas de una vez gana la del mismo contexto y luego la mas cercana a donde
// estaba; si ya no esta tal cual, lo que hay entre su contexto de antes y el
// de despues; si tampoco, la parecida (unas pocas letras cambiadas). Si nada,
// la marca queda **sin sitio**: no se pinta, pero no se pierde (vuelve si el
// texto vuelve). El formato del fichero: `pindesk_docs::ancla_texto`.

/** Las marcas, en el orden de la barra (los nombres del fichero). */
export const MARCAS = ['resaltar', 'ondulada', 'subrayar', 'tachar', 'tapar']

/** Rosa, lila, azul, verde y amarillo (`marcas_texto::COLORES`). */
export const COLORES = ['#F47286', '#B99CF5', '#5BA4F5', '#3FC88C', '#F5C218']

/** El amarillo de serie (`marcas_texto::COLOR_INICIAL`). */
export const COLOR_INICIAL = 4

/** Lo que llevan delante los ids (el grupo de las marcas del PDF). */
export const GRUPO = 'marca-texto'

/** Cuantas unidades de contexto se guardan a cada lado de la cita. */
export const CONTEXTO = 32

const blanco = c => /\s/.test(c)
// Una mitad de un par sustituto no se corta: el borde va antes o despues.
const alta = c => c >= 0xd800 && c <= 0xdbff
const baja = c => c >= 0xdc00 && c <= 0xdfff
function entero(t, a, b) {
  a = Math.max(0, Math.min(a, t.length))
  b = Math.max(a, Math.min(b, t.length))
  if (a > 0 && a < t.length && baja(t.charCodeAt(a)) && alta(t.charCodeAt(a - 1))) a--
  if (b > 0 && b < t.length && baja(t.charCodeAt(b)) && alta(t.charCodeAt(b - 1))) b++
  return [a, b]
}

/**
 * **El ancla de lo elegido** `desde..hasta` en `texto`: sin los blancos de
 * los bordes. `null` si no queda nada.
 */
export function anclar(texto, desde, hasta) {
  let [a, b] = entero(texto, Math.min(desde, hasta), Math.max(desde, hasta))
  while (a < b && blanco(texto[a])) a++
  while (b > a && blanco(texto[b - 1])) b--
  if (a >= b) return null
  const [ca] = entero(texto, a - CONTEXTO, a)
  const [, cb] = entero(texto, b, b + CONTEXTO)
  return {
    cita: { type: 'TextQuoteSelector', exact: texto.slice(a, b), prefix: texto.slice(ca, a), suffix: texto.slice(b, cb) },
    posicion: { type: 'TextPositionSelector', start: a, end: b },
  }
}

function apariciones(t, aguja, tope = 2000) {
  const v = []
  if (!aguja) return v
  for (let i = t.indexOf(aguja); i >= 0 && v.length < tope; i = t.indexOf(aguja, i + 1)) v.push(i)
  return v
}
function comunDetras(a, b) {
  let n = 0
  while (n < a.length && n < b.length && a[a.length - 1 - n] === b[b.length - 1 - n]) n++
  return n
}
function comunDelante(a, b) {
  let n = 0
  while (n < a.length && n < b.length && a[n] === b[n]) n++
  return n
}
/** Cuanto del contexto guardado sigue alrededor de `i..j`. */
function contexto(t, i, j, antes, despues) {
  return comunDetras(t.slice(Math.max(0, i - antes.length), i), antes) + comunDelante(t.slice(j, j + despues.length), despues)
}

/** La cita cambio: lo que hay entre los dos contextos (como en
 * `md_comentarios::por_contexto`). */
function porContexto(t, pos, n, antes, despues) {
  if (antes.length + despues.length < 8) return null
  const limite = n * 2 + 64
  const inicios = antes ? apariciones(t, antes).map(i => i + antes.length) : pos > limite ? [] : [0]
  let mejor = null
  for (const i of inicios) {
    let j
    if (!despues) j = t.length - i <= limite ? t.length : -1
    else {
      const d = t.slice(i, i + limite + despues.length).indexOf(despues)
      j = d < 0 ? -1 : i + d
    }
    if (j > i && /\S/.test(t.slice(i, j)) && (!mejor || Math.abs(i - pos) < Math.abs(mejor[0] - pos))) mejor = [i, j]
  }
  return mejor
}

/**
 * La distancia de edicion de `q` con el mejor trozo de `v` que acaba en
 * cada sitio (Sellers: empezar en cualquier sitio de `v` no cuesta). Devuelve
 * `[distancia, fin]` del mejor (el primero si empatan).
 */
function mejorFin(q, v) {
  const n = q.length
  let prev = new Array(n + 1)
  for (let k = 0; k <= n; k++) prev[k] = k
  let mejor = [prev[n], 0]
  for (let j = 1; j <= v.length; j++) {
    const fila = new Array(n + 1)
    fila[0] = 0
    for (let k = 1; k <= n; k++) {
      const c = q[k - 1] === v[j - 1] ? 0 : 1
      fila[k] = Math.min(prev[k - 1] + c, prev[k] + 1, fila[k - 1] + 1)
    }
    if (fila[n] < mejor[0]) mejor = [fila[n], j]
    prev = fila
  }
  return mejor
}
const alReves = s => s.split('').reverse().join('')

/**
 * **La cita parecida** (unas pocas letras cambiadas: una errata corregida,
 * una coma de mas). Se buscan trozos de la cita que sigan tal cual (si hay
 * `k` cambios, de `k + 1` trozos al menos uno sigue entero) y alrededor de
 * cada uno el trozo de texto mas parecido. Vale con un cambio por cada cinco
 * letras como mucho; una cita de menos de 5 letras no se busca asi (casaria
 * con cualquier cosa).
 */
export function parecida(t, q, pos, antes = '', despues = '') {
  const n = q.length
  if (n < 5 || n > 2000) return null
  const k = Math.max(1, Math.floor(n / 5))
  const largo = Math.max(3, Math.floor(n / (k + 1)))
  const vistos = new Set()
  let mejor = null
  for (let o = 0; o + largo <= n; o += largo) {
    for (const i of apariciones(t, q.slice(o, o + largo), 200)) {
      const c = i - o
      const clave = Math.floor(c / 4)
      if (vistos.has(clave)) continue
      vistos.add(clave)
      const v0 = Math.max(0, c - k)
      const v = t.slice(v0, Math.min(t.length, c + n + k))
      const [d, fin] = mejorFin(q, v)
      if (d > k) continue
      // El principio: lo mismo desde el final, al reves.
      const izq = v.slice(0, fin)
      const [, finR] = mejorFin(alReves(q), alReves(izq))
      let a = v0 + izq.length - finR
      let b = v0 + fin
      ;[a, b] = entero(t, a, b)
      if (a >= b) continue
      const ctx = contexto(t, a, b, antes, despues)
      const cand = { a, b, d, ctx, lejos: Math.abs(a - pos) }
      if (!mejor || cand.d < mejor.d || (cand.d === mejor.d && (cand.ctx > mejor.ctx || (cand.ctx === mejor.ctx && cand.lejos < mejor.lejos)))) mejor = cand
    }
  }
  return mejor ? [mejor.a, mejor.b] : null
}

/**
 * **Donde esta hoy lo marcado** en `texto`: `[desde, hasta]` o `null`.
 * Primero la cita tal cual (mismo contexto, lo mas cerca de donde estaba);
 * luego lo de entre sus contextos; luego la parecida.
 */
export function ubicar(texto, m) {
  const q = m?.cita?.exact || ''
  if (!q) return null
  const antes = m.cita.prefix || ''
  const despues = m.cita.suffix || ''
  const pos = m.posicion?.start ?? 0
  // Lo de siempre: el texto no cambio y la cita esta en su sitio.
  if (texto.startsWith(q, pos) && contexto(texto, pos, pos + q.length, antes, despues) === antes.length + despues.length) return [pos, pos + q.length]
  let mejor = null
  for (const i of apariciones(texto, q)) {
    const c = contexto(texto, i, i + q.length, antes, despues)
    if (!mejor || c > mejor.c || (c === mejor.c && Math.abs(i - pos) < Math.abs(mejor.i - pos))) mejor = { i, c }
  }
  if (mejor) return [mejor.i, mejor.i + q.length]
  // El contexto entero y, si tambien se toco, lo mas pegado a la cita.
  const largos = l => [l, ...[16, 8].filter(x => x < l)]
  for (const na of largos(antes.length))
    for (const nd of largos(despues.length)) {
      const r = porContexto(texto, pos, q.length, antes.slice(antes.length - na), despues.slice(0, nd))
      if (r) return r
    }
  return parecida(texto, q, pos, antes, despues)
}

/** Busca la marca y, si esta, pone su ancla al dia (cita, contexto y sitio
 * de ahora). Devuelve donde esta o `null` (el ancla se queda como estaba). */
export function reanclar(texto, m) {
  const r = ubicar(texto, m)
  if (!r) return null
  const n = anclar(texto, r[0], r[1])
  if (!n) return null
  m.cita = { ...m.cita, ...n.cita }
  m.posicion = { ...m.posicion, ...n.posicion }
  return [n.posicion.start, n.posicion.end]
}

/** Un id nuevo, `marca-texto-<ms>` (como el grupo de las del PDF), sin
 * repetir ninguno de `marcas`. */
export function idNuevo(ahora, marcas) {
  const usados = new Set(marcas.map(m => m.id))
  let ms = Math.floor(ahora)
  while (usados.has(`${GRUPO}-${ms}`)) ms++
  return `${GRUPO}-${ms}`
}

/** Una marca nueva sobre `desde..hasta` (`null` si no hay texto). */
export function nueva(texto, desde, hasta, marca, color, ahora, marcas) {
  const a = anclar(texto, desde, hasta)
  if (!a || !MARCAS.includes(marca)) return null
  return { id: idNuevo(ahora, marcas), marca, color: COLORES[color] || COLORES[COLOR_INICIAL], cuando: Math.floor(ahora), ...a }
}

/** Las marcas (con su sitio de ahora, `sitios[id] = [a, b]`) que pisan
 * `desde..hasta`. Un rango vacio pisa la que lo contiene. */
export function pisadas(marcas, sitios, desde, hasta) {
  return marcas.filter(m => {
    const s = sitios[m.id]
    if (!s) return false
    return desde === hasta ? s[0] <= desde && desde < s[1] : s[0] < hasta && desde < s[1]
  })
}

/**
 * **Deshacer y rehacer** las marcas (Ctrl+Z / Ctrl+Y dentro de la pagina):
 * se guarda la lista entera de antes de cada cambio (son pocas y cortas).
 */
export class Historia {
  constructor() {
    this.atras = []
    this.adelante = []
  }
  /** Antes de cambiar: lo de ahora a la pila. */
  apuntar(marcas) {
    this.atras.push(JSON.stringify(marcas))
    if (this.atras.length > 200) this.atras.shift()
    this.adelante = []
  }
  hayAtras() {
    return this.atras.length > 0
  }
  hayAdelante() {
    return this.adelante.length > 0
  }
  /** La lista de antes (o `null`). */
  deshacer(marcas) {
    if (!this.atras.length) return null
    this.adelante.push(JSON.stringify(marcas))
    return JSON.parse(this.atras.pop())
  }
  rehacer(marcas) {
    if (!this.adelante.length) return null
    this.atras.push(JSON.stringify(marcas))
    return JSON.parse(this.adelante.pop())
  }
}

/** Lo que pinta cada marca: color de fondo, raya, letra (`tapar` no deja
 * leer). `oscuro` no cambia los colores: son los mismos en los dos temas,
 * como en el PDF. */
export function estiloDe(m) {
  const c = /^#[0-9a-f]{6}$/i.test(m.color || '') ? m.color : COLORES[COLOR_INICIAL]
  switch (m.marca) {
    case 'resaltar':
      return { fondo: c + '66' }
    case 'ondulada':
      return { raya: `underline wavy ${c}`, grosor: '1.5px' }
    case 'subrayar':
      return { raya: `underline ${c}`, grosor: '2px' }
    case 'tachar':
      return { raya: `line-through ${c}`, grosor: '2px' }
    case 'tapar':
      return { fondo: c, letra: c }
    default:
      return null
  }
}

/** La clase CSS de una marca (una por marca y color: `pp-mt-resaltar-4`). */
export function claseDe(m) {
  const k = COLORES.findIndex(c => c.toLowerCase() === (m.color || '').toLowerCase())
  return `pp-mt-${MARCAS.includes(m.marca) ? m.marca : 'resaltar'}-${k < 0 ? COLOR_INICIAL : k}`
}

/** Las reglas CSS de todas las clases (`claseDe`), con `sel(clase)` para el
 * selector (`.pp-mt-…` en la nota, `::highlight(pp-mt-…)` en una pagina). */
export function reglasCss(sel) {
  const r = []
  for (const marca of MARCAS)
    COLORES.forEach((color, k) => {
      const e = estiloDe({ marca, color })
      const d = []
      if (e.fondo) d.push(`background-color:${e.fondo}`)
      if (e.raya) d.push(`text-decoration:${e.raya}`, `text-decoration-thickness:${e.grosor}`, 'text-decoration-skip-ink:none')
      if (e.letra) d.push(`color:${e.letra}`)
      r.push(`${sel(`pp-mt-${marca}-${k}`)}{${d.join(';')}}`)
    })
  return r.join('\n')
}
