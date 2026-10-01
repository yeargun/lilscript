//! Direct source execution, independent of program folding and target code.
use super::*;
use crate::check::{ExpressionResolution, NominalId, ShapeTag};
use crate::primitive::EnumOperation;

impl<'program, 'ast, 'src> ReferenceInterpreter<'program, 'ast, 'src> {
    pub(super) fn enum_value(
        &self,
        id: NominalId,
        ordinal: usize,
        span: Span,
    ) -> Result<Value, InterpretError> {
        let value = self
            .semantics
            .nominal_enum(id)
            .and_then(|info| info.values.get(ordinal))
            .ok_or_else(|| InterpretError::new(span, "unknown enum variant"))?;
        Self::tag_value(*value, span)
    }

    fn tag_value(value: ShapeTag<'_>, span: Span) -> Result<Value, InterpretError> {
        Ok(match value {
            ShapeTag::Int(value) => Value::Int(value),
            ShapeTag::Bool(value) => Value::Bool(value),
            ShapeTag::String(value) => string_value(decode_source_units(value, span)?),
        })
    }

    pub(super) fn evaluate_enum(
        &mut self,
        expression: &Expr<'ast, 'src>,
    ) -> Result<Option<Value>, InterpretError> {
        let resolution = self.semantics.view().expression_resolution(expression.id);
        let (id, operation, left, right) = match (resolution, &expression.kind) {
            (
                ExpressionResolution::Enum {
                    declaration,
                    operation,
                },
                ExprKind::Member { object, .. } | ExprKind::OptionalMember { object, .. },
            ) => {
                let value = self.evaluate(object)?;
                if matches!(
                    (&expression.kind, &value),
                    (ExprKind::OptionalMember { .. }, Value::Null)
                ) {
                    return Ok(Some(Value::Null));
                }
                (declaration, operation, value, None)
            }
            (
                ExpressionResolution::Enum {
                    declaration,
                    operation,
                },
                ExprKind::Call { args, .. },
            ) => (
                declaration,
                operation,
                self.evaluate(&args[0].expression)?,
                None,
            ),
            (_, ExprKind::Call { callee, args, .. }) => {
                let ExpressionResolution::Enum {
                    declaration,
                    operation,
                } = self.semantics.view().expression_resolution(callee.id)
                else {
                    return Ok(None);
                };
                let ExprKind::Member { object, .. } = &callee.kind else {
                    return Ok(None);
                };
                let left = self.evaluate(object)?;
                (
                    declaration,
                    operation,
                    left,
                    Some(self.evaluate(&args[0].expression)?),
                )
            }
            _ => return Ok(None),
        };
        self.step(expression.span())?;
        if operation == EnumOperation::Abi {
            return Ok(Some(left));
        }
        if operation == EnumOperation::Has {
            let (Value::Int(value), Some(Value::Int(bits))) = (left, right) else {
                return Err(InterpretError::new(
                    expression.span(),
                    "invalid flag operands",
                ));
            };
            return Ok(Some(Value::Bool(value & bits == bits)));
        }
        let definition = self.semantics.nominal_enum(id).unwrap();
        if definition.declaration.is_flags() {
            let Value::Int(bits) = left else {
                return Err(InterpretError::new(expression.span(), "invalid flag value"));
            };
            return Ok(Some(if (bits as u32) & !definition.flag_mask == 0 {
                Value::Int(bits)
            } else {
                Value::Null
            }));
        }
        for ordinal in 0..definition.values.len() {
            self.step(expression.span())?;
            let candidate = self.enum_value(id, ordinal, expression.span())?;
            if values_equal(&left, &candidate) {
                return Ok(Some(if operation == EnumOperation::Ordinal {
                    Value::Int(ordinal as i32)
                } else {
                    candidate
                }));
            }
        }
        Ok(Some(Value::Null))
    }

    pub(super) fn payload_matches(
        &mut self,
        value: &Value,
        name: &str,
    ) -> Result<bool, InterpretError> {
        let declaration = self
            .class_declaration(name)
            .ok_or_else(|| InterpretError::new(Span::default(), "unknown payload type"))?;
        if declaration.shape {
            let fields = match value {
                Value::Record(fields) => fields,
                Value::Instance(value) => &value.fields,
                _ => return Ok(false),
            };
            for member in declaration.members {
                if let crate::ast::ClassMember::Field(field) = member {
                    if field.discriminant {
                        let literal = field.initializer.as_ref().unwrap();
                        let expected = self.evaluate(literal)?;
                        return Ok(fields
                            .borrow()
                            .get(field.name.name)
                            .is_some_and(|actual| values_equal(actual, &expected)));
                    }
                }
            }
            return Err(InterpretError::new(
                declaration.span,
                "payload shape has no tag",
            ));
        }
        let Value::Instance(value) = value else {
            return Ok(false);
        };
        let mut current = self.class_declaration(&value.name);
        while let Some(class) = current {
            self.step(class.span)?;
            if class.name.name == name {
                return Ok(true);
            }
            current = match class.base {
                Some(crate::ast::TypeRef {
                    kind: TypeKind::Named { name, .. },
                    ..
                }) => self.class_declaration(name),
                _ => None,
            };
        }
        Ok(false)
    }
}
