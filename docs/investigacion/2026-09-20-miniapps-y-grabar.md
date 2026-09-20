# Las mini-apps del chat, grabar, y lo que el móvil hace con el audio

Inventario para portar a PixPin Max lo que el usuario pidió el 20-sep-2026:
«seguir con la implementación de las mini apps y demás funciones para grabar y
más que tienen el chat».

- **Android leído**: `1xmanMAX/PIXPIN_PRO_ANDROID`, commit `0dc83ac`
  («v0.72.0: llamada secreta, zoom propio de los documentos…», 20-sep-2026).
  Todas las líneas citadas son de ese commit.
- **PC leído**: rama `pin-en-vivo`, commit `46549d4` («El foco oscurece lo mismo
  que en el móvil, y el inventario dice cómo va esto»).
- **No se ejecutó la aplicación** y no se tocó ningún fichero de código.
- Antecedente: `docs/investigacion/2026-09-18-chat-android-inventario.md`, que
  ya listaba estas cosas como «sale con aviso».

---

## 0. Lo primero: la nota vieja sigue siendo cierta, y hay un hallazgo

`crates/pixpin-proyecto/src/mini.rs:3-15` dice que en Android son siete y que
viajan como un mensaje de clase `MINIAPP` con el documento en `Mensaje.texto` y
la palabra del tipo en `Mensaje.miniapp`. **Comprobado en la v0.72: sigue
exactamente así, y no hay más de siete.**

`app/src/main/java/com/forge/pixpin/mini/MiniApps.kt:100-190` —
`enum class MiniApp(val id: String, val nombre: Int)` con siete valores y ni uno
más: `TAREAS("tareas")` :103, `GASTOS("gastos")` :112,
`CRONOMETRO("cronometro")` :128, `TEMPORIZADOR("temporizador")` :143,
`CONTADOR("contador")` :157, `RULETA("ruleta")` :168, `ALARMA("alarma")` :179.
`MiniApp.de(id)` :213 devuelve `null` ante una palabra desconocida a propósito,
para que un mensaje de una versión futura siga en la lista como texto en vez de
tumbar la conversación. El documento es Markdown dentro del mensaje, y la
decisión está razonada en la cabecera del fichero, :18-60.

Ojo al nombre: **`pin/MiniApps.kt` NO es el catálogo**. Es el dibujado Compose de
los *pines* (`Ledger` :46, `TimerBody` :81, `ChecklistBody` :195, `CounterBody`
:241, `LedgerBody` :262, `TableBody` :311). El catálogo es `mini/MiniApps.kt`.

### El hallazgo

**El PC ya tiene el modelo de `tareas` escrito, probado y sin conectar a nada.**
`crates/pixpin-proyecto/src/mini.rs` tiene `TAREAS`, `leer_tareas`,
`escribir_tareas`, `titulo`, `alternar`, `anadir`, `cuenta` y seis pruebas
(:158-242). Un `grep -rn "mini::"` sobre todo el repo **no da ni un acierto**
fuera del propio fichero: ninguna pantalla lo llama. Es decir: la mini-app de
tareas está a un panel de distancia, no a una implementación.

Y el formato coincide al carácter con el de Android:
`mini.rs:82-94` escribe `- [x] texto` con `linea_de_titulo` = `"# T\n\n"`, igual
que `Tareas.kt:87-90` + `Cabecera.linea` (`MiniApps.kt:251-254`).

### Lo que el PC tiene hoy de mini-apps

- Una mini-app **propia**, la hoja de cálculo: `miniapp = "tabla"`
  (`crates/pixpin-proyecto/src/tabla.rs:35`), con su documento en **JSON** dentro
  de `texto` (`tabla.rs:38-55`, `Tabla::leer/escribir` :154-158). No existe en
  Android y está bien documentado por qué (`tabla.rs:1-26`).
- Se crea desde el clip: `apps/pixpin/src/ventana_chat.rs:8416-8420`
  (`Accion::MiniApps` abre un menú con **una sola entrada**, «Tabla») y
  :5248-5258 (`cuaderno::Mensaje::miniapp`).
- Se pinta: `ventana_chat.rs:5411-5423` (`Ojeada::Tabla`), rejilla en
  `crates/pixpin-ui/src/tabla.rs`.

---

## 1. Las siete mini-aplicaciones, una por una

Reglas comunes de formato (`mini/MiniApps.kt`): título en la primera línea con
`^#{1,6}\s+(.*)$` (:227) y **siempre** un renglón en blanco detrás (:251-254);
`enUnaLinea` convierte `\r\n`, `\r` y `\n` en espacios (:270-271); ningún
documento acaba en `\n` salvo gastos, que acaba en `**`. Todos **nacen vacíos**
(`documentoNuevo`, :200).

La pantalla es una sola, `mini/MiniActivity.kt` (769 líneas), con despacho en
:155-163. **No hay botón de guardar** (:64-68): cada toque reescribe el JSONL
entero (`guardar` :102-110). El título se edita en la propia barra superior y
conserva el cuerpo tal cual (:139-148).

### 1.1 Tareas — `mini/Tareas.kt`

Lista de casillas: añadir, alternar, renombrar, borrar, mover y limpiar las
hechas (:39-213). El «3 de 7» se calcula del documento, no de un contador
guardado (:165-179).

**Formato exacto.** Lectura con `CASILLA = Regex("""^\s*[-*+]\s+\[([ xX])]\s?(.*)$""")`
(:51); escritura `"- [${if (t.hecha) "x" else " "}] ${enUnaLinea(t.texto)}".trimEnd()`
(:87-89), unido por `\n`. Escribe **solo tareas**: lo demás se pierde (:77-85).

```
# La compra

- [x] pan
- [ ] leche
- [ ] 
```

**Cómo se ve.** `DeTareas`, `MiniActivity.kt:169-223`: `Checkbox` → `alternar`
(:187-190), texto 16 sp **tachado** y en `onSurfaceVariant` cuando está hecha
(:194-201), `Close` de 18 dp para borrar (:202-208), línea de añadir abajo
(:212-221).

**Datos que viajan.** En el `.pixpin`/cuaderno: `texto` (el documento),
`miniapp="tareas"`, `clase="MINIAPP"`. Nada más. **Nada en fichero aparte.**

**Android**: ninguna dependencia de aparato. **PC hoy**: modelo listo
(`mini.rs`), pantalla **no** (ni una llamada a `mini::`). **Esfuerzo**: bajo.
**Valor**: alto — es la mini-app que se usa a diario en obra.

### 1.2 Gastos — `mini/Gastos.kt`

Conceptos con importe y total, en enteros de unidades menores (`Long` de
céntimos), con la moneda **dentro del documento** (`Libro` :87-97).

**Formato exacto** (`escribir` :169-186): una tabla de Markdown, el código de
moneda en el rótulo de la cabecera, importes **canónicos** con punto decimal y
sin miles (`BigDecimal.valueOf(centimos, decimales).toPlainString()`, :265-266),
y el total en negrita al final. **El total no se guarda como dato**: se recalcula
al escribir y se ignora al leer (:66-73, :235-236).

```
# Viaje a Lisboa

| Concepto | Importe (EUR) |
| --- | ---: |
| Cena | 42.50 |
| Tren | 18.00 |
| Devolución | -5.00 |

**Total: 55,50 €**
```

Detalles que hay que copiar o se rompe la ida y vuelta: la moneda se lee con
`CODIGO = Regex("""\(\s*([A-Za-z]{3})\s*\)""")` (:124); la fila separadora se
salta con `GUIONES = Regex("""^:?-{3,}:?$""")` (:126) solo si **todas** las
celdas casan (:154); el concepto es la primera celda y el importe **la última**
(:155-158); las celdas se escapan con `\\` y `\|` (`celdaSegura` :363-364,
desescape en `celdas()` :367-382); leer es tolerante (`centimosDe` :310-331:
normaliza `−`→`-`, decide el separador decimal mirando el **último** `.`/`,`, y
trunca decimales sobrantes). Topes :345-346.

**Cómo se ve.** `DeGastos`, `MiniActivity.kt:226-354`: filas con concepto a una
línea y `Close`, divisor, total a 20 sp en `primary` (:269-281), y abajo campo de
concepto + **botón de signo `+`/`−`** (:298-318, porque el teclado decimal de
Android no trae menos) + campo de importe decimal + `Add`.

**Android**: ninguna API de aparato; `java.util.Currency`/`NumberFormat`.
**PC hoy**: no. **Esfuerzo**: medio (el parser de importes es la mitad del
trabajo). **Valor**: alto.

### 1.3 Cronómetro — `mini/Tiempos.kt` (`Cronometro` :30-43)

Tiempo que sube. Guarda **lo acumulado** y **el instante de arranque**, no el
número visible, así que sobrevive a que maten el proceso (:12-21).

**Formato exacto** (`documento` :88-89, `escribirCronometro` :103-110): líneas
`- clave: valor`. `llevado` siempre; `desde` solo si corre; `vueltas` solo si las
hay, separadas por **espacios**.

```
# Tanda de series

- llevado: 42350
- desde: 1758351600000
- vueltas: 10250 20400 33900
```

Lectura tolerante en `valores()` (:78-86): quita el `-`, corta en el primer `:`,
**baja la clave a minúsculas**. `llevado` se acota a ≥ 0 (:93-101).

**Cómo se ve.** `DeCronometro`, `MiniActivity.kt:364-428`: número a 56 sp,
repintado cada **100 ms** solo mientras corre (:369-374) y **sin tocar disco**
(:356-362); Arrancar/Parar, Vuelta (solo habilitado si corre), Reiniciar; lista
de vueltas en orden inverso.

La burbuja enseña lo de **la última vez que se tocó**, no el tiempo vivo
(`MiniApps.kt:120-139`), y está razonado: una lista no puede repintarse 60 veces
por segundo.

**Android**: solo `System.currentTimeMillis()` y corrutinas. **PC hoy**: no.
**Esfuerzo**: bajo. **Valor**: medio.

### 1.4 Temporizador — `mini/Tiempos.kt` (`Temporizador` :46-64)

Tiempo que baja hacia `finEn`; `restante()` no baja de cero (:60-61). Por defecto
5 min (:48), tope 24 h (`TOPE_DE_DURACION` :211).

**Formato exacto** (:136-142): `duracion` siempre; `finEn` (con F mayúscula al
escribir) solo si corre.

```
# Pasta

- duracion: 420000
- finEn: 1758352020000
```

**Trampa para el porteador**: al leer se busca **`v["finen"]`** en minúsculas
(:128-134), porque `valores()` baja la clave. Escribe `finEn`, lee `finen`.

**Cómo se ve.** `DeTemporizador`, `MiniActivity.kt:431-497`: repinta cada 200 ms;
número en **`colorScheme.error`** cuando está vencido (:446-453); cuatro botones
`-5 / -1 / +1 / +5` minutos (:456-470); botón grande Arrancar/Parar.

**Android**: **AlarmManager**. `Recordatorios.poner(this, aviso(id), finEn)`
(:475), id `"mini:" + idDelMensaje` (`pin/RecordatorioReceiver.kt:25`),
`setAlarmClock` con caída a `set(RTC_WAKEUP, …)` (`pin/Recordatorios.kt:35-46`).
Al vencer, `RecordatorioReceiver` saca el título en un pin
(`RecordatorioReceiver.kt:39-49`). **No hay vibración ni sonido propios en
ninguna mini-app.**

**PC hoy**: no. **Windows**: no hace falta AlarmManager — un hilo con un
`Instant` objetivo y, al vencer, la ventana de pin que ya existe
(`crates/pixpin-pin/src/ventana.rs`) o un aviso del sistema
(`Windows.UI.Notifications.ToastNotification`). **Esfuerzo**: bajo (sin aviso) /
medio (con aviso). **Valor**: medio.

### 1.5 Alarma — `mini/Tiempos.kt` (`Alarma` :67-71)

Una hora del día y un interruptor. Por defecto 8:00 desactivada (:68-70).

**Formato exacto** (:169-175), siempre las dos claves:

```
# Despertar

- hora: 7:05
- activa: sí
```

**Dos trampas**: la hora **no** se rellena a dos cifras (`7:05`, `8:00`) pero el
minuto sí; y `activa` se codifica literalmente **`sí`/`no` con tilde**, y al leer
es una comparación exacta `v["activa"] == "sí"` (:159-167) — cualquier otra cosa
es `false`. Un PC que escriba `si` sin tilde apaga la alarma en el móvil.

**Cómo se ve.** `DeAlarma`, `MiniActivity.kt:500-558`: hora a 64 sp, `Switch`, y
cuatro botones con aritmética modular (`-1 h` = `(hora+23)%24`, etc. :544-555).
Cada cambio **rehace la alarma del sistema entera** (`conLaAlarma` :505-520) con
`proximaVezQueSean(now, hora, minuto, TimeZone.getDefault())`.

**Android**: AlarmManager, igual que el temporizador. **PC hoy**: no.
**Esfuerzo**: bajo. **Valor**: bajo en un ordenador de sobremesa (quien madruga
no lo hace con el PC), pero el **formato** hay que respetarlo igual para no
romper el documento del móvil.

### 1.6 Contador — `mini/Contador.kt` (`Cuenta` :19-23, `Contador` :26-60)

Un número que sube y baja con un **paso guardado junto al contador** (no global).
Tope `1_000_000_000` (:28-29).

**Formato exacto** (:49-50), siempre las dos líneas:

```
# Cajas descargadas

- valor: 144
- paso: 12
```

Al leer, **el paso se corrige**: `?.takeIf { it > 0 } ?: 1L` (:45), para aguantar
un fichero tocado a mano.

**Cómo se ve.** `DeContador`, `MiniActivity.kt:571-617`: número a 72 sp, dos
botones de **96 dp** (`−` y `+`, texto a 34 sp) porque se usa sin mirar
(:564-570), fila de pasos `1, 5, 10, 12` y Reiniciar.

**Android**: ninguna. **PC hoy**: no. **Esfuerzo**: muy bajo. **Valor**: medio —
contar cajas o viajes de camión es exactamente el caso de obra.

### 1.7 Ruleta — `mini/Ruleta.kt` (sorteo) + `mini/Contador.kt` (`RuletaDoc` :68-91)

Lista de nombres y un sorteo. El sorteo es el mismo que usa el pin, con el azar
inyectado como `() -> Double` para poder probarlo (`Ruleta.kt:5-13, 48-52`).

**Formato exacto** (`RuletaDoc.escribir`, `Contador.kt:72-73`): **un nombre por
línea, a pelo, sin viñetas ni claves**.

```
# Quién friega

Ana
Luis
Marta
```

Al leer es tolerante: `Ruleta.nombres` (`Ruleta.kt:33-38`) parte por
**`\n`, `,` o `;`**, quita blancos, **conserva los repetidos** (doble
probabilidad, razonado :29-31) y corta en `MAXIMO = 40` (:16-17).

**Cómo se ve.** `DeRuleta`, `MiniActivity.kt:626-711`: cartel con el elegido a
24 sp y botón para quitarlo de la lista; lista con `Close`; botón «Sortear» solo
habilitado con ≥ 2 nombres (:691-699). **La pantalla no dibuja rueda ni anima**:
`anguloFinal`/`anguloDePorcion` (`Ruleta.kt:55-73`) solo las usa el pin.

**Android**: `Math.random()`. **PC hoy**: no; el PC ya tiene azar propio en
`crates/pixpin-motor2d/src/azar.rs`. **Esfuerzo**: muy bajo. **Valor**: bajo.

### 1.8 ¿Tiene sentido en escritorio?

| Mini-app | ¿Tiene sentido en el PC? | Por qué |
|---|---|---|
| Tareas | **Sí, la primera** | Se lee y se tacha con el teclado; el modelo ya está escrito |
| Gastos | **Sí** | Teclear importes es más rápido con teclado que con el pulgar |
| Contador | Sí | Barato y útil; el teclado da `+`/`−` gratis |
| Cronómetro | Sí | Un reloj de pantalla grande se ve desde lejos |
| Temporizador | Sí, con aviso | Sin aviso del sistema es media función |
| Ruleta | Regular | Inofensiva y barata, pero nadie sortea en el PC |
| Alarma | **No como función**, sí como formato | Despertarse con el ordenador no es un caso real; leer y **conservar** el documento sí |

---

## 2. Grabar y el audio

### 2.1 Grabar una nota de voz — `pin/Voz.kt`

`MediaRecorder` (:55-71), fuente `MIC` (:61), contenedor **MPEG-4** (:62), códec
**AAC** (:63), **22 050 Hz** (`MUESTREO` :140), **32 000 bps** (`BITS_POR_SEGUNDO`
:141). **`setAudioChannels` no se llama nunca**: mono es el defecto implícito.
Extensión **`.m4a`** (:40), en `filesDir/voz` (:35-36) — almacén privado, no la
galería. Duración con `MediaPlayer.prepare()/duration` (:125-135); mínimo
aceptable `MINIMO_MS = 700` (:138).

`parar()` (:80-85) devuelve `false` si `stop()` lanza —grabación demasiado corta,
fichero ilegible— y quien llama borra
(`pin/GrabadoraActivity.kt:134-151`).

### 2.2 Los picos de la onda — `pin/Voz.kt` + `guardados/Onda.kt`

Esto es lo que el PC **ya pinta** y conviene no desviarse ni un número:

- Captura: `Voz.pico(grabador)` = `MediaRecorder.getMaxAmplitude()`, entero en
  **0..32767**, máximo *desde la llamada anterior* (`Voz.kt:111-114`).
- Cadencia: **`MS_ENTRE_PICOS = 50` ms** (20 por segundo) (`Voz.kt:122`).
- Acumulador `class Picos(tope = 256)` (`Onda.kt:169-192`): guarda 1 de cada
  `paso`; al llegar al tope promedia de dos en dos (`aMitad` :147-158) y **dobla
  el paso** (:185). Así una nota de 30 s y una de 10 min ocupan lo mismo.
- **Tope: `PICOS_GUARDADOS = 256`** (`Onda.kt:96`).
- **Se guardan crudos**: `picos: List<Int>` del `Mensaje`
  (`guardados/Mensajes.kt:136`), array JSON de enteros 0..32767 dentro de la línea
  del JSONL. **No se normalizan ni se cuantizan a 5 bits** como hace Telegram; la
  normalización es al pintar (`aBarras` `Onda.kt:113-130`, contra el máximo de esa
  nota, suelo `0.05f` :86).
- Geometría: **50 barras** (`BARRAS_DE_LA_ONDA` `Onda.kt:43`), grueso 2 dp (:65),
  paso 3 dp (:68).

**El PC ya hace esto y hace bien**: `crates/pixpin-ui/src/chat.rs:1632-1683`
(`ONDA_BARRAS = 50`, `ONDA_PASO = 3`, `barras_de_onda` con máximo por tramo, no
media) y `apps/pixpin/src/ventana_chat.rs:3511-3545` (`pintar_onda`). Los picos
se leen de `resto["picos"]` (`ventana_chat.rs:7189-7200`) porque el `Mensaje` de
Rust no declara ese campo. **Si el PC llega a grabar, tiene que escribir `picos`
con ese mismo contrato**: enteros 0..32767, máximo 256, uno cada 50 ms con el
promediado a la mitad.

**⚠ Hallazgo**: `pin/GrabadoraActivity.kt` **no** captura picos — los pines de
voz de esa pantalla salen sin onda. El único sitio que los rellena es
`guardados/TelepronterActivity.kt:199-200, 223, 246`.

### 2.3 El reproductor y su barra — `guardados/Reproductor.kt` + `BarraDelReproductor.kt`

**Un singleton para toda la aplicación** (razonado :9-19), sobre `MediaPlayer`.
Estado (`data class Estado` :23-30): `ruta`, `titulo`, `sonando`, `posicionMs`,
`duracionMs`, `velocidad = 1f`, expuesto como `MutableStateFlow` (:32) que
consumen la barra, las burbujas, la biblioteca y la pantalla de letra. Latido de
posición cada **250 ms** (:41-51).

**Velocidades: `listOf(1f, 1.25f, 1.5f, 2f, 0.75f)` (:38) — son cinco, no tres**,
y se recorren en ciclo (`otraVelocidad` :128-135). Se aplican con
`PlaybackParams().setSpeed()` **justo antes de `start()`** (:84-86) y
**sobreviven** a cambiar de pista y a `parar()` (:76, :101-106). Salto
`SALTO_MS = 10_000` (:35), `saltar` :109-115, `irA(fraccion)` :118-125.

La barra (`BarraDelReproductor.kt:46-106`): no pinta nada sin ruta (:48); mientras
se arrastra el `Slider` **sigue al dedo, no al reloj** (`arrastrando` :50,
:97-105); Replay10 / Play-Pause / Forward10 / velocidad / cerrar (:75-93);
`velocidadLegible` escribe `1,5×` con coma (:113-114); `duracionLegible` está en
`pin/Voz.kt:150-162` y aguanta horas (`1:02:09`).

**PC hoy**: **no reproduce nada.** La onda se pinta «sin borde de avance porque
aquí no se reproduce» (`ventana_chat.rs:3505-3508`). No hay `cpal`, `rodio`,
`symphonia` ni Media Foundation en ningún `Cargo.toml`.

### 2.4 Transcripción — `guardados/Transcriptor.kt` + tres motores

**Ninguno de los tres es la nube.** Los tres corren en el aparato; la red solo
sirve para bajar modelos.

| Motor | Fichero | Dónde | Modelos |
|---|---|---|---|
| **Vosk** (Kaldi, Apache-2) — **el de por defecto** | `guardados/MotorVosk.kt` | En el aparato, offline tras la 1ª descarga | `vosk-model-small-{es,en-us,pt,fr,de,it,ca}` (:32-40), ~40 MB, de `alphacephei.com/vosk/models/` (:41) a `filesDir/vosk/<modelo>` |
| **Whisper** vía sherpa-onnx (Apache-2) | `guardados/MotorWhisper.kt` | En el aparato, solo arm64-v8a | `tiny`/`base`/`small` (104/161/375 MB, :48), por defecto `tiny` (:49), de `huggingface.co/csukuangfj/sherpa-onnx-whisper-*` (:54) |
| **Google** | `guardados/MotorGoogle.kt` | **En el aparato también**: `createOnDeviceSpeechRecognizer` (:78, 104, 165) + `EXTRA_PREFER_OFFLINE = true` (:170). Pide Android 13 | Modelos del propio sistema |

El motor sale de Ajustes, con **Vosk** como caída (`Transcriptor.kt:231`).

**Preproceso común** (:431-542): cualquier formato (`.m4a`, `.ogg`, `.mp3`,
`.wav`, `.amr`) se decodifica con `MediaExtractor`+`MediaCodec` a **PCM 16 bit,
mono, 16 kHz** (`HERCIOS = 16_000` :32), con remuestreo por interpolación lineal
y mezcla a mono (`Remuestreador` :502-542).

**Troceado.** `cortes(pcm)` (:343-394) parte **por frases según energía**:
ventanas de 20 ms (:330), umbral = `max(120.0, percentil60 × 0.12)` (:361-363),
silencio mínimo 250 ms (:331), y se corta **en mitad del silencio** (:371); los
trozos de más de 8 s (:328) se parten en su `puntoMasCallado` buscando en los
últimos 4 s (:329, :397-418). `agrupar` (:269-279) junta frases en tandas de
hasta **20 s** para Whisper y Google; **Vosk no trocea**, come bloques de 8000
bytes (`MotorVosk.kt:164-171`).

**Anti-alucinación de Whisper** (`creible` :298-324): descarta alfabetos ajenos
(≥ 20 % de letras raras), una lista `JUNK` («Subtítulos realizados por…
amara.org», «suscríbete», «thanks for watching», :284-288) y 8 repeticiones
seguidas de la misma palabra.

**Dónde se guarda** (`MensajesStore.kt:183-189`):

- **`Mensaje.transcripcion: String?`** ← el texto con marcas de tiempo, formato
  `[1:23] lo que se dijo` separado por `\n\n` (`conTiempos` `Transcriptor.kt:66-87`).
- `Mensaje.estadoDelTexto: String?` ← `"bien"` / `"aviso"` / `"mal"`, más el
  valor especial `"letra"` que marca «es música» (`Mensajes.kt:590-594, 601`).
- `Mensaje.hojaDelTexto: String?` ← id de la hoja de notas del proyecto donde
  también queda.
- Vecinos: `marcas: List<Int>` (banderitas en ms) y `turnos: List<TurnoDeVoz>`.

**No se transcribe sola**: `MensajesStore.anadir` lleva `transcribir=false` por
defecto (:147) y hoy solo lo pide «practicar la pronunciación».

**PC hoy**: `Mensaje.transcripcion` **ya existe** en Rust
(`crates/pixpin-proyecto/src/cuaderno.rs:134-135`) y **ya se usa**: `resumen()`
la prefiere al texto (:179-190) y la búsqueda busca sobre el resumen
(`ventana_chat.rs:3868-3870`). Lo que no hay es quien la produzca:
`ventana_chat.rs:8158-8168` saca «Pasar a texto» y «Letra o texto» con aviso.

### 2.5 «Leer en voz alta»: **no es lo que parece**

**En PixPin Android no hay `TextToSpeech` en ningún sitio.** Se buscó
`TextToSpeech|speak(|UtteranceProgress|setSpeechRate` sobre todo el código
descargado: cero aciertos. Lo único que hay de `android.speech.*` es
`SpeechRecognizer` (reconocer, no sintetizar).

«Leer en voz alta» en esta aplicación significa que **el usuario lee mientras el
teléfono graba**: es la entrada de menú que abre el teleprónter o el pronunciador
(`guardados/MensajesActivity.kt:1689-1691`; `TelepronterActivity.kt:86`).

Si algún día se quiere síntesis de verdad en el PC, **hay que inventarla**: no se
porta nada. En Windows sería SAPI5 (`ISpVoice::Speak`) o
`Windows.Media.SpeechSynthesis.SpeechSynthesizer`.

### 2.6 Cámara y vídeo en el chat

**No hay nada de cámara ni de vídeo en el chat de Android.** Lo que sale al
buscar «cámara» es `croquis3d/Camara3D.kt`, `croquis3d/Croquis3DCamara.kt` y
`ui/theme/Camara.kt`: son **cámaras de vista 3D**, no el objetivo del teléfono.
`Clase` no tiene un valor `VIDEO` (`Mensajes.kt:35-73`: `NOTA, IMAGEN, ARCHIVO,
VOZ, DIBUJO, PAGINA, PROYECTO, MINIAPP, TABLA, CROQUIS`). El PC, en cambio, sí
tiene vídeo propio: `crates/pixpin-record` (MP4) y `crates/pixpin-pin/src/video.rs`.
**No hay nada que portar aquí.**

### 2.7 `motor/AudioLigero.kt`

Recomprime un audio para que pese menos al exportar a HTML (:11-22). Calidades
`original` / `ligero` (32 000 bps) / `ultraligero` (12 000 bps) / `sin` (:49-60),
a **mono 16 kHz** (:54), reutilizando `Transcriptor.decodificar()` (:67) y
codificando AAC con `MediaCodec`+`MediaMuxer` a `.m4a` (:75-134). `hayAudioEn`
(:33-41) decide si siquiera preguntar por la calidad al exportar. **Solo importa
si el PC exporta a web con audios dentro; hoy no graba ninguno.**

---

## 3. Lo demás del chat que el PC no tiene (`guardados/` entero)

### 3.1 Recordatorios

Un mensaje al que se le pone hora; cuando llega, sale como pin.
**Se guarda en `Mensaje.recuerdaEn: Long?`** (epoch ms, `Mensajes.kt:283`) — nada
más, no hay fichero de alarmas. Android usa `AlarmManager.setAlarmClock`
(`pin/Recordatorios.kt:35-46`), `TimePickerDialog`
(`pin/HoraDelRecordatorioActivity.kt:33-41`) y `RecordatorioReceiver`
(`pin/RecordatorioReceiver.kt`) con prefijos `"mini:"` y `"msg:"` (:17-29). Al
vencer un `msg:` **borra la alarma fantasma** (`recuerdaEn = null`, :58) y, si es
una nota de voz, abre la «llamada secreta» (:64).

**PC hoy**: aviso en `ventana_chat.rs:8100`. **Windows**: no hace falta ninguna
API especial —un hilo con el instante objetivo y la ventana de pin que ya
existe—; para que suene con el PC bloqueado, `ToastNotification`.
**Valor alto, esfuerzo bajo-medio.** Es lo más barato de toda esta lista.

### 3.2 Aligerar el PDF — `pdf/ComprimirPdf.kt`

Baja las fotos de dentro del PDF a **200 ppp** y las reguarda en JPEG, sin tocar
texto ni vectores, y **nunca a peor**: si una foto no gana un cuarto se deja, y
si el fichero entero no baja un 15 % o deja de leerse, se queda el original
(:15-30). Reescribe el documento entero en vez de rehacerlo por páginas, para no
perder marcadores ni formularios (:31-37).

**No guarda datos nuevos**: sustituye el fichero adjunto. **PC hoy**: aviso en
`ventana_chat.rs:8054-8055`. `crates/pixpin-pdf` sabe leer y escribir PDF pero no
recomprime. **Windows**: nada del sistema — es trabajo puro de `pixpin-pdf` +
`pixpin-codec` (que ya codifica JPEG). **Valor medio, esfuerzo alto.**

### 3.3 Teleprónter — `guardados/TelepronterActivity.kt` (540 líneas)

El texto baja solo mientras uno lo lee en voz alta **grabándose**, y se cronometra
en qué milisegundo cada párrafo cruza la franja de lectura (al 30 % de la
pantalla, :505; captura en :178-186).

**Qué guarda** (`terminar()` :221-260): un cuerpo con cada párrafo prefijado por
`[m:ss] ` (:230-233), una hoja de notas del proyecto (:239), el `.m4a` como
adjunto `lectura-<ahora>.m4a` (:240) y **un `Mensaje` de `clase = VOZ`** con
`ruta`, `duracionMs`, **`picos`**, `transcripcion` = el cuerpo,
`estadoDelTexto = "bien"` y `hojaDelTexto` (:244-250). **No pasa por ningún
reconocedor**: los tiempos salen del desplazamiento.

Controles: tamaño 16-48 sp, elegir fuente, velocidad 10-150 dp/s (por defecto 45)
con nombre en palabras (:531-537), Ensayar/Pausa, Grabar↔Terminar, cuenta atrás
de 3 a 96 sp (:411-433). El texto se saca de las notas de los proyectos y de los
`.md`/`.txt` del chat (`fuentes()` :482-500).

**PC hoy**: aviso en `ventana_chat.rs:7879`. **Windows**: depende de grabar audio
(§2.1). Todo lo demás —desplazamiento, cronometrado, marcas— es aritmética.
**Valor medio, esfuerzo alto** (depende de la grabación).

### 3.4 Pronunciar — `guardados/PronunciarActivity.kt` (485 líneas)

Mantener pulsado → hablar → soltar → oírse al momento → repetir. Cada toma pisa
la anterior. **No hay TTS**: se graba con `Voz` (:190-191) y se oye con
`MediaPlayer` (:176-184). Guía opcional: texto, imagen o PDF del teléfono o del
chat (:135-140, :167, :458-466).

**Qué guarda** (:210-226): el `.m4a` como `pronunciar-<ahora>.m4a` y un `Mensaje`
`clase = VOZ` con `ruta`, `duracionMs` y `proyecto` — **sin picos y sin
transcripción**, porque la pone luego el transcriptor: es **el único sitio de la
aplicación que pide transcribir automáticamente** (`transcribir = true` + idioma,
:219-222, documentado en `MensajesStore.kt:76-79`).

**PC hoy**: aviso en `ventana_chat.rs:7884`. **Windows**: grabar (§2.1) +
reproducir (§2.3) + transcribir (§2.4). **Valor bajo en escritorio** (practicar
pronunciación se hace con el móvil en la mano), **esfuerzo alto**.

### 3.5 Conversación — `guardados/ConversacionActivity.kt` + `Conversaciones.kt`

Graba una reunión **por turnos**: de 2 a 6 personas con nombre, cada una con su
micrófono grande; se mantiene pulsado el propio para hablar (:215-252).

**Qué guarda** (`terminar()` :283-333): une los `.m4a` **sin recodificar** con
`MediaExtractor`+`MediaMuxer` corriendo el `presentationTimeUs` (:365-404),
transcribe turno a turno, compone líneas con `**Nombre:** ` de prefijo (:299),
apunta la hoja (:308) y guarda un `Mensaje` `clase = VOZ` con `transcripcion`,
`estadoDelTexto` según cuántas líneas salieron frente a cuántos turnos hubo, y
**`turnos: List<TurnoDeVoz>`** (`{quien, desdeMs, hastaMs}`, `Mensajes.kt:298`)
que es lo que permite re-transcribir con los nombres puestos (:314-327).

`Conversaciones.kt` es otra cosa y es **puro**: la lista de chats al estilo
Telegram, en una sola pasada, con la general siempre la primera (:47-87). **El PC
ya tiene su equivalente** (`Accion::Proyectos`, `ventana_chat.rs:7926-7930`).

**PC hoy**: aviso en `ventana_chat.rs:7874`. **Valor medio** (un acta de reunión
de obra es útil), **esfuerzo muy alto**: grabar + unir + transcribir.

### 3.6 Biblioteca de audio — `guardados/BibliotecaDeAudioActivity.kt`

**Una vista, no un almacén** (:47-56). No guarda nada propio: filtra
`clase == VOZ && ruta != null && !enBuzon` de `MensajesStore` y los parte en dos
grupos, música (`estadoDelTexto == "letra"`) y notas (:67-73, :96-105). Un toque
reproduce; arriba, la barra del reproductor, cuyo título abre `LetraActivity`
(:84-86).

**Ojo**: `motor/Biblioteca.kt` y `motor/BibliotecaStore.kt` **no son esto** — son
la biblioteca de figuras de dibujo (`filesDir/pins/draw/figuras.json`).

**PC hoy**: aviso en `ventana_chat.rs:7925`. **Es puro filtrado y una lista: la
parte barata está hecha en cuanto haya reproductor.** **Valor medio, esfuerzo
bajo** una vez exista §2.3.

### 3.7 Letra o texto — `guardados/LetraActivity.kt`

Pantalla de la transcripción o la letra, con tamaño de fuente regulable, marcas
(`Flag`) y saltar al minuto tocando una línea. **PC hoy**: aviso en
`ventana_chat.rs:8167`. Depende de §2.3 y §2.4.

### 3.8 Lo que Windows no puede dar

Nada de esta lista es imposible en Windows. Lo que **no** tiene equivalente es:

- **`setAlarmClock` con Doze**: Windows no tiene Doze, así que el problema
  desaparece; un hilo basta mientras la aplicación corra, y un `ToastNotification`
  programado si debe saltar con la aplicación cerrada.
- **`SpeechRecognizer` on-device de Google**: no existe.
  `Windows.Media.SpeechRecognition.SpeechRecognizer` sí es local y viene con el
  sistema, pero está pensado para dictado en vivo y pide el paquete de idioma
  instalado. **Recomendación**: usar **sherpa-onnx o whisper.cpp**, que son los
  mismos modelos Apache-2 que el móvil ya baja, y así la transcripción da el
  mismo texto en los dos aparatos.
- **`MediaRecorder`/`MediaCodec`/`MediaMuxer`**: en Windows, **WASAPI** para
  capturar (o el crate `cpal`) y **Media Foundation** (`IMFSinkWriter` con el
  codificador AAC del sistema) para escribir el `.m4a`. Para leerlos,
  `symphonia` con las funciones `isomp4` + `aac`, o Media Foundation.
- **TTS**: no se porta porque no existe en Android; si se quiere, SAPI5.

---

## 4. Tabla, ordenada por valor y esfuerzo

| # | Cosa | Android (fichero:líneas) | Dato que viaja | PC hoy | Windows | Valor | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 1 | **Mini-app tareas (pantalla)** | `mini/Tareas.kt:39-213`, `mini/MiniActivity.kt:169-223` | `texto`+`miniapp="tareas"` | **Modelo listo y sin usar**: `mini.rs` entero | ninguna | **Alto** | **Muy bajo** |
| 2 | **Recordatorios** | `Mensajes.kt:283`, `pin/Recordatorios.kt:35-46` | `recuerdaEn` (cuaderno) | aviso, `ventana_chat.rs:8100` | hilo + `ToastNotification` | **Alto** | Bajo |
| 3 | Mini-app contador | `mini/Contador.kt:26-60`, `MiniActivity.kt:571-617` | `texto`+`miniapp="contador"` | no | ninguna | Medio | Muy bajo |
| 4 | Mini-app cronómetro | `mini/Tiempos.kt:30-43,103-124`, `MiniActivity.kt:364-428` | `texto`+`miniapp="cronometro"` | no | ninguna | Medio | Bajo |
| 5 | Mini-app ruleta | `mini/Ruleta.kt`, `mini/Contador.kt:68-91` | `texto`+`miniapp="ruleta"` | no | ninguna | Bajo | Muy bajo |
| 6 | **Mini-app gastos** | `mini/Gastos.kt:99-383`, `MiniActivity.kt:226-354` | `texto`+`miniapp="gastos"` | no | ninguna | **Alto** | Medio |
| 7 | Mini-app temporizador | `mini/Tiempos.kt:46-64,128-155` | `texto`+`miniapp="temporizador"` | no | aviso del sistema | Medio | Bajo |
| 8 | Mini-app alarma | `mini/Tiempos.kt:67-71,159-175` | `texto`+`miniapp="alarma"` | no | aviso del sistema | Bajo | Bajo |
| 9 | **Reproducir notas de voz** | `guardados/Reproductor.kt:23-141`, `BarraDelReproductor.kt:46-106` | nada nuevo | **no reproduce**, solo pinta la onda (`ventana_chat.rs:3511`) | Media Foundation, o `symphonia`+`cpal` | **Alto** | Medio |
| 10 | Biblioteca de audio | `guardados/BibliotecaDeAudioActivity.kt:67-139` | nada (es una vista) | aviso, `ventana_chat.rs:7925` | la de arriba | Medio | Bajo (tras 9) |
| 11 | Letra o texto | `guardados/LetraActivity.kt` | lee `transcripcion`, `marcas` | aviso, `ventana_chat.rs:8167` | la de arriba | Medio | Bajo (tras 9) |
| 12 | **Grabar notas de voz + picos** | `pin/Voz.kt:35-141`, `guardados/Onda.kt:96,169-192` | `ruta`, `duracionMs`, **`picos`** | no | **WASAPI** + **Media Foundation (AAC)** | **Alto** | Alto |
| 13 | Transcribir | `guardados/Transcriptor.kt`, `MotorVosk/Whisper/Google.kt` | `transcripcion`, `estadoDelTexto`, `hojaDelTexto` | `transcripcion` se lee (`cuaderno.rs:134`) pero nadie la escribe | sherpa-onnx/whisper.cpp (**no** la nube) | Medio | Muy alto |
| 14 | Aligerar el PDF | `pdf/ComprimirPdf.kt` | sustituye el adjunto | aviso, `ventana_chat.rs:8054` | ninguna: `pixpin-pdf`+`pixpin-codec` | Medio | Alto |
| 15 | Teleprónter | `guardados/TelepronterActivity.kt:221-260` | `VOZ` + `picos` + `transcripcion` + `hojaDelTexto` | aviso, `ventana_chat.rs:7879` | depende de 12 | Medio | Alto |
| 16 | Conversación | `guardados/ConversacionActivity.kt:283-404` | `VOZ` + **`turnos`** + `transcripcion` | aviso, `ventana_chat.rs:7874` | depende de 12 y 13 | Medio | Muy alto |
| 17 | Pronunciar | `guardados/PronunciarActivity.kt:210-226` | `VOZ`, transcrito después | aviso, `ventana_chat.rs:7884` | depende de 12 y 13 | Bajo | Alto |
| 18 | «Leer en voz alta» (TTS) | **no existe en Android** | — | no | SAPI5 / `SpeechSynthesis` | Bajo | Medio |
| 19 | Cámara / vídeo en el chat | **no existe en Android** | — | el PC ya tiene `pixpin-record` | — | — | — |

---

## 5. Reparto en tres grupos que no comparten ficheros

**Lo que ya tiene otro agente**, y que por tanto **no aparece en ningún grupo**:
`apps/pixpin/src/ventana_chat.rs` y todo `crates/pixpin-ui/*` (está retomando
«Abrir con», renombrar y grupos de ventanas).

Eso obliga a una regla: **los tres grupos entregan núcleo puro y módulos nuevos,
y dejan el enganche a la pantalla anotado al final de su trabajo, para una tanda
de costura posterior cuando `ventana_chat.rs` se libere.** Un módulo nuevo bajo
`apps/pixpin/src/` sí se puede crear (fichero propio); lo que no se puede es
editar `ventana_chat.rs` ni `main.rs` a la vez que el otro agente.

### Grupo A — Las seis mini-apps que faltan (núcleo puro)

Trabajo de dominio, sin pantalla, sin sistema operativo, todo comprobable en
`cargo test`. Es el grupo con mejor relación valor/esfuerzo.

**Ficheros (todos nuevos o exclusivos de este grupo):**
- `crates/pixpin-proyecto/src/mini.rs` — **ampliar**: añadir las constantes
  `GASTOS`, `CRONOMETRO`, `TEMPORIZADOR`, `CONTADOR`, `RULETA`, `ALARMA` y un
  `Resumen` equivalente a `ResumenMini`.
- `crates/pixpin-proyecto/src/mini/gastos.rs` (nuevo)
- `crates/pixpin-proyecto/src/mini/tiempos.rs` (nuevo: cronómetro, temporizador,
  alarma)
- `crates/pixpin-proyecto/src/mini/contador.rs` (nuevo: contador y ruleta)
- `crates/pixpin-proyecto/src/lib.rs` — solo la línea del `mod`.

**Entrega**: leer y escribir los siete documentos con ida y vuelta exacta contra
los ejemplos literales de §1, más el resumen de burbuja de cada una. **Ni una
línea de interfaz.**

### Grupo B — Oír: reproductor, biblioteca y letra

**Ficheros (nuevos, y un crate nuevo):**
- `crates/pixpin-audio/` (**crate nuevo**): `lib.rs`, `salida.rs` (decodificar
  `.m4a`/AAC y sacarlo por la tarjeta), `reloj.rs` (el `Estado` del §2.3:
  `ruta`, `sonando`, `posicion_ms`, `duracion_ms`, `velocidad`, las **cinco**
  velocidades `[1.0, 1.25, 1.5, 2.0, 0.75]`, el salto de 10 s, `ir_a(fraccion)`).
- `apps/pixpin/src/audio.rs` (nuevo): el singleton del reproductor.
- `apps/pixpin/src/biblioteca_audio.rs` (nuevo): el filtrado de §3.6, que es
  puro y se puede probar sin ventana.
- `Cargo.toml` raíz — solo la línea del miembro nuevo.

**Entrega**: reproducir un `.m4a` del móvil con posición, salto y velocidad, y la
lista de la biblioteca calculada. La **barra** y la pantalla de letra se dejan
anotadas para la tanda de costura (viven en `pixpin-ui`/`ventana_chat.rs`).

### Grupo C — Recordatorios y aligerar el PDF

Dos cosas sin relación entre sí, juntas porque **no rozan ningún fichero de los
otros dos grupos**.

**Ficheros:**
- `apps/pixpin/src/recordatorios.rs` (nuevo): leer y escribir
  `resto["recuerdaEn"]`, saber cuál vence antes, y el hilo que despierta.
- `crates/pixpin-shell/src/aviso.rs` (nuevo): el `ToastNotification` de Windows.
- `crates/pixpin-pdf/src/aligerar.rs` (nuevo) + la línea del `mod` en
  `crates/pixpin-pdf/src/lib.rs`: las reglas de §3.2 (200 ppp, nunca a peor, el
  15 % de mínimo).

**Entrega**: un recordatorio puesto sobrevive a cerrar y abrir, y un PDF de
escaneo baja de peso sin perder texto ni vectores.

### El único roce, y cómo se evita

`crates/pixpin-store/i18n/es-ES/main.ftl` y `.../en-US/main.ftl` son **un solo
fichero por idioma** (`crates/pixpin-store/src/idioma.rs:16-17`, con `include_str!`
y una prueba que exige las mismas claves en los dos, :246-247). Los tres grupos
querrán añadir rótulos.

Salida: **que ninguno de los tres toque el `.ftl`**. Los rótulos nuevos se
apuntan en el mensaje de entrega de cada grupo y se añaden todos juntos en la
tanda de costura, con `ventana_chat.rs`. Si hiciera falta antes, el primer paso
de esa tanda es partir el `.ftl` en varios (`main.ftl`, `voz.ftl`, `mini.ftl`) y
juntarlos en `idioma.rs`.

---

## 6. Riesgos de compatibilidad

La sincronización ya funciona. Lo que sigue es lo que **no** se puede romper.

### 6.1 La buena noticia: el esquema ya está completo

`crates/pixpin-sincro/src/kotlin.rs:84-117` declara el `Mensaje` de Android campo
a campo **y ya trae los 32 campos de la v0.72**, incluidos `picos`, `soloLaFoto`,
`unido`, `estadoDelTexto`, `hojaDelTexto`, `marcas`, `turnos`, `recibidoDe`,
`vieneDe` y `recuerdaEn`. `CLASES` (:64-67) trae las diez, `MINIAPP` incluida.
**Se comprobó campo por campo contra `guardados/Mensajes.kt:115-284` de la v0.72:
no falta ninguno y el orden coincide.** No hay que añadir nada al esquema.

Y el `Mensaje` de Rust, que declara menos campos, **no pierde los demás**:
`#[serde(flatten)] pub resto` (`cuaderno.rs:150-153`), con una prueba que lo
demuestra (:514-523). Por eso `picos` se lee de `resto`
(`ventana_chat.rs:7189-7200`) y por eso `unido` y `vieneDe` también (:3708-3711,
:7333-7337).

### 6.2 Lo que sí puede romperse

1. **El resumen de sincronización es un hash del JSON canónico**
   (`kotlin.rs:3-10`): un campo de menos, o uno escrito con otro tipo, y **todo
   lo nacido en el PC se vería cambiado en cada vuelta**. Regla: **todo mensaje
   nuevo se crea con `cuaderno::Mensaje::*` y se escribe con `cuaderno::anadir`**
   (`cuaderno.rs:235-301`), nunca a mano. La prueba
   `una_nota_escrita_en_el_pc_sale_con_todo_lo_que_pondria_kotlin`
   (`kotlin.rs:368-382`) es la que lo vigila; conviene añadirle un caso de `VOZ`
   con `picos` y otro de `MINIAPP` de tareas.

2. **`picos` es `List<Int>`, no `List<Long>` ni flotantes.** Si el PC graba, tiene
   que escribir enteros crudos **0..32767**, **como mucho 256**, uno cada 50 ms
   con el promediado a la mitad de `Onda.kt:177-188`. Un array normalizado a
   `[0,1]` lo lee el móvil como todo ceros y la onda desaparece.

3. **`duracionMs` es `Int` en Kotlin** (`Mensajes.kt:128`,
   `kotlin.rs:94` → `T::Entero`) y `i64` en Rust (`cuaderno.rs:124`). Hoy da
   igual; el día que el PC escriba duraciones, un valor fuera del rango de `Int`
   hace que **kotlinx no lea la línea entera**.

4. **Los documentos de mini-app se escriben con la ortografía del móvil o no se
   leen.** Tres trampas reales:
   - Alarma: `activa: sí` **con tilde** (`Tiempos.kt:172`); `si` se lee como
     `false` (:166).
   - Alarma: la hora **sin** rellenar a dos cifras (`7:05`), el minuto **con**
     (`Tiempos.kt:170`).
   - Temporizador: se escribe `finEn` pero se lee `finen`, porque `valores()`
     baja la clave (`Tiempos.kt:84, 131`).
   - Gastos: el importe **canónico** con punto y sin miles (`Gastos.kt:265-266`),
     y el escape `\\`/`\|` en las celdas (:363-364).

5. **La palabra de `miniapp` es un dato guardado y no se renombra**
   (`mini/MiniApps.kt:94-99`, `tabla.rs:32-35`). Y al revés: una palabra que
   Android no conozca **no tumba nada** (`MiniApp.de` :206-213) — es el hueco por
   el que viaja la hoja de cálculo del PC.

6. **Cuidado con la hoja de cálculo del PC.** Su documento es **JSON** dentro de
   `texto` (`tabla.rs:38-55`), no Markdown. Es legal, pero en el móvil se ve como
   un churro de JSON en la burbuja. Si alguna vez molesta, la salida es escribirla
   también como tabla de Markdown, igual que los gastos, y no tocar `miniapp`.

7. **Hoy el PC rotula «Tabla» cualquier mini-app.** `ventana_chat.rs:7296`
   (`Some(Clase::MiniApp) if nombre.is_empty() => textos.t("chat-tabla")`) y
   :7242 miran **la clase, no la palabra**. Una lista de tareas llegada del móvil
   aparece etiquetada «Tabla». No rompe el fichero —el texto se conserva— pero es
   una mentira en pantalla, y es lo primero que hay que arreglar en la tanda de
   costura.

8. **Al editar en el sitio hay que usar `cuaderno::reemplazar`**
   (`cuaderno.rs:303-315`), que **copia tal cual las líneas que no entiende**. Es
   la única forma de marcar una casilla sin perder lo que escribió una versión más
   nueva del móvil. Android hace lo mismo con `MensajesStore.reescribir`
   (`MensajesStore.kt:275-306`), que además **anota los borrados** para que otro
   aparato no los resucite (:280-284).

9. **Campos nuevos que el PC tendría que empezar a escribir** si porta estas
   funciones, todos ya reconocidos por `kotlin.rs`: `picos` y `duracionMs` (nota
   de voz), `transcripcion` + `estadoDelTexto` + `hojaDelTexto` (transcribir),
   `marcas` (letra), `turnos` (conversación), `recuerdaEn` (recordatorios). **No
   hace falta inventar ninguno.**
