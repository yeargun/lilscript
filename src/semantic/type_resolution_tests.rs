use super::*;

fn binding<'model, 'src>(model: &'model SemanticModel<'_, 'src>, name: &str) -> &'model Type<'src> {
    let mut symbols = model.symbols().iter().filter(|symbol| symbol.name == name);
    let symbol = symbols.next().expect("fixture binding");
    assert!(symbols.next().is_none(), "ambiguous fixture binding {name}");
    &symbol.ty
}

#[test]
fn nested_builtin_types_preserve_every_resolved_argument() {
    for depth in [1, 8, 32] {
        for name in ["Map", "Set", "Task", "Generator", "Record"] {
            let mut spelling = "string[]".to_string();
            let mut expected = Type::Array(Box::new(Type::String));
            for _ in 0..depth {
                spelling = if name == "Map" {
                    format!("Map<string,{spelling} >")
                } else {
                    format!("{name}<{spelling} >")
                };
                expected = match name {
                    "Map" => Type::Map(Box::new(Type::String), Box::new(expected)),
                    "Set" => Type::Set(Box::new(expected)),
                    "Task" => Type::Task(Box::new(expected)),
                    "Generator" => Type::Generator(Box::new(expected)),
                    "Record" => Type::Record(Box::new(expected)),
                    _ => unreachable!(),
                };
            }
            let source = format!("void consume({spelling} value){{}}");
            let arena = bumpalo::Bump::new();
            let program = crate::parse_source(&arena, &source).unwrap();
            let model = analyze(&program).unwrap();
            assert_eq!(binding(&model, "value"), &expected, "{name}/{depth}");
            let Type::Function(signature) = binding(&model, "consume") else {
                panic!("function fixture")
            };
            assert_eq!(signature.params[0].ty, expected, "{name}/{depth}");
            assert_eq!(*signature.return_type, Type::Void);
        }
    }
}

#[test]
fn explicit_and_contextual_collection_constructors_keep_nested_types() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        r#"
        auto explicitMap=new Map<string,Record<int[]> >();
        Map<string,Record<int[]> > inferredMap=new Map();
        auto explicitSet=new Set<int[]>();
        Set<int[]> inferredSet=new Set();
        auto dynamicMap=new Map<JsValue,Task<Record<bool> > >();
        auto nestedSet=new Set<Map<string,Generator<int[]> > >();
        "#,
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    let map = Type::Map(
        Box::new(Type::String),
        Box::new(Type::Record(Box::new(Type::Array(Box::new(Type::Int))))),
    );
    for name in ["explicitMap", "inferredMap"] {
        assert_eq!(binding(&model, name), &map);
    }
    let set = Type::Set(Box::new(Type::Array(Box::new(Type::Int))));
    for name in ["explicitSet", "inferredSet"] {
        assert_eq!(binding(&model, name), &set);
    }
    assert_eq!(
        binding(&model, "dynamicMap"),
        &Type::Map(
            Box::new(Type::TypeParameter("$js")),
            Box::new(Type::Task(Box::new(Type::Record(Box::new(Type::Bool))))),
        )
    );
    assert_eq!(
        binding(&model, "nestedSet"),
        &Type::Set(Box::new(Type::Map(
            Box::new(Type::String),
            Box::new(Type::Generator(Box::new(Type::Array(Box::new(Type::Int))))),
        )))
    );
}

#[test]
fn nominal_parameter_metadata_stays_canonical_and_task_generator_shadowing_stays_nominal() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        r#"
        struct Task<T>{T value;}
        struct Plain{int value;}
        class Generator<T>{T value;init(T input){this.value=input;}}
        class Empty{int value;}
        void inspect(Task<Record<int[]> > first,Generator<Set<string> > second,Plain third,Empty fourth){}
        void repeat(Task<Record<bool> > fifth,Generator<int[]> sixth){}
        "#,
    )
    .unwrap();
    let model = analyze(&program).unwrap();
    let task = model.struct_info("Task").unwrap();
    assert_eq!(task.type_params, ["T"]);
    assert_eq!(task.fields["value"].ty, Type::TypeParameter("T"));
    assert_eq!(
        binding(&model, "first"),
        &Type::StructInstance {
            declaration: task.declaration,
            args: vec![Type::Record(Box::new(Type::Array(Box::new(Type::Int))))],
        }
    );
    assert_eq!(
        binding(&model, "fifth"),
        &Type::StructInstance {
            declaration: task.declaration,
            args: vec![Type::Record(Box::new(Type::Bool))],
        }
    );
    let generator = model.class_info("Generator").unwrap();
    assert_eq!(generator.type_params, ["T"]);
    assert_eq!(generator.fields["value"].ty, Type::TypeParameter("T"));
    assert_eq!(
        binding(&model, "second"),
        &Type::ClassInstance {
            name: "Generator",
            args: vec![Type::Set(Box::new(Type::String))],
        }
    );
    assert_eq!(
        binding(&model, "sixth"),
        &Type::ClassInstance {
            name: "Generator",
            args: vec![Type::Array(Box::new(Type::Int))],
        }
    );
    assert_eq!(
        binding(&model, "third"),
        &Type::Struct(model.struct_info("Plain").unwrap().declaration)
    );
    assert_eq!(binding(&model, "fourth"), &Type::Class("Empty"));
}

#[test]
fn arity_resolution_order_and_key_diagnostics_keep_exact_spans() {
    for (source, marked, message) in [
        (
            "void f(Map<int> value){}",
            "Map<int",
            "type `Map` expects 2 type arguments, found 1",
        ),
        (
            "void f(Set value){}",
            "Set",
            "type `Set` expects 1 type arguments, found 0",
        ),
        (
            "void f(Task<int,string> value){}",
            "Task<int,string",
            "type `Task` expects 1 type arguments, found 2",
        ),
        (
            "void f(Generator value){}",
            "Generator",
            "type `Generator` expects 1 type arguments, found 0",
        ),
        (
            "void f(Record<int,string> value){}",
            "Record<int,string",
            "type `Record` expects 1 type arguments, found 2",
        ),
        (
            "void f(Task<void> value){}",
            "void",
            "type argument cannot have type `void`",
        ),
        (
            "void f(Generator<Missing> value){}",
            "Missing",
            "unknown type `Missing`",
        ),
        (
            "void f(Record<Map<Missing,Other> > value){}",
            "Missing",
            "unknown type `Missing`",
        ),
        (
            "struct Key{int n;}void f(Map<Key,Missing> value){}",
            "Missing",
            "unknown type `Missing`",
        ),
        (
            "struct Key{int n;}void f(Map<Key,int> value){}",
            "Map<Key,int",
            "Map key type `Key` has no portable identity contract",
        ),
        (
            "struct Key{int n;}void f(Set<Key> value){}",
            "Set<Key",
            "Set element type `Key` has no portable identity contract",
        ),
        (
            "auto value=new Map<int>();",
            "new Map<int>()",
            "type `Map` expects 2 type arguments, found 1",
        ),
        (
            "auto value=new Set<int,string>();",
            "new Set<int,string>()",
            "type `Set` expects 1 type arguments, found 2",
        ),
        (
            "auto value=new Map<Missing,Other>(1);",
            "new Map<Missing,Other>(1)",
            "`Map` constructor expects 0 arguments, found 1",
        ),
        (
            "auto value=new Set<Missing>(1);",
            "new Set<Missing>(1)",
            "`Set` constructor expects 0 arguments, found 1",
        ),
        (
            "struct Key{int n;}auto value=new Map<Key,Missing>();",
            "Missing",
            "unknown type `Missing`",
        ),
        (
            "struct Key{int n;}auto value=new Map<Key,int>();",
            "new Map<Key,int>()",
            "Map key type `Key` has no portable identity contract",
        ),
        (
            "struct Key{int n;}auto value=new Set<Key>();",
            "new Set<Key>()",
            "Set element type `Key` has no portable identity contract",
        ),
        (
            "struct Box<T>{T value;}void f(Box<int,string> value){}",
            "Box<int,string",
            "type `Box` expects 1 type arguments, found 2",
        ),
        (
            "class Box<T>{T value;}void f(Box value){}",
            "Box",
            "type `Box` expects 1 type arguments, found 0",
        ),
    ] {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let start = source.rfind(marked).unwrap();
        assert_eq!(
            analyze(&program).unwrap_err(),
            SemanticError::new(Span::new(start, start + marked.len()), message),
            "{source}"
        );
    }
}
