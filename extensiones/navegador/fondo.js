// El trabajador de fondo de la extension: el menu contextual, lo que pide
// el menu del atajo (`menu.js`) y la charla con el puente.
//
// Cada accion junta lo que hace falta de la pagina (la seleccion con sus
// saltos de linea, la imagen ya leida, la captura entera) y se lo manda al
// puente con `chrome.runtime.sendNativeMessage`, que arranca
// `pixpin-puente.exe`, le da el mensaje y espera su respuesta.

import {
  HOST,
  MENU,
  TIPOS_DIRECTOS,
  TOPE_IMAGEN_BASE64,
  altoQueCabe,
  base64DeBytes,
  bytesDeBase64,
  explicarFallo,
  filaDe,
  mensajeDe,
  trozos,
} from "./nucleo.js";

// ------------------------------------------------------------ el menu

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: "pixpin",
      title: "PixPin Max",
      contexts: ["selection", "image", "page"],
    });
    for (const f of MENU) {
      chrome.contextMenus.create({
        id: f.accion,
        parentId: "pixpin",
        title: f.titulo,
        contexts: [f.contexto],
      });
    }
  });
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  if (!filaDe(info.menuItemId) || !tab?.id) return;
  hacer(info.menuItemId, {
    tabId: tab.id,
    frameId: info.frameId ?? 0,
    url: tab.url,
    titulo: tab.title,
    seleccion: info.selectionText,
    imagen: info.srcUrl,
    preguntarEnLaPagina: true,
  });
});

// Lo que pide el menu del atajo.
chrome.runtime.onMessage.addListener((m, _de, responder) => {
  if (m?.tipo !== "pixpin-accion") return false;
  hacer(m.accion, m.datos).then(responder, (e) => responder({ ok: false, error: explicarFallo(e) }));
  return true;
});

// ------------------------------------------------------------ hacer

/**
 * Hace `accion` con lo de la pestaña `d.tabId`. Nunca lanza: lo que falla
 * se dice con una notificacion y se devuelve.
 */
async function hacer(accion, d) {
  try {
    const fila = filaDe(accion);
    const datos = { url: d.url, titulo: d.titulo };
    if (fila.contexto === "selection") {
      datos.texto = (await seleccionDe(d.tabId, d.frameId)) || d.seleccion || d.texto || "";
      if (accion === "asistente") {
        datos.pregunta = d.pregunta;
        if (d.preguntarEnLaPagina) {
          const p = await preguntarEnLaPagina(d.tabId, d.frameId);
          if (p === null) return { ok: false, cancelado: true };
          datos.pregunta = p;
        }
      }
    } else if (fila.contexto === "image") {
      datos.imagen = await imagenDe(d.imagen);
    } else if (accion === "captura_pagina") {
      const c = await capturaEntera(d.tabId);
      datos.imagen = c.imagen;
      if (c.cortada) datos.titulo = `${d.titulo ?? ""} (cortada: la página es muy larga)`.trim();
    }
    const r = await alPuente(mensajeDe(accion, datos));
    if (!r?.ok) throw new Error(r?.error || "PixPin no contestó");
    senal(d.tabId, true);
    return r;
  } catch (e) {
    const texto = explicarFallo(e);
    avisar(texto);
    senal(d.tabId, false);
    return { ok: false, error: texto };
  }
}

function alPuente(mensaje) {
  return chrome.runtime.sendNativeMessage(HOST, mensaje);
}

/**
 * La seleccion con sus saltos de linea (la de `info.selectionText` viene
 * aplanada). En una pagina donde no se puede inyectar (la tienda, chrome://)
 * no hay nada y se usa la del menu.
 */
async function seleccionDe(tabId, frameId = 0) {
  try {
    const [r] = await chrome.scripting.executeScript({
      target: { tabId, frameIds: [frameId] },
      func: () => String(getSelection?.() ?? ""),
    });
    return r?.result ?? "";
  } catch {
    return "";
  }
}

/** Pregunta en la pagina que se le quiere preguntar al asistente. `null`: cancelado. */
async function preguntarEnLaPagina(tabId, frameId = 0) {
  try {
    const [r] = await chrome.scripting.executeScript({
      target: { tabId, frameIds: [frameId] },
      func: () => prompt("¿Qué le pregunto al asistente de PixPin sobre este texto?", "Explícame este texto"),
    });
    return r?.result ?? null;
  } catch {
    return "";
  }
}

/** La imagen de `src`, como `{tipo, base64}` que el puente acepta. */
async function imagenDe(src) {
  if (!src) throw new Error("No llegó la imagen");
  const resp = await fetch(src, { credentials: "include" });
  if (!resp.ok) throw new Error(`No se pudo leer la imagen (${resp.status})`);
  let blob = await resp.blob();
  let tipo = (blob.type || "").toLowerCase();
  if (!TIPOS_DIRECTOS.includes(tipo)) {
    // SVG, AVIF, ICO…: a PNG con un lienzo (si el navegador sabe pintarla).
    try {
      const bmp = await createImageBitmap(blob);
      const lienzo = new OffscreenCanvas(bmp.width, bmp.height);
      lienzo.getContext("2d").drawImage(bmp, 0, 0);
      blob = await lienzo.convertToBlob({ type: "image/png" });
      tipo = "image/png";
    } catch {
      throw new Error(`PixPin no sabe leer imágenes ${tipo || "de este tipo"}`);
    }
  }
  const base64 = base64DeBytes(new Uint8Array(await blob.arrayBuffer()));
  if (base64.length > TOPE_IMAGEN_BASE64) throw new Error("La imagen es demasiado grande");
  return { tipo, base64 };
}

// ------------------------------------------------- la pagina entera

/**
 * **La pagina entera** con el protocolo de depuracion de Chrome
 * (`Page.captureScreenshot` con `captureBeyondViewport`): pinta la pagina
 * completa sin desplazarla, asi que las cabeceras fijas salen una vez y no
 * en cada trozo, que es lo que estropea el «desplazar y coser». Se captura
 * por trozos de `ALTO_TROZO` y se cosen en un lienzo. Mientras dura, Chrome
 * enseña la barra «PixPin Max está depurando este navegador».
 */
async function capturaEntera(tabId) {
  const destino = { tabId };
  try {
    await chrome.debugger.attach(destino, "1.3");
  } catch (e) {
    throw new Error(`Esta página no deja capturarse (${e.message ?? e})`);
  }
  try {
    const cdp = (metodo, p) => chrome.debugger.sendCommand(destino, metodo, p);
    const m = await cdp("Page.getLayoutMetrics");
    const tam = m.cssContentSize ?? m.contentSize;
    const ancho = Math.ceil(tam.width);
    const escala = m.cssVisualViewport?.zoom ?? 1;
    // Lo que de verdad mide un px CSS en la imagen (pantallas al 150 %…).
    const ventana = await chrome.scripting
      .executeScript({ target: { tabId }, func: () => devicePixelRatio })
      .then(([r]) => r?.result ?? 1)
      .catch(() => 1);
    const { alto, cortada } = altoQueCabe(tam.height, ventana * escala);
    const partes = trozos(alto);
    const fotos = [];
    for (const t of partes) {
      const r = await cdp("Page.captureScreenshot", {
        format: "png",
        captureBeyondViewport: true,
        fromSurface: true,
        clip: { x: 0, y: t.y, width: ancho, height: t.alto, scale: 1 },
      });
      fotos.push(r.data);
    }
    if (fotos.length === 1) {
      return { imagen: await recortarSiHaceFalta({ tipo: "image/png", base64: fotos[0] }), cortada };
    }
    // Coser: cada trozo debajo del anterior, con su alto real en pixeles.
    const mapas = [];
    for (const f of fotos) {
      mapas.push(await createImageBitmap(new Blob([bytesDeBase64(f)], { type: "image/png" })));
    }
    const lienzo = new OffscreenCanvas(mapas[0].width, mapas.reduce((s, b) => s + b.height, 0));
    const ctx = lienzo.getContext("2d");
    let y = 0;
    for (const b of mapas) {
      ctx.drawImage(b, 0, y);
      y += b.height;
      b.close();
    }
    return { imagen: await deLienzo(lienzo), cortada };
  } finally {
    await chrome.debugger.detach(destino).catch(() => {});
  }
}

/** El lienzo en PNG; si no cabe en el puente, en JPEG. */
async function deLienzo(lienzo) {
  let blob = await lienzo.convertToBlob({ type: "image/png" });
  let tipo = "image/png";
  if ((blob.size * 4) / 3 > TOPE_IMAGEN_BASE64) {
    blob = await lienzo.convertToBlob({ type: "image/jpeg", quality: 0.85 });
    tipo = "image/jpeg";
  }
  const base64 = base64DeBytes(new Uint8Array(await blob.arrayBuffer()));
  if (base64.length > TOPE_IMAGEN_BASE64) throw new Error("La captura es demasiado grande");
  return { tipo, base64 };
}

/** Un PNG de un solo trozo que no cabe se pasa a JPEG. */
async function recortarSiHaceFalta(img) {
  if (img.base64.length <= TOPE_IMAGEN_BASE64) return img;
  const bmp = await createImageBitmap(new Blob([bytesDeBase64(img.base64)], { type: img.tipo }));
  const lienzo = new OffscreenCanvas(bmp.width, bmp.height);
  lienzo.getContext("2d").drawImage(bmp, 0, 0);
  return deLienzo(lienzo);
}

// ------------------------------------------------------------ avisos

function avisar(texto) {
  chrome.notifications.create({
    type: "basic",
    iconUrl: "iconos/pixpin.png",
    title: "PixPin Max",
    message: texto,
  });
}

/** Un ✓ o un ✗ en el icono, dos segundos. */
function senal(tabId, bien) {
  const opciones = tabId ? { tabId } : {};
  chrome.action.setBadgeBackgroundColor({ ...opciones, color: bien ? "#2E7BE6" : "#C62828" });
  chrome.action.setBadgeText({ ...opciones, text: bien ? "✓" : "✗" });
  setTimeout(() => chrome.action.setBadgeText({ ...opciones, text: "" }), 2000);
}
