use super::*;

#[test]
fn s4_function_identity_contains_arity_but_not_default_expressions() {
    let arena = bumpalo::Bump::new();
    let syntax=crate::parse_source(&arena,"int first(int value=3){return value;}int second(int value=9){return value;}auto f=first;f=second;print(f());").unwrap();
    let checked = analyze(&syntax).unwrap();
    let signatures: Vec<_> = checked
        .symbols()
        .iter()
        .filter(|symbol| matches!(symbol.name, "first" | "second" | "f"))
        .map(|s| &s.ty)
        .collect();
    assert_eq!(signatures.len(), 3);
    assert!(signatures.windows(2).all(|pair| pair[0] == pair[1]));
    let Type::Function(signature) = signatures[0] else {
        panic!()
    };
    assert_eq!(signature.required_params(), 0);
    assert!(signature.accepts_arity(0) && signature.accepts_arity(1));
    assert!(!signature.accepts_arity(2));
    assert_eq!(
        crate::interpreter::interpret_program(&syntax, &checked).unwrap(),
        "9\n"
    );
}

#[test]
fn s4_optional_and_rest_contracts_remain_distinct() {
    let callable = |parameter| {
        Type::Function(FunctionType::new(FunctionSignature {
            params: vec![parameter],
            return_type: Box::new(Type::Int),
        }))
    };
    let required = callable(FunctionParameter::value(Type::Int));
    let optional = callable(FunctionParameter::optional(Type::Int));
    assert_ne!(required, optional);
    assert!(is_type_assignable(&required, &optional));
    assert!(!is_type_assignable(&optional, &required));
    let mut rest = FunctionParameter::value(Type::Array(Box::new(Type::Int)));
    rest.rest = true;
    assert_ne!(
        callable(rest),
        callable(FunctionParameter::value(Type::Array(Box::new(Type::Int))))
    );
}
