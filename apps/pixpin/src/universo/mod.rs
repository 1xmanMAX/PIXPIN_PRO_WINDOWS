//! El universo dentro de la app: traduce el chat a fichas, lee los
//! cuadernos sin parar la ventana y engancha el nucleo puro
//! (`pixpin-universo`) al editor.

// Las piezas llegan tarea a tarea y el binario solo usa lo que ya esta
// enganchado al editor: sin esto, cada pieza aun sin llamante romperia el
// `clippy -D warnings`. Se quita cuando la ultima entrada este conectada.
#![allow(dead_code)]

pub mod cargador;
pub mod estrellas;
pub mod fichas;
pub mod pintar;
