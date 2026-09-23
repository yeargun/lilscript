//! Private product transport reuses immutable source snapshots and the existing
//! physical context. Incoming formals and mutable local storage are independent
//! choices; source signatures and logical argument positions remain unchanged.
use super::*;
use crate::semantic_program::function_layout::ProductTransport;

#[derive(Default)]
pub(super) struct Parameters {
    // Sorted source positions; only parameters requiring a physical adapter.
    entries: Vec<(u32, Incoming)>,
}
enum Incoming {
    Packed(js::BindingId),
    Fields(Vec<js::BindingId>),
}

impl Formation<'_, '_, '_, '_, '_> {
    fn function_lookup(&mut self) -> Result<(), FormationError> {
        let work = self.demand.function_lookup_work();
        if work != 0 {
            self.work(work)?;
        }
        Ok(())
    }
    fn product_parameter_index(
        &mut self,
        context: ContextId,
        position: u32,
    ) -> Result<Option<usize>, FormationError> {
        let count = self.contexts[context.index()]
            .as_ref()
            .unwrap()
            .product_parameters
            .entries
            .len();
        if count == 0 {
            return Ok(None);
        }
        self.work((usize::BITS - count.leading_zeros()) as usize)?;
        Ok(self.contexts[context.index()]
            .as_ref()
            .unwrap()
            .product_parameters
            .entries
            .binary_search_by_key(&position, |entry| entry.0)
            .ok())
    }
    fn incoming_product_binding(
        &mut self,
        context: ContextId,
        index: usize,
        slot: usize,
    ) -> Result<js::BindingId, FormationError> {
        self.work(1)?;
        match &self.contexts[context.index()]
            .as_ref()
            .unwrap()
            .product_parameters
            .entries[index]
            .1
        {
            Incoming::Packed(binding) if slot == 0 => Ok(*binding),
            Incoming::Fields(fields) => fields.get(slot).copied().ok_or_else(|| {
                self.error(Span::default(), "product formal outside physical width")
            }),
            _ => Err(self.error(Span::default(), "invalid packed product formal slot")),
        }
    }
    fn new_product_formal(
        &mut self,
        context: ContextId,
        position: usize,
        slot: usize,
        region: js::RegionId,
    ) -> Result<js::BindingId, FormationError> {
        let spelling = self.format(format_args!(
            "incoming_{}_{}_{}",
            context.index(),
            position,
            slot
        ))?;
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
    pub(super) fn plan_product_parameters(
        &mut self,
        context: ContextId,
    ) -> Result<(), FormationError> {
        if self.demand.context(context).kind.is_inline() {
            return Ok(());
        }
        let data = self.data(context);
        self.function_lookup()?;
        let layout = self.demand.function_layout(self.semantic(context));
        let mut parameters = Parameters::default();
        for (position, &cell) in data.parameters.iter().enumerate() {
            self.work(1)?;
            self.product_lookup()?;
            self.function_lookup()?;
            let local = self.demand.product_for_cell(cell);
            let fields = layout.is_some_and(|layout| {
                layout.transport(position as u32) == ProductTransport::Fields
            });
            if !fields && local.is_none() {
                continue;
            }
            let region =
                self.plan(context).regions[self.program.cells[cell.index()].region.index()];
            let incoming = if fields {
                self.function_lookup()?;
                let schema = layout.unwrap().parameter(position as u32).unwrap().schema;
                self.work(1)?;
                let definition = self
                    .program
                    .structs
                    .get(schema.index())
                    .filter(|definition| definition.identity == schema)
                    .ok_or_else(|| {
                        self.error(Span::default(), "missing product parameter schema")
                    })?;
                let mut bindings = self
                    .budget
                    .vector(AllocationClass::Scratch, definition.fields.len())?;
                for slot in 0..definition.fields.len() {
                    self.work(1)?;
                    let binding = self.new_product_formal(context, position, slot, region)?;
                    self.budget
                        .push(AllocationClass::Scratch, &mut bindings, binding)?;
                }
                Incoming::Fields(bindings)
            } else {
                Incoming::Packed(self.new_product_formal(context, position, 0, region)?)
            };
            self.budget.push(
                AllocationClass::Scratch,
                &mut parameters.entries,
                (position as u32, incoming),
            )?;
        }
        self.contexts[context.index()]
            .as_mut()
            .unwrap()
            .product_parameters = parameters;
        Ok(())
    }
    /// Fields formals already are independent mutable storage in each real
    /// function invocation. Unused formals still retain the fixed private ABI.
    pub(super) fn product_formal_slot(
        &mut self,
        context: ContextId,
        cell: CellId,
        slot: usize,
    ) -> Result<Option<js::BindingId>, FormationError> {
        let CellBinding::Parameter(position) = self.program.cells[cell.index()].binding else {
            return Ok(None);
        };
        let Some(index) = self.product_parameter_index(context, position)? else {
            return Ok(None);
        };
        if matches!(
            self.contexts[context.index()]
                .as_ref()
                .unwrap()
                .product_parameters
                .entries[index]
                .1,
            Incoming::Fields(_)
        ) {
            Ok(Some(self.incoming_product_binding(context, index, slot)?))
        } else {
            Ok(None)
        }
    }
    /// This prefix precedes address-carrier setup and the original body. It
    /// performs raw transport only, never source Field<Int> normalization.
    pub(super) fn product_parameter_prefix(
        &mut self,
        context: ContextId,
    ) -> Result<Vec<js::Statement>, FormationError> {
        let mut prefix = Vec::new();
        let count = self.contexts[context.index()]
            .as_ref()
            .unwrap()
            .product_parameters
            .entries
            .len();
        for index in 0..count {
            self.work(1)?;
            let (position, incoming) = &self.contexts[context.index()]
                .as_ref()
                .unwrap()
                .product_parameters
                .entries[index];
            let cell = self.data(context).parameters[*position as usize];
            let width = match incoming {
                Incoming::Packed(_) => None,
                Incoming::Fields(fields) => Some(fields.len()),
            };
            self.product_lookup()?;
            if let Some(family) = self.demand.product_for_cell(cell) {
                if let Some(binding) = self.product_cell_handle(context, cell)? {
                    let count = self.demand.products()[family].fields().len();
                    let mut components = self.budget.vector(AllocationClass::Retained, count)?;
                    for slot in 0..count {
                        self.work(1)?;
                        let incoming = self.incoming_product_binding(
                            context,
                            index,
                            if width.is_some() { slot } else { 0 },
                        )?;
                        let value = self.reference(incoming)?;
                        let value = if width.is_some() {
                            value
                        } else {
                            self.slot(value, slot)?
                        };
                        self.append(&mut components, value)?;
                    }
                    let value = self.product_bank_value(cell, components)?;
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut prefix,
                        js::Statement::Let {
                            binding,
                            value: Some(value),
                        },
                    )?;
                    continue;
                }
                if width.is_some() {
                    continue; // Fields→lexical fields uses independent actual formals.
                }
                let incoming = self.incoming_product_binding(context, index, 0)?;
                for slot in 0..self.demand.products()[family].fields().len() {
                    self.work(1)?;
                    let Some(products::StorageLocation::Binding(binding)) =
                        self.product_cell_slot(context, cell, slot as u32)?
                    else {
                        continue;
                    };
                    let source = self.reference(incoming)?;
                    let value = self.slot(source, slot)?;
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut prefix,
                        js::Statement::Let {
                            binding,
                            value: Some(value),
                        },
                    )?;
                }
            } else {
                let width = width
                    .ok_or_else(|| self.error(Span::default(), "unused packed product adapter"))?;
                let binding = self.cell_binding(context, cell)?;
                let mut components = self.budget.vector(AllocationClass::Retained, width)?;
                for slot in 0..width {
                    let incoming = self.incoming_product_binding(context, index, slot)?;
                    let value = self.reference(incoming)?;
                    self.append(&mut components, value)?;
                }
                let value = self.product(components)?;
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut prefix,
                    js::Statement::Let {
                        binding,
                        value: Some(value),
                    },
                )?;
            }
        }
        Ok(prefix)
    }
    pub(super) fn physical_parameters(
        &mut self,
        context: ContextId,
    ) -> Result<Vec<js::BindingId>, FormationError> {
        let cells = &self.data(context).parameters;
        let mut parameters = self.budget.vector(AllocationClass::Retained, cells.len())?;
        for (position, &cell) in cells.iter().enumerate() {
            self.work(1)?;
            if let Some(index) = self.product_parameter_index(context, position as u32)? {
                let width = match &self.contexts[context.index()]
                    .as_ref()
                    .unwrap()
                    .product_parameters
                    .entries[index]
                    .1
                {
                    Incoming::Packed(_) => 1,
                    Incoming::Fields(fields) => fields.len(),
                };
                for slot in 0..width {
                    let binding = self.incoming_product_binding(context, index, slot)?;
                    self.append(&mut parameters, binding)?;
                }
            } else {
                let binding = self.cell_binding(context, cell)?;
                self.append(&mut parameters, binding)?;
                // A typed caller always passes a value of this type.
                let defined = matches!(
                    self.program.types[self.program.cells[cell.index()].ty.index()],
                    Type::Int
                        | Type::Float
                        | Type::Bool
                        | Type::String
                        | Type::Enum(_)
                        | Type::Array(_)
                        | Type::Record(_)
                        | Type::Map(_, _)
                        | Type::Set(_)
                        | Type::Regex
                        | Type::Struct(_)
                        | Type::Class(_)
                        | Type::StructInstance { .. }
                        | Type::ClassInstance { .. }
                        | Type::Function(_)
                ) && !references::is_reference(self.program, cell);
                if defined {
                    self.budget.push(
                        AllocationClass::Retained,
                        &mut self.module.defined_parameters,
                        binding,
                    )?;
                }
                if references::is_reference(self.program, cell) {
                    let path = self.reference_parameter_path(context, cell)?;
                    self.append(&mut parameters, path)?;
                }
            }
        }
        Ok(parameters)
    }
    pub(super) fn packed_product_snapshot(
        &mut self,
        context: ContextId,
        value: ValueId,
    ) -> Result<js::ExprId, FormationError> {
        self.product_lookup()?;
        let family = self
            .demand
            .product_for_value(self.semantic(context), value)
            .ok_or_else(|| self.error(Span::default(), "missing packed snapshot family"))?;
        let count = self.demand.products()[family].fields().len();
        let mut fields = self.budget.vector(AllocationClass::Retained, count)?;
        for slot in 0..count {
            self.work(1)?;
            let component = self.product_snapshot_component(context, value, slot as u32)?;
            self.append(&mut fields, component)?;
        }
        self.product(fields)
    }
    /// The ordinary packed producer must be captured by the common effective
    /// use/placement owner before this recipe consumes multiple projections.
    pub(super) fn actual_product_component(
        &mut self,
        caller: ContextId,
        value: ValueId,
        slot: usize,
    ) -> Result<js::ExprId, FormationError> {
        self.product_lookup()?;
        if self
            .demand
            .product_for_value(self.semantic(caller), value)
            .is_some()
        {
            self.product_snapshot_component(caller, value, slot as u32)
        } else {
            let packed = self.value(caller, value)?;
            self.slot(packed, slot)
        }
    }
    pub(super) fn initialize_inline_product_parameter(
        &mut self,
        context: ContextId,
        caller: ContextId,
        cell: CellId,
        value: ValueId,
        schedule: &mut Vec<js::ExprId>,
    ) -> Result<bool, FormationError> {
        self.product_lookup()?;
        let Some(family) = self.demand.product_for_cell(cell) else {
            return Ok(false);
        };
        let count = self.demand.products()[family].fields().len();
        if let Some(binding) = self.product_cell_handle(context, cell)? {
            let mut components = self.budget.vector(AllocationClass::Retained, count)?;
            for slot in 0..count {
                self.work(1)?;
                let component = self.actual_product_component(caller, value, slot)?;
                self.append(&mut components, component)?;
            }
            let value = self.product_bank_value(cell, components)?;
            let assignment = self.assign(binding, value, None)?;
            self.append(schedule, assignment)?;
            return Ok(true);
        }
        for slot in 0..count {
            self.work(1)?;
            let Some(binding) = self.product_cell_slot(context, cell, slot as u32)? else {
                continue;
            };
            let component = self.actual_product_component(caller, value, slot)?;
            let assignment = self.assign_product_location(binding, component, None)?;
            self.append(schedule, assignment)?;
        }
        Ok(true)
    }
    pub(super) fn append_prepared_argument(
        &mut self,
        before: &mut Vec<js::ExprId>,
        arguments: &mut Vec<js::ExprId>,
        value: js::ExprId,
    ) -> Result<(), FormationError> {
        if arguments.is_empty() && !before.is_empty() {
            self.append(before, value)?;
            let value = self.sequence(std::mem::take(before))?.unwrap();
            self.append(arguments, value)
        } else {
            self.append(arguments, value)
        }
    }
    pub(super) fn append_product_argument(
        &mut self,
        caller: ContextId,
        call: CallId,
        position: u32,
        value: ValueId,
        before: &mut Vec<js::ExprId>,
        arguments: &mut Vec<js::ExprId>,
    ) -> Result<bool, FormationError> {
        self.work(1)?;
        self.function_lookup()?;
        if self.demand.call_transport(caller, call, position) == ProductTransport::Fields {
            self.function_lookup()?;
            let parameter = self
                .demand
                .call_product_parameter(caller, call, position)
                .ok_or_else(|| self.error(Span::default(), "missing expanded product argument"))?;
            let schema = parameter.schema;
            self.work(1)?;
            let definition = self
                .program
                .structs
                .get(schema.index())
                .filter(|definition| definition.identity == schema)
                .ok_or_else(|| self.error(Span::default(), "missing expanded argument schema"))?;
            for slot in 0..definition.fields.len() {
                self.work(1)?;
                let component = self.actual_product_component(caller, value, slot)?;
                self.append_prepared_argument(before, arguments, component)?;
            }
            Ok(true)
        } else {
            let value = self.value(caller, value)?;
            self.append_prepared_argument(before, arguments, value)?;
            Ok(false)
        }
    }
}
