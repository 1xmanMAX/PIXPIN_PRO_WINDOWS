# S1-B3 · Scroll y cuentagotas — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** que `Ctrl+Alt+S` capture una región haciendo scroll y cosiendo hasta el final, dejando el resultado como pin y en el portapapeles; y que `Ctrl+Alt+D` copie el color bajo el cursor.

**Architecture:** el cosido es puro en `pixpin-codec::cosido` (puerto del Android más franjas fijas); el shell gana `rueda_en` y `escape_pulsado`; el ejecutable gana `scroll.rs` (el bucle capturar→rueda→asentar→coser) y dos modos nuevos del overlay.

**Spec:** `docs/superpowers/specs/2026-09-02-s1b3-scroll-cuentagotas-design.md` (D73–D78).

## Global Constraints

Las de siempre: `forbid(unsafe_code)` en `apps/pixpin`; capas; puerta estándar y commit por tarea; rama `s1b3-scroll-cuentagotas`.

---

### Task 1: El cosido, puro

**Files:** Create `crates/pixpin-codec/src/cosido.rs`; modify `lib.rs` (`pub mod cosido; pub use cosido::{Cosedor, Plan, Orden, Resultado, firmas, encontrar_desplazamiento, es_lisa, franjas_fijas, SIN_ENCAJE}`).

**Interfaces:**
```rust
pub const SIN_ENCAJE: i32 = -1;
pub const FILAS_COLA: usize = 48; pub const MAX_FILAS_COLA: usize = 384;
pub const PASO_MUESTREO: usize = 5; pub const TOLERANCIA: i64 = 40; pub const VARIACION_MINIMA: i64 = 300;
pub fn firma_de_fila(rgba: &[u8], ancho: usize, paso: usize) -> i64;   // luminancia 77/151/28 >> 8
pub fn firmas(imagen: &ImagenRgba, paso: usize) -> Vec<i64>;
pub fn encontrar_desplazamiento(cola: &[i64], marco: &[i64], tolerancia: i64) -> i32;
pub fn es_lisa(cola: &[i64], variacion_minima: i64) -> bool;
/// Filas identicas por arriba y por abajo entre dos marcos que SI se han movido (D74).
pub fn franjas_fijas(anterior: &[i64], actual: &[i64], tolerancia: i64) -> (usize, usize);
pub enum Resultado { Primero, Anadido, SinMovimiento, Incierto, Lleno }
pub struct Orden { pub resultado: Resultado, pub desde: usize, pub filas: usize }
pub struct Plan { .. }  // Plan::nuevo(alto_maximo); plan(&mut self, firmas: &[i64], filas: usize) -> Orden; alto(); reiniciar()
pub struct Cosedor { .. } // Cosedor::nuevo(ancho, alto_maximo); anadir(&mut self, marco: &ImagenRgba) -> Resultado; terminar(self) -> Option<ImagenRgba>
```
`Plan` lleva `pie: usize` (filas de pie detectadas): la cola se toma del final del contenido SIN pie; `Orden.desde/filas` excluyen el pie; `Cosedor::terminar` añade el pie del último marco una vez.

- [ ] Tests (CI): los 9 del Android portados 1:1 (`Random` de Kotlin no es reproducible en Rust: usar un LCG propio para `pagina(filas, semilla)`), más `una_pagina_recorrida_en_pasos_se_cose_igual_que_el_original`, `la_cabecera_y_el_pie_fijos_salen_una_sola_vez`, `tres_marcos_iguales_no_anaden_nada`, `el_alto_maximo_devuelve_lleno`.
- [ ] Implementar; puerta; commit `El cosido de la captura con scroll, puro: firmas, encaje y franjas fijas`.

### Task 2: Rueda y Escape en el shell

**Files:** Create `crates/pixpin-shell/src/entrada.rs`; `lib.rs`.

- [ ] `pub fn rueda_en(p: Punto, muescas: i32)`: `SetCursorPos` + `SendInput` con `MOUSEEVENTF_WHEEL`, `mouseData = -120 * muescas` (negativo = hacia abajo). `pub fn escape_pulsado() -> bool` con `GetAsyncKeyState(VK_ESCAPE)`.
- [ ] Test de escritorio: `escape_pulsado()` es `false` sin nadie pulsando. Commit `La rueda y el Escape que necesita la captura con scroll`.

### Task 3: El bucle de scroll y el modo del overlay

**Files:** Create `apps/pixpin/src/scroll.rs`; modify `overlay.rs` (`ModoConfirmacion::Scroll`, `QueAccion::Scroll`, `AccionFinal::Scroll { region }`), `main.rs` (`mod scroll;` y el brazo `ID_SCROLL`).

- [ ] `pub fn ejecutar_scroll(recursos: &mut Recursos, region: Rect) -> Result<Option<ImagenRgba>>`: monitor que contiene el centro; `Cosedor::nuevo(region.ancho, 20_000)`; bucle: capturar (`congelar_monitor` → `recortar` → `a_imagen`) hasta dos iguales seguidas (≤ 1 s, 60 ms entre intentos) → `anadir` → según `Resultado`: `Anadido`/`Primero` → sin_movimiento = 0; `SinMovimiento` → +1 (3 → fin); `Incierto` → seguir; `Lleno` → fin; luego `rueda_en(centro, 3)`; salir también por `escape_pulsado()` o 30 s. Log al terminar: pasos, alto, motivo.
- [ ] En `main.rs`: `ID_SCROLL` abre el overlay con `ModoConfirmacion::Scroll`; al recibir `AccionFinal::Scroll { region }` → `scroll::ejecutar_scroll` → `copiar_imagen` + `pinear_imagen_centrada`.
- [ ] Puerta; commit `Ctrl+Alt+S: capturar con scroll y dejar la pagina cosida como pin`.

### Task 4: El cuentagotas

**Files:** `apps/pixpin/src/overlay.rs` (`ModoConfirmacion::Cuentagotas`: en `BotonPulsado` y en `Enter` → `pixpin_codec::copiar_texto(&texto_color(formato, muestra))` → `Continuar::No`; sin recuadro: en este modo `BotonPulsado` no llega al estado), `main.rs` (`ID_CUENTAGOTAS`).

- [ ] Puerta; commit `Ctrl+Alt+D: el cuentagotas copia el color bajo el cursor`.

### Task 5: E2E, medidas, documentos y PR

- [ ] Bloc de notas con 400 líneas numeradas; `Ctrl+Alt+S`; seleccionar el área de texto por arrastre; `Enter`; comprobar el pin, el log (pasos, alto, motivo) y que en la imagen las líneas van seguidas (OCR no hay: revisar la captura a ojo). `Ctrl+Alt+D` sobre el fondo del terminal → portapapeles `#RRGGBB`.
- [ ] `medidas/2026-09-02-equipo-desarrollo-s1b3.md`; plan maestro; spec; memoria; PR → CI → merge.
