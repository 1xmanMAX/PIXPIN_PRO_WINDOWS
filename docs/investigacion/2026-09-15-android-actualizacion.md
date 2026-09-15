# PixPin Android v0.50.1: los tres códigos y qué exige a Windows

Fecha: 15-sep-2026. Repositorio `1xmanMAX/PIXPIN_PRO_ANDROID`, commit `05722d9`
(«v0.50.1: tres códigos, sincronizar fusionando, modelos IFC/OBJ, presentar e imprimir, copias de
seguridad»). La base anterior analizada para Windows es del 6-sep (`docs/investigacion/2026-09-06-android-*`).
Rutas de Android relativas a `app/src/main/java/com/forge/pixpin/`.

## Qué trae la versión (del mensaje del commit)

- Tres códigos por cosa (único, número·aparato, fecha); compartir con «Actualizar / Crear como nuevo».
- Sincronizar «como git y sin maestro»: fusión por figura, celda, párrafo y trazo; solo viajan los cambios.
- Copias de seguridad antes de recibir y sincronizar; recibir nunca quita lienzos; registro del chat.
- Modelos IFC (Revit) y OBJ en el croquis 3D.
- Presentar, imprimir y `.pptx` a PDF; modo visualización del lienzo.
- El chat viaja en el `.pixpin`; escaneos a 300 ppp; fusionar páginas.

## Los tres códigos (`sincro/Codigos.kt`)

| Código | Campo | Cómo nace | Dónde |
|---|---|---|---|
| Único | `uid` (10 signos) | Al azar con `SecureRandom` sobre `SIGNOS` (`Codigos.kt:37-38`); lo guardado antes, `Codigos.de("m:"/"h:"/"p:" + id)` = los 10 primeros bytes de SHA-256 módulo 31 (`Codigos.kt:41-44`, `:61-65`) | `Mensaje.uid` (`guardados/Mensajes.kt:251`), `Hoja.uid` (`motor/Proyectos.kt:241`), `Proyecto.uid` (`motor/Proyectos.kt:122`) |
| De chat | `numero` + `aparato` → `47·K7Q2`; antes `47a` con `letra` | `Codigos.deChat` (`Codigos.kt:51-56`) | `Mensaje.numero`, `.letra`, `.aparato` (`Mensajes.kt:234,244,258`) |
| Fecha | `Mensaje.cuando`; en proyecto `creado` + `aparato` | Hora de creación en el aparato donde nació | `Proyecto.creado`, `.aparato` (`Proyectos.kt:124,126`) |

- Alfabeto: `SIGNOS = "23456789ABCDEFGHJKMNPQRSTUVWXYZ"` (31, sin 0/O/1/I/L) (`sincro/Identidad.kt:65`).
- Código del aparato: 4 signos de SHA-256 de su id (`Identidad.kt:37-41`).
- **Misma cosa** solo si coinciden los tres (`Codigos.kt:71-77`); si no, se crea aparte («duplicar y no pisar»).
- `sellar` pone los que falten sin tocar los que hay (`Codigos.kt:83-96`); copiar a propósito los renueva (`Codigos.kt:102`).

## Hueco que rompía la compatibilidad — resuelto (fase A)

Windows (`pixpin-proyecto`) no tenía `uid`/`creado`/`aparato` ni los campos del mensaje, y
**al volver a guardar un `.pixpin` los perdía** (`Paquete::guardar` reescribe `proyecto.json`).
Android lo recibiría sin sus códigos, no lo reconocería como la misma cosa y lo duplicaría.

Hecho el 15-sep:
- `pixpin-proyecto::codigos`: SHA-256 propio (vectores NIST), `de`, `nuevo`, `de_aparato`,
  `de_chat`, `unico`, con valores cruzados calculados con el algoritmo de Android.
- `Proyecto` (`uid`, `creado`, `aparato`, `mismo_que`, `sellar`), `Hoja` (`uid`), `Mensaje`
  (`numero`, `letra`, `uid`, `aparato`, `origen`, `codigo_chat`, `mismo_que`).
- `Proyecto`, `Hoja`, `Mensaje` y `Manifiesto` guardan **todo campo desconocido** (`resto`,
  `serde(flatten)`): un `proyecto.json` de Android sale idéntico tras leerlo y escribirlo.

## Fases siguientes

- **B · Identidad de este equipo.** Un id de aparato persistente en el almacén y su código de 4
  signos; sellar al crear y al guardar lo que nace en Windows (`creado` y `aparato` incluidos).
- **C · Recibir «Actualizar / Crear como nuevo».** Al abrir un `.pixpin` que ya existe: si
  `mismo_que`, ofrecer poner al día; si no, crear aparte con códigos nuevos; copia de seguridad antes.
- **D · El chat dentro del `.pixpin`.** Leer y escribir el chat que ahora viaja en el paquete
  (`sincro/ChatQueViaja.kt`, 168 líneas) con el cuaderno de Windows.
- **E · Sincronización en red local.** `sincro/Protocolo.kt` (845 líneas), `Red.kt`, `Presencia.kt`,
  `Fusion.kt`, `Diferencia.kt`, `Mezcla.kt`: descubrimiento `_pixpin._tcp`, TCP cifrado con la
  clave del grupo, fusión por figura, celda, párrafo y trazo. Es la fase grande: pide su propio
  análisis del protocolo antes de escribir nada, para interoperar byte a byte.
- **F · Lo demás de la versión.** IFC/OBJ en croquis 3D, presentar/imprimir, fusionar páginas:
  sin impacto en el formato de intercambio salvo que se guarden en el paquete (a verificar).
