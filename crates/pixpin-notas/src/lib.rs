//! **El editor de notas Markdown de PixPin** (H12): el puerto del
//! `MarkdownEditorActivity` del movil con su `EditorVivo`, sobre el
//! `RichEdit` de Windows (`msftedit.dll`), que trae gratis el IME, el
//! corrector ortografico, deshacer, el zoom con Ctrl+rueda y la
//! accesibilidad. Ver [`editor`].

pub mod editor;

pub use editor::{Pedido, Rotulos, correr};
