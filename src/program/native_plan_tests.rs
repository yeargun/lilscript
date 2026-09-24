//! Checked edits must re-establish native target obligations. Neither operation
//! arena order nor the original frontend's successful check proves them.
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::program::publication::{CheckpointLimit, Compilation};

const SOURCE: &str = "int answer(){return 42;}print(answer());";

fn with_program(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    inspect(from_checked_source(&syntax, &semantics).unwrap());
}

fn native_result(program: Program<'_>, expected_feature: Option<&str>) {
    program
        .verify()
        .expect("test edit remains a checked program");
    let policy = crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::Native)
        .unwrap();
    let mut compilation = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 1_000_000,
                optional_work: 1_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 10_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 2 },
    )
    .unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let bytes = compilation.ledger().retained_bytes();
    let mut entered = false;
    let result = compilation.with_native_c(source, &policy, WorkDomain::Baseline, |output| {
        entered = true;
        assert!(!output.as_str().is_empty());
    });
    match expected_feature {
        None => {
            result.unwrap();
            assert!(entered);
        }
        Some(expected) => {
            assert!(
                !entered,
                "native rejection must precede the C output boundary"
            );
            assert!(
                matches!(result, Err(NativeError::Unsupported { feature, .. }) if feature == expected),
                "expected {expected}, got {result:?}"
            );
        }
    }
    assert_eq!(
        compilation.ledger().retained_bytes(),
        bytes,
        "partial native planning must roll back"
    );
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

#[test]
fn checked_edit_cannot_move_callable_initialization_after_module_execution() {
    with_program(SOURCE, |program| native_result(program, None));
    with_program(SOURCE, |mut program| {
        let module = program.initialization[0];
        // Move the original exclusively owned unit, without retaining an
        // unaccounted Program/Arc sibling beside Compilation adoption.
        let mut working = program.units.remove(module.index()).into_working();
        let data = working.get_mut();
        let initialize = data.regions[data.entry.index()]
            .operations
            .iter()
            .copied()
            .find(|id| {
                matches!(data.operations[id.index()].kind, OperationKind::Initialize(cell)
                if matches!(program.cells[cell.index()].binding, CellBinding::Function(_)))
            })
            .unwrap();
        let call = data.regions[data.entry.index()]
            .operations
            .iter()
            .copied()
            .find(|id| matches!(data.operations[id.index()].kind, OperationKind::Call(_)))
            .unwrap();
        assert!(
            initialize.index() < call.index(),
            "original arena order stays unchanged"
        );
        let region = &mut data.regions[data.entry.index()];
        let position = region
            .operations
            .iter()
            .position(|id| *id == initialize)
            .unwrap();
        region.operations.remove(position);
        region.operations.push(initialize);
        assert!(
            region.operations.iter().position(|id| *id == call).unwrap()
                < region
                    .operations
                    .iter()
                    .position(|id| *id == initialize)
                    .unwrap()
        );
        program.units.insert(module.index(), working.freeze());
        let error = program.verify().unwrap_err();
        assert!(
            error == "module instantiation creation has no paired initialization",
            "the common verifier must reject an invalid schedule before any target: {error}"
        );
    });
}

#[test]
fn checked_nonvoid_body_without_structured_return_cannot_emit_c_fallthrough() {
    with_program(SOURCE, |mut program| {
        let function = program
            .cells
            .iter()
            .find_map(|cell| match cell.binding {
                CellBinding::Function(unit) => Some(unit),
                _ => None,
            })
            .unwrap();
        let mut working = program.units.remove(function.index()).into_working();
        let data = working.get_mut();
        let returned = data
            .operations
            .iter()
            .position(|operation| matches!(operation.kind, OperationKind::Return))
            .unwrap();
        let parent = data.operations[returned].region;
        let child = RegionId::from_index(data.regions.len()).unwrap();
        data.regions.push(Region {
            parent: Some(parent),
            operations: Vec::new(),
            result: None,
            span: data.operations[returned].span,
        });
        data.operations[returned].kind = OperationKind::Block(child);
        data.operations[returned].operands = OperandRange { start: 0, len: 0 };
        program.units.insert(function.index(), working.freeze());
        native_result(program, Some("native nonvoid fallthrough"));
    });
}
