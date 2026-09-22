//! Product components use the existing context, storage-writer and demand
//! worklists. These sparse indexes select physical recipes; the original SSA,
//! Place DAG and family witnesses remain the only semantic graph.
use super::super::raw_domains::{Recipes, ResultRecipe};
use super::*;

#[derive(Debug)]
pub(super) struct ProductBank {
    pub(super) cell: CellId,
    family: usize,
    pub(super) slots: Vec<Storage>,
}
#[derive(Debug)]
pub(super) struct ProductSnapshot {
    pub(super) value: ValueId,
    family: usize,
    slots: Vec<bool>,
}
fn lookup_work(len: usize) -> usize {
    (usize::BITS - len.leading_zeros()) as usize
}

impl<'program, 'src> DemandPlan<'program, 'src> {
    pub(in crate::semantic_program) fn product_lookup_work(&self) -> usize {
        // Parameter-only families can have cells and no operation overlays.
        // Every sparse lookup stays bounded by its actual retained owner.
        lookup_work(
            self.product_operations
                .len()
                .max(self.product_cell_index.len())
                .max(self.product_value_index.len()),
        )
    }
    pub(in crate::semantic_program) fn products(&self) -> &[&'program ProductFamily] {
        &self.products
    }
    pub(in crate::semantic_program) fn product_for_cell(&self, cell: CellId) -> Option<usize> {
        let owner = self.program.cells[cell.index()].owner;
        self.product_cell_index
            .binary_search_by_key(&(owner, cell), |entry| entry.0)
            .ok()
            .map(|i| self.product_cell_index[i].1)
    }
    pub(in crate::semantic_program) fn product_for_value(
        &self,
        unit: UnitId,
        value: ValueId,
    ) -> Option<usize> {
        self.product_value_index
            .binary_search_by_key(&(unit, value), |entry| entry.0)
            .ok()
            .map(|i| self.product_value_index[i].1)
    }
    pub(in crate::semantic_program) fn product_operation(
        &self,
        unit: UnitId,
        operation: OpId,
    ) -> Option<&ProductOperationKind> {
        self.product_operations
            .binary_search_by_key(&(unit, operation), |entry| entry.0)
            .ok()
            .map(|i| &self.product_operations[i].1)
    }
    pub(in crate::semantic_program) fn product_cells(
        &self,
        context: ContextId,
    ) -> impl Iterator<Item = (CellId, usize)> + '_ {
        self.context(context)
            .product_banks
            .iter()
            .map(|bank| (bank.cell, bank.family))
    }
    pub(in crate::semantic_program) fn product_snapshots(
        &self,
        context: ContextId,
    ) -> impl Iterator<Item = (ValueId, usize)> + '_ {
        self.context(context)
            .product_snapshots
            .iter()
            .map(|snapshot| (snapshot.value, snapshot.family))
    }
    pub(in crate::semantic_program) fn needs_product_slot(
        &self,
        context: ContextId,
        cell: CellId,
        slot: u32,
    ) -> bool {
        self.cell_owner(context, cell)
            .and_then(|owner| {
                let banks = &self.context(owner).product_banks;
                banks
                    .binary_search_by_key(&cell, |bank| bank.cell)
                    .ok()
                    .map(|index| banks[index].slots[slot as usize].observation.is_observed())
            })
            .unwrap_or(false)
    }
    /// Final Demand plans contain every materialized context's possible writes.
    /// A negative rewrite bit applies to the actual lexical owner, not the
    /// reader's context. Inline entry assignments reuse banks, so they are not
    /// stable even when their original source has no Store operation.
    pub(in crate::semantic_program) fn stable_product_slot_visited<E>(
        &self,
        context: ContextId,
        cell: CellId,
        slot: u32,
        mut visit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        let Some(owner) = self.cell_owner_visited(context, cell, || visit(1))? else {
            return Ok(false);
        };
        let banks = &self.context(owner).product_banks;
        visit(1 + lookup_work(banks.len()))?;
        let Some(storage) = banks
            .binary_search_by_key(&cell, |bank| bank.cell)
            .ok()
            .and_then(|bank| banks[bank].slots.get(slot as usize))
        else {
            return Ok(false);
        };
        Ok(storage.observation.is_observed()
            && !storage.rewritten
            && !self.context(owner).kind.is_inline())
    }
    pub(in crate::semantic_program) fn needs_snapshot_slot(
        &self,
        context: ContextId,
        value: ValueId,
        slot: u32,
    ) -> bool {
        let snapshots = &self.context(context).product_snapshots;
        snapshots
            .binary_search_by_key(&value, |snapshot| snapshot.value)
            .ok()
            .is_some_and(|index| snapshots[index].slots[slot as usize])
    }
    pub(super) fn index_products(
        &mut self,
        implementations: &'program ImplementationMap,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        for family in implementations.products() {
            budget.work(1)?;
            let index = self.products.len();
            budget.push(&mut self.products, family)?;
            for cell in family.cells() {
                budget.work(1)?;
                let unit = self.program.cells[cell.cell.index()].owner;
                budget.push(&mut self.product_cell_index, ((unit, cell.cell), index))?;
            }
            for entry in family.operations() {
                budget.work(1)?;
                let unit = entry.operation.unit;
                let operation = entry.operation.operation;
                budget.push(
                    &mut self.product_operations,
                    ((unit, operation), entry.kind),
                )?;
                let value = match entry.kind {
                    ProductOperationKind::Construct { value }
                    | ProductOperationKind::Snapshot { value, .. }
                    | ProductOperationKind::Copy { value, .. } => Some(value),
                    _ => None,
                };
                if let Some(value) = value {
                    budget.push(&mut self.product_value_index, ((unit, value), index))?;
                }
            }
        }
        for len in [
            self.product_cell_index.len(),
            self.product_value_index.len(),
            self.product_operations.len(),
        ] {
            budget.work(sort_work(len)?)?;
        }
        self.product_cell_index
            .sort_unstable_by_key(|entry| entry.0);
        self.product_value_index
            .sort_unstable_by_key(|entry| entry.0);
        self.product_operations
            .sort_unstable_by_key(|entry| entry.0);
        budget.work(
            self.product_cell_index.len()
                + self.product_value_index.len()
                + self.product_operations.len(),
        )?;
        for entries in [
            self.product_cell_index
                .windows(2)
                .any(|pair| pair[0].0 == pair[1].0),
            self.product_value_index
                .windows(2)
                .any(|pair| pair[0].0 == pair[1].0),
        ] {
            if entries {
                return Err(unsupported("overlapping product demand recipes"));
            }
        }
        if self
            .product_operations
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err(unsupported("overlapping product operation recipes"));
        }
        Ok(())
    }
    pub(super) fn allocate_product_context(
        &self,
        unit: UnitId,
        budget: &mut Budget<'_>,
    ) -> Result<(Vec<ProductBank>, Vec<ProductSnapshot>), DemandError> {
        budget.work(
            2 * (lookup_work(self.product_cell_index.len())
                + lookup_work(self.product_value_index.len())),
        )?;
        let start = self
            .product_cell_index
            .partition_point(|entry| entry.0 .0 < unit);
        let end = self
            .product_cell_index
            .partition_point(|entry| entry.0 .0 <= unit);
        let mut banks = budget.vector(end - start)?;
        for &((_, cell), family) in &self.product_cell_index[start..end] {
            let slots = budget.filled(self.products[family].fields().len(), Storage::default())?;
            banks.push(ProductBank {
                cell,
                family,
                slots,
            });
        }
        let start = self
            .product_value_index
            .partition_point(|entry| entry.0 .0 < unit);
        let end = self
            .product_value_index
            .partition_point(|entry| entry.0 .0 <= unit);
        let mut snapshots = budget.vector(end - start)?;
        for &((_, value), family) in &self.product_value_index[start..end] {
            let slots = budget.filled(self.products[family].fields().len(), false)?;
            snapshots.push(ProductSnapshot {
                value,
                family,
                slots,
            });
        }
        Ok((banks, snapshots))
    }
    pub(super) fn product_storage(
        &self,
        context: ContextId,
        cell: CellId,
        slot: u32,
        budget: &mut Budget<'_>,
    ) -> Result<StorageId, DemandError> {
        let owner = self
            .cell_owner_visited(context, cell, || budget.work(1))?
            .ok_or_else(|| unsupported("product storage outside activation"))?;
        let banks = &self.context(owner).product_banks;
        budget.work(lookup_work(banks.len()))?;
        let index = banks
            .binary_search_by_key(&cell, |bank| bank.cell)
            .map_err(|_| unsupported("missing product bank"))?;
        Ok(StorageId::Product(owner, index, slot))
    }
    pub(super) fn need_snapshot(
        &mut self,
        context: ContextId,
        value: ValueId,
        slot: u32,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        budget.work(1 + lookup_work(self.context(context).product_snapshots.len()))?;
        let index = self
            .context(context)
            .product_snapshots
            .binary_search_by_key(&value, |snapshot| snapshot.value)
            .map_err(|_| unsupported("missing product snapshot"))?;
        if !self.context(context).product_snapshots[index].slots[slot as usize] {
            self.contexts[context.index()].product_snapshots[index].slots[slot as usize] = true;
            self.contexts[context.index()].values[value.index()] = ObservationDemand::Exact;
            budget.push(&mut self.pending, Pending::Snapshot(context, index, slot))?;
        }
        Ok(())
    }
    pub(super) fn need_whole_snapshot(
        &mut self,
        context: ContextId,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        budget.work(lookup_work(self.product_value_index.len()))?;
        let family = self
            .product_for_value(self.context(context).unit, value)
            .unwrap();
        for slot in 0..self.products[family].fields().len() {
            self.need_snapshot(context, value, slot as u32, budget)?;
        }
        Ok(())
    }
    pub(super) fn visit_snapshot(
        &mut self,
        context: ContextId,
        snapshot: usize,
        slot: u32,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        let unit = self.context(context).unit;
        let value = self.context(context).product_snapshots[snapshot].value;
        let data = self.program.units[unit.index()].data();
        let operation = data.values[value.index()].definition;
        self.need_production(context, operation, budget)?;
        budget.work(lookup_work(self.product_operations.len()))?;
        match *self
            .product_operation(unit, operation)
            .ok_or_else(|| unsupported("missing product snapshot producer"))?
        {
            ProductOperationKind::Construct { .. } => {
                let input = data
                    .operands(data.operations[operation.index()].operands)
                    .unwrap()[slot as usize];
                self.need_value(context, input, ObservationDemand::Exact, budget)?;
            }
            ProductOperationKind::Snapshot { cell, .. } => {
                let storage = self.product_storage(context, cell, slot, budget)?;
                self.need_storage(storage, budget)?;
            }
            ProductOperationKind::Copy { input, .. } => {
                self.need_snapshot(context, input, slot, budget)?
            }
            _ => return Err(unsupported("invalid product snapshot producer")),
        }
        Ok(())
    }
    pub(super) fn product_writer_input(
        &mut self,
        storage: StorageId,
        context: ContextId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        let StorageId::Product(_, _, slot) = storage else {
            return Ok(());
        };
        budget.work(lookup_work(self.product_operations.len()))?;
        if let Some(
            ProductOperationKind::Initialize { input, .. }
            | ProductOperationKind::Assign { input, .. },
        ) = self
            .product_operation(self.context(context).unit, operation)
            .copied()
        {
            // A later slot may become demanded after this operation's shared
            // dependencies were visited. Follow that new edge here once per
            // writer/slot rather than rescan or clear operation deduplication.
            self.need_snapshot(context, input, slot, budget)?;
        }
        Ok(())
    }
    pub(super) fn seed_product_operation(
        &mut self,
        context: ContextId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<bool, DemandError> {
        let unit = self.context(context).unit;
        budget.work(lookup_work(self.product_operations.len()))?;
        let Some(recipe) = self.product_operation(unit, operation).copied() else {
            return Ok(false);
        };
        match recipe {
            ProductOperationKind::Initialize { cell, .. }
            | ProductOperationKind::Assign { cell, .. } => {
                budget.work(lookup_work(self.product_cell_index.len()))?;
                let family = self.product_for_cell(cell).unwrap();
                for slot in 0..self.products[family].fields().len() {
                    let storage = self.product_storage(context, cell, slot as u32, budget)?;
                    if matches!(recipe, ProductOperationKind::Assign { .. }) {
                        budget.work(1)?;
                        self.storage_mut(storage).rewritten = true;
                    }
                    self.wait_for(storage, context, operation, budget)?;
                }
            }
            ProductOperationKind::Access(access) => {
                match self.program.units[unit.index()].data().operations[operation.index()].kind {
                    OperationKind::Store(_) => {
                        let ProductAccessRoot::Cell(cell) = access.root() else {
                            return Err(unsupported("writable product value root"));
                        };
                        let storage = self.product_storage(context, cell, access.slot(), budget)?;
                        budget.work(1)?;
                        self.storage_mut(storage).rewritten = true;
                        self.wait_for(storage, context, operation, budget)?;
                    }
                    OperationKind::Load(_) => {
                        // Raw presence is proved; integer payload conversion is
                        // still the target's selected operation and can call host
                        // hooks, throw, write shared state, or fail to terminate.
                        if super::super::javascript::JavaScriptRecipes.result(
                            self.program,
                            unit,
                            operation,
                        ) == ResultRecipe::NormalizedI32
                        {
                            self.need_operation(context, operation, budget)?;
                        }
                    }
                    OperationKind::CheckPlace(_) => {} // complete raw presence witness
                    _ => return Err(unsupported("invalid product access recipe")),
                }
            }
            ProductOperationKind::Construct { .. }
            | ProductOperationKind::Snapshot { .. }
            | ProductOperationKind::Copy { .. } => {}
        }
        Ok(true)
    }
    pub(super) fn product_parameter_input(
        &mut self,
        context: ContextId,
        bank: usize,
        slot: u32,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        let cell = self.context(context).product_banks[bank].cell;
        if let CellBinding::Parameter(position) = self.program.cells[cell.index()].binding {
            if self.context(context).kind.is_inline() {
                let (caller, value) = self.inline_parameter_source(context, position);
                budget.work(lookup_work(self.product_value_index.len()))?;
                if self
                    .product_for_value(self.context(caller).unit, value)
                    .is_some()
                {
                    self.need_snapshot(caller, value, slot, budget)?;
                } else {
                    self.need_value(caller, value, ObservationDemand::Exact, budget)?;
                }
            }
        }
        Ok(())
    }
    pub(super) fn preserve_products(
        &mut self,
        context: ContextId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        for bank in 0..self.context(context).product_banks.len() {
            for slot in 0..self.context(context).product_banks[bank].slots.len() {
                self.need_storage(StorageId::Product(context, bank, slot as u32), budget)?;
            }
        }
        Ok(())
    }
    pub(super) fn next_product_input(
        &self,
        cursor: &mut InputCursor,
        recipe: ProductOperationKind,
    ) -> InputStep {
        let context = cursor.context;
        let unit = self.context(context).unit;
        let data = self.program.units[unit.index()].data();
        let operation = &data.operations[cursor.operation.index()];
        let site = EffectiveUseSite::Operation(cursor.operation);
        match recipe {
            ProductOperationKind::Construct { .. } => {
                let operands = data.operands(operation.operands).unwrap();
                let slot = cursor.index;
                let Some(&input) = operands.get(slot) else {
                    return InputStep::Done;
                };
                cursor.index += 1;
                let snapshot = cursor
                    .product_snapshot
                    .expect("selected product construct snapshot");
                if self.context(context).product_snapshots[snapshot].slots[slot] {
                    InputStep::Dependency(InputDependency::Value(EffectiveValueUse {
                        observation: ObservationDemand::Exact,
                        value: input,
                        site,
                        role: EffectiveUseRole::Operand(slot as u32),
                    }))
                } else {
                    InputStep::Skip
                }
            }
            // Component edges for these recipes are followed by Snapshot and
            // Storage events, including edges discovered after op visitation.
            ProductOperationKind::Snapshot { .. }
            | ProductOperationKind::Copy { .. }
            | ProductOperationKind::Initialize { .. }
            | ProductOperationKind::Assign { .. } => InputStep::Done,
            ProductOperationKind::Access(access) => {
                // This exact access certificate establishes current-parent
                // presence, including Preserve mode. Retaining the check
                // operation must not invent a scalar storage dependency.
                if matches!(operation.kind, OperationKind::CheckPlace(_)) {
                    return InputStep::Done;
                }
                if cursor.stage == 0 {
                    cursor.stage = 1;
                    return InputStep::Dependency(match access.root() {
                        ProductAccessRoot::Cell(cell) => {
                            InputDependency::ProductCell(cell, access.slot())
                        }
                        ProductAccessRoot::Value(value) => {
                            InputDependency::ProductSnapshot(value, access.slot())
                        }
                    });
                }
                let operands = data.operands(operation.operands).unwrap();
                let Some(&value) = operands.get(cursor.index) else {
                    return InputStep::Done;
                };
                let position = cursor.index;
                cursor.index += 1;
                InputStep::Dependency(InputDependency::Value(EffectiveValueUse {
                    observation: ObservationDemand::Exact,
                    value,
                    site,
                    role: EffectiveUseRole::Operand(position as u32),
                }))
            }
        }
    }
}
