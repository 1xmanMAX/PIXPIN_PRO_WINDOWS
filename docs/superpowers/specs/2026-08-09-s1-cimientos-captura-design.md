# S1 — Cimientos + Captura · Especificación de diseño

**Sub-proyecto:** S1 de 6 · **Entrega:** v0.1 · **Estado:** aprobado
**Fecha:** 2026-08-09
**Documento padre:** [`2026-08-09-pixpin-pc-master-design.md`](2026-08-09-pixpin-pc-master-design.md)

**Objetivo:** que capturar sea instantáneo y fiable, con una aplicación que arranca en frío
rápido y no consume nada en reposo. Al terminar S1 la aplicación captura, muestra, copia y
guarda. Nada más.

---

## 0. Decisiones que fija este documento

| # | Decisión | Elección |
|---|---|---|
| S1-1 | Nombre del producto y ejecutable | **PixPin Max**, `pixpinmax.exe` |
| S1-2 | Idiomas de la interfaz en v1 | **Español (`es-ES`) e inglés (`en-US`)** |
| S1-3 | Comportamiento de la pantalla al seleccionar | **Congelada por defecto, con modo en vivo conmutable** |
| S1-4 | Granularidad del ajuste automático | **Ventanas y controles**, vía UI Automation |
| S1-5 | Geometría del overlay | **Una ventana por monitor**, no una única sobre el escritorio virtual |
| S1-6 | Detección del respaldo de captura | **Por capacidad en tiempo de ejecución**, no por número de compilación |

La S1-6 **refina la D10 del documento maestro**: allí se fijó la compilación 20348 como frontera
para el borde amarillo de WGC. Codificar ese número es frágil. Se consulta si la propiedad existe
(`ApiInformation.IsPropertyPresent`) en lugar de comparar versiones de Windows. Detectar
capacidades en vez de versiones sobrevive a los cambios de Microsoft. El mínimo soportado sigue
siendo Windows 10 21H2.

---

## 1. Modelo de proceso e hilos

Un solo proceso, cuatro papeles bien separados.

| Hilo | Responsabilidad | Por qué separado |
|---|---|---|
| **Interfaz** | Bombeo de mensajes Win32; **posee todas las `HWND`** | Win32 lo exige: una ventana sólo se toca desde el hilo que la creó |
| **Captura** | Sesión WGC, recepción de frames en textura D3D11 | WGC entrega en un hilo del pool; se canaliza aquí para no ensuciar el hilo de interfaz |
| **UI Automation** | Árbol de controles bajo el cursor, con caché | Las consultas UIA son COM entre procesos y pueden tardar decenas de milisegundos o colgarse si la aplicación destino no responde |
| **Pool (`rayon`)** | Codificación PNG/JPEG/WebP, cosido de scroll, escritura a disco | Trabajo pesado que no debe rozar la interacción |

**Regla innegociable: el overlay nunca espera a nadie.** Pregunta «¿qué control hay bajo el
cursor?» y recibe de inmediato lo último que se sepa. Si UIA todavía no ha contestado, usa el
rectángulo de la ventana y se refina solo cuando llegue la respuesta. Una aplicación de captura
que se congela medio segundo porque la aplicación destino tardó en responder a UIA es una
aplicación rota, y es un fallo muy común en la competencia.

---

## 2. El overlay de selección

### 2.1 Una ventana por monitor

Con DPI mixto —hoy lo normal: portátil al 150% más monitor externo al 100%— una ventana única
que cubra el escritorio virtual obliga al sistema a escalarla desde un solo DPI, y el resultado
es lupa borrosa y bordes desalineados en todos los monitores menos uno.

Una ventana por monitor recibe su propio `WM_DPICHANGED` y dibuja a escala nativa, así que **la
lupa enseña píxeles reales, no interpolados**. El precio es coordinar el arrastre cuando cruza de
un monitor a otro: un problema pequeño y acotado, a cambio de corrección en el caso que más se usa.

Cada ventana: `WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP`, compuesta por DirectComposition, del
tamaño exacto de su monitor.

### 2.2 Secuencia de arranque del overlay

El orden no es negociable:

```
  pulsacion del atajo
        |
        v
  instantanea de TODOS los monitores (WGC, se queda en GPU)
        |
        v
  crear una ventana overlay por monitor, ya con su textura
        |
        v
  mostrar          <-- objetivo: menos de 50 ms desde la pulsacion
```

La instantánea se toma **antes** de que exista el overlay. Si se hiciera después, la captura se
incluiría a sí misma.

### 2.3 Elementos e interacción

- Oscurecido fuera de la selección, imagen nítida dentro.
- **Lupa** siguiendo el cursor: zoom 8×, retícula de píxel, y el color exacto bajo el cursor en
  HEX/RGB. Esta pieza **es también el cuentagotas global** del catálogo: mismo código, dos
  funciones cubiertas.
- Barra con dimensiones y coordenadas en vivo.
- Ocho tiradores de redimensión.

| Tecla | Acción |
|---|---|
| Flechas | Mover un píxel |
| `Shift` + flechas | Mover diez píxeles |
| `Espacio` | Alternar entre pantalla congelada y en vivo |
| `Esc` | Cancelar |
| `Enter` | Confirmar |

### 2.4 Modo en vivo

Congelado es el comportamiento por defecto, porque permite capturar menús y tooltips desplegados
(que se cierran al pulsar), evita que la lupa parpadee con animaciones de fondo, y evita que el
contenido se mueva mientras se ajusta el borde.

`Espacio` reanuda la sesión WGC y el overlay pasa a mostrar frames en vivo, para los casos en que
hay que esperar a que algo cargue o capturar un instante concreto de una animación. Volver a
pulsar congela de nuevo en el frame actual.

### 2.5 El ajuste automático

El hilo de UIA mantiene una caché del árbol de la ventana bajo el cursor y la invalida al cambiar
de ventana. Cuando UIA no está disponible o tarda, se cae al rectángulo de la ventana.

**Ese rectángulo se obtiene con `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)`, nunca con
`GetWindowRect`.** `GetWindowRect` devuelve un rectángulo que incluye la sombra invisible de la
ventana, así que el recuadro sale varios píxeles más grande por cada lado y la captura arrastra un
marco de escritorio no deseado. Es un fallo que se ve en muchas herramientas de captura.

---

## 3. La tubería de captura

**Principio: la imagen no baja a la CPU hasta que el usuario guarda o copia.**

```
  GraphicsCaptureItem  (un monitor, o una HWND concreta)
        |
        v
  Direct3D11CaptureFramePool   B8G8R8A8, 2 buffers
        |
        v
  ID3D11Texture2D  <-- la imagen vive AQUI, en memoria de video
        |
        +--> recorte por region:  CopySubresourceRegion, en GPU
        +--> mostrar en overlay:  bitmap D2D sobre la misma textura
        +--> mosaico / blur / foco:  shader HLSL sobre la misma textura   (S2)
        |
        v
  solo al guardar o copiar:  Map a CPU  ->  codec  ->  disco
```

Una captura 4K son 33 MB. Bajarla a memoria de sistema en cada operación es lo que hace lentas y
glotonas a las herramientas de captura corrientes. Aquí sólo se cruza esa frontera una vez. De ahí
salen los objetivos de mosaico en <5 ms y RAM en reposo bajo el presupuesto.

**Respaldo.** Si la propiedad que suprime el borde de WGC no existe en el sistema (ver S1-6), la
captura de ventana usa DXGI Desktop Duplication y recorta al rectángulo de la ventana.

---

## 4. Captura con scroll

```
  capturar frame  ->  enviar rueda a la ventana destino  ->  esperar a que asiente
        ^                                                          |
        |                                                          v
        +---------  coser  <-  correlacionar solapamiento  <-  capturar frame
```

**La correlación y el cosido son lógica pura**, sin pantalla ni GPU: viven en L0 y son
directamente testeables. Es el puerto de `ScrollMatcher` y `ScrollStitcher` del Android.

Tres problemas que se resuelven en el diseño, no al descubrirlos:

1. **Cabeceras y pies fijos.** Una barra superior que no se mueve al hacer scroll se repite en
   cada frame y el cosido la duplicaría indefinidamente. Se detecta comparando qué franjas quedan
   idénticas entre frames consecutivos, y se excluyen de la costura tomándolas una sola vez.
2. **Scroll suave animado.** Capturar antes de que la animación termine da un frame a medias. Se
   espera a que dos capturas seguidas sean idénticas antes de dar el paso por bueno.
3. **Cuándo parar.** Se para cuando varios frames seguidos no aportan contenido nuevo, cuando se
   alcanza un límite de altura configurable, o cuando el usuario pulsa `Esc`. Sin esto, una página
   infinita captura hasta agotar la memoria.

---

## 5. Cimientos

| Pieza | Decisión |
|---|---|
| **Ajustes** | TOML. **La existencia de `pixpinmax.toml` junto al `.exe` decide el modo portable**, sin banderas ni instaladores distintos. Si no está, se usa `%APPDATA%\PixPinMax\`. |
| **Atajos globales** | `RegisterHotKey`, todos reasignables. Ver 5.1. |
| **Bandeja** | Icono con menú: capturar, ajustes, salir. |
| **Instancia única** | Mutex con nombre; la segunda instancia envía el mensaje a la primera y termina. Evita dos aplicaciones peleando por el mismo atajo global. |
| **Idiomas** | Fluent, `es-ES` y `en-US`, detectado del sistema y forzable en ajustes. |
| **Registro** | `tracing` a fichero rotativo local y volcado de fallos, equivalente a `CrashLog.kt` del Android. **Nada sale del equipo.** |
| **Arranque con Windows** | Opcional, desactivado por defecto. |

### 5.1 Atajos por defecto y qué hace cada uno

En la v0.1 todavía no existe editor, así que hay que distinguir explícitamente qué ocurre al
confirmar la selección; si no, dos atajos harían lo mismo.

| Atajo | Acción | Al confirmar |
|---|---|---|
| `Ctrl+Alt+X` | Capturar región | Aparece una **barra de resultado** junto a la selección: copiar, guardar como, guardar en la carpeta por defecto, descartar |
| `Ctrl+Alt+C` | Capturar y copiar | Va **directo al portapapeles** y el overlay desaparece, sin barra ni confirmación |
| `Ctrl+Alt+S` | Captura con scroll | Igual que `Ctrl+Alt+X`, pero tras completar el recorrido y coser |
| `Ctrl+Alt+D` | Cuentagotas | Copia el color bajo el cursor en el formato configurado y cierra |

La barra de resultado es la que en S2 crecerá hasta ser la entrada al editor, y en S3 ganará el
botón de pinear. En S1 se diseña ya con ese crecimiento en mente, pero sólo se implementan las
cuatro acciones de arriba.

---

## 6. Estrategia de pruebas

### 6.1 Lo que se prueba en CI, sin Windows ni GPU

Todo lo que vive en L0, en milisegundos:

- Geometría de selección y redimensión
- Resolución del ajuste automático dado un árbol de rectángulos
- Correlación y cosido de scroll
- Detección de cabeceras y pies fijos
- Lectura y escritura de ajustes TOML
- Resolución de idiomas

**El truco que hace testeable lo que parece intestable:** se graba una vez una secuencia real de
scroll como PNGs y se versiona como fixture. A partir de ahí el test del cosido es una función
pura de entrada a salida, determinista, y corre sin tocar una pantalla. Lo mismo con el ajuste
automático: se versiona un volcado del árbol UIA de una ventana real y se prueba la resolución
contra él.

### 6.2 Lo que no se puede probar en CI

WGC necesita GPU y sesión de escritorio reales, y los runners de CI no las tienen. Esas partes van
marcadas como pruebas de integración que se ejecutan en la máquina de desarrollo, más una
comprobación de humo en un runner de Windows para asegurar que compila y arranca.

Se documenta explícitamente porque prometer otra cosa sería mentir sobre el alcance de la red de
seguridad.

---

## 7. Criterios de aceptación de la v0.1

### 7.1 Funcionales

- [ ] Capturar región, ventana y monitor
- [ ] Ajuste automático a ventanas y a controles
- [ ] Congelar y alternar a modo en vivo
- [ ] Lupa con zoom 8× y cuentagotas con HEX/RGB
- [ ] Captura con scroll, con cabeceras fijas resueltas y parada automática
- [ ] Barra de resultado con copiar, guardar como, guardar en carpeta y descartar
- [ ] Copiar al portapapeles y guardar a PNG/JPG/WebP
- [ ] Funcionar con tres monitores de escalado distinto
- [ ] Modo portable sin tocar el registro
- [ ] Interfaz en español e inglés

### 7.2 De rendimiento — medidos, no estimados

| Métrica | Objetivo |
|---|---|
| Arranque en frío hasta bandeja | < 300 ms |
| Atajo global → overlay visible | < 50 ms |
| CPU en reposo | 0% |
| RAM en reposo | < 40 MB |
| Tamaño del binario | < 30 MB |

Los tres restantes del presupuesto maestro (latencia de trazo, RAM con diez pines, mosaico 4K)
corresponden a S2 y S3, y no se miden aquí.

---

## 8. Fuera de alcance

Ni anotación, ni pines, ni PDF, ni OCR, ni grabación, ni automatización, ni visor de formatos
avanzados. La v0.1 captura, muestra, copia y guarda.

El único crate de L0 que S1 empieza a llenar es `pixpin-geom`, y sólo con lo que la selección y el
scroll necesitan. `pixpin-model` y el formato Excalidraw entran en S2.
