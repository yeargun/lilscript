//! One direct private reference ABI. Source places stay in the common call
//! arena; this owner selects carriers, immutable paths and ordinary functions
//! in the existing target Module. It never constructs another source graph.
use super::super::callable_inputs::{CallObservations, CallableInputs, InputOutcome};
use super::super::demand::{InlineActual, LocationCheck, PlaceLocation};
use super::super::raw_domains::Admission;
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::primitive::ParameterPassing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceMode {
    PackedOnly,
    Mixed,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PreparedLocation {
    /// Original plan context/call/position suffice; no duplicate Place is retained.
    Local,
    Shared {
        root: js::BindingId,
        path: js::BindingId,
    },
}
#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedReference {
    pub(super) call: CallId,
    pub(super) position: u32,
    pub(super) location: PreparedLocation,
}

pub(super) struct Plan {
    helpers: [Option<js::BindingId>; 5],
    mode: Option<ReferenceMode>,
    schemas: Vec<(usize, js::BindingId, js::BindingId)>,
    rebuilds: Vec<(usize, js::BindingId)>,
    paths: Vec<(UnitId, PlaceId, js::BindingId)>,
    prefix: Vec<js::Statement>,
}
impl Plan {
    pub(super) fn new() -> Self {
        Self {
            helpers: [None; 5],
            mode: None,
            schemas: Vec::new(),
            rebuilds: Vec::new(),
            paths: Vec::new(),
            prefix: Vec::new(),
        }
    }
}

pub(super) fn is_reference(program: &Program<'_>, cell: CellId) -> bool {
    program
        .parameter(cell)
        .is_some_and(|parameter| parameter.passing == ParameterPassing::MutableReference)
}

pub(super) struct Meter<'a, 'b>(pub(super) &'a mut AllocationBudget<'b>);
impl Admission for Meter<'_, '_> {
    type Error = FormationError;
    fn work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.work(
            WorkKind::Analysis,
            u64::try_from(amount).map_err(|_| AllocationError::Capacity)?,
        )?;
        Ok(())
    }
    fn vector<T>(&mut self, capacity: usize) -> Result<Vec<T>, Self::Error> {
        self.work(capacity)?;
        Ok(self.0.vector(AllocationClass::Scratch, capacity)?)
    }
    fn push<T>(&mut self, target: &mut Vec<T>, value: T) -> Result<(), Self::Error> {
        self.work(1)?;
        Ok(self.0.push(AllocationClass::Scratch, target, value)?)
    }
    fn release<T>(&mut self, value: Vec<T>) -> Result<(), Self::Error> {
        let bytes = value
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(AllocationError::Capacity)?;
        drop(value);
        Ok(self.0.release(AllocationClass::Scratch, bytes as u64)?)
    }
    fn invalid(&self, reason: &'static str) -> Self::Error {
        Unsupported {
            span: Span::default(),
            feature: reason,
        }
        .into()
    }
}

impl<'demand, 'program, 'src, 'budget, 'ledger>
    Formation<'demand, 'program, 'src, 'budget, 'ledger>
{
    fn reference_mode(&mut self) -> Result<ReferenceMode, FormationError> {
        if let Some(mode) = self.reference_plan.mode {
            return Ok(mode);
        }
        let demand = self.demand;
        let mut meter = Meter(self.budget);
        let mixed = demand.has_shared_product_locations_visited(|n| meter.work(n))?;
        let mode = if mixed {
            ReferenceMode::Mixed
        } else {
            ReferenceMode::PackedOnly
        };
        self.reference_plan.mode = Some(mode);
        Ok(mode)
    }

    pub(super) fn reference_check_proved(
        &mut self,
        context: ContextId,
        place: PlaceId,
        check: LocationCheck,
    ) -> Result<bool, FormationError> {
        let demand = self.demand;
        let mut meter = Meter(self.budget);
        demand.location_check_proved_visited(PlaceLocation { context, place }, check, |n| {
            meter.work(n)
        })
    }

    pub(super) fn inline_reference_actual(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<Option<InlineActual<'program>>, FormationError> {
        if !is_reference(self.program, cell) {
            return Ok(None);
        }
        let demand = self.demand;
        let mut meter = Meter(self.budget);
        let owner = demand
            .cell_owner_visited(context, cell, || meter.work(1))?
            .ok_or_else(|| meter.invalid("reference outside owning activation"))?;
        if !demand.context(owner).kind.is_inline() {
            return Ok(None);
        }
        meter.work(1)?;
        let CellBinding::Parameter(position) = self.program.cells[cell.index()].binding else {
            unreachable!("verified reference formal")
        };
        Ok(Some(demand.inline_actual(owner, position)))
    }

    // Only compiler-generated schemaCopy/schemaAssign routines reach here.
    // They never observe this or their frame, so a member call is equivalent
    // without detachment. Original source invocation semantics stay separate.
    fn generated_schema_call(
        &mut self,
        callee: js::ExprId,
        arguments: &[js::ExprId],
    ) -> Result<js::ExprId, FormationError> {
        let arguments = self
            .budget
            .copy_slice(AllocationClass::Retained, arguments)?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Reference,
        })
    }
    fn carrier_test(&mut self, root: js::BindingId) -> Result<js::ExprId, FormationError> {
        let object = self.reference(root)?;
        let property = js::Property::Named(self.text("length")?);
        let left = self.expression(js::Expr::Member { object, property })?;
        let right = self.literal(js::Literal::Number(1.0))?;
        self.expression(js::Expr::Binary {
            op: js::Binary::StrictEqual,
            left,
            right,
        })
    }
    fn path_bank_key(&mut self, path: js::BindingId) -> Result<js::ExprId, FormationError> {
        let left = self.binding_slot(path, 0)?;
        let right = self.literal(js::Literal::Number(2.0))?;
        self.expression(js::Expr::Binary {
            op: js::Binary::Add,
            left,
            right,
        })
    }
    fn computed_slot(
        &mut self,
        root: js::BindingId,
        key: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let object = self.reference(root)?;
        self.expression(js::Expr::Member {
            object,
            property: js::Property::Computed(key),
        })
    }
    fn return_if_null(
        &mut self,
        body: js::RegionId,
        binding: js::BindingId,
        value: Option<js::ExprId>,
    ) -> Result<(), FormationError> {
        let branch = self
            .module
            .region_in(self.module.regions[body.index()].scope, self.budget)?;
        self.statement(branch, js::Statement::Return(value))?;
        let condition = self.null_test(binding, true)?;
        self.statement(
            body,
            js::Statement::If {
                condition,
                yes: branch,
                no: None,
            },
        )
    }

    /// Mixed read reuses the original raw traversal after one handle dispatch.
    fn mixed_read_entry(
        &mut self,
        body: js::RegionId,
        value: js::BindingId,
        path: js::BindingId,
    ) -> Result<(), FormationError> {
        let scope = self.module.regions[body.index()].scope;
        let carrier = self.module.region_in(scope, self.budget)?;
        let packed = self.binding_slot(value, 0)?;
        self.assign_binding(carrier, value, packed)?;
        let bank = self.module.region_in(scope, self.budget)?;
        let copy = self.binding_slot(value, 0)?;
        let handle = self.reference(value)?;
        let copied = self.generated_schema_call(copy, &[handle])?;
        self.return_if_null(bank, path, Some(copied))?;
        let key = self.path_bank_key(path)?;
        let component = self.computed_slot(value, key)?;
        self.assign_binding(bank, value, component)?;
        let next = self.binding_slot(path, 2)?;
        self.assign_binding(bank, path, next)?;
        let condition = self.carrier_test(value)?;
        self.statement(
            body,
            js::Statement::If {
                condition,
                yes: carrier,
                no: Some(bank),
            },
        )
    }

    fn generate_mixed_check(
        &mut self,
        binding: js::BindingId,
        scope: js::ScopeId,
    ) -> Result<(), FormationError> {
        let body = self.module.region_in(scope, self.budget)?;
        let root = self.generated_binding(body, "root")?;
        let path = self.generated_binding(body, "path")?;
        // Reading the initialized handle occurs at the call site. Whole-place
        // checks never read its old payload, pack a bank, or coerce a leaf.
        self.return_if_null(body, path, None)?;
        let value = self.generated_binding(body, "value")?;
        self.statement(
            body,
            js::Statement::Let {
                binding: value,
                value: None,
            },
        )?;
        let child_scope = self.module.regions[body.index()].scope;
        let carrier = self.module.region_in(child_scope, self.budget)?;
        let packed = self.binding_slot(root, 0)?;
        self.assign_binding(carrier, value, packed)?;
        let bank = self.module.region_in(child_scope, self.budget)?;
        let next = self.generated_binding(bank, "next")?;
        let tail = self.binding_slot(path, 2)?;
        self.statement(
            bank,
            js::Statement::Let {
                binding: next,
                value: Some(tail),
            },
        )?;
        self.return_if_null(bank, next, None)?;
        let key = self.path_bank_key(path)?;
        let component = self.computed_slot(root, key)?;
        self.assign_binding(bank, value, component)?;
        let tail = self.reference(next)?;
        self.assign_binding(bank, path, tail)?;
        let condition = self.carrier_test(root)?;
        self.statement(
            body,
            js::Statement::If {
                condition,
                yes: carrier,
                no: Some(bank),
            },
        )?;
        let looping = self
            .module
            .region_in(self.module.regions[body.index()].scope, self.budget)?;
        // Every path parent is a private positional product or a stale nullish
        // value. Its own array length checks presence without touching the leaf.
        let object = self.reference(value)?;
        let property = js::Property::Named(self.text("length")?);
        let presence = self.expression(js::Expr::Member { object, property })?;
        self.statement(looping, js::Statement::Evaluate(presence))?;
        let next = self.generated_binding(looping, "next")?;
        let tail = self.binding_slot(path, 2)?;
        self.statement(
            looping,
            js::Statement::Let {
                binding: next,
                value: Some(tail),
            },
        )?;
        self.return_if_null(looping, next, None)?;
        let key = self.binding_slot(path, 0)?;
        let component = self.computed_slot(value, key)?;
        self.assign_binding(looping, value, component)?;
        let tail = self.reference(next)?;
        self.assign_binding(looping, path, tail)?;
        let condition = self.null_test(path, false)?;
        self.statement(
            body,
            js::Statement::Loop {
                condition: Some(condition),
                update: None,
                body: looping,
            },
        )?;
        self.generated_function(binding, body, &[root, path])
    }

    fn generate_mixed_write(
        &mut self,
        binding: js::BindingId,
        scope: js::ScopeId,
    ) -> Result<(), FormationError> {
        let body = self.module.region_in(scope, self.budget)?;
        let root = self.generated_binding(body, "root")?;
        let path = self.generated_binding(body, "path")?;
        let value = self.generated_binding(body, "value")?;
        let replace = self.reference_helper(1)?;
        let carrier = self
            .module
            .region_in(self.module.regions[body.index()].scope, self.budget)?;
        let old = self.binding_slot(root, 0)?;
        let location = self.reference(path)?;
        let incoming = self.reference(value)?;
        let replacement = self.generated_call(replace, &[old, location, incoming])?;
        let target = self.binding_slot(root, 0)?;
        let assigned = self.expression(js::Expr::Assign {
            target,
            value: replacement,
        })?;
        self.statement(carrier, js::Statement::Return(Some(assigned)))?;
        let condition = self.carrier_test(root)?;
        self.statement(
            body,
            js::Statement::If {
                condition,
                yes: carrier,
                no: None,
            },
        )?;
        let whole = self
            .module
            .region_in(self.module.regions[body.index()].scope, self.budget)?;
        let assign = self.binding_slot(root, 1)?;
        let handle = self.reference(root)?;
        let incoming = self.reference(value)?;
        let assigned = self.generated_schema_call(assign, &[handle, incoming])?;
        self.statement(whole, js::Statement::Return(Some(assigned)))?;
        let condition = self.null_test(path, true)?;
        self.statement(
            body,
            js::Statement::If {
                condition,
                yes: whole,
                no: None,
            },
        )?;
        let key = self.path_bank_key(path)?;
        let old = self.computed_slot(root, key)?;
        let tail = self.binding_slot(path, 2)?;
        let incoming = self.reference(value)?;
        let replacement = self.generated_call(replace, &[old, tail, incoming])?;
        let key = self.path_bank_key(path)?;
        let target = self.computed_slot(root, key)?;
        let assigned = self.expression(js::Expr::Assign {
            target,
            value: replacement,
        })?;
        self.statement(body, js::Statement::Evaluate(assigned))?;
        // The only client is a semantic Store, whose result is effect-only.
        // Preserve branch exits above; the final write falls through directly.
        self.generated_function(binding, body, &[root, path, value])
    }

    pub(super) fn product_reference_schema(
        &mut self,
        schema: usize,
    ) -> Result<(js::BindingId, js::BindingId), FormationError> {
        self.reference_mode()?;
        for index in 0..self.reference_plan.schemas.len() {
            self.work(1)?;
            let (found, copy, assign) = self.reference_plan.schemas[index];
            if found == schema {
                return Ok((copy, assign));
            }
        }
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let copy = self.generated_binding(root, "ref_copy")?;
        let body = self.module.region_in(scope, self.budget)?;
        let bank = self.generated_binding(body, "bank")?;
        let count = self.program.structs[schema].fields.len();
        let mut values = self.budget.vector(AllocationClass::Retained, count)?;
        for slot in 0..count {
            self.work(1)?;
            let value = self.binding_slot(bank, slot + 2)?;
            self.append(&mut values, value)?;
        }
        let result = self.product(values)?;
        self.statement(body, js::Statement::Return(Some(result)))?;
        self.generated_function(copy, body, &[bank])?;
        let assign = self.generated_binding(root, "ref_assign")?;
        let body = self.module.region_in(scope, self.budget)?;
        let bank = self.generated_binding(body, "bank")?;
        let packed = self.generated_binding(body, "packed")?;
        let mut staged = self.budget.vector(AllocationClass::Scratch, count)?;
        for slot in 0..count {
            self.work(1)?;
            let component = self.generated_binding(body, "field")?;
            let value = self.binding_slot(packed, slot)?;
            self.statement(
                body,
                js::Statement::Let {
                    binding: component,
                    value: Some(value),
                },
            )?;
            self.budget
                .push(AllocationClass::Scratch, &mut staged, component)?;
        }
        for (slot, component) in staged.iter().copied().enumerate() {
            self.work(1)?;
            let target = self.binding_slot(bank, slot + 2)?;
            let value = self.reference(component)?;
            let assigned = self.expression(js::Expr::Assign { target, value })?;
            self.statement(body, js::Statement::Evaluate(assigned))?;
        }
        self.drop_scratch(staged)?;
        let result = self.reference(packed)?;
        self.statement(body, js::Statement::Return(Some(result)))?;
        self.generated_function(assign, body, &[bank, packed])?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.reference_plan.schemas,
            (schema, copy, assign),
        )?;
        Ok((copy, assign))
    }

    /// Finalized physical demand chooses carriers per owning activation.
    /// Complete source exposure still owns semantic facts and proof eligibility.
    pub(super) fn addressed_cell(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<bool, FormationError> {
        self.work(1)?;
        if is_reference(self.program, cell)
            || self.program.cells[cell.index()].binding == CellBinding::Foreign
        {
            return Ok(false);
        }
        let demand = self.demand;
        let mut meter = Meter(self.budget);
        Ok(
            demand.cell_location_visited(context, cell, |n| meter.work(n))?
                == Some(super::super::demand::LocationDemand::Shared),
        )
    }

    pub(super) fn validate_reference_context(
        &mut self,
        context: ContextId,
    ) -> Result<(), FormationError> {
        let data = self.data(context);
        let mut has_references = false;
        for &cell in &data.parameters {
            self.work(1)?;
            has_references |= is_reference(self.program, cell);
        }
        if !has_references {
            return Ok(());
        }
        if self.contract.execution != JavaScriptExecution::Module {
            return Err(self.error(
                Span::default(),
                "reference callable requires strict module execution",
            ));
        }
        if matches!(
            self.demand.context(context).kind,
            ContextKind::Inline { .. }
        ) {
            // The selected helper already owns complete reference/body/frame
            // evidence. Do not repeat body-wide callable discovery per occurrence.
            return Ok(());
        }
        let uses = self.uses.ok_or_else(|| {
            self.error(
                Span::default(),
                "reference callable requires complete use index",
            )
        })?;
        let semantic = self.semantic(context);
        let mut meter = Meter(self.budget);
        let proof = CallableInputs::for_body_published(
            self.program,
            uses,
            semantic,
            CallObservations::from_execution(self.contract.execution),
            &mut meter,
        )?;
        let InputOutcome::Complete(proof) = proof else {
            return Err(self.error(
                Span::default(),
                "reference callable requires a complete private interface",
            ));
        };
        proof.discard(&mut meter)?;
        Ok(())
    }

    fn generated_binding(
        &mut self,
        region: js::RegionId,
        name: &str,
    ) -> Result<js::BindingId, FormationError> {
        let spelling = self.text(name)?;
        Ok(self.module.binding_in(
            js::Binding {
                source_symbol: None,
                scope: self.module.regions[region.index()].scope,
                spelling,
                pinned: false,
            },
            self.budget,
        )?)
    }
    fn prefix_reference(&mut self, statement: js::Statement) -> Result<(), FormationError> {
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.reference_plan.prefix,
            statement,
        )?;
        Ok(())
    }
    fn generated_call(
        &mut self,
        binding: js::BindingId,
        arguments: &[js::ExprId],
    ) -> Result<js::ExprId, FormationError> {
        let callee = self.reference(binding)?;
        let arguments = self
            .budget
            .copy_slice(AllocationClass::Retained, arguments)?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })
    }
    fn generated_array(&mut self, values: &[js::ExprId]) -> Result<js::ExprId, FormationError> {
        let values = self.budget.copy_slice(AllocationClass::Retained, values)?;
        self.expression(js::Expr::Array(values))
    }
    fn binding_slot(
        &mut self,
        binding: js::BindingId,
        slot: usize,
    ) -> Result<js::ExprId, FormationError> {
        let object = self.reference(binding)?;
        self.slot(object, slot)
    }
    fn null_test(
        &mut self,
        binding: js::BindingId,
        equal: bool,
    ) -> Result<js::ExprId, FormationError> {
        let left = self.reference(binding)?;
        let right = self.literal(js::Literal::Null)?;
        self.expression(js::Expr::Binary {
            op: if equal {
                js::Binary::StrictEqual
            } else {
                js::Binary::StrictNotEqual
            },
            left,
            right,
        })
    }
    fn assign_binding(
        &mut self,
        region: js::RegionId,
        binding: js::BindingId,
        value: js::ExprId,
    ) -> Result<(), FormationError> {
        let target = self.reference(binding)?;
        let assignment = self.expression(js::Expr::Assign { target, value })?;
        self.statement(region, js::Statement::Evaluate(assignment))
    }
    fn generated_function(
        &mut self,
        binding: js::BindingId,
        body: js::RegionId,
        parameters: &[js::BindingId],
    ) -> Result<(), FormationError> {
        let parameters = self
            .budget
            .copy_slice(AllocationClass::Retained, parameters)?;
        let id = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters,
                body,
                arrow: false,
                name: js::FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: crate::structured_js::Suspension::None,
            },
        )?;
        self.prefix_reference(js::Statement::Function {
            binding,
            function: id,
        })
    }

    // Each demanded fixed algorithm is generated once in the admitted Module.
    fn reference_helper(&mut self, kind: usize) -> Result<js::BindingId, FormationError> {
        let mode = self.reference_mode()?;
        if let Some(binding) = self.reference_plan.helpers[kind] {
            return Ok(binding);
        }
        let root = self.module.root;
        let binding = self.generated_binding(
            root,
            [
                "ref_read",
                "ref_replace",
                "ref_append",
                "ref_check",
                "ref_write",
            ][kind],
        )?;
        self.reference_plan.helpers[kind] = Some(binding);
        let scope = self.module.regions[root.index()].scope;
        match kind {
            0 => {
                // read(value,path) walks immutable metadata without converting payloads.
                let body = self.module.region_in(scope, self.budget)?;
                let value = self.generated_binding(body, "value")?;
                let path = self.generated_binding(body, "path")?;
                if mode == ReferenceMode::Mixed {
                    self.mixed_read_entry(body, value, path)?;
                }
                let looping = self
                    .module
                    .region_in(self.module.regions[body.index()].scope, self.budget)?;
                let object = self.reference(value)?;
                let key = self.binding_slot(path, 0)?;
                let next_value = self.expression(js::Expr::Member {
                    object,
                    property: js::Property::Computed(key),
                })?;
                self.assign_binding(looping, value, next_value)?;
                let next = self.binding_slot(path, 2)?;
                self.assign_binding(looping, path, next)?;
                let condition = self.null_test(path, false)?;
                self.statement(
                    body,
                    js::Statement::Loop {
                        condition: Some(condition),
                        update: None,
                        body: looping,
                    },
                )?;
                let result = self.reference(value)?;
                self.statement(body, js::Statement::Return(Some(result)))?;
                self.generated_function(binding, body, &[value, path])?;
            }
            1 => {
                // replace(value,path,new) checks the current path before rebuilding.
                let body = self.module.region_in(scope, self.budget)?;
                let value = self.generated_binding(body, "value")?;
                let path = self.generated_binding(body, "path")?;
                let replacement = self.generated_binding(body, "replacement")?;
                let base = self
                    .module
                    .region_in(self.module.regions[body.index()].scope, self.budget)?;
                let result = self.reference(replacement)?;
                self.statement(base, js::Statement::Return(Some(result)))?;
                let condition = self.null_test(path, true)?;
                self.statement(
                    body,
                    js::Statement::If {
                        condition,
                        yes: base,
                        no: None,
                    },
                )?;
                let object = self.reference(value)?;
                let key = self.binding_slot(path, 0)?;
                let child = self.expression(js::Expr::Member {
                    object,
                    property: js::Property::Computed(key),
                })?;
                let next = self.binding_slot(path, 2)?;
                let supplied = self.reference(replacement)?;
                let updated = self.generated_call(binding, &[child, next, supplied])?;
                let child_binding = self.generated_binding(body, "child")?;
                self.statement(
                    body,
                    js::Statement::Let {
                        binding: child_binding,
                        value: Some(updated),
                    },
                )?;
                let callee = self.binding_slot(path, 1)?;
                let old = self.reference(value)?;
                let slot = self.binding_slot(path, 0)?;
                let child = self.reference(child_binding)?;
                let arguments = self
                    .budget
                    .copy_slice(AllocationClass::Retained, &[old, slot, child])?;
                let result = self.expression(js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Value,
                })?;
                self.statement(body, js::Statement::Return(Some(result)))?;
                self.generated_function(binding, body, &[value, path, replacement])?;
            }
            2 => {
                // append is only needed when forwarding a deeper reference path.
                let body = self.module.region_in(scope, self.budget)?;
                let path = self.generated_binding(body, "path")?;
                let suffix = self.generated_binding(body, "suffix")?;
                let base = self
                    .module
                    .region_in(self.module.regions[body.index()].scope, self.budget)?;
                let result = self.reference(suffix)?;
                self.statement(base, js::Statement::Return(Some(result)))?;
                let condition = self.null_test(path, true)?;
                self.statement(
                    body,
                    js::Statement::If {
                        condition,
                        yes: base,
                        no: None,
                    },
                )?;
                let slot = self.binding_slot(path, 0)?;
                let rebuild = self.binding_slot(path, 1)?;
                let tail = self.binding_slot(path, 2)?;
                let suffix_value = self.reference(suffix)?;
                let tail = self.generated_call(binding, &[tail, suffix_value])?;
                let result = self.generated_array(&[slot, rebuild, tail])?;
                self.statement(body, js::Statement::Return(Some(result)))?;
                self.generated_function(binding, body, &[path, suffix])?;
            }
            3 => {
                debug_assert_eq!(mode, ReferenceMode::Mixed);
                self.generate_mixed_check(binding, scope)?;
            }
            4 => self.generate_mixed_write(binding, scope)?,
            _ => unreachable!("fixed reference recipe"),
        }
        Ok(binding)
    }

    fn schema_rebuild(&mut self, schema: usize) -> Result<js::BindingId, FormationError> {
        for index in 0..self.reference_plan.rebuilds.len() {
            self.work(1)?;
            if self.reference_plan.rebuilds[index].0 == schema {
                return Ok(self.reference_plan.rebuilds[index].1);
            }
        }
        let root = self.module.root;
        let binding = self.generated_binding(root, "ref_product")?;
        let body = self
            .module
            .region_in(self.module.regions[root.index()].scope, self.budget)?;
        let old = self.generated_binding(body, "old")?;
        let selected = self.generated_binding(body, "slot")?;
        let replacement = self.generated_binding(body, "replacement")?;
        let count = self.program.structs[schema].fields.len();
        let mut values = self.budget.vector(AllocationClass::Retained, count)?;
        for slot in 0..count {
            self.work(1)?;
            let left = self.reference(selected)?;
            let right = self.literal(js::Literal::Number(slot as f64))?;
            let condition = self.expression(js::Expr::Binary {
                op: js::Binary::StrictEqual,
                left,
                right,
            })?;
            let yes = self.reference(replacement)?;
            let no = self.binding_slot(old, slot)?;
            let value = self.expression(js::Expr::Conditional { condition, yes, no })?;
            self.append(&mut values, value)?;
        }
        let value = self.product(values)?;
        self.statement(body, js::Statement::Return(Some(value)))?;
        self.generated_function(binding, body, &[old, selected, replacement])?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.reference_plan.rebuilds,
            (schema, binding),
        )?;
        Ok(binding)
    }

    pub(super) fn initialize_reference_context(
        &mut self,
        context: ContextId,
    ) -> Result<(), FormationError> {
        let data = self.data(context);
        let region = self.plan(context).regions[data.entry.index()];
        let mut parameter_paths = Vec::new();
        if !self.demand.context(context).kind.is_inline() {
            for &cell in &data.parameters {
                self.work(1)?;
                if is_reference(self.program, cell) {
                    let path = self.generated_binding(region, "ref_path")?;
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut parameter_paths,
                        (cell, path),
                    )?;
                }
            }
        }
        let mut prepared = Vec::new();
        for (index, call) in data.calls.iter().enumerate() {
            self.work(1)?;
            let call_id = CallId::from_index(index).unwrap();
            let invocation = self.demand.call_invocation(context, call_id);
            let shared = self.demand.needs_operation(context, invocation)
                && !self
                    .demand
                    .child(context, invocation)
                    .is_some_and(|child| self.demand.context(child).kind.is_inline());
            for (position, argument) in data.arguments(call.arguments).unwrap().iter().enumerate() {
                self.work(1)?;
                if !matches!(argument, CallArgument::Reference(_)) {
                    continue;
                }
                let location = if shared {
                    let root = self.generated_binding(region, "ref_root")?;
                    let path = self.generated_binding(region, "ref_place")?;
                    self.statement(
                        region,
                        js::Statement::Let {
                            binding: root,
                            value: None,
                        },
                    )?;
                    self.statement(
                        region,
                        js::Statement::Let {
                            binding: path,
                            value: None,
                        },
                    )?;
                    PreparedLocation::Shared { root, path }
                } else {
                    PreparedLocation::Local
                };
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut prepared,
                    PreparedReference {
                        call: call_id,
                        position: position as u32,
                        location,
                    },
                )?;
            }
        }
        let plan = &mut self.contexts[context.index()].as_mut().unwrap().plan;
        plan.reference_parameters = parameter_paths;
        plan.prepared_references = prepared;
        Ok(())
    }

    pub(super) fn reference_parameter_path(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<js::BindingId, FormationError> {
        // References cannot escape their owning callable activation. A source
        // copy into an ordinary captured cell is not a reference capture.
        if self.program.cells[cell.index()].owner != self.semantic(context) {
            return Err(self.error(Span::default(), "captured reference parameter"));
        }
        for index in 0..self.plan(context).reference_parameters.len() {
            self.work(1)?;
            let (found, path) = self.plan(context).reference_parameters[index];
            if found == cell {
                return Ok(path);
            }
        }
        Err(self.error(Span::default(), "missing reference parameter path"))
    }
    pub(super) fn read_reference(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<js::ExprId, FormationError> {
        if let Some(actual) = self.inline_reference_actual(context, cell)? {
            let CallArgument::Reference(place) = *actual.argument else {
                return Err(self.error(Span::default(), "inline reference actual is a value"));
            };
            return self.value_field(actual.caller, place);
        }
        let carrier = self.cell_binding(context, cell)?;
        let path = self.reference_parameter_path(context, cell)?;
        let helper = self.reference_helper(0)?;
        let value = if self.reference_mode()? == ReferenceMode::Mixed {
            self.reference(carrier)?
        } else {
            self.binding_slot(carrier, 0)?
        };
        let path = self.reference(path)?;
        self.generated_call(helper, &[value, path])
    }
    pub(super) fn reference_integer_load(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<js::ExprId, FormationError> {
        if let Some(actual) = self.inline_reference_actual(context, cell)? {
            let CallArgument::Reference(place) = *actual.argument else {
                return Err(self.error(Span::default(), "inline reference actual is a value"));
            };
            self.work(1)?;
            return match self.data(actual.caller).places[place.index()] {
                Place::Cell(cell) if is_reference(self.program, cell) => {
                    self.reference_integer_load(actual.caller, cell)
                }
                Place::Cell(cell) => self.cell(actual.caller, cell),
                Place::Field { .. } => {
                    let value = self.value_field(actual.caller, place)?;
                    self.expression(js::Expr::ToInt32(value))
                }
                _ => Err(self.error(Span::default(), "inline reference temporary or host base")),
            };
        }
        let path = self.reference_parameter_path(context, cell)?;
        let condition = self.null_test(path, true)?;
        let yes = self.read_reference(context, cell)?;
        let no = self.read_reference(context, cell)?;
        let no = self.expression(js::Expr::ToInt32(no))?;
        self.expression(js::Expr::Conditional { condition, yes, no })
    }
    pub(super) fn store_cell(
        &mut self,
        context: ContextId,
        cell: CellId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if !is_reference(self.program, cell) {
            self.product_lookup()?;
            if self.demand.product_for_cell(cell).is_some() {
                return self.store_product_cell(context, cell, value);
            }
            let target = self.cell(context, cell)?;
            return self.expression(js::Expr::Assign { target, value });
        }
        if let Some(actual) = self.inline_reference_actual(context, cell)? {
            let CallArgument::Reference(place) = *actual.argument else {
                return Err(self.error(Span::default(), "inline reference actual is a value"));
            };
            return self.store_field_expression(actual.caller, place, value, Span::default());
        }
        let carrier = self.cell_binding(context, cell)?;
        let path = self.reference_parameter_path(context, cell)?;
        if self.reference_mode()? == ReferenceMode::Mixed {
            let helper = self.reference_helper(4)?;
            let root = self.reference(carrier)?;
            let path = self.reference(path)?;
            return self.generated_call(helper, &[root, path, value]);
        }
        let helper = self.reference_helper(1)?;
        let old = self.binding_slot(carrier, 0)?;
        let path = self.reference(path)?;
        let replacement = self.generated_call(helper, &[old, path, value])?;
        let target = self.binding_slot(carrier, 0)?;
        self.expression(js::Expr::Assign {
            target,
            value: replacement,
        })
    }
    pub(super) fn carrier_value(
        &mut self,
        context: ContextId,
        cell: CellId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if self.addressed_cell(context, cell)? {
            self.generated_array(&[value])
        } else {
            Ok(value)
        }
    }

    fn static_reference_path(
        &mut self,
        context: ContextId,
        place: PlaceId,
    ) -> Result<js::ExprId, FormationError> {
        if !matches!(
            self.data(context).places[place.index()],
            Place::Field { .. }
        ) {
            return self.literal(js::Literal::Null);
        }
        let unit = self.semantic(context);
        for index in 0..self.reference_plan.paths.len() {
            self.work(1)?;
            let (found_unit, found_place, binding) = self.reference_plan.paths[index];
            if found_unit == unit && found_place == place {
                return self.reference(binding);
            }
        }
        let demand = self.demand;
        let mut meter = Meter(self.budget);
        let needs_rebuild = demand.writes_incoming_references_visited(|n| meter.work(n))?;
        // Keep the original eager schema/path order. The completed physical
        // demand cursor already knows whether any surviving write can invoke
        // the private path rebuild slot; reading/checking/appending cannot.
        let (_, path) = self.field_path(context, place)?;
        let mut result = self.literal(js::Literal::Null)?;
        for recipe in &path {
            self.work(1)?;
            let slot = self.literal(js::Literal::Number(recipe.slot as f64))?;
            let rebuild = if needs_rebuild {
                let rebuild = self.schema_rebuild(recipe.schema)?;
                self.reference(rebuild)?
            } else {
                self.literal(js::Literal::Null)?
            };
            result = self.generated_array(&[slot, rebuild, result])?;
        }
        self.drop_scratch(path)?;
        let binding = self.generated_binding(self.module.root, "ref_path")?;
        self.prefix_reference(js::Statement::Let {
            binding,
            value: Some(result),
        })?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.reference_plan.paths,
            (unit, place, binding),
        )?;
        self.reference(binding)
    }
    fn prepared_location(
        &mut self,
        context: ContextId,
        call: CallId,
        position: u32,
    ) -> Result<PreparedLocation, FormationError> {
        for index in 0..self.plan(context).prepared_references.len() {
            self.work(1)?;
            let row = self.plan(context).prepared_references[index];
            if row.call == call && row.position == position {
                return Ok(row.location);
            }
        }
        Err(self.error(Span::default(), "missing prepared reference storage"))
    }
    pub(super) fn prepared_reference(
        &mut self,
        context: ContextId,
        call: CallId,
        position: u32,
    ) -> Result<(js::BindingId, js::BindingId), FormationError> {
        match self.prepared_location(context, call, position)? {
            PreparedLocation::Shared { root, path } => Ok((root, path)),
            PreparedLocation::Local => Err(self.error(
                Span::default(),
                "local preparation at surviving shared call",
            )),
        }
    }

    /// Original path suffixes retain their UnitId/PlaceId cache identity. A
    /// named incoming formal supplies its already captured runtime prefix.
    /// Current helper certificates exclude nested calls, so an inline incoming
    /// formal cannot prepare another argument; that extension stays explicit.
    fn reference_location(
        &mut self,
        context: ContextId,
        place: PlaceId,
    ) -> Result<(js::ExprId, js::ExprId), FormationError> {
        let data = self.data(context);
        let mut root = place;
        loop {
            self.work(1)?;
            match data.places[root.index()] {
                Place::Field { base, .. } => root = base,
                _ => break,
            }
        }
        let Place::Cell(cell) = data.places[root.index()] else {
            return Err(self.error(Span::default(), "reference temporary or host base"));
        };
        if self.inline_reference_actual(context, cell)?.is_some() {
            return Err(self.error(
                Span::default(),
                "inline forwarding preparation requires a helper-call certificate",
            ));
        }
        let binding = if let Some(handle) = self.product_cell_handle(context, cell)? {
            handle
        } else {
            self.product_lookup()?;
            if self.demand.product_for_cell(cell).is_some() {
                return Err(self.error(
                    Span::default(),
                    "lexical product location requires a discharged check",
                ));
            }
            if !is_reference(self.program, cell) && !self.addressed_cell(context, cell)? {
                return Err(self.error(
                    Span::default(),
                    "shared reference root lacks location demand",
                ));
            }
            self.cell_binding(context, cell)?
        };
        let carrier = self.reference(binding)?;
        let path = if is_reference(self.program, cell) {
            let incoming = self.reference_parameter_path(context, cell)?;
            if root == place {
                self.reference(incoming)?
            } else {
                let helper = self.reference_helper(2)?;
                let prefix = self.reference(incoming)?;
                let suffix = self.static_reference_path(context, place)?;
                self.generated_call(helper, &[prefix, suffix])?
            }
        } else {
            self.static_reference_path(context, place)?
        };
        Ok((carrier, path))
    }

    pub(super) fn prepare_reference(
        &mut self,
        context: ContextId,
        call: CallId,
        position: u32,
    ) -> Result<js::ExprId, FormationError> {
        let data = self.data(context);
        let CallArgument::Reference(place) =
            data.arguments(data.calls[call.index()].arguments).unwrap()[position as usize]
        else {
            return Err(self.error(Span::default(), "reference preparation argument"));
        };
        let proved = self.reference_check_proved(
            context,
            place,
            LocationCheck::Preparation { call, position },
        )?;
        match self.prepared_location(context, call, position)? {
            PreparedLocation::Local if proved => self.literal(js::Literal::Number(0.0)),
            PreparedLocation::Local => self.check_reference_place(context, place),
            PreparedLocation::Shared {
                root: prepared_root,
                path: prepared_path,
            } => {
                let (root, path) = self.reference_location(context, place)?;
                let target = self.reference(prepared_root)?;
                let root_assignment = self.expression(js::Expr::Assign {
                    target,
                    value: root,
                })?;
                let target = self.reference(prepared_path)?;
                let path_assignment = self.expression(js::Expr::Assign {
                    target,
                    value: path,
                })?;
                let mut sequence = self.budget.vector(AllocationClass::Retained, 3)?;
                self.append(&mut sequence, root_assignment)?;
                self.append(&mut sequence, path_assignment)?;
                if !proved {
                    let checked = if self.reference_mode()? == ReferenceMode::PackedOnly {
                        // The private packed recipe already has an inert raw
                        // projection. Reuse it without the leaf's load recipe.
                        self.value_field(context, place)?
                    } else {
                        let helper = self.reference_helper(3)?;
                        let root = self.reference(prepared_root)?;
                        let path = self.reference(prepared_path)?;
                        self.generated_call(helper, &[root, path])?
                    };
                    self.append(&mut sequence, checked)?;
                }
                self.expression(js::Expr::Sequence(sequence))
            }
        }
    }

    /// Check parent presence using the selected storage recipe. Raw reads of
    /// private packed slots are inert; mixed banks need no snapshot or leaf
    /// demand merely to check a location. Neither route applies coercion.
    pub(super) fn check_reference_place(
        &mut self,
        context: ContextId,
        place: PlaceId,
    ) -> Result<js::ExprId, FormationError> {
        if self.reference_mode()? == ReferenceMode::PackedOnly {
            return self.value_field(context, place);
        }
        let original_context = context;
        let (context, root, path) = self.resolved_field_path(context, place)?;
        if let Place::Cell(cell) = self.data(context).places[root.index()] {
            if is_reference(self.program, cell) {
                self.drop_scratch(path)?;
                if context != original_context {
                    return Err(self.error(
                        Span::default(),
                        "inline reference check requires its scoped witness",
                    ));
                }
                let (root, path) = self.reference_location(context, place)?;
                let helper = self.reference_helper(3)?;
                return self.generated_call(helper, &[root, path]);
            }
        }
        // The last (leaf-most) projection is excluded: its old value is not
        // part of place preparation, especially when that value is opaque.
        if path.is_empty() {
            self.drop_scratch(path)?;
            return self.place(context, root);
        }
        let component = if path.len() > 1 {
            self.product_place_component(context, root, path.last().unwrap().slot as u32)?
        } else {
            None
        };
        let (mut parent, remaining) = match component {
            Some(component) => (component, &path[1..path.len() - 1]),
            None => (self.place(context, root)?, &path[1..]),
        };
        for recipe in remaining.iter().rev() {
            self.work(1)?;
            parent = self.slot(parent, recipe.slot)?;
        }
        self.drop_scratch(path)?;
        let property = js::Property::Named(self.text("length")?);
        self.expression(js::Expr::Member {
            object: parent,
            property,
        })
    }
    pub(super) fn finish_reference_prefix(&mut self) -> Result<(), FormationError> {
        let mut prefix = std::mem::take(&mut self.reference_plan.prefix);
        if !prefix.is_empty() {
            let count = prefix.len();
            let root = self.module.root;
            self.work(self.module.regions[root.index()].statements.len())?;
            let statements = &mut self.module.regions[root.index()].statements;
            self.budget
                .reserve_vec(AllocationClass::Retained, statements, count)?;
            statements.extend(prefix.drain(..));
            statements.rotate_right(count);
            let module = self.current_module;
            self.prepend_root_owners(root, count, module)?;
        }
        self.drop_scratch(prefix)
    }
}
