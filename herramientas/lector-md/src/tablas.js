// Las tablas de una nota, dibujadas como tablas: las de barras de Markdown
// (GFM) y las HTML del movil (`Tablas.aHtml`: celdas combinadas y colores;
// ver `pixpin-docs/md_tabla_html.rs`). Solo se pinta: el texto no cambia.

/** Si un bloque HTML es una tabla de las del movil. */
export function esTablaHtml(texto) {
  return /^\s*<table[\s>]/i.test(texto) && /<\/table>\s*$/i.test(texto)
}

function escapar(s) {
  return String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c])
}

/** El Markdown de dentro de una celda: negrita, cursiva, tachado, codigo. */
export function enLinea(texto) {
  let s = escapar(texto)
  s = s.replace(/`([^`]+)`/g, '<code>$1</code>')
  s = s.replace(/\*\*\*([^*]+)\*\*\*/g, '<b><i>$1</i></b>')
  s = s.replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>')
  s = s.replace(/(^|[^\w*])\*([^*\s][^*]*?)\*(?!\w)/g, '$1<i>$2</i>')
  s = s.replace(/~~([^~]+)~~/g, '<s>$1</s>')
  s = s.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<span class="pp-enlace">$1</span>')
  return s
}

/** Parte un renglon `| a | b |` en celdas (con `\|` dentro). */
export function celdas(renglon) {
  let r = renglon.trim()
  if (r.startsWith('|')) r = r.slice(1)
  if (r.endsWith('|') && !r.endsWith('\\|')) r = r.slice(0, -1)
  const out = []
  let actual = ''
  for (let i = 0; i < r.length; i++) {
    if (r[i] === '\\' && r[i + 1] === '|') {
      actual += '|'
      i++
    } else if (r[i] === '|') {
      out.push(actual.trim())
      actual = ''
    } else actual += r[i]
  }
  out.push(actual.trim())
  return out
}

function alineacion(sep) {
  const s = sep.trim()
  if (s.startsWith(':') && s.endsWith(':')) return 'center'
  if (s.endsWith(':')) return 'right'
  return ''
}

/** Una tabla (GFM o HTML) como elemento `<table>`. */
export function tabla(texto) {
  const t = document.createElement('table')
  t.className = 'pp-tabla'
  if (esTablaHtml(texto)) {
    const doc = new DOMParser().parseFromString(texto, 'text/html')
    const origen = doc.querySelector('table')
    for (const tr of origen ? origen.querySelectorAll('tr') : []) {
      const fila = document.createElement('tr')
      for (const c of tr.querySelectorAll('th, td')) {
        const celda = document.createElement(c.tagName.toLowerCase() === 'th' ? 'th' : 'td')
        for (const a of ['colspan', 'rowspan', 'align', 'valign']) {
          const v = c.getAttribute(a)
          if (v && /^[a-z0-9]+$/i.test(v)) celda.setAttribute(a, v)
        }
        const estilo = c.getAttribute('style') || ''
        const fondo = /background(?:-color)?\s*:\s*(#[0-9a-f]{3,8})/i.exec(estilo)
        const color = /(?:^|;)\s*color\s*:\s*(#[0-9a-f]{3,8})/i.exec(estilo)
        if (fondo) celda.style.background = fondo[1]
        if (color) celda.style.color = color[1]
        if (fondo && !color) celda.style.color = oscuro(fondo[1]) ? '#f5f4ef' : '#1f1e1d'
        celda.innerHTML = enLinea(c.textContent)
        fila.append(celda)
      }
      t.append(fila)
    }
    return t
  }
  const renglones = texto.split('\n')
  const cab = celdas(renglones[0] || '')
  const aline = celdas(renglones[1] || '').map(alineacion)
  // Donde empieza cada celda en el texto, para llevar el cursor al pulsar.
  let pos = 0
  const filas = renglones.map(r => {
    const inicio = pos
    pos += r.length + 1
    return { r, inicio }
  })
  filas.forEach(({ r, inicio }, i) => {
    if (i === 1) return
    const fila = document.createElement('tr')
    const cs = celdas(r)
    let col = r.indexOf('|') === 0 ? 1 : 0
    cs.forEach((txt, j) => {
      if (j >= Math.max(cab.length, 1)) return
      const celda = document.createElement(i === 0 ? 'th' : 'td')
      if (aline[j]) celda.style.textAlign = aline[j]
      celda.innerHTML = enLinea(txt)
      const k = r.indexOf(txt, col)
      celda.dataset.desde = String(inicio + (k >= 0 ? k : col))
      col = (k >= 0 ? k + txt.length : col) + 1
      fila.append(celda)
    })
    for (let j = cs.length; j < cab.length; j++) fila.append(document.createElement(i === 0 ? 'th' : 'td'))
    t.append(fila)
  })
  return t
}

function oscuro(hex) {
  let h = hex.slice(1)
  if (h.length === 3) h = h.split('').map(c => c + c).join('')
  const n = parseInt(h.slice(0, 6), 16)
  const r = (n >> 16) & 255
  const g = (n >> 8) & 255
  const b = n & 255
  return 0.299 * r + 0.587 * g + 0.114 * b < 128
}
