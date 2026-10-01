//! Declared plain reference data. Field identity and kind belong to the checker;
//! construction and spread consume that schema, never a host object's key list.
use super::type_admission::TypeQueryAdmission;
use super::type_relation::type_equal_with;
use super::type_substitution::substitute_type_with;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeTag<'src> {
    Int(i32),
    String(&'src str),
    Bool(bool),
}

impl<'src> ShapeTag<'src> {
    pub(super) fn literal(expr: &Expr<'_, 'src>) -> Option<Self> {
        match &expr.kind {
            ExprKind::Int(value, _) => i32::try_from(*value).ok().map(Self::Int),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                expr,
                ..
            } => match expr.kind {
                ExprKind::Int(value, _) => value
                    .checked_neg()
                    .and_then(|value| i32::try_from(value).ok())
                    .map(Self::Int),
                _ => None,
            },
            ExprKind::String(value, _) => Some(Self::String(value)),
            ExprKind::Bool(value, _) => Some(Self::Bool(*value)),
            _ => None,
        }
    }
    pub(super) fn matches_type(self, ty: &Type<'_>) -> bool {
        matches!(
            (self, ty),
            (Self::Int(_), Type::Int)
                | (Self::String(_), Type::String)
                | (Self::Bool(_), Type::Bool)
        )
    }
    fn same(
        self,
        other: Self,
        span: Span,
        query: &mut TypeQueryAdmission<'_, '_>,
    ) -> Result<bool, AdmittedCheckError> {
        match (self, other) {
            (Self::String(a), Self::String(b)) => {
                query
                    .string_literals_equal(a, b)
                    .map_err(|error| match error {
                        crate::literal::StringDecodeError::Resources(error) => error.into(),
                        crate::literal::StringDecodeError::Escape(error) => {
                            AdmittedCheckError::new(
                                span,
                                format!("invalid shape tag escape: {error:?}"),
                            )
                        }
                    })
            }
            _ => Ok(self == other),
        }
    }
}

impl<'view, 'ast, 'src> CheckedView<'view, 'ast, 'src> {
    fn field_tag(&self, owner: NominalId, slot: usize) -> Option<ShapeTag<'src>> {
        self.nominal_class(owner)?
            .discriminant
            .filter(|(index, _)| *index == slot)
            .map(|(_, tag)| tag)
    }
    /// Validate complete retained schemas, including unused and forward joins.
    /// The payload walker owns traversal accounting; only intersections need
    /// schema expansion, and a second walk has precisely admitted storage.
    fn validate_shape_type(
        &self,
        ty: &Type<'src>,
        span: Span,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AdmittedCheckError> {
        use super::type_payload::{measure_payload, Payload, PayloadError};
        let mut scope = budget.scope();
        let mut count = 0;
        let convert = |error: PayloadError<std::convert::Infallible>| match error {
            PayloadError::Allocation(error) => AdmittedCheckError::from(error),
            PayloadError::Visitor(never) => match never {},
        };
        measure_payload(Payload::Type(ty), &mut scope, |node| {
            if matches!(node, Payload::Type(Type::Intersection(_))) {
                count += 1;
            }
            Ok(())
        })
        .map_err(convert)?;
        if count == 0 {
            return Ok(());
        }
        let mut joins = scope.vector(AllocationClass::Scratch, count)?;
        measure_payload(Payload::Type(ty), &mut scope, |node| {
            if let Payload::Type(ty @ Type::Intersection(_)) = node {
                joins.push(ty);
            }
            Ok(())
        })
        .map_err(convert)?;
        for ty in joins {
            self.shape_fields(ty, span, &mut scope)?;
        }
        Ok(())
    }

    pub(super) fn validate_shapes(
        &self,
        module: Option<usize>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AdmittedCheckError> {
        for symbol in self.symbols() {
            budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
            if self.declarations.symbol_modules[symbol.id.0 as usize] == module {
                self.validate_shape_type(&symbol.ty, symbol.span, budget)?;
            }
        }
        for definition in self.structs() {
            if definition.module == module {
                for field in definition.fields.values() {
                    self.validate_shape_type(&field.ty, field.span, budget)?;
                }
            }
        }
        for definition in self.classes() {
            if definition.module == module {
                for field in definition.fields.values() {
                    self.validate_shape_type(&field.ty, field.span, budget)?;
                }
            }
        }
        for source in &self.facts.source_info {
            budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
            if let Some(expression) = source.expression {
                if let Some(ty) = self.expression_type(expression.id) {
                    self.validate_shape_type(ty, expression.span(), budget)?;
                    if let Type::ClassInstance { declaration, .. } = ty {
                        if self.is_shape(ty) && self.is_reflected(declaration.identity) {
                            let mut scope = budget.scope();
                            let fields = self.shape_fields(ty, expression.span(), &mut scope)?;
                            if fields
                                .iter()
                                .any(|(_, field)| matches!(field.ty, Type::TypeParameter(_)))
                            {
                                return Err(AdmittedCheckError::new(expression.span(),
                                    "an erased public shape field needs concrete type arguments to determine optional-key storage"));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn is_shape(&self, ty: &Type<'src>) -> bool {
        match ty {
            Type::Nullable(inner) => self.is_shape(inner),
            Type::Intersection(members) => {
                !members.is_empty() && members.iter().all(|ty| self.is_shape(ty))
            }
            _ => class_type_parts(ty)
                .and_then(|(declaration, _)| self.nominal_class(declaration.identity))
                .is_some_and(|info| info.shape),
        }
    }

    pub(crate) fn shape_fields(
        &self,
        ty: &Type<'src>,
        span: Span,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<(NominalId, FieldInfo<'src>)>, AdmittedCheckError> {
        let mut fields: Vec<(NominalId, FieldInfo<'src>)> = Vec::new();
        let members = match ty {
            Type::Intersection(members) => members.as_slice(),
            _ => std::slice::from_ref(ty),
        };
        let mut query = TypeQueryAdmission::new(budget);
        for member in members {
            query.work(1)?;
            let (declaration, arguments) = class_type_parts(member).ok_or_else(|| {
                AdmittedCheckError::new(span, "spread requires a non-absent declared shape")
            })?;
            let info = self
                .nominal_class(declaration.identity)
                .filter(|info| info.shape)
                .ok_or_else(|| AdmittedCheckError::new(span, "spread requires a declared shape"))?;
            for field in info.fields.values() {
                query.work(fields.len() + 1)?;
                let ty = substitute_type_with(
                    &field.ty,
                    &mut |identity, query: &mut TypeQueryAdmission<'_, '_>| {
                        query.work(info.type_params.len())?;
                        Ok(info
                            .type_params
                            .iter()
                            .position(|parameter| parameter.identity == identity)
                            .and_then(|index| arguments.get(index)))
                    },
                    &mut query,
                )?;
                let field = FieldInfo { ty, ..*field };
                if let Some((prior_owner, prior)) =
                    fields.iter().find(|(_, prior)| prior.name == field.name)
                {
                    if !type_equal_with(&prior.ty, &field.ty, &mut query)?
                        || prior.accessor != field.accessor
                    {
                        return Err(AdmittedCheckError::new(span,
                            format!("intersection field `{}` must have the same type and data/accessor kind", field.name)));
                    }
                    match (
                        self.field_tag(*prior_owner, prior.index),
                        self.field_tag(declaration.identity, field.index),
                    ) {
                        (Some(a), Some(b)) if a.same(b, span, &mut query)? => {}
                        (None, None) => {}
                        _ => {
                            return Err(AdmittedCheckError::new(
                                span,
                                "intersection discriminants must agree",
                            ))
                        }
                    }
                } else {
                    query.push_scratch(&mut fields, (declaration.identity, field))?;
                }
            }
        }
        Ok(fields)
    }
}

impl<'ast, 'src> Analyzer<'_, '_, 'ast, 'src> {
    pub(super) fn shape_fields(
        &mut self,
        ty: &Type<'src>,
        span: Span,
    ) -> Result<Vec<(NominalId, FieldInfo<'src>)>, AdmittedCheckError> {
        CheckedView {
            declarations: self.declarations,
            facts: self.facts,
        }
        .shape_fields(ty, span, self.budget)
    }

    pub(super) fn shape_name(&self, name: Ident<'src>) -> Option<&ClassInfo<'src>> {
        self.facts
            .type_bindings
            .get(name.name)
            .and_then(|&id| self.view().nominal_class(id))
            .filter(|info| info.shape)
    }

    pub(super) fn analyze_shape_literal(
        &mut self,
        name: Option<Ident<'src>>,
        entries: &'ast [RecordElement<'ast, 'src>],
        expected: Option<&Type<'src>>,
        span: Span,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let expected = expected.map(|ty| match ty {
            Type::Nullable(inner) => inner.as_ref(),
            ty => ty,
        });
        let ty = if let Some(name) = name {
            let info = self.shape_name(name).ok_or_else(|| {
                AdmittedCheckError::new(name.span, format!("unknown shape `{}`", name.name))
            })?;
            if info.type_params.is_empty() {
                Type::Class(info.declaration)
            } else if let Some(ty @ Type::ClassInstance { declaration, .. }) =
                expected.filter(|ty| {
                    class_type_parts(ty)
                        .is_some_and(|(declaration, _)| declaration == info.declaration)
                })
            {
                let _ = declaration;
                ty.clone()
            } else {
                return Err(AdmittedCheckError::new(
                    span,
                    "a generic shape literal needs its contextual type arguments",
                ));
            }
        } else {
            expected
                .expect("shape context selected this checker")
                .clone()
        };
        let fields = self.shape_fields(&ty, span)?;
        self.budget.work(
            crate::compilation_policy::WorkKind::Analysis,
            fields.len() as u64 + entries.len() as u64,
        )?;
        let mut provided = self
            .budget
            .filled(AllocationClass::Scratch, fields.len(), false)?;
        for entry in entries {
            match entry {
                RecordElement::Entry(entry) => {
                    let Some((index, (owner, field))) = fields
                        .iter()
                        .enumerate()
                        .find(|(_, (_, field))| field.name == entry.key.name)
                    else {
                        return Err(AdmittedCheckError::new(
                            entry.key.span,
                            format!("shape has no field `{}`", entry.key.name),
                        ));
                    };
                    if let Some(tag) = self.view().field_tag(*owner, field.index) {
                        let actual = ShapeTag::literal(&entry.value).ok_or_else(|| {
                            AdmittedCheckError::new(
                                entry.span,
                                "a shape tag override must be its declared literal",
                            )
                        })?;
                        if !tag.same(
                            actual,
                            entry.span,
                            &mut TypeQueryAdmission::new(self.budget),
                        )? {
                            return Err(AdmittedCheckError::new(
                                entry.span,
                                "a shape tag override must be its declared literal",
                            ));
                        }
                    }
                    let actual = self.analyze_expr(&entry.value, Some(&field.ty))?;
                    self.require_assignable(&field.ty, &actual, entry.value.span())?;
                    self.facts.source_info[entry.key.id.index()].resolution =
                        ExpressionResolution::NominalMember(field.member);
                    self.record_type(entry.key.id, &field.ty)?;
                    provided[index] = true;
                }
                RecordElement::Spread { value, span } => {
                    let actual = self.analyze_expr(value, None)?;
                    let source = self.shape_fields(&actual, *span)?;
                    for (incoming_owner, incoming) in source {
                        let Some((index, (owner, field))) = fields
                            .iter()
                            .enumerate()
                            .find(|(_, (_, field))| field.name == incoming.name)
                        else {
                            return Err(AdmittedCheckError::new(
                                *span,
                                format!(
                                    "spread field `{}` is not declared by the destination shape",
                                    incoming.name
                                ),
                            ));
                        };
                        let expected_tag = self.view().field_tag(*owner, field.index);
                        let incoming_tag = self.view().field_tag(incoming_owner, incoming.index);
                        if let Some(tag) = expected_tag {
                            let agrees = match incoming_tag {
                                Some(incoming) => tag.same(
                                    incoming,
                                    *span,
                                    &mut TypeQueryAdmission::new(self.budget),
                                )?,
                                None => false,
                            };
                            if !agrees {
                                return Err(AdmittedCheckError::new(
                                    *span,
                                    "spread must preserve the destination's declared tag",
                                ));
                            }
                        }
                        self.require_assignable(&field.ty, &incoming.ty, *span)?;
                        if !matches!(incoming.ty, Type::Nullable(_) | Type::Null) {
                            provided[index] = true;
                        }
                    }
                }
            }
        }
        for (index, (_, field)) in fields.iter().enumerate() {
            if !provided[index]
                && !field.has_initializer
                && !matches!(field.ty, Type::Nullable(_) | Type::Null)
            {
                return Err(AdmittedCheckError::new(
                    span,
                    format!("shape construction must provide `{}`", field.name),
                ));
            }
        }
        self.budget
            .release(AllocationClass::Scratch, provided.capacity() as u64)?;
        Ok(ty)
    }
}
