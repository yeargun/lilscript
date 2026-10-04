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
//! * `"a"+"b"` of two string literals is `"ab"`.
//! * `x.length|0` is `x.length` under the numeric-length assumption.
//! * `new RegExp("p","f")` of literal strings is `/p/f` under pristine
//!   builtins, when the pattern is in the subset whose literal and constructor
//!   forms mean the same (`js_regex`). Each evaluation still creates a fresh
//!   object, so the literal is never copied or shared.
//! * `[a,b,c][i]` of inert literals, with `i` an own present index, is the
//!   selected element. Formation calls this rule directly on a value read.
use super::*;

use super::facts_values::Known;

impl Module {
    /// Apply the rules above until none applies. Returns the number of edits.
    /// No rule rewrites an expression that holds an observed literal.
    pub(crate) fn simplify_operators(
        &mut self,
        numeric_lengths: bool,
        year: u16,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut edits = 0;
        let int32 = self.int32_bindings(budget)?;
        // Children precede parents in the arena except after a splice, and a
        // rewrite can expose its parent's rule: a few sweeps reach the
        // fixed point on every tree the formation produces.
        for _ in 0..4 {
            budget.work(Analysis, self.expressions.len() as u64)?;
            let before = edits;
            for index in 0..self.expressions.len() {
                if self.holds_observed(ExprId::new(index)) {
                    continue;
                }
                if let Some(replacement) =
                    self.simplified(ExprId::new(index), numeric_lengths, year, &int32)
                {
                    if self.set_expression(ExprId::new(index), replacement) {
                        edits += 1;
                    }
                }
            }
            if edits == before {
                break;
            }
        }
        edits += self.strip_test_negations(budget)?;
        Ok(edits)
    }

    /// `!!x` where only truthiness is used (a condition, a `!` operand, a
    /// discarded value, an operand of `&&`/`||` in one of those) is `x`:
    /// `ToBoolean` runs no code. A node shared with a value position keeps
    /// its spelling.
    fn strip_test_negations(&mut self, budget: &mut AllocationBudget<'_>) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut tests: Vec<ExprId> = Vec::new();
        let mut reached = budget.filled(AllocationClass::Scratch, self.expressions.len(), 0u32)?;
        let mut regions = vec![self.root];
        let mut pending: Vec<ExprId> = Vec::new();
        loop {
            if let Some(id) = pending.pop() {
                budget.work(Analysis, 1)?;
                reached[id.index()] = reached[id.index()].saturating_add(1);
                let expression = &self.expressions[id.index()];
                match expression {
                    Expr::Conditional { condition, .. } => tests.push(*condition),
                    Expr::Unary { op: Unary::Not, value } => tests.push(*value),
                    _ => {}
                }
                for function in expression.created_functions() {
                    regions.push(self.functions[function.index()].body);
                }
                expression.visit_children(|child| {
                    pending.push(child);
                    Ok::<_, AllocationError>(())
                })?;
            } else if let Some(region) = regions.pop() {
                for statement in &self.regions[region.index()].statements {
                    match statement {
                        Statement::If { condition, .. } => tests.push(*condition),
                        Statement::Loop { condition: Some(condition), .. } => tests.push(*condition),
                        Statement::Evaluate(value) => tests.push(*value),
                        Statement::Function { function, .. } => regions.push(self.functions[function.index()].body),
                        _ => {}
                    }
                    statement.visit_expressions(|root| pending.push(root));
                    statement.visit_regions(|child| regions.push(child));
                }
            } else {
                break;
            }
        }
        let mut edits = 0;
        while let Some(test) = tests.pop() {
            budget.work(Analysis, 1)?;
            if reached[test.index()] != 1 || self.holds_observed(test) {
                continue;
            }
            match self.expressions[test.index()] {
                Expr::Unary { op: Unary::Not, value } => {
                    if let Expr::Unary { op: Unary::Not, value: inner } = self.expressions[value.index()] {
                        if reached[value.index()] == 1 {
                            let replacement = self.expressions[inner.index()].clone();
                            if self.set_expression(test, replacement) {
                                edits += 1;
                                tests.push(test);
                            }
                        }
                    }
                }
                Expr::Binary { op: Binary::And | Binary::Or, left, right } => {
                    tests.push(left);
                    tests.push(right);
                }
                _ => {}
            }
        }
        Ok(edits)
    }

    /// Whether `id`, a child or a grandchild is an observed literal: every
    /// rule reads at most two levels down.
    fn holds_observed(&self, id: ExprId) -> bool {
        if self.observed_literals.is_empty() {
            return false;
        }
        let hit = |id: ExprId| self.observed(id);
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

    /// Bindings that always hold an int32: `int` cells, and a binding whose
    /// one write is its declaration's initializer, an int32 operation
    /// (`a^b`, `a|0`, `a>>b`, an `int` operation).
    pub(super) fn int32_bindings(&self, budget: &mut AllocationBudget<'_>) -> Result<Vec<bool>, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut writes = vec![0u32; self.bindings.len()];
        let mut defined = vec![false; self.bindings.len()];
        for region in &self.regions {
            budget.work(Analysis, region.statements.len() as u64)?;
            for (index, statement) in region.statements.iter().enumerate() {
                match statement {
                    Statement::Let { binding, value: Some(value) } => {
                        writes[binding.index()] += 1;
                        defined[binding.index()] = self.int32_value(*value);
                    }
                    // `let i;i=v`, which declaration merging joins later: the
                    // assignment is the next statement, so nothing reads `i`
                    // before it.
                    Statement::Let { binding, value: None } => {
                        if let Some(Statement::Evaluate(store)) = region.statements.get(index + 1) {
                            if let Expr::Assign { target, value } = self.expressions[store.index()] {
                                if matches!(self.expressions[target.index()], Expr::Binding(found) if found == *binding) {
                                    defined[binding.index()] = self.int32_value(value);
                                }
                            }
                        }
                    }
                    Statement::ForIn { binding, .. } | Statement::ForOf { binding, .. } => {
                        writes[binding.index()] += 2;
                    }
                    _ => {}
                }
            }
        }
        budget.work(Analysis, self.expressions.len() as u64)?;
        for expression in &self.expressions {
            if let Expr::Assign { target, .. } = expression {
                if let Expr::Binding(binding) = self.expressions[target.index()] {
                    writes[binding.index()] += 1;
                }
            }
        }
        Ok((0..self.bindings.len())
            .map(|index| {
                self.bindings[index].class == Some(ValueClass::Int)
                    || !self.bindings[index].pinned && writes[index] == 1 && defined[index]
            })
            .collect())
    }

    fn int32_value(&self, id: ExprId) -> bool {
        match &self.expressions[id.index()] {
            Expr::ToInt32(_) | Expr::IntBinary { .. } | Expr::IntNegate(_) => true,
            Expr::Binary { op, .. } => matches!(
                op,
                Binary::BitAnd | Binary::BitOr | Binary::BitXor | Binary::ShiftLeft | Binary::ShiftRight
            ),
            Expr::Unary { op: Unary::BitNot, .. } => true,
            Expr::Literal(Literal::Number(n)) => n.fract() == 0.0 && *n >= -2147483648.0 && *n <= 2147483647.0,
            _ => false,
        }
    }

    fn simplified(&self, id: ExprId, numeric_lengths: bool, year: u16, int32: &[bool]) -> Option<Expr> {
        let es2018 = year >= 2018;
        let node = |id: ExprId| &self.expressions[id.index()];
        let same = |a: ExprId, b: ExprId| matches!((node(a), node(b)), (Expr::Binding(x), Expr::Binding(y)) if x == y);
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
                op: op @ (Binary::Or | Binary::And | Binary::Nullish),
                left,
                right,
            } => {
                // A literal left operand decides: `!1||x` is `x`, `!0&&x` is
                // `x`, `!0||x` is `!0`, and `null??x` is `x` (inlined tests
                // leave these behind).
                if let Expr::Literal(literal) = node(*left) {
                    let decides = match literal {
                        Literal::Null | Literal::Undefined => Some(false),
                        Literal::Bool(value) => Some(*value),
                        Literal::Number(value) => Some(*value != 0.0 && !value.is_nan()),
                        Literal::String(value) => Some(!value.is_empty()),
                    };
                    if let Some(truthy) = decides {
                        let right_wins = match op {
                            Binary::Or => !truthy,
                            Binary::And => truthy,
                            _ => matches!(literal, Literal::Null | Literal::Undefined),
                        };
                        return Some(node(if right_wins { *right } else { *left }).clone());
                    }
                }
                if *op == Binary::Nullish {
                    return None;
                }
                self.nullish_pair(*op, *left, *right)
            }
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
                // `"a"+"b"` is `"ab"`: both operands are literals, so the sum
                // runs nothing and concatenates exactly their code units.
                // Constant inlining leaves these behind (`MOBILE+"/"`), and a
                // longer sum folds from the left across sweeps.
                if let (Expr::Literal(Literal::String(a)), Expr::Literal(Literal::String(b))) =
                    (node(*left), node(*right))
                {
                    return Some(Expr::Literal(Literal::String(a.concat(b))));
                }
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
            // `...[...x]` is `...x`: the fresh array holds exactly what
            // iterating `x` gives, in order, and its own (pristine) iterator
            // gives that back.
            Expr::Spread(inner) if self.pristine_builtins => match node(*inner) {
                Expr::Array(items) => match items.as_slice() {
                    [only] => match node(*only) {
                        Expr::Spread(source) => Some(Expr::Spread(*source)),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            },
            // `(x|0)&k` is `x&k`: a bitwise operator converts its operands
            // with ToInt32 (ToUint32 for shift counts and `>>>`'s left), which
            // reduce modulo 2^32 as `|0` already did; with a literal on the
            // other side the one conversion happens at the same point.
            Expr::Binary {
                op: op @ (Binary::BitAnd
                | Binary::BitOr
                | Binary::BitXor
                | Binary::ShiftLeft
                | Binary::ShiftRight
                | Binary::UnsignedShiftRight),
                left,
                right,
            } => {
                let literal = |id: ExprId| matches!(node(id), Expr::Literal(Literal::Number(_)));
                match (node(*left), node(*right)) {
                    (Expr::ToInt32(inner), _) if literal(*right) => Some(Expr::Binary {
                        op: *op,
                        left: *inner,
                        right: *right,
                    }),
                    (_, Expr::ToInt32(inner)) if literal(*left) => Some(Expr::Binary {
                        op: *op,
                        left: *left,
                        right: *inner,
                    }),
                    _ => None,
                }
            }
            // With a boolean test `c`: `c?!0:x` is `c||x` and `c?x:!1` is
            // `c&&x` (the same value either way, `c` evaluated once).
            Expr::Conditional { condition, yes, no }
                if self.known(*condition) == Some(Known::Boolean)
                    && (matches!(node(*yes), Expr::Literal(Literal::Bool(true)))
                        || matches!(node(*no), Expr::Literal(Literal::Bool(false)))) =>
            {
                if matches!(node(*yes), Expr::Literal(Literal::Bool(true))) {
                    Some(Expr::Binary { op: Binary::Or, left: *condition, right: *no })
                } else {
                    Some(Expr::Binary { op: Binary::And, left: *condition, right: *yes })
                }
            }
            // `i<0?i+4294967296:i` of an int32 `i` is its unsigned value,
            // `i>>>0` (and so `i>=0?i:i+4294967296`).
            Expr::Conditional { condition, yes, no } => {
                let int = |id: ExprId| match node(id) {
                    Expr::Binding(binding) if int32[binding.index()] => Some(*binding),
                    _ => None,
                };
                let lifted = |id: ExprId, binding: BindingId| {
                    matches!(node(id), Expr::Binary { op: Binary::Add, left, right }
                        if int(*left) == Some(binding)
                            && matches!(node(*right), Expr::Literal(Literal::Number(n)) if *n == 4294967296.0))
                };
                let Expr::Binary { op, left, right } = node(*condition) else {
                    return None;
                };
                let zero = matches!(node(*right), Expr::Literal(Literal::Number(n)) if *n == 0.0);
                let binding = int(*left).filter(|_| zero)?;
                let (negative, positive) = match op {
                    Binary::Less => (*yes, *no),
                    Binary::GreaterEqual => (*no, *yes),
                    _ => return None,
                };
                (lifted(negative, binding) && int(positive) == Some(binding)).then(|| Expr::Binary {
                    op: Binary::UnsignedShiftRight,
                    left: *left,
                    right: *right,
                })
            }
            // `globalThis.RegExp` is `RegExp` (M8.2 A2, diagnosis C19): an
            // ECMAScript builtin is a property of the global object, and
            // under unpatched builtins (R10) reading it either way gives the
            // same value. Naming never gives a binding a host name the tree
            // reads, so the bare name cannot be captured.
            Expr::Member {
                object,
                property: Property::Named(name),
            } if self.pristine_builtins
                && matches!(
                    crate::catalog::host_kind(name),
                    crate::catalog::HostKind::Standard(_)
                )
                && self.global_object(*object) =>
            {
                Some(Expr::Host(name.as_str().into()))
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
            } => match (
                &self.expressions[operand.index()],
                &self.expressions[literal.index()],
            ) {
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
        // `x===null||x===void 0` is `x==null` when no value is `document.all`
        // (and `x!==null&&x!==void 0` is `x!=null`): the strict pair tests
        // exactly the two values the loose test does. The loose comparison
        // node is the left one, rewritten in place.
        if self.no_document_all && a.0 == strict && b.0 == strict && nullish(&a.2) && nullish(&b.2) && a.2 != b.2 {
            let Expr::Binary { left: operand, right: literal, .. } = self.expressions[left.index()] else {
                return None;
            };
            return Some(Expr::Binary { op: loose, left: operand, right: literal });
        }
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
            Binary::Or => (
                Binary::NotEqual,
                Binary::StrictEqual,
                Binary::StrictNotEqual,
            ),
            _ => return None,
        };
        // (operator, operand, literal node) of `binding op literal`.
        let comparison = |id: ExprId| match &self.expressions[id.index()] {
            Expr::Binary {
                op,
                left: operand,
                right: literal,
            } => match (
                &self.expressions[operand.index()],
                &self.expressions[literal.index()],
            ) {
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
        let null =
            |id: ExprId| matches!(self.expressions[id.index()], Expr::Literal(Literal::Null));
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

    /// `globalThis`: the host's name, or a declared extern of that spelling
    /// that is not an import and that no code assigns.
    fn global_object(&self, id: ExprId) -> bool {
        match &self.expressions[id.index()] {
            Expr::Host(host) => host.name == "globalThis",
            Expr::Binding(binding) => {
                let declared = &self.bindings[binding.index()];
                declared.pinned
                    && declared.spelling == "globalThis"
                    && !self.imports.iter().any(|import| import.binding == *binding)
                    && !self.expressions.iter().any(|expression| {
                        matches!(expression, Expr::Assign { target, .. }
                            if matches!(self.expressions[target.index()], Expr::Binding(found) if found == *binding))
                    })
            }
            _ => false,
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
        if !matches!(&self.expressions[callee.index()], Expr::Host(host) if host.kind == crate::catalog::HostKind::Standard(crate::catalog::Global::RegExp))
        {
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


}

/// A value-read rule shared with direct target formation. The caller must
/// supply a value occurrence, never an assignment place or receiver callee.
/// Every element is an inert literal and the selected property is own and
/// present, so this removes only an unobserved fresh allocation. It performs
/// no normalization and copies no payload.
pub(crate) fn literal_array_projection(
    module: &Module,
    value: ExprId,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<Option<ExprId>, crate::output_budget::AllocationError> {
    budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
    let Expr::Member {
        object,
        property: Property::Computed(key),
    } = &module.expressions[value.index()]
    else {
        return Ok(None);
    };
    let (Expr::Array(elements), Expr::Literal(Literal::Number(index))) = (
        &module.expressions[object.index()],
        &module.expressions[key.index()],
    ) else {
        return Ok(None);
    };
    if *index < 0.0 || index.fract() != 0.0 || *index >= elements.len() as f64 {
        return Ok(None);
    }
    for element in elements {
        budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
        if !matches!(module.expressions[element.index()], Expr::Literal(_)) {
            return Ok(None);
        }
    }
    Ok(Some(elements[*index as usize]))
}
