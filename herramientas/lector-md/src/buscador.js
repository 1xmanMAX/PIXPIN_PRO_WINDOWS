// **El buscador de las notas** (Ctrl+F, 20-oct-2026). El usuario: «mejora
// su UI… quita esos botones de forma que luzca mejor». En vez del panel de
// CodeMirror (siguiente, anterior, todo, mayusculas, regex, palabra,
// reemplazar…), una caja limpia: la lupa, el texto, «3/12» y la ✕.
// Intro va al siguiente, Mayus+Intro al anterior y Esc cierra. El motor de
// busqueda es el mismo de CodeMirror (resalta todas y lleva la seleccion).

import { SearchQuery, setSearchQuery, getSearchQuery, findNext, findPrevious, closeSearchPanel } from '@codemirror/search'

/** Las coincidencias y cual es la elegida (1..n; 0 si ninguna lo es). */
function cuenta(state, q) {
  if (!q.valid || !q.search) return { n: 0, i: 0 }
  const sel = state.selection.main
  let n = 0
  let i = 0
  const c = q.getCursor(state)
  for (let r = c.next(); !r.done; r = c.next()) {
    n++
    if (r.value.from === sel.from && r.value.to === sel.to) i = n
    if (n > 9999) break
  }
  return { n, i }
}

/** El panel, para `search({ createPanel })`. */
/** `textos`: los del idioma de ahora (cambia sin rehacer el editor). */
export function crearBuscador(textos) {
  return view => {
    const dom = document.createElement('div')
    dom.className = 'pp-buscador'
    dom.innerHTML =
      '<span class="pp-buscador-lupa" aria-hidden="true"></span>' +
      '<input class="pp-buscador-texto" type="text" spellcheck="false" main-field="true">' +
      '<span class="pp-buscador-cuenta"></span>' +
      '<button class="pp-buscador-cerrar" type="button" aria-label="">✕</button>'
    const campo = dom.querySelector('input')
    const numero = dom.querySelector('.pp-buscador-cuenta')
    const cerrar = dom.querySelector('button')
    campo.placeholder = textos().buscar || 'Buscar'
    cerrar.setAttribute('aria-label', textos().cerrarBuscar || 'Cerrar')
    campo.value = getSearchQuery(view.state).search

    const pintar = () => {
      const q = getSearchQuery(view.state)
      const { n, i } = cuenta(view.state, q)
      numero.textContent = q.search ? (n ? `${i || '–'}/${n}` : '0/0') : ''
      dom.classList.toggle('sin', !!q.search && n === 0)
    }
    const poner = () => {
      view.dispatch({ effects: setSearchQuery.of(new SearchQuery({ search: campo.value })) })
      // Al escribir, la primera desde donde esta el cursor.
      if (campo.value) findNext(view)
      pintar()
    }
    campo.addEventListener('input', poner)
    campo.addEventListener('keydown', e => {
      if (e.key === 'Enter') {
        e.preventDefault()
        ;(e.shiftKey ? findPrevious : findNext)(view)
        pintar()
      } else if (e.key === 'Escape') {
        e.preventDefault()
        closeSearchPanel(view)
        view.focus()
      }
    })
    cerrar.addEventListener('click', () => {
      closeSearchPanel(view)
      view.focus()
    })
    return {
      dom,
      top: true,
      mount() {
        campo.focus()
        campo.select()
        pintar()
      },
      update(u) {
        if (u.docChanged || u.selectionSet) pintar()
      },
    }
  }
}
