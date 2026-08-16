use std::fmt::Write;

use crate::stable_hash::StableHashMap as AHashMap;
use indexmap::IndexMap;

use crate::ast::{
    ArrayBinding, ArrayElement, ArrowBody, AssignmentOp, BinaryOp, ClassDecl, ClassMember, Expr,
    ForInitializer, FunctionDecl, Ident, Item, MatchPattern, Program, RecordElement, Stmt,
    StructDecl, TemplatePart, TypeKind, TypeRef, UnaryOp, UpdateOp, VarDecl,
};
use crate::codegen_ir_js::emit_optimized_ir_js;
use crate::lower::{lower_to_control_flow, LowerError};
use crate::optimizer::{optimize_control_flow, SsaError};
use crate::semantic::{analyze, SemanticError, SemanticModel, Type};
use crate::span::Span;

fn decoded_source_string(value: &str) -> String {
    let encoded = format!("\"{value}\"");
    serde_json::from_str(&encoded).unwrap_or_else(|_| value.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodegenOptions {
    pub mangle: bool,
    pub dissolve_structs: bool,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        Self {
            mangle: true,
            dissolve_structs: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenError {
    pub span: Span,
    pub message: String,
}

impl CodegenError {
    pub(crate) fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub const fn span(&self) -> Span {
        self.span
    }
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for CodegenError {}

#[derive(Debug)]
pub enum CompileError {
    Semantic(SemanticError),
    Lower(LowerError),
    Optimize(SsaError),
    Codegen(CodegenError),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Semantic(error) => error.fmt(f),
            Self::Lower(error) => error.fmt(f),
            Self::Optimize(error) => error.fmt(f),
            Self::Codegen(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Semantic(error) => Some(error),
            Self::Lower(error) => Some(error),
            Self::Optimize(error) => Some(error),
            Self::Codegen(error) => Some(error),
        }
    }
}

impl CompileError {
    pub const fn span(&self) -> Span {
        match self {
            Self::Semantic(error) => error.span,
            Self::Lower(error) => error.span,
            Self::Optimize(error) => error.span,
            Self::Codegen(error) => error.span,
        }
    }
}

impl From<SemanticError> for CompileError {
    fn from(value: SemanticError) -> Self {
        Self::Semantic(value)
    }
}

impl From<LowerError> for CompileError {
    fn from(value: LowerError) -> Self {
        Self::Lower(value)
    }
}

impl From<SsaError> for CompileError {
    fn from(value: SsaError) -> Self {
        Self::Optimize(value)
    }
}

impl From<CodegenError> for CompileError {
    fn from(value: CodegenError) -> Self {
        Self::Codegen(value)
    }
}

pub fn compile_to_js<'ast, 'src>(program: &Program<'ast, 'src>) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    optimize_control_flow(&mut ir)?;
    emit_optimized_ir_js(&ir).map_err(Into::into)
}

pub struct JsEmitter<'src> {
    options: CodegenOptions,
    mangler: Mangler,
    scopes: Vec<Scope<'src>>,
    enums: AHashMap<&'src str, IndexMap<&'src str, i64>>,
    structs: AHashMap<&'src str, StructLayout<'src>>,
    classes: AHashMap<&'src str, ClassLayout<'src>>,
    expression_types: AHashMap<Span, Type<'src>>,
    binding_types: AHashMap<Span, Type<'src>>,
}

#[derive(Debug, Default)]
struct Scope<'src> {
    names: IndexMap<&'src str, String>,
    var_struct_types: IndexMap<&'src str, &'src str>,
}

#[derive(Debug)]
struct StructLayout<'src> {
    fields: IndexMap<&'src str, usize>,
}

#[derive(Debug)]
struct ClassLayout<'src> {
    base: Option<&'src str>,
    members: IndexMap<&'src str, String>,
}

impl<'src> JsEmitter<'src> {
    pub fn new(options: CodegenOptions) -> Self {
        Self {
            options,
            mangler: Mangler::default(),
            scopes: vec![Scope::default()],
            enums: AHashMap::default(),
            structs: AHashMap::default(),
            classes: AHashMap::default(),
            expression_types: AHashMap::default(),
            binding_types: AHashMap::default(),
        }
    }

    pub fn emit_program<'ast>(
        mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<String, CodegenError> {
        self.prepare_program(program)?;

        let mut out = String::new();
        for item in program.items {
            self.emit_item(item, &mut out)?;
        }
        Ok(out)
    }

    pub fn emit_checked_program<'ast>(
        mut self,
        program: &Program<'ast, 'src>,
        semantics: &SemanticModel<'src>,
    ) -> Result<String, CodegenError> {
        self.expression_types = semantics.expression_types().clone();
        self.binding_types = semantics.binding_types().clone();
        self.prepare_program(program)?;

        let mut out = String::new();
        for item in program.items {
            self.emit_item(item, &mut out)?;
        }
        Ok(out)
    }

    fn prepare_program<'ast>(&mut self, program: &Program<'ast, 'src>) -> Result<(), CodegenError> {
        self.collect_enums(program)?;
        self.collect_structs(program)?;
        self.collect_classes(program)?;

        for item in program.items {
            match item {
                Item::Class(decl) => {
                    self.declare_name(decl.name)?;
                }
                Item::Function(function) => {
                    self.declare_name(function.name)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn collect_enums<'ast>(&mut self, program: &Program<'ast, 'src>) -> Result<(), CodegenError> {
        for item in program.items {
            let Item::Enum(declaration) = item else {
                continue;
            };
            if self.enums.contains_key(declaration.name.name) {
                return Err(CodegenError::new(
                    declaration.name.span,
                    format!("duplicate enum `{}`", declaration.name.name),
                ));
            }
            let mut variants = IndexMap::new();
            for (value, variant) in declaration.variants.iter().enumerate() {
                if variants.insert(variant.name, value as i64).is_some() {
                    return Err(CodegenError::new(
                        variant.span,
                        format!("duplicate enum variant `{}`", variant.name),
                    ));
                }
            }
            self.enums.insert(declaration.name.name, variants);
        }
        Ok(())
    }

    fn collect_structs<'ast>(&mut self, program: &Program<'ast, 'src>) -> Result<(), CodegenError> {
        for item in program.items {
            if let Item::Struct(decl) = item {
                self.register_struct(decl)?;
            }
        }
        Ok(())
    }

    fn register_struct<'ast>(&mut self, decl: &StructDecl<'ast, 'src>) -> Result<(), CodegenError> {
        if self.structs.contains_key(decl.name.name) {
            return Err(CodegenError::new(
                decl.name.span,
                format!("duplicate struct `{}`", decl.name.name),
            ));
        }

        let mut fields = IndexMap::new();
        for (index, field) in decl.fields.iter().enumerate() {
            if fields.insert(field.name.name, index).is_some() {
                return Err(CodegenError::new(
                    field.name.span,
                    format!(
                        "duplicate field `{}` in struct `{}`",
                        field.name.name, decl.name.name
                    ),
                ));
            }
        }

        self.structs.insert(decl.name.name, StructLayout { fields });
        Ok(())
    }

    fn collect_classes<'ast>(&mut self, program: &Program<'ast, 'src>) -> Result<(), CodegenError> {
        let mut shared_members = IndexMap::new();
        let mut shared_mangler = Mangler::default();
        for item in program.items {
            let Item::Class(decl) = item else {
                continue;
            };
            for member in decl.members {
                let name = match member {
                    ClassMember::Field(field) => field.name.name,
                    ClassMember::Method(method) => method.name.name,
                    ClassMember::Constructor(_) => continue,
                };
                shared_members.entry(name).or_insert_with(|| {
                    if self.options.mangle {
                        shared_mangler.next_name()
                    } else {
                        name.to_string()
                    }
                });
            }
        }
        for item in program.items {
            let Item::Class(decl) = item else {
                continue;
            };
            if self.classes.contains_key(decl.name.name) {
                return Err(CodegenError::new(
                    decl.name.span,
                    format!("duplicate class `{}`", decl.name.name),
                ));
            }

            let mut members = IndexMap::new();
            for member in decl.members {
                let ident = match member {
                    ClassMember::Field(field) => field.name,
                    ClassMember::Method(method) => method.name,
                    ClassMember::Constructor(_) => continue,
                };
                if members.contains_key(ident.name) {
                    return Err(CodegenError::new(
                        ident.span,
                        format!(
                            "duplicate member `{}` in class `{}`",
                            ident.name, decl.name.name
                        ),
                    ));
                }
                let emitted = shared_members[ident.name].clone();
                members.insert(ident.name, emitted);
            }
            let base = decl.base.and_then(|base| match base.kind {
                TypeKind::Named { name, .. } => Some(name),
                _ => None,
            });
            self.classes
                .insert(decl.name.name, ClassLayout { base, members });
        }
        Ok(())
    }

    fn emit_item<'ast>(
        &mut self,
        item: &Item<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        match item {
            Item::Enum(_) | Item::Struct(_) => Ok(()),
            Item::Class(decl) => self.emit_class(decl, out),
            Item::Function(function) => self.emit_function(function, out),
            Item::Extern(_) | Item::ExternClass(_) | Item::ExternGlobal(_) => Ok(()),
            Item::Stmt(stmt) => self.emit_stmt(stmt, out),
        }
    }

    fn emit_function<'ast>(
        &mut self,
        function: &FunctionDecl<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let js_name = self
            .resolve_name(&function.name)
            .ok_or_else(|| CodegenError::new(function.name.span, "unregistered function"))?
            .to_string();
        if function.is_async {
            out.push_str("async ");
        }
        if function.is_generator {
            out.push_str("function* ");
            write!(out, "{}(", js_name).expect("writing to String cannot fail");
        } else {
            write!(out, "function {}(", js_name).expect("writing to String cannot fail");
        }

        self.push_scope();
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let param_name = self.declare_name(param.name)?;
            out.push_str(&param_name);
            if let Some(default) = &param.default {
                out.push('=');
                self.emit_expr(default, out)?;
            }
        }
        out.push(')');
        self.emit_stmt_list_as_block(function.body, out)?;
        self.pop_scope();
        Ok(())
    }

    fn emit_class<'ast>(
        &mut self,
        class: &ClassDecl<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let js_name = self
            .resolve_name(&class.name)
            .ok_or_else(|| CodegenError::new(class.name.span, "unregistered class"))?
            .to_string();
        write!(out, "class {js_name}").expect("writing to String cannot fail");
        if let Some(base) = class.base {
            let TypeKind::Named { name, .. } = base.kind else {
                return Err(CodegenError::new(base.span, "invalid base class type"));
            };
            let base = self
                .resolve_name(&Ident {
                    name,
                    span: base.span,
                })
                .ok_or_else(|| CodegenError::new(base.span, "unregistered base class"))?;
            write!(out, " extends {base}").expect("writing to String cannot fail");
        }
        out.push('{');

        for member in class.members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            let emitted = self
                .class_member_name(class.name.name, field.name)?
                .to_string();
            out.push_str(&emitted);
            out.push(';');
        }

        for member in class.members {
            let ClassMember::Constructor(constructor) = member else {
                continue;
            };
            out.push_str("constructor(");
            self.push_scope();
            for (index, param) in constructor.params.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                let name = self.declare_name(param.name)?;
                out.push_str(&name);
                if let Some(default) = &param.default {
                    out.push('=');
                    self.emit_expr(default, out)?;
                }
            }
            out.push(')');
            self.emit_stmt_list_as_block(constructor.body, out)?;
            self.pop_scope();
        }

        for member in class.members {
            let ClassMember::Method(method) = member else {
                continue;
            };
            let emitted = self
                .class_member_name(class.name.name, method.name)?
                .to_string();
            if method.is_async {
                out.push_str("async ");
            }
            if method.is_generator {
                out.push('*');
            }
            out.push_str(&emitted);
            out.push('(');
            self.push_scope();
            for (index, param) in method.params.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                let name = self.declare_name(param.name)?;
                out.push_str(&name);
                if let Some(default) = &param.default {
                    out.push('=');
                    self.emit_expr(default, out)?;
                }
            }
            out.push(')');
            self.emit_stmt_list_as_block(method.body, out)?;
            self.pop_scope();
        }
        out.push('}');
        Ok(())
    }

    fn emit_stmt<'ast>(
        &mut self,
        stmt: &Stmt<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        match stmt {
            Stmt::VarDecl(decl) => self.emit_var_decl(decl, out),
            Stmt::ArrayDestructure {
                bindings, value, ..
            } => {
                let mut nullable = Vec::new();
                out.push_str("let[");
                for (index, binding) in bindings.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    match binding {
                        ArrayBinding::Hole(_) => {}
                        ArrayBinding::Name(name) => {
                            let name = self.declare_name(*name)?;
                            out.push_str(&name);
                            nullable.push(name);
                        }
                        ArrayBinding::Rest(name) => {
                            out.push_str("...");
                            out.push_str(&self.declare_name(*name)?);
                        }
                    }
                }
                out.push_str("]=");
                self.emit_expr(value, out)?;
                out.push(';');
                for name in nullable {
                    out.push_str(&name);
                    out.push_str("??=null;");
                }
                Ok(())
            }
            Stmt::RecordDestructure {
                bindings,
                rest,
                value,
                ..
            } => {
                let mut nullable = Vec::new();
                out.push_str("let{");
                for (index, binding) in bindings.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    write_string_literal(binding.key.name, out);
                    out.push(':');
                    let name = self.declare_name(binding.name)?;
                    out.push_str(&name);
                    nullable.push(name);
                }
                let rest = rest.map(|rest| self.declare_name(rest)).transpose()?;
                if let Some(rest) = &rest {
                    if !bindings.is_empty() {
                        out.push(',');
                    }
                    out.push_str("...");
                    out.push_str(rest);
                }
                out.push_str("}=");
                self.emit_expr(value, out)?;
                out.push(';');
                for name in nullable {
                    out.push_str(&name);
                    out.push_str("??=null;");
                }
                if let Some(rest) = rest {
                    out.push_str("Object.setPrototypeOf(");
                    out.push_str(&rest);
                    out.push_str(",null);");
                }
                Ok(())
            }
            Stmt::Expr(expr) => {
                self.emit_expr(expr, out)?;
                out.push(';');
                Ok(())
            }
            Stmt::Return { value, .. } => {
                out.push_str("return");
                if let Some(value) = value {
                    out.push(' ');
                    self.emit_expr(value, out)?;
                }
                out.push(';');
                Ok(())
            }
            Stmt::Throw { value, .. } => {
                out.push_str("throw ");
                self.emit_expr(value, out)?;
                out.push(';');
                Ok(())
            }
            Stmt::SuperCall { args, .. } => {
                out.push_str("super(");
                for (index, argument) in args.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    self.emit_expr(argument, out)?;
                }
                out.push_str(");");
                Ok(())
            }
            Stmt::Yield {
                value, delegate, ..
            } => {
                out.push_str(if *delegate { "yield*" } else { "yield " });
                self.emit_expr(value, out)?;
                out.push(';');
                Ok(())
            }
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                out.push_str("try");
                self.push_scope();
                self.emit_stmt_list_as_block(body, out)?;
                self.pop_scope();
                if let Some(clause) = catch {
                    self.push_scope();
                    out.push_str("catch");
                    if let Some(binding) = clause.binding {
                        out.push('(');
                        out.push_str(&self.declare_name(binding.name)?);
                        out.push(')');
                    }
                    self.emit_stmt_list_as_block(clause.body, out)?;
                    self.pop_scope();
                }
                if let Some(body) = finally {
                    out.push_str("finally");
                    self.push_scope();
                    self.emit_stmt_list_as_block(body, out)?;
                    self.pop_scope();
                }
                Ok(())
            }
            Stmt::Block { body, .. } => {
                self.push_scope();
                self.emit_stmt_list_as_block(body, out)?;
                self.pop_scope();
                Ok(())
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                out.push_str("if(");
                self.emit_expr(condition, out)?;
                out.push(')');
                self.emit_control_body(then_branch, out)?;
                if let Some(else_branch) = else_branch {
                    out.push_str("else");
                    self.emit_control_body(else_branch, out)?;
                }
                Ok(())
            }
            Stmt::While {
                condition, body, ..
            } => {
                out.push_str("while(");
                self.emit_expr(condition, out)?;
                out.push(')');
                self.emit_control_body(body, out)
            }
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                self.push_scope();
                out.push_str("for(");
                if let Some(initializer) = initializer {
                    match initializer {
                        ForInitializer::VarDecl(decl) => self.emit_var_decl_core(decl, out)?,
                        ForInitializer::Expr(expr) => self.emit_expr(expr, out)?,
                    }
                }
                out.push(';');
                if let Some(condition) = condition {
                    self.emit_expr(condition, out)?;
                }
                out.push(';');
                if let Some(update) = update {
                    self.emit_expr(update, out)?;
                }
                out.push(')');
                self.emit_control_body(body, out)?;
                self.pop_scope();
                Ok(())
            }
            Stmt::ForIn {
                key, object, body, ..
            } => {
                self.push_scope();
                let key = self.declare_name(*key)?;
                out.push_str("for(let ");
                out.push_str(&key);
                out.push_str(" in ");
                self.emit_expr(object, out)?;
                out.push(')');
                self.emit_control_body(body, out)?;
                self.pop_scope();
                Ok(())
            }
            Stmt::ForOf {
                element,
                iterable,
                body,
                ..
            } => {
                self.push_scope();
                let element = self.declare_name(*element)?;
                out.push_str("for(let ");
                out.push_str(&element);
                out.push_str(" of ");
                self.emit_expr(iterable, out)?;
                out.push(')');
                self.emit_control_body(body, out)?;
                self.pop_scope();
                Ok(())
            }
            Stmt::Break(_) => {
                out.push_str("break;");
                Ok(())
            }
            Stmt::Continue(_) => {
                out.push_str("continue;");
                Ok(())
            }
        }
    }

    fn emit_control_body<'ast>(
        &mut self,
        statement: &Stmt<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        self.push_scope();
        match statement {
            Stmt::Block { body, .. } => self.emit_stmt_list_as_block(body, out)?,
            statement => {
                out.push('{');
                self.emit_stmt(statement, out)?;
                out.push('}');
            }
        }
        self.pop_scope();
        Ok(())
    }

    fn emit_stmt_list_as_block<'ast>(
        &mut self,
        body: &[Stmt<'ast, 'src>],
        out: &mut String,
    ) -> Result<(), CodegenError> {
        out.push('{');
        for stmt in body {
            self.emit_stmt(stmt, out)?;
        }
        out.push('}');
        Ok(())
    }

    fn emit_var_decl<'ast>(
        &mut self,
        decl: &VarDecl<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        self.emit_var_decl_core(decl, out)?;
        out.push(';');
        Ok(())
    }

    fn emit_var_decl_core<'ast>(
        &mut self,
        decl: &VarDecl<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let js_name = self.declare_name(decl.name)?;
        let semantic_struct = self
            .binding_types
            .get(&decl.name.span)
            .and_then(|ty| match ty {
                Type::Struct(name) | Type::StructInstance { name, .. } => Some(*name),
                _ => None,
            });
        let syntax_struct =
            named_struct_type(decl.ty).filter(|name| self.structs.contains_key(name));
        if let Some(struct_name) = semantic_struct.or(syntax_struct) {
            self.current_scope_mut()
                .var_struct_types
                .insert(decl.name.name, struct_name);
        }

        write!(out, "let {}", js_name).expect("writing to String cannot fail");
        if let Some(initializer) = &decl.initializer {
            out.push('=');
            self.emit_expr(initializer, out)?;
        }
        Ok(())
    }

    fn emit_expr<'ast>(
        &mut self,
        expr: &Expr<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        match expr {
            Expr::Int(value, _) => write!(out, "{value}").expect("writing to String cannot fail"),
            Expr::Float(value, _) => write_float(*value, out),
            Expr::String(value, _) => write_string_literal(value, out),
            Expr::Bool(value, _) => out.push_str(if *value { "true" } else { "false" }),
            Expr::Null(_) => out.push_str("null"),
            Expr::DynamicImport { source, .. } => {
                out.push_str("import(");
                write_string_literal(source, out);
                out.push(')');
            }
            Expr::Ident(ident) => {
                if ident.name == "this" {
                    out.push_str("this");
                } else {
                    out.push_str(self.resolve_name(ident).unwrap_or(ident.name));
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                out.push('[');
                for (index, element) in elements.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    match element {
                        ArrayElement::Value(value) => self.emit_expr(value, out)?,
                        ArrayElement::Spread { value, .. } => {
                            out.push_str("...");
                            self.emit_expr(value, out)?;
                        }
                    }
                }
                out.push(']');
            }
            Expr::RecordLiteral { entries, .. } => {
                out.push_str("{__proto__:null");
                for entry in *entries {
                    out.push(',');
                    match entry {
                        RecordElement::Spread { value, .. } => {
                            out.push_str("...");
                            self.emit_expr(value, out)?;
                        }
                        RecordElement::Entry(entry) => {
                            if decoded_source_string(entry.key.name) == "__proto__" {
                                out.push('[');
                                write_string_literal(entry.key.name, out);
                                out.push(']');
                            } else {
                                write_string_literal(entry.key.name, out);
                            }
                            out.push(':');
                            self.emit_expr(&entry.value, out)?;
                        }
                    }
                }
                out.push('}');
            }
            Expr::StructLiteral { name, values, span } => {
                if self.options.dissolve_structs {
                    self.emit_struct_literal(name, values, *span, out)?;
                } else {
                    self.emit_object_literal(name, values, *span, out)?;
                }
            }
            Expr::New { class, args, .. } => {
                let name = self.resolve_name(class).ok_or_else(|| {
                    CodegenError::new(class.span, format!("unknown class `{}`", class.name))
                })?;
                out.push_str("new ");
                out.push_str(name);
                out.push('(');
                for (index, arg) in args.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    self.emit_expr(arg, out)?;
                }
                out.push(')');
            }
            Expr::Member {
                object,
                property,
                span,
            } => {
                if matches!(object, Expr::Ident(Ident { name: "Task", .. })) {
                    out.push_str("Promise.");
                    out.push_str(property.name);
                } else {
                    self.emit_member_expr(object, *property, *span, out)?;
                }
            }
            Expr::OptionalMember {
                object, property, ..
            } => {
                out.push('(');
                self.emit_expr(object, out)?;
                out.push_str("?.");
                out.push_str(property.name);
                out.push_str("??null)");
            }
            Expr::Call { callee, args, .. } => {
                if matches!(callee, Expr::Ident(Ident { name: "print", .. })) {
                    out.push_str("console.log");
                } else {
                    self.emit_expr(callee, out)?;
                }
                out.push('(');
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    self.emit_expr(arg, out)?;
                }
                out.push(')');
            }
            Expr::ArrowFunction { params, body, .. } => {
                self.push_scope();
                out.push('(');
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    let name = self.declare_name(param.name)?;
                    out.push_str(&name);
                    if let Some(default) = &param.default {
                        out.push('=');
                        self.emit_expr(default, out)?;
                    }
                }
                out.push_str(")=>");
                match body {
                    ArrowBody::Expr(expr) => self.emit_expr(expr, out)?,
                    ArrowBody::Block(statements) => {
                        self.emit_stmt_list_as_block(statements, out)?
                    }
                }
                self.pop_scope();
            }
            Expr::Unary { op, expr, .. } => {
                out.push_str(match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                });
                self.emit_parenthesized_if_binary(expr, out)?;
            }
            Expr::Await { task, .. } => {
                out.push_str("await(");
                self.emit_expr(task, out)?;
                out.push(')');
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                self.emit_binary_expr(*op, lhs, rhs, out)?;
            }
            Expr::TypeCheck { value, target, .. } => match target.kind {
                TypeKind::Int | TypeKind::Float => {
                    out.push_str("typeof(");
                    self.emit_expr(value, out)?;
                    out.push_str(")==\"number\"");
                }
                TypeKind::String => {
                    out.push_str("typeof(");
                    self.emit_expr(value, out)?;
                    out.push_str(")==\"string\"");
                }
                TypeKind::Bool => {
                    out.push_str("typeof(");
                    self.emit_expr(value, out)?;
                    out.push_str(")==\"boolean\"");
                }
                TypeKind::Array(_) => {
                    out.push_str("Array.isArray(");
                    self.emit_expr(value, out)?;
                    out.push(')');
                }
                TypeKind::Function { .. } => {
                    out.push_str("typeof(");
                    self.emit_expr(value, out)?;
                    out.push_str(")==\"function\"");
                }
                _ => {
                    return Err(CodegenError::new(
                        target.span,
                        "type has no JavaScript type guard",
                    ));
                }
            },
            Expr::Index { object, index, .. } => {
                self.emit_expr(object, out)?;
                out.push('[');
                self.emit_expr(index, out)?;
                out.push(']');
            }
            Expr::OptionalIndex { object, index, .. } => {
                out.push('(');
                self.emit_expr(object, out)?;
                out.push_str("?.[");
                self.emit_expr(index, out)?;
                out.push_str("]??null)");
            }
            Expr::Match { value, arms, span } => {
                if arms.is_empty() {
                    return Err(CodegenError::new(*span, "match expression has no arms"));
                }
                out.push_str("(($lilmatch)=>");
                for arm in &arms[..arms.len() - 1] {
                    let MatchPattern::EnumVariant {
                        enum_name,
                        variant,
                        span,
                    } = arm.pattern
                    else {
                        return Err(CodegenError::new(
                            arm.pattern.span(),
                            "wildcard match arm must be last",
                        ));
                    };
                    let value = self
                        .enums
                        .get(enum_name.name)
                        .and_then(|variants| variants.get(variant.name))
                        .copied()
                        .ok_or_else(|| CodegenError::new(span, "unknown enum match pattern"))?;
                    write!(out, "$lilmatch==={value}?").expect("writing to String cannot fail");
                    self.emit_expr(&arm.value, out)?;
                    out.push(':');
                }
                self.emit_expr(&arms.last().expect("checked nonempty").value, out)?;
                out.push_str(")(");
                self.emit_expr(value, out)?;
                out.push(')');
            }
            Expr::Assignment {
                op, target, value, ..
            } => {
                self.emit_expr(target, out)?;
                out.push_str(assignment_op_js(*op));
                self.emit_expr(value, out)?;
            }
            Expr::Update {
                op, target, prefix, ..
            } => {
                let operator = match op {
                    UpdateOp::Increment => "++",
                    UpdateOp::Decrement => "--",
                };
                if *prefix {
                    out.push_str(operator);
                }
                self.emit_expr(target, out)?;
                if !prefix {
                    out.push_str(operator);
                }
            }
            Expr::Template { parts, .. } => {
                out.push('`');
                for part in *parts {
                    match part {
                        TemplatePart::String(value, _) => out.push_str(value),
                        TemplatePart::Expr(expression) => {
                            out.push_str("${");
                            self.emit_expr(expression, out)?;
                            out.push('}');
                        }
                    }
                }
                out.push('`');
            }
        }
        Ok(())
    }

    fn emit_struct_literal<'ast>(
        &mut self,
        name: &Ident<'src>,
        values: &[Expr<'ast, 'src>],
        span: Span,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let expected_fields = match self.structs.get(name.name) {
            Some(layout) => layout.fields.len(),
            None => {
                return Err(CodegenError::new(
                    name.span,
                    format!("unknown struct `{}`", name.name),
                ));
            }
        };

        if values.len() != expected_fields {
            return Err(CodegenError::new(
                span,
                format!(
                    "struct `{}` expects {} values, got {}",
                    name.name,
                    expected_fields,
                    values.len()
                ),
            ));
        }

        out.push('[');
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            self.emit_expr(value, out)?;
        }
        out.push(']');
        Ok(())
    }

    fn emit_object_literal<'ast>(
        &mut self,
        name: &Ident<'src>,
        values: &[Expr<'ast, 'src>],
        span: Span,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let field_names = match self.structs.get(name.name) {
            Some(layout) => layout.fields.keys().copied().collect::<Vec<_>>(),
            None => {
                return Err(CodegenError::new(
                    name.span,
                    format!("unknown struct `{}`", name.name),
                ));
            }
        };

        if values.len() != field_names.len() {
            return Err(CodegenError::new(
                span,
                format!(
                    "struct `{}` expects {} values, got {}",
                    name.name,
                    field_names.len(),
                    values.len()
                ),
            ));
        }

        out.push('{');
        for (index, field_name) in field_names.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(field_name);
            out.push(':');
            self.emit_expr(&values[index], out)?;
        }
        out.push('}');
        Ok(())
    }

    fn emit_member_expr<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        property: Ident<'src>,
        span: Span,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        if let Expr::Ident(enum_name) = object {
            if let Some(value) = self
                .enums
                .get(enum_name.name)
                .and_then(|variants| variants.get(property.name))
            {
                write!(out, "{value}").expect("writing to String cannot fail");
                return Ok(());
            }
        }
        if self.options.dissolve_structs {
            let semantic_struct =
                self.expression_types
                    .get(&object.span())
                    .and_then(|ty| match ty {
                        Type::Struct(name) | Type::StructInstance { name, .. } => Some(*name),
                        _ => None,
                    });
            let scoped_struct = if let Expr::Ident(object_ident) = object {
                self.resolve_var_struct_type(object_ident.name)
            } else {
                None
            };
            if let Some(struct_name) = semantic_struct.or(scoped_struct) {
                let index = match self.structs.get(struct_name) {
                    Some(layout) => match layout.fields.get(property.name) {
                        Some(index) => *index,
                        None => {
                            return Err(CodegenError::new(
                                property.span,
                                format!(
                                    "struct `{}` has no field `{}`",
                                    struct_name, property.name
                                ),
                            ));
                        }
                    },
                    None => {
                        return Err(CodegenError::new(
                            object.span(),
                            format!("unknown struct `{struct_name}`"),
                        ));
                    }
                };
                self.emit_expr(object, out)?;
                write!(out, "[{}]", index).expect("writing to String cannot fail");
                return Ok(());
            }
        }

        if let Some(
            Type::Class(class_name)
            | Type::ClassInstance {
                name: class_name, ..
            },
        ) = self.expression_types.get(&object.span())
        {
            let emitted = self.class_member_name(class_name, property)?.to_string();
            self.emit_expr(object, out)?;
            out.push('.');
            out.push_str(&emitted);
            return Ok(());
        }

        self.emit_expr(object, out)?;
        if is_safe_property_name(property.name) {
            out.push('.');
            out.push_str(property.name);
        } else {
            return Err(CodegenError::new(
                span,
                format!("unsupported property name `{}`", property.name),
            ));
        }
        Ok(())
    }

    fn class_member_name(
        &self,
        class_name: &str,
        member: Ident<'src>,
    ) -> Result<&str, CodegenError> {
        let mut current = Some(class_name);
        while let Some(name) = current {
            let Some(layout) = self.classes.get(name) else {
                break;
            };
            if let Some(emitted) = layout.members.get(member.name) {
                return Ok(emitted);
            }
            current = layout.base;
        }
        Err(CodegenError::new(
            member.span,
            format!("class `{class_name}` has no member `{}`", member.name),
        ))
    }

    fn emit_binary_expr<'ast>(
        &mut self,
        op: BinaryOp,
        lhs: &Expr<'ast, 'src>,
        rhs: &Expr<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        if op == BinaryOp::UnsignedShiftRight {
            out.push('(');
        }
        self.emit_expr_with_binary_parentheses(lhs, op, true, out)?;
        out.push_str(binary_op_js(op));
        self.emit_expr_with_binary_parentheses(rhs, op, false, out)?;
        if op == BinaryOp::UnsignedShiftRight {
            out.push_str("|0)");
        }
        Ok(())
    }

    fn emit_expr_with_binary_parentheses<'ast>(
        &mut self,
        expr: &Expr<'ast, 'src>,
        parent: BinaryOp,
        is_left: bool,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let needs_parens = match expr {
            Expr::Assignment { .. } => true,
            Expr::Binary { op, .. } => {
                let child = binary_precedence(*op);
                let parent = binary_precedence(parent);
                child < parent
                    || (!is_left
                        && child == parent
                        && matches!(
                            op,
                            BinaryOp::Sub
                                | BinaryOp::Div
                                | BinaryOp::Mod
                                | BinaryOp::ShiftLeft
                                | BinaryOp::ShiftRight
                                | BinaryOp::UnsignedShiftRight
                                | BinaryOp::Less
                                | BinaryOp::LessEq
                                | BinaryOp::Greater
                                | BinaryOp::GreaterEq
                        ))
            }
            _ => false,
        };

        if needs_parens {
            out.push('(');
        }
        self.emit_expr(expr, out)?;
        if needs_parens {
            out.push(')');
        }
        Ok(())
    }

    fn emit_parenthesized_if_binary<'ast>(
        &mut self,
        expr: &Expr<'ast, 'src>,
        out: &mut String,
    ) -> Result<(), CodegenError> {
        let needs_parens = matches!(expr, Expr::Binary { .. });
        if needs_parens {
            out.push('(');
        }
        self.emit_expr(expr, out)?;
        if needs_parens {
            out.push(')');
        }
        Ok(())
    }

    fn declare_name(&mut self, ident: Ident<'src>) -> Result<String, CodegenError> {
        if self.current_scope().names.contains_key(ident.name) {
            return Err(CodegenError::new(
                ident.span,
                format!("duplicate binding `{}`", ident.name),
            ));
        }

        let js_name = if self.options.mangle {
            self.mangler.next_name()
        } else {
            ident.name.to_string()
        };
        self.current_scope_mut()
            .names
            .insert(ident.name, js_name.clone());
        Ok(js_name)
    }

    fn resolve_name(&self, ident: &Ident<'src>) -> Option<&str> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.names.get(ident.name).map(String::as_str))
    }

    fn resolve_var_struct_type(&self, name: &'src str) -> Option<&'src str> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.var_struct_types.get(name).copied())
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope::default());
    }

    fn pop_scope(&mut self) {
        debug_assert!(self.scopes.len() > 1);
        self.scopes.pop();
    }

    fn current_scope(&self) -> &Scope<'src> {
        self.scopes
            .last()
            .expect("JS emitter always has at least one scope")
    }

    fn current_scope_mut(&mut self) -> &mut Scope<'src> {
        self.scopes
            .last_mut()
            .expect("JS emitter always has at least one scope")
    }
}

#[derive(Debug, Default)]
struct Mangler {
    next: usize,
}

impl Mangler {
    fn next_name(&mut self) -> String {
        let name = encode_identifier(self.next);
        self.next += 1;
        name
    }
}

fn encode_identifier(mut index: usize) -> String {
    const FIRST: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";
    const REST: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$0123456789";

    let mut out = String::new();
    out.push(FIRST[index % FIRST.len()] as char);
    index /= FIRST.len();

    while index > 0 {
        index -= 1;
        out.push(REST[index % REST.len()] as char);
        index /= REST.len();
    }

    out
}

fn named_struct_type<'ast, 'src>(ty: TypeRef<'ast, 'src>) -> Option<&'src str> {
    match ty.kind {
        TypeKind::Named { name, .. } => Some(name),
        _ => None,
    }
}

fn binary_op_js(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::Xor => "^",
        BinaryOp::ShiftLeft => "<<",
        BinaryOp::ShiftRight => ">>",
        BinaryOp::UnsignedShiftRight => ">>>",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEq => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::Nullish => "??",
    }
}

fn assignment_op_js(op: AssignmentOp) -> &'static str {
    match op {
        AssignmentOp::Assign => "=",
        AssignmentOp::Nullish => "??=",
        AssignmentOp::Add => "+=",
        AssignmentOp::Sub => "-=",
        AssignmentOp::Mul => "*=",
        AssignmentOp::Div => "/=",
        AssignmentOp::Mod => "%=",
        AssignmentOp::BitAnd => "&=",
        AssignmentOp::BitOr => "|=",
        AssignmentOp::Xor => "^=",
        AssignmentOp::ShiftLeft => "<<=",
        AssignmentOp::ShiftRight => ">>=",
        AssignmentOp::UnsignedShiftRight => ">>>=",
    }
}

fn binary_precedence(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Or | BinaryOp::Nullish => 1,
        BinaryOp::And => 2,
        BinaryOp::BitOr => 3,
        BinaryOp::Xor => 4,
        BinaryOp::BitAnd => 5,
        BinaryOp::Eq | BinaryOp::NotEq => 6,
        BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => 7,
        BinaryOp::ShiftLeft | BinaryOp::ShiftRight | BinaryOp::UnsignedShiftRight => 8,
        BinaryOp::Add | BinaryOp::Sub => 9,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 10,
    }
}

fn write_float(value: f64, out: &mut String) {
    if value.fract() == 0.0 {
        write!(out, "{}", value.trunc() as i64).expect("writing to String cannot fail");
    } else {
        write!(out, "{value}").expect("writing to String cannot fail");
    }
}

fn write_string_literal(value: &str, out: &mut String) {
    out.push('"');
    out.push_str(value);
    out.push('"');
}

fn is_safe_property_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::parser::parse_source;

    #[test]
    fn eliminates_unused_variable_decl() {
        let arena = Bump::new();
        let program = parse_source(&arena, "int x = 5;").unwrap();
        assert_eq!(compile_to_js(&program).unwrap(), "");
    }

    #[test]
    fn dissolves_struct_literal_to_array() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int x = 5; struct Point { int x; int y; } Point p = Point{10, 20};",
        )
        .unwrap();

        assert_eq!(compile_to_js(&program).unwrap(), "");
    }

    #[test]
    fn emits_array_method_and_typed_arrow_callback() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] numbers = [1, 2, 3]; auto doubled = numbers.map((int x) => x * 2);",
        )
        .unwrap();

        assert_eq!(compile_to_js(&program).unwrap(), "");
    }

    #[test]
    fn dissolves_internal_classes_after_devirtualization() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Vector{float x;float length(){return this.x;}} Vector v=new Vector(); float n=v.length();",
        )
        .unwrap();

        assert_eq!(compile_to_js(&program).unwrap(), "");
    }

    #[test]
    fn checked_codegen_rejects_invalid_lilscript() {
        let arena = Bump::new();
        let program = parse_source(&arena, "int value=\"not an int\";").unwrap();
        assert!(matches!(
            compile_to_js(&program),
            Err(CompileError::Semantic(_))
        ));
    }
}
