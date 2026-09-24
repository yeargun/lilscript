//! Canonical schemas and interface kinds survive original-source lowering.
//! Mutated fixtures exercise the common verifier/admission, before targets.
use super::publication::{CheckpointLimit, Compilation, PublicationError};
use super::uses::{CellUseSite, UseIndex};
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use crate::check::{FunctionParameter, FunctionSignature, FunctionType, StructType};

fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    inspect(program);
}

fn modules(
    sources: &[&str],
    dependencies: &[&[usize]],
    order: &[usize],
    inspect: impl FnOnce(Program<'_>, &[crate::ast::SourceIdentity]),
) {
    let graph = crate::module::ModuleSet {
        modules: sources
            .iter()
            .zip(dependencies)
            .enumerate()
            .map(|(id, (source, deps))| crate::module::ModuleSource {
                path: format!("/nominal-core-{id}.lil").into(),
                source: (*source).into(),
                dependencies: deps.to_vec(),
                foreign_dependencies: Vec::new(),
                dynamic_dependencies: Vec::new(),
                offset: 0,
            })
            .collect(),
        dependency_order: order.to_vec(),
        root: 0,
        eager: vec![true; sources.len()],
    };
    let arena = bumpalo::Bump::new();
    let syntax: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let checked = crate::check::analyze_modules(&syntax, &graph).unwrap();
    let identities: Vec<_> = syntax
        .iter()
        .map(|source| source.source_identity().clone())
        .collect();
    let program = from_checked_modules(&syntax, &checked).unwrap();
    program.verify().unwrap();
    inspect(program, &identities);
}

fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 20_000_000,
            optional_work: 20_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 20_000_000,
        },
    )
    .unwrap()
}

const DIAMOND: [&str; 4] = [
    r#"import {P as Left} from "./left";import {P as Right} from "./right";Left a=Left{3};Right b=a;export {Left as PublicP};export int result=b.x;"#,
    r#"import {P} from "./shape";export {P};"#,
    r#"import {P} from "./shape";export {P};"#,
    "export struct P { int x; }",
];
const DIAMOND_DEPS: [&[usize]; 4] = [&[1, 2], &[3], &[3], &[]];

#[test]
fn diamond_type_aliases_keep_one_schema_original_owner_and_no_runtime_cell() {
    modules(
        &DIAMOND,
        &DIAMOND_DEPS,
        &[3, 1, 2, 0],
        |program, sources| {
            assert_eq!(program.structs().len(), 1);
            let definition = &program.structs()[0];
            assert_eq!(definition.name, "P");
            assert_eq!(definition.module.index(), 3);
            assert!(program.modules()[3].source.same(&sources[3]));
            assert_eq!(
                definition.span.start, 7,
                "span remains in original source coordinates"
            );
            assert_eq!(program.fields().len(), 1);
            assert_eq!(program.exports().len(), 2);
            assert_eq!(program.value_exports().count(), 1);
            let nominal = InterfaceTarget::Struct(definition.identity);
            for module in program.modules() {
                for import in &module.imports {
                    assert_eq!(import.target, nominal);
                }
            }
            assert_eq!(program.exports()[0].target, nominal);
            assert!(program
                .cells()
                .iter()
                .all(|cell| !matches!(cell.name.as_str(), "P" | "Left" | "Right" | "PublicP")));
            assert_eq!(
                program
                    .initialization()
                    .iter()
                    .map(|id| id.index())
                    .collect::<Vec<_>>(),
                [3, 1, 2, 0]
            );
            let (_, result) = program.value_exports().next().unwrap();
            let mut budget = ledger();
            let uses = UseIndex::build(&program, &mut budget, WorkDomain::Baseline).unwrap();
            assert!(
                uses.cell(result)
                    .unwrap()
                    .sites()
                    .contains(&CellUseSite::Export { index: 1 }),
                "type erasure preserves the original interface position"
            );
            uses.discard(&mut budget).unwrap();
            assert_eq!(budget.retained_bytes(), 0);
            let mut compilation =
                Compilation::new(budget, CheckpointLimit { max_live: 2 }).unwrap();
            let source = compilation
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            compilation.discard(source).unwrap();
            assert_eq!(compilation.finish().retained_bytes(), 0);
        },
    );
}

const PRIVATE: [&str; 3] = [
    r#"import {makeA} from "./a";import {makeB} from "./b";auto a=makeA();auto b=makeB();print(a.x);print(b.x);"#,
    "struct Pair { int x; } export Pair makeA(){return Pair{1};}",
    "struct Pair { int x; } export Pair makeB(){return Pair{2};}",
];

#[test]
fn equal_spelling_private_schemas_remain_incompatible_for_projection_and_construction() {
    modules(&PRIVATE, &[&[1, 2], &[], &[]], &[1, 2, 0], |program, _| {
        let [left, right] = program.structs() else {
            panic!("two original declarations")
        };
        assert_eq!(left.name, right.name);
        assert_ne!(left.identity, right.identity);
        assert_ne!(left.module, right.module);
        let left_ty = Type::Struct(StructType {
            identity: left.identity,
            name: "Pair",
        });
        let right_ty = Type::Struct(StructType {
            identity: right.identity,
            name: "Pair",
        });
        assert_ne!(left_ty, right_ty);
        assert_eq!(
            program
                .cells()
                .iter()
                .filter(|cell| cell.name == "a" || cell.name == "b")
                .count(),
            2
        );
        let root = program.modules()[0].initializer;
        let mut forged = program.clone();
        let mut working = forged.units[root.index()].clone().into_working();
        let place = working.get_mut().places.iter_mut().find(|place| matches!(place, Place::Field { field, .. } if *field == program.fields[left.fields.start].identity)).unwrap();
        let Place::Field { field, .. } = place else {
            unreachable!()
        };
        *field = program.fields[right.fields.start].identity;
        forged.units[root.index()] = working.freeze();
        assert!(forged
            .verify()
            .unwrap_err()
            .contains("incompatible nominal owner"));
        drop(forged);
        let (unit, op) = program.units().iter().find_map(|unit| unit.data().operations.iter().position(|op| matches!(op.kind, OperationKind::Allocate { kind: AllocationKind::Struct(id), .. } if id == left.identity)).map(|op| (unit.id(), op))).unwrap();
        let mut forged = program.clone();
        let mut working = forged.units[unit.index()].clone().into_working();
        let OperationKind::Allocate { kind, .. } = &mut working.get_mut().operations[op].kind
        else {
            unreachable!("selected struct allocation");
        };
        *kind = AllocationKind::Struct(right.identity);
        forged.units[unit.index()] = working.freeze();
        assert!(
            forged.verify().is_err(),
            "matching field count and spelling cannot substitute another nominal identity"
        );
        program.verify().unwrap();
    });
}

#[test]
fn unused_nested_nominal_contracts_reject_wrong_names_ids_and_arity_before_adoption() {
    for shape in 0..4 {
        checked(
            "struct A { int x; } struct B { int x; } print(1);",
            |mut program| {
                let a = program.structs()[0].identity;
                let b = program.structs()[1].identity;
                let malformed = match shape {
                    0 => Type::Struct(StructType {
                        identity: a,
                        name: "B",
                    }),
                    1 => {
                        Arc::make_mut(&mut program.structs).pop();
                        Type::Struct(StructType {
                            identity: b,
                            name: "B",
                        })
                    }
                    2 => Type::StructInstance {
                        declaration: StructType {
                            identity: a,
                            name: "A",
                        },
                        args: vec![Type::Int],
                    },
                    _ => Type::Struct(StructType {
                        identity: a,
                        name: "forged",
                    }),
                };
                let mut nested = Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(Type::Record(Box::new(malformed)))],
                    return_type: Box::new(Type::Void),
                }));
                if shape == 3 {
                    for _ in 0..256 {
                        nested = Type::Array(Box::new(nested));
                    }
                }
                Arc::make_mut(&mut program.types).push(nested);
                let reason = program.verify().unwrap_err();
                assert!(reason.contains("nominal type"), "{reason}");
                let mut compilation =
                    Compilation::new(ledger(), CheckpointLimit { max_live: 2 }).unwrap();
                let retained = compilation.ledger().retained_bytes();
                let work = compilation.ledger().work_used(WorkDomain::Baseline);
                let error = compilation
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap_err();
                assert!(
                    matches!(error, PublicationError::InvalidNominalContract(found) if found == reason),
                    "{error:?}"
                );
                assert_eq!(compilation.checkpoint_count(), 0);
                assert_eq!(compilation.ledger().retained_bytes(), retained);
                assert!(compilation.ledger().work_used(WorkDomain::Baseline) > work);
                assert_eq!(compilation.finish().retained_bytes(), 0);
            },
        );
    }
}

#[test]
fn nominal_module_interfaces_reject_kind_substitution_and_dangling_original_owner() {
    modules(&DIAMOND, &DIAMOND_DEPS, &[3, 1, 2, 0], |program, _| {
        let (_, cell) = program.value_exports().next().unwrap();
        let mut forged = program.clone();
        Arc::make_mut(&mut forged.modules)[0].imports[0].target = InterfaceTarget::Value(cell);
        assert!(forged.verify().unwrap_err().contains("canonical export"));
        drop(forged);
        let mut forged = program.clone();
        Arc::make_mut(&mut forged.structs)[0].module =
            ModuleId::from_index(program.modules().len()).unwrap();
        assert!(forged.verify().is_err());
        drop(forged);
        let mut forged = program.clone();
        Arc::make_mut(&mut forged.structs)[0].name = "alias-is-not-declaration".into();
        assert!(forged.verify().unwrap_err().contains("diagnostic name"));
        program.verify().unwrap();
    });
}

#[test]
fn single_source_export_occurrences_distinguish_type_declarations_from_same_named_values() {
    checked("struct Item { int x; } export int Item=7;", |program| {
        assert!(matches!(
            program.exports()[0].target,
            InterfaceTarget::Value(_)
        ));
        assert_eq!(program.value_exports().next().unwrap().0, "Item");
    });
    checked("export struct Item { int x; } int Item=7;", |program| {
        assert_eq!(
            program.exports()[0].target,
            InterfaceTarget::Struct(program.structs()[0].identity)
        );
        assert_eq!(program.value_exports().count(), 0);
    });
}

#[test]
fn type_only_interface_scan_is_admitted_and_work_refusal_releases_demand_scratch() {
    use super::demand::{DemandError, DemandMode, DemandPlan};
    use crate::compilation_policy::{BudgetError, CompilationRequest};

    const MANY: usize = 8_192;
    checked("export struct Item { int x; }", |mut program| {
        let policy = crate::config::ProjectConfig::default()
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let contract = policy.javascript_contract().unwrap();
        assert!(program.cells().is_empty());
        assert!(program
            .units()
            .iter()
            .all(|unit| unit.data().operations.is_empty()));
        let target = program.exports()[0].target;
        let mut small_cost = 0;
        for count in [1, MANY] {
            // Original interface construction is outside the measured Demand
            // phase. The graph and all operation/value/cell arenas stay empty.
            let exports = Arc::make_mut(&mut program.exports);
            exports.clear();
            exports.extend((0..count).map(|index| Export {
                name: format!("Type{index}"),
                target,
            }));
            Arc::make_mut(&mut program.modules)[0].exports = 0..count;
            program.tables_revision = RevisionId::fresh();
            program.verify().unwrap();
            assert_eq!(program.value_exports().count(), 0);

            let mut budget = ledger();
            let demand = DemandPlan::build(
                &program,
                None,
                None,
                contract,
                DemandMode::Prune,
                Some((&mut budget, WorkDomain::Optional)),
            )
            .unwrap();
            let complete_cost = budget.work_used(WorkDomain::Optional);
            assert!(
                budget.retained_bytes() > 0,
                "the complete plan owns scratch"
            );
            demand.discard(Some(&mut budget)).unwrap();
            assert_eq!(budget.retained_bytes(), 0);
            if count == 1 {
                small_cost = complete_cost;
                continue;
            }
            assert!(complete_cost >= MANY as u64);
            assert!(complete_cost > small_cost);

            // Exactly the allowance that completed the smaller empty program
            // cannot silently scan thousands of erased type-only exports.
            let mut limited = BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    baseline_work: 20_000_000,
                    optional_work: small_cost,
                    baseline_retained_bytes: 0,
                    retained_bytes: 20_000_000,
                },
            )
            .unwrap();
            let error = match DemandPlan::build(
                &program,
                None,
                None,
                contract,
                DemandMode::Prune,
                Some((&mut limited, WorkDomain::Optional)),
            ) {
                Ok(demand) => {
                    demand.discard(Some(&mut limited)).unwrap();
                    panic!("large type-only scan bypassed its work admission");
                }
                Err(error) => error,
            };
            assert!(matches!(
                error,
                DemandError::Budget(BudgetError::WorkExhausted(WorkDomain::Optional))
            ));
            assert!(limited.work_used(WorkDomain::Optional) > 0);
            assert!(limited.work_used(WorkDomain::Optional) <= small_cost);
            assert_eq!(limited.work_used(WorkDomain::Baseline), 0);
            assert_eq!(
                limited.retained_bytes(),
                0,
                "partial Demand ownership is released on refusal"
            );
        }
    });
}

#[test]
fn member_lookup_uses_canonical_order_and_rejects_reordered_schema_adoption() {
    modules(
        &[
            r#"import {A} from "./a"; import {B} from "./b"; A a=A{1,2}; B b=B{3}; print(a.y+b.z);"#,
            "export struct A { int x; int y; }",
            "export struct B { int z; }",
        ],
        &[&[1, 2], &[], &[]],
        &[1, 2, 0],
        |mut program, _| {
            assert_eq!(program.fields().len(), 3);
            for field in program.fields() {
                assert!(std::ptr::eq(program.field(field.identity).unwrap(), field));
            }
            assert!(program
                .fields()
                .windows(2)
                .all(|pair| pair[0].identity.index() < pair[1].identity.index()));
            // Leave each field's own schema slot untouched: reversing only
            // identity order tests the common lookup invariant explicitly.
            Arc::make_mut(&mut program.fields).swap(0, 1);
            assert!(program
                .verify()
                .unwrap_err()
                .contains("canonical member order"));
            let mut compilation =
                Compilation::new(ledger(), CheckpointLimit { max_live: 2 }).unwrap();
            let before = compilation.ledger().retained_bytes();
            let error = compilation
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap_err();
            assert!(
                matches!(error, PublicationError::InvalidNominalContract(reason) if reason.contains("canonical member order")),
                "{error:?}"
            );
            assert_eq!(compilation.checkpoint_count(), 0);
            assert_eq!(compilation.ledger().retained_bytes(), before);
            assert_eq!(compilation.finish().retained_bytes(), 0);
        },
    );
}
