//! Nominal field types are instantiated from their checked binder identities.
//! Verification and boundary formation share this substitution; storage recipes
//! continue to use stable declaration/member identities.
use super::*;
use crate::check::type_admission::TypeQueryAdmission;
use crate::check::type_substitution::substitute_type_with;
use crate::output_budget::{AllocationBudget, AllocationError};
use std::borrow::Cow;
use crate::check::binary_types::TypeConstructionAdmission;

pub(super) fn is_shape(program: &Program<'_>, ty: &Type<'_>) -> bool {
    match ty {
        Type::Class(declaration) | Type::ClassInstance { declaration, .. } =>
            program.class(declaration.identity).is_some_and(|class| class.shape),
        Type::Intersection(members) => !members.is_empty() && members.iter().all(|ty| is_shape(program, ty)),
        Type::Nullable(inner) => is_shape(program, inner),
        _ => false,
    }
}

/// The ordered union of checked fields. A joined view never admits conflicting
/// field types/kinds, and keeps the original field identities for consumers.
pub(super) fn shape_fields<'src>(program: &Program<'src>, ty: &Type<'src>,
    query: &mut TypeQueryAdmission<'_, '_>) -> Result<Option<Vec<(FieldRef, Type<'src>)>>, AllocationError> {
    let members = match ty { Type::Intersection(members) => members.as_slice(), _ => std::slice::from_ref(ty) };
    let mut fields: Vec<(FieldRef, Type<'src>)> = Vec::new();
    for member in members {
        query.work(1)?;
        let (Type::Class(declaration) | Type::ClassInstance { declaration, .. }) = member else { return Ok(None); };
        let Some(class) = program.class(declaration.identity).filter(|class| class.shape) else { return Ok(None); };
        for (slot, &(key, _)) in class.fields.iter().enumerate() {
            query.work(fields.len() + 1)?;
            let field = FieldRef { nominal: declaration.identity, slot: u32::try_from(slot).map_err(|_| AllocationError::Capacity)? };
            let Some(ty) = class_field_type(program, member, field, query)? else { return Ok(None); };
            if let Some((prior, prior_ty)) = fields.iter().find(|(prior,_)| program.class_field(*prior).is_some_and(|(name,_)| name == key)) {
                if !crate::check::type_relation::type_equal_with(prior_ty, &ty, query)?
                    || program.class(prior.nominal).unwrap().accessors[prior.slot as usize] != class.accessors[slot] {
                    return Ok(None);
                }
                let prior_tag = program.class(prior.nominal).unwrap().discriminant
                    .filter(|(slot,_)| *slot == prior.slot).map(|(_,tag)| tag);
                let tag = class.discriminant.filter(|(tag_slot,_)| *tag_slot as usize == slot).map(|(_,tag)| tag);
                let same = match (prior_tag, tag) {
                    (Some(Constant::String(a)), Some(Constant::String(b))) => {
                        query.work(program.strings[a.index()].storage_bytes() + program.strings[b.index()].storage_bytes())?;
                        program.strings[a.index()] == program.strings[b.index()]
                    }
                    _ => prior_tag == tag,
                };
                if !same { return Ok(None); }
            } else {
                let ty = query.clone_type(&ty)?;
                // This vector is query scratch, paid through the same owner as
                // field substitution; type payloads retain their own admission.
                query.push_scratch(&mut fields, (field, ty))?;
            }
        }
    }
    Ok(Some(fields))
}

pub(super) fn struct_definition<'a, 'src>(
    program: &'a Program<'src>,
    ty: &Type<'src>,
) -> Option<&'a StructDefinition> {
    let (declaration, arity) = match ty {
        Type::Struct(declaration) => (declaration, 0),
        Type::StructInstance { declaration, args } => (declaration, args.len()),
        _ => return None,
    };
    program
        .structs
        .get(declaration.identity.index())
        .filter(|schema| {
            schema.identity == declaration.identity && schema.type_parameters.len() == arity
        })
}

pub(super) fn field_type<'a, 'src>(
    program: &'a Program<'src>,
    owner: &Type<'src>,
    field: &Field,
    query: &mut TypeQueryAdmission<'_, '_>,
) -> Result<Option<Cow<'a, Type<'src>>>, AllocationError> {
    let Some(schema) =
        struct_definition(program, owner).filter(|schema| schema.identity == field.owner)
    else {
        return Ok(None);
    };
    let ty = &program.types[field.ty.index()];
    let Type::StructInstance { args, .. } = owner else {
        return Ok(Some(Cow::Borrowed(ty)));
    };
    let resolved = substitute_type_with(
        ty,
        &mut |id, query: &mut TypeQueryAdmission<'_, '_>| {
            query.work(schema.type_parameters.len())?;
            Ok(schema
                .type_parameters
                .iter()
                .position(|parameter| *parameter == id)
                .and_then(|index| args.get(index)))
        },
        query,
    )?;
    Ok(Some(Cow::Owned(resolved)))
}

/// Follow a checked value-place path once. Structs retain their semantic type
/// even when a physical recipe later flattens or shares their storage.
pub(super) fn place_type<'a, 'src>(
    program: &'a Program<'src>,
    data: &UnitData,
    place: PlaceId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<Cow<'a, Type<'src>>>, AllocationError> {
    budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
    match data.places[place.index()] {
        Place::Cell(cell) => Ok(Some(Cow::Borrowed(
            &program.types[program.cells[cell.index()].ty.index()],
        ))),
        Place::Value(value) => Ok(Some(Cow::Borrowed(
            &program.types[data.values[value.index()].ty.index()],
        ))),
        Place::Field { base, field } => {
            let Some(owner) = place_type(program, data, base, budget)? else {
                return Ok(None);
            };
            field_type(
                program,
                &owner,
                program.field(field).expect("verified member"),
                &mut TypeQueryAdmission::new(budget),
            )
        }
        Place::Index { receiver, .. } => Ok(
            match &program.types[data.values[receiver.index()].ty.index()] {
                Type::Array(inner) | Type::Record(inner) => Some(Cow::Borrowed(inner.as_ref())),
                _ => None,
            },
        ),
        Place::ClassField { receiver, field } => class_field_type(
            program,
            &program.types[data.values[receiver.index()].ty.index()],
            field,
            &mut TypeQueryAdmission::new(budget),
        ),
        _ => Ok(None),
    }
}

pub(super) fn class_field_type<'a, 'src>(
    program: &'a Program<'src>,
    receiver: &Type<'src>,
    field: FieldRef,
    query: &mut TypeQueryAdmission<'_, '_>,
) -> Result<Option<Cow<'a, Type<'src>>>, AllocationError> {
    if let Type::Intersection(members) = receiver {
        for member in members {
            query.work(1)?;
            if matches!(member, Type::Class(d) | Type::ClassInstance { declaration: d, .. } if d.identity == field.nominal) {
                return class_field_type(program, member, field, query);
            }
        }
        return Ok(None);
    }
    let (declaration, arguments) = match receiver {
        Type::Class(declaration) => (declaration, &[][..]),
        Type::ClassInstance { declaration, args } => (declaration, args.as_slice()),
        _ => return Ok(None),
    };
    let Some(class) = program.class(declaration.identity) else {
        return Ok(None);
    };
    if class.type_params.len() != arguments.len() {
        return Ok(None);
    }
    let Some((_, ty)) = class.fields.get(field.slot as usize) else {
        return Ok(None);
    };
    let ty = &program.types[ty.index()];
    if arguments.is_empty() {
        return Ok(Some(Cow::Borrowed(ty)));
    }
    let resolved = substitute_type_with(
        ty,
        &mut |id, query: &mut TypeQueryAdmission<'_, '_>| {
            query.work(class.type_params.len())?;
            Ok(class
                .type_params
                .iter()
                .position(|parameter| *parameter == id)
                .and_then(|index| arguments.get(index)))
        },
        query,
    )?;
    Ok(Some(Cow::Owned(resolved)))
}

/// Instantiate a constructor's runtime signature using its class's checked
/// binders. Construction, rest packing and verification share this forward
/// substitution; no target re-infers type arguments from actual operands.
pub(super) fn class_signature<'src>(
    program: &Program<'src>,
    class: NominalId,
    arguments: &[Type<'src>],
    signature: &crate::check::FunctionType<'src>,
    query: &mut TypeQueryAdmission<'_, '_>,
) -> Result<Option<crate::check::FunctionType<'src>>, AllocationError> {
    let Some(definition) = program.class(class) else {
        return Ok(None);
    };
    if definition.type_params.len() != arguments.len() {
        return Ok(None);
    }
    if arguments.is_empty() {
        return Ok(Some(signature.clone()));
    }
    crate::check::type_substitution::substitute_signature_with(
        signature,
        &mut |id, query: &mut TypeQueryAdmission<'_, '_>| {
            query.work(definition.type_params.len())?;
            Ok(definition
                .type_params
                .iter()
                .position(|parameter| *parameter == id)
                .map(|index| &arguments[index]))
        },
        query,
    )
    .map(Some)
}
