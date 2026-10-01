//! Canonical ABI enum operations. Helpers own repeated comparisons so each
//! source operand is evaluated once, including effectful `.from` arguments.
use super::*;
use crate::primitive::EnumOperation;

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    fn enum_membership(
        &mut self,
        declaration: NominalId,
        binding: js::BindingId,
    ) -> Result<js::ExprId, FormationError> {
        let definition = self
            .program
            .enum_definition(declaration)
            .ok_or_else(|| self.error(Span::default(), "enum domain is missing"))?;
        if definition.abi == crate::ast::EnumAbi::Flags {
            let value = self.reference(binding)?;
            let kind = self.expression(js::Expr::Unary {
                op: js::Unary::TypeOf,
                value,
            })?;
            let number = self.string(&"number".into())?;
            let number = self.literal(js::Literal::String(number))?;
            let kind = self.expression(js::Expr::Binary {
                op: js::Binary::StrictEqual,
                left: kind,
                right: number,
            })?;
            let value = self.reference(binding)?;
            let integer = self.expression(js::Expr::ToInt32(value))?;
            let value = self.reference(binding)?;
            let integer = self.expression(js::Expr::Binary {
                op: js::Binary::StrictEqual,
                left: integer,
                right: value,
            })?;
            let left = self.expression(js::Expr::Binary {
                op: js::Binary::And,
                left: kind,
                right: integer,
            })?;
            let value = self.reference(binding)?;
            let mask = self.literal(js::Literal::Number((!definition.flag_mask as i32) as f64))?;
            let bits = self.expression(js::Expr::Binary {
                op: js::Binary::BitAnd,
                left: value,
                right: mask,
            })?;
            let zero = self.literal(js::Literal::Number(0.0))?;
            let right = self.expression(js::Expr::Binary {
                op: js::Binary::StrictEqual,
                left: bits,
                right: zero,
            })?;
            return self.expression(js::Expr::Binary {
                op: js::Binary::And,
                left,
                right,
            });
        }
        let mut condition = None;
        for variant in &definition.variants {
            self.work(1)?;
            let left = self.reference(binding)?;
            let right = self.shape_tag_literal(variant.value)?;
            let equal = self.expression(js::Expr::Binary {
                op: js::Binary::StrictEqual,
                left,
                right,
            })?;
            condition = Some(match condition {
                Some(left) => self.expression(js::Expr::Binary {
                    op: js::Binary::Or,
                    left,
                    right: equal,
                })?,
                None => equal,
            });
        }
        condition.ok_or_else(|| self.error(Span::default(), "empty enum domain"))
    }

    pub(super) fn enum_operation(
        &mut self,
        declaration: NominalId,
        operation: EnumOperation,
        arguments: &[js::ExprId],
    ) -> Result<js::ExprId, FormationError> {
        if operation == EnumOperation::Abi
            || operation == EnumOperation::Ordinal
                && self
                    .program
                    .enum_definition(declaration)
                    .is_some_and(|d| d.abi == crate::ast::EnumAbi::Ordinal)
        {
            return Ok(arguments[0]);
        }
        let key = shapes::Helper::Enum(declaration, operation);
        let helper = if let Some(helper) = self.shape_helper(&key)? {
            helper
        } else {
            let (helper, body, parameters) = self.shape_helper_start(
                key,
                if operation == EnumOperation::Has {
                    2
                } else {
                    1
                },
            )?;
            let result = match operation {
                EnumOperation::From => {
                    let condition = self.enum_membership(declaration, parameters[0])?;
                    let yes = self.reference(parameters[0])?;
                    let no = self.literal(js::Literal::Null)?;
                    self.expression(js::Expr::Conditional { condition, yes, no })?
                }
                EnumOperation::Has => {
                    let left = self.reference(parameters[0])?;
                    let right = self.reference(parameters[1])?;
                    let left = self.expression(js::Expr::Binary {
                        op: js::Binary::BitAnd,
                        left,
                        right,
                    })?;
                    let right = self.reference(parameters[1])?;
                    self.expression(js::Expr::Binary {
                        op: js::Binary::StrictEqual,
                        left,
                        right,
                    })?
                }
                EnumOperation::Ordinal => {
                    let definition = self.program.enum_definition(declaration).unwrap();
                    let mut result =
                        self.literal(js::Literal::Number((definition.variants.len() - 1) as f64))?;
                    for (ordinal, variant) in definition.variants.iter().enumerate().rev().skip(1) {
                        self.work(1)?;
                        let left = self.reference(parameters[0])?;
                        let right = self.shape_tag_literal(variant.value)?;
                        let condition = self.expression(js::Expr::Binary {
                            op: js::Binary::StrictEqual,
                            left,
                            right,
                        })?;
                        let yes = self.literal(js::Literal::Number(ordinal as f64))?;
                        result = self.expression(js::Expr::Conditional {
                            condition,
                            yes,
                            no: result,
                        })?;
                    }
                    result
                }
                EnumOperation::Abi => unreachable!(),
            };
            self.statement(body, js::Statement::Return(Some(result)))?;
            self.shape_helper_finish(helper, body, parameters)?;
            helper
        };
        self.shape_call(helper, arguments)
    }

    pub(super) fn enum_crossing(
        &mut self,
        ty: &Type<'src>,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if self.contract.checks != crate::compilation_contract::PreconditionChecks::Development {
            return Ok(value);
        }
        let (declaration, optional) = match ty {
            Type::Enum(declaration) => (declaration.identity, false),
            Type::Nullable(inner) => match inner.as_ref() {
                Type::Enum(declaration) => (declaration.identity, true),
                _ => return Ok(value),
            },
            _ => return Ok(value),
        };
        let key = shapes::Helper::EnumCheck(declaration, optional);
        let helper = if let Some(helper) = self.shape_helper(&key)? {
            helper
        } else {
            let (helper, body, parameters) = self.shape_helper_start(key, 1)?;
            let mut valid = self.enum_membership(declaration, parameters[0])?;
            if optional {
                let left = self.reference(parameters[0])?;
                let right = self.literal(js::Literal::Null)?;
                let absent = self.expression(js::Expr::Binary {
                    op: js::Binary::Equal,
                    left,
                    right,
                })?;
                valid = self.expression(js::Expr::Binary {
                    op: js::Binary::Or,
                    left: absent,
                    right: valid,
                })?;
            }
            let invalid = self.expression(js::Expr::Unary {
                op: js::Unary::Not,
                value: valid,
            })?;
            self.boundary_reject(body, invalid, "value is outside the declared enum ABI (R8)")?;
            let value = self.reference(parameters[0])?;
            self.statement(body, js::Statement::Return(Some(value)))?;
            self.shape_helper_finish(helper, body, parameters)?;
            helper
        };
        self.shape_call(helper, &[value])
    }
}
