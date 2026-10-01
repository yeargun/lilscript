//! Authored expansion uses the checked source identities, with a fresh physical
//! cell bank for every occurrence. Nested closures capture that bank; source
//! names/locations remain provenance, never storage identity.
use super::*;

impl<'budget, 'ledger, 'sem, 'ast, 'src> Lower<'budget, 'ledger, 'sem, 'ast, 'src> {
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
            return self.unsupported(span, "inline for conflicts with policy.tactics.loop-unrolling='off'");
        }
        self.program.authored_unrolling = true;
        let ExprKind::ArrayLiteral { elements, .. } = &iterable.kind else {
            return self.unsupported(iterable.span(), "inline for requires a checked constant array literal");
        };
        let symbols = self.semantics.symbols().len();
        if self.cell_aliases.is_empty() {
            self.cell_aliases = self.budget.vector(Scratch, symbols)?;
            self.work(symbols)?;
            for index in 0..symbols {
                self.cell_aliases.push(CellId::from_index(index).ok_or(AllocationError::Capacity)?);
            }
        }
        let element_source = self.semantics.identifier_symbol(element.id).ok_or(Unsupported {
            span: element.span, feature: "inline for element identity",
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
                self.budget.push(Scratch, &mut bindings,
                    (index, self.cell_aliases[index]))?;
            }
        }
        for item in *elements {
            self.work(1)?;
            let ast::ArrayElement::Value(value) = item else {
                return self.unsupported(iterable.span(), "inline for requires scalar literal elements");
            };
            if !value.is_const_scalar() {
                return self.unsupported(value.span(), "inline for requires scalar literal elements");
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
                let id = CellId::from_index(self.program.cells.len()).ok_or(AllocationError::Capacity)?;
                self.budget.push(Retained, building_table(&mut self.program.cells), clone)?;
                self.cell_aliases[source] = id;
            }
            let item = self.expression(unit, iteration, value)?;
            let cell = self.declare(unit, iteration, element)?;
            self.effect(unit, iteration, OperationKind::Initialize(cell), &[item], value.span())?;
            if let Stmt::Block { body, .. } = body {
                self.statements(unit, iteration, body)?;
            } else {
                self.statement(unit, iteration, body)?;
            }
            self.effect(unit, region, OperationKind::Block(iteration), &[], span)?;
        }
        self.work(bindings.len())?;
        for &(source, prior) in &bindings { self.cell_aliases[source] = prior; }
        drop_vector(bindings, Scratch, self.budget)?;
        Ok(())
    }
}
