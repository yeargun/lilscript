//! Borrowed value provenance over original operations and complete cell uses.
//!
//! This query supplies no callable sealing, runtime-domain, purity or layout
//! authority. Its consumer retains the body/use revisions and separately owns
//! the exact CallableInputs scope and the original call's actual argument.
use super::super::raw_domains::Admission;
use super::super::uses::{CellUse, CellUseSite, UseIndex};
use super::super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::semantic_program) enum ReturnOriginUnknown {
    ControlFlow,
    Operation,
    Storage,
    TypeTransfer,
    NotParameter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::semantic_program) enum ReturnedValueOrigin {
    Parameter { position: u32 },
    Unknown(ReturnOriginUnknown),
}

/// Classify one bounded by-value forwarding body, regardless of source spelling
/// or whether its declaration is generic. The result is conditional on normal
/// completion; original reads/copies/return and all caller evaluation remain.
///
/// `consulted_cell` records complete-use dependencies in the caller's existing
/// owner. No copied graph, persistent fact cache or scratch index is installed.
/// The Program/UseIndex pair must already be coherent; body stamps are checked
/// here and consulted cell stamps are delivered before their facts are used.
pub(in crate::semantic_program) fn returned_value_origin<A: Admission>(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    admission: &mut A,
    mut consulted_cell: impl FnMut(CellId, RevisionId, &mut A) -> Result<(), A::Error>,
) -> Result<ReturnedValueOrigin, A::Error> {
    use ReturnOriginUnknown as Why;
    let unknown = |reason| Ok(ReturnedValueOrigin::Unknown(reason));
    admission.work(1)?;
    let data = program
        .unit(body)
        .ok_or_else(|| admission.invalid("return origin body"))?;
    let indexed = uses
        .unit(body)
        .ok_or_else(|| admission.invalid("return origin use body"))?;
    if uses.tables_revision() != program.tables_revision
        || indexed.revision() != program.units[body.index()].revision()
    {
        return Err(admission.invalid("return origin stale uses"));
    }
    if data.regions.len() != 1 || !data.captures.is_empty() {
        return unknown(Why::ControlFlow);
    }
    let root = &data.regions[0];
    if root.operations.len() != data.operations.len() || root.result.is_some() {
        return unknown(Why::ControlFlow);
    }
    for &cell in &data.parameters {
        admission.work(1)?;
        if program.is_reference_parameter(cell)
            || !closed_cell(
                program,
                uses,
                body,
                cell,
                None,
                admission,
                &mut consulted_cell,
            )?
        {
            return unknown(Why::Storage);
        }
    }
    let mut returned = None;
    for (position, &id) in root.operations.iter().enumerate() {
        admission.work(1)?;
        let op = data
            .operations
            .get(id.index())
            .ok_or_else(|| admission.invalid("return origin operation"))?;
        if id.index() != position || op.region.index() != 0 {
            return unknown(Why::ControlFlow);
        }
        let operands = data
            .operands(op.operands)
            .ok_or_else(|| admission.invalid("return origin operands"))?;
        admission.work(operands.len())?;
        match op.kind {
            OperationKind::Constant(_) => {}
            OperationKind::CopyValue => {
                let (Some(input), Some(result)) = (operands.first(), op.result) else {
                    return Err(admission.invalid("return origin copy signature"));
                };
                // An equal-but-distinct table entry can be supported later by
                // the common admitted relation. This cheap restriction never
                // converts assignable/erased types into invariant provenance.
                if data.values[input.index()].ty != data.values[result.index()].ty {
                    return unknown(Why::TypeTransfer);
                }
            }
            OperationKind::Load(place) => {
                let Some(Place::Cell(cell)) = data.places.get(place.index()) else {
                    return unknown(Why::Operation);
                };
                let storage = &program.cells[cell.index()];
                if storage.owner != body
                    || !matches!(
                        storage.binding,
                        CellBinding::Local | CellBinding::Parameter(_)
                    )
                    || op
                        .result
                        .is_none_or(|value| data.values[value.index()].ty != storage.ty)
                {
                    return unknown(Why::Storage);
                }
            }
            OperationKind::Initialize(cell) => {
                let Some(input) = operands.first() else {
                    return Err(admission.invalid("return origin initializer signature"));
                };
                if program.cells[cell.index()].ty != data.values[input.index()].ty {
                    return unknown(Why::TypeTransfer);
                }
                if !closed_cell(
                    program,
                    uses,
                    body,
                    cell,
                    Some(id),
                    admission,
                    &mut consulted_cell,
                )? {
                    return unknown(Why::Storage);
                }
            }
            OperationKind::Return if position + 1 == root.operations.len() => {
                let [value] = operands else {
                    return unknown(Why::NotParameter);
                };
                returned = Some(*value);
            }
            _ => return unknown(Why::Operation),
        }
    }
    let Some(mut value) = returned else {
        return unknown(Why::NotParameter);
    };
    // Every successful edge moves to a strictly earlier original definition.
    // The finite guard also makes malformed cycles an explicit refusal.
    for _ in 0..data.operations.len() {
        admission.work(1)?;
        let definition = data.values[value.index()].definition;
        let op = &data.operations[definition.index()];
        let input = match op.kind {
            OperationKind::CopyValue => data.operands(op.operands).unwrap()[0],
            OperationKind::Load(place) => {
                let Place::Cell(cell) = data.places[place.index()] else {
                    return unknown(Why::NotParameter);
                };
                match program.cells[cell.index()].binding {
                    CellBinding::Parameter(position) => {
                        if data.parameters.get(position as usize) != Some(&cell) {
                            return Err(admission.invalid("return origin parameter identity"));
                        }
                        return Ok(ReturnedValueOrigin::Parameter { position });
                    }
                    CellBinding::Local => {
                        // The same sorted UnitUses arena locates initialization;
                        // do not build a second local-cell index for this query.
                        let entries = indexed.cell_uses();
                        admission.work(search_work(entries.len()))?;
                        let start = entries.partition_point(|entry| entry.cell < cell);
                        let Some(entry) = entries.get(start).filter(|entry| entry.cell == cell)
                        else {
                            return unknown(Why::Storage);
                        };
                        let CellUse::Initialize(initialize) = entry.usage else {
                            return unknown(Why::Storage);
                        };
                        if initialize.index() >= definition.index() {
                            return unknown(Why::Storage);
                        }
                        let initializer = &data.operations[initialize.index()];
                        data.operands(initializer.operands).unwrap()[0]
                    }
                    _ => return unknown(Why::Storage),
                }
            }
            _ => return unknown(Why::NotParameter),
        };
        if data.values[input.index()].definition.index() >= definition.index() {
            return unknown(Why::ControlFlow);
        }
        value = input;
    }
    unknown(Why::ControlFlow)
}

fn search_work(length: usize) -> usize {
    if length == 0 {
        1
    } else {
        (usize::BITS - length.leading_zeros()) as usize + 1
    }
}

fn closed_cell<A: Admission>(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    cell: CellId,
    initialize: Option<OpId>,
    admission: &mut A,
    consulted_cell: &mut impl FnMut(CellId, RevisionId, &mut A) -> Result<(), A::Error>,
) -> Result<bool, A::Error> {
    admission.work(1)?;
    let storage = &program.cells[cell.index()];
    if storage.owner != body
        || !matches!(
            (initialize, storage.binding),
            (Some(_), CellBinding::Local) | (None, CellBinding::Parameter(_))
        )
    {
        return Ok(false);
    }
    let users = uses
        .cell(cell)
        .ok_or_else(|| admission.invalid("return origin cell uses"))?;
    consulted_cell(cell, users.revision(), admission)?;
    let mut origins = 0;
    for site in users.sites() {
        admission.work(1)?;
        match *site {
            CellUseSite::Unit {
                unit,
                usage: CellUse::Initialize(found),
            } if unit == body && initialize == Some(found) => origins += 1,
            CellUseSite::Unit {
                unit,
                usage: CellUse::Parameter(position),
            } if unit == body
                && initialize.is_none()
                && storage.binding == CellBinding::Parameter(position) =>
            {
                origins += 1
            }
            CellUseSite::Unit {
                unit,
                usage: CellUse::Read { operation, place },
            } if unit == body
                && initialize.is_none_or(|id| id.index() < operation.index())
                && matches!(program.unit(body).unwrap().places[place.index()], Place::Cell(found) if found == cell) =>
                {}
            _ => return Ok(false),
        }
    }
    Ok(origins == 1)
}
