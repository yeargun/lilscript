use std::collections::hash_map::Entry;
use std::fs;
use std::path::{Path, PathBuf};

use ahash::{AHashMap, AHashSet};
use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::ast::{
    ArrowBody, ClassDecl, ClassMember, ConstructorDecl, ExportDecl, Expr, ExternDecl, FieldDecl,
    ForInitializer, FunctionDecl, Ident, Item, Param, Program, Stmt, StructDecl, TemplatePart,
    TypeKind, TypeRef, VarDecl,
};
use crate::parser::{parse_source, ParseError};
use crate::span::Span;

pub type ModuleId = usize;

#[derive(Debug, Clone)]
pub struct ModuleSource {
    pub path: PathBuf,
    pub source: String,
    pub dependencies: Vec<ModuleId>,
    pub offset: usize,
}

#[derive(Debug, Clone)]
pub struct ModuleSet {
    pub modules: Vec<ModuleSource>,
    pub dependency_order: Vec<ModuleId>,
    pub root: ModuleId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleError {
    pub path: PathBuf,
    pub source: String,
    pub span: Span,
    pub message: String,
}

impl ModuleError {
    pub(crate) fn new(
        path: impl Into<PathBuf>,
        source: impl Into<String>,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
            span,
            message: message.into(),
        }
    }

    pub fn from_parse(path: &Path, source: &str, error: ParseError) -> Self {
        Self::new(path, source, error.span, error.message)
    }
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for ModuleError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Complete,
}

pub fn discover_modules(root: &Path) -> Result<ModuleSet, ModuleError> {
    discover_modules_inner(root, None)
}

pub fn discover_modules_with_source(root: &Path, source: &str) -> Result<ModuleSet, ModuleError> {
    discover_modules_inner(root, Some(source))
}

fn discover_modules_inner(
    root: &Path,
    root_source: Option<&str>,
) -> Result<ModuleSet, ModuleError> {
    let root_path = canonical_module_path(root).map_err(|message| {
        ModuleError::new(root, root_source.unwrap_or(""), Span::empty(0), message)
    })?;
    let mut overrides = AHashMap::new();
    if let Some(source) = root_source {
        overrides.insert(root_path.clone(), source.to_string());
    }
    let mut loader = ModuleLoader {
        modules: Vec::new(),
        by_path: AHashMap::new(),
        states: Vec::new(),
        dependency_order: Vec::new(),
        stack: Vec::new(),
        overrides,
    };
    let root = loader.visit(&root_path, None)?;
    let mut offset = 0usize;
    for module in &mut loader.modules {
        module.offset = offset;
        offset = offset.saturating_add(module.source.len()).saturating_add(1);
    }
    Ok(ModuleSet {
        modules: loader.modules,
        dependency_order: loader.dependency_order,
        root,
    })
}

struct ModuleLoader {
    modules: Vec<ModuleSource>,
    by_path: AHashMap<PathBuf, ModuleId>,
    states: Vec<VisitState>,
    dependency_order: Vec<ModuleId>,
    stack: Vec<ModuleId>,
    overrides: AHashMap<PathBuf, String>,
}

impl ModuleLoader {
    fn visit(
        &mut self,
        requested: &Path,
        import_site: Option<(&Path, &str, Span)>,
    ) -> Result<ModuleId, ModuleError> {
        let path = canonical_module_path(requested).map_err(|message| {
            let (path, source, span) = import_site.unwrap_or((requested, "", Span::empty(0)));
            ModuleError::new(path, source, span, message)
        })?;
        if let Some(&id) = self.by_path.get(&path) {
            if self.states[id] == VisitState::Visiting {
                let cycle_start = self
                    .stack
                    .iter()
                    .position(|candidate| *candidate == id)
                    .unwrap_or(0);
                let mut cycle = self.stack[cycle_start..]
                    .iter()
                    .map(|module| display_module_path(&self.modules[*module].path))
                    .collect::<Vec<_>>();
                cycle.push(display_module_path(&path));
                let (site_path, source, span) = import_site.unwrap_or((&path, "", Span::empty(0)));
                return Err(ModuleError::new(
                    site_path,
                    source,
                    span,
                    format!("cyclic module import: {}", cycle.join(" -> ")),
                ));
            }
            return Ok(id);
        }

        let source = self
            .overrides
            .get(&path)
            .cloned()
            .map_or_else(|| fs::read_to_string(&path), Ok)
            .map_err(|error| {
                let (site_path, site_source, span) =
                    import_site.unwrap_or((&path, "", Span::empty(0)));
                ModuleError::new(
                    site_path,
                    site_source,
                    span,
                    format!("failed to read module {}: {error}", path.display()),
                )
            })?;
        let arena = Bump::new();
        let program = parse_source(&arena, &source)
            .map_err(|error| ModuleError::from_parse(&path, &source, error))?;
        let imports = program
            .imports
            .iter()
            .map(|import| (import.source.to_string(), import.span))
            .collect::<Vec<_>>();

        let id = self.modules.len();
        self.by_path.insert(path.clone(), id);
        self.states.push(VisitState::Visiting);
        self.modules.push(ModuleSource {
            path: path.clone(),
            source,
            dependencies: Vec::with_capacity(imports.len()),
            offset: 0,
        });
        self.stack.push(id);

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let site_source = self.modules[id].source.clone();
        let mut dependencies = Vec::with_capacity(imports.len());
        for (specifier, span) in imports {
            let dependency_path = resolve_import_path(parent, &specifier).map_err(|message| {
                ModuleError::new(&path, &self.modules[id].source, span, message)
            })?;
            let dependency = self.visit(&dependency_path, Some((&path, &site_source, span)))?;
            dependencies.push(dependency);
        }
        self.modules[id].dependencies = dependencies;
        self.stack.pop();
        self.states[id] = VisitState::Complete;
        self.dependency_order.push(id);
        Ok(id)
    }
}

fn canonical_module_path(path: &Path) -> Result<PathBuf, String> {
    let with_extension = if path.extension().is_none() {
        path.with_extension("lil")
    } else {
        path.to_path_buf()
    };
    if with_extension.extension().and_then(|value| value.to_str()) != Some("lil") {
        return Err(format!(
            "LilScript modules must use the `.lil` extension: {}",
            with_extension.display()
        ));
    }
    with_extension.canonicalize().map_err(|error| {
        format!(
            "cannot resolve LilScript module {}: {error}",
            with_extension.display()
        )
    })
}

fn resolve_import_path(parent: &Path, specifier: &str) -> Result<PathBuf, String> {
    let path = Path::new(specifier);
    if path.is_absolute() || !specifier.starts_with('.') {
        return Err(format!(
            "module path `{specifier}` must be relative and begin with `./` or `../`"
        ));
    }
    Ok(parent.join(path))
}

fn display_module_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| path.to_str().unwrap_or("<module>"))
        .to_string()
}

pub fn parse_modules<'arena>(
    arena: &'arena Bump,
    modules: &'arena ModuleSet,
) -> Result<Vec<Program<'arena, 'arena>>, ModuleError> {
    modules
        .modules
        .iter()
        .map(|module| {
            parse_source(arena, &module.source)
                .map_err(|error| ModuleError::from_parse(&module.path, &module.source, error))
        })
        .collect()
}

pub fn link_modules<'arena>(
    arena: &'arena Bump,
    modules: &'arena ModuleSet,
    programs: &'arena [Program<'arena, 'arena>],
) -> Result<Program<'arena, 'arena>, ModuleError> {
    let mut bindings = Vec::with_capacity(programs.len());
    for (module_id, program) in programs.iter().enumerate() {
        let mut module_bindings = AHashMap::new();
        for item in program.items {
            let Some(name) = top_level_name(item) else {
                continue;
            };
            let internal = if module_id == modules.root || matches!(item, Item::Extern(_)) {
                name.name
            } else {
                let generated = format!("$m{module_id}${}", name.name);
                &*arena.alloc_str(&generated)
            };
            if module_bindings.insert(name.name, internal).is_some() {
                return Err(module_error_at(
                    modules,
                    module_id,
                    name.span,
                    format!("duplicate module binding `{}`", name.name),
                ));
            }
        }
        bindings.push(module_bindings);
    }

    let mut exports = vec![AHashMap::new(); programs.len()];
    for &module_id in &modules.dependency_order {
        let program = &programs[module_id];
        if program.imports.len() != modules.modules[module_id].dependencies.len() {
            return Err(module_error_at(
                modules,
                module_id,
                program.span,
                "internal module dependency mismatch",
            ));
        }
        for (import_index, import) in program.imports.iter().enumerate() {
            let dependency = modules.modules[module_id].dependencies[import_index];
            for specifier in import.specifiers {
                let Some(&internal) = exports[dependency].get(specifier.imported.name) else {
                    return Err(module_error_at(
                        modules,
                        module_id,
                        specifier.imported.span,
                        format!(
                            "module `{}` does not export `{}`",
                            import.source, specifier.imported.name
                        ),
                    ));
                };
                match bindings[module_id].entry(specifier.local.name) {
                    Entry::Vacant(entry) => {
                        entry.insert(internal);
                    }
                    Entry::Occupied(_) => {
                        return Err(module_error_at(
                            modules,
                            module_id,
                            specifier.local.span,
                            format!("duplicate module binding `{}`", specifier.local.name),
                        ));
                    }
                }
            }
        }
        exports[module_id] =
            resolve_exports(modules, module_id, program.exports, &bindings[module_id])?;
    }

    let mut items = BumpVec::new_in(arena);
    let mut seen_externs = AHashMap::<&str, ExternDecl<'arena, 'arena>>::new();
    for &module_id in &modules.dependency_order {
        let mut cloner = ModuleCloner::new(
            arena,
            &bindings[module_id],
            modules.modules[module_id].offset,
        );
        for item in programs[module_id].items {
            let cloned = cloner.clone_item(item);
            if let Item::Extern(decl) = &cloned {
                match seen_externs.entry(decl.name.name) {
                    Entry::Vacant(entry) => {
                        entry.insert(decl.clone());
                    }
                    Entry::Occupied(entry) => {
                        if !extern_contracts_match(entry.get(), decl) {
                            return Err(module_error_at(
                                modules,
                                module_id,
                                item.span(),
                                format!(
                                    "extern `{}` has conflicting declarations across modules",
                                    decl.name.name
                                ),
                            ));
                        }
                        continue;
                    }
                }
            }
            items.push(cloned);
        }
    }
    let items = items.into_bump_slice();
    let root = modules.root;
    let mut linked_exports = BumpVec::new_in(arena);
    for export in programs[root].exports {
        let internal = bindings[root]
            .get(export.local.name)
            .copied()
            .ok_or_else(|| {
                module_error_at(
                    modules,
                    root,
                    export.local.span,
                    format!(
                        "cannot export unknown module binding `{}`",
                        export.local.name
                    ),
                )
            })?;
        let offset = modules.modules[root].offset;
        linked_exports.push(ExportDecl {
            local: Ident {
                name: internal,
                span: Span::new(
                    export.local.span.start + offset,
                    export.local.span.end + offset,
                ),
            },
            exported: Ident {
                name: export.exported.name,
                span: Span::new(
                    export.exported.span.start + offset,
                    export.exported.span.end + offset,
                ),
            },
            span: Span::new(export.span.start + offset, export.span.end + offset),
        });
    }
    let span = items
        .first()
        .zip(items.last())
        .map_or(Span::empty(0), |(first, last)| {
            first.span().merge(last.span())
        });
    Ok(Program {
        imports: &[],
        exports: linked_exports.into_bump_slice(),
        items,
        span,
    })
}

fn extern_contracts_match(left: &ExternDecl<'_, '_>, right: &ExternDecl<'_, '_>) -> bool {
    left.declared_pure == right.declared_pure
        && type_contracts_match(left.return_type, right.return_type)
        && left.params.len() == right.params.len()
        && left
            .params
            .iter()
            .zip(right.params)
            .all(|(left, right)| type_contracts_match(left.ty, right.ty))
}

fn type_contracts_match(left: TypeRef<'_, '_>, right: TypeRef<'_, '_>) -> bool {
    match (left.kind, right.kind) {
        (TypeKind::Int, TypeKind::Int)
        | (TypeKind::Float, TypeKind::Float)
        | (TypeKind::String, TypeKind::String)
        | (TypeKind::Bool, TypeKind::Bool)
        | (TypeKind::Void, TypeKind::Void)
        | (TypeKind::Auto, TypeKind::Auto) => true,
        (
            TypeKind::Named {
                name: left,
                args: left_args,
            },
            TypeKind::Named {
                name: right,
                args: right_args,
            },
        ) => {
            left == right
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| type_contracts_match(*left, *right))
        }
        (TypeKind::Array(left), TypeKind::Array(right)) => type_contracts_match(*left, *right),
        (TypeKind::Nullable(left), TypeKind::Nullable(right)) => {
            type_contracts_match(*left, *right)
        }
        (
            TypeKind::Function {
                params: left_params,
                return_type: left_return,
            },
            TypeKind::Function {
                params: right_params,
                return_type: right_return,
            },
        ) => {
            left_params.len() == right_params.len()
                && left_params
                    .iter()
                    .zip(right_params)
                    .all(|(left, right)| type_contracts_match(*left, *right))
                && type_contracts_match(*left_return, *right_return)
        }
        _ => false,
    }
}

fn resolve_exports<'arena>(
    modules: &ModuleSet,
    module_id: ModuleId,
    declarations: &[ExportDecl<'arena>],
    bindings: &AHashMap<&'arena str, &'arena str>,
) -> Result<AHashMap<&'arena str, &'arena str>, ModuleError> {
    let mut exports = AHashMap::new();
    for export in declarations {
        let Some(&internal) = bindings.get(export.local.name) else {
            return Err(module_error_at(
                modules,
                module_id,
                export.local.span,
                format!(
                    "cannot export unknown module binding `{}`",
                    export.local.name
                ),
            ));
        };
        if exports.insert(export.exported.name, internal).is_some() {
            return Err(module_error_at(
                modules,
                module_id,
                export.exported.span,
                format!("duplicate export `{}`", export.exported.name),
            ));
        }
    }
    Ok(exports)
}

fn top_level_name<'src>(item: &Item<'_, 'src>) -> Option<Ident<'src>> {
    match item {
        Item::Struct(decl) => Some(decl.name),
        Item::Class(decl) => Some(decl.name),
        Item::Function(decl) => Some(decl.name),
        Item::Extern(decl) => Some(decl.name),
        Item::Stmt(Stmt::VarDecl(decl)) => Some(decl.name),
        Item::Stmt(_) => None,
    }
}

fn module_error_at(
    modules: &ModuleSet,
    module_id: ModuleId,
    span: Span,
    message: impl Into<String>,
) -> ModuleError {
    let module = &modules.modules[module_id];
    ModuleError::new(&module.path, &module.source, span, message)
}

pub fn locate_linked_span(modules: &ModuleSet, span: Span) -> (&ModuleSource, Span) {
    let module = modules
        .modules
        .iter()
        .rev()
        .find(|module| span.start >= module.offset)
        .unwrap_or(&modules.modules[modules.root]);
    (
        module,
        Span::new(
            span.start
                .saturating_sub(module.offset)
                .min(module.source.len()),
            span.end
                .saturating_sub(module.offset)
                .min(module.source.len()),
        ),
    )
}

struct ModuleCloner<'arena, 'map> {
    arena: &'arena Bump,
    globals: &'map AHashMap<&'arena str, &'arena str>,
    scopes: Vec<AHashSet<&'arena str>>,
    offset: usize,
}

impl<'arena, 'map> ModuleCloner<'arena, 'map> {
    fn new(
        arena: &'arena Bump,
        globals: &'map AHashMap<&'arena str, &'arena str>,
        offset: usize,
    ) -> Self {
        Self {
            arena,
            globals,
            scopes: Vec::new(),
            offset,
        }
    }

    fn clone_item(&mut self, item: &Item<'arena, 'arena>) -> Item<'arena, 'arena> {
        match item {
            Item::Struct(decl) => Item::Struct(self.clone_struct(decl)),
            Item::Class(decl) => Item::Class(self.clone_class(decl)),
            Item::Function(decl) => Item::Function(self.clone_function(decl, true)),
            Item::Extern(decl) => Item::Extern(ExternDecl {
                declared_pure: decl.declared_pure,
                return_type: self.clone_type(decl.return_type),
                name: self.global_ident(decl.name),
                type_params: self.clone_idents(decl.type_params),
                params: self.clone_params(decl.params),
                span: self.span(decl.span),
            }),
            Item::Stmt(stmt) => Item::Stmt(self.clone_stmt(stmt, true)),
        }
    }

    fn clone_struct(&mut self, decl: &StructDecl<'arena, 'arena>) -> StructDecl<'arena, 'arena> {
        StructDecl {
            name: self.global_ident(decl.name),
            type_params: self.clone_idents(decl.type_params),
            fields: self.clone_fields(decl.fields),
            span: self.span(decl.span),
        }
    }

    fn clone_class(&mut self, decl: &ClassDecl<'arena, 'arena>) -> ClassDecl<'arena, 'arena> {
        let mut members = BumpVec::new_in(self.arena);
        for member in decl.members {
            members.push(match member {
                ClassMember::Field(field) => ClassMember::Field(self.clone_field(field)),
                ClassMember::Constructor(constructor) => {
                    self.push_scope();
                    let params = self.clone_params_and_declare(constructor.params);
                    let body = self.clone_statements(constructor.body);
                    self.pop_scope();
                    ClassMember::Constructor(ConstructorDecl {
                        params,
                        body,
                        span: self.span(constructor.span),
                    })
                }
                ClassMember::Method(method) => {
                    ClassMember::Method(self.clone_function(method, false))
                }
            });
        }
        ClassDecl {
            name: self.global_ident(decl.name),
            type_params: self.clone_idents(decl.type_params),
            members: members.into_bump_slice(),
            span: self.span(decl.span),
        }
    }

    fn clone_function(
        &mut self,
        decl: &FunctionDecl<'arena, 'arena>,
        global_name: bool,
    ) -> FunctionDecl<'arena, 'arena> {
        self.push_scope();
        let params = self.clone_params_and_declare(decl.params);
        let body = self.clone_statements(decl.body);
        self.pop_scope();
        FunctionDecl {
            declared_pure: decl.declared_pure,
            return_type: self.clone_type(decl.return_type),
            name: if global_name {
                self.global_ident(decl.name)
            } else {
                self.plain_ident(decl.name)
            },
            type_params: self.clone_idents(decl.type_params),
            params,
            body,
            span: self.span(decl.span),
        }
    }

    fn clone_fields(
        &mut self,
        fields: &[FieldDecl<'arena, 'arena>],
    ) -> &'arena [FieldDecl<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        cloned.extend(fields.iter().map(|field| self.clone_field(field)));
        cloned.into_bump_slice()
    }

    fn clone_field(&self, field: &FieldDecl<'arena, 'arena>) -> FieldDecl<'arena, 'arena> {
        FieldDecl {
            ty: self.clone_type(field.ty),
            name: self.plain_ident(field.name),
            span: self.span(field.span),
        }
    }

    fn clone_params(
        &mut self,
        params: &[Param<'arena, 'arena>],
    ) -> &'arena [Param<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        cloned.extend(params.iter().map(|param| Param {
            ty: self.clone_type(param.ty),
            name: self.plain_ident(param.name),
            default: param.default.as_ref().map(|value| self.clone_expr(value)),
            span: self.span(param.span),
        }));
        cloned.into_bump_slice()
    }

    fn clone_params_and_declare(
        &mut self,
        params: &[Param<'arena, 'arena>],
    ) -> &'arena [Param<'arena, 'arena>] {
        let cloned = self.clone_params(params);
        for param in cloned {
            self.declare_local(param.name.name);
        }
        cloned
    }

    fn clone_statements(
        &mut self,
        statements: &[Stmt<'arena, 'arena>],
    ) -> &'arena [Stmt<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        for statement in statements {
            cloned.push(self.clone_stmt(statement, false));
        }
        cloned.into_bump_slice()
    }

    fn clone_stmt(&mut self, stmt: &Stmt<'arena, 'arena>, top_level: bool) -> Stmt<'arena, 'arena> {
        match stmt {
            Stmt::VarDecl(decl) => Stmt::VarDecl(self.clone_var_decl(decl, top_level)),
            Stmt::Expr(expr) => Stmt::Expr(self.clone_expr(expr)),
            Stmt::Return { value, span } => Stmt::Return {
                value: value.as_ref().map(|value| self.clone_expr(value)),
                span: self.span(*span),
            },
            Stmt::Block { body, span } => {
                self.push_scope();
                let body = self.clone_statements(body);
                self.pop_scope();
                Stmt::Block {
                    body,
                    span: self.span(*span),
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => Stmt::If {
                condition: self.clone_expr(condition),
                then_branch: self.clone_scoped_stmt(then_branch),
                else_branch: else_branch.map(|branch| self.clone_scoped_stmt(branch)),
                span: self.span(*span),
            },
            Stmt::While {
                condition,
                body,
                span,
            } => Stmt::While {
                condition: self.clone_expr(condition),
                body: self.clone_scoped_stmt(body),
                span: self.span(*span),
            },
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                span,
            } => {
                self.push_scope();
                let initializer = initializer.as_ref().map(|initializer| match initializer {
                    ForInitializer::VarDecl(decl) => {
                        ForInitializer::VarDecl(self.clone_var_decl(decl, false))
                    }
                    ForInitializer::Expr(expr) => ForInitializer::Expr(self.clone_expr(expr)),
                });
                let condition = condition.as_ref().map(|expr| self.clone_expr(expr));
                let update = update.as_ref().map(|expr| self.clone_expr(expr));
                let body = self.arena.alloc(self.clone_stmt(body, false));
                self.pop_scope();
                Stmt::For {
                    initializer,
                    condition,
                    update,
                    body,
                    span: self.span(*span),
                }
            }
            Stmt::Break(span) => Stmt::Break(self.span(*span)),
            Stmt::Continue(span) => Stmt::Continue(self.span(*span)),
        }
    }

    fn clone_scoped_stmt(
        &mut self,
        statement: &Stmt<'arena, 'arena>,
    ) -> &'arena Stmt<'arena, 'arena> {
        self.push_scope();
        let cloned = self.clone_stmt(statement, false);
        self.pop_scope();
        self.arena.alloc(cloned)
    }

    fn clone_var_decl(
        &mut self,
        decl: &VarDecl<'arena, 'arena>,
        top_level: bool,
    ) -> VarDecl<'arena, 'arena> {
        let initializer = decl
            .initializer
            .as_ref()
            .map(|initializer| self.clone_expr(initializer));
        let name = if top_level {
            self.global_ident(decl.name)
        } else {
            let name = self.plain_ident(decl.name);
            self.declare_local(name.name);
            name
        };
        VarDecl {
            ty: self.clone_type(decl.ty),
            name,
            initializer,
            span: self.span(decl.span),
        }
    }

    fn clone_expr(&mut self, expr: &Expr<'arena, 'arena>) -> Expr<'arena, 'arena> {
        match expr {
            Expr::Int(value, span) => Expr::Int(*value, self.span(*span)),
            Expr::Float(value, span) => Expr::Float(*value, self.span(*span)),
            Expr::String(value, span) => Expr::String(value, self.span(*span)),
            Expr::Bool(value, span) => Expr::Bool(*value, self.span(*span)),
            Expr::Null(span) => Expr::Null(self.span(*span)),
            Expr::Ident(ident) => Expr::Ident(self.reference_ident(*ident)),
            Expr::ArrayLiteral { elements, span } => Expr::ArrayLiteral {
                elements: self.clone_exprs(elements),
                span: self.span(*span),
            },
            Expr::StructLiteral { name, values, span } => Expr::StructLiteral {
                name: self.global_ident(*name),
                values: self.clone_exprs(values),
                span: self.span(*span),
            },
            Expr::New {
                class,
                type_args,
                args,
                span,
            } => Expr::New {
                class: self.global_ident(*class),
                type_args: self.clone_types(type_args),
                args: self.clone_exprs(args),
                span: self.span(*span),
            },
            Expr::Member {
                object,
                property,
                span,
            } => Expr::Member {
                object: self.arena.alloc(self.clone_expr(object)),
                property: self.plain_ident(*property),
                span: self.span(*span),
            },
            Expr::Call { callee, args, span } => Expr::Call {
                callee: self.arena.alloc(self.clone_expr(callee)),
                args: self.clone_exprs(args),
                span: self.span(*span),
            },
            Expr::ArrowFunction { params, body, span } => {
                self.push_scope();
                let params = self.clone_params_and_declare(params);
                let body = match body {
                    ArrowBody::Expr(expr) => {
                        ArrowBody::Expr(self.arena.alloc(self.clone_expr(expr)))
                    }
                    ArrowBody::Block(body) => ArrowBody::Block(self.clone_statements(body)),
                };
                self.pop_scope();
                Expr::ArrowFunction {
                    params,
                    body,
                    span: self.span(*span),
                }
            }
            Expr::Unary { op, expr, span } => Expr::Unary {
                op: *op,
                expr: self.arena.alloc(self.clone_expr(expr)),
                span: self.span(*span),
            },
            Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
                op: *op,
                lhs: self.arena.alloc(self.clone_expr(lhs)),
                rhs: self.arena.alloc(self.clone_expr(rhs)),
                span: self.span(*span),
            },
            Expr::TypeCheck {
                value,
                target,
                span,
            } => Expr::TypeCheck {
                value: self.arena.alloc(self.clone_expr(value)),
                target: self.clone_type(*target),
                span: self.span(*span),
            },
            Expr::Index {
                object,
                index,
                span,
            } => Expr::Index {
                object: self.arena.alloc(self.clone_expr(object)),
                index: self.arena.alloc(self.clone_expr(index)),
                span: self.span(*span),
            },
            Expr::Assignment {
                op,
                target,
                value,
                span,
            } => Expr::Assignment {
                op: *op,
                target: self.arena.alloc(self.clone_expr(target)),
                value: self.arena.alloc(self.clone_expr(value)),
                span: self.span(*span),
            },
            Expr::Update {
                op,
                target,
                prefix,
                span,
            } => Expr::Update {
                op: *op,
                target: self.arena.alloc(self.clone_expr(target)),
                prefix: *prefix,
                span: self.span(*span),
            },
            Expr::Template { parts, span } => {
                let mut cloned = BumpVec::new_in(self.arena);
                for part in *parts {
                    cloned.push(match part {
                        TemplatePart::String(value, span) => {
                            TemplatePart::String(value, self.span(*span))
                        }
                        TemplatePart::Expr(expr) => TemplatePart::Expr(self.clone_expr(expr)),
                    });
                }
                Expr::Template {
                    parts: cloned.into_bump_slice(),
                    span: self.span(*span),
                }
            }
        }
    }

    fn clone_exprs(
        &mut self,
        expressions: &[Expr<'arena, 'arena>],
    ) -> &'arena [Expr<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        for expression in expressions {
            cloned.push(self.clone_expr(expression));
        }
        cloned.into_bump_slice()
    }

    fn clone_type(&self, ty: TypeRef<'arena, 'arena>) -> TypeRef<'arena, 'arena> {
        let kind = match ty.kind {
            TypeKind::Named { name, args } => TypeKind::Named {
                name: self.globals.get(name).copied().unwrap_or(name),
                args: self.clone_types(args),
            },
            TypeKind::Array(element) => {
                TypeKind::Array(self.arena.alloc(self.clone_type(*element)))
            }
            TypeKind::Nullable(inner) => {
                TypeKind::Nullable(self.arena.alloc(self.clone_type(*inner)))
            }
            TypeKind::Union(members) => TypeKind::Union(self.clone_types(members)),
            TypeKind::Function {
                params,
                return_type,
            } => {
                let mut cloned = BumpVec::new_in(self.arena);
                cloned.extend(params.iter().map(|param| self.clone_type(*param)));
                TypeKind::Function {
                    params: cloned.into_bump_slice(),
                    return_type: self.arena.alloc(self.clone_type(*return_type)),
                }
            }
            primitive => primitive,
        };
        TypeRef {
            kind,
            span: self.span(ty.span),
        }
    }

    fn clone_types(&self, types: &[TypeRef<'arena, 'arena>]) -> &'arena [TypeRef<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        cloned.extend(types.iter().map(|ty| self.clone_type(*ty)));
        cloned.into_bump_slice()
    }

    fn clone_idents(&self, idents: &[Ident<'arena>]) -> &'arena [Ident<'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        cloned.extend(idents.iter().map(|ident| self.plain_ident(*ident)));
        cloned.into_bump_slice()
    }

    fn global_ident(&self, ident: Ident<'arena>) -> Ident<'arena> {
        Ident {
            name: self.globals.get(ident.name).copied().unwrap_or(ident.name),
            span: self.span(ident.span),
        }
    }

    fn reference_ident(&self, ident: Ident<'arena>) -> Ident<'arena> {
        if self
            .scopes
            .iter()
            .rev()
            .any(|scope| scope.contains(ident.name))
        {
            self.plain_ident(ident)
        } else {
            self.global_ident(ident)
        }
    }

    fn plain_ident(&self, ident: Ident<'arena>) -> Ident<'arena> {
        Ident {
            name: ident.name,
            span: self.span(ident.span),
        }
    }

    fn span(&self, span: Span) -> Span {
        Span::new(span.start + self.offset, span.end + self.offset)
    }

    fn push_scope(&mut self) {
        self.scopes.push(AHashSet::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_local(&mut self, name: &'arena str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_extensionless_relative_imports() {
        assert_eq!(
            resolve_import_path(Path::new("/tmp/project"), "./math").unwrap(),
            PathBuf::from("/tmp/project/math")
        );
    }
}
