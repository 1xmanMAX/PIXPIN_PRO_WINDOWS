// Lista los commits de Excalidraw (oficial y fork de zsviczian) que tocan
// ficheros del mapa desde la revision portada. Uso:
//   node herramientas/novedades-excalidraw.mjs [ruta-clon-oficial] [ruta-clon-zsviczian]
// Por defecto usa "proyectos de referencia/excalidraw" y ".../excalidraw-zsviczian".
import { execFileSync } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

// decodeURIComponent: la ruta del proyecto trae espacios ("THE FORGE"), que
// import.meta.url codifica como %20; sin decodificar, readFileSync no la
// encuentra.
const raiz = decodeURIComponent(new URL("..", import.meta.url).pathname).replace(/^\/([A-Za-z]:)/, "$1");
const mapa = readFileSync(join(raiz, "docs", "excalidraw", "mapa.md"), "utf8");
const filas = [...mapa.matchAll(/^\| excalidraw:(\S+)[^|]*\| (\w+) \|/gm)].map((m) => ({
  ruta: m[1],
  revision: m[2],
}));

const clones = [
  process.argv[2] ?? join(raiz, "proyectos de referencia", "excalidraw"),
  process.argv[3] ?? join(raiz, "proyectos de referencia", "excalidraw-zsviczian"),
];

for (const clon of clones) {
  if (!existsSync(join(clon, ".git"))) {
    console.log(`(no hay clon en ${clon}: git clone https://github.com/excalidraw/excalidraw)`);
    continue;
  }
  execFileSync("git", ["-C", clon, "fetch", "--quiet", "origin"]);
  console.log(`\n== ${clon}`);
  for (const { ruta, revision } of filas) {
    let log = "";
    try {
      log = execFileSync("git", ["-C", clon, "log", "--oneline", `${revision}..origin/HEAD`, "--", ruta], {
        encoding: "utf8",
      }).trim();
    } catch {
      log = `(la revision ${revision} no existe en este clon)`;
    }
    console.log(`\n${ruta} desde ${revision}:\n${log || "  sin cambios"}`);
  }
}
console.log("\nperfect-freehand: comparar la version de packages/excalidraw/package.json con 1.2.0.");
