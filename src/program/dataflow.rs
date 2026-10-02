//! One region-structured forward dataflow solver (plan M5.5; architecture
//! §6 "Derived views").
//!
//! A unit's control flow is its region tree: operations run in order within
//! a region, and a control operation (`If`, `Select`, `ShortCircuit`,
//! `Block`, the loops, `Try`) runs its child regions. An analysis supplies a
//! state, a join and a transfer for each operation that is not a control
//! operation; the solver supplies the flow:
//! * `If` and `Select` join their two branches (`If` without `else` joins the
//!   branch with its entry), and `ShortCircuit` joins its right region with
//!   its entry;
//! * a loop iterates to a fixed point: its head is the join of its entry and
//!   the state its update region (or, for `for…in` and `for…of`, its body)
//!   leaves, with the `continue`s joined in before the update; it exits with
//!   its test's state (unless the test is the constant `true`) joined with
//!   its `break`s. `for…in` and `for…of`
//!   transfer their own operation (the per-iteration binding) at the body's
//!   entry;
//! * a `Try` starts its catch region from the join of the entry and the state
//!   before every operation of its body (any of them may throw), and its
//!   finally region from the join of everything that can reach it; a finally
//!   region's result is also joined into every enclosing target, since the
//!   abrupt exits it guards continue from its end;
//! * `break` and `continue` leave their state to the innermost loop, `return`
//!   to the unit's exit and `throw` to the innermost `Try` (or the exit);
//!   after any of them nothing is reachable.
//!
//! The solution is the state before every operation. States must form a
//! lattice of finite height, so each loop's fixed point is reached; a loop
//! still unsettled after a generous bound (one iteration per operation of
//! the unit, plus two) ends the solve with `Unsettled`, never a partial
//! answer.
//!
//! Prior art: the structured solvers of Cranelift's egraph-free passes and of
//! Binaryen's `LocalGraph` (flow over structured control, joins at merges)
//! and the region-based analyses of the Dragon book, §9.7. Closure's
//! `DataFlowAnalysis` runs a worklist over a CFG built for the purpose; the
//! region tree needs none.
// Its first reader is the cell-SSA view (`cell_ssa`), read in turn by step
// 10's rules; until then only tests solve.
#![allow(dead_code)]

use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};
use std::cell::RefCell;

/// A forward analysis over one unit's regions.
pub(super) trait Forward {
    type State: Clone + PartialEq;
    /// Admitted analyses own fixed-capacity state: transfers, joins and
    /// widening must not allocate. Inspection-only analyses use the defaults.
    fn empty_in(&self, budget: &mut AllocationBudget<'_>) -> Result<Self::State, AllocationError> {
        if budget.is_accounted() {
            return Err(AllocationError::Unaccounted);
        }
        Ok(self.unreachable())
    }
    fn copy_in(
        &self,
        state: &Self::State,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self::State, AllocationError> {
        if budget.is_accounted() {
            return Err(AllocationError::Unaccounted);
        }
        Ok(state.clone())
    }
    fn state_work(&self) -> usize {
        1
    }
    /// The state no path reaches: the identity of `join`.
    fn unreachable(&self) -> Self::State;
    /// Join `from` into `into` (the least upper bound).
    fn join(&self, into: &mut Self::State, from: &Self::State);
    /// The state after `operation`, which is not a control operation, or
    /// the per-iteration binding of a `for…in` or `for…of` at its body's
    /// entry, or a catch binding at its region's entry.
    fn transfer(&self, unit: &UnitData, operation: OpId, state: &mut Self::State);

    /// A compact client-owned projection can observe visits without retaining
    /// a complete operations-by-state history. Like transfers, it cannot grow.
    fn before_operation(&self, _unit: &UnitData, _operation: OpId, _state: &Self::State) {}

    /// A select/short-circuit result after its child regions have been
    /// visited. Storage effects were already joined; a value analysis can
    /// now publish the expression's result without executing them twice.
    fn expression_result(&self, _unit: &UnitData, _operation: OpId, _state: &Self::State) {}

    /// What a branch's outcome adds: the state entering the `taken` side of
    /// `operation`'s test (an `if`/select, or a loop continuing when `taken` and
    /// leaving otherwise). Nothing by default.
    fn branch(&self, _unit: &UnitData, _operation: OpId, _taken: bool, _state: &mut Self::State) {}

    /// Widen a loop head after its first iteration: `next` follows
    /// `previous`, and an analysis whose lattice has infinite ascending
    /// chains jumps ahead here so the loop settles. Nothing by default.
    fn widen(&self, _previous: &Self::State, _next: &mut Self::State) {}
}

/// Why a solve stopped without an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stop<E> {
    /// The caller's budget refused more work.
    Budget(E),
    Resources(AllocationError),
    /// A loop did not settle within the bound: the lattice was not finite.
    Unsettled,
}

/// The state before every operation of the unit.
pub(super) struct Solution<S> {
    before: Vec<S>,
}

impl<S> Solution<S> {
    pub(super) fn before(&self, operation: OpId) -> &S {
        &self.before[operation.index()]
    }
}

/// The accumulators of an enclosing target.
struct Target<S> {
    kind: TargetKind,
    state: S,
    /// For a loop: the `continue`s; unused otherwise.
    continues: S,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Loop,
    Try,
    /// Abrupt paths crossing this guard must still execute its finalizer.
    Finally,
}

// Each state's exact backing has one owner. A branch/loop temporary drops
// its actual state before releasing its reservation, including on refusal.
// The solver's temporary container backing belongs to its enclosing scope.
struct State<'a, 'b, S> {
    value: Option<S>,
    bytes: u64,
    budget: &'a RefCell<AllocationBudget<'b>>,
}
impl<S> std::ops::Deref for State<'_, '_, S> {
    type Target = S;
    fn deref(&self) -> &S {
        self.value.as_ref().unwrap()
    }
}
impl<S> std::ops::DerefMut for State<'_, '_, S> {
    fn deref_mut(&mut self) -> &mut S {
        self.value.as_mut().unwrap()
    }
}
impl<S> Drop for State<'_, '_, S> {
    fn drop(&mut self) {
        drop(self.value.take());
        self.budget
            .borrow_mut()
            .release(Retained, self.bytes)
            .expect("solver state owns its backing");
    }
}

struct Solver<'a, 'b, 'c, A: Forward, W> {
    unit: &'a UnitData,
    analysis: &'a A,
    before: Vec<State<'b, 'c, A::State>>,
    targets: Vec<Target<State<'b, 'c, A::State>>>,
    exit: State<'b, 'c, A::State>,
    bound: usize,
    work: W,
    budget: &'b RefCell<AllocationBudget<'c>>,
}

fn state<'a, 'b, S>(
    budget: &'a RefCell<AllocationBudget<'b>>,
    build: impl FnOnce(&mut AllocationBudget<'_>) -> Result<S, AllocationError>,
) -> Result<State<'a, 'b, S>, AllocationError> {
    let (value, bytes) = budget.borrow_mut().retained_phase(|budget| {
        let value = build(budget)?;
        Ok::<_, AllocationError>((value, budget.retained_bytes(Retained)))
    })?;
    Ok(State {
        value: Some(value),
        bytes,
        budget,
    })
}

/// Inspection solve with operation history. Production fixed-state consumers
/// that need only their transfer results use `solve_discard_in` and allocate
/// no operations-by-state history.
pub(super) fn solve<A: Forward, E>(
    unit: &UnitData,
    analysis: &A,
    entry: A::State,
    work: impl FnMut(usize) -> Result<(), E>,
) -> Result<Solution<A::State>, Stop<E>> {
    let budget = RefCell::new(AllocationBudget::new(None));
    let mut solver = Solver::new(unit, analysis, true, work, &budget)?;
    let entry = state(&budget, |_| Ok(entry)).map_err(Stop::Resources)?;
    solver.region(unit.entry, entry)?;
    Ok(Solution {
        before: std::mem::take(&mut solver.before)
            .into_iter()
            .map(|mut state| state.value.take().unwrap())
            .collect(),
    })
}

pub(super) fn solve_discard_in<A: Forward>(
    unit: &UnitData,
    analysis: &A,
    entry: &A::State,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), Stop<AllocationError>> {
    let budget = RefCell::new(budget.scope());
    let mut solver = Solver::new(unit, analysis, false, |_| Ok(()), &budget)?;
    let entry = solver.copy(entry)?;
    solver.region(unit.entry, entry)?;
    Ok(())
}

impl<'a, 'b, 'c, A: Forward, E, W: FnMut(usize) -> Result<(), E>> Solver<'a, 'b, 'c, A, W> {
    fn new(
        unit: &'a UnitData,
        analysis: &'a A,
        history: bool,
        work: W,
        budget: &'b RefCell<AllocationBudget<'c>>,
    ) -> Result<Self, Stop<E>> {
        let mut before = budget
            .borrow_mut()
            .vector(Scratch, if history { unit.operations.len() } else { 0 })
            .map_err(Stop::Resources)?;
        if history {
            for _ in &unit.operations {
                before.push(
                    state(budget, |budget| analysis.empty_in(budget)).map_err(Stop::Resources)?,
                );
            }
        }
        // At most one active target per region-owning operation. This exact
        // structural upper bound prevents target growth inside recursive flow.
        budget
            .borrow_mut()
            .work(WorkKind::Analysis, unit.operations.len() as u64)
            .map_err(Stop::Resources)?;
        let capacity = unit
            .operations
            .iter()
            .map(|op| match op.kind {
                OperationKind::Loop { .. } | OperationKind::ForIn { .. } | OperationKind::ForOf { .. } => 1,
                OperationKind::Try { finally, .. } => 1 + usize::from(finally.is_some()),
                _ => 0,
            })
            .sum();
        let targets = budget
            .borrow_mut()
            .vector(Scratch, capacity)
            .map_err(Stop::Resources)?;
        let exit = state(budget, |budget| analysis.empty_in(budget)).map_err(Stop::Resources)?;
        Ok(Self {
            unit,
            analysis,
            before,
            targets,
            exit,
            bound: unit.operations.len().saturating_add(2),
            work,
            budget,
        })
    }
    fn charge(&mut self, units: usize) -> Result<(), Stop<E>> {
        (self.work)(units).map_err(Stop::Budget)?;
        let work = units
            .checked_mul(self.analysis.state_work())
            .ok_or(Stop::Resources(AllocationError::Capacity))?;
        self.budget
            .borrow_mut()
            .work(WorkKind::Analysis, work as u64)
            .map_err(Stop::Resources)
    }
    fn copy(&self, value: &A::State) -> Result<State<'b, 'c, A::State>, Stop<E>> {
        state(self.budget, |budget| self.analysis.copy_in(value, budget)).map_err(Stop::Resources)
    }
    fn empty(&self) -> Result<State<'b, 'c, A::State>, Stop<E>> {
        state(self.budget, |budget| self.analysis.empty_in(budget)).map_err(Stop::Resources)
    }

    /// Flow `state` through `region`: the state its normal completion leaves.
    fn region(
        &mut self,
        region: RegionId,
        mut state: State<'b, 'c, A::State>,
    ) -> Result<State<'b, 'c, A::State>, Stop<E>> {
        let unit = self.unit;
        for &operation in &unit.regions[region.index()].operations {
            self.charge(1)?;
            if !self.before.is_empty() {
                self.before[operation.index()] = self.copy(&state)?;
            }
            self.analysis.before_operation(unit, operation, &state);
            // A possible exception crosses intervening finalizers before
            // reaching its catcher. In particular, a catch is outside the
            // body's Try target but remains inside the same finalizer guard.
            for target in self.targets.iter_mut().rev() {
                if matches!(target.kind, TargetKind::Finally | TargetKind::Try) {
                    self.analysis.join(&mut target.state, &state);
                }
                if target.kind == TargetKind::Try { break; }
            }
            state = self.operation(operation, state)?;
        }
        Ok(state)
    }

    fn operation(
        &mut self,
        operation: OpId,
        state: State<'b, 'c, A::State>,
    ) -> Result<State<'b, 'c, A::State>, Stop<E>> {
        let unit = self.unit;
        let analysis = self.analysis;
        Ok(match unit.operations[operation.index()].kind {
            OperationKind::If { yes, no } => {
                let mut taken = self.copy(&state)?;
                analysis.branch(unit, operation, true, &mut taken);
                let mut other = state;
                analysis.branch(unit, operation, false, &mut other);
                let mut out = self.region(yes, taken)?;
                let other = match no {
                    Some(no) => self.region(no, other)?,
                    None => other,
                };
                analysis.join(&mut out, &other);
                out
            }
            OperationKind::Select { yes, no } => {
                let mut taken = self.copy(&state)?;
                analysis.branch(unit, operation, true, &mut taken);
                let mut other = state;
                analysis.branch(unit, operation, false, &mut other);
                let mut out = self.region(yes, taken)?;
                let other = self.region(no, other)?;
                analysis.join(&mut out, &other);
                analysis.expression_result(unit, operation, &out);
                out
            }
            OperationKind::ShortCircuit { right, .. } => {
                let right = self.region(right, self.copy(&state)?)?;
                let mut out = state;
                analysis.join(&mut out, &right);
                analysis.expression_result(unit, operation, &out);
                out
            }
            OperationKind::Block(region) => self.region(region, state)?,
            OperationKind::Loop { test, body, update } => {
                // A test that is the constant `true` never ends the loop:
                // only its `break`s leave it.
                let endless = unit.regions[test.index()].result.is_some_and(|value| {
                    matches!(
                        unit.operations[unit.values[value.index()].definition.index()].kind,
                        OperationKind::Constant(Constant::Boolean(true))
                    )
                });
                self.fixed_point(
                    state,
                    |solver, head| {
                        let tested = solver.region(test, head)?;
                        let mut entered = solver.copy(&tested)?;
                        solver.analysis.branch(unit, operation, true, &mut entered);
                        let bodied = solver.region(body, entered)?;
                        let exits = if endless {
                            solver.empty()?
                        } else {
                            let mut left = tested;
                            solver.analysis.branch(unit, operation, false, &mut left);
                            left
                        };
                        Ok((exits, bodied))
                    },
                    Some(update),
                )?
            }
            OperationKind::ForIn { body, .. } | OperationKind::ForOf { body, .. } => {
                self.fixed_point(
                    state,
                    |solver, head| {
                        let mut entered = solver.copy(&head)?;
                        analysis.transfer(unit, operation, &mut entered);
                        let bodied = solver.region(body, entered)?;
                        // The loop exits from its head, before the next binding.
                        Ok((head, bodied))
                    },
                    None,
                )?
            }
            OperationKind::Try {
                body,
                catch,
                finally,
            } => self.try_operation(operation, state, body, catch, finally)?,
            OperationKind::Break | OperationKind::Continue => {
                let is_break = matches!(
                    unit.operations[operation.index()].kind,
                    OperationKind::Break
                );
                self.finalizers_before(Some(TargetKind::Loop), &state);
                if let Some(target) = self
                    .targets
                    .iter_mut()
                    .rev()
                    .find(|target| target.kind == TargetKind::Loop)
                {
                    let into = if is_break {
                        &mut target.state
                    } else {
                        &mut target.continues
                    };
                    analysis.join(into, &state);
                }
                self.empty()?
            }
            OperationKind::Return => {
                self.finalizers_before(None, &state);
                analysis.join(&mut self.exit, &state);
                self.empty()?
            }
            OperationKind::Throw => {
                self.finalizers_before(Some(TargetKind::Try), &state);
                // The innermost `Try` already joined this state, as the state
                // before one of its body's operations.
                if !self
                    .targets
                    .iter()
                    .any(|target| target.kind == TargetKind::Try)
                {
                    analysis.join(&mut self.exit, &state);
                }
                self.empty()?
            }
            _ => {
                let mut state = state;
                analysis.transfer(unit, operation, &mut state);
                state
            }
        })
    }

    /// A loop to its fixed point. `iteration` flows one iteration from the
    /// head: it returns the state the loop may exit with at its head or test,
    /// and the state its body leaves. The `continue`s join that state before
    /// `update` (when the loop has one) and the result is the next head.
    fn fixed_point(
        &mut self,
        entry: State<'b, 'c, A::State>,
        mut iteration: impl FnMut(
            &mut Self,
            State<'b, 'c, A::State>,
        )
            -> Result<(State<'b, 'c, A::State>, State<'b, 'c, A::State>), Stop<E>>,
        update: Option<RegionId>,
    ) -> Result<State<'b, 'c, A::State>, Stop<E>> {
        let analysis = self.analysis;
        let mut head = self.copy(&entry)?;
        for round in 0..self.bound {
            let target = Target {
                kind: TargetKind::Loop,
                state: self.empty()?,
                continues: self.empty()?,
            };
            self.targets.push(target);
            let input = self.copy(&head)?;
            let result = iteration(self, input);
            let target = self.targets.pop().expect("the loop's own target");
            let (exits, mut bodied) = result?;
            analysis.join(&mut bodied, &target.continues);
            let back = match update {
                Some(update) => self.region(update, bodied)?,
                None => bodied,
            };
            let mut next = self.copy(&entry)?;
            analysis.join(&mut next, &back);
            if round > 0 {
                analysis.widen(&head, &mut next);
            }
            if *next == *head {
                let mut out = exits;
                analysis.join(&mut out, &target.state);
                return Ok(out);
            }
            head = next;
        }
        Err(Stop::Unsettled)
    }

    fn try_operation(
        &mut self,
        operation: OpId,
        state: State<'b, 'c, A::State>,
        body: RegionId,
        catch: Option<(Option<CellId>, RegionId)>,
        finally: Option<RegionId>,
    ) -> Result<State<'b, 'c, A::State>, Stop<E>> {
        let analysis = self.analysis;
        let unit = self.unit;
        if finally.is_some() {
            self.targets.push(Target {
                kind: TargetKind::Finally,
                state: self.empty()?,
                continues: self.empty()?,
            });
        }
        let target = Target {
            kind: TargetKind::Try,
            state: self.copy(&state)?,
            continues: self.empty()?,
        };
        self.targets.push(target);
        let result = self.region(body, state);
        let target = self.targets.pop().expect("the try's own target");
        let mut out = result?;
        let throws = target.state;
        let mut reaching_finally = self.copy(&out)?;
        match catch {
            Some((binding, region)) => {
                let mut entered = throws;
                if binding.is_some() {
                    analysis.transfer(unit, operation, &mut entered);
                }
                let caught = self.region(region, entered)?;
                analysis.join(&mut out, &caught);
                analysis.join(&mut reaching_finally, &caught);
            }
            None => {
                // Without a catch the throw continues outward, through the
                // finally region.
                analysis.join(&mut reaching_finally, &throws);
                self.throw_outward(&throws);
            }
        }
        let Some(finally) = finally else {
            return Ok(out);
        };
        let guard = self.targets.pop().expect("the finalizer's own guard");
        debug_assert!(guard.kind == TargetKind::Finally);
        analysis.join(&mut reaching_finally, &guard.state);
        let continues = *out != *self.empty()?;
        let finished = self.region(finally, reaching_finally)?;
        // The finally region also runs on every abrupt exit it guards, which
        // continues from its end: every enclosing target may receive it.
        for target in &mut self.targets {
            analysis.join(&mut target.state, &finished);
            if target.kind == TargetKind::Loop {
                analysis.join(&mut target.continues, &finished);
            }
        }
        analysis.join(&mut self.exit, &finished);
        // Executing cleanup on an abrupt path does not turn that path into a
        // normal continuation after the try statement.
        if continues { Ok(finished) } else { self.empty() }
    }

    fn finalizers_before(&mut self, destination: Option<TargetKind>, state: &A::State) {
        for target in self.targets.iter_mut().rev() {
            if Some(target.kind) == destination { break; }
            if target.kind == TargetKind::Finally { self.analysis.join(&mut target.state, state); }
        }
    }

    /// A throw leaving a `Try` without a catch reaches the next enclosing
    /// `Try`, or the unit's exit.
    fn throw_outward(&mut self, state: &A::State) {
        let analysis = self.analysis;
        self.finalizers_before(Some(TargetKind::Try), state);
        match self
            .targets
            .iter_mut()
            .rev()
            .find(|target| target.kind == TargetKind::Try)
        {
            Some(target) => analysis.join(&mut target.state, state),
            None => analysis.join(&mut self.exit, state),
        }
    }
}

#[cfg(test)]
#[path = "dataflow_tests.rs"]
mod tests;
