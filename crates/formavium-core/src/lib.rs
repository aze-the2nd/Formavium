//! Formavium-Dokumentkern.
//!
//! Besitzt den gespeicherten Dokumentstand: Modell, Revisionen, Persistenz,
//! LaTeX-Erzeugung und Export. Enthält keine Oberflächen- oder Renderlogik.

pub mod catalog;
pub mod edit;
pub mod export;
pub mod io;
pub mod latex;
pub mod model;
pub mod sample;
pub mod source_edit;

pub use edit::{Command, EditError, Revision, Workspace};
pub use latex::{generate, Generated, Origin};
pub use model::*;
