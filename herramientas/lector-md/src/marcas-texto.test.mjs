// Las cuentas de las marcas de texto:
// `node --test herramientas/lector-md/src/marcas-texto.test.mjs`.
import test from 'node:test'
import assert from 'node:assert/strict'
import { anclar, ubicar, reanclar, parecida, nueva, pisadas, Historia, idNuevo, claseDe, reglasCss, CONTEXTO } from './marcas-texto.mjs'

const T = 'La losa ya está hormigonada. El curado empieza mañana y la losa se riega dos veces al día.'
const sitio = (t, q, n = 0) => {
  let i = -1
  for (let k = 0; k <= n; k++) i = t.indexOf(q, i + 1)
  return [i, i + q.length]
}

test('el ancla lleva la cita sin blancos, su contexto y donde estaba', () => {
  const [a, b] = sitio(T, 'hormigonada')
  const m = anclar(T, a - 1, b + 1)
  assert.equal(m.cita.type, 'TextQuoteSelector')
  assert.equal(m.posicion.type, 'TextPositionSelector')
  assert.equal(m.cita.exact, 'hormigonada.')
  assert.equal(m.posicion.start, a)
  assert.equal(m.cita.prefix, T.slice(0, a))
  assert.ok(m.cita.suffix.length <= CONTEXTO)
  // Caso negativo: solo blancos no es nada.
  assert.equal(anclar('a   b', 1, 4), null)
})

test('una cita que sale dos veces vuelve a la suya por el contexto', () => {
  const [a, b] = sitio(T, 'losa', 1)
  const m = anclar(T, a, b)
  assert.deepEqual(ubicar(T, m), [a, b])
  // Con texto nuevo delante, sigue en la segunda «losa» y no en la primera.
  const t2 = 'Nota: ' + T
  assert.deepEqual(ubicar(t2, m), [a + 6, b + 6])
})

test('si se escribio dentro de la cita, lo de entre sus contextos', () => {
  const [a, b] = sitio(T, 'empieza mañana')
  const m = anclar(T, a, b)
  const t2 = T.replace('empieza mañana', 'empieza el lunes por la mañana')
  const r = ubicar(t2, m)
  assert.equal(t2.slice(r[0], r[1]), 'empieza el lunes por la mañana')
})

test('una errata corregida en la cita y en su contexto: la parecida', () => {
  const t = 'Uno. ' + 'x'.repeat(50) + ' El hormigon se cura con agua limpia siempre. ' + 'y'.repeat(50)
  const [a, b] = sitio(t, 'El hormigon se cura con agua')
  const m = anclar(t, a, b)
  // Se toca la cita y los dos contextos a la vez.
  const t2 = t.replace('hormigon', 'hormigón').replace('x'.repeat(50), 'z'.repeat(50)).replace('limpia siempre', 'clara')
  const r = ubicar(t2, m)
  assert.ok(r, 'la encuentra')
  assert.equal(t2.slice(r[0], r[1]), 'El hormigón se cura con agua')
})

test('caso negativo: lo que ya no esta no se inventa', () => {
  const [a, b] = sitio(T, 'hormigonada')
  const m = anclar(T, a, b)
  assert.equal(ubicar('Otra nota que no habla de nada de eso.', m), null)
  // Demasiado distinta para la parecida.
  assert.equal(parecida('abcdefghijklmnop', 'zzzzzyyyyy', 0), null)
  // Una cita corta no se busca aproximada.
  assert.equal(parecida('gato pato', 'mato', 0), null)
})

test('reanclar pone el ancla al dia y sigue al texto', () => {
  const [a, b] = sitio(T, 'curado')
  const m = anclar(T, a, b)
  const t2 = T.replace('El curado', 'Después, el curado')
  assert.deepEqual(reanclar(t2, m), sitio(t2, 'curado'))
  assert.equal(m.posicion.start, t2.indexOf('curado'))
  assert.ok(m.cita.prefix.endsWith('Después, el '))
})

test('los pares sustitutos no se parten', () => {
  const t = 'hola 😀 adios'
  const i = t.indexOf('😀')
  const m = anclar(t, i + 1, i + 2)
  assert.equal(m.cita.exact, '😀')
})

test('marcas nuevas, las pisadas y los ids sin repetir', () => {
  const [a, b] = sitio(T, 'losa')
  const m1 = nueva(T, a, b, 'resaltar', 4, 1000, [])
  assert.equal(m1.id, 'marca-texto-1000')
  assert.equal(m1.color, '#F5C218')
  const m2 = nueva(T, a, b + 3, 'subrayar', 2, 1000, [m1])
  assert.equal(m2.id, 'marca-texto-1001')
  assert.equal(nueva(T, a, b, 'pintar', 0, 1, []), null)
  const sitios = { [m1.id]: [a, b], [m2.id]: [a, b + 3] }
  assert.deepEqual(pisadas([m1, m2], sitios, b + 1, b + 2).map(m => m.id), [m2.id])
  assert.deepEqual(pisadas([m1, m2], sitios, a + 1, a + 1).length, 2)
  assert.equal(pisadas([m1, m2], sitios, b + 10, b + 12).length, 0)
  assert.equal(idNuevo(5, [{ id: 'marca-texto-5' }, { id: 'marca-texto-6' }]), 'marca-texto-7')
})

test('deshacer y rehacer devuelven la lista de antes', () => {
  const h = new Historia()
  let l = []
  h.apuntar(l)
  l = [{ id: 'a' }]
  h.apuntar(l)
  l = [{ id: 'a' }, { id: 'b' }]
  l = h.deshacer(l)
  assert.deepEqual(l, [{ id: 'a' }])
  l = h.deshacer(l)
  assert.deepEqual(l, [])
  assert.equal(h.deshacer(l), null)
  l = h.rehacer(l)
  assert.deepEqual(l, [{ id: 'a' }])
})

test('cada marca y color tiene su clase y su regla', () => {
  assert.equal(claseDe({ marca: 'tapar', color: '#f47286' }), 'pp-mt-tapar-0')
  // Caso negativo: un color o una marca que no se conocen caen en lo de serie.
  assert.equal(claseDe({ marca: 'otra', color: '#123456' }), 'pp-mt-resaltar-4')
  const css = reglasCss(c => `.${c}`)
  assert.equal(css.split('\n').length, 25)
  assert.match(css, /\.pp-mt-ondulada-2\{text-decoration:underline wavy #5BA4F5/)
  assert.match(css, /\.pp-mt-tapar-4\{background-color:#F5C218;color:#F5C218\}/)
})
