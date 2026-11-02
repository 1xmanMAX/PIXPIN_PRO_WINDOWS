//! **Las marcas sobre el texto reconocido de un pin** (8-oct-2026): resaltar,
//! raya ondulada, subrayar, tachar y tapar lo que se selecciono con el raton.
//!
//! Cada marca es dibujo normal del pin (`.pixpin2d`), no pixeles pintados en
//! la foto: se ve en el lienzo, viaja al copiar o arrastrar el pin (que
//! hornea lo anotado) y se puede borrar. Todo en pixeles de la imagen
//! original, que es el sistema del dibujo del pin y el de las cajas del
//! texto reconocido.
//!
//! Las figuras, el grupo y los pasos son el componente comun de todos los
//! visores (`pindesk_motor2d::marcas_texto`, 1-nov-2026): el lector de PDF y
//! el de Word y PowerPoint marcan con lo mismo.

pub use pindesk_motor2d::marcas_texto::{
    Cambio, GRUPO, PASOS, Paso, elementos, es_marca, tapadas, toca,
};
