use std::fmt;

use ahash::{AHashMap, AHashSet};
use indexmap::IndexMap;

use crate::ast::{
    ArrowBody, AssignmentOp, BinaryOp, ClassDecl, ClassMember, ConstructorDecl, Expr,
    ExternClassMember, ExternDecl, ForInitializer, FunctionDecl, Ident, Item, Program, Stmt,
    StructDecl, TemplatePart, TypeKind, TypeRef, UnaryOp, UpdateOp, VarDecl,
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
    Null,
    Void,
    Array(Box<Type<'src>>),
    Map(Box<Type<'src>>, Box<Type<'src>>),
    Set(Box<Type<'src>>),
    ArrayBuffer,
    SharedArrayBuffer,
    Uint8Array,
    Nullable(Box<Type<'src>>),
    Union(Vec<Type<'src>>),
    Struct(&'src str),
    Class(&'src str),
    StructInstance {
        name: &'src str,
        args: Vec<Type<'src>>,
    },
    ClassInstance {
        name: &'src str,
        args: Vec<Type<'src>>,
    },
    TypeParameter(&'src str),
    Function(FunctionType<'src>),
    GenericFunction(GenericFunctionType<'src>),
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
            Self::Null => f.write_str("null"),
            Self::Void => f.write_str("void"),
            Self::Array(element) => match element.as_ref() {
                Self::Union(_) => write!(f, "({element})[]"),
                _ => write!(f, "{element}[]"),
            },
            Self::Map(key, value) => write!(f, "Map<{key}, {value}>"),
            Self::Set(element) => write!(f, "Set<{element}>"),
            Self::ArrayBuffer => f.write_str("ArrayBuffer"),
            Self::SharedArrayBuffer => f.write_str("SharedArrayBuffer"),
            Self::Uint8Array => f.write_str("Uint8Array"),
            Self::Nullable(inner) => match inner.as_ref() {
                Self::Union(_) => write!(f, "({inner})?"),
                _ => write!(f, "{inner}?"),
            },
            Self::Union(members) => {
                for (index, member) in members.iter().enumerate() {
                    if index != 0 {
                        f.write_str(" | ")?;
                    }
                    write!(f, "{member}")?;
                }
                Ok(())
            }
            Self::Struct(name) | Self::Class(name) => f.write_str(name),
            Self::StructInstance { name, args } | Self::ClassInstance { name, args } => {
                write!(f, "{name}<")?;
                for (index, argument) in args.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{argument}")?;
                }
                f.write_str(">")
            }
            Self::TypeParameter(name) => f.write_str(name),
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
            Self::GenericFunction(function) => {
                f.write_str("function<")?;
                for (index, parameter) in function.type_params.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str(parameter)?;
                }
                write!(f, ">({})", Type::Function(function.signature.clone()))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType<'src> {
    pub params: Vec<Type<'src>>,
    pub defaults: Vec<Option<DefaultValue<'src>>>,
    pub return_type: Box<Type<'src>>,
}

impl FunctionType<'_> {
    pub fn required_params(&self) -> usize {
        self.defaults
            .iter()
            .position(Option::is_some)
            .unwrap_or(self.params.len())
    }

    pub fn accepts_arity(&self, arity: usize) -> bool {
        arity >= self.required_params() && arity <= self.params.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultValue<'src> {
    Int(i64),
    Float(u64),
    String(&'src str),
    Bool(bool),
    Null,
    Array(Vec<DefaultValue<'src>>),
    Arrow(Span),
    Struct {
        name: &'src str,
        values: Vec<DefaultValue<'src>>,
    },
    NewClass {
        name: &'src str,
        args: Vec<DefaultValue<'src>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericFunctionType<'src> {
    pub type_params: Vec<&'src str>,
    pub signature: FunctionType<'src>,
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
    pub type_params: Vec<&'src str>,
    pub fields: IndexMap<&'src str, FieldInfo<'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassInfo<'src> {
    pub name: &'src str,
    pub type_params: Vec<&'src str>,
    pub fields: IndexMap<&'src str, FieldInfo<'src>>,
    pub methods: IndexMap<&'src str, MethodInfo<'src>>,
    pub constructor: Option<FunctionType<'src>>,
    pub external: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodInfo<'src> {
    pub type_params: Vec<&'src str>,
    pub signature: FunctionType<'src>,
    pub declared_pure: bool,
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
    type_check_types: AHashMap<Span, Type<'src>>,
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

    pub(crate) fn type_check_type(&self, span: Span) -> Option<&Type<'src>> {
        self.type_check_types.get(&span)
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

    pub fn is_extern_class(&self, name: &str) -> bool {
        self.classes.get(name).is_some_and(|class| class.external)
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
    narrowings: Vec<AHashMap<SymbolId, Type<'src>>>,
    return_contexts: Vec<ReturnContext<'src>>,
    capture_barriers: Vec<usize>,
    type_parameter_scopes: Vec<AHashSet<&'src str>>,
    loop_depth: usize,
}

type Narrowing<'src> = Option<(SymbolId, Type<'src>)>;

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
                type_check_types: AHashMap::new(),
                binding_types: AHashMap::new(),
                identifier_symbols: AHashMap::new(),
                symbols: Vec::new(),
                structs: AHashMap::new(),
                classes: AHashMap::new(),
            },
            scopes: vec![AHashMap::new()],
            narrowings: vec![AHashMap::new()],
            return_contexts: Vec::new(),
            capture_barriers: Vec::new(),
            type_parameter_scopes: Vec::new(),
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
        self.define_extern_classes(program)?;
        self.declare_functions(program)?;

        for item in program.items {
            match item {
                Item::Struct(_) => {}
                Item::Class(class) => self.analyze_class(class)?,
                Item::ExternClass(class) => self.analyze_extern_class_defaults(class)?,
                Item::Function(function) => self.analyze_function(function, None)?,
                Item::Extern(extern_decl) => self.analyze_extern_defaults(extern_decl)?,
                Item::ExternGlobal(_) => {}
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
            let (name, type_params, span, is_struct, external) = match item {
                Item::Struct(decl) => (decl.name.name, decl.type_params, decl.span, true, false),
                Item::Class(decl) => (decl.name.name, decl.type_params, decl.span, false, false),
                Item::ExternClass(decl) => {
                    (decl.name.name, decl.type_params, decl.span, false, true)
                }
                _ => continue,
            };
            let type_params = validate_type_params(type_params)?;

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
                        type_params,
                        fields: IndexMap::new(),
                        span,
                    },
                );
            } else {
                self.model.classes.insert(
                    name,
                    ClassInfo {
                        name,
                        type_params,
                        fields: IndexMap::new(),
                        methods: IndexMap::new(),
                        constructor: None,
                        external,
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
            self.push_type_params(decl.type_params)?;
            let fields = self.resolve_fields(decl)?;
            self.pop_type_params();
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

            self.push_type_params(decl.type_params)?;

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
                        methods.insert(
                            method.name.name,
                            MethodInfo {
                                type_params: validate_type_params(method.type_params)?,
                                signature: self.function_type(method)?,
                                declared_pure: method.declared_pure,
                            },
                        );
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
                        let defaults =
                            resolve_parameter_defaults(constructor_decl.params, &params)?;
                        constructor = Some(FunctionType {
                            params,
                            defaults,
                            return_type: Box::new(applied_nominal_type(
                                decl.name.name,
                                &validate_type_params(decl.type_params)?,
                                true,
                            )),
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

            self.pop_type_params();

            let constructor_signature = constructor.unwrap_or(FunctionType {
                params: Vec::new(),
                defaults: Vec::new(),
                return_type: Box::new(applied_nominal_type(
                    decl.name.name,
                    &validate_type_params(decl.type_params)?,
                    true,
                )),
            });
            let constructor = if decl.type_params.is_empty() {
                Type::Function(constructor_signature)
            } else {
                Type::GenericFunction(GenericFunctionType {
                    type_params: validate_type_params(decl.type_params)?,
                    signature: constructor_signature,
                })
            };
            self.declare(decl.name, constructor)?;
        }
        Ok(())
    }

    fn define_extern_classes<'ast>(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), SemanticError> {
        for item in program.items {
            let Item::ExternClass(decl) = item else {
                continue;
            };
            self.push_type_params(decl.type_params)?;
            let mut fields = IndexMap::new();
            let mut methods = IndexMap::new();
            for member in decl.members {
                match member {
                    ExternClassMember::Field(field) => {
                        if fields.contains_key(field.name.name)
                            || methods.contains_key(field.name.name)
                        {
                            return Err(SemanticError::new(
                                field.name.span,
                                format!(
                                    "duplicate member `{}` in extern class `{}`",
                                    field.name.name, decl.name.name
                                ),
                            ));
                        }
                        let index = fields.len();
                        fields.insert(
                            field.name.name,
                            FieldInfo {
                                name: field.name.name,
                                ty: self.resolve_value_type(field.ty, "extern class field")?,
                                index,
                                span: field.span,
                            },
                        );
                    }
                    ExternClassMember::Method(method) => {
                        if fields.contains_key(method.name.name)
                            || methods.contains_key(method.name.name)
                        {
                            return Err(SemanticError::new(
                                method.name.span,
                                format!(
                                    "duplicate member `{}` in extern class `{}`",
                                    method.name.name, decl.name.name
                                ),
                            ));
                        }
                        methods.insert(
                            method.name.name,
                            MethodInfo {
                                type_params: validate_type_params(method.type_params)?,
                                signature: self.extern_type(method)?,
                                declared_pure: method.declared_pure,
                            },
                        );
                    }
                }
            }
            self.pop_type_params();
            let info = self
                .model
                .classes
                .get_mut(decl.name.name)
                .expect("extern class name was declared in the first semantic pass");
            info.fields = fields;
            info.methods = methods;
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
                    let signature = self.function_type(function)?;
                    let ty = if function.type_params.is_empty() {
                        Type::Function(signature)
                    } else {
                        Type::GenericFunction(GenericFunctionType {
                            type_params: validate_type_params(function.type_params)?,
                            signature,
                        })
                    };
                    self.declare(function.name, ty)?;
                }
                Item::Extern(extern_decl) => {
                    let signature = self.extern_type(extern_decl)?;
                    let ty = if extern_decl.type_params.is_empty() {
                        Type::Function(signature.clone())
                    } else {
                        Type::GenericFunction(GenericFunctionType {
                            type_params: validate_type_params(extern_decl.type_params)?,
                            signature: signature.clone(),
                        })
                    };
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
                Item::ExternGlobal(global) => {
                    let ty = self.resolve_value_type(global.ty, "extern global")?;
                    self.declare(global.name, ty)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn function_type<'ast>(
        &mut self,
        function: &FunctionDecl<'ast, 'src>,
    ) -> Result<FunctionType<'src>, SemanticError> {
        self.push_type_params(function.type_params)?;
        let signature = self.function_type_in_current_scope(function);
        self.pop_type_params();
        signature
    }

    fn function_type_in_current_scope<'ast>(
        &self,
        function: &FunctionDecl<'ast, 'src>,
    ) -> Result<FunctionType<'src>, SemanticError> {
        let mut params = Vec::with_capacity(function.params.len());
        for param in function.params {
            params.push(self.resolve_value_type(param.ty, "parameter")?);
        }
        let defaults = resolve_parameter_defaults(function.params, &params)?;
        let return_type = self.resolve_type(function.return_type, true, "return type")?;
        Ok(FunctionType {
            params,
            defaults,
            return_type: Box::new(return_type),
        })
    }

    fn extern_type<'ast>(
        &mut self,
        extern_decl: &ExternDecl<'ast, 'src>,
    ) -> Result<FunctionType<'src>, SemanticError> {
        self.push_type_params(extern_decl.type_params)?;
        let mut params = Vec::with_capacity(extern_decl.params.len());
        for param in extern_decl.params {
            params.push(self.resolve_value_type(param.ty, "extern parameter")?);
        }
        let defaults = resolve_parameter_defaults(extern_decl.params, &params)?;
        let return_type = self.resolve_type(extern_decl.return_type, true, "extern return type")?;
        let signature = FunctionType {
            params,
            defaults,
            return_type: Box::new(return_type),
        };
        self.pop_type_params();
        Ok(signature)
    }

    fn analyze_class<'ast>(&mut self, class: &ClassDecl<'ast, 'src>) -> Result<(), SemanticError> {
        self.push_type_params(class.type_params)?;
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
        self.pop_type_params();
        Ok(())
    }

    fn analyze_constructor<'ast>(
        &mut self,
        constructor: &ConstructorDecl<'ast, 'src>,
        class_name: &'src str,
    ) -> Result<(), SemanticError> {
        let parameter_types = constructor
            .params
            .iter()
            .map(|param| self.resolve_value_type(param.ty, "parameter"))
            .collect::<Result<Vec<_>, _>>()?;
        self.analyze_parameter_defaults(constructor.params, &parameter_types)?;
        self.push_scope();
        let class_type_params = self
            .model
            .classes
            .get(class_name)
            .map(|class| class.type_params.clone())
            .unwrap_or_default();
        self.declare(
            Ident {
                name: "this",
                span: constructor.span,
            },
            applied_nominal_type(class_name, &class_type_params, true),
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
        self.push_type_params(function.type_params)?;
        let signature = self.function_type_in_current_scope(function)?;
        self.analyze_parameter_defaults(function.params, &signature.params)?;
        self.push_scope();

        if let Some(class_name) = class_name {
            self.declare(
                Ident {
                    name: "this",
                    span: function.name.span,
                },
                applied_nominal_type(
                    class_name,
                    &self
                        .model
                        .classes
                        .get(class_name)
                        .map(|class| class.type_params.clone())
                        .unwrap_or_default(),
                    true,
                ),
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
        self.pop_type_params();

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

    fn analyze_extern_defaults<'ast>(
        &mut self,
        extern_decl: &ExternDecl<'ast, 'src>,
    ) -> Result<(), SemanticError> {
        self.push_type_params(extern_decl.type_params)?;
        let types = extern_decl
            .params
            .iter()
            .map(|param| self.resolve_value_type(param.ty, "extern parameter"))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.analyze_parameter_defaults(extern_decl.params, &types);
        self.pop_type_params();
        result
    }

    fn analyze_extern_class_defaults<'ast>(
        &mut self,
        class: &crate::ast::ExternClassDecl<'ast, 'src>,
    ) -> Result<(), SemanticError> {
        self.push_type_params(class.type_params)?;
        for member in class.members {
            if let ExternClassMember::Method(method) = member {
                self.analyze_extern_defaults(method)?;
            }
        }
        self.pop_type_params();
        Ok(())
    }

    fn analyze_parameter_defaults<'ast>(
        &mut self,
        params: &[crate::ast::Param<'ast, 'src>],
        types: &[Type<'src>],
    ) -> Result<(), SemanticError> {
        for (param, expected) in params.iter().zip(types) {
            let Some(expression) = &param.default else {
                continue;
            };
            if scalar_default_value(expression).is_some() {
                continue;
            }
            let contextual = match expression {
                Expr::ArrayLiteral { .. } => expected_array_type(expected).unwrap_or(expected),
                _ => expected,
            };
            let actual = self.analyze_expr(expression, Some(contextual))?;
            self.require_assignable(expected, &actual, expression.span())?;
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
                let (then_narrowing, else_narrowing) = self.condition_narrowing(condition)?;
                let then_returns = statement_guarantees_return(then_branch);
                let else_returns =
                    else_branch.is_some_and(|branch| statement_guarantees_return(branch));
                self.push_scope();
                self.apply_narrowing(then_narrowing.clone());
                self.analyze_stmt(then_branch)?;
                let then_survives = self.current_scope_preserves(&then_narrowing);
                self.pop_scope();
                let mut else_survives = else_branch.is_none() && else_narrowing.is_some();
                if let Some(else_branch) = else_branch {
                    self.push_scope();
                    self.apply_narrowing(else_narrowing.clone());
                    self.analyze_stmt(else_branch)?;
                    else_survives = self.current_scope_preserves(&else_narrowing);
                    self.pop_scope();
                }
                if then_returns && !else_returns && else_survives {
                    self.apply_narrowing(else_narrowing);
                } else if else_returns && !then_returns && then_survives {
                    self.apply_narrowing(then_narrowing);
                }
                Ok(())
            }
            Stmt::While {
                condition, body, ..
            } => {
                let condition_type = self.analyze_expr(condition, Some(&Type::Bool))?;
                self.require_assignable(&Type::Bool, &condition_type, condition.span())?;
                let (body_narrowing, _) = self.condition_narrowing(condition)?;
                self.loop_depth += 1;
                self.push_scope();
                self.apply_narrowing(body_narrowing);
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
            if inferred == Type::Null {
                return Err(SemanticError::new(
                    initializer.span(),
                    "cannot infer a variable type from `null`; add an explicit nullable type",
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
                if !is_type_assignable(ty, &actual) {
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
            Expr::Null(_) => Type::Null,
            Expr::Ident(ident) => {
                let (id, declared) = {
                    let symbol = self.resolve(ident)?;
                    (symbol.id, symbol.ty.clone())
                };
                self.model.identifier_symbols.insert(ident.span, id);
                self.narrowed_type(id).cloned().unwrap_or(declared)
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
            Expr::New {
                class,
                type_args,
                args,
                span,
            } => {
                if let Some(ty) =
                    self.analyze_builtin_constructor(*class, type_args, args, *span, expected)?
                {
                    ty
                } else {
                    let info = self.model.classes.get(class.name).cloned().ok_or_else(|| {
                        SemanticError::new(class.span, format!("unknown class `{}`", class.name))
                    })?;
                    if info.external {
                        return Err(SemanticError::new(
                            *span,
                            format!("extern class `{}` cannot be constructed", class.name),
                        ));
                    }
                    let params = info
                        .constructor
                        .as_ref()
                        .map_or(&[][..], |signature| signature.params.as_slice());
                    let accepts_arity = info
                        .constructor
                        .as_ref()
                        .map_or(args.is_empty(), |signature| {
                            signature.accepts_arity(args.len())
                        });
                    if !accepts_arity {
                        return Err(SemanticError::new(
                            *span,
                            format!(
                                "class `{}` constructor expects {} arguments, found {}",
                                class.name,
                                info.constructor
                                    .as_ref()
                                    .map_or(0, FunctionType::required_params),
                                args.len()
                            ),
                        ));
                    }
                    let parameter_names = info.type_params.iter().copied().collect::<AHashSet<_>>();
                    let mut substitutions = AHashMap::new();
                    if !type_args.is_empty() {
                        let resolved = self.resolve_type_arguments(
                            class.name,
                            type_args,
                            &info.type_params,
                            *span,
                        )?;
                        substitutions.extend(info.type_params.iter().copied().zip(resolved));
                    } else if let Some(Type::ClassInstance {
                        name,
                        args: expected_args,
                    }) = expected
                    {
                        if *name == class.name && expected_args.len() == info.type_params.len() {
                            substitutions.extend(
                                info.type_params
                                    .iter()
                                    .copied()
                                    .zip(expected_args.iter().cloned()),
                            );
                        }
                    } else if info.type_params.is_empty() {
                        self.resolve_type_arguments(
                            class.name,
                            type_args,
                            &info.type_params,
                            *span,
                        )?;
                    }
                    for (arg, pattern) in args.iter().zip(params) {
                        let resolved = substitute_type(pattern, &substitutions);
                        let expected = (!contains_type_parameter(&resolved, &parameter_names))
                            .then_some(&resolved);
                        let actual = self.analyze_expr(arg, expected)?;
                        infer_type_arguments(
                            pattern,
                            &actual,
                            &parameter_names,
                            &mut substitutions,
                            arg.span(),
                        )?;
                        let resolved = substitute_type(pattern, &substitutions);
                        if !contains_type_parameter(&resolved, &parameter_names) {
                            self.require_assignable(&resolved, &actual, arg.span())?;
                        }
                    }
                    let resolved_args = info
                        .type_params
                        .iter()
                        .map(|parameter| {
                            substitutions.get(parameter).cloned().ok_or_else(|| {
                                SemanticError::new(
                                    *span,
                                    format!("cannot infer type argument `{parameter}`"),
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if info.type_params.is_empty() {
                        Type::Class(class.name)
                    } else {
                        Type::ClassInstance {
                            name: class.name,
                            args: resolved_args,
                        }
                    }
                }
            }
            Expr::Member {
                object,
                property,
                span,
            } => self.analyze_member(object, *property, *span)?,
            Expr::Call { callee, args, span } => {
                if matches!(
                    callee,
                    Expr::Member {
                        object,
                        property: Ident { name: "imul", .. },
                        ..
                    } if matches!(object, Expr::Ident(Ident { name: "Math", .. }))
                ) {
                    if args.len() != 2 {
                        return Err(SemanticError::new(
                            *span,
                            format!("`Math.imul` expects two arguments, found {}", args.len()),
                        ));
                    }
                    for arg in *args {
                        let actual = self.analyze_expr(arg, Some(&Type::Int))?;
                        self.require_assignable(&Type::Int, &actual, arg.span())?;
                    }
                    Type::Int
                } else if matches!(callee, Expr::Ident(Ident { name: "print", .. })) {
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
                            self.analyze_call(&callee_type, args, *span, expected)?
                        }
                    }
                } else {
                    let callee_type = self.analyze_expr(callee, None)?;
                    self.analyze_call(&callee_type, args, *span, expected)?
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
            Expr::TypeCheck {
                value,
                target,
                span,
            } => {
                let value_type = self.analyze_expr(value, None)?;
                let target_type = self.resolve_value_type(*target, "type guard")?;
                validate_type_guard(&value_type, &target_type, *span)?;
                self.model.type_check_types.insert(*span, target_type);
                Type::Bool
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
                    Type::Uint8Array => Type::Int,
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
                self.invalidate_assigned_narrowing(target);
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
                self.invalidate_assigned_narrowing(target);
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
                    Type::Uint8Array => Type::Int,
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
            Type::StructInstance { name, args } => {
                let info = self
                    .model
                    .structs
                    .get(name)
                    .expect("struct instances always have struct metadata");
                let substitutions = substitutions_for(&info.type_params, &args);
                info.fields
                    .get(property.name)
                    .map(|field| substitute_type(&field.ty, &substitutions))
                    .ok_or_else(|| {
                        SemanticError::new(
                            property.span,
                            format!("struct `{name}` has no field `{}`", property.name),
                        )
                    })
            }
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
                    return Ok(method_callable_type(method, &AHashMap::new()));
                }
                Err(SemanticError::new(
                    property.span,
                    format!("class `{name}` has no member `{}`", property.name),
                ))
            }
            Type::ClassInstance { name, args } => {
                let class = self
                    .model
                    .classes
                    .get(name)
                    .expect("class instances always have class metadata");
                let substitutions = substitutions_for(&class.type_params, &args);
                if let Some(field) = class.fields.get(property.name) {
                    return Ok(substitute_type(&field.ty, &substitutions));
                }
                if let Some(method) = class.methods.get(property.name) {
                    return Ok(method_callable_type(method, &substitutions));
                }
                Err(SemanticError::new(
                    property.span,
                    format!("class `{name}` has no member `{}`", property.name),
                ))
            }
            Type::Array(_) | Type::String if property.name == "length" => Ok(Type::Int),
            Type::Map(_, _) | Type::Set(_) if property.name == "size" => Ok(Type::Int),
            Type::ArrayBuffer | Type::SharedArrayBuffer if property.name == "byteLength" => {
                Ok(Type::Int)
            }
            Type::Uint8Array if matches!(property.name, "length" | "byteLength" | "byteOffset") => {
                Ok(Type::Int)
            }
            Type::Uint8Array if property.name == "buffer" => Ok(normalize_union(vec![
                Type::ArrayBuffer,
                Type::SharedArrayBuffer,
            ])),
            Type::Float => match property.name {
                "abs" | "floor" | "ceil" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: Box::new(Type::Float),
                })),
                "min" | "max" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Float],
                    defaults: vec![None],
                    return_type: Box::new(Type::Float),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("float has no member `{}`", property.name),
                )),
            },
            Type::Array(element) => match property.name {
                "map" | "filter" | "forEach" | "reduce" => Err(SemanticError::new(
                    span,
                    format!("array `{}` must be called", property.name),
                )),
                "push" => Ok(Type::Function(FunctionType {
                    params: vec![*element],
                    defaults: vec![None],
                    return_type: Box::new(Type::Int),
                })),
                "pop" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: element,
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("array has no member `{}`", property.name),
                )),
            },
            Type::Map(key, value) => match property.name {
                "get" => Ok(Type::Function(FunctionType {
                    params: vec![key.as_ref().clone()],
                    defaults: vec![None],
                    return_type: Box::new(nullable_type(value.as_ref().clone())),
                })),
                "set" => Ok(Type::Function(FunctionType {
                    params: vec![key.as_ref().clone(), value.as_ref().clone()],
                    defaults: vec![None, None],
                    return_type: Box::new(Type::Map(key, value)),
                })),
                "has" | "delete" => Ok(Type::Function(FunctionType {
                    params: vec![*key],
                    defaults: vec![None],
                    return_type: Box::new(Type::Bool),
                })),
                "clear" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: Box::new(Type::Void),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("map has no member `{}`", property.name),
                )),
            },
            Type::Set(element) => match property.name {
                "add" => Ok(Type::Function(FunctionType {
                    params: vec![element.as_ref().clone()],
                    defaults: vec![None],
                    return_type: Box::new(Type::Set(element)),
                })),
                "has" | "delete" => Ok(Type::Function(FunctionType {
                    params: vec![*element],
                    defaults: vec![None],
                    return_type: Box::new(Type::Bool),
                })),
                "clear" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: Box::new(Type::Void),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("set has no member `{}`", property.name),
                )),
            },
            Type::ArrayBuffer => buffer_member(property, span, Type::ArrayBuffer),
            Type::SharedArrayBuffer => buffer_member(property, span, Type::SharedArrayBuffer),
            Type::Uint8Array => match property.name {
                "slice" | "subarray" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int, Type::Int],
                    defaults: vec![None, Some(DefaultValue::Int(i32::MAX as i64))],
                    return_type: Box::new(Type::Uint8Array),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("Uint8Array has no member `{}`", property.name),
                )),
            },
            Type::String => match property.name {
                "charCodeAt" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int],
                    defaults: vec![None],
                    return_type: Box::new(Type::Int),
                })),
                "includes" | "startsWith" | "endsWith" => Ok(Type::Function(FunctionType {
                    params: vec![Type::String],
                    defaults: vec![None],
                    return_type: Box::new(Type::Bool),
                })),
                "toUpperCase" | "toLowerCase" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
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
            defaults: vec![None],
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
            || !is_type_assignable(&signature.params[0], &element_type)
            || !is_type_assignable(&element_type, &signature.params[0])
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
            defaults: vec![None, None],
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
            || !is_type_assignable(&accumulator, &signature.return_type)
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
            defaults: vec![None],
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
        expected_return: Option<&Type<'src>>,
    ) -> Result<Type<'src>, SemanticError> {
        if let Type::GenericFunction(function) = callee {
            return self.analyze_generic_call(function, args, span, expected_return);
        }
        let Type::Function(signature) = callee else {
            return Err(SemanticError::new(
                span,
                format!("cannot call a value of type `{callee}`"),
            ));
        };
        if !signature.accepts_arity(args.len()) {
            return Err(SemanticError::new(
                span,
                format!(
                    "function expects {} to {} arguments, found {}",
                    signature.required_params(),
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

    fn analyze_generic_call<'ast>(
        &mut self,
        function: &GenericFunctionType<'src>,
        args: &[Expr<'ast, 'src>],
        span: Span,
        expected_return: Option<&Type<'src>>,
    ) -> Result<Type<'src>, SemanticError> {
        if !function.signature.accepts_arity(args.len()) {
            return Err(SemanticError::new(
                span,
                format!(
                    "function expects {} to {} arguments, found {}",
                    function.signature.required_params(),
                    function.signature.params.len(),
                    args.len()
                ),
            ));
        }
        let parameters = function
            .type_params
            .iter()
            .copied()
            .collect::<AHashSet<_>>();
        let mut substitutions = AHashMap::new();
        if let Some(expected_return) = expected_return {
            infer_type_arguments(
                &function.signature.return_type,
                expected_return,
                &parameters,
                &mut substitutions,
                span,
            )?;
        }
        for (arg, pattern) in args.iter().zip(&function.signature.params) {
            let partially_resolved = substitute_type(pattern, &substitutions);
            let expected = (!contains_type_parameter(&partially_resolved, &parameters))
                .then_some(&partially_resolved);
            let actual = self.analyze_expr(arg, expected)?;
            infer_type_arguments(
                pattern,
                &actual,
                &parameters,
                &mut substitutions,
                arg.span(),
            )?;
            let resolved = substitute_type(pattern, &substitutions);
            if !contains_type_parameter(&resolved, &parameters) {
                self.require_assignable(&resolved, &actual, arg.span())?;
            }
        }
        for parameter in &function.type_params {
            if !substitutions.contains_key(parameter) {
                return Err(SemanticError::new(
                    span,
                    format!("cannot infer type argument `{parameter}`"),
                ));
            }
        }
        Ok(substitute_type(
            &function.signature.return_type,
            &substitutions,
        ))
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
                if !is_type_assignable(expected, &ty) || !is_type_assignable(&ty, expected) {
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

        let defaults = resolve_parameter_defaults(params, &parameter_types)?;
        Ok(Type::Function(FunctionType {
            params: parameter_types,
            defaults,
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
            BinaryOp::Xor if lhs == &Type::Int && rhs == &Type::Int => Ok(Type::Int),
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

    fn analyze_builtin_constructor<'ast>(
        &mut self,
        class: Ident<'src>,
        type_args: &[TypeRef<'ast, 'src>],
        args: &[Expr<'ast, 'src>],
        span: Span,
        expected: Option<&Type<'src>>,
    ) -> Result<Option<Type<'src>>, SemanticError> {
        let ty = match class.name {
            "Map" => {
                if !args.is_empty() {
                    return Err(SemanticError::new(
                        span,
                        format!(
                            "`Map` constructor expects 0 arguments, found {}",
                            args.len()
                        ),
                    ));
                }
                if type_args.is_empty() {
                    let Some(Type::Map(key, value)) = expected else {
                        return Err(SemanticError::new(
                            span,
                            "cannot infer `Map` type arguments; write `new Map<K, V>()`",
                        ));
                    };
                    Type::Map(key.clone(), value.clone())
                } else {
                    let resolved =
                        self.resolve_type_arguments("Map", type_args, &["K", "V"], span)?;
                    validate_collection_key(&resolved[0], span, "Map key")?;
                    Type::Map(Box::new(resolved[0].clone()), Box::new(resolved[1].clone()))
                }
            }
            "Set" => {
                if !args.is_empty() {
                    return Err(SemanticError::new(
                        span,
                        format!(
                            "`Set` constructor expects 0 arguments, found {}",
                            args.len()
                        ),
                    ));
                }
                if type_args.is_empty() {
                    let Some(Type::Set(element)) = expected else {
                        return Err(SemanticError::new(
                            span,
                            "cannot infer `Set` type argument; write `new Set<T>()`",
                        ));
                    };
                    Type::Set(element.clone())
                } else {
                    let resolved = self.resolve_type_arguments("Set", type_args, &["T"], span)?;
                    validate_collection_key(&resolved[0], span, "Set element")?;
                    Type::Set(Box::new(resolved[0].clone()))
                }
            }
            "ArrayBuffer" | "SharedArrayBuffer" => {
                self.resolve_type_arguments(class.name, type_args, &[], span)?;
                if args.len() != 1 {
                    return Err(SemanticError::new(
                        span,
                        format!(
                            "`{}` constructor expects 1 argument, found {}",
                            class.name,
                            args.len()
                        ),
                    ));
                }
                let actual = self.analyze_expr(&args[0], Some(&Type::Int))?;
                self.require_assignable(&Type::Int, &actual, args[0].span())?;
                if class.name == "ArrayBuffer" {
                    Type::ArrayBuffer
                } else {
                    Type::SharedArrayBuffer
                }
            }
            "Uint8Array" => {
                self.resolve_type_arguments(class.name, type_args, &[], span)?;
                if args.len() != 1 {
                    return Err(SemanticError::new(
                        span,
                        format!(
                            "`Uint8Array` constructor expects 1 argument, found {}",
                            args.len()
                        ),
                    ));
                }
                let actual = self.analyze_expr(&args[0], None)?;
                if !matches!(
                    actual,
                    Type::Int | Type::ArrayBuffer | Type::SharedArrayBuffer
                ) {
                    return Err(SemanticError::new(
                        args[0].span(),
                        format!(
                            "`Uint8Array` expects an `int`, `ArrayBuffer`, or `SharedArrayBuffer`, found `{actual}`"
                        ),
                    ));
                }
                Type::Uint8Array
            }
            _ => return Ok(None),
        };
        Ok(Some(ty))
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
            TypeKind::Named { name, args }
                if self
                    .type_parameter_scopes
                    .iter()
                    .rev()
                    .any(|scope| scope.contains(name)) =>
            {
                if !args.is_empty() {
                    return Err(SemanticError::new(
                        ty.span,
                        format!("type parameter `{name}` does not accept type arguments"),
                    ));
                }
                Ok(Type::TypeParameter(name))
            }
            TypeKind::Named { name: "Map", args } => {
                let resolved = self.resolve_type_arguments("Map", args, &["K", "V"], ty.span)?;
                validate_collection_key(&resolved[0], ty.span, "Map key")?;
                Ok(Type::Map(
                    Box::new(resolved[0].clone()),
                    Box::new(resolved[1].clone()),
                ))
            }
            TypeKind::Named { name: "Set", args } => {
                let resolved = self.resolve_type_arguments("Set", args, &["T"], ty.span)?;
                validate_collection_key(&resolved[0], ty.span, "Set element")?;
                Ok(Type::Set(Box::new(resolved[0].clone())))
            }
            TypeKind::Named {
                name: "ArrayBuffer",
                args,
            } => {
                self.resolve_type_arguments("ArrayBuffer", args, &[], ty.span)?;
                Ok(Type::ArrayBuffer)
            }
            TypeKind::Named {
                name: "SharedArrayBuffer",
                args,
            } => {
                self.resolve_type_arguments("SharedArrayBuffer", args, &[], ty.span)?;
                Ok(Type::SharedArrayBuffer)
            }
            TypeKind::Named {
                name: "Uint8Array",
                args,
            } => {
                self.resolve_type_arguments("Uint8Array", args, &[], ty.span)?;
                Ok(Type::Uint8Array)
            }
            TypeKind::Named { name, args } if self.model.structs.contains_key(name) => {
                let parameters = self.model.structs[name].type_params.clone();
                let arguments = self.resolve_type_arguments(name, args, &parameters, ty.span)?;
                if parameters.is_empty() {
                    Ok(Type::Struct(name))
                } else {
                    Ok(Type::StructInstance {
                        name,
                        args: arguments,
                    })
                }
            }
            TypeKind::Named { name, args } if self.model.classes.contains_key(name) => {
                let parameters = self.model.classes[name].type_params.clone();
                let arguments = self.resolve_type_arguments(name, args, &parameters, ty.span)?;
                if parameters.is_empty() {
                    Ok(Type::Class(name))
                } else {
                    Ok(Type::ClassInstance {
                        name,
                        args: arguments,
                    })
                }
            }
            TypeKind::Named { name, .. } => Err(SemanticError::new(
                ty.span,
                format!("unknown type `{name}`"),
            )),
            TypeKind::Array(element) => {
                let element = self.resolve_value_type(*element, "array element")?;
                Ok(Type::Array(Box::new(element)))
            }
            TypeKind::Nullable(inner) => {
                let inner = self.resolve_value_type(*inner, "nullable value")?;
                if matches!(inner, Type::Nullable(_) | Type::Null) {
                    return Err(SemanticError::new(
                        ty.span,
                        "nullable types cannot be nested",
                    ));
                }
                Ok(Type::Nullable(Box::new(inner)))
            }
            TypeKind::Union(members) => {
                let mut resolved = Vec::with_capacity(members.len());
                for member in members {
                    resolved.push(self.resolve_value_type(*member, "union member")?);
                }
                Ok(normalize_union(resolved))
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
                    defaults: vec![None; params.len()],
                    return_type: Box::new(return_type),
                }))
            }
        }
    }

    fn resolve_type_arguments<'ast>(
        &self,
        name: &str,
        args: &[TypeRef<'ast, 'src>],
        parameters: &[&'src str],
        span: Span,
    ) -> Result<Vec<Type<'src>>, SemanticError> {
        if args.len() != parameters.len() {
            return Err(SemanticError::new(
                span,
                format!(
                    "type `{name}` expects {} type arguments, found {}",
                    parameters.len(),
                    args.len()
                ),
            ));
        }
        args.iter()
            .map(|argument| self.resolve_value_type(*argument, "type argument"))
            .collect()
    }

    fn push_type_params(&mut self, params: &[Ident<'src>]) -> Result<(), SemanticError> {
        let names = validate_type_params(params)?;
        for parameter in params {
            if self
                .type_parameter_scopes
                .iter()
                .any(|scope| scope.contains(parameter.name))
            {
                return Err(SemanticError::new(
                    parameter.span,
                    format!(
                        "type parameter `{}` shadows an enclosing type parameter",
                        parameter.name
                    ),
                ));
            }
        }
        self.type_parameter_scopes.push(names.into_iter().collect());
        Ok(())
    }

    fn pop_type_params(&mut self) {
        self.type_parameter_scopes
            .pop()
            .expect("type parameter scope was pushed before it was popped");
    }

    fn require_assignable(
        &self,
        expected: &Type<'src>,
        actual: &Type<'src>,
        span: Span,
    ) -> Result<(), SemanticError> {
        if is_type_assignable(expected, actual) {
            Ok(())
        } else {
            Err(SemanticError::new(
                span,
                format!("expected `{expected}`, found `{actual}`"),
            ))
        }
    }

    fn condition_narrowing<'ast>(
        &self,
        condition: &Expr<'ast, 'src>,
    ) -> Result<(Narrowing<'src>, Narrowing<'src>), SemanticError> {
        if let Expr::Unary {
            op: UnaryOp::Not,
            expr,
            ..
        } = condition
        {
            let (then_narrowing, else_narrowing) = self.condition_narrowing(expr)?;
            return Ok((else_narrowing, then_narrowing));
        }
        if let Expr::TypeCheck {
            value: Expr::Ident(ident),
            span,
            ..
        } = condition
        {
            let symbol = self.resolve(ident)?;
            let target = self
                .model
                .type_check_types
                .get(span)
                .cloned()
                .ok_or_else(|| {
                    SemanticError::new(*span, "type guard was not analyzed before narrowing")
                })?;
            let current = self.narrowed_type(symbol.id).unwrap_or(&symbol.ty);
            let remaining = subtract_guarded_type(current, &target);
            return Ok((
                Some((symbol.id, target)),
                remaining.map(|ty| (symbol.id, ty)),
            ));
        }
        let Expr::Binary { op, lhs, rhs, .. } = condition else {
            return Ok((None, None));
        };
        if !matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            return Ok((None, None));
        }
        let ident = match (*lhs, *rhs) {
            (Expr::Ident(ident), Expr::Null(_)) | (Expr::Null(_), Expr::Ident(ident)) => ident,
            _ => return Ok((None, None)),
        };
        let symbol = self.resolve(ident)?;
        let Type::Nullable(inner) = &symbol.ty else {
            return Ok((None, None));
        };
        let narrowing = Some((symbol.id, inner.as_ref().clone()));
        Ok(if *op == BinaryOp::NotEq {
            (narrowing, None)
        } else {
            (None, narrowing)
        })
    }

    fn apply_narrowing(&mut self, narrowing: Option<(SymbolId, Type<'src>)>) {
        if let Some((symbol, ty)) = narrowing {
            self.narrowings
                .last_mut()
                .expect("semantic analyzer always has a narrowing scope")
                .insert(symbol, ty);
        }
    }

    fn current_scope_preserves(&self, narrowing: &Narrowing<'src>) -> bool {
        let Some((symbol, ty)) = narrowing else {
            return false;
        };
        self.narrowings.last().and_then(|scope| scope.get(symbol)) == Some(ty)
    }

    fn narrowed_type(&self, symbol: SymbolId) -> Option<&Type<'src>> {
        self.narrowings
            .iter()
            .rev()
            .find_map(|scope| scope.get(&symbol))
    }

    fn invalidate_assigned_narrowing<'ast>(&mut self, target: &Expr<'ast, 'src>) {
        let Expr::Ident(ident) = target else {
            return;
        };
        let Some(symbol) = self.model.identifier_symbols.get(&ident.span).copied() else {
            return;
        };
        for scope in &mut self.narrowings {
            scope.remove(&symbol);
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
        self.narrowings.push(AHashMap::new());
    }

    fn pop_scope(&mut self) {
        debug_assert!(self.scopes.len() > 1);
        self.scopes.pop();
        self.narrowings.pop();
    }
}

fn validate_type_params<'src>(params: &[Ident<'src>]) -> Result<Vec<&'src str>, SemanticError> {
    let mut names = Vec::with_capacity(params.len());
    let mut seen = AHashSet::new();
    for parameter in params {
        if !seen.insert(parameter.name) {
            return Err(SemanticError::new(
                parameter.span,
                format!("duplicate type parameter `{}`", parameter.name),
            ));
        }
        names.push(parameter.name);
    }
    Ok(names)
}

fn resolve_parameter_defaults<'ast, 'src>(
    params: &[crate::ast::Param<'ast, 'src>],
    types: &[Type<'src>],
) -> Result<Vec<Option<DefaultValue<'src>>>, SemanticError> {
    params
        .iter()
        .zip(types)
        .map(|(param, ty)| {
            let Some(expression) = &param.default else {
                return Ok(None);
            };
            if let Some((_, actual)) = scalar_default_value(expression) {
                if !is_type_assignable(ty, &actual) {
                    return Err(SemanticError::new(
                        expression.span(),
                        format!("default value has type `{actual}`, expected `{ty}`"),
                    ));
                }
            }
            literal_default_value(expression, ty)
                .map(Some)
                .ok_or_else(|| {
                    SemanticError::new(
                        expression.span(),
                        format!(
                            "default value is not a supported literal for parameter type `{ty}`"
                        ),
                    )
                })
        })
        .collect()
}

fn literal_default_value<'ast, 'src>(
    expression: &Expr<'ast, 'src>,
    expected: &Type<'src>,
) -> Option<DefaultValue<'src>> {
    if let Expr::ArrowFunction { span, .. } = expression {
        return matches!(expected, Type::Function(_)).then_some(DefaultValue::Arrow(*span));
    }
    if let Expr::StructLiteral { name, values, .. } = expression {
        let expected_name = nominal_default_name(expected, false)?;
        if name.name != expected_name {
            return None;
        }
        return values
            .iter()
            .map(uncontextualized_default_value)
            .collect::<Option<Vec<_>>>()
            .map(|values| DefaultValue::Struct {
                name: name.name,
                values,
            });
    }
    if let Expr::New { class, args, .. } = expression {
        let expected_name = nominal_default_name(expected, true)?;
        if class.name != expected_name {
            return None;
        }
        return args
            .iter()
            .map(uncontextualized_default_value)
            .collect::<Option<Vec<_>>>()
            .map(|args| DefaultValue::NewClass {
                name: class.name,
                args,
            });
    }
    if let Expr::ArrayLiteral { elements, .. } = expression {
        let element = expected_array_element(expected)?;
        return elements
            .iter()
            .map(|element_value| literal_default_value(element_value, element))
            .collect::<Option<Vec<_>>>()
            .map(DefaultValue::Array);
    }
    let (value, actual) = scalar_default_value(expression)?;
    is_type_assignable(expected, &actual).then_some(value)
}

fn uncontextualized_default_value<'ast, 'src>(
    expression: &Expr<'ast, 'src>,
) -> Option<DefaultValue<'src>> {
    if let Some((value, _)) = scalar_default_value(expression) {
        return Some(value);
    }
    match expression {
        Expr::ArrayLiteral { elements, .. } => elements
            .iter()
            .map(uncontextualized_default_value)
            .collect::<Option<Vec<_>>>()
            .map(DefaultValue::Array),
        Expr::ArrowFunction { span, .. } => Some(DefaultValue::Arrow(*span)),
        Expr::StructLiteral { name, values, .. } => values
            .iter()
            .map(uncontextualized_default_value)
            .collect::<Option<Vec<_>>>()
            .map(|values| DefaultValue::Struct {
                name: name.name,
                values,
            }),
        Expr::New { class, args, .. } => args
            .iter()
            .map(uncontextualized_default_value)
            .collect::<Option<Vec<_>>>()
            .map(|args| DefaultValue::NewClass {
                name: class.name,
                args,
            }),
        _ => None,
    }
}

fn nominal_default_name<'src>(ty: &Type<'src>, class: bool) -> Option<&'src str> {
    match (class, ty) {
        (false, Type::Struct(name)) | (true, Type::Class(name)) => Some(name),
        (false, Type::StructInstance { name, .. }) | (true, Type::ClassInstance { name, .. }) => {
            Some(name)
        }
        (_, Type::Nullable(inner)) => nominal_default_name(inner, class),
        (_, Type::Union(members)) => members
            .iter()
            .find_map(|member| nominal_default_name(member, class)),
        _ => None,
    }
}

fn expected_array_type<'ty, 'src>(ty: &'ty Type<'src>) -> Option<&'ty Type<'src>> {
    match ty {
        Type::Array(_) => Some(ty),
        Type::Nullable(inner) => expected_array_type(inner),
        Type::Union(members) => members.iter().find_map(expected_array_type),
        _ => None,
    }
}

fn expected_array_element<'ty, 'src>(ty: &'ty Type<'src>) -> Option<&'ty Type<'src>> {
    match ty {
        Type::Array(element) => Some(element),
        Type::Nullable(inner) => expected_array_element(inner),
        Type::Union(members) => members.iter().find_map(expected_array_element),
        _ => None,
    }
}

fn scalar_default_value<'ast, 'src>(
    expression: &Expr<'ast, 'src>,
) -> Option<(DefaultValue<'src>, Type<'src>)> {
    match expression {
        Expr::Int(value, _) => Some((DefaultValue::Int(*value), Type::Int)),
        Expr::Float(value, _) => Some((DefaultValue::Float(value.to_bits()), Type::Float)),
        Expr::String(value, _) => Some((DefaultValue::String(value), Type::String)),
        Expr::Bool(value, _) => Some((DefaultValue::Bool(*value), Type::Bool)),
        Expr::Null(_) => Some((DefaultValue::Null, Type::Null)),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
            ..
        } => match *expr {
            Expr::Int(value, _) => Some((DefaultValue::Int(value.wrapping_neg()), Type::Int)),
            Expr::Float(value, _) => Some((DefaultValue::Float((-value).to_bits()), Type::Float)),
            _ => None,
        },
        _ => None,
    }
}

fn applied_nominal_type<'src>(
    name: &'src str,
    parameters: &[&'src str],
    class: bool,
) -> Type<'src> {
    if parameters.is_empty() {
        return if class {
            Type::Class(name)
        } else {
            Type::Struct(name)
        };
    }
    let args = parameters
        .iter()
        .map(|parameter| Type::TypeParameter(parameter))
        .collect();
    if class {
        Type::ClassInstance { name, args }
    } else {
        Type::StructInstance { name, args }
    }
}

fn substitute_type<'src>(
    ty: &Type<'src>,
    substitutions: &AHashMap<&'src str, Type<'src>>,
) -> Type<'src> {
    match ty {
        Type::TypeParameter(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        Type::Array(element) => Type::Array(Box::new(substitute_type(element, substitutions))),
        Type::Map(key, value) => Type::Map(
            Box::new(substitute_type(key, substitutions)),
            Box::new(substitute_type(value, substitutions)),
        ),
        Type::Set(element) => Type::Set(Box::new(substitute_type(element, substitutions))),
        Type::Nullable(inner) => Type::Nullable(Box::new(substitute_type(inner, substitutions))),
        Type::Union(members) => normalize_union(
            members
                .iter()
                .map(|member| substitute_type(member, substitutions))
                .collect(),
        ),
        Type::StructInstance { name, args } => Type::StructInstance {
            name,
            args: args
                .iter()
                .map(|argument| substitute_type(argument, substitutions))
                .collect(),
        },
        Type::ClassInstance { name, args } => Type::ClassInstance {
            name,
            args: args
                .iter()
                .map(|argument| substitute_type(argument, substitutions))
                .collect(),
        },
        Type::Function(signature) => Type::Function(FunctionType {
            params: signature
                .params
                .iter()
                .map(|parameter| substitute_type(parameter, substitutions))
                .collect(),
            defaults: signature.defaults.clone(),
            return_type: Box::new(substitute_type(&signature.return_type, substitutions)),
        }),
        Type::GenericFunction(function) => Type::GenericFunction(GenericFunctionType {
            type_params: function.type_params.clone(),
            signature: FunctionType {
                params: function
                    .signature
                    .params
                    .iter()
                    .map(|parameter| substitute_type(parameter, substitutions))
                    .collect(),
                defaults: function.signature.defaults.clone(),
                return_type: Box::new(substitute_type(
                    &function.signature.return_type,
                    substitutions,
                )),
            },
        }),
        _ => ty.clone(),
    }
}

fn substitutions_for<'src>(
    parameters: &[&'src str],
    arguments: &[Type<'src>],
) -> AHashMap<&'src str, Type<'src>> {
    parameters
        .iter()
        .copied()
        .zip(arguments.iter().cloned())
        .collect()
}

fn method_callable_type<'src>(
    method: &MethodInfo<'src>,
    substitutions: &AHashMap<&'src str, Type<'src>>,
) -> Type<'src> {
    let signature = match substitute_type(&Type::Function(method.signature.clone()), substitutions)
    {
        Type::Function(signature) => signature,
        _ => unreachable!("substituting a function signature preserves its kind"),
    };
    if method.type_params.is_empty() {
        Type::Function(signature)
    } else {
        Type::GenericFunction(GenericFunctionType {
            type_params: method.type_params.clone(),
            signature,
        })
    }
}

fn contains_type_parameter(ty: &Type<'_>, parameters: &AHashSet<&str>) -> bool {
    match ty {
        Type::TypeParameter(name) => parameters.contains(name),
        Type::Array(element) => contains_type_parameter(element, parameters),
        Type::Map(key, value) => {
            contains_type_parameter(key, parameters) || contains_type_parameter(value, parameters)
        }
        Type::Set(element) => contains_type_parameter(element, parameters),
        Type::Nullable(inner) => contains_type_parameter(inner, parameters),
        Type::Union(members) => members
            .iter()
            .any(|member| contains_type_parameter(member, parameters)),
        Type::StructInstance { args, .. } | Type::ClassInstance { args, .. } => args
            .iter()
            .any(|argument| contains_type_parameter(argument, parameters)),
        Type::Function(signature) => {
            signature
                .params
                .iter()
                .any(|parameter| contains_type_parameter(parameter, parameters))
                || contains_type_parameter(&signature.return_type, parameters)
        }
        Type::GenericFunction(function) => {
            function
                .signature
                .params
                .iter()
                .any(|parameter| contains_type_parameter(parameter, parameters))
                || contains_type_parameter(&function.signature.return_type, parameters)
        }
        _ => false,
    }
}

fn infer_type_arguments<'src>(
    pattern: &Type<'src>,
    actual: &Type<'src>,
    parameters: &AHashSet<&'src str>,
    substitutions: &mut AHashMap<&'src str, Type<'src>>,
    span: Span,
) -> Result<(), SemanticError> {
    match (pattern, actual) {
        (Type::TypeParameter(name), actual) if parameters.contains(name) => {
            if let Some(previous) = substitutions.get(name) {
                if is_type_assignable(previous, actual) {
                    return Ok(());
                }
                if is_type_assignable(actual, previous) {
                    substitutions.insert(name, actual.clone());
                } else {
                    return Err(SemanticError::new(
                        span,
                        format!("conflicting inferences for `{name}`: `{previous}` and `{actual}`"),
                    ));
                }
            } else {
                substitutions.insert(name, actual.clone());
            }
        }
        (Type::Array(pattern), Type::Array(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Map(pattern_key, pattern_value), Type::Map(actual_key, actual_value)) => {
            infer_type_arguments(pattern_key, actual_key, parameters, substitutions, span)?;
            infer_type_arguments(pattern_value, actual_value, parameters, substitutions, span)?;
        }
        (Type::Set(pattern), Type::Set(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Nullable(pattern), Type::Nullable(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Nullable(pattern), actual) if actual != &Type::Null => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Union(pattern), Type::Union(actual)) => {
            for actual_member in actual {
                if let Some(pattern_member) = pattern
                    .iter()
                    .find(|candidate| is_type_assignable(candidate, actual_member))
                {
                    infer_type_arguments(
                        pattern_member,
                        actual_member,
                        parameters,
                        substitutions,
                        span,
                    )?;
                }
            }
        }
        (Type::Function(pattern), Type::Function(actual))
            if pattern.params.len() == actual.params.len() =>
        {
            for (pattern, actual) in pattern.params.iter().zip(&actual.params) {
                infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
            }
            infer_type_arguments(
                &pattern.return_type,
                &actual.return_type,
                parameters,
                substitutions,
                span,
            )?;
        }
        (
            Type::StructInstance {
                name: pattern_name,
                args: pattern_args,
            },
            Type::StructInstance {
                name: actual_name,
                args: actual_args,
            },
        )
        | (
            Type::ClassInstance {
                name: pattern_name,
                args: pattern_args,
            },
            Type::ClassInstance {
                name: actual_name,
                args: actual_args,
            },
        ) if pattern_name == actual_name && pattern_args.len() == actual_args.len() => {
            for (pattern, actual) in pattern_args.iter().zip(actual_args) {
                infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn is_type_assignable(expected: &Type<'_>, actual: &Type<'_>) -> bool {
    if expected == actual {
        return true;
    }
    match (expected, actual) {
        (Type::Float, Type::Int) => true,
        (Type::Array(expected), Type::Array(actual)) => is_type_assignable(expected, actual),
        (Type::Map(expected_key, expected_value), Type::Map(actual_key, actual_value)) => {
            expected_key == actual_key && expected_value == actual_value
        }
        (Type::Set(expected), Type::Set(actual)) => expected == actual,
        (Type::Nullable(_), Type::Null) => true,
        (Type::Nullable(expected), Type::Nullable(actual)) => is_type_assignable(expected, actual),
        (Type::Nullable(expected), actual) => is_type_assignable(expected, actual),
        (Type::Union(expected), Type::Union(actual)) => actual.iter().all(|actual| {
            expected
                .iter()
                .any(|expected| is_type_assignable(expected, actual))
        }),
        (Type::Union(expected), actual) => expected
            .iter()
            .any(|expected| is_type_assignable(expected, actual)),
        (expected, Type::Union(actual)) => actual
            .iter()
            .all(|actual| is_type_assignable(expected, actual)),
        (Type::Function(expected), Type::Function(actual))
            if expected.params.len() == actual.params.len() =>
        {
            expected
                .params
                .iter()
                .zip(&actual.params)
                .all(|(expected, actual)| {
                    is_type_assignable(expected, actual) && is_type_assignable(actual, expected)
                })
                && is_type_assignable(&expected.return_type, &actual.return_type)
        }
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
        AssignmentOp::Xor => BinaryOp::Xor,
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

fn nullable_type<'src>(ty: Type<'src>) -> Type<'src> {
    match ty {
        Type::Null | Type::Nullable(_) => ty,
        Type::Union(members)
            if members
                .iter()
                .any(|member| matches!(member, Type::Null | Type::Nullable(_))) =>
        {
            Type::Union(members)
        }
        Type::Union(members) => Type::Nullable(Box::new(Type::Union(members))),
        ty => Type::Nullable(Box::new(ty)),
    }
}

fn validate_collection_key(ty: &Type<'_>, span: Span, context: &str) -> Result<(), SemanticError> {
    let supported = match ty {
        Type::Struct(_) | Type::StructInstance { .. } | Type::TypeParameter(_) | Type::Void => {
            false
        }
        Type::Nullable(inner) => validate_collection_key(inner, span, context).is_ok(),
        Type::Array(_) | Type::Map(_, _) | Type::Set(_) => true,
        Type::Union(members) => members
            .iter()
            .all(|member| validate_collection_key(member, span, context).is_ok()),
        _ => true,
    };
    if supported {
        Ok(())
    } else {
        Err(SemanticError::new(
            span,
            format!("{context} type `{ty}` has no portable identity contract"),
        ))
    }
}

fn buffer_member<'src>(
    property: Ident<'src>,
    span: Span,
    return_type: Type<'src>,
) -> Result<Type<'src>, SemanticError> {
    match property.name {
        "slice" => Ok(Type::Function(FunctionType {
            params: vec![Type::Int, Type::Int],
            defaults: vec![None, Some(DefaultValue::Int(i32::MAX as i64))],
            return_type: Box::new(return_type),
        })),
        _ => Err(SemanticError::new(
            span,
            format!("buffer has no member `{}`", property.name),
        )),
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
        (Type::Nullable(inner), Type::Null) | (Type::Null, Type::Nullable(inner)) => {
            Some(Type::Nullable(inner.clone()))
        }
        (Type::Null, other) | (other, Type::Null) if !matches!(other, Type::Null | Type::Void) => {
            Some(Type::Nullable(Box::new(other.clone())))
        }
        (Type::Nullable(lhs), Type::Nullable(rhs)) => {
            common_type(lhs, rhs).map(|inner| Type::Nullable(Box::new(inner)))
        }
        (Type::Nullable(nullable), other) | (other, Type::Nullable(nullable)) => {
            common_type(nullable, other).map(|inner| Type::Nullable(Box::new(inner)))
        }
        (Type::Array(lhs), Type::Array(rhs)) => {
            common_type(lhs, rhs).map(|element| Type::Array(Box::new(element)))
        }
        _ if !matches!(lhs, Type::Void | Type::GenericFunction(_))
            && !matches!(rhs, Type::Void | Type::GenericFunction(_)) =>
        {
            Some(normalize_union(vec![lhs.clone(), rhs.clone()]))
        }
        _ => None,
    }
}

fn normalize_union<'src>(members: Vec<Type<'src>>) -> Type<'src> {
    let mut flattened = Vec::new();
    for member in members {
        append_union_member(&mut flattened, member);
    }
    if flattened
        .iter()
        .any(|member| matches!(member, Type::Nullable(_)))
    {
        flattened.retain(|member| member != &Type::Null);
    }
    if flattened.len() == 2 {
        let null = flattened.iter().position(|member| member == &Type::Null);
        if let Some(null) = null {
            let inner = flattened.remove(1 - null);
            return Type::Nullable(Box::new(inner));
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one union member remains")
    } else {
        Type::Union(flattened)
    }
}

fn append_union_member<'src>(flattened: &mut Vec<Type<'src>>, member: Type<'src>) {
    if let Type::Union(nested) = member {
        for member in nested {
            append_union_member(flattened, member);
        }
    } else if !flattened.contains(&member) {
        flattened.push(member);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeTypeCategory {
    Number,
    String,
    Bool,
    Null,
    Array,
    Function,
}

fn runtime_type_category(ty: &Type<'_>) -> Option<RuntimeTypeCategory> {
    match ty {
        Type::Int | Type::Float => Some(RuntimeTypeCategory::Number),
        Type::String => Some(RuntimeTypeCategory::String),
        Type::Bool => Some(RuntimeTypeCategory::Bool),
        Type::Null => Some(RuntimeTypeCategory::Null),
        Type::Array(_) => Some(RuntimeTypeCategory::Array),
        Type::Function(_) | Type::GenericFunction(_) => Some(RuntimeTypeCategory::Function),
        _ => None,
    }
}

fn validate_type_guard(
    value: &Type<'_>,
    target: &Type<'_>,
    span: Span,
) -> Result<(), SemanticError> {
    if matches!(target, Type::Union(_) | Type::Nullable(_)) {
        return Err(SemanticError::new(
            span,
            "an `is` guard target must be one concrete member type",
        ));
    }
    let target_category = runtime_type_category(target).ok_or_else(|| {
        SemanticError::new(
            span,
            format!("type `{target}` has no portable runtime type guard"),
        )
    })?;
    let members = runtime_guard_members(value);
    if !members.iter().any(|member| member == target) {
        return Err(SemanticError::new(
            span,
            format!("type `{target}` is not a member of `{value}`"),
        ));
    }
    if members
        .iter()
        .any(|member| member != target && runtime_type_category(member) == Some(target_category))
    {
        return Err(SemanticError::new(
            span,
            format!("type guard `{target}` is runtime-ambiguous within `{value}`"),
        ));
    }
    Ok(())
}

fn runtime_guard_members<'src>(value: &Type<'src>) -> Vec<Type<'src>> {
    match value {
        Type::Union(members) => members.clone(),
        Type::Nullable(inner) => {
            let mut members = runtime_guard_members(inner);
            members.push(Type::Null);
            members
        }
        value => vec![value.clone()],
    }
}

fn subtract_guarded_type<'src>(value: &Type<'src>, target: &Type<'src>) -> Option<Type<'src>> {
    match value {
        Type::Union(members) => {
            let remaining = members
                .iter()
                .filter(|member| *member != target)
                .cloned()
                .collect::<Vec<_>>();
            (!remaining.is_empty()).then(|| normalize_union(remaining))
        }
        Type::Nullable(inner) if target == &Type::Null => Some(inner.as_ref().clone()),
        Type::Nullable(inner) if target == inner.as_ref() => Some(Type::Null),
        Type::Nullable(inner) => subtract_guarded_type(inner, target)
            .map(|remaining| Type::Nullable(Box::new(remaining))),
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
    match (lhs, rhs) {
        (Type::Null, Type::Null)
        | (Type::Null, Type::Nullable(_))
        | (Type::Nullable(_), Type::Null) => true,
        (Type::Nullable(lhs), Type::Nullable(rhs)) => equality_comparable(lhs, rhs),
        (Type::Nullable(lhs), rhs) => equality_comparable(lhs, rhs),
        (lhs, Type::Nullable(rhs)) => equality_comparable(lhs, rhs),
        (Type::Union(lhs), Type::Union(rhs)) => lhs
            .iter()
            .any(|lhs| rhs.iter().any(|rhs| equality_comparable(lhs, rhs))),
        (Type::Union(lhs), rhs) => lhs.iter().any(|lhs| equality_comparable(lhs, rhs)),
        (lhs, Type::Union(rhs)) => rhs.iter().any(|rhs| equality_comparable(lhs, rhs)),
        _ => {
            (lhs == rhs || (lhs.is_numeric() && rhs.is_numeric()))
                && equality_type_supported(lhs)
                && equality_type_supported(rhs)
        }
    }
}

fn equality_type_supported(ty: &Type<'_>) -> bool {
    match ty {
        Type::Union(members) => members.iter().all(equality_type_supported),
        Type::Null
        | Type::Struct(_)
        | Type::StructInstance { .. }
        | Type::Function(_)
        | Type::GenericFunction(_)
        | Type::Void => false,
        _ => true,
    }
}

fn is_stringable(ty: &Type<'_>) -> bool {
    match ty {
        Type::Union(members) => members.iter().all(is_stringable),
        _ => matches!(ty, Type::String | Type::Int | Type::Float | Type::Bool),
    }
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
        BinaryOp::Xor => "^",
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
    fn validates_explicit_math_imul_calls() {
        check("int value=Math.imul(2147483647,2147483647);").unwrap();

        let arity = check("int value=Math.imul(1);").unwrap_err();
        assert!(arity.message.contains("expects two arguments"), "{arity}");

        let argument = check("int value=Math.imul(1,2.0);").unwrap_err();
        assert!(argument.message.contains("expected `int`"), "{argument}");
    }

    #[test]
    fn rejects_wrong_initializer_type() {
        let error = check("int value=\"no\";").unwrap_err();
        assert!(error.message.contains("expected `int`, found `string`"));
    }

    #[test]
    fn validates_nullable_assignments_calls_and_equality() {
        check(
            "T? maybe<T>(bool present,T value){if(present){return value;}return null;}int? value=maybe(true,7);bool present=value!=null;bool same=value==7;",
        )
        .unwrap();
        let error = check("int value=null;").unwrap_err();
        assert!(error.message.contains("expected `int`, found `null`"));
        let error = check("auto value=null;").unwrap_err();
        assert!(error.message.contains("explicit nullable type"));
        let error = check("int value=1;bool same=value==null;").unwrap_err();
        assert!(error.message.contains("cannot be applied"));
        let error = check(
            "struct Pair{int left;int right;}Pair? maybe=null;Pair pair=Pair{1,2};bool same=maybe==pair;",
        )
        .unwrap_err();
        assert!(error.message.contains("cannot be applied"));
    }

    #[test]
    fn validates_union_assignments_arrays_and_generic_inference() {
        check(
            r#"
                T choose<T>(T left,T right){return left;}
                string|int value=1;
                value="ready";
                int|string reordered=value;
                (string|int)[] values=[1,"two",3];
                string|int selected=choose(reordered,"fallback");
                bool same=selected=="ready";
            "#,
        )
        .unwrap();

        let error = check("string|int value=true;").unwrap_err();
        assert!(
            error
                .message
                .contains("expected `string | int`, found `bool`"),
            "{error}"
        );

        let error = check("bool same=1==true;").unwrap_err();
        assert!(error.message.contains("cannot be applied"), "{error}");
    }

    #[test]
    fn narrows_portably_distinguishable_union_members() {
        check(
            r#"
                string describe(string|int value){
                    if(value is string){return value.toUpperCase();}
                    else{return "number-"+value;}
                }
                string invoke((func(int)->int)|string value){
                    if(value is func(int)->int){return "result-"+value(4);}
                    else{return value;}
                }
                string nested(string|int|bool value){
                    if(value is string){return value;}
                    else if(value is int){return "number-"+value;}
                    else if(value){return "yes";}
                    else{return "no";}
                }
                string nullableUnion((string|int)? value){
                    if(value==null){return "none";}
                    else if(value is string){return value.toUpperCase();}
                    else{return "number-"+value;}
                }
            "#,
        )
        .unwrap();

        let ambiguous = check("bool test(int|float value){return value is int;}").unwrap_err();
        assert!(
            ambiguous.message.contains("runtime-ambiguous"),
            "{ambiguous}"
        );
        let absent = check("bool test(string|int value){return value is bool;}").unwrap_err();
        assert!(absent.message.contains("is not a member"), "{absent}");
    }

    #[test]
    fn propagates_narrowing_from_terminating_guard_branches() {
        check(
            r#"
                string nullable(string? value){
                    if(value==null){return "none";}
                    return value.toUpperCase();
                }
                string unionValue(string|int value){
                    if(value is string){return value;}
                    return "number-"+value;
                }
                string negated(string|int value){
                    if(!(value is string)){return "number-"+value;}
                    return value.toUpperCase();
                }
            "#,
        )
        .unwrap();

        let invalidated = check(
            r#"
                string invalid(string|int value){
                    if(value is string){value=1;}
                    else{return "number";}
                    return value.toUpperCase();
                }
            "#,
        )
        .unwrap_err();
        assert!(
            invalidated.message.contains("string | int"),
            "{invalidated}"
        );
    }

    #[test]
    fn validates_scalar_parameter_defaults_and_optional_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"int add(int value,int amount=2){return value+amount;}int direct=add(3);auto callable=add;int indirect=callable(4);"#,
        )
        .unwrap();

        check(
            r#"
                int sum(int[] values=[1,2,3]){return values.length;}
                int nested(int[][] values=[[1],[2,3]]){return values.length;}
                class Bag {
                    int[] values;
                    init(int[] values=[]){this.values=values;}
                }
                int first=sum();
                int second=nested();
                Bag bag=new Bag();

                int apply(int value,func(int)->int transform=(int current)=>current+1){
                    return transform(value);
                }
                int transformed=apply(4);

                struct Point { int x; int y; }
                int pointSum(Point point=Point{2,3}){return point.x+point.y;}
                class Box {
                    int value;
                    init(int value=4){this.value=value;}
                }
                int boxValue(Box box=new Box()){return box.value;}
            "#,
        )
        .unwrap();

        analyze(&program).unwrap();

        check(
            r#"
                T choose<T>(T current,T next,(func(T,T)->bool)? equals=null){
                    if(equals==null){if(current==next){return current;}return next;}
                    func(T,T)->bool compare=equals;
                    if(compare(current,next)){return current;}
                    return next;
                }
                int result=choose(1,2);
            "#,
        )
        .unwrap();
    }

    #[test]
    fn rejects_invalid_parameter_defaults() {
        let arena = Bump::new();
        let wrong_type =
            parse_source(&arena, r#"int value(int input="no"){return input;}"#).unwrap();
        let error = analyze(&wrong_type).unwrap_err();
        assert!(error.message.contains("default value has type `string`"));

        let non_literal = parse_source(&arena, "int value(int input=1+2){return input;}").unwrap();
        let error = analyze(&non_literal).unwrap_err();
        assert!(error
            .message
            .contains("not a supported literal for parameter type `int`"));

        let arena = Bump::new();
        let wrong_array = parse_source(
            &arena,
            r#"int value(int[] input=["no"]){return input.length;}"#,
        )
        .unwrap();
        let error = analyze(&wrong_array).unwrap_err();
        assert!(error
            .message
            .contains("not a supported literal for parameter type `int[]`"));

        let arena = Bump::new();
        let wrong_callback = parse_source(
            &arena,
            r#"int value(func(int)->int transform=(int input)=>"no"){return transform(1);}"#,
        )
        .unwrap();
        let error = analyze(&wrong_callback).unwrap_err();
        assert!(error.message.contains("expected `function(int) -> int`"));

        let arena = Bump::new();
        let wrong_aggregate = parse_source(
            &arena,
            "struct Left{int value;}struct Right{int value;}int read(Left value=Right{1}){return value.value;}",
        )
        .unwrap();
        let error = analyze(&wrong_aggregate).unwrap_err();
        assert!(error
            .message
            .contains("not a supported literal for parameter type `Left`"));
    }

    #[test]
    fn narrows_nullable_values_in_guarded_branches() {
        check(
            "class Box{int value;init(int value){this.value=value;}}int read(Box? box){if(box!=null){return box.value;}return -1;}int readElse(Box? box){if(box==null){return -1;}else{return box.value;}}",
        )
        .unwrap();

        let error = check(
            "class Box{int value;init(int value){this.value=value;}}void bad(Box? box){if(box!=null){box=null;print(box.value);}}",
        )
        .unwrap_err();
        assert!(error.message.contains("Box?"), "{error}");
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
    fn validates_collections_and_binary_memory() {
        check(
            "Map<string,int> values=new Map();values.set(\"x\",1);int? value=values.get(\"x\");Set<int> seen=new Set<int>();seen.add(1);ArrayBuffer buffer=new ArrayBuffer(4);Uint8Array bytes=new Uint8Array(buffer);bytes[0]=255;Uint8Array tail=bytes.subarray(1);",
        )
        .unwrap();

        let bad_key =
            check("struct Point{int x;}Map<Point,int> values=new Map<Point,int>();").unwrap_err();
        assert!(bad_key.message.contains("portable identity contract"));

        let bad_buffer = check("ArrayBuffer buffer=new ArrayBuffer(\"four\");").unwrap_err();
        assert!(bad_buffer
            .message
            .contains("expected `int`, found `string`"));

        let bad_byte = check("Uint8Array bytes=new Uint8Array(4);bytes[0]=\"wrong\";").unwrap_err();
        assert!(bad_byte.message.contains("expected `int`, found `string`"));
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

    #[test]
    fn checks_typed_host_object_members() {
        check(
            r#"
                extern class Element {
                    string textContent;
                    void setAttribute(string name, string value);
                }
                extern class Document {
                    Element createElement(string tag);
                    Element? querySelector(string selector);
                }
                extern Document document;
                Element element=document.createElement("div");
                element.textContent="ready";
                element.setAttribute("data-state","active");
                Element? existing=document.querySelector("main");
            "#,
        )
        .unwrap();

        let error = check(
            "extern class Document{string title;}extern Document document;document.missing();",
        )
        .unwrap_err();
        assert!(error.message.contains("has no member `missing`"));

        let error = check("extern class Document{}Document value=new Document();").unwrap_err();
        assert!(error.message.contains("cannot be constructed"));
    }

    #[test]
    fn infers_generic_functions_and_substitutes_class_members() {
        check(
            "T identity<T>(T value){return value;}int answer=identity(7);string text=identity(\"ok\");class Box<T>{T value;init(T value){this.value=value;}T get(){return this.value;}}Box<int> box=new Box(7);int value=box.get();",
        )
        .unwrap();
    }

    #[test]
    fn rejects_conflicting_generic_inferences() {
        let error =
            check("T choose<T>(T left,T right){return left;}int value=choose(1,\"wrong\");")
                .unwrap_err();
        assert!(error.message.contains("conflicting inferences"));
    }

    #[test]
    fn infers_zero_argument_generics_from_the_expected_return_type() {
        check("class Box<T>{}Box<T> make<T>(){return new Box<T>();}Box<int> box=make();").unwrap();
    }
}
