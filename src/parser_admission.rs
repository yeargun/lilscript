use super::{ParseError, ParserCore};
use crate::arena_budget::{self, attempt, ArenaAdmission};
use crate::ast::Program;
use crate::compilation_policy::{BudgetLedger, WorkDomain, WorkKind};
use crate::lexer::{AdmittedLexError, Lexed, Token};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use bumpalo::collections::{CollectionAllocErr, Vec as BumpVec};
use bumpalo::Bump;
use std::alloc::Layout;
use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};

#[cfg(test)]
thread_local! {
    static ARENA_ACTIVITY: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

#[cfg(test)]
pub(crate) fn admitted_arena_activity_for_test() -> (usize, usize) {
    ARENA_ACTIVITY.with(Cell::get)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdmittedParseError {
    Syntax(ParseError),
    Resource(AllocationError),
}

impl AdmittedParseError {
    pub(super) fn new(span: crate::span::Span, message: impl Into<String>) -> Self {
        Self::Syntax(ParseError::new(span, message))
    }

    pub(super) fn inspection(self) -> ParseError {
        match self {
            Self::Syntax(error) => error,
            Self::Resource(_) => unreachable!("inspection parser has no resource owner"),
        }
    }
}

impl From<ParseError> for AdmittedParseError {
    fn from(error: ParseError) -> Self {
        Self::Syntax(error)
    }
}

impl From<AllocationError> for AdmittedParseError {
    fn from(error: AllocationError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for AdmittedParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AdmittedParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdmittedSourcesError {
    pub index: usize,
    pub error: AdmittedParseError,
}

/// The list owns Program values and runs element destructors before the arena.
pub(crate) struct ParsedSources<'arena, 'src>(ArenaVec<'arena, Program<'arena, 'src>>);

impl<'arena, 'src> ParsedSources<'arena, 'src> {
    pub(crate) fn push(
        &mut self,
        program: Program<'arena, 'src>,
    ) -> Result<(), AdmittedParseError> {
        self.0.push(program)
    }
}

impl<'arena, 'src> Deref for ParsedSources<'arena, 'src> {
    type Target = [Program<'arena, 'src>];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Debug for ParsedSources<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ParsedSources")
            .field("len", &self.len())
            .finish()
    }
}

/// Owns parser arena backing, lexical token/template storage and lexical work,
/// not diagnostic strings, checker allocations, or process memory.
/// AST borrows cannot outlive this owner; cleanup frees chunks before charges.
pub(crate) struct AdmittedArena<'ledger> {
    bump: Option<Bump>,
    budget: RefCell<AllocationBudget<'ledger>>,
    charged: Cell<usize>,
}

impl<'ledger> AdmittedArena<'ledger> {
    pub(crate) fn new(ledger: &'ledger mut BudgetLedger, domain: WorkDomain) -> Self {
        let bump = Bump::new();
        debug_assert_eq!(bump.allocated_bytes(), 0);
        bump.set_allocation_limit(Some(0));
        let arena = Self {
            bump: Some(bump),
            budget: RefCell::new(AllocationBudget::new(Some((ledger, domain)))),
            charged: Cell::new(0),
        };
        #[cfg(test)]
        ARENA_ACTIVITY.with(|activity| {
            let (live, parses) = activity.get();
            activity.set((live + 1, parses));
        });
        arena
    }

    pub(crate) fn parse<'arena, 'src>(
        &'arena self,
        source: &'src str,
    ) -> Result<Program<'arena, 'src>, AdmittedParseError> {
        #[cfg(test)]
        ARENA_ACTIVITY.with(|activity| {
            let (live, parses) = activity.get();
            activity.set((live, parses + 1));
        });
        ParserCore::new(self.bump(), source, Some(self))?.parse_program()
    }

    pub(crate) fn parse_many<'arena, 'src>(
        &'arena self,
        sources: impl IntoIterator<Item = &'src str>,
    ) -> Result<ParsedSources<'arena, 'src>, AdmittedSourcesError> {
        self.work(0).map_err(|error| AdmittedSourcesError {
            index: 0,
            error: error.into(),
        })?;
        let mut programs = self.parsed_sources();
        for (index, source) in sources.into_iter().enumerate() {
            self.work(1).map_err(|error| AdmittedSourcesError {
                index,
                error: error.into(),
            })?;
            let program = self
                .parse(source)
                .map_err(|error| AdmittedSourcesError { index, error })?;
            programs
                .push(program)
                .map_err(|error| AdmittedSourcesError { index, error })?;
        }
        Ok(programs)
    }

    pub(crate) fn parsed_sources<'arena, 'src>(&'arena self) -> ParsedSources<'arena, 'src> {
        ParsedSources(ArenaVec::new_in(self.bump(), Some(self)))
    }

    pub(crate) fn with_ledger<R>(
        &self,
        inspect: impl FnOnce(&mut BudgetLedger, WorkDomain) -> R,
    ) -> R {
        self.budget.borrow_mut().with_ledger(|owner| {
            let (ledger, domain) = owner.expect("admitted parser owns its ledger borrow");
            inspect(ledger, domain)
        })
    }

    pub(crate) fn allocated_bytes(&self) -> usize {
        self.bump().allocated_bytes()
    }

    fn bump(&self) -> &Bump {
        self.bump.as_ref().expect("live admitted parser arena")
    }
}

impl Drop for AdmittedArena<'_> {
    fn drop(&mut self) {
        drop(self.bump.take());
        #[cfg(test)]
        ARENA_ACTIVITY.with(|activity| {
            let (live, parses) = activity.get();
            activity.set((live.checked_sub(1).expect("live parser arena"), parses));
        });
        self.budget
            .get_mut()
            .release(AllocationClass::Scratch, self.charged.get() as u64)
            .expect("parser arena releases its owned capacity after deallocation");
    }
}

pub(super) trait Admission: ArenaAdmission {
    fn lex<'src>(
        &self,
        source: &'src str,
    ) -> Result<(Lexed<'src, Token<'src>>, u64), AdmittedLexError>;
    fn release_tokens(&self, bytes: u64);
}

impl ArenaAdmission for AdmittedArena<'_> {
    fn work(&self, units: u64) -> Result<(), AllocationError> {
        self.budget.borrow_mut().work(WorkKind::Analysis, units)
    }

    fn reserve(&self, additional: usize) -> Result<(), AllocationError> {
        arena_budget::reserve(self.bump(), &self.charged, additional, |bytes| {
            self.budget
                .borrow_mut()
                .retain(AllocationClass::Scratch, bytes)
        })
    }

    fn settle(&self) {
        arena_budget::settle(self.bump(), &self.charged, |bytes| {
            self.budget
                .borrow_mut()
                .release(AllocationClass::Scratch, bytes)
                .expect("unused parser capacity belongs to this scope");
        });
    }
}

impl Admission for AdmittedArena<'_> {
    fn lex<'src>(
        &self,
        source: &'src str,
    ) -> Result<(Lexed<'src, Token<'src>>, u64), AdmittedLexError> {
        crate::lexer::lex_admitted(source, &mut self.budget.borrow_mut())
    }

    fn release_tokens(&self, bytes: u64) {
        self.budget
            .borrow_mut()
            .release(AllocationClass::Scratch, bytes)
            .expect("lexical token storage releases its owned capacity");
    }
}

pub(super) struct TokenStorage<'arena, 'src> {
    lexed: Option<Lexed<'src, Token<'src>>>,
    admission: Option<&'arena dyn Admission>,
    bytes: u64,
}

impl<'arena, 'src> TokenStorage<'arena, 'src> {
    pub(super) fn new(
        source: &'src str,
        admission: Option<&'arena dyn Admission>,
    ) -> Result<Self, AdmittedLexError> {
        let (lexed, bytes) = match admission {
            Some(owner) => owner.lex(source)?,
            None => (
                crate::lexer::lex(source).map_err(AdmittedLexError::Syntax)?,
                0,
            ),
        };
        Ok(Self {
            lexed: Some(lexed),
            admission,
            bytes,
        })
    }
}

impl<'src> Deref for TokenStorage<'_, 'src> {
    type Target = Lexed<'src, Token<'src>>;

    fn deref(&self) -> &Self::Target {
        self.lexed.as_ref().expect("live lexical token storage")
    }
}

impl DerefMut for TokenStorage<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.lexed.as_mut().expect("live lexical token storage")
    }
}

impl Drop for TokenStorage<'_, '_> {
    fn drop(&mut self) {
        drop(self.lexed.take());
        if let Some(admission) = self.admission {
            admission.release_tokens(self.bytes);
        }
    }
}

pub(super) fn alloc<'arena, T>(
    arena: &'arena Bump,
    admission: Option<&dyn Admission>,
    value: T,
) -> Result<&'arena mut T, AdmittedParseError> {
    let Some(admission) = admission else {
        return Ok(arena.alloc(value));
    };
    let mut value = Some(value);
    attempt(arena, admission, Layout::new::<T>(), || {
        arena
            .try_alloc_with(|| value.take().expect("allocation initializes exactly once"))
            .map_err(|_| AllocationError::AllocationFailed)
    })
    .map_err(Into::into)
}

pub(super) struct ArenaVec<'arena, T> {
    inner: BumpVec<'arena, T>,
    arena: &'arena Bump,
    admission: Option<&'arena dyn Admission>,
}

impl<'arena, T> ArenaVec<'arena, T> {
    pub(super) fn new_in(arena: &'arena Bump, admission: Option<&'arena dyn Admission>) -> Self {
        Self {
            inner: BumpVec::new_in(arena),
            arena,
            admission,
        }
    }

    pub(super) fn push(&mut self, value: T) -> Result<(), AdmittedParseError> {
        if let Some(admission) = self.admission {
            admission.work(1)?;
            if self.inner.len() == self.inner.capacity() {
                let required = self
                    .inner
                    .len()
                    .checked_add(1)
                    .ok_or(AllocationError::Capacity)?;
                let capacity = self
                    .inner
                    .capacity()
                    .checked_mul(2)
                    .unwrap_or(required)
                    .max(required)
                    .max(4);
                let layout = Layout::array::<T>(capacity).map_err(|_| AllocationError::Capacity)?;
                admission.work(self.inner.len() as u64)?;
                let additional = capacity - self.inner.len();
                attempt(self.arena, admission, layout, || {
                    self.inner
                        .try_reserve_exact(additional)
                        .map_err(|error| match error {
                            CollectionAllocErr::CapacityOverflow => AllocationError::Capacity,
                            CollectionAllocErr::AllocErr => AllocationError::AllocationFailed,
                        })
                })?;
            }
        }
        self.inner.push(value);
        Ok(())
    }

    pub(super) fn extend(
        &mut self,
        values: impl IntoIterator<Item = T>,
    ) -> Result<(), AdmittedParseError> {
        for value in values {
            self.push(value)?;
        }
        Ok(())
    }

    pub(super) fn into_bump_slice(self) -> &'arena [T] {
        self.inner.into_bump_slice()
    }
}

impl<T> Deref for ArenaVec<'_, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.inner
    }
}

#[cfg(test)]
#[path = "parser_admission_tests.rs"]
mod tests;
