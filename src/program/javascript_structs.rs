//! The direct product recipe uses immutable positional storage. Logical copies
//! share that storage; updating a place replaces its value ancestors. This is
//! a target recipe, not a source alias rule or permission to expose its arrays.
use super::*;

pub(super) struct Plan {
    pub(super) boundary_types: Vec<bool>,
    // D2 public adapters, created on demand at the export boundary: one
    // wrapper per exported function and one codec per schema and direction.
    // A wrapped function's own name and callable kind are private: sorted
    // units whose public face is their wrapper.
    pub(super) public_units: Vec<UnitId>,
    pub(super) public_exports: Vec<(CellId, js::BindingId, Option<js::FunctionId>)>,
    pub(super) public_codecs: Vec<(usize, bool, js::BindingId)>,
    /// Single-use struct values that flow into a `JsValue` destination: each
    /// is formed through its D2 public encoder, as an exported result is.
    pub(super) public_encodes: Vec<(ContextId, ValueId)>,
    /// One hoisted D2 callable adapter factory per function type.
    pub(super) public_callables: Vec<(TypeId, js::BindingId)>,
    // Sorted once by stable member identity. Entries retain physical slots and
    // their schema owner, so emission never resolves a field by source spelling.
    fields: Vec<(usize, FieldRecipe)>,
}

#[derive(Clone, Copy)]
pub(super) struct FieldRecipe {
    pub(super) slot: usize,
    pub(super) schema: usize,
}

pub(super) fn plan(
    program: &Program<'_>,
    contract: &JavaScriptCompilationContract,
    budget: &mut AllocationBudget<'_>,
) -> Result<Plan, FormationError> {
    if program.structs.is_empty() {
        return Ok(Plan {
            boundary_types: Vec::new(),
            public_units: Vec::new(),
            public_exports: Vec::new(),
            public_codecs: Vec::new(),
            public_encodes: Vec::new(),
            public_callables: Vec::new(),
            fields: Vec::new(),
        });
    }
    let mut boundary_types = budget.vector(AllocationClass::Scratch, program.types.len())?;
    let mut pending = budget.vector(AllocationClass::Scratch, 1)?;
    for ty in program.types.iter() {
        let contains = super::super::facts::contains_nominal_product(
            ty,
            &mut pending,
            &mut references::Meter(budget),
        )?;
        budget.push(AllocationClass::Scratch, &mut boundary_types, contains)?;
    }
    let bytes = pending
        .capacity()
        .checked_mul(std::mem::size_of::<&Type<'_>>())
        .ok_or(AllocationError::Capacity)?;
    drop(pending);
    budget.release(AllocationClass::Scratch, bytes as u64)?;
    let mut public_units = Vec::new();
    if contract.abi.preserve_root_exports {
        budget.work(WorkKind::Render, program.exports().len() as u64)?;
        for (_, cell) in program.value_exports() {
            budget.work(WorkKind::Render, 1)?;
            if !boundary_types[program.cells[cell.index()].ty.index()] {
                continue;
            }
            if !super::public_structs::adaptable_export(program, cell, budget)? {
                return Err(Unsupported {
                    span: program.cells[cell.index()].declaration,
                    feature: "public value-struct ABI adaptation",
                }
                .into());
            }
            if let CellBinding::Function(unit) = program.cells[cell.index()].binding {
                budget.push(AllocationClass::Scratch, &mut public_units, unit)?;
            }
        }
        budget.work(
            WorkKind::Render,
            public_units
                .len()
                .saturating_mul((usize::BITS - public_units.len().leading_zeros()) as usize)
                as u64,
        )?;
        public_units.sort_unstable();
        public_units.dedup();
    }
    let mut fields = budget.vector(AllocationClass::Scratch, program.fields.len())?;
    for (schema, definition) in program.structs.iter().enumerate() {
        budget.work(WorkKind::Render, 1)?;
        for field in &program.fields[definition.fields.clone()] {
            budget.work(WorkKind::Render, 1)?;
            budget.push(
                AllocationClass::Scratch,
                &mut fields,
                (
                    field.identity.index(),
                    FieldRecipe {
                        slot: field.index,
                        schema,
                    },
                ),
            )?;
        }
    }
    budget.work(
        WorkKind::Render,
        fields
            .len()
            .saturating_mul((usize::BITS - fields.len().leading_zeros()) as usize) as u64,
    )?;
    fields.sort_unstable_by_key(|(identity, _)| *identity);
    Ok(Plan {
        boundary_types,
        public_units,
        public_exports: Vec::new(),
        public_codecs: Vec::new(),
        public_encodes: Vec::new(),
        public_callables: Vec::new(),
        fields,
    })
}

impl Plan {
    /// A unit published only through its D2 wrapper.
    pub(super) fn wrapped(&self, unit: UnitId) -> bool {
        self.public_units.binary_search(&unit).is_ok()
    }
}

impl Formation<'_, '_, '_, '_, '_> {
    pub(super) fn validate_struct_schema(
        &mut self,
        identity: NominalId,
        span: Span,
    ) -> Result<(), FormationError> {
        for definition in self.program.structs.iter() {
            self.work(1)?;
            if definition.identity == identity {
                return if definition.type_parameters.is_empty() {
                    Ok(())
                } else {
                    Err(self.error(span, "instantiated value-struct schema"))
                };
            }
        }
        Err(self.error(span, "missing value-struct schema"))
    }

    fn field_recipe(&mut self, field: NominalMemberId) -> Result<FieldRecipe, FormationError> {
        self.work((usize::BITS - self.struct_plan.fields.len().leading_zeros()) as usize)?;
        let index = self
            .struct_plan
            .fields
            .binary_search_by_key(&field.index(), |(identity, _)| *identity)
            .map_err(|_| self.error(Span::default(), "missing value-struct field"))?;
        let recipe = self.struct_plan.fields[index].1;
        if !self.program.structs[recipe.schema]
            .type_parameters
            .is_empty()
        {
            return Err(self.error(Span::default(), "instantiated value-struct field schema"));
        }
        Ok(recipe)
    }

    pub(super) fn struct_field_type(
        &mut self,
        field: NominalMemberId,
    ) -> Result<TypeId, FormationError> {
        let recipe = self.field_recipe(field)?;
        let start = self.program.structs[recipe.schema].fields.start;
        Ok(self.program.fields[start + recipe.slot].ty)
    }

    pub(super) fn field_path(
        &mut self,
        context: ContextId,
        mut place: PlaceId,
    ) -> Result<(PlaceId, Vec<FieldRecipe>), FormationError> {
        let mut path = self.budget.vector(AllocationClass::Scratch, 0)?;
        while let Place::Field { base, field } = self.data(context).places[place.index()] {
            self.work(1)?;
            let recipe = self.field_recipe(field)?;
            self.budget
                .push(AllocationClass::Scratch, &mut path, recipe)?;
            place = base;
        }
        Ok((place, path))
    }

    /// Borrow original source edges across inline reference parameters. The
    /// resulting temporary path owns no alias identity or captured value.
    pub(super) fn resolved_field_path(
        &mut self,
        mut context: ContextId,
        mut place: PlaceId,
    ) -> Result<(ContextId, PlaceId, Vec<FieldRecipe>), FormationError> {
        let mut path = self.budget.vector(AllocationClass::Scratch, 0)?;
        loop {
            self.work(1)?;
            match self.data(context).places[place.index()] {
                Place::Field { base, field } => {
                    let recipe = self.field_recipe(field)?;
                    self.budget
                        .push(AllocationClass::Scratch, &mut path, recipe)?;
                    place = base;
                }
                Place::Cell(cell) if references::is_reference(self.program, cell) => {
                    let Some(actual) = self.inline_reference_actual(context, cell)? else {
                        break;
                    };
                    let CallArgument::Reference(actual_place) = *actual.argument else {
                        return Err(
                            self.error(Span::default(), "inline reference actual is a value")
                        );
                    };
                    context = actual.caller;
                    place = actual_place;
                }
                _ => break,
            }
        }
        Ok((context, place, path))
    }

    pub(super) fn slot(
        &mut self,
        object: js::ExprId,
        slot: usize,
    ) -> Result<js::ExprId, FormationError> {
        let key = self.literal(js::Literal::Number(slot as f64))?;
        self.expression(js::Expr::Member {
            object,
            property: js::Property::Computed(key),
        })
    }

    /// One immutable fixed product constructor shared by ordinary stores and
    /// reference schema recipes. No mutable prototype method is consulted.
    pub(super) fn product(
        &mut self,
        fields: Vec<js::ExprId>,
    ) -> Result<js::ExprId, FormationError> {
        self.expression(js::Expr::Array(fields))
    }

    pub(super) fn value_field(
        &mut self,
        context: ContextId,
        place: PlaceId,
    ) -> Result<js::ExprId, FormationError> {
        let (context, root, path) = self.resolved_field_path(context, place)?;
        let component = if let Some(top) = path.last() {
            self.product_place_component(context, root, top.slot as u32)?
        } else {
            None
        };
        let (mut value, remaining) = match component {
            Some(component) => (component, &path[..path.len() - 1]),
            None => (self.place(context, root)?, path.as_slice()),
        };
        for recipe in remaining.iter().rev() {
            self.work(1)?;
            value = self.slot(value, recipe.slot)?;
        }
        self.drop_scratch(path)?;
        Ok(value)
    }

    /// RHS scheduling has already captured any non-inert value. Read current
    /// ancestors only now: a callback in the RHS may have replaced the root or
    /// its siblings. A compound assignment's old leaf remains its own ValueId.
    pub(super) fn store_value_field(
        &mut self,
        context: ContextId,
        place: PlaceId,
        rhs: ValueId,
        span: Span,
    ) -> Result<js::ExprId, FormationError> {
        let value = self.value(context, rhs)?;
        self.store_field_expression(context, place, value, span)
    }

    pub(super) fn store_field_expression(
        &mut self,
        context: ContextId,
        place: PlaceId,
        replacement: js::ExprId,
        span: Span,
    ) -> Result<js::ExprId, FormationError> {
        let (context, root, path) = self.resolved_field_path(context, place)?;
        let cell = match self.data(context).places[root.index()] {
            Place::Cell(cell) => cell,
            Place::Member { .. } | Place::Index { .. } if !path.is_empty() => {
                return self.store_aggregate_field(context, root, path, replacement);
            }
            _ => {
                return Err(
                    self.error(span, "mutable struct projection through aggregate storage")
                )
            }
        };
        if path.is_empty() {
            self.drop_scratch(path)?;
            return self.store_cell(context, cell, replacement);
        }
        self.product_lookup()?;
        if self.demand.product_for_cell(cell).is_some() {
            return self.store_product_field(context, cell, path, replacement, span);
        }
        // A source refinement can have become stale during reentry. Access
        // every current parent independently of how many siblings survive;
        // reconstructing a one-field product must not resurrect null storage.
        let mut checked_place = self.cell(context, cell)?;
        for recipe in path.iter().rev() {
            self.work(1)?;
            checked_place = self.slot(checked_place, recipe.slot)?;
        }
        let mut replacement = replacement;
        for (depth, recipe) in path.iter().enumerate() {
            self.work(1)?;
            let len = self.program.structs[recipe.schema].fields.len();
            let mut values = self.budget.vector(AllocationClass::Retained, len)?;
            for slot in 0..len {
                self.work(1)?;
                let value = if slot == recipe.slot {
                    replacement
                } else {
                    // The target owns expression occurrences, not a DAG of
                    // executable nodes. Rematerialize inert lexical/slot reads
                    // for each output occurrence; never share an ExprId.
                    let mut parent = self.cell(context, cell)?;
                    for ancestor in path[depth + 1..].iter().rev() {
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
        let store = self.store_cell(context, cell, replacement)?;
        let mut sequence = self.budget.vector(AllocationClass::Retained, 2)?;
        self.append(&mut sequence, checked_place)?;
        self.append(&mut sequence, store)?;
        self.expression(js::Expr::Sequence(sequence))
    }

    /// A field update through an array element, record entry or class field:
    /// rebuild the stored product from its current value and store it back.
    /// Placement captured the root's receiver and key, so every re-read of
    /// the root is inert. As for a cell root, every current parent is read
    /// first, so a stale refinement fails before anything is stored.
    fn store_aggregate_field(
        &mut self,
        context: ContextId,
        root: PlaceId,
        path: Vec<FieldRecipe>,
        replacement: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let mut checked_place = self.place(context, root)?;
        for recipe in path.iter().rev() {
            self.work(1)?;
            checked_place = self.slot(checked_place, recipe.slot)?;
        }
        let mut replacement = replacement;
        for (depth, recipe) in path.iter().enumerate() {
            self.work(1)?;
            let len = self.program.structs[recipe.schema].fields.len();
            let mut values = self.budget.vector(AllocationClass::Retained, len)?;
            for slot in 0..len {
                self.work(1)?;
                let value = if slot == recipe.slot {
                    replacement
                } else {
                    let mut parent = self.place(context, root)?;
                    for ancestor in path[depth + 1..].iter().rev() {
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
        let target = self.place(context, root)?;
        let store = self.expression(js::Expr::Assign {
            target,
            value: replacement,
        })?;
        let mut sequence = self.budget.vector(AllocationClass::Retained, 2)?;
        self.append(&mut sequence, checked_place)?;
        self.append(&mut sequence, store)?;
        self.expression(js::Expr::Sequence(sequence))
    }
}
