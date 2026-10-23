// Pruebas de la extension sin Chrome: `node --test extensiones/navegador/pruebas`.
//
// - El nucleo (menu, mensaje para el puente, trozos de la captura, base64).
// - Que el menu de la extension y la tabla del puente en Rust coinciden.
// - Que el manifiesto es valido, sus ficheros existen y su "key" da el id
//   que registra herramientas/registrar-navegador.ps1.
// - Si esta compilado (PIXPIN_PUENTE_EXE o el target de siempre), una vuelta
//   entera: el mensaje que arma la extension, por la tuberia, al
//   pixpin-puente.exe de verdad en modo «en seco» (sin tocar PixPin).

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync, mkdtempSync, rmSync } from "node:fs";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  HOST,
  MENU,
  ALTO_MAXIMO_PX,
  altoQueCabe,
  base64DeBytes,
  bytesDeBase64,
  explicarFallo,
  mensajeDe,
  pideTexto,
  trozos,
} from "../nucleo.js";

const aqui = dirname(fileURLToPath(import.meta.url));
const ext = join(aqui, "..");
const repo = join(ext, "..", "..");
const PNG_1X1 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

test("el menu y la tabla del puente en Rust tienen las mismas acciones", () => {
  const rust = readFileSync(join(repo, "apps/pixpin-puente/src/traducir.rs"), "utf8");
  const tabla = rust.slice(rust.indexOf("pub const ACCIONES"), rust.indexOf("];", rust.indexOf("pub const ACCIONES")));
  const deRust = [...tabla.matchAll(/\("([a-z_]+)",/g)].map((m) => m[1]);
  assert.equal(deRust.length, 12);
  assert.deepEqual(MENU.map((f) => f.accion).sort(), [...deRust].sort());
});

test("cada fila del menu tiene un contexto de Chrome y las teclas no se repiten", () => {
  for (const f of MENU) assert.ok(["selection", "image", "page"].includes(f.contexto), f.accion);
  const teclas = MENU.filter((f) => f.tecla).map((f) => f.tecla);
  assert.equal(new Set(teclas).size, teclas.length);
  assert.ok(pideTexto("tarea") && !pideTexto("ocr") && !pideTexto("captura_pagina"));
});

test("el mensaje lleva solo lo que hace falta", () => {
  assert.deepEqual(
    mensajeDe("tarea", { texto: "  comprar yeso \n", url: "https://a.pe", titulo: "A", imagen: { base64: "x" } }),
    { v: 1, accion: "tarea", texto: "comprar yeso", url: "https://a.pe", titulo: "A" },
  );
  assert.deepEqual(mensajeDe("asistente", { texto: "x", pregunta: " ¿y esto? " }), {
    v: 1,
    accion: "asistente",
    texto: "x",
    pregunta: "¿y esto?",
  });
  assert.deepEqual(mensajeDe("ocr", { imagen: { tipo: "image/png", base64: "QQ==" }, texto: "no va" }), {
    v: 1,
    accion: "ocr",
    imagen: { tipo: "image/png", base64: "QQ==" },
  });
  assert.deepEqual(mensajeDe("enlace_al_chat", { url: "https://a.pe", titulo: "A" }), {
    v: 1,
    accion: "enlace_al_chat",
    url: "https://a.pe",
    titulo: "A",
  });
});

test("caso negativo: sin seleccion, sin imagen o con una accion que no existe", () => {
  assert.throws(() => mensajeDe("nota", { texto: "   " }), /No hay texto seleccionado/);
  assert.throws(() => mensajeDe("pinear_imagen", {}), /No llegó la imagen/);
  assert.throws(() => mensajeDe("captura_pagina", { url: "https://a.pe" }), /No llegó la imagen/);
  assert.throws(() => mensajeDe("borrar_todo", {}), /desconocida/);
});

test("los trozos de la captura cubren la pagina sin huecos ni solapes", () => {
  for (const alto of [1, 3999, 4000, 4001, 12345, 30000]) {
    const t = trozos(alto);
    assert.equal(t[0].y, 0);
    assert.equal(t.reduce((s, x) => s + x.alto, 0), alto);
    for (let i = 1; i < t.length; i++) assert.equal(t[i].y, t[i - 1].y + t[i - 1].alto);
    assert.ok(t.every((x) => x.alto > 0 && x.alto <= 4000));
  }
  assert.deepEqual(trozos(0), []);
  assert.deepEqual(trozos(NaN), []);
});

test("una pagina demasiado larga se corta a lo que cabe en el lienzo", () => {
  assert.deepEqual(altoQueCabe(8000.4, 1), { alto: 8001, cortada: false });
  assert.deepEqual(altoQueCabe(100000, 1), { alto: ALTO_MAXIMO_PX, cortada: true });
  assert.deepEqual(altoQueCabe(100000, 1.5), { alto: 20000, cortada: true });
});

test("base64 de ida y vuelta, tambien con mas de un trozo", () => {
  const bytes = new Uint8Array(100_003).map((_, i) => (i * 37) % 256);
  const b64 = base64DeBytes(bytes);
  assert.equal(b64, Buffer.from(bytes).toString("base64"));
  assert.deepEqual(bytesDeBase64(b64), bytes);
});

test("los fallos del navegador se explican en cristiano", () => {
  assert.match(explicarFallo(new Error("Specified native messaging host not found.")), /instalar\.ps1/);
  assert.match(explicarFallo("Access to the specified native messaging host is forbidden."), /no reconoce/);
  assert.equal(explicarFallo(new Error("Imagen rota")), "Imagen rota");
});

test("el manifiesto es valido y su clave da el id que se registra", () => {
  const m = JSON.parse(readFileSync(join(ext, "manifest.json"), "utf8"));
  assert.equal(m.manifest_version, 3);
  for (const p of ["contextMenus", "nativeMessaging", "scripting", "debugger"]) assert.ok(m.permissions.includes(p), p);
  for (const f of [m.background.service_worker, m.action.default_popup, ...Object.values(m.icons)]) {
    assert.ok(existsSync(join(ext, f)), f);
  }
  assert.equal(m.commands._execute_action.suggested_key.default, "Alt+Shift+P");
  // El id de una extension sin empaquetar con "key": sha256 de la clave,
  // 32 cifras hex pasadas a a–p.
  const id = [...createHash("sha256").update(Buffer.from(m.key, "base64")).digest("hex").slice(0, 32)]
    .map((c) => String.fromCharCode(97 + parseInt(c, 16)))
    .join("");
  const ps1 = readFileSync(join(repo, "herramientas/registrar-navegador.ps1"), "utf8");
  assert.ok(ps1.includes(`$idExtension = '${id}'`), `registrar-navegador.ps1 no registra ${id}`);
  assert.ok(ps1.includes(`$nombre = '${HOST}'`));
});

function exeDelPuente() {
  const candidatos = [
    process.env.PIXPIN_PUENTE_EXE,
    process.env.CARGO_TARGET_DIR && join(process.env.CARGO_TARGET_DIR, "debug", "pixpin-puente.exe"),
    "F:/pixpin-targets/integ-navegador/debug/pixpin-puente.exe",
  ];
  return candidatos.find((c) => c && existsSync(c));
}

function alPuente(exe, carpeta, cuerpo) {
  const json = Buffer.from(typeof cuerpo === "string" ? cuerpo : JSON.stringify(cuerpo));
  const largo = Buffer.alloc(4);
  largo.writeUInt32LE(json.length);
  const inicio = performance.now();
  const r = spawnSync(exe, ["chrome-extension://hcbhbfnjmdofjkdgeljognmjgppdiflo/"], {
    input: Buffer.concat([largo, json]),
    env: { ...process.env, PIXPIN_PUENTE_EN_SECO: carpeta },
  });
  const ms = performance.now() - inicio;
  assert.equal(r.status, 0);
  assert.equal(r.stdout.readUInt32LE(0), r.stdout.length - 4);
  return { respuesta: JSON.parse(r.stdout.subarray(4).toString("utf8")), ms };
}

test("de la extension al puente de verdad por la tuberia (en seco)", (t) => {
  const exe = exeDelPuente();
  if (!exe) {
    t.skip("pixpin-puente.exe sin compilar");
    return;
  }
  const carpeta = mkdtempSync(join(tmpdir(), "pixpin-ext-"));
  try {
    const tiempos = [];
    for (const f of MENU) {
      const m = mensajeDe(f.accion, {
        texto: "Comprar yeso",
        url: "https://obra.pe/a",
        titulo: "Obra",
        imagen: { tipo: "image/png", base64: PNG_1X1 },
      });
      const { respuesta, ms } = alPuente(exe, carpeta, m);
      tiempos.push(ms);
      assert.equal(respuesta.ok, true, `${f.accion}: ${JSON.stringify(respuesta)}`);
      const pedido = JSON.parse(respuesta.pedido);
      assert.equal(pedido.pixpin, 1);
    }
    const { respuesta } = alPuente(exe, carpeta, "{roto");
    assert.equal(respuesta.ok, false);
    tiempos.sort((a, b) => a - b);
    console.log(`puente (depuracion), ${tiempos.length} vueltas: mediana ${tiempos[6].toFixed(1)} ms, peor ${tiempos.at(-1).toFixed(1)} ms`);
  } finally {
    rmSync(carpeta, { recursive: true, force: true });
  }
});
