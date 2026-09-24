use super::*;

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let facts = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &facts).unwrap();
    program.verify().unwrap();
    inspect(&program);
}

fn edit(program: &Program<'_>, unit: usize, change: impl FnOnce(&mut UnitData)) -> String {
    let mut broken = program.clone();
    let mut working = broken.units[unit].clone().into_working();
    change(working.get_mut());
    broken.units[unit] = working.freeze();
    let error = broken
        .verify()
        .expect_err("the edited program must fail verification");
    program
        .verify()
        .expect("the retained checkpoint remains valid");
    error
}

#[test]
fn malformed_primitive_types_and_eager_short_circuit_are_rejected() {
    checked("int value=1+2;bool flag=true;print(value);", |program| {
        let boolean = TypeId::from_index(
            program
                .types
                .iter()
                .position(|ty| *ty == Type::Bool)
                .unwrap(),
        )
        .unwrap();
        let error = edit(program, 0, |unit| {
            let result = unit
                .operations
                .iter()
                .find(|op| matches!(op.kind, OperationKind::IntBinary(_)))
                .unwrap()
                .result
                .unwrap();
            unit.values[result.index()].ty = boolean;
        });
        assert!(error.contains("type mismatch"), "{error}");
        let error = edit(program, 0, |unit| {
            unit.operations
                .iter_mut()
                .find(|op| matches!(op.kind, OperationKind::IntBinary(_)))
                .unwrap()
                .kind = OperationKind::Binary(BinaryOp::And);
        });
        assert!(error.contains("type mismatch"), "{error}");
    });
}

#[test]
fn conditional_values_require_typed_normal_results() {
    checked(
        "bool flag=true;int value=if(flag){1}else{2};print(value);",
        |program| {
            let error = edit(program, 0, |unit| {
                let yes = unit
                    .operations
                    .iter()
                    .find_map(|op| {
                        if let OperationKind::Select { yes, .. } = op.kind {
                            Some(yes)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                unit.regions[yes.index()].result = None;
            });
            assert!(error.contains("no normal result"), "{error}");
            let error = edit(program, 0, |unit| {
                let (yes, condition) = unit
                    .operations
                    .iter()
                    .find_map(|op| {
                        if let OperationKind::Select { yes, .. } = op.kind {
                            Some((yes, unit.operands(op.operands).unwrap()[0]))
                        } else {
                            None
                        }
                    })
                    .unwrap();
                unit.regions[yes.index()].result = Some(condition);
            });
            assert!(error.contains("type mismatch"), "{error}");
        },
    );
    checked("bool value=true&&false;print(value);", |program| {
        let error = edit(program, 0, |unit| {
            let right = unit
                .operations
                .iter()
                .find_map(|op| {
                    if let OperationKind::ShortCircuit { right, .. } = op.kind {
                        Some(right)
                    } else {
                        None
                    }
                })
                .unwrap();
            unit.regions[right.index()].result = None;
        });
        assert!(error.contains("no normal result"), "{error}");
    });
}

#[test]
fn calls_must_close_the_innermost_prepared_reference() {
    checked(
        "int identity(int value){return value;}print(identity(1));",
        |program| {
            let error = edit(program, 0, |unit| {
                let calls: Vec<_> = unit
                    .operations
                    .iter()
                    .enumerate()
                    .filter_map(|(index, op)| {
                        if let OperationKind::Call(call) = op.kind {
                            Some((index, call))
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(calls.len(), 2);
                unit.operations[calls[0].0].kind = OperationKind::Call(calls[1].1);
                unit.operations[calls[1].0].kind = OperationKind::Call(calls[0].1);
            });
            assert!(error.contains("properly nested"), "{error}");
        },
    );
}

#[test]
fn callable_creation_names_and_return_signatures_are_verified() {
    checked(
        "int identity(int value){return value;}bool flag=true;print(identity(1));",
        |program| {
            let index = program
                .units
                .iter()
                .position(|unit| unit.data().kind == UnitKind::Function)
                .unwrap();
            let error = edit(program, index, |unit| unit.function_name = None);
            assert!(error.contains("creation name"), "{error}");
            let boolean = TypeId::from_index(
                program
                    .types
                    .iter()
                    .position(|ty| *ty == Type::Bool)
                    .unwrap(),
            )
            .unwrap();
            let error = edit(program, index, |unit| {
                let returned = unit
                    .operations
                    .iter()
                    .find(|op| matches!(op.kind, OperationKind::Return))
                    .unwrap();
                let value = unit.operands(returned.operands).unwrap()[0];
                unit.values[value.index()].ty = boolean;
            });
            assert!(error.contains("type mismatch"), "{error}");
        },
    );
}

#[test]
fn changed_callee_checks_its_own_returns_without_rescanning_unrelated_bodies() {
    let mut source = String::from("int edited(){bool flag=true;return 1;}");
    for index in 0..80 {
        source.push_str(&format!("int unrelated{index}(){{return {index};}}"));
    }
    checked(&source, |program| {
        let id = program
            .units
            .iter()
            .find(|unit| unit.data().kind == UnitKind::Function)
            .unwrap()
            .id();
        let mut changed = program.clone();
        let mut unit = changed.units[id.index()].clone().into_working();
        for operation in &mut unit.get_mut().operations {
            if matches!(
                operation.kind,
                OperationKind::Constant(Constant::Integer(1))
            ) {
                operation.kind = OperationKind::Constant(Constant::Integer(9));
            }
        }
        changed.units[id.index()] = unit.freeze();
        let receipt = super::verify::verify_replacements(&changed, &[id]).unwrap();
        assert_eq!(receipt.units, 1);
        assert_eq!(
            receipt.operations,
            changed.unit(id).unwrap().operations.len()
        );
        changed.verify().unwrap();
        assert_eq!(changed.units[0].revision(), program.units[0].revision());

        // Change a return to another locally available, well-typed value. The
        // return contract itself must reject this, even though its creator's
        // unit is unchanged and is not in the verification batch.
        let mut unit = changed.units[id.index()].clone().into_working();
        let data = unit.get_mut();
        let boolean = data
            .operations
            .iter()
            .find_map(|op| {
                matches!(op.kind, OperationKind::Constant(Constant::Boolean(true)))
                    .then_some(op.result)
                    .flatten()
            })
            .unwrap();
        let returned = data
            .operations
            .iter()
            .find(|op| matches!(op.kind, OperationKind::Return))
            .unwrap();
        data.operands[returned.operands.start as usize] = boolean;
        changed.units[id.index()] = unit.freeze();
        assert!(super::verify::verify_replacements(&changed, &[id])
            .unwrap_err()
            .contains("type mismatch"));
        program.verify().unwrap();
    });
}

#[test]
fn complete_nominal_schema_is_owned_even_without_field_reads() {
    checked(
        "struct Point{int x;int y;}export int answer(){return 42;}",
        |program| {
            assert_eq!(program.structs.len(), 1);
            assert_eq!(
                program
                    .fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>(),
                ["x", "y"]
            );
            let mut broken = program.clone();
            std::sync::Arc::make_mut(&mut broken.structs)[0].fields = 1..2;
            assert!(broken.verify().is_err());
            let mut broken = program.clone();
            std::sync::Arc::make_mut(&mut broken.fields)[1].index = 0;
            assert!(broken.verify().is_err());
        },
    );
}

#[test]
fn omitted_foreign_arguments_stay_omitted_for_the_host() {
    // A host function applies its own default: the call passes nothing, so
    // the host observes the same `arguments.length` as from JavaScript.
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(
        &arena,
        "extern int external(int value=7);print(external());",
    )
    .unwrap();
    let facts = crate::analyze(&source).unwrap();
    let program = from_checked_source(&source, &facts).unwrap();
    program.verify().unwrap();
    let javascript = program
        .to_javascript()
        .unwrap()
        .render(crate::js::PrintPolicy::default())
        .unwrap();
    assert!(javascript.contains("a()"), "{javascript}");
    assert!(!javascript.contains("(7)"), "{javascript}");
}
