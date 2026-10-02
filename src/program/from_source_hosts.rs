//! Bindings resolve in the checker to identities. Conversion selects their ABI
//! paths before verification and semantic edits; no delivered text is rewritten.
use super::*;

pub(super) fn apply(
    program: &mut Program<'_>,
    view: CheckedView<'_, '_, '_>,
    config: &crate::config::HostConfig,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), (ModuleId, ConversionError)> {
    let root = program.entry_module();
    let bindings =
        crate::check::capabilities::javascript_bindings(view, config, budget).map_err(|error| {
            (
                root,
                match error {
                    crate::check::AdmittedCheckError::Semantic(error) => {
                        ConversionError::Contract(ContractViolation {
                            span: error.span,
                            message: error.message,
                        })
                    }
                    crate::check::AdmittedCheckError::Resources(error) => {
                        ConversionError::Resources(error)
                    }
                },
            )
        })?;
    // Resolve each original declaration once: aliases cannot chain through
    // another configured source name or change a later lookup's identity.
    for &(symbol, path) in &bindings.cells {
        let index = symbol.0 as usize;
        let cell = &program.cells[index];
        let owner = program.units[cell.owner.index()].data().module;
        for module in program.modules.iter() {
            budget
                .work(WorkKind::Analysis, module.foreign_imports.len() as u64)
                .map_err(|error| (owner, error.into()))?;
            if module
                .foreign_imports
                .iter()
                .any(|import| import.cell.is_some_and(|cell| cell.index() == index))
            {
                return Err((
                    owner,
                    ConversionError::Contract(ContractViolation {
                        span: cell.declaration,
                        message: format!(
                            "host.javascript.{} conflicts with its import extern binding",
                            cell.name
                        ),
                    }),
                ));
            }
        }
        replace(
            &mut building_table(&mut program.cells)[index].name,
            path,
            budget,
        )
        .map_err(|error| (owner, error.into()))?;
    }
    for &(identity, path) in &bindings.classes {
        budget
            .work(WorkKind::Analysis, 1)
            .map_err(|error| (root, error.into()))?;
        let index = program
            .class_index(identity)
            .expect("checked extern class has its definition");
        let owner = program.classes[index].module;
        replace(
            &mut building_table(&mut program.classes)[index].name,
            path,
            budget,
        )
        .map_err(|error| (owner, error.into()))?;
    }
    drop_vector(bindings.cells, Scratch, budget).map_err(|error| (root, error.into()))?;
    drop_vector(bindings.classes, Scratch, budget).map_err(|error| (root, error.into()))?;
    Ok(())
}
fn replace(
    name: &mut String,
    path: &str,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let selected = budget.string(Retained, path)?;
    let old = std::mem::replace(name, selected);
    let capacity = old.capacity();
    drop(old);
    budget.release(Retained, capacity as u64)
}
