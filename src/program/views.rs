//! Derived, revision-keyed views of a checked program (architecture §6
//! "Derived views", §7 "Facts").
//!
//! A view is computed from a `Program` on demand, never edited, and valid only
//! for the unit and table revisions it records. The program owns a lazily
//! filled cache of its views. Cloning starts an empty cache. Source normalization
//! may retain one previous effect analysis to reuse unchanged components;
//! exact dependency checks precede reuse and the cache ends with that phase.
//! Other views invalidate completely, and every cached current view is checked
//! against the program revisions before it is served.
use super::aggregates::ProgramAggregates;
use super::call_graph::Seal;
use super::classes::ProgramClasses;
use super::effects::ProgramEffects;
use super::initialization::ProgramInitialization;
use super::ranges::ProgramRanges;
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
    pub(super) fn of_program_in(
        program: &Program<'_>,
        budget: &mut crate::output_budget::AllocationBudget<'_>,
    ) -> Result<Self, crate::output_budget::AllocationError> {
        Ok(Self {
            tables: program.tables_revision,
            units: super::analysis_storage::collect(
                program
                    .units
                    .iter()
                    .map(|unit| (unit.id(), unit.revision())),
                crate::output_budget::AllocationClass::Retained,
                budget,
            )?,
        })
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
    /// Only source normalization retains a previous generation. At most one
    /// old/current pair per sealing survives, and finish drops both.
    previous_effects: [Option<Cached<ProgramEffects>>; 2],
    reuse_effects: bool,
}

#[derive(Debug, Default)]
struct Slots {
    /// Indexed by `Seal`: effects, with the initialization facts built
    /// beside them, under structural (script) and module sealing of root
    /// storage.
    effects: [OnceLock<Cached<ProgramEffects>>; 2],
    /// Indexed by `Seal`: value ranges (M6.4b), read by every formation.
    ranges: [OnceLock<Arc<ProgramRanges>>; 2],
    classes: [OnceLock<Arc<ProgramClasses>>; 2],
    aggregates: [OnceLock<Arc<ProgramAggregates>>; 2],
}

#[derive(Debug)]
struct Cached<T> {
    value: Arc<T>,
    bytes: u64,
}
impl<T> Cached<T> {
    fn discard(
        self,
        budget: &mut crate::output_budget::AllocationBudget<'_>,
    ) -> Result<(), crate::output_budget::AllocationError> {
        if self.bytes != 0 && Arc::strong_count(&self.value) != 1 {
            return Err(crate::output_budget::AllocationError::WrongOwner);
        }
        let bytes = self.bytes;
        drop(self);
        budget.release(crate::output_budget::AllocationClass::Retained, bytes)
    }
}

impl Clone for ProgramViews {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl ProgramViews {
    pub(super) fn normalization(reuse: bool) -> Self {
        Self {
            reuse_effects: reuse,
            ..Self::default()
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.invalidate_in(&mut crate::output_budget::AllocationBudget::new(None))
            .expect("inspection invalidation");
    }
    pub(super) fn invalidate_in(
        &mut self,
        budget: &mut crate::output_budget::AllocationBudget<'_>,
    ) -> Result<(), crate::output_budget::AllocationError> {
        for (slot, previous) in self
            .slots
            .effects
            .iter_mut()
            .zip(&mut self.previous_effects)
        {
            if let Some(current) = slot.take() {
                if let Some(old) = previous.replace(current) {
                    old.discard(budget)?;
                }
            }
        }
        // Both physical modes retain the same dependency-qualified stages.
        // A veto executes their cold builders without changing reservations.
        self.slots = Slots::default();
        Ok(())
    }
    pub(super) fn discard_in(
        &mut self,
        budget: &mut crate::output_budget::AllocationBudget<'_>,
    ) -> Result<(), crate::output_budget::AllocationError> {
        for slot in &mut self.slots.effects {
            if let Some(value) = slot.take() {
                value.discard(budget)?;
            }
        }
        for previous in &mut self.previous_effects {
            if let Some(value) = previous.take() {
                value.discard(budget)?;
            }
        }
        self.slots = Slots::default();
        Ok(())
    }
}

impl<'src> Program<'src> {
    pub(crate) fn aggregates(&self, seal: Seal) -> Arc<ProgramAggregates> {
        let slot = &self.views.slots.aggregates[seal as usize];
        let build = || Arc::new(ProgramAggregates::build(self, &self.effects(seal)));
        let cached = slot.get_or_init(build);
        if cached.deps().valid_for(self) {
            Arc::clone(cached)
        } else {
            build()
        }
    }

    pub(crate) fn primitive_classes(&self, seal: Seal) -> Arc<ProgramClasses> {
        let slot = &self.views.slots.classes[seal as usize];
        let build = || Arc::new(ProgramClasses::build(self, self.effects(seal).graph()));
        let cached = slot.get_or_init(build);
        if cached.deps().valid_for(self) {
            Arc::clone(cached)
        } else {
            build()
        }
    }

    /// The value ranges under `seal` (`ranges.rs`), computed once per
    /// program from its effects' call graph.
    pub fn ranges(&self, seal: Seal) -> Arc<ProgramRanges> {
        let slot = &self.views.slots.ranges[seal as usize];
        let build = || Arc::new(ProgramRanges::build(self, &self.effects(seal), seal));
        let cached = slot.get_or_init(build);
        if cached.deps().valid_for(self) {
            Arc::clone(cached)
        } else {
            build()
        }
    }

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
        let build = || {
            Arc::new(ProgramEffects::build_reusing(
                self,
                seal,
                self.views.previous_effects[seal as usize]
                    .as_ref()
                    .map(|old| &*old.value),
                self.views.reuse_effects,
            ))
        };
        let cached = slot.get_or_init(|| Cached {
            value: build(),
            bytes: 0,
        });
        if cached.value.deps().valid_for(self) {
            Arc::clone(&cached.value)
        } else {
            build()
        }
    }

    pub(crate) fn effects_in(
        &self,
        seal: Seal,
        budget: &mut crate::output_budget::AllocationBudget<'_>,
    ) -> Result<Arc<ProgramEffects>, crate::output_budget::AllocationError> {
        let slot = &self.views.slots.effects[seal as usize];
        if slot.get().is_none() {
            let cached = budget.retained_phase(|budget| {
                let value = ProgramEffects::build_reusing_in(
                    self,
                    seal,
                    self.views.previous_effects[seal as usize]
                        .as_ref()
                        .map(|old| &*old.value),
                    self.views.reuse_effects,
                    budget,
                )?;
                let value = super::analysis_storage::shared(value, budget)?;
                let bytes = budget.retained_bytes(crate::output_budget::AllocationClass::Retained);
                Ok::<_, crate::output_budget::AllocationError>(Cached { value, bytes })
            })?;
            slot.set(cached)
                .expect("one compiler owns analysis publication");
        }
        let cached = slot.get().unwrap();
        if !cached.value.deps().valid_for(self) || (budget.is_accounted() && cached.bytes == 0) {
            return Err(crate::output_budget::AllocationError::WrongOwner);
        }
        Ok(Arc::clone(&cached.value))
    }
}
