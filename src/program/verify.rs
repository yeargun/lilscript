use super::*;
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};
#[path = "verify_scratch.rs"]
mod scratch;
use crate::check::binary_types::{checked_binary_type_with, BinaryTypeError};
use crate::check::type_admission::TypeQueryAdmission;
use crate::check::type_relation::{
    is_type_assignable_with as structurally_assignable, type_equal_with, RelationAdmission, RelationEvent,
};
use crate::check::type_substitution::substitute_signature_with;

#[cfg(test)]
#[path = "verify_admission_tests.rs"]
mod admission_tests;
use scratch::work;
use std::cmp::Ordering;

#[derive(Debug)]
pub(super) enum VerificationError {
    Invalid(&'static str),
    TypeMismatch {
        unit: UnitId,
        operation: OpId,
        span: crate::span::Span,
    },
    Allocation(AllocationError),
}
impl From<&'static str> for VerificationError {
    fn from(reason: &'static str) -> Self {
        Self::Invalid(reason)
    }
}
impl From<AllocationError> for VerificationError {
    fn from(error: AllocationError) -> Self {
        Self::Allocation(error)
    }
}
impl VerificationError {
    // Only standalone inspection renders operation payloads. Fixed edits carry
    // bounded IDs/span and preserve admission failures as their own variant.
    pub(super) fn into_string(self, program: &Program<'_>) -> String {
        match self {
            Self::Invalid(reason) => reason.to_owned(),
            Self::TypeMismatch {
                unit, operation, ..
            } => {
                let data = program.unit(unit).expect("verified unit identity");
                let operation = &data.operations[operation.index()];
                if std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
                    let operands = data.operands(operation.operands).unwrap_or(&[]);
                    eprintln!(
                        "type mismatch in module {} unit {:?} ({:?}) at {:?}: result {:?}, operands {:?}",
                        data.module.index(),
                        data.function_name.map(|name| &program.strings[name.index()]),
                        data.callable_type.map(|ty| &program.types[ty.index()]),
                        operation.span,
                        operation.result.map(|value| &program.types[data.values[value.index()].ty.index()]),
                        operands
                            .iter()
                            .map(|value| &program.types[data.values[value.index()].ty.index()])
                            .collect::<Vec<_>>(),
                    );
                }
                format!("semantic operation type mismatch: {:?}", operation.kind)
            }
            Self::Allocation(error) => error.to_string(),
        }
    }
}

fn type_matches(
    left: Option<&Type<'_>>,
    right: Option<&Type<'_>>,
    query: &mut TypeQueryAdmission<'_, '_>,
) -> Result<bool, VerificationError> {
    match (left, right) {
        (Some(left), Some(right)) => Ok(type_equal_with(left, right, query)?),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}

fn validate_signature(
    signature: &crate::check::FunctionSignature<'_>,
    query: &mut TypeQueryAdmission<'_, '_>,
) -> Result<(), VerificationError> {
    query.admit(RelationEvent::SignatureValidation(signature))?;
    signature
        .validate_parameters()
        .map_err(VerificationError::Invalid)
}

/// One borrowed type/root summary per canonical place; later operation checks
/// need no recursive projection walk or repeated nominal table search.
#[derive(Clone, Copy)]
struct VerifiedPlace<'program, 'src> {
    ty: Option<&'program Type<'src>>,
    root: PlaceId,
    writable: bool,
}

fn verify_places<'program, 'src>(
    program: &'program Program<'src>,
    unit: &'program UnitData,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<VerifiedPlace<'program, 'src>>, VerificationError> {
    let value_type = |value: ValueId| {
        unit.values
            .get(value.index())
            .and_then(|value| program.types.get(value.ty.index()))
            .ok_or("place has a dangling value or value type")
    };
    let mut places: Vec<VerifiedPlace<'program, 'src>> =
        budget.vector(Scratch, unit.places.len())?;
    work(budget, unit.places.len())?;
    for (index, place) in unit.places.iter().enumerate() {
        let own = PlaceId::from_index(index).ok_or("place table exceeds capacity")?;
        let (ty, root, writable) = match *place {
            Place::Cell(cell) => {
                let cell = program
                    .cells
                    .get(cell.index())
                    .ok_or("dangling place cell")?;
                (
                    Some(&program.types[cell.ty.index()]),
                    own,
                    cell.binding != CellBinding::Foreign,
                )
            }
            Place::Value(value) => (Some(value_type(value)?), own, false),
            Place::Field { base, field } => {
                if base.index() >= index {
                    return Err("field projection base must precede its place".into());
                }
                let parent = places[base.index()];
                TypeQueryAdmission::new(budget)
                    .work((usize::BITS - program.fields.len().leading_zeros()) as usize + 1)?;
                let field = program.field(field).ok_or("unknown semantic field")?;
                let owner = program
                    .structs
                    .get(field.owner.index())
                    .filter(|owner| owner.identity == field.owner)
                    .ok_or("unknown semantic field owner")?;
                let identity = match parent.ty {
                    Some(Type::Struct(declaration)) if owner.type_parameters.is_empty() => {
                        declaration.identity
                    }
                    Some(Type::StructInstance { .. }) | Some(Type::Struct(_)) => {
                        return Err(
                            "generic struct projection requires an instantiated schema".into()
                        );
                    }
                    _ => return Err("field projection base is not a struct value".into()),
                };
                if identity != owner.identity {
                    return Err("field projection has an incompatible nominal owner".into());
                }
                (
                    Some(&program.types[field.ty.index()]),
                    parent.root,
                    parent.writable,
                )
            }
            Place::Member { receiver, key } => {
                if key.index() >= program.strings.len() {
                    return Err("invalid member key".into());
                }
                let ty = match value_type(receiver)? {
                    Type::Array(element) | Type::Record(element) => Some(element.as_ref()),
                    dynamic @ Type::TypeParameter("$js") => Some(dynamic),
                    // An internal class declares its flattened fields.
                    Type::Class(name) => {
                        TypeQueryAdmission::new(budget).work(program.classes.len() + 1)?;
                        program
                            .class(name)
                            .filter(|class| !class.external)
                            .and_then(|class| class.fields.iter().find(|(field, _)| *field == key))
                            .map(|(_, ty)| &program.types[ty.index()])
                    }
                    _ => None,
                };
                (ty, own, true)
            }
            Place::Index { receiver, key } => {
                value_type(key)?;
                let ty = match value_type(receiver)? {
                    Type::Array(element) | Type::Record(element) => Some(element.as_ref()),
                    dynamic @ Type::TypeParameter("$js") => Some(dynamic),
                    _ => None,
                };
                (ty, own, true)
            }
        };
        places.push(VerifiedPlace { ty, root, writable });
    }
    Ok(places)
}

pub(super) fn verify(program: &Program<'_>) -> Result<(), String> {
    verify_selected(program, None, &mut AllocationBudget::new(None))
        .map(|_| ())
        .map_err(|error| error.into_string(program))
}

pub(super) fn verify_admitted(
    program: &Program<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<VerificationReceipt, VerificationError> {
    verify_selected(program, None, budget)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct VerificationReceipt {
    pub units: usize,
    pub operations: usize,
}

/// Publication derives this set from actual changed revisions. Unselected
/// bodies and the exact unchanged type-table owner must already be certified;
/// this path reuses that table validation for unit-only replacements.
pub(super) fn verify_replacements(
    program: &Program<'_>,
    changed: &[UnitId],
) -> Result<VerificationReceipt, String> {
    verify_selected(program, Some(changed), &mut AllocationBudget::new(None))
        .map_err(|error| error.into_string(program))
}

fn verify_selected(
    program: &Program<'_>,
    selected: Option<&[UnitId]>,
    budget: &mut AllocationBudget<'_>,
) -> Result<VerificationReceipt, VerificationError> {
    let mut scope = budget.scope();
    let named_functions = verify_tables(program, &mut scope)?;
    match selected {
        Some(units) => verify_units(
            program,
            units.iter().copied(),
            Some(&named_functions),
            &mut scope,
        ),
        None => verify_units(
            program,
            program.units.iter().map(FrozenUnit::id),
            Some(&named_functions),
            &mut scope,
        ),
    }
}

fn verify_tables(
    program: &Program<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, VerificationError> {
    let fail = |message: &'static str| Err(VerificationError::Invalid(message));
    verify_type_contracts(program, budget)?;
    verify_modules(program, budget)?;
    let mut named_functions = budget.filled(Scratch, program.units.len(), 0usize)?;
    let mut scope = budget.scope();
    let budget = &mut scope;
    let mut enum_names = budget.vector(Scratch, program.enums.len())?;
    let mut enum_name_work = 1;
    work(budget, program.enums.len())?;
    for definition in program.enums.iter() {
        enum_names.push(definition.name.as_str());
        enum_name_work = enum_name_work.max(definition.name.len().saturating_add(1));
        let mut variants = budget.scope();
        let mut names = variants.vector(Scratch, definition.variants.len())?;
        let mut values = variants.vector(Scratch, definition.variants.len())?;
        let mut name_work = 1;
        work(&mut variants, definition.variants.len())?;
        for variant in &definition.variants {
            names.push(variant.name.as_str());
            values.push(variant.value);
            name_work = name_work.max(variant.name.len().saturating_add(1));
        }
        if !scratch::unique(&mut names, &mut variants, name_work, Ord::cmp)?
            || !scratch::unique(&mut values, &mut variants, 1, Ord::cmp)?
        {
            return fail("duplicate enum variant");
        }
    }
    if !scratch::unique(&mut enum_names, budget, enum_name_work, Ord::cmp)? {
        return fail("duplicate enum declaration");
    }
    work(budget, program.cells.len())?;
    let checked_cells = program
        .cells
        .iter()
        .position(|cell| cell.synthetic)
        .unwrap_or(program.cells.len());
    for (index, cell) in program.cells.iter().enumerate() {
        let Some(owner) = program.unit(cell.owner) else {
            return fail("cell has a dangling unit owner");
        };
        let identity = if index < checked_cells {
            !cell.synthetic && cell.source_symbol.0 as usize == index
        } else {
            cell.synthetic
                && (cell.source_symbol.0 as usize) < checked_cells
                && matches!(cell.binding, CellBinding::Local | CellBinding::Function(_))
        };
        if !identity
            || cell.ty.index() >= program.types.len()
            || cell.region.index() >= owner.regions.len()
        {
            return fail("invalid cell identity, type or region");
        }
        if let CellBinding::Parameter(position) = cell.binding {
            if owner.parameters.get(position as usize).map(|id| id.index()) != Some(index)
                || program
                    .parameter(CellId::from_index(index).unwrap())
                    .is_none()
            {
                return fail("parameter binding has an invalid owning ordinal");
            }
        }
        if let CellBinding::Function(function) = cell.binding {
            named_functions[cell.owner.index()] += 1;
            let Some(function) = program.unit(function) else {
                return fail("dangling function binding");
            };
            work(
                budget,
                cell.name
                    .len()
                    .checked_add(1)
                    .ok_or(AllocationError::Capacity)?,
            )?;
            if function.kind != UnitKind::Function
                || function.module != owner.module
                || !function
                    .function_name
                    .and_then(|id| program.strings.get(id.index()))
                    .is_some_and(|name| name.as_unicode() == Some(cell.name.as_str()))
            {
                return fail("function declaration lost its creation name or kind");
            }
            let mut query_scope = budget.scope();
            if !type_matches(
                function
                    .callable_type
                    .and_then(|id| program.types.get(id.index())),
                program.types.get(cell.ty.index()),
                &mut TypeQueryAdmission::new(&mut query_scope),
            )? {
                return fail("function declaration disagrees with its owned signature");
            }
        }
    }
    let mut previous_member = None;
    work(budget, program.fields.len())?;
    for field in program.fields.iter() {
        if previous_member
            .replace(field.identity.index())
            .is_some_and(|previous| previous >= field.identity.index())
        {
            return fail("nominal fields are not in canonical member order");
        }
        if field.ty.index() >= program.types.len() {
            return fail("invalid field identity or type");
        }
    }
    let mut nominal_names = budget.vector(Scratch, program.structs.len())?;
    let mut nominal_name_work = 2;
    let mut owned_fields = budget.filled(Scratch, program.fields.len(), false)?;
    work(budget, program.structs.len())?;
    for (definition_index, definition) in program.structs.iter().enumerate() {
        let Some(members) = program.fields.get(definition.fields.clone()) else {
            return fail("invalid nominal field range");
        };
        if definition.identity.is_class()
            || definition.identity.index() != definition_index
            || definition.module.index() >= program.modules.len()
            || definition.span.start > definition.span.end
        {
            return fail("invalid nominal definition identity");
        }
        nominal_names.push((definition.module, definition.name.as_str()));
        nominal_name_work = nominal_name_work.max(definition.name.len().saturating_add(2));
        {
            let mut parameters = budget.scope();
            let mut names = parameters.vector(Scratch, definition.type_parameters.len())?;
            let mut name_work = 1;
            work(&mut parameters, definition.type_parameters.len())?;
            for name in &definition.type_parameters {
                names.push(name.as_str());
                name_work = name_work.max(name.len().saturating_add(1));
            }
            if !scratch::unique(&mut names, &mut parameters, name_work, Ord::cmp)? {
                return fail("invalid nominal definition identity");
            }
        }
        work(budget, members.len())?;
        for (index, field) in members.iter().enumerate() {
            if std::mem::replace(&mut owned_fields[definition.fields.start + index], true)
                || field.owner != definition.identity
                || field.index != index
            {
                return fail("nominal field has an incorrect or duplicate owner");
            }
        }
    }
    if !scratch::unique(&mut nominal_names, budget, nominal_name_work, Ord::cmp)? {
        return fail("invalid nominal definition identity");
    }
    work(budget, owned_fields.len())?;
    if owned_fields.iter().any(|owned| !owned) {
        return fail("field has no nominal definition");
    }
    Ok(named_functions)
}

/// Only the transaction-owned footprint can reuse unchanged table validation.
pub(super) fn verify_fixed_replacements(
    edits: &super::publication::FixedUnitEdits<'_, '_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<VerificationReceipt, VerificationError> {
    verify_units(
        edits.program(),
        edits.changes().iter().map(|change| change.unit),
        None,
        budget,
    )
}

fn verify_units(
    program: &Program<'_>,
    selected: impl IntoIterator<Item = UnitId>,
    named_functions: Option<&[usize]>,
    budget: &mut AllocationBudget<'_>,
) -> Result<VerificationReceipt, VerificationError> {
    let fail = |message: &'static str| Err(VerificationError::Invalid(message));
    let mut receipt = VerificationReceipt::default();
    for id in selected {
        let mut unit_scope = budget.scope();
        let budget = &mut unit_scope;
        work(budget, 1)?;
        let index = id.index();
        let frozen = program
            .units
            .get(index)
            .ok_or("replacement names a missing unit")?;
        if frozen.id().index() != index {
            return fail("unit identity does not match program owner");
        }
        let unit = frozen.data();
        let places = verify_places(program, unit, budget).inspect_err(|error| {
            if std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
                eprintln!(
                    "place verification failed in module {} unit {:?}: {error:?}",
                    unit.module.index(),
                    unit.function_name.map(|name| &program.strings[name.index()]),
                );
            }
        })?;
        receipt.units += 1;
        receipt.operations += unit.operations.len();
        match (unit.kind, unit.function_name) {
            (UnitKind::ModuleInitialization, None) => {}
            (UnitKind::Function | UnitKind::Closure, Some(name))
                if name.index() < program.strings.len() => {}
            _ => return fail("invalid callable creation name"),
        }
        // Fixed edits cannot alter callable/header/parameter metadata. Its
        // previous full verification already established this equality. Calls,
        // Returns and Closure results still query the declared types below.
        if named_functions.is_some() {
            if let Some(signature) = callable_signature(program, unit)? {
                let mut scope = budget.scope();
                let mut query = TypeQueryAdmission::new(&mut scope);
                validate_signature(signature, &mut query)?;
                if signature.params.len() != unit.parameters.len() {
                    return fail("callable parameters disagree with its owned signature");
                }
                for (parameter, cell) in signature.params.iter().zip(&unit.parameters) {
                    query.admit(RelationEvent::ParameterPair)?;
                    let actual = program
                        .cells
                        .get(cell.index())
                        .and_then(|cell| program.types.get(cell.ty.index()));
                    if !type_matches(actual, Some(&parameter.ty), &mut query)? {
                        return fail("callable parameters disagree with its owned signature");
                    }
                }
            }
        }
        if unit.entry.index() >= unit.regions.len()
            || unit.regions[unit.entry.index()].parent.is_some()
        {
            return fail("invalid unit entry region");
        }
        let prefix_uses = verify_instantiation_prefix(
            program,
            frozen.id(),
            named_functions.map_or(unit.instantiation_prefix as usize / 2, |counts| {
                counts[index]
            }),
            budget,
        )?;
        let prefix_use = |value: ValueId, operation: OpId| -> Result<(), VerificationError> {
            if prefix_uses
                .get(value.index())
                .copied()
                .flatten()
                .is_some_and(|owner| owner != operation)
            {
                Err("instantiated callable value escapes its initialization pair".into())
            } else {
                Ok(())
            }
        };
        work(budget, unit.parameters.len())?;
        for (position, parameter) in unit.parameters.iter().enumerate() {
            if !program.cells.get(parameter.index()).is_some_and(|cell| {
                cell.owner == frozen.id()
                    && cell.region == unit.entry
                    && cell.binding == CellBinding::Parameter(position as u32)
            }) {
                return fail("invalid parameter cell");
            }
        }
        let mut captures = budget.copy_slice(Scratch, &unit.captures)?;
        let unique_captures = scratch::unique(&mut captures, budget, 1, Ord::cmp)?;
        work(
            budget,
            captures
                .len()
                .checked_mul(2)
                .ok_or(AllocationError::Capacity)?,
        )?;
        if captures
            .iter()
            .any(|&cell| program.is_reference_parameter(cell))
            || !unique_captures
            || captures.iter().any(|id| {
                !program
                    .cells
                    .get(id.index())
                    .is_some_and(|cell| cell.owner != frozen.id())
            })
        {
            return fail("invalid or duplicate capture cell");
        }
        let capture_lookup = (usize::BITS - captures.len().leading_zeros()) as usize + 1;
        let mut region_seen = budget.filled(Scratch, unit.regions.len(), false)?;
        let mut op_seen = budget.filled(Scratch, unit.operations.len(), false)?;
        let mut defined = budget.filled(Scratch, unit.values.len(), false)?;
        let mut available = budget.filled(Scratch, unit.values.len(), false)?;
        let mut active_regions = budget.filled(Scratch, unit.regions.len(), false)?;
        let mut introduced = Vec::new();
        // Rewind availability on leaving a region. Branch-local definitions
        // never leak into siblings; parent definitions remain accessible.
        enum Task {
            Enter(RegionId, bool),
            Op(OpId, RegionId, bool),
            Publish(ValueId),
            Leave(RegionId, usize),
        }
        let mut tasks = Vec::new();
        budget.push(Scratch, &mut tasks, Task::Enter(unit.entry, false))?;
        let mut allocations = Vec::new();
        let mut prepared = budget.filled(Scratch, unit.calls.len(), None)?;
        let mut consumed = budget.filled(Scratch, unit.calls.len(), false)?;
        let mut pending_calls = Vec::new();
        let mut arguments_owned = budget.filled(Scratch, unit.call_arguments.len(), false)?;
        let mut instances_owned = budget.filled(Scratch, unit.call_instantiations.len(), false)?;
        let mut has_references = false;
        work(budget, unit.calls.len())?;
        for site in &unit.calls {
            if let Some(id) = site.contract.instantiation {
                let owner = instances_owned
                    .get_mut(id.index())
                    .ok_or("dangling call instantiation")?;
                if *owner {
                    return fail("call instantiation has multiple owners");
                }
                *owner = true;
            }
            let arguments = unit
                .arguments(site.arguments)
                .ok_or("invalid call argument range")?;
            work(budget, arguments.len())?;
            for (offset, argument) in arguments.iter().enumerate() {
                has_references |= matches!(argument, CallArgument::Reference(_));
                let owned = &mut arguments_owned[site.arguments.start as usize + offset];
                if *owned {
                    return fail("call argument ranges overlap");
                }
                *owned = true;
            }
        }
        work(budget, arguments_owned.len())?;
        if arguments_owned.iter().any(|owned| !owned) {
            return fail("unowned call argument");
        }
        work(budget, instances_owned.len())?;
        if instances_owned.iter().any(|owned| !owned) {
            return fail("unowned call instantiation");
        }
        let mut references_prepared = if has_references {
            budget.filled(Scratch, unit.call_arguments.len(), false)?
        } else {
            Vec::new()
        };
        let mut last_reference: Vec<Option<u32>> = if has_references {
            budget.filled(Scratch, unit.calls.len(), None)?
        } else {
            Vec::new()
        };
        while let Some(task) = tasks.pop() {
            work(budget, 1)?;
            match task {
                Task::Enter(region, in_loop) => {
                    let Some(body) = unit.regions.get(region.index()) else {
                        return fail("dangling child region");
                    };
                    if region_seen[region.index()] {
                        return fail("region has multiple owners or a cycle");
                    }
                    region_seen[region.index()] = true;
                    active_regions[region.index()] = true;
                    budget.push(Scratch, &mut tasks, Task::Leave(region, introduced.len()))?;
                    work(budget, body.operations.len())?;
                    for op in body.operations.iter().rev() {
                        budget.push(Scratch, &mut tasks, Task::Op(*op, region, in_loop))?;
                    }
                }
                Task::Leave(region, checkpoint) => {
                    if pending_calls
                        .last()
                        .is_some_and(|(_, owner)| *owner == region)
                    {
                        return fail("prepared call escapes its owning region");
                    }
                    if let Some(result) = unit.regions[region.index()].result {
                        if prefix_uses.get(result.index()).copied().flatten().is_some() {
                            return fail(
                                "instantiated callable value escapes through a region result",
                            );
                        }
                        if !available.get(result.index()).copied().unwrap_or(false) {
                            return fail("region yields an unavailable value");
                        }
                    }
                    work(budget, introduced.len() - checkpoint)?;
                    for value in introduced.drain(checkpoint..) {
                        available[value] = false;
                    }
                    active_regions[region.index()] = false;
                }
                Task::Publish(value) => {
                    available[value.index()] = true;
                    budget.push(Scratch, &mut introduced, value.index())?;
                }
                Task::Op(id, region, in_loop) => {
                    work(budget, 8 + 3 * capture_lookup)?;
                    let Some(op) = unit.operations.get(id.index()) else {
                        return fail("dangling operation");
                    };
                    if op_seen[id.index()] || op.region != region {
                        return fail("operation has incorrect or multiple owners");
                    }
                    op_seen[id.index()] = true;
                    if op.origin.is_some_and(|origin| {
                        origin.index() >= program.modules[unit.module.index()].source.len()
                    }) {
                        return fail("operation origin is outside its source module");
                    }
                    let Some(operands) = unit.operands(op.operands) else {
                        return fail("invalid operand range");
                    };
                    work(
                        budget,
                        operands
                            .len()
                            .checked_mul(2)
                            .ok_or(AllocationError::Capacity)?,
                    )?;
                    if operands
                        .iter()
                        .any(|value| !available.get(value.index()).copied().unwrap_or(false))
                    {
                        return fail("operation reads an unavailable value");
                    }
                    for &value in operands {
                        prefix_use(value, id)?;
                    }
                    if let Some(value) = op.result {
                        let Some(result) = unit.values.get(value.index()) else {
                            return fail("dangling result");
                        };
                        if defined[value.index()]
                            || result.definition != id
                            || result.ty.index() >= program.types.len()
                        {
                            return fail("invalid result definition or type");
                        }
                        defined[value.index()] = true;
                        budget.push(Scratch, &mut tasks, Task::Publish(value))?;
                    }
                    let access_cell = |cell: CellId| -> Result<(), VerificationError> {
                        let cell_info = program.cells.get(cell.index()).ok_or("dangling cell")?;
                        if cell_info.owner == frozen.id() {
                            if !active_regions[cell_info.region.index()] {
                                return Err("cell used outside its lexical region".into());
                            }
                        } else if captures.binary_search(&cell).is_err() {
                            return Err("foreign cell missing from capture signature".into());
                        }
                        Ok(())
                    };
                    let access_place = |place_id: PlaceId| -> Result<(), VerificationError> {
                        let place = places
                            .get(place_id.index())
                            .ok_or("dangling evaluated place")?;
                        let check_value = |value: ValueId| -> Result<(), VerificationError> {
                            prefix_use(value, id)?;
                            if available.get(value.index()).copied().unwrap_or(false) {
                                Ok(())
                            } else {
                                Err("place reads an unavailable captured value".into())
                            }
                        };
                        match unit.places[place.root.index()] {
                            Place::Cell(cell) => access_cell(cell),
                            Place::Value(value) => check_value(value),
                            Place::Field { .. } => {
                                unreachable!("verified root is not a projection")
                            }
                            Place::Member { receiver, key } => {
                                check_value(receiver)?;
                                if key.index() >= program.strings.len() {
                                    return Err("invalid member key".into());
                                }
                                Ok(())
                            }
                            Place::Index { receiver, key } => {
                                check_value(receiver)?;
                                check_value(key)
                            }
                        }
                    };
                    let (arity, result) = match &op.kind {
                        OperationKind::Constant(value) => {
                            if let Constant::String(id) = value {
                                if id.index() >= program.strings.len() {
                                    return fail("invalid string value");
                                }
                            }
                            (Some(0), true)
                        }
                        OperationKind::Initialize(cell) => {
                            access_cell(*cell)?;
                            if program.cells[cell.index()].owner != frozen.id()
                                || matches!(
                                    program.cells[cell.index()].binding,
                                    CellBinding::Parameter(_)
                                )
                            {
                                return fail("initialization does not own its cell");
                            }
                            if matches!(
                                program.cells[cell.index()].binding,
                                CellBinding::Function(_)
                            ) && operands
                                .first()
                                .and_then(|value| prefix_uses.get(value.index()))
                                .copied()
                                .flatten()
                                != Some(id)
                            {
                                return fail(
                                    "named function initialization is outside its prefix pair",
                                );
                            }
                            (Some(1), false)
                        }
                        OperationKind::Load(place) => {
                            access_place(*place)?;
                            (Some(0), true)
                        }
                        OperationKind::CheckPlace(place) => {
                            access_place(*place)?;
                            let checked = places[place.index()];
                            let checkable = matches!(
                                unit.places[place.index()],
                                Place::Field { .. }
                            ) || matches!(unit.places[place.index()], Place::Cell(cell) if program.is_reference_parameter(cell));
                            if !checkable
                                || !matches!(
                                    unit.places[checked.root.index()],
                                    Place::Cell(_) | Place::Member { .. } | Place::Index { .. }
                                )
                                || !checked.writable
                            {
                                return fail(
                                    "place check requires a writable stored field or reference formal",
                                );
                            }
                            (Some(0), false)
                        }
                        OperationKind::Store(place) => {
                            access_place(*place)?;
                            if !places[place.index()].writable {
                                return fail("store target is a read-only evaluated place");
                            }
                            (Some(1), false)
                        }
                        OperationKind::CopyValue | OperationKind::Unary { .. } => (Some(1), true),
                        OperationKind::IsUndefined => (Some(1), true),
                        OperationKind::TypeTest(target) => {
                            if target.index() >= program.types.len() {
                                return fail("type test has a dangling target");
                            }
                            (Some(1), true)
                        }
                        OperationKind::Template => {
                            if operands.is_empty() {
                                return fail("template without operands");
                            }
                            (None, true)
                        }
                        OperationKind::IntBinary(_) | OperationKind::Binary(_) => (Some(2), true),
                        OperationKind::Intrinsic(_) => (None, true),
                        OperationKind::PrepareCall(call) => {
                            let Some(target) = unit.calls.get(call.index()) else {
                                return fail("dangling call preparation");
                            };
                            if prepared[call.index()].is_some() {
                                return fail("call reference is prepared more than once");
                            }
                            match target.contract.signature {
                                Some(signature) if signature.index() >= program.types.len() => {
                                    return fail("call has a dangling checked signature");
                                }
                                None if !matches!(
                                    target.target,
                                    CallTarget::Builtin(_)
                                        | CallTarget::Intrinsic {
                                            operation: ResolvedIntrinsic::Constructor(_),
                                            receiver: None,
                                        }
                                ) =>
                                {
                                    return fail("call lost its checked signature");
                                }
                                _ => {}
                            }
                            match &target.target {
                                CallTarget::Value { callee, invocation } => {
                                    prefix_use(*callee, id)?;
                                    if *invocation == Invocation::Reference
                                        || !available.get(callee.index()).copied().unwrap_or(false)
                                    {
                                        return fail("invalid prepared callable value");
                                    }
                                }
                                CallTarget::Reference { place } => access_place(*place)?,
                                CallTarget::Intrinsic {
                                    receiver: Some(receiver),
                                    ..
                                } => {
                                    prefix_use(*receiver, id)?;
                                    if !available.get(receiver.index()).copied().unwrap_or(false) {
                                        return fail("invalid prepared intrinsic receiver");
                                    }
                                }
                                _ => {}
                            }
                            prepared[call.index()] = Some(region);
                            (Some(0), false)
                        }
                        OperationKind::Call(call) => {
                            if prepared.get(call.index()).copied().flatten() != Some(region)
                                || consumed[call.index()]
                            {
                                return fail(
                                    "call does not consume a fresh prepared reference in its region",
                                );
                            }
                            if pending_calls.pop() != Some((*call, region)) {
                                return fail("prepared calls are not properly nested");
                            }
                            consumed[call.index()] = true;
                            let site = &unit.calls[call.index()];
                            work(budget, unit.arguments(site.arguments).unwrap().len())?;
                            for (position, argument) in
                                unit.arguments(site.arguments).unwrap().iter().enumerate()
                            {
                                match *argument {
                                    CallArgument::Value(value) => {
                                        prefix_use(value, id)?;
                                        if !available.get(value.index()).copied().unwrap_or(false) {
                                            return fail("call reads an unavailable argument");
                                        }
                                    }
                                    CallArgument::Reference(_) => {
                                        if !references_prepared
                                            [site.arguments.start as usize + position]
                                        {
                                            return fail(
                                                "call consumes an unprepared reference argument",
                                            );
                                        }
                                    }
                                }
                            }
                            (Some(0), true)
                        }
                        OperationKind::PrepareReference { call, position } => {
                            let site = unit
                                .calls
                                .get(call.index())
                                .ok_or("dangling reference preparation")?;
                            if pending_calls.last() != Some(&(*call, region))
                                || prepared[call.index()] != Some(region)
                                || consumed[call.index()]
                            {
                                return fail("reference preparation is outside its active call");
                            }
                            let arguments = unit.arguments(site.arguments).unwrap();
                            let Some(CallArgument::Reference(place)) =
                                arguments.get(*position as usize)
                            else {
                                return fail(
                                    "reference preparation does not select a reference argument",
                                );
                            };
                            let index = site.arguments.start as usize + *position as usize;
                            if references_prepared[index]
                                || last_reference[call.index()]
                                    .is_some_and(|previous| previous >= *position)
                            {
                                return fail("reference arguments are not prepared once in order");
                            }
                            let previous = last_reference[call.index()]
                                .map_or(0, |position| position as usize + 1);
                            work(budget, (*position as usize).saturating_sub(previous))?;
                            for argument in &arguments[previous..*position as usize] {
                                if let CallArgument::Value(value) = *argument {
                                    if !available.get(value.index()).copied().unwrap_or(false) {
                                        return fail(
                                            "reference preparation precedes an earlier value argument",
                                        );
                                    }
                                }
                            }
                            access_place(*place)?;
                            let checked = places[place.index()];
                            if !checked.writable
                                || !matches!(unit.places[checked.root.index()], Place::Cell(_))
                            {
                                return fail(
                                    "reference argument requires a writable lexical place",
                                );
                            }
                            references_prepared[index] = true;
                            last_reference[call.index()] = Some(*position);
                            (Some(0), false)
                        }
                        OperationKind::Closure(child) => {
                            let Some(body) = program.unit(*child) else {
                                return fail("dangling closure body");
                            };
                            if body.kind == UnitKind::ModuleInitialization {
                                return fail("module initialization cannot be a closure body");
                            }
                            work(
                                budget,
                                body.captures
                                    .len()
                                    .checked_mul(capture_lookup)
                                    .ok_or(AllocationError::Capacity)?,
                            )?;
                            for cell in &body.captures {
                                access_cell(*cell)?;
                            }
                            (Some(0), true)
                        }
                        OperationKind::Allocate { identity, kind } => {
                            budget.push(Scratch, &mut allocations, *identity)?;
                            let arity = match kind {
                                AllocationKind::Object(keys) | AllocationKind::Record(keys) => {
                                    TypeQueryAdmission::new(budget).work(keys.len())?;
                                    if keys.iter().any(|key| key.index() >= program.strings.len()) {
                                        return fail("invalid aggregate key");
                                    }
                                    Some(keys.len())
                                }
                                _ => None,
                            };
                            (arity, true)
                        }
                        OperationKind::ShortCircuit { .. } | OperationKind::Select { .. } => {
                            (Some(1), true)
                        }
                        OperationKind::If { .. } => (Some(1), false),
                        OperationKind::Loop { .. } | OperationKind::Block(_) => (Some(0), false),
                        OperationKind::ForIn { key, body } => {
                            if !program.cells.get(key.index()).is_some_and(|cell| {
                                cell.owner == frozen.id() && cell.region == *body
                            }) {
                                return fail("invalid for-in key binding");
                            }
                            (Some(1), false)
                        }
                        OperationKind::ForOf { item, body } => {
                            if !program.cells.get(item.index()).is_some_and(|cell| {
                                cell.owner == frozen.id() && cell.region == *body
                            }) {
                                return fail("invalid for-of item binding");
                            }
                            (Some(1), false)
                        }
                        OperationKind::Await => {
                            if unit.suspension != Suspension::Async {
                                return fail("await outside an async body");
                            }
                            (Some(1), true)
                        }
                        OperationKind::LoadModule { module, specifier } => {
                            if module.index() >= program.modules.len()
                                || specifier.index() >= program.strings.len()
                            {
                                return fail("dynamic import of an unknown module");
                            }
                            (Some(0), true)
                        }
                        OperationKind::ConstructClass => (None, true),
                        OperationKind::SuperConstruct => {
                            if unit.host_class.is_none() {
                                return fail("super construction outside a host-derived constructor");
                            }
                            (None, false)
                        }
                        OperationKind::Yield { .. } => {
                            if unit.suspension != Suspension::Generator {
                                return fail("yield outside a generator body");
                            }
                            (Some(1), false)
                        }
                        OperationKind::Try { catch, .. } => {
                            if let Some((Some(cell), catch_region)) = catch {
                                if !program.cells.get(cell.index()).is_some_and(|cell| {
                                    cell.owner == frozen.id() && cell.region == *catch_region
                                }) {
                                    return fail("invalid catch binding");
                                }
                            }
                            (Some(0), false)
                        }
                        OperationKind::Return => {
                            if unit.kind == UnitKind::ModuleInitialization || operands.len() > 1 {
                                return fail("invalid return completion");
                            }
                            (None, false)
                        }
                        OperationKind::Throw => (Some(1), false),
                        OperationKind::Break | OperationKind::Continue => {
                            if !in_loop {
                                return fail("loop completion has no target");
                            }
                            (Some(0), false)
                        }
                    };
                    if arity.is_some_and(|arity| arity != operands.len())
                        || result != op.result.is_some()
                    {
                        return fail("operation signature mismatch");
                    }
                    if let OperationKind::PrepareCall(call) = op.kind {
                        budget.push(Scratch, &mut pending_calls, (call, region))?;
                    }
                    // Expected types and query adapter are dropped inside
                    // verify_types before this allocation scope releases them.
                    {
                        let mut query_scope = budget.scope();
                        verify_types(
                            program,
                            frozen.id(),
                            id,
                            unit,
                            op,
                            operands,
                            &places,
                            &mut query_scope,
                        )?;
                    }
                    for child in op.kind.child_regions() {
                        work(budget, 1)?;
                        if !unit
                            .regions
                            .get(child.index())
                            .is_some_and(|child| child.parent == Some(region))
                        {
                            return fail("child region has incorrect parent");
                        }
                        let loop_context = in_loop
                            || matches!(
                                op.kind,
                                OperationKind::Loop { .. }
                                    | OperationKind::ForIn { .. }
                                    | OperationKind::ForOf { .. }
                            );
                        budget.push(Scratch, &mut tasks, Task::Enter(child, loop_context))?;
                    }
                }
            }
        }
        if !scratch::unique(&mut allocations, budget, 1, Ord::cmp)? {
            return fail("duplicate allocation-site identity");
        }
        for length in [
            region_seen.len(),
            op_seen.len(),
            defined.len(),
            consumed.len(),
        ] {
            work(budget, length)?;
        }
        if region_seen.iter().any(|seen| !seen)
            || op_seen.iter().any(|seen| !seen)
            || defined.iter().any(|seen| !seen)
            || consumed.iter().any(|seen| !seen)
        {
            return fail("unowned semantic storage");
        }
    }
    Ok(receipt)
}

/// The payload owner admits its iterative traversal before invoking these
/// existing node checks; no recursive validation or second type graph exists.
fn verify_type_contracts(
    program: &Program<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), VerificationError> {
    use crate::check::type_payload::{measure_payload, Payload, PayloadError};
    work(budget, program.types.len())?;
    for ty in program.types.iter() {
        measure_payload(Payload::Type(ty), budget, |node| match node {
            Payload::Type(ty) => validate_nominal_type(ty, &program.structs),
            Payload::Signature(signature) => signature.validate_parameters(),
            Payload::Default(_) => Ok(()),
        })
        .map_err(|error| match error {
            PayloadError::Allocation(error) => VerificationError::Allocation(error),
            PayloadError::Visitor(error) => VerificationError::Invalid(error),
        })?;
    }
    Ok(())
}

/// Allocation-free node validation, shared with the already admitted input
/// payload traversal. Nested traversal and diagnostic text work belong to the
/// caller; canonical declaration indexing avoids repeated schema scans.
pub(super) fn validate_nominal_type(
    ty: &Type<'_>,
    structs: &[StructDefinition],
) -> Result<(), &'static str> {
    let (declaration, arguments) = match ty {
        Type::Struct(declaration) => (declaration, None),
        Type::StructInstance { declaration, args } => (declaration, Some(args.len())),
        _ => return Ok(()),
    };
    let definition = structs
        .get(declaration.identity.index())
        .filter(|definition| {
            !declaration.identity.is_class() && definition.identity == declaration.identity
        })
        .ok_or("dangling canonical nominal type identity")?;
    if declaration.name != definition.name {
        return Err("nominal type diagnostic name disagrees with its declaration");
    }
    if arguments.is_some_and(|count| count != definition.type_parameters.len()) {
        return Err("nominal type argument count disagrees with its declaration");
    }
    Ok(())
}

pub(super) fn validate_interface_target(
    program: &Program<'_>,
    target: InterfaceTarget,
) -> Result<(), &'static str> {
    match target {
        InterfaceTarget::Value(cell) if cell.index() < program.cells.len() => Ok(()),
        InterfaceTarget::Struct(identity)
            if !identity.is_class()
                && program
                    .structs
                    .get(identity.index())
                    .is_some_and(|definition| {
                        definition.identity == identity
                            && definition.module.index() < program.modules.len()
                    }) =>
        {
            Ok(())
        }
        InterfaceTarget::Value(_) => Err("dangling export binding"),
        InterfaceTarget::Struct(_) => Err("dangling canonical type export"),
    }
}

/// Interfaces own module identity and alias meaning. The temporary indexes
/// validate each import/export once; they are not another retained module graph.
fn verify_modules(
    program: &Program<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), VerificationError> {
    let mut scope = budget.scope();
    let budget = &mut scope;
    work(budget, 1)?;
    if program.modules.is_empty() || program.entry.index() >= program.modules.len() {
        return Err("invalid semantic entry module".into());
    }
    let mut initializer_owners = budget.filled(Scratch, program.units.len(), None)?;
    let mut export_owners = budget.filled(Scratch, program.exports.len(), None)?;
    let mut exports = budget.vector(Scratch, program.exports.len())?;
    let mut dependencies = Vec::new();
    let mut export_name_work = 2;
    work(budget, program.modules.len())?;
    for (index, module) in program.modules.iter().enumerate() {
        let id = ModuleId::from_index(index).ok_or("semantic module identity capacity")?;
        let unit = program
            .unit(module.initializer)
            .ok_or("dangling module initializer")?;
        if unit.kind != UnitKind::ModuleInitialization
            || unit.module != id
            || initializer_owners[module.initializer.index()]
                .replace(id)
                .is_some()
        {
            return Err("module initializer has an incorrect or duplicate owner".into());
        }
        let values = program
            .exports
            .get(module.exports.clone())
            .ok_or("invalid module export range")?;
        work(budget, values.len())?;
        for (offset, export) in values.iter().enumerate() {
            if export_owners[module.exports.start + offset]
                .replace(id)
                .is_some()
            {
                return Err("overlapping module export ranges".into());
            }
            exports.push((id, export.name.as_str(), export.target));
            export_name_work = export_name_work.max(export.name.len().saturating_add(2));
            validate_interface_target(program, export.target)?;
        }
        work(budget, module.dependencies.len())?;
        for &dependency in &module.dependencies {
            if dependency.index() >= program.modules.len() {
                return Err("dangling static module dependency".into());
            }
            budget.push(Scratch, &mut dependencies, (id, dependency))?;
        }
        work(budget, module.foreign_imports.len())?;
        for import in &module.foreign_imports {
            let cell = program
                .cells
                .get(import.cell.index())
                .ok_or("foreign import names a dangling cell")?;
            if cell.binding != CellBinding::Foreign
                || import.source.is_empty()
                || import.imported.is_empty()
            {
                return Err("foreign import must bind a foreign cell to a named export".into());
            }
        }
    }
    if !scratch::unique(&mut exports, budget, export_name_work, |left, right| {
        (left.0, left.1).cmp(&(right.0, right.1))
    })? {
        return Err("duplicate public export within a module".into());
    }
    scratch::sort(&mut dependencies, budget, 2, Ord::cmp)?;
    work(budget, export_owners.len())?;
    if export_owners.iter().any(Option::is_none) {
        return Err("export table entry has no module owner".into());
    }
    work(budget, program.units.len())?;
    for (index, frozen) in program.units.iter().enumerate() {
        let unit = frozen.data();
        if frozen.id().index() != index || unit.module.index() >= program.modules.len() {
            return Err("unit identity or source module is invalid".into());
        }
        if (unit.kind == UnitKind::ModuleInitialization) != initializer_owners[index].is_some() {
            return Err("initialization unit has no matching module interface".into());
        }
        if unit.kind != UnitKind::ModuleInitialization && unit.instantiation_prefix != 0 {
            return Err("noninitializer unit has a module instantiation prefix".into());
        }
    }
    let expected = super::module_contract::initialization_order_admitted(
        &program.modules,
        program.entry,
        budget,
    )
    .map_err(|error| match error {
        crate::module::StaticOrderError::Invalid(reason) => VerificationError::Invalid(reason),
        crate::module::StaticOrderError::Resources(error) => VerificationError::Allocation(error),
    })?;
    work(budget, expected.len())?;
    if expected.as_slice() != program.initialization.as_slice() {
        return Err(
            "module initialization schedule differs from ordered static dependencies".into(),
        );
    }
    work(budget, program.modules.len())?;
    for (index, module) in program.modules.iter().enumerate() {
        let id = ModuleId::from_index(index).unwrap();
        work(budget, module.imports.len())?;
        for import in &module.imports {
            let dependency = scratch::find(&dependencies, budget, |row, budget| {
                scratch::scalar(row, &(id, import.module), budget)
            })?;
            let export = scratch::find(&exports, budget, |row, budget| {
                let module = scratch::scalar(&row.0, &import.module, budget)?;
                if module == Ordering::Equal {
                    scratch::text(row.1, &import.name, budget)
                } else {
                    Ok(module)
                }
            })?;
            if dependency.is_none() || export.map(|row| row.2) != Some(import.target) {
                return Err("module import does not name its dependency's canonical export".into());
            }
        }
    }
    work(budget, program.modules.len())?;
    for module in program.modules.iter() {
        work(budget, module.exports.len())?;
        for export in &program.exports[module.exports.clone()] {
            let InterfaceTarget::Value(cell) = export.target else {
                continue;
            };
            let cell = &program.cells[cell.index()];
            let owner = program
                .unit(cell.owner)
                .ok_or("export has a dangling cell owner")?;
            if owner.kind != UnitKind::ModuleInitialization || cell.region != owner.entry {
                return Err("module export does not refer to module-level storage".into());
            }
        }
    }
    Ok(())
}

/// The only value use allowed for a module-instantiated callable is its paired
/// cell initialization. Subsequent source evaluation reads the canonical cell,
/// so separating all prefixes from suffixes cannot move another value recipe.
fn verify_instantiation_prefix(
    program: &Program<'_>,
    id: UnitId,
    named_functions: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Option<OpId>>, VerificationError> {
    let unit = program.unit(id).ok_or("missing instantiation unit")?;
    work(budget, 1)?;
    if unit.kind != UnitKind::ModuleInitialization {
        if unit.instantiation_prefix != 0 || named_functions != 0 {
            return Err("named function storage is outside module instantiation".into());
        }
        return Ok(Vec::new());
    }
    let operations = &unit.regions[unit.entry.index()].operations;
    let prefix = unit.instantiation_prefix as usize;
    if prefix > operations.len() || prefix % 2 != 0 || prefix / 2 != named_functions {
        return Err("module instantiation prefix does not cover its named function cells".into());
    }
    if named_functions == 0 {
        return Ok(Vec::new());
    }
    let mut allowed = budget.filled(Scratch, unit.values.len(), None)?;
    let mut scope = budget.scope();
    let mut cells = scope.vector(Scratch, named_functions)?;
    let mut bodies = scope.vector(Scratch, named_functions)?;
    work(&mut scope, prefix)?;
    for pair in operations[..prefix].chunks_exact(2) {
        let creation = unit
            .operations
            .get(pair[0].index())
            .ok_or("dangling instantiation creation")?;
        let initialize = unit
            .operations
            .get(pair[1].index())
            .ok_or("dangling instantiation initialization")?;
        let OperationKind::Closure(body) = creation.kind else {
            return Err("module instantiation prefix contains ordinary evaluation".into());
        };
        let OperationKind::Initialize(cell) = initialize.kind else {
            return Err("module instantiation creation has no paired initialization".into());
        };
        let child = program
            .unit(body)
            .ok_or("dangling instantiated function body")?;
        let storage = program
            .cells
            .get(cell.index())
            .ok_or("dangling instantiated function cell")?;
        let value = creation
            .result
            .ok_or("instantiated function has no value")?;
        if child.kind != UnitKind::Function
            || child.module != unit.module
            || storage.owner != id
            || storage.region != unit.entry
            || storage.binding != CellBinding::Function(body)
            || creation.region != unit.entry
            || initialize.region != unit.entry
            || unit.operands(creation.operands) != Some(&[][..])
            || unit.operands(initialize.operands) != Some(&[value][..])
            || initialize.result.is_some()
            || value.index() >= unit.values.len()
            || allowed[value.index()].replace(pair[1]).is_some()
        {
            return Err("invalid or repeated named function instantiation pair".into());
        }
        cells.push(cell);
        bodies.push(body);
    }
    if !scratch::unique(&mut cells, &mut scope, 1, Ord::cmp)?
        || !scratch::unique(&mut bodies, &mut scope, 1, Ord::cmp)?
    {
        return Err("invalid or repeated named function instantiation pair".into());
    }
    Ok(allowed)
}

fn callable_signature<'a, 'src>(
    program: &'a Program<'src>,
    unit: &UnitData,
) -> Result<Option<&'a crate::check::FunctionSignature<'src>>, VerificationError> {
    match (
        unit.kind,
        unit.callable_type
            .and_then(|id| program.types.get(id.index())),
    ) {
        (UnitKind::ModuleInitialization, None) if unit.callable_type.is_none() => Ok(None),
        (UnitKind::Function | UnitKind::Closure, Some(Type::Function(signature))) => {
            Ok(Some(signature))
        }
        (UnitKind::Function | UnitKind::Closure, Some(Type::GenericFunction(function))) => {
            Ok(Some(&function.signature))
        }
        _ => Err("invalid owned callable signature".into()),
    }
}

/// Check representation-independent operation signatures. Source refinements
/// still require their own proof owner before edits may move narrowed reads;
/// compatible declared/result types alone do not establish such a proof.
fn verify_types(
    program: &Program<'_>,
    unit_id: UnitId,
    operation_id: OpId,
    unit: &UnitData,
    operation: &Operation,
    operands: &[ValueId],
    places: &[VerifiedPlace<'_, '_>],
    budget: &mut AllocationBudget<'_>,
) -> Result<(), VerificationError> {
    let mut query = TypeQueryAdmission::new(budget);
    query.work(1)?;
    let value_type = |id: ValueId| &program.types[unit.values[id.index()].ty.index()];
    let operand = |index: usize| value_type(operands[index]);
    let result = operation.result.map(value_type);
    let error = || VerificationError::TypeMismatch {
        unit: unit_id,
        operation: operation_id,
        span: operation.span,
    };
    let expect = |valid| if valid { Ok(()) } else { Err(error()) };
    let region_result = |id: RegionId| -> Result<&Type<'_>, VerificationError> {
        let body = unit
            .regions
            .get(id.index())
            .ok_or("dangling expression region")?;
        let value = body
            .result
            .ok_or("expression region has no normal result")?;
        let value = unit
            .values
            .get(value.index())
            .ok_or("dangling expression result")?;
        program
            .types
            .get(value.ty.index())
            .ok_or(VerificationError::Invalid("invalid expression result type"))
    };
    let statement_region = |id: RegionId| -> Result<(), VerificationError> {
        if unit
            .regions
            .get(id.index())
            .ok_or("dangling statement region")?
            .result
            .is_some()
        {
            Err("statement region unexpectedly yields a value".into())
        } else {
            Ok(())
        }
    };
    match &operation.kind {
        OperationKind::Constant(constant) => {
            let literal_type = match constant {
                Constant::Integer(value) if matches!(result, Some(Type::Enum(_))) => {
                    let Some(Type::Enum(name)) = result else {
                        unreachable!()
                    };
                    // Enum identity is still name-owned. Pay this actual query,
                    // including compared name bytes, without building an index.
                    for definition in program.enums.iter() {
                        query.work(
                            definition
                                .name
                                .len()
                                .min(name.len())
                                .checked_add(1)
                                .ok_or(AllocationError::Capacity)?,
                        )?;
                        if definition.name == *name {
                            for variant in &definition.variants {
                                query.work(1)?;
                                if variant.value == *value {
                                    return Ok(());
                                }
                            }
                        }
                    }
                    return Err(error());
                }
                Constant::Integer(_) => Type::Int,
                Constant::Number(_) => Type::Float,
                Constant::String(_) => Type::String,
                Constant::Boolean(_) => Type::Bool,
                Constant::Null => Type::Null,
                Constant::Undefined => Type::TypeParameter("$js"),
            };
            expect(class_assignable(program, 
                result.unwrap(),
                &literal_type,
                &mut query,
            )?)
        }
        OperationKind::Initialize(cell) => {
            let cell = &program.cells[cell.index()];
            expect(
                cell.binding != CellBinding::Foreign
                    && cell.region == operation.region
                    && class_assignable(program, 
                        &program.types[cell.ty.index()],
                        operand(0),
                        &mut query,
                    )?,
            )
        }
        OperationKind::Load(place) | OperationKind::Store(place) => {
            let writing = matches!(operation.kind, OperationKind::Store(_));
            match &unit.places[place.index()] {
                Place::Cell(cell) => {
                    let cell = &program.cells[cell.index()];
                    let declared = &program.types[cell.ty.index()];
                    // Compatibility preserves source refinement, not motion.
                    expect(if writing {
                        cell.binding != CellBinding::Foreign
                            && class_assignable(program, declared, operand(0), &mut query)?
                    } else {
                        class_assignable(program, declared, result.unwrap(), &mut query)?
                    })
                }
                Place::Value(_) | Place::Field { .. } => {
                    let declared = places[place.index()].ty.ok_or("missing value place type")?;
                    expect(if writing {
                        places[place.index()].writable
                            && class_assignable(program, declared, operand(0), &mut query)?
                    } else {
                        class_assignable(program, declared, result.unwrap(), &mut query)?
                    })
                }
                _ => Ok(()),
            }
        }
        OperationKind::CopyValue => {
            expect(type_matches(result, Some(operand(0)), &mut query)? && !operand(0).is_void())
        }
        OperationKind::IsUndefined => {
            expect(matches!(result, Some(Type::Bool)) && !operand(0).is_void())
        }
        OperationKind::TypeTest(target) => expect(
            matches!(result, Some(Type::Bool))
                && !operand(0).is_void()
                && crate::primitive::runtime_type_test(&program.types[target.index()]).is_some(),
        ),
        OperationKind::Template => {
            if !matches!(result, Some(Type::String)) {
                return Err(error());
            }
            for &value in operands {
                if !crate::check::binary_types::is_stringable_with(value_type(value), &mut query)? {
                    return Err(error());
                }
            }
            Ok(())
        }
        OperationKind::IntBinary(_) => expect(
            matches!(result, Some(Type::Int))
                && operands
                    .iter()
                    .all(|id| matches!(value_type(*id), Type::Int)),
        ),
        OperationKind::Binary(op) => {
            if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Nullish) {
                return Err(error());
            }
            let expected = checked_binary_type_with(*op, operand(0), operand(1), &mut query)
                .map_err(|failure| match failure {
                    BinaryTypeError::Semantic(_) => error(),
                    BinaryTypeError::Admission(error) => VerificationError::Allocation(error),
                })?;
            expect(type_matches(result, Some(&expected), &mut query)?)
        }
        OperationKind::Unary { op, integer } => expect(match op {
            UnaryOp::Not => {
                !integer && matches!(operand(0), Type::Bool) && matches!(result, Some(Type::Bool))
            }
            UnaryOp::Neg => {
                operand(0).is_numeric()
                    && type_matches(result, Some(operand(0)), &mut query)?
                    && *integer == matches!(operand(0), Type::Int)
            }
        }),
        OperationKind::Intrinsic(ResolvedIntrinsic::Property(_)) => expect(operands.len() == 1),
        OperationKind::Intrinsic(_) => {
            Err("standalone intrinsic must identify a property read".into())
        }
        OperationKind::ShortCircuit { kind, right } => {
            let right = region_result(*right)?;
            match kind {
                ShortCircuit::BooleanAnd | ShortCircuit::BooleanOr | ShortCircuit::Nullish => {
                    let op = match kind {
                        ShortCircuit::BooleanAnd => BinaryOp::And,
                        ShortCircuit::BooleanOr => BinaryOp::Or,
                        _ => BinaryOp::Nullish,
                    };
                    let expected = checked_binary_type_with(op, operand(0), right, &mut query)
                        .map_err(|failure| match failure {
                            BinaryTypeError::Semantic(_) => error(),
                            BinaryTypeError::Admission(error) => {
                                VerificationError::Allocation(error)
                            }
                        })?;
                    expect(type_matches(result, Some(&expected), &mut query)?)
                }
                ShortCircuit::JavaScriptAnd | ShortCircuit::JavaScriptOr => expect(
                    !operand(0).is_void()
                        && !right.is_void()
                        && matches!(result, Some(Type::TypeParameter("$js"))),
                ),
            }
        }
        OperationKind::Select { yes, no } => expect(
            matches!(operand(0), Type::Bool)
                && class_assignable(program, result.unwrap(), region_result(*yes)?, &mut query)?
                && class_assignable(program, result.unwrap(), region_result(*no)?, &mut query)?,
        ),
        OperationKind::If { yes, no } => {
            statement_region(*yes)?;
            if let Some(no) = no {
                statement_region(*no)?;
            }
            expect(matches!(operand(0), Type::Bool))
        }
        OperationKind::ForIn { key, body } => {
            statement_region(*body)?;
            expect(
                !operand(0).is_void()
                    && matches!(program.types[program.cells[key.index()].ty.index()], Type::String),
            )
        }
        OperationKind::ForOf { item, body } => {
            statement_region(*body)?;
            let Type::Generator(element) = operand(0) else {
                return Err(error());
            };
            expect(class_assignable(
                program,
                &program.types[program.cells[item.index()].ty.index()],
                element,
                &mut query,
            )?)
        }
        OperationKind::Await => {
            let Type::Task(fulfilled) = operand(0) else {
                return Err(error());
            };
            expect(type_matches(result, Some(fulfilled), &mut query)?)
        }
        OperationKind::LoadModule { module, .. } => expect(matches!(
            result,
            Some(Type::Task(namespace))
                if matches!(namespace.as_ref(), Type::ModuleNamespace(found) if *found as usize == module.index())
        )),
        // A host-derived class's own constructor, with every parameter.
        OperationKind::ConstructClass => {
            let Some(Type::Class(name) | Type::ClassInstance { name, .. }) = result else {
                return Err(error());
            };
            query.work(program.classes.len())?;
            let class = program
                .classes
                .iter()
                .position(|class| class.name == *name)
                .ok_or("construction of an unknown class")?;
            let parameters = host_constructor_parameters(program, class, &mut query)?
                .ok_or("construction of a class without a host constructor")?;
            // The first operand is the constructor itself.
            let Some((&constructor, arguments)) = operands.split_first() else {
                return Err(error());
            };
            if !matches!(value_type(constructor), Type::Function(_)) {
                return Err(error());
            }
            expect(arguments.len() == parameters.len() && {
                let mut all = true;
                for (&operand, expected) in arguments.iter().zip(parameters) {
                    all &= class_assignable(program, expected, value_type(operand), &mut query)?;
                }
                all
            })
        }
        // The base constructor of this constructor's class.
        OperationKind::SuperConstruct => {
            let class = unit.host_class.ok_or("super construction outside a constructor")? as usize;
            let base = program
                .classes
                .get(class)
                .and_then(|class| class.base.as_deref())
                .ok_or("super construction without a base class")?;
            query.work(program.classes.len())?;
            let base = program
                .classes
                .iter()
                .position(|class| class.name == base)
                .ok_or("super construction of an unknown base")?;
            let parameters = host_constructor_parameters(program, base, &mut query)?
                .ok_or("super construction of a base without a constructor")?;
            expect(operands.len() == parameters.len() && {
                let mut all = true;
                for (&operand, expected) in operands.iter().zip(parameters) {
                    all &= class_assignable(program, expected, value_type(operand), &mut query)?;
                }
                all
            })
        }
        OperationKind::Yield { delegate } => {
            let signature =
                callable_signature(program, unit)?.ok_or("initialization cannot yield")?;
            let Type::Generator(element) = signature.return_type.as_ref() else {
                return Err(error());
            };
            let yielded = match (delegate, operand(0)) {
                (false, value) => value,
                (true, Type::Array(inner) | Type::Generator(inner)) => inner.as_ref(),
                (true, typed) => match crate::typed_array::TypedArrayKind::from_type(typed) {
                    Some(kind) if kind.element_is_float() => &Type::Float,
                    Some(_) => &Type::Int,
                    None => return Err(error()),
                },
            };
            expect(class_assignable(program, element, yielded, &mut query)?)
        }
        OperationKind::Loop { test, body, update } => {
            statement_region(*body)?;
            let test_body = unit.regions.get(test.index()).ok_or("dangling loop test")?;
            if test_body.result.is_some() {
                expect(matches!(region_result(*test)?, Type::Bool))?;
            } else if !test_body.operations.is_empty() {
                return Err("nonempty loop test has no condition result".into());
            }
            if unit
                .regions
                .get(update.index())
                .ok_or("dangling loop update")?
                .result
                .is_some()
            {
                region_result(*update)?;
            }
            Ok(())
        }
        OperationKind::Block(body) => statement_region(*body),
        OperationKind::Try {
            body,
            catch,
            finally,
        } => {
            if catch.is_none() && finally.is_none() {
                return Err("try has no catch or finally region".into());
            }
            statement_region(*body)?;
            if let Some((_, catch)) = catch {
                statement_region(*catch)?;
            }
            if let Some(finally) = finally {
                statement_region(*finally)?;
            }
            Ok(())
        }
        OperationKind::Closure(child) => {
            let body = program.unit(*child).ok_or("dangling closure body")?;
            expect(type_matches(
                body.callable_type
                    .and_then(|id| program.types.get(id.index())),
                result,
                &mut query,
            )?)
        }
        OperationKind::Allocate { kind, .. } => match kind {
            AllocationKind::SpreadArray(spread) => {
                let Some(Type::Array(element)) = result else {
                    return Err(error());
                };
                if spread.len() != operands.len() {
                    return Err(error());
                }
                for (value, &spreads) in operands.iter().zip(spread) {
                    let actual = value_type(*value);
                    let valid = if spreads {
                        // A spread operand is a checked array of the element.
                        matches!(actual, Type::Array(inner)
                            if class_assignable(program, element, inner, &mut query)?)
                    } else {
                        class_assignable(program, element, actual, &mut query)?
                    };
                    if !valid {
                        return Err(error());
                    }
                }
                Ok(())
            }
            AllocationKind::Array | AllocationKind::Record(_) => {
                let element = match (kind, result) {
                    (AllocationKind::Array, Some(Type::Array(element)))
                    | (AllocationKind::Record(_), Some(Type::Record(element))) => element,
                    _ => return Err(error()),
                };
                for value in operands {
                    if !class_assignable(program, element, value_type(*value), &mut query)? {
                        return Err(error());
                    }
                }
                Ok(())
            }
            // A JS object literal, or an internal class instance holding
            // exactly its declared fields in order.
            AllocationKind::Object(keys) => expect(match result {
                Some(Type::TypeParameter("$js")) => true,
                Some(Type::Class(name) | Type::ClassInstance { name, .. }) => program.class(name).is_some_and(|class| {
                    !class.external
                        && class.fields.len() == keys.len()
                        && class.fields.iter().zip(keys).all(|((key, _), actual)| key == actual)
                }),
                _ => false,
            }),
            AllocationKind::Struct(identity) => {
                let definition = program
                    .structs
                    .get(identity.index())
                    .filter(|definition| definition.identity == *identity)
                    .ok_or("unknown struct allocation")?;
                let identity = match result {
                    Some(Type::Struct(declaration)) if definition.type_parameters.is_empty() => {
                        declaration.identity
                    }
                    Some(Type::Struct(_)) | Some(Type::StructInstance { .. }) => {
                        return Err(
                            "generic struct construction requires an instantiated schema".into(),
                        );
                    }
                    _ => return Err(error()),
                };
                expect(
                    identity == definition.identity && operands.len() == definition.fields.len(),
                )?;
                for (field, &value) in program.fields[definition.fields.clone()]
                    .iter()
                    .zip(operands)
                {
                    if !class_assignable(program, 
                        &program.types[field.ty.index()],
                        value_type(value),
                        &mut query,
                    )? {
                        return Err(error());
                    }
                }
                Ok(())
            }
        },
        OperationKind::PrepareCall(call) => match unit.calls[call.index()].target {
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Method(_),
                receiver: Some(_),
            } => Ok(()),
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Constructor(constructor),
                receiver: None,
            } if crate::primitive::builtin_constructor(constructor) => Ok(()),
            CallTarget::Intrinsic { .. } => Err(
                "prepared intrinsic must identify a receiver method or supported constructor"
                    .into(),
            ),
            _ => Ok(()),
        },
        OperationKind::Call(call) => {
            let site = &unit.calls[call.index()];
            let arguments = unit
                .arguments(site.arguments)
                .ok_or("invalid call argument range")?;
            query.work(
                arguments
                    .len()
                    .checked_mul(2)
                    .ok_or(AllocationError::Capacity)?,
            )?;
            let value_argument = |argument: &CallArgument| match *argument {
                CallArgument::Value(value) => Some(value_type(value)),
                CallArgument::Reference(_) => None,
            };
            let supplied = site.contract.supplied as usize;
            let convention = match site.target {
                CallTarget::Intrinsic { operation, .. } => operation
                    .call_default_convention()
                    .ok_or("prepared intrinsic has no call convention")?,
                ref target if super::host_call(program, unit, target) => {
                    DefaultConvention::PreserveOmission
                }
                _ => DefaultConvention::MaterializeAtCaller,
            };
            if site.contract.defaults != convention || supplied > arguments.len() {
                return Err("call argument convention disagrees with its target".into());
            }
            if convention == DefaultConvention::PreserveOmission && supplied != arguments.len() {
                return Err("preserved omission has synthesized arguments".into());
            }
            // MaterializeAtCaller: arguments past `supplied` are the omitted
            // parameters' checked defaults, evaluated by the caller.
            if let CallTarget::Builtin(builtin) = site.target {
                if crate::primitive::builtin_call_contract(builtin).is_none()
                    && crate::primitive::host_builtin(builtin)
                {
                    // A host builtin: checked by the checker, operands are values.
                    return expect(
                        site.contract.signature.is_none()
                            && site.contract.instantiation.is_none()
                            && supplied == arguments.len()
                            && arguments
                                .iter()
                                .all(|argument| matches!(argument, CallArgument::Value(_))),
                    );
                }
                let contract = crate::primitive::builtin_call_contract(builtin)
                    .ok_or("builtin has no supported semantic call contract")?;
                expect(
                    site.contract.signature.is_none()
                        && site.contract.instantiation.is_none()
                        && supplied == arguments.len()
                        && arguments.len() == contract.arity
                        && arguments
                            .iter()
                            .all(|argument| matches!(argument, CallArgument::Value(_)))
                        && type_matches(result, Some(&contract.result), &mut query)?,
                )?;
                if let Some(expected) = contract.argument.as_ref() {
                    for argument in arguments {
                        if !type_matches(value_argument(argument), Some(expected), &mut query)? {
                            return Err(error());
                        }
                    }
                }
                return Ok(());
            }
            if let CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Constructor(constructor),
                receiver: None,
            } = site.target
            {
                if crate::primitive::intrinsic_call_contract(ResolvedIntrinsic::Constructor(
                    constructor,
                ))
                .is_none()
                {
                    // A builtin whose result the checker chose: verify the
                    // checked shape against the actual operands and result.
                    query.work(arguments.len() + 1)?;
                    if !crate::primitive::builtin_constructor(constructor)
                        || site.contract.signature.is_some()
                        || site.contract.instantiation.is_some()
                        || supplied != arguments.len()
                    {
                        return Err(error());
                    }
                    let first = match arguments.first() {
                        Some(argument) => Some(value_argument(argument).ok_or_else(error)?),
                        None => None,
                    };
                    if arguments.iter().any(|argument| value_argument(argument).is_none()) {
                        return Err(error());
                    }
                    let result = result.ok_or_else(error)?;
                    return expect(crate::primitive::constructor_accepts(
                        constructor,
                        arguments.len(),
                        first,
                        result,
                    ));
                }
            }
            if let CallTarget::Intrinsic {
                operation: operation @ ResolvedIntrinsic::Constructor(_),
                receiver,
            } = site.target
            {
                let contract = crate::primitive::intrinsic_call_contract(operation)
                    .ok_or("constructor has no supported semantic call contract")?;
                query.work(contract.defaults.len())?;
                expect(
                    site.contract.signature.is_none()
                        && site.contract.instantiation.is_none()
                        && receiver.is_none()
                        && contract.receiver.is_none()
                        && supplied == arguments.len()
                        && contract.accepts_arity(supplied),
                )?;
                for (expected, actual) in contract.parameters.iter().zip(arguments) {
                    let Some(actual) = value_argument(actual) else {
                        return Err(error());
                    };
                    if !class_assignable(program, expected, actual, &mut query)? {
                        return Err(error());
                    }
                }
                return expect(type_matches(result, Some(contract.result), &mut query)?);
            }
            let signature = site
                .contract
                .signature
                .ok_or("call lost its checked signature")?;
            let callee_type = program
                .types
                .get(signature.index())
                .ok_or("call has a dangling checked signature")?;
            if let CallTarget::Value { callee, .. } = site.target {
                if !type_equal_with(callee_type, value_type(callee), &mut query)? {
                    return Err("call signature disagrees with its prepared value".into());
                }
            }
            if let CallTarget::Reference { place } = site.target {
                if let Some(declared) = places[place.index()].ty {
                    if !type_equal_with(declared, callee_type, &mut query)?
                        && !(matches!(declared, Type::Nullable(_) | Type::Union(_))
                            && class_assignable(program, declared, callee_type, &mut query)?)
                    {
                        return Err("call signature disagrees with its prepared place".into());
                    }
                }
            }
            if let CallTarget::Intrinsic {
                operation,
                receiver: Some(receiver),
            } = site.target
            {
                if let Some(contract) = crate::primitive::intrinsic_call_contract(operation) {
                    if !type_matches(
                        contract.receiver.as_ref(),
                        Some(value_type(receiver)),
                        &mut query,
                    )? || !contract.matches_with(callee_type, &mut query)?
                    {
                        return Err("call signature disagrees with its primitive operation".into());
                    }
                }
            }
            let effective = if let Type::GenericFunction(function) = callee_type {
                validate_signature(&function.signature, &mut query)?;
                for parameter in &function.signature.params {
                    query.admit(RelationEvent::ParameterPair)?;
                    if parameter.passing != crate::primitive::ParameterPassing::Value {
                        return Err("generic reference call ABI is unsupported".into());
                    }
                }
                let id = site
                    .contract
                    .instantiation
                    .ok_or("generic call lost its checked instantiation")?;
                let instance = unit
                    .call_instantiations
                    .get(id.index())
                    .ok_or("dangling call instantiation")?;
                if instance.declaration != signature
                    || instance.arguments.len() != function.type_params.len()
                {
                    return Err("generic call instantiation disagrees with declaration".into());
                }
                for &argument in &instance.arguments {
                    query.work(1)?;
                    if program.types.get(argument.index()).is_none() {
                        return Err("generic call has a dangling type argument".into());
                    }
                }
                let effective = program
                    .types
                    .get(instance.signature.index())
                    .ok_or("generic call has a dangling effective signature")?;
                if !matches!(effective, Type::Function(_)) {
                    return Err(
                        "generic call effective signature is not ordinary callable metadata".into(),
                    );
                }
                // Read the original ordered checker decision, then apply the
                // shared forward substitution. No inverse signature matching.
                let substituted = substitute_signature_with(
                    &function.signature,
                    &mut |name: &str, query: &mut TypeQueryAdmission<'_, '_>| {
                        for (parameter, &argument) in
                            function.type_params.iter().zip(&instance.arguments)
                        {
                            query.work(1)?;
                            query.work(parameter.len())?;
                            query.work(name.len())?;
                            if *parameter == name {
                                return Ok(Some(&program.types[argument.index()]));
                            }
                        }
                        Ok(None)
                    },
                    &mut query,
                )?;
                let expected = Type::Function(substituted);
                if !type_equal_with(&expected, effective, &mut query)? {
                    return Err(
                        "generic call effective signature disagrees with retained arguments".into(),
                    );
                }
                effective
            } else {
                if site.contract.instantiation.is_some() {
                    return Err("nongeneric call has a generic instantiation".into());
                }
                callee_type
            };
            let argument_matches = |expected: &crate::check::FunctionParameter<'_>,
                                    actual: &CallArgument,
                                    query: &mut TypeQueryAdmission<'_, '_>|
             -> Result<bool, VerificationError> {
                Ok(match (expected.passing, *actual) {
                    (crate::primitive::ParameterPassing::Value, CallArgument::Value(value)) => {
                        class_assignable(program, &expected.ty, value_type(value), query)?
                    }
                    (
                        crate::primitive::ParameterPassing::MutableReference,
                        CallArgument::Reference(place),
                    ) => type_matches(places[place.index()].ty, Some(&expected.ty), query)?,
                    _ => false,
                })
            };
            let has_references = arguments
                .iter()
                .any(|argument| matches!(argument, CallArgument::Reference(_)));
            if has_references && !matches!(site.target, CallTarget::Value { .. }) {
                return Err("reference arguments require an owned callable value ABI".into());
            }
            let full_arity = |signature: &crate::check::FunctionType<'_>,
                              query: &mut TypeQueryAdmission<'_, '_>|
             -> Result<bool, VerificationError> {
                query.work(signature.params.len())?;
                Ok(signature.accepts_arity(supplied)
                    && match convention {
                        // Omitted trailing arrow defaults are applied by the
                        // guarded callee; every other omission is evaluated.
                        DefaultConvention::MaterializeAtCaller => {
                            arguments.len() <= signature.params.len()
                                && arguments.len() >= supplied
                                && signature.params[arguments.len()..].iter().all(|parameter| {
                                    matches!(
                                        parameter.default,
                                        Some(crate::check::DefaultValue::Arrow(_))
                                    )
                                })
                        }
                        DefaultConvention::PreserveOmission => arguments.len() == supplied,
                    })
            };
            if convention == DefaultConvention::MaterializeAtCaller && supplied < arguments.len() {
                // Arguments past `supplied` claim to be the omitted
                // parameters' checked defaults. Each must be exactly that
                // evaluation, or a forged count could relabel real arguments.
                let declared = match callee_type {
                    Type::Function(signature) => &signature.params,
                    Type::GenericFunction(function) => &function.signature.params,
                    _ => return Err("caller default evaluations need a callable signature".into()),
                };
                for position in supplied..arguments.len() {
                    query.work(1)?;
                    let CallArgument::Value(value) = arguments[position] else {
                        return Err("caller default evaluations must be values".into());
                    };
                    let Some(default) = declared.get(position).and_then(|p| p.default.as_ref()) else {
                        return Err("caller default evaluations need a checked default".into());
                    };
                    if !materialized_default(unit, default, value, arguments) {
                        return Err(
                            "caller default evaluations disagree with the checked defaults".into()
                        );
                    }
                }
            }
            match effective {
                Type::Function(signature) => {
                    validate_signature(signature, &mut query)?;
                    for (parameter, argument) in signature.params.iter().zip(arguments) {
                        query.admit(RelationEvent::ParameterPair)?;
                        if matches!(
                            (parameter.passing, argument),
                            (
                                crate::primitive::ParameterPassing::Value,
                                CallArgument::Reference(_)
                            ) | (
                                crate::primitive::ParameterPassing::MutableReference,
                                CallArgument::Value(_)
                            )
                        ) {
                            return Err(
                                "call argument passing mode disagrees with its signature".into()
                            );
                        }
                    }
                    expect(full_arity(signature, &mut query)?)?;
                    for (expected, actual) in signature.params.iter().zip(arguments) {
                        query.admit(RelationEvent::ParameterPair)?;
                        if !argument_matches(expected, actual, &mut query)? {
                            return Err(error());
                        }
                    }
                    expect(type_matches(
                        result,
                        Some(signature.return_type.as_ref()),
                        &mut query,
                    )?)
                }
                Type::TypeParameter("$js") => expect(
                    !has_references
                        && supplied == arguments.len()
                        && type_matches(result, Some(callee_type), &mut query)?,
                ),
                Type::Union(members) => {
                    for member in members {
                        query.work(1)?;
                        if let Type::Function(signature) = member {
                            validate_signature(signature, &mut query)?;
                        }
                    }
                    expect(!has_references && supplied == arguments.len())?;
                    for member in members {
                        query.work(1)?;
                        let Type::Function(signature) = member else {
                            return Err(error());
                        };
                        expect(signature.params.len() == supplied)?;
                        for (expected, actual) in signature.params.iter().zip(arguments) {
                            query.admit(RelationEvent::ParameterPair)?;
                            if !argument_matches(expected, actual, &mut query)? {
                                return Err(error());
                            }
                        }
                        let Some(result) = result else {
                            return Err(error());
                        };
                        if !class_assignable(program, result, &signature.return_type, &mut query)? {
                            return Err(error());
                        }
                    }
                    Ok(())
                }
                _ => Err(error()),
            }
        }
        OperationKind::Throw => expect(!operand(0).is_void()),
        OperationKind::Return => {
            let signature =
                callable_signature(program, unit)?.ok_or("initialization cannot return")?;
            let actual = if operands.is_empty() {
                &Type::Void
            } else {
                operand(0)
            };
            // An async body resolves its task with `T`; a generator's body
            // returns nothing.
            let expected = match (unit.suspension, signature.return_type.as_ref()) {
                (Suspension::Async, Type::Task(fulfilled)) => fulfilled.as_ref(),
                (Suspension::Generator, Type::Generator(_)) => &Type::Void,
                (Suspension::None, declared) => declared,
                _ => return Err(error()),
            };
            expect(class_assignable(program, expected, actual, &mut query)?)
        }
        OperationKind::PrepareReference { .. }
        | OperationKind::CheckPlace(_)
        | OperationKind::Break
        | OperationKind::Continue => Ok(()),
    }
}

/// Whether `value` is the caller-side evaluation of `default`, as conversion
/// produces it: the default's constant, a copy of an earlier argument, a load
/// of the named binding, or a fresh array. String contents are not decoded.
fn materialized_default(
    unit: &UnitData,
    default: &crate::check::DefaultValue<'_>,
    value: ValueId,
    arguments: &[CallArgument],
) -> bool {
    use crate::check::DefaultValue;
    let definition = |value: ValueId| {
        unit.values
            .get(value.index())
            .and_then(|data| unit.operations.get(data.definition.index()))
    };
    // A value transfer may wrap the evaluation in one copy.
    let mut value = value;
    if let Some(operation) = definition(value) {
        if matches!(operation.kind, OperationKind::CopyValue) {
            if let Some(&[source]) = unit.operands(operation.operands) {
                value = source;
            }
        }
    }
    let Some(operation) = definition(value) else {
        return false;
    };
    match (default, &operation.kind) {
        (DefaultValue::Int(expected), OperationKind::Constant(Constant::Integer(actual))) => {
            i64::from(*actual) == *expected
        }
        (DefaultValue::Int(expected), OperationKind::Constant(Constant::Number(bits))) => {
            f64::from_bits(*bits) == *expected as f64
        }
        (DefaultValue::Float(expected), OperationKind::Constant(Constant::Number(bits))) => {
            bits == expected
        }
        (DefaultValue::String(_), OperationKind::Constant(Constant::String(_))) => true,
        (DefaultValue::Bool(expected), OperationKind::Constant(Constant::Boolean(actual))) => {
            actual == expected
        }
        (DefaultValue::Null, OperationKind::Constant(Constant::Null))
        | (DefaultValue::Undefined, OperationKind::Constant(Constant::Undefined)) => true,
        (DefaultValue::Parameter(index), _) => {
            matches!(arguments.get(*index), Some(CallArgument::Value(earlier)) if *earlier == value)
        }
        (DefaultValue::Symbol(symbol), OperationKind::Load(place)) => matches!(
            unit.places.get(place.index()),
            Some(Place::Cell(cell)) if cell.index() == symbol.0 as usize
        ),
        (
            DefaultValue::Array(_),
            OperationKind::Allocate {
                kind: AllocationKind::Array,
                ..
            },
        ) => true,
        (
            DefaultValue::Struct { .. },
            OperationKind::Allocate {
                kind: AllocationKind::Struct(_),
                ..
            },
        )
        | (
            DefaultValue::NewClass { .. },
            OperationKind::Allocate {
                kind: AllocationKind::Object(_),
                ..
            },
        ) => true,
        _ => false,
    }
}

/// The checker's assignability: the structural relation, then upcasts along
/// an internal class's declared base chain, which that relation cannot see.
/// Arrays stay invariant, as in the checker.
/// The parameters of a class's JavaScript constructor: an extern class's
/// declared host constructor, or a host-derived class's constructor unit
/// without its instance parameter.
fn host_constructor_parameters<'program, 'src>(
    program: &'program Program<'src>,
    class: usize,
    query: &mut TypeQueryAdmission<'_, '_>,
) -> Result<Option<Vec<&'program Type<'src>>>, AllocationError> {
    let definition = &program.classes[class];
    let signature = if definition.external {
        match definition.constructor.map(|ty| &program.types[ty.index()]) {
            Some(Type::Function(signature)) => {
                return Ok(Some(signature.params.iter().map(|parameter| &parameter.ty).collect()))
            }
            _ => return Ok(None),
        }
    } else {
        query.work(program.units.len())?;
        let Some(unit) = program
            .units
            .iter()
            .find(|unit| unit.data().host_class == Some(class as u32))
        else {
            return Ok(None);
        };
        match unit.data().callable_type.map(|ty| &program.types[ty.index()]) {
            Some(Type::Function(signature)) => signature,
            _ => return Ok(None),
        }
    };
    Ok(Some(signature.params.iter().skip(1).map(|parameter| &parameter.ty).collect()))
}

fn class_assignable(
    program: &Program<'_>,
    expected: &Type<'_>,
    actual: &Type<'_>,
    query: &mut TypeQueryAdmission<'_, '_>,
) -> Result<bool, AllocationError> {
    if structurally_assignable(expected, actual, query)? {
        return Ok(true);
    }
    query.work(1)?;
    Ok(match (expected, actual) {
        (Type::Array(expected), Type::Array(actual)) => {
            class_assignable(program, expected, actual, query)?
                && class_assignable(program, actual, expected, query)?
        }
        (Type::Task(expected), Type::Task(actual))
        | (Type::Generator(expected), Type::Generator(actual))
        | (Type::Nullable(expected), Type::Nullable(actual)) => {
            class_assignable(program, expected, actual, query)?
        }
        (Type::Nullable(expected), actual) => class_assignable(program, expected, actual, query)?,
        (Type::Union(expected), Type::Union(actual)) => {
            let mut all = true;
            for actual in actual {
                let mut any = false;
                for expected in expected {
                    any |= class_assignable(program, expected, actual, query)?;
                }
                all &= any;
            }
            all
        }
        (Type::Union(expected), actual) => {
            let mut any = false;
            for expected in expected {
                any |= class_assignable(program, expected, actual, query)?;
            }
            any
        }
        (expected, Type::Union(actual)) => {
            let mut all = true;
            for actual in actual {
                all &= class_assignable(program, expected, actual, query)?;
            }
            all
        }
        // An instance of a generic class, or of a class with a generic base:
        // walk up the chain, substituting each class's parameters into its
        // base's arguments, and compare the arguments where the names meet.
        (
            Type::ClassInstance { .. } | Type::Class(_),
            Type::ClassInstance { .. } | Type::Class(_),
        ) if matches!(expected, Type::ClassInstance { .. })
            || matches!(actual, Type::ClassInstance { .. }) =>
        {
            let (target, target_args): (&str, &[Type<'_>]) = match expected {
                Type::ClassInstance { name, args } => (name, args),
                Type::Class(name) => (name, &[]),
                _ => unreachable!(),
            };
            let (mut current, mut args): (&str, Vec<Type<'_>>) = match actual {
                Type::ClassInstance { name, args } => (name, args.clone()),
                Type::Class(name) => (name, Vec::new()),
                _ => unreachable!(),
            };
            loop {
                query.work(program.classes.len())?;
                if current == target {
                    if args.len() != target_args.len() {
                        break false;
                    }
                    let mut equal = true;
                    for (left, right) in target_args.iter().zip(&args) {
                        equal &= type_equal_with(left, right, query)?;
                    }
                    break equal;
                }
                let Some(class) = program.class(current) else {
                    break false;
                };
                let Some(base) = class.base.as_deref() else {
                    break false;
                };
                let mut next = Vec::with_capacity(class.base_arguments.len());
                for &argument in &class.base_arguments {
                    next.push(crate::check::type_substitution::substitute_type_with(
                        &program.types[argument.index()],
                        &mut |name: &str, query: &mut TypeQueryAdmission<'_, '_>| {
                            query.work(class.type_params.len())?;
                            Ok(class
                                .type_params
                                .iter()
                                .position(|parameter| parameter == name)
                                .and_then(|index| args.get(index)))
                        },
                        query,
                    )?);
                }
                current = base;
                args = next;
            }
        }
        (Type::Class(expected), Type::Class(actual)) => {
            // Conversion admits only non-generic inheritance.
            let mut current: &str = actual;
            loop {
                query.work(program.classes.len())?;
                let Some(base) = program.class(current).and_then(|class| class.base.as_deref()) else {
                    break false;
                };
                if base == *expected {
                    break true;
                }
                current = base;
            }
        }
        _ => false,
    })
}
