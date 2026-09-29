//! The cell-SSA view (M5.5): the definitions reaching each whole-cell load,
//! through branches, loops and a nested function that writes the cell.
use super::super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

fn source_program<'src>(arena: &'src bumpalo::Bump, source: &'src str) -> Program<'src> {
    let syntax = crate::parse_source(arena, source)
        .unwrap_or_else(|error| panic!("parse: {error:?}\n{source}"));
    let checked =
        crate::analyze(&syntax).unwrap_or_else(|error| panic!("check: {error:?}\n{source}"));
    let program = from_checked_source(&syntax, &checked)
        .unwrap_or_else(|error| panic!("convert: {error:?}\n{source}"));
    program.verify().unwrap();
    program
}

fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: 64_000_000,
        },
    )
    .unwrap()
}

/// What each whole-cell load of `name` in the unit named `function` reads,
/// in operation order: the kind of each reaching definition.
fn loads(source: &str, function: &str, name: &str) -> Vec<Option<Vec<&'static str>>> {
    let arena = bumpalo::Bump::new();
    let program = source_program(&arena, source);
    let mut ledger = ledger();
    let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
    let unit = program
        .units
        .iter()
        .position(|unit| {
            unit.data()
                .function_name
                .is_some_and(|label| program.strings[label.index()].as_unicode() == Some(function))
        })
        .map(|index| UnitId::from_index(index).unwrap())
        .unwrap_or_else(|| panic!("no unit named {function}"));
    let ssa = CellSsa::build(&program, &uses, unit, |_| Ok::<_, ()>(())).unwrap();
    let data = program.units[unit.index()].data();
    let mut found = Vec::new();
    for (index, operation) in data.operations.iter().enumerate() {
        let OperationKind::Load(place) = operation.kind else {
            continue;
        };
        let Place::Cell(cell) = data.places[place.index()] else {
            continue;
        };
        if program.cells[cell.index()].name != name {
            continue;
        }
        let load = OpId::from_index(index).unwrap();
        found.push(ssa.reaching(load).map(|definitions| {
            definitions
                .iter()
                .map(|definition| match definition {
                    Definition::Entry => "entry",
                    Definition::Operation(op) => match data.operations[op.index()].kind {
                        OperationKind::Initialize(_) => "init",
                        OperationKind::Declare(_) => "declare",
                        OperationKind::Store(_) => "store",
                        OperationKind::ForOf { .. } | OperationKind::ForIn { .. } => "binding",
                        OperationKind::Try { .. } => "catch",
                        _ => "other",
                    },
                })
                .collect()
        }));
    }
    found
}

#[test]
fn a_straight_line_reads_its_last_definition() {
    let source = "int f(int a){int x=a;x=x+1;return x;} print(f(1));";
    assert_eq!(loads(source, "f", "x"), vec![Some(vec!["init"]), Some(vec!["store"])]);
    assert_eq!(loads(source, "f", "a"), vec![Some(vec!["entry"])]);
}

#[test]
fn a_branch_joins_its_definitions() {
    let source = "int g(bool c){int x=1;if(c){x=2;}return x;} print(g(true));";
    assert_eq!(loads(source, "g", "x"), vec![Some(vec!["init", "store"])]);
    let source = "int h(bool c){int x;if(c){x=2;}else{x=3;}return x;} print(h(true));";
    assert_eq!(loads(source, "h", "x"), vec![Some(vec!["store", "store"])]);
}

#[test]
fn a_loop_reaches_its_fixed_point() {
    let source =
        "int s(int n){int t=0;for(int i=0;i<n;i=i+1){t=t+i;}return t;} print(s(3));";
    // The read in the body sees the initialization and the body's store;
    // so does the read after the loop.
    assert_eq!(
        loads(source, "s", "t"),
        vec![Some(vec!["init", "store"]), Some(vec!["init", "store"])]
    );
}

#[test]
fn a_break_and_a_return_leave_their_definitions_to_their_targets() {
    let source = "int b(int n){int x=0;while(true){x=1;if(n>0){break;}x=2;}return x;} print(b(1));";
    // Only the definition before the `break` leaves the loop.
    assert_eq!(loads(source, "b", "x"), vec![Some(vec!["store"])]);
}

#[test]
fn a_cell_a_nested_function_writes_is_not_tracked() {
    let source = "int k(){int x=1;auto f=()=>{x=2;};f();return x;} print(k());";
    assert_eq!(loads(source, "k", "x"), vec![None]);
    // Only read by the nested function: still the owner's alone.
    let source = "int r(){int x=1;auto f=()=>x;return x+f();} print(r());";
    assert_eq!(loads(source, "r", "x"), vec![Some(vec!["init"])]);
}
