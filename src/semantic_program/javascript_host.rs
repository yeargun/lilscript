//! The `JS.*`, `Object.*`, `JSON.*` and `Task.*` builtins: typed spellings of
//! host operations on `JsValue`s. Each renders exactly as the legacy emitter
//! spells it, and none assumes a pristine host. `JS.methodN` adapters come
//! from one hoisted factory per calling convention.
use super::*;

impl Formation<'_, '_, '_, '_, '_> {
    fn host_call(
        &mut self,
        callee: js::ExprId,
        arguments: Vec<js::ExprId>,
        invocation: Invocation,
    ) -> Result<js::Expr, FormationError> {
        Ok(js::Expr::Call {
            callee,
            arguments,
            invocation,
        })
    }

    fn host_member(&mut self, object: js::ExprId, key: js::ExprId) -> Result<js::ExprId, FormationError> {
        // A literal identifier key prints as `o.name`; others as `o[key]`.
        self.expression(js::Expr::Member {
            object,
            property: js::Property::Computed(key),
        })
    }

    fn host_binary(
        &mut self,
        op: js::Binary,
        left: js::ExprId,
        right: js::ExprId,
    ) -> Result<js::Expr, FormationError> {
        Ok(js::Expr::Binary { op, left, right })
    }

    /// `e => function(a,...){return e(this,a,...)}`, one per convention.
    fn method_factory(&mut self, builtin: BuiltinCall) -> Result<js::BindingId, FormationError> {
        let (arity, receiver, rest) = match builtin {
            BuiltinCall::JsMethod0 => (0, true, false),
            BuiltinCall::JsMethod1 => (1, true, false),
            BuiltinCall::JsMethod2 => (2, true, false),
            BuiltinCall::JsMethod3 => (3, true, false),
            BuiltinCall::JsMethod4 => (4, true, false),
            BuiltinCall::JsMethod5 => (5, true, false),
            BuiltinCall::JsMethod6 => (6, true, false),
            BuiltinCall::JsMethod7 => (7, true, false),
            BuiltinCall::JsMethod8 => (8, true, false),
            BuiltinCall::JsMethod9 => (9, true, false),
            BuiltinCall::JsMethod10 => (10, true, false),
            BuiltinCall::JsMethodRest => (0, true, true),
            BuiltinCall::JsStaticRest => (0, false, true),
            _ => return Err(self.error(Span::default(), "not a method adapter")),
        };
        let key = (arity as u8) | (u8::from(receiver) << 4) | (u8::from(rest) << 5);
        for index in 0..self.host_factories.len() {
            self.work(1)?;
            if self.host_factories[index].0 == key {
                return Ok(self.host_factories[index].1);
            }
        }
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let outer_body = self.module.region_in(scope, self.budget)?;
        let outer_scope = self.module.regions[outer_body.index()].scope;
        let target = self.fresh_binding(outer_scope, "method")?;
        let inner_body = self.module.region_in(outer_scope, self.budget)?;
        let inner_scope = self.module.regions[inner_body.index()].scope;
        let mut parameters = self.budget.vector(AllocationClass::Retained, arity)?;
        let mut forwarded = self.budget.vector(AllocationClass::Retained, arity + 2)?;
        if receiver {
            let this = self.expression(js::Expr::This)?;
            self.append(&mut forwarded, this)?;
        }
        for _ in 0..arity {
            self.work(1)?;
            let parameter = self.fresh_binding(inner_scope, "argument")?;
            self.append(&mut parameters, parameter)?;
            let value = self.reference(parameter)?;
            self.append(&mut forwarded, value)?;
        }
        if rest {
            let arguments = self.text("arguments")?;
            let arguments = self.expression(js::Expr::Host(arguments))?;
            self.append(&mut forwarded, arguments)?;
        }
        let callee = self.reference(target)?;
        let call = self.expression(js::Expr::Call {
            callee,
            arguments: forwarded,
            invocation: Invocation::Value,
        })?;
        self.statement(inner_body, js::Statement::Return(Some(call)))?;
        let inner = js::FunctionId::try_new(self.module.functions.len()).ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters,
                body: inner_body,
                // The adapter's own receiver is the host's `this`.
                arrow: false,
                name: js::FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: crate::structured_js::Suspension::None,
            },
        )?;
        let inner = self.expression(js::Expr::Function(inner))?;
        self.statement(outer_body, js::Statement::Return(Some(inner)))?;
        let mut outer_parameters = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut outer_parameters, target)?;
        let outer = js::FunctionId::try_new(self.module.functions.len()).ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters: outer_parameters,
                body: outer_body,
                arrow: false,
                name: js::FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: crate::structured_js::Suspension::None,
            },
        )?;
        let binding = self.fresh_binding(scope, "method_adapter")?;
        // Hoisted, so an adapter created during initialization is available.
        self.statement(
            root,
            js::Statement::Function {
                binding,
                function: outer,
            },
        )?;
        self.budget
            .push(AllocationClass::Scratch, &mut self.host_factories, (key, binding))?;
        Ok(binding)
    }

    fn fresh_binding(&mut self, scope: js::ScopeId, spelling: &str) -> Result<js::BindingId, FormationError> {
        let spelling = self.text(spelling)?;
        Ok(self.module.binding_in(
            js::Binding {
                source_symbol: None,
                scope,
                spelling,
                pinned: false,
            },
            self.budget,
        )?)
    }

    /// A constant string, possibly after effects scheduled before it; an
    /// object literal evaluates each key before its value, in order.
    pub(super) fn string_key(&self, key: js::ExprId) -> bool {
        match &self.module.expressions[key.index()] {
            js::Expr::Literal(js::Literal::String(_)) => true,
            js::Expr::Sequence(items) => items.last().is_some_and(|last| self.string_key(*last)),
            _ => false,
        }
    }

    /// One host builtin call with its already formed argument expressions.
    pub(super) fn host_builtin(
        &mut self,
        builtin: BuiltinCall,
        arguments: Vec<js::ExprId>,
        span: Span,
    ) -> Result<js::Expr, FormationError> {
        use BuiltinCall as B;
        let count = arguments.len();
        let wrong = |this: &Self| this.error(span, "host builtin operand count");
        let argument = |index: usize| arguments[index];
        let static_path: Option<&'static [&'static str]> = match builtin {
            B::ObjectKeys => Some(&["Object", "keys"]),
            B::ObjectValues => Some(&["Object", "values"]),
            B::ObjectHasOwn
                if self
                    .contract
                    .ecmascript
                    .allows(JsSyntaxFeature::ObjectHasOwn) =>
            {
                Some(&["Object", "hasOwn"])
            }
            B::ObjectHasOwn => Some(&["Object", "prototype", "hasOwnProperty", "call"]),
            B::ObjectAssign => Some(&["Object", "assign"]),
            B::JsonStringify => Some(&["JSON", "stringify"]),
            B::JsonParse => Some(&["JSON", "parse"]),
            B::TaskResolve => Some(&["Promise", "resolve"]),
            B::TaskReject => Some(&["Promise", "reject"]),
            B::TaskAll => Some(&["Promise", "all"]),
            B::JsEncodeURI => Some(&["encodeURI"]),
            B::JsEncodeURIComponent => Some(&["encodeURIComponent"]),
            B::JsIsArray => Some(&["Array", "isArray"]),
            _ => None,
        };
        if let Some(path) = static_path {
            let callee = self.host_path(path)?;
            let invocation = if path.len() == 1 {
                Invocation::Value
            } else {
                Invocation::Reference
            };
            return self.host_call(callee, arguments, invocation);
        }
        let prototype: Option<(&'static str, &'static str)> = match builtin {
            B::JsArrayPush => Some(("push", "call")),
            B::JsArrayPop => Some(("pop", "call")),
            B::JsArraySlice => Some(("slice", "call")),
            B::JsArrayIndexOf => Some(("indexOf", "call")),
            B::JsArraySort => Some(("sort", "call")),
            B::JsArraySplice => Some(("splice", "call")),
            B::JsArrayJoin => Some(("join", "call")),
            B::JsArrayShift => Some(("shift", "call")),
            B::JsArrayUnshift => Some(("unshift", "call")),
            B::JsArrayConcatApply => Some(("concat", "apply")),
            _ => None,
        };
        if let Some((method, invoke)) = prototype {
            if count == 0 {
                return Err(wrong(self));
            }
            let callee = self.host_path(&["Array", "prototype", method, invoke])?;
            return self.host_call(callee, arguments, Invocation::Reference);
        }
        let method: Option<&'static str> = match builtin {
            B::JsStringSlice => Some("slice"),
            B::JsStringIndexOf => Some("indexOf"),
            B::JsStringReplace => Some("replace"),
            B::JsStringMatch => Some("match"),
            B::JsStringSplit => Some("split"),
            B::JsRegexTest => Some("test"),
            B::JsRegexExec => Some("exec"),
            _ => None,
        };
        if let Some(method) = method {
            let Some((&receiver, rest)) = arguments.split_first() else {
                return Err(wrong(self));
            };
            let property = js::Property::Named(self.text(method)?);
            let callee = self.expression(js::Expr::Member {
                object: receiver,
                property,
            })?;
            let mut forwarded = self.budget.vector(AllocationClass::Retained, rest.len())?;
            for &value in rest {
                self.append(&mut forwarded, value)?;
            }
            return self.host_call(callee, forwarded, Invocation::Reference);
        }
        let binary = |op| (op, 2usize);
        let comparison = match builtin {
            B::JsAdd => Some(binary(js::Binary::Add)),
            B::JsMod => Some(binary(js::Binary::Remainder)),
            B::JsLessThan => Some(binary(js::Binary::Less)),
            B::JsLessThanOrEqual => Some(binary(js::Binary::LessEqual)),
            B::JsGreaterThan => Some(binary(js::Binary::Greater)),
            B::JsGreaterThanOrEqual => Some(binary(js::Binary::GreaterEqual)),
            B::JsStrictEqual => Some(binary(js::Binary::StrictEqual)),
            B::JsStrictNotEqual => Some(binary(js::Binary::StrictNotEqual)),
            _ => None,
        };
        if let Some((op, operands)) = comparison {
            if count != operands {
                return Err(wrong(self));
            }
            return self.host_binary(op, argument(0), argument(1));
        }
        Ok(match builtin {
            B::JsUndefined if count == 0 => js::Expr::Literal(js::Literal::Undefined),
            B::JsArray => js::Expr::Array(arguments),
            B::JsObject => {
                if count % 2 != 0 {
                    return Err(wrong(self));
                }
                let mut entries = self.budget.vector(AllocationClass::Retained, count / 2)?;
                for pair in arguments.chunks_exact(2) {
                    self.work(1)?;
                    // Keys are syntax, as in the legacy emitter: only a
                    // constant string names a plain-object entry.
                    if !self.string_key(pair[0]) {
                        return Err(self.error(span, "plain-object key is not a constant string"));
                    }
                    self.append(&mut entries, (js::Property::Computed(pair[0]), pair[1]))?;
                }
                js::Expr::Object(entries)
            }
            B::JsTypeOf if count == 1 => js::Expr::Unary {
                op: js::Unary::TypeOf,
                value: argument(0),
            },
            B::JsIsNullish if count == 1 => {
                let null = self.literal(js::Literal::Null)?;
                self.host_binary(js::Binary::Equal, argument(0), null)?
            }
            B::JsIsFalse if count == 1 => {
                let literal = self.literal(js::Literal::Bool(false))?;
                self.host_binary(js::Binary::StrictEqual, argument(0), literal)?
            }
            B::JsIsUndefined if count == 1 => {
                let literal = self.literal(js::Literal::Undefined)?;
                self.host_binary(js::Binary::StrictEqual, argument(0), literal)?
            }
            B::JsString if count == 1 => {
                if matches!(
                    self.module.expressions[argument(0).index()],
                    js::Expr::Literal(js::Literal::String(_))
                ) {
                    return Ok(self.module.expressions[argument(0).index()].clone());
                }
                let empty = self.string(&crate::literal::StringValue::default())?;
                let empty = self.literal(js::Literal::String(empty))?;
                self.host_binary(js::Binary::Add, argument(0), empty)?
            }
            // Under the numeric-lengths assumption a length is already a Number.
            B::JsNumber
                if count == 1
                    && self.compact
                    && self.contract.assumptions.numeric_lengths
                    && self.length_member(argument(0)) =>
            {
                self.module.expressions[argument(0).index()].clone()
            }
            B::JsNumber if count == 1 => js::Expr::Unary {
                op: js::Unary::Plus,
                value: argument(0),
            },
            B::JsAssume if count == 1 => self.module.expressions[argument(0).index()].clone(),
            B::JsBox if count == 1 => {
                let callee = self.host_path(&["Object"])?;
                self.host_call(callee, arguments, Invocation::Value)?
            }
            B::JsCall if count >= 2 => {
                let (callee, rest) = (argument(0), &arguments[1..]);
                if matches!(
                    self.module.expressions[rest[0].index()],
                    js::Expr::Literal(js::Literal::Undefined)
                ) {
                    let mut forwarded = self.budget.vector(AllocationClass::Retained, rest.len())?;
                    for &value in &rest[1..] {
                        self.append(&mut forwarded, value)?;
                    }
                    return self.host_call(callee, forwarded, Invocation::Value);
                }
                let property = js::Property::Named(self.text("call")?);
                let call = self.expression(js::Expr::Member {
                    object: callee,
                    property,
                })?;
                let mut forwarded = self.budget.vector(AllocationClass::Retained, rest.len())?;
                for &value in rest {
                    self.append(&mut forwarded, value)?;
                }
                self.host_call(call, forwarded, Invocation::Reference)?
            }
            B::JsApply if count == 3 => {
                let property = js::Property::Named(self.text("apply")?);
                let apply = self.expression(js::Expr::Member {
                    object: argument(0),
                    property,
                })?;
                let mut forwarded = self.budget.vector(AllocationClass::Retained, 2)?;
                self.append(&mut forwarded, argument(1))?;
                self.append(&mut forwarded, argument(2))?;
                self.host_call(apply, forwarded, Invocation::Reference)?
            }
            B::JsConstruct if count >= 1 => {
                let mut forwarded = self.budget.vector(AllocationClass::Retained, count - 1)?;
                for &value in &arguments[1..] {
                    self.append(&mut forwarded, value)?;
                }
                js::Expr::Construct {
                    callee: argument(0),
                    arguments: forwarded,
                }
            }
            B::JsInvoke if count >= 2 => {
                let callee = self.host_member(argument(0), argument(1))?;
                let mut forwarded = self.budget.vector(AllocationClass::Retained, count - 2)?;
                for &value in &arguments[2..] {
                    self.append(&mut forwarded, value)?;
                }
                self.host_call(callee, forwarded, Invocation::Reference)?
            }
            B::JsGet if count == 2 => js::Expr::Member {
                object: argument(0),
                property: js::Property::Computed(argument(1)),
            },
            B::JsSet if count == 3 => {
                let target = self.host_member(argument(0), argument(1))?;
                js::Expr::Assign {
                    target,
                    value: argument(2),
                }
            }
            B::JsDelete if count == 2 => {
                let target = self.host_member(argument(0), argument(1))?;
                js::Expr::Unary {
                    op: js::Unary::Delete,
                    value: target,
                }
            }
            B::JsHas if count == 2 => {
                // `key in Object(value)`: a primitive receiver is boxed.
                let boxer = self.host_path(&["Object"])?;
                let mut boxed = self.budget.vector(AllocationClass::Retained, 1)?;
                self.append(&mut boxed, argument(0))?;
                let object = self.expression(js::Expr::Call {
                    callee: boxer,
                    arguments: boxed,
                    invocation: Invocation::Value,
                })?;
                self.host_binary(js::Binary::In, argument(1), object)?
            }
            B::JsIn if count == 2 => self.host_binary(js::Binary::In, argument(0), argument(1))?,
            B::JsMethod0
            | B::JsMethod1
            | B::JsMethod2
            | B::JsMethod3
            | B::JsMethod4
            | B::JsMethod5
            | B::JsMethod6
            | B::JsMethod7
            | B::JsMethod8
            | B::JsMethod9
            | B::JsMethod10
            | B::JsMethodRest
            | B::JsStaticRest
                if count == 1 =>
            {
                let factory = self.method_factory(builtin)?;
                let callee = self.reference(factory)?;
                self.host_call(callee, arguments, Invocation::Value)?
            }
            _ => return Err(self.error(span, "semantic JavaScript builtin implementation")),
        })
    }
}
