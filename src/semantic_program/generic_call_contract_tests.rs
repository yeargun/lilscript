//! Original checked call instances, before any target/layout adaptation.
use super::publication::{
    CheckpointLimit, Compilation, OperationPatch, PublicationError, SemanticId, UnitPatch,
};
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 20_000_000,
            optional_work: 20_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 40_000_000,
        },
    )
    .unwrap()
}
fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program);
}

#[test]
fn imported_generic_calls_keep_original_arguments_and_one_body_for_point_and_integer() {
    let sources = [
        "import {Point} from \"./point\";import {snapshot as forward} from \"./snapshot\";Point point=Point{2,3};Point saved=forward(point);export int count=forward(7);",
        "export T snapshot<T>(T value){T saved=value;return saved;}",
        "export struct Point{int x;int y;}",
    ];
    let graph = crate::module::ModuleSet {
        modules: sources
            .iter()
            .enumerate()
            .map(|(id, source)| crate::module::ModuleSource {
                path: format!("/generic-instance-{id}.lil").into(),
                source: (*source).into(),
                dependencies: if id == 0 { vec![2, 1] } else { Vec::new() },
                foreign_dependencies: Vec::new(),
                dynamic_dependencies: Vec::new(),
                offset: 0,
            })
            .collect(),
        dependency_order: vec![2, 1, 0],
        root: 0,
        eager: vec![true; 3],
    };
    let arena = bumpalo::Bump::new();
    let syntax: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let semantics = crate::semantic::analyze_modules(&syntax, &graph).unwrap();
    let program = from_checked_modules(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    let generic: Vec<_> = program
        .units
        .iter()
        .filter(|unit| {
            unit.data()
                .callable_type
                .is_some_and(|ty| matches!(program.types[ty.index()], Type::GenericFunction(_)))
        })
        .collect();
    assert_eq!(generic.len(), 1);
    assert_eq!(generic[0].data().module.index(), 1);
    assert!(program.modules[1].source.same(syntax[1].source_identity()));
    let root = program.unit(program.initialization[0]).unwrap();
    // Initialization order is dependency order; locate the actual original root.
    let root = if root.module.index() == 0 {
        root
    } else {
        program
            .units
            .iter()
            .find(|unit| {
                unit.data().kind == UnitKind::ModuleInitialization
                    && unit.data().module.index() == 0
            })
            .unwrap()
            .data()
    };
    assert_eq!(root.call_instantiations.len(), 2);
    let point = program.structs()[0].identity;
    let mut seen_point = false;
    let mut seen_int = false;
    for op in &root.operations {
        let OperationKind::Call(id) = op.kind else {
            continue;
        };
        let call = &root.calls[id.index()];
        let Some(instance) = call.contract.instantiation else {
            continue;
        };
        let instance = &root.call_instantiations[instance.index()];
        assert_eq!(
            instance.declaration,
            generic[0].data().callable_type.unwrap()
        );
        assert_eq!(instance.arguments.len(), 1);
        let original = semantics
            .view(0)
            .unwrap()
            .call_instantiation(op.origin.unwrap())
            .unwrap();
        assert_eq!(
            program.types[instance.arguments[0].index()],
            original.type_arguments[0]
        );
        assert_eq!(
            program.types[instance.signature.index()],
            Type::Function(original.signature.clone())
        );
        match program.types[instance.arguments[0].index()] {
            Type::Struct(declaration) => {
                assert_eq!(declaration.identity, point);
                seen_point = true;
            }
            Type::Int => seen_int = true,
            _ => panic!("unexpected retained argument"),
        }
    }
    assert!(seen_point && seen_int);
}

#[test]
fn normalized_union_call_retains_the_decision_that_an_effective_signature_cannot_invert() {
    checked(
        "T|int unite<T>(T value){return value;}int result=unite(3);",
        |program| {
            let instance = program
                .units
                .iter()
                .flat_map(|unit| &unit.data().call_instantiations)
                .next()
                .unwrap();
            assert_eq!(instance.arguments.len(), 1);
            assert_eq!(program.types[instance.arguments[0].index()], Type::Int);
            let Type::Function(effective) = &program.types[instance.signature.index()] else {
                panic!("effective")
            };
            assert_eq!(*effective.return_type, Type::Int);
            let Type::GenericFunction(declaration) = &program.types[instance.declaration.index()]
            else {
                panic!("declaration")
            };
            assert!(matches!(*declaration.signature.return_type, Type::Union(_)));
        },
    );
}

#[test]
fn mutated_instance_arguments_signatures_and_ownership_fail_common_verification() {
    checked("T relay<T>(T value){return value;}int a=relay(3);string b=relay(\"s\");int plain(int n){return n;}int c=plain(4);", |program| {
        let unit = program.units.iter().find(|unit| unit.data().call_instantiations.len() == 2).unwrap().id();
        for case in 0..5 {
            let mut changed = program.clone();
            let mut working = changed.units[unit.index()].clone().into_working();
            let data = working.get_mut();
            match case {
                0 => { data.call_instantiations[0].arguments[0] = data.call_instantiations[1].arguments[0]; }
                1 => { data.call_instantiations[0].signature = data.call_instantiations[0].arguments[0]; }
                2 => { data.call_instantiations[0].arguments.clear(); }
                3 => { data.calls.iter_mut().find(|call| call.contract.instantiation.is_some()).unwrap().contract.instantiation = None; }
                4 => {
                    let id = data.calls.iter().find_map(|call| call.contract.instantiation).unwrap();
                    data.calls.iter_mut().find(|call| call.contract.instantiation.is_none()).unwrap().contract.instantiation = Some(id);
                }
                _ => unreachable!(),
            }
            changed.units[unit.index()] = working.freeze();
            assert!(changed.verify().is_err(), "instance mutation {case}");
        }
        program.verify().unwrap();
    });
}

fn edit(
    compiler: &mut Compilation<'_>,
    source: SemanticId,
    unit: UnitId,
    operation: OpId,
    value: &OperationKind,
    advance: bool,
) -> Result<SemanticId, PublicationError> {
    let revision = compiler.view(source)?.unit_revision(unit).unwrap();
    let patches = [UnitPatch {
        unit,
        expected_revision: revision,
        operations: &[OperationPatch {
            operation,
            kind: value,
            operands: &[],
        }],
        places: &[],
    }];
    if advance {
        compiler.advance_source(source, &patches, WorkDomain::Baseline)
    } else {
        compiler.edit_source(source, &patches, WorkDomain::Baseline)
    }
}
fn argument_storage(compiler: &mut Compilation<'_>, source: SemanticId, unit: UnitId) -> usize {
    compiler
        .with_semantic(source, |program, uses, _| {
            assert!(uses.valid_for(program));
            let data = program.unit(unit).unwrap();
            assert_eq!(data.call_instantiations.len(), 1);
            data.call_instantiations[0].arguments.as_ptr() as usize
        })
        .unwrap()
}

#[test]
fn sparse_instance_payload_follows_retained_fork_exclusive_retry_and_both_discard_orders() {
    for reverse in [false, true] {
        checked(
            "T relay<T>(T value){return value;}int run(){return relay(11);}",
            |program| {
                let unit = program
                    .units
                    .iter()
                    .find(|unit| !unit.data().call_instantiations.is_empty())
                    .unwrap()
                    .id();
                let operation = OpId::from_index(
                    program
                        .unit(unit)
                        .unwrap()
                        .operations
                        .iter()
                        .position(|op| {
                            matches!(op.kind, OperationKind::Constant(Constant::Integer(11)))
                        })
                        .unwrap(),
                )
                .unwrap();
                let mut compiler =
                    Compilation::new(ledger(), CheckpointLimit { max_live: 4 }).unwrap();
                let base = compiler
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                let baseline = argument_storage(&mut compiler, base, unit);
                let next = edit(
                    &mut compiler,
                    base,
                    unit,
                    operation,
                    &OperationKind::Constant(Constant::Integer(12)),
                    false,
                )
                .unwrap();
                let copied = argument_storage(&mut compiler, next, unit);
                assert_ne!(
                    baseline, copied,
                    "fork owns separately charged type-argument buffer"
                );
                let result = edit(
                    &mut compiler,
                    next,
                    unit,
                    operation,
                    &OperationKind::Constant(Constant::Null),
                    true,
                );
                assert!(matches!(result, Err(PublicationError::InvalidReplacement)));
                assert_eq!(argument_storage(&mut compiler, next, unit), copied);
                let next = edit(
                    &mut compiler,
                    next,
                    unit,
                    operation,
                    &OperationKind::Constant(Constant::Integer(13)),
                    true,
                )
                .unwrap();
                assert_eq!(
                    argument_storage(&mut compiler, next, unit),
                    copied,
                    "exclusive edit keeps immutable call-instance payload"
                );
                assert_eq!(argument_storage(&mut compiler, base, unit), baseline);
                if reverse {
                    compiler.discard(next).unwrap();
                    compiler.discard(base).unwrap();
                } else {
                    compiler.discard(base).unwrap();
                    compiler.discard(next).unwrap();
                }
                assert_eq!(compiler.finish().retained_bytes(), 0);
            },
        );
    }
}
