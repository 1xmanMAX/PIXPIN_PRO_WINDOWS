// **Marcar el texto de una pagina web** (2-nov-2026): el visor de HTML
// pone este guion en cada pagina que abre (`apps/pindesk/src/marcas_web.rs`,
// antes que los de la propia pagina). Al elegir texto con el raton sale la
// burbuja con los botones de la barrita del PDF (`barra-marcas.js`); las
// marcas se pintan con la API de resaltados de CSS (`CSS.highlights`), que
// no toca el DOM de la pagina: su programa y su estilo siguen como estaban.
//
// Las posiciones cuentan sobre el texto de la pagina: los nodos de texto de
// `<body>` en orden, sin `<script>`, `<style>`, `<noscript>`, `<template>`,
// los campos de un formulario ni lo que pone PinDesk (`ancla_texto`,
// `sobre: "html"`). Si la pagina cambia (una que se escribe sola), las
// marcas se vuelven a buscar por su cita.
//
// Con PinDesk se habla por `chrome.webview` con `pixpin_marcas`: `pedir` al
// cargar y `lista` (la entera) tras cada cambio; PinDesk contesta con
// `__pixpinMarcas.recibir({marcas, rotulos})`. Ctrl+Z y Ctrl+Y (fuera de un
// campo de texto) deshacen y rehacen la ultima.

import * as mt from './marcas-texto.mjs'
import { montarBotones, CSS as CSS_BOTONES } from './barra-marcas.js'

const FUERA = /^(SCRIPT|STYLE|NOSCRIPT|TEMPLATE|TEXTAREA|SELECT|OPTION|INPUT)$/
const PROPIO = 'pixpin-riel-capa'

function mandar(que, resto = {}) {
  try {
    window.chrome.webview.postMessage(JSON.stringify({ pixpin_marcas: que, ...resto }))
  } catch {
    /* sin PinDesk */
  }
}

function iniciar() {
  if (window.__pixpinMarcas || !document.body) return
  const M = (window.__pixpinMarcas = {})
  let marcas = []
  let color = mt.COLOR_INICIAL
  let elegida = null
  let sitios = {}
  let indice = null
  const historia = new mt.Historia()
  const resaltados = window.CSS && window.CSS.highlights

  // ---------------------------------------------------------- el texto
  const anfitrion = document.createElement('div')
  anfitrion.id = 'pixpin-marcas'

  function aceptar(n) {
    for (let p = n.parentNode; p && p !== document.body; p = p.parentNode) {
      if (p.nodeType !== 1) continue
      if (FUERA.test(p.tagName) || p.id === PROPIO || p === anfitrion) return NodeFilter.FILTER_REJECT
    }
    return NodeFilter.FILTER_ACCEPT
  }
  /** Los nodos de texto con donde empieza cada uno, y el texto entero. */
  function elIndice() {
    if (indice) return indice
    const nodos = []
    const inicios = []
    const trozos = []
    let n = 0
    const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, { acceptNode: aceptar })
    for (let t; (t = w.nextNode()); ) {
      nodos.push(t)
      inicios.push(n)
      trozos.push(t.data)
      n += t.data.length
    }
    const donde = new Map(nodos.map((t, i) => [t, i]))
    return (indice = { nodos, inicios, donde, texto: trozos.join('') })
  }
  /** Un punto del DOM (de una seleccion) a posicion en el texto. */
  function posDe(nodo, off) {
    const ix = elIndice()
    if (nodo.nodeType === 3) {
      const i = ix.donde.get(nodo)
      return i == null ? null : ix.inicios[i] + Math.min(off, nodo.data.length)
    }
    // Un elemento: el primer texto desde su hijo `off`.
    const desde = nodo.childNodes[off] || null
    const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, { acceptNode: aceptar })
    if (desde) {
      w.currentNode = desde
      const t = desde.nodeType === 3 && aceptar(desde) === NodeFilter.FILTER_ACCEPT ? desde : w.nextNode()
      if (t && ix.donde.has(t)) return ix.inicios[ix.donde.get(t)]
      return ix.texto.length
    }
    // Despues del ultimo hijo: el texto que sigue al elemento.
    w.currentNode = nodo
    let ultimo = nodo
    while (ultimo.lastChild) ultimo = ultimo.lastChild
    w.currentNode = ultimo
    const t = w.nextNode()
    return t && ix.donde.has(t) ? ix.inicios[ix.donde.get(t)] : ix.texto.length
  }
  /** Una posicion del texto a un punto del DOM: un principio cae al
   * principio del nodo que sigue, un final al final del que acaba ahi. */
  function puntoDe(p, alFinal) {
    const { nodos, inicios } = elIndice()
    if (!nodos.length) return null
    // El ultimo nodo que empieza antes de `p` (o en `p`, si es principio).
    let lo = 0
    let hi = nodos.length - 1
    while (lo < hi) {
      const m = (lo + hi + 1) >> 1
      if (alFinal ? inicios[m] < p : inicios[m] <= p) lo = m
      else hi = m - 1
    }
    const n = nodos[lo]
    return [n, Math.max(0, Math.min(p - inicios[lo], n.data.length))]
  }
  function rangoDe(a, b) {
    const x = puntoDe(a, false)
    const y = puntoDe(b, true)
    if (!x || !y) return null
    const r = document.createRange()
    try {
      r.setStart(x[0], x[1])
      r.setEnd(y[0], y[1])
    } catch {
      return null
    }
    return r
  }
  /** Lo elegido ahora, en posiciones del texto. */
  function elegido() {
    const s = getSelection()
    if (!s || s.isCollapsed || !s.rangeCount) return null
    const r = s.getRangeAt(0)
    if (!document.body.contains(r.commonAncestorContainer)) return null
    const a = posDe(r.startContainer, r.startOffset)
    const b = posDe(r.endContainer, r.endOffset)
    if (a == null || b == null || a === b) return null
    return { from: Math.min(a, b), to: Math.max(a, b) }
  }

  // ---------------------------------------------------------- pintar
  const estilo = document.createElement('style')
  estilo.textContent = mt.reglasCss(c => `::highlight(${c})`) + '\n::highlight(pp-mt-elegida){text-decoration:underline dotted #5b9bf0;text-decoration-thickness:2px}'
  ;(document.head || document.documentElement).append(estilo)
  const nombres = new Set()

  function pintar() {
    if (!resaltados) return
    const t = elIndice().texto
    const grupos = {}
    sitios = {}
    for (const m of marcas) {
      const r = mt.ubicar(t, m)
      if (!r) continue
      sitios[m.id] = r
      const rg = rangoDe(r[0], r[1])
      if (!rg) continue
      ;(grupos[mt.claseDe(m)] ||= []).push(rg)
      if (m.id === elegida) (grupos['pp-mt-elegida'] ||= []).push(rg.cloneRange())
    }
    for (const n of nombres) if (!grupos[n]) resaltados.delete(n)
    nombres.clear()
    for (const [n, rs] of Object.entries(grupos)) {
      resaltados.set(n, new Highlight(...rs))
      nombres.add(n)
    }
  }

  // La pagina cambio (una que se escribe sola): otra vez el indice y las
  // marcas por su cita, al rato.
  let repintar = 0
  new MutationObserver(lista => {
    if (lista.every(r => r.target === anfitrion || anfitrion.contains(r.target) || r.target === estilo)) return
    indice = null
    clearTimeout(repintar)
    repintar = setTimeout(pintar, 300)
  }).observe(document.body, { childList: true, subtree: true, characterData: true })

  // ---------------------------------------------------------- la burbuja
  anfitrion.setAttribute('aria-hidden', 'true')
  anfitrion.style.cssText = 'position:fixed;left:0;top:0;z-index:2147483647;display:none;margin:0;padding:0;border:0'
  const sombra = anfitrion.attachShadow({ mode: 'open' })
  const css = document.createElement('style')
  const oscuro = matchMedia('(prefers-color-scheme: dark)')
  const tema = () =>
    oscuro.matches
      ? ':host{--menu:#20201f;--raya:#3a3a39;--texto:#e6e4df;--pastilla:#313130}'
      : ':host{--menu:#ffffff;--raya:#e4e2dc;--texto:#1f1f1e;--pastilla:#efede8}'
  css.textContent = CSS_BOTONES + '\n.burbuja{height:38px;padding:4px;display:flex;align-items:center;background:var(--menu);border:1px solid var(--raya);border-radius:10px;box-shadow:0 6px 22px rgba(0,0,0,.28);font-family:Segoe UI,system-ui,sans-serif}\n' + tema()
  oscuro.addEventListener?.('change', () => (css.textContent = css.textContent.replace(/:host\{[^}]*\}$/, tema())))
  const burbuja = document.createElement('div')
  burbuja.className = 'burbuja'
  sombra.append(css, burbuja)
  const botones = montarBotones(burbuja, alPulsar)
  botones.ponerColor(color)
  document.documentElement.append(anfitrion)
  let rangoDeMarca = null

  function esconder() {
    anfitrion.style.display = 'none'
  }
  function mostrar(r) {
    const rg = r && rangoDe(r.from, r.to)
    const cajas = rg ? [...rg.getClientRects()].filter(c => c.width || c.height) : []
    if (!cajas.length) return esconder()
    anfitrion.style.display = 'block'
    const b = burbuja.getBoundingClientRect()
    const primera = cajas[0]
    const ultima = cajas[cajas.length - 1]
    let y = primera.top - b.height - 8
    if (y < 6) y = ultima.bottom + 8
    const x = Math.max(6, Math.min(primera.left - 12, innerWidth - b.width - 6))
    anfitrion.style.left = x + 'px'
    anfitrion.style.top = Math.min(y, innerHeight - b.height - 6) + 'px'
  }

  function guardar() {
    mandar('lista', { marcas })
  }
  function cambiar(nuevas) {
    historia.apuntar(marcas)
    marcas = nuevas
    pintar()
    guardar()
  }

  function alPulsar(que) {
    const r = elegido() || rangoDeMarca
    if (que === 'copiar') {
      const t = getSelection()?.toString() || (r ? elIndice().texto.slice(r.from, r.to) : '')
      if (t) navigator.clipboard?.writeText(t).catch(() => document.execCommand('copy'))
      return esconder()
    }
    if (typeof que === 'object') {
      color = que.color
      botones.ponerColor(color)
      if (elegida) cambiar(marcas.map(m => (m.id === elegida ? { ...m, color: mt.COLORES[color] } : m)))
      return
    }
    if (que === 'quitar') {
      const fuera = elegida ? marcas.filter(m => m.id === elegida) : r ? mt.pisadas(marcas, sitios, r.from, r.to) : []
      elegida = null
      rangoDeMarca = null
      if (fuera.length) cambiar(marcas.filter(m => !fuera.includes(m)))
      else pintar()
      getSelection()?.removeAllRanges()
      return esconder()
    }
    if (elegida) return cambiar(marcas.map(m => (m.id === elegida ? { ...m, marca: que, color: mt.COLORES[color] } : m)))
    if (!r) return
    const m = mt.nueva(elIndice().texto, r.from, r.to, que, color, Date.now(), marcas)
    if (m) cambiar([...marcas, m])
  }

  /** La marca bajo el raton (la ultima puesta si hay varias). */
  function marcaEn(x, y) {
    const c = document.caretRangeFromPoint?.(x, y)
    if (!c) return null
    const p = posDe(c.startContainer, c.startOffset)
    if (p == null) return null
    const bajo = mt.pisadas(marcas, sitios, p, p)
    return bajo.length ? bajo[bajo.length - 1] : null
  }

  addEventListener(
    'mousedown',
    e => {
      if (e.composedPath().includes(anfitrion)) return
      esconder()
    },
    true,
  )
  addEventListener(
    'mouseup',
    e => {
      if (e.button !== 0 || e.composedPath().includes(anfitrion)) return
      setTimeout(() => {
        const r = elegido()
        if (r) {
          if (elegida) {
            elegida = null
            pintar()
          }
          rangoDeMarca = null
          return mostrar(r)
        }
        // Un clic en una marca la elige: otra marca la cambia, un color la
        // pinta y el circulo la quita.
        const m = marcaEn(e.clientX, e.clientY)
        const antes = elegida
        elegida = m?.id || null
        rangoDeMarca = m ? { from: sitios[m.id][0], to: sitios[m.id][1] } : null
        if (antes !== elegida) pintar()
        if (rangoDeMarca) mostrar(rangoDeMarca)
      })
    },
    true,
  )
  addEventListener('scroll', esconder, { capture: true, passive: true })
  addEventListener('resize', esconder, { passive: true })
  document.addEventListener('selectionchange', () => {
    if (anfitrion.style.display !== 'none' && !rangoDeMarca && !elegido()) esconder()
  })
  const editable = n => !!n && (n.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(n.tagName || ''))
  addEventListener(
    'keydown',
    e => {
      if (e.isComposing || e.altKey || editable(e.target) || editable(document.activeElement)) return
      const k = (e.key || '').toLowerCase()
      if (e.key === 'Escape' && anfitrion.style.display !== 'none') {
        esconder()
        return
      }
      if (!e.ctrlKey || (k !== 'z' && k !== 'y')) return
      const l = k === 'y' || e.shiftKey ? historia.rehacer(marcas) : historia.deshacer(marcas)
      if (!l) return
      e.preventDefault()
      e.stopPropagation()
      marcas = l
      elegida = null
      rangoDeMarca = null
      esconder()
      pintar()
      guardar()
    },
    true,
  )

  // ---------------------------------------------------------- PinDesk
  M.recibir = d => {
    if (d.rotulos) botones.ponerRotulos(d.rotulos)
    if (Array.isArray(d.marcas)) {
      marcas = d.marcas.filter(m => m && m.id && m.cita)
      elegida = null
      pintar()
    }
  }
  /** Para las pruebas: el texto y lo que hay. */
  M.estado = () => ({ texto: elIndice().texto, marcas, sitios })
  M.deshacer = () => {
    const l = historia.deshacer(marcas)
    if (!l) return false
    marcas = l
    pintar()
    guardar()
    return true
  }
  mandar('pedir')
}

if (window.top === window) {
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', iniciar, { once: true })
  else iniciar()
}
