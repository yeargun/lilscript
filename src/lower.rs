use ahash::{AHashMap, AHashSet};

use crate::ast::{
    ArrowBody, AssignmentOp, BinaryOp, ClassMember, ConstructorDecl, Expr, ExternDecl,
    ForInitializer, FunctionDecl, Ident, Item, Param, Program, Stmt, TemplatePart, UnaryOp,
    UpdateOp, VarDecl,
};
use crate::ir::{
    AggregateField, AggregateLayout, BlockId, ConstValue, ControlFlowBlock, ControlFlowFunction,
    ControlFlowInstruction, ControlFlowModule, ControlFlowOp, ControlShape, ExportBinding,
    FunctionId, FunctionKind, Intrinsic, IrBinaryOp, IrExport, IrGlobal, IrLocal, IrParameter,
    IrUnaryOp, LocalId, Phi, TemplateOperand, Terminator, ValueId,
};
use crate::semantic::{EscapeState, SemanticModel, SymbolId, Type};
use crate::span::Span;

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
    global_symbols: AHashSet<SymbolId>,
    globals: Vec<IrGlobal<'src>>,
    exports: Vec<IrExport<'src>>,
}

impl<'model, 'ast, 'src> ModuleLowerer<'model, 'ast, 'src> {
    fn new(
        program: &Program<'ast, 'src>,
        semantics: &'model SemanticModel<'src>,
    ) -> Result<Self, LowerError> {
        let mut lowerer = Self {
            semantics,
            plans: vec![PlannedFunction::Entry],
            function_symbols: AHashMap::new(),
            method_functions: AHashMap::new(),
            constructors: AHashMap::new(),
            arrows: AHashMap::new(),
            arrow_captures: AHashMap::new(),
            global_symbols: AHashSet::new(),
            globals: Vec::new(),
            exports: Vec::new(),
        };

        let mut function_names = AHashMap::new();
        let mut global_names = AHashMap::new();
        let mut type_names = AHashSet::new();

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
                        span: decl.span,
                    });
                }
                Item::Struct(decl) => {
                    type_names.insert(decl.name.name);
                }
                _ => {}
            }
        }

        let mut exported_names = AHashSet::new();
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

        let mut arrows = Vec::new();
        collect_program_arrows(program, &mut arrows);
        for (params, body, span) in arrows {
            let id = FunctionId(lowerer.plans.len() as u32);
            let captures = collect_arrow_captures(
                body,
                span,
                semantics,
                &lowerer.global_symbols,
                &lowerer.function_symbols,
            );
            lowerer.arrows.insert(span, id);
            lowerer.arrow_captures.insert(span, captures.clone());
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
                &self.global_symbols,
                plan_span(plan, program.span),
            );

            match plan {
                PlannedFunction::Entry => {
                    builder.kind = FunctionKind::Entry;
                    builder.return_type = Type::Void;
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
                    builder.return_type = self
                        .semantics
                        .class_info(class)
                        .and_then(|info| info.methods.get(function.name.name))
                        .map(|signature| (*signature.return_type).clone())
                        .ok_or_else(|| {
                            LowerError::new(function.name.span, "missing method signature")
                        })?;
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

        Ok(ControlFlowModule {
            functions,
            globals: self.globals,
            exports: self.exports,
            structs: self
                .semantics
                .structs()
                .map(|info| AggregateLayout {
                    name: info.name,
                    fields: info
                        .fields
                        .values()
                        .map(|field| AggregateField {
                            name: field.name,
                            ty: field.ty.clone(),
                            index: field.index,
                        })
                        .collect(),
                })
                .collect(),
            classes: self
                .semantics
                .classes()
                .map(|info| AggregateLayout {
                    name: info.name,
                    fields: info
                        .fields
                        .values()
                        .map(|field| AggregateField {
                            name: field.name,
                            ty: field.ty.clone(),
                            index: field.index,
                        })
                        .collect(),
                })
                .collect(),
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
    return_type: Type<'src>,
    semantics: &'model SemanticModel<'src>,
    function_symbols: &'maps AHashMap<SymbolId, FunctionId>,
    method_functions: &'maps AHashMap<(&'src str, &'src str), FunctionId>,
    constructors: &'maps AHashMap<&'src str, FunctionId>,
    arrows: &'maps AHashMap<Span, FunctionId>,
    arrow_captures: &'maps AHashMap<Span, Vec<SymbolId>>,
    global_symbols: &'maps AHashSet<SymbolId>,
    params: Vec<IrParameter<'src>>,
    capture_count: usize,
    locals: Vec<IrLocal<'src>>,
    local_by_symbol: AHashMap<SymbolId, LocalId>,
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
        global_symbols: &'maps AHashSet<SymbolId>,
        span: Span,
    ) -> Self {
        Self {
            id,
            name: None,
            kind: FunctionKind::Function,
            declared_pure: false,
            return_type: Type::Void,
            semantics,
            function_symbols,
            method_functions,
            constructors,
            arrows,
            arrow_captures,
            global_symbols,
            params: Vec::new(),
            capture_count: 0,
            locals: Vec::new(),
            local_by_symbol: AHashMap::new(),
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

    fn add_this(&mut self, span: Span, class: &'src str) -> Result<(), LowerError> {
        let symbol = self
            .semantics
            .identifier_symbol(span)
            .ok_or_else(|| LowerError::new(span, "missing `this` symbol"))?;
        self.add_param(symbol, "this", Type::Class(class), span)
    }

    fn add_params<'ast>(&mut self, params: &[Param<'ast, 'src>]) -> Result<(), LowerError> {
        for param in params {
            let symbol = self.symbol(param.name)?;
            let ty = self
                .semantics
                .binding_type(param.name.span)
                .cloned()
                .ok_or_else(|| LowerError::new(param.span, "missing parameter type"))?;
            self.add_param(symbol, param.name.name, ty, param.span)?;
        }
        Ok(())
    }

    fn add_captures(&mut self, captures: &[SymbolId]) -> Result<(), LowerError> {
        for symbol in captures {
            let capture = self
                .semantics
                .symbols()
                .get(symbol.0 as usize)
                .ok_or_else(|| LowerError::new(self.span, "missing captured symbol"))?;
            self.add_param(capture.id, capture.name, capture.ty.clone(), capture.span)?;
            self.capture_count += 1;
        }
        Ok(())
    }

    fn add_param(
        &mut self,
        symbol: SymbolId,
        name: &'src str,
        ty: Type<'src>,
        span: Span,
    ) -> Result<(), LowerError> {
        let local = self.add_local(symbol, name, ty.clone(), span);
        let value = self.new_value(EscapeState::LocalOnly);
        self.params.push(IrParameter {
            symbol,
            local,
            value,
            name,
            ty,
            span,
        });
        self.emit_effect(ControlFlowOp::StoreLocal { local, value }, span)
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
            Stmt::Expr(expression) => {
                self.lower_expr(expression)?;
                Ok(())
            }
            Stmt::Return { value, .. } => {
                let value = value
                    .as_ref()
                    .map(|value| self.lower_expr(value))
                    .transpose()?;
                self.terminate(Terminator::Return(value))
            }
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

    fn lower_var_decl<'ast>(&mut self, decl: &VarDecl<'ast, 'src>) -> Result<(), LowerError> {
        let symbol = self.symbol(decl.name)?;
        let ty = self
            .semantics
            .binding_type(decl.name.span)
            .cloned()
            .ok_or_else(|| LowerError::new(decl.span, "missing variable type"))?;
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
        } else {
            let local = self.add_local(symbol, decl.name.name, ty, decl.span);
            self.emit_effect(ControlFlowOp::StoreLocal { local, value }, decl.span)
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
            Expr::Ident(ident) => self.lower_ident(*ident, ty),
            Expr::ArrayLiteral { elements, span } => {
                let values = elements
                    .iter()
                    .map(|element| self.lower_expr(element))
                    .collect::<Result<Vec<_>, _>>()?;
                self.emit_value(ControlFlowOp::Array(values), ty, *span)
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
            Expr::New { class, args, span } => {
                let args = args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<Result<Vec<_>, _>>()?;
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
            Expr::Member {
                object,
                property,
                span,
            } => self.lower_member(object, *property, ty, *span),
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
                    .map(|symbol| self.lower_symbol_value(*symbol, *span))
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
            Expr::Index {
                object,
                index,
                span,
            } => {
                let object = self.lower_expr(object)?;
                let index = self.lower_expr(index)?;
                self.emit_value(ControlFlowOp::IndexGet { object, index }, ty, *span)
            }
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

    fn lower_ident(&mut self, ident: Ident<'src>, _ty: Type<'src>) -> Result<ValueId, LowerError> {
        let symbol = self.symbol(ident)?;
        self.lower_symbol_value(symbol, ident.span).map_err(|_| {
            LowerError::new(
                ident.span,
                format!(
                    "capturing `{}` is not represented in this closure",
                    ident.name
                ),
            )
        })
    }

    fn lower_symbol_value(&mut self, symbol: SymbolId, span: Span) -> Result<ValueId, LowerError> {
        let ty = self
            .semantics
            .symbols()
            .get(symbol.0 as usize)
            .map(|symbol| symbol.ty.clone())
            .ok_or_else(|| LowerError::new(span, "missing symbol type"))?;
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

    fn lower_member<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        property: Ident<'src>,
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        let object_type = self.expression_type(object)?;
        let object_value = self.lower_expr(object)?;
        match object_type {
            Type::Struct(owner) => {
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
            Type::Class(owner) => {
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
            Type::String if property.name == "length" => self.emit_value(
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::StringLength,
                    receiver: Some(object_value),
                    args: Vec::new(),
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

    fn lower_call<'ast>(
        &mut self,
        callee: &Expr<'ast, 'src>,
        args: &[Expr<'ast, 'src>],
        ty: Type<'src>,
        span: Span,
    ) -> Result<ValueId, LowerError> {
        if let Expr::Ident(ident) = callee {
            if ident.name == "print" {
                let args = self.lower_args(args)?;
                return self.emit_value(
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::Print,
                        receiver: None,
                        args,
                    },
                    ty,
                    span,
                );
            }
            let symbol = self.symbol(*ident)?;
            if let Some(function) = self.function_symbols.get(&symbol).copied() {
                let args = self.lower_args(args)?;
                return self.emit_value(ControlFlowOp::CallDirect { function, args }, ty, span);
            }
        }

        if let Expr::Member {
            object, property, ..
        } = callee
        {
            let receiver_type = self.expression_type(object)?;
            let receiver = self.lower_expr(object)?;
            let args = self.lower_args(args)?;
            if let Some(intrinsic) = member_intrinsic(&receiver_type, property.name) {
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
            if let Type::Class(class) = receiver_type {
                let function = self
                    .method_functions
                    .get(&(class, property.name))
                    .copied()
                    .ok_or_else(|| LowerError::new(property.span, "missing method function"))?;
                return self.emit_value(
                    ControlFlowOp::CallMethod {
                        receiver,
                        class,
                        method: property.name,
                        function,
                        args,
                    },
                    ty,
                    span,
                );
            }
        }

        let callee = self.lower_expr(callee)?;
        let args = self.lower_args(args)?;
        self.emit_value(ControlFlowOp::CallValue { callee, args }, ty, span)
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
            local: LocalId(u32::MAX),
            ty,
            incoming: vec![(short_block, short), (rhs_end, rhs)],
            span,
        });
        Ok(out)
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
                let (owner, index) = match object_type {
                    Type::Struct(owner) => {
                        let index = self
                            .semantics
                            .struct_info(owner)
                            .and_then(|info| info.fields.get(property.name))
                            .map(|field| field.index)
                            .ok_or_else(|| LowerError::new(*span, "missing struct field"))?;
                        (owner, index)
                    }
                    Type::Class(owner) => {
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
        Ok(ControlFlowFunction {
            id: self.id,
            name: self.name,
            kind: self.kind,
            declared_pure: self.declared_pure,
            params: self.params,
            capture_count: self.capture_count,
            return_type: self.return_type,
            locals: self.locals,
            blocks: self.blocks,
            shapes: self.shapes,
            entry: BlockId(0),
            value_count: self.next_value,
            value_escapes: self.value_escapes,
            locals_promoted: false,
            live: true,
            span: self.span,
        })
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
    match function_type {
        Type::Function(signature) => Ok(*signature.return_type),
        _ => Err(LowerError::new(
            function.name.span,
            "function symbol does not have a callable type",
        )),
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
        BinaryOp::Eq => IrBinaryOp::Eq,
        BinaryOp::NotEq => IrBinaryOp::NotEq,
        BinaryOp::Less => IrBinaryOp::Less,
        BinaryOp::LessEq => IrBinaryOp::LessEq,
        BinaryOp::Greater => IrBinaryOp::Greater,
        BinaryOp::GreaterEq => IrBinaryOp::GreaterEq,
        BinaryOp::And => IrBinaryOp::And,
        BinaryOp::Or => IrBinaryOp::Or,
    }
}

fn lower_assignment_op(op: AssignmentOp) -> IrBinaryOp {
    match op {
        AssignmentOp::Assign => unreachable!(),
        AssignmentOp::Add => IrBinaryOp::Add,
        AssignmentOp::Sub => IrBinaryOp::Sub,
        AssignmentOp::Mul => IrBinaryOp::Mul,
        AssignmentOp::Div => IrBinaryOp::Div,
        AssignmentOp::Mod => IrBinaryOp::Mod,
    }
}

fn member_intrinsic(receiver: &Type<'_>, property: &str) -> Option<Intrinsic> {
    match (receiver, property) {
        (Type::Array(_), "map") => Some(Intrinsic::ArrayMap),
        (Type::Array(_), "filter") => Some(Intrinsic::ArrayFilter),
        (Type::Array(_), "reduce") => Some(Intrinsic::ArrayReduce),
        (Type::Array(_), "forEach") => Some(Intrinsic::ArrayForEach),
        (Type::Array(_), "push") => Some(Intrinsic::ArrayPush),
        (Type::Array(_), "pop") => Some(Intrinsic::ArrayPop),
        (Type::String, "includes") => Some(Intrinsic::StringIncludes),
        (Type::String, "startsWith") => Some(Intrinsic::StringStartsWith),
        (Type::String, "endsWith") => Some(Intrinsic::StringEndsWith),
        (Type::String, "toUpperCase") => Some(Intrinsic::StringToUpperCase),
        (Type::String, "toLowerCase") => Some(Intrinsic::StringToLowerCase),
        _ => None,
    }
}

type ArrowRef<'ast, 'src> = (&'ast [Param<'ast, 'src>], &'ast ArrowBody<'ast, 'src>, Span);

fn collect_arrow_captures<'ast, 'src>(
    body: &ArrowBody<'ast, 'src>,
    arrow_span: Span,
    semantics: &SemanticModel<'src>,
    globals: &AHashSet<SymbolId>,
    functions: &AHashMap<SymbolId, FunctionId>,
) -> Vec<SymbolId> {
    let mut used = AHashSet::new();
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
            Stmt::Expr(expression) => collect_expr_symbols(expression, semantics, out),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    collect_expr_symbols(value, semantics, out);
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
        Expr::ArrayLiteral { elements, .. } => {
            for element in *elements {
                collect_expr_symbols(element, semantics, out);
            }
        }
        Expr::StructLiteral { values, .. } | Expr::New { args: values, .. } => {
            for value in *values {
                collect_expr_symbols(value, semantics, out);
            }
        }
        Expr::Member { object, .. } | Expr::Unary { expr: object, .. } => {
            collect_expr_symbols(object, semantics, out)
        }
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
        Expr::Int(_, _) | Expr::Float(_, _) | Expr::String(_, _) | Expr::Bool(_, _) => {}
    }
}

fn collect_program_arrows<'ast, 'src>(
    program: &Program<'ast, 'src>,
    out: &mut Vec<ArrowRef<'ast, 'src>>,
) {
    for item in program.items {
        match item {
            Item::Struct(_) => {}
            Item::Class(class) => {
                for member in class.members {
                    match member {
                        ClassMember::Field(_) => {}
                        ClassMember::Constructor(constructor) => {
                            collect_stmt_arrows(constructor.body, out)
                        }
                        ClassMember::Method(method) => collect_stmt_arrows(method.body, out),
                    }
                }
            }
            Item::Function(function) => collect_stmt_arrows(function.body, out),
            Item::Extern(_) => {}
            Item::Stmt(statement) => collect_one_stmt_arrows(statement, out),
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
        Stmt::Expr(expression) => collect_expr_arrows(expression, out),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                collect_expr_arrows(value, out);
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
            match body {
                ArrowBody::Expr(expression) => collect_expr_arrows(expression, out),
                ArrowBody::Block(statements) => collect_stmt_arrows(statements, out),
            }
        }
        Expr::ArrayLiteral { elements, .. } => {
            for element in *elements {
                collect_expr_arrows(element, out);
            }
        }
        Expr::StructLiteral { values, .. } | Expr::New { args: values, .. } => {
            for value in *values {
                collect_expr_arrows(value, out);
            }
        }
        Expr::Member { object, .. } | Expr::Unary { expr: object, .. } => {
            collect_expr_arrows(object, out)
        }
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
        Expr::Index { object, index, .. } => {
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
        | Expr::Ident(_) => {}
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
    fn lowers_short_circuit_to_phi() {
        let module = lower("bool a=true;bool b=false;bool c=a&&b;");
        assert!(module.functions[0]
            .blocks
            .iter()
            .any(|block| !block.phis.is_empty()));
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
}
