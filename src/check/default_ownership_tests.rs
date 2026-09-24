use super::*;
use std::sync::Arc;

fn signature(default: Option<DefaultValue<'static>>) -> FunctionType<'static> {
    FunctionType::new(FunctionSignature {
        params: vec![FunctionParameter {
            ty: Type::Int,
            passing: ParameterPassing::Value,
            default,
        }],
        return_type: Box::new(Type::Int),
    })
}

fn wrapped(kind: usize, value: Type<'static>) -> Type<'static> {
    match kind {
        0 => value,
        1 => Type::Array(Box::new(value)),
        2 => Type::Record(Box::new(value)),
        3 => Type::Set(Box::new(value)),
        4 => Type::Task(Box::new(value)),
        5 => Type::Generator(Box::new(value)),
        6 => Type::Nullable(Box::new(value)),
        7 => Type::Map(Box::new(Type::Int), Box::new(value)),
        8 => Type::Map(Box::new(value), Box::new(Type::String)),
        9 => Type::Union(vec![Type::Int, value]),
        10 => Type::StructInstance {
            declaration: StructType {
                identity: NominalId::new(0, false),
                name: "Container",
            },
            args: vec![value],
        },
        11 => Type::ClassInstance {
            name: "Container",
            args: vec![value],
        },
        _ => unreachable!(),
    }
}

fn signatures<'a, 'src>(ty: &'a Type<'src>) -> Vec<&'a FunctionType<'src>> {
    fn visit<'a, 'src>(ty: &'a Type<'src>, output: &mut Vec<&'a FunctionType<'src>>) {
        match ty {
            Type::Array(value)
            | Type::Record(value)
            | Type::Set(value)
            | Type::Task(value)
            | Type::Generator(value)
            | Type::Nullable(value) => visit(value, output),
            Type::Map(key, value) => {
                visit(key, output);
                visit(value, output);
            }
            Type::Union(members)
            | Type::StructInstance { args: members, .. }
            | Type::ClassInstance { args: members, .. } => {
                for member in members {
                    visit(member, output);
                }
            }
            Type::Function(signature) => {
                output.push(signature);
                for parameter in &signature.params {
                    visit(&parameter.ty, output);
                }
                visit(&signature.return_type, output);
            }
            Type::GenericFunction(function) => {
                output.push(&function.signature);
                for parameter in &function.signature.params {
                    visit(&parameter.ty, output);
                }
                visit(&function.signature.return_type, output);
            }
            _ => {}
        }
    }
    let mut output = Vec::new();
    visit(ty, &mut output);
    output
}

#[test]
fn default_stripping_preserves_unchanged_signatures_through_every_type_container() {
    for kind in 0..12 {
        for generic in [false, true] {
            for has_default in [false, true] {
                let original_signature = signature(has_default.then_some(DefaultValue::Int(7)));
                let value = if generic {
                    Type::GenericFunction(GenericFunctionType {
                        type_params: vec!["T"],
                        signature: original_signature.clone(),
                    })
                } else {
                    Type::Function(original_signature.clone())
                };
                let original = wrapped(kind, value);
                let mut stripped = original.clone();
                strip_parameter_defaults_from_type(&mut stripped);
                let found = signatures(&stripped);
                assert_eq!(found.len(), 1);
                assert_eq!(
                    Arc::ptr_eq(&found[0].0, &original_signature.0),
                    !has_default,
                    "container {kind}, generic {generic}"
                );
                assert_eq!(found[0].params[0].default, None);
                assert_eq!(found[0].required_params(), 1);
                assert!(!found[0].accepts_arity(0));
                assert_eq!(found[0].params[0].passing, ParameterPassing::Value);
                assert_eq!(found[0].params[0].ty, Type::Int);
                assert_eq!(*found[0].return_type, Type::Int);
                assert_eq!(
                    original_signature.params[0].default,
                    has_default.then_some(DefaultValue::Int(7))
                );
                if !has_default {
                    assert_eq!(stripped, original);
                }
                let prior = stripped.clone();
                strip_parameter_defaults_from_type(&mut stripped);
                assert_eq!(stripped, prior);
                assert!(Arc::ptr_eq(
                    &signatures(&stripped)[0].0,
                    &signatures(&prior)[0].0
                ));
            }
        }
    }
}

#[test]
fn default_stripping_detaches_changed_ancestors_but_not_unaffected_siblings() {
    let unchanged = signature(None);
    let changed = signature(Some(DefaultValue::Int(7)));
    let original = Type::Function(FunctionType::new(FunctionSignature {
        params: vec![
            FunctionParameter {
                ty: Type::Int,
                passing: ParameterPassing::MutableReference,
                default: None,
            },
            FunctionParameter::value(Type::Function(unchanged.clone())),
            FunctionParameter::value(Type::Nullable(Box::new(Type::Array(Box::new(
                Type::GenericFunction(GenericFunctionType {
                    type_params: vec!["T"],
                    signature: changed.clone(),
                }),
            ))))),
        ],
        return_type: Box::new(Type::Function(unchanged.clone())),
    }));
    let mut stripped = original.clone();
    strip_parameter_defaults_from_type(&mut stripped);
    let before = signatures(&original);
    let after = signatures(&stripped);
    assert_eq!(after.len(), 4);
    for (index, shared) in [(0, false), (1, true), (2, false), (3, true)] {
        assert_eq!(Arc::ptr_eq(&before[index].0, &after[index].0), shared);
    }
    assert_eq!(
        after[0].params[0].passing,
        ParameterPassing::MutableReference
    );
    assert_eq!(after[0].validate_parameters(), Ok(()));
    assert_eq!(after[2].params[0].default, None);
    assert_eq!(before[2].params[0].default, Some(DefaultValue::Int(7)));
    assert!(!signature_has_default_matching(after[0], |_| true));
    assert!(signature_has_default_matching(before[0], |_| true));
    let prior = stripped.clone();
    strip_parameter_defaults_from_type(&mut stripped);
    for (old, new) in signatures(&prior).iter().zip(signatures(&stripped)) {
        assert!(Arc::ptr_eq(&old.0, &new.0));
    }
}

#[test]
fn default_stripping_finds_defaults_only_inside_a_return_type() {
    let unchanged = signature(None);
    let returned = signature(Some(DefaultValue::Int(9)));
    let original = Type::Function(FunctionType::new(FunctionSignature {
        params: vec![FunctionParameter::value(Type::Function(unchanged.clone()))],
        return_type: Box::new(Type::Nullable(Box::new(Type::GenericFunction(
            GenericFunctionType {
                type_params: vec!["T"],
                signature: returned.clone(),
            },
        )))),
    }));
    let mut stripped = original.clone();
    strip_parameter_defaults_from_type(&mut stripped);
    let before = signatures(&original);
    let after = signatures(&stripped);
    assert_eq!(after.len(), 3);
    assert!(!Arc::ptr_eq(&before[0].0, &after[0].0));
    assert!(Arc::ptr_eq(&before[1].0, &after[1].0));
    assert!(!Arc::ptr_eq(&before[2].0, &after[2].0));
    assert_eq!(after[2].params[0].default, None);
    assert_eq!(before[2].params[0].default, Some(DefaultValue::Int(9)));
    assert!(!signature_has_default_matching(after[0], |_| true));
}

#[test]
fn default_stripping_erases_pending_and_resolved_metadata_without_rebinding_it() {
    let nodes = crate::ast::SourceNodes::default();
    let expression = nodes.expression(ExprKind::Int(7, Span::default()));
    for default in [
        DefaultValue::Symbol(SymbolId(17)),
        DefaultValue::Parameter(0),
        DefaultValue::PendingIdentifier {
            expression: expression.id,
            span: Span::default(),
        },
        DefaultValue::PendingUndefined {
            expression: expression.id,
            span: Span::default(),
        },
        DefaultValue::Array(vec![DefaultValue::Int(7)]),
    ] {
        let original = signature(Some(default.clone()));
        let pending = signature_has_pending_bindings(&original);
        let mut stripped = Type::Function(original.clone());
        strip_parameter_defaults_from_type(&mut stripped);
        let result = signatures(&stripped)[0];
        assert_eq!(result.params[0].default, None);
        assert!(!signature_has_pending_bindings(result));
        assert_eq!(original.params[0].default, Some(default));
        assert_eq!(signature_has_pending_bindings(&original), pending);
        assert!(!Arc::ptr_eq(&result.0, &original.0));
    }
}

#[test]
fn source_default_contracts_keep_named_aliases_optional_and_mutable_storage_required() {
    for source in [
        "int answer(int value=7){return value;}auto alias=answer;int result=alias();",
        "int answer(int value){return value;}func(int)->int alias=answer;int result=alias(4);",
        "auto alias=(int value=7)=>value;int result=alias(4);",
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        analyze(&syntax).unwrap();
    }
    for source in [
        "int answer(int value=7){return value;}func(int)->int alias=answer;int result=alias();",
        "auto alias=(int value=7)=>value;int result=alias();",
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let error = analyze(&syntax).unwrap_err();
        assert!(
            error.message.contains("expects 1 to 1 arguments, found 0"),
            "{error}"
        );
    }
}
