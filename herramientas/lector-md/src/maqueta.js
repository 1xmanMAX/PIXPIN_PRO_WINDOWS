// **La maqueta de la tinta** (20-oct-2026): la medida de la columna con que
// se anoto una nota, para que la tinta caiga siempre sobre lo anotado. Ver
// `apps/pixpin/src/visor_html/capa_web.rs` (`Maqueta`) y el riel, que la
// guarda y decide; aqui solo se mide la columna, se pone a una medida y se
// dice cuanto se movio (el desfase con que se cuelga la capa de la tinta).
//
// El riel la usa por `window.__pixpinMaqueta`:
//   medir()        -> {ancho, tam, ox} de ahora (pixeles CSS)
//   poner(m, fija) -> la maqueta guardada (o null) y si la columna va a ella
//   desfase(s)     -> lo que la columna se corrio de lado desde que se anoto

let maqueta = null
let fija = false

const columna = () => document.querySelector('.papel .cm-content')
const tamDeLetra = () =>
  parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--tam')) || 16

/** El borde izquierdo de la columna dentro de la caja que se desplaza. */
function izquierda(c, s) {
  const caja = s.getBoundingClientRect()
  return c.getBoundingClientRect().left - caja.left - s.clientLeft + s.scrollLeft
}

function aplicar(m) {
  const c = columna()
  if (!c) return
  document.documentElement.style.setProperty('--tam', m.tam + 'px')
  Object.assign(c.style, {
    boxSizing: 'border-box',
    width: m.ancho + 'px',
    minWidth: m.ancho + 'px',
    maxWidth: 'none',
    flex: 'none',
  })
  document.body.classList.add('pp-maqueta-fija')
}

function soltar() {
  const c = columna()
  if (c) for (const k of ['boxSizing', 'width', 'minWidth', 'maxWidth', 'flex']) c.style[k] = ''
  document.body.classList.remove('pp-maqueta-fija')
}

export function instalarMaqueta(alCambiar) {
  window.__pixpinMaqueta = {
    medir() {
      const c = columna()
      const s = c?.closest('.cm-scroller')
      if (!c || !s) return null
      return { ancho: c.offsetWidth, tam: tamDeLetra(), ox: izquierda(c, s) }
    },
    poner(m, siFija) {
      const antes = fija
      maqueta = m && m.ancho > 0 ? m : null
      fija = !!(siFija && maqueta)
      if (fija) aplicar(maqueta)
      else if (antes) soltar()
      if (fija !== antes) alCambiar?.()
    },
    desfase(s) {
      const c = columna()
      if (!maqueta || !c || !s || !s.contains(c)) return 0
      return izquierda(c, s) - (maqueta.ox || 0)
    },
  }
}

/** Si el editor se rehizo (otra columna), la medida otra vez. */
export function reaplicarMaqueta() {
  if (fija && maqueta) aplicar(maqueta)
}
