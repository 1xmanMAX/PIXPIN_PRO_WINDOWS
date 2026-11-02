// **Los botones de marcar** (2-nov-2026), los mismos y en el mismo orden
// que la barrita del PDF y del pin (`pindesk_ui::barra_marcar`): copiar, los
// cinco colores, las cinco marcas (una «A» con su marca, del color elegido)
// y el circulo tachado de quitar. Los usan la burbuja de la nota (con
// «Comentar» delante) y la de una pagina web (`marcas-pagina.js`, dentro de
// su sombra para que el estilo de la pagina no la toque).

import { COLORES, MARCAS } from './marcas-texto.mjs'

/** Los rotulos de serie (los manda PinDesk en el idioma de la app). */
export const ROTULOS = {
  copiar: 'Copiar',
  resaltar: 'Resaltar',
  ondulada: 'Ondulada',
  subrayar: 'Subrayar',
  tachar: 'Tachar',
  tapar: 'Tapar',
  quitar: 'Quitar la marca',
  color: 'Color',
}

const COPIAR = '<svg viewBox="0 0 16 16"><rect x="5" y="5" width="8.5" height="8.5" rx="1.5"/><path d="M3 10.5V3.8A1.3 1.3 0 0 1 4.3 2.5H10.5"/></svg>'

/** El estilo de los botones (va en la hoja de la nota y en la sombra de la
 * pagina). `--mc` es el color elegido. */
export const CSS = `
.pp-marcas{display:flex;align-items:center;gap:2px}
.pp-marcas .sep{width:1px;height:20px;background:var(--raya,#3a3a39);margin:0 3px}
.pp-marcas button{height:30px;min-width:30px;border:none;border-radius:6px;background:none;color:var(--texto,#e6e4df);display:flex;align-items:center;justify-content:center;cursor:pointer;padding:0;font:600 15px/1 'Segoe UI',system-ui,sans-serif}
.pp-marcas button:hover{background:var(--pastilla,#313130)}
.pp-marcas button svg{width:15px;height:15px;fill:none;stroke:currentColor;stroke-width:1.5;stroke-linecap:round;stroke-linejoin:round}
.pp-marcas .color{min-width:22px;width:22px}
.pp-marcas .color i{display:block;width:14px;height:14px;border-radius:50%;background:var(--c)}
.pp-marcas .color.puesto i{box-shadow:0 0 0 2px var(--menu,#20201f),0 0 0 3.5px var(--c)}
.pp-marcas .a{position:relative;width:20px;height:22px;display:flex;align-items:center;justify-content:center}
.pp-marcas [data-mm=resaltar] .a{background:color-mix(in srgb,var(--mc) 55%,transparent);border-radius:3px}
.pp-marcas [data-mm=ondulada] .a{text-decoration:underline wavy var(--mc);text-decoration-thickness:1.5px;text-underline-offset:3px}
.pp-marcas [data-mm=subrayar] .a{text-decoration:underline var(--mc);text-decoration-thickness:2px;text-underline-offset:3px}
.pp-marcas [data-mm=tachar] .a{text-decoration:line-through var(--mc);text-decoration-thickness:2px}
.pp-marcas [data-mm=tapar] .a::after{content:'';position:absolute;left:1px;right:1px;top:50%;height:9px;border-radius:2px;background:var(--mc)}
.pp-marcas [data-mm=quitar] .a{width:16px;height:16px;border-radius:50%;background:color-mix(in srgb,currentColor 35%,transparent)}
.pp-marcas [data-mm=quitar] .a::after{content:'';position:absolute;left:3px;right:3px;top:50%;height:1.8px;background:currentColor;transform:rotate(-45deg)}
`

/**
 * Los botones, dentro de `donde`. `alPulsar(que)`: `'copiar'`, `'quitar'`,
 * una de `MARCAS` o `{ color: k }`. Devuelve `{ ponerColor(k),
 * ponerRotulos(r) }`.
 */
export function montarBotones(donde, alPulsar, { conCopiar = true } = {}) {
  const caja = document.createElement('span')
  caja.className = 'pp-marcas'
  const boton = (attr, valor, html) => {
    const b = document.createElement('button')
    b.type = 'button'
    b.setAttribute(attr, valor)
    b.innerHTML = html
    caja.append(b)
    return b
  }
  if (conCopiar) {
    boton('data-mm', 'copiar', COPIAR)
    caja.append(Object.assign(document.createElement('span'), { className: 'sep' }))
  }
  const colores = COLORES.map((c, k) => {
    const b = boton('data-mc', String(k), '<i></i>')
    b.className = 'color'
    b.style.setProperty('--c', c)
    return b
  })
  caja.append(Object.assign(document.createElement('span'), { className: 'sep' }))
  for (const m of [...MARCAS, 'quitar']) boton('data-mm', m, `<span class="a">${m === 'quitar' ? '' : 'A'}</span>`)
  caja.addEventListener('mousedown', e => e.preventDefault())
  caja.addEventListener('click', e => {
    const b = e.target.closest('button')
    if (!b) return
    e.stopPropagation()
    if (b.dataset.mc != null) alPulsar({ color: Number(b.dataset.mc) })
    else alPulsar(b.dataset.mm)
  })
  donde.append(caja)
  const api = {
    ponerColor(k) {
      caja.style.setProperty('--mc', COLORES[k] || COLORES[4])
      colores.forEach((b, i) => b.classList.toggle('puesto', i === k))
    },
    ponerRotulos(r) {
      const t = { ...ROTULOS, ...(r || {}) }
      caja.querySelectorAll('[data-mm]').forEach(b => (b.title = t[b.dataset.mm] || ''))
      colores.forEach(b => (b.title = t.color))
    },
    caja,
  }
  api.ponerRotulos()
  return api
}
