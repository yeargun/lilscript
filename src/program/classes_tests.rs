use super::*;

fn inspect(source: &str, check: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    check(&from_checked_source(&syntax, &semantics).unwrap());
}

#[test]
fn classes_follow_closed_producers_and_revision_owners() {
    inspect(
        "int pass(int value){return value;}int seed=2;print(pass(seed));",
        |program| {
            let closed = program.primitive_classes(Seal::Module);
            let open = program.primitive_classes(Seal::StructuralOnly);
            for (index, cell) in program.cells.iter().enumerate() {
                let id = CellId::from_index(index).unwrap();
                if cell.name == "seed" || cell.name == "value" {
                    assert!(closed.cell(id).primitive(), "{}", cell.name);
                    assert!(!open.cell(id).primitive(), "{}", cell.name);
                }
            }
            assert!(std::sync::Arc::ptr_eq(
                &closed,
                &program.primitive_classes(Seal::Module)
            ));
            let copy = program.clone();
            assert!(!std::sync::Arc::ptr_eq(
                &closed,
                &copy.primitive_classes(Seal::Module)
            ));
        },
    );
    inspect(
        "extern int host;int pass(int value){return value;}print(pass(host));",
        |program| {
            let classes = program.primitive_classes(Seal::Module);
            for (index, cell) in program.cells.iter().enumerate() {
                if cell.name == "host" || cell.name == "value" {
                    assert!(!classes.cell(CellId::from_index(index).unwrap()).primitive());
                }
            }
        },
    );
}
