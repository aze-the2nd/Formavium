//! Inhaltsarten von Modulen für die Bibliotheksfilter.
//!
//! Verschachtelte Inhalte zählen mit. Mehrere Filter sind UND-verknüpft.

use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::{Element, ElementKind, ModuleDef, ModuleRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Text,
    Table,
    Image,
    Latex,
}

#[derive(Debug, Error, PartialEq)]
pub enum CatalogError {
    #[error("Modul {0} verweist zyklisch auf sich selbst")]
    Cycle(ModuleRef),
    #[error("Untermodul {0} ist nicht verfügbar")]
    Missing(ModuleRef),
}

/// Ermittelt die enthaltenen Inhaltsarten eines Moduls einschließlich
/// seiner Untermodule. `lookup` liefert Definitionen aus Dokument oder Bibliothek.
pub fn content_kinds<'a>(
    def: &'a ModuleDef,
    lookup: &dyn Fn(&ModuleRef) -> Option<&'a ModuleDef>,
) -> Result<BTreeSet<ContentKind>, CatalogError> {
    let mut kinds = BTreeSet::new();
    let mut stack = HashSet::new();
    walk(def, lookup, &mut stack, &mut kinds)?;
    Ok(kinds)
}

fn walk<'a>(
    def: &'a ModuleDef,
    lookup: &dyn Fn(&ModuleRef) -> Option<&'a ModuleDef>,
    stack: &mut HashSet<ModuleRef>,
    kinds: &mut BTreeSet<ContentKind>,
) -> Result<(), CatalogError> {
    let me = def.reference();
    if !stack.insert(me.clone()) {
        return Err(CatalogError::Cycle(me));
    }
    walk_elements(&def.body, lookup, stack, kinds)?;
    stack.remove(&me);
    Ok(())
}

fn walk_elements<'a>(
    body: &'a [Element],
    lookup: &dyn Fn(&ModuleRef) -> Option<&'a ModuleDef>,
    stack: &mut HashSet<ModuleRef>,
    kinds: &mut BTreeSet<ContentKind>,
) -> Result<(), CatalogError> {
    for el in body {
        match &el.kind {
            ElementKind::Heading { .. } | ElementKind::Paragraph { .. } => {
                kinds.insert(ContentKind::Text);
            }
            ElementKind::Table { .. } => {
                kinds.insert(ContentKind::Table);
            }
            ElementKind::Image { .. } => {
                kinds.insert(ContentKind::Image);
            }
            ElementKind::Latex { .. } => {
                kinds.insert(ContentKind::Latex);
            }
            ElementKind::Row { children } => walk_elements(children, lookup, stack, kinds)?,
            ElementKind::Module { module } => {
                let sub = lookup(module).ok_or_else(|| CatalogError::Missing(module.clone()))?;
                walk(sub, lookup, stack, kinds)?;
            }
        }
    }
    Ok(())
}

/// UND-Filter: alle gewünschten Arten müssen enthalten sein, weitere sind zulässig.
pub fn matches_all(kinds: &BTreeSet<ContentKind>, filters: &[ContentKind]) -> bool {
    filters.iter().all(|f| kinds.contains(f))
}
