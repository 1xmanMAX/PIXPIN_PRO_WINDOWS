# PixPin Max — Rendimiento en equipos modestos

**Estado:** COMPLETO — aprobado por secciones el 2026-08-31. Pendiente de revisión final del usuario.
**Método:** skill `superpowers:brainstorming` (diseño → aprobación → spec → plan → implementación)
**Alcance:** transversal. Reescribe §4.4 del [documento maestro](2026-08-09-pixpin-pc-master-design.md) y afecta a los seis sub-proyectos.

---

## 1. El problema

El documento maestro fijó un presupuesto de rendimiento (§4.4) pensando en una máquina razonable. La
petición ahora es más dura: **que la app vaya fluida también en equipos con poca RAM y CPU lenta**, y
que lo haga sola, sin que el usuario configure nada.

Eso no es afinar números. Obliga a decidir tres cosas que hoy no están escritas en ningún sitio:

1. Qué hace la app cuando no hay GPU utilizable, o cuando la que hay es peor que la CPU.
2. Qué está dispuesta a ceder cuando el equipo no llega, y qué no cederá nunca.
3. Cómo se comprueba que eso funciona, en vez de esperar que funcione.

### 1.1 La premisa del lenguaje, contrastada

La petición original pedía «usar lenguajes que lleguen directo al núcleo del sistema». **Ya se está en
ese lenguaje**, y la decisión D4 del maestro lo razonó: Rust compila por el mismo backend LLVM que C++,
sin runtime, sin recolector de basura, con intrínsecos SIMD, `#[repr(C)]`, punteros crudos y ensamblador
en línea. No hay ninguna capa entre el código y el procesador que quitar. Añadir C++ propio no compraría
un ciclo; sólo traería errores de memoria que hoy el compilador impide.

El reparto vigente ya es el de máximo rendimiento: **HLSL** para lo masivamente paralelo, **librerías C
por FFI** para lo que ya está afinado a mano, **Rust** para todo lo demás; y en release, LTO completo,
`codegen-units = 1`, `panic = "abort"` y símbolos despojados.

**Conclusión que gobierna este documento:** la velocidad en un equipo modesto no se gana cambiando de
lenguaje, sino **no haciendo trabajo**. Todas las decisiones de aquí abajo son formas de no hacerlo.

## 2. La máquina suelo

Definida por el usuario y disponible físicamente para medir:

| | |
|---|---|
| CPU | Intel Core i3 de 3.ª generación (Ivy Bridge, 2012) — **2 núcleos físicos, 4 lógicos** |
| RAM | **4 GB** |
| Gráficos | Intel HD 4000 integrada, **memoria compartida con el sistema** |
| Sistema | Windows 10 |

**No es un ejemplo: es normativa.** Los números de este documento se confirman midiendo ahí, y el
informe de medición forma parte de la definición de terminado.

### 2.1 Cuatro hechos que este equipo impone

1. **Ivy Bridge tiene AVX, pero no AVX2 ni FMA3** (llegaron con Haswell, la 4.ª generación). Un binario
   compilado con `target-cpu=native` o `x86-64-v3` **muere con instrucción ilegal** en la máquina suelo,
   en el arranque y sin mensaje útil.
2. **Dos núcleos físicos.** Cualquier pool dimensionado a un número fijo, o a núcleos lógicos, compite
   consigo mismo.
3. **La HD 4000 comparte memoria.** Esto corrige media premisa del maestro: «la imagen vive en la GPU y
   no baja a la CPU» ahorra *la copia*, que es real y vale mucho, pero **no ahorra RAM** — la textura
   sale igualmente de los 4 GB. El presupuesto de memoria debe contabilizar la memoria de vídeo como
   memoria del sistema cuando la VRAM dedicada es cero.
4. **4 GB con Windows 10 dejan libres ~1,5-2 GB.** El objetivo «< 40 MB en reposo» era sensato, pero no
   había ningún techo escrito para el **pico** —capturar, editar y guardar a la vez—, que es donde una
   app se lleva por delante un equipo así.

## 3. Decisiones

Continúan la numeración del documento maestro (D1-D12).

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D13 | **Estrategia** | **Dos niveles con degradación automática** (`Completo`/`Ligero`) **más disciplina de recursos permanente** | Un único camino afinado al suelo penalizaría a todas las máquinas; sólo la disciplina no resuelve el caso sin GPU |
| D14 | **Qué se cede** | **El lujo visual. Nunca la latencia ni el archivo final** | Lo que el usuario percibe como calidad es el resultado, no la previsualización |
| D15 | **Quién decide** | **Automático al arrancar, anulable en `pixpinmax.toml`** | La anulación no es un lujo: es lo que permite ejercitar y medir la ruta ligera en cualquier máquina |
| D16 | **Dónde vive la decisión** | Hechos desde Win32 en `pixpin-shell` (L1); política pura en **`pixpin-nivel`** (L0, crate nuevo) | Mismo patrón ya validado con `DisposicionMonitores`: la aritmética delicada se prueba con equipos inventados |
| D17 | **Baseline del procesador** | **`x86-64` fijo y explícito**; SIMD por detección en tiempo de ejecución | Ivy Bridge no tiene AVX2; subir el baseline rompe la máquina suelo sin aviso |
| D18 | **Unidad del presupuesto de memoria** | **Copias vivas de la imagen**, no megabytes absolutos | 40 MB es holgado en 1080p y ridículo en 4K; la pantalla es la unidad que no envejece |
| D19 | **Evidencia** | Dos carriles: puertas automáticas sin GPU + informe manual en la máquina suelo, **commiteado** | Lección de S1-A: «verificado» sólo puede significar «lo ejecuté» |

## 4. Arquitectura: el crate `pixpin-nivel`

**Crate nuevo, L0, `#![forbid(unsafe_code)]`.** Sube el conteo a 16 crates de librería más el ejecutable;
`apps/pixpin/tests/capas.rs` se actualiza.

**Por qué un crate propio y no un módulo:** el nivel lo necesitan `pixpin-render` y `pixpin-gpu`, que
están en L1. `pixpin-store` está en L2 y no puede ser su casa sin romper la regla de capas. Y no encaja
ni en `pixpin-geom` (geometría) ni en `pixpin-model` (documento Excalidraw).

```
crates/pixpin-nivel/
  src/hechos.rs        Hechos del equipo, rellenados desde pixpin-shell
  src/nivel.rs         Nivel, Preferencia, Razon, Decision, decidir()
  src/presupuesto.rs   Presupuesto derivado de Hechos + Nivel
  src/medicion.rs      asignador contador, tras la feature `medicion` (solo dev-dependency)
```

### 4.1 Interfaz

```rust
pub struct Hechos {
    pub ram_fisica_bytes: u64,
    pub nucleos_fisicos: u32,
    pub nucleos_logicos: u32,
    pub vram_dedicada_bytes: u64,
    pub gpu_es_software: bool,
    pub nivel_caracteristica: u32,   // 0xb000 == D3D_FEATURE_LEVEL_11_0
}

pub enum Nivel { Completo, Ligero }
pub enum Preferencia { Auto, Forzado(Nivel) }

pub enum Razon {
    PocaRam, PocosNucleos, GpuPorSoftware,
    NivelCaracteristicaBajo, GraficosIntegradosConPocaRam,
    ForzadoPorAjuste,
}

pub struct Decision { pub nivel: Nivel, pub razones: Vec<Razon> }

pub fn decidir(hechos: &Hechos, preferencia: Preferencia) -> Decision;

pub struct Presupuesto {
    pub copias_vivas_max: u8,
    pub cache_bytes_max: u64,
    pub hilos_trabajo: u32,
    pub ram_reposo_objetivo_bytes: u64,
}

impl Presupuesto {
    pub fn desde(hechos: &Hechos, nivel: Nivel) -> Presupuesto;
}
```

`Vec<Razon>` asigna, y está bien: ocurre una vez al arrancar, nunca en el camino caliente.

Los cuatro campos de `Presupuesto` se derivan así, y las fórmulas están en 6.1 y 6.2:
`copias_vivas_max` y `cache_bytes_max` dependen del **nivel** y de la RAM física; `hilos_trabajo` depende
sólo de los **núcleos físicos**, no del nivel — un equipo potente forzado a `Ligero` sigue teniendo los
núcleos que tiene, y limitarlos no haría la app más fluida, sólo más lenta.

### 4.2 Recogida de hechos — `pixpin-shell` (L1, `unsafe` permitido)

Cuatro llamadas baratas, del orden de microsegundos:

| Dato | API |
|---|---|
| RAM física | `GlobalMemoryStatusEx` → `ullTotalPhys` |
| Núcleos físicos y lógicos | `GetLogicalProcessorInformationEx` (`RelationProcessorCore`) |
| VRAM dedicada, adaptador software | `IDXGIFactory1::EnumAdapters1` → `DXGI_ADAPTER_DESC1` |
| Nivel de característica | el que devuelve `D3D11CreateDevice` |

**Nada de micro-benchmarks en el arranque:** serían no deterministas y se comerían el presupuesto de
300 ms hasta la bandeja. Los hechos bastan.

### 4.3 La regla de decisión

Cae a `Ligero` si se cumple **cualquiera**:

| Condición | Razón |
|---|---|
| `ram_fisica_bytes` < 6 GiB | `PocaRam` |
| `nucleos_fisicos` <= 2 | `PocosNucleos` |
| `gpu_es_software` | `GpuPorSoftware` |
| `nivel_caracteristica` < 0xb000 | `NivelCaracteristicaBajo` |
| `vram_dedicada_bytes` == 0 **y** `ram_fisica_bytes` < 8 GiB | `GraficosIntegradosConPocaRam` |

La máquina suelo cae por **dos razones independientes** (`PocaRam` y `PocosNucleos`), y eso es
deliberado: un test permanente afirma que un i3 de 3.ª generación con 4 GB da `Ligero`, y con dos
razones distintas el test no se vuelve frágil si mañana se afina un umbral.

**Los cinco umbrales son provisionales** hasta que se midan en la máquina suelo. La spec se corrige con
el número medido y el comando que lo produjo.

### 4.4 El nivel se decide una vez y viaja como dato

Se decide **al arrancar** y no cambia en caliente: un nivel que muta a mitad de sesión crearía rutas
mixtas imposibles de reproducir y de medir. Un cambio del ajuste se aplica al reiniciar.

`Nivel` y `Presupuesto` se pasan **por parámetro** desde `apps/pixpin`. Sin estado global, sin
`static mut`, sin `thread_local`. Así cualquier test construye el nivel que quiera y ejercita la ruta
ligera en cualquier máquina — que es lo único que evita que esa ruta se pudra sin usarse.

En `pixpinmax.toml`:

```toml
[rendimiento]
nivel = "auto"      # auto | completo | ligero
```

Al arrancar, `tracing` deja una línea con hechos y razones, al fichero local de siempre:
*«nivel ligero: RAM física 4,0 GB (< 6), núcleos físicos 2 (<= 2), GPU integrada sin VRAM dedicada»*.
Cero telemetría, cero red — es lo que permitirá diagnosticar un «me va lento» sin adivinar.

## 5. Política de degradación

### 5.1 Los tres intocables

Iguales en los dos niveles, sin excepción:

| Invariante | Qué significa exactamente |
|---|---|
| **Atajo global → overlay visible** | < 50 ms **también en `Ligero`**. Si no se llega, se recorta lo que se dibuja, nunca el plazo |
| **Latencia de trazo** | **<= un fotograma del refresco real del monitor** — 8 ms a 120 Hz, 16 ms a 60 Hz. Corrige el «< 8 ms» del maestro, que perseguía un número sin significado en una pantalla de 60 Hz |
| **Los bytes del archivo guardado** | Un PNG hecho en `Ligero` es **byte a byte idéntico** al de `Completo`: misma resolución nativa, mismo recorte, mismo color, mismo codificador |

El tercero es además la puerta de calidad más valiosa del diseño, porque **se mide sin GPU y sin
escritorio**: dado el mismo búfer de entrada, las dos rutas codifican y el test compara los bytes.

### 5.2 Lo que cae en `Ligero`

| Qué | `Completo` | `Ligero` | Por qué es seguro |
|---|---|---|---|
| Previsualización de efectos (mosaico, desenfoque, foco) | Resolución completa, en vivo | Sobre una copia a la mitad; al confirmar se aplica exacto sobre el original | Lo que se ve al arrastrar es orientativo; el resultado es idéntico |
| Animaciones y transiciones | Fundidos del overlay, lupa animada | Aparición instantánea | Una animación de 150 ms en un equipo lento se percibe como lentitud, no como pulido |
| Transparencia cara | Sombras difuminadas, desenfoque de fondo | Fondos sólidos, borde de 1 px | El desenfoque de fondo es de lo más caro que existe en una iGPU compartida |
| Overlay en vivo (`Espacio` reanuda WGC) | Refresco libre | 30 fps, y arranca congelado | El modo congelado ya es el predeterminado; el vivo es la excepción |
| Miniaturas del historial | Precalculadas al capturar | Generadas al mirarlas, descartables | Precalcular miniaturas que nadie abre gasta RAM y CPU a cambio de nada |

### 5.3 Precalentamiento diferido

Crear el dispositivo Direct3D 11 cuesta del orden de 100-200 ms y unos megas de RAM, y enfrenta dos
puertas: con el dispositivo creado al arrancar, el reposo pesa más; sin él, **el primer atajo del día no
cumple los 50 ms**.

Como la latencia es sagrada y la RAM en reposo es negociable: la app llega a la bandeja **sin**
dispositivo (arranque en frío rápido, RAM mínima) y, unos segundos después, en un hilo de prioridad
baja, lo crea y lo deja caliente. Si el usuario pulsa el atajo antes, se crea en ese momento y esa única
vez se paga el camino lento. Igual en los dos niveles.

### 5.4 Lo que no es degradación, sino corrección universal

Van siempre, en las dos rutas, porque son mejores en cualquier máquina:

- **Render dirigido por eventos** — ya decidido y ya conseguido: 0 % de CPU en reposo.
- **Invalidación por región** — al mover el rectángulo de selección se redibuja el rectángulo sucio, no
  la pantalla. En 1080p es la diferencia entre 2 millones de píxeles por fotograma y unos miles.
- **Cero copias** — la imagen no cruza a memoria de sistema hasta que se guarda o se copia.
- **Carga perezosa** — catálogos de idioma, iconos y códecs se cargan al usarse, no al arrancar.

### 5.5 Lo que expresamente se descarta

No se baja la resolución de captura. No se cambia el formato de salida. **No se apagan funciones
enteras:** `Ligero` nunca hace que a alguien le falte un botón.

## 6. Disciplina de recursos

### 6.1 El presupuesto se expresa en pantallas

Una pantalla 1080p en BGRA son 8,3 MB; una 4K, 33 MB.

**Definición, para que el tope no sea interpretable:** una *copia viva* es un búfer de píxeles del
tamaño de la región capturada, esté en memoria de sistema o en una textura de la GPU. Los búferes a
resolución reducida cuentan por su tamaño real — la copia de previsualización de 5.2, que es la mitad de
ancho y la mitad de alto, cuenta **un cuarto** de copia, y por eso cabe dentro del tope de `Ligero`.

| Concepto | `Completo` | `Ligero` | En la máquina suelo a 1080p |
|---|---|---|---|
| Copias vivas de la imagen | <= 6 (permite instantáneas de deshacer) | **<= 3**: capturada, de trabajo, de codificación | <= 25 MB |
| RAM en reposo | <= 40 MB | <= 30 MB | hoy se va por 13 MB |
| Caché (historial, miniaturas) | mín(1 % de la RAM física, 128 MB) | mín(0,5 % de la RAM física, 16 MB) | 16 MB, con evicción LRU |

**Cuando la VRAM dedicada es cero, la memoria de textura se contabiliza contra el presupuesto**, porque
sale de la misma RAM. `DXGI_ADAPTER_DESC1` lo dice sin necesidad de adivinar.

El tope no es un comentario: `Presupuesto` es un valor que las cachés reciben. Una caché sin tope
inyectado no compila.

### 6.2 Hilos: cero en reposo, dimensionados a núcleos físicos

Pool de trabajo = **núcleos físicos − 1, mínimo 1**. En la máquina suelo: **un hilo**. Nada de
constantes, nada de contar hilos lógicos y crear cuatro que se pelean por dos núcleos.

En reposo no existe ningún hilo de trabajo: sólo el hilo de mensajes, que duerme. Los hilos se crean con
el trabajo y mueren con él; con uno o dos, crearlos cuesta menos que tenerlos ociosos ocupando pila.

### 6.3 Cero asignaciones en el camino caliente

El camino caliente está bien delimitado: mover el ratón sobre el overlay, arrastrar la selección,
dibujar un trazo. Ahí no se asigna: búferes reutilizados con `clear()`, que conserva la capacidad, y
estructuras de tamaño fijo.

Y se comprueba. `pixpin-nivel`, tras la feature `medicion` usada sólo como dependencia de desarrollo,
expone un **asignador global que cuenta asignaciones**. El test ejecuta mil iteraciones del bucle puro y
afirma **cero asignaciones después de la primera**. Corre en segundos, sin GPU y sin escritorio.

### 6.4 Lo que expresamente no se hará

Nada de asignador de arena global ni de sustituir el asignador de Windows por `mimalloc`. El mismo
problema se ataca no asignando en el camino caliente, que es más barato y más comprobable. Y ningún
`unsafe` añadido «por rendimiento» sin una medición previa que lo justifique: en S1-A un `unsafe` mal
razonado ya escondió una fuga que pasó todas las puertas automáticas.

## 7. Compilación y SIMD

1. Se crea `.cargo/config.toml` fijando **explícitamente** `-C target-cpu=x86-64` para
   `x86_64-pc-windows-msvc`, con el comentario que explica por qué. Hoy es el valor por defecto: pasa a
   ser una decisión escrita en vez de una casualidad.
2. Un **test permanente** lee ese fichero y falla si aparece `target-cpu=native`, `x86-64-v2/v3/v4` o
   `+avx2`. En este proyecto las reglas las hace cumplir un test, no un párrafo — igual que la regla de
   capas.
3. Donde el SIMD valga la pena (conversión de color, cosido del scroll, codificación) se despacha **en
   tiempo de ejecución** con `is_x86_feature_detected!("avx2")` y ruta SSE2 de respaldo. La ruta baseline
   es la que se prueba siempre; la AVX2 se valida comparando su salida contra la baseline, byte a byte,
   sin necesitar GPU.
4. En `aarch64-pc-windows-msvc` no hace falta nada: NEON es baseline.
5. Sigue prohibido `opt-level = "z"`: el objetivo es la velocidad, y el tope de tamaño de binario está
   lejísimos (30 MB permitidos, 0,84 MB reales).

## 8. Medición

### 8.1 Carril automático — sin GPU, sin escritorio

- La decisión de nivel sobre equipos inventados, **incluida la máquina suelo exacta**.
- **Equivalencia de bytes** entre `Completo` y `Ligero` para el mismo búfer de entrada.
- **Cero asignaciones** en el camino caliente, con el asignador contador.
- Topes de caché bajo presión sintética: 10.000 entradas, sin pasarse del tope, con evicción efectiva.
- El guardián de `.cargo/config.toml`.
- Equivalencia entre la ruta SIMD baseline y la AVX2.

### 8.2 Carril manual — la máquina suelo, con un comando

`pixpinmax.exe --medir` ejecuta el arnés y escribe un informe: arranque en frío hasta la bandeja,
atajo→overlay, RAM en reposo y en pico (`GetProcessMemoryInfo`, uso privado), fps del overlay en vivo,
tiempo de mosaico, y el nivel decidido con sus razones.

La salida se guarda en `medidas/AAAA-MM-DD-<maquina>.md` y **se commitea**. Los números se acumulan y una
regresión se ve comparando dos ficheros, en vez de confiando en la memoria de nadie.

### 8.3 Presupuesto revisado — reemplaza §4.4 del documento maestro

| Métrica | `Completo` | `Ligero` (suelo: i3 3.ª gen, 4 GB, HD 4000) | Carril |
|---|---|---|---|
| Arranque en frío hasta bandeja | < 300 ms | < 300 ms | manual |
| **Atajo → overlay visible** | **< 50 ms** | **< 50 ms** (sagrado) | manual |
| **Latencia de trazo** | **<= 1 fotograma del refresco real** | ídem (16 ms a 60 Hz) | manual |
| CPU en reposo | 0 % | 0 % | manual |
| RAM en reposo | < 40 MB | < 30 MB | manual |
| Copias vivas de la imagen | <= 6 | <= 3 | automático |
| Asignaciones en el camino caliente | 0 | 0 | automático |
| Bytes del archivo guardado | idénticos entre niveles | ídem | automático |
| Mosaico sobre región 4K | < 5 ms | *< 30 ms — provisional* | manual |
| RAM con 10 pines | < 150 MB | < 80 MB | manual (S3) |
| Tamaño del binario | < 30 MB | ídem (hoy: 0,84 MB) | automático |

Dos honestidades: el **primer** arranque sobre un disco mecánico frío lo manda el disco, no la app — se
mide y se anota, pero no es una puerta que se pueda prometer. Y el número del mosaico en HD 4000 es una
conjetura hasta medirlo; queda marcado como provisional.

## 9. Impacto en documentos y planes

**Documento maestro** — §4.4 se reescribe entera remitiendo a 8.3 de aquí; el diagrama de capas de §2
suma `pixpin-nivel` en L0; D4 gana la nota del baseline del procesador; §4.5 suma la regla de
`.cargo/config.toml`; §4.6 añade `pixpin-nivel` a la lista de crates con `forbid(unsafe_code)`.

**Plan de S1-B1** (`../plans/2026-08-09-s1b1-captura.md`, a 1 de 10 tareas) — no se tira nada:

- Task 1 (geometría) está hecha y no se toca. Se marcan sus checkboxes, que quedaron sin marcar aunque
  el trabajo está commiteado.
- **Entra una tarea nueva justo detrás:** crear `pixpin-nivel` (hechos, nivel, presupuesto), la recogida
  de hechos en `pixpin-shell`, el ajuste `[rendimiento] nivel` en TOML y el guardián de
  `.cargo/config.toml`. Va primero porque todo lo demás la recibe por parámetro.
- Tasks 5-8 (D3D11, WGC, bajada a CPU, codificación) reciben `Nivel` y `Presupuesto` por parámetro y
  añaden la puerta de equivalencia de bytes.
- Task 10 (cableado del atajo) añade el precalentamiento diferido y la línea de `tracing` con las razones.
- La definición de terminado suma el informe de medición en la máquina suelo.

`apps/pixpin/tests/capas.rs` pasa a esperar 16 crates de librería más el ejecutable.

## 10. Fuera de alcance

- **Un tercer nivel** (por ejemplo «mínimo» para equipos aún peores). Dos niveles ya obligan a probar dos
  caminos; un tercero se añade sólo si la medición en la máquina suelo demuestra que hace falta.
- **Indicador visible del nivel en la interfaz.** Por ahora el nivel se registra en el log, que basta para
  diagnosticar. Si aparece la necesidad, es texto traducido y un elemento de menú.
- **Optimizaciones específicas de S2-S6** (tiles para gigapíxel, pool de decodificación, codificación por
  hardware). Este documento fija la política; cada sub-proyecto la aplica en su spec.

## 11. Preguntas abiertas

- Los cinco umbrales de 4.3 y el objetivo de mosaico en HD 4000 son **provisionales**. Se cierran con la
  primera medición en la máquina suelo, anotando el comando que la produjo.
- El repositorio sigue **sin remoto**, así que las puertas del carril automático sólo corren en local. Si
  se activa la CI, corren todas salvo las que exigen escritorio.
