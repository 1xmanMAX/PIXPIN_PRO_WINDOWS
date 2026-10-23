// Lo que la extension decide sin tocar el navegador: el menu, el mensaje
// para el puente (`pixpin-puente.exe`) y las cuentas de la captura de la
// pagina entera. Aparte de `fondo.js` para probarlo en Node
// (`pruebas/nucleo.test.js`) sin Chrome.
//
// La extension NO sabe nada de los pedidos de PixPin: manda QUE se eligio
// con lo que habia en la pagina, y el puente lo traduce
// (`apps/pixpin-puente/src/traducir.rs`). Las dos tablas de acciones tienen
// que coincidir; la prueba de Node lo comprueba leyendo la de Rust.

/** El nombre del puente registrado en HKCU (herramientas/registrar-navegador.ps1). */
export const HOST = "com.pixpinmax.puente";

/** Version del mensaje que entiende el puente. */
export const VERSION = 1;

/**
 * El menu: una fila por accion. `contexto` es el de chrome.contextMenus
 * (`selection`, `image`, `page`); `tecla`, la del menu del atajo.
 */
export const MENU = [
  { accion: "tarea", contexto: "selection", titulo: "Nueva tarea", tecla: "1" },
  { accion: "nota", contexto: "selection", titulo: "Nueva nota", tecla: "2" },
  { accion: "chat", contexto: "selection", titulo: "Al chat", tecla: "3" },
  { accion: "leccion", contexto: "selection", titulo: "Lección aprendida", tecla: "4" },
  { accion: "leer", contexto: "selection", titulo: "Leer en voz alta", tecla: "5" },
  { accion: "asistente", contexto: "selection", titulo: "Preguntar al asistente…", tecla: "6" },
  { accion: "pinear_imagen", contexto: "image", titulo: "Pinear la imagen" },
  { accion: "imagen_al_chat", contexto: "image", titulo: "Imagen al chat" },
  { accion: "ocr", contexto: "image", titulo: "Texto de la imagen (OCR)" },
  { accion: "captura_pagina", contexto: "page", titulo: "Captura de la página entera", tecla: "7" },
  { accion: "enlace_al_chat", contexto: "page", titulo: "Enlace al chat", tecla: "8" },
  { accion: "pagina_al_movil", contexto: "page", titulo: "Enviar la página al móvil", tecla: "9" },
];

/** Las imagenes que el puente acepta tal cual; las demas se pasan a PNG. */
export const TIPOS_DIRECTOS = ["image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp"];

/** Lo mas que acepta el puente (`protocolo::TOPE_ENTRADA`), con margen para el JSON. */
export const TOPE_IMAGEN_BASE64 = 46 * 1024 * 1024;

/** Lo mas alto que se captura (px de imagen): el lienzo de Chrome no pasa de 32 767. */
export const ALTO_MAXIMO_PX = 30000;

/** Cada trozo de la captura de la pagina entera (px CSS). */
export const ALTO_TROZO = 4000;

/** La fila del menu de una accion, o `undefined`. */
export function filaDe(accion) {
  return MENU.find((f) => f.accion === accion);
}

/** Si la accion necesita texto seleccionado. */
export function pideTexto(accion) {
  return filaDe(accion)?.contexto === "selection";
}

/**
 * El mensaje para el puente. `datos`: `{texto, url, titulo, imagen, pregunta}`
 * (`imagen` = `{tipo, base64}`). Lo que no hace falta no se manda.
 */
export function mensajeDe(accion, datos = {}) {
  const fila = filaDe(accion);
  if (!fila) throw new Error(`Acción desconocida: ${accion}`);
  const m = { v: VERSION, accion };
  if (fila.contexto === "selection") {
    const texto = (datos.texto ?? "").trim();
    if (!texto) throw new Error("No hay texto seleccionado");
    m.texto = texto;
    if (accion === "asistente" && datos.pregunta?.trim()) m.pregunta = datos.pregunta.trim();
  }
  if (fila.contexto === "image" || accion === "captura_pagina") {
    if (!datos.imagen?.base64) throw new Error("No llegó la imagen");
    m.imagen = { tipo: datos.imagen.tipo, base64: datos.imagen.base64 };
  }
  if (datos.url) m.url = String(datos.url);
  if (datos.titulo) m.titulo = String(datos.titulo);
  return m;
}

/**
 * Los trozos `{y, alto}` (px CSS) en que se captura una pagina de `alto`
 * px. Chrome hace mal las capturas muy altas de un tiron; en trozos de
 * `ALTO_TROZO`, bien.
 */
export function trozos(alto, altoTrozo = ALTO_TROZO) {
  const fuera = [];
  if (!(alto > 0) || !(altoTrozo > 0)) return fuera;
  for (let y = 0; y < alto; y += altoTrozo) {
    fuera.push({ y, alto: Math.min(altoTrozo, alto - y) });
  }
  return fuera;
}

/**
 * Cuanto de la pagina cabe (px CSS) con `escala` px de imagen por px CSS,
 * y si hubo que cortarla.
 */
export function altoQueCabe(altoPagina, escala) {
  const tope = Math.floor(ALTO_MAXIMO_PX / Math.max(escala, 0.1));
  const alto = Math.ceil(altoPagina);
  return alto > tope ? { alto: tope, cortada: true } : { alto, cortada: false };
}

/** Bytes a base64 sin pasar por cadenas enormes de una vez. */
export function base64DeBytes(bytes) {
  let s = "";
  const paso = 0x8000;
  for (let i = 0; i < bytes.length; i += paso) {
    s += String.fromCharCode.apply(null, bytes.subarray(i, i + paso));
  }
  return btoa(s);
}

/** Base64 a bytes (lo que devuelve `Page.captureScreenshot`). */
export function bytesDeBase64(b64) {
  const s = atob(b64);
  const v = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) v[i] = s.charCodeAt(i);
  return v;
}

/** El aviso de un fallo del puente que entiende una persona. */
export function explicarFallo(error) {
  const t = String(error?.message ?? error ?? "");
  if (/native messaging host not found|Specified native messaging host not found/i.test(t)) {
    return "No está instalado el puente de PixPin: ejecuta herramientas\\instalar.ps1";
  }
  if (/Access to the specified native messaging host is forbidden/i.test(t)) {
    return "El puente de PixPin no reconoce esta extensión: vuelve a ejecutar herramientas\\instalar.ps1";
  }
  if (/exited|disconnected/i.test(t)) {
    return "El puente de PixPin se cerró sin contestar";
  }
  return t || "Algo falló";
}
