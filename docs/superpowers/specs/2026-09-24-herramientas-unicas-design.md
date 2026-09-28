# Herramientas de dibujo unicas (2026-09-24)

## Lo que pidio el usuario

> «que la tinta del visor de epub/docx/pdf sea la misma del editor avanzado
> del canvas, que tenga las mismas herramientas y todo, pero que se puedan
> activar y desactivar algunas herramientas dentro de la configuracion
> general de la app; estas herramientas de dibujo tienen que ser usadas en
> todo lugar que necesite estas herramientas […]; ademas implementa una
> funcion para poder anotar en pantalla, como un anotador de pantalla».

El QUE esta decidido. Este documento ordena el COMO.

## Punto de partida (medido leyendo el codigo)

Hoy hay **cuatro** maneras de dibujar, y solo una es completa:

| Anfitrion | Que usa | Herramientas |
|---|---|---|
| Lienzo (`ventana_editor.rs`, 6.300 lineas) | `Gesto` + `Escena` del motor, caja `BOTONES_EDITOR`, panel lateral, capa de tinta DComp, horneado, zonas | las 28 |
| Lector (`lector_tinta.rs` + `visor.rs`/`lector_pdf.rs`) | maquina propia (`Capa::pulsar/mover/soltar`) | lapiz, resaltador, goma, 5 colores |
| Pin (`pines.rs` + `pixpin-ui::Anotador` + `pixpin-pin::Paleta`) | maquina vieja `Anotador` | las 11 de `BOTONES` |
| Pantalla (`capa.rs`) | la misma maquina vieja `Anotador` | las 11 de `BOTONES` |

El motor (`pixpin-motor2d::gesto::Gesto`) ya sabe hacer TODAS las
herramientas; lo que no es comun es **lo que hay alrededor**: la lectura de
teclas, la caja, el panel, el lazo, el cuentagotas, la goma, el
portapapeles, la forma rapida, el suavizado y la traduccion de `Orden` a
Direct2D. Todo eso vive hoy dentro de `abrir_en_modo`, una funcion de 2.400
lineas del editor.

## Decision: un nucleo `dibujo`, y el editor como anfitrion de la pantalla

### El nucleo (`apps/pixpin/src/dibujo/`)

Se **extrae** del editor (se mueve, no se reescribe) lo que no depende de su
ventana:

- `dibujo/permitidas.rs` — **las herramientas activas** segun los ajustes
  (`[herramientas]` del TOML). Guarda una copia global que `main` fija al
  arrancar y la ventana de ajustes al guardar, y da a cada anfitrion su caja
  (`botones(base)`: la lista base menos lo apagado y menos lo que ese
  anfitrion no sabe hacer) y su tabla de letras (`herramienta_de_letra`,
  que devuelve `None` para una herramienta apagada). **Una sola puerta**:
  la herramienta desactivada no aparece en la barra ni responde a su letra
  ni a su atajo del motor (`S` lazo, `K` cuentagotas) en ningun anfitrion.
- `dibujo/teclas.rs` — lo que era `tecla_a_herramienta`, `tecla_de`,
  `tecla_a_pluma`, `tecla_del_motor`, `aplicar_orden`, `volteando`,
  `elegir_herramienta`, `pulsar_boton`, `a_evento`, `con_modificadores`,
  `forma_de`: puras, con sus pruebas.
- `dibujo/mano.rs` — **`Mano`**: el estado de la mano que no es del motor
  (portapapeles interno, goma pulsada, pausa de la forma rapida,
  predictor, filtro de 1 euro) y la entrada: `interfaz(ev)` (clic en el
  panel, en la caja, teclas mientras se escribe un texto) y
  `herramienta(ev)` (atajos del motor, letras, pluma, Ctrl+C/X/V/D/G/L,
  lazo, cuentagotas, goma y, al final, `Gesto::evento` + `construir`).
  Devuelve **que cambio** (`Atendido`), no pinta: cada anfitrion decide
  como repintar. Asi el editor conserva intactas sus optimizaciones
  (horneado, `Compuesto::Zona`, `repintar_zona`, capa de tinta DComp) y el
  lector conserva su pasada unica.
- `dibujo/pintar.rs` — la traduccion de `Orden` a `Pintor` (`dibujar_orden`,
  `por_cada_orden`, grano, grafito, punta predicha) y, para anfitriones sin
  capas, `pintar_escena` + `pintar_encima` (marco, tiradores, marquesina,
  lazo, iman) — las mismas llamadas que usa `pintar` del editor.
- `dibujo/construir.rs` — el antiguo `ventana_editor/construir.rs` (lo
  usan el gesto y el pintado de angulos: es del nucleo).
- `dibujo/tema.rs` — **la tinta sobre papel oscuro**: puerto de
  `DrawTheme.adaptar`/`esDeNoche` (`motor/Theme.kt`). Se aplica al pintar,
  nunca a los datos, y SOLO si el papel es de noche (Pizarra, Cosmos, Azul
  noche, Verde pizarra, Negro, y el papel oscuro del lector de Word):
  sobre papel claro se pinta como hasta hoy, para no cambiar dibujos ya
  hechos. El papel se fija por fotograma (`tema::con_papel`); sin fijarlo,
  nada cambia (las puertas de rendimiento no lo notan).

Los anfitriones solo aportan: **fondo**, **transformada documento→pantalla**
(`Camara`), **que herramientas admite** y **donde se guarda**.

### Los anfitriones

1. **Lienzo** (editor): igual que hoy, llamando al nucleo. Nada de su
   presentacion cambia.
2. **Lector** (Word/EPUB/HTML en `visor.rs`, PDF en `lector_pdf.rs`): cada
   capa (`lector_tinta::Capa`, una por documento u hoja) lleva ahora su
   `Gesto` y una `Mano` compartida; la barra propia se sustituye por la caja
   del editor (arriba, al centro, con «Salir» = «Listo») y el panel lateral
   de propiedades. **Mismo fichero, mismo sitio**: `Capa::leer`, `guardar`,
   `ruta_de_capa`, `ruta_de_hoja`, `carpeta_de` no cambian de firma (los
   usa `compartir.rs`); el resaltador se sigue escribiendo con opacidad 35
   y grosor de renglon (16) para que la version anterior lo lea igual. La
   tinta se sigue pintando en la MISMA pasada que el texto o la hoja, con la
   misma transformada: no puede temblar al desplazar ni al acercar.
   Admite todo menos lo que no tiene sentido sobre un documento: lupa
   (necesita la pantalla), escalar (su cajetin es del editor), mosaico (su
   pasada lee los pixeles de la escena) y emoji (universo).
3. **Pin** (anotar en el sitio, doble clic): el `Anotador` viejo se
   sustituye por `Gesto` + `Mano`; la paleta junto al pin es la caja del
   nucleo (filtrada). Sigue guardando en `.pixpin2d` (el formato no cambia:
   migrar a `.excalidraw` no es trivial con los pines del movil de por
   medio). El panel de propiedades completo queda en «Abrir en el lienzo»
   (que ya abre el editor sobre la imagen del pin).
4. **Pantalla** (nuevo anotador): **es el propio editor** con un anfitrion
   `Pantalla { modo }`: ventana a todo el escritorio virtual (todos los
   monitores; coordenadas fisicas, la camara fija a 1:1 aunque el DPI sea
   mixto), sin navegar (ni rueda ni espacio+arrastre), sin marcas ni F11.
   - **Viva**: el papel es transparente (la swapchain ya es
     premultiplicada); **Espacio** alterna pasante y **Ctrl** mantenido lo
     es mientras dure, como `capa.rs`.
   - **Congelada**: el fondo es la foto del escritorio virtual tomada ANTES
     de abrir la ventana.
   - **Esc** sale (si no hay un texto abierto: entonces lo cierra, como en
     el lienzo). Al salir con dibujo se hace lo de hoy: foto de la pantalla
     con lo anotado sin interfaz, y `main` pregunta si guardarlo y lo pinea
     (D54). Asi Esc nunca pierde lo dibujado sin avisar.
   - Tiene la superficie completa: capa de tinta DComp, horneado, panel.
   - Se abre desde la **bandeja** (`anotar`, `anotar-congelada` pasan a
     `en_bandeja = true`); **sin atajo de fabrica** (preferencia firme del
     usuario): el atajo sigue siendo opcional en el TOML. `capa.rs` queda
     solo como envoltorio de `ejecutar_capa` que ahora llama al editor.

## Ajustes

`[herramientas]` en `pixpin-store::Ajustes`: una tabla `apagadas = [...]`
con los nombres estables de las herramientas (`"lazo"`, `"grafito"`…). Por
defecto vacia (todas activas). Se elige lista de apagadas y no una casilla
por campo porque una herramienta nueva nace activa sin tocar ficheros
viejos, y un nombre desconocido se ignora. La mano, deshacer, rehacer y
salir no se pueden apagar (sin la mano no se puede ni elegir lo dibujado).

`ventana_ajustes.rs`: seccion «Herramientas de dibujo» con una casilla por
herramienta; al guardar se escribe el TOML y se refresca la copia global,
asi un lector o un lienzo que se abra despues ya sale sin ella. Textos en
los dos `.ftl` (`herramientas-*`).

## Pendientes del editor

- **Papel oscuro**: `dibujo::tema` (arriba).
- **Horneado sobre marcas (F5)**: `hornear_trazo` pintaba el trazo encima
  del redondel de una marca hasta el siguiente fotograma entero. Repintar
  la marca encima la volveria mas opaca (se pinta al 82 %); lo exacto es
  **no hornear** si la zona del trazo toca una marca y dejar que vaya el
  fotograma entero, que ya la pinta en su sitio. Solo cuesta un fotograma
  entero al rozar una marca.

## Medir

- Lector: banco sin ventana (`FueraDePantalla`) que pinta un fotograma del
  lector con documento + capa de N trazos + trazo en curso, como mide
  `ventana_editor/medir.rs` el del editor. Puerta: cabe en el
  presupuesto de un fotograma a 60 Hz en release.
- Pantalla: es el editor, asi que sus puertas son las del editor
  (`medir.rs`, `puertas.rs`, `pixpin-render/tests`). Se anade el banco de
  la tinta viva a tamano de dos monitores (3840x1080) y la cuenta de
  memoria de las superficies del anotador en dos monitores.

## Pruebas (nombres de frase, con casos negativos)

- Una herramienta apagada no esta en la caja ni responde a su letra ni a su
  atajo del motor, en cada anfitrion (lienzo, lector, pin, pantalla).
- Lo anotado en el lector se guarda en el mismo fichero que antes y lo lee
  la `Capa::leer` de siempre (formas y flechas incluidas); el resaltador
  sigue escribiendose al 35 %.
- La tinta negra sobre papel de pizarra sale clara; sobre papel blanco no
  cambia; un color que ya se lee no cambia.
- El horneado no se hace si el trazo toca una marca.
- Anotador de pantalla: sin dibujo, Esc cierra sin foto; con dibujo, la
  foto se entrega a quien pregunta (no se tira).
- Los ajustes por defecto tienen todas activas; un nombre desconocido se
  ignora; el TOML va y vuelve.

## Estado al cerrar la tanda (2026-09-24)

Hecho tal como arriba. Donde vive cada cosa:

| Pieza | Fichero |
|---|---|
| Que sale en cada anfitrion | `apps/pixpin/src/dibujo/permitidas.rs` |
| Teclas y botones (movido del editor) | `apps/pixpin/src/dibujo/teclas.rs` |
| La mano (entrada comun) | `apps/pixpin/src/dibujo/mano.rs` |
| Orden → Direct2D, escena entera, lo de encima | `apps/pixpin/src/dibujo/pintar.rs` |
| Tinta sobre papel de noche | `apps/pixpin/src/dibujo/tema.rs` |
| Las cuatro de construir (movido) | `apps/pixpin/src/dibujo/construir.rs` |
| Lector: anfitrion `Tinta` | `apps/pixpin/src/lector_tinta.rs` (+ `visor.rs`, `lector_pdf.rs`) |
| Pin: anotar en el sitio | `apps/pixpin/src/pines.rs` (`Anotacion`) |
| Anotador de pantalla | `apps/pixpin/src/ventana_editor/pantalla.rs` + `abrir_pantalla`; `capa.rs` hace las fotos |
| Ajustes | `crates/pixpin-store/src/herramientas.rs`, pestana «Herramientas» en `ventana_ajustes.rs` |

Decisiones tomadas por el camino:

- **Barra y panel en coordenadas del escritorio, pintados corridos.** La
  caja y el panel siempre se han colocado con las coordenadas de los clics
  (las del escritorio) y se pintaban tal cual en la ventana: solo cuadraba
  con la ventana en la esquina (0,0) del monitor principal. El anotador, que
  empieza en la x negativa de un monitor a la izquierda, lo destapo; ahora
  `pintar` corre la barra y el panel (`corrimiento_ui`), lo que de paso
  arregla el lienzo en ventana (F11).
- **Pin: barra arriba, no columna.** Treinta botones en columna no caben
  en 1080; la paleta del pin es ahora la barra del lienzo arriba en el centro
  del monitor del pin.
- **Lector: la letra de una herramienta es de la herramienta mientras se
  anota** (la M de la mano, la G de la escala grafica, la A de la cota), y
  el lector la vuelve a tener si se apaga esa herramienta. Se sale de anotar
  con Escape o con el boton del riel. Ctrl+Z en el PDF sigue yendo a la hoja
  del ultimo gesto.
- **Anotador de pantalla: sin lupa por ahora.** La lupa del lienzo es una
  vista sin cristal; la de la capa vieja (con sesion WGC) no se ha portado.

## Numeros (release, esta maquina, sin ensenar ninguna ventana)

- **Lector anotando** (1920 x 1080, 300 trazos + trazo vivo hasta 720
  puntos, fotograma entero con el documento): media 7,0 ms, p95 9,2 ms,
  peor 12 ms (sin tinta: 2,3 ms). Puerta nueva:
  `visor::pruebas::un_fotograma_del_lector_anotando_cabe_en_un_fotograma_de_60_hz`.
- **Anotador de pantalla**, fotograma del trazo en la capa de tinta (B2, el
  del lienzo): 1 monitor media 1,6 ms / p95 2,5 ms; 2 monitores (3840 x
  1080) media 1,5 ms / p95 2,0 ms. Banco:
  `ventana_editor::medir::medir_el_anotador_de_pantalla_en_uno_y_dos_monitores`.
- **Memoria del anotador abierto en 2 monitores**: privada +34 MB (viva) /
  +81 MB (congelada, con la foto en CPU); video medido +290 MB / +310 MB con
  la ventana sin ensenar. Esa cifra de video es un techo: sin composicion
  DWM no suelta las superficies de DirectComposition que se repintan (sin
  capas el mismo banco da +68 MB). Por cuenta: 2 mapas de 3842 x 1082 +
  interfaz + tinta ≈ 67 MB, mas la foto (17 MB) en congelada. El colchon de
  256 px del lienzo no se monta en el anotador (1 px): un 40 % menos de
  superficie de escena a dos monitores.
- Puertas de siempre en verde: `pixpin-motor2d/tests/puertas.rs` (14),
  `pixpin-render/tests` (GPU), soltar el lapiz 0,5 ms con 50 y con 2.000
  elementos, empezar y acabar de arrastrar 1,1-1,6 ms.
