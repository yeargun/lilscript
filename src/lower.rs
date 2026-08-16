use crate::stable_hash::{StableHashMap as AHashMap, StableHashSet as AHashSet};

use crate::ast::{
    ArrayBinding, ArrayElement, ArrowBody, AssignmentOp, BinaryOp, ClassMember, ConstructorDecl,
    Expr, ExternClassMember, ExternDecl, ForInitializer, FunctionDecl, Ident, Item, MatchArm,
    Param, Program, RecordElement, Stmt, TemplatePart, UnaryOp, UpdateOp, VarDecl,
};
use crate::ir::{
    AggregateField, AggregateLayout, ArrayOperand, BlockId, ConstValue, ControlFlowBlock,
    ControlFlowFunction, ControlFlowInstruction, ControlFlowModule, ControlFlowOp, ControlShape,
    ExportBinding, FunctionId, FunctionKind, FunctionOrigin, Intrinsic, IrBinaryOp, IrExport,
    IrForeignImport, IrForeignImportSpecifier, IrGlobal, IrLazyModule, IrLocal, IrParameter,
    IrUnaryOp, LocalId, Phi, RecordOperand, TemplateOperand, Terminator, ValueId,
};
use crate::semantic::{
    BuiltinCall, DefaultValue, EscapeState, FunctionType, SemanticModel, SymbolId, Type,
};
use crate::span::Span;
use crate::typed_array::{is_typed_array_range_intrinsic, TypedArrayKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    pub span: Span,
    pub message: String,
}

impl LowerError {
    fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for LowerError {}

pub fn lower_to_control_flow<'ast, 'src>(
    program: &Program<'ast, 'src>,
    semantics: &SemanticModel<'src>,
) -> Result<ControlFlowModule<'src>, LowerError> {
    ModuleLowerer::new(program, semantics)?.lower(program)
}

enum PlannedFunction<'ast, 'src> {
    Entry,
    Function(&'ast FunctionDecl<'ast, 'src>),
    Extern(&'ast ExternDecl<'ast, 'src>),
    Method {
        class: &'src str,
        function: &'ast FunctionDecl<'ast, 'src>,
    },
    Constructor {
        class: &'src str,
        constructor: &'ast ConstructorDecl<'ast, 'src>,
    },
    Arrow {
        params: &'ast [Param<'ast, 'src>],
        body: &'ast ArrowBody<'ast, 'src>,
        captures: Vec<SymbolId>,
        span: Span,
    },
}

struct ModuleLowerer<'model, 'ast, 'src> {
    semantics: &'model SemanticModel<'src>,
    plans: Vec<PlannedFunction<'ast, 'src>>,
    function_symbols: AHashMap<SymbolId, FunctionId>,
    method_functions: AHashMap<(&'src str, &'src str), FunctionId>,
    constructors: AHashMap<&'src str, FunctionId>,
    arrows: AHashMap<Span, FunctionId>,
    arrow_captures: AHashMap<Span, Vec<SymbolId>>,
    mutable_capture_symbols: AHashSet<SymbolId>,
    global_symbols: AHashSet<SymbolId>,
    external_globals: AHashSet<SymbolId>,
    globals: Vec<IrGlobal<'src>>,
    exports: Vec<IrExport<'src>>,
    lazy_modules: Vec<IrLazyModule<'src>>,
}

impl<'model, 'ast, 'src> ModuleLowerer<'model, 'ast, 'src> {
    fn new(
        program: &Program<'ast, 'src>,
        semantics: &'model SemanticModel<'src>,
    ) -> Result<Self, LowerError> {
        let mut lowerer = Self {
            semantics,
            plans: vec![PlannedFunction::Entry],
            function_symbols: AHashMap::default(),
            method_functions: AHashMap::default(),
            constructors: AHashMap::default(),
            arrows: AHashMap::default(),
            arrow_captures: AHashMap::default(),
            mutable_capture_symbols: AHashSet::default(),
            global_symbols: AHashSet::default(),
            external_globals: AHashSet::default(),
            globals: Vec::new(),
            exports: Vec::new(),
            lazy_modules: Vec::new(),
        };

        let mut function_names = AHashMap::default();
        let mut global_names = AHashMap::default();
        let mut type_names = AHashSet::default();

        for item in program.items {
            match item {
                Item::Function(function) => {
                    let id = FunctionId(lowerer.plans.len() as u32);
                    let symbol = lowerer.binding_symbol(function.name)?;
                    lowerer.function_symbols.insert(symbol, id);
                    function_names.insert(function.name.name, id);
                    lowerer.plans.push(PlannedFunction::Function(function));
                }
                Item::Extern(extern_decl) => {
                    let id = FunctionId(lowerer.plans.len() as u32);
                    let symbol = lowerer.binding_symbol(extern_decl.name)?;
                    lowerer.function_symbols.insert(symbol, id);
                    function_names.insert(extern_decl.name.name, id);
                    lowerer.plans.push(PlannedFunction::Extern(extern_decl));
                }
                Item::Class(class) => {
                    type_names.insert(class.name.name);
                    if class.object && !global_names.contains_key(class.name.name) {
                        let symbol = lowerer.binding_symbol(class.name)?;
                        let ty = semantics
                            .binding_type(class.name.span)
                            .cloned()
                            .unwrap_or(Type::Class(class.name.name));
                        lowerer.global_symbols.insert(symbol);
                        global_names.insert(class.name.name, symbol);
                        lowerer.globals.push(IrGlobal {
                            symbol,
                            name: class.name.name,
                            ty,
                            external: false,
                            span: class.span,
                        });
                    }
                    for member in class.members {
                        let id = FunctionId(lowerer.plans.len() as u32);
                        match member {
                            ClassMember::Method(function) => {
                                lowerer
                                    .method_functions
                                    .insert((class.name.name, function.name.name), id);
                                lowerer.plans.push(PlannedFunction::Method {
                                    class: class.name.name,
                                    function,
                                });
                            }
                            ClassMember::Constructor(constructor) => {
                                lowerer.constructors.insert(class.name.name, id);
                                lowerer.plans.push(PlannedFunction::Constructor {
                                    class: class.name.name,
                                    constructor,
                                });
                            }
                            ClassMember::Field(_) => continue,
                        }
                    }
                }
                Item::Enum(decl) => {
                    type_names.insert(decl.name.name);
                }
                Item::ExternClass(class) => {
                    type_names.insert(class.name.name);
                }
                Item::ExternGlobal(global) => {
                    let symbol = lowerer.binding_symbol(global.name)?;
                    let ty = semantics
                        .binding_type(global.name.span)
                        .cloned()
                        .ok_or_else(|| {
                            LowerError::new(global.name.span, "missing extern global type")
                        })?;
                    lowerer.global_symbols.insert(symbol);
                    lowerer.external_globals.insert(symbol);
                    global_names.insert(global.name.name, symbol);
                    lowerer.globals.push(IrGlobal {
                        symbol,
                        name: global.name.name,
                        ty,
                        external: true,
                        span: global.span,
                    });
                }
                Item::Stmt(Stmt::VarDecl(decl)) => {
                    let symbol = lowerer.binding_symbol(decl.name)?;
                    let ty = semantics
                        .binding_type(decl.name.span)
                        .cloned()
                        .ok_or_else(|| LowerError::new(decl.name.span, "missing binding type"))?;
                    lowerer.global_symbols.insert(symbol);
                    global_names.insert(decl.name.name, symbol);
                    lowerer.globals.push(IrGlobal {
                        symbol,
                        name: decl.name.name,
                        ty,
                        external: false,
                        span: decl.span,
                    });
                }
                Item::Stmt(Stmt::ArrayDestructure { bindings, span, .. }) => {
                    for binding in *bindings {
                        let name = match binding {
                            ArrayBinding::Hole(_) => continue,
                            ArrayBinding::Name(name) | ArrayBinding::Rest(name) => *name,
                        };
                        let symbol = lowerer.binding_symbol(name)?;
                        let ty = semantics.binding_type(name.span).cloned().ok_or_else(|| {
                            LowerError::new(name.span, "missing destructured binding type")
                        })?;
                        lowerer.global_symbols.insert(symbol);
                        global_names.insert(name.name, symbol);
                        lowerer.globals.push(IrGlobal {
                            symbol,
                            name: name.name,
                            ty,
                            external: false,
                            span: *span,
                        });
                    }
                }
                Item::Stmt(Stmt::RecordDestructure { bindings, span, .. }) => {
                    for binding in *bindings {
                        let name = binding.name;
                        let symbol = lowerer.binding_symbol(name)?;
                        let ty = semantics.binding_type(name.span).cloned().ok_or_else(|| {
                            LowerError::new(name.span, "missing destructured binding type")
                        })?;
                        lowerer.global_symbols.insert(symbol);
                        global_names.insert(name.name, symbol);
                        lowerer.globals.push(IrGlobal {
                            symbol,
                            name: name.name,
                            ty,
                            external: false,
                            span: *span,
                        });
                    }
                    if let Item::Stmt(Stmt::RecordDestructure {
                        rest: Some(name), ..
                    }) = item
                    {
                        let symbol = lowerer.binding_symbol(*name)?;
                        let ty = semantics.binding_type(name.span).cloned().ok_or_else(|| {
                            LowerError::new(name.span, "missing record rest binding type")
                        })?;
                        lowerer.global_symbols.insert(symbol);
                        global_names.insert(name.name, symbol);
                        lowerer.globals.push(IrGlobal {
                            symbol,
                            name: name.name,
                            ty,
                            external: false,
                            span: *span,
                        });
                    }
                }
                Item::Struct(decl) => {
                    type_names.insert(decl.name.name);
                }
                _ => {}
            }
        }

        let mut exported_names = AHashSet::default();
        for export in program.exports {
            if !exported_names.insert(export.exported.name) {
                return Err(LowerError::new(
                    export.exported.span,
                    format!("duplicate export `{}`", export.exported.name),
                ));
            }
            let binding = if let Some(function) = function_names.get(export.local.name) {
                ExportBinding::Function(*function)
            } else if let Some(global) = global_names.get(export.local.name) {
                ExportBinding::Global(*global)
            } else if type_names.contains(export.local.name) {
                ExportBinding::TypeOnly
            } else {
                return Err(LowerError::new(
                    export.local.span,
                    format!(
                        "cannot export unknown module binding `{}`",
                        export.local.name
                    ),
                ));
            };
            lowerer.exports.push(IrExport {
                name: export.exported.name,
                binding,
                span: export.span,
            });
        }

        let mut lazy_module_indexes = AHashMap::default();
        for import in program.dynamic_imports {
            if lazy_module_indexes.contains_key(&import.module) {
                continue;
            }
            let mut exports = Vec::with_capacity(import.exports.len());
            for export in import.exports {
                if !semantics.dynamic_export_used(import.module, export.exported) {
                    continue;
                }
                let binding = if let Some(function) = function_names.get(export.binding) {
                    ExportBinding::Function(*function)
                } else if let Some(global) = global_names.get(export.binding) {
                    ExportBinding::Global(*global)
                } else {
                    return Err(LowerError::new(
                        import.span,
                        format!(
                            "dynamic module export `{}` is type-only and has no runtime value",
                            export.exported
                        ),
                    ));
                };
                exports.push(IrExport {
                    name: export.exported,
                    binding,
                    span: import.span,
                });
            }
            lazy_module_indexes.insert(import.module, lowerer.lazy_modules.len());
            lowerer.lazy_modules.push(IrLazyModule {
                id: import.module,
                source: import.source,
                exports,
                span: import.span,
            });
        }

        let mut arrows = Vec::new();
        collect_program_arrows(program, &mut arrows);
        for (params, body, span) in arrows {
            let id = FunctionId(lowerer.plans.len() as u32);
            let captures = collect_arrow_captures(
                params,
                body,
                span,
                semantics,
                &lowerer.global_symbols,
                &lowerer.function_symbols,
            );
            lowerer.arrows.insert(span, id);
            lowerer.arrow_captures.insert(span, captures.clone());
            lowerer.mutable_capture_symbols.extend(
                captures
                    .iter()
                    .copied()
                    .filter(|symbol| semantics.symbol_is_assigned(*symbol)),
            );
            lowerer.plans.push(PlannedFunction::Arrow {
                params,
                body,
                captures,
                span,
            });
        }

        Ok(lowerer)
    }

    fn lower(self, program: &Program<'ast, 'src>) -> Result<ControlFlowModule<'src>, LowerError> {
        let mut functions = Vec::with_capacity(self.plans.len());
        for (index, plan) in self.plans.iter().enumerate() {
            let id = FunctionId(index as u32);
            let mut builder = FunctionBuilder::new(
                id,
                self.semantics,
                &self.function_symbols,
                &self.method_functions,
                &self.constructors,
                &self.arrows,
                &self.arrow_captures,
                &self.mutable_capture_symbols,
                &self.global_symbols,
                &self.external_globals,
                plan_span(plan, program.span),
            );

            match plan {
                PlannedFunction::Entry => {
                    builder.kind = FunctionKind::Entry;
                    builder.return_type = Type::Void;
                    builder.lower_object_singletons()?;
                    for item in program.items {
                        if let Item::Stmt(statement) = item {
                            builder.lower_stmt(statement)?;
                        }
                    }
                }
                PlannedFunction::Function(function) => {
                    builder.name = Some(function.name.name);
                    builder.kind = FunctionKind::Function;
                    builder.declared_pure = function.declared_pure;
                    builder.is_async = function.is_async;
                    builder.is_generator = function.is_generator;
                    builder.return_type = resolve_declared_return(self.semantics, function)?;
                    builder.add_params(function.params)?;
                    builder.lower_statements(function.body)?;
                }
                PlannedFunction::Extern(extern_decl) => {
                    builder.name = Some(extern_decl.name.name);
                    builder.kind = FunctionKind::Extern;
                    builder.declared_pure = extern_decl.declared_pure;
                    builder.return_type =
                        resolve_symbol_return(self.semantics, extern_decl.name, "extern function")?;
                    builder.add_params(extern_decl.params)?;
                }
                PlannedFunction::Method { class, function } => {
                    builder.name = Some(function.name.name);
                    builder.kind = FunctionKind::Method { class };
                    builder.declared_pure = function.declared_pure;
                    builder.is_async = function.is_async;
                    builder.is_generator = function.is_generator;
                    builder.return_type = self
                        .semantics
                        .class_info(class)
                        .and_then(|info| info.methods.get(function.name.name))
                        .map(|method| (*method.signature.return_type).clone())
                        .ok_or_else(|| {
                            LowerError::new(function.name.span, "missing method signature")
                        })?;
                    if function.is_async {
                        let Type::Task(value) = builder.return_type else {
                            return Err(LowerError::new(
                                function.name.span,
                                "async method signature does not return Task<T>",
                            ));
                        };
                        builder.return_type = *value;
                    } else if function.is_generator {
                        let Type::Generator(_) = builder.return_type else {
                            return Err(LowerError::new(
                                function.name.span,
                                "generator method signature does not return Generator<T>",
                            ));
                        };
                        builder.return_type = Type::Void;
                    }
                    builder.add_this(function.name.span, class)?;
                    builder.add_params(function.params)?;
                    builder.lower_statements(function.body)?;
                }
                PlannedFunction::Constructor { class, constructor } => {
                    builder.name = Some("init");
                    builder.kind = FunctionKind::Constructor { class };
                    builder.return_type = Type::Void;
                    builder.add_this(constructor.span, class)?;
                    builder.add_params(constructor.params)?;
                    builder.lower_statements(constructor.body)?;
                }
                PlannedFunction::Arrow {
                    params,
                    body,
                    captures,
                    ..
                } => {
                    builder.kind = FunctionKind::Closure;
                    let function_type = self
                        .semantics
                        .expression_type(plan_span(plan, program.span))
                        .cloned()
                        .ok_or_else(|| {
                            LowerError::new(plan_span(plan, program.span), "missing arrow type")
                        })?;
                    let Type::Function(signature) = function_type else {
                        return Err(LowerError::new(
                            plan_span(plan, program.span),
                            "arrow expression does not have a function type",
                        ));
                    };
                    builder.return_type = *signature.return_type;
                    builder.add_captures(captures)?;
                    builder.add_params(params)?;
                    match body {
                        ArrowBody::Expr(expression) => {
                            let value = builder.lower_expr(expression)?;
                            builder.terminate(Terminator::Return(
                                (builder.return_type != Type::Void).then_some(value),
                            ))?;
                        }
                        ArrowBody::Block(statements) => builder.lower_statements(statements)?,
                    }
                }
            }
            functions.push(builder.finish()?);
        }

        let mut structs = self
            .semantics
            .structs()
            .map(|info| AggregateLayout {
                name: info.name,
                base: None,
                fields: info
                    .fields
                    .values()
                    .map(|field| AggregateField {
                        name: field.name,
                        ty: field.ty.clone(),
                        index: field.index,
                    })
                    .collect(),
                object: false,
            })
            .collect::<Vec<_>>();
        structs.sort_unstable_by_key(|layout| layout.name);

        let mut classes = self
            .semantics
            .classes()
            .map(|info| AggregateLayout {
                name: info.name,
                base: info.base.as_ref().and_then(|base| match base {
                    Type::Class(name) | Type::ClassInstance { name, .. } => Some(*name),
                    _ => None,
                }),
                fields: info
                    .fields
                    .values()
                    .map(|field| AggregateField {
                        name: field.name,
                        ty: field.ty.clone(),
                        index: field.index,
                    })
                    .collect(),
                object: info.object,
            })
            .collect::<Vec<_>>();
        classes.sort_unstable_by_key(|layout| layout.name);

        Ok(ControlFlowModule {
            functions,
            globals: self.globals,
            foreign_imports: program
                .foreign_imports
                .iter()
                .map(|import| IrForeignImport {
                    source: import.source,
                    specifiers: import
                        .specifiers
                        .iter()
                        .map(|specifier| IrForeignImportSpecifier {
                            imported: specifier.imported.name,
                            local: specifier.local.name,
                        })
                        .collect(),
                    span: import.span,
                })
                .collect(),
            js_host_aliases: Vec::new(),
            exports: self.exports,
            lazy_modules: self.lazy_modules,
            structs,
            classes,
            entry: FunctionId(0),
        })
    }

    fn binding_symbol(&self, ident: Ident<'src>) -> Result<SymbolId, LowerError> {
        self.semantics.identifier_symbol(ident.span).ok_or_else(|| {
            LowerError::new(ident.span, format!("missing symbol for `{}`", ident.name))
        })
    }
}

struct FunctionBuilder<'model, 'maps, 'src> {
    id: FunctionId,
    name: Option<&'src str>,
    kind: FunctionKind<'src>,
    declared_pure: bool,
    is_async: bool,
    is_generator: bool,
    return_type: Type<'src>,
    semantics: &'model SemanticModel<'src>,
    function_symbols: &'maps AHashMap<SymbolId, FunctionId>,
    method_functions: &'maps AHashMap<(&'src str, &'src str), FunctionId>,
    constructors: &'maps AHashMap<&'src str, FunctionId>,
    arrows: &'maps AHashMap<Span, FunctionId>,
    arrow_captures: &'maps AHashMap<Span, Vec<SymbolId>>,
    mutable_capture_symbols: &'maps AHashSet<SymbolId>,
    global_symbols: &'maps AHashSet<SymbolId>,
    external_globals: &'maps AHashSet<SymbolId>,
    params: Vec<IrParameter<'src>>,
    capture_count: usize,
    mutable_capture_locals: Vec<LocalId>,
    locals: Vec<IrLocal<'src>>,
    local_by_symbol: AHashMap<SymbolId, LocalId>,
    direct_value_by_symbol: AHashMap<SymbolId, ValueId>,
    blocks: Vec<ControlFlowBlock<'src>>,
    shapes: Vec<ControlShape>,
    current: BlockId,
    next_value: u32,
    value_escapes: Vec<EscapeState>,
    loop_targets: Vec<(BlockId, BlockId)>,
    span: Span,
}

impl<'model, 'maps, 'src> FunctionBuilder<'model, 'maps, 'src> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: FunctionId,
        semantics: &'model SemanticModel<'src>,
        function_symbols: &'maps AHashMap<SymbolId, FunctionId>,
        method_functions: &'maps AHashMap<(&'src str, &'src str), FunctionId>,
        constructors: &'maps AHashMap<&'src str, FunctionId>,
        arrows: &'maps AHashMap<Span, FunctionId>,
        arrow_captures: &'maps AHashMap<Span, Vec<SymbolId>>,
        mutable_capture_symbols: &'maps AHashSet<SymbolId>,
        global_symbols: &'maps AHashSet<SymbolId>,
        external_globals: &'maps AHashSet<SymbolId>,
        span: Span,
    ) -> Self {
        Self {
            id,
            name: None,
            kind: FunctionKind::Function,
            declared_pure: false,
            is_async: false,
            is_generator: false,
            return_type: Type::Void,
            semantics,
            function_symbols,
            method_functions,
            constructors,
            arrows,
            arrow_captures,
            mutable_capture_symbols,
            global_symbols,
            external_globals,
            params: Vec::new(),
            capture_count: 0,
            mutable_capture_locals: Vec::new(),
            locals: Vec::new(),
            local_by_symbol: AHashMap::default(),
            direct_value_by_symbol: AHashMap::default(),
            blocks: vec![ControlFlowBlock {
                id: BlockId(0),
                phis: Vec::new(),
                instructions: Vec::new(),
                terminator: None,
                span,
            }],
            shapes: Vec::new(),
            current: BlockId(0),
            next_value: 0,
            value_escapes: Vec::new(),
            loop_targets: Vec::new(),
            span,
        }
    }

    fn add_this(&mut self, span: Span, _class: &'src str) -> Result<(), LowerError> {
        let symbol = self
            .semantics
            .identifier_symbol(span)
            .ok_or_else(|| LowerError::new(span, "missing `this` symbol"))?;
        let ty = self
            .semantics
            .symbols()
            .get(symbol.0 as usize)
            .map(|symbol| symbol.ty.clone())
            .ok_or_else(|| LowerError::new(span, "missing `this` type"))?;
        self.add_param(symbol, "this", ty, None, span)
    }

    fn lower_object_singletons(&mut self) -> Result<(), LowerError> {
        let objects = self
            .semantics
            .classes()
            .filter(|info| info.object)
            .cloned()
            .collect::<Vec<_>>();
        for info in objects {
            let symbol = self
                .semantics
                .symbols()
                .iter()
                .find(|symbol| symbol.name == info.name)
                .map(|symbol| symbol.id)
                .ok_or_else(|| {
                    LowerError::new(info.span, format!("missing object `{}`", info.name))
                })?;
            let mut method_values = Vec::with_capacity(info.fields.len());
            for field in info.fields.values() {
                let function = self
                    .method_functions
                    .get(&(info.name, field.name))
                    .copied()
                    .ok_or_else(|| {
                        LowerError::new(
                            info.span,
                            format!("missing object method `{}`", field.name),
                        )
                    })?;
                let value = self.emit_value(
                    ControlFlowOp::Closure {
                        function,
                        captures: Vec::new(),
                    },
                    field.ty.clone(),
                    info.span,
                )?;
                method_values.push((field.name, field.index, value));
            }
            let object = self.emit_value(
                ControlFlowOp::NewClass {
                    class: info.name,
                    constructor: None,
                    args: Vec::new(),
                },
                Type::Class(info.name),
                info.span,
            )?;
            for (field, index, value) in method_values {
                self.emit_effect(
                    ControlFlowOp::FieldSet {
                        object,
                        owner: info.name,
                        field,
                        index,
                        value,
                    },
                    info.span,
                )?;
            }
            self.emit_effect(
                ControlFlowOp::StoreGlobal {
                    global: symbol,
                    value: object,
                },
                info.span,
            )?;
        }
        Ok(())
    }

    fn add_params<'ast>(&mut self, params: &[Param<'ast, 'src>]) -> Result<(), LowerError> {
        for param in params {
            let symbol = self.symbol(param.name)?;
            let ty = self
                .semantics
                .binding_type(param.name.span)
                .cloned()
                .ok_or_else(|| LowerError::new(param.span, "missing parameter type"))?;
            self.add_param(
                symbol,
                param.name.name,
                ty,
                self.resolve_parameter_default(param.default.as_ref())?,
                param.span,
            )?;
        }
        Ok(())
    }

    fn resolve_parameter_default<'ast>(
        &self,
        expression: Option<&Expr<'ast, 'src>>,
    ) -> Result<Option<crate::ir::IrParamDefault>, LowerError> {
        let Some(expression) = expression else {
            return Ok(None);
        };
        if self.semantics.builtin_call(expression.span()) == Some(BuiltinCall::JsUndefined) {
            return Ok(Some(crate::ir::IrParamDefault::Undefined));
        }
        if let Some(value) = scalar_parameter_default(expression) {
            return Ok(Some(crate::ir::IrParamDefault::Const(value)));
        }
        let Expr::Ident(identifier) = expression else {
            return Ok(Some(crate::ir::IrParamDefault::CallerMaterialized));
        };
        let symbol = self
            .semantics
            .identifier_symbol(identifier.span)
            .ok_or_else(|| LowerError::new(identifier.span, "missing default identifier symbol"))?;
        if let Some(parameter) = self
            .params
            .iter()
            .find(|parameter| parameter.symbol == symbol)
        {
            return Ok(Some(crate::ir::IrParamDefault::Value(parameter.value)));
        }
        Ok(Some(crate::ir::IrParamDefault::CallerMaterialized))
    }

    fn add_captures(&mut self, captures: &[SymbolId]) -> Result<(), LowerError> {
        for symbol in captures {
            let capture = self
                .semantics
                .symbols()
                .get(symbol.0 as usize)
                .ok_or_else(|| LowerError::new(self.span, "missing captured symbol"))?;
            self.add_param_internal(
                capture.id,
                capture.name,
                capture.ty.clone(),
                None,
                capture.span,
                !self.mutable_capture_symbols.contains(symbol),
            )?;
            self.capture_count += 1;
        }
        Ok(())
    }

    fn add_param(
        &mut self,
        symbol: SymbolId,
        name: &'src str,
        ty: Type<'src>,
        default: Option<crate::ir::IrParamDefault>,
        span: Span,
    ) -> Result<(), LowerError> {
        self.add_param_internal(symbol, name, ty, default, span, true)
    }

    fn add_param_internal(
        &mut self,
        symbol: SymbolId,
        name: &'src str,
        ty: Type<'src>,
        default: Option<crate::ir::IrParamDefault>,
        span: Span,
        initialize_local: bool,
    ) -> Result<(), LowerError> {
        let local = self.add_local(symbol, name, ty.clone(), span);
        let value = self.new_value(EscapeState::LocalOnly);
        self.params.push(IrParameter {
            symbol,
            local,
            value,
            name,
            ty,
            default,
            span,
        });
        if initialize_local {
            self.emit_effect(ControlFlowOp::StoreLocal { local, value }, span)?;
        }
        Ok(())
    }

    fn lower_statements<'ast>(
        &mut self,
        statements: &[Stmt<'ast, 'src>],
    ) -> Result<(), LowerError> {
        for statement in statements {
            self.lower_stmt(statement)?;
        }
        Ok(())
    }

    fn lower_stmt<'ast>(&mut self, statement: &Stmt<'ast, 'src>) -> Result<(), LowerError> {
        if !self.block_open(self.current) {
            self.current = self.add_block(statement.span());
        }
        match statement {
            Stmt::VarDecl(decl) => self.lower_var_decl(decl),
            Stmt::ArrayDestructure {
                bindings,
                value,
                span,
            } => self.lower_array_destructure(bindings, value, *span),
            Stmt::RecordDestructure {
                bindings,
                rest,
                value,
                span,
            } => self.lower_record_destructure(bindings, *rest, value, *span),
            Stmt::Expr(expression) => {
                self.lower_expr(expression)?;
                Ok(())
            }
            Stmt::Return {
                value: Some(Expr::Match { value, arms, span }),
                ..
            } => self.lower_return_match(value, arms, *span),
            Stmt::Return { value, .. } => {
                let value = value
                    .as_ref()
                    .map(|value| self.lower_expr(value))
                    .transpose()?;
                self.terminate(Terminator::Return(value))
            }
            Stmt::Throw { value, .. } => {
                let value = self.lower_expr(value)?;
                self.terminate(Terminator::Throw(value))
            }
            Stmt::SuperCall { args, span } => self.lower_super_call(args, *span),
            Stmt::Yield {
                value,
                delegate,
                span,
            } => {
                let value = self.lower_expr(value)?;
                self.emit_effect(
                    ControlFlowOp::Intrinsic {
                        intrinsic: if *delegate {
                            Intrinsic::GeneratorYieldDelegated
                        } else {
                            Intrinsic::GeneratorYield
                        },
                        receiver: Some(value),
                        args: Vec::new(),
                    },
                    *span,
                )
            }
            Stmt::Try {
                body,
                catch,
                finally,
                span,
            } => self.lower_try(body, catch.as_ref(), *finally, *span),
            Stmt::Block { body, .. } => self.lower_statements(body),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => self.lower_if(condition, then_branch, *else_branch, *span),
            Stmt::While {
                condition,
                body,
                span,
            } => self.lower_while(condition, body, *span),
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                span,
            } => self.lower_for(
                initializer.as_ref(),
                condition.as_ref(),
                update.as_ref(),
                body,
                *span,
            ),
            Stmt::ForIn {
                key,
                object,
                body,
                span,
                ..
            } => self.lower_for_in(*key, object, body, *span),
            Stmt::ForOf {
                element,
                iterable,
                body,
                span,
                ..
            } => self.lower_for_of(*element, iterable, body, *span),
            Stmt::Break(span) => {
                let (_, break_target) = self.loop_targets.last().copied().ok_or_else(|| {
                    LowerError::new(*span, "`break` reached lowering outside a loop")
                })?;
                self.terminate(Terminator::Jump(break_target))
            }
            Stmt::Continue(span) => {
                let (continue_target, _) = self.loop_targets.last().copied().ok_or_else(|| {
                    LowerError::new(*span, "`continue` reached lowering outside a loop")
                })?;
                self.terminate(Terminator::Jump(continue_target))
            }
        }
    }

    fn lower_super_call<'ast>(
        &mut self,
        args: &[Expr<'ast, 'src>],
        span: Span,
    ) -> Result<(), LowerError> {
        let FunctionKind::Constructor { class } = self.kind else {
            return Err(LowerError::new(
                span,
                "`super` reached lowering outside a constructor",
            ));
        };
        let base = self
            .semantics
            .base_class_name(class)
            .ok_or_else(|| LowerError::new(span, "constructor has no base class"))?;
        let Some(function) = self.constructors.get(base).copied() else {
            if args.is_empty() {
                return Ok(());
            }
            return Err(LowerError::new(
                span,
                "implicit base constructor cannot receive arguments",
            ));
        };
        let signature = self
            .semantics
            .base_constructor(class)
            .map(|(_, signature)| signature)
            .ok_or_else(|| LowerError::new(span, "missing base constructor signature"))?;
        let receiver = self
            .params
            .first()
            .filter(|param| param.name == "this")
            .map(|param| param.value)
            .ok_or_else(|| LowerError::new(span, "derived constructor is missing `this`"))?;
        let args = self.lower_args_with_defaults(args, &signature, span)?;
        self.emit_effect(
            ControlFlowOp::CallMethod {
                receiver,
                class: base,
                method: "init",
                function,
                args,
            },
            span,
        )
    }

    fn lower_var_decl<'ast>(&mut self, decl: &VarDecl<'ast, 'src>) -> Result<(), LowerError> {
        let symbol = self.symbol(decl.name)?;
        let ty = self
            .semantics
            .binding_type(decl.name.span)
            .cloned()
            .ok_or_else(|| LowerError::new(decl.span, "missing variable type"))?;
        let slot_first = !self.global_symbols.contains(&symbol)
            && self.mutable_capture_symbols.contains(&symbol);
        let local = if slot_first {
            Some(self.add_local(symbol, decl.name.name, ty.clone(), decl.span))
        } else {
            None
        };
        let value = decl
            .initializer
            .as_ref()
            .map(|initializer| self.lower_expr(initializer))
            .transpose()?
            .ok_or_else(|| {
                LowerError::new(
                    decl.span,
                    "variables without initializers are not lowerable",
                )
            })?;
        if self.global_symbols.contains(&symbol) {
            self.emit_effect(
                ControlFlowOp::StoreGlobal {
                    global: symbol,
                    value,
                },
                decl.span,
            )
        } else if let Some(local) = local {
            self.emit_effect(ControlFlowOp::StoreLocal { local, value }, decl.span)
        } else {
            let local = self.add_local(symbol, decl.name.name, ty, decl.span);
            self.emit_effect(ControlFlowOp::StoreLocal { local, value }, decl.span)
        }
    }

    fn lower_array_destructure<'ast>(
        &mut self,
        bindings: &[ArrayBinding<'src>],
        expression: &Expr<'ast, 'src>,
        _span: Span,
    ) -> Result<(), LowerError> {
        let array = self.lower_expr(expression)?;
        for (index, binding) in bindings.iter().enumerate() {
            let (name, value) = match binding {
                ArrayBinding::Hole(_) => continue,
                ArrayBinding::Name(name) => {
                    let ty = self
                        .semantics
                        .binding_type(name.span)
                        .cloned()
                        .ok_or_else(|| LowerError::new(name.span, "missing array binding type"))?;
                    (
                        *name,
                        self.emit_value(
                            ControlFlowOp::ArrayGetOptional {
                                object: array,
                                index,
                            },
                            ty,
                            name.span,
                        )?,
                    )
                }
                ArrayBinding::Rest(name) => {
                    let start = self.emit_value(
                        ControlFlowOp::Const(ConstValue::Int(index as i64)),
                        Type::Int,
                        name.span,
                    )?;
                    let ty = self
                        .semantics
                        .binding_type(name.span)
                        .cloned()
                        .ok_or_else(|| LowerError::new(name.span, "missing rest binding type"))?;
                    (
                        *name,
                        self.emit_value(
                            ControlFlowOp::Intrinsic {
                                intrinsic: Intrinsic::ArraySlice,
                                receiver: Some(array),
                                args: vec![start],
                            },
                            ty,
                            name.span,
                        )?,
                    )
                }
            };
            let symbol = self.symbol(name)?;
            let ty = self
                .semantics
                .binding_type(name.span)
                .cloned()
                .ok_or_else(|| LowerError::new(name.span, "missing destructured binding type"))?;
            self.store_destructured_binding(symbol, name, ty, value)?;
        }
        Ok(())
    }

    fn lower_record_destructure<'ast>(
        &mut self,
        bindings: &[crate::ast::RecordBinding<'src>],
        rest: Option<Ident<'src>>,
        expression: &Expr<'ast, 'src>,
        _span: Span,
    ) -> Result<(), LowerError> {
        let record = self.lower_expr(expression)?;
        for binding in bindings {
            let ty = self
                .semantics
                .binding_type(binding.name.span)
                .cloned()
                .ok_or_else(|| LowerError::new(binding.name.span, "missing record binding type"))?;
            let value = self.emit_value(
                ControlFlowOp::RecordFieldGet {
                    object: record,
                    property: binding.key.name,
                },
                ty.clone(),
                binding.span,
            )?;
            let symbol = self.symbol(binding.name)?;
            self.store_destructured_binding(symbol, binding.name, ty, value)?;
        }
        if let Some(name) = rest {
            let ty = self
                .semantics
                .binding_type(name.span)
                .cloned()
                .ok_or_else(|| LowerError::new(name.span, "missing record rest binding type"))?;
            let value = self.emit_value(
                ControlFlowOp::RecordRest {
                    object: record,
                    excluded: bindings.iter().map(|binding| binding.key.name).collect(),
                },
                ty.clone(),
                name.span,
            )?;
            let symbol = self.symbol(name)?;
            self.store_destructured_binding(symbol, name, ty, value)?;
        }
        Ok(())
    }

    fn store_destructured_binding(
        &mut self,
        symbol: SymbolId,
        name: Ident<'src>,
        ty: Type<'src>,
        value: ValueId,
    ) -> Result<(), LowerError> {
        if self.global_symbols.contains(&symbol) {
            self.emit_effect(
                ControlFlowOp::StoreGlobal {
                    global: symbol,
                    value,
                },
                name.span,
            )
        } else {
            let local = self.add_local(symbol, name.name, ty, name.span);
            self.emit_effect(ControlFlowOp::StoreLocal { local, value }, name.span)
        }
    }

    fn lower_if<'ast>(
        &mut self,
        condition: &Expr<'ast, 'src>,
        then_branch: &Stmt<'ast, 'src>,
        else_branch: Option<&Stmt<'ast, 'src>>,
        span: Span,
    ) -> Result<(), LowerError> {
        let condition = self.lower_expr(condition)?;
        let then_block = self.add_block(then_branch.span());
        let else_block = self.add_block(else_branch.map_or(span, Stmt::span));
        let merge_block = self.add_block(span);
        self.shapes.push(ControlShape::If {
            header: self.current,
            then_block,
            else_block,
            merge_block,
        });
        self.terminate(Terminator::Branch {
            condition,
            then_block,
            else_block,
        })?;

        self.current = then_block;
        self.lower_stmt(then_branch)?;
        self.jump_if_open(merge_block)?;

        self.current = else_block;
        if let Some(else_branch) = else_branch {
            self.lower_stmt(else_branch)?;
        }
        self.jump_if_open(merge_block)?;
        self.current = merge_block;
        Ok(())
    }

    fn lower_try<'ast>(
        &mut self,
        body: &[Stmt<'ast, 'src>],
        catch: Option<&crate::ast::CatchClause<'ast, 'src>>,
        finally: Option<&[Stmt<'ast, 'src>]>,
        span: Span,
    ) -> Result<(), LowerError> {
        let header = self.current;
        let body_block = self.add_block(span);
        let catch_block = catch.map(|clause| self.add_block(clause.span));
        let finally_block = finally.map(|_| self.add_block(span));
        let merge_block = self.add_block(span);
        self.terminate(Terminator::Try {
            body: body_block,
            catch_block,
        })?;

        let continuation = finally_block.unwrap_or(merge_block);
        self.current = body_block;
        self.lower_statements(body)?;
        self.jump_if_open(continuation)?;

        let mut catch_value = None;
        if let (Some(clause), Some(block)) = (catch, catch_block) {
            self.current = block;
            if let Some(binding) = clause.binding {
                let value = self.emit_value(
                    ControlFlowOp::CaughtException,
                    Type::TypeParameter("$js"),
                    binding.span,
                )?;
                let symbol = self.symbol(binding.name)?;
                self.direct_value_by_symbol.insert(symbol, value);
                catch_value = Some(value);
                self.lower_statements(clause.body)?;
                self.direct_value_by_symbol.remove(&symbol);
            } else {
                self.lower_statements(clause.body)?;
            }
            self.jump_if_open(continuation)?;
        }

        if let (Some(finally), Some(block)) = (finally, finally_block) {
            self.current = block;
            self.lower_statements(finally)?;
            self.jump_if_open(merge_block)?;
        }

        self.shapes.push(ControlShape::Try {
            header,
            body: body_block,
            catch_block,
            finally_block,
            merge_block,
            catch_value,
        });
        self.current = merge_block;
        Ok(())
    }

    fn lower_while<'ast>(
        &mut self,
        condition: &Expr<'ast, 'src>,
        body: &Stmt<'ast, 'src>,
        span: Span,
    ) -> Result<(), LowerError> {
        let condition_block = self.add_block(condition.span());
        let body_block = self.add_block(body.span());
        let exit_block = self.add_block(span);
        self.shapes.push(ControlShape::Loop {
            header: condition_block,
            body: body_block,
            update: None,
            exit: exit_block,
        });
        self.terminate(Terminator::Jump(condition_block))?;
        self.current = condition_block;
        let condition = self.lower_expr(condition)?;
        self.terminate(Terminator::Branch {
            condition,
            then_block: body_block,
            else_block: exit_block,
        })?;
        self.loop_targets.push((condition_block, exit_block));
        self.current = body_block;
        self.lower_stmt(body)?;
        self.jump_if_open(condition_block)?;
        self.loop_targets.pop();
        self.current = exit_block;
        Ok(())
    }

    fn lower_for<'ast>(
        &mut self,
        initializer: Option<&ForInitializer<'ast, 'src>>,
        condition: Option<&Expr<'ast, 'src>>,
        update: Option<&Expr<'ast, 'src>>,
        body: &Stmt<'ast, 'src>,
        span: Span,
    ) -> Result<(), LowerError> {
        if let Some(initializer) = initializer {
            match initializer {
                ForInitializer::VarDecl(decl) => self.lower_var_decl(decl)?,
                ForInitializer::Expr(expression) => {
                    self.lower_expr(expression)?;
                }
            }
        }
        let condition_block = self.add_block(condition.map_or(span, Expr::span));
        let body_block = self.add_block(body.span());
        let update_block = self.add_block(update.map_or(span, Expr::span));
        let exit_block = self.add_block(span);
        self.shapes.push(ControlShape::Loop {
            header: condition_block,
            body: body_block,
            update: Some(update_block),
            exit: exit_block,
        });
        self.terminate(Terminator::Jump(condition_block))?;

        self.current = condition_block;
        if let Some(condition) = condition {
            let condition = self.lower_expr(condition)?;
            self.terminate(Terminator::Branch {
                condition,
                then_block: body_block,
                else_block: exit_block,
            })?;
        } else {
            self.terminate(Terminator::Jump(body_block))?;
        }

        self.loop_targets.push((update_block, exit_block));
        self.current = body_block;
        self.lower_stmt(body)?;
        self.jump_if_open(update_block)?;
        self.loop_targets.pop();

        self.current = update_block;
        if let Some(update) = update {
            self.lower_expr(update)?;
        }
        self.jump_if_open(condition_block)?;
        self.current = exit_block;
        Ok(())
    }

    fn lower_for_in<'ast>(
        &mut self,
        key: Ident<'src>,
        object: &Expr<'ast, 'src>,
        body: &Stmt<'ast, 'src>,
        span: Span,
    ) -> Result<(), LowerError> {
        let object = self.lower_expr(object)?;
        let header = self.add_block(span);
        let body_block = self.add_block(body.span());
        let exit = self.add_block(span);
        self.terminate(Terminator::Jump(header))?;

        self.current = header;
        let key_value = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::JsForInKey,
                receiver: Some(object),
                args: Vec::new(),
            },
            Type::String,
            key.span,
        )?;
        let condition = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::JsForInHasNext,
                receiver: Some(object),
                args: vec![key_value],
            },
            Type::Bool,
            span,
        )?;
        self.shapes.push(ControlShape::ForIn {
            header,
            body: body_block,
            exit,
            object,
            key: key_value,
        });
        self.terminate(Terminator::Branch {
            condition,
            then_block: body_block,
            else_block: exit,
        })?;

        let symbol = self.symbol(key)?;
        self.direct_value_by_symbol.insert(symbol, key_value);
        self.loop_targets.push((header, exit));
        self.current = body_block;
        self.lower_stmt(body)?;
        self.jump_if_open(header)?;
        self.loop_targets.pop();
        self.direct_value_by_symbol.remove(&symbol);
        self.current = exit;
        Ok(())
    }

    fn lower_for_of<'ast>(
        &mut self,
        element: Ident<'src>,
        iterable: &Expr<'ast, 'src>,
        body: &Stmt<'ast, 'src>,
        span: Span,
    ) -> Result<(), LowerError> {
        let iterable_type = self.expression_type(iterable)?;
        if let Type::Generator(element_type) = &iterable_type {
            return self.lower_generator_for_of(
                element,
                iterable,
                body,
                span,
                element_type.as_ref().clone(),
            );
        }
        let element_type = match &iterable_type {
            Type::Array(element) => element.as_ref().clone(),
            ty if TypedArrayKind::from_type(ty).is_some() => {
                if TypedArrayKind::from_type(ty).is_some_and(TypedArrayKind::element_is_float) {
                    Type::Float
                } else {
                    Type::Int
                }
            }
            _ => {
                return Err(LowerError::new(
                    iterable.span(),
                    "for-of iterable lost its indexed collection type",
                ));
            }
        };
        let iterable = self.lower_expr(iterable)?;
        let symbol = self.symbol(element)?;

        // The synthetic counter is an IR implementation detail. Reusing the
        // element symbol is safe because only LocalId identifies storage; it
        // deliberately does not replace the source binding in local_by_symbol.
        let index_local = LocalId(self.locals.len() as u32);
        self.locals.push(IrLocal {
            id: index_local,
            symbol,
            name: "$for_of_index",
            ty: Type::Int,
            span,
        });
        let declared_element_type = self
            .semantics
            .binding_type(element.span)
            .cloned()
            .ok_or_else(|| LowerError::new(element.span, "missing for-of element type"))?;
        let element_local =
            self.add_local(symbol, element.name, declared_element_type, element.span);

        let zero = self.emit_value(ControlFlowOp::Const(ConstValue::Int(0)), Type::Int, span)?;
        self.emit_effect(
            ControlFlowOp::StoreLocal {
                local: index_local,
                value: zero,
            },
            span,
        )?;

        let header = self.add_block(span);
        let body_block = self.add_block(body.span());
        let update = self.add_block(span);
        let exit = self.add_block(span);
        self.shapes.push(ControlShape::Loop {
            header,
            body: body_block,
            update: Some(update),
            exit,
        });
        self.terminate(Terminator::Jump(header))?;

        self.current = header;
        let index = self.emit_value(ControlFlowOp::LoadLocal(index_local), Type::Int, span)?;
        let length_intrinsic = match &iterable_type {
            Type::Array(_) => Intrinsic::ArrayLength,
            ty => TypedArrayKind::from_type(ty)
                .expect("semantic analysis validated typed array")
                .length_intrinsic(),
        };
        let length = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: length_intrinsic,
                receiver: Some(iterable),
                args: Vec::new(),
            },
            Type::Int,
            span,
        )?;
        let condition = self.emit_value(
            ControlFlowOp::Binary {
                op: IrBinaryOp::Less,
                lhs: index,
                rhs: length,
            },
            Type::Bool,
            span,
        )?;
        self.terminate(Terminator::Branch {
            condition,
            then_block: body_block,
            else_block: exit,
        })?;

        self.current = body_block;
        let value = self.emit_value(
            ControlFlowOp::IndexGet {
                object: iterable,
                index,
            },
            element_type,
            element.span,
        )?;
        self.emit_effect(
            ControlFlowOp::StoreLocal {
                local: element_local,
                value,
            },
            element.span,
        )?;
        self.loop_targets.push((update, exit));
        self.lower_stmt(body)?;
        self.jump_if_open(update)?;
        self.loop_targets.pop();

        self.current = update;
        let previous = self.emit_value(ControlFlowOp::LoadLocal(index_local), Type::Int, span)?;
        let one = self.emit_value(ControlFlowOp::Const(ConstValue::Int(1)), Type::Int, span)?;
        let next = self.emit_value(
            ControlFlowOp::Binary {
                op: IrBinaryOp::Add,
                lhs: previous,
                rhs: one,
            },
            Type::Int,
            span,
        )?;
        self.emit_effect(
            ControlFlowOp::StoreLocal {
                local: index_local,
                value: next,
            },
            span,
        )?;
        self.terminate(Terminator::Jump(header))?;
        self.current = exit;
        Ok(())
    }

    fn lower_generator_for_of<'ast>(
        &mut self,
        element: Ident<'src>,
        iterable: &Expr<'ast, 'src>,
        body: &Stmt<'ast, 'src>,
        span: Span,
        element_type: Type<'src>,
    ) -> Result<(), LowerError> {
        let iterable = self.lower_expr(iterable)?;
        let header = self.add_block(span);
        let body_block = self.add_block(body.span());
        let exit = self.add_block(span);
        self.terminate(Terminator::Jump(header))?;

        self.current = header;
        let element_value = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::JsForOfValue,
                receiver: Some(iterable),
                args: Vec::new(),
            },
            element_type,
            element.span,
        )?;
        let condition = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::JsForOfHasNext,
                receiver: Some(iterable),
                args: vec![element_value],
            },
            Type::Bool,
            span,
        )?;
        self.shapes.push(ControlShape::ForOf {
            header,
            body: body_block,
            exit,
            iterable,
            element: element_value,
        });
        self.terminate(Terminator::Branch {
            condition,
            then_block: body_block,
            else_block: exit,
        })?;

        let symbol = self.symbol(element)?;
        self.direct_value_by_symbol.insert(symbol, element_value);
        self.loop_targets.push((header, exit));
        self.current = body_block;
        self.lower_stmt(body)?;
        self.jump_if_open(header)?;
        self.loop_targets.pop();
        self.direct_value_by_symbol.remove(&symbol);
        self.current = exit;
        Ok(())
    }

    fn lower_expr<'ast>(&mut self, expression: &Expr<'ast, 'src>) -> Result<ValueId, LowerError> {
        let ty = self.expression_type(expression)?;
        match expression {
            Expr::Int(value, span) => {
                self.emit_value(ControlFlowOp::Const(ConstValue::Int(*value)), ty, *span)
            }
            Expr::Float(value, span) => {
                self.emit_value(ControlFlowOp::Const(ConstValue::Float(*value)), ty, *span)
            }
            Expr::String(value, span) => self.emit_value(
                ControlFlowOp::Const(ConstValue::String((*value).to_string())),
                ty,
                *span,
            ),
            Expr::Bool(value, span) => {
                self.emit_value(ControlFlowOp::Const(ConstValue::Bool(*value)), ty, *span)
            }
            Expr::Null(span) => self.emit_value(ControlFlowOp::Const(ConstValue::Null), ty, *span),
            Expr::DynamicImport { span, .. } => {
                let module = self
                    .semantics
                    .dynamic_import_module(*span)
                    .ok_or_else(|| LowerError::new(*span, "missing dynamic module metadata"))?;
                self.emit_value(ControlFlowOp::DynamicImport { module }, ty, *span)
            }
            Expr::Ident(ident) => self.lower_ident(*ident, ty),
            Expr::ArrayLiteral { elements, span } => {
                if elements
                    .iter()
                    .all(|element| matches!(element, ArrayElement::Value(_)))
                {
                    let values = elements
                        .iter()
                        .map(|element| self.lower_expr(element.value()))
                        .collect::<Result<Vec<_>, _>>()?;
                    self.emit_value(ControlFlowOp::Array(values), ty, *span)
                } else {
                    let operands = elements
                        .iter()
                        .map(|element| {
                            let value = self.lower_expr(element.value())?;
                            Ok(match element {
                                ArrayElement::Value(_) => ArrayOperand::Value(value),
                                ArrayElement::Spread { .. } => ArrayOperand::Spread(value),
                            })
                        })
                        .collect::<Result<Vec<_>, LowerError>>()?;
                    self.emit_value(ControlFlowOp::ArraySpread(operands), ty, *span)
                }
            }
            Expr::RecordLiteral { entries, span } => {
                if entries
                    .iter()
                    .all(|entry| matches!(entry, RecordElement::Entry(_)))
                {
                    let values = entries
                        .iter()
                        .map(|entry| match entry {
                            RecordElement::Entry(entry) => {
                                Ok((entry.key.name, self.lower_expr(&entry.value)?))
                            }
                            RecordElement::Spread { .. } => unreachable!(),
                        })
                        .collect::<Result<Vec<_>, LowerError>>()?;
                    self.emit_value(ControlFlowOp::Record(values), ty, *span)
                } else {
                    let operands = entries
                        .iter()
                        .map(|entry| match entry {
                            RecordElement::Entry(entry) => Ok(RecordOperand::Entry(
                                entry.key.name,
                                self.lower_expr(&entry.value)?,
                            )),
                            RecordElement::Spread { value, .. } => {
                                Ok(RecordOperand::Spread(self.lower_expr(value)?))
                            }
                        })
                        .collect::<Result<Vec<_>, LowerError>>()?;
                    self.emit_value(ControlFlowOp::RecordSpread(operands), ty, *span)
                }
            }
            Expr::StructLiteral { name, values, span } => {
                let fields = values
                    .iter()
                    .map(|value| self.lower_expr(value))
                    .collect::<Result<Vec<_>, _>>()?;
                self.emit_value(
                    ControlFlowOp::Struct {
                        name: name.name,
                        fields,
                    },
                    ty,
                    *span,
                )
            }
            Expr::New {
                class, args, span, ..
            } => {
                let builtin = match &ty {
                    Type::Map(_, _) => Some(Intrinsic::MapNew),
                    Type::Set(_) => Some(Intrinsic::SetNew),
                    Type::ArrayBuffer => Some(Intrinsic::ArrayBufferNew),
                    Type::SharedArrayBuffer => Some(Intrinsic::SharedArrayBufferNew),
                    Type::Symbol => Some(Intrinsic::SymbolNew),
                    Type::Regex => Some(Intrinsic::RegexNew),
                    ty if let Some(kind) = TypedArrayKind::from_type(ty) => {
                        Some(kind.new_intrinsic())
                    }
                    _ => None,
                };
                if let Some(intrinsic) = builtin {
                    let args = self.lower_args(args)?;
                    return self.emit_value(
                        ControlFlowOp::Intrinsic {
                            intrinsic,
                            receiver: None,
                            args,
                        },
                        ty,
                        *span,
                    );
                }
                let args = if let Some(signature) = self
                    .semantics
                    .class_info(class.name)
                    .and_then(|info| info.constructor.as_ref())
                    .cloned()
                {
                    self.lower_args_with_defaults(args, &signature, *span)?
                } else {
                    self.lower_args(args)?
                };
                self.emit_value(
                    ControlFlowOp::NewClass {
                        class: class.name,
                        constructor: self.constructors.get(class.name).copied(),
                        args,
                    },
                    ty,
                    *span,
                )
            }
            Expr::Await { task, span } => {
                let task = self.lower_expr(task)?;
                self.emit_value(ControlFlowOp::Await { task }, ty, *span)
            }
            Expr::Member {
                object,
                property,
                span,
            } => {
                if let Some(value) = self.semantics.enum_variant_value(*span) {
                    self.emit_value(ControlFlowOp::Const(ConstValue::Int(value)), ty, *span)
                } else {
                    self.lower_member(object, *property, ty, *span)
                }
            }
            Expr::OptionalMember {
                object,
                property,
                span,
            } => self.lower_optional_member(object, *property, ty, *span),
            Expr::Call { callee, args, span } => self.lower_call(callee, args, ty, *span),
            Expr::ArrowFunction { span, .. } => {
                let function = self.arrows.get(span).copied().ok_or_else(|| {
                    LowerError::new(*span, "arrow function was not assigned an IR function")
                })?;
                let captures = self
                    .arrow_captures
                    .get(span)
                    .ok_or_else(|| LowerError::new(*span, "arrow captures were not analyzed"))?
                    .iter()
                    .map(|symbol| self.lower_capture_value(*symbol, *span))
                    .collect::<Result<Vec<_>, _>>()?;
                self.emit_value(ControlFlowOp::Closure { function, captures }, ty, *span)
            }
            Expr::Unary { op, expr, span } => {
                let value = self.lower_expr(expr)?;
                self.emit_value(
                    ControlFlowOp::Unary {
                        op: match op {
                            UnaryOp::Neg => IrUnaryOp::Neg,
                            UnaryOp::Not => IrUnaryOp::Not,
                        },
                        value,
                    },
                    ty,
                    *span,
                )
            }
            Expr::Binary {
                op: BinaryOp::Nullish,
                lhs,
                rhs,
                span,
            } => {
                let present = self.semantics.optional_present_type(lhs.span());
                if present.is_some_and(|present| !matches!(present, Type::Nullable(_))) {
                    match lhs {
                        Expr::OptionalMember {
                            object,
                            property,
                            span: optional_span,
                        } => self.lower_optional_member_with_fallback(
                            object,
                            *property,
                            *optional_span,
                            rhs,
                            ty,
                            *span,
                        ),
                        Expr::OptionalIndex {
                            object,
                            index,
                            span: optional_span,
                        } => self.lower_optional_index_with_fallback(
                            object,
                            index,
                            *optional_span,
                            rhs,
                            ty,
                            *span,
                        ),
                        _ => self.lower_nullish(lhs, rhs, ty, *span),
                    }
                } else {
                    self.lower_nullish(lhs, rhs, ty, *span)
                }
            }
            Expr::Binary { op, lhs, rhs, span } if matches!(op, BinaryOp::And | BinaryOp::Or) => {
                self.lower_short_circuit(*op, lhs, rhs, ty, *span)
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lhs = self.lower_expr(lhs)?;
                let rhs = self.lower_expr(rhs)?;
                self.emit_value(
                    ControlFlowOp::Binary {
                        op: lower_binary_op(*op),
                        lhs,
                        rhs,
                    },
                    ty,
                    *span,
                )
            }
            Expr::TypeCheck { value, span, .. } => {
                let value = self.lower_expr(value)?;
                let target = self
                    .semantics
                    .type_check_type(*span)
                    .cloned()
                    .ok_or_else(|| LowerError::new(*span, "missing type guard target"))?;
                self.emit_value(ControlFlowOp::TypeCheck { value, target }, ty, *span)
            }
            Expr::Index {
                object,
                index,
                span,
            } => {
                let object = self.lower_expr(object)?;
                let index = self.lower_expr(index)?;
                self.emit_value(ControlFlowOp::IndexGet { object, index }, ty, *span)
            }
            Expr::OptionalIndex {
                object,
                index,
                span,
            } => self.lower_optional_index(object, index, ty, *span),
            Expr::Match { value, arms, span } => self.lower_match(value, arms, ty, *span),
            Expr::Assignment {
                op,
                target,
                value,
                span,
            } => self.lower_assignment(*op, target, value, ty, *span),
            Expr::Update {
                op,
                target,
                prefix,
                span,
            } => self.lower_update(*op, target, *prefix, ty, *span),
            Expr::Template { parts, span } => {
                let mut operands = Vec::with_capacity(parts.len());
                for part in *parts {
                    match part {
                        TemplatePart::String(value, _) => {
                            operands.push(TemplateOperand::String((*value).to_string()))
                        }
                        TemplatePart::Expr(expression) => {
                            operands.push(TemplateOperand::Value(self.lower_expr(expression)?))
                        }
                    }
                }
                self.emit_value(ControlFlowOp::Template(operands), ty, *span)
            }
        }
    }

    fn lower_ident(&mut self, ident: Ident<'src>, ty: Type<'src>) -> Result<ValueId, LowerError> {
        let symbol = self.symbol(ident)?;
        let value = self.lower_symbol_value(symbol, ident.span).map_err(|_| {
            LowerError::new(
                ident.span,
                format!(
                    "capturing `{}` is not represented in this closure",
                    ident.name
                ),
            )
        })?;
        let declared = self
            .semantics
            .symbols()
            .get(symbol.0 as usize)
            .ok_or_else(|| LowerError::new(ident.span, "missing identifier type"))?
            .ty
            .clone();
        let current = declared;
        if let Type::Nullable(inner) = &current {
            if inner.as_ref() == &ty {
                return self.emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::UnwrapNullable,
                        receiver: Some(value),
                        args: Vec::new(),
                    },
                    ty,
                    ident.span,
                );
            }
            if matches!(inner.as_ref(), Type::Union(members) if members.contains(&ty)) {
                let unwrapped = self.emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::UnwrapNullable,
                        receiver: Some(value),
                        args: Vec::new(),
                    },
                    inner.as_ref().clone(),
                    ident.span,
                )?;
                return self.emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::UnwrapUnion,
                        receiver: Some(unwrapped),
                        args: Vec::new(),
                    },
                    ty,
                    ident.span,
                );
            }
        }
        if matches!(current, Type::Union(ref members) if members.contains(&ty)) {
            return self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::UnwrapUnion,
                    receiver: Some(value),
                    args: Vec::new(),
                },
                ty,
                ident.span,
            );
        }
        Ok(value)
    }

    fn lower_symbol_value(&mut self, symbol: SymbolId, span: Span) -> Result<ValueId, LowerError> {
        let ty = self
            .semantics
            .symbols()
            .get(symbol.0 as usize)
            .map(|symbol| symbol.ty.clone())
            .ok_or_else(|| LowerError::new(span, "missing symbol type"))?;
        if let Some(value) = self.direct_value_by_symbol.get(&symbol).copied() {
            return Ok(value);
        }
        if let Some(local) = self.local_by_symbol.get(&symbol).copied() {
            return self.emit_value(ControlFlowOp::LoadLocal(local), ty, span);
        }
        if self.global_symbols.contains(&symbol) {
            return self.emit_value(ControlFlowOp::LoadGlobal(symbol), ty, span);
        }
        if let Some(function) = self.function_symbols.get(&symbol).copied() {
            return self.emit_value(
                ControlFlowOp::Closure {
                    function,
                    captures: Vec::new(),
                },
                ty,
                span,
            );
        }
        Err(LowerError::new(
            span,
            "symbol is unavailable in this function",
        ))
    }

    fn lower_capture_value(&mut self, symbol: SymbolId, span: Span) -> Result<ValueId, LowerError> {
        if !self.mutable_capture_symbols.contains(&symbol) {
            return self.lower_symbol_value(symbol, span);
        }
        let ty = self
            .semantics
            .symbols()
            .get(symbol.0 as usize)
            .map(|symbol| symbol.ty.clone())
            .ok_or_else(|| LowerError::new(span, "missing mutable capture type"))?;
        let local =
            self.local_by_symbol.get(&symbol).copied().ok_or_else(|| {
                LowerError::new(span, "mutable capture has no lexical local storage")
            })?;
        self.emit_value(ControlFlowOp::CaptureLocal(local), ty, span)
    }

    fn lower_member<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        property: Ident<'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let object_type = self.expression_type(object)?;
        let object_value = self.lower_expr(object)?;
        self.lower_member_value(object_type, object_value, property, ty, span)
    }

    fn lower_member_value(
        &mut self,
        object_type: Type<'src>,
        object_value: ValueId,
        property: Ident<'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        match object_type {
            Type::Record(_) => self.emit_value(
                ControlFlowOp::RecordFieldGet {
                    object: object_value,
                    property: property.name,
                },
                ty,
                span,
            ),
            Type::Struct(owner) | Type::StructInstance { name: owner, .. } => {
                let field = self
                    .semantics
                    .struct_info(owner)
                    .and_then(|info| info.fields.get(property.name))
                    .ok_or_else(|| LowerError::new(property.span, "missing struct field"))?;
                self.emit_value(
                    ControlFlowOp::FieldGet {
                        object: object_value,
                        owner,
                        field: property.name,
                        index: field.index,
                    },
                    ty,
                    span,
                )
            }
            Type::Class(owner) | Type::ClassInstance { name: owner, .. } => {
                if self.semantics.is_extern_class(owner) {
                    let info = self
                        .semantics
                        .class_info(owner)
                        .ok_or_else(|| LowerError::new(property.span, "missing extern class"))?;
                    if info.methods.contains_key(property.name) {
                        return Err(LowerError::new(
                            property.span,
                            "extern methods must be called through their receiver",
                        ));
                    }
                    return self.emit_value(
                        ControlFlowOp::HostFieldGet {
                            object: object_value,
                            property: property.name,
                        },
                        ty,
                        span,
                    );
                }
                let field = self
                    .semantics
                    .class_info(owner)
                    .and_then(|info| info.fields.get(property.name))
                    .ok_or_else(|| {
                        LowerError::new(
                            property.span,
                            "bound method values require call-site lowering",
                        )
                    })?;
                self.emit_value(
                    ControlFlowOp::FieldGet {
                        object: object_value,
                        owner,
                        field: property.name,
                        index: field.index,
                    },
                    ty,
                    span,
                )
            }
            Type::Array(_) if property.name == "length" => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayLength,
                    receiver: Some(object_value),
                    args: Vec::new(),
                },
                ty,
                span,
            ),
            Type::TypeParameter("$js") if property.name == "length" => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayLength,
                    receiver: Some(object_value),
                    args: Vec::new(),
                },
                ty,
                span,
            ),
            Type::TypeParameter("$js") if matches!(property.name, "message" | "specifier") => self
                .emit_value(
                    ControlFlowOp::HostFieldGet {
                        object: object_value,
                        property: property.name,
                    },
                    ty,
                    span,
                ),
            Type::Map(_, _) if property.name == "size" => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::MapSize,
                    receiver: Some(object_value),
                    args: Vec::new(),
                },
                ty,
                span,
            ),
            Type::Set(_) if property.name == "size" => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::SetSize,
                    receiver: Some(object_value),
                    args: Vec::new(),
                },
                ty,
                span,
            ),
            Type::ArrayBuffer | Type::SharedArrayBuffer if property.name == "byteLength" => self
                .emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::BufferByteLength,
                        receiver: Some(object_value),
                        args: Vec::new(),
                    },
                    ty,
                    span,
                ),
            object_ty if let Some(kind) = TypedArrayKind::from_type(&object_ty) => {
                let intrinsic = kind.property_intrinsic(property.name).ok_or_else(|| {
                    LowerError::new(
                        span,
                        format!("member `{}` must be called in this context", property.name),
                    )
                })?;
                self.emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic,
                        receiver: Some(object_value),
                        args: Vec::new(),
                    },
                    ty,
                    span,
                )
            }
            Type::Union(members)
                if property.name == "length"
                    && members.iter().all(indexed_collection_has_length) =>
            {
                self.emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::ArrayLength,
                        receiver: Some(object_value),
                        args: Vec::new(),
                    },
                    ty,
                    span,
                )
            }
            Type::String if property.name == "length" => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::StringLength,
                    receiver: Some(object_value),
                    args: Vec::new(),
                },
                ty,
                span,
            ),
            Type::Regex => {
                let intrinsic = match property.name {
                    "source" => Intrinsic::RegexSource,
                    "flags" => Intrinsic::RegexFlags,
                    "global" => Intrinsic::RegexGlobal,
                    "ignoreCase" => Intrinsic::RegexIgnoreCase,
                    "multiline" => Intrinsic::RegexMultiline,
                    "dotAll" => Intrinsic::RegexDotAll,
                    "sticky" => Intrinsic::RegexSticky,
                    "unicode" => Intrinsic::RegexUnicode,
                    _ => {
                        return Err(LowerError::new(
                            span,
                            format!("member `{}` must be called in this context", property.name),
                        ));
                    }
                };
                self.emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic,
                        receiver: Some(object_value),
                        args: Vec::new(),
                    },
                    ty,
                    span,
                )
            }
            Type::ModuleNamespace(_) | Type::ModuleLoadError => self.emit_value(
                ControlFlowOp::HostFieldGet {
                    object: object_value,
                    property: property.name,
                },
                ty,
                span,
            ),
            _ => Err(LowerError::new(
                span,
                format!("member `{}` must be called in this context", property.name),
            )),
        }
    }

    fn lower_optional_member<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        property: Ident<'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let Type::Nullable(inner) = self.expression_type(object)? else {
            return Err(LowerError::new(
                span,
                "optional member receiver is not nullable",
            ));
        };
        let present_ty = self
            .semantics
            .optional_present_type(span)
            .cloned()
            .ok_or_else(|| LowerError::new(span, "missing optional member result type"))?;
        let object_value = self.lower_expr(object)?;
        let (present_block, absent_block, merge_block) =
            self.begin_optional_access(object_value, span)?;

        self.current = present_block;
        let unwrapped = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::UnwrapNullable,
                receiver: Some(object_value),
                args: Vec::new(),
            },
            inner.as_ref().clone(),
            span,
        )?;
        let present = self.lower_member_value(*inner, unwrapped, property, present_ty, span)?;
        let present_end = self.current;
        self.terminate(Terminator::Jump(merge_block))?;

        self.finish_optional_access(
            object_value,
            present_end,
            present,
            absent_block,
            merge_block,
            ty,
            span,
        )
    }

    fn lower_optional_index<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        index: &Expr<'ast, 'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let Type::Nullable(inner) = self.expression_type(object)? else {
            return Err(LowerError::new(
                span,
                "optional index receiver is not nullable",
            ));
        };
        let present_ty = self
            .semantics
            .optional_present_type(span)
            .cloned()
            .ok_or_else(|| LowerError::new(span, "missing optional index result type"))?;
        let object_value = self.lower_expr(object)?;
        let (present_block, absent_block, merge_block) =
            self.begin_optional_access(object_value, span)?;

        self.current = present_block;
        let unwrapped = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::UnwrapNullable,
                receiver: Some(object_value),
                args: Vec::new(),
            },
            inner.as_ref().clone(),
            span,
        )?;
        let index = self.lower_expr(index)?;
        let present = self.emit_value(
            ControlFlowOp::IndexGet {
                object: unwrapped,
                index,
            },
            present_ty,
            span,
        )?;
        let present_end = self.current;
        self.terminate(Terminator::Jump(merge_block))?;

        self.finish_optional_access(
            object_value,
            present_end,
            present,
            absent_block,
            merge_block,
            ty,
            span,
        )
    }

    fn lower_optional_member_with_fallback<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        property: Ident<'src>,
        optional_span: Span,
        fallback: &Expr<'ast, 'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let Type::Nullable(inner) = self.expression_type(object)? else {
            return Err(LowerError::new(
                span,
                "optional member receiver is not nullable",
            ));
        };
        let present_ty = self
            .semantics
            .optional_present_type(optional_span)
            .cloned()
            .ok_or_else(|| LowerError::new(span, "missing optional member result type"))?;
        let object_value = self.lower_expr(object)?;
        let (present_block, absent_block, merge_block) =
            self.begin_optional_access(object_value, span)?;
        self.current = present_block;
        let unwrapped = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::UnwrapNullable,
                receiver: Some(object_value),
                args: Vec::new(),
            },
            inner.as_ref().clone(),
            span,
        )?;
        let present = self.lower_member_value(*inner, unwrapped, property, present_ty, span)?;
        let present_end = self.current;
        self.terminate(Terminator::Jump(merge_block))?;
        self.finish_optional_access_with_fallback(
            object_value,
            present_end,
            present,
            absent_block,
            merge_block,
            fallback,
            ty,
            span,
        )
    }

    fn lower_optional_index_with_fallback<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        index: &Expr<'ast, 'src>,
        optional_span: Span,
        fallback: &Expr<'ast, 'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let Type::Nullable(inner) = self.expression_type(object)? else {
            return Err(LowerError::new(
                span,
                "optional index receiver is not nullable",
            ));
        };
        let present_ty = self
            .semantics
            .optional_present_type(optional_span)
            .cloned()
            .ok_or_else(|| LowerError::new(span, "missing optional index result type"))?;
        let object_value = self.lower_expr(object)?;
        let (present_block, absent_block, merge_block) =
            self.begin_optional_access(object_value, span)?;
        self.current = present_block;
        let unwrapped = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::UnwrapNullable,
                receiver: Some(object_value),
                args: Vec::new(),
            },
            inner.as_ref().clone(),
            span,
        )?;
        let index = self.lower_expr(index)?;
        let present = self.emit_value(
            ControlFlowOp::IndexGet {
                object: unwrapped,
                index,
            },
            present_ty,
            span,
        )?;
        let present_end = self.current;
        self.terminate(Terminator::Jump(merge_block))?;
        self.finish_optional_access_with_fallback(
            object_value,
            present_end,
            present,
            absent_block,
            merge_block,
            fallback,
            ty,
            span,
        )
    }

    fn begin_optional_access(
        &mut self,
        object: ValueId,
        span: Span,
    ) -> Result<(BlockId, BlockId, BlockId), LowerError> {
        let null = self.emit_value(ControlFlowOp::Const(ConstValue::Null), Type::Null, span)?;
        let present = self.emit_value(
            ControlFlowOp::Binary {
                op: IrBinaryOp::NotEq,
                lhs: object,
                rhs: null,
            },
            Type::Bool,
            span,
        )?;
        let header = self.current;
        let present_block = self.add_block(span);
        let absent_block = self.add_block(span);
        let merge_block = self.add_block(span);
        self.shapes.push(ControlShape::If {
            header,
            then_block: present_block,
            else_block: absent_block,
            merge_block,
        });
        self.terminate(Terminator::Branch {
            condition: present,
            then_block: present_block,
            else_block: absent_block,
        })?;
        Ok((present_block, absent_block, merge_block))
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_optional_access(
        &mut self,
        object_value: ValueId,
        present_block: BlockId,
        present: ValueId,
        absent_block: BlockId,
        merge_block: BlockId,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        self.current = absent_block;
        let absent = self.emit_value(ControlFlowOp::Const(ConstValue::Null), Type::Null, span)?;
        self.terminate(Terminator::Jump(merge_block))?;
        self.current = merge_block;
        let out = self.new_value(EscapeState::LocalOnly);
        self.block_mut(merge_block).phis.push(Phi {
            out,
            origin: crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::OptionalAccess {
                object: object_value,
            }),
            ty,
            incoming: vec![(present_block, present), (absent_block, absent)],
            span,
        });
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_optional_access_with_fallback<'ast>(
        &mut self,
        object_value: ValueId,
        present_block: BlockId,
        present: ValueId,
        absent_block: BlockId,
        merge_block: BlockId,
        fallback: &Expr<'ast, 'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        self.current = absent_block;
        let absent = self.lower_expr(fallback)?;
        let absent_end = self.current;
        self.terminate(Terminator::Jump(merge_block))?;
        self.current = merge_block;
        let out = self.new_value(EscapeState::LocalOnly);
        self.block_mut(merge_block).phis.push(Phi {
            out,
            origin: crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::OptionalAccess {
                object: object_value,
            }),
            ty,
            incoming: vec![(present_block, present), (absent_end, absent)],
            span,
        });
        Ok(out)
    }

    fn lower_call<'ast>(
        &mut self,
        callee: &Expr<'ast, 'src>,
        args: &[Expr<'ast, 'src>],
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        if let Some(builtin) = self.semantics.builtin_call(span) {
            return self.lower_builtin_call(builtin, args, ty, span);
        }
        if let Expr::Member {
            object, property, ..
        } = callee
        {
            let receiver_type = self.expression_type(object)?;
            if matches!(receiver_type, Type::Task(_))
                && matches!(property.name, "then" | "catch" | "finally")
            {
                let receiver = self.lower_expr(object)?;
                let args = self.lower_args(args)?;
                return self.emit_value(
                    ControlFlowOp::HostCall {
                        receiver,
                        method: property.name,
                        args,
                        pure: false,
                    },
                    ty,
                    span,
                );
            }
            if let Some(intrinsic) = member_intrinsic(&receiver_type, property.name) {
                let receiver = self.lower_expr(object)?;
                let args = if matches!(intrinsic, Intrinsic::BufferSlice)
                    || is_typed_array_range_intrinsic(intrinsic)
                {
                    let callee_type = self.expression_type(callee)?;
                    let signature = match &callee_type {
                        Type::Function(signature) => Some(signature.clone()),
                        Type::GenericFunction(function) => Some(function.signature.clone()),
                        _ => None,
                    };
                    self.lower_args_with_optional_signature(args, signature.as_ref(), span)?
                } else {
                    self.lower_args(args)?
                };
                return self.emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic,
                        receiver: Some(receiver),
                        args,
                    },
                    ty,
                    span,
                );
            }
        }
        let callee_type = self.expression_type(callee)?;
        let signature = match &callee_type {
            Type::Function(signature) => Some(signature.clone()),
            Type::GenericFunction(function) => Some(function.signature.clone()),
            _ => None,
        };
        if let Expr::Ident(ident) = callee {
            let symbol = self.symbol(*ident)?;
            if let Some(function) = self.function_symbols.get(&symbol).copied() {
                let provided_args = args.len();
                let args =
                    self.lower_args_with_optional_signature(args, signature.as_ref(), span)?;
                return self.emit_value(
                    ControlFlowOp::CallDirect {
                        function,
                        args,
                        provided_args,
                    },
                    ty,
                    span,
                );
            }
        }

        if let Expr::Member {
            object, property, ..
        } = callee
        {
            let receiver_type = self.expression_type(object)?;
            if let Type::Class(class) | Type::ClassInstance { name: class, .. } = receiver_type {
                if self.semantics.is_extern_class(class) {
                    if let Some(method) = self
                        .semantics
                        .class_info(class)
                        .and_then(|info| info.methods.get(property.name))
                    {
                        let receiver = self.lower_expr(object)?;
                        let args = self.lower_args_with_optional_signature(
                            args,
                            signature.as_ref(),
                            span,
                        )?;
                        return self.emit_value(
                            ControlFlowOp::HostCall {
                                receiver,
                                method: property.name,
                                args,
                                pure: method.declared_pure,
                            },
                            ty,
                            span,
                        );
                    }
                }
                let owner = self
                    .semantics
                    .class_method_owner(class, property.name)
                    .unwrap_or(class);
                if let Some(function) = self.method_functions.get(&(owner, property.name)).copied()
                {
                    let receiver = self.lower_expr(object)?;
                    let args =
                        self.lower_args_with_optional_signature(args, signature.as_ref(), span)?;
                    return self.emit_value(
                        ControlFlowOp::CallMethod {
                            receiver,
                            class: owner,
                            method: property.name,
                            function,
                            args,
                        },
                        ty,
                        span,
                    );
                }
            }
        }

        let callee = self.lower_expr(callee)?;
        let args = self.lower_args_with_optional_signature(args, signature.as_ref(), span)?;
        self.emit_value(ControlFlowOp::CallValue { callee, args }, ty, span)
    }

    fn lower_builtin_call<'ast>(
        &mut self,
        builtin: BuiltinCall,
        args: &[Expr<'ast, 'src>],
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        use BuiltinCall::*;

        if builtin == JsObject {
            return self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsPlainObject,
                    receiver: None,
                    args: Vec::new(),
                },
                ty,
                span,
            );
        }
        if builtin == JsArray {
            return self.emit_value(ControlFlowOp::Array(Vec::new()), ty, span);
        }
        if builtin == JsUndefined {
            return self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsUndefined,
                    receiver: None,
                    args: Vec::new(),
                },
                ty,
                span,
            );
        }
        if matches!(builtin, JsOr | JsAnd) {
            return self.lower_js_short_circuit(builtin, args, ty, span);
        }

        let values = self.lower_args(args)?;
        let static_intrinsic = match builtin {
            Print => Some(Intrinsic::Print),
            MathImul => Some(Intrinsic::IntImul),
            ObjectKeys => Some(Intrinsic::RecordKeys),
            ObjectValues => Some(Intrinsic::RecordValues),
            ObjectHasOwn => Some(Intrinsic::RecordHasOwn),
            ObjectAssign => Some(Intrinsic::RecordAssign),
            JsonStringify => Some(Intrinsic::JsonStringify),
            JsonParse => Some(Intrinsic::JsonParse),
            TaskResolve => Some(Intrinsic::TaskResolve),
            TaskReject => Some(Intrinsic::TaskReject),
            TaskAll => Some(Intrinsic::TaskAll),
            _ => None,
        };
        if let Some(intrinsic) = static_intrinsic {
            return self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic,
                    receiver: None,
                    args: values,
                },
                ty,
                span,
            );
        }

        let receiver_intrinsic = match builtin {
            JsTypeOf => Some(Intrinsic::JsTypeOf),
            JsIsNullish => Some(Intrinsic::JsIsNullish),
            JsIsFalse => Some(Intrinsic::JsIsFalse),
            JsIsUndefined => Some(Intrinsic::JsIsUndefined),
            JsString => Some(Intrinsic::JsStringify),
            JsNumber => Some(Intrinsic::JsNumber),
            JsAdd => Some(Intrinsic::JsAdd),
            JsMod => Some(Intrinsic::JsMod),
            JsLessThan => Some(Intrinsic::JsLessThan),
            JsLessThanOrEqual => Some(Intrinsic::JsLessThanOrEqual),
            JsGreaterThan => Some(Intrinsic::JsGreaterThan),
            JsGreaterThanOrEqual => Some(Intrinsic::JsGreaterThanOrEqual),
            JsAssume => Some(Intrinsic::UnwrapUnion),
            JsStrictEqual => Some(Intrinsic::JsStrictEqual),
            JsStrictNotEqual => Some(Intrinsic::JsStrictNotEqual),
            JsBox => Some(Intrinsic::JsBox),
            JsArrayPop => Some(Intrinsic::JsArrayPop),
            JsArraySlice => Some(Intrinsic::JsArraySlice),
            JsArrayIndexOf => Some(Intrinsic::JsArrayIndexOf),
            JsArraySort => Some(Intrinsic::JsArraySort),
            JsArraySplice => Some(Intrinsic::JsArraySplice),
            JsArrayJoin => Some(Intrinsic::JsArrayJoin),
            JsArrayShift => Some(Intrinsic::JsArrayShift),
            JsIsArray => Some(Intrinsic::JsIsArray),
            JsStringSlice => Some(Intrinsic::JsStringSlice),
            JsStringIndexOf => Some(Intrinsic::JsStringIndexOf),
            JsStringReplace => Some(Intrinsic::JsStringReplace),
            JsStringMatch => Some(Intrinsic::JsStringMatch),
            JsStringSplit => Some(Intrinsic::JsStringSplit),
            JsRegexTest => Some(Intrinsic::RegexTest),
            JsRegexExec => Some(Intrinsic::JsRegexExec),
            _ => None,
        };
        if let Some(intrinsic) = receiver_intrinsic {
            let (&receiver, tail) = values
                .split_first()
                .ok_or_else(|| LowerError::new(span, "JavaScript intrinsic requires a receiver"))?;
            return self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic,
                    receiver: Some(receiver),
                    args: tail.to_vec(),
                },
                ty,
                span,
            );
        }

        match builtin {
            JsCall => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsCall,
                    receiver: values.first().copied(),
                    args: values[1..].to_vec(),
                },
                ty,
                span,
            ),
            JsConstruct => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsConstruct,
                    receiver: values.first().copied(),
                    args: values[1..].to_vec(),
                },
                ty,
                span,
            ),
            JsInvoke => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsInvoke,
                    receiver: values.first().copied(),
                    args: values[1..].to_vec(),
                },
                ty,
                span,
            ),
            JsApply => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsApply,
                    receiver: values.first().copied(),
                    args: values[1..].to_vec(),
                },
                ty,
                span,
            ),
            JsMethod0 | JsMethod1 | JsMethodRest | JsStaticRest => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: match builtin {
                        JsMethod0 => Intrinsic::JsMethod0,
                        JsMethod1 => Intrinsic::JsMethod1,
                        JsMethodRest => Intrinsic::JsMethodRest,
                        JsStaticRest => Intrinsic::JsStaticRest,
                        _ => unreachable!(),
                    },
                    receiver: None,
                    args: values,
                },
                ty,
                span,
            ),
            JsGet => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsGetProperty,
                    receiver: Some(values[0]),
                    args: vec![values[1]],
                },
                ty,
                span,
            ),
            JsSet => {
                self.emit_effect(
                    ControlFlowOp::IndexSet {
                        object: values[0],
                        index: values[1],
                        value: values[2],
                    },
                    span,
                )?;
                self.emit_void_result(span)
            }
            JsDelete | JsHas | JsIn | JsArrayPush | JsArrayConcatApply | JsArrayUnshift => {
                let intrinsic = match builtin {
                    JsDelete => Intrinsic::JsDeleteProperty,
                    JsHas => Intrinsic::JsHasProperty,
                    JsIn => Intrinsic::JsInProperty,
                    JsArrayPush => Intrinsic::JsArrayPush,
                    JsArrayConcatApply => Intrinsic::JsArrayConcatApply,
                    JsArrayUnshift => Intrinsic::JsArrayUnshift,
                    _ => unreachable!(),
                };
                let (receiver, args) = if builtin == JsIn {
                    (values[1], vec![values[0]])
                } else {
                    (values[0], values[1..].to_vec())
                };
                self.emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic,
                        receiver: Some(receiver),
                        args,
                    },
                    ty,
                    span,
                )
            }
            JsObject | JsArray | JsUndefined | JsOr | JsAnd => unreachable!(),
            _ => Err(LowerError::new(span, "unhandled JavaScript builtin")),
        }
    }

    fn lower_js_short_circuit<'ast>(
        &mut self,
        builtin: BuiltinCall,
        args: &[Expr<'ast, 'src>],
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let [lhs_expr, rhs_expr] = args else {
            return Err(LowerError::new(
                span,
                "JavaScript short-circuit intrinsic requires two operands",
            ));
        };
        let lhs = self.lower_expr(lhs_expr)?;
        let condition = self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::JsTruthy,
                receiver: Some(lhs),
                args: Vec::new(),
            },
            Type::Bool,
            lhs_expr.span(),
        )?;
        let header = self.current;
        let rhs_block = self.add_block(rhs_expr.span());
        let short_block = self.add_block(span);
        let merge_block = self.add_block(span);
        let (then_block, else_block) = match builtin {
            BuiltinCall::JsAnd => (rhs_block, short_block),
            BuiltinCall::JsOr => (short_block, rhs_block),
            _ => unreachable!(),
        };
        self.shapes.push(ControlShape::If {
            header,
            then_block,
            else_block,
            merge_block,
        });
        self.terminate(Terminator::Branch {
            condition,
            then_block,
            else_block,
        })?;

        self.current = short_block;
        self.terminate(Terminator::Jump(merge_block))?;

        self.current = rhs_block;
        let rhs = self.lower_expr(rhs_expr)?;
        let rhs_end = self.current;
        self.terminate(Terminator::Jump(merge_block))?;

        self.current = merge_block;
        let out = self.new_value(EscapeState::LocalOnly);
        self.block_mut(merge_block).phis.push(Phi {
            out,
            origin: crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::ShortCircuit {
                op: match builtin {
                    BuiltinCall::JsAnd => crate::ir::ShortCircuitOp::JavaScriptAnd,
                    BuiltinCall::JsOr => crate::ir::ShortCircuitOp::JavaScriptOr,
                    _ => unreachable!(),
                },
                lhs,
            }),
            ty,
            incoming: vec![(short_block, lhs), (rhs_end, rhs)],
            span,
        });
        Ok(out)
    }

    fn emit_void_result(&mut self, span: Span) -> Result<ValueId, LowerError> {
        if !self.block_open(self.current) {
            return Err(LowerError::new(span, "cannot emit into a terminated block"));
        }
        let out = self.new_value(EscapeState::LocalOnly);
        self.block_mut(self.current)
            .instructions
            .push(ControlFlowInstruction {
                out: Some(out),
                ty: Some(Type::Void),
                op: ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsUndefined,
                    receiver: None,
                    args: Vec::new(),
                },
                span,
            });
        Ok(out)
    }

    fn lower_args_with_optional_signature<'ast>(
        &mut self,
        args: &[Expr<'ast, 'src>],
        signature: Option<&FunctionType<'src>>,
        span: Span,
    ) -> Result<Vec<ValueId>, LowerError> {
        match signature {
            Some(signature) => self.lower_args_with_defaults(args, signature, span),
            None => self.lower_args(args),
        }
    }

    fn lower_args_with_defaults<'ast>(
        &mut self,
        args: &[Expr<'ast, 'src>],
        signature: &FunctionType<'src>,
        span: Span,
    ) -> Result<Vec<ValueId>, LowerError> {
        let mut values = self.lower_args(args)?;
        for index in args.len()..signature.params.len() {
            let default = signature
                .defaults
                .get(index)
                .and_then(Option::as_ref)
                .ok_or_else(|| LowerError::new(span, "missing lowered parameter default"))?;
            let value =
                self.lower_default_value(default, &signature.params[index], span, &values)?;
            values.push(value);
        }
        Ok(values)
    }

    fn lower_default_value(
        &mut self,
        default: &DefaultValue<'src>,
        ty: &Type<'src>,
        span: Span,
        materialized_args: &[ValueId],
    ) -> Result<ValueId, LowerError> {
        if let DefaultValue::Parameter(index) = default {
            return materialized_args.get(*index).copied().ok_or_else(|| {
                LowerError::new(
                    span,
                    format!("default references unavailable parameter {index}"),
                )
            });
        }
        if let DefaultValue::Struct { name, values } = default {
            let fields = self
                .semantics
                .struct_info(name)
                .ok_or_else(|| LowerError::new(span, format!("missing struct `{name}`")))?
                .fields
                .values()
                .map(|field| field.ty.clone())
                .collect::<Vec<_>>();
            if fields.len() != values.len() {
                return Err(LowerError::new(
                    span,
                    format!("default struct `{name}` has the wrong field count"),
                ));
            }
            let values = values
                .iter()
                .zip(&fields)
                .map(|(value, field_ty)| {
                    self.lower_default_value(value, field_ty, span, materialized_args)
                })
                .collect::<Result<Vec<_>, _>>()?;
            return self.emit_value(
                ControlFlowOp::Struct {
                    name,
                    fields: values,
                },
                ty.clone(),
                span,
            );
        }
        if let DefaultValue::NewClass { name, args } = default {
            let signature = self
                .semantics
                .class_info(name)
                .and_then(|class| class.constructor.as_ref())
                .cloned();
            let mut lowered = Vec::new();
            if let Some(signature) = signature {
                for (index, argument) in args.iter().enumerate() {
                    let expected = signature.params.get(index).ok_or_else(|| {
                        LowerError::new(span, format!("too many default arguments for `{name}`"))
                    })?;
                    lowered.push(self.lower_default_value(
                        argument,
                        expected,
                        span,
                        materialized_args,
                    )?);
                }
                for index in args.len()..signature.params.len() {
                    let omitted = signature
                        .defaults
                        .get(index)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| {
                            LowerError::new(
                                span,
                                format!("missing default constructor argument for `{name}`"),
                            )
                        })?;
                    let value = self.lower_default_value(
                        omitted,
                        &signature.params[index],
                        span,
                        &lowered,
                    )?;
                    lowered.push(value);
                }
            } else if !args.is_empty() {
                return Err(LowerError::new(
                    span,
                    format!("default construction for `{name}` does not accept arguments"),
                ));
            }
            return self.emit_value(
                ControlFlowOp::NewClass {
                    class: name,
                    constructor: self.constructors.get(name).copied(),
                    args: lowered,
                },
                ty.clone(),
                span,
            );
        }
        if let DefaultValue::Arrow(arrow_span) = default {
            let function = self.arrows.get(arrow_span).copied().ok_or_else(|| {
                LowerError::new(*arrow_span, "default arrow was not assigned an IR function")
            })?;
            let captures = self
                .arrow_captures
                .get(arrow_span)
                .ok_or_else(|| {
                    LowerError::new(*arrow_span, "default arrow captures were not analyzed")
                })?
                .iter()
                .map(|symbol| self.lower_capture_value(*symbol, *arrow_span))
                .collect::<Result<Vec<_>, _>>()?;
            return self.emit_value(
                ControlFlowOp::Closure { function, captures },
                ty.clone(),
                *arrow_span,
            );
        }
        if let DefaultValue::Array(elements) = default {
            let element_ty = default_array_element_type(ty).ok_or_else(|| {
                LowerError::new(span, "array default has no compatible array parameter type")
            })?;
            let values = elements
                .iter()
                .map(|element| {
                    self.lower_default_value(element, element_ty, span, materialized_args)
                })
                .collect::<Result<Vec<_>, _>>()?;
            return self.emit_value(
                ControlFlowOp::Array(values),
                Type::Array(Box::new(element_ty.clone())),
                span,
            );
        }
        if let DefaultValue::Symbol(symbol) = default {
            return self.lower_symbol_value(*symbol, span);
        }
        if let DefaultValue::PendingIdentifier(default_span) = default {
            return Err(LowerError::new(
                *default_span,
                "parameter-default binding was not finalized",
            ));
        }
        if let DefaultValue::PendingUndefined(default_span) = default {
            return Err(LowerError::new(
                *default_span,
                "parameter-default builtin was not finalized",
            ));
        }
        if matches!(default, DefaultValue::Undefined) {
            return self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsUndefined,
                    receiver: None,
                    args: Vec::new(),
                },
                ty.clone(),
                span,
            );
        }
        let value = match default {
            DefaultValue::Int(value) => ConstValue::Int(*value),
            DefaultValue::Float(bits) => ConstValue::Float(f64::from_bits(*bits)),
            DefaultValue::String(value) => ConstValue::String((*value).to_string()),
            DefaultValue::Bool(value) => ConstValue::Bool(*value),
            DefaultValue::Null => ConstValue::Null,
            DefaultValue::Symbol(_)
            | DefaultValue::PendingIdentifier(_)
            | DefaultValue::PendingUndefined(_) => unreachable!(),
            DefaultValue::Undefined => unreachable!(),
            DefaultValue::Parameter(_) => unreachable!(),
            DefaultValue::Array(_)
            | DefaultValue::Arrow(_)
            | DefaultValue::Struct { .. }
            | DefaultValue::NewClass { .. } => unreachable!(),
        };
        self.emit_value(ControlFlowOp::Const(value), ty.clone(), span)
    }

    fn lower_args<'ast>(&mut self, args: &[Expr<'ast, 'src>]) -> Result<Vec<ValueId>, LowerError> {
        args.iter().map(|arg| self.lower_expr(arg)).collect()
    }

    fn lower_short_circuit<'ast>(
        &mut self,
        op: BinaryOp,
        lhs: &Expr<'ast, 'src>,
        rhs: &Expr<'ast, 'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let lhs = self.lower_expr(lhs)?;
        let header = self.current;
        let rhs_block = self.add_block(rhs.span());
        let short_block = self.add_block(span);
        let merge_block = self.add_block(span);
        let (then_block, else_block, short_value) = match op {
            BinaryOp::And => (rhs_block, short_block, false),
            BinaryOp::Or => (short_block, rhs_block, true),
            _ => unreachable!(),
        };
        self.shapes.push(ControlShape::If {
            header,
            then_block,
            else_block,
            merge_block,
        });
        self.terminate(Terminator::Branch {
            condition: lhs,
            then_block,
            else_block,
        })?;

        self.current = short_block;
        let short = self.emit_value(
            ControlFlowOp::Const(ConstValue::Bool(short_value)),
            Type::Bool,
            span,
        )?;
        self.terminate(Terminator::Jump(merge_block))?;

        self.current = rhs_block;
        let rhs = self.lower_expr(rhs)?;
        let rhs_end = self.current;
        self.terminate(Terminator::Jump(merge_block))?;

        self.current = merge_block;
        let out = self.new_value(EscapeState::LocalOnly);
        self.block_mut(merge_block).phis.push(Phi {
            out,
            origin: crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::ShortCircuit {
                op: match op {
                    BinaryOp::And => crate::ir::ShortCircuitOp::BooleanAnd,
                    BinaryOp::Or => crate::ir::ShortCircuitOp::BooleanOr,
                    _ => unreachable!(),
                },
                lhs,
            }),
            ty,
            incoming: vec![(short_block, short), (rhs_end, rhs)],
            span,
        });
        Ok(out)
    }

    fn lower_nullish<'ast>(
        &mut self,
        lhs_expr: &Expr<'ast, 'src>,
        rhs_expr: &Expr<'ast, 'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        // Keep the complete nullish operation in its own structured region.
        // If earlier statements share this block, JavaScript expression
        // reconstruction would otherwise have to absorb those unrelated
        // effects and conservatively fall back to a temporary plus `??=`.
        // The jump is target-neutral and gives every backend the same SSA;
        // the JavaScript backend can now emit an effectful lhs exactly once as
        // native `lhs??rhs`.
        if !self.block(self.current).instructions.is_empty() {
            let header = self.add_block(span);
            self.terminate(Terminator::Jump(header))?;
            self.current = header;
        }
        let lhs_ty = self.expression_type(lhs_expr)?;
        let lhs = self.lower_expr(lhs_expr)?;
        let null = self.emit_value(
            ControlFlowOp::Const(ConstValue::Null),
            Type::Null,
            lhs_expr.span(),
        )?;
        let present = self.emit_value(
            ControlFlowOp::Binary {
                op: IrBinaryOp::NotEq,
                lhs,
                rhs: null,
            },
            Type::Bool,
            span,
        )?;
        let header = self.current;
        let present_block = self.add_block(span);
        let rhs_block = self.add_block(rhs_expr.span());
        let merge_block = self.add_block(span);
        self.shapes.push(ControlShape::If {
            header,
            then_block: present_block,
            else_block: rhs_block,
            merge_block,
        });
        self.terminate(Terminator::Branch {
            condition: present,
            then_block: present_block,
            else_block: rhs_block,
        })?;

        self.current = present_block;
        let present_value = self.unwrap_nullish_value(lhs, &lhs_ty, &ty, span)?;
        self.terminate(Terminator::Jump(merge_block))?;

        self.current = rhs_block;
        let rhs = self.lower_expr(rhs_expr)?;
        let rhs_end = self.current;
        self.terminate(Terminator::Jump(merge_block))?;

        self.current = merge_block;
        let out = self.new_value(EscapeState::LocalOnly);
        self.block_mut(merge_block).phis.push(Phi {
            out,
            origin: crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Nullish { lhs }),
            ty,
            incoming: vec![(present_block, present_value), (rhs_end, rhs)],
            span,
        });
        Ok(out)
    }

    fn lower_match<'ast>(
        &mut self,
        value: &Expr<'ast, 'src>,
        arms: &[MatchArm<'ast, 'src>],
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let enum_ty = self.expression_type(value)?;
        let Type::Enum(_) = enum_ty else {
            return Err(LowerError::new(span, "match value is not an enum"));
        };
        let scrutinee = self.lower_expr(value)?;
        let merge_block = self.add_block(span);
        let mut incoming = Vec::with_capacity(arms.len());

        for (index, arm) in arms.iter().enumerate() {
            let is_last = index + 1 == arms.len();
            if !is_last {
                let discriminant = self
                    .semantics
                    .enum_variant_value(arm.pattern.span())
                    .ok_or_else(|| LowerError::new(arm.span, "missing match discriminant"))?;
                let constant = self.emit_value(
                    ControlFlowOp::Const(ConstValue::Int(discriminant)),
                    enum_ty.clone(),
                    arm.pattern.span(),
                )?;
                let condition = self.emit_value(
                    ControlFlowOp::Binary {
                        op: IrBinaryOp::Eq,
                        lhs: scrutinee,
                        rhs: constant,
                    },
                    Type::Bool,
                    arm.pattern.span(),
                )?;
                let header = self.current;
                let arm_block = self.add_block(arm.span);
                let next_block = self.add_block(span);
                self.shapes.push(ControlShape::If {
                    header,
                    then_block: arm_block,
                    else_block: next_block,
                    merge_block,
                });
                self.terminate(Terminator::Branch {
                    condition,
                    then_block: arm_block,
                    else_block: next_block,
                })?;
                self.current = arm_block;
                let result = self.lower_expr(&arm.value)?;
                let arm_end = self.current;
                self.terminate(Terminator::Jump(merge_block))?;
                incoming.push((arm_end, result));
                self.current = next_block;
            } else {
                let result = self.lower_expr(&arm.value)?;
                let arm_end = self.current;
                self.terminate(Terminator::Jump(merge_block))?;
                incoming.push((arm_end, result));
            }
        }

        self.current = merge_block;
        let out = self.new_value(EscapeState::LocalOnly);
        self.block_mut(merge_block).phis.push(Phi {
            out,
            origin: crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Conditional),
            ty,
            incoming,
            span,
        });
        Ok(out)
    }

    fn lower_return_match<'ast>(
        &mut self,
        value: &Expr<'ast, 'src>,
        arms: &[MatchArm<'ast, 'src>],
        span: Span,
    ) -> Result<(), LowerError> {
        let enum_ty = self.expression_type(value)?;
        let Type::Enum(_) = enum_ty else {
            return Err(LowerError::new(span, "match value is not an enum"));
        };
        let scrutinee = self.lower_expr(value)?;
        let Some((last, tested_arms)) = arms.split_last() else {
            return Err(LowerError::new(span, "match expression has no arms"));
        };
        if tested_arms.is_empty() {
            let result = self.lower_expr(&last.value)?;
            return self.terminate(Terminator::Return(Some(result)));
        }
        let fallback_block = self.add_block(last.span);

        for (index, arm) in tested_arms.iter().enumerate() {
            let discriminant = self
                .semantics
                .enum_variant_value(arm.pattern.span())
                .ok_or_else(|| LowerError::new(arm.span, "missing match discriminant"))?;
            let constant = self.emit_value(
                ControlFlowOp::Const(ConstValue::Int(discriminant)),
                enum_ty.clone(),
                arm.pattern.span(),
            )?;
            let condition = self.emit_value(
                ControlFlowOp::Binary {
                    op: IrBinaryOp::Eq,
                    lhs: scrutinee,
                    rhs: constant,
                },
                Type::Bool,
                arm.pattern.span(),
            )?;
            let header = self.current;
            let arm_block = self.add_block(arm.span);
            let next_block = if index + 1 == tested_arms.len() {
                fallback_block
            } else {
                self.add_block(span)
            };
            self.shapes.push(ControlShape::If {
                header,
                then_block: arm_block,
                else_block: next_block,
                merge_block: fallback_block,
            });
            self.terminate(Terminator::Branch {
                condition,
                then_block: arm_block,
                else_block: next_block,
            })?;
            self.current = arm_block;
            let result = self.lower_expr(&arm.value)?;
            self.terminate(Terminator::Return(Some(result)))?;
            self.current = next_block;
        }
        debug_assert_eq!(self.current, fallback_block);
        let result = self.lower_expr(&last.value)?;
        self.terminate(Terminator::Return(Some(result)))
    }

    fn unwrap_nullish_value(
        &mut self,
        value: ValueId,
        source: &Type<'src>,
        fallback: &Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let Type::Nullable(inner) = source else {
            return Ok(value);
        };
        self.emit_value(
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::UnwrapNullable,
                receiver: Some(value),
                args: Vec::new(),
            },
            if inner.as_ref() == fallback {
                fallback.clone()
            } else {
                inner.as_ref().clone()
            },
            span,
        )
    }

    fn lower_assignment<'ast>(
        &mut self,
        op: AssignmentOp,
        target: &Expr<'ast, 'src>,
        value: &Expr<'ast, 'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let place = self.lower_place(target)?;
        if op == AssignmentOp::Nullish {
            let target_ty = self.expression_type(target)?;
            let current = self.load_place(&place, target.span())?;
            let null = self.emit_value(
                ControlFlowOp::Const(ConstValue::Null),
                Type::Null,
                target.span(),
            )?;
            let absent = self.emit_value(
                ControlFlowOp::Binary {
                    op: IrBinaryOp::Eq,
                    lhs: current,
                    rhs: null,
                },
                Type::Bool,
                span,
            )?;
            let header = self.current;
            let rhs_block = self.add_block(value.span());
            let present_block = self.add_block(span);
            let merge_block = self.add_block(span);
            self.shapes.push(ControlShape::If {
                header,
                then_block: rhs_block,
                else_block: present_block,
                merge_block,
            });
            self.terminate(Terminator::Branch {
                condition: absent,
                then_block: rhs_block,
                else_block: present_block,
            })?;

            self.current = rhs_block;
            let rhs = self.lower_expr(value)?;
            self.store_place(&place, rhs, span)?;
            let rhs_end = self.current;
            self.terminate(Terminator::Jump(merge_block))?;

            self.current = present_block;
            let present_value = self.unwrap_nullish_value(current, &target_ty, &ty, span)?;
            self.terminate(Terminator::Jump(merge_block))?;

            self.current = merge_block;
            let out = self.new_value(EscapeState::LocalOnly);
            self.block_mut(merge_block).phis.push(Phi {
                out,
                origin: crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Nullish {
                    lhs: current,
                }),
                ty,
                incoming: vec![(rhs_end, rhs), (present_block, present_value)],
                span,
            });
            return Ok(out);
        }
        let value = if op == AssignmentOp::Assign {
            self.lower_expr(value)?
        } else {
            let current = self.load_place(&place, target.span())?;
            let rhs = self.lower_expr(value)?;
            self.emit_value(
                ControlFlowOp::Binary {
                    op: lower_assignment_op(op),
                    lhs: current,
                    rhs,
                },
                ty,
                span,
            )?
        };
        self.store_place(&place, value, span)?;
        Ok(value)
    }

    fn lower_update<'ast>(
        &mut self,
        op: UpdateOp,
        target: &Expr<'ast, 'src>,
        prefix: bool,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let place = self.lower_place(target)?;
        let old = self.load_place(&place, target.span())?;
        let one = self.emit_value(ControlFlowOp::Const(ConstValue::Int(1)), Type::Int, span)?;
        let new = self.emit_value(
            ControlFlowOp::Binary {
                op: match op {
                    UpdateOp::Increment => IrBinaryOp::Add,
                    UpdateOp::Decrement => IrBinaryOp::Sub,
                },
                lhs: old,
                rhs: one,
            },
            ty,
            span,
        )?;
        self.store_place(&place, new, span)?;
        Ok(if prefix { new } else { old })
    }

    fn lower_place<'ast>(
        &mut self,
        expression: &Expr<'ast, 'src>,
    ) -> Result<Place<'src>, LowerError> {
        let ty = self.expression_type(expression)?;
        match expression {
            Expr::Ident(ident) => {
                let symbol = self.symbol(*ident)?;
                if let Some(local) = self.local_by_symbol.get(&symbol).copied() {
                    Ok(Place::Local { local, ty })
                } else if self.global_symbols.contains(&symbol) {
                    if self.external_globals.contains(&symbol) {
                        return Err(LowerError::new(
                            ident.span,
                            "extern global bindings are read-only",
                        ));
                    }
                    Ok(Place::Global { symbol, ty })
                } else {
                    Err(LowerError::new(ident.span, "binding is not mutable here"))
                }
            }
            Expr::Member {
                object,
                property,
                span,
            } => {
                let object_type = self.expression_type(object)?;
                let object_value = self.lower_expr(object)?;
                if matches!(object_type, Type::Record(_)) {
                    return Ok(Place::RecordField {
                        object: object_value,
                        property: property.name,
                        ty,
                    });
                }
                let (owner, index) = match object_type {
                    Type::Struct(owner) | Type::StructInstance { name: owner, .. } => {
                        let index = self
                            .semantics
                            .struct_info(owner)
                            .and_then(|info| info.fields.get(property.name))
                            .map(|field| field.index)
                            .ok_or_else(|| LowerError::new(*span, "missing struct field"))?;
                        (owner, index)
                    }
                    Type::Class(owner) | Type::ClassInstance { name: owner, .. } => {
                        if self.semantics.is_extern_class(owner) {
                            return Ok(Place::HostField {
                                object: object_value,
                                property: property.name,
                                ty,
                            });
                        }
                        let index = self
                            .semantics
                            .class_info(owner)
                            .and_then(|info| info.fields.get(property.name))
                            .map(|field| field.index)
                            .ok_or_else(|| LowerError::new(*span, "missing class field"))?;
                        (owner, index)
                    }
                    _ => return Err(LowerError::new(*span, "member is not assignable")),
                };
                Ok(Place::Field {
                    object: object_value,
                    owner,
                    field: property.name,
                    index,
                    ty,
                })
            }
            Expr::Index {
                object,
                index,
                span: _,
            } => Ok(Place::Index {
                object: self.lower_expr(object)?,
                index: self.lower_expr(index)?,
                ty,
            }),
            _ => Err(LowerError::new(
                expression.span(),
                "expression is not an assignable place",
            )),
        }
    }

    fn load_place(&mut self, place: &Place<'src>, span: Span) -> Result<ValueId, LowerError> {
        match place {
            Place::Local { local, ty } => {
                self.emit_value(ControlFlowOp::LoadLocal(*local), ty.clone(), span)
            }
            Place::Global { symbol, ty } => {
                self.emit_value(ControlFlowOp::LoadGlobal(*symbol), ty.clone(), span)
            }
            Place::Field {
                object,
                owner,
                field,
                index,
                ty,
            } => self.emit_value(
                ControlFlowOp::FieldGet {
                    object: *object,
                    owner,
                    field,
                    index: *index,
                },
                ty.clone(),
                span,
            ),
            Place::HostField {
                object,
                property,
                ty,
            } => self.emit_value(
                ControlFlowOp::HostFieldGet {
                    object: *object,
                    property,
                },
                ty.clone(),
                span,
            ),
            Place::RecordField {
                object,
                property,
                ty,
            } => self.emit_value(
                ControlFlowOp::RecordFieldGet {
                    object: *object,
                    property,
                },
                ty.clone(),
                span,
            ),
            Place::Index { object, index, ty } => self.emit_value(
                ControlFlowOp::IndexGet {
                    object: *object,
                    index: *index,
                },
                ty.clone(),
                span,
            ),
        }
    }

    fn store_place(
        &mut self,
        place: &Place<'src>,
        value: ValueId,
        span: Span,
    ) -> Result<(), LowerError> {
        let op = match place {
            Place::Local { local, .. } => ControlFlowOp::StoreLocal {
                local: *local,
                value,
            },
            Place::Global { symbol, .. } => ControlFlowOp::StoreGlobal {
                global: *symbol,
                value,
            },
            Place::Field {
                object,
                owner,
                field,
                index,
                ..
            } => ControlFlowOp::FieldSet {
                object: *object,
                owner,
                field,
                index: *index,
                value,
            },
            Place::HostField {
                object, property, ..
            } => ControlFlowOp::HostFieldSet {
                object: *object,
                property,
                value,
            },
            Place::RecordField {
                object, property, ..
            } => ControlFlowOp::RecordFieldSet {
                object: *object,
                property,
                value,
            },
            Place::Index { object, index, .. } => ControlFlowOp::IndexSet {
                object: *object,
                index: *index,
                value,
            },
        };
        self.emit_effect(op, span)
    }

    fn expression_type<'ast>(
        &self,
        expression: &Expr<'ast, 'src>,
    ) -> Result<Type<'src>, LowerError> {
        self.semantics
            .expression_type(expression.span())
            .cloned()
            .ok_or_else(|| LowerError::new(expression.span(), "missing expression type"))
    }

    fn symbol(&self, ident: Ident<'src>) -> Result<SymbolId, LowerError> {
        self.semantics.identifier_symbol(ident.span).ok_or_else(|| {
            LowerError::new(ident.span, format!("missing symbol for `{}`", ident.name))
        })
    }

    fn add_local(
        &mut self,
        symbol: SymbolId,
        name: &'src str,
        ty: Type<'src>,
        span: Span,
    ) -> LocalId {
        let id = LocalId(self.locals.len() as u32);
        self.locals.push(IrLocal {
            id,
            symbol,
            name,
            ty,
            span,
        });
        self.local_by_symbol.insert(symbol, id);
        if self.mutable_capture_symbols.contains(&symbol) {
            self.mutable_capture_locals.push(id);
        }
        id
    }

    fn add_block(&mut self, span: Span) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        self.blocks.push(ControlFlowBlock {
            id,
            phis: Vec::new(),
            instructions: Vec::new(),
            terminator: None,
            span,
        });
        id
    }

    fn emit_value(
        &mut self,
        op: ControlFlowOp<'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        if !self.block_open(self.current) {
            return Err(LowerError::new(span, "cannot emit into a terminated block"));
        }
        let out = self.new_value(EscapeState::LocalOnly);
        self.block_mut(self.current)
            .instructions
            .push(ControlFlowInstruction {
                out: Some(out),
                ty: Some(ty),
                op,
                span,
            });
        Ok(out)
    }

    fn emit_effect(&mut self, op: ControlFlowOp<'src>, span: Span) -> Result<(), LowerError> {
        if !self.block_open(self.current) {
            return Err(LowerError::new(span, "cannot emit into a terminated block"));
        }
        self.block_mut(self.current)
            .instructions
            .push(ControlFlowInstruction {
                out: None,
                ty: None,
                op,
                span,
            });
        Ok(())
    }

    fn new_value(&mut self, escape: EscapeState) -> ValueId {
        let value = ValueId(self.next_value);
        self.next_value += 1;
        self.value_escapes.push(escape);
        value
    }

    fn terminate(&mut self, terminator: Terminator) -> Result<(), LowerError> {
        if !self.block_open(self.current) {
            return Err(LowerError::new(
                self.block(self.current).span,
                "basic block already has a terminator",
            ));
        }
        self.block_mut(self.current).terminator = Some(terminator);
        Ok(())
    }

    fn jump_if_open(&mut self, target: BlockId) -> Result<(), LowerError> {
        if self.block_open(self.current) {
            self.terminate(Terminator::Jump(target))?;
        }
        Ok(())
    }

    fn block(&self, id: BlockId) -> &ControlFlowBlock<'src> {
        &self.blocks[id.0 as usize]
    }

    fn block_mut(&mut self, id: BlockId) -> &mut ControlFlowBlock<'src> {
        &mut self.blocks[id.0 as usize]
    }

    fn block_open(&self, id: BlockId) -> bool {
        self.block(id).terminator.is_none()
    }

    fn finish(mut self) -> Result<ControlFlowFunction<'src>, LowerError> {
        if self.block_open(self.current) {
            let terminator = if self.return_type == Type::Void {
                Terminator::Return(None)
            } else {
                Terminator::Unreachable
            };
            self.terminate(terminator)?;
        }
        for block in &mut self.blocks {
            if block.terminator.is_none() {
                block.terminator = Some(Terminator::Unreachable);
            }
        }
        let mut value_local_hints = vec![None; self.next_value as usize];
        for parameter in &self.params {
            value_local_hints[parameter.value.0 as usize] = Some(parameter.name);
        }
        Ok(ControlFlowFunction {
            id: self.id,
            name: self.name,
            kind: self.kind,
            origin: FunctionOrigin::Source,
            declared_pure: self.declared_pure,
            is_async: self.is_async,
            is_generator: self.is_generator,
            params: self.params,
            capture_count: self.capture_count,
            mutable_capture_locals: self.mutable_capture_locals,
            return_type: self.return_type,
            locals: self.locals,
            blocks: self.blocks,
            shapes: self.shapes,
            entry: BlockId(0),
            value_count: self.next_value,
            value_local_hints,
            value_escapes: self.value_escapes,
            locals_promoted: false,
            live: true,
            span: self.span,
        })
    }
}

fn scalar_parameter_default(expression: &Expr<'_, '_>) -> Option<ConstValue> {
    match expression {
        Expr::Int(value, _) => Some(ConstValue::Int(*value)),
        Expr::Float(value, _) => Some(ConstValue::Float(*value)),
        Expr::String(value, _) => Some(ConstValue::String((*value).to_string())),
        Expr::Bool(value, _) => Some(ConstValue::Bool(*value)),
        Expr::Null(_) => Some(ConstValue::Null),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
            ..
        } => match scalar_parameter_default(expr)? {
            ConstValue::Int(value) => Some(ConstValue::Int(-value)),
            ConstValue::Float(value) => Some(ConstValue::Float(-value)),
            _ => None,
        },
        _ => None,
    }
}

fn default_array_element_type<'ty, 'src>(ty: &'ty Type<'src>) -> Option<&'ty Type<'src>> {
    match ty {
        Type::Array(element) => Some(element),
        Type::Nullable(inner) => default_array_element_type(inner),
        Type::Union(members) => members.iter().find_map(default_array_element_type),
        _ => None,
    }
}

fn indexed_collection_has_length(ty: &Type<'_>) -> bool {
    match ty {
        Type::Array(_) | Type::String => true,
        ty if TypedArrayKind::from_type(ty).is_some() => true,
        Type::Union(members) => members.iter().all(indexed_collection_has_length),
        _ => false,
    }
}

enum Place<'src> {
    Local {
        local: LocalId,
        ty: Type<'src>,
    },
    Global {
        symbol: SymbolId,
        ty: Type<'src>,
    },
    Field {
        object: ValueId,
        owner: &'src str,
        field: &'src str,
        index: usize,
        ty: Type<'src>,
    },
    HostField {
        object: ValueId,
        property: &'src str,
        ty: Type<'src>,
    },
    RecordField {
        object: ValueId,
        property: &'src str,
        ty: Type<'src>,
    },
    Index {
        object: ValueId,
        index: ValueId,
        ty: Type<'src>,
    },
}

fn resolve_declared_return<'ast, 'src>(
    semantics: &SemanticModel<'src>,
    function: &FunctionDecl<'ast, 'src>,
) -> Result<Type<'src>, LowerError> {
    let symbol = semantics
        .identifier_symbol(function.name.span)
        .ok_or_else(|| LowerError::new(function.name.span, "missing function symbol"))?;
    let function_type = semantics
        .symbols()
        .get(symbol.0 as usize)
        .map(|symbol| symbol.ty.clone())
        .ok_or_else(|| LowerError::new(function.name.span, "missing function type"))?;
    let return_type = match function_type {
        Type::Function(signature) => *signature.return_type,
        Type::GenericFunction(function) => *function.signature.return_type,
        _ => Err(LowerError::new(
            function.name.span,
            "function symbol does not have a callable type",
        ))?,
    };
    if function.is_async {
        let Type::Task(value) = return_type else {
            return Err(LowerError::new(
                function.name.span,
                "async function symbol does not return Task<T>",
            ));
        };
        Ok(*value)
    } else if function.is_generator {
        let Type::Generator(_) = return_type else {
            return Err(LowerError::new(
                function.name.span,
                "generator function symbol does not return Generator<T>",
            ));
        };
        Ok(Type::Void)
    } else {
        Ok(return_type)
    }
}

fn resolve_symbol_return<'src>(
    semantics: &SemanticModel<'src>,
    name: Ident<'src>,
    kind: &str,
) -> Result<Type<'src>, LowerError> {
    let symbol = semantics
        .identifier_symbol(name.span)
        .ok_or_else(|| LowerError::new(name.span, format!("missing {kind} symbol")))?;
    match semantics
        .symbols()
        .get(symbol.0 as usize)
        .map(|symbol| symbol.ty.clone())
    {
        Some(Type::Function(signature)) => Ok(*signature.return_type),
        Some(Type::GenericFunction(function)) => Ok(*function.signature.return_type),
        _ => Err(LowerError::new(
            name.span,
            format!("{kind} symbol does not have a callable type"),
        )),
    }
}

fn plan_span(plan: &PlannedFunction<'_, '_>, program_span: Span) -> Span {
    match plan {
        PlannedFunction::Entry => program_span,
        PlannedFunction::Function(function) | PlannedFunction::Method { function, .. } => {
            function.span
        }
        PlannedFunction::Extern(extern_decl) => extern_decl.span,
        PlannedFunction::Constructor { constructor, .. } => constructor.span,
        PlannedFunction::Arrow { span, .. } => *span,
    }
}

fn lower_binary_op(op: BinaryOp) -> IrBinaryOp {
    match op {
        BinaryOp::Add => IrBinaryOp::Add,
        BinaryOp::Sub => IrBinaryOp::Sub,
        BinaryOp::Mul => IrBinaryOp::Mul,
        BinaryOp::Div => IrBinaryOp::Div,
        BinaryOp::Mod => IrBinaryOp::Mod,
        BinaryOp::BitAnd => IrBinaryOp::BitAnd,
        BinaryOp::BitOr => IrBinaryOp::BitOr,
        BinaryOp::Xor => IrBinaryOp::Xor,
        BinaryOp::ShiftLeft => IrBinaryOp::ShiftLeft,
        BinaryOp::ShiftRight => IrBinaryOp::ShiftRight,
        BinaryOp::UnsignedShiftRight => IrBinaryOp::UnsignedShiftRight,
        BinaryOp::Eq => IrBinaryOp::Eq,
        BinaryOp::NotEq => IrBinaryOp::NotEq,
        BinaryOp::Less => IrBinaryOp::Less,
        BinaryOp::LessEq => IrBinaryOp::LessEq,
        BinaryOp::Greater => IrBinaryOp::Greater,
        BinaryOp::GreaterEq => IrBinaryOp::GreaterEq,
        BinaryOp::And => IrBinaryOp::And,
        BinaryOp::Or => IrBinaryOp::Or,
        BinaryOp::Nullish => unreachable!("nullish expressions lower through control flow"),
    }
}

fn lower_assignment_op(op: AssignmentOp) -> IrBinaryOp {
    match op {
        AssignmentOp::Assign | AssignmentOp::Nullish => unreachable!(),
        AssignmentOp::Add => IrBinaryOp::Add,
        AssignmentOp::Sub => IrBinaryOp::Sub,
        AssignmentOp::Mul => IrBinaryOp::Mul,
        AssignmentOp::Div => IrBinaryOp::Div,
        AssignmentOp::Mod => IrBinaryOp::Mod,
        AssignmentOp::BitAnd => IrBinaryOp::BitAnd,
        AssignmentOp::BitOr => IrBinaryOp::BitOr,
        AssignmentOp::Xor => IrBinaryOp::Xor,
        AssignmentOp::ShiftLeft => IrBinaryOp::ShiftLeft,
        AssignmentOp::ShiftRight => IrBinaryOp::ShiftRight,
        AssignmentOp::UnsignedShiftRight => IrBinaryOp::UnsignedShiftRight,
    }
}

fn member_intrinsic(receiver: &Type<'_>, property: &str) -> Option<Intrinsic> {
    match (receiver, property) {
        (Type::TypeParameter("$js"), "truthy") => Some(Intrinsic::JsTruthy),
        (Type::TypeParameter("$js"), "isArray") => Some(Intrinsic::JsIsArray),
        (Type::TypeParameter("$js"), "isObject") => Some(Intrinsic::JsIsObject),
        (Type::Array(_), "map") => Some(Intrinsic::ArrayMap),
        (Type::Array(_), "filter") => Some(Intrinsic::ArrayFilter),
        (Type::Array(_), "reduce") => Some(Intrinsic::ArrayReduce),
        (Type::Array(_), "forEach") => Some(Intrinsic::ArrayForEach),
        (Type::Array(_), "push") => Some(Intrinsic::ArrayPush),
        (Type::Array(_), "pop") => Some(Intrinsic::ArrayPop),
        (Type::Array(_), "indexOf") => Some(Intrinsic::ArrayIndexOf),
        (Type::Array(_), "includes") => Some(Intrinsic::ArrayIncludes),
        (Type::Array(_), "join") => Some(Intrinsic::ArrayJoin),
        (Type::Array(_), "some") => Some(Intrinsic::ArraySome),
        (Type::Array(_), "every") => Some(Intrinsic::ArrayEvery),
        (Type::Array(_), "findIndex") => Some(Intrinsic::ArrayFindIndex),
        (Type::Array(_), "concat") => Some(Intrinsic::ArrayConcat),
        (Type::Array(_), "copyWithin") => Some(Intrinsic::ArrayCopyWithin),
        (Type::Array(_), "reverse") => Some(Intrinsic::ArrayReverse),
        (Type::Array(_), "slice") => Some(Intrinsic::ArraySlice),
        (Type::Array(_), "splice") => Some(Intrinsic::ArraySplice),
        (Type::Array(_), "fill") => Some(Intrinsic::ArrayFill),
        (Type::Map(_, _), "get") => Some(Intrinsic::MapGet),
        (Type::Map(_, _), "set") => Some(Intrinsic::MapSet),
        (Type::Map(_, _), "has") => Some(Intrinsic::MapHas),
        (Type::Map(_, _), "delete") => Some(Intrinsic::MapDelete),
        (Type::Map(_, _), "clear") => Some(Intrinsic::MapClear),
        (Type::Set(_), "add") => Some(Intrinsic::SetAdd),
        (Type::Set(_), "has") => Some(Intrinsic::SetHas),
        (Type::Set(_), "delete") => Some(Intrinsic::SetDelete),
        (Type::Set(_), "clear") => Some(Intrinsic::SetClear),
        (Type::ArrayBuffer | Type::SharedArrayBuffer, "slice") => Some(Intrinsic::BufferSlice),
        (ty, "set") if TypedArrayKind::from_type(ty).is_some() => Some(Intrinsic::TypedArraySet),
        (ty, "fill") if TypedArrayKind::from_type(ty).is_some() => Some(Intrinsic::TypedArrayFill),
        (ty, "copyWithin") if TypedArrayKind::from_type(ty).is_some() => {
            Some(Intrinsic::TypedArrayCopyWithin)
        }
        (ty, method) if let Some(kind) = TypedArrayKind::from_type(ty) => {
            kind.method_intrinsic(method)
        }
        (Type::Float, "abs") => Some(Intrinsic::FloatAbs),
        (Type::Float, "floor") => Some(Intrinsic::FloatFloor),
        (Type::Float, "ceil") => Some(Intrinsic::FloatCeil),
        (Type::Float, "round") => Some(Intrinsic::FloatRound),
        (Type::Float, "sqrt") => Some(Intrinsic::FloatSqrt),
        (Type::Float, "sin") => Some(Intrinsic::FloatSin),
        (Type::Float, "cos") => Some(Intrinsic::FloatCos),
        (Type::Float, "acos") => Some(Intrinsic::FloatAcos),
        (Type::Float, "exp") => Some(Intrinsic::FloatExp),
        (Type::Float, "log") => Some(Intrinsic::FloatLog),
        (Type::Float, "tan") => Some(Intrinsic::FloatTan),
        (Type::Float, "atan2") => Some(Intrinsic::FloatAtan2),
        (Type::Float, "hypot") => Some(Intrinsic::FloatHypot),
        (Type::Float, "min") => Some(Intrinsic::FloatMin),
        (Type::Float, "max") => Some(Intrinsic::FloatMax),
        (Type::Float, "toInt") => Some(Intrinsic::FloatToInt),
        (Type::Int, "toString") => Some(Intrinsic::IntToString),
        (Type::Int, "toUnsignedString") => Some(Intrinsic::IntToUnsignedString),
        (Type::String, "includes") => Some(Intrinsic::StringIncludes),
        (Type::String, "indexOf") => Some(Intrinsic::StringIndexOf),
        (Type::String, "lastIndexOf") => Some(Intrinsic::StringLastIndexOf),
        (Type::String, "repeat") => Some(Intrinsic::StringRepeat),
        (Type::String, "charCodeAt") => Some(Intrinsic::StringCharCodeAt),
        (Type::String, "charAt") => Some(Intrinsic::StringCharAt),
        (Type::String, "startsWith") => Some(Intrinsic::StringStartsWith),
        (Type::String, "endsWith") => Some(Intrinsic::StringEndsWith),
        (Type::String, "toUpperCase") => Some(Intrinsic::StringToUpperCase),
        (Type::String, "toLowerCase") => Some(Intrinsic::StringToLowerCase),
        (Type::String, "truthy") => Some(Intrinsic::JsTruthy),
        (Type::Regex, "test") => Some(Intrinsic::RegexTest),
        _ => None,
    }
}

type ArrowRef<'ast, 'src> = (&'ast [Param<'ast, 'src>], &'ast ArrowBody<'ast, 'src>, Span);

fn collect_arrow_captures<'ast, 'src>(
    params: &[Param<'ast, 'src>],
    body: &ArrowBody<'ast, 'src>,
    arrow_span: Span,
    semantics: &SemanticModel<'src>,
    globals: &AHashSet<SymbolId>,
    functions: &AHashMap<SymbolId, FunctionId>,
) -> Vec<SymbolId> {
    let mut used = AHashSet::default();
    for param in params {
        if let Some(default) = &param.default {
            collect_expr_symbols(default, semantics, &mut used);
        }
    }
    match body {
        ArrowBody::Expr(expression) => collect_expr_symbols(expression, semantics, &mut used),
        ArrowBody::Block(statements) => collect_stmt_symbols(statements, semantics, &mut used),
    }
    let mut captures = used
        .into_iter()
        .filter(|symbol| !globals.contains(symbol) && !functions.contains_key(symbol))
        .filter(|symbol| {
            semantics
                .symbols()
                .get(symbol.0 as usize)
                .is_some_and(|symbol| {
                    symbol.span.start < arrow_span.start || symbol.span.end > arrow_span.end
                })
        })
        .collect::<Vec<_>>();
    captures.sort_by_key(|symbol| symbol.0);
    captures
}

fn collect_stmt_symbols<'ast, 'src>(
    statements: &[Stmt<'ast, 'src>],
    semantics: &SemanticModel<'src>,
    out: &mut AHashSet<SymbolId>,
) {
    for statement in statements {
        match statement {
            Stmt::VarDecl(decl) => {
                if let Some(initializer) = &decl.initializer {
                    collect_expr_symbols(initializer, semantics, out);
                }
            }
            Stmt::ArrayDestructure { value, .. } | Stmt::RecordDestructure { value, .. } => {
                collect_expr_symbols(value, semantics, out)
            }
            Stmt::Expr(expression) => collect_expr_symbols(expression, semantics, out),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    collect_expr_symbols(value, semantics, out);
                }
            }
            Stmt::Throw { value, .. } => collect_expr_symbols(value, semantics, out),
            Stmt::SuperCall { args, .. } => {
                for argument in *args {
                    collect_expr_symbols(argument, semantics, out);
                }
            }
            Stmt::Yield { value, .. } => collect_expr_symbols(value, semantics, out),
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                collect_stmt_symbols(body, semantics, out);
                if let Some(clause) = catch {
                    collect_stmt_symbols(clause.body, semantics, out);
                }
                if let Some(body) = finally {
                    collect_stmt_symbols(body, semantics, out);
                }
            }
            Stmt::Block { body, .. } => collect_stmt_symbols(body, semantics, out),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                collect_expr_symbols(condition, semantics, out);
                collect_stmt_symbols(std::slice::from_ref(*then_branch), semantics, out);
                if let Some(else_branch) = else_branch {
                    collect_stmt_symbols(std::slice::from_ref(*else_branch), semantics, out);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_expr_symbols(condition, semantics, out);
                collect_stmt_symbols(std::slice::from_ref(*body), semantics, out);
            }
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                if let Some(initializer) = initializer {
                    match initializer {
                        ForInitializer::VarDecl(decl) => {
                            if let Some(initializer) = &decl.initializer {
                                collect_expr_symbols(initializer, semantics, out);
                            }
                        }
                        ForInitializer::Expr(expression) => {
                            collect_expr_symbols(expression, semantics, out)
                        }
                    }
                }
                if let Some(condition) = condition {
                    collect_expr_symbols(condition, semantics, out);
                }
                if let Some(update) = update {
                    collect_expr_symbols(update, semantics, out);
                }
                collect_stmt_symbols(std::slice::from_ref(*body), semantics, out);
            }
            Stmt::ForIn { object, body, .. } => {
                collect_expr_symbols(object, semantics, out);
                collect_stmt_symbols(std::slice::from_ref(*body), semantics, out);
            }
            Stmt::ForOf { iterable, body, .. } => {
                collect_expr_symbols(iterable, semantics, out);
                collect_stmt_symbols(std::slice::from_ref(*body), semantics, out);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }
}

fn collect_expr_symbols<'ast, 'src>(
    expression: &Expr<'ast, 'src>,
    semantics: &SemanticModel<'src>,
    out: &mut AHashSet<SymbolId>,
) {
    match expression {
        Expr::Ident(ident) => {
            if let Some(symbol) = semantics.identifier_symbol(ident.span) {
                out.insert(symbol);
            }
        }
        Expr::ArrowFunction { body, .. } => match body {
            ArrowBody::Expr(expression) => collect_expr_symbols(expression, semantics, out),
            ArrowBody::Block(statements) => collect_stmt_symbols(statements, semantics, out),
        },
        Expr::Match { value, arms, .. } => {
            collect_expr_symbols(value, semantics, out);
            for arm in *arms {
                collect_expr_symbols(&arm.value, semantics, out);
            }
        }
        Expr::ArrayLiteral { elements, .. } => {
            for element in *elements {
                collect_expr_symbols(element.value(), semantics, out);
            }
        }
        Expr::RecordLiteral { entries, .. } => {
            for entry in *entries {
                collect_expr_symbols(entry.value(), semantics, out);
            }
        }
        Expr::StructLiteral { values, .. } | Expr::New { args: values, .. } => {
            for value in *values {
                collect_expr_symbols(value, semantics, out);
            }
        }
        Expr::Member { object, .. }
        | Expr::OptionalMember { object, .. }
        | Expr::Unary { expr: object, .. }
        | Expr::Await { task: object, .. }
        | Expr::TypeCheck { value: object, .. } => collect_expr_symbols(object, semantics, out),
        Expr::Call { callee, args, .. } => {
            collect_expr_symbols(callee, semantics, out);
            for arg in *args {
                collect_expr_symbols(arg, semantics, out);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_symbols(lhs, semantics, out);
            collect_expr_symbols(rhs, semantics, out);
        }
        Expr::Index { object, index, .. } => {
            collect_expr_symbols(object, semantics, out);
            collect_expr_symbols(index, semantics, out);
        }
        Expr::OptionalIndex { object, index, .. } => {
            collect_expr_symbols(object, semantics, out);
            collect_expr_symbols(index, semantics, out);
        }
        Expr::Assignment { target, value, .. } => {
            collect_expr_symbols(target, semantics, out);
            collect_expr_symbols(value, semantics, out);
        }
        Expr::Update { target, .. } => collect_expr_symbols(target, semantics, out),
        Expr::Template { parts, .. } => {
            for part in *parts {
                if let TemplatePart::Expr(expression) = part {
                    collect_expr_symbols(expression, semantics, out);
                }
            }
        }
        Expr::Int(_, _)
        | Expr::Float(_, _)
        | Expr::String(_, _)
        | Expr::Bool(_, _)
        | Expr::Null(_)
        | Expr::DynamicImport { .. } => {}
    }
}

fn collect_program_arrows<'ast, 'src>(
    program: &Program<'ast, 'src>,
    out: &mut Vec<ArrowRef<'ast, 'src>>,
) {
    for item in program.items {
        match item {
            Item::Enum(_) => {}
            Item::Struct(_) => {}
            Item::Class(class) => {
                for member in class.members {
                    match member {
                        ClassMember::Field(_) => {}
                        ClassMember::Constructor(constructor) => {
                            collect_param_arrows(constructor.params, out);
                            collect_stmt_arrows(constructor.body, out)
                        }
                        ClassMember::Method(method) => {
                            collect_param_arrows(method.params, out);
                            collect_stmt_arrows(method.body, out);
                        }
                    }
                }
            }
            Item::ExternClass(class) => {
                for member in class.members {
                    if let ExternClassMember::Method(method) = member {
                        collect_param_arrows(method.params, out);
                    }
                }
            }
            Item::Function(function) => {
                collect_param_arrows(function.params, out);
                collect_stmt_arrows(function.body, out);
            }
            Item::Extern(extern_decl) => collect_param_arrows(extern_decl.params, out),
            Item::ExternGlobal(_) => {}
            Item::Stmt(statement) => collect_one_stmt_arrows(statement, out),
        }
    }
}

fn collect_param_arrows<'ast, 'src>(
    params: &'ast [Param<'ast, 'src>],
    out: &mut Vec<ArrowRef<'ast, 'src>>,
) {
    for param in params {
        if let Some(default) = &param.default {
            collect_expr_arrows(default, out);
        }
    }
}

fn collect_stmt_arrows<'ast, 'src>(
    statements: &'ast [Stmt<'ast, 'src>],
    out: &mut Vec<ArrowRef<'ast, 'src>>,
) {
    for statement in statements {
        collect_one_stmt_arrows(statement, out);
    }
}

fn collect_one_stmt_arrows<'ast, 'src>(
    statement: &'ast Stmt<'ast, 'src>,
    out: &mut Vec<ArrowRef<'ast, 'src>>,
) {
    match statement {
        Stmt::VarDecl(decl) => {
            if let Some(initializer) = &decl.initializer {
                collect_expr_arrows(initializer, out);
            }
        }
        Stmt::ArrayDestructure { value, .. } | Stmt::RecordDestructure { value, .. } => {
            collect_expr_arrows(value, out)
        }
        Stmt::Expr(expression) => collect_expr_arrows(expression, out),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                collect_expr_arrows(value, out);
            }
        }
        Stmt::Throw { value, .. } => collect_expr_arrows(value, out),
        Stmt::SuperCall { args, .. } => {
            for argument in *args {
                collect_expr_arrows(argument, out);
            }
        }
        Stmt::Yield { value, .. } => collect_expr_arrows(value, out),
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            collect_stmt_arrows(body, out);
            if let Some(clause) = catch {
                collect_stmt_arrows(clause.body, out);
            }
            if let Some(body) = finally {
                collect_stmt_arrows(body, out);
            }
        }
        Stmt::Block { body, .. } => collect_stmt_arrows(body, out),
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_arrows(condition, out);
            collect_one_stmt_arrows(then_branch, out);
            if let Some(else_branch) = else_branch {
                collect_one_stmt_arrows(else_branch, out);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            collect_expr_arrows(condition, out);
            collect_one_stmt_arrows(body, out);
        }
        Stmt::For {
            initializer,
            condition,
            update,
            body,
            ..
        } => {
            if let Some(initializer) = initializer {
                match initializer {
                    ForInitializer::VarDecl(decl) => {
                        if let Some(initializer) = &decl.initializer {
                            collect_expr_arrows(initializer, out);
                        }
                    }
                    ForInitializer::Expr(expression) => collect_expr_arrows(expression, out),
                }
            }
            if let Some(condition) = condition {
                collect_expr_arrows(condition, out);
            }
            if let Some(update) = update {
                collect_expr_arrows(update, out);
            }
            collect_one_stmt_arrows(body, out);
        }
        Stmt::ForIn { object, body, .. } => {
            collect_expr_arrows(object, out);
            collect_one_stmt_arrows(body, out);
        }
        Stmt::ForOf { iterable, body, .. } => {
            collect_expr_arrows(iterable, out);
            collect_one_stmt_arrows(body, out);
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_expr_arrows<'ast, 'src>(
    expression: &'ast Expr<'ast, 'src>,
    out: &mut Vec<ArrowRef<'ast, 'src>>,
) {
    match expression {
        Expr::ArrowFunction { params, body, span } => {
            out.push((params, body, *span));
            collect_param_arrows(params, out);
            match body {
                ArrowBody::Expr(expression) => collect_expr_arrows(expression, out),
                ArrowBody::Block(statements) => collect_stmt_arrows(statements, out),
            }
        }
        Expr::Match { value, arms, .. } => {
            collect_expr_arrows(value, out);
            for arm in *arms {
                collect_expr_arrows(&arm.value, out);
            }
        }
        Expr::ArrayLiteral { elements, .. } => {
            for element in *elements {
                collect_expr_arrows(element.value(), out);
            }
        }
        Expr::RecordLiteral { entries, .. } => {
            for entry in *entries {
                collect_expr_arrows(entry.value(), out);
            }
        }
        Expr::StructLiteral { values, .. } | Expr::New { args: values, .. } => {
            for value in *values {
                collect_expr_arrows(value, out);
            }
        }
        Expr::Member { object, .. }
        | Expr::OptionalMember { object, .. }
        | Expr::Unary { expr: object, .. }
        | Expr::Await { task: object, .. }
        | Expr::TypeCheck { value: object, .. } => collect_expr_arrows(object, out),
        Expr::Call { callee, args, .. } => {
            collect_expr_arrows(callee, out);
            for arg in *args {
                collect_expr_arrows(arg, out);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_arrows(lhs, out);
            collect_expr_arrows(rhs, out);
        }
        Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
            collect_expr_arrows(object, out);
            collect_expr_arrows(index, out);
        }
        Expr::Assignment { target, value, .. } => {
            collect_expr_arrows(target, out);
            collect_expr_arrows(value, out);
        }
        Expr::Update { target, .. } => collect_expr_arrows(target, out),
        Expr::Template { parts, .. } => {
            for part in *parts {
                if let TemplatePart::Expr(expression) = part {
                    collect_expr_arrows(expression, out);
                }
            }
        }
        Expr::Int(_, _)
        | Expr::Float(_, _)
        | Expr::String(_, _)
        | Expr::Bool(_, _)
        | Expr::Null(_)
        | Expr::Ident(_)
        | Expr::DynamicImport { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::{analyze, parse_source};

    fn lower(source: &str) -> ControlFlowModule<'_> {
        let arena = Box::leak(Box::new(Bump::new()));
        let source = Box::leak(source.to_string().into_boxed_str());
        let program = parse_source(arena, source).unwrap();
        let semantics = Box::leak(Box::new(analyze(&program).unwrap()));
        lower_to_control_flow(&program, semantics).unwrap()
    }

    #[test]
    fn lowers_branches_and_loops_to_blocks() {
        let module = lower("int sum=0;for(int i=0;i<3;i++){sum+=i;}if(sum==3){print(sum);}");
        let entry = &module.functions[module.entry.0 as usize];
        assert!(entry.blocks.len() >= 8);
        assert!(entry
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Some(Terminator::Branch { .. }))));
    }

    #[test]
    fn lowers_methods_as_directly_identified_calls() {
        let module = lower(
            "class Box{int value;init(int value){this.value=value;}int get(){return this.value;}}Box b=new Box(7);int n=b.get();",
        );
        assert!(module.functions[0].blocks.iter().any(|block| block
            .instructions
            .iter()
            .any(|instruction| matches!(instruction.op, ControlFlowOp::CallMethod { .. }))));
    }

    #[test]
    fn preserves_source_arity_while_materializing_direct_call_defaults() {
        let module = lower(
            "int adjust(int value,int amount=2){return value+amount;}int omitted=adjust(3);int explicit=adjust(4,5);",
        );
        let adjust = module
            .functions
            .iter()
            .find(|function| function.name == Some("adjust"))
            .unwrap()
            .id;
        let calls = module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::CallDirect {
                    function,
                    args,
                    provided_args,
                } if *function == adjust => Some((args.len(), *provided_args)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(calls, vec![(2, 1), (2, 2)]);
    }

    #[test]
    fn lowers_short_circuit_to_phi() {
        let module = lower("bool a=true;bool b=false;bool c=a&&b;");
        assert!(module.functions[0]
            .blocks
            .iter()
            .any(|block| !block.phis.is_empty()));
    }

    #[test]
    fn lowers_recursive_local_function_through_its_own_slot() {
        let module = lower(
            "int walk(int n){func(int)->int step=(int x)=>{if(x<=0){return 0;}return x+step(x-1);};return step(n);}print(walk(4));",
        );
        assert!(module
            .functions
            .iter()
            .any(|function| function.kind == FunctionKind::Closure && function.capture_count >= 1));
        assert!(module.functions.iter().any(|function| function
            .blocks
            .iter()
            .any(|block| block.instructions.iter().any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::CaptureLocal(_)
            )))));
    }

    #[test]
    fn lowers_lexical_closure_captures_explicitly() {
        let module = lower(
            "int apply(int factor){auto callback=(int value)=>value*factor;return callback(4);}print(apply(3));",
        );
        let closure = module
            .functions
            .iter()
            .find(|function| function.kind == FunctionKind::Closure)
            .unwrap();
        assert_eq!(closure.capture_count, 1);
        assert!(module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .any(
                |block| block.instructions.iter().any(|instruction| matches!(
                    &instruction.op,
                    ControlFlowOp::Closure { captures, .. } if captures.len() == 1
                ))
            ));
    }

    #[test]
    fn lowers_inherited_methods_and_base_constructor_calls_directly() {
        let module = lower(
            "class Base{int value;init(int value){this.value=value;}int get(){return this.value;}}class Child extends Base{int bonus;init(int value,int bonus){super(value);this.bonus=bonus;}}Child child=new Child(7,2);int result=child.get();",
        );
        let child = module
            .classes
            .iter()
            .find(|layout| layout.name == "Child")
            .unwrap();
        assert_eq!(child.fields.len(), 2);
        assert_eq!(child.fields[0].name, "value");
        assert_eq!(child.fields[1].name, "bonus");
        let base_constructor = module
            .functions
            .iter()
            .find(|function| matches!(function.kind, FunctionKind::Constructor { class: "Base" }))
            .unwrap()
            .id;
        let child_constructor = module
            .functions
            .iter()
            .find(|function| matches!(function.kind, FunctionKind::Constructor { class: "Child" }))
            .unwrap();
        assert!(child_constructor
            .blocks
            .iter()
            .any(
                |block| block.instructions.iter().any(|instruction| matches!(
                    instruction.op,
                    ControlFlowOp::CallMethod { function, .. } if function == base_constructor
                ))
            ));
    }

    #[test]
    fn lowers_generators_to_yield_intrinsics_and_native_for_of_shapes() {
        let module = lower(
            "generator int values(){yield 1;yield* [2,3];}int sum=0;for(int value of values()){sum+=value;}",
        );
        let generator = module
            .functions
            .iter()
            .find(|function| function.name == Some("values"))
            .unwrap();
        assert!(generator.is_generator);
        assert!(generator
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::GeneratorYield,
                    ..
                }
            )));
        assert!(module.functions[module.entry.0 as usize]
            .shapes
            .iter()
            .any(|shape| matches!(shape, ControlShape::ForOf { .. })));
    }
}
