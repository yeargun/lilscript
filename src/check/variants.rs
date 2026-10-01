//! Payload sums reuse nominal unions, with one checked identity domain.
use super::*;

impl<'ast, 'src> Analyzer<'_, '_, 'ast, 'src> {
    pub(super) fn analyze_payload_match(
        &mut self,
        value: &Type<'src>,
        arms: &'ast [crate::ast::MatchArm<'ast, 'src>],
        expected: Option<&Type<'src>>,
        span: Span,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let members = match value {
            Type::Union(members) => members.as_slice(),
            Type::Class(_) | Type::ClassInstance { .. } => std::slice::from_ref(value),
            _ => {
                return Err(AdmittedCheckError::new(
                    span,
                    "payload matching requires a closed nominal union",
                ))
            }
        };
        // A tag/identity test must distinguish every pair. Testing erased type
        // arguments, overlapping ancestors, or unrelated shape keys cannot do so.
        for (index, member) in members.iter().enumerate() {
            self.budget
                .work(crate::compilation_policy::WorkKind::Analysis, 1)?;
            let Some((declaration, _)) = class_type_parts(member) else {
                return Err(AdmittedCheckError::new(
                    span,
                    "each payload variant must be a class or tagged shape",
                ));
            };
            self.class_guard(value, member, span)?;
            let info = &self.declarations.classes[declaration.identity.index()];
            for previous in &members[..index] {
                self.budget
                    .work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                let (other, _) = class_type_parts(previous).unwrap();
                let other = &self.declarations.classes[other.identity.index()];
                let disjoint = if info.shape && other.shape {
                    let (slot, tag) = info.discriminant.unwrap();
                    let (other_slot, other_tag) = other.discriminant.unwrap();
                    info.fields.get_index(slot).unwrap().0
                        == other.fields.get_index(other_slot).unwrap().0
                        && !tag.same(
                            other_tag,
                            span,
                            &mut type_admission::TypeQueryAdmission::new(self.budget),
                        )?
                } else {
                    !info.shape
                        && !other.shape
                        && !self.is_assignable(member, previous)
                        && !self.is_assignable(previous, member)
                };
                if !disjoint {
                    return Err(AdmittedCheckError::new(span, "payload variants need disjoint class identities or distinct values of the same shape tag"));
                }
            }
        }
        let mut covered = self
            .budget
            .vector(AllocationClass::Scratch, members.len())?;
        covered.resize(members.len(), false);
        let mut wildcard = false;
        let mut result = None;
        for (index, arm) in arms.iter().enumerate() {
            let mut nested_arrow_end = 0;
            let mut suspension = None;
            let mut work = Ok(());
            crate::ast_walk::expression(&arm.value, &mut |expression| {
                if work.is_err() {
                    return;
                }
                work = self
                    .budget
                    .work(crate::compilation_policy::WorkKind::Analysis, 1);
                if expression.span().start < nested_arrow_end {
                    return;
                }
                match expression.kind {
                    ExprKind::ArrowFunction { .. } => nested_arrow_end = expression.span().end,
                    ExprKind::Await { .. } => suspension = Some(expression.span()),
                    _ => {}
                }
            });
            work?;
            if let Some(span) = suspension {
                return Err(AdmittedCheckError::new(span, "payload arms require a non-suspending expression; await before matching or call a separate async function (R8)"));
            }
            self.push_scope()?;
            match arm.pattern {
                MatchPattern::Payload {
                    variant,
                    binding,
                    span,
                } => {
                    let identity = self
                        .facts
                        .type_bindings
                        .get(variant.name)
                        .copied()
                        .ok_or_else(|| {
                            AdmittedCheckError::new(variant.span, "unknown payload variant type")
                        })?;
                    let position = members
                        .iter()
                        .position(|member| {
                            class_type_parts(member).is_some_and(|(d, _)| d.identity == identity)
                        })
                        .ok_or_else(|| {
                            AdmittedCheckError::new(
                                span,
                                "payload pattern must name a member of the exact union",
                            )
                        })?;
                    if covered[position] {
                        return Err(AdmittedCheckError::new(
                            span,
                            "duplicate payload variant arm",
                        ));
                    }
                    covered[position] = true;
                    self.record_type_binding(variant.id, &members[position])?;
                    self.declare(binding, members[position].clone())?;
                }
                MatchPattern::Wildcard(span) if index + 1 == arms.len() => {
                    let _ = span;
                    wildcard = true;
                }
                _ => {
                    return Err(AdmittedCheckError::new(
                        arm.pattern.span(),
                        "a payload match has typed bindings and an optional final `_`",
                    ))
                }
            }
            let actual = self.analyze_expr(&arm.value, expected)?;
            self.pop_scope();
            result = Some(match result {
                Some(previous) => common_type(&previous, &actual).ok_or_else(|| {
                    AdmittedCheckError::new(
                        arm.span,
                        "payload match arms need a common result type",
                    )
                })?,
                None => actual,
            });
        }
        if !wildcard && covered.iter().any(|covered| !covered) {
            return Err(AdmittedCheckError::new(
                span,
                "non-exhaustive payload match; cover every variant or add a final `_`",
            ));
        }
        self.budget
            .release(AllocationClass::Scratch, covered.capacity() as u64)?;
        result.ok_or_else(|| AdmittedCheckError::new(span, "a match requires at least one arm"))
    }
}
