// Los comentarios de la nota, como en el editor de antes (pixpin-notas,
// `panel_comentarios.rs`): tarjetas a la derecha a la altura de su frase,
// el texto comentado resaltado, responder, resolver, editar y borrar. Las
// cuentas (anclar, reanclar, fusionar) las hace PixPin (`comentarios_web.rs`):
// aqui solo se pintan y se piden cambios.

import { StateField, StateEffect } from '@codemirror/state'
import { EditorView, Decoration } from '@codemirror/view'

const poner = StateEffect.define()

/** Lo resaltado: los hilos abiertos con sitio; el activo, mas fuerte. */
export const resaltados = StateField.define({
  create: () => Decoration.none,
  update(v, tr) {
    v = v.map(tr.changes)
    for (const e of tr.effects) if (e.is(poner)) v = e.value
    return v
  },
  provide: f => EditorView.decorations.from(f),
})

const COLORES = ['#5b8def', '#d9822b', '#3aa776', '#b45fc9', '#d2555a', '#2f9fb3']
function colorDe(aparato) {
  let h = 0
  for (const c of aparato || '') h = (h * 31 + c.charCodeAt(0)) >>> 0
  return COLORES[h % COLORES.length]
}

function fecha(ms, tx) {
  const d = new Date(ms)
  const hoy = new Date()
  const mes = tx.meses[d.getMonth()]
  if (d.getFullYear() !== hoy.getFullYear()) return `${d.getDate()} ${mes} ${d.getFullYear()}`
  return `${d.getDate()} ${mes}, ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

export function crearComentarios({ mandar, vistaDe, tx: txDe, abrirMenu, insignia, boton }) {
  let hilos = []
  let abierto = false
  let activo = null
  let verResueltos = false
  let componiendo = null // { tipo: 'nuevo'|'responder'|'editar', id, desde, hasta }
  let panel = null

  const tx = () => txDe()
  const texto = () => vistaDe().state.doc.toString()

  function pedir() {
    mandar({ tipo: 'comentarios_pedir', texto: texto() })
  }

  function recibir(msg) {
    hilos = msg.hilos || []
    const n = msg.abiertos || 0
    insignia.textContent = n ? String(n) : ''
    insignia.hidden = !n
    if (msg.nuevo) {
      activo = msg.nuevo
      abrir(true)
    }
    resaltar()
    pintar()
  }

  function resaltar() {
    const v = vistaDe()
    if (!v) return
    const marcas = hilos
      .filter(h => !h.resuelto && h.desde != null && h.hasta > h.desde)
      .sort((a, b) => a.desde - b.desde)
      .map(h => Decoration.mark({ class: 'pp-comentado' + (h.id === activo ? ' activo' : ''), attributes: { 'data-hilo': h.id } }).range(h.desde, Math.min(h.hasta, v.state.doc.length)))
    v.dispatch({ effects: poner.of(Decoration.set(marcas, true)) })
  }

  function abrir(si) {
    abierto = si
    document.body.classList.toggle('con-comentarios', si)
    boton.classList.toggle('activo', si)
    if (si && !panel) montar()
    if (panel) panel.hidden = !si
    pintar()
    vistaDe()?.requestMeasure()
  }

  function montar() {
    panel = document.createElement('div')
    panel.className = 'pp-panel-com'
    vistaDe().scrollDOM.append(panel)
  }

  /** Comentar lo elegido (o la palabra del cursor). */
  function comentar() {
    const v = vistaDe()
    const r = v.state.selection.main
    componiendo = { tipo: 'nuevo', desde: r.from, hasta: r.to }
    activo = null
    abrir(true)
  }

  function tarjetaDe(h) {
    const t = tx()
    const c = document.createElement('div')
    c.className = 'pp-tarjeta-com' + (h.id === activo ? ' activa' : '') + (h.resuelto ? ' resuelta' : '')
    c.dataset.id = h.id
    const cab = document.createElement('div')
    cab.className = 'cab'
    cab.innerHTML = `<span class="avatar"></span><span class="quien"><b></b><small></small></span>`
    cab.querySelector('.avatar').style.background = colorDe(h.aparato)
    cab.querySelector('.avatar').textContent = (h.autor || '?').trim().charAt(0).toUpperCase()
    cab.querySelector('b').textContent = h.autor
    cab.querySelector('small').textContent = fecha(h.cuando, t) + (h.editado ? ' · ' + t.editado : '') + (h.resuelto ? ' · ' + t.resuelto : '')
    if (h.id === activo) {
      const ok = document.createElement('button')
      ok.className = 'mini' + (h.resuelto ? ' hecho' : '')
      ok.textContent = '✓'
      ok.title = t.resolver
      ok.onclick = e => {
        e.stopPropagation()
        mandar({ tipo: 'comentario_resolver', texto: texto(), hilo: h.id, si: !h.resuelto })
      }
      const mas = document.createElement('button')
      mas.className = 'mini'
      mas.textContent = '⋯'
      mas.onclick = e => {
        e.stopPropagation()
        const r = mas.getBoundingClientRect()
        abrirMenu(
          [
            { texto: t.editar, accion: () => editar(h.id, h.texto) },
            { texto: t.borrar, accion: () => confirm(t.borrarHilo) && mandar({ tipo: 'comentario_borrar', texto: texto(), id: h.id }) },
          ],
          r.left,
          r.bottom + 4,
        )
      }
      cab.append(ok, mas)
    }
    c.append(cab)
    const cita = document.createElement('div')
    cita.className = 'cita'
    cita.textContent = h.cita
    c.append(cita)
    if (componiendo?.tipo === 'editar' && componiendo.id === h.id) c.append(caja(t.guardar, h.texto))
    else {
      const p = document.createElement('div')
      p.className = 'cuerpo'
      p.textContent = h.texto
      c.append(p)
    }
    for (const r of h.respuestas) {
      const d = document.createElement('div')
      d.className = 'respuesta'
      d.innerHTML = `<div class="cab"><span class="avatar chica"></span><span class="quien"><b></b><small></small></span></div>`
      d.querySelector('.avatar').style.background = colorDe(r.aparato)
      d.querySelector('.avatar').textContent = (r.autor || '?').trim().charAt(0).toUpperCase()
      d.querySelector('b').textContent = r.autor
      d.querySelector('small').textContent = fecha(r.cuando, t) + (r.editado ? ' · ' + t.editado : '')
      if (h.id === activo) {
        const mas = document.createElement('button')
        mas.className = 'mini'
        mas.textContent = '⋯'
        mas.onclick = e => {
          e.stopPropagation()
          const rr = mas.getBoundingClientRect()
          abrirMenu(
            [
              { texto: t.editar, accion: () => editar(r.id, r.texto) },
              { texto: t.borrar, accion: () => mandar({ tipo: 'comentario_borrar', texto: texto(), id: r.id }) },
            ],
            rr.left,
            rr.bottom + 4,
          )
        }
        d.querySelector('.cab').append(mas)
      }
      if (componiendo?.tipo === 'editar' && componiendo.id === r.id) d.append(caja(t.guardar, r.texto))
      else {
        const p = document.createElement('div')
        p.className = 'cuerpo'
        p.textContent = r.texto
        d.append(p)
      }
      c.append(d)
    }
    if (h.id === activo && !h.resuelto) {
      if (componiendo?.tipo === 'responder' && componiendo.id === h.id) c.append(caja(t.responder))
      else {
        const pr = document.createElement('div')
        pr.className = 'pildora-responder'
        pr.textContent = t.responderPista
        pr.onclick = e => {
          e.stopPropagation()
          componiendo = { tipo: 'responder', id: h.id }
          pintar()
        }
        c.append(pr)
      }
    }
    if (h.desde == null) {
      const s = document.createElement('div')
      s.className = 'sin-ancla'
      s.textContent = t.sinAncla
      c.prepend(s)
    }
    c.addEventListener('mousedown', e => {
      if (e.target.closest('textarea, button')) return
      e.preventDefault()
      if (activo !== h.id) {
        activo = h.id
        componiendo = null
        resaltar()
        pintar()
      }
      if (h.desde != null) vistaDe().dispatch({ effects: EditorView.scrollIntoView(h.desde, { y: 'center' }) })
    })
    return c
  }

  function editar(id, actual) {
    componiendo = { tipo: 'editar', id, actual }
    pintar()
  }

  /** La caja para escribir: Intro envia, Mayus+Intro salta, Esc cancela. */
  function caja(rotulo, inicial = '') {
    const t = tx()
    const d = document.createElement('div')
    d.className = 'redactar'
    const area = document.createElement('textarea')
    area.value = inicial
    area.placeholder = componiendo?.tipo === 'nuevo' ? t.pistaComentar : t.responderPista
    const pie = document.createElement('div')
    pie.className = 'pie'
    const no = document.createElement('button')
    no.className = 'cancelar'
    no.textContent = t.cancelar
    const si = document.createElement('button')
    si.className = 'enviar'
    si.textContent = rotulo
    pie.append(no, si)
    d.append(area, pie)
    const enviar = () => {
      const cuerpo = area.value.trim()
      if (!cuerpo) return
      const c = componiendo
      componiendo = null
      if (c.tipo === 'nuevo') mandar({ tipo: 'comentario_nuevo', texto: texto(), desde: c.desde, hasta: c.hasta, cuerpo })
      else if (c.tipo === 'responder') mandar({ tipo: 'comentario_responder', texto: texto(), hilo: c.id, cuerpo })
      else if (c.tipo === 'editar') mandar({ tipo: 'comentario_editar', texto: texto(), id: c.id, cuerpo })
    }
    const cancelar = () => {
      componiendo = null
      pintar()
      vistaDe().focus()
    }
    area.addEventListener('keydown', e => {
      e.stopPropagation()
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        enviar()
      } else if (e.key === 'Escape') {
        e.preventDefault()
        cancelar()
      }
    })
    si.onclick = enviar
    no.onclick = cancelar
    requestAnimationFrame(() => area.focus())
    return d
  }

  /** Las tarjetas, cada una a la altura de su frase; si chocan, bajan. */
  function pintar() {
    if (!panel || !abierto) return
    const v = vistaDe()
    const t = tx()
    panel.textContent = ''
    const arriba = document.createElement('div')
    arriba.className = 'pp-com-arriba'
    const resueltos = hilos.filter(h => h.resuelto).length
    if (resueltos) {
      const a = document.createElement('a')
      a.textContent = (verResueltos ? t.ocultarResueltos : t.verResueltos) + ` (${resueltos})`
      a.onclick = () => {
        verResueltos = !verResueltos
        pintar()
      }
      arriba.append(a)
    }
    panel.append(arriba)
    const visibles = hilos.filter(h => verResueltos || !h.resuelto)
    if (!visibles.length && !componiendo) {
      const n = document.createElement('div')
      n.className = 'pp-com-nada'
      n.innerHTML = `<b></b><span></span>`
      n.querySelector('b').textContent = t.sinComentarios
      n.querySelector('span').textContent = t.pistaComentarios
      panel.append(n)
      return
    }
    const top = pos => {
      const b = v.lineBlockAt(Math.min(pos, v.state.doc.length))
      return b.top + v.documentPadding.top
    }
    const piezas = []
    if (componiendo?.tipo === 'nuevo') {
      const c = document.createElement('div')
      c.className = 'pp-tarjeta-com activa'
      c.append(caja(t.comentar))
      piezas.push({ y: top(componiendo.desde), el: c })
    }
    for (const h of visibles) piezas.push({ y: h.desde == null ? 0 : top(h.desde), el: tarjetaDe(h), id: h.id })
    piezas.sort((a, b) => a.y - b.y)
    let fondo = 30
    for (const p of piezas) {
      panel.append(p.el)
      const y = Math.max(p.y, fondo)
      p.el.style.top = y + 'px'
      if (p.id === activo) p.el.style.transform = 'translateX(-14px)'
      fondo = y + p.el.offsetHeight + 10
    }
    panel.style.height = fondo + 'px'
  }

  // Un clic en el texto comentado activa su tarjeta.
  function alClicEnTexto(e) {
    const m = e.target.closest?.('.pp-comentado')
    if (!m) return
    activo = m.dataset.hilo
    if (!abierto) abrir(true)
    resaltar()
    pintar()
  }

  return { pedir, recibir, comentar, abrir: () => abrir(!abierto), pintar, alClicEnTexto, abierto: () => abierto }
}
