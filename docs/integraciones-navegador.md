# La extensión del navegador (Chrome y Edge)

Clic derecho en una página → **PixPin Max ▸** y lo que ves va a PixPin, sin copiar y pegar.
También con el atajo **Alt+Mayús+P**, que abre el mismo menú con teclas (1–9).

| Sobre… | Opción | Qué hace en PixPin | Pedido |
|---|---|---|---|
| Texto seleccionado | Nueva tarea | La apunta en el **Inbox**, con la dirección de la página detrás | `anadir_tarea` |
| | Nueva nota | Nota nueva, abierta, con un enlace `[título](url)` al pie | `nota_nueva` |
| | Al chat | Mensaje en «Mensajes guardados», con título y dirección | `chat` |
| | Lección aprendida | La ficha de lección nueva, con «Fuente: …» | `leccion_nueva` |
| | Leer en voz alta | Lo lee la voz de PixPin sin abrir nada; se para con los mandos de Windows o las teclas multimedia | `leer_en_voz` (nuevo) |
| | Preguntar al asistente… | Pide la pregunta (o «Explícame este texto») y se la manda al asistente con el texto | `asistente` |
| Imagen | Pinear la imagen | La saca a la pantalla como pin | `pinear` |
| | Imagen al chat | Foto en «Mensajes guardados», con la página de donde salió | `chat` + `imagenes` |
| | Texto de la imagen (OCR) | OCR de Windows → portapapeles; el globo dice lo copiado | `ocr` (nuevo) |
| La página | Captura de la página entera | Toda la página, de arriba abajo, como foto en el chat | `chat` + `imagenes` |
| | Enlace al chat | Título y dirección en el chat | `chat` |
| | Enviar la página al móvil | Una página `.html` con el enlace a todos los móviles del grupo (el móvil la abre y salta a la dirección) | `enviar_al_movil` |

Si PixPin está cerrada, se abre sola. Un ✓ o ✗ en el icono de la extensión dice cómo fue; lo que
falla sale también como notificación del navegador.

## Instalar

1. **El puente.** `herramientas\instalar.ps1` compila `pixpin-puente.exe`, lo copia junto a
   `pixpinmax.exe` (`%LOCALAPPDATA%\Programs\PixPinMax\`) y lo registra para este usuario con
   `herramientas\registrar-navegador.ps1`:
   - escribe `pixpin-puente.json` (el manifiesto del host) junto al exe, y
   - pone su ruta en `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.pixpinmax.puente` y en
     `HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.pixpinmax.puente`.

   Para ver lo que haría sin tocar nada:
   `powershell -ExecutionPolicy Bypass -File herramientas\registrar-navegador.ps1 -SoloMostrar`.
   Para quitarlo: `… registrar-navegador.ps1 -Quitar`.

2. **La extensión, sin empaquetar.**
   - Chrome: `chrome://extensions` → activa **Modo de desarrollador** (arriba a la derecha) →
     **Cargar extensión sin empaquetar** → elige la carpeta `extensiones\navegador` del repositorio.
   - Edge: `edge://extensions` → **Modo de desarrollador** (abajo a la izquierda) → **Cargar
     desempaquetada** → la misma carpeta.
   - El id tiene que salir **`hcbhbfnjmdofjkdgeljognmjgppdiflo`** (lo fija la `key` de
     `manifest.json`); es el único que el puente deja entrar.
   - El atajo se cambia en `chrome://extensions/shortcuts` (`edge://extensions/shortcuts`).
   - Tras cambiar ficheros de la extensión, el botón ⟳ de su tarjeta la recarga.

## Cómo funciona

```
menú / Alt+Mayús+P ─► fondo.js ─► chrome.runtime.sendNativeMessage("com.pixpinmax.puente", {…})
                                     │  (4 bytes de largo + JSON por la entrada estándar)
                                     ▼
                              pixpin-puente.exe ─► WM_COPYDATA (pedido v1) ─► PixPin Max
                                     │  contesta {"ok":true,"aviso":"…"} y se cierra
```

- **La extensión** (`extensiones/navegador/`, Manifest V3, JS sin dependencias ni empaquetador):
  `nucleo.js` (el menú y el mensaje, puro), `fondo.js` (menú contextual, selección, imagen,
  captura), `menu.html`/`menu.js` (el menú del atajo). Manda **qué se eligió** con lo que había:
  `{v:1, accion, texto?, pregunta?, url?, titulo?, imagen?: {tipo, base64}, aparato?}`.
- **El puente** (`apps/pixpin-puente`): lee un mensaje, lo traduce a un pedido de
  `docs/protocolo-pedidos.md` (`traducir.rs`, una tabla), deja la imagen en
  `<raíz>\cache\navegador\nav-<ms>-<pid>.<ext>` y manda el pedido con
  `pixpin_shell::mensajero::enviar_pedido`. Si nadie contesta, arranca `pixpinmax.exe` (el de su
  carpeta o el instalado) y reintenta hasta 8 s. Lo de la caché se borra a los 7 días.
- **Rápido**: el puente no espera a que PixPin haga nada (OCR, voz, móvil van en hilos de la app);
  solo escribe la imagen y manda el pedido.

### Decisiones

- **Página entera con `chrome.debugger`** (`Page.captureScreenshot` con
  `captureBeyondViewport`), no desplazando y cosiendo: pinta la página completa sin moverla, así
  que las cabeceras fijas salen una vez, no hay que esperar al límite de `captureVisibleTab` (2 por
  segundo) y no se mueve lo que el usuario está mirando. Se pide en trozos de 4 000 px y se cosen
  en un `OffscreenCanvas`. Lo que cuesta: mientras captura, el navegador enseña la barra «PixPin Max
  está depurando este navegador». No funciona en `chrome://` ni en la tienda (se avisa). Una página
  de más de 30 000 px de imagen se corta ahí (y lo dice el título); si el PNG no cabe en el puente
  (46 MB en base64), va en JPEG.
- **Las imágenes** se leen con `fetch` desde la extensión (por eso pide acceso a todas las webs:
  sin él, las de otro dominio no se pueden leer). PNG, JPEG, GIF, WebP y BMP van tal cual; SVG,
  AVIF y demás se pasan a PNG con un lienzo. El puente comprueba los primeros bytes: solo escribe
  en el disco lo que de verdad es una imagen.
- **Al móvil va una página `.html`**, porque `enviar_al_movil` solo lleva ficheros: la página
  redirige a la dirección y lleva el enlace por si no salta. Solo `http(s)`.
- **Un texto enorme** se recorta hasta caber en un pedido (64 KB) y el aviso lo dice.
- **Dos acciones nuevas en la app**: `leer_en_voz {texto, titulo?}` y `ocr {ruta}`
  (`apps/pixpin/src/pedidos.rs`, con pruebas).

## Pruebas

- Rust: `cargo test -p pixpin-puente` (protocolo: largo, JSON roto, mensaje enorme rechazado sin
  leerlo, respuesta de más de 1 MB; traducción de las 12 acciones; una vuelta con un cartero de
  mentira; el exe de verdad por tuberías en modo en seco, midiendo).
- App: `cargo test -p pixpin --bins -- pedidos::`.
- JS: `node --test extensiones/navegador/pruebas/nucleo.test.mjs` (menú = tabla del puente,
  mensajes, trozos de la captura, base64, manifiesto y su id; y, si `pixpin-puente.exe` está
  compilado, cada acción por la tubería al exe real en modo en seco).
- El modo en seco: con la variable `PIXPIN_PUENTE_EN_SECO=<carpeta>`, el puente usa esa carpeta
  como raíz, **no busca PixPin** y devuelve el pedido en la respuesta. Solo para pruebas.

## Lo que no hace (todavía)

- No está en las tiendas de Chrome ni de Edge: se carga sin empaquetar.
- Brave, Vivaldi y otros Chromium leen sus propias claves del registro; no se registran.
- La captura entera no fuerza la carga de imágenes perezosas que aún no se han visto.
