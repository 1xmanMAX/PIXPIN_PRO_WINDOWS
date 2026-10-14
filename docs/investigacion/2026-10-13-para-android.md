# Para Android: lo que cambió en el PC (11 al 13-oct-2026)

Punto de entrada único para quien trabaje en PixPin Android
(`1xmanMAX/PIXPIN_PRO_ANDROID`). Todo está en `main` de
`1xmanMAX/PIXPIN_PRO_WINDOWS` (6fb6b4f y siguientes). Cada punto remite a su
guía detallada en esta misma carpeta.

## El asistente (14-oct): manual para portarlo

Todo el asistente del PC (motores, las 45 herramientas, los formatos que se sincronizan, la UI y
los algoritmos) está en **[`docs/manual-asistente-android.md`](../manual-asistente-android.md)**,
con los esquemas exactos en `docs/manual-asistente-android/herramientas.json`. Lo urgente para
no perder datos aunque Android aún no tenga asistente: que `Mensaje` conserve `lienzoIa` y
`fuentesIa` (o todo lo que no conoce), §3.1 del manual.

## Qué hay que hacer en Android (por orden)

0. **URGENTE — el visor de planos pisa lo que llega del PC** (ver la sección
   «Lo anotado en un plano del PC que no se veía en el móvil», al final):
   `PlanoActivity` lee la capa una vez y la reescribe en cada `onPause`, así
   que lo anotado en el PC con el plano abierto en el móvil no se ve y se
   borra en los dos aparatos. Releer al llegar algo, no guardar sin cambios y
   juntar al guardar, como hace ya el PC.
1. **Guardar el formato nuevo de las celdas** (A1 de
   `2026-10-12-que-se-sincroniza.md`). El PC añade a `EstiloDeCelda` las claves
   `k` (cursiva), `s` (subrayado), `t` (tachado), `c` (color de letra), `w`
   (ajustar texto) y `nf` (`numero` | `moneda` | `porcentaje` | `fecha`), junto
   a las de siempre (`n`, `a`, `f`, `e`). Android ya las ignora al leer
   (`ignoreUnknownKeys`), pero **las pierde al volver a guardar**: hay que
   añadirlas con esos mismos nombres, pintarlas y, si se puede, ofrecer los
   mismos botones (negrita, cursiva, subrayado, tachado, resaltar, color,
   alinear, formato de número, ajustar, borrar formato).
2. **Abrir la mini-app `tabla` creada en el PC** (A2): el `texto` del mensaje es
   exactamente el JSON de `TablaDeCalculo`. Hoy Android lo enseña como texto.
3. **Anotaciones de los planos CAD** (`2026-10-12-anotaciones-cad-android.md`):
   el PC ya usa los mismos ficheros que Android (`anot-<uid>.excalidraw.gz` +
   `anot-<uid>.hoja`, unidad = letra más común × 1,4 / 20 como v0.115) y guarda
   cotas y marcos en los grupos `plano-cota` / `plano-marco`. En Android:
   que `CapaDelPlano.con` junte también **por forma** (no solo por id) para no
   borrar y recrear cotas llegadas del PC, y un orden explícito de los marcos.
4. **Fórmulas** (`2026-10-11-formulas-android.md`): el motor del PC tiene ~90
   funciones con su tabla de pruebas; copiarlo para que una hoja calcule igual
   en los dos aparatos.
5. **Hojas editadas de un Excel del chat** (A3): hoy Android usa
   `tablas/libro-<msg>-<i>.json` y el PC mensajes mini-app con
   `referencia = libro-<msg>-<i>`; ninguno ve las ediciones del otro.
   **Pendiente de decisión del usuario.** Recomendación: que Android lea
   también los mensajes del PC. No implementar hasta que el usuario elija.
6. **A4** (logo de proyecto, pines, ajustes): necesitan diseño antes de tocar
   nada. Ojo: Android reescribe el proyecto sin los campos que no conoce, así
   que el PC no puede añadirle campos hasta que Android los conserve.

## Lo que el PC ya cambió y Android ya hacía (no hay que tocar nada)

- Archivar un proyecto nacido en el PC ahora viaja (marca `archivado`).
- Un adjunto borrado en un aparato se borra también del disco del otro
  (`adjuntosFuera`); el PC solo borra lo de `guardados/` que nadie más usa.
- Las tablas del móvil (`TABLA`, `tablas/<ref>.json`) se abren y editan en el
  PC y vuelven con el mismo JSON, conservando lo que el PC no conoce.
- Word en modo texto: imágenes dentro de celdas de tabla contadas como en
  Android (antes el PC desfasaba las siguientes).
- Modo oscuro de los visores: el PDF usa la misma matriz que «Invertir
  colores» (`MATRIZ_INVERTIDA`). Es una preferencia de cada aparato; no viaja.

## Cómo comprobarlo

Las pruebas del PC con un «móvil» simulado que habla el protocolo de verdad
están en `crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs` y
`apps/pixpin/src/plano_cad/anotar/pruebas_con_el_movil.rs`: describen, caso a
caso, qué espera recibir y mandar cada lado.

## URGENTE: las voces de Microsoft dejaron de hablar (12-oct)

Desde el 12-oct `speech.platform.bing.com/.../edge/v1` contesta **403** a la
petición de hablar (la lista de voces sigue funcionando). El PC ahora habla por
`api.msedgeservices.com/tts/cognitiveservices/websocket/v1?Ocp-Apim-Subscription-Key=…`
con el mismo token y el mismo `Sec-MS-GEC` (arreglo de rany2/edge-tts #491),
probado en vivo. En Android hay que cambiar `EdgeVoz.urlDeHablar` igual. Ver
`crates/pixpin-voz/src/edge.rs`.

## También nuevo en el PC (13-oct)

- Lo anotado sobre un PDF leído «como texto» viaja como `anot-<uid>-texto`
  (tinta, `.marcas`, `.maqueta`, `.voz`, `.sitio`), como en Android.
- Grafito con `dureza`, guías (`reference`, `referenciasVisibles`), fotos y
  audios añadidos a lecciones viejas: mismo formato que Android, con pruebas
  de ida y vuelta en `crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs`.
- Chats compartidos con personas: compatibles byte a byte con v0.122
  (comprobado contra Java); falta probar con teléfonos reales.

## Lo anotado en un plano del PC que no se veía en el móvil (13-oct, tarde)

El usuario: «las anotaciones de AutoCAD no se están sincronizando de la PC al
celular, solo del celular a la PC». Arreglado en el PC lo que era del PC (rama
`cad-sincro`); lo de Android, aquí.

### Lo que se midió en sus datos

- El PC guarda lo anotado de un plano del chat donde toca:
  `lienzos/anot-<uid>.excalidraw` de la carpeta del chat (lo que la
  sincronización manda como `pins/draw/anot-<uid>.excalidraw.gz`) y
  `android/pins/draw/anot-<uid>.hoja`. **Viaja**: la base de la última vuelta
  con el móvil tiene, para `anot-7G5G8874YS` (*11.15 PLANO DE SEÑALIZACIÓN
  VIAL HORIZONTAL*), el mismo resumen que la capa del PC con el rectángulo
  que el usuario dibujó allí a las 13:04:58 (`pc104`).
- **Cae en su sitio**: la caja de ese plano en el PC es
  `[-721.1, -371.6, 754.3, 407.8]` (origen en el centro de lo que importa) y
  la que dice la `.hoja` del móvil `[-738.0, -389.9, 738.0, 389.7]` (origen en
  el centro de la caja); con `encajeDe` el rectángulo cae en el mismo punto
  del plano en los dos.
- El móvil cortó la conexión cinco veces entre las 12:45 y las 13:03 mientras
  el usuario dibujaba en el plano («An existing connection was forcibly
  closed by the remote host»). Conviene mirar por qué.

### Lo que falla en Android: `PlanoActivity` no relee y pisa al salir

1. **Lee la capa una sola vez**, al abrir (`LaunchedEffect(original)`). Lo que
   trae la sincronización con el plano abierto (o en la pila de pantallas) no
   se ve: `Disco` avisa con `Cambio.ARCHIVOS` y `Red` sube
   `ExcalidrawStore.revision`, pero `PlanoActivity` no la mira.
2. **Guarda en cada `onPause`, aunque no haya cambiado nada**, la capa que
   tiene en memoria (`guardarCapa` → `ExcalidrawStore.guardar`), que no tiene
   lo que llegó del PC: el fichero del móvil lo pierde.
3. En la vuelta siguiente la base lo tiene y el móvil no: para la fusión es
   **un borrado en el móvil**, y se borra también en el PC.

Así: el usuario dibuja en el PC con el plano abierto en el móvil, sincroniza,
no lo ve; sale del plano y lo del PC desaparece en los dos. Al revés no
pasaba porque el PC solo escribe si se anotó algo.

**Qué cambiar** (lo que hace ya el PC, `apps/pixpin/src/plano_cad/anotar.rs`):

- **Releer al llegar algo**: observar `ExcalidrawStore.revisionDe(idCapa)` (o
  fecha y tamaño de `anot-<uid>.excalidraw.gz` y de su `.hoja`) y, si no se
  está anotando y no hay nada sin guardar, volver a leer la capa con su
  `.hoja` (`encajeDe` → `unidadesEntre` → `aLaMemoria`) y con ella las cotas y
  los marcos (`CapaDelPlano.leer`). En el PC: `MotorDelPlano::al_dia`, mirado
  al pintar, como mucho cada 0,7 s.
- **No guardar si no cambió nada**: `onPause` no es un cambio.
- **Al guardar, juntar con lo del disco si cambió desde que se leyó**, con
  `Fusion.json` (la de la sincronización): `base` = el texto de la capa tal
  como se leyó o escribió aquí la última vez, `mio` = lo que se va a
  escribir, `suyo` = lo que hay ahora. Si la `.hoja` del disco es otra, pasar
  antes `base` y `mio` a sus unidades (`transformar`, sin subir versiones).
  Después, releer lo junto. En el PC: `MotorDelPlano::guardar_juntando`.

### Ids repetidos (arreglado en el PC; Android debería comprobarlo)

El PC nombraba lo nuevo `pc<n>` con el número de elementos leídos más uno.
Si el móvil quitaba uno (lo borrado no se guarda, `purgeDeleted`), el
siguiente trazo del PC salía **con el id del último trazo del PC**. Con dos
figuras con el mismo id la fusión no junta figura por figura (`conIds` falla)
y en el móvil una tapa a la otra: lo nuevo del PC no aparecía. Desde hoy lo
nuevo del PC cuenta desde el mayor `pc<n>` del fichero
(`pixpin_motor2d::excalidraw::a_escena`). En los datos del usuario ya hay
cuatro dibujos así (`anot-34FJGS8TT2-p3`, `anot-34FJGS8TT2-p4`, `FM5RQ7G7DE`,
`Z2C3WQFTHJ`; ninguno es de un plano). En Android: que una escena con ids
repetidos no esconda ninguno al pintar y, al guardar, dar id nuevo al
repetido de versión más baja, para que esos dibujos se arreglen solos.

### Lo ya anotado

- Los `lienzos/anot-*.excalidraw` del PC **no son copias que no viajaron**:
  son el sitio de siempre del PC para `pins/draw/anot-*.excalidraw.gz`, con
  el mismo contenido que el móvil (mismos resúmenes en la base). No hay que
  moverlos.
- Lo que el móvil ya pisó al salir del plano se borró en los dos y no vuelve.
- Tres trazos del PC en `anot-3DSRX435ZU` (*VERTICAL - TAHUANTINSUYO*, 12-oct
  22:01, con el PC de antes) caen fuera del plano en los dos aparatos: se
  dibujaron sin `.hoja`, con la unidad y el origen de antes (el centro de la
  caja entera, que en ese plano llega hasta el (0, 0) del mundo); la `.hoja`
  de hoy ya se escribió con el origen nuevo. No se mueven solos.

## Las letras de los planos viajan entre tus aparatos (14-oct)

Para que el móvil no pida letras al abrir planos, las que el usuario **añade él
mismo** con «Aa» → «Añadir letras…» viajan a sus otros aparatos, **nunca** a
personas de chats compartidos. PixPin no reparte letras de Autodesk ni de
Microsoft: solo las que el usuario puso en su carpeta.

- **Un mensaje `ARCHIVO` por letra** en el chat «Asistente», id fijo
  `auto-letra-<limpio(nombre en minúsculas)>` (`auto-letra-romans.shx`),
  `nombre` = el del fichero (el plano lo pide por él), `ruta` =
  `pixpin:files/guardados/fuentes-shx/<nombre>`. Si ya está, no se toca: el
  fichero cambia y la sincronización lo manda por su resumen.
- **La carpeta del móvil pasa a ser `files/guardados/fuentes-shx`**
  (`LectorDePlanos.carpetaShx`), moviendo la de antes (`files/fuentes-shx`) la
  primera vez. Motivo: `fuentes-shx/` no está en `Disco.CARPETAS` (no viaja) y
  fuera de `guardados/` borrar el mensaje no se lleva el fichero
  (`adjuntosFuera`). En el PC esa ruta del chat «Asistente» es su carpeta de
  siempre, `<raíz>/fuentes-shx` (`vista::LETRAS_PORTATIL`).
- **Añadir**: copiar y poner el mensaje de cada una que **sea letra por dentro**
  (SHX empieza por `AutoCAD-86`; TTF `00 01 00 00`/`true`/`typ1`, OTF `OTTO`,
  TTC `ttcf`) y con nombre sin `/ \ " ' ( ) < >`. Topes: 10 MB por letra y
  64 MB entre todas; lo que pase se queda solo en ese aparato.
  Al abrir un plano, señalar también las que ya estaban sin mensaje.
- **«Quitar las mías»**: borrar ficheros y mensajes **con marca de borrado**;
  el otro aparato los quita al sincronizar (ya lo hace `adjuntosFuera`).
- **Rechazar**: un fichero con nombre de letra que no lo es por dentro, o un
  mensaje `auto-letra-*` que señala otra cosa, se borra con su mensaje (con
  marca). **Valla**: en Android `files/` es uno solo, así que una persona de un
  chat compartido no puede escribir en `guardados/fuentes-shx/` (tirarlo en
  `soloDeEsteChat`); solo cuenta lo que señala el chat «Asistente».
- **Al llegar letras con el plano abierto**: la huella (`huellaDeLetras`)
  cambia; releer el plano sin mover la vista, como al añadir con «Aa».
- Pruebas del PC: `apps/pixpin/src/letras_que_viajan/pruebas.rs` (ida y vuelta
  con el móvil simulado, borrado en los dos sentidos, chat compartido y
  fichero falso).
