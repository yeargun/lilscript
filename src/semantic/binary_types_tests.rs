use super::*;
use crate::primitive::ParameterPassing;
use crate::semantic::{
    DefaultValue, FunctionParameter, FunctionSignature, FunctionType, GenericFunctionType,
    NominalId, StructType,
};

#[path = "binary_types_old_oracle.rs"]
mod old;

fn function(passing: ParameterPassing, default: Option<DefaultValue<'static>>) -> Type<'static> {
    Type::Function(FunctionType::new(FunctionSignature {
        params: vec![FunctionParameter {
            ty: Type::Int,
            passing,
            default,
        }],
        return_type: Box::new(Type::String),
    }))
}

fn corpus() -> Vec<Type<'static>> {
    let named = StructType {
        identity: NominalId::new(0, false),
        name: "First",
    };
    let alias = StructType {
        identity: named.identity,
        name: "Alias",
    };
    let other = StructType {
        identity: NominalId::new(1, false),
        name: "First",
    };
    let mut types = vec![
        Type::Int,
        Type::Float,
        Type::String,
        Type::Bool,
        Type::Null,
        Type::Void,
        Type::TypeParameter("$js"),
        Type::TypeParameter("T"),
        Type::Class("Object"),
        Type::Class("Other"),
        Type::Enum("Choice"),
        Type::Symbol,
        Type::Regex,
        Type::Struct(named),
        Type::Struct(alias),
        Type::Struct(other),
        Type::StructInstance {
            declaration: named,
            args: vec![Type::Int],
        },
        Type::ClassInstance {
            name: "Container",
            args: vec![Type::String],
        },
        Type::Array(Box::new(Type::Int)),
        Type::Array(Box::new(Type::Float)),
        Type::Record(Box::new(Type::Int)),
        Type::Record(Box::new(Type::Float)),
        Type::Task(Box::new(Type::Int)),
        Type::Task(Box::new(Type::Float)),
        Type::Generator(Box::new(Type::String)),
        Type::Nullable(Box::new(Type::Int)),
        Type::Nullable(Box::new(Type::Bool)),
        Type::Nullable(Box::new(Type::Void)),
        Type::Union(vec![]),
        Type::Union(vec![Type::Int, Type::Bool]),
        Type::Union(vec![Type::Bool, Type::Int]),
        Type::Union(vec![
            Type::Int,
            Type::Union(vec![Type::String, Type::Int]),
            Type::Null,
        ]),
        function(ParameterPassing::Value, None),
        function(ParameterPassing::MutableReference, None),
        function(ParameterPassing::Value, Some(DefaultValue::Int(3))),
        function(ParameterPassing::Value, Some(DefaultValue::Int(4))),
        function(
            ParameterPassing::Value,
            Some(DefaultValue::Array(vec![
                DefaultValue::String("nested default"),
                DefaultValue::Float(0x8000_0000_0000_0000),
            ])),
        ),
        Type::GenericFunction(GenericFunctionType {
            type_params: vec!["T"],
            signature: FunctionType::new(FunctionSignature {
                params: vec![FunctionParameter::value(Type::TypeParameter("T"))],
                return_type: Box::new(Type::TypeParameter("T")),
            }),
        }),
    ];
    let mut left = Type::Union(vec![Type::Int, Type::Bool]);
    let mut right = Type::Union(vec![Type::Bool, Type::Int]);
    for _ in 0..5 {
        left = Type::Array(Box::new(left));
        right = Type::Array(Box::new(right));
        types.push(left.clone());
        types.push(right.clone());
    }
    types
}

#[test]
fn shared_binary_and_common_types_match_frozen_prior_rules_and_diagnostics() {
    let operations = [
        BinaryOp::Add,
        BinaryOp::Sub,
        BinaryOp::Mul,
        BinaryOp::Div,
        BinaryOp::Mod,
        BinaryOp::BitAnd,
        BinaryOp::BitOr,
        BinaryOp::Xor,
        BinaryOp::ShiftLeft,
        BinaryOp::ShiftRight,
        BinaryOp::UnsignedShiftRight,
        BinaryOp::Eq,
        BinaryOp::NotEq,
        BinaryOp::Less,
        BinaryOp::LessEq,
        BinaryOp::Greater,
        BinaryOp::GreaterEq,
        BinaryOp::And,
        BinaryOp::Or,
        BinaryOp::Nullish,
    ];
    let types = corpus();
    let span = Span::new(17, 29);
    for (i, lhs) in types.iter().enumerate() {
        assert_eq!(
            is_stringable_plain(lhs),
            old::old_is_stringable(lhs),
            "stringable {i}"
        );
        for (j, rhs) in types.iter().enumerate() {
            assert_eq!(
                common_type_plain(lhs, rhs),
                old::old_common_type(lhs, rhs),
                "common {i}/{j}"
            );
            assert_eq!(
                equality_comparable_plain(lhs, rhs),
                old::old_equality_comparable(lhs, rhs),
                "equality {i}/{j}"
            );
            for op in operations {
                assert_eq!(
                    checked_binary_type_plain(op, lhs, rhs, span),
                    old::old_checked_binary_type(op, lhs, rhs, span),
                    "binary {op:?} {i}/{j}",
                );
            }
        }
    }
}

#[test]
fn normalization_keeps_source_order_nullability_and_nominal_identity() {
    let types = corpus();
    for left in &types {
        for right in &types {
            let input = vec![
                Type::Union(vec![
                    left.clone(),
                    Type::Union(vec![right.clone(), left.clone()]),
                ]),
                Type::Null,
            ];
            assert_eq!(
                normalize_union_plain(input.clone()),
                old::old_normalize_union(input)
            );
        }
    }
    let first = StructType {
        identity: NominalId::new(0, false),
        name: "Original",
    };
    let alias = StructType {
        identity: first.identity,
        name: "Alias",
    };
    assert!(matches!(
        normalize_union_plain(vec![Type::Struct(first), Type::Struct(alias)]),
        Type::Struct(StructType {
            name: "Original",
            ..
        })
    ));
    assert_eq!(normalize_union_plain(vec![]), Type::Union(vec![]));
    assert_eq!(
        normalize_union_plain(vec![Type::Null, Type::Int]),
        Type::Nullable(Box::new(Type::Int))
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Work(usize),
    Equality,
    OtherRelation,
    Clone,
    Box,
    Push { grows: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    Work,
    Memory,
    Capacity,
}

#[derive(Default)]
struct Probe {
    events: Vec<Event>,
    fail: Option<(usize, Refusal)>,
    constructed: usize,
}
impl Probe {
    fn before(&mut self, event: Event) -> Result<(), Refusal> {
        let position = self.events.len();
        self.events.push(event);
        if let Some((stop, error)) = self.fail {
            if stop == position {
                return Err(error);
            }
        }
        Ok(())
    }
}
impl RelationAdmission for Probe {
    type Error = Refusal;
    fn admit(&mut self, event: RelationEvent<'_, '_>) -> Result<(), Self::Error> {
        self.before(match event {
            RelationEvent::TypeWork(units) => Event::Work(units),
            RelationEvent::TypeEquality { .. } => Event::Equality,
            _ => Event::OtherRelation,
        })
    }
}
impl TypeConstructionAdmission for Probe {
    fn clone_type<'src>(&mut self, value: &Type<'src>) -> Result<Type<'src>, Self::Error> {
        self.before(Event::Clone)?;
        self.constructed += 1;
        Ok(value.clone())
    }
    fn box_type<'src>(&mut self, value: Type<'src>) -> Result<Box<Type<'src>>, Self::Error> {
        self.before(Event::Box)?;
        self.constructed += 1;
        Ok(Box::new(value))
    }
    fn push_type<'src>(
        &mut self,
        values: &mut Vec<Type<'src>>,
        value: Type<'src>,
    ) -> Result<(), Self::Error> {
        self.before(Event::Push {
            grows: values.len() == values.capacity(),
        })?;
        self.constructed += 1;
        values.push(value);
        Ok(())
    }
}

fn nullish_operands() -> (Type<'static>, Type<'static>) {
    (
        Type::Nullable(Box::new(Type::Array(Box::new(Type::Union(vec![
            Type::Int,
            Type::String,
            Type::Nullable(Box::new(Type::Bool)),
        ]))))),
        Type::Array(Box::new(Type::Union(vec![
            Type::String,
            Type::Null,
            Type::Class("Object"),
            Type::Int,
        ]))),
    )
}

#[test]
fn every_query_and_construction_refusal_remains_a_refusal_before_the_operation() {
    let (lhs, rhs) = nullish_operands();
    let mut complete = Probe::default();
    let expected = checked_binary_type_with(BinaryOp::Nullish, &lhs, &rhs, &mut complete).unwrap();
    assert_eq!(
        expected,
        old::old_checked_binary_type(BinaryOp::Nullish, &lhs, &rhs, Span::empty(0)).unwrap()
    );
    assert!(complete
        .events
        .iter()
        .any(|event| matches!(event, Event::Push { grows: true })));
    assert!(complete.events.contains(&Event::Clone));
    assert!(complete.events.contains(&Event::Box));
    assert!(complete.events.contains(&Event::Equality));
    for stop in 0..complete.events.len() {
        for refusal in [Refusal::Work, Refusal::Memory, Refusal::Capacity] {
            let mut probe = Probe {
                fail: Some((stop, refusal)),
                ..Probe::default()
            };
            assert_eq!(
                checked_binary_type_with(BinaryOp::Nullish, &lhs, &rhs, &mut probe),
                Err(BinaryTypeError::Admission(refusal))
            );
            assert_eq!(probe.events, complete.events[..=stop]);
            let completed_constructions = complete.events[..stop]
                .iter()
                .filter(|event| matches!(event, Event::Clone | Event::Box | Event::Push { .. }))
                .count();
            assert_eq!(
                probe.constructed, completed_constructions,
                "construction executed after refusal at {stop}"
            );
        }
    }
    // The borrowed source types survive every partial temporary result path.
    assert_eq!((lhs, rhs), nullish_operands());
}

#[test]
fn scalar_checks_and_lazy_rejections_need_no_type_construction() {
    let mut probe = Probe::default();
    assert_eq!(
        checked_binary_type_with(BinaryOp::Add, &Type::Int, &Type::Float, &mut probe),
        Ok(Type::Float)
    );
    assert_eq!(probe.constructed, 0);
    let long_name = "long-diagnostic-name-".repeat(4096);
    let named = Type::Class(&long_name);
    assert_eq!(
        checked_binary_type_with(BinaryOp::Mul, &named, &Type::String, &mut probe),
        Err(BinaryTypeError::Semantic(BinaryTypeReason::InvalidOperands))
    );
    assert_eq!(
        checked_binary_type_with(BinaryOp::Nullish, &named, &Type::String, &mut probe),
        Err(BinaryTypeError::Semantic(
            BinaryTypeReason::NullishLeftRequired
        ))
    );
    assert_eq!(probe.constructed, 0);
    let span = Span::new(4, 8);
    assert_eq!(
        checked_binary_type_plain(BinaryOp::Mul, &named, &Type::String, span),
        old::old_checked_binary_type(BinaryOp::Mul, &named, &Type::String, span)
    );
    let mut denied = Probe {
        fail: Some((0, Refusal::Work)),
        ..Probe::default()
    };
    assert_eq!(
        checked_binary_type_with(BinaryOp::Mul, &named, &Type::String, &mut denied),
        Err(BinaryTypeError::Admission(Refusal::Work))
    );
}

#[test]
fn union_candidates_and_repeated_common_queries_are_paid_each_time() {
    let left = Type::Union(vec![Type::Class("A"), Type::Class("B"), Type::Class("C")]);
    let right = Type::Union(vec![Type::Class("X"), Type::Class("Y"), Type::Class("Z")]);
    let mut probe = Probe::default();
    assert!(!equality_comparable_with(&left, &right, &mut probe).unwrap());
    assert_eq!(
        probe
            .events
            .iter()
            .filter(|event| **event == Event::Equality)
            .count(),
        9
    );
    let first_count = probe.events.len();
    assert!(!equality_comparable_with(&left, &right, &mut probe).unwrap());
    assert_eq!(probe.events.len(), first_count * 2);
    let mut one = Probe::default();
    let result = common_type_with(&left, &right, &mut one).unwrap();
    let first = one.events.clone();
    drop(result);
    let result = common_type_with(&left, &right, &mut one).unwrap();
    assert_eq!(&one.events[first.len()..], &first);
    drop(result);
}
