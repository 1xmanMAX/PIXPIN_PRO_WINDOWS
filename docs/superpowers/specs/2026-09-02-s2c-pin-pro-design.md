# S2-C · Pin pro: vídeo y documentos

**Fecha:** 2026-09-02 · **Estado:** aprobado (decisiones auto-aprobadas bajo la
autorización del usuario del 2026-09-01) · **Documento padre:**
[`2026-09-01-s2-pines-almacen-design.md`](2026-09-01-s2-pines-almacen-design.md)
(cuyo §11 dejaba «vídeo con reproducción» fuera de S2 a propósito: entra aquí).

De lo que el plan maestro asignó a S2-C, dos cosas ya están hechas: la **rueda
como zoom** (S3-B, corregida su escala en S3-C) y la **lupa y el foco en el
pin** (S3-C). Quedan los dos tipos de pin que faltan del «pines de todo tipo»
del usuario: **vídeos** y **documentos**.

## 1. Lo que pidió el usuario, literal

> «Pines de todo tipo: imágenes, vídeos, texto, archivos, carpetas y documentos»

Imágenes, texto (notas), archivos y carpetas (fichas) existen desde S2-B. Un
vídeo pineado hoy es una ficha con icono; un PDF, igual. Un vídeo tiene que
**verse moviéndose** y un documento tiene que **verse**, no ser un icono.

## 2. Decisiones

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D62 | **Qué es un «documento» en S2-C** | Cualquier archivo del que la Shell de Windows dé una **miniatura** (`IShellItemImageFactory`): PDF, Office, imágenes raras, incluso vídeos si falla su reproducción. El pin enseña la miniatura grande con el nombre debajo. Sin miniatura, ficha como hasta ahora | Es la vista previa que ya tiene el Explorador, para todos los formatos que Windows conozca, sin escribir un lector por formato. El **lector de PDF de verdad** (páginas, anotar, editar) es S6 y no se adelanta |
| D63 | **Vídeo** | Reproducción real con **Media Foundation `IMFMediaEngine`** en modo *frame server* sobre el dispositivo D3D11 compartido: cada fotograma se transfiere a una textura y el pin la pinta como pinta una imagen. **En bucle y silenciado** por defecto | Es el reproductor del sistema: códecs por hardware, todos los contenedores que Windows sepa abrir (MP4, MKV, WebM, MOV, TS), cero dependencias nuevas. Un pin flotante que arranca con sonido es una agresión; el sonido se pide |
| D64 | **Dónde vive el reproductor** | Módulo `pixpin-pin::video` (L2, `unsafe` auditado), sin crate nuevo | El pin es su único consumidor. `pixpin-record` (S5) es grabar, otra cosa. Un crate de una sola función y un solo cliente es ceremonia |
| D65 | **El almacén no cambia** | Vídeos y documentos son `TipoEntrada::Archivo` por referencia (D28), como hoy. La **presentación** (ficha, documento o vídeo) la decide el gestor al crear la ventana: por extensión para el vídeo, por si la Shell da miniatura para el documento | Cero migración del índice. El almacén guarda «este archivo»; cómo se enseña es cosa de la vista, y puede mejorar sin tocar datos |
| D66 | **Dispositivo con soporte de vídeo** | `Dispositivo::nuevo` añade `D3D11_CREATE_DEVICE_VIDEO_SUPPORT` y protección multihilo; si el driver lo rechaza, se reintenta sin el flag y los vídeos caen a documento (miniatura) | Media Foundation exige las dos cosas para compartir el dispositivo. El reintento es la red de seguridad del suelo (HD 4000 lo soporta, pero un driver raro no puede dejar sin capturas) |
| D67 | **Ritmo de repintado** | Un temporizador en la ventana del pin (`SetTimer`, 16 ms en `Completo`, 33 ms en `Ligero`) pregunta `OnVideoStreamTick`; solo si hay fotograma nuevo se transfiere y se repinta. En pausa el temporizador se para | Es el mecanismo canónico del *frame server*, y el nivel se aplica igual que en el modo vivo del overlay (D14) |
| D68 | **Controles** | Doble clic y `Espacio` (pin enfocado): reproducir/pausar. Menú del clic derecho: **Reproducir/Pausar** y **Sonido** (alterna silencio). Sin barra de progreso ni cromo (D23) | Lo mínimo para usarlo como pin: verlo, pararlo, oírlo si hace falta. Buscar en el vídeo es cosa de un reproductor, no de un pin |
| D69 | **Restauración** | Un vídeo vuelve reproduciéndose, en bucle y **siempre silenciado**; un documento vuelve con su miniatura recién pedida (no se guarda) | Arrancar Windows y que suene un pin es inaceptable; la miniatura es barata y así siempre refleja el archivo actual |
| D70 | **Anotar sobre vídeo y documento** | No en S2-C: son referencias sin objeto propio en el almacén, y `ruta_anotacion` ya devuelve `None` para ellas. El doble clic en un vídeo es reproducir/pausar; en un documento, abrir con la app predeterminada (como la ficha) | Anotar una referencia exige decidir dónde vive el `.pixpin2d` de un archivo ajeno. Es una decisión de S6 (PDF), no de aquí |
| D71 | **Tamaños** | El vídeo nace con su tamaño nativo (o al 80 % del área de trabajo si no cabe) y se redimensiona proporcional como una imagen. El documento pide la miniatura a **1024 px** y nace con lo que la Shell devuelva (normalmente 256–1024), proporcional; el nombre va en una franja de 28 px lógicos debajo | Un vídeo es una imagen en movimiento y se comporta igual. La Shell decide el máximo que sabe extraer |
| D72 | **Fallos** | Vídeo que Media Foundation no puede abrir (códec ausente, archivo roto) → documento (miniatura), y se registra. Sin miniatura → ficha. Archivo desaparecido → ficha «no encontrado» (D28) | La degradación es siempre hacia algo que ya funciona; nunca un pin vacío ni un error modal |

## 3. Arquitectura

```
pixpin-pin (L2)
  contenido.rs   Contenido::Video { nombre, ruta, ancho, alto }
                 Contenido::Documento { nombre, vista: ImagenRgba }
                 presentacion_de(ruta) -> Presentacion   (puro, probado)
  icono.rs       miniatura_de(ruta, lado) -> Option<ImagenRgba>   (IShellItemImageFactory)
  video.rs       Reproductor::nuevo(d3d, ruta) / tick() -> Option<&ID3D11Texture2D>
                 / pausar / reanudar / silenciar / dimensiones      (IMFMediaEngine)
  ventana.rs     PinInterno.video: Option<Reproductor>; temporizador; pintado del fotograma
  menu.rs        CMD_REPRODUCIR, CMD_SONIDO; TextosPin.reproducir / pausar / sonido
pixpin-capture (L2)
  dispositivo.rs VIDEO_SUPPORT + multihilo, con reintento sin el flag (D66)
apps/pixpin
  pines.rs       al pinear o restaurar un Archivo: presentacion_de → ficha / documento / vídeo
  main.rs        textos nuevos del catálogo; el intervalo del temporizador según el nivel
pixpin-store     i18n: pin-reproducir, pin-pausar, pin-sonido
```

**Flujo del vídeo:** `Pin::nuevo` con `Contenido::Video` crea el `Reproductor`
sobre el `ID3D11Device` compartido, arranca el temporizador y pone
`autoplay`+`loop`+`muted`. En cada `WM_TIMER`, `tick()` pregunta al motor si hay
fotograma; si lo hay, `TransferVideoFrame` a una textura propia del pin
(BGRA, tamaño del vídeo) y se repinta con `bitmap_desde_textura`. El bitmap
D2D envuelve la textura sin copiar. La sombra, el grupo, el imán, el menú y las
anotaciones (ninguna, D70) no cambian.

**Flujo del documento:** `miniatura_de(ruta, 1024)` en el momento de crear la
ventana (pinear o restaurar). Es una llamada a la Shell que puede tardar
(un PDF grande: cientos de ms); se hace **en el hilo de interfaz** en S2-C y se
mide. Si la puerta de §5 no se cumple, la siguiente iteración la lleva a un
hilo y el pin nace con el icono hasta que llega la miniatura.

**`presentacion_de(ruta)`** es pura: extensiones de vídeo (`mp4 mkv webm mov
avi m4v ts wmv`) → `Video`; el resto → `Documento` si hay miniatura, `Ficha` si
no. La comprobación de miniatura no es pura y la hace el llamador; la función
devuelve `DocumentoSiHayMiniatura` y el gestor resuelve.

## 4. Interacción

| Gesto | Vídeo | Documento |
|---|---|---|
| Arrastrar | mover (como todos) | mover |
| Esquinas | redimensionar proporcional | redimensionar proporcional |
| Rueda | zoom (D55) | zoom |
| Doble clic | reproducir / pausar | abrir con la app predeterminada |
| `Espacio` | reproducir / pausar | — |
| `Ctrl+C` | copiar la ruta como archivo (como la ficha) | igual |
| Menú | Reproducir/Pausar · Sonido · Abrir ubicación · Grupo · Cerrar · Eliminar | Abrir ubicación · Grupo · Cerrar · Eliminar |
| `Esc` | cerrar (D21) | cerrar |

## 5. Rendimiento — puertas de la fase

| Métrica | Objetivo |
|---|---|
| Vídeo 1080p reproduciéndose, `Completo` | ≤ 15 % de un núcleo (decodificación por hardware) |
| Vídeo en pausa, y documento en reposo | 0 % de CPU (temporizador parado) |
| RAM con un vídeo 1080p | ≤ +60 MB sobre la base |
| Pin de documento: pinear → visible | ≤ 300 ms con un PDF de 10 MB (si no, la miniatura pasa a un hilo) |
| Restauración con un vídeo y un documento | < 500 ms, como S2 |

## 6. Pruebas

- **Puras (CI):** `presentacion_de` por extensión (mayúsculas, sin extensión,
  extensiones raras); la geometría de la franja del nombre del documento.
- **Escritorio (`--ignored`):** `miniatura_de` sobre un PNG generado en el test
  (la Shell siempre da miniatura de un PNG) devuelve una imagen de lado ≤ 1024;
  `Reproductor::nuevo` sobre una ruta inexistente devuelve error y no deja
  hilos vivos; el dispositivo con `VIDEO_SUPPORT` se crea.
- **E2E sobre el binario release:** pinear un MKV de `Videos\` por
  `Ctrl+Alt+V` → se ve moviéndose, doble clic lo para y el temporizador se
  detiene (CPU 0), menú Sonido, reiniciar → vuelve en bucle y silenciado;
  pinear un PDF de `Downloads\` → miniatura con nombre, doble clic lo abre.

## 7. Criterios de aceptación

- [x] Un vídeo pineado se reproduce en bucle y silenciado; doble clic lo pausa y reanuda; el menú alterna el sonido (`Espacio` comparte el camino del doble clic; sin verificar por automatización)
- [x] Cualquier archivo con miniatura se pinea como vista previa con su nombre; sin miniatura sigue siendo ficha (verificado con PNG; el equipo de pruebas no tiene proveedor de miniaturas de PDF)
- [x] Vídeo y documento sobreviven al reinicio con las reglas de D69
- [x] Un vídeo que Media Foundation no abre cae a documento o ficha con un aviso en el log, nunca a un pin vacío
- [x] El índice del almacén no cambia de formato
- [x] Las puertas de §5 medidas y anotadas en `medidas/2026-09-02-equipo-desarrollo-s2c.md` (dos no cumplidas del todo: CPU reproduciendo 14–16 % frente a ≤ 15 %, y miniatura en frío 554 ms frente a ≤ 300 ms; las dos con su siguiente paso anotado)

## 8. Fuera de alcance

Barra de progreso o búsqueda en el vídeo · lector de PDF con páginas (S6) ·
anotar sobre vídeo o documento (D70) · arrastrar desde el Explorador ·
subtítulos · pines de ventanas vivas de otras aplicaciones (maestro S3, fase propia).
