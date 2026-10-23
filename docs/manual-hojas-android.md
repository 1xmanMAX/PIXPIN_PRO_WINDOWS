# Manual de las hojas de cálculo para PixPin Android

Para quien mantenga la hoja de cálculo de Android (`1xmanMAX/PIXPIN_PRO_ANDROID`) frente a la del
PC (`1xmanMAX/PIXPIN_PRO_WINDOWS`, 15-oct-2026). Dice **en qué formato se guarda una hoja
editada**, dónde vive, cómo viaja y qué tiene que leer y escribir el móvil para que **los dos
aparatos editen la misma hoja sin perder nada**.

Decisión del usuario (15-oct-2026): «las hojas de Excel editadas tienen **UN SOLO formato**, el
del PC, y Android lee el del PC».

Revisado contra Android `main` en `ce0a904` (v0.125.0, 15-oct-2026): `motor/TablaDeCalculo.kt`,
`motor/TablasEnDisco.kt`, `guardados/TablasDelChat.kt`, `guardados/LibroDelChat.kt`,
`excel/ExcelActivity.kt`, `tabla/TablaActivity.kt`, `sincro/Fusion.kt`.

---

## Índice

0. [Resumen en diez líneas](#0-resumen-en-diez-líneas)
1. [Inventario: qué se guarda hoy en cada aparato](#1-inventario-qué-se-guarda-hoy-en-cada-aparato)
2. [El formato único](#2-el-formato-único)
3. [Dónde vive cada hoja](#3-dónde-vive-cada-hoja)
4. [Cómo viaja por la sincronización](#4-cómo-viaja-por-la-sincronización)
5. [Lo viejo: qué se lee y cómo se migra](#5-lo-viejo-qué-se-lee-y-cómo-se-migra)
6. [Qué tiene que hacer Android (lista)](#6-qué-tiene-que-hacer-android-lista)
7. [Ejemplos](#7-ejemplos)
8. [Pruebas de compatibilidad](#8-pruebas-de-compatibilidad)
9. [Dónde está cada cosa en el PC](#9-dónde-está-cada-cosa-en-el-pc)

---

## 0. Resumen en diez líneas

1. Una hoja editada es **siempre el mismo JSON**: `Tabla` en el PC, `TablaDeCalculo` en el móvil
   (`nombre`, `celdas`, `anchos`, `estilos`, `tocado` y lo que no se conozca). No hay otro formato:
   ni un `.xlsx` reescrito ni un sidecar distinto.
2. El `.xlsx` (o `.ods`/`.csv`) original **no se toca nunca**: editar un Excel del chat crea un
   mensaje-tabla por hoja con `referencia = "libro-<mensaje>-<hoja>"`.
3. Ese JSON vive en uno de tres sitios: el `texto` de un mensaje `MINIAPP` con `miniapp = "tabla"`
   (lo normal), o el fichero `tablas/<referencia>.json` de un mensaje `TABLA` del móvil.
4. El tercer sitio es **viejo y solo se lee**: `tablas/libro-<mensaje>-<hoja>.json` (lo que el
   móvil guardaba antes del 13-oct). Al guardar pasa a mensajes.
5. Orden al abrir un Excel del chat: **mensajes > ficheros viejos > leer el libro**.
6. Lo que no se conoce **se conserva** al guardar (claves de la tabla y de cada estilo).
7. `tocado` dice si una hoja se editó: al guardar, `max(ahora, cuando_del_mensaje + 2001)`.
8. Leer es **tolerante**: un objeto JSON siempre se abre (estilos-palabra, números en celdas...);
   lo que no se puede convertir va a `viejo_<campo>`. Solo lo que no es un objeto es un error.
9. Lo que no se lee **no se abre en blanco ni se guarda encima**.
10. La fusión junta el `texto`-tabla de un mensaje **celda a celda** en los dos aparatos.

---

## 1. Inventario: qué se guarda hoy en cada aparato

### 1.1 PC (después de esta ronda)

| Camino | Qué escribe | Dónde | Rust |
|---|---|---|---|
| Editor de hojas (pin de hoja), tabla del chat | JSON `Tabla` | `texto` del mensaje `MINIAPP`/`tabla` | `apps/pixpin/src/hoja_pin.rs` (`Editable::guardar`) |
| Editor de hojas, Excel del chat | JSON `Tabla`, uno por hoja | mensajes nuevos `MINIAPP`/`tabla` con `referencia = libro-<id>-<hoja>`; el `.xlsx` no cambia | `hoja_pin.rs`, `hoja_pin/libro.rs` |
| Editor de hojas, tabla del móvil (`TABLA`) | JSON `Tabla` | `android/tablas/<referencia>.json` (el fichero que viaja) | `hoja_pin.rs`, `pixpin_proyecto::vista::guardar_tabla_del_movil` |
| «Tabla nueva» del clip del chat | JSON `Tabla` vacío | mensaje `MINIAPP`/`tabla` | `ventana_chat.rs` |
| Pedido `tabla_nueva` (asistente, «tabla escaneada → tabla del chat», Flow Launcher) | JSON `Tabla` relleno | mensaje `MINIAPP`/`tabla` | `pedidos.rs` (`nueva_tabla`, `tabla_de_filas`) |
| Asistente, «tabla escaneada → Excel» | un `.xlsx` **nuevo** (no una hoja editada: es un adjunto) | mensaje `ARCHIVO` | `asistente/documentos_acciones.rs` (`escribir_excel`) |
| Importar un `.xlsx`/`.ods`/`.csv` para editar | nada hasta guardar; al guardar, como «Excel del chat» | — | `pixpin_proyecto::importar_hojas` |
| Pegar desde Excel/Sheets | celdas dentro de la `Tabla` que se edita | — | `pixpin_proyecto::portapapeles_tabla` |

Todos pasan por `Tabla::escribir` (JSON con sangría de 2 espacios) y se leen con `Tabla::leer`.
El PC **no escribe** nunca `tablas/libro-*.json`.

### 1.2 Android (`ce0a904`)

| Camino | Qué escribe | Dónde |
|---|---|---|
| Tablas sueltas y de proyecto (`TablaActivity`) | JSON `TablaDeCalculo` (compacto) | `files/tablas/<id>.json`; el mensaje `TABLA` lleva `referencia = <id>` |
| Mini-app `tabla` del chat (`mini-<mensaje>`) | JSON `TablaDeCalculo` | `texto` del mensaje (`TablasDelChat`) |
| Hoja editada de un Excel del chat (`libro-<mensaje>-<hoja>`) | JSON `TablaDeCalculo` | mensajes `MINIAPP`/`tabla`, como el PC (desde el 13-oct) |
| Excel leído la primera vez (copia de trabajo) | JSON `TablaDeCalculo` | `files/tablas/libro-<mensaje>-<hoja>.json` (`TablasEnDisco.enArchivo`, no crea mensajes) |
| Antes del 13-oct, hoja editada de un Excel | JSON `TablaDeCalculo` | **solo** `files/tablas/libro-<mensaje>-<hoja>.json` (lo viejo) |
| Visor de Excel (`ExcelActivity`, `Pxhj`) | nada: PXHL/PXHG/PXHT son formatos **de pintado** en memoria, no de guardado | — |

Conclusión: **el formato ya es uno** (el mismo JSON en los dos). Lo que cambia es el sitio, y eso
está fijado en §3. Lo que faltaba en el PC y se hizo en esta ronda: leer lo viejo (§5), juntar el
`texto`-tabla celda a celda al sincronizar (§4) y no perder nada de un JSON con forma rara.

---

## 2. El formato único

Un objeto JSON. UTF-8. Los espacios no importan (el PC escribe con sangría, el móvil compacto;
los dos leen los dos). Ningún campo es obligatorio: `{}` es una tabla vacía.

| Clave | Tipo | Qué es |
|---|---|---|
| `nombre` | cadena | El nombre. En una hoja de Excel, `«<libro> · <hoja>»` (la pestaña es lo de después de ` · `). |
| `celdas` | objeto `"B7"` → cadena | **Lo tecleado**, no el resultado: `"12"`, `"Total"`, `"=SUMA(B2:B9)"`. Las fórmulas empiezan por `=`. Una celda vacía **no se escribe** (borrar una celda la quita del mapa). Referencias en mayúsculas, columnas como Excel (`Z`, `AA`...), filas desde 1. |
| `anchos` | objeto `"B"` → entero | Ancho de la columna cambiado a mano, en unidades lógicas (px al 100 % en el PC, dp en el móvil: mismo número). |
| `estilos` | objeto `"B7"` → `EstiloDeCelda` | Ver abajo. Un estilo vacío no se escribe. |
| `tocado` | entero (ms desde 1970, UTC) | Última vez que se guardó. Ver §2.2. |
| `protegida` | booleano | Solo del móvil (la página web). El PC no la usa y **la conserva**. |
| cualquier otra | lo que sea | **Se conserva tal cual** al guardar. Incluye `viejo_*` (§5.1). |

### 2.1 `EstiloDeCelda`

Nombres de una letra porque se repiten en cada celda. **Lo que vale lo de por defecto no se
escribe** (`encodeDefaults = false`; en el PC, `skip_serializing_if`).

| Clave | Tipo | Qué es |
|---|---|---|
| `n` | bool | Negrita |
| `k` | bool | Cursiva (la `k` de Ctrl+K del Excel en español; la `c` ya es el color) |
| `s` | bool | Subrayado |
| `t` | bool | Tachado |
| `w` | bool | Ajustar texto (varias líneas) |
| `e` | bool | Editable aunque la tabla esté protegida (no es formato: «Borrar formato» la deja) |
| `a` | `"i"` \| `"c"` \| `"d"` | Alineación; sin ella, la que toque por el valor |
| `f` | `"#rrggbb"` | Relleno (resaltar) |
| `c` | `"#rrggbb"` | Color de letra |
| `nf` | `"numero"` \| `"moneda"` \| `"porcentaje"` \| `"fecha"` | Formato de número; sin él, «General». Una palabra desconocida se ve como «General» y **se conserva**. |
| cualquier otra | — | Se conserva tal cual. |

Las palabras de `nf` y las letras de `a` son datos guardados: **no se cambian nunca**.

### 2.2 `tocado` y «editada»

- Al guardar una hoja que es un mensaje: `tocado = max(ahora, mensaje.cuando + 2001)`
  (`editada_a` en el PC, `TablasDelChat.editadaA` en el móvil).
- Una hoja de Excel está **editada** si `tocado > mensaje.cuando + 2000`. Si alguna hoja de un
  libro lo está, el PC enseña el libro por sus tablas; si no, enseña el `.xlsx` original (con sus
  estilos, más rápido).
- Al crear los mensajes de un libro, cada hoja va **un milisegundo detrás** de la anterior
  (`cuando = ahora + i`), y la hoja cambiada lleva `editada_a`; las demás, el `tocado` de leerlas.

---

## 3. Dónde vive cada hoja

| Qué | Contenedor | Clave para encontrarla |
|---|---|---|
| Tabla del chat (mini-app) | `texto` de un mensaje `clase = "MINIAPP"`, `miniapp = "tabla"` | el id del mensaje (`mini-<id>` en el móvil) |
| Hoja editada de un Excel del chat | `texto` de un mensaje `MINIAPP`/`tabla` | `referencia = "libro-<id del mensaje del Excel>-<hoja desde 0>"` |
| Tabla del móvil (suelta o de proyecto) | fichero `tablas/<referencia>.json` (en el PC: `android/tablas/` de la carpeta del chat) | mensaje `clase = "TABLA"` con esa `referencia` |
| **Viejo**: hoja de Excel editada en el móvil antes del 13-oct | fichero `tablas/libro-<id>-<hoja>.json` | — (solo se lee, §5.2) |
| El Excel original | el adjunto (`clase = "ARCHIVO"`) | **nunca se escribe** |

Precedencia al abrir un Excel del chat (igual en los dos):

1. Si hay mensajes `tabla` con `referencia = libro-<id>-*` → esas son las hojas (ordenadas por el
   número de hoja).
2. Si no, si existen `tablas/libro-<id>-0.json`, `-1.json`... (consecutivos desde 0) → esas.
3. Si no, se lee el libro.

Al guardar en los casos 2 y 3 se crean los mensajes de **todas** las hojas (no solo la editada),
en orden, para que el otro aparato las vea igual.

---

## 4. Cómo viaja por la sincronización

- **Mensajes** (`MINIAPP`/`tabla`): viajan con la fusión de mensajes de siempre, campo a campo.
  El `texto` de dos versiones cambiadas a la vez se junta **celda a celda** si las dos son una
  tabla (`Fusion.tablaJunta` en el móvil; `fusion::tabla_junta` en el PC desde el 15-oct). Antes,
  en el PC, iba por párrafos: un choque entero o, con el JSON con sangría del PC, un JSON roto.
  El resultado sale en JSON compacto.
- Qué se considera «una tabla» en un `texto`: empieza por `{`, contiene `"celdas"`, es un objeto
  JSON válido y `celdas` es un objeto. Si no, sigue por párrafos como una nota.
- **Ficheros `tablas/*.json`**: viajan como texto portátil (`Disco.esTexto`) y se juntan con la
  misma fusión de objetos (celda a celda; `tocado` decide los choques de dentro). La
  sincronización trae también los `tablas/libro-<id>-<hoja>.json` de cada `ARCHIVO`.
- El `.xlsx` original viaja como cualquier adjunto y no cambia.

---

## 5. Lo viejo: qué se lee y cómo se migra

### 5.1 JSON con forma rara (`Tabla::leer` / `Tabla::rescatar` en el PC)

Si el JSON es un objeto pero no encaja en la forma de §2, el PC no se rinde: lo rescata campo a
campo y **al guardar escribe la forma de ahora**. Lo que no se puede convertir no se tira.

| Llega | Se lee como |
|---|---|
| estilo-palabra (PC de antes del 12-oct): `"B1": "negrita"` | `{"n": true}`. Palabras: `negrita`/`bold`, `cursiva`/`italic`, `subrayado`, `tachado`, `ajustar`/`wrap`, `izquierda`/`centro`/`derecha`. Separadas por espacio, coma o `+`. |
| estilo-palabra desconocida: `"C1": "brillante"` | estilo vacío con `{"viejo": "brillante"}` dentro (se conserva) |
| booleano raro en un estilo: `"n": 1`, `"n": "true"` | `true` |
| campo conocido de un estilo con tipo imposible: `"f": 7` | se queda en ese estilo como `"f": 7` (desconocido) |
| número o booleano en `celdas`: `"B1": 12.5`, `"C1": true` | `"12.5"`, `"VERDADERO"` |
| `null` en `celdas` | celda vacía (no se escribe) |
| lista u objeto en `celdas` | `viejo_celdas: {"E1": [...]}` |
| ancho con decimales o en texto: `120.4`, `"90"` | `120`, `90` |
| ancho imposible: `"ancho"` | `viejo_anchos: {"C": "ancho"}` |
| `tocado` como texto: `"1789000000000"` | `1789000000000` |
| `celdas`/`anchos`/`estilos` que no son un objeto | `viejo_<campo>` |

Lo que **no** es un objeto JSON (texto suelto, lista, número, JSON cortado) **es un error**: la
hoja no se abre, y nadie guarda encima. El fichero o el mensaje se quedan como están.

### 5.2 Las hojas de Excel que el móvil guardaba solo en fichero

`tablas/libro-<id>-<hoja>.json` (en el PC, `android/tablas/`). El PC (desde el 15-oct):

- **las lee** si el libro no tiene mensajes-tabla, hasta la primera que falte o no se entienda
  (como `TablasDelChat.crearLasDelLibro`); si ni la primera se entiende, lee el libro;
- en modo ver, un libro con estas hojas se ve **por ellas** (lo editado no se esconde tras el
  original);
- al guardar crea los mensajes de todas las hojas (formato único);
- **no escribe ni borra** esos ficheros nunca: uno roto sigue tal cual.

---

## 6. Qué tiene que hacer Android (lista)

Lo que **ya hace bien** (no tocar): mismo JSON y mismos nombres (`TablaDeCalculo`,
`EstiloDeCelda` con los seis campos del PC); conservar lo desconocido al guardar
(`TablaDeCalculo.aJsonSobre`, también dentro de cada estilo); hojas de Excel como mensajes
(`TablasDelChat`); `editadaA`; fusión celda a celda del `texto`; migrar sus ficheros viejos a
mensajes (`LibroDelChat.tablas` → `TablasDelChat.migrar`).

Lo que **falta o conviene cambiar** (por orden de importancia):

1. **No abrir en blanco lo que no se lee.** En `TablaActivity`, `almacen.cargar(id) ?:
   TablaDeCalculo(nombre = nombreNuevo(...))` trata igual «no existe» que «existe pero no se
   lee»: una tabla con un JSON que `deJson` no acepta se abre vacía y, al guardar, **se pisa**
   (en fichero, `aJsonSobre` solo rescata las claves de arriba si el viejo es un objeto; en un
   mensaje, `alGuardar` reescribe el `texto`). Hay que distinguir: si el fichero existe o el
   mensaje tiene texto y no se lee, avisar y **no** abrir para editar.
2. **Leer tolerante, como el PC** (§5.1). Hoy `TablaDeCalculo.deJson` devuelve `null` ante un solo
   campo con otro tipo; por ejemplo, un estilo-palabra (`"B1": "negrita"`) del PC viejo hace que
   la tabla entera no se abra. Lo mínimo: si `deJson` falla y el texto es un `JsonObject`,
   rescatar campo a campo con las mismas reglas y los mismos `viejo_<campo>`.
3. **Conservar `viejo_*`** y lo desconocido de cada estilo: ya lo hace `aJsonSobre`; que siga así
   (no añadir `viejo_*` a `CLAVES`).
4. **Al migrar los ficheros viejos a mensajes** (`crearLasDelLibro` con `editada == null`), poner
   `tocado = editadaA(ahora, cuando)` en cada hoja. Hoy conservan el `tocado` del fichero, que es
   anterior al `cuando` del mensaje nuevo: el PC las toma por «no editadas» y enseña el `.xlsx`
   original, escondiendo lo que se editó en el móvil. El precio (una hoja que solo se leyó se ve
   por sus tablas, sin los estilos del original) es menor que esconder una edición.
5. **No escribir más `tablas/libro-*.json` como copia de trabajo** cuando el libro ya tiene
   mensajes; y, a medio plazo, que la copia de trabajo no viaje (el PC la tomaría por una hoja
   vieja editada si el libro aún no tiene mensajes). Si sigue viajando, el comportamiento del PC
   es el de §5.2, que es seguro (no pierde nada), pero puede enseñar un libro por tablas sin que
   nadie lo haya editado.
6. Nada que hacer con `Pxhj`: es solo para pintar.

---

## 7. Ejemplos

### 7.1 Una hoja como la escribe el PC

```json
{
  "nombre": "Cuentas · Gastos",
  "celdas": {
    "A1": "Concepto",
    "B1": "Importe",
    "A2": "Cemento",
    "B2": "120",
    "B4": "=SUMA(B2:B3)"
  },
  "anchos": {
    "A": 180
  },
  "estilos": {
    "A1": { "n": true, "f": "#fff2cc" },
    "B4": { "n": true, "nf": "moneda", "c": "#c00000" }
  },
  "tocado": 1789500002001,
  "protegida": true
}
```

### 7.2 La misma, como la escribe el móvil

```json
{"nombre":"Cuentas · Gastos","celdas":{"A1":"Concepto","B1":"Importe","A2":"Cemento","B2":"120","B4":"=SUMA(B2:B3)"},"anchos":{"A":180},"estilos":{"A1":{"n":true,"f":"#fff2cc"},"B4":{"n":true,"c":"#c00000","nf":"moneda"}},"tocado":1789500002001,"protegida":true}
```

Las dos se leen igual en los dos aparatos (prueba
`el_formato_unico_va_y_vuelve_igual_desde_el_pc_y_desde_android`).

### 7.3 El mensaje de una hoja editada de un Excel

```json
{
  "id": "1789500000000",
  "cuando": 1789500000000,
  "clase": "MINIAPP",
  "miniapp": "tabla",
  "nombre": "Cuentas · Gastos",
  "texto": "{\n  \"nombre\": \"Cuentas · Gastos\",\n  \"celdas\": {...},\n  \"tocado\": 1789500002001\n}",
  "referencia": "libro-1789400000000-0",
  "proyecto": "<id del proyecto>",
  "numero": 12,
  "uid": "…",
  "aparato": "PC"
}
```

`1789400000000` es el id del mensaje `ARCHIVO` del `.xlsx`; `-0` es la primera hoja.

### 7.4 Una tabla del móvil (`TABLA`)

Mensaje `{"clase":"TABLA","referencia":"t-1789","nombre":"Metrados",...}` y, aparte, el fichero
`tablas/t-1789.json` con el JSON de §2. El PC lo edita **en ese fichero** (no en el mensaje).

### 7.5 Una tabla vieja y lo que queda al guardarla

Llega:

```json
{"nombre":"Cuentas","celdas":{"A1":"Total","B1":12.5,"E1":[1,2]},
 "estilos":{"A1":"negrita","C1":"brillante"},"tocado":"1789000000000"}
```

Se guarda:

```json
{
  "nombre": "Cuentas",
  "celdas": { "A1": "Total", "B1": "12.5" },
  "anchos": {},
  "estilos": { "A1": { "n": true }, "C1": { "viejo": "brillante" } },
  "tocado": 1789000000000,
  "viejo_celdas": { "E1": [1, 2] }
}
```

---

## 8. Pruebas de compatibilidad

En el PC (`cargo test -p pixpin-proyecto --lib tabla::` , `cargo test -p pixpin-sincro --lib fusion::`,
`cargo test -p pixpin --bin pixpinmax hoja_pin::`):

| Prueba | Qué asegura |
|---|---|
| `tabla::pruebas::el_formato_unico_va_y_vuelve_igual_desde_el_pc_y_desde_android` | ida y vuelta; el JSON compacto del móvil da la misma tabla |
| `tabla::pruebas::una_tabla_de_android_se_lee_y_se_devuelve_sin_perder_nada` | lo desconocido vuelve (arriba y en cada estilo) |
| `tabla::pruebas::una_tabla_vieja_o_rara_se_rescata_sin_perder_nada` | §5.1 entera; migrar es estable (leer lo migrado da lo mismo) |
| `tabla::pruebas::lo_que_no_es_un_objeto_json_no_se_toma_por_una_tabla` | caso negativo |
| `fusion::pruebas::la_tabla_del_texto_de_un_mensaje_se_junta_celda_a_celda` | §4, JSON del PC contra JSON del móvil |
| `hoja_pin::pruebas_movil::las_hojas_viejas_del_movil_se_editan_y_al_guardar_pasan_a_mensajes` | §5.2 |
| `hoja_pin::pruebas_movil::una_hoja_vieja_rota_no_se_pierde_y_se_lee_el_libro` | caso negativo de §5.2 |
| `hoja_pin::pruebas_movil::sin_su_fichero_o_sin_referencia_una_tabla_del_movil_no_se_abre` | un fichero roto no se abre ni se pisa |

Para el móvil, las mismas con JUnit (los JSON de §7 sirven de datos): `deJson` de 7.1 y 7.2 da
lo mismo; 7.5 se abre (punto 2 de §6); `"no es json"` no se abre ni se pisa (punto 1); migrar
ficheros viejos deja hojas «editadas» (punto 4).

---

## 9. Dónde está cada cosa en el PC

| Pieza | Rust |
|---|---|
| El formato (`Tabla`, `EstiloDeCelda`, `leer` tolerante) | `crates/pixpin-proyecto/src/tabla.rs` |
| Ficheros de tablas del móvil y hojas viejas | `crates/pixpin-proyecto/src/vista.rs` (`tabla_del_movil`, `tablas_viejas_del_libro`) |
| Editor (ver/editar/guardar, migrar lo viejo) | `apps/pixpin/src/hoja_pin.rs` |
| Mensajes de las hojas de un libro, «editada» | `apps/pixpin/src/hoja_pin/libro.rs` |
| Tabla nueva rellena (asistente, pedidos) | `apps/pixpin/src/pedidos.rs` (`nueva_tabla`, `tabla_de_filas`) |
| Fusión celda a celda | `crates/pixpin-sincro/src/fusion.rs` (`tabla_junta`) |
| Qué ficheros de tablas viajan | `crates/pixpin-sincro/src/disco.rs`, `valla.rs` |
