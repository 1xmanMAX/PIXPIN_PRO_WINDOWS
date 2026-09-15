// Genera crates/pixpin-render/src/iconos_excalidraw.rs desde
// docs/excalidraw/iconos.json (extraido de packages/excalidraw/components/
// icons.tsx de Excalidraw, licencia MIT).
//
// Uso: node herramientas/iconos-excalidraw.mjs
//
// Cada icono sale como una `pub const` de tipo `Icono`. No hay tabla por
// nombre a proposito: sin ella el enlazador tira los iconos que nadie usa y
// el ejecutable solo carga los que pinta la interfaz.

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const raiz = join(dirname(fileURLToPath(import.meta.url)), "..");
const iconos = JSON.parse(
  readFileSync(join(raiz, "docs/excalidraw/iconos.json"), "utf8"),
);

const constante = (nombre) =>
  nombre
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/([A-Z])([A-Z][a-z])/g, "$1_$2")
    .toUpperCase() || "ICONO";

const pintura = (v) => {
  if (v === undefined || v === null || v === "none") return "Pintura::Nada";
  if (v === "#fff" || v === "#ffffff" || v === "white") return "Pintura::Blanco";
  // currentColor, var(--icon-fill-color) y el negro de los logotipos: todo
  // toma el color que pide la interfaz.
  return "Pintura::Actual";
};

const numero = (x) => {
  const s = String(Number(x));
  return s.includes(".") || s.includes("e") ? s : `${s}.0`;
};

// transform de SVG (o `css:` del estilo del <svg>) a matrix(a b c d e f).
function matriz(transform, vista) {
  if (!transform) return null;
  let m = [1, 0, 0, 1, 0, 0];
  // m = m * n: en SVG, la transformacion escrita a la izquierda es la
  // exterior, asi que se acumulan por la derecha.
  const por = (n) => {
    const [a, b, c, d, e, f] = m;
    const [A, B, C, D, E, F] = n;
    m = [
      a * A + c * B,
      b * A + d * B,
      a * C + c * D,
      b * C + d * D,
      a * E + c * F + e,
      b * E + d * F + f,
    ];
  };
  let texto = transform;
  if (texto.startsWith("css:")) {
    // Transformada CSS sobre el <svg>: el origen es el centro de la caja.
    texto = texto.slice(4);
    const cx = vista[0] + vista[2] / 2;
    const cy = vista[1] + vista[3] / 2;
    const g = /rotate\(\s*(-?[\d.]+)deg\s*\)/.exec(texto);
    if (!g) throw new Error(`transform css desconocido: ${transform}`);
    texto = `rotate(${g[1]} ${cx} ${cy})`;
  }
  const re = /(\w+)\(([^)]*)\)/g;
  let r;
  while ((r = re.exec(texto))) {
    const v = r[2].split(/[\s,]+/).filter(Boolean).map(Number);
    switch (r[1]) {
      case "translate":
        por([1, 0, 0, 1, v[0], v[1] ?? 0]);
        break;
      case "scale":
        por([v[0], 0, 0, v[1] ?? v[0], 0, 0]);
        break;
      case "rotate": {
        const rad = (v[0] * Math.PI) / 180;
        const cos = Math.cos(rad);
        const sin = Math.sin(rad);
        const [cx, cy] = [v[1] ?? 0, v[2] ?? 0];
        por([1, 0, 0, 1, cx, cy]);
        por([cos, sin, -sin, cos, 0, 0]);
        por([1, 0, 0, 1, -cx, -cy]);
        break;
      }
      case "matrix":
        por(v);
        break;
      default:
        throw new Error(`transform desconocido: ${transform}`);
    }
  }
  return m.map((x) => (Math.abs(x) < 1e-9 ? 0 : Math.round(x * 1e6) / 1e6));
}

const cadena = (s) => JSON.stringify(s);

let salida = `//! Iconos de Excalidraw (https://github.com/excalidraw/excalidraw).
//!
//! GENERADO por \`herramientas/iconos-excalidraw.mjs\` desde
//! \`docs/excalidraw/iconos.json\`; no editar a mano.
//!
//! Copyright (c) 2020 Excalidraw. Licencia MIT (ver THIRD-PARTY-NOTICES.md).
//! Algunos iconos vienen de Tabler Icons (MIT), como anota el origen.
#![allow(dead_code)]

use crate::icono::{Icono, Pintura, TrazoIcono};

`;

const vistos = new Set();
let cuantos = 0;
for (const icono of iconos) {
  if (icono.noSvg || !icono.trazos || !icono.viewBox) continue;
  const vista = icono.viewBox.split(/\s+/).map(Number);
  let nombre = constante(icono.nombre);
  // Excalidraw tiene parejas que solo difieren en la mayuscula inicial
  // (`HelpIcon` y `helpIcon`): la de minuscula lleva `_MIN`.
  if (vistos.has(nombre) && /^[a-z]/.test(icono.nombre)) nombre += "_MIN";
  if (vistos.has(nombre)) throw new Error(`nombre repetido: ${nombre}`);
  vistos.add(nombre);

  const definiciones = new Map();
  for (const t of icono.trazos) {
    if (t.rol) definiciones.set(t.rol.split("#")[1], t.d);
  }
  const lienzoEntero = `M0 0h${vista[2]}v${vista[3]}H0z`.toLowerCase();

  const trazos = icono.trazos.filter((t) => !t.rol);
  const partes = trazos.map((t) => {
    let mascara = "None";
    if (t.mascara) {
      const id = /url\(#([^)]+)\)/.exec(t.mascara)?.[1];
      const d = definiciones.get(id);
      // Un recorte del tamano de la caja entera no recorta nada.
      if (d && d.replace(/\s+/g, "").toLowerCase() !== lienzoEntero.replace(/\s+/g, "")) {
        mascara = `Some(${cadena(d)})`;
      }
    }
    const m = matriz(t.transform, vista);
    return `        TrazoIcono {
            d: ${cadena(t.d)},
            relleno: ${pintura(t.fill)},
            trazo: ${pintura(t.stroke)},
            grosor: ${numero(t.strokeWidth ?? 1)},
            extremo_redondo: ${t.linecap === "round"},
            union_redonda: ${t.linejoin === "round"},
            opacidad: ${numero(t.opacity ?? 1)},
            par_impar: ${t.fillRule === "evenodd"},
            matriz: ${m ? `Some([${m.map(numero).join(", ")}])` : "None"},
            mascara: ${mascara},
        },`;
  });
  salida += `/// ${icono.nombre}${icono.uso ? ` — ${icono.uso}` : ""}
pub const ${nombre}: Icono = Icono {
    vista: (${vista.map(numero).join(", ")}),
    trazos: &[
${partes.join("\n")}
    ],
};

`;
  cuantos++;
}

const destino = join(raiz, "crates/pixpin-render/src/iconos_excalidraw.rs");
writeFileSync(destino, salida.replace(/\n+$/, "\n"));
console.log(`${cuantos} iconos -> ${destino}`);
