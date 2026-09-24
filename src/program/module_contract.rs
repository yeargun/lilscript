//! Target-independent static initialization scheduling. Module interfaces own
//! the graph; this adapter shares the source module owner's iterative walk.
use super::{ModuleId, ModuleInterface, UnitId};
use crate::module::StaticOrderError;
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};

pub(super) fn initialization_order(
    modules: &[ModuleInterface],
    entry: ModuleId,
) -> Result<Vec<UnitId>, String> {
    initialization_order_admitted(modules, entry, &mut AllocationBudget::new(None)).map_err(
        |error| match error {
            StaticOrderError::Invalid(reason) => reason.to_owned(),
            StaticOrderError::Resources(error) => error.to_string(),
        },
    )
}

pub(super) fn initialization_order_admitted(
    modules: &[ModuleInterface],
    entry: ModuleId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<UnitId>, StaticOrderError> {
    let mut initializers = budget.vector(Scratch, modules.len())?;
    let mut scope = budget.scope();
    let order = crate::module::initialization_order_admitted(
        entry.index(),
        modules.len(),
        |module| modules[module].dependencies.iter().map(|id| id.index()),
        &mut scope,
    )?;
    // A module the static order leaves out must be one `import()` loads.
    let reachable = crate::module::static_evaluation_order_admitted(
        entry.index(),
        modules.len(),
        |module| {
            modules[module]
                .dependencies
                .iter()
                .chain(&modules[module].dynamic_dependencies)
                .map(|id| id.index())
        },
        &mut scope,
    )?;
    if reachable.len() != modules.len() {
        return Err(StaticOrderError::Invalid(
            "semantic module interface is unreachable from the entry",
        ));
    }
    scope.work(
        crate::compilation_policy::WorkKind::Analysis,
        u64::try_from(order.len()).map_err(|_| AllocationError::Capacity)?,
    )?;
    for module in order {
        initializers.push(modules[module].initializer);
    }
    Ok(initializers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::*;
    use std::sync::Arc;

    fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        program.verify().unwrap();
        inspect(&program);
    }

    fn cyclic_interfaces<'src>(source: &Program<'src>) -> Program<'src> {
        let mut program = source.clone();
        let root = program.entry;
        let initializer = UnitId::from_index(program.units.len()).unwrap();
        let module = ModuleId::from_index(program.modules.len()).unwrap();
        let mut data = UnitData::empty(UnitKind::ModuleInitialization);
        data.module = module;
        program
            .units
            .push(WorkingUnit::new(initializer, data).freeze());
        let arena = bumpalo::Bump::new();
        let empty = crate::parse_source(&arena, "").unwrap();
        let value = program.exports()[0].target;
        let name = program.exports()[0].name.clone();
        let start = program.exports.len();
        Arc::make_mut(&mut program.exports).push(Export {
            name: name.clone(),
            target: value,
        });
        Arc::make_mut(&mut program.modules)[root.index()]
            .dependencies
            .push(module);
        Arc::make_mut(&mut program.modules).push(ModuleInterface {
            source: empty.source_identity().clone(),
            initializer,
            dependencies: vec![root],
            dynamic_dependencies: Vec::new(),
            namespace: Vec::new(),
            imports: vec![ModuleImport {
                module: root,
                name,
                target: value,
                span: Span::default(),
            }],
            exports: start..start + 1,
            foreign_imports: Vec::new(),
        });
        program.initialization = Arc::new(initialization_order(&program.modules, root).unwrap());
        program.verify().unwrap();
        program
    }

    fn rejects<'src>(
        program: &Program<'src>,
        change: impl FnOnce(&mut Program<'src>),
        fragment: &str,
    ) {
        let mut changed = program.clone();
        change(&mut changed);
        let error = changed
            .verify()
            .expect_err("edited interface must be rejected");
        assert!(error.contains(fragment), "{error}");
        program.verify().unwrap();
    }

    #[test]
    fn cyclic_aliases_share_cells_but_names_and_exports_belong_to_modules() {
        checked("export int value=7;export int other=9;", |source| {
            let program = cyclic_interfaces(source);
            assert_eq!(program.initialization.len(), 2);
            assert_eq!(
                program.initialization[1],
                program.modules[program.entry.index()].initializer
            );
            assert_eq!(program.exports().len(), 2);
            assert_eq!(program.exports.len(), 3);
            assert_eq!(program.exports[0].target, program.exports[2].target);
            let mut dependent_entry = program.clone();
            dependent_entry.entry = ModuleId::from_index(1).unwrap();
            dependent_entry.initialization = Arc::new(
                initialization_order(&dependent_entry.modules, dependent_entry.entry).unwrap(),
            );
            dependent_entry.verify().unwrap();
            assert_eq!(dependent_entry.exports().len(), 1);
        });
    }

    #[test]
    fn forged_imports_export_ownership_and_initialization_orders_are_rejected() {
        checked("export int value=7;export int other=9;", |source| {
            let program = cyclic_interfaces(source);
            rejects(
                &program,
                |p| Arc::make_mut(&mut p.modules)[1].imports[0].target = p.exports[1].target,
                "canonical export",
            );
            rejects(
                &program,
                |p| Arc::make_mut(&mut p.modules)[1].imports[0].name = "missing".into(),
                "canonical export",
            );
            rejects(
                &program,
                |p| Arc::make_mut(&mut p.modules)[1].dependencies.clear(),
                "canonical export",
            );
            rejects(
                &program,
                |p| Arc::make_mut(&mut p.modules)[1].exports = 0..1,
                "overlapping",
            );
            rejects(
                &program,
                |p| Arc::make_mut(&mut p.modules)[1].exports = 2..2,
                "no module owner",
            );
            rejects(
                &program,
                |p| Arc::make_mut(&mut p.exports)[1].name = p.exports[0].name.clone(),
                "duplicate public export within",
            );
            rejects(
                &program,
                |p| Arc::make_mut(&mut p.initialization).reverse(),
                "schedule differs",
            );
            rejects(
                &program,
                |p| Arc::make_mut(&mut p.modules)[0].dependencies.clear(),
                "unreachable",
            );
            rejects(
                &program,
                |p| p.entry = ModuleId::from_index(99).unwrap(),
                "entry module",
            );
        });
    }

    #[test]
    fn module_origins_and_callable_source_ownership_are_verified() {
        checked("export int answer(){return 7;}print(answer());", |source| {
            let mut program = cyclic_interfaces(source);
            let function = program
                .cells
                .iter()
                .find_map(|cell| match cell.binding {
                    CellBinding::Function(body) => Some(body),
                    _ => None,
                })
                .unwrap();
            let mut working = program.units[function.index()].clone().into_working();
            working.get_mut().module = ModuleId::from_index(1).unwrap();
            program.units[function.index()] = working.freeze();
            assert!(program
                .verify()
                .unwrap_err()
                .contains("creation name or kind"));
            let arena = bumpalo::Bump::new();
            let empty = crate::parse_source(&arena, "").unwrap();
            rejects(
                source,
                |p| Arc::make_mut(&mut p.modules)[0].source = empty.source_identity().clone(),
                "origin is outside",
            );
        });
    }

    #[test]
    fn named_prefix_values_cannot_escape_into_calls_or_region_results() {
        checked("export int answer(){return 7;}print(answer());", |source| {
            let root = source.modules[source.entry.index()].initializer;
            let unit = source.unit(root).unwrap();
            let creation = unit.regions[unit.entry.index()].operations[0];
            let value = unit.operations[creation.index()].result.unwrap();
            rejects(
                source,
                |p| {
                    let mut unit = p.units[root.index()].clone().into_working();
                    let data = unit.get_mut();
                    let call = data
                        .calls
                        .iter_mut()
                        .find(|call| matches!(call.target, CallTarget::Value { .. }))
                        .unwrap();
                    if let CallTarget::Value { callee, .. } = &mut call.target {
                        *callee = value;
                    }
                    p.units[root.index()] = unit.freeze();
                },
                "escapes its initialization pair",
            );
            rejects(
                source,
                |p| {
                    let mut unit = p.units[root.index()].clone().into_working();
                    let data = unit.get_mut();
                    data.regions[data.entry.index()].result = Some(value);
                    p.units[root.index()] = unit.freeze();
                },
                "escapes through a region result",
            );
            rejects(
                source,
                |p| {
                    let mut unit = p.units[root.index()].clone().into_working();
                    unit.get_mut().instantiation_prefix = 0;
                    p.units[root.index()] = unit.freeze();
                },
                "does not cover",
            );
        });
    }

    #[test]
    fn creator_location_and_source_imports_do_not_restrict_valid_semantic_reuse() {
        checked(
            "export int answer(){return 7;}int privateValue=9;",
            |source| {
                let mut program = cyclic_interfaces(source);
                let root = program.modules[0].initializer;
                let other = program.modules[1].initializer;
                let creation = program.unit(root).unwrap().regions[0].operations[0];
                let mut copied = program.unit(root).unwrap().operations[creation.index()].clone();
                let original_value = copied.result.unwrap();
                let ty = program.unit(root).unwrap().values[original_value.index()].ty;
                let private = program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "privateValue")
                    .unwrap();
                let private = CellId::from_index(private).unwrap();
                let mut working = program.units[other.index()].clone().into_working();
                let data = working.get_mut();
                copied.result = Some(ValueId::from_index(0).unwrap());
                copied.region = data.entry;
                copied.operands = OperandRange { start: 0, len: 0 };
                copied.origin = None;
                data.operations.push(copied);
                data.values.push(Value {
                    ty,
                    definition: OpId::from_index(0).unwrap(),
                });
                data.places.push(Place::Cell(private));
                data.captures.push(private);
                data.operations.push(Operation {
                    kind: OperationKind::Load(PlaceId::from_index(0).unwrap()),
                    operands: OperandRange { start: 0, len: 0 },
                    result: Some(ValueId::from_index(1).unwrap()),
                    region: data.entry,
                    origin: None,
                    span: Span::default(),
                });
                data.values.push(Value {
                    ty: program.cells[private.index()].ty,
                    definition: OpId::from_index(1).unwrap(),
                });
                data.regions[data.entry.index()]
                    .operations
                    .extend([OpId::from_index(0).unwrap(), OpId::from_index(1).unwrap()]);
                program.units[other.index()] = working.freeze();
                // The reused function still owns module zero's original nodes.
                // Its new creator and transformed private-cell load are in module
                // one, with the ordinary semantic capture signature preserved.
                program.verify().unwrap();
                source.verify().unwrap();
            },
        );
    }
}
