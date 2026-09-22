//! Fixed imports use the same target construction and source calling recipes.
//! Adapters transfer already frozen raw components; they never apply source
//! Field<Int> normalization, inspect printed text or duplicate a producer body.
use super::*;
use crate::semantic_program::function_layout::ProductTransport;

impl Formation<'_, '_, '_, '_, '_> {
    fn resource_binding(
        &mut self,
        scope: js::ScopeId,
        spelling: &str,
    ) -> Result<js::BindingId, FormationError> {
        let spelling = self.text(spelling)?;
        Ok(self.module.binding_in(
            js::Binding {
                source_symbol: None,
                scope,
                spelling,
                pinned: false,
            },
            self.budget,
        )?)
    }

    pub(super) fn plan_resource_import(&mut self) -> Result<(), FormationError> {
        let Some(export) = self.demand.resource().consumer_import() else {
            return Ok(());
        };
        self.work(1)?;
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let imported = self.resource_binding(scope, "fixed_import")?;
        self.module.import_in(
            export.source_specifier(),
            export.export_name(),
            imported,
            self.budget,
        )?;
        self.work(self.demand.function_lookup_work())?;
        let consumer = self.demand.function_layout(export.body());
        let mut compatible = true;
        for parameter in export.parameters() {
            self.work(1)?;
            let incoming = consumer.map_or(ProductTransport::Packed, |layout| {
                layout.transport(parameter.position())
            });
            compatible &= incoming == parameter.transport();
        }
        if compatible {
            self.imported_binding = Some(imported);
            return Ok(());
        }

        let body = self.module.region_in(scope, self.budget)?;
        let body_scope = self.module.regions[body.index()].scope;
        let mut parameters = self
            .budget
            .vector(AllocationClass::Retained, export.parameters().len())?;
        let mut arguments = self
            .budget
            .vector(AllocationClass::Retained, export.parameters().len())?;
        for parameter in export.parameters() {
            self.work(1)?;
            let incoming = consumer.map_or(ProductTransport::Packed, |layout| {
                layout.transport(parameter.position())
            });
            let width = parameter.fields().len();
            match (incoming, parameter.transport()) {
                (ProductTransport::Packed, ProductTransport::Packed) => {
                    let binding = self.resource_binding(body_scope, "packed")?;
                    self.append(&mut parameters, binding)?;
                    let value = self.reference(binding)?;
                    self.append(&mut arguments, value)?;
                }
                (ProductTransport::Packed, ProductTransport::Fields) => {
                    let binding = self.resource_binding(body_scope, "packed")?;
                    self.append(&mut parameters, binding)?;
                    for slot in 0..width {
                        self.work(1)?;
                        let value = self.reference(binding)?;
                        let value = self.slot(value, slot)?;
                        self.append(&mut arguments, value)?;
                    }
                }
                (ProductTransport::Fields, ProductTransport::Packed) => {
                    let mut fields = self.budget.vector(AllocationClass::Retained, width)?;
                    for _ in 0..width {
                        self.work(1)?;
                        let binding = self.resource_binding(body_scope, "field")?;
                        self.append(&mut parameters, binding)?;
                        let value = self.reference(binding)?;
                        self.append(&mut fields, value)?;
                    }
                    let value = self.product(fields)?;
                    self.append(&mut arguments, value)?;
                }
                (ProductTransport::Fields, ProductTransport::Fields) => {
                    for _ in 0..width {
                        self.work(1)?;
                        let binding = self.resource_binding(body_scope, "field")?;
                        self.append(&mut parameters, binding)?;
                        let value = self.reference(binding)?;
                        self.append(&mut arguments, value)?;
                    }
                }
            }
        }
        let callee = self.reference(imported)?;
        let value = self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })?;
        self.statement(body, js::Statement::Return(Some(value)))?;
        let function = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters,
                body,
                arrow: false,
                // The sealed call interface excludes identity/name/frame escape.
                name: js::FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: crate::structured_js::Suspension::None,
            },
        )?;
        let binding = self.resource_binding(scope, "fixed_adapter")?;
        self.statement(root, js::Statement::Function { binding, function })?;
        self.imported_binding = Some(binding);
        Ok(())
    }
}
