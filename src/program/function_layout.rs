//! A private product calling convention over the existing complete call owner.
//! Parameters retain their source type/mode/ordinal; only physical transport
//! expands. The existing product worklist certifies raw actuals independently
//! of selecting either caller or callee scalar storage.
use super::callable_inputs::{CallObservations, CallableInputs, InputOutcome};
use super::product_family::{
    self, invalid, unknown, Attempt, OutputAdmission, ProductDependencies, ResultIn,
};
use super::uses::UseIndex;
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{BudgetError, BudgetLedger, WorkDomain};
use crate::primitive::ParameterPassing;
use std::mem::size_of;

pub(super) const FUNCTION_LAYOUT_PLAN: u32 = 7;
pub(super) const FUNCTION_LAYOUT_VERSION: u32 = 3;
pub(super) use super::product_family::{FamilyError, FamilyRequest};
pub use super::product_family::{ResourceLimit, UnknownReason};
pub(super) type FamilyOutcome = product_family::Outcome<FunctionLayout>;
pub(super) type FamilyAnalysis = product_family::Analysis<FunctionLayout>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductTransport {
    Packed,
    Fields,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ParameterLayout {
    pub position: u32,
    pub schema: NominalId,
}

#[derive(Debug)]
#[must_use = "retain in Compilation or discard through the original ledger"]
pub(super) struct FunctionLayout {
    inputs: CallableInputs,
    parameters: Vec<ParameterLayout>,
    dependencies: ProductDependencies,
    charge: (WorkDomain, u64),
}
impl FunctionLayout {
    pub(super) fn body(&self) -> UnitId {
        self.inputs.body()
    }
    pub(super) fn inputs(&self) -> &CallableInputs {
        &self.inputs
    }
    pub(super) fn parameters(&self) -> &[ParameterLayout] {
        &self.parameters
    }
    pub(super) fn parameter(&self, position: u32) -> Option<&ParameterLayout> {
        self.parameters
            .binary_search_by_key(&position, |parameter| parameter.position)
            .ok()
            .map(|index| &self.parameters[index])
    }
    pub(super) fn transport(&self, position: u32) -> ProductTransport {
        if self.parameter(position).is_some() {
            ProductTransport::Fields
        } else {
            ProductTransport::Packed
        }
    }
    pub(super) fn dependencies(&self) -> &ProductDependencies {
        &self.dependencies
    }
    pub(super) fn retained_bytes(&self) -> u64 {
        self.charge.1
    }
    pub(super) fn discard(self, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
        let (domain, bytes) = self.charge;
        drop(self);
        ledger.release(domain, bytes)
    }
}
pub(super) fn analyze(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<FamilyAnalysis, FamilyError> {
    analyze_impl(program, uses, body, request, ledger, domain, false)
}
pub(super) fn analyze_published(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<FamilyAnalysis, FamilyError> {
    analyze_impl(program, uses, body, request, ledger, domain, true)
}
fn analyze_impl(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
    coherent: bool,
) -> Result<FamilyAnalysis, FamilyError> {
    product_family::run(
        request,
        FUNCTION_LAYOUT_PLAN,
        FUNCTION_LAYOUT_VERSION,
        ledger,
        domain,
        |budget| discover(program, uses, body, coherent, budget),
        |family, charge| family.charge = charge,
    )
}
fn discover(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    coherent: bool,
    budget: &mut Attempt<'_>,
) -> ResultIn<FunctionLayout> {
    product_family::check_index(program, uses, coherent, budget)?;
    if budget.request.execution != JavaScriptExecution::Module {
        return Err(unknown(UnknownReason::ExecutionBoundary));
    }
    let data = program
        .unit(body)
        .ok_or_else(|| invalid("function layout body"))?;
    let Some(Type::Function(signature)) = data
        .callable_type
        .and_then(|ty| program.types.get(ty.index()))
    else {
        return Err(unknown(UnknownReason::CallableInterface));
    };
    budget.reserve(size_of::<FunctionLayout>() as u64, true)?;
    let mut parameters = Vec::new();
    for (position, parameter) in signature.params.iter().enumerate() {
        budget.work(1)?;
        if parameter.default.is_some() {
            return Err(unknown(UnknownReason::CallableInterface));
        }
        if parameter.passing != ParameterPassing::Value {
            continue;
        }
        let Type::Struct(declaration) = &parameter.ty else {
            continue;
        };
        let schema = program
            .structs
            .get(declaration.identity.index())
            .filter(|schema| schema.identity == declaration.identity)
            .ok_or_else(|| invalid("function layout schema"))?;
        if declaration.identity.is_class() || !schema.type_parameters.is_empty() {
            return Err(unknown(UnknownReason::UnsupportedSchema));
        }
        budget.push(
            &mut parameters,
            ParameterLayout {
                position: u32::try_from(position).map_err(|_| FamilyError::Capacity)?,
                schema: declaration.identity,
            },
            true,
        )?;
    }
    if parameters.is_empty() {
        return Err(unknown(UnknownReason::NoProductParameters));
    }
    let InputOutcome::Complete(inputs) = CallableInputs::for_body_published(
        program,
        uses,
        body,
        CallObservations::from_execution(budget.request.execution),
        &mut OutputAdmission(budget),
    )?
    else {
        return Err(unknown(UnknownReason::CallableInterface));
    };
    let dependencies =
        product_family::certify_parameters(program, uses, &inputs, &parameters, budget)?;
    Ok(FunctionLayout {
        inputs,
        parameters,
        dependencies,
        charge: (WorkDomain::Optional, 0),
    })
}
