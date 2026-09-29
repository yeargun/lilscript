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

/// Whether any path reaches the state.
struct Reached;

impl Forward for Reached {
    type State = bool;

    fn unreachable(&self) -> bool {
        false
    }

    fn join(&self, into: &mut bool, from: &bool) {
        *into |= *from;
    }

    fn transfer(&self, _: &UnitData, _: OpId, _: &mut bool) {}
}

pub(super) fn apply(editor: &mut Editor<'_>, receipt: &mut RuleReceipt) -> bool {
    let program = editor.program();
    let created = created_units(program);
    let mut removals: Vec<(UnitId, Vec<OpId>)> = Vec::new();
    for frozen in &program.units {
        let unit = frozen.id();
        if !created[unit.index()] {
            continue;
        }
        let data = frozen.data();
        let Ok(solution) = dataflow::solve(data, &Reached, true, |_| Ok::<(), ()>(())) else {
            continue;
        };
        // The outermost unreachable operations: what they own goes with them.
        // A region whose unreachable part initializes a cell keeps it whole:
        // the kept initialization reads the value computed before it.
        let mut dead = Vec::new();
        let mut regions = vec![data.entry];
        while let Some(region) = regions.pop() {
            let operations = &data.regions[region.index()].operations;
            let unreached = |op: &&OpId| !*solution.before(**op);
            // A region that yields a value (a branch of an expression, a
            // loop's test or update) keeps its operations: its result names
            // one of them.
            let keeps = data.regions[region.index()].result.is_some()
                || operations.iter().filter(unreached).any(|op| {
                    matches!(
                        data.operations[op.index()].kind,
                        OperationKind::Initialize(_)
                    )
                });
            for &op in operations {
                let kind = &data.operations[op.index()].kind;
                if !*solution.before(op) {
                    if !keeps && !matches!(kind, OperationKind::Declare(_)) {
                        dead.push(op);
                    }
                    continue;
                }
                regions.extend(kind.child_regions());
            }
        }
        if !dead.is_empty() {
            removals.push((unit, dead));
        }
    }
    if removals.is_empty() {
        return false;
    }
    for (unit, dead) in removals {
        let (data, _) = editor.unit_and_cells(unit);
        for op in dead {
            edit::detach(data, op);
            receipt.unreachable_operations += 1;
        }
    }
    true
}
