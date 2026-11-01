// Pruebas de los gestos de las paginas exportadas, sin navegador:
// `node --test crates/pindesk-motor2d/pruebas_js`.
//
// - Las cuentas: la rueda pasa a desplazamiento en las dos direcciones, los `deltaMode` a
//   pixeles, Ctrl (el pellizco del trackpad) a acercar, y acercar deja quieto el punto.
// - `conectar` con una caja y una ventana de mentira: la rueda no es pasiva, los avisos se
//   suman en un fotograma, Safari pellizca con `gesturechange` y los dedos arrastran.
// - Que las copias del fichero (plano de pindesk-cad y apple/nucleo) son la misma.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const aqui = dirname(fileURLToPath(import.meta.url));
const repo = join(aqui, "..", "..", "..");
const RUTA = join(aqui, "..", "src", "exportar_html", "gestos_vista.js");
const FUENTE = readFileSync(RUTA, "utf8");

/** Carga el modulo en un contexto con la ventana que se le de. */
function cargar(ventana = {}) {
  const ctx = vm.createContext({ Math, Set, Map, isFinite, ...ventana });
  vm.runInContext(FUENTE + "\nthis.GestosVista = GestosVista;", ctx);
  return ctx.GestosVista;
}
const G = cargar();
const cerca = (a, b, eps = 1e-9) => assert.ok(Math.abs(a - b) < eps, `${a} != ${b}`);

test("dos dedos en el trackpad desplazan en las dos direcciones, siguiendo a los dedos", () => {
  // Un circulo: los deltas dan la vuelta y la vista con ellos.
  let x = 0, y = 0;
  for (let i = 0; i < 36; i++) {
    const a = (i / 36) * 2 * Math.PI;
    const r = G.deRueda({ deltaX: 10 * Math.cos(a), deltaY: 10 * Math.sin(a), deltaMode: 0 });
    assert.equal(r.zoom, undefined);
    cerca(r.dx, -10 * Math.cos(a));
    cerca(r.dy, -10 * Math.sin(a));
    x += r.dx; y += r.dy;
  }
  cerca(x, 0, 1e-6);
  cerca(y, 0, 1e-6);
  const diag = G.deRueda({ deltaX: 3, deltaY: -4, deltaMode: 0 });
  assert.deepEqual({ ...diag }, { dx: -3, dy: 4 });
});

test("deltaMode de lineas y de paginas se pasan a pixeles", () => {
  assert.equal(G.enPixeles(3, 1), 3 * G.LINEA);
  assert.equal(G.enPixeles(1, 2, 600), 600);
  assert.equal(G.enPixeles(1, 2), 800);
  assert.equal(G.enPixeles(7.5, 0), 7.5);
  assert.equal(G.enPixeles(undefined, 0), 0);
  assert.deepEqual({ ...G.deRueda({ deltaX: 0, deltaY: 3, deltaMode: 1 }) }, { dx: 0, dy: -3 * G.LINEA });
});

test("la rueda del raton: vertical, con Mayusculas de lado, con Ctrl acerca", () => {
  assert.deepEqual({ ...G.deRueda({ deltaX: 0, deltaY: 100, deltaMode: 0 }) }, { dx: 0, dy: -100 });
  assert.deepEqual({ ...G.deRueda({ deltaX: 0, deltaY: 100, deltaMode: 0, shiftKey: true }) }, { dx: -100, dy: 0 });
  // Si el navegador ya lo dio de lado (macOS), no se cambia otra vez.
  assert.deepEqual({ ...G.deRueda({ deltaX: 100, deltaY: 0, deltaMode: 0, shiftKey: true }) }, { dx: -100, dy: 0 });
  // Ctrl+rueda acerca con tope: una muesca no da un salto.
  const z = G.deRueda({ deltaX: 0, deltaY: -100, deltaMode: 0, ctrlKey: true }).zoom;
  cerca(z, Math.exp(G.TOPE_ZOOM * G.K_ZOOM));
  assert.ok(z > 1 && z < 1.3);
});

test("el pellizco del trackpad (ctrlKey) es acercar, suave y simetrico", () => {
  const dentro = G.deRueda({ deltaX: 0, deltaY: -4, deltaMode: 0, ctrlKey: true }).zoom;
  const fuera = G.deRueda({ deltaX: 0, deltaY: 4, deltaMode: 0, ctrlKey: true }).zoom;
  assert.ok(dentro > 1 && fuera < 1);
  cerca(dentro * fuera, 1);
  cerca(dentro, Math.exp(0.04));
  // Con Ctrl no se desplaza aunque traiga deltaX.
  assert.equal(G.deRueda({ deltaX: 5, deltaY: 1, deltaMode: 0, ctrlKey: true }).dx, undefined);
});

test("acercar alrededor de un punto lo deja donde estaba", () => {
  const v = { x: 37, y: -12, z: 1.7 };
  const p = { x: 210, y: 145 };
  const mundo = { x: (p.x - v.x) / v.z, y: (p.y - v.y) / v.z };
  for (const f of [1.25, 0.6, 3]) {
    const n = G.ampliarEn(v, f, p.x, p.y);
    cerca(n.z, v.z * f);
    cerca(mundo.x * n.z + n.x, p.x);
    cerca(mundo.y * n.z + n.y, p.y);
  }
  // Con tope, el punto tambien se queda.
  const t = G.ampliarEn(v, 100, p.x, p.y, 0.2, 4);
  assert.equal(t.z, 4);
  cerca(mundo.x * t.z + t.x, p.x);
});

test("dos dedos en la pantalla: acercan y arrastran a la vez, y la palma no cuenta", () => {
  const p = G.pellizco({ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 10, y: 20 }, { x: 210, y: 20 });
  cerca(p.f, 2);
  assert.deepEqual([p.cx, p.cy, p.dx, p.dy], [110, 20, 60, 20]);
  assert.equal(G.pellizco({ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 0, y: 0 }, { x: 300, y: 0 }), null);
  assert.equal(G.pellizco({ x: 0, y: 0 }, { x: 0, y: 0 }, { x: 0, y: 0 }, { x: 1, y: 0 }), null);
  assert.equal(G.pellizco({ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 900, y: 0 }, { x: 1000, y: 0 }), null);
});

/** Una caja de mentira que guarda sus escuchas. */
function cajaFalsa() {
  const escuchas = {};
  return {
    escuchas,
    style: {},
    clientHeight: 500,
    getBoundingClientRect: () => ({ left: 0, top: 0, width: 400, height: 500 }),
    setPointerCapture() {},
    addEventListener(tipo, fn, op) { (escuchas[tipo] ||= []).push({ fn, op }); },
    lanzar(tipo, e) {
      let parado = false;
      for (const { fn } of escuchas[tipo] || []) fn({ preventDefault() { parado = true; }, ...e });
      return parado;
    },
  };
}
function conVentana() {
  const cola = [];
  const G2 = cargar({
    window: { requestAnimationFrame: (fn) => cola.push(fn) },
    requestAnimationFrame: (fn) => { cola.push(fn); return cola.length; },
    document: { documentElement: { style: {} } },
  });
  const fotograma = () => { const c = cola.splice(0); c.forEach((fn) => fn()); };
  return { G2, fotograma };
}

test("conectar: rueda no pasiva, todo en un fotograma, y la pagina no se mueve sola", () => {
  const { G2, fotograma } = conVentana();
  const caja = cajaFalsa();
  const hecho = [];
  G2.conectar(caja, {
    mover: (dx, dy) => hecho.push(["mover", dx, dy]),
    ampliar: (f, x, y) => hecho.push(["ampliar", f, x, y]),
  });
  assert.equal(caja.style.touchAction, "none");
  assert.equal(caja.style.overscrollBehavior, "none");
  assert.equal(caja.escuchas.wheel[0].op.passive, false);
  assert.ok(caja.lanzar("wheel", { deltaX: 3, deltaY: 4, deltaMode: 0 }));
  caja.lanzar("wheel", { deltaX: 2, deltaY: -1, deltaMode: 0 });
  assert.equal(hecho.length, 0, "se espera al fotograma");
  fotograma();
  assert.deepEqual(hecho, [["mover", -5, -3]]);
  hecho.length = 0;
  caja.lanzar("wheel", { deltaX: 0, deltaY: -10, deltaMode: 0, ctrlKey: true, clientX: 50, clientY: 60 });
  caja.lanzar("wheel", { deltaX: 0, deltaY: -10, deltaMode: 0, ctrlKey: true, clientX: 50, clientY: 60 });
  fotograma();
  assert.equal(hecho.length, 1);
  cerca(hecho[0][1], Math.exp(0.2));
  assert.deepEqual(hecho[0].slice(2), [50, 60]);
});

test("conectar: Safari pellizca con gesturechange, y en el iPad no cuenta doble", () => {
  const { G2, fotograma } = conVentana();
  const caja = cajaFalsa();
  const hecho = [];
  G2.conectar(caja, { mover() {}, ampliar: (f, x, y) => hecho.push([f, x, y]) });
  assert.ok(caja.lanzar("gesturestart", { scale: 1 }));
  caja.lanzar("gesturechange", { scale: 1.5, clientX: 10, clientY: 20 });
  caja.lanzar("gesturechange", { scale: 2, clientX: 10, clientY: 20 });
  // La rueda con Ctrl que llegara durante el gesto no suma.
  caja.lanzar("wheel", { deltaX: 0, deltaY: -10, deltaMode: 0, ctrlKey: true, clientX: 10, clientY: 20 });
  fotograma();
  assert.equal(hecho.length, 1);
  cerca(hecho[0][0], 2);
  caja.lanzar("gestureend", {});
  hecho.length = 0;
  // Con dedos en la pantalla, del pellizco se encargan los dedos.
  caja.lanzar("pointerdown", { pointerId: 1, pointerType: "touch", clientX: 0, clientY: 0 });
  caja.lanzar("gesturestart", { scale: 1 });
  caja.lanzar("gesturechange", { scale: 1.5 });
  fotograma();
  assert.equal(hecho.length, 0);
});

test("conectar con dedos: uno arrastra, dos acercan y mueven el centro", () => {
  const { G2, fotograma } = conVentana();
  const caja = cajaFalsa();
  const v = { x: 0, y: 0, z: 1 };
  let agarrado = null;
  G2.conectar(caja, {
    dedos: true,
    mover: (dx, dy) => { v.x += dx; v.y += dy; },
    ampliar: (f, x, y) => Object.assign(v, G2.ampliarEn(v, f, x, y)),
    agarrado: (b) => { agarrado = b; },
  });
  caja.lanzar("pointerdown", { pointerId: 1, pointerType: "touch", clientX: 100, clientY: 100 });
  assert.equal(agarrado, true);
  caja.lanzar("pointermove", { pointerId: 1, clientX: 130, clientY: 90 });
  fotograma();
  assert.deepEqual(v, { x: 30, y: -10, z: 1 });
  caja.lanzar("pointerdown", { pointerId: 2, pointerType: "touch", clientX: 230, clientY: 90 });
  // El punto del mundo bajo el centro de los dedos sigue debajo del centro.
  const mundo = { x: (180 - v.x) / v.z, y: (90 - v.y) / v.z };
  caja.lanzar("pointermove", { pointerId: 2, clientX: 330, clientY: 90 });
  fotograma();
  cerca(v.z, 2);
  cerca(mundo.x * v.z + v.x, 230);
  cerca(mundo.y * v.z + v.y, 90);
  caja.lanzar("pointerup", { pointerId: 1 });
  caja.lanzar("pointerup", { pointerId: 2 });
  assert.equal(agarrado, false);
});

test("las copias del modulo son la misma", () => {
  for (const r of [
    "crates/pindesk-cad/src/gestos_vista.js",
    "apple/nucleo/pindesk-motor2d/src/exportar_html/gestos_vista.js",
    "apple/nucleo/pindesk-cad/src/gestos_vista.js",
  ]) {
    const ruta = join(repo, r);
    if (!existsSync(ruta)) continue;
    assert.equal(readFileSync(ruta, "utf8"), FUENTE, `${r} no es igual: copialo otra vez`);
  }
});
