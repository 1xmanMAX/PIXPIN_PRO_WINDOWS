//! **El editor de notas Markdown de PixPin** (H12): el puerto del
//! `MarkdownEditorActivity` del movil con su `EditorVivo`, sobre el
//! `RichEdit` de Windows (`msftedit.dll`), que trae gratis el IME, el
//! corrector ortografico, deshacer, el zoom con Ctrl+rueda, la
//! accesibilidad y las tablas; con la cabecera, la barra y los menus del
//! editor de documentos de Claude. Ver [`editor`].

pub mod barra_flotante;
pub mod disposicion;
pub mod editor;
pub mod incrustados;
pub mod integracion;
mod imagenes;
mod letras;
pub mod vista;
mod menu;
pub mod panel_comentarios;
mod pintor;
pub mod tabla_ancha;
pub mod tabla_rtf;
pub mod tema;
/// Exportar la nota a Word (H12, 1-oct): el editor y la hoja de compartir.
pub mod word;

pub use editor::comentarios::DeComentarios;
pub use editor::{Pedido, Rotulos, correr};
pub use panel_comentarios::RotulosComentarios;
