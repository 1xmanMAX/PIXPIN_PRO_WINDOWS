# S2-B: tipos, grupos y portapapeles — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** completar v0.2 — pines de nota y de archivo/carpeta, pinear el
portapapeles con `Ctrl+Alt+V`, menú contextual completo, grupos con sombra de
color y ocultar/mostrar en bloque, imán de bordes, teclado completo, y las
puertas de rendimiento de §7 medidas con 10 pines.

**Architecture:** todo cuelga del diseño S2 (D20-D30). El pin gana un enum de
contenido (imagen/nota/ficha); el almacén gana grupos y dos tipos nuevos; el
gestor `Pines` del ejecutable sigue siendo la única pieza que ve almacén y
ventanas a la vez.

**Tech Stack:** el existente (Rust, crate `windows` 0.62, D2D/DComp/DWrite).

**Spec:** `docs/superpowers/specs/2026-09-01-s2-pines-almacen-design.md`

## Global Constraints

- Baseline `target-cpu=x86-64`; niveles Completo/Ligero por parámetro, nunca global.
- `#![forbid(unsafe_code)]` en geom/store/apps; `unsafe` con `// SAFETY:` en pin/shell/codec/render.
- Capas: L2 no ve L2 (pixpin-pin NUNCA usa pixpin-store ni pixpin-capture).
- Todo texto visible pasa por Fluent (catálogo es/en); el pin recibe textos ya traducidos.
- Puerta estándar por tarea; commit por tarea; tests con `-- --test-threads=1`.

## Decisiones auto-aprobadas de este plan (recomendadas; el usuario delegó)

- **D31** Sombra: se mantiene el dibujo por anillos de S2-A, teñido por grupo y
  más intenso con foco, SIN caché de bitmap: `pintar` solo corre al
  redimensionar/enfocar/cambiar grupo, así que el requisito real (mover no
  repinta) ya se cumple. Si las medidas de la última tarea lo desmienten, se
  cachea entonces.
- **D32** Pin de portapapeles: nace centrado en el monitor del cursor. Imagen:
  tamaño natural limitado al 80 % del área de trabajo. Nota: bloque medido con
  DirectWrite, máx 480×640 lógicos. Ficha: 280×72 lógicos fijos (redimensiona
  solo a lo ancho).
- **D33** Tema claro/oscuro: `AppsUseLightTemma` de HKCU leído al crear el pin;
  sin suscripción a cambios en v1 (el pin nuevo ya sale con el tema nuevo).
- **D34** El retardo de escritura del índice (300 ms de la spec 5.2) se
  implementa donde nace la ráfaga: las flechas mueven la ventana ya y agrupan
  la persistencia con un `SetTimer` de 300 ms en la propia ventana del pin;
  el resto de gestos ya escriben solo al soltar. El almacén no gana relojes.
- **D35** Colores de paleta (RGB 0-1, para teñir sombra): rojo .86/.20/.18,
  naranja .95/.45/.10, ámbar .95/.68/.10, verde .20/.66/.33, cian .10/.66/.68,
  azul .16/.44/.86, violeta .48/.28/.78, rosa .87/.28/.60.

## Estructura de ficheros

```
crates/pixpin-geom/src/pin_geometria.rs   + iman_de_bordes (T1)
crates/pixpin-store/src/almacen.rs        grupos, nota, archivo (T2)
crates/pixpin-codec/src/portapapeles.rs   + leer() (T3)
crates/pixpin-shell/src/entorno.rs        + tema_claro() (T4)
crates/pixpin-pin/src/icono.rs            icono de archivo → RGBA (T4)
crates/pixpin-pin/src/contenido.rs        enum Contenido + medidas naturales (T5)
crates/pixpin-pin/src/ventana.rs          nota/ficha, sombra grupo+foco, teclado, imán (T5-T7)
crates/pixpin-pin/src/menu.rs             menú contextual (T8)
crates/pixpin-shell/src/bandeja.rs        sección «grupos ocultos» (T10)
apps/pixpin/src/pines.rs                  gestor v2 (T9-T11)
apps/pixpin/src/main.rs                   Ctrl+Alt+V, cableado (T9-T11)
crates/pixpin-store/src/ajustes.rs        Atajos.portapapeles (T9)
crates/pixpin-shell/src/atajos.rs         ID_PORTAPAPELES=6 (T9)
```

---

## Task 1: Imán de bordes (puro)

**Files:** Modify `crates/pixpin-geom/src/pin_geometria.rs`, `lib.rs` (re-export).

**Produces:** `pub fn iman_de_bordes(rect: Rect, area_trabajo: Rect, umbral: i32) -> Rect`
— si un borde del rect queda a ≤ `umbral` px del borde homólogo del área de
trabajo, el rect se adhiere a él (sin cambiar tamaño). Bordes opuestos a la
vez: gana el más cercano por eje.

- [ ] **Step 1: Tests que fallan** (en `pruebas` de pin_geometria.rs)

```rust
    #[test]
    fn el_iman_adhiere_al_borde_cercano() {
        let area = Rect { x: 0, y: 0, ancho: 1000, alto: 800 };
        let cerca = Rect { x: 5, y: 300, ancho: 100, alto: 100 };
        let r = iman_de_bordes(cerca, area, 8);
        assert_eq!((r.x, r.y), (0, 300), "izquierda a 5 px se adhiere; y no cambia");
        let abajo = Rect { x: 300, y: 694, ancho: 100, alto: 100 };
        let r = iman_de_bordes(abajo, area, 8);
        assert_eq!((r.x, r.y), (300, 700), "borde inferior del pin a 6 px del area");
    }

    #[test]
    fn lejos_del_borde_el_iman_no_toca_nada() {
        let area = Rect { x: 0, y: 0, ancho: 1000, alto: 800 };
        let lejos = Rect { x: 200, y: 200, ancho: 100, alto: 100 };
        assert_eq!(iman_de_bordes(lejos, area, 8), lejos);
    }
```

- [ ] **Step 2: Implementar**

```rust
/// Adhiere el rect a los bordes del area de trabajo si queda a menos de
/// `umbral` pixeles. El tamano no cambia; por eje gana el borde mas cercano.
pub fn iman_de_bordes(rect: Rect, area_trabajo: Rect, umbral: i32) -> Rect {
    let mut r = rect;
    let d_izq = (r.izquierda() - area_trabajo.izquierda()).abs();
    let d_der = (area_trabajo.derecha() - r.derecha()).abs();
    if d_izq <= umbral && d_izq <= d_der {
        r.x = area_trabajo.izquierda();
    } else if d_der <= umbral {
        r.x = area_trabajo.derecha() - r.ancho as i32;
    }
    let d_arr = (r.arriba() - area_trabajo.arriba()).abs();
    let d_aba = (area_trabajo.abajo() - r.abajo()).abs();
    if d_arr <= umbral && d_arr <= d_aba {
        r.y = area_trabajo.arriba();
    } else if d_aba <= umbral {
        r.y = area_trabajo.abajo() - r.alto as i32;
    }
    r
}
```

- [ ] **Step 3: Puerta + commit** — «Iman de bordes: el pin se adhiere al area de trabajo»

---

## Task 2: Almacén v2 — grupos, notas y archivos

**Files:** Modify `crates/pixpin-store/src/almacen.rs`, `lib.rs`.

**Produces:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorGrupo { Rojo, Naranja, Ambar, Verde, Cian, Azul, Violeta, Rosa }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grupo { pub id: u32, pub color: ColorGrupo, #[serde(default)] pub oculto: bool }

// TipoEntrada gana Nota y Archivo. Entrada gana `ruta: Option<PathBuf>`
// (solo archivo; por referencia, D28) con #[serde(default)]; `objeto` pasa a
// String posiblemente vacia para archivo (compatible con lo escrito por S2-A).
```

Métodos nuevos de `Almacen` (todos persisten como los existentes):
- `guardar_nota(texto: &str, origen: &str, pin: Option<PinGuardado>) -> Result<u64>` — escribe `objetos/AAAA/MM/NNNNNN.txt` UTF-8.
- `guardar_archivo(ruta: &Path, pin: Option<PinGuardado>) -> Result<u64>` — solo referencia; `objeto` queda vacía.
- `poner_grupo(id_entrada: u64, color: Option<ColorGrupo>) -> Result<Option<u32>>` — busca o crea el grupo del color; devuelve el id de grupo o None. Un grupo que se queda sin entradas se elimina del índice.
- `poner_grupo_oculto(id_grupo: u32, oculto: bool) -> Result<()>`.
- `grupos() -> &[Grupo]`; `grupo_de(id_entrada) -> Option<Grupo>`.
- `eliminar(id_entrada: u64) -> Result<()>` — borra la entrada del índice y su objeto del disco (única acción destructiva); limpia el grupo si queda vacío.

- [ ] **Step 1: Tests que fallan** — nota ida y vuelta (texto con acentos),
  archivo por referencia (la ruta NO se copia, `objeto` vacía), poner_grupo
  crea/reutiliza/limpia (asignar rojo a dos entradas = un solo grupo; quitar a
  ambas = grupo desaparece), oculto persiste, eliminar borra índice y fichero
  pero NUNCA el archivo referenciado, y un índice de S2-A (sin `grupos` ni
  `ruta`) sigue abriendo (compatibilidad `serde(default)`).
- [ ] **Step 2: Implementar** siguiendo los patrones existentes (contador,
  temporal+rename, `ErrorAlmacen`).
- [ ] **Step 3: Puerta + commit** — «Almacen v2: grupos por color, notas y archivos por referencia»

---

## Task 3: Leer el portapapeles

**Files:** Modify `crates/pixpin-codec/src/portapapeles.rs`, `lib.rs`.

**Produces:**

```rust
pub enum ContenidoPortapapeles {
    Imagen(ImagenRgba),
    Texto(String),
    Rutas(Vec<std::path::PathBuf>),
}
/// None si el portapapeles esta vacio o trae un formato ajeno.
/// Prioridad: CF_HDROP > CF_DIB(imagen) > CF_UNICODETEXT — quien copia
/// archivos en el Explorador tambien deja texto, y el archivo es lo que quiso.
pub fn leer() -> Option<ContenidoPortapapeles>
```

CF_DIB: cabecera `BITMAPINFOHEADER`, biBitCount 24/32, filas de abajo arriba y
con relleno a 4 bytes — invertir y compactar a RGBA. CF_UNICODETEXT: UTF-16
hasta el NUL. CF_HDROP: `DragQueryFileW`. Mismo patrón Open/Close con guardia
que el `copiar_imagen` existente.

- [ ] **Step 1: Test `#[ignore]`** (el portapapeles es global): `copiar_imagen`
  de un 4×3 conocido → `leer()` devuelve `Imagen` idéntica píxel a píxel.
  Texto: `SetClipboardData` no está en codec para texto… escribir el texto con
  PowerShell no es determinista: usar el propio Win32 en el test (permitido,
  es un test de codec) para poner texto y rutas y leerlos de vuelta.
- [ ] **Step 2: Implementar + puerta (incluida `--ignored`) + commit** —
  «El portapapeles se lee: imagen, texto y archivos»

---

## Task 4: Tema del sistema e icono de archivo

**Files:** Modify `crates/pixpin-shell/src/entorno.rs`; Create `crates/pixpin-pin/src/icono.rs`.

**Produces:**
- `pixpin_shell::entorno::tema_claro() -> bool` — HKCU `...\Themes\Personalize\AppsUseLightTheme`, 1 si falta (D33).
- `pixpin_pin::icono_de(ruta: &Path) -> Option<ImagenRgba>` — `SHGetFileInfoW`
  (SHGFI_ICON|SHGFI_LARGEICON, y SHGFI_USEFILEATTRIBUTES si la ruta no existe
  para que la referencia rota tenga icono genérico) → `DrawIconEx` sobre una
  DIB section 32bpp premultiplicada → RGBA. `DestroyIcon` siempre.

- [ ] **Step 1: Tests** — `tema_claro()` devuelve algo (smoke, no ignored);
  `icono_de` de `C:\Windows\notepad.exe` da 32×32 con algún píxel no
  transparente (`#[ignore]`, pide sesión); `icono_de` de una ruta inexistente
  devuelve Some (icono genérico).
- [ ] **Step 2: Implementar + puerta + commit** — «Tema del sistema e icono real de archivo»

---

## Task 5: El contenido del pin: imagen, nota o ficha

**Files:** Create `crates/pixpin-pin/src/contenido.rs`; Modify `ventana.rs`, `lib.rs`.

**Produces:**

```rust
/// Lo que el pin muestra. El pin no sabe de almacenes: recibe el contenido
/// ya cargado y avisa por callback de lo que el usuario pide.
pub enum Contenido {
    Imagen(ImagenRgba),
    Nota { texto: String },
    Archivo { nombre: String, detalle: String, icono: Option<ImagenRgba>, existe: bool },
}
/// Tamano natural en px logicos (D32): imagen = nativa/escala; nota = medida
/// DWrite (max 480x640); ficha = 280x72. PURO salvo la medicion de texto,
/// que recibe un medidor inyectado: `fn(texto,&str, tam: f32) -> (f32,f32)`.
pub fn tamano_natural(contenido: &Contenido, medidor: &dyn Fn(&str, f32) -> (f32, f32)) -> (u32, u32)
```

`Pin::nuevo` pasa a recibir `Contenido` + `tema_claro: bool` en vez de
`&ImagenRgba` (S2-A solo construye imágenes: el gestor adapta). `pintar`
según tipo: nota = tarjeta redondeada blanca/negra + texto 14 px lógicos con
margen 12; ficha = tarjeta + icono 32 + nombre (16 px) + detalle gris (12 px);
si `!existe`, detalle = texto «no encontrado» YA TRADUCIDO recibido en
`TextosPin`. Redimensión de ficha: solo ancho (la máquina de estados recibe
`solo_ancho: bool` y las esquinas mantienen el alto).

- [ ] **Step 1: Tests puros** — `tamano_natural` de ficha = (280,72); de nota
  corta < máx; de nota kilométrica se recorta a 480×640; imagen 600×450 a
  escala 150 = (400,300). Máquina de estados: con `solo_ancho`, la esquina
  cambia ancho y conserva alto (test negativo: alto intacto).
- [ ] **Step 2: Implementar** (el dibujo reusa `Pintor::{rellenar_redondeado,texto,bitmap}`).
- [ ] **Step 3: Test de escritorio `#[ignore]`** — crear un pin de nota y uno
  de ficha, sobreviven y se destruyen limpio (patrón del test de S2-A).
- [ ] **Step 4: Puerta + commit** — «El pin muestra notas y fichas de archivo, no solo imagenes»

---

## Task 6: Sombra por grupo y por foco

**Files:** Modify `crates/pixpin-pin/src/ventana.rs`, `estado.rs` si hace falta.

**Produces:** `Pin::poner_color(&self, color: Option<(f32,f32,f32)>)` (retinta
y repinta); el WndProc trata `WM_SETFOCUS`/`WM_KILLFOCUS` repintando con
sombra más intensa/normal (D30). El color RGB viene del gestor (paleta D35):
el pin no conoce `ColorGrupo` (vive en pixpin-store, L2 prohibido).

- [ ] **Step 1: Implementar** — `PinInterno` gana `color_sombra: Option<(f32,f32,f32)>`
  y `enfocado: bool`; los anillos multiplican su alfa ×1.6 con foco y usan el
  color del grupo (negro sin grupo). `poner_color` vía `SendMessageW` con
  mensaje propio (`WM_APP+1`) o mutando por el puntero del USERDATA desde el
  hilo propietario (los pines viven en el hilo principal: acceso directo tras
  `interno_de`, sin mensaje).
- [ ] **Step 2: Test de escritorio** — crear pin, `poner_color`, no muere;
  foco/desenfoque con dos pines no muere.
- [ ] **Step 3: Puerta + commit** — «La sombra dice el grupo y el foco»

---

## Task 7: Teclado completo e imán

**Files:** Modify `crates/pixpin-pin/src/ventana.rs`, `estado.rs`.

- Flechas / `Shift`+flechas: mueven 1/10 px lógicos (× escala en físicos);
  `SetWindowPos` inmediato + `SetTimer(300 ms)`; el `WM_TIMER` emite
  `CambioPin::Movido` una vez y mata el timer (D34).
- `Ctrl+C` (WM_KEYDOWN 'C' + `GetKeyState(VK_CONTROL)`): emite
  `CambioPin::CopiarPedido`.
- Imán: en `GestoTerminado` y al soltar flechas, aplica
  `iman_de_bordes(rect, area_trabajo_del_monitor_actual, 8×escala/100)` con
  `MonitorFromWindow`+`GetMonitorInfoW` (rcWork) antes de persistir y de
  colocar la ventana.

- [ ] **Step 1: Tests** — estado puro: nuevas transiciones si las hay; el imán
  ya está probado en T1. Test de escritorio: pin + flecha sintetizada mueve y
  el callback llega una sola vez por ráfaga.
- [ ] **Step 2: Implementar + puerta + commit** — «Flechas, Ctrl+C y el iman de bordes»

---

## Task 8: El menú del clic derecho

**Files:** Create `crates/pixpin-pin/src/menu.rs`; Modify `ventana.rs`, `lib.rs`.

**Produces:**

```rust
pub struct TextosPin {  // ya traducidos; el pin no conoce Fluent
    pub copiar: String, pub guardar_como: String, pub abrir_ubicacion: String,
    pub tamano_original: String, pub grupo: String, pub sin_grupo: String,
    pub colores: [String; 8], pub ocultar_grupo: String, pub cerrar: String,
    pub eliminar: String, pub no_encontrado: String,
}
pub enum CambioPin {  // se amplia
    Movido(Rect), Redimensionado(Rect), Cerrado,
    CopiarPedido, GuardarComoPedido, AbrirPedido, AbrirUbicacionPedido,
    GrupoPedido(Option<u8>),  // indice 0-7 en la paleta; None = sin grupo
    OcultarGrupoPedido, EliminarPedido,
}
```

`WM_RBUTTONUP` → menú según el diseño 4.3 (submenú Grupo con 9 entradas,
separadores, «Abrir ubicación» en archivo, «Tamaño original» solo
imagen/nota, «Ocultar este grupo» solo con grupo). Patrón exacto de
`Bandeja::mostrar_menu` (clausura que garantiza `DestroyMenu`,
`SetForegroundWindow` antes de `TrackPopupMenu`, `TPM_RETURNCMD`). «Tamaño
original» se aplica dentro (AlternarTamano a nativo); «Cerrar» reusa la ruta
de Esc; el resto emite su variante y el gestor decide. Doble clic en ficha
emite `AbrirPedido` en vez de alternar tamaño.

- [ ] **Step 1: Implementar** (el menú es Win32 puro; la lógica de qué
  entradas mostrar va en una función pura `entradas_del_menu(tipo, con_grupo)
  -> Vec<EntradaMenu>` con test en CI).
- [ ] **Step 2: Test** — puro: imagen sin grupo no ofrece «Ocultar grupo» ni
  «Abrir ubicación»; archivo ofrece «Abrir ubicación» y no «Tamaño original».
  Escritorio: abrir y cerrar el menú sintetizado no mata el pin.
- [ ] **Step 3: Puerta + commit** — «El menu del clic derecho, completo y traducido»

---

## Task 9: `Ctrl+Alt+V` — pinear el portapapeles

**Files:** Modify `crates/pixpin-shell/src/atajos.rs` (`ID_PORTAPAPELES: u32 = 6`),
`crates/pixpin-store/src/ajustes.rs` (`Atajos.portapapeles`, defecto
`"Ctrl+Alt+V"`), `apps/pixpin/src/main.rs`, `apps/pixpin/src/pines.rs`,
ficheros `.ftl` de idioma.

El brazo del atajo: `leer()` → según contenido, `pines.pinear_nota/…archivo/…imagen_centrada`
en el monitor del cursor (D32), `SW_SHOWNOACTIVATE` (no roba el foco, 4.4).
Portapapeles vacío/ajeno → log informativo, sin diálogo.

- [ ] **Step 1: Test** — ajustes: el sexto atajo por defecto es `Ctrl+Alt+V`
  (línea en el test de valores por defecto).
- [ ] **Step 2: Implementar + puerta + commit** — «Ctrl+Alt+V: el portapapeles queda pineado sin robar el foco»

---

## Task 10: Gestor v2 y bandeja con grupos ocultos

**Files:** Modify `apps/pixpin/src/pines.rs`, `main.rs`,
`crates/pixpin-shell/src/bandeja.rs`, `.ftl`.

- `Pines` gana: `pinear_nota`, `pinear_archivo`, `pinear_imagen_centrada`,
  y el manejo de TODOS los `CambioPin` nuevos: CopiarPedido (imagen→`copiar_imagen`,
  nota→texto al portapapeles [nueva `codec::copiar_texto`], archivo→ruta como
  texto), GuardarComoPedido (diálogo de S1 reutilizado; en archivo no aparece),
  AbrirPedido/`AbrirUbicacionPedido` (`ShellExecuteW open` / `explorer /select,`),
  GrupoPedido (almacén `poner_grupo` + `poner_color` con la paleta D35),
  OcultarGrupoPedido (marca oculto + cierra las ventanas del grupo SIN tocar
  sus `pin` del índice), EliminarPedido (diálogo de confirmación
  `pixpin_shell::dialogo` + `eliminar` + cerrar ventana).
- Bandeja: `mostrar_menu` gana sección dinámica «Grupos ocultos» (etiqueta +
  n.º de pines por grupo, ids WM_COMMAND 200+); elegir uno →
  `Evento::MostrarGrupo(id)` → el gestor restaura sus pines donde estaban.
- La restauración al arrancar salta las entradas cuyo grupo está oculto.

- [ ] **Step 1: Tests** — almacén ya probado; puro nuevo: ninguno grande.
  Escritorio: ciclo agrupar→ocultar→mostrar con 2 pines sintéticos no pierde
  el rect (el índice conserva `pin` mientras está oculto).
- [ ] **Step 2: Implementar + puerta + commit** — «Grupos de verdad: sombra de color, ocultar y volver»

---

## Task 11: Restauración en paralelo y presupuesto de texturas

**Files:** Modify `apps/pixpin/src/pines.rs`, `crates/pixpin-nivel/src/lib.rs`
(campo `bytes_texturas_pines` en `Presupuesto`: Completo 64 MB, Ligero 24 MB).

- `restaurar` decodifica en paralelo: `std::thread::scope`, `presupuesto.hilos`
  trabajadores sobre un canal de rutas, el hilo principal crea cada ventana en
  cuanto llega su imagen (mpsc). El log anota `ms` del primero y del último.
- Presupuesto: al cargar, si los bytes acumulados de texturas superan el tope,
  las imágenes restantes se decodifican y reescalan a **media resolución**
  (`image::imageops::resize` a mitad) antes de subir; al enfocar un pin a
  media resolución se recarga nítido del disco (campo `nitido: bool` en el
  mapa de vivos + recarga en `WM_SETFOCUS` vía `CambioPin::Enfocado`).

- [ ] **Step 1: Tests** — `Presupuesto::desde` con los dos niveles da los
  topes nuevos (test en pixpin-nivel). La lógica «cuáles bajan a media» es
  pura: `fn plan_de_carga(tamanos: &[(u64, u64)], tope: u64) -> Vec<bool>`
  (true = nítido) con test: entra todo → todos nítidos; no cabe → los últimos
  a media (los primeros del índice son los más antiguos).
- [ ] **Step 2: Implementar + puerta + commit** — «Restauracion en paralelo con presupuesto de texturas»

---

## Task 12: E2E, medidas con 10 pines y cierre de fase

**Files:** Create `medidas/<fecha>-equipo-desarrollo-s2b.md`; Modify este plan
(casillas) y la spec si algún número se desmiente.

- [ ] **Step 1: Flujo manual sintetizado** (binario release): Ctrl+Alt+V con
  imagen/texto/archivos copiados (los tres tipos aparecen, foco intacto);
  menú derecho: agrupar dos pines en verde, ocultar desde el menú, volver
  desde la bandeja; Ctrl+C de cada tipo; flechas; imán (arrastrar cerca del
  borde adhiere); eliminar con confirmación; reiniciar y ver reaparecer todo.
- [ ] **Step 2: Puertas §7 con 10 pines** — CPU 0 %, RAM < 100/60 MB,
  arranque 10 pines < 500 ms (Completo, medido), primer pin < 200 ms, mover
  ≤ 1 fotograma (inspección + medida si hay duda). Anotar valores reales
  aunque fallen.
- [ ] **Step 3: Cerrar** — suite completa + `--ignored` + deny + push + PR +
  CI + merge; actualizar plan maestro y memoria.

## Definición de terminado (criterios §9 de la spec)

Los siete criterios de aceptación de v0.2, verificados y con medidas en `medidas/`.
