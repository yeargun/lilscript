//! Nominal reference construction. Checked declarations select callables and
//! fields; ordinary target functions, objects and references own execution.

use super::*;
use crate::ast::{ClassDecl, ClassMember};
use crate::semantic::{NominalMember, NominalMemberId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum NominalCallable {
    Initialize(NominalId),
    Method(NominalMemberId),
}

impl<'sem, 'src> Lower<'sem, 'src> {
    fn class_id(&self, class: &ClassDecl<'_, 'src>) -> Result<NominalId, Unsupported> {
        self.semantics
            .nominal_id(&Type::Class(class.name.name))
            .ok_or(Unsupported {
                span: class.span,
                feature: "missing checked nominal declaration",
            })
    }

    fn nominal_function(&self, key: NominalCallable, span: Span) -> Result<BindingId, Unsupported> {
        self.nominal_functions
            .get(&key)
            .copied()
            .ok_or(Unsupported {
                span,
                feature: "nominal callable outside the supported source slice",
            })
    }

    fn declare_nominal_function(&mut self, key: NominalCallable, spelling: String) {
        let binding = self.module.binding(Binding {
            source_symbol: None,
            scope: ScopeId::new(0),
            spelling,
            pinned: false,
        });
        assert!(self.nominal_functions.insert(key, binding).is_none());
    }

    pub(super) fn declare_class(&mut self, class: &ClassDecl<'_, 'src>) -> Result<(), Unsupported> {
        if class.object {
            return Err(Unsupported {
                span: class.span,
                feature: "public singleton callable-field construction",
            });
        }
        let owner = self.class_id(class)?;
        self.declare_nominal_function(
            NominalCallable::Initialize(owner),
            format!("initialize_{}", class.name.name),
        );
        let info = self.semantics.nominal_class(owner).unwrap();
        for member in class.members {
            if let ClassMember::Method(method) = member {
                self.declare_nominal_function(
                    NominalCallable::Method(info.methods[method.name.name].member),
                    format!("{}_{}", class.name.name, method.name.name),
                );
            }
        }
        Ok(())
    }

    fn receiver(&mut self, span: Span, body: RegionId) -> Result<BindingId, Unsupported> {
        // The checker already allocated this symbol. Translate that actual
        // declaration; do not replace every identifier spelled `this` later.
        let binding = self.declare(ast::Ident { name: "this", span }, body)?;
        self.module.bindings[binding.index()].spelling = "receiver".into();
        Ok(binding)
    }

    fn finish_nominal_function(
        &mut self,
        key: NominalCallable,
        parameters: Vec<BindingId>,
        body: RegionId,
        span: Span,
    ) -> Result<(), Unsupported> {
        let function = FunctionId::new(self.module.functions.len());
        self.module.functions.push(Function {
            parameters,
            body,
            arrow: false,
            // Generated implementations have no source function-value identity.
            // Bound method values and public constructors have separate contracts.
            name: FunctionName::Unobserved,
            strict: false,
            length: None,
            suspension: crate::structured_js::Suspension::None,
        });
        let binding = self.nominal_function(key, span)?;
        self.push(RegionId::new(0), Statement::Function { binding, function });
        Ok(())
    }

    pub(super) fn class(&mut self, class: &ClassDecl<'_, 'src>) -> Result<(), Unsupported> {
        let owner = self.class_id(class)?;
        let info = self.semantics.nominal_class(owner).unwrap();
        let constructor = class.members.iter().find_map(|member| match member {
            ClassMember::Constructor(constructor) => Some(constructor),
            _ => None,
        });
        let body = self.module.region(ScopeId::new(0));
        let checkpoint = self.bound_symbols.len();
        let (mut parameters, receiver) = if let Some(constructor) = constructor {
            let receiver = self.receiver(constructor.span, body)?;
            let symbol = self.module.bindings[receiver.index()]
                .source_symbol
                .unwrap();
            let instance = if self.semantics.symbol_is_assigned(symbol) {
                // The source receiver is a local cell. Rebinding it cannot
                // replace the instance produced by the enclosing construction.
                // Read-only receivers share the parameter directly; the checker
                // already owns this write fact, including writes in closures.
                let instance = self.module.binding(Binding {
                    source_symbol: None,
                    scope: self.module.regions[body.index()].scope,
                    spelling: "instance".into(),
                    pinned: false,
                });
                let value = self.module.expression(Expr::Binding(instance), None);
                self.push(
                    body,
                    Statement::Let {
                        binding: receiver,
                        value: Some(value),
                    },
                );
                instance
            } else {
                receiver
            };
            (self.parameters(constructor.params, body)?, instance)
        } else {
            let receiver = self.module.binding(Binding {
                source_symbol: None,
                scope: self.module.regions[body.index()].scope,
                spelling: "receiver".into(),
                pinned: false,
            });
            (Vec::new(), receiver)
        };
        // Construction evaluates supplied/default arguments before allocation.
        // Keeping the fresh instance last preserves that order without a wrapper.
        parameters.push(receiver);
        self.constructor = Some((owner, receiver));
        if let Some(constructor) = constructor {
            self.statements(constructor.body, body)?;
        }
        let value = self.module.expression(Expr::Binding(receiver), None);
        self.push(body, Statement::Return(Some(value)));
        self.constructor = None;
        self.restore_bindings(checkpoint);
        self.finish_nominal_function(
            NominalCallable::Initialize(owner),
            parameters,
            body,
            class.span,
        )?;

        for member in class.members {
            let ClassMember::Method(method) = member else {
                continue;
            };
            if method.is_async || method.is_generator {
                return Err(Unsupported {
                    span: method.span,
                    feature: "async or generator method",
                });
            }
            let body = self.module.region(ScopeId::new(0));
            let checkpoint = self.bound_symbols.len();
            let receiver = self.receiver(method.name.span, body)?;
            let mut parameters = vec![receiver];
            parameters.extend(self.parameters(method.params, body)?);
            self.statements(method.body, body)?;
            self.restore_bindings(checkpoint);
            self.finish_nominal_function(
                NominalCallable::Method(info.methods[method.name.name].member),
                parameters,
                body,
                method.span,
            )?;
        }
        Ok(())
    }

    pub(super) fn nominal_field(
        &mut self,
        source: &ast::Expr<'_, 'src>,
        object: &ast::Expr<'_, 'src>,
        region: RegionId,
    ) -> Result<Expr, Unsupported> {
        match self.semantics.resolved_member(source.id) {
            Some(NominalMember::Field { owner, field }) if owner.is_class() => Ok(Expr::Member {
                object: self.expression(object, region)?,
                property: Property::Named(field.name.into()),
            }),
            Some(NominalMember::Method { .. }) => Err(Unsupported {
                span: source.span(),
                feature: "nominal methods require a receiver call",
            }),
            _ => Err(Unsupported {
                span: source.span(),
                feature: "nominal value/place outside the supported source slice",
            }),
        }
    }

    pub(super) fn nominal_call(
        &mut self,
        callee: &ast::Expr<'_, 'src>,
        member: NominalMemberId,
        provided: &[ast::Argument<'_, 'src>],
        region: RegionId,
    ) -> Result<Expr, Unsupported> {
        let ast::ExprKind::Member { object, .. } = &callee.kind else {
            return Err(Unsupported {
                span: callee.span(),
                feature: "nominal call requires a checked receiver",
            });
        };
        let binding = self.nominal_function(NominalCallable::Method(member), callee.span())?;
        let target = self.module.expression(Expr::Binding(binding), None);
        let mut arguments = vec![self.expression(object, region)?];
        arguments.extend(self.call_arguments(callee, provided, region)?);
        Ok(Expr::Call {
            callee: target,
            arguments,
            invocation: Invocation::Value,
        })
    }

    fn default_field(&mut self, ty: &Type<'src>, span: Span) -> Result<ExprId, Unsupported> {
        let node = match ty {
            Type::Int | Type::Float | Type::Enum(_) => Expr::Literal(Literal::Number(0.0)),
            Type::Bool => Expr::Literal(Literal::Bool(false)),
            Type::String => Expr::Literal(Literal::String(StringValue::default())),
            Type::Array(_) => Expr::Array(Vec::new()),
            Type::Record(_) => {
                let null = self.module.expression(Expr::Literal(Literal::Null), None);
                Expr::Object(vec![(Property::Named("__proto__".into()), null)])
            }
            Type::Union(types) => {
                return self.default_field(
                    types.first().ok_or(Unsupported {
                        span,
                        feature: "empty union has no field default",
                    })?,
                    span,
                );
            }
            Type::Null
            | Type::Nullable(_)
            | Type::Class(_)
            | Type::ClassInstance { .. }
            | Type::TypeParameter(_)
            | Type::Function(_)
            | Type::GenericFunction(_) => Expr::Literal(Literal::Null),
            _ => {
                let operation = crate::primitive::constructor_intrinsic(ty).ok_or(Unsupported {
                    span,
                    feature: "field default outside the supported source types",
                })?;
                let constructor = native_constructor(operation).ok_or(Unsupported {
                    span,
                    feature: "field default outside the supported source types",
                })?;
                let arguments = (0..constructor.arity)
                    .map(|_| {
                        self.module
                            .expression(Expr::Literal(Literal::Number(0.0)), None)
                    })
                    .collect();
                Expr::ConstructIntrinsic {
                    operation,
                    arguments,
                }
            }
        };
        Ok(self.module.expression(node, None))
    }

    pub(super) fn construct_class(
        &mut self,
        source: &ast::Expr<'_, 'src>,
        owner: NominalId,
        provided: &[ast::Argument<'_, 'src>],
        region: RegionId,
    ) -> Result<Expr, Unsupported> {
        let info = self.semantics.nominal_class(owner).ok_or(Unsupported {
            span: source.span(),
            feature: "construction requires a checked class declaration",
        })?;
        let binding = self.nominal_function(NominalCallable::Initialize(owner), source.span())?;
        let callee = self.module.expression(Expr::Binding(binding), None);
        let mut arguments =
            self.checked_arguments(provided, info.constructor.as_ref(), region, source.span())?;
        let mut fields = Vec::with_capacity(info.fields.len());
        for field in info.fields.values() {
            let property = if field.name == "__proto__" {
                Property::Computed(
                    self.module
                        .expression(Expr::Literal(Literal::String(field.name.into())), None),
                )
            } else {
                Property::Named(field.name.into())
            };
            fields.push((property, self.default_field(&field.ty, source.span())?));
        }
        // This allocation retains its nominal provenance for later legal layout
        // choices. It does not inherit a call's effects or a constructor body.
        arguments.push(
            self.module
                .expression(Expr::Object(fields), Some(source.id)),
        );
        Ok(Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })
    }

    pub(super) fn super_call(
        &mut self,
        provided: &[ast::Argument<'_, 'src>],
        region: RegionId,
        span: Span,
    ) -> Result<ExprId, Unsupported> {
        let (owner, receiver) = self.constructor.ok_or(Unsupported {
            span,
            feature: "super call outside its checked constructor",
        })?;
        let class = self.semantics.nominal_class(owner).unwrap();
        let base = class
            .base
            .as_ref()
            .and_then(|ty| self.semantics.nominal_id(ty))
            .ok_or(Unsupported {
                span,
                feature: "missing checked base class",
            })?;
        let binding = self.nominal_function(NominalCallable::Initialize(base), span)?;
        let callee = self.module.expression(Expr::Binding(binding), None);
        let receiver = self.module.expression(Expr::Binding(receiver), None);
        let (receiver, capture) = self.snapshot(receiver, region);
        let signature = self
            .semantics
            .base_constructor(class.name)
            .map(|(_, signature)| signature);
        let mut arguments = self.checked_arguments(provided, signature.as_ref(), region, span)?;
        arguments.push(self.module.expression(Expr::Binding(receiver), None));
        let node = self.sequence(
            vec![capture],
            Expr::Call {
                callee,
                arguments,
                invocation: Invocation::Value,
            },
        );
        Ok(self.module.expression(node, None))
    }
}
