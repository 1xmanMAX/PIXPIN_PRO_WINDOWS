# S3-B y S3-C · Anotar el pin y anotar la pantalla

**Fecha:** 2026-09-02 · **Estado:** aprobado (decisiones auto-aprobadas bajo la
autorización del usuario del 2026-09-01) · **Depende de:** S3-A
(`pixpin-motor2d`, ya construido y medido).

Las dos fases comparten spec porque comparten casi todo: la misma caja de
herramientas, el mismo motor, la misma máquina de interacción. Lo único que
cambia es **sobre qué se dibuja**.

## 1. Lo que pidió el usuario, literal

> «poder hacer anotaciones dentro del pin […] con un lápiz resaltador líneas
> […] la función de foco la función de lupa […] esas mismas funciones de
> anotación que se puedan usar para anotar sobre la pantalla […] que tenga dos
> modos: uno en el que captura la pantalla entera y así lo pone estático, y
> otro sobre la pantalla que se está moviendo, pone como una capa encima y
> poder entrar sobre esa capa mientras la pantalla se mueve, poder seguir
> moviendo, haciendo clic y todo»

## 2. Decisiones

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D46 | **Dónde vive la interacción** | Un módulo **puro** `pixpin-ui::anotador`: herramienta activa, gesto en curso, y qué elemento produce. Las ventanas (pin, capa) solo le pasan eventos | Es la lección de todo el proyecto: la lógica probada en CI, el Win32 en el borde. Y así el pin y la pantalla comparten comportamiento sin copiar código |
| D47 | **Entrar y salir de anotar en el pin** | **Doble clic** entra (hoy alterna tamaño; pasa al menú, como pide el diseño de S2 §4.5); `Esc` sale guardando; el pin **no se puede mover ni redimensionar** mientras se anota | Sin un modo explícito, arrastrar sería ambiguo: ¿mueves el pin o dibujas? |
| D48 | **Dónde se guarda la anotación del pin** | Un `.pixpin2d` **junto al objeto**, mismo nombre y otra extensión (`000041.png` → `000041.pixpin2d`). El índice no cambia | El objeto original **nunca se toca** (regla del almacén desde S2-A): la anotación es una capa, y borrarla es borrar un fichero |
| D49 | **Los dos modos de pantalla** | **Congelada**: el overlay de S1-B2 con la caja de herramientas encima. **Capa viva**: una ventana transparente a pantalla completa, siempre encima, sobre la que se dibuja mientras la pantalla de abajo sigue funcionando | Son los dos que pidió el usuario, y comparten todo menos el fondo |
| D50 | **Pasar los clics a la aplicación de abajo** | La capa viva tiene dos estados: **dibujando** (recibe el ratón) y **pasante** (`WS_EX_TRANSPARENT`: los clics atraviesan y el dibujo se sigue viendo). Se alterna con el mismo atajo que la abrió, y con `Ctrl` mantenido | Es exactamente «poder entrar sobre esa capa mientras la pantalla se mueve, y poder seguir haciendo clic»: sin esto, la capa secuestra el escritorio |
| D51 | **Foco** | Oscurece todo menos una zona: un rectángulo o elipse que se arrastra, con el resto al 60 % de negro. Es un elemento más de la escena, así que se mueve, se borra y se deshace como cualquier otro | Es lo que se usa para señalar algo en una demostración |
| D52 | **Lupa** | Amplía una zona redonda alrededor del cursor (×2 por defecto, la rueda cambia el factor). **No es un elemento**: es una vista, no se guarda | Ampliar es para mirar, no para dejar constancia. La lupa del overlay de S1-B2 ya tiene el código de muestreo |
| D53 | **Caja de herramientas** | Barra flotante propia, dibujada con `pixpin-render` como la del overlay: lápiz, resaltador, línea, flecha, rectángulo, elipse, texto, foco, lupa, borrar, deshacer, color, grosor | Una barra nativa de Win32 no puede vivir sobre una ventana de composición sin cromo |
| D54 | **Salida de la capa viva** | `Esc` cierra y **pregunta si guardar** si hay algo dibujado; guardar crea un pin con la captura anotada | Cerrar sin avisar tirando cinco minutos de anotaciones es el peor fallo posible aquí |
| D55 | **Rueda del ratón** | Sobre un pin **sin** anotar: zoom del pin (lo que pidió el usuario). Anotando: cambia el grosor del trazo. Con la lupa: cambia el aumento | Cada modo le da a la rueda lo más útil de ese modo |

Decisiones tomadas al ejecutar S3-C (auto-aprobadas bajo la autorización del
2026-09-01; el detalle está en `docs/superpowers/plans/2026-09-02-s3c-anotar-pantalla.md`):

| # | Decisión | Elección |
|---|---|---|
| D56 | Cómo se entra al modo congelado | Un segundo atajo, `Ctrl+Alt+Shift+A` (ajuste `anotar_congelada`). Misma capa con la captura como fondo; sin modo pasante |
| D57 | Texto in situ | Vive en la máquina pura: clic abre un texto, cada carácter lo alarga, `Enter` confirma, `Escape` cancela, `Retroceso` borra, cambiar de herramienta confirma. El IME compone junto al punto de escritura |
| D58 | Caja de herramientas del pin | Una ventana paleta aparte (`WS_EX_NOACTIVATE`) colocada por `CajaHerramientas::colocar`, viva solo mientras se anota; la crea y la pinta el gestor |
| D59 | Captura final de la capa | Se captura la pantalla con la capa visible pero sin caja ni lupa (repintar, `DwmFlush` ×2, capturar); después se destruye la capa y se pregunta |
| D60 | Lupa sobre pantalla viva | Una `SesionViva` WGC con el tope de fps del nivel, abierta solo mientras la lupa está activa; la lupa se coloca fuera de su propia región fuente |
| D61 | Imágenes incrustadas | Fuera de S3-C: llegan con el almacén de bitmaps por anotación del PDF (S6) |

Precisión a D50 aprendida en la prueba: `WS_EX_TRANSPARENT` solo deja pasar el
ratón en una ventana `WS_EX_LAYERED`; se ponen los dos. Y el atajo global
pulsado con la capa abierta llega al bucle modal como `EventoOverlay::Atajo`
en vez de quedarse en la cola de la ventana principal.

## 3. La máquina de anotación (pura, `pixpin-ui::anotador`)

```rust
pub enum Herramienta {
    Mano,        // seleccionar y mover elementos
    Lapiz, Resaltador, Linea, Flecha, Rectangulo, Elipse, Texto,
    Foco, Lupa, Borrador,
}

pub enum EventoAnotador {
    Pulsar(Punto2), Mover(Punto2), Soltar(Punto2),
    Rueda(i32), Tecla(TeclaAnotador), CambiarHerramienta(Herramienta),
}

pub enum EfectoAnotador {
    Nada,
    Repintar,
    ElementoNuevo(Elemento),      // terminado: el consumidor lo anade a la escena
    ElementoEnCurso(Elemento),    // previsualizacion mientras se arrastra
    BorrarElemento(u64),
    Deshacer, Rehacer,
    PedirTexto(Punto2),           // el consumidor abre su editor de texto
    Salir { guardar: bool },
}
```

`Anotador::procesar(evento) -> EfectoAnotador`. Todo el comportamiento —qué
pasa al arrastrar con cada herramienta, cuándo un clic es clic y cuándo es
arrastre, qué borra el borrador— se prueba sin abrir una ventana.

## 4. Rendimiento

| Métrica | Objetivo |
|---|---|
| Trazo siguiendo al ratón | ≤ 1 fotograma real de retraso |
| Capa viva sobre pantalla en movimiento | 60 fps en `Completo`, 30 en `Ligero` |
| CPU de la capa viva en reposo (sin dibujar) | 0 % |
| Entrar en modo anotación en un pin | < 50 ms |

El motor ya deja margen: 100 elementos se regeneran en menos de un
milisegundo, así que el presupuesto se lo lleva la composición, no la
geometría.

## 5. Subfases

- **S3-B** — anotar el pin: máquina pura, caja de herramientas, dibujo dentro
  del pin, `.pixpin2d` junto al objeto, y la rueda como zoom.
- **S3-C** — anotar la pantalla: modo congelado sobre el overlay, capa viva
  con paso de clics, foco, lupa y el guardado como pin.

## 6. Criterios de aceptación

- [x] Doble clic en un pin entra en modo anotación; `Esc` sale y guarda (S3-B)
- [x] Lápiz, resaltador, líneas, flechas, formas y texto dibujan dentro del pin (texto: S3-C)
- [x] La anotación sobrevive a cerrar el pin y a reiniciar la aplicación (S3-B, revalidado en S3-C)
- [x] El objeto original del almacén no se modifica nunca
- [x] Atajo → capa viva sobre la pantalla; se dibuja encima sin congelarla
- [x] Con la capa en «pasante», los clics llegan a la aplicación de abajo
- [x] Foco y lupa funcionan en los dos modos (y en el pin)
- [x] La rueda hace zoom sobre un pin y cambia el grosor mientras se anota
- [x] Las cuatro puertas de §4 medidas y anotadas en `medidas/2026-09-02-equipo-desarrollo-s3c.md`
