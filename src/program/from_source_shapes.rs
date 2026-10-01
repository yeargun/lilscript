//! Shape construction snapshots each declared spread key at its source position.
use super::*;

impl<'sem, 'ast, 'src> Lower<'_, '_, 'sem, 'ast, 'src> {
    fn shape_fallback(
        &mut self,
        unit: UnitId,
        region: RegionId,
        left: ValueId,
        right: RegionId,
        fallback: ValueId,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let left_type = self.units[unit.index()].values[left.index()].ty;
        let right_type = self.units[unit.index()].values[fallback.index()].ty;
        let result = crate::check::binary_types::checked_binary_type_with(
            BinaryOp::Nullish,
            &self.program.types[left_type.index()],
            &self.program.types[right_type.index()],
            &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget),
        )
        .map_err(|error| match error {
            crate::check::binary_types::BinaryTypeError::Admission(error) => {
                ConversionError::Resources(error)
            }
            crate::check::binary_types::BinaryTypeError::Semantic(_) => Unsupported {
                span,
                feature: "checked shape fallback types",
            }
            .into(),
        })?;
        let ty = self.ty(&result)?;
        self.units[unit.index()].regions[right.index()].result = Some(fallback);
        self.value(
            unit,
            region,
            OperationKind::ShortCircuit {
                kind: ShortCircuit::Nullish,
                right,
            },
            &[left],
            ty,
            None,
            span,
        )
    }

    pub(super) fn construct_shape(
        &mut self,
        unit: UnitId,
        region: RegionId,
        entries: &'ast [ast::RecordElement<'ast, 'src>],
        ty: TypeId,
        origin: Option<SourceNodeId>,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let fields = self
            .semantics
            .shape_fields(&self.program.types[ty.index()], span, self.budget)
            .map_err(|error| shape_schema_error(error, span))?;
        let mut values = self.budget.filled(Scratch, fields.len(), None)?;
        let mut default_if_absent = self.budget.filled(Scratch, fields.len(), false)?;
        for entry in entries {
            self.work(1)?;
            match entry {
                RecordElement::Entry(entry) => {
                    let index = fields
                        .iter()
                        .position(|(_, field)| field.name == entry.key.name)
                        .ok_or(Unsupported {
                            span: entry.span,
                            feature: "checked shape key",
                        })?;
                    let value = self.expression(unit, region, &entry.value)?;
                    values[index] = Some(self.copy_value(unit, region, value, entry.span)?);
                    default_if_absent[index] = false;
                }
                RecordElement::Spread { value, span } => {
                    let source = self.expression(unit, region, value)?;
                    let source_ty = self.units[unit.index()].values[source.index()].ty;
                    let source_fields = self
                        .semantics
                        .shape_fields(&self.program.types[source_ty.index()], *span, self.budget)
                        .map_err(|error| shape_schema_error(error, *span))?;
                    for (owner, field) in source_fields {
                        self.work(fields.len() + 1)?;
                        let index = fields
                            .iter()
                            .position(|(_, target)| target.name == field.name)
                            .ok_or(Unsupported {
                                span: *span,
                                feature: "checked spread field",
                            })?;
                        let field_ty = self.ty(&field.ty)?;
                        let place = self.push_place(
                            unit,
                            Place::ClassField {
                                receiver: source,
                                field: FieldRef {
                                    nominal: owner,
                                    slot: u32::try_from(field.index)
                                        .map_err(|_| AllocationError::Capacity)?,
                                },
                            },
                        )?;
                        let value = self.value(
                            unit,
                            region,
                            OperationKind::Load(place),
                            &[],
                            field_ty,
                            None,
                            *span,
                        )?;
                        let mut value = self.copy_value(unit, region, value, *span)?;
                        // An absent optional source key contributes no property. It
                        // cannot erase a preceding explicit value or earlier spread.
                        if matches!(field.ty, Type::Nullable(_) | Type::Null) {
                            if let Some(prior) = values[index] {
                                let right = self.region(unit, region, *span)?;
                                value =
                                    self.shape_fallback(unit, region, value, right, prior, *span)?;
                            } else {
                                default_if_absent[index] = fields[index].1.has_initializer;
                            }
                        } else {
                            default_if_absent[index] = false;
                        }
                        values[index] = Some(value);
                    }
                }
            }
        }
        let mut operands = self.budget.vector(Scratch, fields.len())?;
        let mut keys = self.budget.vector(Retained, fields.len())?;
        for (index, (owner, field)) in fields.iter().enumerate() {
            self.work(1)?;
            let field_ty = self.ty(&field.ty)?;
            let value = match values[index] {
                Some(value) if default_if_absent[index] => {
                    let right = self.region(unit, region, span)?;
                    let fallback =
                        self.field_value(unit, right, *owner, field.index, field_ty, span)?;
                    self.shape_fallback(unit, region, value, right, fallback, span)?
                }
                Some(value) => value,
                None => self.field_value(unit, region, *owner, field.index, field_ty, span)?,
            };
            self.budget.push(Scratch, &mut operands, value)?;
            let key = self.string(field.name)?;
            self.budget.push(Retained, &mut keys, key)?;
        }
        let kind = match &self.program.types[ty.index()] {
            Type::Class(declaration) | Type::ClassInstance { declaration, .. } => {
                AllocationKind::Instance {
                    class: declaration.identity,
                    keys,
                }
            }
            Type::Intersection(_) => AllocationKind::Object(keys),
            _ => return self.unsupported(span, "shape construction type"),
        };
        let allocation = self.allocation(unit, kind)?;
        let result = self.value(unit, region, allocation, &operands, ty, origin, span)?;
        drop_vector(values, Scratch, self.budget)?;
        drop_vector(default_if_absent, Scratch, self.budget)?;
        drop_vector(operands, Scratch, self.budget)?;
        Ok(result)
    }
}

fn shape_schema_error(error: crate::check::AdmittedCheckError, span: Span) -> ConversionError {
    match error {
        crate::check::AdmittedCheckError::Resources(error) => ConversionError::Resources(error),
        crate::check::AdmittedCheckError::Semantic(_) => Unsupported {
            span,
            feature: "checked shape schema",
        }
        .into(),
    }
}
