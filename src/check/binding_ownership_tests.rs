use super::*;
use std::sync::Arc;

fn canonical_binding<'model, 'ast, 'src>(
    model: &'model CheckedModule<'ast, 'src>,
    node: SourceNodeId,
) -> &'model Type<'src> {
    let id = model.identifier_symbol(node).unwrap();
    let canonical = &model.declarations.symbols[id.0 as usize].ty;
    assert!(matches!(
        model.facts.binding_types.get(&node),
        Some(BindingType::Symbol(symbol)) if *symbol == id
    ));
    assert!(std::ptr::eq(model.binding_type(node).unwrap(), canonical));
    canonical
}

#[test]
fn source_bindings_borrow_canonical_nested_types_but_identifier_uses_do_not() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        "int[][] matrix=[[1,2],[3]];extern void consume(int[][] detached);int[][] echo(int[][] value){return value;}int[][] result=echo(matrix);",
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    for symbol in model.symbols() {
        canonical_binding(&model, symbol.node);
    }
    for name in ["matrix", "detached", "value", "result"] {
        let symbol = model
            .symbols()
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap();
        let Type::Array(outer) = canonical_binding(&model, symbol.node) else {
            panic!("{name} must retain its nested array type");
        };
        let Type::Array(inner) = &**outer else {
            panic!("{name} must retain its inner array type");
        };
        assert_eq!(**inner, Type::Int);
        let Type::Array(canonical_outer) = &symbol.ty else {
            unreachable!();
        };
        assert!(std::ptr::eq(&**outer, &**canonical_outer));
    }
    let mut uses = 0;
    for info in &model.facts.source_info {
        let Some(Expr {
            kind: ExprKind::Ident(ident),
            ..
        }) = info.expression
        else {
            continue;
        };
        assert!(model.identifier_symbol(ident.id).is_some());
        assert!(model.binding_type(ident.id).is_none());
        uses += 1;
    }
    assert!(uses >= 3);
    assert!(model.identifier_index_is_consistent());
}

#[test]
fn declare_and_detached_binding_move_existing_nested_payloads_without_cloning() {
    fn payload() -> Type<'static> {
        Type::Map(
            Box::new(Type::String),
            Box::new(Type::Array(Box::new(Type::Union(vec![
                Type::Int,
                Type::Float,
            ])))),
        )
    }
    fn addresses<'src>(
        ty: &Type<'src>,
    ) -> (
        *const Type<'src>,
        *const Type<'src>,
        *const Type<'src>,
        *const Type<'src>,
    ) {
        let Type::Map(key, value) = ty else {
            unreachable!()
        };
        let Type::Array(element) = &**value else {
            unreachable!()
        };
        let Type::Union(members) = &**element else {
            unreachable!()
        };
        (&**key, &**value, &**element, members.as_ptr())
    }
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "int first=1;int second=2;").unwrap();
    let Item::Stmt(Stmt::VarDecl(first)) = &program.items[0] else {
        unreachable!()
    };
    let Item::Stmt(Stmt::VarDecl(second)) = &program.items[1] else {
        unreachable!()
    };
    let mut facts = ModuleFacts::new(program.source_identity());
    let mut declarations = DeclarationTables::default();
    let mut initialization = ModuleInitialization::default();
    let declared = payload();
    let detached = payload();
    let expected_declared = addresses(&declared);
    let expected_detached = addresses(&detached);
    let mut budget = AllocationBudget::new(None);
    let (declared_id, detached_id) = {
        let mut analyzer = Analyzer::new(
            &mut facts,
            &mut declarations,
            &mut initialization,
            None,
            &mut budget,
        )
        .unwrap();
        (
            analyzer.declare(first.name, declared).unwrap(),
            analyzer.record_detached(second.name, detached).unwrap(),
        )
    };
    assert_ne!(declared_id, detached_id);
    assert_eq!(
        addresses(&declarations.symbols[declared_id.0 as usize].ty),
        expected_declared
    );
    assert_eq!(
        addresses(&declarations.symbols[detached_id.0 as usize].ty),
        expected_detached
    );
    let view = CheckedView {
        declarations: &declarations,
        facts: &facts,
    };
    for (node, id) in [(first.name.id, declared_id), (second.name.id, detached_id)] {
        assert!(std::ptr::eq(
            view.binding_type(node).unwrap(),
            &declarations.symbols[id.0 as usize].ty
        ));
    }
    assert!(declarations.identifier_index_is_consistent(std::slice::from_ref(&facts)));
}

#[test]
fn nominal_only_binding_nodes_preserve_inline_types_without_value_symbols() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        "struct Point{int x;}Point value=Point{7};export {Point as PublicPoint};",
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    let Item::Struct(declaration) = &program.items[0] else {
        unreachable!()
    };
    let nominal = model.view().struct_type("Point").unwrap();
    for node in [declaration.name.id, program.exports[0].local.id] {
        assert!(model.identifier_symbol(node).is_none());
        assert_eq!(model.binding_type(node), Some(&Type::Struct(nominal)));
        assert!(
            matches!(model.facts.binding_types.get(&node), Some(BindingType::Inline(Type::Struct(value))) if *value == nominal)
        );
    }
    let value = model
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "value")
        .unwrap();
    assert_eq!(
        canonical_binding(&model, value.node),
        &Type::Struct(nominal)
    );
    assert_eq!(
        model.view().export_target(program.exports[0].local.id),
        Some(InterfaceTarget::Type(nominal.identity))
    );
}

#[test]
fn finalized_defaults_and_model_clones_resolve_bindings_against_their_own_symbols() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        "int seed=7;int choose(int value=seed){return value;}auto alias=choose;int result=alias();",
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    let seed = model
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "seed")
        .unwrap()
        .id;
    let clone = model.clone();
    for name in ["choose", "alias"] {
        let symbol = model
            .symbols()
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap();
        let Type::Function(original) = canonical_binding(&model, symbol.node) else {
            unreachable!()
        };
        let Type::Function(copied) = canonical_binding(&clone, symbol.node) else {
            unreachable!()
        };
        assert_eq!(original.params[0].default, Some(DefaultValue::Symbol(seed)));
        assert_eq!(copied.params[0].default, Some(DefaultValue::Symbol(seed)));
        assert!(!signature_has_pending_bindings(original));
        assert!(!signature_has_pending_bindings(copied));
        assert!(!std::ptr::eq(
            model.binding_type(symbol.node).unwrap(),
            clone.binding_type(symbol.node).unwrap()
        ));
        assert!(Arc::ptr_eq(&original.0, &copied.0));
    }
    let expected: Vec<_> = model
        .symbols()
        .iter()
        .map(|symbol| (symbol.node, symbol.ty.clone()))
        .collect();
    drop(model);
    for (node, ty) in expected {
        assert_eq!(canonical_binding(&clone, node), &ty);
    }
    assert!(clone.identifier_index_is_consistent());
}

#[test]
fn canonical_binding_entry_does_not_enlarge_the_existing_type_map_payload() {
    assert!(
        std::mem::size_of::<BindingType<'_>>() <= std::mem::size_of::<Type<'_>>(),
        "binding entry {} bytes exceeds previous Type payload {} bytes",
        std::mem::size_of::<BindingType<'_>>(),
        std::mem::size_of::<Type<'_>>()
    );
    assert!(
        std::mem::size_of::<(Span, BindingType<'_>)>() <= std::mem::size_of::<(Span, Type<'_>)>(),
        "binding row {} bytes exceeds previous type row {} bytes",
        std::mem::size_of::<(Span, BindingType<'_>)>(),
        std::mem::size_of::<(Span, Type<'_>)>()
    );
}
