# S1-A — Lecciones para S1-B, S1-C y los sub-proyectos siguientes

**Rama:** `s1a-cimientos` · 25 commits · 58 ficheros · ~3.800 líneas
**Resultado:** 39 tests + 6 que exigen escritorio · `fmt`, `clippy -D warnings` y `cargo-deny` limpios
**Presupuesto medido:** CPU en reposo **0%** · RAM **≈13 MB** (objetivo <40) · binario **0,84 MB** (objetivo <30)

Las once tareas se ejecutaron con revisión independiente de cada una y una revisión final de toda la rama. Este documento recoge lo que costó dinero aprender, para no volver a pagarlo.

---

## 1. Un test que sólo se ha visto pasar no prueba nada

**Cuatro tests de este plan no podían fallar en la propiedad que su nombre prometía.** Los cuatro los escribió el mismo autor que escribió el plan, y los cuatro estaban en verde:

| Test | Prometía | Comprobaba en realidad |
|---|---|---|
| `ninguna_dependencia_sube_de_capa` | que ninguna capa dependa hacia arriba | nada: se burlaba con `foo = { package = "pixpin-shell" }` |
| `el_formato_es_reversible` | que el orden de modificadores se canoniza | nada: todas sus cadenas ya venían canónicas |
| `se_crea_y_se_destruye_sin_fugas` | que la ventana se destruye | que registrar una clase dos veces no falla |
| `copiar_titulo` (versión inicial) | que no se parten pares sustitutos UTF-16 | nada: contaba unidades, no caracteres |

**Regla para los planes siguientes:** todo test que proteja una invariante debe traer **su caso negativo explícito**, y el brief debe exigir la secuencia rojo→verde como evidencia entregable. No basta con «los tests pasan».

## 2. «Verificado por inspección» suele significar «no verificado»

Tres defectos reales se escondieron tras esa frase. El más claro: el autor del truncado de `szTip` **detectó el riesgo por su cuenta**, escribió el arreglo, lo documentó en el comentario y lo declaró verificado. Contaba unidades UTF-16 en lugar de caracteres, así que el fallo seguía intacto: 126 caracteres ASCII más un emoji dejaban `szTip[126] = 0xd83d`, un sustituto alto suelto, entregado a Windows como cadena malformada.

Lo encontró un revisor que **no leyó: montó un banco de pruebas aparte y lo reprodujo**.

**Regla:** si algo se puede comprobar ejecutándolo, «verificado» sólo puede significar que se ejecutó. Los briefs deben pedir el comando y su salida, no la conclusión.

## 3. Un comentario `// SAFETY:` que miente es peor que no tenerlo

En `appdata()`, el comentario afirmaba que el buffer de `SHGetKnownFolderPath` siempre se liberaba. No era cierto: un `?` en la conversión salía de la función antes de `CoTaskMemFree`. **Fuga de memoria COM que pasó todas las puertas automáticas**, incluido el lint que exige documentar cada bloque `unsafe`.

El `// SAFETY:` es el contrato que el siguiente lector se cree sin volver a comprobarlo. Si miente, el error deja de detenerse ahí y se propaga.

**Regla:** un `// SAFETY:` debe decir **por qué se cumplen las precondiciones de la API**, no parafrasear la línea. Y cuando enuncia una obligación del llamante, debe decirlo como obligación, no como hecho verificado.

## 4. Lo que ninguna revisión por tarea puede ver

La revisión final de rama encontró cinco fallos que sólo aparecen mirando el conjunto. Dos ejemplos que justifican su coste ellos solos:

- **El icono incrustado no se usaba.** Se validó el `.ico` byte a byte, se incrustó, se confirmó su presencia en el binario — y `bandeja.rs` cargaba `IDI_APPLICATION`. Nadie cargaba el recurso 1. Cada eslabón estaba bien; fallaba la cadena.
- **El icono de bandeja y los atajos se liberaban contra una ventana ya destruida.** `ejecutar(self)` consumía la ventana, así que `NIM_DELETE` y `UnregisterHotKey` corrían sobre un `HWND` muerto, con los resultados descartados por `let _ =`. Funcionaba sólo porque Windows limpiaba por nosotros.

**Regla:** ningún sub-proyecto se da por terminado sin revisión de rama completa, sobre el modelo más capaz disponible.

## 5. Las tandas de corrección también introducen fallos

La tanda que arregló los cinco hallazgos finales **introdujo dos nuevos**: el guardia de `tracing` quedó local a `arrancar()`, de modo que el error de arranque nunca llegaba al log —con un comentario afirmando que sí—, y `LoadImageW` sin `LR_SHARED` empezó a filtrar un `HICON`.

**Regla:** toda corrección se re-revisa. Una corrección sin revisar es código sin revisar.

## 6. La arquitectura se degrada por omisión, no por decisión

Al añadir el crate `windows` a `apps/pixpin` para un `MessageBoxW`, el ejecutable se convirtió en un sitio con `unsafe` sin auditar. No hubo ninguna decisión de hacerlo: simplemente `apps/pixpin` no estaba en ninguna de las dos listas del documento maestro, y el test de capas ignora los crates externos.

Se corrigió moviendo el diálogo a `pixpin-shell` y añadiendo `#![forbid(unsafe_code)]` a `main.rs` — **la única guarda que lo habría detectado**.

**Regla:** cada crate debe llevar explícitamente su política de `unsafe`. Una lista en un documento no se hace cumplir sola.

---

## Notas concretas para S1-B (Direct2D, captura, overlay, lupa, snap)

- **`procedimiento` llama a `PostQuitMessage` ante cualquier `WM_DESTROY`.** En cuanto los overlays por monitor compartan o copien ese `WndProc`, cerrar un overlay matará la aplicación entera. **Dales su propia clase y su propio procedimiento.**
- **`ejecutar(&self)` con `FnMut(Evento) -> Continuar` no da al callback acceso a la ventana.** Ya obliga a `main.rs` a copiarse el `HWND` antes. Direct2D, la llegada de frames de WGC y los callbacks de UI Automation van a querer alcanzarla: conviene pasar `&VentanaMensajes` al callback ahora, mientras sólo hay un llamante.
- **`PENDIENTES` es un `RefCell<Vec>` del hilo de interfaz.** WGC entrega frames en un hilo del pool, así que S1-B necesita un canal entre hilos de verdad más un `PostMessage` para despertar el bucle, no esta cola.
- **`estan_los_dieciseis_paquetes` fija el número 16 a mano.** Es fricción deliberada, pero hay que presupuestar la edición de `capas.rs` en el plan de S1-B.
- **`--test-threads=1` es estructural y lo será más.** Varios tests toman recursos globales del sistema. Que ningún test nuevo tome el mismo nombre global.
- **`arranque.rs` es el único módulo que maneja un handle de Win32 sin RAII.** Hoy es seguro porque no hay ningún `?` entre abrir y cerrar — exactamente la forma del defecto que causó la fuga COM. Envolver `HKEY` antes de que S1-B añada más código de registro.
- **La CI no puede ejecutar los tests que exigen sesión de escritorio.** Están marcados `#[ignore]` y se ejecutan con `cargo test -- --ignored`. Cualquier test nuevo de bandeja, atajos o registro necesita la misma marca.

## Nota para S2 (motor de anotación)

El `motor` del Android **es un puerto de Excalidraw**, confirmado por el autor. Excalidraw es MIT, igual que este proyecto, así que —a diferencia de QuickView, que es GPL-3.0 y ajeno— **sí se puede leer y adaptar su código**, no sólo sus ideas. Verificar la licencia antes de apoyarse en ello.
