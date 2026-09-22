//! A physical resource borrows the original semantic program and one sealed
//! export contract. Selecting roots never constructs a replacement program.
use super::physical_export::PhysicalExport;
use super::{CellId, Program, UnitId};

#[derive(Clone, Copy, Default)]
pub(super) enum ResourceView<'a> {
    #[default]
    Whole,
    Producer(&'a PhysicalExport),
    Consumer(&'a PhysicalExport),
}

impl<'a> ResourceView<'a> {
    pub(super) fn is_whole(self) -> bool {
        matches!(self, Self::Whole)
    }
    pub(super) fn includes_initializer(self, unit: UnitId) -> bool {
        match self {
            Self::Whole => true,
            Self::Producer(export) => unit == export.initializer(),
            Self::Consumer(export) => unit != export.initializer(),
        }
    }
    pub(super) fn entry_initializer(self, program: &Program<'_>) -> UnitId {
        match self {
            Self::Producer(export) => export.initializer(),
            Self::Whole | Self::Consumer(_) => program.modules[program.entry.index()].initializer,
        }
    }
    pub(super) fn imported(self, cell: CellId) -> bool {
        matches!(self, Self::Consumer(export) if cell == export.cell())
    }
    pub(super) fn producer_export(self) -> Option<&'a PhysicalExport> {
        match self {
            Self::Producer(export) => Some(export),
            _ => None,
        }
    }
    pub(super) fn consumer_import(self) -> Option<&'a PhysicalExport> {
        match self {
            Self::Consumer(export) => Some(export),
            _ => None,
        }
    }
}
