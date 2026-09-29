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

/// A forward analysis over one unit's regions.
pub(super) trait Forward {
    type State: Clone + PartialEq;
    /// The state no path reaches: the identity of `join`.
    fn unreachable(&self) -> Self::State;
    /// Join `from` into `into` (the least upper bound).
    fn join(&self, into: &mut Self::State, from: &Self::State);
    /// The state after `operation`, which is not a control operation, or
    /// the per-iteration binding of a `for…in` or `for…of` at its body's
    /// entry, or a catch binding at its region's entry.
    fn transfer(&self, unit: &UnitData, operation: OpId, state: &mut Self::State);
}

/// Why a solve stopped without an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stop<E> {
    /// The caller's budget refused more work.
    Budget(E),
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
}

struct Solver<'a, A: Forward, W> {
    unit: &'a UnitData,
    analysis: &'a A,
    before: Vec<A::State>,
    /// Innermost last.
    targets: Vec<Target<A::State>>,
    /// Returns and throws that leave the unit.
    exit: A::State,
    bound: usize,
    work: W,
}

/// Solve `analysis` over `unit` from `entry`, charging `work` per operation
/// visited.
pub(super) fn solve<A: Forward, E>(
    unit: &UnitData,
    analysis: &A,
    entry: A::State,
    work: impl FnMut(usize) -> Result<(), E>,
) -> Result<Solution<A::State>, Stop<E>> {
    let mut solver = Solver {
        unit,
        analysis,
        before: vec![analysis.unreachable(); unit.operations.len()],
        targets: Vec::new(),
        exit: analysis.unreachable(),
        bound: unit.operations.len() + 2,
        work,
    };
    solver.region(unit.entry, entry)?;
    Ok(Solution {
        before: solver.before,
    })
}

impl<A: Forward, E, W: FnMut(usize) -> Result<(), E>> Solver<'_, A, W> {
    fn charge(&mut self, units: usize) -> Result<(), Stop<E>> {
        (self.work)(units).map_err(Stop::Budget)
    }

    /// Flow `state` through `region`: the state its normal completion leaves.
    fn region(&mut self, region: RegionId, mut state: A::State) -> Result<A::State, Stop<E>> {
        let unit = self.unit;
        for &operation in &unit.regions[region.index()].operations {
            self.charge(1)?;
            self.before[operation.index()] = state.clone();
            // Any operation of a `Try`'s body may throw to its catch.
            if let Some(target) = self
                .targets
                .iter_mut()
                .rev()
                .find(|target| target.kind == TargetKind::Try)
            {
                self.analysis.join(&mut target.state, &state);
            }
            state = self.operation(operation, state)?;
        }
        Ok(state)
    }

    fn operation(&mut self, operation: OpId, state: A::State) -> Result<A::State, Stop<E>> {
        let unit = self.unit;
        let analysis = self.analysis;
        Ok(match unit.operations[operation.index()].kind {
            OperationKind::If { yes, no } => {
                let mut out = self.region(yes, state.clone())?;
                let other = match no {
                    Some(no) => self.region(no, state)?,
                    None => state,
                };
                analysis.join(&mut out, &other);
                out
            }
            OperationKind::Select { yes, no } => {
                let mut out = self.region(yes, state.clone())?;
                let other = self.region(no, state)?;
                analysis.join(&mut out, &other);
                out
            }
            OperationKind::ShortCircuit { right, .. } => {
                let right = self.region(right, state.clone())?;
                let mut out = state;
                analysis.join(&mut out, &right);
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
                self.fixed_point(state, |solver, head| {
                    let tested = solver.region(test, head)?;
                    let bodied = solver.region(body, tested.clone())?;
                    let exits = if endless {
                        solver.analysis.unreachable()
                    } else {
                        tested
                    };
                    Ok((exits, bodied))
                }, Some(update))?
            }
            OperationKind::ForIn { body, .. } | OperationKind::ForOf { body, .. } => {
                self.fixed_point(state, |solver, head| {
                    let mut entered = head.clone();
                    analysis.transfer(unit, operation, &mut entered);
                    let bodied = solver.region(body, entered)?;
                    // The loop exits from its head, before the next binding.
                    Ok((head, bodied))
                }, None)?
            }
            OperationKind::Try {
                body,
                catch,
                finally,
            } => self.try_operation(operation, state, body, catch, finally)?,
            OperationKind::Break | OperationKind::Continue => {
                let is_break = matches!(unit.operations[operation.index()].kind, OperationKind::Break);
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
                analysis.unreachable()
            }
            OperationKind::Return => {
                analysis.join(&mut self.exit, &state);
                analysis.unreachable()
            }
            OperationKind::Throw => {
                // The innermost `Try` already joined this state, as the state
                // before one of its body's operations.
                if !self
                    .targets
                    .iter()
                    .any(|target| target.kind == TargetKind::Try)
                {
                    analysis.join(&mut self.exit, &state);
                }
                analysis.unreachable()
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
        entry: A::State,
        mut iteration: impl FnMut(&mut Self, A::State) -> Result<(A::State, A::State), Stop<E>>,
        update: Option<RegionId>,
    ) -> Result<A::State, Stop<E>> {
        let analysis = self.analysis;
        let mut head = entry.clone();
        for _ in 0..self.bound {
            self.targets.push(Target {
                kind: TargetKind::Loop,
                state: analysis.unreachable(),
                continues: analysis.unreachable(),
            });
            let result = iteration(self, head.clone());
            let target = self.targets.pop().expect("the loop's own target");
            let (exits, mut bodied) = result?;
            analysis.join(&mut bodied, &target.continues);
            let back = match update {
                Some(update) => self.region(update, bodied)?,
                None => bodied,
            };
            let mut next = entry.clone();
            analysis.join(&mut next, &back);
            if next == head {
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
        state: A::State,
        body: RegionId,
        catch: Option<(Option<CellId>, RegionId)>,
        finally: Option<RegionId>,
    ) -> Result<A::State, Stop<E>> {
        let analysis = self.analysis;
        let unit = self.unit;
        self.targets.push(Target {
            kind: TargetKind::Try,
            state: state.clone(),
            continues: analysis.unreachable(),
        });
        let result = self.region(body, state);
        let target = self.targets.pop().expect("the try's own target");
        let mut out = result?;
        let throws = target.state;
        let mut reaching_finally = out.clone();
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
        Ok(finished)
    }

    /// A throw leaving a `Try` without a catch reaches the next enclosing
    /// `Try`, or the unit's exit.
    fn throw_outward(&mut self, state: &A::State) {
        let analysis = self.analysis;
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
