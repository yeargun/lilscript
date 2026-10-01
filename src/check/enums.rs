//! Closed enum domains. Declaration ordinals never select an externally
//! observable spelling; explicit ABI values are the canonical representation.
use super::*;
use crate::ast::{EnumAbi, EnumLiteral};
use crate::primitive::EnumOperation;

fn holds_ordinal(
    ty: &Type<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    use super::type_payload::{Payload, PayloadError};
    let mut found = false;
    let result = super::type_payload::measure_payload(Payload::Type(ty), budget, |node| {
        if let Payload::Type(Type::Enum(declaration)) = node {
            found |= declaration.abi == EnumAbi::Ordinal;
        }
        Ok::<_, std::convert::Infallible>(())
    });
    match result {
        Ok(_) => Ok(found),
        Err(PayloadError::Allocation(error)) => Err(error),
        Err(PayloadError::Visitor(never)) => match never {},
    }
}

pub(super) fn check(
    view: CheckedView<'_, '_, '_>,
    module: Option<usize>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AdmittedCheckError> {
    let refuse = |span| {
        AdmittedCheckError::new(span, "observing this enum requires a declared `int` or `string` ABI; use `.ordinal` for its declaration index (R8)")
    };
    for definition in &view.declarations.enums {
        budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
        if definition.module == module
            && definition.declaration.abi == EnumAbi::Ordinal
            && view.is_reflected(definition.declaration.identity)
        {
            return Err(refuse(definition.span));
        }
    }
    for info in &view.facts.source_info {
        let Some(expression) = info.expression else {
            continue;
        };
        budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
        let mut check = |value: &Expr<'_, '_>| -> Result<(), AdmittedCheckError> {
            if let Some(ty) = view.expression_type(value.id) {
                if holds_ordinal(ty, budget)? {
                    return Err(refuse(value.span()));
                }
            }
            Ok(())
        };
        match &expression.kind {
            ExprKind::Convert { value, .. }
            | ExprKind::DynamicUnary {
                op: DynamicUnaryOp::TypeOf,
                expr: value,
                ..
            } => check(value)?,
            ExprKind::Template { parts, .. } => {
                for part in *parts {
                    if let TemplatePart::Expr(value) = part {
                        check(value)?;
                    }
                }
            }
            ExprKind::Binary {
                op: BinaryOp::Add,
                lhs,
                rhs,
                ..
            } if view.expression_type(expression.id) == Some(&Type::String) => {
                check(lhs)?;
                check(rhs)?;
            }
            ExprKind::Call { args, .. }
                if matches!(
                    view.builtin_call(expression.id),
                    Some(
                        BuiltinCall::Print
                            | BuiltinCall::JsonStringify
                            | BuiltinCall::JsString
                            | BuiltinCall::JsNumber
                    )
                ) =>
            {
                for argument in *args {
                    check(&argument.expression)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

impl<'ast, 'src> Analyzer<'_, '_, 'ast, 'src> {
    pub(super) fn define_enums(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), AdmittedCheckError> {
        for item in program.items {
            let Item::Enum(declaration) = item else {
                continue;
            };
            let mut variants = IndexMap::new();
            let mut values = self
                .budget
                .vector(AllocationClass::Scratch, declaration.variants.len())?;
            let mut next = 0i64;
            let mut flag_mask = 0u32;
            for (ordinal, variant) in declaration.variants.iter().enumerate() {
                let span = variant.name.span;
                if variants.insert(variant.name.name, ordinal as i64).is_some() {
                    return Err(AdmittedCheckError::new(
                        span,
                        format!(
                            "duplicate variant `{}` in enum `{}`",
                            variant.name.name, declaration.name.name
                        ),
                    ));
                }
                let value = match (declaration.abi, variant.value) {
                    (EnumAbi::Ordinal, None) => ShapeTag::Int(
                        i32::try_from(ordinal)
                            .map_err(|_| AdmittedCheckError::new(span, "too many enum variants"))?,
                    ),
                    (EnumAbi::Ordinal, Some(_)) => {
                        return Err(AdmittedCheckError::new(
                            span,
                            "explicit enum values require an `int` or `string` ABI",
                        ))
                    }
                    (EnumAbi::String, Some(EnumLiteral::String(text, _))) => ShapeTag::String(text),
                    (EnumAbi::String, _) => {
                        return Err(AdmittedCheckError::new(
                            span,
                            "every string ABI variant needs a string literal value",
                        ))
                    }
                    (EnumAbi::Int | EnumAbi::Flags, literal) => {
                        let raw = match literal {
                            Some(EnumLiteral::Int(value, _)) => value,
                            Some(_) => {
                                return Err(AdmittedCheckError::new(
                                    span,
                                    "an integer ABI variant needs an integer value",
                                ))
                            }
                            None if declaration.abi == EnumAbi::Flags => {
                                if ordinal >= 32 {
                                    return Err(AdmittedCheckError::new(
                                        span,
                                        "flag sets have at most 32 independent bits",
                                    ));
                                }
                                i64::from((1u32 << ordinal) as i32)
                            }
                            None => next,
                        };
                        let value = i32::try_from(raw).map_err(|_| {
                            AdmittedCheckError::new(span, "enum ABI value is outside int32")
                        })?;
                        next = raw + 1;
                        if declaration.abi == EnumAbi::Flags {
                            let bit = value as u32;
                            if !bit.is_power_of_two() || bit & flag_mask != 0 {
                                return Err(AdmittedCheckError::new(
                                    span,
                                    "each declared flag must name a distinct nonzero bit",
                                ));
                            }
                            flag_mask |= bit;
                        }
                        ShapeTag::Int(value)
                    }
                };
                let mut query = type_admission::TypeQueryAdmission::new(self.budget);
                // Decode even a sole string declaration, retaining its source span.
                value.same(value, span, &mut query)?;
                for &previous in &values {
                    if value.same(previous, span, &mut query)? {
                        return Err(AdmittedCheckError::new(
                            span,
                            "enum ABI values must be unique",
                        ));
                    }
                }
                self.budget
                    .push(AllocationClass::Scratch, &mut values, value)?;
            }
            let identity = self.facts.type_bindings[declaration.name.name];
            let info = &mut self.declarations.enums[identity.index()];
            info.variants = variants;
            info.values = values;
            info.flag_mask = flag_mask;
        }
        Ok(())
    }

    pub(super) fn analyze_enum_from(
        &mut self,
        callee: &'ast Expr<'ast, 'src>,
        args: &'ast [Argument<'ast, 'src>],
        id: SourceNodeId,
        span: Span,
    ) -> Result<Option<Type<'src>>, AdmittedCheckError> {
        let ExprKind::Member {
            object, property, ..
        } = &callee.kind
        else {
            return Ok(None);
        };
        let ExprKind::Ident(name) = &object.kind else {
            return Ok(None);
        };
        if !self.builtin_namespace_is_unshadowed(name.name) {
            return Ok(None);
        }
        let Some(&identity) = self
            .facts
            .type_bindings
            .get(name.name)
            .filter(|id| id.is_enum())
        else {
            return Ok(None);
        };
        if property.name != "from" {
            return Ok(None);
        }
        let declaration = self.declarations.enums[identity.index()].declaration;
        if declaration.abi == EnumAbi::Ordinal {
            return Err(AdmittedCheckError::new(
                span,
                "`from` requires an enum with a declared ABI",
            ));
        }
        if args.len() != 1 {
            return Err(AdmittedCheckError::new(
                span,
                "enum `from` expects one ABI value",
            ));
        }
        let expected = declaration.primitive();
        let actual = self.analyze_value_argument(&args[0], Some(&expected))?;
        self.require_assignable(&expected, &actual, args[0].span)?;
        self.facts.source_info[id.index()].resolution = ExpressionResolution::Enum {
            declaration: identity,
            operation: EnumOperation::From,
        };
        Ok(Some(Type::nullable(Box::new(Type::Enum(declaration)))))
    }

    pub(super) fn enum_member(
        &mut self,
        declaration: EnumType<'src>,
        property: Ident<'src>,
        id: SourceNodeId,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let (operation, result) = match property.name {
            "abi" if declaration.abi != EnumAbi::Ordinal => {
                (EnumOperation::Abi, declaration.primitive())
            }
            "ordinal" if !declaration.is_flags() => (EnumOperation::Ordinal, Type::Int),
            "has" if declaration.is_flags() => (
                EnumOperation::Has,
                Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(Type::Enum(declaration))],
                    return_type: Box::new(Type::Bool),
                })),
            ),
            "abi" => {
                return Err(AdmittedCheckError::new(
                    property.span,
                    "this enum has no declared ABI; use `.ordinal`",
                ))
            }
            _ => {
                return Err(AdmittedCheckError::new(
                    property.span,
                    format!(
                        "enum `{}` has no member `{}`",
                        declaration.name, property.name
                    ),
                ))
            }
        };
        self.facts.source_info[id.index()].resolution = ExpressionResolution::Enum {
            declaration: declaration.identity,
            operation,
        };
        Ok(result)
    }
}
