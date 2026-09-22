//! Shared classification of the existing lexical activation contract.
use super::{Cell, CellBinding, UnitKind};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ambient {
    This,
    Arguments,
}
pub(super) fn classify(cell: &Cell) -> Option<Ambient> {
    if cell.binding != CellBinding::Foreign {
        return None;
    }
    match cell.name.as_str() {
        "this" => Some(Ambient::This),
        "arguments" => Some(Ambient::Arguments),
        _ => None,
    }
}
pub(super) fn inherits(kind: UnitKind) -> bool {
    kind == UnitKind::Closure
}
