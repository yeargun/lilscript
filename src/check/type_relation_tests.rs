use super::*;
use crate::check::{FunctionParameter, FunctionType, GenericFunctionType, NominalId, StructType};
use crate::primitive::ParameterPassing;

fn function(params: Vec<FunctionParameter<'static>>, result: Type<'static>) -> Type<'static> {
    Type::Function(FunctionType::new(FunctionSignature {
        params,
        return_type: Box::new(result),
    }))
}

fn reordered(depth: usize) -> (Type<'static>, Type<'static>) {
    let mut left = Type::Union(vec![Type::Int, Type::Bool]);
    let mut right = Type::Union(vec![Type::Bool, Type::Int]);
    for _ in 0..depth {
        left = Type::Array(Box::new(left));
        right = Type::Array(Box::new(right));
    }
    (left, right)
}

fn function_nested(depth: usize) -> (Type<'static>, Type<'static>) {
    let (mut left, mut right) = reordered(0);
    for _ in 0..depth {
        left = function(vec![FunctionParameter::value(left)], Type::Int);
        right = function(vec![FunctionParameter::value(right)], Type::Int);
    }
    (left, right)
}

fn corpus() -> Vec<Type<'static>> {
    let mut types = vec![
        Type::Int,
        Type::Float,
        Type::Bool,
        Type::String,
        Type::Null,
        Type::Void,
        Type::TypeParameter("$js"),
        Type::TypeParameter("T"),
        Type::Enum("Number"),
        Type::Class("Number"),
        Type::Struct(StructType {
            identity: NominalId::new(0, false),
            name: "P",
        }),
        Type::Struct(StructType {
            identity: NominalId::new(1, false),
            name: "P",
        }),
        Type::Union(vec![]),
        Type::Union(vec![Type::Int]),
        Type::Union(vec![Type::Int, Type::Int]),
        Type::Union(vec![Type::Int, Type::Bool]),
        Type::Union(vec![Type::Bool, Type::Int]),
        Type::Union(vec![Type::Float, Type::TypeParameter("$js")]),
        Type::Union(vec![Type::Null, Type::Union(vec![Type::Bool, Type::Int])]),
    ];
    let bases = types.clone();
    for ty in bases {
        types.extend([
            Type::Nullable(Box::new(ty.clone())),
            Type::Array(Box::new(ty.clone())),
            Type::Record(Box::new(ty.clone())),
            Type::Task(Box::new(ty.clone())),
            Type::Generator(Box::new(ty.clone())),
            Type::Set(Box::new(ty.clone())),
            Type::Map(Box::new(Type::String), Box::new(ty.clone())),
            function(vec![FunctionParameter::value(ty)], Type::Int),
        ]);
    }
    for ty in [Type::Int, Type::Float, Type::TypeParameter("$js")] {
        for default in [None, Some(DefaultValue::Int(3)), Some(DefaultValue::Int(4))] {
            types.push(function(
                vec![FunctionParameter {
                    ty: ty.clone(),
                    passing: ParameterPassing::Value,
                    default,
                }],
                Type::Float,
            ));
        }
        types.push(function(
            vec![FunctionParameter {
                ty: ty.clone(),
                passing: ParameterPassing::MutableReference,
                default: None,
            }],
            Type::Int,
        ));
        // Invalid signatures are deliberate: equality-first behavior is part
        // of this common relation, while the checker owns their legality.
        types.push(function(
            vec![FunctionParameter {
                ty,
                passing: ParameterPassing::MutableReference,
                default: Some(DefaultValue::Int(3)),
            }],
            Type::Int,
        ));
    }
    types.push(function(
        vec![
            FunctionParameter::defaulted(Type::Int, DefaultValue::Int(1)),
            FunctionParameter::value(Type::Int),
        ],
        Type::Int,
    ));
    let generic = GenericFunctionType {
        type_params: vec!["T"],
        signature: FunctionType::new(FunctionSignature {
            params: vec![FunctionParameter::value(Type::TypeParameter("T"))],
            return_type: Box::new(Type::TypeParameter("T")),
        }),
    };
    types.push(Type::GenericFunction(generic));
    for depth in 0..=4 {
        let (left, right) = reordered(depth);
        types.extend([left, right]);
        let (left, right) = function_nested(depth);
        types.extend([left, right]);
    }
    types
}

#[test]
fn shared_relation_matches_previous_directional_and_symmetric_contracts() {
    let types = corpus();
    let mut admission = Unmetered;
    for (i, expected) in types.iter().enumerate() {
        for (j, actual) in types.iter().enumerate() {
            let forward = old_relation(expected, actual);
            let reverse = old_relation(actual, expected);
            assert_eq!(
                is_type_assignable_plain(expected, actual),
                forward,
                "directional pair {i}/{j}"
            );
            assert_eq!(
                is_type_assignable_with(expected, actual, &mut admission),
                Ok(forward)
            );
            assert_eq!(
                relate(expected, actual, RelationMode::Invariant, &mut admission),
                Ok(forward && reverse),
                "invariant pair {i}/{j}"
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    TypeWork,
    Visit,
    TypeEquality,
    DefaultEquality,
    SignatureValidation,
    ParameterPair,
}

fn kind(event: RelationEvent<'_, '_>) -> Kind {
    match event {
        RelationEvent::TypeWork(_) => Kind::TypeWork,
        RelationEvent::Visit { .. } => Kind::Visit,
        RelationEvent::TypeEquality { .. } => Kind::TypeEquality,
        RelationEvent::DefaultEquality { .. } => Kind::DefaultEquality,
        RelationEvent::SignatureValidation(_) => Kind::SignatureValidation,
        RelationEvent::ParameterPair => Kind::ParameterPair,
    }
}

#[derive(Default)]
struct Counting {
    visits: usize,
    compared_nodes: usize,
    validation_records: usize,
    events: Vec<Kind>,
}

impl RelationAdmission for Counting {
    type Error = Infallible;

    fn admit(&mut self, event: RelationEvent<'_, '_>) -> Result<(), Infallible> {
        self.events.push(kind(event));
        match event {
            RelationEvent::Visit { .. } => self.visits += 1,
            RelationEvent::TypeEquality { left, right } => {
                self.compared_nodes += payload_nodes(left) + payload_nodes(right);
            }
            RelationEvent::SignatureValidation(signature) => {
                self.validation_records += signature.params.len()
            }
            _ => {}
        }
        Ok(())
    }
}

// Test-only sizing for these generated shapes. This is not a production
// allocation or text-byte estimator, and no performance claim uses it as one.
fn payload_nodes(ty: &Type<'_>) -> usize {
    let mut pending = vec![ty];
    let mut nodes = 0;
    while let Some(ty) = pending.pop() {
        nodes += 1;
        match ty {
            Type::Array(inner)
            | Type::Record(inner)
            | Type::Set(inner)
            | Type::Task(inner)
            | Type::Generator(inner)
            | Type::Nullable(inner) => pending.push(inner),
            Type::Map(key, value) => pending.extend([key.as_ref(), value.as_ref()]),
            Type::Union(members)
            | Type::StructInstance { args: members, .. }
            | Type::ClassInstance { args: members, .. } => pending.extend(members),
            Type::Function(signature) => {
                pending.push(&signature.return_type);
                pending.extend(signature.params.iter().map(|p| &p.ty));
            }
            Type::GenericFunction(function) => {
                pending.push(&function.signature.return_type);
                pending.extend(function.signature.params.iter().map(|p| &p.ty));
            }
            _ => {}
        }
    }
    nodes
}

#[test]
fn reordered_union_array_nesting_does_not_repeat_symmetric_descent_or_equality() {
    let mut prior = None;
    for depth in [8, 16, 32, 64] {
        let (left, right) = reordered(depth);
        let mut count = Counting::default();
        assert_eq!(is_type_assignable_with(&left, &right, &mut count), Ok(true));
        assert!(
            count.visits <= depth + 24,
            "depth {depth}: {} visits",
            count.visits
        );
        assert!(
            count.compared_nodes <= 2 * depth + 64,
            "depth {depth}: {} compared nodes",
            count.compared_nodes
        );
        if let Some((old_depth, old_visits, old_nodes)) = prior {
            assert!(count.visits - old_visits <= depth - old_depth);
            assert!(count.compared_nodes - old_nodes <= 2 * (depth - old_depth));
        }
        prior = Some((depth, count.visits, count.compared_nodes));
    }
}

#[test]
fn nested_function_parameters_use_one_invariant_relation_per_parameter() {
    for depth in [4, 8, 16, 32] {
        let (left, right) = function_nested(depth);
        let mut count = Counting::default();
        assert_eq!(is_type_assignable_with(&left, &right, &mut count), Ok(true));
        assert!(count.visits <= 2 * depth + 24);
        assert_eq!(count.validation_records, 2 * depth);
        // Derived equality still inspects nested function payload at each
        // level. This asserts no exponential relation descent, not linear
        // total equality work for all recursive callable shapes.
        assert!(count.compared_nodes <= 16 * (depth + 1) * (depth + 1));
    }
}

#[test]
fn equal_invalid_signatures_keep_equality_shortcut_and_bit_defaults() {
    let invalid = || {
        function(
            vec![FunctionParameter {
                ty: Type::Int,
                passing: ParameterPassing::MutableReference,
                default: Some(DefaultValue::Float(f64::NAN.to_bits())),
            }],
            Type::Int,
        )
    };
    let (left, right) = (invalid(), invalid());
    let mut count = Counting::default();
    assert_eq!(is_type_assignable_with(&left, &right, &mut count), Ok(true));
    assert_eq!(count.events, [Kind::Visit, Kind::TypeEquality]);
    assert!(old_relation(&left, &right));
    let left = Type::Array(Box::new(left));
    let right = Type::Array(Box::new(right));
    assert!(is_type_assignable_plain(&left, &right));
}

#[test]
fn admission_refusal_is_never_a_false_or_successful_type_result() {
    let parameter = |ty| FunctionParameter {
        ty,
        passing: ParameterPassing::Value,
        default: Some(DefaultValue::Array(vec![
            DefaultValue::String("payload"),
            DefaultValue::Float(f64::NAN.to_bits()),
        ])),
    };
    let (left, right) = reordered(2);
    let left = function(vec![parameter(left)], Type::Float);
    let right = function(vec![parameter(right)], Type::Int);
    let mut count = Counting::default();
    assert_eq!(is_type_assignable_with(&left, &right, &mut count), Ok(true));
    for required in [
        Kind::Visit,
        Kind::TypeEquality,
        Kind::DefaultEquality,
        Kind::SignatureValidation,
        Kind::ParameterPair,
    ] {
        assert!(
            count.events.contains(&required),
            "missing exercised event {required:?}"
        );
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Refused(usize);
    struct Stop {
        at: usize,
        seen: usize,
    }
    impl RelationAdmission for Stop {
        type Error = Refused;
        fn admit(&mut self, _: RelationEvent<'_, '_>) -> Result<(), Refused> {
            let event = self.seen;
            self.seen += 1;
            if event == self.at {
                Err(Refused(event))
            } else {
                Ok(())
            }
        }
    }
    for at in 0..count.events.len() {
        let mut stop = Stop { at, seen: 0 };
        assert_eq!(
            is_type_assignable_with(&left, &right, &mut stop),
            Err(Refused(at))
        );
        assert_eq!(stop.seen, at + 1, "continued after refusal");
        assert_eq!(
            is_type_assignable_with(&left, &right, &mut Counting::default()),
            Ok(true)
        );
    }
    let mut stop = Stop { at: 0, seen: 0 };
    assert_eq!(
        is_type_assignable_with(&Type::Int, &Type::String, &mut stop),
        Err(Refused(0))
    );
    assert!(!is_type_assignable_plain(&Type::Int, &Type::String));
}

// The previous common algorithm is retained only as a bounded differential
// test oracle. Production has one relation implementation.

fn old_relation(expected: &Type<'_>, actual: &Type<'_>) -> bool {
    if expected == actual {
        return true;
    }
    match (expected, actual) {
        (Type::TypeParameter("$js"), _) => !actual.is_void(),
        (Type::Float, Type::Int) => true,
        (Type::Array(expected), Type::Array(actual)) => {
            old_relation(expected, actual) && old_relation(actual, expected)
        }
        (Type::Record(expected), Type::Record(actual)) => expected == actual,
        (Type::Map(expected_key, expected_value), Type::Map(actual_key, actual_value)) => {
            expected_key == actual_key && expected_value == actual_value
        }
        (Type::Set(expected), Type::Set(actual)) => expected == actual,
        (Type::Task(expected), Type::Task(actual)) => old_relation(expected, actual),
        (Type::Generator(expected), Type::Generator(actual)) => old_relation(expected, actual),
        (Type::Nullable(_), Type::Null) => true,
        (Type::Nullable(expected), Type::Nullable(actual)) => old_relation(expected, actual),
        (Type::Nullable(expected), actual) => old_relation(expected, actual),
        (Type::Union(expected), Type::Union(actual)) => actual.iter().all(|actual| {
            expected
                .iter()
                .any(|expected| old_relation(expected, actual))
        }),
        (Type::Union(expected), actual) => expected
            .iter()
            .any(|expected| old_relation(expected, actual)),
        (expected, Type::Union(actual)) => {
            actual.iter().all(|actual| old_relation(expected, actual))
        }
        (Type::Function(expected), Type::Function(actual))
            if expected.params.len() == actual.params.len() =>
        {
            expected.validate_parameters().is_ok()
                && actual.validate_parameters().is_ok()
                && expected
                    .params
                    .iter()
                    .zip(&actual.params)
                    .all(|(expected, actual)| {
                        expected.passing == actual.passing
                            && old_relation(&expected.ty, &actual.ty)
                            && old_relation(&actual.ty, &expected.ty)
                            && expected
                                .default
                                .as_ref()
                                .is_none_or(|default| actual.default.as_ref() == Some(default))
                    })
                && old_relation(&expected.return_type, &actual.return_type)
        }
        _ => false,
    }
}
