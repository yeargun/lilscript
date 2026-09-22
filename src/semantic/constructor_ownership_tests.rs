use super::*;

fn assert_binding_type<'src>(model: &SemanticModel<'_, 'src>, name: &str, expected: Type<'src>) {
    let matches = model
        .symbols()
        .iter()
        .filter(|symbol| symbol.name == name)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "binding {name}");
    assert_eq!(matches[0].ty, expected, "binding {name}");
}

fn assert_error(source: &str, marked: &str, message: &str) {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let start = source.rfind(marked).expect("marked diagnostic source");
    assert_eq!(
        analyze(&program).unwrap_err(),
        SemanticError::new(Span::new(start, start + marked.len()), message),
        "{source}"
    );
}

#[test]
fn repeated_construction_preserves_generic_inference_and_unrelated_member_types() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        r#"
        class Plain {
            int stored;
            string[][] unrelated;
            init(int plainValue) { this.stored=plainValue; }
            string[][] echo(string[][] text) { return text; }
        }
        class Box<T> {
            T value;
            T[][] nested;
            init(T boxValue) { this.value=boxValue; }
            U[][] echo<U>(U[][] input) { return input; }
        }
        auto firstPlain=new Plain(1);
        Plain secondPlain=new Plain(2);
        auto inferredBox=new Box(7);
        auto explicitBox=new Box<string>("text");
        Box<bool> contextualBox=new Box(true);
        auto nestedBox=new Box(new Plain(3));
        auto anotherBox=new Box(8);
        int readValue=inferredBox.value;
        string[][] textResult=secondPlain.echo([["ok"]]);
        int[][] genericResult=inferredBox.echo([[1]]);
        "#,
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    for name in ["firstPlain", "secondPlain"] {
        assert_binding_type(&model, name, Type::Class("Plain"));
    }
    for (name, argument) in [
        ("inferredBox", Type::Int),
        ("explicitBox", Type::String),
        ("contextualBox", Type::Bool),
        ("nestedBox", Type::Class("Plain")),
        ("anotherBox", Type::Int),
    ] {
        assert_binding_type(
            &model,
            name,
            Type::ClassInstance {
                name: "Box",
                args: vec![argument],
            },
        );
    }
    assert_binding_type(&model, "readValue", Type::Int);
    assert_binding_type(
        &model,
        "textResult",
        Type::Array(Box::new(Type::Array(Box::new(Type::String)))),
    );
    assert_binding_type(
        &model,
        "genericResult",
        Type::Array(Box::new(Type::Array(Box::new(Type::Int)))),
    );
    let plain = model.class_info("Plain").unwrap();
    assert_eq!(
        plain.fields["unrelated"].ty,
        Type::Array(Box::new(Type::Array(Box::new(Type::String))))
    );
    assert_eq!(plain.constructor.as_ref().unwrap().params[0].ty, Type::Int);
    let boxed = model.class_info("Box").unwrap();
    assert_eq!(boxed.type_params, ["T"]);
    assert_eq!(boxed.fields["value"].ty, Type::TypeParameter("T"));
    assert_eq!(
        boxed.fields["nested"].ty,
        Type::Array(Box::new(Type::Array(Box::new(Type::TypeParameter("T")))))
    );
    assert_eq!(
        boxed.constructor.as_ref().unwrap().params[0].ty,
        Type::TypeParameter("T")
    );
    assert_eq!(boxed.methods["echo"].type_params, ["U"]);
    assert_eq!(
        boxed.methods["echo"].signature.return_type.as_ref(),
        &Type::Array(Box::new(Type::Array(Box::new(Type::TypeParameter("U")))))
    );
}

#[test]
fn three_level_super_calls_preserve_substitution_and_constructor_defaults() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        r#"
        int seed=7;
        class Base<T> {
            T[] values;
            int count;
            init(T[] baseValues,int baseCount=seed) {
                this.values=baseValues;
                this.count=baseCount;
            }
        }
        class Mid<U> extends Base<U[]> {
            U[][][] unrelated;
            init(U[][] middleValues,int middleCount=seed) {
                super(middleValues,middleCount);
            }
        }
        class Leaf extends Mid<int> {
            init(int[][] leafValues,int leafCount=seed) {
                super(leafValues);
                this.count=leafCount;
            }
        }
        auto firstLeaf=new Leaf([[1]]);
        auto secondLeaf=new Leaf([[2]],9);
        auto middle=new Mid<string>([["a"]]);
        int[][] inheritedValues=firstLeaf.values;
        int inheritedCount=secondLeaf.count;
        "#,
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    let seed = model
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "seed")
        .unwrap()
        .id;
    for name in ["Base", "Mid", "Leaf"] {
        let signature = model
            .class_info(name)
            .unwrap()
            .constructor
            .as_ref()
            .unwrap();
        assert_eq!(signature.required_params(), 1);
        assert!(signature.accepts_arity(1));
        assert!(signature.accepts_arity(2));
        assert!(!signature.accepts_arity(0));
        assert!(!signature.accepts_arity(3));
        assert_eq!(
            signature.params[1].default,
            Some(DefaultValue::Symbol(seed))
        );
        assert!(!signature_has_pending_bindings(signature));
    }
    let (base_name, base_signature) = model.base_constructor("Mid").unwrap();
    assert_eq!(base_name, "Base");
    assert_eq!(
        base_signature.params[0].ty,
        Type::Array(Box::new(Type::Array(Box::new(Type::TypeParameter("U")))))
    );
    let (middle_name, middle_signature) = model.base_constructor("Leaf").unwrap();
    assert_eq!(middle_name, "Mid");
    assert_eq!(
        middle_signature.params[0].ty,
        Type::Array(Box::new(Type::Array(Box::new(Type::Int))))
    );
    assert_eq!(
        middle_signature.params[1].default,
        Some(DefaultValue::Symbol(seed))
    );
    assert_binding_type(&model, "firstLeaf", Type::Class("Leaf"));
    assert_binding_type(&model, "secondLeaf", Type::Class("Leaf"));
    assert_binding_type(
        &model,
        "middle",
        Type::ClassInstance {
            name: "Mid",
            args: vec![Type::String],
        },
    );
    assert_binding_type(
        &model,
        "inheritedValues",
        Type::Array(Box::new(Type::Array(Box::new(Type::Int)))),
    );
    assert_binding_type(&model, "inheritedCount", Type::Int);
}

#[test]
fn implicit_and_no_base_constructors_keep_contextual_generic_types() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        r#"
        class Plain { int[][] payload; }
        class Child extends Plain {}
        class Explicit extends Plain { init() { super(); } }
        class Empty<T> { T[][] payload; }
        class Own { int value; init(int inputValue=3) { this.value=inputValue; } }
        auto plain=new Plain();
        auto child=new Child();
        auto explicitChild=new Explicit();
        Empty<int> contextual=new Empty();
        auto explicitGeneric=new Empty<string>();
        auto ownDefault=new Own();
        auto ownValue=new Own(4);
        "#,
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    for (binding, class) in [
        ("plain", "Plain"),
        ("child", "Child"),
        ("explicitChild", "Explicit"),
        ("ownDefault", "Own"),
        ("ownValue", "Own"),
    ] {
        assert_binding_type(&model, binding, Type::Class(class));
    }
    for (binding, argument) in [("contextual", Type::Int), ("explicitGeneric", Type::String)] {
        assert_binding_type(
            &model,
            binding,
            Type::ClassInstance {
                name: "Empty",
                args: vec![argument],
            },
        );
    }
    for name in ["Plain", "Child", "Empty"] {
        assert!(model.class_info(name).unwrap().constructor.is_none());
    }
    assert!(model.base_constructor("Explicit").is_none());
    assert_eq!(
        model
            .class_info("Own")
            .unwrap()
            .constructor
            .as_ref()
            .unwrap()
            .required_params(),
        0
    );
}

#[test]
fn construction_errors_preserve_exact_precedence_and_inference_revalidation() {
    for (source, marked, message) in [
        (
            "auto value=new Missing(missing);",
            "Missing",
            "unknown class `Missing`",
        ),
        (
            "auto value=new Missing(ref missing);",
            "ref missing",
            "constructors do not support mutable-reference arguments",
        ),
        (
            "extern class Foreign{}auto value=new Foreign<Missing>(missing);",
            "new Foreign<Missing>(missing)",
            "extern class `Foreign` cannot be constructed",
        ),
        (
            "object Api{int value(){return 1;}}auto value=new Api<Missing>(missing);",
            "new Api<Missing>(missing)",
            "object `Api` cannot be constructed with `new`",
        ),
        (
            "class Box<T>{init(T value){}}auto value=new Box<int,string>();",
            "new Box<int,string>()",
            "class `Box` constructor expects 1 arguments, found 0",
        ),
        (
            "class Box<T>{init(T value){}}auto value=new Box<int,string>(missing);",
            "new Box<int,string>(missing)",
            "type `Box` expects 1 type arguments, found 2",
        ),
        (
            "class Plain{}auto value=new Plain(missing);",
            "new Plain(missing)",
            "class `Plain` constructor expects 0 arguments, found 1",
        ),
        (
            "class Box{init(int value){}}auto value=new Box(\"bad\");",
            "\"bad\"",
            "expected `int`, found `string`",
        ),
        (
            "class Box{init(ref int value){}}",
            "init(ref int value){}",
            "constructors do not support mutable-reference parameters",
        ),
        (
            "class Empty<T>{}auto value=new Empty();",
            "new Empty()",
            "cannot infer type argument `T`",
        ),
        (
            "class Pair<T>{init(T left,T right){}}auto value=new Pair(1,\"bad\");",
            "\"bad\"",
            "conflicting inferences for `T`: `int` and `string`",
        ),
        (
            "struct Entry{int value;}class Pair<T>{T[][] unrelated;init(T[] left,T[] right){}}Entry[] entries=[Entry{1}];JsValue[] values=[\"bad\"];auto pair=new Pair(entries,values);",
            "entries",
            "expected `JsValue[]`, found `Entry[]`",
        ),
    ] {
        assert_error(source, marked, message);
    }
}

#[test]
fn super_errors_preserve_constructor_structure_before_argument_diagnostics() {
    for (source, marked, message) in [
        (
            "class Plain{init(){super(missing);}}",
            "init(){super(missing);}",
            "`super` is only valid in a derived class constructor",
        ),
        (
            "class Base{init(){}}class Child extends Base{init(){print(missing);super();super();}}",
            "init(){print(missing);super();super();}",
            "a derived constructor may call `super` only once",
        ),
        (
            "class Base{init(int value){}}class Child extends Base{init(){print(missing);}}",
            "init(){print(missing);}",
            "derived constructor must begin with `super(...)` for `Base`",
        ),
        (
            "class Base{init(int value){}}class Child extends Base{init(){print(missing);super(1);}}",
            "init(){print(missing);super(1);}",
            "`super(...)` must be the first statement in a derived constructor",
        ),
        (
            "class Base{}class Child extends Base{init(){super(missing);}}",
            "super(missing);",
            "implicit base constructor `Base` expects no arguments",
        ),
        (
            "class Base<T>{init(T value){}}class Child extends Base<int>{init(){super(\"bad\");}}",
            "\"bad\"",
            "expected `int`, found `string`",
        ),
        (
            "class Base{init(int value){}}class Child extends Base{init(){super(ref missing);}}",
            "ref missing",
            "super calls do not support mutable-reference arguments",
        ),
        (
            "class Base{init(){}}class Child extends Base{}",
            "class Child extends Base{}",
            "class `Child` must declare `init` and call its base constructor",
        ),
    ] {
        assert_error(source, marked, message);
    }
}
