//! Target capabilities are checked while original declarations and spans are
//! available. Layout admission still validates physical recipes; it cannot turn
//! a missing language capability into a late, spanless code-generation error.
use super::type_payload::{measure_payload, Payload, PayloadError};
use super::*;
use crate::compilation_policy::WorkKind;

fn check_type(
    view: CheckedView<'_, '_, '_>,
    ty: &Type<'_>,
    span: Span,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AdmittedCheckError> {
    let result = measure_payload(Payload::Type(ty), budget, |node| {
        let Payload::Type(ty) = node else {
            return Ok(());
        };
        let reason = crate::catalog::native_type_capability(ty).or_else(|| match ty {
            Type::Class(declaration) | Type::ClassInstance { declaration, .. }
                if view
                    .nominal_class(declaration.identity)
                    .is_some_and(|class| class.external) =>
            {
                Some("extern classes require a JavaScript target")
            }
            _ => None,
        });
        match reason {
            Some(reason) => Err(CheckError::new(span, reason)),
            None => Ok(()),
        }
    });
    match result {
        Ok(_) => Ok(()),
        Err(PayloadError::Visitor(error)) => Err(error.into()),
        Err(PayloadError::Allocation(error)) => Err(error.into()),
    }
}

/// Called only when native is requested, before lowering or optional search.
/// Module ownership qualifies symbols; source-node IDs qualify occurrences.
pub(crate) fn native(
    source: &Program<'_, '_>,
    view: CheckedView<'_, '_, '_>,
    module: Option<usize>,
    exports: &[ModuleExport<'_>],
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AdmittedCheckError> {
    let mut scope = budget.scope();
    let budget = &mut scope;
    let mut seen = budget.filled(AllocationClass::Scratch, view.type_count(), false)?;
    for symbol in view.symbols() {
        budget.work(WorkKind::Analysis, 1)?;
        if view.declarations.symbol_modules[symbol.id.0 as usize] == module {
            check_type(view, &symbol.ty, symbol.span, budget)?;
        }
    }
    for definition in view.structs() {
        if definition.module != module {
            continue;
        }
        for field in definition.fields.values() {
            check_type(view, &field.ty, field.span, budget)?;
        }
    }
    for definition in view.classes() {
        if definition.module != module {
            continue;
        }
        for field in definition.fields.values() {
            check_type(view, &field.ty, field.span, budget)?;
        }
    }
    for info in &view.facts.source_info {
        budget.work(WorkKind::Analysis, 1)?;
        let Some(expression) = info.expression else {
            continue;
        };
        if let Some(id) = view.expression_type_id(expression.id) {
            if !seen[id.index()] {
                check_type(view, view.checked_type(id), expression.span(), budget)?;
                seen[id.index()] = true;
            }
        }
    }
    let mut failure = Ok(());
    crate::ast_walk::each_statement(source, &mut |statement| {
        if failure.is_err() {
            return;
        }
        failure = budget
            .work(WorkKind::Analysis, 1)
            .map_err(AdmittedCheckError::from);
        if failure.is_err() {
            return;
        }
        let unsupported = match statement {
            Stmt::Try { span, .. } | Stmt::Throw { span, .. } => {
                Some((*span, "native exceptions are not implemented yet"))
            }
            Stmt::Yield { span, .. } => Some((*span, "native generators are not implemented yet")),
            _ => None,
        };
        if let Some((span, message)) = unsupported {
            failure = Err(AdmittedCheckError::new(span, message));
        }
    });
    failure?;
    for export in exports {
        budget.work(WorkKind::Analysis, 1)?;
        if matches!(export.target, InterfaceTarget::Value(_)) {
            return Err(AdmittedCheckError::new(
                export.span,
                "native exported ABI is not implemented yet",
            ));
        }
    }
    Ok(())
}

/// Enforce the selected source contract before any lowering or optimization.
/// Its field proof is also consumed by the compatibility lint; no second
/// syntactic assignment recognizer decides whether a default is observed.
pub(crate) fn language(
    view: CheckedView<'_, '_, '_>,
    module: Option<usize>,
    contract: crate::config::LanguageConfig,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AdmittedCheckError> {
    if contract.field_initialization == crate::config::FieldInitialization::Legacy {
        return Ok(());
    }
    for class in view.classes() {
        budget.work(WorkKind::Analysis, 1)?;
        if class.module != module || class.external {
            continue;
        }
        if let Some(span) = class.initialization.before_super {
            return Err(AdmittedCheckError::new(
                span,
                "`this` is used before its base constructor initializes it (R3)",
            ));
        }
        if let Some(&member) = class.initialization.implicit.first() {
            let Some(NominalMember::Field { field, .. }) = view.nominal_member(member) else {
                unreachable!("constructor flow records checked field identities");
            };
            return Err(AdmittedCheckError::new(field.span,format!(
                "field `{}` is not initialized before every observation or normal constructor completion (R3); assign it on every path before reading `this`, or declare an explicit initializer",
                field.name,
            )));
        }
    }
    Ok(())
}
