//! Exact operator simplifications on the finished target tree.
//!
//! Every rule here replaces an expression with one that evaluates the same
//! operands, in the same order, with the same effects, and produces the same
//! value. None consults a port, a spelling preference or a codec: each removes
//! work the value never needed, so it runs under target compaction for every
//! objective.
//!
//! * `!(a==b)` is `a!=b`, and so for `!=`, `===` and `!==`.
//! * `x===void 0||x==null` is `x==null` for a binding `x`, since `==null`
//!   already holds for `undefined`; `x!==void 0&&x!=null` is `x!=null`.
//! * `+e` is `e`, `!!e` is `e` and `e+""` is `e` when `e` already has that
//!   primitive type: numbers from numeric operators and, with pristine
//!   builtins, from `Math`, `Date.now`, `parseInt` and their kind; booleans from
//!   comparisons and boolean builtins; strings from literals, `typeof`, string
//!   sums and string builtins.
//! * `s+(e+"")` is `s+e` when `s` is a string: both convert `e` once, after
//!   `s` is evaluated and before the sum.
//! * `x.length|0` is `x.length` under the numeric-length assumption.
//! * `new RegExp("p","f")` of literal strings is `/p/f` under pristine
//!   builtins, when the pattern is in the subset whose literal and constructor
//!   forms mean the same (`js_regex`). Each evaluation still creates a fresh
//!   object, so the literal is never copied or shared.
use super::*;

/// The primitive type an expression's value is known to have.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Known {
    Number,
    Boolean,
    String,
}

/// `Math` members that return a Number for every argument they accept.
const MATH_NUMBERS: &[&str] = &[
    "abs", "acos", "acosh", "asin", "asinh", "atan", "atan2", "atanh", "cbrt", "ceil", "clz32",
    "cos", "cosh", "exp", "expm1", "floor", "fround", "hypot", "imul", "log", "log10", "log1p",
    "log2", "max", "min", "pow", "random", "round", "sign", "sin", "sinh", "sqrt", "tan", "tanh",
    "trunc",
];
/// Global functions returning a Number.
const GLOBAL_NUMBERS: &[&str] = &["parseInt", "parseFloat", "Number"];
/// Static members returning a Number, keyed by their host object.
const STATIC_NUMBERS: &[(&str, &str)] = &[
    ("Date", "now"),
    ("Number", "parseInt"),
    ("Number", "parseFloat"),
];
/// Global functions returning a Boolean.
const GLOBAL_BOOLEANS: &[&str] = &["isNaN", "isFinite", "Boolean"];
/// Static members returning a Boolean.
const STATIC_BOOLEANS: &[(&str, &str)] = &[
    ("Number", "isInteger"),
    ("Number", "isFinite"),
    ("Number", "isNaN"),
    ("Number", "isSafeInteger"),
    ("Array", "isArray"),
    ("Object", "is"),
    ("Object", "isFrozen"),
    ("Object", "isSealed"),
    ("Object", "isExtensible"),
    ("Object", "hasOwn"),
    ("Reflect", "has"),
    ("ArrayBuffer", "isView"),
];
/// Global functions returning a String.
const GLOBAL_STRINGS: &[&str] = &[
    "String",
    "encodeURIComponent",
    "encodeURI",
    "decodeURIComponent",
    "decodeURI",
    "escape",
    "unescape",
];
/// Static members returning a String.
const STATIC_STRINGS: &[(&str, &str)] = &[
    ("String", "fromCharCode"),
    ("String", "fromCodePoint"),
    ("String", "raw"),
];

impl Module {
    /// Apply the rules above until none applies. Returns the number of edits.
    /// `protected` (ascending) lists literals with an observed alternative:
    /// no rule rewrites an expression that holds one.
    pub(crate) fn simplify_operators(
        &mut self,
        numeric_lengths: bool,
        year: u16,
        protected: &[ExprId],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut edits = 0;
        // Children precede parents in the arena except after a splice, and a
        // rewrite can expose its parent's rule: a few sweeps reach the
        // fixed point on every tree the formation produces.
        for _ in 0..4 {
            budget.work(Analysis, self.expressions.len() as u64)?;
            let before = edits;
            for index in 0..self.expressions.len() {
                if self.holds_protected(ExprId::new(index), protected) {
                    continue;
                }
                if let Some(replacement) = self.simplified(ExprId::new(index), numeric_lengths, year)
                {
                    self.expressions[index] = replacement;
                    edits += 1;
                }
            }
            if edits == before {
                break;
            }
        }
        Ok(edits)
    }

    /// Whether `id`, a child or a grandchild is a protected literal: every
    /// rule reads at most two levels down.
    fn holds_protected(&self, id: ExprId, protected: &[ExprId]) -> bool {
        if protected.is_empty() {
            return false;
        }
        let hit = |id: ExprId| protected.binary_search(&id).is_ok();
        let mut found = hit(id);
        let _ = self.expressions[id.index()].visit_children(|child| {
            found |= hit(child);
            self.expressions[child.index()].visit_children(|grandchild| {
                found |= hit(grandchild);
                Ok::<_, ()>(())
            })
        });
        found
    }

    fn simplified(&self, id: ExprId, numeric_lengths: bool, year: u16) -> Option<Expr> {
        let es2018 = year >= 2018;
        let node = |id: ExprId| &self.expressions[id.index()];
        let same = |a: ExprId, b: ExprId| {
            matches!((node(a), node(b)), (Expr::Binding(x), Expr::Binding(y)) if x == y)
        };
        // `x===void 0?null:x`, `x==null?d:x` and `x!=null?x:d` are `x??…` for
        // a binding `x`: one read of it instead of two, and `d` runs exactly
        // when `x` is null or undefined (`null` in the first, whichever `x`
        // is null or undefined, gives `null`).
        if year >= 2020 {
            if let Expr::Conditional { condition, yes, no } = node(id) {
                if let Expr::Binary { op, left, right } = node(*condition) {
                    let tested = match (node(*left), node(*right)) {
                        (Expr::Binding(_), Expr::Literal(literal)) => Some((*left, literal)),
                        (Expr::Literal(literal), Expr::Binding(_)) => Some((*right, literal)),
                        _ => None,
                    };
                    if let Some((tested, literal)) = tested {
                        let nullish = matches!(literal, Literal::Null | Literal::Undefined);
                        let (value, fallback) = match op {
                            Binary::StrictEqual
                                if matches!(literal, Literal::Undefined)
                                    && matches!(node(*yes), Expr::Literal(Literal::Null)) =>
                            {
                                (*no, *yes)
                            }
                            Binary::Equal if nullish => (*no, *yes),
                            Binary::NotEqual if nullish => (*yes, *no),
                            _ => (tested, tested),
                        };
                        if value != tested && same(value, tested) {
                            return Some(Expr::Binary {
                                op: Binary::Nullish,
                                left: value,
                                right: fallback,
                            });
                        }
                    }
                }
            }
        }
        match node(id) {
            Expr::Unary {
                op: Unary::Not,
                value,
            } => match node(*value) {
                Expr::Binary { op, left, right } => {
                    let op = match op {
                        Binary::Equal => Binary::NotEqual,
                        Binary::NotEqual => Binary::Equal,
                        Binary::StrictEqual => Binary::StrictNotEqual,
                        Binary::StrictNotEqual => Binary::StrictEqual,
                        _ => return None,
                    };
                    Some(Expr::Binary {
                        op,
                        left: *left,
                        right: *right,
                    })
                }
                Expr::Unary {
                    op: Unary::Not,
                    value: inner,
                } if self.known(*inner) == Some(Known::Boolean) => Some(node(*inner).clone()),
                _ => None,
            },
            Expr::Unary {
                op: Unary::Plus,
                value,
            } if self.known(*value) == Some(Known::Number) => Some(node(*value).clone()),
            Expr::ToInt32(value) if numeric_lengths && self.is_length(*value) => {
                Some(node(*value).clone())
            }
            Expr::Binary {
                op: op @ (Binary::Or | Binary::And),
                left,
                right,
            } => self.nullish_pair(*op, *left, *right),
            // Operands of one primitive type compare alike either way:
            // `typeof x==="string"` is `typeof x=="string"`.
            Expr::Binary {
                op: op @ (Binary::StrictEqual | Binary::StrictNotEqual),
                left,
                right,
            } if self.known(*left).is_some() && self.known(*left) == self.known(*right) => {
                Some(Expr::Binary {
                    op: if *op == Binary::StrictEqual {
                        Binary::Equal
                    } else {
                        Binary::NotEqual
                    },
                    left: *left,
                    right: *right,
                })
            }
            Expr::Binary {
                op: Binary::Add,
                left,
                right,
            } => {
                if self.empty_string(*right) && self.known(*left) == Some(Known::String) {
                    return Some(node(*left).clone());
                }
                // `s+(e+"")`: `s` is a string, so the sum converts `e` just as
                // the inner sum did, at the same point.
                if self.known(*left) == Some(Known::String) {
                    if let Some(value) = self.string_conversion(*right) {
                        return Some(Expr::Binary {
                            op: Binary::Add,
                            left: *left,
                            right: value,
                        });
                    }
                }
                // `(e+"")+s` for a literal `s`: evaluating `s` runs nothing, so
                // converting `e` after it is unobservable.
                if matches!(node(*right), Expr::Literal(Literal::String(_))) {
                    if let Some(value) = self.string_conversion(*left) {
                        return Some(Expr::Binary {
                            op: Binary::Add,
                            left: value,
                            right: *right,
                        });
                    }
                }
                None
            }
            Expr::Construct { callee, arguments } if self.pristine_builtins => {
                self.regex_literal(*callee, arguments, es2018)
            }
            // `o["name"]` is `o.name`: one spelling for every rule that reads a
            // property by name.
            Expr::Member {
                object,
                property: Property::Computed(key),
            } => self.identifier_key(*key).map(|name| Expr::Member {
                object: *object,
                property: Property::Named(name),
            }),
            // In a literal, `__proto__:` sets the prototype while
            // `["__proto__"]:` defines a property, so that key stays computed.
            Expr::Object(entries)
                if entries.iter().any(|(key, _)| {
                    matches!(key, Property::Computed(key)
                        if self.identifier_key(*key).is_some_and(|name| name != "__proto__"))
                }) =>
            {
                Some(Expr::Object(
                    entries
                        .iter()
                        .map(|(key, value)| {
                            let key = match key {
                                Property::Computed(id) => match self.identifier_key(*id) {
                                    Some(name) if name != "__proto__" => Property::Named(name),
                                    _ => key.clone(),
                                },
                                Property::Named(_) => key.clone(),
                            };
                            (key, *value)
                        })
                        .collect(),
                ))
            }
            _ => None,
        }
    }

    /// The name a literal string key spells, when it is an IdentifierName.
    fn identifier_key(&self, key: ExprId) -> Option<String> {
        match &self.expressions[key.index()] {
            Expr::Literal(Literal::String(value)) => value
                .as_unicode()
                .filter(|name| identifier_name(name))
                .map(str::to_owned),
            _ => None,
        }
    }

    /// `x===void 0||x==null` → `x==null`, and `x!==void 0&&x!=null` →
    /// `x!=null`, in either order, for one binding read twice. A loose test
    /// that the strict one narrows is the strict test of the other nullish
    /// value: `x==null&&x!==void 0` → `x===null`, `x!=null||x===void 0` →
    /// `x!==null` (and with `null` and `void 0` swapped).
    fn nullish_pair(&self, op: Binary, left: ExprId, right: ExprId) -> Option<Expr> {
        if let Some(narrowed) = self.nullish_narrowing(op, left, right) {
            return Some(narrowed);
        }
        let (strict, loose) = match op {
            Binary::Or => (Binary::StrictEqual, Binary::Equal),
            _ => (Binary::StrictNotEqual, Binary::NotEqual),
        };
        let comparison = |id: ExprId| match &self.expressions[id.index()] {
            Expr::Binary {
                op,
                left: operand,
                right: literal,
            } => match (&self.expressions[operand.index()], &self.expressions[literal.index()])
            {
                (Expr::Binding(binding), Expr::Literal(value)) => {
                    Some((*op, *binding, value.clone()))
                }
                _ => None,
            },
            _ => None,
        };
        let (a, b) = (comparison(left)?, comparison(right)?);
        if a.1 != b.1 {
            return None;
        }
        let nullish = |value: &Literal| matches!(value, Literal::Null | Literal::Undefined);
        let undefined = |value: &Literal| matches!(value, Literal::Undefined);
        // One side is the loose test against `null`/`undefined`; the other the
        // strict test against `undefined`, which the loose one already covers.
        let keep = if a.0 == loose && nullish(&a.2) && b.0 == strict && undefined(&b.2) {
            left
        } else if b.0 == loose && nullish(&b.2) && a.0 == strict && undefined(&a.2) {
            right
        } else {
            return None;
        };
        Some(self.expressions[keep.index()].clone())
    }

    fn nullish_narrowing(&self, op: Binary, left: ExprId, right: ExprId) -> Option<Expr> {
        let (loose, strict, result) = match op {
            Binary::And => (Binary::Equal, Binary::StrictNotEqual, Binary::StrictEqual),
            Binary::Or => (Binary::NotEqual, Binary::StrictEqual, Binary::StrictNotEqual),
            _ => return None,
        };
        // (operator, operand, literal node) of `binding op literal`.
        let comparison = |id: ExprId| match &self.expressions[id.index()] {
            Expr::Binary {
                op,
                left: operand,
                right: literal,
            } => match (&self.expressions[operand.index()], &self.expressions[literal.index()]) {
                (Expr::Binding(binding), Expr::Literal(Literal::Null | Literal::Undefined)) => {
                    Some((*op, *binding, *operand, *literal))
                }
                _ => None,
            },
            _ => None,
        };
        let (a, b) = (comparison(left)?, comparison(right)?);
        if a.1 != b.1 {
            return None;
        }
        let (wide, narrow) = if a.0 == loose && b.0 == strict {
            (a, b)
        } else if b.0 == loose && a.0 == strict {
            (b, a)
        } else {
            return None;
        };
        // The loose test's literal must be the other nullish value.
        let null = |id: ExprId| matches!(self.expressions[id.index()], Expr::Literal(Literal::Null));
        if null(narrow.3) == null(wide.3) {
            return None;
        }
        Some(Expr::Binary {
            op: result,
            left: wide.2,
            right: wide.3,
        })
    }

    /// The value inside `e+""` when `id` is exactly that conversion.
    fn string_conversion(&self, id: ExprId) -> Option<ExprId> {
        match &self.expressions[id.index()] {
            Expr::Binary {
                op: Binary::Add,
                left,
                right,
            } if self.empty_string(*right) => Some(*left),
            _ => None,
        }
    }

    fn empty_string(&self, id: ExprId) -> bool {
        matches!(&self.expressions[id.index()], Expr::Literal(Literal::String(value)) if value.is_empty())
    }

    fn is_length(&self, id: ExprId) -> bool {
        matches!(
            &self.expressions[id.index()],
            Expr::Member { property: Property::Named(name), .. } if name == "length"
        )
    }

    /// `new RegExp(p[, f])` of literal strings, as a literal when the pattern
    /// and flags are in the proven subset.
    fn regex_literal(&self, callee: ExprId, arguments: &[ExprId], es2018: bool) -> Option<Expr> {
        if !matches!(&self.expressions[callee.index()], Expr::Host(name) if name == "RegExp") {
            return None;
        }
        let text = |id: &ExprId| match &self.expressions[id.index()] {
            Expr::Literal(Literal::String(value)) => value.as_unicode(),
            _ => None,
        };
        let (pattern, flags) = match arguments {
            [pattern] => (text(pattern)?, ""),
            [pattern, flags] => (text(pattern)?, text(flags)?),
            _ => return None,
        };
        crate::js_regex::literal_from_decoded_checked(pattern, flags, es2018).map(Expr::Regex)
    }

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
            Expr::Literal(Literal::Bool(_))
            | Expr::Unary {
                op: Unary::Not, ..
            } => Some(Known::Boolean),
            Expr::Literal(Literal::String(_))
            | Expr::Template(_)
            | Expr::Unary {
                op: Unary::TypeOf,
                ..
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

    /// The result type of a call to a pristine builtin, from its spelling as
    /// a checked host reference: `Math.trunc`, `Date.now`, `parseInt`, and
    /// `Object.prototype.toString.call`.
    fn builtin_result(&self, callee: ExprId, _arguments: usize) -> Option<Known> {
        let host = |id: ExprId| match &self.expressions[id.index()] {
            Expr::Host(name) => Some(name.as_str()),
            _ => None,
        };
        match &self.expressions[callee.index()] {
            Expr::Host(name) => {
                let name = name.as_str();
                if GLOBAL_NUMBERS.contains(&name) {
                    Some(Known::Number)
                } else if GLOBAL_BOOLEANS.contains(&name) {
                    Some(Known::Boolean)
                } else if GLOBAL_STRINGS.contains(&name) {
                    Some(Known::String)
                } else {
                    None
                }
            }
            Expr::Member { object, property } => {
                // `Math.trunc` and `Math["trunc"]` name the same property.
                let member = match property {
                    Property::Named(member) => member.as_str(),
                    Property::Computed(key) => match &self.expressions[key.index()] {
                        Expr::Literal(Literal::String(key)) => key.as_unicode()?,
                        _ => return None,
                    },
                };
                if let Some(object) = host(*object) {
                    let key = (object, member);
                    if object == "Math" && MATH_NUMBERS.contains(&member)
                        || STATIC_NUMBERS.contains(&key)
                    {
                        return Some(Known::Number);
                    }
                    if STATIC_BOOLEANS.contains(&key) {
                        return Some(Known::Boolean);
                    }
                    if STATIC_STRINGS.contains(&key) {
                        return Some(Known::String);
                    }
                    return None;
                }
                // `Object.prototype.toString.call(v)` is always a string, and
                // `Object.prototype.hasOwnProperty.call(o,k)` a boolean.
                if member == "call" {
                    if let Expr::Member {
                        object: method,
                        property: Property::Named(name),
                    } = &self.expressions[object.index()]
                    {
                        let known = match name.as_str() {
                            "toString" => Some(Known::String),
                            "hasOwnProperty" | "propertyIsEnumerable" | "isPrototypeOf" => {
                                Some(Known::Boolean)
                            }
                            _ => None,
                        };
                        if let Some(known) = known {
                            if let Expr::Member {
                                object: prototype,
                                property: Property::Named(prototype_key),
                            } = &self.expressions[method.index()]
                            {
                                if prototype_key == "prototype" && host(*prototype) == Some("Object")
                                {
                                    return Some(known);
                                }
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}
