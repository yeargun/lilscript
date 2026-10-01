//! Canonical checked types shared by all source modules. Source facts hold a
//! compact identity; names remain diagnostic data on the canonical type.
use super::type_admission::TypeQueryAdmission;
use super::type_payload::{measure_payload, Payload, PayloadError, PayloadMeasure};
use super::{type_relation::storage_equal_with, Type};
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};
use std::convert::Infallible;
use std::hash::{BuildHasher, Hash, Hasher};
use std::num::NonZeroU32;

/// A type identity is qualified by its checked compilation, like a SymbolId.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CheckedTypeId(NonZeroU32);
impl CheckedTypeId {
    pub(crate) fn index(self) -> usize {
        self.0.get() as usize - 1
    }
    fn new(index: usize) -> Result<Self, AllocationError> {
        u32::try_from(index)
            .ok()
            .and_then(|n| n.checked_add(1))
            .and_then(NonZeroU32::new)
            .map(Self)
            .ok_or(AllocationError::Capacity)
    }
}

#[derive(Clone, Debug)]
struct Entry<'src> {
    fingerprint: u64,
    ty: Type<'src>,
}
#[derive(Clone, Debug, Default)]
pub(super) struct TypePool<'src> {
    entries: Vec<Entry<'src>>,
    slots: Vec<Option<CheckedTypeId>>,
    pub(super) absence_pins: bool,
}
impl<'src> TypePool<'src> {
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn get(&self, id: CheckedTypeId) -> &Type<'src> {
        &self.entries[id.index()].ty
    }
    #[cfg(test)]
    pub(super) fn storage_bytes(&self) -> u64 {
        let buffers = self.entries.capacity() * std::mem::size_of::<Entry<'_>>()
            + self.slots.capacity() * std::mem::size_of::<Option<CheckedTypeId>>();
        buffers as u64
            + self
                .entries
                .iter()
                .map(|entry| {
                    measure_payload(
                        Payload::Type(&entry.ty),
                        &mut AllocationBudget::new(None),
                        |_| Ok::<_, Infallible>(()),
                    )
                    .unwrap()
                    .owned_bytes
                })
                .sum::<u64>()
    }
    pub(super) fn intern(
        &mut self,
        ty: &Type<'src>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<CheckedTypeId, AllocationError> {
        let (hash, measured, pins) = fingerprint(ty, budget)?;
        self.absence_pins |= pins;
        if !self.slots.is_empty() {
            let mut position = hash as usize & (self.slots.len() - 1);
            while let Some(id) = self.slots[position] {
                budget.work(WorkKind::Analysis, 1)?;
                let known = &self.entries[id.index()];
                if known.fingerprint == hash
                    && storage_equal_with(&known.ty, ty, &mut TypeQueryAdmission::new(budget))?
                {
                    return Ok(id);
                }
                position = (position + 1) & (self.slots.len() - 1);
            }
        }
        let id = CheckedTypeId::new(self.entries.len())?;
        if self.entries.len() >= self.slots.len() / 2 {
            let capacity = self
                .slots
                .len()
                .checked_mul(2)
                .ok_or(AllocationError::Capacity)?
                .max(8);
            let mut slots = budget.filled(Scratch, capacity, None)?;
            for (index, entry) in self.entries.iter().enumerate() {
                budget.work(WorkKind::Analysis, 1)?;
                let mut position = entry.fingerprint as usize & (capacity - 1);
                while slots[position].is_some() {
                    budget.work(WorkKind::Analysis, 1)?;
                    position = (position + 1) & (capacity - 1);
                }
                slots[position] = Some(CheckedTypeId::new(index)?);
            }
            let old = std::mem::replace(&mut self.slots, slots);
            let bytes = u64::try_from(
                old.capacity()
                    .checked_mul(std::mem::size_of::<Option<CheckedTypeId>>())
                    .ok_or(AllocationError::Capacity)?,
            )
            .map_err(|_| AllocationError::Capacity)?;
            drop(old);
            budget.release(Scratch, bytes)?;
        }
        budget.reserve_vec(Scratch, &mut self.entries, 1)?;
        budget.work(
            WorkKind::Analysis,
            measured
                .nodes
                .checked_mul(2)
                .ok_or(AllocationError::Capacity)?,
        )?;
        budget.retain(Scratch, measured.owned_bytes)?;
        let canonical = ty.clone();
        let mut position = hash as usize & (self.slots.len() - 1);
        while self.slots[position].is_some() {
            budget.work(WorkKind::Analysis, 1)?;
            position = (position + 1) & (self.slots.len() - 1);
        }
        self.entries.push(Entry {
            fingerprint: hash,
            ty: canonical,
        });
        self.slots[position] = Some(id);
        Ok(id)
    }
}

fn fingerprint(
    ty: &Type<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(u64, PayloadMeasure, bool), AllocationError> {
    let mut hash = crate::stable_hash::StableBuildHasher::default().build_hasher();
    let mut pins = false;
    let measured = measure_payload(Payload::Type(ty), budget, |node| {
        match node {
            Payload::Type(ty) => {
                std::mem::discriminant(ty).hash(&mut hash);
                match ty {
                    Type::Enum(d) | Type::Class(d) | Type::Struct(d) => d.identity.hash(&mut hash),
                    Type::ClassInstance { declaration, args }
                    | Type::StructInstance { declaration, args } => {
                        declaration.identity.hash(&mut hash);
                        args.len().hash(&mut hash);
                    }
                    Type::Union(members) | Type::Intersection(members) => members.len().hash(&mut hash),
                    Type::ModuleNamespace(module) => module.hash(&mut hash),
                    Type::Nullable(inner) => {
                        inner.boundary.hash(&mut hash);
                        pins |= inner.boundary != super::AbsencePin::Auto;
                    },
                    Type::GenericFunction(function) => {
                        function.type_params.len().hash(&mut hash);
                        for parameter in &function.type_params {
                            parameter.identity.hash(&mut hash);
                        }
                    }
                    Type::TypeParameter(parameter) => parameter.identity.hash(&mut hash),
                    _ => {}
                }
            }
            Payload::Signature(signature) => {
                signature.params.len().hash(&mut hash);
                for parameter in &signature.params {
                    parameter.passing.hash(&mut hash);
                    parameter.optional.hash(&mut hash);
                    parameter.rest.hash(&mut hash);
                    parameter.receiver.hash(&mut hash);
                }
            }
            Payload::Default(_) => unreachable!("defaults are declaration expressions"),
        }
        Ok::<_, Infallible>(())
    });
    match measured {
        Ok(measured) => Ok((hash.finish(), measured, pins)),
        Err(PayloadError::Allocation(error)) => Err(error),
        Err(PayloadError::Visitor(never)) => match never {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{
        FunctionParameter, FunctionSignature, FunctionType, GenericFunctionType, TypeParameter,
    };
    #[test]
    fn s4_type_pool_shares_structure_and_preserves_declaring_binders() {
        let mut pool = TypePool::default();
        let mut budget = AllocationBudget::new(None);
        let array = Type::Array(Box::new(Type::Int));
        assert_eq!(
            pool.intern(&array, &mut budget).unwrap(),
            pool.intern(&array, &mut budget).unwrap()
        );
        let a = TypeParameter::fixture("A");
        let b = TypeParameter::fixture("B");
        let generic = |p| {
            Type::GenericFunction(GenericFunctionType {
                type_params: vec![p],
                signature: FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(Type::TypeParameter(p))],
                    return_type: Box::new(Type::TypeParameter(p)),
                }),
            })
        };
        assert!(crate::check::is_type_assignable(&generic(a), &generic(b)));
        assert_ne!(
            pool.intern(&generic(a), &mut budget).unwrap(),
            pool.intern(&generic(b), &mut budget).unwrap()
        );
        let mut renamed = a;
        renamed.name = "renamed";
        assert_eq!(
            pool.intern(&generic(a), &mut budget).unwrap(),
            pool.intern(&generic(renamed), &mut budget).unwrap()
        );
        assert_ne!(
            pool.intern(&Type::TypeParameter(a), &mut budget).unwrap(),
            pool.intern(&Type::TypeParameter(b), &mut budget).unwrap()
        );
        assert_eq!(pool.len(), 5);
    }
}
