//! Physical callable signature planning inside the native target owner.
//! Source types stay borrowed. The explicit stack follows only supported
//! function parameter/result edges and never creates a semantic type graph.

use super::{native_type, work, NativeError, NativeType, TypeTables};
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};
use crate::primitive::ParameterPassing;
use crate::semantic::type_admission::TypeQueryAdmission;
use crate::semantic::type_relation::type_equal_with;
use crate::semantic::{FunctionSignature, Type};
use crate::semantic_program::Program;
use std::mem::size_of;

#[derive(Debug)]
pub(in crate::semantic_program) struct NativeSignature<'program, 'src> {
    pub(in crate::semantic_program) source: &'program FunctionSignature<'src>,
    pub(in crate::semantic_program) parameters: Vec<NativeType>,
    pub(in crate::semantic_program) result: NativeType,
    pub(in crate::semantic_program) needed: bool,
    ty: &'program Type<'src>,
}

struct Frame<'program, 'src> {
    ty: &'program Type<'src>,
    values: Vec<NativeType>,
}

/// Lookup uses the common paid equality owner. No borrowed signature gets
/// reconstructed as a cloned Type just to test its semantic identity.
pub(super) fn lookup(
    signatures: &[NativeSignature<'_, '_>],
    ty: &Type<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<usize>, NativeError> {
    for (index, signature) in signatures.iter().enumerate() {
        work(budget, 1)?;
        let mut query = budget.scope();
        if type_equal_with(signature.ty, ty, &mut TypeQueryAdmission::new(&mut query))? {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

/// The parameter/result schema of a callable source type.
pub(super) fn signature_of<'a, 'src>(ty: &'a Type<'src>) -> Option<&'a FunctionSignature<'src>> {
    match ty {
        Type::Function(signature) => Some(signature),
        Type::GenericFunction(function) => Some(&function.signature),
        _ => None,
    }
}

pub(super) fn register<'program, 'src>(
    program: &'program Program<'src>,
    tables: &mut TypeTables<'program, 'src>,
    ty: &'program Type<'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<usize, NativeError> {
    if let Some(index) = lookup(&tables.signatures, ty, budget)? {
        return Ok(index);
    }
    let mut stack = Vec::new();
    push_frame(&mut stack, ty, budget)?;
    let result = loop {
        let frame = stack.last().unwrap();
        let Some(source) = signature_of(frame.ty) else {
            unreachable!("only callable types enter the native signature stack")
        };
        work(budget, 1)?;
        if frame.values.len() > source.params.len() {
            let mut frame = stack.pop().unwrap();
            let result = frame.values.pop().unwrap();
            let index = tables.signatures.len();
            budget.push(
                Scratch,
                &mut tables.signatures,
                NativeSignature {
                    source,
                    parameters: frame.values,
                    result,
                    needed: false,
                    ty: frame.ty,
                },
            )?;
            if let Some(parent) = stack.last_mut() {
                // The frame vector admitted its complete parameter/result
                // capacity before the child was traversed.
                parent.values.push(NativeType::Callable(index));
            } else {
                break index;
            }
            continue;
        }
        let child = if let Some(parameter) = source.params.get(frame.values.len()) {
            &parameter.ty
        } else {
            &source.return_type
        };
        if matches!(child, Type::Function(_) | Type::GenericFunction(_)) {
            if source
                .params
                .get(frame.values.len())
                .is_some_and(|parameter| parameter.passing == ParameterPassing::MutableReference)
            {
                return Err(NativeError::unsupported(
                    None,
                    None,
                    Default::default(),
                    "native mutable-reference callable payload",
                ));
            }
            if let Some(index) = lookup(&tables.signatures, child, budget)? {
                stack
                    .last_mut()
                    .unwrap()
                    .values
                    .push(NativeType::Callable(index));
            } else {
                push_frame(&mut stack, child, budget)?;
            }
        } else {
            let Some(value) = native_type(program, child, tables, budget)? else {
                if std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
                    eprintln!("native callable payload type refused: {child} in {}", frame.ty);
                }
                return Err(NativeError::unsupported(
                    None,
                    None,
                    Default::default(),
                    "native callable payload type",
                ));
            };
            if value == NativeType::Void && frame.values.len() < source.params.len() {
                return Err(NativeError::unsupported(
                    None,
                    None,
                    Default::default(),
                    "native void callable parameter",
                ));
            }
            stack.last_mut().unwrap().values.push(value);
        }
    };
    let bytes = stack
        .capacity()
        .checked_mul(size_of::<Frame<'_, '_>>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(AllocationError::Capacity)?;
    drop(stack);
    budget.release(Scratch, bytes)?;
    Ok(result)
}

fn push_frame<'program, 'src>(
    stack: &mut Vec<Frame<'program, 'src>>,
    ty: &'program Type<'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), NativeError> {
    let Some(signature) = signature_of(ty) else {
        return Err(NativeError::unsupported(
            None,
            None,
            Default::default(),
            "native callable type",
        ));
    };
    // Defaults need no physical ABI: callers evaluate them, except an arrow
    // default, whose omission is an empty callable the callee's guard sees.
    work(budget, signature.params.len())?;
    let capacity = signature
        .params
        .len()
        .checked_add(1)
        .ok_or(AllocationError::Capacity)?;
    let values = budget.vector(Scratch, capacity)?;
    budget.push(Scratch, stack, Frame { ty, values })?;
    Ok(())
}

/// Child signature IDs precede their parents. Marking a used physical ABI is
/// one paid descending walk, with no second graph or recursive helper demand.
pub(super) fn require(
    signatures: &mut [NativeSignature<'_, '_>],
    index: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), NativeError> {
    if signatures[index].needed {
        return Ok(());
    }
    signatures[index].needed = true;
    for current in (0..=index).rev() {
        work(budget, 1)?;
        if !signatures[current].needed {
            continue;
        }
        for position in 0..=signatures[current].parameters.len() {
            work(budget, 1)?;
            let value = signatures[current]
                .parameters
                .get(position)
                .copied()
                .unwrap_or(signatures[current].result);
            if let NativeType::Callable(child) = value {
                debug_assert!(child < current);
                signatures[child].needed = true;
            }
        }
    }
    Ok(())
}
