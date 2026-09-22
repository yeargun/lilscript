//! Query-local type work and temporary construction under the caller's existing
//! allocation scope. Results must be dropped before that scope releases them.

use super::binary_types::TypeConstructionAdmission;
use super::type_payload::{measure_payload, Payload, PayloadError, PayloadMeasure};
use super::type_relation::{RelationAdmission, RelationEvent};
use super::type_substitution::SubstitutionAdmission;
use super::{DefaultValue, FunctionParameter, FunctionSignature, FunctionType, Type};
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use std::convert::Infallible;

pub(crate) struct TypeQueryAdmission<'scope, 'ledger> {
    budget: &'scope mut AllocationBudget<'ledger>,
    metered: bool,
}

impl<'scope, 'ledger> TypeQueryAdmission<'scope, 'ledger> {
    pub(crate) fn new(budget: &'scope mut AllocationBudget<'ledger>) -> Self {
        let metered = budget.is_accounted();
        Self { budget, metered }
    }

    pub(crate) fn work(&mut self, units: usize) -> Result<(), AllocationError> {
        if self.metered {
            self.budget.work(WorkKind::Analysis, count(units)?)?;
        }
        Ok(())
    }

    fn measure(&mut self, root: Payload<'_, '_>) -> Result<PayloadMeasure, AllocationError> {
        match measure_payload(root, self.budget, |_| Ok::<_, Infallible>(())) {
            Ok(measured) => Ok(measured),
            Err(PayloadError::Allocation(error)) => Err(error),
            Err(PayloadError::Visitor(never)) => match never {},
        }
    }

    fn equality(
        &mut self,
        left: Payload<'_, '_>,
        right: Payload<'_, '_>,
    ) -> Result<(), AllocationError> {
        let left = self.measure(left)?;
        let right = self.measure(right)?;
        // Each existing derived equality visits bounded inline fields per
        // payload record and can compare each borrowed name/default byte.
        // This is a per-comparison conservative bound, not an assignability
        // bound or a claim that shared Arc payload is actually traversed.
        let nodes = left
            .nodes
            .checked_add(right.nodes)
            .ok_or(AllocationError::Capacity)?;
        let text = left
            .text_bytes
            .checked_add(right.text_bytes)
            .ok_or(AllocationError::Capacity)?;
        let work = nodes
            .checked_mul(16)
            .and_then(|nodes| nodes.checked_add(text))
            .ok_or(AllocationError::Capacity)?;
        self.budget.work(WorkKind::Analysis, work)
    }
}

fn count(value: usize) -> Result<u64, AllocationError> {
    u64::try_from(value).map_err(|_| AllocationError::Capacity)
}

impl RelationAdmission for TypeQueryAdmission<'_, '_> {
    type Error = AllocationError;

    fn admit(&mut self, event: RelationEvent<'_, '_>) -> Result<(), AllocationError> {
        if !self.metered {
            // Inspection/full unmetered verification must not pay cost-only
            // payload traversals or allocate their temporary branch stacks.
            return Ok(());
        }
        match event {
            RelationEvent::TypeWork(units) => self.work(units),
            RelationEvent::Visit { .. } | RelationEvent::ParameterPair => self.work(1),
            RelationEvent::TypeEquality { left, right } => {
                self.equality(Payload::Type(left), Payload::Type(right))
            }
            RelationEvent::DefaultEquality { left, right } => {
                self.equality(Payload::Default(left), Payload::Default(right))
            }
            RelationEvent::SignatureValidation(signature) => self.work(
                signature
                    .params
                    .len()
                    .checked_add(1)
                    .ok_or(AllocationError::Capacity)?,
            ),
        }
    }
}

impl TypeConstructionAdmission for TypeQueryAdmission<'_, '_> {
    fn clone_type<'src>(&mut self, value: &Type<'src>) -> Result<Type<'src>, AllocationError> {
        if self.metered {
            let measured = self.measure(Payload::Type(value))?;
            // Prepay cloning and later temporary destruction. Actual cloning
            // shares FunctionType Arc payload; the full nested allowance is
            // deliberately conservative and remains until query-scope Drop.
            let work = measured
                .nodes
                .checked_mul(2)
                .ok_or(AllocationError::Capacity)?;
            self.budget.work(WorkKind::Analysis, work)?;
            self.budget
                .retain(AllocationClass::Scratch, measured.owned_bytes)?;
        }
        // Recoverable admission completes before standard Clone. Like the
        // existing admitted Box owners, allocator OOM here may abort; no claim
        // of recoverable allocator failure is made for standard Type::clone.
        Ok(value.clone())
    }

    fn box_type<'src>(&mut self, value: Type<'src>) -> Result<Box<Type<'src>>, AllocationError> {
        self.budget.boxed(AllocationClass::Scratch, value)
    }

    fn push_type<'src>(
        &mut self,
        values: &mut Vec<Type<'src>>,
        value: Type<'src>,
    ) -> Result<(), AllocationError> {
        self.budget.push(AllocationClass::Scratch, values, value)
    }
}

impl SubstitutionAdmission for TypeQueryAdmission<'_, '_> {
    fn parameters<'src>(
        &mut self,
        count: usize,
    ) -> Result<Vec<FunctionParameter<'src>>, AllocationError> {
        self.budget.vector(AllocationClass::Scratch, count)
    }
    fn clone_default<'src>(
        &mut self,
        value: &DefaultValue<'src>,
    ) -> Result<DefaultValue<'src>, AllocationError> {
        if self.metered {
            let measured = self.measure(Payload::Default(value))?;
            self.budget.work(
                WorkKind::Analysis,
                measured
                    .nodes
                    .checked_mul(2)
                    .ok_or(AllocationError::Capacity)?,
            )?;
            self.budget
                .retain(AllocationClass::Scratch, measured.owned_bytes)?;
        }
        Ok(value.clone())
    }
    fn signature<'src>(
        &mut self,
        value: FunctionSignature<'src>,
    ) -> Result<FunctionType<'src>, AllocationError> {
        self.work(1)?;
        self.budget.retain(
            AllocationClass::Scratch,
            (std::mem::size_of::<FunctionSignature<'src>>() + 2 * std::mem::size_of::<usize>())
                as u64,
        )?;
        // Like existing admitted Box/Arc owners, standard allocator OOM may
        // abort after admission. There is no recoverable allocator claim here.
        Ok(FunctionType::new(value))
    }
    fn parameter_names<'src>(
        &mut self,
        names: &[&'src str],
    ) -> Result<Vec<&'src str>, AllocationError> {
        self.work(names.len())?;
        let mut result = self.budget.vector(AllocationClass::Scratch, names.len())?;
        result.extend_from_slice(names);
        Ok(result)
    }
}

#[cfg(test)]
#[path = "type_admission_tests.rs"]
mod tests;
