use crate::arena_budget::{self, ArenaAdmission};
use crate::compilation_policy::{BudgetLedger, WorkDomain, WorkKind};
use crate::output_budget::AllocationError;
use bumpalo::Bump;
use std::alloc::Layout;
use std::cell::{Cell, RefCell};

/// Source text stays at a stable address without borrowing the compilation's
/// ledger. The private factory must discard it against that same ledger after
/// all syntax and semantic borrowers have dropped.
#[must_use = "discard stable source storage against its factory's ledger"]
pub(crate) struct StableSourceArena {
    bump: Option<Bump>,
    charged: Cell<usize>,
    domain: WorkDomain,
}

impl StableSourceArena {
    pub(crate) fn new(domain: WorkDomain) -> Self {
        let bump = Bump::new();
        debug_assert_eq!(bump.allocated_bytes(), 0);
        bump.set_allocation_limit(Some(0));
        Self {
            bump: Some(bump),
            charged: Cell::new(0),
            domain,
        }
    }

    pub(crate) fn store<'src>(
        &'src self,
        source: &str,
        ledger: &mut BudgetLedger,
    ) -> Result<&'src str, AllocationError> {
        ledger.charge(self.domain, WorkKind::Render, source.len() as u64)?;
        let admission = SourceAdmission {
            arena: self,
            ledger: RefCell::new(ledger),
        };
        let layout = Layout::array::<u8>(source.len()).map_err(|_| AllocationError::Capacity)?;
        arena_budget::attempt(self.bump(), &admission, layout, || {
            self.bump()
                .try_alloc_str(source)
                .map_err(|_| AllocationError::AllocationFailed)
        })
        .map(|stored| &*stored)
    }

    pub(crate) fn allocated_bytes(&self) -> usize {
        self.bump().allocated_bytes()
    }

    pub(crate) fn discard(mut self, ledger: &mut BudgetLedger) -> Result<(), AllocationError> {
        drop(self.bump.take());
        ledger.release(self.domain, self.charged.get() as u64)?;
        self.charged.set(0);
        Ok(())
    }

    fn bump(&self) -> &Bump {
        self.bump.as_ref().expect("live stable source arena")
    }
}

struct SourceAdmission<'arena, 'ledger> {
    arena: &'arena StableSourceArena,
    ledger: RefCell<&'ledger mut BudgetLedger>,
}

impl ArenaAdmission for SourceAdmission<'_, '_> {
    fn work(&self, units: u64) -> Result<(), AllocationError> {
        self.ledger
            .borrow_mut()
            .charge(self.arena.domain, WorkKind::Analysis, units)
            .map_err(Into::into)
    }

    fn reserve(&self, additional: usize) -> Result<(), AllocationError> {
        arena_budget::reserve(
            self.arena.bump(),
            &self.arena.charged,
            additional,
            |bytes| {
                self.ledger
                    .borrow_mut()
                    .retain(self.arena.domain, bytes)
                    .map_err(Into::into)
            },
        )
    }

    fn settle(&self) {
        arena_budget::settle(self.arena.bump(), &self.arena.charged, |bytes| {
            self.ledger
                .borrow_mut()
                .release(self.arena.domain, bytes)
                .expect("unused source arena capacity belongs to its factory");
        });
    }
}

#[cfg(test)]
#[path = "module_source_arena_tests.rs"]
mod tests;
