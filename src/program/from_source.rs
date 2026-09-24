use super::*;
use crate::ast::{
    self, ArrayElement, ArrowBody, AssignmentOp, ExprKind, ForInitializer, Item, RecordElement,
    Stmt,
};
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};
use crate::check::{
    CheckedModules, ClassInfo, ExpressionResolution, NominalMember, CheckedModule, CheckedView,
};

/// The frontend owns its tables exclusively until the checked Program is
/// returned. Sharing during construction is an owner bug, not a reason to
/// silently copy a table or expose mutation on a published Program.
fn building_table<T>(table: &mut Arc<Vec<T>>) -> &mut Vec<T> {
    Arc::get_mut(table).expect("frontend owns unpublished semantic tables")
}

fn vector_bytes<T>(values: &Vec<T>) -> Result<u64, AllocationError> {
    values
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(AllocationError::Capacity)
}

fn drop_vector<T>(
    values: Vec<T>,
    class: crate::output_budget::AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let bytes = vector_bytes(&values)?;
    drop(values);
    budget.release(class, bytes)
}

fn table<T>(
    values: Vec<T>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Arc<Vec<T>>, AllocationError> {
    budget.work(WorkKind::Analysis, 1)?;
    budget.retain(
        Retained,
        (std::mem::size_of::<Vec<T>>() + 2 * std::mem::size_of::<usize>()) as u64,
    )?;
    Ok(Arc::new(values))
}

fn empty_unit(
    kind: UnitKind,
    budget: &mut AllocationBudget<'_>,
) -> Result<UnitData, AllocationError> {
    // UnitData::empty owns exactly its singleton entry-region backing.
    budget.work(WorkKind::Analysis, 1)?;
    budget.retain(Retained, std::mem::size_of::<Region>() as u64)?;
    Ok(UnitData::empty(kind))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    pub span: Span,
    pub feature: &'static str,
}

/// A source-owned conversion diagnostic for a checked module set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleUnsupported {
    pub module: crate::module::ModuleId,
    pub error: Unsupported,
}

#[derive(Debug)]
pub(crate) enum ConversionError {
    Unsupported(Unsupported),
    Resources(AllocationError),
}

#[derive(Debug)]
pub(crate) struct ModuleConversionError {
    pub module: crate::module::ModuleId,
    pub error: ConversionError,
}

impl From<Unsupported> for ConversionError {
    fn from(error: Unsupported) -> Self {
        Self::Unsupported(error)
    }
}

impl From<AllocationError> for ConversionError {
    fn from(error: AllocationError) -> Self {
        Self::Resources(error)
    }
}

pub(crate) fn from_checked_source_admitted<'ast, 'src>(
    source: &ast::Program<'ast, 'src>,
    semantics: &CheckedModule<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<publication::PreparedProgram<'src>, ConversionError> {
    let mut scope = budget.scope();
    let program = convert_source(source, semantics, &mut scope)?;
    verify_conversion(&program, source.span, &mut scope)?;
    publication::PreparedProgram::new(program, &mut scope)
        .map_err(|error| publication_conversion_error(error, source.span))
}

pub(crate) fn from_checked_modules_admitted<'ast, 'src>(
    sources: &[ast::Program<'ast, 'src>],
    semantics: &CheckedModules<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<publication::PreparedProgram<'src>, ModuleConversionError> {
    let mut scope = budget.scope();
    let program = convert_modules(sources, semantics, &mut scope)?;
    verify_conversion(&program, sources[semantics.root()].span, &mut scope).map_err(|error| {
        ModuleConversionError {
            module: semantics.root(),
            error,
        }
    })?;
    publication::PreparedProgram::new(program, &mut scope).map_err(|error| ModuleConversionError {
        module: semantics.root(),
        error: publication_conversion_error(error, sources[semantics.root()].span),
    })
}

fn publication_conversion_error(
    error: publication::PublicationError,
    span: Span,
) -> ConversionError {
    use publication::PublicationError;
    match error {
        PublicationError::Budget(error) => AllocationError::Budget(error).into(),
        PublicationError::Capacity => AllocationError::Capacity.into(),
        PublicationError::AllocationFailed => AllocationError::AllocationFailed.into(),
        error => {
            if std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
                eprintln!("semantic publication: {error:?}");
            }
            invalid_conversion(span).into()
        }
    }
}

/// Single-source clients use the same owners and source lowering as modules.
/// Unsupported contracts are visible.
pub fn from_checked_source<'ast, 'src>(
    source: &ast::Program<'ast, 'src>,
    semantics: &CheckedModule<'ast, 'src>,
) -> Result<Program<'src>, Unsupported> {
    let mut budget = AllocationBudget::new(None);
    let result = (|| {
        let program = convert_source(source, semantics, &mut budget)?;
        verify_conversion(&program, source.span, &mut budget)?;
        Ok(program)
    })();
    result.map_err(|error| match error {
        ConversionError::Unsupported(error) => error,
        ConversionError::Resources(error) => {
            if std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
                eprintln!("semantic conversion resources: {error:?}");
            }
            invalid_conversion(source.span)
        }
    })
}

fn invalid_conversion(span: Span) -> Unsupported {
    Unsupported {
        span,
        feature: "invalid semantic conversion",
    }
}

fn verify_conversion(
    program: &Program<'_>,
    span: Span,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), ConversionError> {
    super::verify::verify_admitted(program, budget)
        .map(|_| ())
        .map_err(|error| match error {
            super::verify::VerificationError::Allocation(error) => {
                ConversionError::Resources(error)
            }
            error => {
                // The diagnostic stays a fixed feature name; the verifier's
                // reason is available to compiler developers on request.
                if std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
                    eprintln!("semantic verification: {}", error.into_string(program));
                }
                ConversionError::Unsupported(invalid_conversion(span))
            }
        })
}

fn convert_source<'ast, 'src>(
    source: &ast::Program<'ast, 'src>,
    semantics: &CheckedModule<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Program<'src>, ConversionError> {
    if !semantics.belongs_to(source.source_identity()) {
        return Err(Unsupported {
            span: source.span,
            feature: "checker/source ownership mismatch",
        }
        .into());
    }
    if !source.imports.is_empty()
        || !source.foreign_imports.is_empty()
        || source
            .exports
            .iter()
            .any(|export| export.kind == crate::ast::ExportKind::ConstructorValue)
    {
        return Err(Unsupported {
            span: source.span,
            feature: "module graph conversion",
        }
        .into());
    }
    let module = ModuleId::from_index(0).unwrap();
    let root = UnitId::from_index(0).unwrap();
    let mut lower = Lower::new(
        semantics.view(),
        std::slice::from_ref(source),
        module,
        budget,
    )?;
    lower.add_cells(|_| Some(0))?;
    lower.register_source(source)?;
    lower.emit_source(root, source)?;
    lower.source_exports(module, source)?;
    lower.finish()
}

/// Consumes per-source facts and canonical shared declarations directly.
/// Thin borrowed views select a source during lowering; they own no AST/table copy.
pub fn from_checked_modules<'ast, 'src>(
    sources: &[ast::Program<'ast, 'src>],
    semantics: &CheckedModules<'ast, 'src>,
) -> Result<Program<'src>, ModuleUnsupported> {
    let mut budget = AllocationBudget::new(None);
    let result = (|| {
        let program = convert_modules(sources, semantics, &mut budget)?;
        verify_conversion(&program, sources[semantics.root()].span, &mut budget).map_err(
            |error| ModuleConversionError {
                module: semantics.root(),
                error,
            },
        )?;
        Ok(program)
    })();
    result.map_err(|error: ModuleConversionError| ModuleUnsupported {
        module: error.module,
        error: match error.error {
            ConversionError::Unsupported(error) => error,
            ConversionError::Resources(_) => invalid_conversion(
                sources
                    .get(error.module)
                    .map_or(Span::default(), |source| source.span),
            ),
        },
    })
}

fn convert_modules<'ast, 'src>(
    sources: &[ast::Program<'ast, 'src>],
    semantics: &CheckedModules<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Program<'src>, ModuleConversionError> {
    let fail = |module, feature| ModuleConversionError {
        module,
        error: Unsupported {
            span: sources
                .get(module)
                .map_or(Span::default(), |source| source.span),
            feature,
        }
        .into(),
    };
    if sources.is_empty()
        || sources.len() != semantics.interfaces().len()
        || semantics.root() >= sources.len()
    {
        return Err(fail(
            semantics.root(),
            "checked module/source count or entry mismatch",
        ));
    }
    for (module, source) in sources.iter().enumerate() {
        budget
            .work(WorkKind::Analysis, 1)
            .map_err(|error| ModuleConversionError {
                module,
                error: error.into(),
            })?;
        let view = semantics
            .view(module)
            .ok_or_else(|| fail(module, "missing checked module view"))?;
        if !view.belongs_to(source.source_identity()) {
            return Err(fail(module, "checker/source ownership mismatch"));
        }
    }
    let entry = ModuleId::from_index(semantics.root())
        .ok_or_else(|| fail(semantics.root(), "semantic module capacity"))?;
    let mut lower =
        Lower::new(semantics.view(0).unwrap(), sources, entry, budget).map_err(|error| {
            ModuleConversionError {
                module: semantics.root(),
                error,
            }
        })?;
    let convert_interfaces =
        |lower: &mut Lower<'_, '_, '_, 'ast, 'src>| -> Result<(), ModuleConversionError> {
            for (module, interface) in semantics.interfaces().iter().enumerate() {
                let make_error = |error| ModuleConversionError { module, error };
                lower.work(1).map_err(make_error)?;
                if interface.module != module {
                    return Err(fail(module, "checked module interface identity mismatch"));
                }
                let mut dependencies = lower
                    .budget
                    .vector(Retained, interface.dependencies.len())
                    .map_err(|error| make_error(error.into()))?;
                for &target in &interface.dependencies {
                    lower.work(1).map_err(make_error)?;
                    let target = ModuleId::from_index(target)
                        .filter(|_| target < sources.len())
                        .ok_or_else(|| fail(module, "checked dependency outside module set"))?;
                    lower
                        .budget
                        .push(Retained, &mut dependencies, target)
                        .map_err(|error| make_error(error.into()))?;
                }
                let mut imports = lower
                    .budget
                    .vector(Retained, interface.imports.len())
                    .map_err(|error| make_error(error.into()))?;
                for import in &interface.imports {
                    lower.work(1).map_err(make_error)?;
                    let target_module = ModuleId::from_index(import.module)
                        .filter(|_| import.module < sources.len())
                        .ok_or_else(|| fail(module, "checked import outside module set"))?;
                    let target = interface_target(import.target)
                        .ok_or_else(|| fail(module, "semantic import identity capacity"))?;
                    let name = lower
                        .budget
                        .string(Retained, import.imported)
                        .map_err(|error| make_error(error.into()))?;
                    lower
                        .budget
                        .push(
                            Retained,
                            &mut imports,
                            ModuleImport {
                                module: target_module,
                                name,
                                target,
                                span: import.span,
                            },
                        )
                        .map_err(|error| make_error(error.into()))?;
                }
                let mut dynamic = lower
                    .budget
                    .vector(Retained, interface.dynamic_dependencies.len())
                    .map_err(|error| make_error(error.into()))?;
                for &target in &interface.dynamic_dependencies {
                    lower.work(1).map_err(make_error)?;
                    let target = ModuleId::from_index(target)
                        .filter(|_| target < sources.len())
                        .ok_or_else(|| fail(module, "checked dependency outside module set"))?;
                    lower
                        .budget
                        .push(Retained, &mut dynamic, target)
                        .map_err(|error| make_error(error.into()))?;
                }
                let data = &mut building_table(&mut lower.program.modules)[module];
                data.dependencies = dependencies;
                data.dynamic_dependencies = dynamic;
                data.imports = imports;
            }
            Ok(())
        };
    convert_interfaces(&mut lower)?;
    let order =
        module_contract::initialization_order_admitted(&lower.program.modules, entry, lower.budget)
            .map_err(|error| match error {
                crate::module::StaticOrderError::Invalid(_) => fail(
                    semantics.root(),
                    "invalid checked module initialization graph",
                ),
                crate::module::StaticOrderError::Resources(error) => ModuleConversionError {
                    module: semantics.root(),
                    error: error.into(),
                },
            })?;
    lower
        .work(order.len())
        .map_err(|error| ModuleConversionError {
            module: semantics.root(),
            error,
        })?;
    if order
        .iter()
        .map(|unit| unit.index())
        .ne(semantics.initialization_order().iter().copied())
    {
        return Err(fail(
            semantics.root(),
            "checker/module initialization schedule mismatch",
        ));
    }
    let initialization = building_table(&mut lower.program.initialization);
    initialization.clear();
    lower
        .budget
        .extend_copy(Retained, initialization, &order)
        .map_err(|error| ModuleConversionError {
            module: semantics.root(),
            error: error.into(),
        })?;
    drop_vector(order, Scratch, lower.budget).map_err(|error| ModuleConversionError {
        module: semantics.root(),
        error: error.into(),
    })?;
    lower
        .add_cells(|symbol| semantics.symbol_module(symbol))
        .map_err(|error| ModuleConversionError {
            module: semantics.root(),
            error,
        })?;
    // A namespace serves exactly the exports some module reads from it.
    for module in 0..sources.len() {
        let view = semantics.view(module).unwrap();
        for (target, name) in view.used_dynamic_exports() {
            lower.work(1).map_err(|error| ModuleConversionError { module, error })?;
            let target = target as usize;
            let cell = semantics
                .interfaces()
                .get(target)
                .and_then(|interface| {
                    interface
                        .exports
                        .iter()
                        .find(|export| export.external == name)
                })
                .and_then(|export| interface_target(export.target))
                .and_then(|target| match target {
                    InterfaceTarget::Value(cell) => Some(cell),
                    InterfaceTarget::Struct(_) => None,
                })
                .ok_or_else(|| fail(module, "dynamic export has no runtime cell"))?;
            let name = lower
                .budget
                .string(Retained, name)
                .map_err(|error| ModuleConversionError {
                    module,
                    error: error.into(),
                })?;
            let namespace = &mut building_table(&mut lower.program.modules)[target].namespace;
            if !namespace.iter().any(|(known, _)| *known == name) {
                lower
                    .budget
                    .push(Retained, namespace, (name, cell))
                    .map_err(|error| ModuleConversionError {
                        module,
                        error: error.into(),
                    })?;
            }
        }
    }
    for module in building_table(&mut lower.program.modules) {
        module.namespace.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    }
    // Register all canonical callable identities before lowering cyclic bodies.
    for (module, source) in sources.iter().enumerate() {
        lower.semantics = semantics.view(module).unwrap();
        lower.current_module = ModuleId::from_index(module).unwrap();
        lower
            .register_source(source)
            .map_err(|error| ModuleConversionError { module, error })?;
    }
    for (module, source) in sources.iter().enumerate() {
        lower.semantics = semantics.view(module).unwrap();
        lower.current_module = ModuleId::from_index(module).unwrap();
        lower
            .emit_source(UnitId::from_index(module).unwrap(), source)
            .map_err(|error| ModuleConversionError { module, error })?;
        lower
            .foreign_imports(module, source, &semantics.interfaces()[module].foreign_sources)
            .map_err(|error| ModuleConversionError { module, error })?;
        let start = lower.program.exports.len();
        for export in &semantics.interfaces()[module].exports {
            let target = interface_target(export.target)
                .ok_or_else(|| fail(module, "checked export identity capacity"))?;
            lower
                .add_export(export.external, target)
                .map_err(|error| ModuleConversionError { module, error })?;
        }
        building_table(&mut lower.program.modules)[module].exports =
            start..lower.program.exports.len();
    }
    lower.finish().map_err(|error| ModuleConversionError {
        module: semantics.root(),
        error,
    })
}

fn interface_target(target: crate::check::InterfaceTarget) -> Option<InterfaceTarget> {
    Some(match target {
        crate::check::InterfaceTarget::Value(symbol) => {
            InterfaceTarget::Value(CellId::from_index(symbol.0 as usize)?)
        }
        crate::check::InterfaceTarget::Struct(identity) => InterfaceTarget::Struct(identity),
    })
}

struct Lower<'budget, 'ledger, 'sem, 'ast, 'src> {
    semantics: CheckedView<'sem, 'ast, 'src>,
    current_module: ModuleId,
    program: Program<'src>,
    units: Vec<UnitData>,
    allocations: Vec<usize>,
    /// Each converted class body: a method or `init` function unit.
    class_methods: Vec<ClassMethod<'src>>,
    /// The class and `this` cell of the `init` being converted, for `super`.
    current_class: Option<(&'src str, CellId)>,
    budget: &'budget mut AllocationBudget<'ledger>,
}

/// The receiver of a class function call: a value the caller computed first,
/// or the instance a construction creates after its explicit arguments.
#[derive(Clone, Copy)]
enum CallReceiver<'src> {
    Value(ValueId),
    Construction { class: &'src str, ty: TypeId, origin: Option<SourceNodeId> },
}

/// A class method or `init` (`name: None`), converted to a function unit that
/// takes the instance as its first parameter and reached through a synthetic
/// function cell. Dispatch is static: overriding is rejected by the checker.
#[derive(Clone, Copy)]
struct ClassMethod<'src> {
    class: &'src str,
    name: Option<&'src str>,
    unit: UnitId,
    cell: CellId,
}

impl<'budget, 'ledger, 'sem, 'ast, 'src> Lower<'budget, 'ledger, 'sem, 'ast, 'src> {
    fn new(
        semantics: CheckedView<'sem, 'ast, 'src>,
        sources: &[ast::Program<'ast, 'src>],
        entry: ModuleId,
        budget: &'budget mut AllocationBudget<'ledger>,
    ) -> Result<Self, ConversionError> {
        let mut units = budget.vector(Scratch, sources.len())?;
        let mut modules = budget.vector(Retained, sources.len())?;
        let mut initialization = budget.vector(Retained, sources.len())?;
        for (index, source) in sources.iter().enumerate() {
            let module = ModuleId::from_index(index).ok_or(Unsupported {
                span: source.span,
                feature: "semantic module capacity",
            })?;
            let initializer = UnitId::from_index(index).unwrap();
            let mut data = empty_unit(UnitKind::ModuleInitialization, budget)?;
            data.module = module;
            budget.push(Scratch, &mut units, data)?;
            budget.push(
                Retained,
                &mut modules,
                ModuleInterface {
                    source: source.source_identity().clone(),
                    initializer,
                    dependencies: Vec::new(),
                    dynamic_dependencies: Vec::new(),
                    namespace: Vec::new(),
                    imports: Vec::new(),
                    exports: 0..0,
                    foreign_imports: Vec::new(),
                },
            )?;
            budget.push(Retained, &mut initialization, initializer)?;
        }
        Ok(Self {
            semantics,
            current_module: ModuleId::from_index(0).unwrap(),
            program: Program {
                tables_revision: RevisionId::fresh(),
                units: Vec::new(),
                cells: table(Vec::new(), budget)?,
                types: table(Vec::new(), budget)?,
                strings: table(Vec::new(), budget)?,
                structs: table(Vec::new(), budget)?,
                enums: table(Vec::new(), budget)?,
                fields: table(Vec::new(), budget)?,
                classes: table(Vec::new(), budget)?,
                exports: table(Vec::new(), budget)?,
                initialization: table(initialization, budget)?,
                modules: table(modules, budget)?,
                entry,
            },
            units,
            allocations: budget.filled(Scratch, sources.len(), 0)?,
            class_methods: budget.vector(Scratch, 0)?,
            current_class: None,
            budget,
        })
    }
    fn add_cells(
        &mut self,
        owner: impl Fn(SymbolId) -> Option<usize>,
    ) -> Result<(), ConversionError> {
        let semantics = self.semantics;
        for symbol in semantics.symbols() {
            self.work(1)?;
            if symbol.is_foreign() && symbol.name == "eval" {
                return self.unsupported(symbol.span, "source eval contract");
            }
            let module = owner(symbol.id)
                .filter(|&module| module < self.program.modules.len())
                .ok_or(Unsupported {
                    span: symbol.span,
                    feature: "missing checked symbol source owner",
                })?;
            let ty = self.ty(&symbol.ty)?;
            let name = self.budget.string(Retained, symbol.name)?;
            self.budget.push(
                Retained,
                building_table(&mut self.program.cells),
                Cell {
                    source_symbol: symbol.id,
                    name,
                    ty,
                    owner: self.program.modules[module].initializer,
                    region: RegionId::from_index(0).unwrap(),
                    declaration: symbol.span,
                    assigned: semantics.symbol_is_assigned(symbol.id),
                    binding: if symbol.is_foreign() {
                        CellBinding::Foreign
                    } else {
                        CellBinding::Local
                    },
                    synthetic: false,
                },
            )?;
        }
        Ok(())
    }
    fn register_source(
        &mut self,
        source: &ast::Program<'ast, 'src>,
    ) -> Result<(), ConversionError> {
        let semantics = self.semantics;
        // Function bindings are established before executing initialization. Their
        // bodies can therefore refer to later declarations and mutually recurse.
        for item in source.items {
            self.work(1)?;
            match item {
                Item::Function(function) => {
                    let id = self.add_unit(UnitKind::Function)?;
                    let name = self.string(function.name.name)?;
                    self.units[id.index()].function_name = Some(name);
                    let cell = self.cell(function.name)?;
                    self.units[id.index()].callable_type =
                        Some(self.program.cells[cell.index()].ty);
                    building_table(&mut self.program.cells)[cell.index()].binding =
                        CellBinding::Function(id);
                }
                // The shared declaration owner already classified these
                // cells; foreign declarations have no source body to register.
                Item::Extern(_) | Item::ExternGlobal(_) => {}
                Item::Struct(declaration) => {
                    let identity = semantics
                        .binding_type(declaration.name.span)
                        .and_then(|ty| semantics.nominal_id(ty))
                        .ok_or(Unsupported {
                            span: declaration.span,
                            feature: "missing checked struct identity",
                        })?;
                    let definition = semantics.nominal_struct(identity).ok_or(Unsupported {
                        span: declaration.span,
                        feature: "missing checked struct schema",
                    })?;
                    let owner = match definition.module {
                        Some(module) => ModuleId::from_index(module),
                        None if self.program.modules.len() == 1 => Some(self.current_module),
                        None => None,
                    }
                    .filter(|owner| *owner == self.current_module)
                    .ok_or(Unsupported {
                        span: declaration.span,
                        feature: "checked struct declaration source owner mismatch",
                    })?;
                    let start = self.program.fields.len();
                    for field in definition.fields.values() {
                        self.work(1)?;
                        if self.program.fields.last().is_some_and(|previous| {
                            previous.identity.index() >= field.member.index()
                        }) {
                            return self.unsupported(
                                declaration.span,
                                "checked nominal fields are not in canonical member order",
                            );
                        }
                        let ty = self.ty(&field.ty)?;
                        let name = self.budget.string(Retained, field.name)?;
                        self.budget.push(
                            Retained,
                            building_table(&mut self.program.fields),
                            Field {
                                identity: field.member,
                                owner: identity,
                                name,
                                ty,
                                index: field.index,
                            },
                        )?;
                    }
                    let mut type_parameters =
                        self.budget.vector(Retained, definition.type_params.len())?;
                    for name in &definition.type_params {
                        let name = self.budget.string(Retained, name)?;
                        self.budget.push(Retained, &mut type_parameters, name)?;
                    }
                    let name = self.budget.string(Retained, definition.declaration.name)?;
                    self.budget.push(
                        Retained,
                        building_table(&mut self.program.structs),
                        StructDefinition {
                            identity,
                            name,
                            module: owner,
                            span: definition.span,
                            type_parameters,
                            fields: start..self.program.fields.len(),
                        },
                    )?;
                }
                Item::Enum(declaration) => {
                    let checked =
                        semantics
                            .enum_info(declaration.name.name)
                            .ok_or(Unsupported {
                                span: declaration.span,
                                feature: "missing checked enum domain",
                            })?;
                    let mut variants = self.budget.vector(Retained, checked.variants.len())?;
                    for (name, value) in &checked.variants {
                        let name = self.budget.string(Retained, name)?;
                        let value = i32::try_from(*value).map_err(|_| Unsupported {
                            span: declaration.span,
                            feature: "enum value outside language domain",
                        })?;
                        self.budget
                            .push(Retained, &mut variants, EnumVariant { name, value })?;
                    }
                    let name = self.budget.string(Retained, checked.name)?;
                    self.budget.push(
                        Retained,
                        building_table(&mut self.program.enums),
                        EnumDefinition { name, variants },
                    )?;
                }
                Item::Stmt(_) => {}
                Item::Class(declaration) if !declaration.object => {
                    self.register_class(declaration)?;
                }
                Item::ExternClass(declaration) => {
                    self.register_extern_class(declaration)?;
                }
                _ => return self.unsupported(item.span(), "nominal callable declarations"),
            }
        }
        Ok(())
    }
    fn emit_source(
        &mut self,
        root: UnitId,
        source: &ast::Program<'ast, 'src>,
    ) -> Result<(), ConversionError> {
        let region = RegionId::from_index(0).unwrap();
        for item in source.items {
            self.work(1)?;
            if let Item::Class(declaration) = item {
                self.emit_class(root, region, declaration)?;
            }
            if let Item::Function(function) = item {
                let cell = self.cell(function.name)?;
                let CellBinding::Function(unit) = self.program.cells[cell.index()].binding else {
                    unreachable!()
                };
                self.units[unit.index()].suspension = if function.is_async {
                    Suspension::Async
                } else if function.is_generator {
                    Suspension::Generator
                } else {
                    Suspension::None
                };
                self.parameters(unit, function.params)?;
                self.statements(unit, region, function.body)?;
                let ty = self.program.cells[cell.index()].ty;
                let value = self.value(
                    root,
                    region,
                    OperationKind::Closure(unit),
                    &[],
                    ty,
                    None,
                    function.span,
                )?;
                self.effect(
                    root,
                    region,
                    OperationKind::Initialize(cell),
                    &[value],
                    function.span,
                )?;
            }
        }
        self.units[root.index()].instantiation_prefix = u32::try_from(
            self.units[root.index()].regions[region.index()]
                .operations
                .len(),
        )
        .map_err(|_| Unsupported {
            span: source.span,
            feature: "semantic instantiation prefix capacity",
        })?;
        for item in source.items {
            self.work(1)?;
            if let Item::Stmt(statement) = item {
                self.statement(root, region, statement)?;
            }
        }
        Ok(())
    }
    fn source_exports(
        &mut self,
        module: ModuleId,
        source: &ast::Program<'ast, 'src>,
    ) -> Result<(), ConversionError> {
        let start = self.program.exports.len();
        for export in source.exports {
            self.work(1)?;
            if export.kind != ast::ExportKind::Binding {
                return self.unsupported(export.span, "constructor export conversion");
            }
            let target = self
                .semantics
                .export_target(export.local.span)
                .and_then(interface_target)
                .ok_or(Unsupported {
                    span: export.span,
                    feature: "missing checked export target",
                })?;
            self.add_export(export.exported.name, target)?;
        }
        building_table(&mut self.program.modules)[module.index()].exports =
            start..self.program.exports.len();
        Ok(())
    }
    fn finish(mut self) -> Result<Program<'src>, ConversionError> {
        // Propagate transitive captures without cloning either closure list.
        loop {
            let mut changed = false;
            for unit in 0..self.units.len() {
                self.work(1)?;
                for operation in 0..self.units[unit].operations.len() {
                    self.work(1)?;
                    let OperationKind::Closure(child) = self.units[unit].operations[operation].kind
                    else {
                        continue;
                    };
                    for capture in 0..self.units[child.index()].captures.len() {
                        self.work(1)?;
                        let cell = self.units[child.index()].captures[capture];
                        if self.reference(UnitId::from_index(unit).unwrap(), cell)? {
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let mut units = std::mem::take(&mut self.units);
        for (index, mut data) in units.drain(..).enumerate() {
            let n = data.captures.len();
            let sort_work = n
                .checked_mul(usize::BITS as usize - n.leading_zeros() as usize + 1)
                .ok_or(AllocationError::Capacity)?;
            self.work(sort_work)?;
            data.captures.sort_unstable();
            self.budget.work(WorkKind::Analysis, 1)?;
            self.budget.retain(
                Retained,
                (std::mem::size_of::<UnitData>() + 2 * std::mem::size_of::<usize>()) as u64,
            )?;
            let frozen = WorkingUnit::new(UnitId::from_index(index).unwrap(), data).freeze();
            self.budget
                .push(Retained, &mut self.program.units, frozen)?;
        }
        drop_vector(units, Scratch, self.budget)?;
        drop_vector(self.allocations, Scratch, self.budget)?;
        drop_vector(self.class_methods, Scratch, self.budget)?;
        Ok(self.program)
    }
}

impl<'sem, 'ast, 'src> Lower<'_, '_, 'sem, 'ast, 'src> {
    fn work(&mut self, units: usize) -> Result<(), ConversionError> {
        self.budget.work(
            WorkKind::Analysis,
            u64::try_from(units).map_err(|_| AllocationError::Capacity)?,
        )?;
        Ok(())
    }
    fn unsupported<T>(&self, span: Span, feature: &'static str) -> Result<T, ConversionError> {
        Err(Unsupported { span, feature }.into())
    }
    fn ty(&mut self, ty: &Type<'src>) -> Result<TypeId, ConversionError> {
        use crate::check::type_admission::TypeQueryAdmission;
        use crate::check::type_payload::{
            measure_payload, Payload, PayloadError, PayloadMeasure,
        };
        fn measure(
            ty: &Type<'_>,
            budget: &mut AllocationBudget<'_>,
        ) -> Result<PayloadMeasure, AllocationError> {
            match measure_payload(Payload::Type(ty), budget, |_| {
                Ok::<_, std::convert::Infallible>(())
            }) {
                Ok(measured) => Ok(measured),
                Err(PayloadError::Allocation(error)) => Err(error),
                Err(PayloadError::Visitor(never)) => match never {},
            }
        }
        for (index, known) in self.program.types.iter().enumerate() {
            if crate::check::type_relation::type_equal_with(
                known,
                ty,
                &mut TypeQueryAdmission::new(self.budget),
            )? {
                return TypeId::from_index(index).ok_or(AllocationError::Capacity.into());
            }
        }
        let id = TypeId::from_index(self.program.types.len()).ok_or(AllocationError::Capacity)?;
        let allowance = if self.budget.is_accounted() {
            let measured = measure(ty, self.budget)?;
            self.budget.work(
                WorkKind::Analysis,
                measured
                    .nodes
                    .checked_mul(2)
                    .ok_or(AllocationError::Capacity)?,
            )?;
            // The common payload measure includes shared signature backing
            // conservatively; publication uses this same charge partition.
            self.budget.retain(Retained, measured.owned_bytes)?;
            Some(measured.owned_bytes)
        } else {
            None
        };
        let ty = ty.clone();
        if let Some(allowance) = allowance {
            // Vec Clone retains length, not the source's spare capacity. Only
            // this proven unused clone allowance can be reconciled downward.
            let actual = measure(&ty, self.budget)?.owned_bytes;
            let unused = allowance
                .checked_sub(actual)
                .ok_or(AllocationError::Unaccounted)?;
            self.budget.release(Retained, unused)?;
        }
        self.budget
            .push(Retained, building_table(&mut self.program.types), ty)?;
        Ok(id)
    }
    fn expression_type(&mut self, expr: &ast::Expr<'ast, 'src>) -> Result<TypeId, ConversionError> {
        let ty = self.semantics.expression_type(expr.id).ok_or(Unsupported {
            span: expr.span(),
            feature: "missing checked expression type",
        })?;
        self.ty(ty)
    }
    fn string(&mut self, value: &str) -> Result<StringId, ConversionError> {
        for (index, known) in self.program.strings.iter().enumerate() {
            self.budget.work(
                WorkKind::Analysis,
                u64::try_from(value.len().min(known.storage_bytes()) + 1)
                    .map_err(|_| AllocationError::Capacity)?,
            )?;
            if known.as_unicode() == Some(value) {
                return StringId::from_index(index).ok_or(AllocationError::Capacity.into());
            }
        }
        let id =
            StringId::from_index(self.program.strings.len()).ok_or(AllocationError::Capacity)?;
        let value = self.budget.string(Retained, value)?;
        self.budget.push(
            Retained,
            building_table(&mut self.program.strings),
            value.into(),
        )?;
        Ok(id)
    }
    fn owned_string(&mut self, value: StringValue) -> Result<StringId, ConversionError> {
        for (index, known) in self.program.strings.iter().enumerate() {
            self.budget.work(
                WorkKind::Analysis,
                u64::try_from(value.storage_bytes().min(known.storage_bytes()) + 1)
                    .map_err(|_| AllocationError::Capacity)?,
            )?;
            if known == &value {
                let bytes = value.capacity_bytes() as u64;
                drop(value);
                self.budget.release(Retained, bytes)?;
                return StringId::from_index(index).ok_or(AllocationError::Capacity.into());
            }
        }
        let id =
            StringId::from_index(self.program.strings.len()).ok_or(AllocationError::Capacity)?;
        self.budget
            .push(Retained, building_table(&mut self.program.strings), value)?;
        Ok(id)
    }
    /// One `Template` over the chunks and substitutions. A substitution
    /// whose conversion can run code (a `JsValue`) is converted as soon as it
    /// is evaluated, as JavaScript does, so the conversion stays ordered
    /// before any later substitution's evaluation.
    fn template(
        &mut self,
        unit: UnitId,
        region: RegionId,
        parts: &'ast [ast::TemplatePart<'ast, 'src>],
        ty: TypeId,
        origin: Option<SourceNodeId>,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let mut operands = self.budget.vector(Scratch, parts.len())?;
        for (index, part) in parts.iter().enumerate() {
            self.work(1)?;
            match part {
                ast::TemplatePart::String(text, part_span) => {
                    let decoded = StringValue::decode_template_admitted(text, self.budget)
                        .map_err(|error| match error {
                            crate::literal::StringDecodeError::Escape(_) => {
                                ConversionError::Unsupported(Unsupported {
                                    span: *part_span,
                                    feature: "invalid checked template chunk",
                                })
                            }
                            crate::literal::StringDecodeError::Resources(error) => {
                                ConversionError::Resources(error)
                            }
                        })?;
                    if decoded.storage_bytes() == 0 {
                        let bytes = decoded.capacity_bytes() as u64;
                        drop(decoded);
                        self.budget.release(Retained, bytes)?;
                        continue;
                    }
                    let id = self.owned_string(decoded)?;
                    let chunk = self.value(
                        unit,
                        region,
                        OperationKind::Constant(Constant::String(id)),
                        &[],
                        ty,
                        None,
                        *part_span,
                    )?;
                    self.budget.push(Scratch, &mut operands, chunk)?;
                }
                ast::TemplatePart::Expr(expression) => {
                    let value = self.expression(unit, region, expression)?;
                    self.budget.push(Scratch, &mut operands, value)?;
                    let substitution = self.expression_type(expression)?;
                    let later = parts[index + 1..]
                        .iter()
                        .any(|part| matches!(part, ast::TemplatePart::Expr(_)));
                    if later && !effect_free_string_conversion(&self.program.types[substitution.index()]) {
                        let prefix =
                            self.value(unit, region, OperationKind::Template, &operands, ty, None, span)?;
                        operands.clear();
                        self.budget.push(Scratch, &mut operands, prefix)?;
                    }
                }
            }
        }
        let result = if operands.is_empty() {
            let id = self.owned_string(StringValue::default())?;
            self.value(
                unit,
                region,
                OperationKind::Constant(Constant::String(id)),
                &[],
                ty,
                origin,
                span,
            )?
        } else {
            self.value(unit, region, OperationKind::Template, &operands, ty, origin, span)?
        };
        drop_vector(operands, Scratch, self.budget)?;
        Ok(result)
    }
    fn decoded_string(
        &mut self,
        value: &str,
        span: Span,
        feature: &'static str,
    ) -> Result<StringId, ConversionError> {
        let value =
            StringValue::decode_source_admitted(value, self.budget).map_err(
                |error| match error {
                    crate::literal::StringDecodeError::Escape(_) => {
                        ConversionError::Unsupported(Unsupported { span, feature })
                    }
                    crate::literal::StringDecodeError::Resources(error) => {
                        ConversionError::Resources(error)
                    }
                },
            )?;
        self.owned_string(value)
    }
    fn add_export(&mut self, name: &str, target: InterfaceTarget) -> Result<(), ConversionError> {
        let name = self.budget.string(Retained, name)?;
        self.budget.push(
            Retained,
            building_table(&mut self.program.exports),
            Export { name, target },
        )?;
        Ok(())
    }
    fn cell(&self, name: ast::Ident<'src>) -> Result<CellId, ConversionError> {
        let symbol = self
            .semantics
            .identifier_symbol(name.span)
            .ok_or(Unsupported {
                span: name.span,
                feature: "missing checked binding identity",
            })?;
        CellId::from_index(symbol.0 as usize).ok_or_else(|| {
            Unsupported {
                span: name.span,
                feature: "semantic cell capacity",
            }
            .into()
        })
    }
    fn reference(&mut self, unit: UnitId, cell: CellId) -> Result<bool, ConversionError> {
        if self.program.cells[cell.index()].owner == unit {
            return Ok(false);
        }
        self.work(self.units[unit.index()].captures.len())?;
        if self.units[unit.index()].captures.contains(&cell) {
            return Ok(false);
        }
        self.budget
            .push(Retained, &mut self.units[unit.index()].captures, cell)?;
        Ok(true)
    }
    /// Storage a desugaring needs that no source binding names. It borrows
    /// `provenance`'s symbol for naming and diagnostics only.
    fn synthetic_cell(
        &mut self,
        unit: UnitId,
        region: RegionId,
        provenance: ast::Ident<'src>,
        name: &str,
        ty: TypeId,
    ) -> Result<CellId, ConversionError> {
        let source_symbol = self.program.cells[self.cell(provenance)?.index()].source_symbol;
        let id = CellId::from_index(self.program.cells.len()).ok_or(AllocationError::Capacity)?;
        let name = self.budget.string(Retained, name)?;
        self.budget.push(
            Retained,
            building_table(&mut self.program.cells),
            Cell {
                source_symbol,
                name,
                ty,
                owner: unit,
                region,
                declaration: provenance.span,
                assigned: true,
                binding: CellBinding::Local,
                synthetic: true,
            },
        )?;
        Ok(id)
    }
    fn push_place(&mut self, unit: UnitId, place: Place) -> Result<PlaceId, ConversionError> {
        let id = PlaceId::from_index(self.units[unit.index()].places.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget
            .push(Retained, &mut self.units[unit.index()].places, place)?;
        Ok(id)
    }
    fn load_cell(
        &mut self,
        unit: UnitId,
        region: RegionId,
        cell: CellId,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let place = self.push_place(unit, Place::Cell(cell))?;
        let ty = self.program.cells[cell.index()].ty;
        self.value(unit, region, OperationKind::Load(place), &[], ty, None, span)
    }
    /// `for (T element of array)`, including `inline for`, whose unrolling is
    /// an optimization rather than a meaning. The array is evaluated once; each
    /// iteration re-reads its length, then initializes a fresh element binding
    /// from the current index, as the array iterator does.
    fn for_of(
        &mut self,
        unit: UnitId,
        region: RegionId,
        element: ast::Ident<'src>,
        iterable: &ast::Expr<'ast, 'src>,
        body: &Stmt<'ast, 'src>,
        span: Span,
    ) -> Result<(), ConversionError> {
        let collection = self.expression_type(iterable)?;
        if matches!(self.program.types[collection.index()], Type::Generator(_)) {
            // A generator has no index: the loop runs its iterator protocol.
            let generator = self.expression(unit, region, iterable)?;
            let iteration = self.region(unit, region, span)?;
            let item = self.declare(unit, iteration, element)?;
            if let Stmt::Block { body, .. } = body {
                self.statements(unit, iteration, body)?;
            } else {
                self.statement(unit, iteration, body)?;
            }
            self.effect(
                unit,
                region,
                OperationKind::ForOf {
                    item,
                    body: iteration,
                },
                &[generator],
                span,
            )?;
            return Ok(());
        }
        let length = match &self.program.types[collection.index()] {
            Type::Array(_) => crate::primitive::Intrinsic::ArrayLength,
            ty => match crate::typed_array::TypedArrayKind::from_type(ty) {
                Some(kind) => kind.length_intrinsic(),
                None => return self.unsupported(iterable.span(), "for-of over a non-indexed iterable"),
            },
        };
        let int = self.ty(&Type::Int)?;
        let boolean = self.ty(&Type::Bool)?;
        let scope = self.region(unit, region, span)?;
        let array = self.expression(unit, scope, iterable)?;
        let array_cell = self.synthetic_cell(unit, scope, element, "$for_of_array", collection)?;
        self.effect(unit, scope, OperationKind::Initialize(array_cell), &[array], span)?;
        let index_cell = self.synthetic_cell(unit, scope, element, "$for_of_index", int)?;
        let zero =
            self.value(unit, scope, OperationKind::Constant(Constant::Integer(0)), &[], int, None, span)?;
        self.effect(unit, scope, OperationKind::Initialize(index_cell), &[zero], span)?;

        let test = self.region(unit, scope, span)?;
        let index = self.load_cell(unit, test, index_cell, span)?;
        let current = self.load_cell(unit, test, array_cell, span)?;
        let bound = self.value(
            unit,
            test,
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(length)),
            &[current],
            int,
            None,
            span,
        )?;
        let more = self.value(
            unit,
            test,
            OperationKind::Binary(BinaryOp::Less),
            &[index, bound],
            boolean,
            None,
            span,
        )?;
        self.units[unit.index()].regions[test.index()].result = Some(more);

        let iteration = self.region(unit, scope, span)?;
        let cell = self.declare(unit, iteration, element)?;
        let current = self.load_cell(unit, iteration, array_cell, span)?;
        let index = self.load_cell(unit, iteration, index_cell, span)?;
        let item = self.push_place(unit, Place::Index { receiver: current, key: index })?;
        let item_type = self.program.cells[cell.index()].ty;
        let item = self.value(unit, iteration, OperationKind::Load(item), &[], item_type, None, span)?;
        let item = self.copy_value(unit, iteration, item, span)?;
        self.effect(unit, iteration, OperationKind::Initialize(cell), &[item], span)?;
        self.statement(unit, iteration, body)?;

        let update = self.region(unit, scope, span)?;
        let index = self.load_cell(unit, update, index_cell, span)?;
        let one =
            self.value(unit, update, OperationKind::Constant(Constant::Integer(1)), &[], int, None, span)?;
        let next = self.value(
            unit,
            update,
            OperationKind::IntBinary(IntBinary::Add),
            &[index, one],
            int,
            None,
            span,
        )?;
        let target = self.push_place(unit, Place::Cell(index_cell))?;
        self.effect(unit, update, OperationKind::Store(target), &[next], span)?;

        self.effect(
            unit,
            scope,
            OperationKind::Loop {
                test,
                body: iteration,
                update,
            },
            &[],
            span,
        )?;
        self.effect(unit, region, OperationKind::Block(scope), &[], span)?;
        Ok(())
    }
    /// Whether a class has an extern (host) ancestor.
    fn host_derived(&mut self, class: &str, span: Span) -> Result<bool, ConversionError> {
        let mut current = self.class_info(class, span)?;
        while let Some(base) = current.base.as_ref().and_then(base_class_name) {
            self.work(1)?;
            current = self.class_info(base, span)?;
            if current.external {
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// The arguments of a host-derived construction or `super(...)`: each
    /// evaluated in order, every declared parameter supplied.
    fn host_arguments(
        &mut self,
        unit: UnitId,
        region: RegionId,
        arguments: &'ast [ast::Argument<'ast, 'src>],
        expected: usize,
        span: Span,
    ) -> Result<Vec<ValueId>, ConversionError> {
        if arguments.len() != expected {
            return self.unsupported(span, "host class construction with omitted arguments");
        }
        let mut values = Vec::with_capacity(arguments.len());
        for argument in arguments {
            self.work(1)?;
            if argument.passing != crate::primitive::ParameterPassing::Value {
                return self.unsupported(argument.span, "reference argument to a host constructor");
            }
            let value = self.expression(unit, region, &argument.expression)?;
            values.push(self.copy_value(unit, region, value, argument.span)?);
        }
        Ok(values)
    }
    /// `super(...)` in a host-derived constructor: the base constructor runs
    /// first, then this class's own fields take their defaults, in order, as
    /// JavaScript initializes class fields when `super` returns.
    fn host_super(
        &mut self,
        unit: UnitId,
        region: RegionId,
        class: &'src str,
        this: CellId,
        arguments: &'ast [ast::Argument<'ast, 'src>],
        span: Span,
    ) -> Result<(), ConversionError> {
        let info = self.class_info(class, span)?;
        let base = info
            .base
            .as_ref()
            .and_then(base_class_name)
            .ok_or(Unsupported {
                span,
                feature: "super without a base class",
            })?;
        if !self.class_info(base, span)?.external {
            return self.unsupported(span, "host-derived class extending a host-derived class");
        }
        let parameters = self
            .class_info(base, span)?
            .constructor
            .as_ref()
            .map_or(0, |signature| signature.params.len());
        let values = self.host_arguments(unit, region, arguments, parameters, span)?;
        self.effect(unit, region, OperationKind::SuperConstruct, &values, span)?;
        let inherited = self.class_info(base, span)?.fields.len();
        let definition = self
            .program
            .classes
            .iter()
            .position(|definition| definition.name == class)
            .ok_or(Unsupported {
                span,
                feature: "super of an unregistered class",
            })?;
        let own = self.program.classes[definition].fields.len();
        for slot in inherited..own {
            self.work(1)?;
            let (key, ty) = self.program.classes[definition].fields[slot];
            let value = self.field_default(unit, region, ty, span)?;
            let receiver = self.load_cell(unit, region, this, span)?;
            let place = self.push_place(unit, Place::Member { receiver, key })?;
            self.effect(unit, region, OperationKind::Store(place), &[value], span)?;
        }
        Ok(())
    }
    /// `new C(...)` of a class with a host ancestor: its constructor makes a
    /// host object, so there is no field allocation or static `init` call.
    fn construct_host_class(
        &mut self,
        unit: UnitId,
        region: RegionId,
        class: &'src str,
        arguments: &'ast [ast::Argument<'ast, 'src>],
        ty: TypeId,
        origin: Option<SourceNodeId>,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let parameters = self
            .class_info(class, span)?
            .constructor
            .as_ref()
            .map_or(0, |signature| signature.params.len());
        // The constructor is the class value: `new` reads it first.
        let init = self.class_method(class, None)?.ok_or(Unsupported {
            span,
            feature: "host-derived class without init",
        })?;
        self.reference(unit, init.cell)?;
        let constructor = self.load_cell(unit, region, init.cell, span)?;
        let mut values = self.host_arguments(unit, region, arguments, parameters, span)?;
        values.insert(0, constructor);
        self.value(unit, region, OperationKind::ConstructClass, &values, ty, origin, span)
    }
    fn class_info(&self, name: &str, span: Span) -> Result<&'sem ClassInfo<'src>, ConversionError> {
        self.semantics.class_info(name).ok_or_else(|| {
            Unsupported {
                span,
                feature: "missing checked class",
            }
            .into()
        })
    }
    fn class_method(
        &mut self,
        class: &str,
        name: Option<&str>,
    ) -> Result<Option<ClassMethod<'src>>, ConversionError> {
        self.work(self.class_methods.len())?;
        Ok(self
            .class_methods
            .iter()
            .copied()
            .find(|method| method.class == class && method.name == name))
    }
    /// The `init` a construction of `class` runs: its own, else the nearest
    /// base's (a derived class without `init` inherits its base's).
    fn inherited_init(
        &mut self,
        mut class: &'src str,
        span: Span,
    ) -> Result<Option<ClassMethod<'src>>, ConversionError> {
        loop {
            self.work(1)?;
            if let Some(init) = self.class_method(class, None)? {
                return Ok(Some(init));
            }
            let info = self.class_info(class, span)?;
            let Some(base) = &info.base else {
                return Ok(None);
            };
            class = base_class_name(base).ok_or(Unsupported {
                span,
                feature: "class base is not a class",
            })?;
        }
    }
    /// Binds each `import extern` local to the foreign cell of the extern
    /// value declaration the checker matched it with.
    fn foreign_imports(
        &mut self,
        module: usize,
        source: &ast::Program<'ast, 'src>,
        sources: &[String],
    ) -> Result<(), ConversionError> {
        for (import, specifier) in source.foreign_imports.iter().zip(sources) {
            self.work(1)?;
            if import.specifiers.is_empty() {
                return self.unsupported(import.span, "foreign side-effect import conversion");
            }
            for binding in import.specifiers {
                self.work(source.items.len())?;
                let declaration = source.items.iter().find_map(|item| match item {
                    Item::Extern(declaration) if declaration.name.name == binding.local.name => {
                        Some(declaration.name)
                    }
                    Item::ExternGlobal(declaration)
                        if declaration.name.name == binding.local.name =>
                    {
                        Some(declaration.name)
                    }
                    _ => None,
                });
                let Some(declaration) = declaration else {
                    return self.unsupported(binding.local.span, "foreign import without extern value");
                };
                let cell = self.cell(declaration)?;
                let source = self.budget.string(Retained, specifier)?;
                let imported = self.budget.string(Retained, binding.imported.name)?;
                self.budget.push(
                    Retained,
                    &mut building_table(&mut self.program.modules)[module].foreign_imports,
                    ForeignImport {
                        cell,
                        source,
                        imported,
                    },
                )?;
            }
        }
        Ok(())
    }
    /// An `extern class` is a host object interface: its fields are exact
    /// host properties and its methods host calls on the receiver, so it
    /// has no units. Its declared field types type member places.
    fn register_extern_class(
        &mut self,
        declaration: &'ast ast::ExternClassDecl<'ast, 'src>,
    ) -> Result<(), ConversionError> {
        let span = declaration.span;
        let info = self.class_info(declaration.name.name, span)?;
        if !info.type_params.is_empty() {
            return self.unsupported(span, "generic extern class conversion");
        }
        let base = match &info.base {
            Some(base) => {
                let name = base_class_name(base).ok_or(Unsupported {
                    span,
                    feature: "class base is not a class",
                })?;
                Some(self.budget.string(Retained, name)?)
            }
            None => None,
        };
        let mut fields = self.budget.vector(Retained, info.fields.len())?;
        for field in info.fields.values() {
            self.work(1)?;
            let key = self.string(field.name)?;
            let ty = self.ty(&field.ty)?;
            self.budget.push(Retained, &mut fields, (key, ty))?;
        }
        let name = self.budget.string(Retained, declaration.name.name)?;
        let constructor = match &info.constructor {
            Some(signature) => Some(self.ty(&Type::Function(signature.clone()))?),
            None => None,
        };
        self.budget.push(
            Retained,
            building_table(&mut self.program.classes),
            ClassDefinition {
                name,
                external: true,
                base,
                type_params: Vec::new(),
                base_arguments: Vec::new(),
                constructor,
                fields,
            },
        )?;
        Ok(())
    }
    fn register_class(
        &mut self,
        declaration: &'ast ast::ClassDecl<'ast, 'src>,
    ) -> Result<(), ConversionError> {
        let span = declaration.span;
        let info = self.class_info(declaration.name.name, span)?;
        let base = match &info.base {
            Some(base) => {
                let name = base_class_name(base).ok_or(Unsupported {
                    span,
                    feature: "class base is not a class",
                })?;
                Some(self.budget.string(Retained, name)?)
            }
            None => None,
        };
        let mut fields = self.budget.vector(Retained, info.fields.len())?;
        let mut ordered = self.budget.vector(Scratch, info.fields.len())?;
        for field in info.fields.values() {
            self.work(1)?;
            self.budget.push(Scratch, &mut ordered, field)?;
        }
        ordered.sort_by_key(|field| field.index);
        for field in ordered.iter() {
            self.work(1)?;
            let key = self.string(field.name)?;
            let ty = self.ty(&field.ty)?;
            self.budget.push(Retained, &mut fields, (key, ty))?;
        }
        drop_vector(ordered, Scratch, self.budget)?;
        let name = self.budget.string(Retained, declaration.name.name)?;
        let mut type_params = self.budget.vector(Retained, info.type_params.len())?;
        for parameter in &info.type_params {
            let parameter = self.budget.string(Retained, parameter)?;
            self.budget.push(Retained, &mut type_params, parameter)?;
        }
        let mut base_arguments = Vec::new();
        if let Some(Type::ClassInstance { args, .. }) = &info.base {
            base_arguments = self.budget.vector(Retained, args.len())?;
            for argument in args {
                let argument = self.ty(argument)?;
                self.budget.push(Retained, &mut base_arguments, argument)?;
            }
        }
        self.budget.push(
            Retained,
            building_table(&mut self.program.classes),
            ClassDefinition {
                name,
                external: false,
                base,
                type_params,
                base_arguments,
                constructor: None,
                fields,
            },
        )?;
        // A class with a host ancestor stays a real subclass: its `init` is
        // the JavaScript constructor, which `super(...)` must reach.
        let class_index = u32::try_from(self.program.classes.len() - 1)
            .map_err(|_| AllocationError::Capacity)?;
        let host = self.host_derived(declaration.name.name, span)?;
        if host
            && !declaration
                .members
                .iter()
                .any(|member| matches!(member, ast::ClassMember::Constructor(_)))
        {
            return self.unsupported(span, "a class with a host ancestor needs an init");
        }
        for member in declaration.members {
            self.work(1)?;
            let (name, this_span, signature) = match member {
                ast::ClassMember::Field(_) => continue,
                ast::ClassMember::Method(function) => {
                    if function.is_async || function.is_generator {
                        return self.unsupported(function.span, "suspending method conversion");
                    }
                    let method = info.methods.get(function.name.name).ok_or(Unsupported {
                        span: function.span,
                        feature: "missing checked method",
                    })?;
                    if !method.type_params.is_empty() {
                        return self.unsupported(function.span, "generic method conversion");
                    }
                    (Some(function.name.name), function.name.span, method.signature.clone())
                }
                ast::ClassMember::Constructor(constructor) => {
                    let signature = info.constructor.clone().ok_or(Unsupported {
                        span: constructor.span,
                        feature: "missing checked constructor",
                    })?;
                    (None, constructor.span, signature)
                }
            };
            let this = ast::Ident {
                name: "this",
                span: this_span,
            };
            let this_cell = self.cell(this)?;
            let receiver = self.program.types[self.program.cells[this_cell.index()].ty.index()].clone();
            let mut params = Vec::with_capacity(signature.params.len() + 1);
            params.push(crate::check::FunctionParameter::value(receiver));
            params.extend(signature.params.iter().cloned());
            // `init` initializes the instance it is given and returns nothing;
            // `new` produces the instance, so a bare `return;` ends `init`.
            let return_type = if name.is_none() {
                Box::new(Type::Void)
            } else {
                signature.return_type.clone()
            };
            let function = crate::check::FunctionType::new(crate::check::FunctionSignature {
                params,
                return_type,
            });
            // A generic class's bodies are generic over its parameters; each
            // call instantiates them from the receiver's type arguments.
            let callable = self.ty(&if info.type_params.is_empty() {
                Type::Function(function)
            } else {
                Type::GenericFunction(crate::check::GenericFunctionType {
                    type_params: info.type_params.clone(),
                    signature: function,
                })
            })?;
            let unit = self.add_unit(UnitKind::Function)?;
            if host && name.is_none() {
                self.units[unit.index()].host_class = Some(class_index);
            }
            let label = name.unwrap_or("init");
            let function_name = self.string(label)?;
            self.units[unit.index()].function_name = Some(function_name);
            self.units[unit.index()].callable_type = Some(callable);
            let initializer = self.program.modules[self.current_module.index()].initializer;
            let cell = self.synthetic_cell(
                initializer,
                RegionId::from_index(0).unwrap(),
                this,
                label,
                callable,
            )?;
            let data = &mut building_table(&mut self.program.cells)[cell.index()];
            data.binding = CellBinding::Function(unit);
            data.assigned = false;
            self.budget.push(
                Scratch,
                &mut self.class_methods,
                ClassMethod {
                    class: declaration.name.name,
                    name,
                    unit,
                    cell,
                },
            )?;
        }
        Ok(())
    }
    /// Method and `init` bodies, created in the module's instantiation prefix
    /// like declared functions so later declarations can call them.
    fn emit_class(
        &mut self,
        root: UnitId,
        region: RegionId,
        declaration: &'ast ast::ClassDecl<'ast, 'src>,
    ) -> Result<(), ConversionError> {
        for member in declaration.members {
            self.work(1)?;
            let (name, this_span, params, body, span) = match member {
                ast::ClassMember::Field(_) => continue,
                ast::ClassMember::Method(function) => (
                    Some(function.name.name),
                    function.name.span,
                    function.params,
                    function.body,
                    function.span,
                ),
                ast::ClassMember::Constructor(constructor) => (
                    None,
                    constructor.span,
                    constructor.params,
                    constructor.body,
                    constructor.span,
                ),
            };
            let method = self
                .class_method(declaration.name.name, name)?
                .ok_or(Unsupported {
                    span,
                    feature: "unregistered class body",
                })?;
            let this = ast::Ident {
                name: "this",
                span: this_span,
            };
            let this_cell = self.cell(this)?;
            let outer = self.current_class.replace((declaration.name.name, this_cell));
            self.parameters_with_receiver(method.unit, Some(this), params)?;
            self.statements(method.unit, RegionId::from_index(0).unwrap(), body)?;
            self.current_class = outer;
            let ty = self.program.cells[method.cell.index()].ty;
            let value = self.value(root, region, OperationKind::Closure(method.unit), &[], ty, None, span)?;
            self.effect(root, region, OperationKind::Initialize(method.cell), &[value], span)?;
        }
        Ok(())
    }
    /// A field's value before any `init` statement runs: the legacy defaults.
    fn field_default(
        &mut self,
        unit: UnitId,
        region: RegionId,
        ty: TypeId,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let constant = match &self.program.types[ty.index()] {
            Type::Int | Type::Enum(_) => Constant::Integer(0),
            Type::Float => Constant::Number(0f64.to_bits()),
            Type::Bool => Constant::Boolean(false),
            Type::String => Constant::String(self.owned_string(StringValue::default())?),
            Type::Union(members) => {
                let first = members.first().cloned().ok_or(Unsupported {
                    span,
                    feature: "empty union field default",
                })?;
                let first = self.ty(&first)?;
                return self.field_default(unit, region, first, span);
            }
            Type::Array(_) => {
                let kind = self.allocation(unit, AllocationKind::Array)?;
                return self.value(unit, region, kind, &[], ty, None, span);
            }
            Type::Record(_) => {
                let keys = self.budget.vector(Retained, 0)?;
                let kind = self.allocation(unit, AllocationKind::Record(keys))?;
                return self.value(unit, region, kind, &[], ty, None, span);
            }
            Type::Map(_, _) | Type::Set(_) => {
                let constructor = if matches!(self.program.types[ty.index()], Type::Map(_, _)) {
                    crate::primitive::Intrinsic::MapNew
                } else {
                    crate::primitive::Intrinsic::SetNew
                };
                return self.construct_builtin(unit, region, constructor, None, ty, span);
            }
            Type::ArrayBuffer | Type::SharedArrayBuffer => {
                let constructor = if matches!(self.program.types[ty.index()], Type::ArrayBuffer) {
                    crate::primitive::Intrinsic::ArrayBufferNew
                } else {
                    crate::primitive::Intrinsic::SharedArrayBufferNew
                };
                let int = self.ty(&Type::Int)?;
                let zero = self.value(
                    unit,
                    region,
                    OperationKind::Constant(Constant::Integer(0)),
                    &[],
                    int,
                    None,
                    span,
                )?;
                return self.construct_builtin(unit, region, constructor, Some(zero), ty, span);
            }
            other if crate::typed_array::TypedArrayKind::from_type(other).is_some() => {
                let kind = crate::typed_array::TypedArrayKind::from_type(other).unwrap();
                let int = self.ty(&Type::Int)?;
                let zero = self.value(
                    unit,
                    region,
                    OperationKind::Constant(Constant::Integer(0)),
                    &[],
                    int,
                    None,
                    span,
                )?;
                return self.construct_builtin(unit, region, kind.new_intrinsic(), Some(zero), ty, span);
            }
            Type::Nullable(_) | Type::TypeParameter("$js") | Type::Null => Constant::Null,
            // Legacy leaves a non-nullable class, struct or callable field
            // null until `init` assigns it. That is not a value of the field's
            // type, so the constant keeps its own type.
            _ => {
                let null = self.ty(&Type::Null)?;
                return self.value(
                    unit,
                    region,
                    OperationKind::Constant(Constant::Null),
                    &[],
                    null,
                    None,
                    span,
                );
            }
        };
        self.value(unit, region, OperationKind::Constant(constant), &[], ty, None, span)
    }
    fn construct_builtin(
        &mut self,
        unit: UnitId,
        region: RegionId,
        constructor: crate::primitive::Intrinsic,
        argument: Option<ValueId>,
        ty: TypeId,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let contract = CallContract {
            signature: None,
            instantiation: None,
            supplied: u32::from(argument.is_some()),
            defaults: DefaultConvention::PreserveOmission,
        };
        let (kind, operands) = self.prepare_call_values(
            unit,
            region,
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Constructor(constructor),
                receiver: None,
            },
            contract,
            argument.as_slice(),
            span,
        )?;
        let value = self.value(unit, region, kind, &operands, ty, None, span)?;
        drop_vector(operands, Scratch, self.budget)?;
        Ok(value)
    }
    /// The object a construction starts from: every field at its default.
    fn class_instance(
        &mut self,
        unit: UnitId,
        region: RegionId,
        class: &'src str,
        ty: TypeId,
        origin: Option<SourceNodeId>,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        self.work(self.program.classes.len())?;
        let Some(definition) = self.program.classes.iter().position(|definition| definition.name == class) else {
            return self.unsupported(span, "construction of an unconverted class");
        };
        let fields = self.program.classes[definition].fields.len();
        let mut keys = self.budget.vector(Retained, fields)?;
        let mut values = self.budget.vector(Scratch, fields)?;
        for index in 0..fields {
            self.work(1)?;
            let (key, field_ty) = self.program.classes[definition].fields[index];
            self.budget.push(Retained, &mut keys, key)?;
            let value = self.field_default(unit, region, field_ty, span)?;
            self.budget.push(Scratch, &mut values, value)?;
        }
        let kind = self.allocation(unit, AllocationKind::Object(keys))?;
        let instance = self.value(unit, region, kind, &values, ty, origin, span)?;
        drop_vector(values, Scratch, self.budget)?;
        Ok(instance)
    }
    /// `new C(args)`: an object holding every field's default, then `init`.
    fn construct_class(
        &mut self,
        unit: UnitId,
        region: RegionId,
        class: &'src str,
        arguments: &'ast [ast::Argument<'ast, 'src>],
        ty: TypeId,
        origin: Option<SourceNodeId>,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        if self.host_derived(class, span)? {
            return self.construct_host_class(unit, region, class, arguments, ty, origin, span);
        }
        // `new C(args)` evaluates its explicit arguments, then creates the
        // instance with every field at its default, then runs `init`, which
        // materializes omitted parameters: JavaScript's order, where fields
        // are initialized on entry, ahead of parameter defaults.
        match self.inherited_init(class, span)? {
            Some(init) => {
                if arguments
                    .iter()
                    .all(|argument| argument.passing == crate::primitive::ParameterPassing::Value)
                {
                    let mut values = self.budget.vector(Scratch, arguments.len())?;
                    for argument in arguments {
                        let value = self.expression(unit, region, &argument.expression)?;
                        let value = self.copy_value(unit, region, value, argument.span)?;
                        self.budget.push(Scratch, &mut values, value)?;
                    }
                    let instance =
                        self.construct_class_values(unit, region, class, &values, ty, origin, span)?;
                    drop_vector(values, Scratch, self.budget)?;
                    return Ok(instance);
                }
                // A reference argument is a place the prepared call owns; the
                // instance is created inside that call, after the arguments.
                let receiver = CallReceiver::Construction { class, ty, origin };
                let (_, instance) =
                    self.call_class_function_with(unit, region, init, receiver, ty, arguments, span)?;
                Ok(instance)
            }
            None if arguments.is_empty() => self.class_instance(unit, region, class, ty, origin, span),
            None => self.unsupported(span, "arguments to a class without init"),
        }
    }
    /// `new C(...)` whose arguments are already evaluated (a default).
    fn construct_class_values(
        &mut self,
        unit: UnitId,
        region: RegionId,
        class: &'src str,
        arguments: &[ValueId],
        ty: TypeId,
        origin: Option<SourceNodeId>,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        if self.host_derived(class, span)? {
            let init = self.class_method(class, None)?.ok_or(Unsupported {
                span,
                feature: "host-derived class without init",
            })?;
            self.reference(unit, init.cell)?;
            let constructor = self.load_cell(unit, region, init.cell, span)?;
            let mut values = Vec::with_capacity(arguments.len() + 1);
            values.push(constructor);
            values.extend_from_slice(arguments);
            return self.value(unit, region, OperationKind::ConstructClass, &values, ty, origin, span);
        }
        let instance = self.class_instance(unit, region, class, ty, origin, span)?;
        match self.inherited_init(class, span)? {
            Some(init) => {
                let mut values = self.budget.vector(Scratch, arguments.len() + 1)?;
                self.budget.push(Scratch, &mut values, instance)?;
                for &argument in arguments {
                    self.budget.push(Scratch, &mut values, argument)?;
                }
                let signature = self.program.cells[init.cell.index()].ty;
                let instantiation = self.class_instantiation(unit, init, ty, span)?;
                self.reference(unit, init.cell)?;
                let callee = self.load_cell(unit, region, init.cell, span)?;
                let contract = CallContract {
                    signature: Some(signature),
                    instantiation,
                    supplied: u32::try_from(values.len()).map_err(|_| AllocationError::Capacity)?,
                    defaults: DefaultConvention::MaterializeAtCaller,
                };
                let (kind, operands) = self.prepare_call_values(
                    unit,
                    region,
                    CallTarget::Value {
                        callee,
                        invocation: Invocation::Value,
                    },
                    contract,
                    &values,
                    span,
                )?;
                drop_vector(values, Scratch, self.budget)?;
                let result = self.class_call_result(unit, signature, instantiation, span)?;
                self.value(unit, region, kind, &operands, result, None, span)?;
                drop_vector(operands, Scratch, self.budget)?;
            }
            None if arguments.is_empty() => {}
            None => return self.unsupported(span, "arguments to a class without init"),
        }
        Ok(instance)
    }
    /// For a generic class body, this call's instantiation: the receiver's
    /// type arguments substituted into the body's generic signature.
    fn class_instantiation(
        &mut self,
        unit: UnitId,
        method: ClassMethod<'src>,
        receiver: TypeId,
        span: Span,
    ) -> Result<Option<CallInstantiationId>, ConversionError> {
        let declaration = self.program.cells[method.cell.index()].ty;
        let Type::GenericFunction(function) = &self.program.types[declaration.index()] else {
            return Ok(None);
        };
        let function = function.clone();
        // An inherited body sees the receiver as its own class: substitute
        // each class's parameters into its `extends` arguments, up the chain.
        let (mut class, mut arguments) = match &self.program.types[receiver.index()] {
            Type::ClassInstance { name, args } => (*name, args.clone()),
            Type::Class(name) => (*name, Vec::new()),
            _ => return self.unsupported(span, "generic class body on a non-class receiver"),
        };
        while class != method.class {
            self.work(1)?;
            let info = self.class_info(class, span)?;
            let (base, base_arguments) = match &info.base {
                Some(Type::ClassInstance { name, args }) => (*name, args.clone()),
                Some(Type::Class(name)) => (*name, Vec::new()),
                _ => return self.unsupported(span, "generic class body on another class's receiver"),
            };
            let parameters = info.type_params.clone();
            let mut substituted = Vec::with_capacity(base_arguments.len());
            for argument in &base_arguments {
                substituted.push(
                    crate::check::type_substitution::substitute_type_with(
                        argument,
                        &mut |name: &str, _: &mut crate::check::type_relation::Unmetered| {
                            Ok(parameters
                                .iter()
                                .position(|parameter| *parameter == name)
                                .and_then(|index| arguments.get(index)))
                        },
                        &mut crate::check::type_relation::Unmetered,
                    )
                    .unwrap_or_else(|never| match never {}),
                );
            }
            class = base;
            arguments = substituted;
        }
        if arguments.len() != function.type_params.len() {
            return self.unsupported(span, "generic class receiver arity");
        }
        let effective = crate::check::type_substitution::substitute_type_with(
            &Type::Function(function.signature.clone()),
            &mut |name: &str, _: &mut crate::check::type_relation::Unmetered| {
                Ok(function
                    .type_params
                    .iter()
                    .position(|parameter| *parameter == name)
                    .map(|index| &arguments[index]))
            },
            &mut crate::check::type_relation::Unmetered,
        )
        .unwrap_or_else(|never| match never {});
        let effective = self.ty(&effective)?;
        let mut ids = self.budget.vector(Retained, arguments.len())?;
        for argument in &arguments {
            let argument = self.ty(argument)?;
            self.budget.push(Retained, &mut ids, argument)?;
        }
        let id = CallInstantiationId::from_index(self.units[unit.index()].call_instantiations.len())
            .ok_or(Unsupported {
                span,
                feature: "generic call instance capacity",
            })?;
        self.budget.push(
            Retained,
            &mut self.units[unit.index()].call_instantiations,
            CallInstantiation {
                declaration,
                arguments: ids,
                signature: effective,
            },
        )?;
        Ok(Some(id))
    }
    /// The result type of a class body call, after any instantiation.
    fn class_call_result(
        &mut self,
        unit: UnitId,
        signature: TypeId,
        instantiation: Option<CallInstantiationId>,
        span: Span,
    ) -> Result<TypeId, ConversionError> {
        let effective = instantiation
            .map(|id| self.units[unit.index()].call_instantiations[id.index()].signature)
            .unwrap_or(signature);
        let result = match &self.program.types[effective.index()] {
            Type::Function(signature) => (*signature.return_type).clone(),
            _ => return self.unsupported(span, "class body without a callable type"),
        };
        self.ty(&result)
    }
    /// A static call of a method or `init` with the instance first.
    fn call_class_function(
        &mut self,
        unit: UnitId,
        region: RegionId,
        method: ClassMethod<'src>,
        receiver: ValueId,
        arguments: &'ast [ast::Argument<'ast, 'src>],
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let receiver_type = self.units[unit.index()].values[receiver.index()].ty;
        let (value, _) = self.call_class_function_with(
            unit,
            region,
            method,
            CallReceiver::Value(receiver),
            receiver_type,
            arguments,
            span,
        )?;
        Ok(value)
    }
    /// A class function call whose receiver is either already computed or the
    /// instance a construction creates after its explicit arguments. Returns
    /// the call's result and the receiver.
    fn call_class_function_with(
        &mut self,
        unit: UnitId,
        region: RegionId,
        method: ClassMethod<'src>,
        receiver: CallReceiver<'src>,
        receiver_type: TypeId,
        arguments: &'ast [ast::Argument<'ast, 'src>],
        span: Span,
    ) -> Result<(ValueId, ValueId), ConversionError> {
        self.reference(unit, method.cell)?;
        let callee = self.load_cell(unit, region, method.cell, span)?;
        let signature = self.program.cells[method.cell.index()].ty;
        let instantiation = self.class_instantiation(unit, method, receiver_type, span)?;
        let result = self.class_call_result(unit, signature, instantiation, span)?;
        let contract = CallContract {
            signature: Some(signature),
            instantiation,
            supplied: u32::try_from(arguments.len() + 1).map_err(|_| Unsupported {
                span,
                feature: "semantic call argument capacity",
            })?,
            defaults: DefaultConvention::MaterializeAtCaller,
        };
        let (kind, operands, receiver) = self.prepare_call_with_receiver(
            unit,
            region,
            CallTarget::Value {
                callee,
                invocation: Invocation::Value,
            },
            contract,
            Some(receiver),
            arguments,
            span,
        )?;
        let value = self.value(unit, region, kind, &operands, result, None, span)?;
        drop_vector(operands, Scratch, self.budget)?;
        Ok((value, receiver.expect("a class function call has a receiver")))
    }
    fn declare(
        &mut self,
        unit: UnitId,
        region: RegionId,
        name: ast::Ident<'src>,
    ) -> Result<CellId, ConversionError> {
        let cell = self.cell(name)?;
        let data = &mut building_table(&mut self.program.cells)[cell.index()];
        data.owner = unit;
        data.region = region;
        Ok(cell)
    }
    fn add_unit(&mut self, kind: UnitKind) -> Result<UnitId, ConversionError> {
        let id = UnitId::from_index(self.units.len()).ok_or(AllocationError::Capacity)?;
        let mut data = empty_unit(kind, self.budget)?;
        data.module = self.current_module;
        self.budget.push(Scratch, &mut self.units, data)?;
        self.budget.push(Scratch, &mut self.allocations, 0)?;
        Ok(id)
    }
    fn infer_creation_name(
        &mut self,
        unit: UnitId,
        expression: &ast::Expr<'ast, 'src>,
        value: ValueId,
        name: &str,
    ) -> Result<(), ConversionError> {
        if matches!(expression.kind, ExprKind::ArrowFunction { .. }) {
            let name = self.string(name)?;
            self.infer_creation_id(unit, expression, value, name);
        }
        Ok(())
    }
    fn infer_creation_id(
        &mut self,
        unit: UnitId,
        expression: &ast::Expr<'ast, 'src>,
        value: ValueId,
        name: StringId,
    ) {
        if !matches!(expression.kind, ExprKind::ArrowFunction { .. }) {
            return;
        }
        let definition = self.units[unit.index()].values[value.index()].definition;
        let OperationKind::Closure(child) =
            self.units[unit.index()].operations[definition.index()].kind
        else {
            unreachable!()
        };
        self.units[child.index()].function_name = Some(name);
    }
    fn region(
        &mut self,
        unit: UnitId,
        parent: RegionId,
        span: Span,
    ) -> Result<RegionId, ConversionError> {
        let data = &mut self.units[unit.index()];
        let id = RegionId::from_index(data.regions.len()).ok_or(AllocationError::Capacity)?;
        self.budget.push(
            Retained,
            &mut data.regions,
            Region {
                parent: Some(parent),
                operations: Vec::new(),
                result: None,
                span,
            },
        )?;
        Ok(id)
    }
    fn operation(
        &mut self,
        unit: UnitId,
        region: RegionId,
        kind: OperationKind,
        operands: &[ValueId],
        ty: Option<TypeId>,
        origin: Option<SourceNodeId>,
        span: Span,
    ) -> Result<Option<ValueId>, ConversionError> {
        let data = &mut self.units[unit.index()];
        let range = OperandRange {
            start: u32::try_from(data.operands.len()).map_err(|_| AllocationError::Capacity)?,
            len: u32::try_from(operands.len()).map_err(|_| AllocationError::Capacity)?,
        };
        range
            .start
            .checked_add(range.len)
            .ok_or(AllocationError::Capacity)?;
        self.budget
            .extend_copy(Retained, &mut data.operands, operands)?;
        let op = OpId::from_index(data.operations.len()).ok_or(AllocationError::Capacity)?;
        let result = if let Some(ty) = ty {
            let value = ValueId::from_index(data.values.len()).ok_or(AllocationError::Capacity)?;
            self.budget
                .push(Retained, &mut data.values, Value { ty, definition: op })?;
            Some(value)
        } else {
            None
        };
        self.budget.push(
            Retained,
            &mut data.operations,
            Operation {
                kind,
                operands: range,
                result,
                region,
                origin,
                span,
            },
        )?;
        self.budget
            .push(Retained, &mut data.regions[region.index()].operations, op)?;
        Ok(result)
    }
    fn value(
        &mut self,
        unit: UnitId,
        region: RegionId,
        kind: OperationKind,
        operands: &[ValueId],
        ty: TypeId,
        origin: Option<SourceNodeId>,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        Ok(self
            .operation(unit, region, kind, operands, Some(ty), origin, span)?
            .unwrap())
    }
    fn effect(
        &mut self,
        unit: UnitId,
        region: RegionId,
        kind: OperationKind,
        operands: &[ValueId],
        span: Span,
    ) -> Result<(), ConversionError> {
        self.operation(unit, region, kind, operands, None, None, span)?;
        Ok(())
    }
    fn copy_value(
        &mut self,
        unit: UnitId,
        region: RegionId,
        value: ValueId,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let ty = self.units[unit.index()].values[value.index()].ty;
        fn value_transfer(
            ty: &Type<'_>,
            budget: &mut AllocationBudget<'_>,
        ) -> Result<bool, AllocationError> {
            budget.work(WorkKind::Analysis, 1)?;
            Ok(match ty {
                Type::Struct(_) | Type::StructInstance { .. } => true,
                Type::Nullable(inner) => value_transfer(inner, budget)?,
                Type::Union(members) => {
                    let mut found = false;
                    for member in members {
                        if value_transfer(member, budget)? {
                            found = true;
                            break;
                        }
                    }
                    found
                }
                Type::TypeParameter(name) => *name != "$js",
                _ => false,
            })
        }
        if value_transfer(&self.program.types[ty.index()], self.budget)? {
            self.value(
                unit,
                region,
                OperationKind::CopyValue,
                &[value],
                ty,
                None,
                span,
            )
        } else {
            Ok(value)
        }
    }
    fn parameters(
        &mut self,
        unit: UnitId,
        parameters: &[ast::Param<'ast, 'src>],
    ) -> Result<(), ConversionError> {
        self.parameters_with_receiver(unit, None, parameters)
    }
    /// A class body's `this` is its first parameter; source parameters follow.
    fn parameters_with_receiver(
        &mut self,
        unit: UnitId,
        receiver: Option<ast::Ident<'src>>,
        parameters: &[ast::Param<'ast, 'src>],
    ) -> Result<(), ConversionError> {
        let offset = usize::from(receiver.is_some());
        if let Some(receiver) = receiver {
            let cell = self.declare(unit, RegionId::from_index(0).unwrap(), receiver)?;
            building_table(&mut self.program.cells)[cell.index()].binding = CellBinding::Parameter(0);
            self.budget
                .push(Retained, &mut self.units[unit.index()].parameters, cell)?;
        }
        for (index, parameter) in parameters.iter().enumerate() {
            self.work(1)?;
            let position = index + offset;
            let cell = self.declare(unit, RegionId::from_index(0).unwrap(), parameter.name)?;
            building_table(&mut self.program.cells)[cell.index()].binding =
                CellBinding::Parameter(position as u32);
            self.budget
                .push(Retained, &mut self.units[unit.index()].parameters, cell)?;
        }
        // Typed callers supply every default themselves. A host or an erased
        // caller can still omit one, so the body applies it to `undefined`,
        // in parameter order, before any other code runs.
        let entry = RegionId::from_index(0).unwrap();
        for (index, parameter) in parameters.iter().enumerate() {
            self.work(1)?;
            let position = index + offset;
            if parameter.default.is_none() {
                continue;
            }
            let signature = self.units[unit.index()]
                .callable_type
                .and_then(|ty| match &self.program.types[ty.index()] {
                    Type::Function(signature) => Some(signature.clone()),
                    Type::GenericFunction(function) => Some(function.signature.clone()),
                    _ => None,
                })
                .ok_or(Unsupported {
                    span: parameter.span,
                    feature: "parameter default without a checked signature",
                })?;
            let Some(default) = signature.params[position].default.clone() else {
                return self.unsupported(parameter.span, "parameter default lost its checked value");
            };
            if matches!(default, crate::check::DefaultValue::Undefined) {
                // Absence already is `undefined`; there is nothing to apply.
                continue;
            }
            let cell = self.units[unit.index()].parameters[position];
            building_table(&mut self.program.cells)[cell.index()].assigned = true;
            let ty = self.program.cells[cell.index()].ty;
            let boolean = self.ty(&Type::Bool)?;
            let current = self.load_cell(unit, entry, cell, parameter.span)?;
            let missing = self.value(
                unit,
                entry,
                OperationKind::IsUndefined,
                &[current],
                boolean,
                None,
                parameter.span,
            )?;
            let yes = self.region(unit, entry, parameter.span)?;
            let value = match default {
                crate::check::DefaultValue::Parameter(earlier) => {
                    let earlier = self.units[unit.index()].parameters[earlier + offset];
                    self.load_cell(unit, yes, earlier, parameter.span)?
                }
                default => self.default_value(unit, yes, &default, ty, &[], true, parameter.span)?,
            };
            let target = self.push_place(unit, Place::Cell(cell))?;
            self.effect(unit, yes, OperationKind::Store(target), &[value], parameter.span)?;
            self.effect(
                unit,
                entry,
                OperationKind::If { yes, no: None },
                &[missing],
                parameter.span,
            )?;
        }
        Ok(())
    }
    fn statements(
        &mut self,
        unit: UnitId,
        region: RegionId,
        statements: &[Stmt<'ast, 'src>],
    ) -> Result<(), ConversionError> {
        for (index, statement) in statements.iter().enumerate() {
            self.statement(unit, region, statement)?;
            // Control never reaches what follows. A later declaration stays,
            // since a closure lowered earlier may name it.
            if terminates(statement) && !statements[index + 1..].iter().any(declares) {
                break;
            }
        }
        Ok(())
    }
    fn statement_region(
        &mut self,
        unit: UnitId,
        parent: RegionId,
        statement: &Stmt<'ast, 'src>,
    ) -> Result<RegionId, ConversionError> {
        let region = self.region(unit, parent, statement.span())?;
        if let Stmt::Block { body, .. } = statement {
            self.statements(unit, region, body)?;
        } else {
            self.statement(unit, region, statement)?;
        }
        Ok(region)
    }
    /// `auto [a, , b, ...rest] = values;`: each name reads its position as
    /// `T?` (an absent position is null), holes read nothing, and the rest
    /// is a shallow copy from its position. The source is evaluated once.
    fn array_destructure(
        &mut self,
        unit: UnitId,
        region: RegionId,
        bindings: &[ast::ArrayBinding<'src>],
        value: &ast::Expr<'ast, 'src>,
    ) -> Result<(), ConversionError> {
        let source = self.expression(unit, region, value)?;
        let source_ty = self.units[unit.index()].values[source.index()].ty;
        let Type::Array(element) = self.program.types[source_ty.index()].clone() else {
            return self.unsupported(value.span(), "array destructuring of a non-array");
        };
        let int = self.ty(&Type::Int)?;
        for (position, binding) in bindings.iter().enumerate() {
            self.work(1)?;
            let (ast::ArrayBinding::Name(name) | ast::ArrayBinding::Rest(name)) = *binding else {
                continue;
            };
            let span = name.span;
            let index = self.value(
                unit,
                region,
                OperationKind::Constant(Constant::Integer(
                    i32::try_from(position).map_err(|_| AllocationError::Capacity)?,
                )),
                &[],
                int,
                None,
                span,
            )?;
            let cell = self.declare(unit, region, name)?;
            let ty = self.program.cells[cell.index()].ty;
            let item = if matches!(binding, ast::ArrayBinding::Rest(_)) {
                let signature = self.ty(&Type::Function(crate::check::FunctionType::new(
                    crate::check::FunctionSignature {
                        params: vec![
                            crate::check::FunctionParameter::defaulted(
                                Type::Int,
                                crate::check::DefaultValue::Int(0),
                            ),
                            crate::check::FunctionParameter::defaulted(
                                Type::Int,
                                crate::check::DefaultValue::Int(i32::MAX as i64),
                            ),
                        ],
                        return_type: Box::new(Type::Array(element.clone())),
                    },
                )))?;
                let contract = CallContract {
                    signature: Some(signature),
                    instantiation: None,
                    supplied: 1,
                    defaults: DefaultConvention::PreserveOmission,
                };
                let (kind, operands) = self.prepare_call_values(
                    unit,
                    region,
                    CallTarget::Intrinsic {
                        operation: ResolvedIntrinsic::Method(crate::primitive::Intrinsic::ArraySlice),
                        receiver: Some(source),
                    },
                    contract,
                    &[index],
                    span,
                )?;
                let rest = self.value(unit, region, kind, &operands, ty, None, span)?;
                drop_vector(operands, Scratch, self.budget)?;
                rest
            } else {
                let place = self.push_place(unit, Place::Index { receiver: source, key: index })?;
                self.value(unit, region, OperationKind::Load(place), &[], ty, None, span)?
            };
            let item = self.copy_value(unit, region, item, span)?;
            self.effect(unit, region, OperationKind::Initialize(cell), &[item], span)?;
        }
        Ok(())
    }
    /// `auto {a, "b-c": b, ...rest} = record;`: each name reads its key as
    /// `T?`, and the rest is a fresh null-prototype record of every other
    /// own key, in JavaScript's own-key order. The source is evaluated once.
    fn record_destructure(
        &mut self,
        unit: UnitId,
        region: RegionId,
        bindings: &[ast::RecordBinding<'src>],
        rest: Option<ast::Ident<'src>>,
        value: &ast::Expr<'ast, 'src>,
        span: Span,
    ) -> Result<(), ConversionError> {
        let source = self.expression(unit, region, value)?;
        let source_ty = self.units[unit.index()].values[source.index()].ty;
        let Type::Record(element) = self.program.types[source_ty.index()].clone() else {
            return self.unsupported(value.span(), "record destructuring of a non-record");
        };
        let mut keys = self.budget.vector(Scratch, bindings.len())?;
        for binding in bindings {
            self.work(1)?;
            let key = self.decoded_string(binding.key.name, binding.key.span, "invalid record binding key")?;
            self.budget.push(Scratch, &mut keys, key)?;
            let cell = self.declare(unit, region, binding.name)?;
            let ty = self.program.cells[cell.index()].ty;
            let place = self.push_place(unit, Place::Member { receiver: source, key })?;
            let item = self.value(unit, region, OperationKind::Load(place), &[], ty, None, binding.span)?;
            let item = self.copy_value(unit, region, item, binding.span)?;
            self.effect(unit, region, OperationKind::Initialize(cell), &[item], binding.span)?;
        }
        if let Some(name) = rest {
            let cell = self.declare(unit, region, name)?;
            let record_ty = self.program.cells[cell.index()].ty;
            let empty = self.budget.vector(Retained, 0)?;
            let kind = self.allocation(unit, AllocationKind::Record(empty))?;
            let record = self.value(unit, region, kind, &[], record_ty, None, span)?;
            let string = self.ty(&Type::String)?;
            let boolean = self.ty(&Type::Bool)?;
            let present = self.ty(&element)?;
            let iteration = self.region(unit, region, span)?;
            let key_cell = self.synthetic_cell(unit, iteration, name, "$rest_key", string)?;
            let key = self.load_cell(unit, iteration, key_cell, span)?;
            // Nested tests, one per named key: copy only the other keys.
            let mut body = iteration;
            for &named in keys.iter() {
                self.work(1)?;
                let literal = self.value(
                    unit,
                    body,
                    OperationKind::Constant(Constant::String(named)),
                    &[],
                    string,
                    None,
                    span,
                )?;
                let other = self.value(
                    unit,
                    body,
                    OperationKind::Binary(BinaryOp::NotEq),
                    &[key, literal],
                    boolean,
                    None,
                    span,
                )?;
                let inner = self.region(unit, body, span)?;
                self.effect(unit, body, OperationKind::If { yes: inner, no: None }, &[other], span)?;
                body = inner;
            }
            let from = self.push_place(unit, Place::Index { receiver: source, key })?;
            let item = self.value(unit, body, OperationKind::Load(from), &[], present, None, span)?;
            let to = self.push_place(unit, Place::Index { receiver: record, key })?;
            self.effect(unit, body, OperationKind::Store(to), &[item], span)?;
            self.effect(
                unit,
                region,
                OperationKind::ForIn {
                    key: key_cell,
                    body: iteration,
                },
                &[source],
                span,
            )?;
            let record = self.copy_value(unit, region, record, span)?;
            self.effect(unit, region, OperationKind::Initialize(cell), &[record], span)?;
        }
        drop_vector(keys, Scratch, self.budget)?;
        Ok(())
    }
    /// A member read from an already evaluated receiver, as `place` spells
    /// the same member of a receiver expression.
    fn member_of_value(
        &mut self,
        unit: UnitId,
        region: RegionId,
        expr: &ast::Expr<'ast, 'src>,
        receiver: ValueId,
        property: ast::Ident<'src>,
        ty: TypeId,
    ) -> Result<ValueId, ConversionError> {
        let span = expr.span();
        if let ExpressionResolution::Primitive(operation @ ResolvedIntrinsic::Property(_)) =
            self.semantics.expression_resolution(expr.id)
        {
            return self.value(
                unit,
                region,
                OperationKind::Intrinsic(operation),
                &[receiver],
                ty,
                Some(expr.id),
                span,
            );
        }
        let place = match self.semantics.resolved_member(expr.id) {
            Some(NominalMember::Field { owner, .. }) if owner.is_class() => Place::Member {
                receiver,
                key: self.string(property.name)?,
            },
            Some(NominalMember::Field { field, .. }) => {
                let base = self.push_place(unit, Place::Value(receiver))?;
                Place::Field {
                    base,
                    field: field.member,
                }
            }
            _ => Place::Member {
                receiver,
                key: self.string(property.name)?,
            },
        };
        let place = self.push_place(unit, place)?;
        self.value(unit, region, OperationKind::Load(place), &[], ty, Some(expr.id), span)
    }
    /// The remaining arms of a `match` as one value region.
    fn match_region(
        &mut self,
        unit: UnitId,
        parent: RegionId,
        scrutinee: ValueId,
        arms: &[ast::MatchArm<'ast, 'src>],
        ty: TypeId,
        span: Span,
    ) -> Result<RegionId, ConversionError> {
        self.work(1)?;
        let region = self.region(unit, parent, span)?;
        let result = if let [last] = arms {
            self.expression(unit, region, &last.value)?
        } else {
            let condition = self.match_test(unit, region, scrutinee, arms[0].pattern)?;
            let yes = self.expression_region(unit, region, &arms[0].value)?;
            let no = self.match_region(unit, region, scrutinee, &arms[1..], ty, span)?;
            self.value(
                unit,
                region,
                OperationKind::Select { yes, no },
                &[condition],
                ty,
                None,
                span,
            )?
        };
        self.units[unit.index()].regions[region.index()].result = Some(result);
        Ok(region)
    }
    /// `scrutinee === pattern`, the pattern spelled as a scrutinee-typed constant.
    fn match_test(
        &mut self,
        unit: UnitId,
        region: RegionId,
        scrutinee: ValueId,
        pattern: ast::MatchPattern<'src>,
    ) -> Result<ValueId, ConversionError> {
        let span = pattern.span();
        let constant = match pattern {
            ast::MatchPattern::EnumVariant { span, .. } => {
                let value = self.semantics.enum_variant_value(span).ok_or(Unsupported {
                    span,
                    feature: "match pattern lost its checked discriminant",
                })?;
                Constant::Integer(i32::try_from(value).map_err(|_| Unsupported {
                    span,
                    feature: "enum discriminant outside int",
                })?)
            }
            ast::MatchPattern::Int(value, _) => Constant::Integer(i32::try_from(value).map_err(|_| {
                Unsupported {
                    span,
                    feature: "integer pattern outside int",
                }
            })?),
            ast::MatchPattern::String(value, _) => {
                Constant::String(self.decoded_string(value, span, "invalid match string pattern")?)
            }
            ast::MatchPattern::Bool(value, _) => Constant::Boolean(value),
            ast::MatchPattern::Wildcard(_) => {
                return self.unsupported(span, "match wildcard before the last arm")
            }
        };
        let scrutinee_ty = self.units[unit.index()].values[scrutinee.index()].ty;
        let pattern = self.value(
            unit,
            region,
            OperationKind::Constant(constant),
            &[],
            scrutinee_ty,
            None,
            span,
        )?;
        let boolean = self.ty(&Type::Bool)?;
        self.value(
            unit,
            region,
            OperationKind::Binary(BinaryOp::Eq),
            &[scrutinee, pattern],
            boolean,
            None,
            span,
        )
    }
    fn expression_region(
        &mut self,
        unit: UnitId,
        parent: RegionId,
        expression: &ast::Expr<'ast, 'src>,
    ) -> Result<RegionId, ConversionError> {
        let region = self.region(unit, parent, expression.span())?;
        let result = self.expression(unit, region, expression)?;
        self.units[unit.index()].regions[region.index()].result = Some(result);
        Ok(region)
    }
    fn statement(
        &mut self,
        unit: UnitId,
        region: RegionId,
        statement: &Stmt<'ast, 'src>,
    ) -> Result<(), ConversionError> {
        self.work(1)?;
        match statement {
            Stmt::ArrayDestructure {
                bindings, value, ..
            } => self.array_destructure(unit, region, bindings, value)?,
            Stmt::RecordDestructure {
                bindings,
                rest,
                value,
                span,
            } => self.record_destructure(unit, region, bindings, *rest, value, *span)?,
            Stmt::VarDecl(declaration) => {
                let cell = self.declare(unit, region, declaration.name)?;
                let value = if let Some(initializer) = &declaration.initializer {
                    let value = self.expression(unit, region, initializer)?;
                    self.infer_creation_name(unit, initializer, value, declaration.name.name)?;
                    value
                } else {
                    return self.unsupported(declaration.span, "default variable initialization");
                };
                let value = self.copy_value(unit, region, value, declaration.span)?;
                self.effect(
                    unit,
                    region,
                    OperationKind::Initialize(cell),
                    &[value],
                    declaration.span,
                )?;
            }
            Stmt::Expr(expr) => {
                self.expression(unit, region, expr)?;
            }
            Stmt::Return { value, span } => {
                let value = value
                    .as_ref()
                    .map(|value| self.expression(unit, region, value))
                    .transpose()?;
                let value = value
                    .map(|v| self.copy_value(unit, region, v, *span))
                    .transpose()?;
                self.effect(unit, region, OperationKind::Return, value.as_slice(), *span)?;
            }
            Stmt::Throw { value, span } => {
                let value = self.expression(unit, region, value)?;
                self.effect(unit, region, OperationKind::Throw, &[value], *span)?;
            }
            Stmt::Block { body, span } => {
                let child = self.region(unit, region, *span)?;
                self.statements(unit, child, body)?;
                self.effect(unit, region, OperationKind::Block(child), &[], *span)?;
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let condition = self.expression(unit, region, condition)?;
                let yes = self.statement_region(unit, region, then_branch)?;
                let no = else_branch
                    .map(|s| self.statement_region(unit, region, s))
                    .transpose()?;
                self.effect(
                    unit,
                    region,
                    OperationKind::If { yes, no },
                    &[condition],
                    *span,
                )?;
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let test = self.expression_region(unit, region, condition)?;
                let body = self.statement_region(unit, region, body)?;
                let update = self.region(unit, region, *span)?;
                self.effect(
                    unit,
                    region,
                    OperationKind::Loop { test, body, update },
                    &[],
                    *span,
                )?;
            }
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                span,
            } => {
                // The initializer's lexical cells outlive individual loop-body
                // activations; the enclosing block owns their declarations.
                let scope = self.region(unit, region, *span)?;
                if let Some(initializer) = initializer {
                    match initializer {
                        ForInitializer::VarDecl(decl) => {
                            self.statement(unit, scope, &Stmt::VarDecl(decl.clone()))?
                        }
                        ForInitializer::Expr(expr) => {
                            self.expression(unit, scope, expr)?;
                        }
                    }
                }
                let test = if let Some(condition) = condition {
                    self.expression_region(unit, scope, condition)?
                } else {
                    self.region(unit, scope, *span)?
                };
                let body = self.statement_region(unit, scope, body)?;
                let update = if let Some(update) = update {
                    self.expression_region(unit, scope, update)?
                } else {
                    self.region(unit, scope, *span)?
                };
                self.effect(
                    unit,
                    scope,
                    OperationKind::Loop { test, body, update },
                    &[],
                    *span,
                )?;
                self.effect(unit, region, OperationKind::Block(scope), &[], *span)?;
            }
            Stmt::ForOf {
                element,
                iterable,
                body,
                span,
                ..
            } => self.for_of(unit, region, *element, iterable, body, *span)?,
            Stmt::ForIn {
                key,
                object,
                body,
                span,
                ..
            } => {
                let object = self.expression(unit, region, object)?;
                let iteration = self.region(unit, region, *span)?;
                let cell = self.declare(unit, iteration, *key)?;
                if let Stmt::Block { body, .. } = body {
                    self.statements(unit, iteration, body)?;
                } else {
                    self.statement(unit, iteration, body)?;
                }
                self.effect(
                    unit,
                    region,
                    OperationKind::ForIn {
                        key: cell,
                        body: iteration,
                    },
                    &[object],
                    *span,
                )?;
            }
            Stmt::Yield {
                value,
                delegate,
                span,
            } => {
                let value = self.expression(unit, region, value)?;
                let value = self.copy_value(unit, region, value, *span)?;
                self.effect(
                    unit,
                    region,
                    OperationKind::Yield {
                        delegate: *delegate,
                    },
                    &[value],
                    *span,
                )?;
            }
            Stmt::SuperCall { args, span } => {
                let (class, this) = self.current_class.ok_or(Unsupported {
                    span: *span,
                    feature: "super outside a class constructor",
                })?;
                if self.host_derived(class, *span)? {
                    return self.host_super(unit, region, class, this, args, *span);
                }
                let base = self
                    .class_info(class, *span)?
                    .base
                    .as_ref()
                    .and_then(base_class_name)
                    .ok_or(Unsupported {
                        span: *span,
                        feature: "super without a base class",
                    })?;
                match self.inherited_init(base, *span)? {
                    Some(init) => {
                        let receiver = self.load_cell(unit, region, this, *span)?;
                        self.call_class_function(unit, region, init, receiver, args, *span)?;
                    }
                    None if args.is_empty() => {}
                    None => return self.unsupported(*span, "super arguments without a base init"),
                }
            }
            Stmt::Try {
                body,
                catch,
                finally,
                span,
            } => {
                let body_region = self.region(unit, region, *span)?;
                self.statements(unit, body_region, body)?;
                let catch = if let Some(catch) = catch {
                    let catch_region = self.region(unit, region, catch.span)?;
                    let cell = catch
                        .binding
                        .map(|b| self.declare(unit, catch_region, b.name))
                        .transpose()?;
                    self.statements(unit, catch_region, catch.body)?;
                    Some((cell, catch_region))
                } else {
                    None
                };
                let finally = if let Some(body) = finally {
                    let child = self.region(unit, region, *span)?;
                    self.statements(unit, child, body)?;
                    Some(child)
                } else {
                    None
                };
                self.effect(
                    unit,
                    region,
                    OperationKind::Try {
                        body: body_region,
                        catch,
                        finally,
                    },
                    &[],
                    *span,
                )?;
            }
            Stmt::Break(span) => self.effect(unit, region, OperationKind::Break, &[], *span)?,
            Stmt::Continue(span) => {
                self.effect(unit, region, OperationKind::Continue, &[], *span)?
            }
            _ => return self.unsupported(statement.span(), "statement conversion"),
        }
        Ok(())
    }

    fn binary_kind(&self, op: BinaryOp, ty: TypeId) -> OperationKind {
        let integer = matches!(self.program.types[ty.index()], Type::Int | Type::Enum(_));
        let int = match op {
            BinaryOp::Add => Some(IntBinary::Add),
            BinaryOp::Sub => Some(IntBinary::Subtract),
            BinaryOp::Mul => Some(IntBinary::Multiply),
            BinaryOp::Div => Some(IntBinary::Divide),
            BinaryOp::Mod => Some(IntBinary::Remainder),
            BinaryOp::UnsignedShiftRight => Some(IntBinary::UnsignedShiftRight),
            _ => None,
        };
        if let Some(op) = int.filter(|_| integer) {
            OperationKind::IntBinary(op)
        } else {
            OperationKind::Binary(op)
        }
    }
    fn eager_binary(expr: &ast::Expr<'ast, 'src>) -> bool {
        matches!(expr.kind, ExprKind::Binary { op, .. } if !matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Nullish))
    }
    fn binary_expression(
        &mut self,
        unit: UnitId,
        region: RegionId,
        expression: &ast::Expr<'ast, 'src>,
    ) -> Result<ValueId, ConversionError> {
        // Dense arithmetic chains use an explicit postorder stack. Each leaf
        // retains its left-to-right evaluation; no depth-based semantic fallback.
        let mut pending = self.budget.copy_slice(Scratch, &[(expression, false)])?;
        let mut values = Vec::new();
        while let Some((expr, ready)) = pending.pop() {
            self.work(1)?;
            if let ExprKind::Binary { op, lhs, rhs, .. } = &expr.kind {
                if Self::eager_binary(expr) {
                    if !ready {
                        self.budget.extend_copy(
                            Scratch,
                            &mut pending,
                            &[(expr, true), (rhs, false), (lhs, false)],
                        )?;
                        continue;
                    }
                    let right = values.pop().unwrap();
                    let left = values.pop().unwrap();
                    let ty = self.expression_type(expr)?;
                    let kind = self.binary_kind(*op, ty);
                    let value = self.value(
                        unit,
                        region,
                        kind,
                        &[left, right],
                        ty,
                        Some(expr.id),
                        expr.span(),
                    )?;
                    self.budget.push(Scratch, &mut values, value)?;
                    continue;
                }
            }
            let value = self.expression(unit, region, expr)?;
            self.budget.push(Scratch, &mut values, value)?;
        }
        let result = values.pop().unwrap();
        drop_vector(values, Scratch, self.budget)?;
        drop_vector(pending, Scratch, self.budget)?;
        Ok(result)
    }
    fn place(
        &mut self,
        unit: UnitId,
        region: RegionId,
        expr: &ast::Expr<'ast, 'src>,
    ) -> Result<PlaceId, ConversionError> {
        self.work(1)?;
        let place = match &expr.kind {
            ExprKind::Ident(name) => {
                let cell = self.cell(*name)?;
                self.reference(unit, cell)?;
                Place::Cell(cell)
            }
            ExprKind::Member {
                object, property, ..
            } if matches!(
                self.semantics.resolved_member(expr.id),
                Some(NominalMember::Field { owner, .. }) if owner.is_class()
            ) =>
            {
                // A class instance is a shared mutable object: its field is a
                // named member of the evaluated reference, not a value path.
                let receiver = self.expression(unit, region, object)?;
                Place::Member {
                    receiver,
                    key: self.string(property.name)?,
                }
            }
            ExprKind::Member {
                object, property, ..
            } => {
                if let Some(NominalMember::Field { field, .. }) =
                    self.semantics.resolved_member(expr.id)
                {
                    let member = field.member;
                    let base = if matches!(
                        object.kind,
                        ExprKind::Ident(_) | ExprKind::Member { .. } | ExprKind::Index { .. }
                    ) {
                        let base = self.place(unit, region, object)?;
                        self.narrowed_base(unit, region, object, base)?
                    } else {
                        let value = self.expression(unit, region, object)?;
                        let id = PlaceId::from_index(self.units[unit.index()].places.len())
                            .ok_or(AllocationError::Capacity)?;
                        self.budget.push(
                            Retained,
                            &mut self.units[unit.index()].places,
                            Place::Value(value),
                        )?;
                        id
                    };
                    Place::Field {
                        base,
                        field: member,
                    }
                } else {
                    let receiver = self.expression(unit, region, object)?;
                    Place::Member {
                        receiver,
                        key: self.string(property.name)?,
                    }
                }
            }
            ExprKind::Index { object, index, .. } => {
                let receiver = self.expression(unit, region, object)?;
                let key = self.expression(unit, region, index)?;
                Place::Index { receiver, key }
            }
            _ => return self.unsupported(expr.span(), "evaluated place conversion"),
        };
        let id = PlaceId::from_index(self.units[unit.index()].places.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget
            .push(Retained, &mut self.units[unit.index()].places, place)?;
        Ok(id)
    }

    /// A struct projection through a place the checker narrowed from `P?`
    /// to `P` projects the narrowed value, since a projection's base must be
    /// a struct. Writing through it stays a temporary-value refusal.
    fn narrowed_base(
        &mut self,
        unit: UnitId,
        region: RegionId,
        object: &ast::Expr<'ast, 'src>,
        base: PlaceId,
    ) -> Result<PlaceId, ConversionError> {
        self.work(1)?;
        let declared = match self.units[unit.index()].places[base.index()] {
            Place::Cell(cell) => Some(self.program.cells[cell.index()].ty),
            Place::Field { field, .. } => self.program.field(field).map(|field| field.ty),
            Place::Member { receiver, key } => {
                self.work(self.program.classes.len())?;
                let receiver = self.units[unit.index()].values[receiver.index()].ty;
                match &self.program.types[receiver.index()] {
                    Type::Class(name) => self
                        .program
                        .class(name)
                        .and_then(|class| class.fields.iter().find(|(field, _)| *field == key))
                        .map(|(_, ty)| *ty),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(declared) = declared else {
            return Ok(base);
        };
        if !matches!(self.program.types[declared.index()], Type::Nullable(_)) {
            return Ok(base);
        }
        let narrowed = self.expression_type(object)?;
        if matches!(self.program.types[narrowed.index()], Type::Nullable(_)) {
            return Ok(base);
        }
        let value = self.value(
            unit,
            region,
            OperationKind::Load(base),
            &[],
            narrowed,
            Some(object.id),
            object.span(),
        )?;
        let id = PlaceId::from_index(self.units[unit.index()].places.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget
            .push(Retained, &mut self.units[unit.index()].places, Place::Value(value))?;
        Ok(id)
    }

    /// Keep LHS evaluation before RHS without snapshotting the storage path.
    /// A field update later reads the current value of this same root, so a
    /// callback's sibling/ancestor updates are not reconstructed from stale data.
    fn prepare_mutable_place(
        &mut self,
        unit: UnitId,
        region: RegionId,
        place: PlaceId,
        check: bool,
        span: Span,
    ) -> Result<(), ConversionError> {
        let selected = &self.units[unit.index()].places[place.index()];
        let reference_formal = if let Place::Cell(cell) = selected {
            let cell = &self.program.cells[cell.index()];
            if let CellBinding::Parameter(position) = cell.binding {
                self.units[cell.owner.index()]
                    .parameter(position, &self.program.types)
                    .is_some_and(|parameter| {
                        parameter.passing == crate::primitive::ParameterPassing::MutableReference
                    })
            } else {
                false
            }
        } else {
            false
        };
        if !matches!(selected, Place::Field { .. }) && !reference_formal {
            return Ok(());
        }
        let mut root = place;
        while let Place::Field { base, .. } = self.units[unit.index()].places[root.index()] {
            self.work(1)?;
            root = base;
        }
        match self.units[unit.index()].places[root.index()] {
            // An array element, record entry or class field is rebuilt in
            // place from its current value, like a cell root.
            Place::Cell(_) | Place::Member { .. } | Place::Index { .. } => {}
            Place::Value(_) => {
                return self.unsupported(span, "mutation of a temporary struct value")
            }
            Place::Field { .. } => unreachable!("field bases were walked to their root"),
        }
        if check {
            // Preparing a projected assignment (including a reference formal
            // that dynamically designates a field) observes actual ancestors,
            // including null from a stale source refinement, before its RHS.
            // Compound/update operations already load their old leaf here.
            self.effect(unit, region, OperationKind::CheckPlace(place), &[], span)?;
        }
        Ok(())
    }
    fn expression(
        &mut self,
        unit: UnitId,
        region: RegionId,
        expr: &ast::Expr<'ast, 'src>,
    ) -> Result<ValueId, ConversionError> {
        self.work(1)?;
        if Self::eager_binary(expr) {
            return self.binary_expression(unit, region, expr);
        }
        let ty = self.expression_type(expr)?;
        let span = expr.span();
        let origin = Some(expr.id);
        let (kind, operands) = match &expr.kind {
            ExprKind::Int(value, _) => (
                OperationKind::Constant(Constant::Integer(*value as i32)),
                vec![],
            ),
            ExprKind::Float(value, _) => (
                OperationKind::Constant(Constant::Number(value.to_bits())),
                vec![],
            ),
            ExprKind::String(value, _) => {
                let id = self.decoded_string(value, span, "invalid checked string")?;
                (OperationKind::Constant(Constant::String(id)), vec![])
            }
            ExprKind::Bool(value, _) => {
                (OperationKind::Constant(Constant::Boolean(*value)), vec![])
            }
            ExprKind::Null(_) => (OperationKind::Constant(Constant::Null), vec![]),
            ExprKind::DynamicImport { source, span } => {
                let module = self
                    .semantics
                    .dynamic_import_module(*span)
                    .and_then(|module| ModuleId::from_index(module as usize))
                    .filter(|module| module.index() < self.program.modules.len())
                    .ok_or(Unsupported {
                        span: *span,
                        feature: "unresolved dynamic import",
                    })?;
                // The namespace's members are read once the task settles.
                let members = self.program.modules[module.index()].namespace.len();
                for index in 0..members {
                    let cell = self.program.modules[module.index()].namespace[index].1;
                    self.reference(unit, cell)?;
                }
                let specifier = self.string(source)?;
                (OperationKind::LoadModule { module, specifier }, vec![])
            }
            ExprKind::Ident(_) | ExprKind::Index { .. } => {
                (OperationKind::Load(self.place(unit, region, expr)?), vec![])
            }
            ExprKind::Member { object, .. } => {
                if let Some(value) = self.semantics.enum_variant_value(span) {
                    (
                        OperationKind::Constant(Constant::Integer(value as i32)),
                        vec![],
                    )
                } else if let ExpressionResolution::Primitive(
                    operation @ ResolvedIntrinsic::Property(_),
                ) = self.semantics.expression_resolution(expr.id)
                {
                    let receiver = self.expression(unit, region, object)?;
                    (
                        OperationKind::Intrinsic(operation),
                        self.budget.copy_slice(Scratch, &[receiver])?,
                    )
                } else {
                    (OperationKind::Load(self.place(unit, region, expr)?), vec![])
                }
            }
            ExprKind::Unary {
                op, expr: value, ..
            } => {
                let value = self.expression(unit, region, value)?;
                (
                    OperationKind::Unary {
                        op: *op,
                        integer: matches!(self.program.types[ty.index()], Type::Int),
                    },
                    self.budget.copy_slice(Scratch, &[value])?,
                )
            }
            ExprKind::Binary { op, lhs, rhs, .. } => {
                let left = self.expression(unit, region, lhs)?;
                let right = self.expression_region(unit, region, rhs)?;
                let kind = match op {
                    BinaryOp::And => ShortCircuit::BooleanAnd,
                    BinaryOp::Or => ShortCircuit::BooleanOr,
                    BinaryOp::Nullish => ShortCircuit::Nullish,
                    _ => unreachable!(),
                };
                (
                    OperationKind::ShortCircuit { kind, right },
                    self.budget.copy_slice(Scratch, &[left])?,
                )
            }
            // `a?.m` and `a?.[i]`: the receiver is evaluated once; when it is
            // present the access reads the narrowed receiver (the index only
            // then), and otherwise the result is null.
            ExprKind::OptionalMember { object, span: access, .. }
            | ExprKind::OptionalIndex { object, span: access, .. } => {
                let receiver = self.expression(unit, region, object)?;
                let optional = self.units[unit.index()].values[receiver.index()].ty;
                let Type::Nullable(inner) = self.program.types[optional.index()].clone() else {
                    return self.unsupported(span, "optional access on a non-nullable receiver");
                };
                let inner = self.ty(&inner)?;
                let present = self
                    .semantics
                    .optional_present_type(*access)
                    .cloned()
                    .ok_or(Unsupported {
                        span,
                        feature: "optional access lost its checked present type",
                    })?;
                let present = self.ty(&present)?;
                let null_ty = self.ty(&Type::Null)?;
                let boolean = self.ty(&Type::Bool)?;
                let null = self.value(
                    unit,
                    region,
                    OperationKind::Constant(Constant::Null),
                    &[],
                    null_ty,
                    None,
                    span,
                )?;
                let test = self.value(
                    unit,
                    region,
                    OperationKind::Binary(BinaryOp::NotEq),
                    &[receiver, null],
                    boolean,
                    None,
                    span,
                )?;
                let yes = self.region(unit, region, span)?;
                let whole = self.push_place(unit, Place::Value(receiver))?;
                let narrowed = self.value(
                    unit,
                    yes,
                    OperationKind::Load(whole),
                    &[],
                    inner,
                    None,
                    span,
                )?;
                let accessed = match &expr.kind {
                    ExprKind::OptionalMember { property, .. } => {
                        self.member_of_value(unit, yes, expr, narrowed, *property, present)?
                    }
                    ExprKind::OptionalIndex { index, .. } => {
                        let key = self.expression(unit, yes, index)?;
                        let place = self.push_place(unit, Place::Index { receiver: narrowed, key })?;
                        self.value(unit, yes, OperationKind::Load(place), &[], present, None, span)?
                    }
                    _ => unreachable!(),
                };
                self.units[unit.index()].regions[yes.index()].result = Some(accessed);
                let no = self.region(unit, region, span)?;
                let absent = self.value(
                    unit,
                    no,
                    OperationKind::Constant(Constant::Null),
                    &[],
                    null_ty,
                    None,
                    span,
                )?;
                self.units[unit.index()].regions[no.index()].result = Some(absent);
                (
                    OperationKind::Select { yes, no },
                    self.budget.copy_slice(Scratch, &[test])?,
                )
            }
            // The scrutinee is evaluated once; each arm tests strict equality
            // with its pattern, and the last arm is the checked fallback.
            ExprKind::Match { value, arms, .. } => {
                let scrutinee = self.expression(unit, region, value)?;
                if let [only] = arms {
                    let arm = self.expression(unit, region, &only.value)?;
                    if self.units[unit.index()].values[arm.index()].ty != ty {
                        return self.unsupported(span, "single-arm match with a widened result");
                    }
                    (
                        OperationKind::CopyValue,
                        self.budget.copy_slice(Scratch, &[arm])?,
                    )
                } else {
                    let condition = self.match_test(unit, region, scrutinee, arms[0].pattern)?;
                    let yes = self.expression_region(unit, region, &arms[0].value)?;
                    let no = self.match_region(unit, region, scrutinee, &arms[1..], ty, span)?;
                    (
                        OperationKind::Select { yes, no },
                        self.budget.copy_slice(Scratch, &[condition])?,
                    )
                }
            }
            ExprKind::If {
                condition,
                then_value,
                else_value,
                ..
            } => {
                let condition = self.expression(unit, region, condition)?;
                let yes = self.expression_region(unit, region, then_value)?;
                let no = self.expression_region(unit, region, else_value)?;
                (
                    OperationKind::Select { yes, no },
                    self.budget.copy_slice(Scratch, &[condition])?,
                )
            }
            ExprKind::Assignment {
                op, target, value, ..
            } => {
                if *op == AssignmentOp::Nullish {
                    return self.unsupported(span, "nullish place assignment");
                }
                let place = self.place(unit, region, target)?;
                self.prepare_mutable_place(
                    unit,
                    region,
                    place,
                    *op == AssignmentOp::Assign,
                    target.span(),
                )?;
                let old = if *op != AssignmentOp::Assign {
                    Some(self.value(
                        unit,
                        region,
                        OperationKind::Load(place),
                        &[],
                        ty,
                        Some(target.id),
                        target.span(),
                    )?)
                } else {
                    None
                };
                let rhs = self.expression(unit, region, value)?;
                if *op == AssignmentOp::Assign {
                    if let ExprKind::Ident(name) = target.kind {
                        self.infer_creation_name(unit, value, rhs, name.name)?;
                    }
                }
                let value = if let Some(old) = old {
                    let op = match op {
                        AssignmentOp::Add => BinaryOp::Add,
                        AssignmentOp::Sub => BinaryOp::Sub,
                        AssignmentOp::Mul => BinaryOp::Mul,
                        AssignmentOp::Div => BinaryOp::Div,
                        AssignmentOp::Mod => BinaryOp::Mod,
                        AssignmentOp::BitAnd => BinaryOp::BitAnd,
                        AssignmentOp::BitOr => BinaryOp::BitOr,
                        AssignmentOp::Xor => BinaryOp::Xor,
                        AssignmentOp::ShiftLeft => BinaryOp::ShiftLeft,
                        AssignmentOp::ShiftRight => BinaryOp::ShiftRight,
                        AssignmentOp::UnsignedShiftRight => BinaryOp::UnsignedShiftRight,
                        _ => unreachable!(),
                    };
                    self.value(
                        unit,
                        region,
                        self.binary_kind(op, ty),
                        &[old, rhs],
                        ty,
                        origin,
                        span,
                    )?
                } else {
                    rhs
                };
                let copied = self.copy_value(unit, region, value, span)?;
                self.effect(unit, region, OperationKind::Store(place), &[copied], span)?;
                return Ok(value);
            }
            ExprKind::Update {
                op, target, prefix, ..
            } => {
                let place = self.place(unit, region, target)?;
                self.prepare_mutable_place(unit, region, place, false, target.span())?;
                let old = self.value(
                    unit,
                    region,
                    OperationKind::Load(place),
                    &[],
                    ty,
                    Some(target.id),
                    target.span(),
                )?;
                let one = if matches!(self.program.types[ty.index()], Type::Float) {
                    Constant::Number(1.0f64.to_bits())
                } else {
                    Constant::Integer(1)
                };
                let one = self.value(
                    unit,
                    region,
                    OperationKind::Constant(one),
                    &[],
                    ty,
                    None,
                    span,
                )?;
                let binary = if *op == ast::UpdateOp::Increment {
                    BinaryOp::Add
                } else {
                    BinaryOp::Sub
                };
                let value = self.value(
                    unit,
                    region,
                    self.binary_kind(binary, ty),
                    &[old, one],
                    ty,
                    origin,
                    span,
                )?;
                self.effect(unit, region, OperationKind::Store(place), &[value], span)?;
                return Ok(if *prefix { value } else { old });
            }
            ExprKind::Call { callee, args, .. } => {
                let resolution = self.semantics.expression_resolution(expr.id);
                if let ExpressionResolution::Builtin(
                    builtin @ (BuiltinCall::JsAnd | BuiltinCall::JsOr),
                ) = resolution
                {
                    let left = self.expression(unit, region, &args[0].expression)?;
                    let right = self.expression_region(unit, region, &args[1].expression)?;
                    let kind = if builtin == BuiltinCall::JsAnd {
                        ShortCircuit::JavaScriptAnd
                    } else {
                        ShortCircuit::JavaScriptOr
                    };
                    return Ok(self.value(
                        unit,
                        region,
                        OperationKind::ShortCircuit { kind, right },
                        &[left],
                        ty,
                        origin,
                        span,
                    )?);
                }
                if let ExprKind::Member { object, .. } = &callee.kind {
                    if let Some(NominalMember::Method { name, method, .. }) =
                        self.semantics.resolved_member(callee.id)
                    {
                        if let Some(found) = self.class_method(method.owner, Some(name))? {
                            // Static dispatch: the checker rejects overriding.
                            let receiver = self.expression(unit, region, object)?;
                            return self.call_class_function(unit, region, found, receiver, args, span);
                        }
                    }
                }
                let target = if let ExpressionResolution::Builtin(builtin) = resolution {
                    CallTarget::Builtin(builtin)
                } else if let ExpressionResolution::Primitive(
                    operation @ ResolvedIntrinsic::Method(_),
                ) = self.semantics.expression_resolution(callee.id)
                {
                    let ExprKind::Member { object, .. } = &callee.kind else {
                        return self.unsupported(span, "detached primitive method");
                    };
                    let receiver = Some(self.expression(unit, region, object)?);
                    CallTarget::Intrinsic {
                        operation,
                        receiver,
                    }
                } else {
                    if matches!(
                        callee.kind,
                        ExprKind::Member { .. } | ExprKind::Index { .. }
                    ) {
                        let place = self.place(unit, region, callee)?;
                        if matches!(self.units[unit.index()].places[place.index()], Place::Field { .. }) {
                            // A function held in a value struct has no
                            // receiver: `s.f(x)` calls the loaded value, so
                            // the callee never sees the struct as `this`.
                            let ty = self.expression_type(callee)?;
                            let callee = self.value(
                                unit,
                                region,
                                OperationKind::Load(place),
                                &[],
                                ty,
                                Some(callee.id),
                                callee.span(),
                            )?;
                            CallTarget::Value {
                                callee,
                                invocation: Invocation::Value,
                            }
                        } else {
                            CallTarget::Reference { place }
                        }
                    } else {
                        CallTarget::Value {
                            callee: self.expression(unit, region, callee)?,
                            invocation: Invocation::Value,
                        }
                    }
                };
                let signature = self
                    .semantics
                    .expression_type(callee.id)
                    .map(|ty| self.ty(ty))
                    .transpose()?;
                if let CallTarget::Builtin(builtin) = target {
                    if crate::primitive::builtin_call_contract(builtin).is_none()
                        && !crate::primitive::host_builtin(builtin)
                    {
                        return self.unsupported(span, "semantic builtin call contract");
                    }
                } else if signature.is_none() {
                    return self.unsupported(callee.span(), "missing checked call signature");
                }
                let defaults = match target {
                    CallTarget::Intrinsic { operation, .. } => {
                        operation.call_default_convention().ok_or(Unsupported {
                            span,
                            feature: "prepared intrinsic call convention",
                        })?
                    }
                    ref target if host_call(&self.program, &self.units[unit.index()], target) => {
                        DefaultConvention::PreserveOmission
                    }
                    _ => DefaultConvention::MaterializeAtCaller,
                };
                if defaults == DefaultConvention::MaterializeAtCaller {
                    let parameters =
                        signature.and_then(|id| match &self.program.types[id.index()] {
                            Type::Function(signature) => Some(signature.params.len()),
                            Type::GenericFunction(function) => {
                                Some(function.signature.params.len())
                            }
                            _ => None,
                        });
                    let _ = parameters;
                }
                let semantics = self.semantics;
                let instantiation = if let Some(instance) = semantics.call_instantiation(expr.id) {
                    let declaration = signature.ok_or(Unsupported {
                        span,
                        feature: "generic call lost declaration signature",
                    })?;
                    let mut arguments = self
                        .budget
                        .vector(Retained, instance.type_arguments.len())?;
                    for ty in &instance.type_arguments {
                        let ty = self.ty(ty)?;
                        self.budget.push(Retained, &mut arguments, ty)?;
                    }
                    let effective = self.ty(&Type::Function(instance.signature.clone()))?;
                    let id = CallInstantiationId::from_index(
                        self.units[unit.index()].call_instantiations.len(),
                    )
                    .ok_or(Unsupported {
                        span,
                        feature: "generic call instance capacity",
                    })?;
                    self.budget.push(
                        Retained,
                        &mut self.units[unit.index()].call_instantiations,
                        CallInstantiation {
                            declaration,
                            arguments,
                            signature: effective,
                        },
                    )?;
                    Some(id)
                } else {
                    None
                };
                let contract = CallContract {
                    signature,
                    instantiation,
                    supplied: u32::try_from(args.len()).map_err(|_| Unsupported {
                        span,
                        feature: "semantic call argument capacity",
                    })?,
                    defaults,
                };
                self.prepare_call(unit, region, target, contract, args, callee.span())?
            }
            ExprKind::New { class, args, .. } => {
                let ExpressionResolution::Primitive(operation @ ResolvedIntrinsic::Constructor(_)) =
                    self.semantics.expression_resolution(expr.id)
                else {
                    return self.construct_class(unit, region, class.name, args, ty, origin, span);
                };
                let ResolvedIntrinsic::Constructor(constructor) = operation else {
                    unreachable!("matched constructor resolution")
                };
                if !crate::primitive::builtin_constructor(constructor) {
                    return self.unsupported(span, "semantic constructor call contract");
                }
                if crate::primitive::intrinsic_call_contract(operation)
                    .is_some_and(|signature| signature.receiver.is_some())
                {
                    return self.unsupported(span, "constructor has a method receiver");
                }
                let contract = CallContract {
                    signature: None,
                    instantiation: None,
                    supplied: u32::try_from(args.len()).map_err(|_| Unsupported {
                        span,
                        feature: "semantic constructor argument capacity",
                    })?,
                    defaults: operation.call_default_convention().ok_or(Unsupported {
                        span,
                        feature: "prepared constructor call convention",
                    })?,
                };
                self.prepare_call(
                    unit,
                    region,
                    CallTarget::Intrinsic {
                        operation,
                        receiver: None,
                    },
                    contract,
                    args,
                    class.span,
                )?
            }
            ExprKind::ArrowFunction { params, body, .. } => {
                let child = self.add_unit(UnitKind::Closure)?;
                self.units[child.index()].callable_type = Some(ty);
                self.parameters(child, params)?;
                let name = self.string("")?;
                self.units[child.index()].function_name = Some(name);
                let root = RegionId::from_index(0).unwrap();
                match body {
                    ArrowBody::Expr(expression) => {
                        let result = self.expression(child, root, expression)?;
                        let result = self.copy_value(child, root, result, expression.span())?;
                        self.effect(
                            child,
                            root,
                            OperationKind::Return,
                            &[result],
                            expression.span(),
                        )?;
                    }
                    ArrowBody::Block(body) => self.statements(child, root, body)?,
                }
                (OperationKind::Closure(child), vec![])
            }
            ExprKind::ArrayLiteral { elements, .. } => {
                let mut values = self.budget.vector(Scratch, elements.len())?;
                let spread = elements
                    .iter()
                    .any(|element| matches!(element, ArrayElement::Spread { .. }));
                let mut flags = self.budget.vector(Retained, if spread { elements.len() } else { 0 })?;
                for element in *elements {
                    let (value, spreads) = match element {
                        ArrayElement::Value(value) => (value, false),
                        ArrayElement::Spread { value, .. } => (value, true),
                    };
                    let result = self.expression(unit, region, value)?;
                    let value = if spreads {
                        result
                    } else {
                        self.copy_value(unit, region, result, value.span())?
                    };
                    self.budget.push(Scratch, &mut values, value)?;
                    if spread {
                        self.budget.push(Retained, &mut flags, spreads)?;
                    }
                }
                let kind = if spread {
                    AllocationKind::SpreadArray(flags)
                } else {
                    drop_vector(flags, Retained, self.budget)?;
                    AllocationKind::Array
                };
                (self.allocation(unit, kind)?, values)
            }
            ExprKind::RecordLiteral { entries, .. } | ExprKind::ObjectLiteral { entries, .. } => {
                let mut keys = self.budget.vector(Retained, entries.len())?;
                let mut values = self.budget.vector(Scratch, entries.len())?;
                for entry in *entries {
                    let RecordElement::Entry(entry) = entry else {
                        return self.unsupported(span, "spread record construction");
                    };
                    let key =
                        self.decoded_string(entry.key.name, entry.span, "record key decoding")?;
                    self.budget.push(Retained, &mut keys, key)?;
                    let value = self.expression(unit, region, &entry.value)?;
                    self.infer_creation_id(unit, &entry.value, value, key);
                    let value = self.copy_value(unit, region, value, entry.value.span())?;
                    self.budget.push(Scratch, &mut values, value)?;
                }
                let kind = if matches!(expr.kind, ExprKind::RecordLiteral { .. }) {
                    AllocationKind::Record(keys)
                } else {
                    AllocationKind::Object(keys)
                };
                (self.allocation(unit, kind)?, values)
            }
            ExprKind::StructLiteral { values, .. } => {
                let nominal = self
                    .semantics
                    .nominal_id(&self.program.types[ty.index()])
                    .ok_or(Unsupported {
                        span,
                        feature: "struct construction identity",
                    })?;
                let mut operands = self.budget.vector(Scratch, values.len())?;
                for value in *values {
                    let result = self.expression(unit, region, value)?;
                    let result = self.copy_value(unit, region, result, value.span())?;
                    self.budget.push(Scratch, &mut operands, result)?;
                }
                (
                    self.allocation(unit, AllocationKind::Struct(nominal))?,
                    operands,
                )
            }
            ExprKind::Template { parts, .. } => {
                return self.template(unit, region, parts, ty, origin, span);
            }
            ExprKind::TypeCheck {
                value,
                span: check,
                ..
            } => {
                let target = self.semantics.type_check_type(*check).ok_or(Unsupported {
                    span,
                    feature: "missing checked type-test target",
                })?;
                let target = self.ty(target)?;
                let value = self.expression(unit, region, value)?;
                (
                    OperationKind::TypeTest(target),
                    self.budget.copy_slice(Scratch, &[value])?,
                )
            }
            ExprKind::Await { task, .. } => {
                let task = self.expression(unit, region, task)?;
                (OperationKind::Await, self.budget.copy_slice(Scratch, &[task])?)
            }
            _ => return self.unsupported(span, "expression conversion"),
        };
        // A record literal typed by a `JsValue` context is still a record: it
        // is allocated as `Record<JsValue>`, which takes every entry.
        let ty = match (&kind, &self.program.types[ty.index()]) {
            (
                OperationKind::Allocate {
                    kind: AllocationKind::Record(_),
                    ..
                },
                Type::TypeParameter("$js"),
            ) => self.ty(&Type::Record(Box::new(Type::TypeParameter("$js"))))?,
            _ => ty,
        };
        let result = self.value(unit, region, kind, &operands, ty, origin, span)?;
        drop_vector(operands, Scratch, self.budget)?;
        Ok(result)
    }
    /// All calls, including construction, enter one checked scheduling
    /// envelope. The target is fixed before any supplied argument evaluation.
    /// Omitted defaults remain the responsibility of the retained contract.
    fn prepare_call(
        &mut self,
        unit: UnitId,
        region: RegionId,
        target: CallTarget,
        contract: CallContract,
        arguments: &'ast [ast::Argument<'ast, 'src>],
        preparation: Span,
    ) -> Result<(OperationKind, Vec<ValueId>), ConversionError> {
        let (kind, operands, _) =
            self.prepare_call_with_receiver(unit, region, target, contract, None, arguments, preparation)?;
        Ok((kind, operands))
    }
    /// A call whose operands are values the caller already evaluated, in order.
    fn prepare_call_values(
        &mut self,
        unit: UnitId,
        region: RegionId,
        target: CallTarget,
        contract: CallContract,
        values: &[ValueId],
        preparation: Span,
    ) -> Result<(OperationKind, Vec<ValueId>), ConversionError> {
        let call = CallId::from_index(self.units[unit.index()].calls.len()).ok_or(Unsupported {
            span: preparation,
            feature: "semantic call capacity",
        })?;
        self.budget.push(
            Retained,
            &mut self.units[unit.index()].calls,
            CallSite {
                target,
                contract,
                arguments: ArgumentRange { start: 0, len: 0 },
            },
        )?;
        self.effect(unit, region, OperationKind::PrepareCall(call), &[], preparation)?;
        let mut arguments = self.budget.vector(Scratch, values.len())?;
        for &value in values {
            self.budget
                .push(Scratch, &mut arguments, CallArgument::Value(value))?;
        }
        if contract.defaults == DefaultConvention::MaterializeAtCaller {
            self.materialize_defaults(unit, region, contract, &mut arguments, preparation)?;
        }
        let data = &mut self.units[unit.index()];
        let start = u32::try_from(data.call_arguments.len()).map_err(|_| Unsupported {
            span: preparation,
            feature: "semantic call argument capacity",
        })?;
        let len = u32::try_from(arguments.len()).map_err(|_| Unsupported {
            span: preparation,
            feature: "semantic call argument capacity",
        })?;
        self.budget
            .extend_copy(Retained, &mut data.call_arguments, &arguments)?;
        self.units[unit.index()].calls[call.index()].arguments = ArgumentRange { start, len };
        drop_vector(arguments, Scratch, self.budget)?;
        Ok((OperationKind::Call(call), Vec::new()))
    }
    /// `receiver`, when present, is the already-evaluated first argument: the
    /// instance a class method or `init` runs on.
    fn prepare_call_with_receiver(
        &mut self,
        unit: UnitId,
        region: RegionId,
        target: CallTarget,
        contract: CallContract,
        receiver: Option<CallReceiver<'src>>,
        arguments: &'ast [ast::Argument<'ast, 'src>],
        preparation: Span,
    ) -> Result<(OperationKind, Vec<ValueId>, Option<ValueId>), ConversionError> {
        let call = CallId::from_index(self.units[unit.index()].calls.len()).ok_or(Unsupported {
            span: preparation,
            feature: "semantic call capacity",
        })?;
        self.budget.push(
            Retained,
            &mut self.units[unit.index()].calls,
            CallSite {
                target,
                contract,
                arguments: ArgumentRange { start: 0, len: 0 },
            },
        )?;
        self.effect(
            unit,
            region,
            OperationKind::PrepareCall(call),
            &[],
            preparation,
        )?;
        let mut values = self.budget.vector(Scratch, arguments.len() + 1)?;
        let mut receiver_value = None;
        if let Some(CallReceiver::Value(receiver)) = receiver {
            self.budget
                .push(Scratch, &mut values, CallArgument::Value(receiver))?;
            receiver_value = Some(receiver);
        }
        let offset = usize::from(receiver.is_some());
        for (index, argument) in arguments.iter().enumerate() {
            let position = index + offset;
            match argument.passing {
                crate::primitive::ParameterPassing::Value => {
                    let value = self.expression(unit, region, &argument.expression)?;
                    let value = self.copy_value(unit, region, value, argument.span)?;
                    self.budget
                        .push(Scratch, &mut values, CallArgument::Value(value))?;
                }
                crate::primitive::ParameterPassing::MutableReference => {
                    let place = self.place(unit, region, &argument.expression)?;
                    self.effect(
                        unit,
                        region,
                        OperationKind::PrepareReference {
                            call,
                            position: position as u32,
                        },
                        &[],
                        argument.span,
                    )?;
                    self.budget
                        .push(Scratch, &mut values, CallArgument::Reference(place))?;
                }
            }
        }
        // A construction's instance exists once its explicit arguments are
        // evaluated and before its parameter defaults, as in JavaScript: a
        // class's fields are initialized on entry, ahead of the defaults.
        if let Some(CallReceiver::Construction { class, ty, origin }) = receiver {
            let instance = self.class_instance(unit, region, class, ty, origin, preparation)?;
            values.insert(0, CallArgument::Value(instance));
            receiver_value = Some(instance);
        }
        if contract.defaults == DefaultConvention::MaterializeAtCaller {
            self.materialize_defaults(unit, region, contract, &mut values, preparation)?;
        }
        let data = &mut self.units[unit.index()];
        let start = u32::try_from(data.call_arguments.len()).map_err(|_| Unsupported {
            span: preparation,
            feature: "semantic call argument capacity",
        })?;
        let len = u32::try_from(values.len()).map_err(|_| Unsupported {
            span: preparation,
            feature: "semantic call argument capacity",
        })?;
        start.checked_add(len).ok_or(Unsupported {
            span: preparation,
            feature: "semantic call argument capacity",
        })?;
        self.budget
            .extend_copy(Retained, &mut data.call_arguments, &values)?;
        data.calls[call.index()].arguments = ArgumentRange { start, len };
        drop_vector(values, Scratch, self.budget)?;
        Ok((OperationKind::Call(call), Vec::new(), receiver_value))
    }
    /// Omitted parameters with checked defaults are evaluated by the caller
    /// after every supplied argument, as the callee would evaluate them on
    /// entry. A default naming an earlier parameter reuses that argument.
    fn materialize_defaults(
        &mut self,
        unit: UnitId,
        region: RegionId,
        contract: CallContract,
        values: &mut Vec<CallArgument>,
        span: Span,
    ) -> Result<(), ConversionError> {
        let Some(declared) = contract.signature else {
            return Ok(());
        };
        // A generic callee's parameter types come from this call's instance.
        let effective = contract
            .instantiation
            .map(|id| self.units[unit.index()].call_instantiations[id.index()].signature)
            .unwrap_or(declared);
        let signature = match &self.program.types[effective.index()] {
            Type::Function(signature) => signature.clone(),
            _ => return Ok(()),
        };
        let declared = match &self.program.types[declared.index()] {
            Type::Function(signature) => signature.clone(),
            Type::GenericFunction(function) => function.signature.clone(),
            _ => return Ok(()),
        };
        // Trailing parameters whose default only the callee can build (an
        // arrow) are simply omitted; JavaScript supplies `undefined` and the
        // guarded body applies them.
        let mut end = signature.params.len();
        while end > values.len()
            && matches!(
                declared.params[end - 1].default,
                Some(crate::check::DefaultValue::Arrow(_))
            )
        {
            end -= 1;
        }
        for position in values.len()..end {
            self.work(1)?;
            let Some(default) = declared.params[position].default.as_ref() else {
                return self.unsupported(span, "omitted argument without a checked default");
            };
            if matches!(default, crate::check::DefaultValue::Arrow(_)) {
                return self.unsupported(span, "arrow default before a caller-evaluated default");
            }
            let ty = self.ty(&signature.params[position].ty)?;
            let value = self.default_value(unit, region, default, ty, values, false, span)?;
            self.budget.push(Scratch, values, CallArgument::Value(value))?;
        }
        Ok(())
    }
    /// `callee` is true inside the guarded body, false at a typed call site.
    fn default_value(
        &mut self,
        unit: UnitId,
        region: RegionId,
        default: &crate::check::DefaultValue<'src>,
        ty: TypeId,
        values: &[CallArgument],
        callee: bool,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        use crate::check::DefaultValue;
        let constant = match default {
            // Converting one source arrow at every call site would redeclare
            // its parameters. Callers omit it; the guarded body creates the
            // closure, once per call, as JavaScript does.
            DefaultValue::Arrow(_) if !callee => {
                return self.unsupported(span, "arrow default evaluated by a caller");
            }
            DefaultValue::Arrow(source) => {
                let expression = self.semantics.source_expression(*source).ok_or(Unsupported {
                    span,
                    feature: "missing checked arrow default",
                })?;
                return self.expression(unit, region, expression);
            }
            DefaultValue::Struct { name, values: fields } => {
                let identity = self
                    .semantics
                    .struct_info(name)
                    .map(|info| info.declaration.identity)
                    .ok_or(Unsupported {
                        span,
                        feature: "missing checked struct default",
                    })?;
                let schema = self
                    .program
                    .structs
                    .iter()
                    .position(|definition| definition.identity == identity)
                    .ok_or(Unsupported {
                        span,
                        feature: "struct default before its schema",
                    })?;
                let range = self.program.structs[schema].fields.clone();
                if range.len() != fields.len() {
                    return self.unsupported(span, "struct default has the wrong field count");
                }
                let mut operands = self.budget.vector(Scratch, fields.len())?;
                for (offset, field) in fields.iter().enumerate() {
                    let field_ty = self.program.fields[range.start + offset].ty;
                    let value = self.default_value(unit, region, field, field_ty, values, callee, span)?;
                    self.budget.push(Scratch, &mut operands, value)?;
                }
                let kind = self.allocation(unit, AllocationKind::Struct(identity))?;
                let result = self.value(unit, region, kind, &operands, ty, None, span)?;
                drop_vector(operands, Scratch, self.budget)?;
                return Ok(result);
            }
            DefaultValue::NewClass { name, args } => {
                let signature = self
                    .semantics
                    .class_info(name)
                    .and_then(|info| info.constructor.clone());
                let mut operands = self.budget.vector(Scratch, args.len())?;
                for (index, argument) in args.iter().enumerate() {
                    let parameter = signature
                        .as_ref()
                        .and_then(|signature| signature.params.get(index))
                        .map(|parameter| parameter.ty.clone())
                        .ok_or(Unsupported {
                            span,
                            feature: "constructor default without a checked parameter",
                        })?;
                    let parameter = self.ty(&parameter)?;
                    let value = self.default_value(unit, region, argument, parameter, values, callee, span)?;
                    self.budget.push(Scratch, &mut operands, value)?;
                }
                let result = self.construct_class_values(unit, region, name, &operands, ty, None, span)?;
                drop_vector(operands, Scratch, self.budget)?;
                return Ok(result);
            }
            DefaultValue::Int(value) if matches!(self.program.types[ty.index()], Type::Float) => {
                Constant::Number((*value as f64).to_bits())
            }
            DefaultValue::Int(value) => Constant::Integer(i32::try_from(*value).map_err(|_| {
                Unsupported {
                    span,
                    feature: "integer default outside the language domain",
                }
            })?),
            DefaultValue::Float(bits) => Constant::Number(*bits),
            DefaultValue::String(text) => Constant::String(self.decoded_string(
                text,
                span,
                "invalid checked default string",
            )?),
            DefaultValue::Bool(value) => Constant::Boolean(*value),
            DefaultValue::Null => Constant::Null,
            DefaultValue::Undefined => Constant::Undefined,
            DefaultValue::Parameter(index) => {
                let Some(CallArgument::Value(value)) = values.get(*index) else {
                    return self.unsupported(span, "default names an unavailable parameter");
                };
                return self.copy_value(unit, region, *value, span);
            }
            DefaultValue::Symbol(symbol) => {
                let cell = CellId::from_index(symbol.0 as usize).ok_or(AllocationError::Capacity)?;
                self.reference(unit, cell)?;
                let value = self.load_cell(unit, region, cell, span)?;
                return self.copy_value(unit, region, value, span);
            }
            DefaultValue::Array(elements) => {
                let element = match &self.program.types[ty.index()] {
                    Type::Array(element) => self.ty(&element.clone())?,
                    _ => return self.unsupported(span, "array default for a non-array parameter"),
                };
                let mut items = self.budget.vector(Scratch, elements.len())?;
                for item in elements {
                    let value = self.default_value(unit, region, item, element, values, callee, span)?;
                    self.budget.push(Scratch, &mut items, value)?;
                }
                let kind = self.allocation(unit, AllocationKind::Array)?;
                let result = self.value(unit, region, kind, &items, ty, None, span)?;
                drop_vector(items, Scratch, self.budget)?;
                return Ok(result);
            }
            _ => return self.unsupported(span, "unresolved checked default"),
        };
        self.value(unit, region, OperationKind::Constant(constant), &[], ty, None, span)
    }
    fn allocation(
        &mut self,
        unit: UnitId,
        kind: AllocationKind,
    ) -> Result<OperationKind, ConversionError> {
        let identity = AllocationId::from_index(self.allocations[unit.index()])
            .ok_or(AllocationError::Capacity)?;
        self.allocations[unit.index()] += 1;
        Ok(OperationKind::Allocate { identity, kind })
    }
}

#[cfg(test)]
#[path = "lower_admission_tests.rs"]
mod tests;

/// Converting these to a string runs no user code and cannot throw.
fn effect_free_string_conversion(ty: &Type<'_>) -> bool {
    match ty {
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Enum(_) => true,
        Type::Union(members) => members.iter().all(effect_free_string_conversion),
        _ => false,
    }
}

/// The class a `base` type names (`Base` or `Base<T>`).
fn base_class_name<'src>(base: &Type<'src>) -> Option<&'src str> {
    match base {
        Type::Class(name) => Some(name),
        Type::ClassInstance { name, .. } => Some(name),
        _ => None,
    }
}

/// Whether control never continues past `statement`: every path returns,
/// throws or jumps.
fn terminates(statement: &Stmt<'_, '_>) -> bool {
    match statement {
        Stmt::Return { .. } | Stmt::Throw { .. } | Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::Block { body, .. } => body.iter().any(terminates),
        Stmt::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => terminates(then_branch) && terminates(else_branch),
        _ => false,
    }
}

/// Whether `statement` declares a binding in its enclosing block.
fn declares(statement: &Stmt<'_, '_>) -> bool {
    matches!(
        statement,
        Stmt::VarDecl(_) | Stmt::ArrayDestructure { .. } | Stmt::RecordDestructure { .. }
    )
}
