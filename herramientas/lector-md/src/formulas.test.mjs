// Los casos de FormulasTest.kt de Android: `node --test src/formulas.test.mjs`.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { leer, plano, componer } from './formulas.mjs'

const t = p => plano(p).replace(/ /g, '')

test('un exponente se cuelga de su base', () => {
  const p = leer('x^2')
  assert.equal(p.t, 'indices')
  assert.equal(t(p), 'x^2')
})
test('el exponente entre llaves puede ser largo', () => assert.equal(t(leer('x^{n+1}')), 'x^n+1'))
test('arriba y abajo a la vez sobre la misma base', () => {
  const p = leer('x^2_i')
  assert.equal(t(p.arriba), '2')
  assert.equal(t(p.abajo), 'i')
})
test('un subindice solo', () => {
  const p = leer('a_1')
  assert.equal(t(p.abajo), '1')
  assert.equal(p.arriba, null)
})
test('una fraccion tiene arriba y abajo', () => {
  const p = leer('\\frac{a}{b}')
  assert.equal(t(p.arriba), 'a')
  assert.equal(t(p.abajo), 'b')
})
test('una fraccion dentro de otra', () => {
  const p = leer('\\frac{\\frac{a}{b}}{c}')
  assert.equal(p.arriba.t, 'fraccion')
  assert.equal(t(p), '((a/b)/c)')
})
test('la fraccion sin llaves coge un caracter de cada lado', () => assert.equal(t(leer('\\frac12')), '(1/2)'))
test('una raiz normal y una con indice', () => {
  assert.equal(t(leer('\\sqrt{x+1}')), 'sqrt(x+1)')
  const c = leer('\\sqrt[3]{x}')
  assert.equal(t(c.indice), '3')
  assert.equal(t(c.dentro), 'x')
})
test('las letras griegas y los operadores salen como simbolos', () => {
  for (const [a, b] of [['\\pi', 'π'], ['\\leq', '≤'], ['\\sum', '∑'], ['\\infty', '∞'], ['\\Delta', 'Δ']]) assert.equal(t(leer(a)), b)
})
test('las funciones se quedan como palabra', () => {
  assert.equal(t(leer('\\sin')), 'sin')
  assert.equal(t(leer('\\log')), 'log')
})
test('un mandato desconocido se enseña tal cual', () => assert.ok(t(leer('\\loquesea')).includes('loquesea')))
test('las letras van en cursiva y los numeros no', () => {
  const p = leer('2x')
  assert.equal(p.partes[0].cursiva, false)
  assert.equal(p.partes[1].cursiva, true)
})
test('una formula completa se entiende entera', () => {
  const p = t(leer('x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}'))
  for (const s of ['±', 'sqrt(', 'b^2', '/2']) assert.ok(p.includes(s), p)
})
test('los parentesis agrupan', () => {
  const p = leer('(a+b)^2')
  assert.equal(p.base.t, 'agrupado')
  assert.equal(t(p.arriba), '2')
})
test('la barra hace una fraccion', () => {
  const p = leer('1/2')
  assert.equal(p.t, 'fraccion')
  assert.equal(t(p), '(1/2)')
})
test('la fraccion se ata mas fuerte que la suma', () => {
  const p = leer('1/2 + x')
  assert.equal(p.partes[0].t, 'fraccion')
  assert.ok(t(p).includes('(1/2)'))
})
test('los exponentes se escriben con el circunflejo de siempre', () => {
  assert.equal(t(leer('x^2')), 'x^2')
  assert.equal(t(leer('x^{10}')), 'x^10')
})
test('las palabras corrientes hacen su simbolo', () => {
  assert.equal(t(leer('pi')), 'π')
  assert.equal(t(leer('inf')), '∞')
  assert.equal(t(leer('alpha')), 'α')
})
test('raiz y sqrt hacen lo mismo', () => {
  assert.equal(leer('raiz(9)').t, 'raiz')
  assert.equal(leer('sqrt(9)').t, 'raiz')
  assert.equal(t(leer('raiz(x+1)')), 'sqrt(x+1)')
})
test('las parejas de signos se convierten', () => {
  for (const [a, b] of [['a <= b', '≤'], ['a != b', '≠'], ['a +- b', '±'], ['a -> b', '→']]) assert.ok(t(leer(a)).includes(b))
})
test('pegado significa multiplicado', () => {
  const p = t(leer('2x'))
  assert.equal(p, '2x')
  assert.ok(!p.includes('·'))
})
test('el asterisco si pinta su punto', () => assert.ok(t(leer('2*x')).includes('·')))
test('una funcion sale derecha y con su argumento', () => {
  const p = t(leer('sin(x)'))
  assert.ok(p.startsWith('sin') && p.includes('x'))
})
test('el latex de antes sigue funcionando', () => {
  assert.equal(t(leer('\\frac{a}{b}')), '(a/b)')
  assert.equal(t(leer('\\pi')), 'π')
  assert.equal(leer('\\sqrt{x}').t, 'raiz')
})
test('una formula de verdad escrita a lo facil', () => {
  const p = t(leer('x = (-b +- sqrt(b^2 - 4ac)) / (2a)'))
  for (const s of ['±', 'sqrt(', 'b^2']) assert.ok(p.includes(s), p)
})
test('nada de esto revienta', () => {
  for (const s of ['', '^', '_', '{', '}', '\\', '\\frac', '\\sqrt[', '((((', 'x^^2']) {
    leer(s)
    componer(s)
  }
})
test('una llave sin cerrar no pierde lo de dentro', () => assert.ok(t(leer('x^{2')).includes('2')))

// La composicion: la fraccion con su raya, el exponente subido, sin HTML colado.
test('se compone en html y lo escrito no se cuela como html', () => {
  assert.ok(componer('1/2').includes('ppf-frac'))
  assert.ok(componer('x^2').includes('<sup class="ppf-sup">'))
  assert.ok(componer('sqrt(x)').includes('ppf-raiz'))
  assert.ok(!componer('a<b').includes('<b'))
  assert.ok(componer('a<b').includes('&lt;'))
})
