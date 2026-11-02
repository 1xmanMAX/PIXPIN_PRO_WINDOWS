// **Marcar el texto de la nota leyendo** (2-nov-2026): lo elegido con el
// raton se resalta, se subraya (recto u ondulado), se tacha o se tapa, con
// los botones de la barrita del PDF en la misma burbuja que «Comentar».
//
// Las marcas van sobre el Markdown (las mismas posiciones que los
// comentarios, `comentarios.js`), como decoraciones de CodeMirror: siguen al
// texto al escribir y al cambiar el ancho. PinDesk las guarda en su fichero
// (`anot-<uid>.marcas-texto.json`, `pindesk_docs::ancla_texto`) y aqui se
// vuelven a anclar por su cita al abrir (`marcas-texto.mjs`). Ctrl+Z y
// Ctrl+Y leyendo deshacen y rehacen la ultima.

import { StateField, StateEffect } from '@codemirror/state'
import { EditorView, Decoration } from '@codemirror/view'
import * as mt from './marcas-texto.mjs'

const poner = StateEffect.define()

/** Lo pintado: una decoracion por marca con sitio. */
export const marcasPintadas = StateField.define({
  create: () => Decoration.none,
  update(v, tr) {
    v = v.map(tr.changes)
    for (const e of tr.effects) if (e.is(poner)) v = e.value
    return v
  },
  provide: f => EditorView.decorations.from(f),
})

/** El estilo de las clases (`marcas-texto.mjs::reglasCss`), una vez. */
function ponerEstilo() {
  if (document.getElementById('pp-mt-estilo')) return
  const s = document.createElement('style')
  s.id = 'pp-mt-estilo'
  s.textContent = mt.reglasCss(c => `.${c}`) + '\n.pp-mt{border-radius:2px;cursor:pointer}'
  document.head.append(s)
}

export function crearMarcas({ vistaDe, mandar }) {
  let marcas = []
  let color = mt.COLOR_INICIAL
  // La marca elegida con un clic (su id): otra marca la cambia, un color la
  // pinta de ese color y el circulo la quita.
  let elegida = null
  const historia = new mt.Historia()
  let temporizador = 0
  ponerEstilo()

  const texto = () => vistaDe().state.doc.toString()

  /** Donde esta cada marca ahora (lo pintado, que sigue al texto). */
  function sitios() {
    const s = {}
    const v = vistaDe()
    if (!v) return s
    v.state.field(marcasPintadas).between(0, v.state.doc.length, (a, b, d) => {
      const id = d.spec.attributes?.['data-mt']
      if (id && b > a) s[id] = [a, b]
    })
    return s
  }

  /** Busca cada marca por su cita y la pinta donde este. */
  function pintar() {
    const v = vistaDe()
    if (!v) return
    const t = texto()
    const rangos = []
    for (const m of marcas) {
      const r = mt.ubicar(t, m)
      if (r) rangos.push(Decoration.mark({ class: 'pp-mt ' + mt.claseDe(m) + (m.id === elegida ? ' pp-mt-elegida' : ''), attributes: { 'data-mt': m.id } }).range(r[0], r[1]))
    }
    v.dispatch({ effects: poner.of(Decoration.set(rangos, true)) })
  }

  /** Las anclas al dia con lo pintado (se escribio dentro de una marca o a
   * su lado): solo se manda si alguna cita cambio. */
  function actualizar() {
    const t = texto()
    const s = sitios()
    let cambio = false
    for (const m of marcas) {
      const r = s[m.id]
      if (!r) continue
      const n = mt.anclar(t, r[0], r[1])
      if (!n) continue
      if (n.cita.exact !== m.cita.exact) cambio = true
      m.cita = n.cita
      m.posicion = n.posicion
    }
    if (cambio) guardar()
  }

  function guardar() {
    mandar({ tipo: 'marcas_texto', marcas })
  }

  function cambiar(nuevas) {
    historia.apuntar(marcas)
    marcas = nuevas
    pintar()
    guardar()
  }

  /** Lo que pulsa la burbuja. `rango`: lo elegido (posiciones del texto). */
  function pulsar(que, rango) {
    if (typeof que === 'object') {
      color = que.color
      if (elegida) cambiar(marcas.map(m => (m.id === elegida ? { ...m, color: mt.COLORES[color] } : m)))
      return 'color'
    }
    if (que === 'quitar') {
      const quitar = elegida ? marcas.filter(m => m.id === elegida) : rango ? mt.pisadas(marcas, sitios(), rango.from, rango.to) : []
      elegida = null
      if (quitar.length) cambiar(marcas.filter(m => !quitar.includes(m)))
      return 'hecho'
    }
    if (!mt.MARCAS.includes(que)) return null
    if (elegida) {
      const id = elegida
      cambiar(marcas.map(m => (m.id === id ? { ...m, marca: que, color: mt.COLORES[color] } : m)))
      return 'hecho'
    }
    if (!rango || rango.from === rango.to) return null
    const m = mt.nueva(texto(), rango.from, rango.to, que, color, Date.now(), marcas)
    if (m) cambiar([...marcas, m])
    return 'hecho'
  }

  return {
    recibir(lista) {
      marcas = Array.isArray(lista) ? lista.filter(m => m && m.id && m.cita) : []
      elegida = null
      pintar()
    },
    pintar,
    pulsar,
    color: () => color,
    /** Un clic sobre una marca la elige; devuelve su rango o `null`. */
    elegirEn(target) {
      const el = target?.closest?.('.pp-mt')
      const id = el?.dataset.mt || null
      if (id === elegida) return id ? sitios()[id] : null
      elegida = id
      pintar()
      const r = id && sitios()[id]
      return r || null
    },
    soltar() {
      if (!elegida) return
      elegida = null
      pintar()
    },
    elegida: () => elegida,
    /** La nota cambio: escrita aqui (las marcas siguen solas; se ponen al
     * dia al rato) o entera de fuera (se buscan otra vez por su cita). */
    alCambiar(deFuera) {
      clearTimeout(temporizador)
      if (deFuera) pintar()
      else temporizador = setTimeout(actualizar, 700)
    },
    deshacer() {
      const l = historia.deshacer(marcas)
      if (!l) return false
      marcas = l
      elegida = null
      pintar()
      guardar()
      return true
    },
    rehacer() {
      const l = historia.rehacer(marcas)
      if (!l) return false
      marcas = l
      pintar()
      guardar()
      return true
    },
    hayAtras: () => historia.hayAtras(),
  }
}
