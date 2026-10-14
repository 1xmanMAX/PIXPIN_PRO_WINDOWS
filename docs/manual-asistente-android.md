# Manual del asistente para PixPin Android

Para quien vaya a portar a Android (`1xmanMAX/PIXPIN_PRO_ANDROID`) el asistente que ya tiene el
PC (`1xmanMAX/PIXPIN_PRO_WINDOWS`, rama `main`, 14-oct-2026). No hace falta haber visto el
código del PC. Cada apartado dice qué hace el PC, dónde está en Rust y qué tiene que hacer el
móvil para dar **el mismo resultado y sincronizar sin perder nada**.

Pedido del usuario: «un pequeño manual para que la gente que está haciendo la versión de
Android pueda implementarlo también de la misma forma y que sea compatible para sincronización
y todo eso».

Ficheros que acompañan a este manual (`docs/manual-asistente-android/`):

| Fichero | Qué es |
|---|---|
| `herramientas.json` | Las **45 herramientas** con su esquema JSON exacto, en la forma `tools` de OpenAI. Es lo que el PC le manda al modelo en la primera vuelta de un turno: el catálogo **ya pasado por `agente::con_topes(…, 16)`** (ver §2.4). |
| `sistema-claude.txt` | Las instrucciones de sistema tal como salen hacia Claude con el catálogo entero. |
| `casos-del-banco.json` | Los 42 pedidos del banco de pruebas, para medir lo mismo en el móvil. |

Se sacaron con la prueba que ya existe en el PC: `cargo test -p pixpin-ia --test banco -- --ignored --exact volcar_para_haiku`
(con `PIXPIN_IA_VOLCADO=<carpeta>`). Para regenerarlos después de un cambio en el PC, hay que repetir esa orden.

Revisado contra Android `main` en `5683c3b` (v0.124.0, 14-oct-2026).

---

## Índice

0. [Resumen en diez líneas](#0-resumen-en-diez-líneas)
1. [Arquitectura](#1-arquitectura)
2. [Catálogo común de herramientas](#2-catálogo-común-de-herramientas)
3. [Formatos que se sincronizan (lo más importante)](#3-formatos-que-se-sincronizan)
4. [Comportamiento de la UI](#4-comportamiento-de-la-ui)
5. [Algoritmos que hay que portar tal cual](#5-algoritmos-que-hay-que-portar-tal-cual)
6. [Pruebas de compatibilidad](#6-pruebas-de-compatibilidad)
7. [Pendientes conocidos](#7-pendientes-conocidos)
8. [Dónde encaja cada pieza en Android](#8-dónde-encaja-cada-pieza-en-android)

---

## 0. Resumen en diez líneas

1. El asistente es **un chat más**: el chat «Asistente», y `@asistente` desde cualquier otro chat.
   Sus respuestas son notas normales con `aparato = "IA"`. No hay un formato nuevo de mensaje.
2. Tiene tres motores con **un solo contrato**:
   - local: llama.cpp con GGUF (Qwen3.5-2B, o 0.8B como «ligero», y Gemma 4 E2B opcional para mirar fotos);
   - Claude `claude-haiku-5-5`;
   - DeepSeek `deepseek-flash`.
3. Las herramientas son **las mismas 45** en los dos aparatos, con el mismo JSON Schema y la misma
   validación. Lo que dibujan son **elementos de Excalidraw**, que el lienzo del móvil ya lee.
4. Lo que hace en la app se apoya en el **protocolo de pedidos** (`docs/protocolo-pedidos.md`):
   cada herramienta produce un pedido `{"pixpin":1,"accion":…}`.
5. Lo que piensa y su hoja de trabajo van en `pins/draw/anot-<uid>.asistente.json`, junto a su nota.
6. Las automatizaciones, las recetas y la biblioteca de figuras viajan como ficheros que señala un
   mensaje ARCHIVO de **id fijo** del chat «Asistente» (`auto-automatizaciones`, `auto-figuras`).
7. **Nunca viajan**: las claves de la API, el índice de documentos, el estado de las
   automatizaciones y los modelos.
8. **Bloqueante en Android hoy**: `Mensaje` y `Proyecto` tiran los campos que no conocen. Hay que
   añadir `lienzoIa` y `fuentesIa` a `Mensaje` antes de que el móvil reescriba notas del asistente
   (§3.1), o el PC las pierde.

---

## 1. Arquitectura

### 1.1 Las piezas (PC)

| Pieza | Rust | Qué hace |
|---|---|---|
| Contrato del motor | `crates/pixpin-ia/src/motor.rs` | `trait Motor { completar(&Peticion, al_trozo) -> Respuesta; soltar(); catalogo_completo() }`. La conversación tiene la forma de la API de chat de OpenAI. |
| Modelos locales | `modelos.rs`, `descarga.rs`, `servidor.rs` | Qué se baja, con su SHA-256. `llama-server` corre en un proceso aparte. |
| Relevo | `relevo.rs` | Varios modelos locales, **uno cargado a la vez**. |
| Nube | `nube/{mod,claude,deepseek,instrucciones,claves,http}.rs` | El mismo `Motor` sobre las API de Claude y DeepSeek. |
| Bucle de un turno | `agente.rs` | Pedir → validar → ejecutar → devolver el resultado → reintentar. |
| Herramientas | `herramientas.rs`, `acciones.rs`, `acciones_documentos.rs`, `svg.rs`, `funciones.rs`, `ecuacion.rs`, `cronograma.rs`, `figuras.rs`, `documentos/` | El catálogo común y la validación. |
| Selección por turno | `seleccion.rs` | Las 6 herramientas que vienen a cuento (solo con el modelo local). |
| Memoria | `pensar.rs`, `hoja_trabajo.rs`, `contexto.rs` | Pensamiento, petición mejorada, hoja de trabajo y reinicio al 70 %. |
| Al lienzo | `lienzo.rs`, `calcar.rs`, `svg.rs` | De una llamada validada a elementos de Excalidraw. |
| App | `apps/pixpin/src/asistente/**` | Ejecutor, chat, burbuja en vivo, lienzo propio, documentos, recetas. |
| UI | `apps/pixpin/src/ventana_chat/{asistente,pensado,voz_del_asistente,aviso_asistente}.rs` | La ventana. |
| Automatizaciones | `apps/pixpin/src/automatizaciones*.rs` | Disparadores y pasos. |
| Figuras | `apps/pixpin/src/biblioteca_figuras.rs` | La biblioteca de figuras, viajando por el chat «Asistente». |

### 1.2 El contrato (`motor.rs`)

La conversación sigue la forma de `/v1/chat/completions` de OpenAI, así que cambiar de motor no toca
ni el bucle ni el chat:

```text
Mensaje { rol: system|user|assistant|tool, partes: [Texto | Imagen{bytes,mime}],
          llamadas: [LlamadaCruda{id, nombre, argumentos(texto JSON, puede venir roto)}],
          id_llamada (solo en rol tool) }
Peticion { mensajes, herramientas, max_tokens (1024), temperatura (0.7), pensar (false) }
Trozo    = Texto(s) | Pensando(s) | Llamando{nombre}      // streaming
Seguir   = Si | Parar                                     // botón «parar»
Respuesta{ texto, llamadas, fin: Natural|Herramientas|Longitud|Cortado, pensamiento, medidas }
```

- La imagen va **antes** del texto en el mensaje del usuario (lo recomienda Qwen). La foto se reduce
  antes a unos **768 px** de lado (`apps/pixpin/src/asistente/foto.rs`).
- El cuerpo que va a llama.cpp (`Peticion::a_json`):
  `temperature 0.7, top_p 0.8, top_k 20, presence_penalty 1.5, stream, chat_template_kwargs.enable_thinking`.
- **En Android**: una interfaz Kotlin `Motor` con `completar(peticion, alTrozo: (Trozo) -> Seguir): Respuesta`,
  que se llama desde una corrutina en `Dispatchers.IO`, y `soltar()`.

### 1.3 Motores

#### Local: llama.cpp y GGUF

Son los mismos ficheros, con la misma huella, en el PC y en el móvil (`modelos.rs`):

| id | Fichero | Bytes | SHA-256 |
|---|---|---|---|
| `qwen3.5-2b` (por omisión) | `https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf` | 1 280 835 840 | `aaf42c8b7c3cab2bf3d69c355048d4a0ee9973d48f16c731c0520ee914699223` |
| (visión del 2B) | `https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/mmproj-F16.gguf` | 668 227 264 | `7035e9cb8d7c6a9681d07eef9a364783e86ea4cd73faab2eabb4f43a101830c7` |
| `qwen3.5-0.8b` (ligero) | `https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/Qwen3.5-0.8B-Q4_K_M.gguf` | 532 517 120 | `bd258782e35f7f458f8aced1adc053e6e92e89bc735ba3be89d38a06121dc517` |
| (visión del 0.8B) | `https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/mmproj-F16.gguf` | 204 987 232 | `56e4c6cfe73b0c82e3e82bc518d7591997e61d81f723fc41a586f4fa69ea2453` |
| `gemma-4-e2b` (mejor vista, opcional) | `https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF/resolve/0314792d7f1f7e229411f620751375812bb9faf2/gemma-4-E2B-it-Q4_K_M.gguf` | 3 106 738 272 | `740185b21d22ceb83a11c3aa62ad5842ef32c70f6096d756bbee85a1e4ec34b8` |
| (visión de Gemma) | `https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF/resolve/0314792d7f1f7e229411f620751375812bb9faf2/mmproj-F16.gguf` | 985 654 080 | `140be8d7849741f88c50757d529b84373ee8e27052cc2236855b537f4a8215fa` |

- Todo es Apache-2.0. El motor del PC es llama.cpp `b11580`, con el zip de Windows y Vulkan
  (`6e520f0b…dff2`). En Android: el binario `llama-…-bin-android-arm64` de la misma versión, o
  llama.cpp compilado con el NDK por JNI.
- La descarga se hace a un `.parte`, se comprueba la huella, se renombra y se deja una marca `<fichero>.ok`.
  Si la huella no coincide, se borra. Va **siempre fuera de la copia de seguridad**: en el PC a
  `%LOCALAPPDATA%\PixPinMax\modelos`; en Android a `noBackupFilesDir`, y nunca a `filesDir/pins`
  ni a nada que sincronice.
- Siempre **sin pensar** (`enable_thinking: false`), salvo en la vuelta de planear si el ajuste
  `pensar` está encendido. El 0.8B entra en bucles cuando piensa.
- Contexto de **8192** tokens (`servidor::CONTEXTO`). Se carga en la primera petición y se suelta
  a los **5 min** sin uso (`Config::reposo`). Matar el proceso devuelve toda la memoria.
- Medido en el PC: el 2B llama a las herramientas bien y ve bien (~2,9 GB de RAM). El 0.8B **no
  sirve con herramientas** y su visión inventa; es solo para conversar en equipos flojos. Por omisión:
  0.8B si el aparato tiene < 8 GB de RAM y 2B en el resto (`RAM_PARA_EL_2B_MB = 8192`).
  Plan B solo para Android si va lento: LiteRT-LM con Gemma 4 E2B, sin tocar el protocolo.

#### Relevo de un solo modelo (`relevo.rs`)

Si están bajados Qwen y Gemma, **nunca hay dos cargados a la vez**.

- Antes de contestar, un motor del relevo **suelta al que contestó el último** y entonces carga el suyo.
- Contestan de uno en uno: un mutex hace de «turno».
- Gemma se usa **solo** cuando hay una foto y se pide *mirarla*: describir, leer lo que pone, contar…
  Lo decide `seleccion::pide_mejor_vista(pedido, con_foto)`. Para dibujar, calcar o mandar algo a la
  app se usa siempre el de siempre.

#### Nube (`nube/`)

| | Claude | DeepSeek |
|---|---|---|
| Modelo | `claude-haiku-5-5` («Claude Haiku 5.5») | `deepseek-flash` (sirve DeepSeek-V4.1-Flash) |
| URL | `https://api.anthropic.com/v1/messages` (`anthropic-version: 2023-06-01`) | `https://api.deepseek.com/chat/completions` |
| Pensar | `thinking: {type:"adaptive", display:"summarized"}`, esfuerzo `medium` | `thinking: {type: enabled}`, esfuerzo `high`. Hay que **devolver el `reasoning_content`** en las vueltas siguientes; si contesta 400 por eso, se repite sin pensar. |
| Muestreo | **sin** temperature/top_p/top_k (dan 400 en Haiku 5.5) y sin prefill | sin temperature/top_p (no tienen efecto pensando) |
| Herramientas | `name`, `description`, `input_schema` y `eager_input_streaming: true`; **sin `strict`** | forma OpenAI; **sin modo estricto** (solo existe en la URL beta y obliga a que todos los campos sean obligatorios) |
| Tope de salida | 16 000 (el pensamiento cuenta dentro; se suman 8192 de reserva a lo que pide el agente, sin pasar de 16 000) | 16 000 |
| Caché | un punto al final del `system` y otro en el último bloque del último mensaje | por prefijo; el sistema sale igual byte a byte |
| Claves | `https://platform.claude.com/settings/keys` | `https://platform.deepseek.com/api_keys` |

**Por qué sin `strict` en Claude (hallazgo medido el 13-oct con la API real).** Con `strict: true`,
Haiku 5.5 dibujaba figuras sin tamaño ni texto (`{"tipo":"rectangulo","x":0,"y":0}`) y números
deformes, y el banco caía al **50 %** en los dibujos. Sin `strict`, el **100 %**. La decodificación
restringida sigue el orden del esquema (alfabético), y tras «tipo» solo le quedaban «x» e «y».
La validación estricta la hace igual `herramientas::validar` antes de ejecutar. Las grabaciones están
en `crates/pixpin-ia/src/nube/grabaciones/casa-{con,sin}-strict.sse`, y la prueba es
`la_casa_sin_strict_trae_tamanos_y_colores_y_la_de_strict_no`. **Android: no pongas `strict`.**

Otras reglas de la nube que hay que copiar:

- **Reintentos** (`ESPERAS = [1 s, 4 s]`, respetando `retry-after` hasta 20 s): con 429, 5xx, 529,
  `overloaded_error` o un corte de red, y **solo si aún no se enseñó nada** en la burbuja, para no
  duplicar texto.
- **Errores en palabras del usuario** (`nube::clasificar`, copiar los textos):
  - 401: clave mala;
  - 402 o «credit balance»: sin saldo;
  - 403: sin permiso;
  - 404: modelo retirado («Actualiza PixPin»);
  - 413: foto demasiado grande.
- **`MotorConRespaldo`**: si la nube falla con un error de nube antes de enseñar nada y hay modelo
  local, la burbuja dice `⚠️ <Proveedor>: <aviso>\nSigo con el asistente local.` y contesta el local.
  Durante **3 min** (`PAUSA_TRAS_FALLO`) se va directo al local. Se controla con el ajuste `respaldo_local`.
- Una respuesta cortada por `max_tokens` con una llamada a medias **no se ejecuta**: vuelve como
  llamada rota.
- Un `refusal` vacía las llamadas y añade «<Proveedor> no quiso hacer esto. Prueba a pedirlo de otra forma.».
- Con la nube, `catalogo_completo() == true`: se mandan **todas** las herramientas en cada turno
  (prefijo estable que se cachea). Con el local se eligen 6 (§2.2).

### 1.4 Cómo se elige el motor

Los ajustes del PC (`[asistente]` en el TOML, `crates/pixpin-store/src/ajustes.rs::Asistente`):

| Clave | Valores | Uso |
|---|---|---|
| `motor` | `""`/`local`, `claude`, `deepseek` | Quién contesta. |
| `modelo` | `qwen3.5-2b` (vacío = este), `qwen3.5-0.8b` | El local de siempre. |
| `mejor_vista` | bool, apagado | Gemma para mirar fotos. |
| `respaldo_local` | bool, encendido | Seguir con el local si la nube falla. |
| `pensar` | `None` = automático (la nube sí, el local no), true/false | Pensar antes de contestar. |
| `documentos_a_la_nube` | `""` = preguntar, `si`, `no` | Si se mandan trozos de documentos a la nube (§4.8). |
| `revisar_lo_dicho` | bool | El dictado se queda en la caja, sin enviarse. |
| `leer_respuestas` | bool | Leer cada respuesta en voz alta al acabar. |

Estos ajustes **son de cada aparato y no viajan**. En Android: DataStore `pixpin_settings`
(`data/SettingsRepository.kt`), con las mismas claves.

### 1.5 Claves de la API: cifradas y nunca sincronizadas

- En el PC (`nube/claves.rs`): DPAPI de usuario, con una entropía propia (`PixPinMax/asistente/nube/v1`).
  Cada clave va en su fichero `%LOCALAPPDATA%\PixPinMax\claves\<claude|deepseek>.clave`: nunca en
  el TOML, nunca en la carpeta que sincroniza o se exporta, nunca en el registro de la app. Para
  enseñarla se usa `enmascarar` (`sk-ant-…AbCd`) y `parece_clave` (16-400 caracteres imprimibles).
- **En Android**: una clave AES-GCM del **Android Keystore** (no exportable), y la clave de la API
  cifrada con ella en un fichero propio en `noBackupFilesDir` (o `EncryptedSharedPreferences`).
  Ojo: el manifiesto tiene `android:allowBackup="true"` sin reglas, así que hay que excluir ese
  fichero con `dataExtractionRules`/`fullBackupContent`. **Nunca** en DataStore en claro, ni en
  `filesDir/pins`, ni en un `.pixpin`, ni en el registro de errores (`crashlog`).
- El usuario pega la clave en cada aparato. No hay forma de pasarla de uno a otro, y eso es a propósito.

---

## 2. Catálogo común de herramientas

### 2.1 La lista

Los esquemas exactos están en `docs/manual-asistente-android/herramientas.json`. Con nombre y qué
produce en datos:

**Lienzo** (las que dibujan producen elementos de Excalidraw; §3.4):

| Herramienta | Obligatorio | Resultado |
|---|---|---|
| `dibujar_en_lienzo` | `elementos[]` (`tipo` + `caja`/`centro`+`radio`/`puntos`/`x`,`y`…) | Elementos `rectangle`, `ellipse`, `diamond`, `line`, `arrow`, `freedraw`, `text`, `pixpin-region`, `pixpin-point`, `pixpin-axes`… |
| `grafica` | `tipo` (barras/lineas/pastel), `etiquetas[]`, `valores[]` | Rectángulos, rayas y textos editables; el pastel con `pixpin-region` |
| `dibujar_svg` | `svg` | El SVG convertido en figuras (`svg.rs`) |
| `graficar_funcion` | `funciones[]` (fórmulas en x) | El instrumento `pixpin-axes` con sus curvas |
| `escribir_ecuacion` | `ecuacion` | La ecuación compuesta del motor del lienzo |
| `cronograma` | `tareas[]` (`nombre`, `inicio`) | `pixpin-gantt` |
| `calcar_imagen` | — (la última foto) | `pixpin-region` (manchas) o `line` (contornos); §5.1 |
| `figuras_guardadas` | `accion` guardar/poner/lista | Lee o escribe `figuras.json`; §3.7 |

**Leer y buscar**:

| Herramienta | Resultado |
|---|---|
| `ocr` | Texto del adjunto. En el chat queda una nota y un `<nombre> (texto).txt` con `aparato = "IA"`. |
| `ayuda_app` | Contesta con `conocimiento.rs` (índice de la ayuda). |
| `buscar_en_mis_cosas` | BM25 sobre documentos y chats; citas `[n]` → `fuentesIa` (§3.1, §5.4). |
| `buscar_historial` | **No está en el catálogo**: solo se añade cuando hay memoria (§2.5). |

**App** (tabla `acciones.rs`; cada una arma un pedido del protocolo):

| Herramienta | Pedido | Confirmar |
|---|---|---|
| `abrir_chat` | `ventana_principal` | |
| `abrir_fichero` | `abrir {que:{tipo:"fichero",ruta}}` | |
| `abrir_mensaje` | `abrir {que:{tipo:"mensaje",codigo}}` | |
| `abrir_nota` | `abrir {que:{tipo:"nota",codigo}}` | |
| `abrir_hoja` | `abrir {que:{tipo:"hoja",referencia}}` | |
| `escribir_en_chat` | `chat {texto}` | |
| `nota_nueva` | `nota_nueva {texto?}` | |
| `lienzo_nuevo` | `lienzo_nuevo {nombre?}` | |
| `grabar_voz` | `grabar {nombre}` | |
| `reproducir_audio` | `reproducir {ruta,titulo?}` | |
| `lista_nueva` | `lista_nueva {titulo}` | |
| `anadir_tarea` | `anadir_tarea {texto,codigo?}` (sin código: Inbox) | |
| `marcar_tarea` | `marcar_tarea {codigo,indice,hecha=true}` | |
| `mover_tarea` | `mover_tarea {codigo,indice,a_codigo}` | |
| `quitar_tarea` | `quitar_tarea {codigo,indice}` | «¿Quito la tarea {indice} de la lista {codigo}?» |
| `borrar_lista` | `borrar_lista {codigo}` | «¿Borro la lista {codigo} entera, con todas sus tareas? No se puede deshacer.» |
| `abrir_ventana` | `ventana {cual: tareas/galeria/lecciones/sincronizar}` | |
| `capturar_pantalla` | `capturar {modo:"zona"}` | |
| `pinear_ultima_captura`, `pinear_fichero` | `pinear_ultima`, `pinear {ruta}` | |
| `conservar_captura`, `borrar_captura`, `copiar_imagen` | iguales | `borrar_captura`, sí |
| `caja_para_soltar` | `soltar` | |
| `leccion_nueva`, `buscar_lecciones` | `leccion_nueva`, `lecciones` | |
| `sincronizar_movil`, `enviar_al_movil` | `sincronizar`, `enviar_al_movil` | `enviar_al_movil`, sí (sale del aparato) |
| `crear_automatizacion` | `automatizacion {operacion:"crear",receta}` | sí |

Para la columna «Confirmar» manda el código: `crates/pixpin-ia/src/acciones.rs`, campo `confirmar`
de cada fila de `ACCIONES`. Lo que **borra o saca algo del aparato** pregunta siempre (§4.3).

**Documentos** (`acciones_documentos.rs`, que ejecuta `apps/pixpin/src/asistente/documentos_acciones.rs`):

| Herramienta | Qué hace en datos |
|---|---|
| `tabla_de_imagen` | OCR con posiciones → `tabla_escaneada` → pedido `tabla_nueva` (mini-app `tabla`) o un `.xlsx` al chat (§3.6) |
| `anotar_documento` | Escribe figuras en la **misma capa** que el lector o el visor de planos (§3.8) y abre ahí |
| `buscar_en_documento` | Pedido `abrir` con `pagina`+`buscar` (PDF/Office) o `zona:[x,y,ancho]` (plano) |
| `abrir_por_nombre` | Busca el adjunto por nombre o tema; si hay varios, se lo dice al modelo para que pregunte |
| `exportar_documento` | El componente «Exportar» común; lo hecho entra al chat |

Regla de la casa: el modelo **nunca pide rutas ni códigos** al usuario. Nombra el archivo con las
palabras del usuario y la app lo resuelve.

### 2.2 Selección por turno (solo modelo local)

Lo hace `seleccion::para_el_pedido(pedido, con_imagen)`, con **6 como mucho**. Con 45, un 2B se
pierde y el catálogo solo ya se come ~3000 tokens.

1. Pregunta de ayuda («cómo…», «dónde…», «qué es…»): solo `ayuda_app`. Excepción: si habla de
   sus documentos (`es_sobre_sus_documentos`) o de buscar dentro de uno, van primero esas
   herramientas, luego `buscar_en_mis_cosas` y `ayuda_app`.
2. Con foto y un pedido sobre ella (`pide_la_imagen`), sin hablar de tabla ni Excel:
   - si pide *interpretarla* («con figuras», «a lápiz», «grafito», «boceto»…), `dibujar_svg` y `dibujar_en_lienzo`;
   - si no, **solo `calcar_imagen`**. Medido: con las dos, el 2B dibujaba 4 triángulos en vez de calcar.
3. Charla («hola», «gracias», ≤ 4 palabras): ninguna herramienta.
4. Lo demás se puntúa por claves (`claves_base` y el campo `claves` de cada acción):
   - frase con espacio o `:`: 3 puntos;
   - palabra que empieza por la raíz: 2 puntos;
   - se quedan las de ≥ 2 puntos, por orden;
   - sin ninguna: `dibujar_en_lienzo`, `grafica`, `ayuda_app`.

Antes de comparar, el pedido se normaliza con `acciones::normalizar` (minúsculas y sin tildes).
**Copia las listas de claves tal cual**: son datos, no lógica.

La vuelta de **planear** (§2.5) puede añadir herramientas del catálogo entero (`Opciones::ampliar`).

### 2.3 Validación (`herramientas::validar`) y reintentos (`agente.rs`)

**La estructura es estricta**:

- nombre conocido, tipos y obligatorios;
- coordenadas entre −200 y 2000, medidas entre 1 y 2000, letra entre 6 y 200;
- como mucho 60 figuras, 40 datos y 240 números en `puntos`;
- tantas etiquetas como valores, y el pastel sin negativos;
- páginas válidas.

**Lo cosmético es tolerante**:

- un color desconocido pasa a negro, con un aviso;
- `"120"` y `"20px"` valen como números;
- sinónimos: «círculo», «circle», «bar».

Si el modelo no puso color, se saca del pedido (`colores_del_pedido`).

**Antes de validar**:

- `agente::sanear` lleva al borde las coordenadas que se salen un poco;
- `herramientas::reparar_json` intenta arreglar una vez el JSON roto: vallas de Markdown, texto
  alrededor, comas colgantes.

**Si falla**, la llamada **no se ejecuta** y el modelo recibe el motivo en un mensaje `tool` que
empieza por `ERROR:`. Por ejemplo:

- `ERROR: la herramienta «x» no existe. Las que hay son: …`
- `ERROR: los argumentos no son JSON valido. …`

**El bucle**:

- `VUELTAS = 4` vueltas modelo → herramienta → modelo, más `REINTENTOS = 2`.
- Una llamada cortada por tokens (JSON roto, `fin == Longitud`, o el HTTP 500 «Failed to parse
  tool call arguments» de llama-server) baja el tope de listas a `TOPE_LISTAS_REINTENTO = 10` y le
  dice al modelo: «ERROR: tu llamada a la herramienta salio cortada porque era demasiado larga.
  Vuelve a llamarla con 10 elementos como mucho, …».
- Al acabar sin texto:
  - si hizo algo, «Hecho.»;
  - si no hizo nada, «No sé qué decir sobre eso. ¿Qué quieres que haga?»;
  - si se agotaron las vueltas con algo hecho, «Hecho en parte: lo demas no me salio con las herramientas.»;
  - sin nada hecho, «No he sabido hacerlo con las herramientas; prueba a pedirlo de otra forma o por partes.»

**El resultado al modelo**: el texto que devuelve el ejecutor («dibujadas 4 figuras…»), más
` (avisos: …)` si hubo arreglos cosméticos. Con Claude, un resultado que empieza por `ERROR` lleva
`is_error: true`.

### 2.4 Topes por petición (`agente::con_topes`, `tope_de_tokens`)

En **cada** petición, local y nube, el catálogo pasa por `con_topes(herramientas, tope)`:

- toda lista de objetos lleva `maxItems ≤ tope`, con tope 16 y 10 tras un corte;
- las listas de números o textos, ≤ 2·tope;
- todo `string` sin `enum`, `maxLength ≤ 120`.

`llama-server` convierte el esquema en una gramática, y así una llamada no crece hasta cortarse.
`max_tokens` = lo que quepa en 8192: se estima a 3 letras por token y 1000 por foto, y se acota
entre 512 y 3072. Por eso `herramientas.json` lleva `maxItems: 16`/`32` y `maxLength: 120`: es lo
que ve el modelo. La **validación** usa los límites de verdad (60 figuras, etc.), no estos.

> ⚠️ Aviso detectado al escribir este manual: el tope de 120 letras cae también sobre `svg` de
> `dibujar_svg`, `pasos` de `crear_automatizacion` y `texto` de `nota_nueva`. Con la gramática
> de llama.cpp eso **corta** un SVG a 120 letras. Con Claude, sin `strict`, es solo una pista. Está
> apuntado en §7 para el PC. En Android, copia la función igual (para dar los mismos resultados)
> y aplica el arreglo cuando lo haga el PC.

### 2.5 Instrucciones base, variante por proveedor y memoria

**Prompt base**: `agente::instrucciones()` + `conocimiento::RESUMEN`. Es el mismo para los tres
motores; el texto completo es la sección `<rol>` de `sistema-claude.txt`.

**Reglas comunes**: `nube/instrucciones.rs::REGLAS`, con variables `{ancho}`, `{alto}`,
`{max_figuras}`, `{max_datos}` y `{n_herramientas}`. Cada regla solo sale si está su herramienta.

**Presentación por proveedor** (`instrucciones::para`):

| Proveedor | Forma |
|---|---|
| Claude | `<rol>…</rol>`, `<entorno>…</entorno>`, `<reglas>- …</reglas>` y la línea final «Las reglas de este mensaje valen para toda la conversacion…». Sin fecha, para que el sistema se cachee. |
| DeepSeek | base + `# Entorno` + `# Reglas` numeradas y el recordatorio «Recuerda: las herramientas se usan con una llamada de funcion (tool_calls)…» |
| Local | el base tal cual en el primer `system`, con la hoja y el plan debajo (`contexto::sistema_con_hoja`) |

**Memoria del turno** (`agente::Opciones`, `apps/pixpin/src/asistente/turno.rs::Memoria`):

- **Planear** (petición mejorada). Hay una vuelta corta antes de actuar:
  - sistema `pensar::instrucciones_del_plan`, que pide cinco líneas `ENTIENDO:` / `HARE:` / `HERRAMIENTAS:` / `DATOS:` / `DECIDIDO:`;
  - tope 320 tokens (640 si piensa) y temperatura 0.6;
  - lo lee `Plan::leer`, tolerante con mayúsculas, tildes y Markdown;
  - si no sale el plan, se usa `Plan::por_omision(pedido)`;
  - si pensó hasta agotar el tope, hay una segunda vuelta sin pensar.
- **Hoja de trabajo**: va en el sistema (§3.2).
- **Contexto al 70 %**: §5.5.
- **`buscar_historial`**: busca en la conversación guardada. Vale **una vez por turno**; después
  se quita. Medido: si no, el 2B la llama seis veces seguidas.

---

## 3. Formatos que se sincronizan

> Regla general: **nada de formatos nuevos en el protocolo**. Todo va en mensajes del chat, en
> ficheros `pins/draw/…` o `guardados/…` que ya viajan, y en el `resto` de las cosas que ya
> existen. La sincronización del PC y la del móvil (`Disco.kt`, `Fusion.kt`) no cambian.

### 3.1 Respuestas del asistente: notas con `aparato = "IA"`

- Cada respuesta es **una nota normal** (`clase: "NOTA"`), con `aparato: "IA"` y su `numero`. La
  chapa sale `#48·IA`. La pregunta del usuario es una nota normal con su aparato (p. ej. `K7Q2`).
- En el chat «Asistente», la respuesta va suelta. En otro chat (`@asistente`), lleva
  `respondeA: <id de la pregunta>`.
- La nota nace en cuanto se pregunta, con el texto «💭 …», y se reescribe en el disco **cada 2 s**
  mientras contesta (`GUARDAR_CADA`) y al acabar. Una sincronización a mitad puede llevarse un texto
  a medias, que se arregla en la vuelta siguiente: es la fusión de párrafos de siempre.
- Al acabar se añaden **campos extra en la raíz del mensaje**. En el PC van en `resto`, el
  `#[serde(flatten)]` del mensaje:
  - `lienzoIa`: la `referencia` del lienzo donde dibujó. De ahí sale la tarjeta «Abrir lienzo».
  - `fuentesIa`: las citas, `[{n, titulo, proyecto, pedido}]` (`documentos::herramienta::cita_a_json`).
    De ahí sale «Abrir fuente».

Ejemplo de dos líneas de `guardados.jsonl` del chat «Asistente», en la forma portátil que viaja:

```json
{"id":"1792044000123","cuando":1792044000123,"clase":"NOTA","texto":"Dibújame una casa roja y ábrela","proyecto":"pr-1791999000000","numero":47,"uid":"7G5G8874YS","aparato":"K7Q2"}
{"id":"1792044000456","cuando":1792044000456,"clase":"NOTA","texto":"Listo: dibujé la casa roja en «Lienzo del asistente».","proyecto":"pr-1791999000000","numero":48,"uid":"Q2W7X1KD9P","aparato":"IA","lienzoIa":"3DSRX435ZU"}
```

Y una respuesta que citó documentos:

```json
{"id":"1792044100000","cuando":1792044100000,"clase":"NOTA","aparato":"IA","numero":50,"uid":"Z2C3WQFTHJ","proyecto":"pr-1791999000000",
 "texto":"La cuneta del km 12 queda pendiente de revisión de cotas [1].",
 "fuentesIa":[{"n":1,"titulo":"Informe cunetas.pdf · pág. 12","proyecto":"Obra",
   "pedido":{"pixpin":1,"accion":"abrir","que":{"tipo":"fichero","ruta":"F:\\Obra\\Informe cunetas.pdf","pagina":12}}}]}
```

**⚠️ Bloqueante en Android.** `Mensaje` (`guardados/Mensajes.kt`) se lee con `ignoreUnknownKeys`
y **no conserva** lo que no conoce. Si el móvil vuelve a escribir una de estas notas (al fijarla,
reaccionar o transcribir), el campo desaparece; en la vuelta siguiente la fusión de tres bandas lo
toma por un borrado en el móvil y **el PC también lo pierde**. Hay que hacer una de dos cosas:

- añadir a `Mensaje` `val lienzoIa: String? = null` y `val fuentesIa: JsonArray? = null`, como
  mínimo; o
- (mejor y para siempre) un `resto: JsonObject` que se rellene al leer y se escriba al guardar,
  como ya hace `TablaDeCalculo.aJsonSobre`.

Notas para leer `fuentesIa` en el móvil:

- `pedido.que.tipo == "mensaje"` (`proyecto`, `codigo`) se abre igual en los dos aparatos.
- `tipo == "fichero"` lleva una **ruta absoluta del PC**. En Android, busca el adjunto por el
  nombre del fichero entre los mensajes con `ruta` (el último segmento de `que.ruta`) y ábrelo en
  `pagina`; si no está, enseña la tarjeta sin acción.
- Las citas que cree el móvil deben usar la ruta portátil `pixpin:files/…` cuando la haya, para que
  el PC también las abra (el PC ya traduce `pixpin:files/` con `vista::ruta_real`).

Otros mensajes que deja el asistente, todos con `aparato: "IA"`:

| Qué | Mensaje |
|---|---|
| Su lienzo propio | `clase: "DIBUJO"`, `nombre: "Lienzo del asistente"`, `referencia: <id>`, `ruta: "lienzos/<id>.excalidraw"` (en el móvil, `pins/draw/<id>.excalidraw.gz`) |
| Texto de un OCR | Una nota con el texto y un `ARCHIVO` `«<nombre> (texto).txt»` |
| Tabla escaneada | La mini-app `tabla` (§3.6). La escribe el pedido `tabla_nueva` con el aparato del equipo, no con «IA». |

En el índice de documentos (§5.4), **las notas con `aparato == "IA"` no se indexan**: así el
asistente no se cita a sí mismo.

### 3.2 Lo que pensó y la hoja de trabajo: `pins/draw/anot-<uid>.asistente.json`

Uno por respuesta, con el **código único de su nota** (`Codigos.unico(m)` = su `uid`). Es la
terminación `ASISTENTE = ".asistente.json"` de `crates/pixpin-sincro/src/anotado.rs`, que es la
lista `TERMINACIONES` de `AnotacionesDelAdjunto.kt` más esta. Viaja con su nota y **se borra con
ella**, igual que `.comentarios.json`. En el PC está en `proyectos/<chat>/android/pins/draw/`;
en el móvil, en `files/pins/draw/`. Es JSON en claro, sin gzip, escrito de forma atómica.

```json
{
  "version": 1,
  "pensamiento": "El usuario quiere una casa roja. Uso dibujar_en_lienzo con un rectangulo y un triangulo rojos…",
  "segundos": 12.5,
  "plan": {"entiendo": "Dibujar una casa roja en el lienzo.", "hare": "llamar a dibujar_en_lienzo con figuras rojas",
           "herramientas": ["dibujar_en_lienzo"], "datos": ["color rojo"], "decidido": [], "del_modelo": true},
  "hoja": {
    "objetivo": "Dibujar una casa roja en el lienzo.",
    "hechos": ["color rojo"], "hecho": ["T1: [dibujar_en_lienzo] Listo: dibujé la casa roja…"],
    "falta": [], "decisiones": [],
    "turnos": [{"n": 1, "peticion": "Dibujar una casa roja en el lienzo.", "pensado": "El usuario quiere una casa roja. … ",
                "resultado": "Listo: dibujé la casa roja…"}],
    "reinicio_desde": null, "reinicios": 0
  },
  "reinicio": false
}
```

Reglas (`hoja_trabajo.rs`):

- **Topes**: hechos 8, hecho 6, falta 4, decisiones 6, turnos 4; 160 letras por línea, 220 el
  pensamiento resumido y 20 000 el pensamiento entero. Al leer se **recortan**: una hoja escrita a
  mano no puede llenar el contexto.
- Una `version` distinta de 1 o un fichero roto **no se lee a medias**: se ignora y se mira la
  respuesta anterior; si no hay ninguna, la hoja empieza vacía.
- **La hoja del chat** es la del registro de la **última** respuesta que lo tenga.
- `reescribir(plan, resumen_del_turno)` cada turno:
  - objetivo = `plan.entiendo`;
  - `datos` → hechos y `decidido` → decisiones, sin repetir;
  - una línea `T<n>: [herramientas] primera línea de la respuesta`, más ` (sin acabar)` si quedó incompleta;
  - `falta` se vacía cuando el turno acaba bien.
- `resumir(pensamiento)` = primera frase `…` última frase, sin llamar al modelo.

Cómo se junta: es un fichero suelto, así que gana el tocado más tarde, como un `.leccion`. Cada
respuesta tiene el suyo y solo lo escribe quien la escribió.

### 3.3 El chat fijo «Asistente»

- **En el PC** es un proyecto con la marca `asistente: true` en el `resto` de su ficha
  (`crates/pixpin-proyecto/src/almacen.rs::asegurar_asistente`). Se llama «Asistente», sale arriba
  en la lista y renombrado sigue siendo él.
- **Lo que viaja hoy**: el proyecto portátil **no lleva la marca**. `vista::proyecto_local` solo
  pone `id`, `nombre`, `uid`, `creado`, `aparato` y `archivado`, porque Android reescribe el
  proyecto sin los campos que no conoce. El móvil recibe un chat normal llamado «Asistente».
- **Qué hacer en Android, para no tener dos chats «Asistente»**:
  1. Reconocer como chat del asistente el proyecto que tenga un mensaje con id
     `auto-automatizaciones` o `auto-figuras`, o si no, el que se llame «Asistente» y tenga notas con
     `aparato: "IA"`. Crear uno solo si no hay ninguno.
  2. Añadir `asistente: Boolean = false` a `Proyecto` (`motor/Proyectos.kt`) y **conservarlo**. Cuando
     Android lo conserve, el PC podrá mandarlo (se cambia en `vista::proyecto_local` y se avisa aquí).
- **No se comparte nunca con personas**: por eso lleva lo que solo es de los aparatos propios
  (§3.5, §3.7).

### 3.4 Lo dibujado: elementos de Excalidraw (`lienzo.rs`)

- Las coordenadas del modelo son de un lienzo de **800 × 600**, con el origen arriba a la izquierda.
- `Fabrica` da ids `ia-<semilla hex>-<n>` y grupos `ia-<semilla>-<que>-<n>`. Todo elemento lleva
  `customData: {"origen":"pixpin-ia"}`, `version: 1`, `roughness: 1`, `strokeWidth: 2`,
  `fillStyle: "solid"` y `backgroundColor: "transparent"` si no tiene relleno.
- Los colores con nombre se traducen a la paleta de Excalidraw (`herramientas::color`). Series de
  gráficas: `#1971c2 #e03131 #2f9e44 #f08c00 #6741d9 #0c8599 #c2255c #868e96`.
- Los puntos van relativos al primero. El PC los escribe como `[x,y]` y, al mandarlos al móvil, los
  pasa a `{"x":…,"y":…}` (`crates/pixpin-proyecto/src/para_el_movil.rs`). En Android basta con
  escribirlos como los escribe hoy `Element`.
- Solo se usan `type` que el móvil sabe leer (`TIPOS_DEL_MOVIL`): `rectangle`, `diamond`, `ellipse`,
  `arrow`, `line`, `freedraw`, `text`, `image`, `frame` y los `pixpin-*`. Un `type` desconocido tira
  la escena entera en kotlinx.

**Adónde va lo dibujado** (`apps/pixpin/src/asistente/al_lienzo.rs`):

| Situación | Destino |
|---|---|
| Hay un lienzo abierto | Ahí, centrado en la vista |
| `@lienzo:<Nombre_con_guiones>` en la pregunta | En ese lienzo del chat, sin abrirlo |
| Si no | En el **lienzo propio del asistente** de ese chat: el último `DIBUJO` con `aparato == "IA"`. Si no existe, se crea con el nombre «Lienzo del asistente». |

- Sin un lienzo abierto, lo nuevo va **a la derecha de lo que ya había**, para no taparlo
  (`meter_al_lado`).
- Siempre en **un solo paso de deshacer**, y guardado por el camino de siempre del lienzo, que es
  lo que mira la sincronización.
- Si la pregunta pedía abrirlo («…y ábrelo», «muéstramelo»: `chat::pide_abrir`), se abre al
  acabar. Si no, la respuesta lleva `lienzoIa` y su tarjeta «Abrir lienzo».

### 3.5 Automatizaciones y recetas: `automatizaciones.json`

| | PC | Móvil |
|---|---|---|
| Fichero | `proyectos/<chat Asistente>/android/guardados/asistente/automatizaciones.json` | `files/guardados/asistente/automatizaciones.json` |
| Ruta portátil | `pixpin:files/guardados/asistente/automatizaciones.json` | la misma |
| Mensaje que lo hace viajar | `ARCHIVO`, **id `auto-automatizaciones`**, `nombre: "automatizaciones.json"`, `ruta` = la portátil, texto «⚙️ Las automatizaciones del asistente (se cambian en Ajustes → Asistente)» | el mismo, **uno solo** (si ya está, no se toca) |

Formato: JSON en camelCase (`apps/pixpin/src/automatizaciones.rs`):

```json
{ "version": 1,
  "automatizaciones": [
    { "id": "auto-1789412424738-0", "nombre": "Capturas a la obra con OCR", "activa": true,
      "disparador": { "tipo": "al_capturar" },
      "pasos": [
        { "tipo": "accion", "herramienta": "escribir_en_chat", "args": { "texto": "Captura {fecha} {hora}" },
          "proyecto": "Obra", "imagenes": ["{captura}"] },
        { "tipo": "asistente", "pedido": "Hazle OCR a la captura", "imagenes": ["{captura}"] }
      ],
      "aparato": "K7Q2", "creada": 1789412424738, "tocada": 1789412424738 },
    { "id": "auto-1760390000000-0", "nombre": "Resumir", "activa": true,
      "disparador": { "tipo": "a_mano" },
      "pasos": [ { "tipo": "asistente", "pedido": "Resume lo último en 5 puntos" } ],
      "creada": 1760390000000, "tocada": 1760390000000,
      "boton": true, "icono": "📝", "orden": 0 }
  ] }
```

**Disparadores**:

| `disparador.tipo` | Cuándo salta |
|---|---|
| `al_capturar` | Al guardarse una captura nueva |
| `a_la_hora` | `hora` local `HH:MM`; `dias` 1-7 (vacío = todos). Si el aparato estaba apagado, corre al encender si no han pasado 6 h. |
| `al_sincronizar` | Al acabar una vuelta que trajo algo |
| `a_mano` | Solo cuando se pide |

**Pasos**: `{"tipo":"accion", herramienta, args?, proyecto?, imagenes?}` o
`{"tipo":"asistente", pedido, proyecto?, imagenes?}`. Como mucho 10. Variables: `{captura}`,
`{resultado}`, `{fecha}`, `{hora}`, `{sincro}` y `{nombre}`. El primer paso que falla para los demás.

**Reglas**:

- `aparato`: solo corre en el aparato que la creó; sin él, en todos. Las de otro aparato se listan
  pero no corren.
- **Conservar lo desconocido**:
  - una entrada que no se entiende (de una versión más nueva) se guarda tal cual y se lista como rota;
  - los campos extra de una automatización (`boton`, `icono`, `orden`, lo que añada el móvil) van en
    `resto` (`#[serde(flatten)]`);
  - los campos extra de la raíz del fichero también se conservan.

  Android tiene que hacer lo mismo (`JsonObject` crudo, o `aJsonSobre` como las tablas). Prueba
  del PC: `el_formato_va_y_vuelve_sin_perder_lo_que_no_entiende`.
- **Sin bucles**:
  - `al_capturar` no puede `capturar`;
  - `al_sincronizar` no puede `sincronizar`;
  - ninguna puede crear o tocar automatizaciones, tampoco desde un paso del asistente;
  - como mucho 30 veces por hora y no dos veces en 3 s (`motor::Guardia`).
- Lo que borra o saca algo **pregunta siempre**, también aquí. La pregunta sale en la burbuja del
  chat «Asistente» (§4.3).
- **No viaja** lo que ya corrió: `<raiz>\asistente\automatizaciones-estado.json` en el PC; en
  Android, un fichero en `noBackupFilesDir` o DataStore.

**Recetas** = automatización `a_mano`, `activa`, con un solo paso `asistente` y `boton: true`,
más `icono` (un emoji) y `orden` (0…n, de izquierda a derecha; mover renumera). Nombre ≤ 40 letras,
pedido ≤ 2000, como mucho 40 botones. Sin `aparato`. Borrar una receta la quita del fichero.

**Cómo se junta**: es un fichero suelto, así que gana el tocado más tarde, como un `.leccion`.
Para no pisar una edición del otro aparato: leer, cambiar y escribir enseguida, con cerrojo (el PC
usa un `Mutex` en `automatizaciones::cambiar`), y releer cuando la sincronización traiga el fichero.

### 3.6 Tablas que crea el asistente (`tabla_nueva`)

Pedido (protocolo):

```json
{"pixpin":1,"accion":"tabla_nueva","proyecto":"pr-1","nombre":"Costos","filas":[["Calle","Costo"],["Jr. Antisuyo","1 250,50"]],"cabecera":true,"abrir":true}
```

Resultado: un mensaje `MINIAPP` con `miniapp: "tabla"` cuyo **`texto` es exactamente el JSON de
`TablaDeCalculo`**. Es lo mismo que hace ya `TablasDelChat.kt` en el móvil (`pedidos::nueva_tabla`,
prueba `una_tabla_nueva_llega_rellena_al_chat`):

- `filas[f][c]` va a la celda de la columna `c` y la fila `f`, con A1 arriba a la izquierda;
- una celda vacía no se escribe;
- los números escritos como en papel se guardan como número: `"1 250,50"` → `"1250.50"`;
- `cabecera: true` pone `estilos` `{"n":true}` en la primera fila;
- las columnas anchas se ensanchan en `anchos`;
- sin nombre, se llama «Tabla».

```json
{"nombre":"Costos","celdas":{"A1":"Calle","B1":"Costo","A2":"Jr. Antisuyo","B2":"1250.50"},"anchos":{"A":110},"estilos":{"A1":{"n":true},"B1":{"n":true}},"tocado":1792044000000}
```

Con `destino: "excel"` se escribe un `.xlsx` y entra al chat como `ARCHIVO`, con el pedido `chat`
y `archivos`. Se junta celda a celda con `Fusion` (ya funciona).

### 3.7 Biblioteca de figuras: `figuras.json`

| | PC | Móvil |
|---|---|---|
| Fichero | `proyectos/<chat Asistente>/android/pins/draw/figuras.json` | `files/pins/draw/figuras.json` (**donde ya lo lee y escribe `BibliotecaStore`**) |
| Mensaje | `ARCHIVO`, **id `auto-figuras`**, `ruta: "pixpin:files/pins/draw/figuras.json"`, `nombre: "figuras.json"`, texto «🧩 Las figuras guardadas del lienzo (se usan desde el menu de figuras)» | lo mismo, uno solo |

- El formato es el de `BibliotecaStore.kt`: `[{id, nombre, elementos:[Element… normalizados a (0,0)], propia:true}]`.
- Las nuevas van **al principio** y los `id` viajan intactos. Al estampar, ids y semillas nuevos y
  todo agrupado.
- **Lo que falta en Android**:
  1. al guardar o quitar una figura, asegurar en el chat «Asistente» el mensaje `auto-figuras`;
  2. subir `BibliotecaStore.revision` cuando la sincronización traiga el fichero.
- Prueba del PC: `biblioteca_figuras::pruebas::la_biblioteca_va_al_movil_y_vuelve_por_el_chat_del_asistente`.
- Gana el tocado más tarde (fichero suelto).

### 3.8 Lo que anota el asistente en documentos y planos

`anotar_documento` escribe en **las mismas capas** que ya usan el lector y el visor de planos
(`sincro/AnotacionesDelAdjunto.kt`, `planos/CapaDelPlano.kt`):

| Documento | Capa |
|---|---|
| PDF adjunto, página n (desde 0) | `pins/draw/anot-<uid>-p<n>.excalidraw.gz` + su `.hoja` |
| Word, PowerPoint o libro | `anot-<uid>.excalidraw.gz` + `.hoja` (`anot-<uid>-texto` si es un PDF leído como texto) |
| Plano DWG/DXF | `anot-<uid>.excalidraw.gz` + `anot-<uid>.hoja` |
| PDF de un proyecto | la hoja del proyecto |

**Unidades**:

- PDF y Office: la hoja mide `ANCHO_HOJA = 1400` de ancho y `1400 · alto/ancho` de alto.
- Planos: con `encajeDe` y `Encaje` (la unidad de v0.115).

**Cómo se marca**:

| `tipo` | Figura |
|---|---|
| `resaltar`, `subrayar`, `tachar` | Las marcas de texto de los pines (`pines::marcas_texto`), sobre **todas** las apariciones de la página |
| `encerrar` | Elipse alrededor de la primera aparición |
| `marco` | Rectángulo; en un plano, el `frame` del grupo `plano-marco` |
| `nota` | Papel amarillo (`#fff3a3`) con el texto, a la derecha |
| `dibujar` | Línea por `puntos`, en fracciones 0-1 |

- Colores por omisión: resaltar `#ffd43b`; subrayar, tachar, encerrar y dibujar `#e03131`; marco
  `#3d8bff`; nota `#343a40`.
- Todo lo que pone el asistente va en el grupo **`asistente-anotado`**, para encontrarlo y
  borrarlo junto.
- Se guarda **juntando con lo que haya en el disco**, con la misma fusión de tres bandas de la
  sincronización (en el plano, `MotorDelPlano::guardar_juntando` en el PC y
  `CapaDelPlano.juntarConElDisco` en Android).

---

### 3.9 Qué NO se sincroniza

| Qué | Dónde vive (PC) | En Android |
|---|---|---|
| Claves de la API | `%LOCALAPPDATA%\PixPinMax\claves\*.clave` (DPAPI) | Keystore, fuera de la copia de seguridad |
| Modelos y motor | `%LOCALAPPDATA%\PixPinMax\modelos\` | `noBackupFilesDir/modelos/` |
| Índice de documentos | `<raiz>\cache\documentos\` (`fuentes.json` + `f/<huella>.json`) | `cacheDir/documentos/` |
| Estado de las automatizaciones (última vez que corrió cada una) | `<raiz>\asistente\automatizaciones-estado.json` | local |
| Ajustes `[asistente]` | TOML | DataStore |
| Lo que va pensando en vivo | memoria | memoria |
| Recetas o automatizaciones con `aparato` de otro equipo | viajan, pero **no corren** | igual |

---

## 4. Comportamiento de la UI

Lo que hay que copiar, sacado de `apps/pixpin/src/ventana_chat/*` y `asistente/chat.rs`.

### 4.1 Burbujas a cada lado

- En el chat «Asistente», las del usuario a la derecha y las del asistente (`aparato == "IA"`) a la
  izquierda. Hoy `MensajesActivity.Burbuja` pone todas a un lado: hace falta el lado según
  `chat::es_del_asistente(m)`.
- La burbuja nace al instante con «💭 …» y crece en vivo. Mientras escribe acaba en ` ▍`
  (`EnVivo::mostrar`).
- **Líneas de estado** mientras usa una herramienta (`chat::rotulo_de`):

  | Herramienta | Rótulo |
  |---|---|
  | `dibujar_en_lienzo` | ✏️ dibujando en el lienzo… |
  | `grafica` | 📊 haciendo la gráfica… |
  | `ocr` | 🔎 leyendo el documento… |
  | `buscar_historial` | 🗂️ buscando en la conversación… |
  | `calcar_imagen` | 🖼️ calcando la imagen… |
  | `ayuda_app` | 📖 mirando la ayuda… |
  | otra | ⚙️ trabajando… |
  | (al reintentar) | 🔁 corrigiendo… |
  | (al volver un resultado) | 💭 … |

- Los fallos se dicen **al final**, en una línea: `⚠️ a · b`.
- **Parado**: añade ` ⏹ (parado)`. **Error**: `⚠️ <aviso en palabras del usuario>` (`agente::para_el_usuario`).
- El botón de enviar pasa a **parar** mientras contesta.
- Una foto sola en el chat «Asistente» se pregunta como: «Te mando esta imagen. Descríbela en una o
  dos frases y pregúntame qué quiero hacer con ella.»
- La foto que ve el modelo es la última `IMAGEN` del usuario entre los **últimos 4 mensajes**
  (`FOTO_ATRAS`), aunque haya una respuesta en medio.

### 4.2 «Pensando…» plegable y petición mejorada (`ventana_chat/pensado.rs`)

Encima del texto de la respuesta, hasta tres renglones:

1. **«Pensando…»**, abierto mientras piensa, con el **final** de lo pensado (700 letras). Al acabar
   se pliega solo y dice **«Pensó 12 s»**. Un toque lo abre entero (hasta 6000 letras).
2. **«📝 Entiendo: …»** (`Plan::lineas`), plegado en una línea; un toque enseña de dos a cuatro
   líneas: `Entiendo:`, `Haré:`, `Con:` y `Datos:`.
3. **«Contexto renovado»** si el turno empezó reiniciado.

Al acabar, se lee del `.asistente.json` (§3.2). Lo plegado o abierto es solo de la vista y no viaja.

### 4.3 Tarjeta de confirmación «Sí / No» dentro de la burbuja

- Lo que borra o envía no abre un cuadro del sistema. El turno espera y la burbuja enseña la
  pregunta con **«No»** (relleno, por omisión) y **«Sí»** (borde rojo).
- Caduca (cuenta como no) a los **5 min**, al cerrar el chat o al pulsar parar.
- Contestada, **queda en el texto** de la nota:
  `❓ <pregunta> → ✅ Sí` · `→ ✖️ No` · `→ ⌛ sin respuesta: no se hizo nada`.
- Con un «No», la herramienta devuelve un resultado con «dijo que no». Eso no cuenta como fallo.
- Desde Flow o una automatización (sin ventana), la pregunta sale en la burbuja del chat «Asistente»
  (`asistente/permiso.rs`).

### 4.4 `@` para elegir lienzo

- Al escribir `@` al principio de una palabra sale una lista de 6 como mucho
  (`chat::opciones_de_mencion`):
  - `🤖 Asistente`, fuera de su chat;
  - los lienzos del chat (🎨) y las hojas (📄), del más nuevo al más viejo, sin repetir referencia.
- Elegir uno escribe `@lienzo:<Nombre_con_guiones_bajos> `. `resolver_lienzo` la cambia por
  «el lienzo «Nombre»» para el modelo y fija el destino.
- Lienzo sin nombre: «Lienzo <numero>»; hoja de PDF: «Hoja <página+1>».
- Las comparaciones van sin tildes ni mayúsculas.

### 4.5 Tarjetas «Abrir lienzo» y «Abrir fuente»

- **«Abrir lienzo»**: debajo de una respuesta con `lienzoIa` que casa con un `DIBUJO`/`PAGINA` del
  chat con esa `referencia`.
- **«Abrir fuente»**: la primera de `fuentesIa`, con su `titulo` («Tesis.pdf · pág. 12») y «+N»
  si hay más. Ejecuta su `pedido` (§3.1, para la ruta en el móvil).

### 4.6 Micrófono y altavoz (`ventana_chat/voz_del_asistente.rs`)

**Micrófono**:

- En el chat «Asistente» (y en otro con la caja vacía salvo `@asistente`), el botón redondo es el
  **micrófono del asistente**. Un toque empieza y otro acaba; o se **mantiene pulsado** y se suelta
  (≥ 450 ms = mantener).
- Lo grabado **no se guarda como nota de voz**: se transcribe y se borra.
- Lo oído **se envía solo**. Con `revisar_lo_dicho`, se queda en la caja.
- El PC usa Whisper. En Android: `Transcriptor.transcribirAqui` con el motor elegido
  (`motor_de_voz`; Whisper de sherpa-onnx recomendado), quitando las marcas `[m:ss]`.

**Altavoz**:

- Cada respuesta acabada lleva un **altavoz** junto a la hora, que la lee con el lector común. En
  Android: `LectorEnVoz.delApp(ctx)`, con las voces de Edge si están encendidas en `lectura`.
- **No se lee**: el pensamiento (`<think>…</think>`), el JSON de las herramientas, los bloques de
  código, las líneas de estado ni los avisos (`texto_para_leer`).
- Hablar al micrófono calla la lectura.
- Con `leer_respuestas`, cada respuesta se lee sola al acabar.

### 4.7 Recetas como botones

- Una fila de botones encima de la caja del chat «Asistente», ordenados por `orden`, con
  `icono` + `nombre`. Un toque manda el `pedido` como si se escribiera.
- **Crear**: «Guardar como receta» en el menú de una respuesta (toma la pregunta que contestó).
- **Editar** (clic derecho en el PC; pulsación larga en el móvil): renombrar o editar el pedido
  (en la propia caja de escribir), icono, mover a izquierda o derecha, borrar.

### 4.8 Aviso de nube para documentos

Con Claude o DeepSeek, la primera vez que `buscar_en_mis_cosas` encuentra algo se pregunta (con la
tarjeta Sí/No):

> El asistente está usando un modelo de la nube. Para contestar sobre tus documentos le mandaría
> trozos de ellos (lo que encuentre en tus PDF, notas y chats) por internet.
>
> ¿Lo permites? Lo que elijas se recuerda y se cambia en Ajustes › Asistente.

La respuesta se guarda en `documentos_a_la_nube`. Sin permiso, al modelo le llega: «el usuario no
permite mandar sus documentos a la nube; dile que puede permitirlo en Ajustes › Asistente o usar el
modelo local, que busca sin salir del PC».

### 4.9 Primera vez y aviso flotante

- En el chat «Asistente», con el motor local y sin modelo, sale una tarjeta «Bajar el asistente
  (1,9 GB)» con barra y cancelar. Con `mejor_vista` y sin Gemma, «Bajar 4,1 GB» / «Ahora no».
  Con la nube no hay tarjeta.
- El aviso flotante (`aviso_asistente.rs`) es la respuesta de un pedido hecho sin abrir el chat:
  «💭 …», luego la respuesta corta (220 letras), «Abrir» y un aspa; se va a los 15 s. En Android,
  el equivalente es una notificación con «Abrir» (p. ej. al terminar una automatización).

---

## 5. Algoritmos que hay que portar tal cual

Son deterministas y sin Windows, así que dan **el mismo resultado** en los dos aparatos. Copia
también sus constantes y sus pruebas.

### 5.1 Calcar una imagen: `crates/pixpin-ia/src/calcar.rs`

**Manchas de color** (lo normal):

1. Reducir la imagen a 160 / 256 / 384 px de lado (detalle bajo / medio / alto).
2. k-medias en RGB con 5 / 8 / 12 colores.
3. Filtro de mayoría para alisar las etiquetas.
4. Cada mancha conexa se recorre con **marching squares** (contornos y agujeros).
5. **Ramer-Douglas-Peucker** con tolerancia 1,4 / 1,0 / 0,7.
6. Suavizado que respeta las esquinas.
7. Salen como `pixpin-region`. El fondo (el color que más toca el borde) no se dibuja. Área mínima:
   0,0015 / 0,0004 / 0,00015.

**Solo contornos**: Canny (gauss, Sobel, supresión de no máximos, histéresis con umbrales de la
imagen) → cadenas de píxeles (mínimo 14 / 9 / 6) → RDP → suavizado → `line`.

**Topes**: 40 / 90 / 160 elementos, los más grandes. Parámetros de la herramienta: `detalle`,
`colores`, `solo_contornos` y `zona` en fracciones; `ajustar_al_pedido` los afina con las palabras
del pedido.

### 5.2 SVG → elementos: `crates/pixpin-ia/src/svg.rs`

- **Entiende**: `path` (M L H V C S Q T A Z, absolutos y relativos; las curvas y arcos pasan a
  polilíneas), `rect` (con esquinas redondeadas), `circle`, `ellipse`, `line`, `polyline`,
  `polygon`, `text`/`tspan`, `g`/`svg` con `transform`, `use #id`, `<style>` por clase, tipo e id,
  y los degradados como su color medio.
- **Saneado**: es texto hostil. Sin DTD ni entidades. Se ignoran `script`, `foreignObject`, `image`
  y las referencias externas.
- **Topes**: 200 000 bytes, 400 figuras, 60 000 puntos, 20 000 nodos y 32 niveles de anidamiento.
  Si se pasa, se rechaza con el motivo.
- `trazo`: `liso`, `a_mano`, `lapiz` o `grafito` (con `dureza`).

### 5.3 Tabla escaneada: `crates/pixpin-ia/src/tabla_escaneada.rs`

Recibe palabras con su caja, del OCR o de la capa de texto del PDF. Todo se mide en altos de letra.

1. **Filas**: se ordena por el centro vertical y se junta mientras se solape más de la mitad del
   alto menor.
2. **Celdas**: dentro de la fila, palabras separadas menos de `HUECO_EN_CELDA = 0.9` altos de letra.
3. **Columnas**: por intervalos, de la celda más estrecha a la más ancha. Una celda que pisa dos o
   más columnas ya hechas es **combinada**: su texto va en la primera y el resto queda vacío.
4. Menos de 2 filas o 2 columnas: «no se ve una tabla».

El modelo solo decide tabla o Excel, el nombre y si hay cabecera (`parece_cabecera` si no lo dice).
En Android, el OCR es ML Kit Text Recognition (`Text.TextBlock.lines.elements` con `boundingBox`)
o el texto del PDF.

### 5.4 Buscar en mis cosas (BM25): `crates/pixpin-ia/src/documentos/`

- **Palabras** (`texto.rs`):
  - minúsculas y sin tildes, y `ñ` → `n`;
  - partir por todo lo que no es letra o número;
  - fuera las palabras vacías (español e inglés);
  - raíz ligera: quitar `aciones`, `amientos`… `es`, `s` y luego la vocal final, si quedan ≥ 3 letras;
  - en la pregunta se quitan además las palabras de preguntar (`DE_LA_PREGUNTA`).

  Copia las dos listas literalmente.
- **Trozos** de **160 palabras con 30 de solape** (`PALABRAS_POR_TROZO`, `SOLAPE`), como mucho
  4000 por fuente.
- **BM25**: k1 = 1,2 y b = 0,75; +0,5 si la palabra está en el nombre de la fuente; corte relativo
  0,45 del mejor; 2 trozos por fuente como mucho; fragmento de 420 letras; 5 resultados.
- **Fuentes**: PDF, Word, Excel, PowerPoint, notas, `.txt`/`.csv` y **chats** (sus mensajes;
  **sin** las notas `aparato == "IA"`).
- **Al día por firma** (tamaño + fecha): solo se relee lo que cambió. Es la idea de
  `IndiceDelBuscador.kt`; en Android, en un `WorkManager` de prioridad baja.
- **Alcance**: desde un chat compartido solo se busca en ese chat. Lo filtra el índice, no el modelo.
- **Resultado al modelo**: numerado `[1]`, `[2]`… y seguido entre búsquedas del mismo turno
  (`Citas`). La respuesta guarda las que el modelo nombró (`fuentesIa`).

### 5.5 Reinicio de contexto al 70 %: `crates/pixpin-ia/src/contexto.rs`

- `estimar`: letras / 3,2 + 450 por imagen + 6 por mensaje + las llamadas + el catálogo.
- Si `tokens + 1536 > 0,70 · ventana`, se reinicia: el turno arranca con sistema (+ hoja + plan) +
  los **2 últimos** mensajes de texto + el nuevo (`preparar`, `reiniciar`). La hoja apunta
  `reinicio_desde` (id de nota) y `reinicios += 1`, y el sistema añade «La conversacion de antes ya
  no esta cargada: si necesitas algo de ella, usa buscar_historial.».
- Dentro de un turno largo (`podar_en_turno`), los resultados viejos de más de 200 letras se dejan
  en 160 + «(resultado recortado para ahorrar contexto; usa buscar_historial si lo necesitas)».
- `buscar_historial`: puntúa por palabra (1,0) o por raíz de 5 letras (0,6) sobre fuentes
  etiquetadas «#12 usuario», «#13 asistente», «T3 pensado».

### 5.6 Intérprete rápido de órdenes simples: **aún no está en `main`**

Está en la rama `ia-rapidez-tareas` (`crates/pixpin-ia/src/rapido.rs`, `tareas.rs`), sin unir.
Convierte órdenes cortas («abre la galería», «apunta comprar yeso») en la llamada sin pasar por el
modelo (< 100 ms). No hace nada si duda, y **nunca** lo que borra. Cuando se una a `main` se añadirá
aquí. **No lo portes antes.**

---

## 6. Pruebas de compatibilidad

### 6.1 Lo que ya tiene el PC

| Prueba | Para qué |
|---|---|
| `cargo test -p pixpin-ia` | Unitarias: validación, catálogo, agente con `MotorGuionado`, hoja, contexto, calcar, svg, tabla, BM25, nube con un servidor falso y las grabaciones `.sse` |
| `cargo test -p pixpin-ia --test banco -- --ignored --nocapture` | **Banco** con el modelo de verdad: 42 casos con comprobación automática. `PIXPIN_IA_MOTOR=local,gemma,claude,deepseek` saca una tabla comparativa. |
| **Modo grabado** | `volcar_para_haiku` escribe `sistema.txt`, `herramientas.json` y `casos.json`. Un «cerebro» contesta en `respuestas.json` (`{"<caso>":{"texto":"…","llamadas":[{"nombre":"…","entrada":{…}}]}}`). Con `PIXPIN_IA_MOTOR=grabado PIXPIN_IA_GRABADO=<respuestas.json>`, el banco lo pasa por la misma validación y ejecución. **Sirve para comprobar el móvil**: graba lo que contesta el modelo en Android, pásalo por el banco del PC y compara. |
| `apps/pixpin/src/asistente/documentos_acciones/banco.rs` | Tabla escaneada, anotar PDF y plano, buscar en plano, abrir por nombre, exportar |
| `crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs` | Un **móvil simulado** que habla el protocolo de verdad |
| `apps/pixpin/src/plano_cad/anotar/pruebas_con_el_movil.rs` | Capas de plano de ida y vuelta |
| `automatizaciones::pruebas` | `el_formato_va_y_vuelve_sin_perder_lo_que_no_entiende`, `guardar_crea_el_fichero_y_un_solo_mensaje_que_lo_hace_viajar` |
| `biblioteca_figuras::pruebas` | `la_biblioteca_va_al_movil_y_vuelve_por_el_chat_del_asistente` |

Ojo: `cargo test` a secas en el workspace del PC da rojos falsos. Hay que lanzar los paquetes
concretos.

### 6.2 Cómo comprobar que Android da lo mismo

1. **Catálogo**: genera en Kotlin la lista `tools` y compárala con `herramientas.json` (mismo orden,
   mismos nombres y mismos esquemas tras `con_topes(16)`). Debe salir igual byte a byte tras
   normalizar el orden de las claves.
2. **Validación**: porta las pruebas de `herramientas.rs` (positivas y negativas: JSON roto,
   herramienta desconocida, fuera de rango, gráfica descuadrada…), con los **mismos textos de
   error**: el modelo aprende de ellos.
3. **Al lienzo**: con la misma semilla y la misma llamada, `lienzo::documento` y el Kotlin deben dar
   los mismos elementos. Usa los JSON de las pruebas de `lienzo.rs` como dorados.
4. **Calcar, SVG, tabla escaneada y BM25**: mismas entradas → mismas salidas, con tolerancia de
   1e-6 en coordenadas.
5. **Sincronización** (con un PC de verdad):
   - una respuesta con `lienzoIa` hecha en el PC → abrirla en el móvil, fijarla y sincronizar →
     el PC sigue teniendo `lienzoIa`;
   - recetas: crear una en el móvil y otra en el PC, sincronizar → las dos en los dos, con un solo
     mensaje `auto-automatizaciones`;
   - figuras: lo mismo con `auto-figuras`;
   - el `.asistente.json` de una respuesta del móvil se ve en el PC («Pensó N s», «📝 Entiendo»);
   - borrar la respuesta en un lado → el `.asistente.json` desaparece en los dos;
   - `anotar_documento` en el móvil sobre un PDF → la marca se ve en el lector del PC en la misma
     página y en el mismo sitio.

### 6.3 Lista de comprobación final

- [ ] `Mensaje` conserva `lienzoIa` y `fuentesIa` (o todo el `resto`).
- [ ] `Proyecto` conserva los campos que no conoce (o al menos `asistente`).
- [ ] Un solo chat «Asistente», reconocido como en §3.3.
- [ ] Respuestas con `aparato: "IA"`, `numero` y `uid`; `respondeA` fuera de su chat.
- [ ] `anot-<uid>.asistente.json` con `version: 1`; se lee recortando; uno roto se ignora; se borra con su nota.
- [ ] `automatizaciones.json` y `figuras.json` en sus rutas, con **un** mensaje ARCHIVO de id fijo cada uno, y conservando lo desconocido.
- [ ] Recetas: `a_mano` + `boton` + `icono` + `orden`, con los topes 40/2000/40.
- [ ] Lo dibujado: Excalidraw con `customData.origen = "pixpin-ia"`, ids `ia-…`, solo `type` conocidos, en un paso de deshacer, al lienzo propio «Lienzo del asistente» si no hay otro.
- [ ] Anotaciones en `anot-<uid>-p<n>` / `anot-<uid>` + `.hoja`, grupo `asistente-anotado`, juntando con el disco.
- [ ] Tablas: mini-app `tabla` con el JSON de `TablaDeCalculo` en `texto`.
- [ ] Claves: Keystore, fuera de la copia de seguridad, nunca en el registro.
- [ ] Modelos con las mismas URL y SHA-256, en `noBackupFilesDir`; uno cargado a la vez.
- [ ] Claude sin `strict`; DeepSeek devolviendo `reasoning_content`.
- [ ] Lo que borra o sale del aparato pregunta en la burbuja, con «No» por omisión y caducidad de 5 min.
- [ ] Las mismas 45 herramientas, la misma selección de 6 con el local y el catálogo entero con la nube.

---

## 7. Pendientes conocidos

De `docs/investigacion/2026-10-13-para-android.md`, revisados contra Android `5683c3b`
(v0.124.0, 14-oct):

| Pendiente | Estado en Android |
|---|---|
| `PlanoActivity` pisaba la capa en cada `onPause` | **Arreglado en v0.124.0**: `guardarCapa()` no escribe si no cambió y junta con el disco si cambió (`selloDeLaCapa`). Comprobar con `pruebas_con_el_movil.rs`. Falta confirmar que **relee** al llegar algo con el plano abierto (`ExcalidrawStore.revisionDe`). |
| Voces Edge con servidor nuevo | **Hecho**: `EdgeVoz` habla por `api.msedgeservices.com/.../websocket/v1` con `Sec-MS-GEC`. |
| Formato nuevo de celdas (`k s t c w nf`) | `EstiloDeCelda` ya las tiene. Falta comprobar los botones y que se pinten. |
| Ids repetidos | **Hecho** (`IdsRepetidos.arreglar`). |
| Abrir la mini-app `tabla` creada en el PC | Hecho con `TablasDelChat.kt` (comprobar). |
| `CapaDelPlano.con` que junte por forma | Ids por FNV-1a de la forma: comprobar con cotas llegadas del PC. |
| Hojas de Excel editadas (A3) | **Pendiente de decisión del usuario**. |
| `Proyecto` que tira campos (A4) | Pendiente. Bloquea mandar `asistente: true` (§3.3). |

**Nuevos, solo del asistente**:

1. `Mensaje` sin `resto`: pierde `lienzoIa` y `fuentesIa`. **Bloqueante** (§3.1).
2. Mensaje `auto-figuras` al guardar en `BibliotecaStore`, y releer al llegar (§3.7).
3. Todo lo del asistente: no hay nada de IA en Android hoy (ni motor, ni claves, ni automatizaciones).
4. **Para el PC** (encontrado al escribir esto): `agente::con_topes` pone `maxLength: 120` también
   en `dibujar_svg.svg`, `crear_automatizacion.pasos` y `nota_nueva.texto`. Con la gramática de
   llama.cpp eso corta un SVG. Conviene eximir los campos de texto largo. Cuando se arregle, el
   móvil copia el cambio.
5. `fuentesIa` con rutas absolutas del PC: el móvil las resuelve por nombre (§3.1). A medio plazo,
   que el PC guarde la ruta portátil cuando la haya.

---

## 8. Dónde encaja cada pieza en Android

| Pieza del PC | Dónde ponerla en Android | Qué hay ya |
|---|---|---|
| `Motor`, `agente`, `herramientas`, `seleccion`, `lienzo`, `contexto`, `hoja_trabajo`, `pensar` | Un paquete nuevo `ia/` (Kotlin puro, con pruebas JVM) | nada |
| llama.cpp | `ia/MotorLlama.kt`: binario `llama-server` arm64 en un proceso, o JNI | sherpa-onnx ya se carga por JNI (`MotorWhisper.kt`): el mismo patrón |
| Nube | `ia/nube/{Claude,DeepSeek}.kt` con OkHttp y SSE | OkHttp ya está (`VozDeEdge.kt`) |
| Claves | `ia/nube/Claves.kt` con Keystore | nada; excluir de la copia de seguridad |
| Chat «Asistente», burbujas, tarjetas | `guardados/MensajesActivity.kt` (`Burbuja`, línea ~2635; `BarraDeEscribir`, ~5167) | burbujas a un solo lado; micrófono de notas de voz |
| Dictado | `Transcriptor.transcribirAqui` | Vosk, Whisper (sherpa-onnx) y Google |
| Leer en voz alta | `LectorEnVoz.delApp(ctx)` + `usarEdge` | sí |
| Lienzo | `motor/Element.kt`, `Scene.kt`, `ExcalidrawStore.guardar`, `revisionDe` | sí, mismos `type` |
| Figuras | `motor/BibliotecaStore.kt` | sí, mismo fichero; falta el mensaje `auto-figuras` |
| Tablas | `motor/TablaDeCalculo.kt`, `guardados/TablasDelChat.kt` | sí |
| Anotar documentos | `sincro/AnotacionesDelAdjunto.kt`, `planos/CapaDelPlano.kt` (`encajeDe`, `juntarConElDisco`) | sí |
| Índice de documentos | El patrón de `IndiceDelBuscador.kt` + `WorkManager` | sí, otro índice |
| Automatizaciones | `ia/Automatizaciones.kt`; disparadores con `WorkManager` (a la hora), el final de `SincronizacionAutomatica` (al sincronizar) y la captura | nada |
| Sincronización | Nada nuevo: `Disco.kt` ya lleva `pins/draw/anot-*` (añadir `.asistente.json` a `TERMINACIONES`) y `guardados/` | añadir la terminación |
| Ajustes | `data/SettingsRepository.kt` (claves del §1.4) | DataStore |

Documentos relacionados: `docs/investigacion/2026-10-13-ia-local.md` (medidas y decisiones del
motor local), `docs/protocolo-pedidos.md` (todos los pedidos), `docs/investigacion/2026-10-12-que-se-sincroniza.md`
y, en el repositorio de Android, `docs/plan-sincronizacion.md` y `docs/planos-que-viajan.md`.
