use super::*;
use std::sync::Arc;

fn define_headers<'ast, 'src>(
    analyzer: &mut Analyzer<'_, '_, 'ast, 'src>,
    program: &Program<'ast, 'src>,
) {
    analyzer.declare_nominal_types(program).unwrap();
    analyzer.define_classes(program).unwrap();
    analyzer.define_extern_classes(program).unwrap();
}

fn array_payloads<'src>(ty: &Type<'src>) -> (*const Type<'src>, *const Type<'src>) {
    let Type::Array(outer) = ty else {
        panic!("expected outer array: {ty:?}");
    };
    let Type::Array(inner) = &**outer else {
        panic!("expected inner array: {ty:?}");
    };
    (&**outer, &**inner)
}

#[test]
fn hierarchy_resolution_moves_own_field_and_method_payloads_without_cloning() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        r#"
        class Child extends Base {
            int[][] own;
            T childMethod<T>(T value) { return value; }
        }
        class Base {
            int[][] inherited;
            T baseMethod<T>(T value) { return value; }
        }
        class Plain {
            int[][] plain;
            T plainMethod<T>(T value) { return value; }
        }
        extern class ForeignChild extends ForeignBase {
            int[][] own;
            T childMethod<T>(T value);
        }
        extern class ForeignBase {
            int[][] inherited;
            T baseMethod<T>(T value);
        }
        "#,
    )
    .unwrap();
    let mut facts = ModuleFacts::new(program.source_identity());
    let mut declarations = DeclarationTables::default();
    let mut initialization = ModuleInitialization::default();
    let mut budget = AllocationBudget::new(None);
    let mut analyzer = Analyzer::new(
        &mut facts,
        &mut declarations,
        &mut initialization,
        None,
        &mut budget,
    )
    .unwrap();
    define_headers(&mut analyzer, &program);
    let before = [
        ("Child", "own", "childMethod"),
        ("Base", "inherited", "baseMethod"),
        ("Plain", "plain", "plainMethod"),
        ("ForeignChild", "own", "childMethod"),
        ("ForeignBase", "inherited", "baseMethod"),
    ]
    .map(|(name, field_name, method_name)| {
        let info = &analyzer.declarations.classes[name];
        let field = &info.fields[field_name];
        let method = &info.methods[method_name];
        assert_eq!(method.type_params, ["T"]);
        (
            name,
            field_name,
            field.member,
            array_payloads(&field.ty),
            method_name,
            method.member,
            method.type_params.as_ptr(),
            Arc::as_ptr(&method.signature.0),
        )
    });
    analyzer.resolve_class_hierarchies().unwrap();
    for (name, field_name, field_id, payloads, method_name, method_id, params, signature) in before
    {
        let info = &analyzer.declarations.classes[name];
        let field = &info.fields[field_name];
        let method = &info.methods[method_name];
        assert_eq!(field.member, field_id);
        assert_eq!(array_payloads(&field.ty), payloads, "{name}.{field_name}");
        assert_eq!(method.member, method_id);
        assert_eq!(method.type_params.as_ptr(), params, "{name}.{method_name}");
        assert_eq!(Arc::as_ptr(&method.signature.0), signature);
        assert_eq!(method.owner, name);
    }
    for (child, base) in [("Child", "Base"), ("ForeignChild", "ForeignBase")] {
        let child = &analyzer.declarations.classes[child];
        let base = &analyzer.declarations.classes[base];
        assert_eq!(child.fields["own"].index, 1);
        assert_eq!(
            child.fields["inherited"].member,
            base.fields["inherited"].member
        );
        assert_eq!(
            child.methods["baseMethod"].member,
            base.methods["baseMethod"].member
        );
        assert_ne!(
            array_payloads(&child.fields["inherited"].ty),
            array_payloads(&base.fields["inherited"].ty),
            "instantiated inherited fields still need their own nested type payload"
        );
    }
}

#[test]
fn merged_object_members_keep_order_identity_and_shared_callable_payloads() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        "object Api{int[][] first(int[][] value){return value;}}object Api{string second(string value){return value;}}object Api{bool third(bool value){return value;}}",
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    let info = model.class_info("Api").unwrap();
    assert!(info.object);
    assert_eq!(
        info.fields.keys().copied().collect::<Vec<_>>(),
        ["first", "second", "third"]
    );
    assert_eq!(
        info.methods.keys().copied().collect::<Vec<_>>(),
        ["first", "second", "third"]
    );
    let owner = model.nominal_id(&Type::Class("Api")).unwrap();
    let mut identities = AHashSet::default();
    for (index, name) in ["first", "second", "third"].into_iter().enumerate() {
        let field = &info.fields[name];
        let method = &info.methods[name];
        let Type::Function(field_signature) = &field.ty else {
            panic!("object member must retain its callable field");
        };
        assert!(Arc::ptr_eq(&field_signature.0, &method.signature.0));
        assert_eq!(field.index, index);
        assert_eq!(method.owner, "Api");
        assert!(identities.insert(field.member));
        assert!(identities.insert(method.member));
        let field_definition = model.declarations.nominal_members[field.member.index()];
        let method_definition = model.declarations.nominal_members[method.member.index()];
        assert_eq!(field_definition.owner, owner);
        assert_eq!(method_definition.owner, owner);
        assert!(matches!(field_definition.slot, MemberSlot::Field(slot) if slot as usize == index));
        assert!(
            matches!(method_definition.slot, MemberSlot::Method(slot) if slot as usize == index)
        );
        assert!(matches!(
            model.nominal_member(field.member),
            Some(NominalMember::Field { owner: actual, field: canonical })
                if actual == owner && std::ptr::eq(canonical, field)
        ));
        assert!(matches!(
            model.nominal_member(method.member),
            Some(NominalMember::Method { owner: actual, name: actual_name, method: canonical })
                if actual == owner && actual_name == name && std::ptr::eq(canonical, method)
        ));
    }
    assert_eq!(identities.len(), 6);
    let mut symbol = None;
    for item in program.items {
        let Item::Class(declaration) = item else {
            unreachable!();
        };
        let actual = model.identifier_symbol(declaration.name.span).unwrap();
        assert_eq!(*symbol.get_or_insert(actual), actual);
    }
}

#[test]
fn three_level_generic_inheritance_preserves_substitutions_and_declaring_slots() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        r#"
        class Leaf extends Mid<int> {
            string leaf;
            string own() { return this.leaf; }
        }
        class Mid<U> extends Base<U[]> {
            U middle;
            U mid() { return this.middle; }
        }
        class Base<T> {
            T value;
            T read() { return this.value; }
        }
        "#,
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    let base = model.class_info("Base").unwrap();
    let middle = model.class_info("Mid").unwrap();
    let leaf = model.class_info("Leaf").unwrap();
    assert_eq!(base.fields["value"].ty, Type::TypeParameter("T"));
    assert_eq!(
        middle.fields["value"].ty,
        Type::Array(Box::new(Type::TypeParameter("U")))
    );
    assert_eq!(leaf.fields["value"].ty, Type::Array(Box::new(Type::Int)));
    assert_eq!(middle.fields["middle"].ty, Type::TypeParameter("U"));
    assert_eq!(leaf.fields["middle"].ty, Type::Int);
    assert_eq!(
        leaf.methods["read"].signature.return_type.as_ref(),
        &Type::Array(Box::new(Type::Int))
    );
    assert_eq!(
        leaf.methods["mid"].signature.return_type.as_ref(),
        &Type::Int
    );
    assert_eq!(
        leaf.fields.keys().copied().collect::<Vec<_>>(),
        ["value", "middle", "leaf"]
    );
    assert_eq!(
        leaf.methods.keys().copied().collect::<Vec<_>>(),
        ["read", "mid", "own"]
    );
    assert_eq!(base.fields["value"].member, middle.fields["value"].member);
    assert_eq!(base.fields["value"].member, leaf.fields["value"].member);
    assert_eq!(base.methods["read"].member, leaf.methods["read"].member);
    assert_eq!(middle.fields["middle"].member, leaf.fields["middle"].member);
    assert_eq!(middle.methods["mid"].member, leaf.methods["mid"].member);
    for (name, field_name, method_name, index) in [
        ("Base", "value", "read", 0),
        ("Mid", "middle", "mid", 1),
        ("Leaf", "leaf", "own", 2),
    ] {
        let info = model.class_info(name).unwrap();
        let owner = model.nominal_id(&Type::Class(name)).unwrap();
        let field = &info.fields[field_name];
        let method = &info.methods[method_name];
        assert_eq!(field.index, index);
        assert_eq!(method.owner, name);
        let field_definition = model.declarations.nominal_members[field.member.index()];
        let method_definition = model.declarations.nominal_members[method.member.index()];
        assert_eq!(field_definition.owner, owner);
        assert_eq!(method_definition.owner, owner);
        assert!(matches!(field_definition.slot, MemberSlot::Field(slot) if slot as usize == index));
        assert!(
            matches!(method_definition.slot, MemberSlot::Method(slot) if slot as usize == index)
        );
        assert!(matches!(
            model.nominal_member(field.member),
            Some(NominalMember::Field { owner: actual, field: canonical })
                if actual == owner && std::ptr::eq(canonical, field)
        ));
        assert!(matches!(
            model.nominal_member(method.member),
            Some(NominalMember::Method { owner: actual, name: actual_name, method: canonical })
                if actual == owner && actual_name == method_name && std::ptr::eq(canonical, method)
        ));
    }
}

#[test]
fn canonical_class_and_object_signatures_finalize_pending_default_bindings() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        "int seed=7;class Base{int read(int value=seed){return value;}}class Child extends Base{int own(int value=seed){return value;}}object Api{int read(int value=seed){return value;}}extern class Foreign{int read(int value=seed);}",
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    let seed = model
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "seed")
        .unwrap()
        .id;
    for (class, method) in [
        ("Base", "read"),
        ("Child", "read"),
        ("Child", "own"),
        ("Api", "read"),
        ("Foreign", "read"),
    ] {
        let info = model.class_info(class).unwrap();
        let signature = &info.methods[method].signature;
        assert_eq!(
            signature.params[0].default,
            Some(DefaultValue::Symbol(seed))
        );
        assert!(!signature_has_pending_bindings(signature));
        if let Some(field) = info.fields.get(method) {
            let Type::Function(signature) = &field.ty else {
                panic!("expected object method field");
            };
            assert_eq!(
                signature.params[0].default,
                Some(DefaultValue::Symbol(seed))
            );
            assert!(!signature_has_pending_bindings(signature));
        }
    }
}

#[test]
fn hierarchy_errors_keep_original_spans_messages_and_own_member_storage() {
    for (source, index, field, message) in [
        ("class Left extends Right{}class Right extends Left{}", 0, None, "inheritance cycle involving class `Left`"),
        // An internal class may extend a host class (see
        // `an_internal_class_may_extend_a_host_class`); the reverse stays refused.
        ("class Base{}extern class Child extends Base{}", 1, None, "an extern class cannot extend an internal class"),
        ("class Base{int value;}class Child extends Base{int spare;int value;}", 1, Some("value"), "class `Child` cannot shadow inherited member `value`"),
        ("class Base{int value(){return 1;}}class Child extends Base{int spare;int value;}", 1, Some("value"), "class `Child` cannot shadow inherited member `value`"),
        ("class Base{int value(){return 1;}}class Child extends Base{int spare;int value(){return 2;}}", 1, None, "class `Child` cannot override inherited member `value`"),
        ("class Base{int value;}class Child extends Base{int spare;int value(){return 2;}}", 1, None, "class `Child` cannot override inherited member `value`"),
    ] {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let mut facts = ModuleFacts::new(program.source_identity());
        let mut declarations = DeclarationTables::default();
        let mut initialization = ModuleInitialization::default();
        let mut budget = AllocationBudget::new(None);
        let mut analyzer = Analyzer::new(&mut facts, &mut declarations, &mut initialization, None, &mut budget).unwrap();
        define_headers(&mut analyzer, &program);
        let (name, declaration_span) = match &program.items[index] {
            Item::Class(declaration) => (declaration.name.name, declaration.span),
            Item::ExternClass(declaration) => (declaration.name.name, declaration.span),
            _ => unreachable!(),
        };
        let info = &analyzer.declarations.classes[name];
        let expected_span = field.map_or(declaration_span, |field| info.fields[field].span);
        let own_fields = info.fields.clone();
        let own_methods = info.methods.clone();
        let error = analyzer.resolve_class_hierarchies().unwrap_err();
        assert_eq!(error, AdmittedSemanticError::Semantic(SemanticError::new(expected_span, message)), "{source}");
        let info = &analyzer.declarations.classes[name];
        assert_eq!(info.fields, own_fields, "failed hierarchy must not consume own fields");
        assert_eq!(info.methods, own_methods, "failed hierarchy must not consume own methods");
        assert_eq!(AdmittedSemanticError::Semantic(analyze(&program).unwrap_err()), error);
    }
}

#[test]
fn an_internal_class_may_extend_a_host_class() {
    let source = "extern class Error{string message;init(string message);}class VFileMessage extends Error{string reason;init(string reason){super(reason);this.reason=reason;}}";
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    assert!(crate::semantic::analyze(&program).is_ok(), "an internal subclass of a host class must check");
}

#[test]
fn host_class_rules_that_still_hold() {
    for (source, message) in [
        // A host constructor signature exists only for `super(...)`.
        ("extern class Error{init(string message);}Error e=new Error(\"x\");", "extern class `Error` cannot be constructed"),
        ("extern class Error{init(string message);init(string other);}", "an extern class declares its host constructor at most once"),
        ("extern class Error{init(string message=\"x\");}", "a host constructor signature cannot declare parameter defaults"),
        ("extern class Error{init(string message);}class M extends Error{init(){super(1);}}", "expected"),
        ("extern class Error{}class M extends Error{init(string reason){super(reason);}}", "implicit base constructor `Error` expects no arguments"),
    ] {
        let arena = bumpalo::Bump::new();
        let error = match crate::parse_source(&arena, source) {
            Err(error) => error.to_string(),
            Ok(program) => match crate::semantic::analyze(&program) {
                Err(error) => error.to_string(),
                Ok(_) => panic!("expected a refusal for {source}"),
            },
        };
        assert!(error.contains(message), "{source}: expected `{message}`, got `{error}`");
    }
}
