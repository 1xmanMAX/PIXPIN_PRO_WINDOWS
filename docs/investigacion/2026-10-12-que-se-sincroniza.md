# Qué se sincroniza entre el PC y el móvil (12-oct-2026)

Pedido del usuario: «todo tiene que ser sincronizable, todo cambio». Aquí está, cosa por cosa,
lo que el usuario puede crear o cambiar en el PC y si viaja al móvil y vuelve, también al borrar
y al cambiar dos veces. Las anotaciones de los planos CAD quedan fuera (las lleva otro agente).

Fuentes: `crates/pixpin-sincro` (el cable, puerto de `sincro/` de Android),
`crates/pixpin-proyecto/src/vista.rs` (el `Disco` del PC) y el Kotlin **actual** de Android
(`1xmanMAX/PIXPIN_PRO_ANDROID`, `95c581a` v0.121.0, leído con `gh api`): `sincro/Disco.kt`
(`alcance`, `aplicarMensajes`, `guardarProyecto`), `sincro/AnotacionesDelAdjunto.kt`,
`sincro/GaleriaQueViaja.kt`, `motor/TablaDeCalculo.kt`, `motor/TablasEnDisco.kt`,
`guardados/LibroDelChat.kt`, `motor/Proyectos.kt`, `lecciones/LeccionesStore.kt`.

## Cómo viaja todo (en corto)

Se sincroniza **por chat** (la conversación general y una por proyecto). En cada chat viajan
tres cosas, cada una con su regla de «quién gana»:

1. **Los mensajes** (`guardados.jsonl`), por su código único; editar cambia su resumen y viaja;
   borrar deja una marca (`sincro/borrados.jsonl`) que viaja y no deja que vuelva. Las notas se
   juntan párrafo a párrafo (diff3, `fusion::parrafos`).
2. **El proyecto** (`proyecto.json`): datos sueltos campo a campo y hojas una a una
   (`mezcla.rs`); borrar el proyecto entero deja lápida (`chatsborrados.jsonl`).
3. **Los archivos que alcanza** (`disco::alcance_de` + `DiscoPc::alcance`): adjuntos, lienzos,
   tablas, croquis, lo anotado `anot-<uid>…` y lo que va dentro de lienzos y croquis. Los de
   texto (lienzos, croquis, `tablas/*.json`) se juntan figura a figura / celda a celda contra lo
   acordado (`sincro/objetos`); los demás, gana el más nuevo.

Aparte, fuera de los chats, la **galería de capturas** (`galeria.rs`, peticiones `galeria`,
`galeriajunta`, `damecaptura`, `poncaptura`).

## La tabla

✅ viaja · ❌ no viaja · ➖ no aplica · **arreglado** = hueco cerrado en esta rama.

| Cosa | PC→móvil | móvil→PC | Borrar | Nota (dónde se demuestra) |
|---|---|---|---|---|
| Mensajes de texto del chat (crear, editar, fijar, emoji, responder) | ✅ | ✅ | ✅ marca | `DiscoPc::mensajes`/`aplicar_mensajes`; prueba `lo_cambiado_en_un_lado_pasa_y_en_los_dos_se_junta`, `lo_borrado_en_el_pc_deja_marca…` |
| Editar el mismo mensaje en los dos (cambiar dos veces) | ✅ | ✅ | ➖ | `fusion::parrafos` (diff3 por párrafos); `la_primera_vez_junta_todo_sin_pisar…` |
| Adjuntos (imagen, archivo, voz con su transcripción) | ✅ | ✅ | ✅ **arreglado** | Antes el PC quitaba el mensaje pero dejaba el fichero en disco (Android sí lo borra: `adjuntosFuera`). Ahora `DiscoPc::borrar_adjuntos`; pruebas `un_adjunto_borrado_en_el_movil_se_va_tambien_del_disco_del_pc` y el negativo `un_fichero_que_otro_mensaje_sigue_senalando_no_se_borra` |
| Adjuntos con «(1)» en el nombre | ✅ | ✅ | ✅ | `DiscoPc::alcance` (ruta entera); `los_md_con_parentesis_…` |
| Renombrar un adjunto o un documento (F2 en el lector) | ✅ | ✅ | ➖ | Cambia el `nombre` del mensaje, no el fichero (`renombrar_doc.rs`, `ventana_chat/renombrar.rs`): viaja como edición |
| Notas Markdown (hoja nota de proyecto y nota del chat) | ✅ | ✅ | ✅ | `hoja.nota` en `proyecto.json` + mensaje `NOTA` (`notas_md/guardar.rs`); se juntan por párrafos |
| Comentarios de notas | ✅ | ✅ | ✅ | `anot-<uid>.comentarios.json`; `los_comentarios_de_una_nota_viajan_con_ella_y_se_van_con_ella`, `…_de_una_hoja_nota_del_movil…` |
| Lienzos (dibujo, con sus fotos dentro) | ✅ | ✅ | ✅ | `lienzos/<d>.excalidraw` ↔ `pins/draw/<d>.excalidraw.gz`; figura a figura: `un_lienzo_cambiado_en_los_dos_se_junta…`, `un_lienzo_dibujado_en_el_pc_llega_al_movil…` |
| Marcadores de un lienzo (`.marcas` junto al lienzo) | ✅ | ✅ | ✅ | `anotado::marcas_junto_a`; `los_marcadores_de_un_lienzo_la_voz_el_sitio…` |
| Foto anotada (foto + su lienzo) | ✅ | ✅ | ✅ | `una_foto_anotada_en_el_pc_llega_al_movil…` |
| Tinta, marcadores, espacios, maqueta, voz, sitio y marco sobre PDF / Word / libro del chat | ✅ | ✅ | ✅ | `anot-<uid>…` (`anotado_del_adjunto.rs`, `pixpin_proyecto::anotado`); `lo_anotado_sobre_un_pdf_un_word_y_un_libro…`, `el_marco_de_la_tinta_viaja…`, `borrar_el_mensaje_se_lleva_lo_anotado…` |
| `.pixpin-lectura` (aumento, página del PDF) | ❌ | ❌ | ➖ | **Por diseño en los dos**: es de cada pantalla (en Android, sus preferencias) |
| `.pixpin-anotado` de un documento abierto desde el Explorador (no del chat) | ❌ | ➖ | ➖ | No es de ningún chat; el móvil tampoco hace viajar lo que no es del chat. Para que viaje, hay que meterlo en el chat |
| Tablas hechas en el **móvil** (`TABLA`, `tablas/<ref>.json`) | ✅ | ✅ | ✅ | **arreglado**: llegaban pero el PC no las abría. Ahora el pin de hoja las abre y guarda en SU fichero (`vista::tabla_del_movil`, `hoja_pin::Origen::TablaDelMovil`); celda a celda: `una_tabla_del_movil_se_cambia_en_el_pc_y_vuelve_celda_a_celda`, `hoja_pin/pruebas_movil.rs` |
| Tablas hechas en el **PC** (mini-app `tabla`, el JSON en el `texto`) | ⚠️ | ✅ | ✅ marca | Viaja el mensaje, pero Android no conoce la mini-app `tabla` y enseña el JSON como texto. **Para Android** (abajo) |
| Formato nuevo de celdas `k,s,t,c,w,nf` (`tabla.rs`) | ⚠️ | ✅ | ➖ | Viaja y se junta (la fusión es de JSON genérico), pero el `EstiloDeCelda` de Android solo guarda `n,a,f,e`: si el móvil vuelve a guardar la tabla, **los pierde**. **Para Android** |
| Excel / CSV del chat editado (hojas de un libro) | ❌ | ❌ | ➖ | **Dos formatos para lo mismo**: Android guarda cada hoja en `tablas/libro-<msg>-<i>.json` (`LibroDelChat`); el PC, en mensajes mini-app `tabla` con `referencia = libro-<msg>-<i>` (`hoja_pin/libro.rs`). Cada uno ve el original o sus propias ediciones. Hace falta decidir uno (abajo) |
| Tareas y listas (tachar, fechas, imágenes) | ✅ | ✅ | ✅ marca | Mini-app `tareas` en el `texto`; las imágenes son `![](pixpin:files/…)` dentro y viajan por `en_texto`; vuelta rápida: `tests/tareas_al_instante.rs` |
| Alarmas / recordatorios (`recuerdaEn`) | ❌ | ❌ | ➖ | **Por diseño en los dos**: es de cada aparato (`kotlin::texto_de_base` lo quita del resumen) |
| Lecciones aprendidas | ✅ | ✅ | ✅ **arreglado** | Mensaje `ARCHIVO` + `guardados/lecciones/<id>.leccion`; se juntan (`disco::juntar_leccion`). Borradas en el móvil dejaban el `.leccion` en el PC: lo cierra el arreglo de adjuntos |
| Croquis 3D | ✅ | ✅ | ✅ | `croquis3d/<id>.croquis.gz`, trazo a trazo (el PC los pasa tal cual) |
| Proyecto: crear, renombrar, hojas, su orden, quitar hojas, PDF | ✅ | ✅ | ✅ lápida | `proyecto_portatil`/`guardar_proyecto`, `mezcla.rs`; `un_proyecto_del_movil_sale_identico…`, `un_proyecto_borrado_viaja_con_su_lapida…`, `…recuperado_de_la_papelera…` |
| Proyecto: archivar / desarchivar | ✅ **arreglado** | ✅ | ➖ | Un proyecto nacido en el PC (sin `proyecto.json`) guardaba el archivado en su ficha y no salía. Ahora `proyecto_local` lo exporta y `guardar_proyecto` quita la marca cuando llega el `proyecto.json`; `archivar_un_proyecto_viaja_en_los_dos_sentidos_y_dos_veces` y el negativo `una_marca_de_archivado_que_no_es_si_no_archiva_nada` |
| Proyecto: logo de imagen | ❌ | ➖ | ➖ | Solo del PC (`ventana_chat/logo.rs`, `logo.png` fuera del alcance). Android no tiene logos: **Para Android** si se quiere |
| Proyecto: carpeta elegida (`ubicacion`) | ❌ | ➖ | ➖ | Por diseño: es una ruta de este PC |
| Galería de capturas (conservar, prorrogar, quitar antes de tiempo, fecha de irse) | ✅ | ✅ | ✅ | `pixpin-sincro/src/galeria.rs` + `apps/pixpin/src/sincronizar/galeria.rs`; `galeria/pruebas.rs` |
| Pines del escritorio y sus marcas de texto | ❌ | ❌ | ❌ | Los pines no son de ningún chat, ni en el PC ni en el móvil; las marcas de texto van al `.pixpin2d` del pin (`pines.rs::cambiar_dibujo`). Para que viajen, el pin tendría que ir a un chat (**Para Android**, diseño) |
| Nombre del aparato y grupo | ✅ | ✅ | ✅ salir | `Identidad` (`guardar_identidad`) |
| Ajustes (tema, atajos, idioma, días de caducidad…) | ❌ | ❌ | ➖ | Ninguno de los dos los sincroniza; los «días hasta borrar» viajan dentro de la galería como fecha de cada captura |

## Lo arreglado en el PC (esta rama)

1. **Archivar un proyecto nacido en el PC viaja** (`vista.rs`: `MARCA_ARCHIVADO`,
   `archivado_en_ficha`, `proyecto_local`, `guardar_proyecto`). Formato: el `archivado` del
   `Proyecto` de Android, sin cambios.
2. **Un adjunto borrado en el otro aparato se va también del disco del PC**
   (`DiscoPc::borrar_adjuntos`, llamado desde `aplicar_mensajes`), como `adjuntosFuera` de
   `Disco.aplicarMensajes`: solo lo de `guardados/`. Más prudente que Android: solo clases
   archivo/imagen/voz (un lienzo o una página son hojas) y nunca un fichero que otro mensaje o
   el documento del proyecto siguen señalando.
3. **Las tablas del móvil se abren, se editan y se guardan en el PC**
   (`vista::tabla_del_movil`, `vista::guardar_tabla_del_movil`, `hoja_pin::Que::TablaDelMovil`,
   `Origen::TablaDelMovil`). Se guardan en su fichero `tablas/<ref>.json` con el mismo JSON
   (`TablaDeCalculo`), y lo que el PC no conoce (`protegida`) se conserva.

## Para Android

Huecos que necesitan cambios en el móvil. No se han tocado en el PC para no inventar formato.

### A1. Guardar el formato nuevo de las celdas

`motor/TablaDeCalculo.kt`, `EstiloDeCelda`: añadir los seis campos del PC, con los mismos
nombres de una letra y sin escribir lo que vale lo de por defecto (como ya hace con `n,a,f,e`):

```kotlin
data class EstiloDeCelda(
    val n: Boolean = false, val a: String? = null, val f: String? = null, val e: Boolean = false,
    val k: Boolean = false,   // cursiva
    val s: Boolean = false,   // subrayado
    val t: Boolean = false,   // tachado
    val c: String? = null,    // color de la letra, "#rrggbb"
    val w: Boolean = false,   // ajustar texto
    val nf: String? = null    // "numero" | "moneda" | "porcentaje" | "fecha"; null = General
)
```

Sin esto, una tabla con formato del PC que se edita en el móvil vuelve sin cursiva, color, etc.
(La prueba `una_tabla_del_movil_se_cambia_en_el_pc_y_vuelve_celda_a_celda` simula justo ese
guardado de Android.)

### A2. Abrir la mini-app `tabla` del PC

Una tabla creada en el PC es un mensaje `MINIAPP` con `miniapp = "tabla"` y en `texto` el JSON
**exacto** de `TablaDeCalculo` (`nombre`, `celdas`, `anchos`, `estilos`, `tocado`, …). Android la
enseña como texto. Propuesta: que la burbuja abra su editor de tablas con
`TablaDeCalculo` leído del `texto` y que guardar escriba el JSON de vuelta en el `texto` del
mismo mensaje (el PC hace igual; la fusión de mensajes ya lo junta).

### A3. Un solo formato para las hojas editadas de un Excel del chat

Hoy hay dos: Android, `tablas/libro-<mensaje>-<hoja>.json` (`LibroDelChat`); el PC, un mensaje
mini-app `tabla` por hoja con `referencia = "libro-<mensaje>-<hoja>"` y el JSON en el `texto`
(`apps/pixpin/src/hoja_pin/libro.rs`). El dato es el mismo (`TablaDeCalculo`) y la clave también
(`libro-<mensaje>-<hoja>`); cambia solo dónde vive. Propuesta, por orden de menor cambio:

- **Android lee también las del PC**: en `LibroDelChat.tablas`, si hay mensajes `MINIAPP`
  `tabla` del mismo chat con `referencia = idDeHoja(m, i)`, usar esos (y guardar en ellos).
- O **el PC escribe las de Android** (`tablas/libro-…json`, que el `Disco` del PC ya sabe
  colocar en `android/tablas/`). Esto toca el editor de hojas del PC (rama `vh-excel`) y lo
  dejo para que lo decida el coordinador: no se debe hacer sin decidir qué pasa con los libros
  ya editados en un formato.

### A4. Lo que Android no tiene

- **Logo de proyecto**: si se quiere en los dos, un campo nuevo en `Proyecto`
  (`logo: String?`, ruta portátil `pixpin:files/proyectos/logo-<id>.png`). Ojo: Android lee el
  proyecto con `ignoreUnknownKeys` y lo vuelve a escribir sin lo que no conoce, así que el PC no
  puede añadir campos al proyecto hasta que Android los tenga.
- **Pines y sus marcas de texto**: no son de ningún chat en ningún aparato. Para que viajen
  haría falta decidir en qué chat viven (p. ej. «Mensajes guardados» como mensaje `IMAGEN` con
  su lienzo `pins/draw/foto-<id>`, que es como ya viaja una foto anotada).
- **Ajustes**: ninguno de los dos los sincroniza. Si el usuario los quiere comunes, haría falta
  elegir cuáles (los de cada aparato, como atajos o rutas, no) y un archivo propio
  (`sincro/ajustes.json`, gana el más nuevo por clave).
