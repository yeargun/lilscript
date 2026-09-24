//! Derived, revision-keyed views of a checked program (architecture §6
//! "Derived views", §7 "Facts").
//!
//! A view is computed from a `Program` on demand, never edited, and valid only
//! for the unit and table revisions it records. The program owns a lazily
//! filled cache of its views. Cloning or rebuilding a program starts an empty
//! cache, so an edited copy never reads a view of the program it was copied
//! from; a cached view is also checked against its dependencies.
use super::call_graph::Seal;
use super::effects::ProgramEffects;
use super::initialization::ProgramInitialization;
use super::*;
use std::sync::{Arc, OnceLock};

/// What a derived answer depends on: the table revision and the revision of
/// every unit it read. A fact is valid for a program only while all of them
/// are unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deps {
    pub tables: RevisionId,
    pub units: Vec<(UnitId, RevisionId)>,
}

impl Deps {
    /// Every unit of the program: the dependency of a whole-program view.
    pub fn of_program(program: &Program<'_>) -> Self {
        Self {
            tables: program.tables_revision,
            units: program
                .units
                .iter()
                .map(|unit| (unit.id(), unit.revision()))
                .collect(),
        }
    }
    pub fn valid_for(&self, program: &Program<'_>) -> bool {
        self.tables == program.tables_revision
            && self.units.iter().all(|&(unit, revision)| {
                program
                    .units
                    .get(unit.index())
                    .is_some_and(|frozen| frozen.id() == unit && frozen.revision() == revision)
            })
    }
}

/// One derived answer (architecture §7): known with what it depends on,
/// unknown for a stated reason, or cut off by a bound. Consumers treat the
/// last two alike; the distinction is evidence for receipts and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fact<T> {
    Known(T, Deps),
    Unknown(Reason),
    Truncated(Limit),
}

impl<T> Fact<T> {
    pub fn known(&self) -> Option<&T> {
        match self {
            Self::Known(value, _) => Some(value),
            _ => None,
        }
    }
}

/// Why a fact is not known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// An async or generator body: calling it schedules work the summary
    /// does not describe.
    Suspending,
    /// A constructor of a class with a host ancestor runs host code.
    HostConstructor,
}

/// The bound that cut a computation short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// A recursive component's summaries did not settle within the bound.
    Iterations,
}

/// The program's lazily computed views. See the module comment for how
/// cloning and sharing keep a view with the program it describes.
#[derive(Debug, Default)]
pub struct ProgramViews {
    slots: Slots,
}

#[derive(Debug, Default)]
struct Slots {
    /// Indexed by `Seal`: effects, with the initialization facts built
    /// beside them, under structural (script) and module sealing of root
    /// storage.
    effects: [OnceLock<Arc<ProgramEffects>>; 2],
}

impl Clone for ProgramViews {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<'src> Program<'src> {
    /// The initialization facts under `seal` (`initialization.rs`), built
    /// with the effect summaries they need and that read them.
    pub fn initialization_facts(&self, seal: Seal) -> Arc<ProgramInitialization> {
        Arc::clone(self.effects(seal).initialization())
    }

    /// The effect summaries and call graph under `seal`, computed once per
    /// program. A cached view whose dependencies no longer match (a unit was
    /// replaced in place) is recomputed rather than served.
    pub fn effects(&self, seal: Seal) -> Arc<ProgramEffects> {
        let slot = &self.views.slots.effects[seal as usize];
        let cached = slot.get_or_init(|| Arc::new(ProgramEffects::build(self, seal)));
        if cached.deps().valid_for(self) {
            Arc::clone(cached)
        } else {
            Arc::new(ProgramEffects::build(self, seal))
        }
    }
}
