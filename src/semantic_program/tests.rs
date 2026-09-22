use super::*;

fn checked<T>(source: &str, inspect: impl FnOnce(&Program<'_>) -> T) -> T {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    inspect(&program)
}

#[test]
fn retained_programs_share_tables_and_isolate_changed_unit_arenas() {
    checked(
        r#"
        struct Pair{int number;string label;}
        enum Color{Red,Blue}
        export int changed(){return 1;}
        export string stable(){return "retained";}
        Pair pair=Pair{2,"payload"};print(Color.Blue);
    "#,
        |program| {
            // Exercise nonempty tables, including nested schema and string
            // payloads, so equal empty-vector sentinel pointers cannot hide a
            // deep copy when retaining a candidate.
            assert!(!program.cells.is_empty());
            assert!(!program.types.is_empty());
            assert!(!program.strings.is_empty());
            assert!(!program.structs.is_empty());
            assert!(!program.enums.is_empty());
            assert!(!program.fields.is_empty());
            assert!(!program.exports.is_empty());
            assert!(!program.initialization.is_empty());

            let mut branch = program.clone();
            let changed = program
                .cells
                .iter()
                .find_map(|cell| match cell.binding {
                    CellBinding::Function(unit) if cell.name == "changed" => Some(unit.index()),
                    _ => None,
                })
                .unwrap();
            let mut working = branch.units[changed].clone().into_working();
            let operation = working
                .get_mut()
                .operations
                .iter_mut()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Constant(Constant::Integer(1))
                    )
                })
                .unwrap();
            // A source-like edit tests storage isolation; changing a constant
            // is not claimed to be an equivalent optimization.
            operation.kind = OperationKind::Constant(Constant::Integer(7));
            branch.units[changed] = working.freeze();
            branch.verify().unwrap();
            program.verify().unwrap();

            assert_eq!(branch.tables_revision, program.tables_revision);
            assert!(Arc::ptr_eq(&branch.cells, &program.cells));
            assert!(Arc::ptr_eq(&branch.types, &program.types));
            assert!(Arc::ptr_eq(&branch.strings, &program.strings));
            assert!(Arc::ptr_eq(&branch.structs, &program.structs));
            assert!(Arc::ptr_eq(&branch.enums, &program.enums));
            assert!(Arc::ptr_eq(&branch.fields, &program.fields));
            assert!(Arc::ptr_eq(&branch.exports, &program.exports));
            assert!(Arc::ptr_eq(&branch.initialization, &program.initialization));
            for (index, (original, retained)) in program.units.iter().zip(&branch.units).enumerate()
            {
                if index == changed {
                    assert_ne!(original.revision(), retained.revision());
                    assert!(!std::ptr::eq(original.data(), retained.data()));
                    assert!(original.data().operations.iter().any(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::Constant(Constant::Integer(1))
                        )
                    }));
                    assert!(retained.data().operations.iter().any(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::Constant(Constant::Integer(7))
                        )
                    }));
                } else {
                    assert_eq!(original.revision(), retained.revision());
                    assert!(std::ptr::eq(original.data(), retained.data()));
                }
            }
        },
    );
}

#[test]
fn callable_units_own_checked_signatures_including_generic_declarations() {
    checked(
        "T identity<T>(T value){return value;}int twice(int n){return n*2;}auto callback=(int n)=>n+1;",
        |program| {
            let root = program.unit(program.initialization[0]).unwrap();
            assert_eq!(root.callable_type, None);
            let mut generic = false;
            let mut closure = false;
            for cell in program.cells.iter() {
                if let CellBinding::Function(unit) = cell.binding {
                    let signature = program.unit(unit).unwrap().callable_type.unwrap();
                    assert_eq!(signature, cell.ty);
                    generic |= matches!(program.types[signature.index()], Type::GenericFunction(_));
                }
            }
            for unit in program.units.iter() {
                for operation in &unit.data().operations {
                    if let OperationKind::Closure(child) = operation.kind {
                        let child = program.unit(child).unwrap();
                        assert_eq!(
                            child.callable_type,
                            Some(unit.data().values[operation.result.unwrap().index()].ty)
                        );
                        closure |= child.kind == UnitKind::Closure;
                    }
                }
            }
            assert!(generic && closure);
        },
    );
}

#[test]
fn cells_captures_and_instance_owners_survive_checked_conversion() {
    checked("func()->int factory(int start){int count=start;auto step=()=>{count+=1;return count;};return step;}auto first=factory(2);auto second=factory(9);print(first());print(second());", |program| {
        let count = program.cells.iter().position(|cell| cell.name == "count").unwrap();
        let cell = CellId::from_index(count).unwrap();
        let owner = program.cells[count].owner;
        assert_ne!(owner, program.initialization[0]);
        let closure = program.units.iter().find(|unit| unit.data().kind == UnitKind::Closure).unwrap();
        assert!(closure.data().captures.contains(&cell));
        assert_ne!(closure.id(), owner);
        assert!(closure.data().operations.iter().any(|op| matches!(op.kind, OperationKind::Store(_))));
    });
}

#[test]
fn places_capture_receiver_and_old_value_before_rhs_effects() {
    checked(
        "int[] values=[10];int rhs(){values=[90];return 2;}print(values[0]+=rhs());",
        |program| {
            let root = program.unit(program.initialization[0]).unwrap();
            let (store_index, place) = root
                .operations
                .iter()
                .enumerate()
                .find_map(|(index, op)| {
                    if let OperationKind::Store(place) = op.kind {
                        Some((index, place))
                    } else {
                        None
                    }
                })
                .unwrap();
            let Place::Index { receiver, .. } = root.places[place.index()] else {
                panic!("indexed place")
            };
            let receiver_index = root.values[receiver.index()].definition.index();
            let rhs_index = root
                .operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::Call(_)))
                .unwrap();
            let old_index = root
                .operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::Load(id) if id == place))
                .unwrap();
            assert!(receiver_index < old_index && old_index < rhs_index && rhs_index < store_index);
        },
    );
}

#[test]
fn known_string_computation_and_utf16_payload_remain_separate() {
    checked(r#"string value="\ud800"+"x";print(value);"#, |program| {
        let root = program.unit(program.initialization[0]).unwrap();
        assert_eq!(program.strings.len(), 2);
        assert!(program
            .strings
            .iter()
            .any(|value| value.code_units().collect::<Vec<_>>() == [0xd800]));
        assert!(root
            .operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::Binary(BinaryOp::Add))));
    });
}

#[test]
fn cleanup_and_loop_completions_have_explicit_owners() {
    checked("int work(int n){int sum=0;while(n>0){try{n-=1;if(n==2){continue;}sum+=n;}finally{sum+=1;}}return sum;}print(work(4));", |program| {
        let function = program.units.iter().find(|unit| unit.data().kind == UnitKind::Function).unwrap().data();
        assert!(function.operations.iter().any(|op| matches!(op.kind, OperationKind::Try { finally: Some(_), .. })));
        assert!(function.operations.iter().any(|op| matches!(op.kind, OperationKind::Continue)));
        assert!(function.operations.iter().any(|op| matches!(op.kind, OperationKind::Loop { .. })));
    });
}

#[test]
fn verifier_rejects_stale_captures_and_dangling_values() {
    checked(
        "func()->int factory(int x){return ()=>x;}auto read=factory(3);print(read());",
        |program| {
            let mut broken = program.clone();
            let index = broken
                .units
                .iter()
                .position(|unit| unit.data().kind == UnitKind::Closure)
                .unwrap();
            let mut edit = broken.units[index].clone().into_working();
            edit.get_mut().captures.clear();
            broken.units[index] = edit.freeze();
            assert!(broken.verify().unwrap_err().contains("capture"));
            let mut broken = program.clone();
            let index = broken.initialization[0].index();
            let mut edit = broken.units[index].clone().into_working();
            edit.get_mut().operands[0] = ValueId::from_index(99999).unwrap();
            broken.units[index] = edit.freeze();
            assert!(broken.verify().is_err());
            program.verify().unwrap();
        },
    );
}

#[test]
fn source_identity_mismatch_is_rejected_before_conversion() {
    let arena = bumpalo::Bump::new();
    let one = crate::parse_source(&arena, "print(1);").unwrap();
    let two = crate::parse_source(&arena, "print(1);").unwrap();
    let facts = crate::analyze(&one).unwrap();
    assert_eq!(
        from_checked_source(&two, &facts).unwrap_err().feature,
        "checker/source ownership mismatch"
    );
}

#[test]
fn exported_struct_schema_survives_without_local_field_reads() {
    checked(
        "struct Point{int x;int y;}struct Empty{}export Point make(){return Point{1,2};}",
        |program| {
            assert_eq!(program.structs.len(), 2);
            let point = &program.structs[0];
            assert_eq!(point.name, "Point");
            let fields = &program.fields[point.fields.clone()];
            assert_eq!(
                fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>(),
                ["x", "y"]
            );
            assert!(fields.iter().all(|field| field.owner == point.identity));
            assert!(program.structs[1].fields.is_empty());
            assert_eq!(program.exports[0].name, "make");
            assert!(program.units.iter().all(|unit| unit
                .data()
                .places
                .iter()
                .all(|place| !matches!(place, Place::Field { .. }))));
        },
    );
}

#[test]
fn enum_variants_are_values_without_a_runtime_namespace_load() {
    checked("enum Color{Red,Blue}print(Color.Blue);", |program| {
        let root = program.unit(program.initialization[0]).unwrap();
        assert!(root
            .operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(1)))));
        assert!(root.places.is_empty());
        assert_eq!(program.enums[0].name, "Color");
        assert_eq!(
            program.enums[0]
                .variants
                .iter()
                .map(|variant| (variant.name.as_str(), variant.value))
                .collect::<Vec<_>>(),
            [("Red", 0), ("Blue", 1)]
        );
        let mut broken = program.clone();
        let mut changed = broken.units[0].clone().into_working();
        let literal = changed
            .get_mut()
            .operations
            .iter_mut()
            .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(1))))
            .unwrap();
        literal.kind = OperationKind::Constant(Constant::Integer(42));
        broken.units[0] = changed.freeze();
        assert!(broken.verify().is_err());
    });
}
