//! Selected product storage belongs to the existing physical context. Mutable
//! snapshot components freeze at their semantic definition; proved stable
//! slots reuse their activation's binding. Neither recipe delays a mutable
//! bank read past the original whole-value observation.
use super::*;
use crate::program::product_family::ProductOperationKind;

#[derive(Clone, Copy)]
enum Component {
    Absent,
    Frozen(js::BindingId),
    Stable(StorageLocation),
    Literal(ValueId),
    // Planning only. Every alias is flattened before formation reads storage.
    Forward(ValueId),
}

#[derive(Default)]
pub(super) struct Storage {
    cells: Vec<(CellId, CellBank)>,
    snapshots: Vec<(ValueId, Vec<Component>)>,
}
/// Physical storage chosen after the one effective-demand fixed point.
#[derive(Clone, Copy)]
pub(super) enum StorageLocation {
    Binding(js::BindingId),
    Indexed { bank: js::BindingId, slot: u32 },
}
struct CellBank {
    handle: Option<js::BindingId>,
    slots: Vec<Option<Bank>>,
}
#[derive(Clone, Copy)]
struct Bank {
    location: StorageLocation,
    stable: bool,
}

impl Formation<'_, '_, '_, '_, '_> {
    pub(super) fn product_lookup(&mut self) -> Result<(), FormationError> {
        let work = self.demand.product_lookup_work();
        if work != 0 {
            self.work(work)?;
        }
        Ok(())
    }
    pub(super) fn plan_product_context(
        &mut self,
        context: ContextId,
    ) -> Result<(), FormationError> {
        let mut storage = Storage::default();
        for (cell, family) in self.demand.product_cells(context) {
            self.work(1)?;
            let fields = self.demand.products()[family].fields().len();
            let mut slots = self.budget.filled(AllocationClass::Scratch, fields, None)?;
            let region =
                self.plan(context).regions[self.program.cells[cell.index()].region.index()];
            let budget = &mut self.budget;
            let shared = self
                .demand
                .product_location_visited(context, cell, |work| {
                    budget.work(WorkKind::Render, work as u64)
                })?
                == Some(super::super::demand::LocationDemand::Shared);
            let handle = if shared {
                let spelling = self.format(format_args!(
                    "product_bank_{}_{}",
                    context.index(),
                    cell.index()
                ))?;
                let handle = self.module.binding_in(
                    js::Binding {
                        source_symbol: None,
                        scope: self.module.regions[region.index()].scope,
                        spelling,
                        pinned: false,
                    },
                    self.budget,
                )?;
                if self.demand.context(context).kind.is_inline() {
                    self.statement(
                        region,
                        js::Statement::Let {
                            binding: handle,
                            value: None,
                        },
                    )?;
                }
                Some(handle)
            } else {
                None
            };
            for (slot, binding) in slots.iter_mut().enumerate() {
                self.work(1)?;
                self.product_lookup()?;
                self.work(2)?; // This setup iterator yields only this context's own cells.
                if !self.demand.needs_product_slot(context, cell, slot as u32) {
                    continue;
                }
                let budget = &mut self.budget;
                let stable = self.demand.stable_product_slot_visited(
                    context,
                    cell,
                    slot as u32,
                    |work| budget.work(WorkKind::Render, work as u64),
                )?;
                if let Some(bank) = handle {
                    *binding = Some(Bank {
                        location: StorageLocation::Indexed {
                            bank,
                            slot: u32::try_from(slot)
                                .ok()
                                .and_then(|slot| slot.checked_add(2))
                                .ok_or(AllocationError::Capacity)?,
                        },
                        stable,
                    });
                    continue;
                }
                if let Some(incoming) = self.product_formal_slot(context, cell, slot)? {
                    *binding = Some(Bank {
                        location: StorageLocation::Binding(incoming),
                        stable,
                    });
                    continue;
                }
                let spelling = self.format(format_args!(
                    "product_{}_{}_{}",
                    context.index(),
                    cell.index(),
                    slot
                ))?;
                let created = self.module.binding_in(
                    js::Binding {
                        source_symbol: None,
                        scope: self.module.regions[region.index()].scope,
                        spelling,
                        pinned: false,
                    },
                    self.budget,
                )?;
                *binding = Some(Bank {
                    location: StorageLocation::Binding(created),
                    stable,
                });
                if self.demand.context(context).kind.is_inline() {
                    self.statement(
                        region,
                        js::Statement::Let {
                            binding: created,
                            value: None,
                        },
                    )?;
                }
            }
            self.budget.push(
                AllocationClass::Scratch,
                &mut storage.cells,
                (cell, CellBank { handle, slots }),
            )?;
        }
        // Banks precede their value snapshots. Install their owner/stability
        // metadata once so captured-cell snapshots use the same lookup owner.
        self.contexts[context.index()].as_mut().unwrap().products = storage;
        let mut snapshots = Vec::new();
        let mut forwarding = false;
        for (value, family) in self.demand.product_snapshots(context) {
            self.work(1)?;
            let fields = self.demand.products()[family].fields().len();
            let mut slots =
                self.budget
                    .filled(AllocationClass::Scratch, fields, Component::Absent)?;
            let data = self.data(context);
            let definition_id = data.values[value.index()].definition;
            let definition = &data.operations[definition_id.index()];
            self.product_lookup()?;
            let choice = self
                .demand
                .product_operation(self.semantic(context), definition_id)
                .copied()
                .ok_or_else(|| self.error(definition.span, "missing product snapshot producer"))?;
            let region = self.plan(context).regions[definition.region.index()];
            for (slot, component) in slots.iter_mut().enumerate() {
                self.work(1)?;
                self.product_lookup()?;
                if !self.demand.needs_snapshot_slot(context, value, slot as u32) {
                    continue;
                }
                match choice {
                    ProductOperationKind::Copy { input, .. } => {
                        *component = Component::Forward(input);
                        forwarding = true;
                        continue;
                    }
                    ProductOperationKind::Construct { .. } => {
                        let operand = data.operands(definition.operands).unwrap()[slot];
                        let operation =
                            &data.operations[data.values[operand.index()].definition.index()];
                        self.work(1)?;
                        if matches!(operation.kind, OperationKind::Constant(_)) {
                            *component = Component::Literal(operand);
                            continue;
                        }
                    }
                    ProductOperationKind::Snapshot { cell, .. } => {
                        let bank = self
                            .product_cell_bank(context, cell, slot as u32)?
                            .ok_or_else(|| {
                                self.error(definition.span, "missing product snapshot bank")
                            })?;
                        if bank.stable {
                            *component = Component::Stable(bank.location);
                            continue;
                        }
                    }
                    _ => {
                        return Err(
                            self.error(definition.span, "unsupported product snapshot producer")
                        )
                    }
                }
                let spelling = self.format(format_args!(
                    "snapshot_{}_{}_{}",
                    context.index(),
                    value.index(),
                    slot
                ))?;
                let binding = self.module.binding_in(
                    js::Binding {
                        source_symbol: None,
                        scope: self.module.regions[region.index()].scope,
                        spelling,
                        pinned: false,
                    },
                    self.budget,
                )?;
                *component = Component::Frozen(binding);
                self.statement(
                    region,
                    js::Statement::Let {
                        binding,
                        value: None,
                    },
                )?;
            }
            self.budget
                .push(AllocationClass::Scratch, &mut snapshots, (value, slots))?;
        }
        if forwarding {
            // Resolve existing immutable SSA copy edges once, independently of
            // arena numbering. No copied edge graph or per-read alias walk.
            let mut path = self
                .budget
                .vector(AllocationClass::Scratch, snapshots.len())?;
            let lookup = (usize::BITS - snapshots.len().leading_zeros()) as usize;
            for start in 0..snapshots.len() {
                self.work(1)?;
                for slot in 0..snapshots[start].1.len() {
                    self.work(1)?;
                    let mut current = start;
                    let resolved = loop {
                        self.work(1)?;
                        match snapshots[current].1[slot] {
                            Component::Forward(input) => {
                                if path.len() == snapshots.len() {
                                    return Err(
                                        self.error(Span::default(), "cyclic product snapshot copy")
                                    );
                                }
                                path.push(current);
                                self.work(lookup)?;
                                current = snapshots
                                    .binary_search_by_key(&input, |(value, _)| *value)
                                    .map_err(|_| {
                                        self.error(
                                            Span::default(),
                                            "missing product copy source snapshot",
                                        )
                                    })?;
                            }
                            Component::Absent if !path.is_empty() => {
                                return Err(self.error(
                                    Span::default(),
                                    "undemanded product copy source component",
                                ))
                            }
                            component => break component,
                        }
                    };
                    while let Some(index) = path.pop() {
                        self.work(1)?;
                        snapshots[index].1[slot] = resolved;
                    }
                }
            }
            self.drop_scratch(path)?;
        }
        self.contexts[context.index()]
            .as_mut()
            .unwrap()
            .products
            .snapshots = snapshots;
        Ok(())
    }

    pub(super) fn product_cell_binding(
        &mut self,
        context: ContextId,
        cell: CellId,
        slot: u32,
    ) -> Result<StorageLocation, FormationError> {
        self.product_cell_slot(context, cell, slot)?
            .ok_or_else(|| self.error(Span::default(), "undemanded product cell slot"))
    }
    pub(super) fn product_cell_slot(
        &mut self,
        context: ContextId,
        cell: CellId,
        slot: u32,
    ) -> Result<Option<StorageLocation>, FormationError> {
        Ok(self
            .product_cell_bank(context, cell, slot)?
            .map(|bank| bank.location))
    }
    fn product_cell_row(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<(ContextId, usize), FormationError> {
        let budget = &mut self.budget;
        let owner = self
            .demand
            .cell_owner_visited(context, cell, || budget.work(WorkKind::Render, 1))?
            .ok_or_else(|| self.error(Span::default(), "product cell outside its activation"))?;
        let len = self.contexts[owner.index()]
            .as_ref()
            .unwrap()
            .products
            .cells
            .len();
        self.work((usize::BITS - len.leading_zeros()) as usize)?;
        let index = self.contexts[owner.index()]
            .as_ref()
            .unwrap()
            .products
            .cells
            .binary_search_by_key(&cell, |(cell, _)| *cell)
            .map_err(|_| self.error(Span::default(), "missing product cell bank"))?;
        Ok((owner, index))
    }
    fn product_cell_bank(
        &mut self,
        context: ContextId,
        cell: CellId,
        slot: u32,
    ) -> Result<Option<Bank>, FormationError> {
        let (owner, index) = self.product_cell_row(context, cell)?;
        Ok(self.contexts[owner.index()]
            .as_ref()
            .unwrap()
            .products
            .cells[index]
            .1
            .slots
            .get(slot as usize)
            .copied()
            .flatten())
    }
    pub(super) fn product_cell_handle(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<Option<js::BindingId>, FormationError> {
        self.product_lookup()?;
        if self.demand.product_for_cell(cell).is_none() {
            return Ok(None);
        }
        let (owner, index) = self.product_cell_row(context, cell)?;
        Ok(self.contexts[owner.index()]
            .as_ref()
            .unwrap()
            .products
            .cells[index]
            .1
            .handle)
    }
    pub(super) fn read_product_location(
        &mut self,
        location: StorageLocation,
    ) -> Result<js::ExprId, FormationError> {
        match location {
            StorageLocation::Binding(binding) => self.reference(binding),
            StorageLocation::Indexed { bank, slot } => {
                let bank = self.reference(bank)?;
                self.slot(bank, slot as usize)
            }
        }
    }
    pub(super) fn assign_product_location(
        &mut self,
        location: StorageLocation,
        value: js::ExprId,
        origin: Option<crate::ast::SourceNodeId>,
    ) -> Result<js::ExprId, FormationError> {
        match location {
            StorageLocation::Binding(binding) => self.assign(binding, value, origin),
            StorageLocation::Indexed { .. } => {
                let target = self.read_product_location(location)?;
                Ok(self.module.expression_in(
                    js::Expr::Assign { target, value },
                    origin,
                    self.budget,
                )?)
            }
        }
    }
    /// One backing object per addressed activation. Components are already raw
    /// frozen values; the schema functions introduce no source conversion.
    pub(super) fn product_bank_value(
        &mut self,
        cell: CellId,
        mut components: Vec<js::ExprId>,
    ) -> Result<js::ExprId, FormationError> {
        self.product_lookup()?;
        let family = self
            .demand
            .product_for_cell(cell)
            .ok_or_else(|| self.error(Span::default(), "missing addressed product family"))?;
        let schema = self.demand.products()[family].schema().index();
        let (copy, assign) = self.product_reference_schema(schema)?;
        let copy = self.reference(copy)?;
        let assign = self.reference(assign)?;
        self.work(components.len())?;
        self.budget
            .reserve_vec(AllocationClass::Retained, &mut components, 2)?;
        components.push(copy);
        components.push(assign);
        components.rotate_right(2);
        self.product(components)
    }
    pub(super) fn packed_product_cell(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<js::ExprId, FormationError> {
        self.product_lookup()?;
        let family = self
            .demand
            .product_for_cell(cell)
            .ok_or_else(|| self.error(Span::default(), "missing product cell family"))?;
        let count = self.demand.products()[family].fields().len();
        let mut components = self.budget.vector(AllocationClass::Retained, count)?;
        for slot in 0..count {
            self.work(1)?;
            let location = self.product_cell_binding(context, cell, slot as u32)?;
            let value = self.read_product_location(location)?;
            self.append(&mut components, value)?;
        }
        self.product(components)
    }

    pub(super) fn store_product_cell(
        &mut self,
        context: ContextId,
        cell: CellId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        self.product_lookup()?;
        let family = self
            .demand
            .product_for_cell(cell)
            .ok_or_else(|| self.error(Span::default(), "missing product replacement family"))?;
        let count = self.demand.products()[family].fields().len();
        let mut schedule = self
            .budget
            .vector(AllocationClass::Retained, count.saturating_add(2))?;
        // Reuse existing SSA capture where available. Otherwise the physical
        // unpack recipe evaluates its one packed input once, before any store.
        let binding = if let js::Expr::Binding(binding) = self.module.expressions[value.index()] {
            binding
        } else {
            let region = self.plan(context).regions[self.data(context).entry.index()];
            let spelling = self.format(format_args!(
                "product_replacement_{}_{}",
                context.index(),
                self.module.bindings.len()
            ))?;
            let binding = self.module.binding_in(
                js::Binding {
                    source_symbol: None,
                    scope: self.module.regions[region.index()].scope,
                    spelling,
                    pinned: false,
                },
                self.budget,
            )?;
            self.statement(
                region,
                js::Statement::Let {
                    binding,
                    value: None,
                },
            )?;
            let capture = self.assign(binding, value, None)?;
            self.append(&mut schedule, capture)?;
            binding
        };
        for slot in 0..count {
            self.work(1)?;
            let Some(location) = self.product_cell_slot(context, cell, slot as u32)? else {
                continue;
            };
            let source = self.reference(binding)?;
            let component = self.slot(source, slot)?;
            let assignment = self.assign_product_location(location, component, None)?;
            self.append(&mut schedule, assignment)?;
        }
        let result = self.reference(binding)?;
        self.append(&mut schedule, result)?;
        self.sequence(schedule)?
            .ok_or_else(|| self.error(Span::default(), "empty product replacement"))
    }

    fn product_snapshot_slot(
        &mut self,
        context: ContextId,
        value: ValueId,
        slot: u32,
    ) -> Result<Component, FormationError> {
        let len = self.contexts[context.index()]
            .as_ref()
            .unwrap()
            .products
            .snapshots
            .len();
        self.work((usize::BITS - len.leading_zeros()) as usize)?;
        let snapshots = &self.contexts[context.index()]
            .as_ref()
            .unwrap()
            .products
            .snapshots;
        let index = snapshots
            .binary_search_by_key(&value, |(value, _)| *value)
            .map_err(|_| self.error(Span::default(), "missing product value snapshot"))?;
        snapshots[index]
            .1
            .get(slot as usize)
            .copied()
            .ok_or_else(|| self.error(Span::default(), "product snapshot slot outside schema"))
    }
    pub(super) fn product_snapshot_component(
        &mut self,
        context: ContextId,
        value: ValueId,
        slot: u32,
    ) -> Result<js::ExprId, FormationError> {
        match self.product_snapshot_slot(context, value, slot)? {
            Component::Frozen(binding) => self.reference(binding),
            Component::Stable(location) => self.read_product_location(location),
            Component::Literal(value) => self.value(context, value),
            Component::Absent => {
                Err(self.error(Span::default(), "undemanded product snapshot component"))
            }
            Component::Forward(_) => {
                Err(self.error(Span::default(), "unresolved product snapshot copy"))
            }
        }
    }

    /// Return a selected top-level component, or None for ordinary packed
    /// storage. The original Place DAG remains the only nested path owner.
    pub(super) fn product_place_component(
        &mut self,
        context: ContextId,
        root: PlaceId,
        slot: u32,
    ) -> Result<Option<js::ExprId>, FormationError> {
        self.work(1)?;
        self.product_lookup()?;
        let binding = match self.data(context).places[root.index()] {
            Place::Cell(cell) if self.demand.product_for_cell(cell).is_some() => {
                self.product_cell_binding(context, cell, slot)?
            }
            Place::Value(value)
                if self
                    .demand
                    .product_for_value(self.semantic(context), value)
                    .is_some() =>
            {
                return Ok(Some(self.product_snapshot_component(context, value, slot)?));
            }
            _ => return Ok(None),
        };
        Ok(Some(self.read_product_location(binding)?))
    }

    pub(super) fn initialize_product(
        &mut self,
        context: ContextId,
        region: js::RegionId,
        cell: CellId,
        input: ValueId,
    ) -> Result<(), FormationError> {
        self.product_lookup()?;
        let family = self
            .demand
            .product_for_cell(cell)
            .ok_or_else(|| self.error(Span::default(), "missing product initialization family"))?;
        let count = self.demand.products()[family].fields().len();
        if let Some(binding) = self.product_cell_handle(context, cell)? {
            let mut components = self.budget.vector(AllocationClass::Retained, count)?;
            for slot in 0..count {
                self.work(1)?;
                let value = self.product_snapshot_component(context, input, slot as u32)?;
                self.append(&mut components, value)?;
            }
            let value = self.product_bank_value(cell, components)?;
            return self.statement(
                region,
                js::Statement::Let {
                    binding,
                    value: Some(value),
                },
            );
        }
        for slot in 0..count {
            self.work(1)?;
            let Some(StorageLocation::Binding(binding)) =
                self.product_cell_slot(context, cell, slot as u32)?
            else {
                continue;
            };
            let value = self.product_snapshot_component(context, input, slot as u32)?;
            self.statement(
                region,
                js::Statement::Let {
                    binding,
                    value: Some(value),
                },
            )?;
        }
        Ok(())
    }

    pub(super) fn product_expression(
        &mut self,
        context: ContextId,
        operation: &Operation,
        choice: ProductOperationKind,
    ) -> Result<Option<js::ExprId>, FormationError> {
        if matches!(choice, ProductOperationKind::Initialize { .. })
            && !self.demand.context(context).kind.is_inline()
        {
            return Err(self.error(
                operation.span,
                "product initialization in expression region",
            ));
        }
        let mut schedule = Vec::new();
        let (output, source_cell) = match choice {
            ProductOperationKind::Construct { value } => (value, None),
            ProductOperationKind::Snapshot { cell, value } => (value, Some(cell)),
            // Reusing an immutable snapshot is a raw value copy, never a view
            // of mutable cell banks. All aliases were flattened during setup.
            ProductOperationKind::Copy { .. } => return Ok(None),
            ProductOperationKind::Initialize { cell, input }
            | ProductOperationKind::Assign { cell, input } => {
                self.product_lookup()?;
                let family = self.demand.product_for_cell(cell).ok_or_else(|| {
                    self.error(operation.span, "missing product assignment family")
                })?;
                if matches!(choice, ProductOperationKind::Initialize { .. }) {
                    if let Some(binding) = self.product_cell_handle(context, cell)? {
                        let count = self.demand.products()[family].fields().len();
                        let mut components =
                            self.budget.vector(AllocationClass::Retained, count)?;
                        for slot in 0..count {
                            self.work(1)?;
                            let value =
                                self.product_snapshot_component(context, input, slot as u32)?;
                            self.append(&mut components, value)?;
                        }
                        let value = self.product_bank_value(cell, components)?;
                        return Ok(Some(self.assign(binding, value, operation.origin)?));
                    }
                }
                for slot in 0..self.demand.products()[family].fields().len() {
                    self.work(1)?;
                    let Some(binding) = self.product_cell_slot(context, cell, slot as u32)? else {
                        continue;
                    };
                    let value = self.product_snapshot_component(context, input, slot as u32)?;
                    let assigned =
                        self.assign_product_location(binding, value, operation.origin)?;
                    self.append(&mut schedule, assigned)?;
                }
                // Common Store is effect-only. Source assignment expressions
                // already retain their RHS value through ordinary SSA copies.
                return self.sequence(schedule);
            }
            ProductOperationKind::Access(_) => {
                return Err(self.error(
                    operation.span,
                    "product access bypassed ordinary load/store owner",
                ))
            }
        };
        self.product_lookup()?;
        let family = self
            .demand
            .product_for_value(self.semantic(context), output)
            .ok_or_else(|| self.error(operation.span, "missing product snapshot family"))?;
        for slot in 0..self.demand.products()[family].fields().len() {
            self.work(1)?;
            let binding = match self.product_snapshot_slot(context, output, slot as u32)? {
                Component::Frozen(binding) => binding,
                Component::Literal(_) | Component::Stable(_) | Component::Absent => continue,
                Component::Forward(_) => {
                    return Err(self.error(operation.span, "unresolved product capture component"))
                }
            };
            let value = if let Some(cell) = source_cell {
                let binding = self.product_cell_binding(context, cell, slot as u32)?;
                self.read_product_location(binding)?
            } else {
                let operand = self.data(context).operands(operation.operands).unwrap()[slot];
                self.value(context, operand)?
            };
            let captured = self.assign(binding, value, operation.origin)?;
            self.append(&mut schedule, captured)?;
        }
        self.sequence(schedule)
    }
    /// Retain the current-parent rebuild rule inside the selected top-level
    /// slot. The RHS has already run; independent sibling slots stay untouched.
    pub(super) fn store_product_field(
        &mut self,
        context: ContextId,
        cell: CellId,
        path: Vec<structs::FieldRecipe>,
        replacement: js::ExprId,
        span: Span,
    ) -> Result<js::ExprId, FormationError> {
        let top = path
            .last()
            .copied()
            .ok_or_else(|| self.error(span, "empty product field path"))?;
        let binding = self.product_cell_binding(context, cell, top.slot as u32)?;
        let nested = &path[..path.len() - 1];
        // ProductAccess closes every root/nested-product writer and proves
        // initialized, present parents even after RHS reentry. No duplicate
        // raw check is needed. Sibling reconstruction below still reads the
        // CURRENT parents after the already-frozen source RHS.
        let mut replacement = replacement;
        for (depth, recipe) in nested.iter().enumerate() {
            self.work(1)?;
            let count = self.program.structs[recipe.schema].fields.len();
            let mut values = self.budget.vector(AllocationClass::Retained, count)?;
            for slot in 0..count {
                self.work(1)?;
                let value = if slot == recipe.slot {
                    replacement
                } else {
                    let mut parent = self.read_product_location(binding)?;
                    for ancestor in nested[depth + 1..].iter().rev() {
                        self.work(1)?;
                        parent = self.slot(parent, ancestor.slot)?;
                    }
                    self.slot(parent, slot)?
                };
                self.append(&mut values, value)?;
            }
            replacement = self.product(values)?;
        }
        self.drop_scratch(path)?;
        self.assign_product_location(binding, replacement, None)
    }
}
