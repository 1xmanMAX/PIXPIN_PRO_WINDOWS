// Genera crates/pixpin-motor2d/tests/oraculo/tinta.json con la tinta REAL de
// Excalidraw. Las dos envolturas y getSvgPathFromStroke son copia literal de
// excalidraw/excalidraw packages/element/src/shape.ts @afa3a65 (MIT,
// Copyright (c) 2020 Excalidraw); getStroke y LaserPointer vienen de npm, en
// las versiones exactas que fija packages/excalidraw/package.json.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { getStroke } from "perfect-freehand";
import { LaserPointer } from "@excalidraw/laser-pointer/dist/esm.js";
import { casos } from "./casos.mjs";

const aqui = dirname(fileURLToPath(import.meta.url));
const destino = join(aqui, "..", "..", "crates", "pixpin-motor2d", "tests", "oraculo", "tinta.json");

function variable(c) {
  const simulatePressure = c.presiones.length === 0;
  const input = simulatePressure
    ? c.puntos
    : c.puntos.map(([x, y], i) => [x, y, c.presiones[i]]);
  return getStroke(input, {
    simulatePressure,
    size: c.grosor * 4.25,
    thinning: 0.6,
    smoothing: 0.5,
    streamline: c.streamline,
    easing: (t) => Math.sin((t * Math.PI) / 2),
    last: true,
  });
}

function constante(c) {
  const lp = new LaserPointer({
    size: c.grosor * 1.4,
    streamline: c.streamline,
    simplify: 0,
    sizeMapping: (d) => Math.max(0.1, d.pressure),
  });
  c.puntos.forEach(([x, y]) => lp.addPoint([x, y, 1]));
  return lp.getStrokeOutline().map(([x, y]) => [x, y]);
}

const med = (A, B) => [(A[0] + B[0]) / 2, (A[1] + B[1]) / 2];
const TO_FIXED_PRECISION = /(\s?[A-Z]?,?-?[0-9]*\.[0-9]{0,2})(([0-9]|e|-)*)/g;

// Construye la cadena una sola vez; svg_crudo es la versión sin la
// truncación final (esa regex corrompe números en notación exponencial),
// svg es la versión con la truncación tal cual la hace Excalidraw.
function caminosSvgDesdeTrazo(points) {
  if (!points.length) return { crudo: "", truncado: "" };
  const max = points.length - 1;
  const crudo = points
    .reduce(
      (acc, point, i, arr) => {
        if (i === max) acc.push(point, med(point, arr[0]), "L", arr[0], "Z");
        else acc.push(point, med(point, arr[i + 1]));
        return acc;
      },
      ["M", points[0], "Q"],
    )
    .join(" ");
  return { crudo, truncado: crudo.replace(TO_FIXED_PRECISION, "$1") };
}

const salida = {
  origen: { excalidraw: "afa3a65", "perfect-freehand": "1.2.0", "@excalidraw/laser-pointer": "1.3.1" },
  casos: casos.map((c) => {
    const contorno = c.variabilidad === "constant" ? constante(c) : variable(c);
    const { crudo, truncado } = caminosSvgDesdeTrazo(contorno);
    return { ...c, contorno, svg: truncado, svg_crudo: crudo };
  }),
};

mkdirSync(dirname(destino), { recursive: true });
writeFileSync(destino, JSON.stringify(salida, null, 1) + "\n");
console.log(`${salida.casos.length} casos -> ${destino}`);
