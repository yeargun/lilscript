//! Sealed physical function export for one explicit two-resource cut.
//!
//! This is target evidence over one retained semantic snapshot, not an external
//! type summary. Construction consumes the existing complete FunctionLayout
//! proof even when the selected transport remains Packed. All adapters transfer
//! raw payloads; schema Int fields are not a license to coerce during transport.
use super::function_layout::{FunctionLayout, ProductTransport};
use super::implementations::{FunctionEvidence, ImplementationMap, SharedFunction};
use super::publication::CandidateError;
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget, AllocationClass::Retained, AllocationError, RetainedCharge,
};
use crate::primitive::ParameterPassing;
use std::mem::size_of;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalParameter {
    position: u32,
    schema: NominalId,
    transport: ProductTransport,
    fields: [NominalMemberId; 2],
    field_types: [TypeId; 2],
}
impl PhysicalParameter {
    pub fn position(&self) -> u32 {
        self.position
    }
    pub fn schema(&self) -> NominalId {
        self.schema
    }
    pub fn transport(&self) -> ProductTransport {
        self.transport
    }
    pub fn fields(&self) -> &[NominalMemberId] {
        &self.fields
    }
    pub fn field_types(&self) -> &[TypeId] {
        &self.field_types
    }
}

#[derive(Debug)]
pub(super) struct PhysicalExport {
    source_identity: RevisionId,
    cell: CellId,
    body: UnitId,
    initializer: UnitId,
    signature: TypeId,
    result: TypeId,
    source_specifier: String,
    export_name: String,
    parameters: [PhysicalParameter; 1],
    /// Same complete actual/capture/creator owner as private call transport.
    /// It proves raw product presence; it does not assert primitive leaf data.
    proof: FunctionEvidence,
    owner: RevisionId,
    charge: RetainedCharge<RevisionId>,
}
impl PhysicalExport {
    pub(super) fn description(
        &self,
    ) -> super::implementation_identity::PhysicalExportDescription<'_> {
        super::implementation_identity::PhysicalExportDescription::new(
            self.cell,
            self.body,
            self.initializer,
            self.signature,
            self.result,
            &self.source_specifier,
            &self.export_name,
            &self.parameters,
        )
    }
    pub(super) fn source_identity(&self) -> RevisionId {
        self.source_identity
    }
    pub(super) fn cell(&self) -> CellId {
        self.cell
    }
    pub(super) fn body(&self) -> UnitId {
        self.body
    }
    pub(super) fn initializer(&self) -> UnitId {
        self.initializer
    }
    pub(super) fn signature(&self) -> TypeId {
        self.signature
    }
    pub(super) fn result(&self) -> TypeId {
        self.result
    }
    pub(super) fn source_specifier(&self) -> &str {
        &self.source_specifier
    }
    pub(super) fn export_name(&self) -> &str {
        &self.export_name
    }
    pub(super) fn parameters(&self) -> &[PhysicalParameter] {
        &self.parameters
    }
    pub(super) fn validation_work(&self) -> Option<u64> {
        self.proof
            .borrow()
            .dependencies()
            .validation_work()?
            .checked_add(1)
    }
    pub(super) fn valid_for_published(
        &self,
        identity: RevisionId,
        program: &Program<'_>,
        uses: &UseIndex,
    ) -> bool {
        identity == self.source_identity
            && self
                .proof
                .borrow()
                .dependencies()
                .valid_for_published(program, uses)
    }
    /// Physical comparison is admitted before string scans. Raw result and
    /// unbound strict invocation are fixed by this constructor's closed scope.
    pub(super) fn same_abi(
        &self,
        other: &Self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        if self.source_identity != other.source_identity
            || self.cell != other.cell
            || self.body != other.body
            || self.initializer != other.initializer
            || self.signature != other.signature
            || self.result != other.result
            || self.parameters != other.parameters
        {
            return Ok(false);
        }
        let bytes = self
            .source_specifier
            .len()
            .checked_add(other.source_specifier.len())
            .and_then(|n| n.checked_add(self.export_name.len()))
            .and_then(|n| n.checked_add(other.export_name.len()))
            .ok_or(AllocationError::Capacity)?;
        budget.work(
            WorkKind::Analysis,
            u64::try_from(bytes).map_err(|_| AllocationError::Capacity)?,
        )?;
        Ok(
            self.source_specifier == other.source_specifier
                && self.export_name == other.export_name,
        )
    }
    pub(super) fn retained_bytes(&self) -> u64 {
        self.charge.bytes() + self.proof.retained_bytes()
    }
    pub(super) fn shared_proof_bytes(&self, other: &Self) -> u64 {
        if self.proof.same_owner(&other.proof) {
            self.proof.retained_bytes()
        } else {
            0
        }
    }
    pub(super) fn contains_function_owner(&self, owner: &Arc<SharedFunction>) -> bool {
        self.proof.owns_shared(owner)
    }
}

/// Only explicitly produced resources pay for this immutable Arc owner.
/// No public Clone or freely retainable borrowed semantic payload is exposed.
#[derive(Debug)]
#[must_use = "retain in the compilation or discard through its original budget"]
pub(super) struct SharedPhysicalExport(Arc<PhysicalExport>);
impl SharedPhysicalExport {
    pub(super) fn borrow(&self) -> &PhysicalExport {
        &self.0
    }
    pub(super) fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub(super) fn share(&self, budget: &mut AllocationBudget<'_>) -> Result<Self, AllocationError> {
        budget.work(WorkKind::Edit, 1)?;
        Ok(Self(Arc::clone(&self.0)))
    }
    pub(super) fn discard(self, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
        if let Ok(value) = Arc::try_unwrap(self.0) {
            let PhysicalExport {
                source_specifier,
                export_name,
                proof,
                owner,
                charge,
                ..
            } = value;
            drop(source_specifier);
            drop(export_name);
            budget.with_ledger(|ledger| {
                let (ledger, _) = ledger.ok_or(AllocationError::Unaccounted)?;
                proof.discard(ledger)?;
                charge.discard(&owner, ledger).map_err(|(_, error)| error)
            })?;
        }
        Ok(())
    }
    /// The passed proof is an admitted source query, never caller-authored ABI
    /// assertions. Refusal consumes it but never an artifact or checkpoint.
    pub(super) fn derive(
        owner: RevisionId,
        source_identity: RevisionId,
        program: &Program<'_>,
        selected: &ImplementationMap,
        cell: CellId,
        source_specifier: &str,
        export_name: &str,
        proof: FunctionEvidence,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, CandidateError> {
        let prepared = (|| {
            let mut storage = budget.scope();
            storage.work(
                WorkKind::Analysis,
                u64::try_from(
                    source_specifier
                        .len()
                        .checked_add(export_name.len())
                        .ok_or(AllocationError::Capacity)?,
                )
                .map_err(|_| AllocationError::Capacity)?,
            )?;
            // A relative filename is an explicit resource request. Do not infer
            // a source module filename, append extensions, or normalize bytes.
            if !source_specifier.starts_with("./")
                || source_specifier.len() <= 2
                || !crate::structured_js::identifier_name(export_name)
            {
                return Err(CandidateError::InvalidRequest);
            }
            let (body, initializer, signature, result, parameter) =
                check_cut(program, selected, cell, proof.borrow(), &mut storage)?;
            let source_specifier = storage.string(Retained, source_specifier)?;
            let export_name = storage.string(Retained, export_name)?;
            let wrapper = size_of::<PhysicalExport>()
                .checked_add(2 * size_of::<usize>())
                .ok_or(AllocationError::Capacity)?;
            storage.retain(
                Retained,
                u64::try_from(wrapper).map_err(|_| AllocationError::Capacity)?,
            )?;
            let bytes = wrapper
                .checked_add(source_specifier.capacity())
                .and_then(|n| n.checked_add(export_name.capacity()))
                .ok_or(AllocationError::Capacity)?;
            let charge = storage.detach_retained(
                owner,
                u64::try_from(bytes).map_err(|_| AllocationError::Capacity)?,
            )?;
            Ok::<_, CandidateError>((
                body,
                initializer,
                signature,
                result,
                parameter,
                source_specifier,
                export_name,
                charge,
            ))
        })();
        let (
            body,
            initializer,
            signature,
            result,
            parameter,
            source_specifier,
            export_name,
            charge,
        ) = match prepared {
            Ok(value) => value,
            Err(error) => {
                budget.with_ledger(|ledger| {
                    proof.discard(ledger.expect("physical export compilation ledger").0)
                })?;
                return Err(error);
            }
        };
        Ok(Self(Arc::new(PhysicalExport {
            source_identity,
            cell,
            body,
            initializer,
            signature,
            result,
            source_specifier,
            export_name,
            parameters: [parameter],
            proof,
            owner,
            charge,
        })))
    }
}

fn check_cut(
    program: &Program<'_>,
    selected: &ImplementationMap,
    cell: CellId,
    proof: &FunctionLayout,
    budget: &mut AllocationBudget<'_>,
) -> Result<(UnitId, UnitId, TypeId, TypeId, PhysicalParameter), CandidateError> {
    budget.work(WorkKind::Analysis, 1)?;
    let declaration = program
        .cells
        .get(cell.index())
        .ok_or(CandidateError::InvalidRequest)?;
    let CellBinding::Function(body) = declaration.binding else {
        return Err(CandidateError::InvalidRequest);
    };
    let unit = program.unit(body).ok_or(CandidateError::InvalidRequest)?;
    let initializer = program
        .unit(declaration.owner)
        .ok_or(CandidateError::InvalidRequest)?;
    let signature = unit.callable_type.ok_or(CandidateError::InvalidRequest)?;
    let Type::Function(callable) = &program.types[signature.index()] else {
        return Err(CandidateError::InvalidRequest);
    };
    if declaration.assigned
        || unit.kind != UnitKind::Function
        || !unit.captures.is_empty()
        || initializer.kind != UnitKind::ModuleInitialization
        || unit.module == program.entry_module()
        || unit.module != initializer.module
        || callable.params.len() != 1
        || !proof.inputs().runtime_inputs_sealed()
        || proof.body() != body
    {
        return Err(CandidateError::InvalidRequest);
    }
    let parameter = &callable.params[0];
    if parameter.passing != ParameterPassing::Value || parameter.default.is_some() {
        return Err(CandidateError::InvalidRequest);
    }
    let Type::Struct(nominal) = &parameter.ty else {
        return Err(CandidateError::InvalidRequest);
    };
    let schema = program
        .structs
        .get(nominal.identity.index())
        .filter(|s| s.identity == nominal.identity)
        .ok_or(CandidateError::InvalidRequest)?;
    if nominal.identity.is_class()
        || !schema.type_parameters.is_empty()
        || schema.fields.len() != 2
        || proof.parameters().len() != 1
        || proof.parameters()[0].position != 0
        || proof.parameters()[0].schema != schema.identity
    {
        return Err(CandidateError::InvalidRequest);
    }
    let fields = program
        .fields
        .get(schema.fields.clone())
        .ok_or(CandidateError::InvalidRequest)?;
    for field in fields {
        budget.work(WorkKind::Analysis, 1)?;
        if !matches!(
            program.types[field.ty.index()],
            Type::Int | Type::Float | Type::Bool
        ) {
            return Err(CandidateError::InvalidRequest);
        }
    }
    let result_kind = scalar_kind(&callable.return_type).ok_or(CandidateError::InvalidRequest)?;
    let mut result = None;
    for (index, ty) in program.types.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        if scalar_kind(ty) == Some(result_kind) {
            result = TypeId::from_index(index);
            break;
        }
    }
    let result = result.ok_or(CandidateError::InvalidRequest)?;
    let mut transport = ProductTransport::Packed;
    for layout in selected.functions() {
        budget.work(WorkKind::Analysis, 1)?;
        if layout.body() == body {
            transport = layout.transport(0);
        }
    }
    for helper in selected.helpers() {
        budget.work(WorkKind::Analysis, 1)?;
        if helper.root().cell == cell {
            return Err(CandidateError::ConflictingChoice);
        }
    }
    // A two-resource initialization cut is exact in this first slice: the
    // producer owns only this function declaration and has no import effects.
    let module = &program.modules()[unit.module.index()];
    if module.initializer != declaration.owner
        || !module.dependencies.is_empty()
        || initializer.regions.len() != 1
        || initializer.operations.len() != 2
        || initializer.instantiation_prefix != 2
    {
        return Err(CandidateError::InvalidRequest);
    }
    budget.work(WorkKind::Analysis, 2)?;
    if !matches!(initializer.operations[0].kind, OperationKind::Closure(id) if id == body)
        || !matches!(initializer.operations[1].kind, OperationKind::Initialize(id) if id == cell)
    {
        return Err(CandidateError::InvalidRequest);
    }
    let producers = proof.inputs().producers();
    budget.work(
        WorkKind::Analysis,
        u64::try_from(producers.len()).map_err(|_| AllocationError::Capacity)?,
    )?;
    if producers.len() != 1
        || producers[0].cell != cell
        || producers[0].creation.unit != declaration.owner
    {
        return Err(CandidateError::InvalidRequest);
    }
    for call in proof.inputs().calls() {
        budget.work(WorkKind::Analysis, 1)?;
        if call.caller == body {
            return Err(CandidateError::InvalidRequest);
        }
    }
    for module in program.modules() {
        budget.work(WorkKind::Analysis, 1)?;
        for import in &module.imports {
            budget.work(WorkKind::Analysis, 1)?;
            if import.module == unit.module
                && matches!(import.target, InterfaceTarget::Value(id) if id != cell)
            {
                return Err(CandidateError::InvalidRequest);
            }
        }
    }
    Ok((
        body,
        declaration.owner,
        signature,
        result,
        PhysicalParameter {
            position: 0,
            schema: schema.identity,
            transport,
            fields: [fields[0].identity, fields[1].identity],
            field_types: [fields[0].ty, fields[1].ty],
        },
    ))
}
fn scalar_kind(ty: &Type<'_>) -> Option<u8> {
    match ty {
        Type::Int => Some(0),
        Type::Float => Some(1),
        Type::Bool => Some(2),
        _ => None,
    }
}
