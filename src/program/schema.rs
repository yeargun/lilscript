//! Nominal field types are instantiated from their checked binder identities.
//! Verification and boundary formation share this substitution; storage recipes
//! continue to use stable declaration/member identities.
use super::*;
use crate::check::type_admission::TypeQueryAdmission;
use crate::check::type_substitution::substitute_type_with;
use crate::output_budget::{AllocationBudget, AllocationError};
use std::borrow::Cow;

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
