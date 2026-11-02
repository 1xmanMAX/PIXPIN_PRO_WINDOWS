# El HTML exportado: trackpad, ratón y pantalla táctil

PinDesk saca páginas web de un solo fichero que se abren en cualquier navegador sin instalar
nada: el **lienzo** (dibujo y tablas, `pindesk-motor2d/src/exportar_html.rs`) y el **plano de
AutoCAD** (`pindesk-cad/src/web.rs`). Las dos se mueven igual porque usan el mismo módulo,
`gestos_vista.js` (`GestosVista`), copiado en los dos crates —y en `apple/nucleo`— y vigilado
por una prueba para que no se separen.

## Cómo se mueve la vista

| Gesto | Qué hace |
|---|---|
| **Dos dedos en el trackpad**, en cualquier dirección (también en círculo) | Mueve la vista siguiendo a los dedos: se usan `deltaX` y `deltaY` a la vez. |
| **Pellizcar en el trackpad** (juntar o separar los dedos) | Acerca o aleja hacia el cursor, y solo eso. Chrome, Edge y Firefox lo mandan como rueda con `ctrlKey`; Safari, como `gesturechange`. |
| **Rueda del ratón** | Mueve en vertical, como un documento. |
| **Mayúsculas + rueda** | Mueve de lado. |
| **Ctrl (o ⌘) + rueda** | Acerca o aleja hacia el cursor. |
| **Pantalla táctil**: un dedo | Arrastra la vista (en el lienzo, con la mano; con el lápiz en la mano, raya). |
| **Pantalla táctil**: dos dedos | Acercan y mueven a la vez: lo que hay bajo los dedos se queda bajo los dedos. |
| `+` / `−` / `0` | Acercar, alejar y verlo entero (en el plano también `F`; doble clic encaja). |

## Detalles

- El acercamiento es `exp(−deltaY · 0,01)` por aviso, con tope de 25 px de rueda (×1,28): el
  pellizco va suave y una muesca de Ctrl+rueda no da saltos. Ir y volver deja la vista igual.
- `deltaMode` 1 (líneas, Firefox con ratón) cuenta 40 px por línea; `deltaMode` 2, el alto de la
  caja por página.
- **No se adivina si es ratón o trackpad.** Es lo que más falla, y así el trackpad se comporta
  siempre igual. Por eso en el plano la rueda ya no acerca sola: acerca con Ctrl, con el
  pellizco o con `+`/`−`. (El visor de planos de la aplicación no cambia.)
- La página nunca se desplaza ni se amplía por su cuenta: la rueda escucha con
  `{passive:false}` y hace `preventDefault`, igual que los `gesture*` de Safari, y la caja
  lleva `touch-action:none` y `overscroll-behavior:none`.
- En el iPad el pellizco de la pantalla llega también como `gesturechange`: mientras haya
  dedos posados, de él se encargan los Pointer Events y el gesto no cuenta, para no acercar el
  doble.
- Un pellizco que salta (la distancia se dobla o se parte entre dos avisos, o el centro salta
  más de 250 px) es la palma apoyada y se ignora.
- Los avisos se suman y se aplican una vez por fotograma (`requestAnimationFrame`).

## Pruebas

- `node --test crates/pindesk-motor2d/pruebas_js/gestos_vista.test.mjs`: las cuentas (rueda a
  desplazamiento en las dos direcciones, `deltaMode`, acercar dejando quieto el punto, el
  pellizco de dos dedos) y `conectar` con una caja de mentira; y que las copias son iguales.
- `cargo test -p pindesk-motor2d exportar_html` y `-p pindesk-cad web::`: que la página lleva el
  módulo antes que el visor y la rueda no pasiva.
