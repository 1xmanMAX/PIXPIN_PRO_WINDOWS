// El menu del atajo (Alt+Mayús+P): lo mismo que el menu contextual, con
// teclas. Las cifras 1–9 eligen; «Preguntar al asistente» abre una caja
// para escribir la pregunta (en el menu contextual la pide la pagina).
// Lo hace todo `fondo.js`; esto solo elige y ensena como fue.

import { MENU } from "./nucleo.js";

const $ = (id) => document.getElementById(id);
const [pestana] = await chrome.tabs.query({ active: true, currentWindow: true });

let texto = "";
try {
  const [r] = await chrome.scripting.executeScript({
    target: { tabId: pestana.id },
    func: () => String(getSelection?.() ?? ""),
  });
  texto = (r?.result ?? "").trim();
} catch {
  // chrome://, la tienda…: no se puede leer la seleccion.
}
if (texto) $("vista").textContent = `«${texto.replace(/\s+/g, " ").slice(0, 120)}»`;

const botones = new Map();
for (const f of MENU) {
  if (f.contexto === "image") continue; // las imagenes, con clic derecho
  const b = document.createElement("button");
  b.innerHTML = `<kbd></kbd><span></span>`;
  b.querySelector("kbd").textContent = f.tecla ?? "";
  b.querySelector("span").textContent = f.titulo;
  b.disabled = f.contexto === "selection" && !texto;
  b.addEventListener("click", () => elegir(f.accion));
  $(f.contexto === "selection" ? "seleccion" : "pagina").append(b);
  if (f.tecla) botones.set(f.tecla, f.accion);
}

document.addEventListener("keydown", (e) => {
  if (e.target === $("pregunta")) {
    if (e.key === "Enter") mandar("asistente", $("pregunta").value);
    return;
  }
  const accion = botones.get(e.key);
  if (accion) {
    e.preventDefault();
    elegir(accion);
  }
});

function elegir(accion) {
  const fila = MENU.find((f) => f.accion === accion);
  if (fila.contexto === "selection" && !texto) return;
  if (accion === "asistente") {
    $("pregunta").style.display = "block";
    $("pregunta").focus();
    return;
  }
  mandar(accion);
}

async function mandar(accion, pregunta) {
  $("estado").className = "";
  $("estado").textContent = accion === "captura_pagina" ? "Capturando la página…" : "Enviando…";
  for (const b of document.querySelectorAll("button")) b.disabled = true;
  const r = await chrome.runtime.sendMessage({
    tipo: "pixpin-accion",
    accion,
    datos: {
      tabId: pestana.id,
      frameId: 0,
      url: pestana.url,
      titulo: pestana.title,
      texto,
      pregunta,
    },
  });
  if (r?.ok) {
    $("estado").textContent = r.aviso ?? "Hecho";
    setTimeout(() => window.close(), 700);
  } else {
    $("estado").className = "mal";
    $("estado").textContent = r?.error ?? "Algo falló";
  }
}
