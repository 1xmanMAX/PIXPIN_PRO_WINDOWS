// Las formulas de las notas, entendidas y compuestas (13-oct-2026): el
// `object Formulas` de Android (`motormd/Formulas.kt`) y su `FormulaUi.kt`,
// portados tal cual (el mismo arbol que `crates/pixpin-notas/src/formulas.rs`).
//
// `$x^2$` se lee a un arbol y se compone en HTML: una fraccion con su raya,
// un exponente pequeño y subido, una raiz con su tejado. Entiende el LaTeX de
// unas notas y la escritura corriente (`1/2`, `sqrt(x)`, `raiz(x)`, `pi`,
// `<=`). Lo que no entiende se enseña tal cual. Sin KaTeX: un navegador
// entero para poner un exponente sale caro, y esto cabe aqui.
//
// Pruebas: `node --test src/formulas.test.mjs` (los casos de FormulasTest.kt).

const SIMBOLOS = {
  alpha: 'α', beta: 'β', gamma: 'γ', delta: 'δ', epsilon: 'ε', zeta: 'ζ', eta: 'η', theta: 'θ',
  iota: 'ι', kappa: 'κ', lambda: 'λ', mu: 'μ', nu: 'ν', xi: 'ξ', pi: 'π', rho: 'ρ',
  sigma: 'σ', tau: 'τ', upsilon: 'υ', phi: 'φ', chi: 'χ', psi: 'ψ', omega: 'ω',
  Gamma: 'Γ', Delta: 'Δ', Theta: 'Θ', Lambda: 'Λ', Xi: 'Ξ', Pi: 'Π', Sigma: 'Σ', Phi: 'Φ',
  Psi: 'Ψ', Omega: 'Ω',
  times: '×', div: '÷', pm: '±', mp: '∓', cdot: '·', ast: '∗', star: '⋆',
  leq: '≤', le: '≤', geq: '≥', ge: '≥', neq: '≠', ne: '≠', approx: '≈', equiv: '≡',
  sim: '∼', propto: '∝', infty: '∞', partial: '∂', nabla: '∇',
  sum: '∑', prod: '∏', int: '∫', oint: '∮', sqrt: '√', angle: '∠', degree: '°',
  in: '∈', notin: '∉', subset: '⊂', supset: '⊃', cup: '∪', cap: '∩', emptyset: '∅', forall: '∀',
  exists: '∃', neg: '¬', land: '∧', lor: '∨',
  rightarrow: '→', to: '→', leftarrow: '←', leftrightarrow: '↔', Rightarrow: '⇒', Leftarrow: '⇐',
  ldots: '…', cdots: '⋯', perp: '⊥', parallel: '∥',
}
const FUNCIONES = new Set(['sin', 'cos', 'tan', 'cot', 'sec', 'csc', 'arcsin', 'arccos', 'arctan', 'sinh', 'cosh', 'tanh',
  'log', 'ln', 'exp', 'lim', 'max', 'min', 'det', 'gcd', 'mod'])
const PALABRAS = { inf: '∞', infinito: '∞', infinity: '∞', grados: '°', deg: '°', sum: '∑', suma: '∑', prod: '∏', int: '∫', raiz: '√' }
const PAREJAS = [['<=', '≤'], ['>=', '≥'], ['!=', '≠'], ['/=', '≠'], ['+-', '±'], ['-+', '∓'], ['==', '≡'], ['~=', '≈'],
  ['->', '→'], ['=>', '⇒'], ['<-', '←'], ['<->', '↔']]

const has = (o, k) => Object.prototype.hasOwnProperty.call(o, k)
const esLetra = c => /\p{L}/u.test(c)
const esCifra = c => c >= '0' && c <= '9'
const texto = (t, cursiva = false) => ({ t: 'texto', texto: t, cursiva })

/** Lee una formula a su arbol. Nunca falla. */
export function leer(formula) {
  const s = Array.from(formula)
  let i = 0
  const fin = f => i >= s.length || (f != null && s[i] === f)
  const saltar = () => { while (s[i] === ' ') i++ }

  function expresion(f) {
    const partes = [termino(f)]
    for (;;) {
      // Los espacios se saltan antes de mirar el signo (`1/2 + x`).
      saltar()
      if (fin(f)) break
      const signo = signoSuelto()
      if (signo == null) break
      partes.push(texto(` ${signo} `))
      partes.push(termino(f))
    }
    return partes.length === 1 ? partes[0] : { t: 'fila', partes }
  }
  function signoSuelto() {
    for (const [escrito, simbolo] of PAREJAS) {
      const e = Array.from(escrito)
      if (e.every((c, k) => s[i + k] === c)) { i += e.length; return simbolo }
    }
    const c = s[i]
    if (c != null && '+-=<>≤≥≠≈'.includes(c)) { i++; return c }
    return null
  }
  // La barra hace una fraccion de verdad; pegado es multiplicado.
  function termino(f) {
    let izq = factor(f)
    for (;;) {
      const antes = i
      saltar()
      if (fin(f)) { i = antes; break }
      const c = s[i]
      if (c === '/') { i++; izq = { t: 'fraccion', arriba: izq, abajo: factor(f) } }
      else if (c === '*') { i++; izq = { t: 'fila', partes: [izq, texto('·'), factor(f)] } }
      else if (esDeFactor(c)) izq = { t: 'fila', partes: [izq, factor(f)] }
      else { i = antes; break }
    }
    return izq
  }
  const esDeFactor = c => esLetra(c) || esCifra(c) || c === '(' || c === '{' || c === '\\' || c === '.'
  function factor(f) {
    let base = atomo(f)
    while (s[i] === '^' || s[i] === '_') {
      const arriba = s[i] === '^'
      i++
      base = conIndice(base, atomo(f), arriba)
    }
    return base
  }
  function atomo(f) {
    saltar()
    if (fin(f)) return texto('')
    const c = s[i]
    if (c === '\\') return mandato()
    if (c === '(' || c === '[') return agrupado(c)
    if (c === '{') { i++; const d = expresion('}'); if (i < s.length) i++; return d }
    if (esCifra(c) || c === '.') return numero()
    if (esLetra(c)) return palabra()
    i++
    return texto(c)
  }
  function numero() {
    let n = ''
    while (i < s.length && (esCifra(s[i]) || s[i] === '.' || s[i] === ',')) n += s[i++]
    return texto(n)
  }
  function palabra() {
    const desde = i
    let w = ''
    while (i < s.length && esLetra(s[i])) w += s[i++]
    const minus = w.toLowerCase()
    if (w === 'sqrt' || minus === 'raiz' || minus === 'raíz') return { t: 'raiz', dentro: argumento(), indice: null }
    if (FUNCIONES.has(minus)) return { t: 'fila', partes: [texto(minus), argumento()] }
    if (has(SIMBOLOS, w)) return texto(SIMBOLOS[w])
    if (has(PALABRAS, minus)) return texto(PALABRAS[minus])
    // Letras sueltas: cada una es una variable.
    i = desde + 1
    return texto(w[0], true)
  }
  function argumento() {
    saltar()
    if (s[i] === '(' || s[i] === '{') {
      const cierra = s[i] === '(' ? ')' : '}'
      i++
      const dentro = expresion(cierra)
      if (i < s.length) i++
      return dentro
    }
    return atomo(null)
  }
  function grupo() {
    if (i >= s.length) return texto('')
    if (s[i] === '{') { i++; const d = expresion('}'); if (i < s.length) i++; return d }
    if (s[i] === '\\') return mandato()
    const c = s[i++]
    return texto(c, esLetra(c))
  }
  function mandato() {
    i++
    let p = ''
    while (i < s.length && esLetra(s[i])) p += s[i++]
    if (p === 'frac' || p === 'dfrac' || p === 'tfrac') { const a = grupo(); return { t: 'fraccion', arriba: a, abajo: grupo() } }
    if (p === 'sqrt') {
      let indice = null
      if (s[i] === '[') { i++; indice = expresion(']'); if (i < s.length) i++ }
      return { t: 'raiz', dentro: grupo(), indice }
    }
    if (p === 'left' || p === 'right') return texto('')
    if (FUNCIONES.has(p)) return texto(p)
    if (has(SIMBOLOS, p)) return texto(SIMBOLOS[p])
    if (p === '') { const c = i < s.length ? s[i] : ''; i++; return texto(c) }
    return texto('\\' + p)
  }
  function agrupado(abre) {
    const cierra = abre === '(' ? ')' : ']'
    const desde = i
    i++
    const dentro = expresion(cierra)
    if (s[i] === cierra) { i++; return { t: 'agrupado', dentro, abre, cierra } }
    i = desde + 1
    return texto(abre)
  }
  return expresion(null)
}

function conIndice(base, indice, arriba) {
  if (base.t === 'indices') return arriba ? { ...base, arriba: indice } : { ...base, abajo: indice }
  return arriba ? { t: 'indices', base, arriba: indice, abajo: null } : { t: 'indices', base, arriba: null, abajo: indice }
}

/** El arbol en texto plano (el de FormulasTest): `(a/b)`, `x^2_i`, `sqrt(x)`. */
export function plano(p) {
  switch (p.t) {
    case 'texto': return p.texto
    case 'fila': return p.partes.map(plano).join('')
    case 'fraccion': return `(${plano(p.arriba)}/${plano(p.abajo)})`
    case 'indices': return plano(p.base) + (p.arriba ? '^' + plano(p.arriba) : '') + (p.abajo ? '_' + plano(p.abajo) : '')
    case 'raiz': return `sqrt(${plano(p.dentro)})`
    case 'agrupado': return p.abre + plano(p.dentro) + p.cierra
  }
  return ''
}

const escapar = t => t.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

/**
 * **Compone el arbol en HTML** (`FormulaUi.kt`): cada construccion es una
 * disposicion (la fraccion, una columna con raya; el exponente, mas pequeño
 * y subido; la raiz, su signo y una raya encima de lo de dentro). Lo de
 * dentro va mas pequeño por la clase, asi un exponente dentro de otro sale
 * bien sin casos especiales.
 */
export function html(p) {
  switch (p.t) {
    case 'texto':
      return p.cursiva ? `<i>${escapar(p.texto)}</i>` : `<span>${escapar(p.texto)}</span>`
    case 'fila':
      return p.partes.map(html).join('')
    case 'fraccion':
      return `<span class="ppf-frac"><span class="ppf-num">${html(p.arriba)}</span><span class="ppf-den">${html(p.abajo)}</span></span>`
    case 'indices': {
      const base = html(p.base)
      if (p.arriba && p.abajo) return `${base}<span class="ppf-ind"><span>${html(p.arriba)}</span><span>${html(p.abajo)}</span></span>`
      if (p.arriba) return `${base}<sup class="ppf-sup">${html(p.arriba)}</sup>`
      return `${base}<sub class="ppf-sub">${html(p.abajo)}</sub>`
    }
    case 'raiz':
      return `<span class="ppf-raiz">${p.indice ? `<sup class="ppf-raiz-i">${html(p.indice)}</sup>` : ''}<span class="ppf-signo">√</span><span class="ppf-dentro">${html(p.dentro)}</span></span>`
    case 'agrupado':
      return `<span class="ppf-par">${escapar(p.abre)}</span>${html(p.dentro)}<span class="ppf-par">${escapar(p.cierra)}</span>`
  }
  return ''
}

/** La formula `latex` lista para la pagina. */
export function componer(latex) {
  return html(leer(latex))
}
