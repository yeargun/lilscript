//! Unreachable operations (plan M7.8a's "dead code after folding";
//! architecture §8.4's floor): an operation no path reaches never runs, and
//! goes. After a `return`, a `throw`, a `break` or a `continue`, or after a
//! statement every path of which leaves, nothing in its region runs; a
//! folded branch that ends in an exit leaves such a tail behind (`if(true)
//! return a; return b`).
//!
//! Reachability is the region-structured solver's (M5.5, `dataflow.rs`) with
//! a two-point lattice. A declaration keeps its place even where no path
//! reaches it: a closure created earlier may still name its cell, whose
//! temporal dead zone then never ends, and that is the program's meaning. A
//! region whose unreachable part initializes a cell is left whole, since
//! the kept initialization reads a value computed before it, and so is a
//! region that yields a value, whose result names one of its operations.
//!
//! Prior art: Closure's `UnreachableCodeElimination` (on its control-flow
//! graph, `closure-compiler@0da58e1`), Terser's `dead_code`
//! (`terser@8fa44c8 lib/compress/index.js`), esbuild's `mangleStmts` dropping
//! statements after a jump (`esbuild@f6058f8 internal/js_parser/js_parser.go`).

use super::super::dataflow::{self, Forward};
use super::edit::{self, Editor};
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};

/// Whether any path reaches the state.
struct Reached<'a>(std::cell::RefCell<&'a mut [bool]>);

impl Forward for Reached<'_> {
    type State = bool;

    fn empty_in(&self, _: &mut AllocationBudget<'_>) -> Result<bool, AllocationError> {
        Ok(false)
    }
    fn copy_in(&self, state: &bool, _: &mut AllocationBudget<'_>) -> Result<bool, AllocationError> {
        Ok(*state)
    }
    fn before_operation(&self, _: &UnitData, operation: OpId, state: &bool) {
        self.0.borrow_mut()[operation.index()] = *state;
    }
    fn unreachable(&self) -> bool {
        false
    }

    fn join(&self, into: &mut bool, from: &bool) {
        *into |= *from;
    }

    fn transfer(&self, _: &UnitData, _: OpId, _: &mut bool) {}
}

pub(super) fn apply(
    editor: &mut Editor<'_>,
    dirty: &mut super::dirty::DirtyUnits,
    created: &[bool],
    receipt: &mut RuleReceipt,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    dirty.prepare(editor.program().units.len(), budget)?;
    budget.with_temporary_context(
        editor,
        |editor, budget| {
            let program = editor.program();
            let mut stats = RuleReceipt::default();
            let mut removals = Vec::new();
            for frozen in &program.units {
                let unit = frozen.id();
                let dead = dirty.plan(
                    program,
                    unit,
                    super::dirty::LocalRule::Unreachable,
                    created[unit.index()],
                    &mut stats,
                    budget,
                    |budget| {
                        if created[unit.index()] {
                            plan(frozen.data(), budget)
                        } else {
                            Ok(Vec::new())
                        }
                    },
                    Vec::is_empty,
                )?;
                if !dead.is_empty() {
                    budget.push(Retained, &mut removals, (unit, dead))?;
                }
            }
            Ok::<_, super::RuleError>((
                removals,
                stats.local_units_visited,
                stats.local_units_reused,
            ))
        },
        |(removals, visited, reused), editor, budget| {
            receipt.local_units_visited += visited;
            receipt.local_units_reused += reused;
            for (unit, dead) in removals {
                let data = editor.unit_mut_in(*unit, budget)?;
                for &op in dead {
                    edit::detach(data, op);
                    receipt.unreachable_operations += 1;
                }
            }
            Ok(!removals.is_empty())
        },
    )
}

fn plan(data: &UnitData, budget: &mut AllocationBudget<'_>) -> Result<Vec<OpId>, AllocationError> {
    budget.retained_phase(|budget| {
        let mut before = budget.filled(Scratch, data.operations.len(), false)?;
        match dataflow::solve_discard_in(
            data,
            &Reached(std::cell::RefCell::new(&mut before)),
            &true,
            budget,
        ) {
            Ok(()) => {}
            Err(dataflow::Stop::Unsettled) => return Ok(Vec::new()),
            Err(dataflow::Stop::Resources(error) | dataflow::Stop::Budget(error)) => {
                return Err(error)
            }
        }
        let mut dead = Vec::new();
        let mut regions = storage::collect(std::iter::once(data.entry), Scratch, budget)?;
        while let Some(region) = regions.pop() {
            let operations = &data.regions[region.index()].operations;
            budget.work(WorkKind::Analysis, operations.len() as u64 + 1)?;
            // A retained declaration/initializer or expression result may still
            // name the unreachable operands, so keep its complete region.
            let keeps = data.regions[region.index()].result.is_some()
                || operations
                    .iter()
                    .filter(|op| !before[op.index()])
                    .any(|op| {
                        matches!(
                            data.operations[op.index()].kind,
                            OperationKind::Initialize(_)
                        )
                    });
            for &op in operations {
                budget.work(WorkKind::Analysis, 1)?;
                let kind = &data.operations[op.index()].kind;
                if !before[op.index()] {
                    if !keeps && !matches!(kind, OperationKind::Declare(_)) {
                        budget.push(Retained, &mut dead, op)?;
                    }
                    continue;
                }
                for child in kind.child_regions() {
                    budget.push(Scratch, &mut regions, child)?;
                }
            }
        }
        Ok(dead)
    })
}
