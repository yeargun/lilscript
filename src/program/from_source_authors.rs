//! Authored expansion uses the checked source identities, with a fresh physical
//! cell bank for every occurrence. Nested closures capture that bank; source
//! names/locations remain provenance, never storage identity.
use super::*;

impl<'budget, 'ledger, 'sem, 'ast, 'src> Lower<'budget, 'ledger, 'sem, 'ast, 'src> {
    pub(super) fn enter_choices(
        &mut self,
        unit: UnitId,
        choices: crate::representation::RegionalChoices,
        span: Span,
    ) -> Result<crate::representation::RegionalChoices, ConversionError> {
        if !choices.allowed_by(self.choices_allowed) {
            return self.unsupported(span, "@choose conflicts with resolved tactic permissions; enable its documented tactic and target-compaction in TOML");
        }
        self.program.authored_choices = self.program.authored_choices.requests(choices);
        let data = &mut self.units[unit.index()];
        data.regions[data.entry.index()].authored = choices;
        Ok(std::mem::replace(&mut self.authored, choices))
    }

    pub(super) fn enter_pool(
        &mut self,
        pool: bool,
        span: Span,
    ) -> Result<Option<(ModuleId, Span)>, ConversionError> {
        if pool && !self.pool_allowed {
            return self.unsupported(
                span,
                "@pool conflicts with policy.tactics.string-pooling='off'",
            );
        }
        self.program.authored_pooling |= pool;
        Ok(std::mem::replace(
            &mut self.pool_region,
            pool.then_some((self.current_module, span)),
        ))
    }

    pub(super) fn authored_string(
        &mut self,
        string: StringId,
        span: Span,
    ) -> Result<(), ConversionError> {
        if !self.pool_region.is_some_and(|(module, region)| {
            module == self.current_module && region.start <= span.start && span.end <= region.end
        }) {
            return Ok(());
        }
        let values = &mut building_table(&mut self.program.modules)[self.current_module.index()]
            .pooled_strings;
        self.budget.work(
            WorkKind::Analysis,
            u64::from(usize::BITS - values.len().leading_zeros()) + 1,
        )?;
        if let Err(index) = values.binary_search_by_key(&string.index(), |id| id.index()) {
            self.budget
                .work(WorkKind::Analysis, (values.len() - index) as u64)?;
            self.budget.reserve_vec(Retained, values, 1)?;
            values.insert(index, string);
        }
        Ok(())
    }

    pub(super) fn inline_for(
        &mut self,
        unit: UnitId,
        region: RegionId,
        element: ast::Ident<'src>,
        iterable: &ast::Expr<'ast, 'src>,
        body: &Stmt<'ast, 'src>,
        span: Span,
    ) -> Result<(), ConversionError> {
        if !self.unroll {
            return self.unsupported(
                span,
                "inline for conflicts with policy.tactics.loop-unrolling='off'",
            );
        }
        self.program.authored_unrolling = true;
        let ExprKind::ArrayLiteral { elements, .. } = &iterable.kind else {
            return self.unsupported(
                iterable.span(),
                "inline for requires a checked constant array literal",
            );
        };
        let symbols = self.semantics.symbols().len();
        if self.cell_aliases.is_empty() {
            self.cell_aliases = self.budget.vector(Scratch, symbols)?;
            self.work(symbols)?;
            for index in 0..symbols {
                self.cell_aliases
                    .push(CellId::from_index(index).ok_or(AllocationError::Capacity)?);
            }
        }
        let element_source = self
            .semantics
            .identifier_symbol(element.id)
            .ok_or(Unsupported {
                span: element.span,
                feature: "inline for element identity",
            })?;
        let mut bindings = self.budget.vector(Scratch, 1)?;
        self.work(symbols)?;
        let scope = body.span();
        // Checked declarations, not identifier mentions. Outer captures keep
        // their storage; nested arrow parameters and local declarations clone.
        for index in 0..symbols {
            let cell = &self.program.cells[index];
            if self.units[cell.owner.index()].module == self.current_module
                && (index == element_source.0 as usize
                    || cell.declaration.start >= scope.start && cell.declaration.end <= scope.end)
            {
                self.budget
                    .push(Scratch, &mut bindings, (index, self.cell_aliases[index]))?;
            }
        }
        for item in *elements {
            self.work(1)?;
            let ast::ArrayElement::Value(value) = item else {
                return self.unsupported(
                    iterable.span(),
                    "inline for requires scalar literal elements",
                );
            };
            if !value.is_const_scalar() {
                return self
                    .unsupported(value.span(), "inline for requires scalar literal elements");
            }
            let iteration = self.region(unit, region, span)?;
            for &(source, prior) in &bindings {
                self.work(1)?;
                let original = &self.program.cells[prior.index()];
                let name = self.budget.string(Retained, &original.name)?;
                let clone = Cell {
                    source_symbol: original.source_symbol,
                    name,
                    ty: original.ty,
                    owner: unit,
                    region: iteration,
                    declaration: original.declaration,
                    reassigned: original.reassigned,
                    observable_before_initialization: original.observable_before_initialization,
                    binding: CellBinding::Local,
                    synthetic: true,
                    declared_pure: original.declared_pure,
                    debug: original.debug,
                };
                let id = CellId::from_index(self.program.cells.len())
                    .ok_or(AllocationError::Capacity)?;
                self.budget
                    .push(Retained, building_table(&mut self.program.cells), clone)?;
                self.cell_aliases[source] = id;
            }
            let item = self.expression(unit, iteration, value)?;
            let cell = self.declare(unit, iteration, element)?;
            self.effect(
                unit,
                iteration,
                OperationKind::Initialize(cell),
                &[item],
                value.span(),
            )?;
            if let Stmt::Block { body, .. } = body {
                self.statements(unit, iteration, body)?;
            } else {
                self.statement(unit, iteration, body)?;
            }
            self.effect(unit, region, OperationKind::Block(iteration), &[], span)?;
        }
        self.work(bindings.len())?;
        for &(source, prior) in &bindings {
            self.cell_aliases[source] = prior;
        }
        drop_vector(bindings, Scratch, self.budget)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pooling_names_authored_literals_not_borrowed_field_defaults() {
        let arena = bumpalo::Bump::new();
        let syntax=crate::parse_source(&arena,"class Box{string value=\"elsewhere\";}@pool string f(){Box box=new Box();return box.value+\"here\";}print(f());").unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &checked).unwrap();
        let pooled = program
            .modules
            .iter()
            .flat_map(|module| module.pooled_strings.iter())
            .map(|id| program.strings[id.index()].as_unicode().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(pooled, ["here"]);
    }
}
