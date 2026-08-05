use std::fmt;

use ahash::AHashMap;
use indexmap::IndexMap;

use crate::ast::{
    ArrowBody, AssignmentOp, BinaryOp, ClassDecl, ClassMember, ConstructorDecl, Expr, ExternDecl,
    ForInitializer, FunctionDecl, Ident, Item, Program, Stmt, StructDecl, TemplatePart, TypeKind,
    TypeRef, UnaryOp, UpdateOp, VarDecl,
};
use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeState {
    LocalOnly,
    EscapesToTypedCode,
    EscapesToUntypedBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type<'src> {
    Int,
    Float,
    String,
    Bool,
    Void,
    Array(Box<Type<'src>>),
    Struct(&'src str),
    Class(&'src str),
    Function(FunctionType<'src>),
}

impl Type<'_> {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Int | Self::Float)
    }

    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }
}

impl fmt::Display for Type<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => f.write_str("int"),
            Self::Float => f.write_str("float"),
            Self::String => f.write_str("string"),
            Self::Bool => f.write_str("bool"),
            Self::Void => f.write_str("void"),
            Self::Array(element) => write!(f, "{element}[]"),
            Self::Struct(name) | Self::Class(name) => f.write_str(name),
            Self::Function(signature) => {
                f.write_str("function(")?;
                for (index, parameter) in signature.params.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{parameter}")?;
                }
                write!(f, ") -> {}", signature.return_type)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType<'src> {
    pub params: Vec<Type<'src>>,
    pub return_type: Box<Type<'src>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo<'src> {
    pub name: &'src str,
    pub ty: Type<'src>,
    pub index: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructInfo<'src> {
    pub name: &'src str,
    pub fields: IndexMap<&'src str, FieldInfo<'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassInfo<'src> {
    pub name: &'src str,
    pub fields: IndexMap<&'src str, FieldInfo<'src>>,
    pub methods: IndexMap<&'src str, FunctionType<'src>>,
    pub constructor: Option<FunctionType<'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol<'src> {
    pub id: SymbolId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub span: Span,
    pub escape_state: EscapeState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    pub span: Span,
    pub message: String,
}

impl SemanticError {
    fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for SemanticError {}

#[derive(Debug, Clone)]
pub struct SemanticModel<'src> {
    expression_types: AHashMap<Span, Type<'src>>,
    binding_types: AHashMap<Span, Type<'src>>,
    identifier_symbols: AHashMap<Span, SymbolId>,
    symbols: Vec<Symbol<'src>>,
    structs: AHashMap<&'src str, StructInfo<'src>>,
    classes: AHashMap<&'src str, ClassInfo<'src>>,
}

impl<'src> SemanticModel<'src> {
    pub fn expression_type(&self, span: Span) -> Option<&Type<'src>> {
        self.expression_types.get(&span)
    }

    pub fn binding_type(&self, span: Span) -> Option<&Type<'src>> {
        self.binding_types.get(&span)
    }

    pub fn identifier_symbol(&self, span: Span) -> Option<SymbolId> {
        self.identifier_symbols.get(&span).copied()
    }

    pub fn symbols(&self) -> &[Symbol<'src>] {
        &self.symbols
    }

    pub fn struct_info(&self, name: &str) -> Option<&StructInfo<'src>> {
        self.structs.get(name)
    }

    pub fn class_info(&self, name: &str) -> Option<&ClassInfo<'src>> {
        self.classes.get(name)
    }

    pub(crate) fn structs(&self) -> impl Iterator<Item = &StructInfo<'src>> {
        self.structs.values()
    }

    pub(crate) fn classes(&self) -> impl Iterator<Item = &ClassInfo<'src>> {
        self.classes.values()
    }

    pub(crate) fn expression_types(&self) -> &AHashMap<Span, Type<'src>> {
        &self.expression_types
    }

    pub(crate) fn binding_types(&self) -> &AHashMap<Span, Type<'src>> {
        &self.binding_types
    }
}

pub fn analyze<'ast, 'src>(
    program: &Program<'ast, 'src>,
) -> Result<SemanticModel<'src>, SemanticError> {
    Analyzer::new().analyze_program(program)
}

struct Analyzer<'src> {
    model: SemanticModel<'src>,
    scopes: Vec<AHashMap<&'src str, SymbolId>>,
    return_contexts: Vec<ReturnContext<'src>>,
    capture_barriers: Vec<usize>,
    loop_depth: usize,
}

#[derive(Debug)]
enum ReturnContext<'src> {
    Declared {
        ty: Type<'src>,
        saw_return: bool,
    },
    Inferred {
        ty: Option<Type<'src>>,
        saw_return: bool,
    },
}

impl<'src> Analyzer<'src> {
    fn new() -> Self {
        Self {
            model: SemanticModel {
                expression_types: AHashMap::new(),
                binding_types: AHashMap::new(),
                identifier_symbols: AHashMap::new(),
                symbols: Vec::new(),
                structs: AHashMap::new(),
                classes: AHashMap::new(),
            },
            scopes: vec![AHashMap::new()],
            return_contexts: Vec::new(),
            capture_barriers: Vec::new(),
            loop_depth: 0,
        }
    }

    fn analyze_program<'ast>(
        mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<SemanticModel<'src>, SemanticError> {
        if let Some(import) = program.imports.first() {
            return Err(SemanticError::new(
                import.span,
                "imports require file-based compilation so the module graph can be resolved",
            ));
        }
        self.declare_nominal_types(program)?;
        self.define_structs(program)?;
        self.define_classes(program)?;
        self.declare_functions(program)?;

        for item in program.items {
            match item {
                Item::Struct(_) => {}
                Item::Class(class) => self.analyze_class(class)?,
                Item::Function(function) => self.analyze_function(function, None)?,
                Item::Extern(_) => {}
                Item::Stmt(statement) => self.analyze_stmt(statement)?,
            }
        }

        Ok(self.model)
    }

    fn declare_nominal_types<'ast>(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), SemanticError> {
        for item in program.items {
            let (name, span, is_struct) = match item {
                Item::Struct(decl) => (decl.name.name, decl.span, true),
                Item::Class(decl) => (decl.name.name, decl.span, false),
                _ => continue,
            };

            if self.model.structs.contains_key(name) || self.model.classes.contains_key(name) {
                return Err(SemanticError::new(
                    span,
                    format!("duplicate type declaration `{name}`"),
                ));
            }

            if is_struct {
                self.model.structs.insert(
                    name,
                    StructInfo {
                        name,
                        fields: IndexMap::new(),
                        span,
                    },
                );
            } else {
                self.model.classes.insert(
                    name,
                    ClassInfo {
                        name,
                        fields: IndexMap::new(),
                        methods: IndexMap::new(),
                        constructor: None,
                        span,
                    },
                );
            }
        }
        Ok(())
    }

    fn define_structs<'ast>(&mut self, program: &Program<'ast, 'src>) -> Result<(), SemanticError> {
        for item in program.items {
            let Item::Struct(decl) = item else {
                continue;
            };
            let fields = self.resolve_fields(decl)?;
            self.model
                .structs
                .get_mut(decl.name.name)
                .expect("struct name was declared in the first semantic pass")
                .fields = fields;
        }
        Ok(())
    }

    fn define_classes<'ast>(&mut self, program: &Program<'ast, 'src>) -> Result<(), SemanticError> {
        for item in program.items {
            let Item::Class(decl) = item else {
                continue;
            };

            let mut fields = IndexMap::new();
            let mut methods = IndexMap::new();
            let mut constructor = None;
            for member in decl.members {
                match member {
                    ClassMember::Field(field) => {
                        if fields.contains_key(field.name.name)
                            || methods.contains_key(field.name.name)
                        {
                            return Err(SemanticError::new(
                                field.name.span,
                                format!(
                                    "duplicate member `{}` in class `{}`",
                                    field.name.name, decl.name.name
                                ),
                            ));
                        }
                        let ty = self.resolve_value_type(field.ty, "class field")?;
                        let index = fields.len();
                        fields.insert(
                            field.name.name,
                            FieldInfo {
                                name: field.name.name,
                                ty,
                                index,
                                span: field.span,
                            },
                        );
                    }
                    ClassMember::Method(method) => {
                        if fields.contains_key(method.name.name)
                            || methods.contains_key(method.name.name)
                        {
                            return Err(SemanticError::new(
                                method.name.span,
                                format!(
                                    "duplicate member `{}` in class `{}`",
                                    method.name.name, decl.name.name
                                ),
                            ));
                        }
                        methods.insert(method.name.name, self.function_type(method)?);
                    }
                    ClassMember::Constructor(constructor_decl) => {
                        if constructor.is_some() {
                            return Err(SemanticError::new(
                                constructor_decl.span,
                                format!("class `{}` has more than one constructor", decl.name.name),
                            ));
                        }
                        let mut params = Vec::with_capacity(constructor_decl.params.len());
                        for param in constructor_decl.params {
                            params.push(self.resolve_value_type(param.ty, "parameter")?);
                        }
                        constructor = Some(FunctionType {
                            params,
                            return_type: Box::new(Type::Class(decl.name.name)),
                        });
                    }
                }
            }

            let info = self
                .model
                .classes
                .get_mut(decl.name.name)
                .expect("class name was declared in the first semantic pass");
            info.fields = fields;
            info.methods = methods;
            info.constructor = constructor.clone();

            let constructor = Type::Function(constructor.unwrap_or(FunctionType {
                params: Vec::new(),
                return_type: Box::new(Type::Class(decl.name.name)),
            }));
            self.declare(decl.name, constructor)?;
        }
        Ok(())
    }

    fn resolve_fields<'ast>(
        &self,
        decl: &StructDecl<'ast, 'src>,
    ) -> Result<IndexMap<&'src str, FieldInfo<'src>>, SemanticError> {
        let mut fields = IndexMap::new();
        for field in decl.fields {
            if fields.contains_key(field.name.name) {
                return Err(SemanticError::new(
                    field.name.span,
                    format!(
                        "duplicate field `{}` in struct `{}`",
                        field.name.name, decl.name.name
                    ),
                ));
            }
            let ty = self.resolve_value_type(field.ty, "struct field")?;
            let index = fields.len();
            fields.insert(
                field.name.name,
                FieldInfo {
                    name: field.name.name,
                    ty,
                    index,
                    span: field.span,
                },
            );
        }
        Ok(fields)
    }

    fn declare_functions<'ast>(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), SemanticError> {
        for item in program.items {
            match item {
                Item::Function(function) => {
                    let ty = Type::Function(self.function_type(function)?);
                    self.declare(function.name, ty)?;
                }
                Item::Extern(extern_decl) => {
                    let signature = self.extern_type(extern_decl)?;
                    let ty = Type::Function(signature.clone());
                    self.declare(extern_decl.name, ty)?;
                    let mut names = AHashMap::new();
                    for (param, ty) in extern_decl.params.iter().zip(signature.params) {
                        if names.insert(param.name.name, param.name.span).is_some() {
                            return Err(SemanticError::new(
                                param.name.span,
                                format!("duplicate extern parameter `{}`", param.name.name),
                            ));
                        }
                        self.record_detached(param.name, ty);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn function_type<'ast>(
        &self,
        function: &FunctionDecl<'ast, 'src>,
    ) -> Result<FunctionType<'src>, SemanticError> {
        let mut params = Vec::with_capacity(function.params.len());
        for param in function.params {
            params.push(self.resolve_value_type(param.ty, "parameter")?);
        }
        let return_type = self.resolve_type(function.return_type, true, "return type")?;
        Ok(FunctionType {
            params,
            return_type: Box::new(return_type),
        })
    }

    fn extern_type<'ast>(
        &self,
        extern_decl: &ExternDecl<'ast, 'src>,
    ) -> Result<FunctionType<'src>, SemanticError> {
        let mut params = Vec::with_capacity(extern_decl.params.len());
        for param in extern_decl.params {
            params.push(self.resolve_value_type(param.ty, "extern parameter")?);
        }
        let return_type = self.resolve_type(extern_decl.return_type, true, "extern return type")?;
        Ok(FunctionType {
            params,
            return_type: Box::new(return_type),
        })
    }

    fn analyze_class<'ast>(&mut self, class: &ClassDecl<'ast, 'src>) -> Result<(), SemanticError> {
        for member in class.members {
            match member {
                ClassMember::Method(method) => {
                    self.analyze_function(method, Some(class.name.name))?
                }
                ClassMember::Constructor(constructor) => {
                    self.analyze_constructor(constructor, class.name.name)?
                }
                ClassMember::Field(_) => {}
            }
        }
        Ok(())
    }

    fn analyze_constructor<'ast>(
        &mut self,
        constructor: &ConstructorDecl<'ast, 'src>,
        class_name: &'src str,
    ) -> Result<(), SemanticError> {
        self.push_scope();
        self.declare(
            Ident {
                name: "this",
                span: constructor.span,
            },
            Type::Class(class_name),
        )?;
        for param in constructor.params {
            let ty = self.resolve_value_type(param.ty, "parameter")?;
            self.declare(param.name, ty)?;
        }
        self.return_contexts.push(ReturnContext::Declared {
            ty: Type::Void,
            saw_return: false,
        });
        for statement in constructor.body {
            self.analyze_stmt(statement)?;
        }
        self.return_contexts.pop();
        self.pop_scope();
        Ok(())
    }

    fn analyze_function<'ast>(
        &mut self,
        function: &FunctionDecl<'ast, 'src>,
        class_name: Option<&'src str>,
    ) -> Result<(), SemanticError> {
        let signature = self.function_type(function)?;
        self.push_scope();

        if let Some(class_name) = class_name {
            self.declare(
                Ident {
                    name: "this",
                    span: function.name.span,
                },
                Type::Class(class_name),
            )?;
        }

        for (param, ty) in function.params.iter().zip(&signature.params) {
            self.declare(param.name, ty.clone())?;
        }

        self.return_contexts.push(ReturnContext::Declared {
            ty: (*signature.return_type).clone(),
            saw_return: false,
        });
        for statement in function.body {
            self.analyze_stmt(statement)?;
        }
        let context = self
            .return_contexts
            .pop()
            .expect("function analysis pushed a return context");
        self.pop_scope();

        if let ReturnContext::Declared { ty, .. } = context {
            if !ty.is_void() && !statements_guarantee_return(function.body) {
                return Err(SemanticError::new(
                    function.name.span,
                    format!(
                        "function `{}` must return a value of type `{ty}`",
                        function.name.name
                    ),
                ));
            }
        }
        Ok(())
    }

    fn analyze_stmt<'ast>(&mut self, statement: &Stmt<'ast, 'src>) -> Result<(), SemanticError> {
        match statement {
            Stmt::VarDecl(decl) => self.analyze_var_decl(decl),
            Stmt::Expr(expr) => {
                self.analyze_expr(expr, None)?;
                Ok(())
            }
            Stmt::Return { value, span } => self.analyze_return(value.as_ref(), *span),
            Stmt::Block { body, .. } => {
                self.push_scope();
                for statement in *body {
                    self.analyze_stmt(statement)?;
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_type = self.analyze_expr(condition, Some(&Type::Bool))?;
                self.require_assignable(&Type::Bool, &condition_type, condition.span())?;
                self.push_scope();
                self.analyze_stmt(then_branch)?;
                self.pop_scope();
                if let Some(else_branch) = else_branch {
                    self.push_scope();
                    self.analyze_stmt(else_branch)?;
                    self.pop_scope();
                }
                Ok(())
            }
            Stmt::While {
                condition, body, ..
            } => {
                let condition_type = self.analyze_expr(condition, Some(&Type::Bool))?;
                self.require_assignable(&Type::Bool, &condition_type, condition.span())?;
                self.loop_depth += 1;
                self.push_scope();
                self.analyze_stmt(body)?;
                self.pop_scope();
                self.loop_depth -= 1;
                Ok(())
            }
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                self.push_scope();
                if let Some(initializer) = initializer {
                    match initializer {
                        ForInitializer::VarDecl(decl) => self.analyze_var_decl(decl)?,
                        ForInitializer::Expr(expr) => {
                            self.analyze_expr(expr, None)?;
                        }
                    }
                }
                if let Some(condition) = condition {
                    let condition_type = self.analyze_expr(condition, Some(&Type::Bool))?;
                    self.require_assignable(&Type::Bool, &condition_type, condition.span())?;
                }
                if let Some(update) = update {
                    self.analyze_expr(update, None)?;
                }
                self.loop_depth += 1;
                self.analyze_stmt(body)?;
                self.loop_depth -= 1;
                self.pop_scope();
                Ok(())
            }
            Stmt::Break(span) | Stmt::Continue(span) => {
                if self.loop_depth == 0 {
                    Err(SemanticError::new(
                        *span,
                        "loop control statement outside a loop",
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    fn analyze_var_decl<'ast>(&mut self, decl: &VarDecl<'ast, 'src>) -> Result<(), SemanticError> {
        if decl.initializer.is_none() {
            return Err(SemanticError::new(
                decl.span,
                "variable declarations require an initializer",
            ));
        }
        let ty = if decl.ty.is_auto() {
            let initializer = decl.initializer.as_ref().ok_or_else(|| {
                SemanticError::new(decl.span, "`auto` declarations require an initializer")
            })?;
            let inferred = self.analyze_expr(initializer, None)?;
            if inferred.is_void() {
                return Err(SemanticError::new(
                    initializer.span(),
                    "cannot infer a variable type from a void expression",
                ));
            }
            inferred
        } else {
            let declared = self.resolve_value_type(decl.ty, "variable")?;
            if let Some(initializer) = &decl.initializer {
                let actual = self.analyze_expr(initializer, Some(&declared))?;
                self.require_assignable(&declared, &actual, initializer.span())?;
            }
            declared
        };

        self.declare(decl.name, ty)?;
        Ok(())
    }

    fn analyze_return<'ast>(
        &mut self,
        value: Option<&Expr<'ast, 'src>>,
        span: Span,
    ) -> Result<(), SemanticError> {
        let expected = match self.return_contexts.last() {
            Some(ReturnContext::Declared { ty, .. }) => Some(ty.clone()),
            Some(ReturnContext::Inferred { ty, .. }) => ty.clone(),
            None => return Err(SemanticError::new(span, "`return` outside a function")),
        };

        let actual = match value {
            Some(value) => self.analyze_expr(value, expected.as_ref())?,
            None => Type::Void,
        };

        let context = self
            .return_contexts
            .last_mut()
            .expect("return context was checked above");
        match context {
            ReturnContext::Declared { ty, saw_return } => {
                if !is_assignable(ty, &actual) {
                    return Err(SemanticError::new(
                        span,
                        format!("expected return type `{ty}`, found `{actual}`"),
                    ));
                }
                *saw_return = true;
            }
            ReturnContext::Inferred { ty, saw_return } => {
                if let Some(previous) = ty {
                    let Some(common) = common_type(previous, &actual) else {
                        return Err(SemanticError::new(
                            span,
                            format!(
                                "incompatible inferred return types `{previous}` and `{actual}`"
                            ),
                        ));
                    };
                    *previous = common;
                } else {
                    *ty = Some(actual);
                }
                *saw_return = true;
            }
        }
        Ok(())
    }

    fn analyze_expr<'ast>(
        &mut self,
        expr: &Expr<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, SemanticError> {
        let ty = match expr {
            Expr::Int(value, span) => {
                if i32::try_from(*value).is_err() {
                    return Err(SemanticError::new(
                        *span,
                        "integer literal is outside the signed 32-bit range",
                    ));
                }
                Type::Int
            }
            Expr::Float(_, _) => Type::Float,
            Expr::String(_, _) => Type::String,
            Expr::Bool(_, _) => Type::Bool,
            Expr::Ident(ident) => {
                let (id, ty) = {
                    let symbol = self.resolve(ident)?;
                    (symbol.id, symbol.ty.clone())
                };
                self.model.identifier_symbols.insert(ident.span, id);
                ty
            }
            Expr::ArrayLiteral { elements, span } => {
                let expected_element = match expected {
                    Some(Type::Array(element)) => Some(element.as_ref()),
                    _ => None,
                };
                let mut element_type = expected_element.cloned();
                for element in *elements {
                    let actual = self.analyze_expr(element, expected_element)?;
                    element_type = match element_type {
                        Some(ref previous) => {
                            Some(common_type(previous, &actual).ok_or_else(|| {
                                SemanticError::new(
                                    element.span(),
                                    format!(
                                        "array element has type `{actual}`, expected `{previous}`"
                                    ),
                                )
                            })?)
                        }
                        None => Some(actual),
                    };
                }
                let element_type = element_type.ok_or_else(|| {
                    SemanticError::new(
                        *span,
                        "cannot infer the element type of an empty array; add an explicit array type",
                    )
                })?;
                Type::Array(Box::new(element_type))
            }
            Expr::StructLiteral { name, values, span } => {
                let info = self.model.structs.get(name.name).cloned().ok_or_else(|| {
                    SemanticError::new(name.span, format!("unknown struct `{}`", name.name))
                })?;
                if values.len() != info.fields.len() {
                    return Err(SemanticError::new(
                        *span,
                        format!(
                            "struct `{}` expects {} values, found {}",
                            name.name,
                            info.fields.len(),
                            values.len()
                        ),
                    ));
                }
                for (value, field) in values.iter().zip(info.fields.values()) {
                    let actual = self.analyze_expr(value, Some(&field.ty))?;
                    self.require_assignable(&field.ty, &actual, value.span())?;
                }
                Type::Struct(name.name)
            }
            Expr::New { class, args, span } => {
                let info = self.model.classes.get(class.name).cloned().ok_or_else(|| {
                    SemanticError::new(class.span, format!("unknown class `{}`", class.name))
                })?;
                let params = info
                    .constructor
                    .as_ref()
                    .map_or(&[][..], |signature| signature.params.as_slice());
                if args.len() != params.len() {
                    return Err(SemanticError::new(
                        *span,
                        format!(
                            "class `{}` constructor expects {} arguments, found {}",
                            class.name,
                            params.len(),
                            args.len()
                        ),
                    ));
                }
                for (arg, expected) in args.iter().zip(params) {
                    let actual = self.analyze_expr(arg, Some(expected))?;
                    self.require_assignable(expected, &actual, arg.span())?;
                }
                Type::Class(class.name)
            }
            Expr::Member {
                object,
                property,
                span,
            } => self.analyze_member(object, *property, *span)?,
            Expr::Call { callee, args, span } => {
                if matches!(callee, Expr::Ident(Ident { name: "print", .. })) {
                    if args.len() != 1 {
                        return Err(SemanticError::new(
                            *span,
                            format!("`print` expects one argument, found {}", args.len()),
                        ));
                    }
                    self.analyze_expr(&args[0], None)?;
                    Type::Void
                } else if let Expr::Member {
                    object, property, ..
                } = callee
                {
                    match property.name {
                        "map" => self.analyze_array_map(object, args, *span)?,
                        "filter" => self.analyze_array_filter(object, args, *span)?,
                        "forEach" => self.analyze_array_for_each(object, args, *span)?,
                        "reduce" => self.analyze_array_reduce(object, args, *span)?,
                        _ => {
                            let callee_type = self.analyze_expr(callee, None)?;
                            self.analyze_call(&callee_type, args, *span)?
                        }
                    }
                } else {
                    let callee_type = self.analyze_expr(callee, None)?;
                    self.analyze_call(&callee_type, args, *span)?
                }
            }
            Expr::ArrowFunction { params, body, .. } => {
                self.analyze_arrow(params, body, expected)?
            }
            Expr::Unary { op, expr, span } => {
                let operand = self.analyze_expr(expr, None)?;
                match op {
                    UnaryOp::Neg if operand.is_numeric() => operand,
                    UnaryOp::Not if operand == Type::Bool => Type::Bool,
                    UnaryOp::Neg => {
                        return Err(SemanticError::new(
                            *span,
                            format!("unary `-` requires a numeric operand, found `{operand}`"),
                        ));
                    }
                    UnaryOp::Not => {
                        return Err(SemanticError::new(
                            *span,
                            format!("unary `!` requires a bool operand, found `{operand}`"),
                        ));
                    }
                }
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lhs_type = self.analyze_expr(lhs, None)?;
                let rhs_type = self.analyze_expr(rhs, None)?;
                self.analyze_binary(*op, &lhs_type, &rhs_type, *span)?
            }
            Expr::Index {
                object,
                index,
                span,
            } => {
                let object_type = self.analyze_expr(object, None)?;
                let index_type = self.analyze_expr(index, Some(&Type::Int))?;
                self.require_assignable(&Type::Int, &index_type, index.span())?;
                match object_type {
                    Type::Array(element) => *element,
                    Type::String => Type::String,
                    other => {
                        return Err(SemanticError::new(
                            *span,
                            format!("cannot index a value of type `{other}`"),
                        ));
                    }
                }
            }
            Expr::Assignment {
                op,
                target,
                value,
                span,
            } => {
                let target_type = self.analyze_lvalue(target)?;
                let value_type = self.analyze_expr(value, Some(&target_type))?;
                if *op == AssignmentOp::Assign {
                    self.require_assignable(&target_type, &value_type, value.span())?;
                } else {
                    let binary_op = assignment_binary_op(*op);
                    let result =
                        self.analyze_binary(binary_op, &target_type, &value_type, *span)?;
                    self.require_assignable(&target_type, &result, *span)?;
                }
                target_type
            }
            Expr::Update {
                target, op, span, ..
            } => {
                let target_type = self.analyze_lvalue(target)?;
                if !target_type.is_numeric() {
                    return Err(SemanticError::new(
                        *span,
                        format!(
                            "operator `{}` requires a numeric target, found `{target_type}`",
                            match op {
                                UpdateOp::Increment => "++",
                                UpdateOp::Decrement => "--",
                            }
                        ),
                    ));
                }
                target_type
            }
            Expr::Template { parts, .. } => {
                for part in *parts {
                    if let TemplatePart::Expr(expression) = part {
                        let ty = self.analyze_expr(expression, None)?;
                        if !is_stringable(&ty) {
                            return Err(SemanticError::new(
                                expression.span(),
                                format!("type `{ty}` cannot be interpolated into a string"),
                            ));
                        }
                    }
                }
                Type::String
            }
        };

        self.model.expression_types.insert(expr.span(), ty.clone());
        Ok(ty)
    }

    fn analyze_lvalue<'ast>(
        &mut self,
        expression: &Expr<'ast, 'src>,
    ) -> Result<Type<'src>, SemanticError> {
        let ty = match expression {
            Expr::Ident(ident) => {
                let (scope, id, ty) = {
                    let (scope, symbol) = self.resolve_with_scope(ident)?;
                    (scope, symbol.id, symbol.ty.clone())
                };
                if self
                    .capture_barriers
                    .last()
                    .is_some_and(|barrier| scope != 0 && scope < *barrier)
                {
                    return Err(SemanticError::new(
                        ident.span,
                        format!("captured binding `{}` is read-only", ident.name),
                    ));
                }
                self.model.identifier_symbols.insert(ident.span, id);
                ty
            }
            Expr::Member {
                object,
                property,
                span,
            } => self.analyze_member(object, *property, *span)?,
            Expr::Index {
                object,
                index,
                span,
            } => {
                let object_type = self.analyze_expr(object, None)?;
                let index_type = self.analyze_expr(index, Some(&Type::Int))?;
                self.require_assignable(&Type::Int, &index_type, index.span())?;
                match object_type {
                    Type::Array(element) => *element,
                    other => {
                        return Err(SemanticError::new(
                            *span,
                            format!("cannot assign through an index on `{other}`"),
                        ));
                    }
                }
            }
            _ => {
                return Err(SemanticError::new(
                    expression.span(),
                    "expression is not an assignable location",
                ));
            }
        };
        self.model
            .expression_types
            .insert(expression.span(), ty.clone());
        Ok(ty)
    }

    fn analyze_member<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        property: Ident<'src>,
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        let object_type = self.analyze_expr(object, None)?;
        match object_type {
            Type::Struct(name) => self
                .model
                .structs
                .get(name)
                .and_then(|info| info.fields.get(property.name))
                .map(|field| field.ty.clone())
                .ok_or_else(|| {
                    SemanticError::new(
                        property.span,
                        format!("struct `{name}` has no field `{}`", property.name),
                    )
                }),
            Type::Class(name) => {
                let class = self
                    .model
                    .classes
                    .get(name)
                    .expect("class types always have class metadata");
                if let Some(field) = class.fields.get(property.name) {
                    return Ok(field.ty.clone());
                }
                if let Some(method) = class.methods.get(property.name) {
                    return Ok(Type::Function(method.clone()));
                }
                Err(SemanticError::new(
                    property.span,
                    format!("class `{name}` has no member `{}`", property.name),
                ))
            }
            Type::Array(_) | Type::String if property.name == "length" => Ok(Type::Int),
            Type::Array(element) => match property.name {
                "map" | "filter" | "forEach" | "reduce" => Err(SemanticError::new(
                    span,
                    format!("array `{}` must be called", property.name),
                )),
                "push" => Ok(Type::Function(FunctionType {
                    params: vec![*element],
                    return_type: Box::new(Type::Int),
                })),
                "pop" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    return_type: element,
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("array has no member `{}`", property.name),
                )),
            },
            Type::String => match property.name {
                "includes" | "startsWith" | "endsWith" => Ok(Type::Function(FunctionType {
                    params: vec![Type::String],
                    return_type: Box::new(Type::Bool),
                })),
                "toUpperCase" | "toLowerCase" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    return_type: Box::new(Type::String),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("string has no member `{}`", property.name),
                )),
            },
            other => Err(SemanticError::new(
                span,
                format!("type `{other}` has no member `{}`", property.name),
            )),
        }
    }

    fn analyze_array_map<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        args: &[Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        let object_type = self.analyze_expr(object, None)?;
        let Type::Array(element_type) = object_type else {
            return Err(SemanticError::new(
                span,
                format!("`map` requires an array receiver, found `{object_type}`"),
            ));
        };
        if args.len() != 1 {
            return Err(SemanticError::new(
                span,
                format!(
                    "array `map` expects one callback, found {} arguments",
                    args.len()
                ),
            ));
        }

        let callback_expected = Type::Function(FunctionType {
            params: vec![(*element_type).clone()],
            return_type: Box::new(Type::Void),
        });
        let callback = self.analyze_expr(&args[0], Some(&callback_expected))?;
        let Type::Function(signature) = callback else {
            return Err(SemanticError::new(
                args[0].span(),
                format!("array `map` expects a function, found `{callback}`"),
            ));
        };
        if signature.params.len() != 1
            || !is_assignable(&signature.params[0], &element_type)
            || !is_assignable(&element_type, &signature.params[0])
        {
            return Err(SemanticError::new(
                args[0].span(),
                format!(
                    "array `map` callback must accept `{}`, found `{}`",
                    element_type,
                    signature
                        .params
                        .first()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "no parameter".to_string())
                ),
            ));
        }
        if signature.return_type.is_void() {
            return Err(SemanticError::new(
                args[0].span(),
                "array `map` callback must return a value",
            ));
        }
        Ok(Type::Array(signature.return_type))
    }

    fn analyze_array_filter<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        args: &[Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        let element_type = self.array_element_type(object, "filter", span)?;
        let signature = self.analyze_array_callback(args, &element_type, "filter", span)?;
        if signature.return_type.as_ref() != &Type::Bool {
            return Err(SemanticError::new(
                args[0].span(),
                format!(
                    "array `filter` callback must return `bool`, found `{}`",
                    signature.return_type
                ),
            ));
        }
        Ok(Type::Array(Box::new(element_type)))
    }

    fn analyze_array_for_each<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        args: &[Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        let element_type = self.array_element_type(object, "forEach", span)?;
        self.analyze_array_callback(args, &element_type, "forEach", span)?;
        Ok(Type::Void)
    }

    fn analyze_array_reduce<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        args: &[Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        let element_type = self.array_element_type(object, "reduce", span)?;
        if args.len() != 2 {
            return Err(SemanticError::new(
                span,
                format!(
                    "array `reduce` expects a callback and initial value, found {} arguments",
                    args.len()
                ),
            ));
        }
        let accumulator = self.analyze_expr(&args[1], None)?;
        let expected = Type::Function(FunctionType {
            params: vec![accumulator.clone(), element_type],
            return_type: Box::new(accumulator.clone()),
        });
        let callback = self.analyze_expr(&args[0], Some(&expected))?;
        let Type::Function(signature) = callback else {
            return Err(SemanticError::new(
                args[0].span(),
                "array `reduce` expects a function callback",
            ));
        };
        if signature.params.len() != 2
            || !signature
                .params
                .iter()
                .zip(match &expected {
                    Type::Function(expected) => &expected.params,
                    _ => unreachable!(),
                })
                .all(|(actual, expected)| actual == expected)
            || !is_assignable(&accumulator, &signature.return_type)
        {
            return Err(SemanticError::new(
                args[0].span(),
                "array `reduce` callback signature does not match its accumulator and element types",
            ));
        }
        Ok(accumulator)
    }

    fn array_element_type<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        method: &str,
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        let object_type = self.analyze_expr(object, None)?;
        match object_type {
            Type::Array(element) => Ok(*element),
            other => Err(SemanticError::new(
                span,
                format!("`{method}` requires an array receiver, found `{other}`"),
            )),
        }
    }

    fn analyze_array_callback<'ast>(
        &mut self,
        args: &[Expr<'ast, 'src>],
        element_type: &Type<'src>,
        method: &str,
        span: Span,
    ) -> Result<FunctionType<'src>, SemanticError> {
        if args.len() != 1 {
            return Err(SemanticError::new(
                span,
                format!(
                    "array `{method}` expects one callback, found {} arguments",
                    args.len()
                ),
            ));
        }
        let expected = Type::Function(FunctionType {
            params: vec![element_type.clone()],
            return_type: Box::new(Type::Void),
        });
        let callback = self.analyze_expr(&args[0], Some(&expected))?;
        let Type::Function(signature) = callback else {
            return Err(SemanticError::new(
                args[0].span(),
                format!("array `{method}` expects a function callback"),
            ));
        };
        if signature.params.as_slice() != [element_type.clone()] {
            return Err(SemanticError::new(
                args[0].span(),
                format!("array `{method}` callback parameter must be `{element_type}`"),
            ));
        }
        Ok(signature)
    }

    fn analyze_call<'ast>(
        &mut self,
        callee: &Type<'src>,
        args: &[Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        let Type::Function(signature) = callee else {
            return Err(SemanticError::new(
                span,
                format!("cannot call a value of type `{callee}`"),
            ));
        };
        if args.len() != signature.params.len() {
            return Err(SemanticError::new(
                span,
                format!(
                    "function expects {} arguments, found {}",
                    signature.params.len(),
                    args.len()
                ),
            ));
        }
        for (arg, expected) in args.iter().zip(&signature.params) {
            let actual = self.analyze_expr(arg, Some(expected))?;
            self.require_assignable(expected, &actual, arg.span())?;
        }
        Ok((*signature.return_type).clone())
    }

    fn analyze_arrow<'ast>(
        &mut self,
        params: &[crate::ast::Param<'ast, 'src>],
        body: &ArrowBody<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, SemanticError> {
        let expected_signature = match expected {
            Some(Type::Function(signature)) => Some(signature),
            _ => None,
        };
        if let Some(signature) = expected_signature {
            if params.len() != signature.params.len() {
                return Err(SemanticError::new(
                    params.first().map_or(Span::empty(0), |param| param.span),
                    format!(
                        "callback expects {} parameters, found {}",
                        signature.params.len(),
                        params.len()
                    ),
                ));
            }
        }

        let capture_barrier = self.scopes.len();
        self.push_scope();
        self.capture_barriers.push(capture_barrier);
        let mut parameter_types = Vec::with_capacity(params.len());
        for (index, param) in params.iter().enumerate() {
            let ty = self.resolve_value_type(param.ty, "arrow parameter")?;
            if let Some(expected) = expected_signature.and_then(|sig| sig.params.get(index)) {
                if !is_assignable(expected, &ty) || !is_assignable(&ty, expected) {
                    return Err(SemanticError::new(
                        param.span,
                        format!("callback parameter must be `{expected}`, found `{ty}`"),
                    ));
                }
            }
            self.declare(param.name, ty.clone())?;
            parameter_types.push(ty);
        }

        let return_type = match body {
            ArrowBody::Expr(body) => self.analyze_expr(body, None)?,
            ArrowBody::Block(statements) => {
                self.return_contexts.push(ReturnContext::Inferred {
                    ty: None,
                    saw_return: false,
                });
                for statement in *statements {
                    self.analyze_stmt(statement)?;
                }
                match self
                    .return_contexts
                    .pop()
                    .expect("arrow analysis pushed a return context")
                {
                    ReturnContext::Inferred { ty, saw_return }
                        if saw_return && statements_guarantee_return(statements) =>
                    {
                        ty.unwrap_or(Type::Void)
                    }
                    ReturnContext::Inferred { .. } => Type::Void,
                    ReturnContext::Declared { .. } => unreachable!(),
                }
            }
        };
        self.capture_barriers.pop();
        self.pop_scope();

        Ok(Type::Function(FunctionType {
            params: parameter_types,
            return_type: Box::new(return_type),
        }))
    }

    fn analyze_binary(
        &self,
        op: BinaryOp,
        lhs: &Type<'src>,
        rhs: &Type<'src>,
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        match op {
            BinaryOp::Add if lhs == &Type::String || rhs == &Type::String => {
                if is_stringable(lhs) && is_stringable(rhs) {
                    Ok(Type::String)
                } else {
                    Err(invalid_binary(op, lhs, rhs, span))
                }
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
                if lhs.is_numeric() && rhs.is_numeric() =>
            {
                Ok(common_numeric_type(lhs, rhs))
            }
            BinaryOp::Mod if lhs == &Type::Int && rhs == &Type::Int => Ok(Type::Int),
            BinaryOp::Eq | BinaryOp::NotEq if equality_comparable(lhs, rhs) => Ok(Type::Bool),
            BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq
                if (lhs.is_numeric() && rhs.is_numeric())
                    || (lhs == &Type::String && rhs == &Type::String) =>
            {
                Ok(Type::Bool)
            }
            BinaryOp::And | BinaryOp::Or if lhs == &Type::Bool && rhs == &Type::Bool => {
                Ok(Type::Bool)
            }
            _ => Err(invalid_binary(op, lhs, rhs, span)),
        }
    }

    fn resolve_value_type<'ast>(
        &self,
        ty: TypeRef<'ast, 'src>,
        context: &str,
    ) -> Result<Type<'src>, SemanticError> {
        self.resolve_type(ty, false, context)
    }

    fn resolve_type<'ast>(
        &self,
        ty: TypeRef<'ast, 'src>,
        allow_void: bool,
        context: &str,
    ) -> Result<Type<'src>, SemanticError> {
        match ty.kind {
            TypeKind::Int => Ok(Type::Int),
            TypeKind::Float => Ok(Type::Float),
            TypeKind::String => Ok(Type::String),
            TypeKind::Bool => Ok(Type::Bool),
            TypeKind::Void if allow_void => Ok(Type::Void),
            TypeKind::Void => Err(SemanticError::new(
                ty.span,
                format!("{context} cannot have type `void`"),
            )),
            TypeKind::Auto => Err(SemanticError::new(
                ty.span,
                format!("`auto` is not allowed as a {context} type"),
            )),
            TypeKind::Named(name) if self.model.structs.contains_key(name) => {
                Ok(Type::Struct(name))
            }
            TypeKind::Named(name) if self.model.classes.contains_key(name) => Ok(Type::Class(name)),
            TypeKind::Named(name) => Err(SemanticError::new(
                ty.span,
                format!("unknown type `{name}`"),
            )),
            TypeKind::Array(element) => {
                let element = self.resolve_value_type(*element, "array element")?;
                Ok(Type::Array(Box::new(element)))
            }
            TypeKind::Function {
                params,
                return_type,
            } => {
                let mut resolved_params = Vec::with_capacity(params.len());
                for param in params {
                    resolved_params.push(self.resolve_value_type(*param, "function parameter")?);
                }
                let return_type = self.resolve_type(*return_type, true, "function return")?;
                Ok(Type::Function(FunctionType {
                    params: resolved_params,
                    return_type: Box::new(return_type),
                }))
            }
        }
    }

    fn require_assignable(
        &self,
        expected: &Type<'src>,
        actual: &Type<'src>,
        span: Span,
    ) -> Result<(), SemanticError> {
        if is_assignable(expected, actual) {
            Ok(())
        } else {
            Err(SemanticError::new(
                span,
                format!("expected `{expected}`, found `{actual}`"),
            ))
        }
    }

    fn declare(&mut self, ident: Ident<'src>, ty: Type<'src>) -> Result<SymbolId, SemanticError> {
        let scope = self
            .scopes
            .last_mut()
            .expect("semantic analyzer always has a scope");
        if scope.contains_key(ident.name) {
            return Err(SemanticError::new(
                ident.span,
                format!("duplicate binding `{}`", ident.name),
            ));
        }

        let id = SymbolId(self.model.symbols.len() as u32);
        let symbol = Symbol {
            id,
            name: ident.name,
            ty: ty.clone(),
            span: ident.span,
            escape_state: EscapeState::LocalOnly,
        };
        self.model.symbols.push(symbol);
        self.model.binding_types.insert(ident.span, ty);
        self.model.identifier_symbols.insert(ident.span, id);
        scope.insert(ident.name, id);
        Ok(id)
    }

    fn record_detached(&mut self, ident: Ident<'src>, ty: Type<'src>) -> SymbolId {
        let id = SymbolId(self.model.symbols.len() as u32);
        self.model.symbols.push(Symbol {
            id,
            name: ident.name,
            ty: ty.clone(),
            span: ident.span,
            escape_state: EscapeState::LocalOnly,
        });
        self.model.binding_types.insert(ident.span, ty);
        self.model.identifier_symbols.insert(ident.span, id);
        id
    }

    fn resolve(&self, ident: &Ident<'src>) -> Result<&Symbol<'src>, SemanticError> {
        self.resolve_with_scope(ident).map(|(_, symbol)| symbol)
    }

    fn resolve_with_scope(
        &self,
        ident: &Ident<'src>,
    ) -> Result<(usize, &Symbol<'src>), SemanticError> {
        let (scope, id) = self
            .scopes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, scope)| scope.get(ident.name).map(|id| (index, id)))
            .ok_or_else(|| {
                SemanticError::new(ident.span, format!("unknown identifier `{}`", ident.name))
            })?;
        Ok((scope, &self.model.symbols[id.0 as usize]))
    }

    fn push_scope(&mut self) {
        self.scopes.push(AHashMap::new());
    }

    fn pop_scope(&mut self) {
        debug_assert!(self.scopes.len() > 1);
        self.scopes.pop();
    }
}

fn is_assignable(expected: &Type<'_>, actual: &Type<'_>) -> bool {
    if expected == actual {
        return true;
    }
    match (expected, actual) {
        (Type::Float, Type::Int) => true,
        (Type::Array(expected), Type::Array(actual)) => is_assignable(expected, actual),
        _ => false,
    }
}

fn assignment_binary_op(op: AssignmentOp) -> BinaryOp {
    match op {
        AssignmentOp::Assign => unreachable!("plain assignment has no binary operator"),
        AssignmentOp::Add => BinaryOp::Add,
        AssignmentOp::Sub => BinaryOp::Sub,
        AssignmentOp::Mul => BinaryOp::Mul,
        AssignmentOp::Div => BinaryOp::Div,
        AssignmentOp::Mod => BinaryOp::Mod,
    }
}

fn statements_guarantee_return(statements: &[Stmt<'_, '_>]) -> bool {
    statements.iter().any(statement_guarantees_return)
}

fn statement_guarantees_return(statement: &Stmt<'_, '_>) -> bool {
    match statement {
        Stmt::Return { .. } => true,
        Stmt::Block { body, .. } => statements_guarantee_return(body),
        Stmt::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => statement_guarantees_return(then_branch) && statement_guarantees_return(else_branch),
        Stmt::While {
            condition: Expr::Bool(true, _),
            body,
            ..
        } => statement_guarantees_return(body) && !statement_contains_break(body),
        _ => false,
    }
}

fn statement_contains_break(statement: &Stmt<'_, '_>) -> bool {
    match statement {
        Stmt::Break(_) => true,
        Stmt::Block { body, .. } => body.iter().any(statement_contains_break),
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            statement_contains_break(then_branch)
                || else_branch.is_some_and(statement_contains_break)
        }
        Stmt::While { .. } | Stmt::For { .. } => false,
        _ => false,
    }
}

fn common_type<'src>(lhs: &Type<'src>, rhs: &Type<'src>) -> Option<Type<'src>> {
    if lhs == rhs {
        return Some(lhs.clone());
    }
    if lhs.is_numeric() && rhs.is_numeric() {
        return Some(common_numeric_type(lhs, rhs));
    }
    match (lhs, rhs) {
        (Type::Array(lhs), Type::Array(rhs)) => {
            common_type(lhs, rhs).map(|element| Type::Array(Box::new(element)))
        }
        _ => None,
    }
}

fn common_numeric_type<'src>(lhs: &Type<'src>, rhs: &Type<'src>) -> Type<'src> {
    if lhs == &Type::Float || rhs == &Type::Float {
        Type::Float
    } else {
        Type::Int
    }
}

fn equality_comparable(lhs: &Type<'_>, rhs: &Type<'_>) -> bool {
    common_type(lhs, rhs).is_some()
        && !matches!(lhs, Type::Struct(_) | Type::Function(_) | Type::Void)
        && !matches!(rhs, Type::Struct(_) | Type::Function(_) | Type::Void)
}

fn is_stringable(ty: &Type<'_>) -> bool {
    matches!(ty, Type::String | Type::Int | Type::Float | Type::Bool)
}

fn invalid_binary(op: BinaryOp, lhs: &Type<'_>, rhs: &Type<'_>, span: Span) -> SemanticError {
    SemanticError::new(
        span,
        format!(
            "operator `{}` cannot be applied to `{lhs}` and `{rhs}`",
            binary_op_name(op)
        ),
    )
}

fn binary_op_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEq => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::parser::parse_source;

    fn check(source: &str) -> Result<(), SemanticError> {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        analyze(&program).map(|_| ())
    }

    #[test]
    fn infers_auto_and_checks_array_map() {
        check("int[] values=[1,2,3]; auto doubled=values.map((int value)=>value*2);").unwrap();
    }

    #[test]
    fn rejects_wrong_initializer_type() {
        let error = check("int value=\"no\";").unwrap_err();
        assert!(error.message.contains("expected `int`, found `string`"));
    }

    #[test]
    fn rejects_out_of_range_integer_literals() {
        let arena = Bump::new();
        let program = parse_source(&arena, "int value=2147483648;").unwrap();
        assert!(analyze(&program)
            .unwrap_err()
            .message
            .contains("signed 32-bit"));
    }

    #[test]
    fn rejects_unimplemented_struct_and_function_equality() {
        let struct_error = check(
            "struct Pair{int left;int right;}Pair a=Pair{1,2};Pair b=Pair{1,2};bool same=a==b;",
        )
        .unwrap_err();
        assert!(struct_error.message.contains("cannot be applied"));

        let function_error =
            check("auto a=(int value)=>value;auto b=(int value)=>value;bool same=a==b;")
                .unwrap_err();
        assert!(function_error.message.contains("cannot be applied"));
    }

    #[test]
    fn rejects_unknown_identifiers() {
        let error = check("int value=missing+1;").unwrap_err();
        assert!(error.message.contains("unknown identifier `missing`"));
    }

    #[test]
    fn validates_struct_fields_and_literals() {
        check("struct Point{int x;int y;} Point p=Point{10,20}; int x=p.x;").unwrap();
        let error = check("struct Point{int x;} Point p=Point{\"bad\"};").unwrap_err();
        assert!(error.message.contains("expected `int`, found `string`"));
    }

    #[test]
    fn validates_functions_and_returns() {
        check("int add(int a,int b){return a+b;} int result=add(1,2);").unwrap();
        let error = check("int bad(){return true;}").unwrap_err();
        assert!(error.message.contains("expected return type `int`"));
    }

    #[test]
    fn validates_class_construction_and_methods() {
        check(
            "class Vector{float x;float length(){return this.x;}} Vector v=new Vector(); float n=v.length();",
        )
        .unwrap();
    }

    #[test]
    fn validates_control_flow_mutation_and_templates() {
        check(
            "int sum=0;for(int i=0;i<3;i++){sum+=i;}if(sum==3){print(`sum=${sum}`);}else{print(\"bad\");}",
        )
        .unwrap();
    }

    #[test]
    fn validates_constructor_arguments() {
        check(
            "class Pair{int x;int y;init(int x,int y){this.x=x;this.y=y;}} Pair p=new Pair(1,2);",
        )
        .unwrap();
        let error = check("class Pair{init(int x){}} Pair p=new Pair(\"wrong\");").unwrap_err();
        assert!(error.message.contains("expected `int`, found `string`"));
    }

    #[test]
    fn rejects_break_outside_loop() {
        let error = check("break;").unwrap_err();
        assert!(error.message.contains("outside a loop"));
    }

    #[test]
    fn validates_standard_array_and_string_methods() {
        check(
            "int[] xs=[1,2,3];int[] ys=xs.filter((int x)=>x>1);int total=ys.reduce((int a,int x)=>a+x,0);int n=xs.push(4);int last=xs.pop();bool found=\"lilscript\".includes(\"pex\");",
        )
        .unwrap();
    }

    #[test]
    fn requires_all_paths_to_return() {
        check("int choose(bool flag){if(flag){return 1;}else{return 2;}}").unwrap();
        let error = check("int choose(bool flag){if(flag){return 1;}}").unwrap_err();
        assert!(error.message.contains("must return a value"));
    }

    #[test]
    fn checks_explicit_callable_types() {
        check("func(int)->int twice=(int x)=>x*2;int answer=twice(21);").unwrap();
        let error = check("func(int)->bool bad=(int x)=>x+1;").unwrap_err();
        assert!(error.message.contains("expected `function(int) -> bool`"));
    }

    #[test]
    fn rejects_rebinding_captured_values() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int run(int seed){auto next=()=>{seed+=1;return seed;};return next();}",
        )
        .unwrap();
        assert!(analyze(&program)
            .unwrap_err()
            .message
            .contains("captured binding `seed` is read-only"));
    }

    #[test]
    fn rejects_uninitialized_variables() {
        let arena = Bump::new();
        let program = parse_source(&arena, "int value;").unwrap();
        assert!(analyze(&program)
            .unwrap_err()
            .message
            .contains("require an initializer"));
    }

    #[test]
    fn checks_extern_call_signatures() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int hostAdd(int a,int b);int value=hostAdd(1,2);",
        )
        .unwrap();
        analyze(&program).unwrap();

        let invalid = parse_source(
            &arena,
            "extern int hostAdd(int a,int b);int value=hostAdd(1);",
        )
        .unwrap();
        assert!(analyze(&invalid)
            .unwrap_err()
            .message
            .contains("2 arguments"));
    }
}
