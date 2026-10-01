//! Primitive result transfer for formed expressions, including generated
//! helpers. Source semantic proofs stay in program facts; target syntax and
//! the shared host catalog supply conservative facts for physical recipes.
use super::*;
pub(super) use crate::catalog::HostResult as Known;

impl Module {
    /// The primitive type `id` is known to produce, when every evaluation
    /// that completes yields that type.
    pub(super) fn known(&self, id: ExprId) -> Option<Known> {
        self.known_within(id, 64)
    }

    /// `known`, looking at most `depth` operators deep: a long sum is
    /// usually decided by its first string operand.
    fn known_within(&self, id: ExprId, depth: u32) -> Option<Known> {
        let depth = depth.checked_sub(1)?;
        let known = |id: ExprId| self.known_within(id, depth);
        let node = &self.expressions[id.index()];
        match node {
            Expr::Literal(Literal::Number(_))
            | Expr::ToInt32(_)
            | Expr::IntBinary { .. }
            | Expr::IntNegate(_)
            | Expr::Unary {
                op: Unary::Plus, ..
            } => Some(Known::Number),
            Expr::Literal(Literal::Bool(_)) | Expr::Unary { op: Unary::Not, .. } => {
                Some(Known::Boolean)
            }
            Expr::Literal(Literal::String(_))
            | Expr::Template(_)
            | Expr::Unary {
                op: Unary::TypeOf, ..
            } => Some(Known::String),
            // A BigInt operand keeps a BigInt; a number operand keeps a number.
            Expr::Unary {
                op: Unary::Negate | Unary::BitNot,
                value,
            } => (known(*value) == Some(Known::Number)).then_some(Known::Number),
            Expr::Binary { op, left, right } => match op {
                Binary::Less
                | Binary::LessEqual
                | Binary::Greater
                | Binary::GreaterEqual
                | Binary::StrictEqual
                | Binary::StrictNotEqual
                | Binary::Equal
                | Binary::NotEqual
                | Binary::In
                | Binary::InstanceOf => Some(Known::Boolean),
                // BigInt `>>>` throws, so a completed one is a Number.
                Binary::UnsignedShiftRight => Some(Known::Number),
                Binary::Subtract
                | Binary::Multiply
                | Binary::Divide
                | Binary::Remainder
                | Binary::ShiftLeft
                | Binary::ShiftRight
                | Binary::BitAnd
                | Binary::BitOr
                | Binary::BitXor => (known(*left) == Some(Known::Number)
                    && known(*right) == Some(Known::Number))
                .then_some(Known::Number),
                Binary::Add => {
                    let (left, right) = (known(*left), known(*right));
                    if left == Some(Known::String) || right == Some(Known::String) {
                        Some(Known::String)
                    } else if left == Some(Known::Number) && right == Some(Known::Number) {
                        Some(Known::Number)
                    } else {
                        None
                    }
                }
                // `&&`/`||` produce one of their operands.
                Binary::And | Binary::Or => {
                    let left = known(*left)?;
                    (known(*right) == Some(left)).then_some(left)
                }
                Binary::Nullish => None,
            },
            Expr::Conditional { yes, no, .. } => {
                let yes = known(*yes)?;
                (known(*no) == Some(yes)).then_some(yes)
            }
            Expr::Call {
                callee, arguments, ..
            } if self.pristine_builtins => self.builtin_result(*callee, arguments.len()),
            _ => None,
        }
    }

    pub(super) fn literal_property<'a>(&'a self, property: &'a Property) -> Option<&'a str> {
        match property {
            Property::Named(name) => Some(name.as_str()),
            Property::Computed(key) => match &self.expressions[key.index()] {
                Expr::Literal(Literal::String(key)) => key.as_unicode(),
                _ => None,
            },
        }
    }

    fn builtin_result(&self, callee: ExprId, _arguments: usize) -> Option<Known> {
        let host = |id: ExprId| match &self.expressions[id.index()] {
            Expr::Host(Host {
                kind: crate::catalog::HostKind::Standard(global),
                ..
            }) => Some(*global),
            _ => None,
        };
        if let Some(global) = host(callee) {
            return crate::catalog::host_result(global, None);
        }
        let Expr::Member { object, property } = &self.expressions[callee.index()] else {
            return None;
        };
        let key = self.literal_property(property)?;
        if let Some(global) = host(*object) {
            return crate::catalog::host_result(global, Some(key));
        }
        if key != "call" {
            return None;
        }
        let Expr::Member {
            object: prototype,
            property,
        } = &self.expressions[object.index()]
        else {
            return None;
        };
        let key = self.literal_property(property)?;
        let Expr::Member {
            object: owner,
            property,
        } = &self.expressions[prototype.index()]
        else {
            return None;
        };
        if self.literal_property(property) != Some("prototype") {
            return None;
        }
        crate::catalog::host_prototype_result(host(*owner)?, key)
    }
}
