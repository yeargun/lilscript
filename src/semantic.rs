use std::fmt;

use crate::stable_hash::{StableHashMap as AHashMap, StableHashSet as AHashSet};
use indexmap::IndexMap;

use crate::ast::{
    ArrayBinding, ArrayElement, ArrowBody, AssignmentOp, BinaryOp, ClassDecl, ClassMember,
    ConstructorDecl, Expr, ExternClassMember, ExternDecl, ForInitializer, FunctionDecl, Ident,
    Item, MatchPattern, Program, RecordElement, Stmt, StructDecl, TemplatePart, TypeKind, TypeRef,
    UnaryOp, UpdateOp, VarDecl,
};
use crate::span::Span;
use crate::typed_array::TypedArrayKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeState {
    LocalOnly,
    EscapesToTypedCode,
    EscapesToUntypedBoundary,
}

/// A call whose identity was resolved by semantic analysis rather than by a
/// runtime binding. Keeping this fact in the semantic model prevents later
/// stages from guessing from identifier spelling (and therefore accidentally
/// treating a shadowed user binding as a language intrinsic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinCall {
    Print,
    MathImul,
    ObjectKeys,
    ObjectValues,
    ObjectHasOwn,
    ObjectAssign,
    JsonStringify,
    JsonParse,
    TaskResolve,
    TaskReject,
    TaskAll,
    JsObject,
    JsArray,
    JsUndefined,
    JsTypeOf,
    JsIsNullish,
    JsIsFalse,
    JsIsUndefined,
    JsString,
    JsNumber,
    JsAdd,
    JsMod,
    JsLessThan,
    JsLessThanOrEqual,
    JsGreaterThan,
    JsGreaterThanOrEqual,
    JsAssume,
    JsStrictEqual,
    JsStrictNotEqual,
    JsOr,
    JsAnd,
    JsCall,
    JsConstruct,
    JsInvoke,
    JsApply,
    JsMethod0,
    JsMethod1,
    JsMethod2,
    JsMethod3,
    JsMethodRest,
    JsStaticRest,
    JsGet,
    JsSet,
    JsDelete,
    JsHas,
    JsIn,
    JsBox,
    JsArrayPush,
    JsArrayPop,
    JsArraySlice,
    JsArrayIndexOf,
    JsArraySort,
    JsArraySplice,
    JsArrayConcatApply,
    JsArrayJoin,
    JsArrayShift,
    JsArrayUnshift,
    JsIsArray,
    JsStringSlice,
    JsStringIndexOf,
    JsStringReplace,
    JsStringMatch,
    JsStringSplit,
    JsRegexTest,
    JsRegexExec,
    JsEncodeURI,
    JsEncodeURIComponent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type<'src> {
    Int,
    Float,
    Enum(&'src str),
    String,
    Bool,
    Null,
    Void,
    Array(Box<Type<'src>>),
    Record(Box<Type<'src>>),
    Map(Box<Type<'src>>, Box<Type<'src>>),
    Set(Box<Type<'src>>),
    ArrayBuffer,
    SharedArrayBuffer,
    Int8Array,
    Uint8Array,
    Uint8ClampedArray,
    Int16Array,
    Uint16Array,
    Int32Array,
    Uint32Array,
    Float32Array,
    Float64Array,
    Symbol,
    Regex,
    Task(Box<Type<'src>>),
    Generator(Box<Type<'src>>),
    ModuleNamespace(u32),
    ModuleLoadError,
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
            Self::Enum(name) => f.write_str(name),
            Self::String => f.write_str("string"),
            Self::Bool => f.write_str("bool"),
            Self::Null => f.write_str("null"),
            Self::Void => f.write_str("void"),
            Self::Array(element) => match element.as_ref() {
                Self::Union(_) => write!(f, "({element})[]"),
                _ => write!(f, "{element}[]"),
            },
            Self::Record(value) => write!(f, "Record<{value}>"),
            Self::Map(key, value) => write!(f, "Map<{key}, {value}>"),
            Self::Set(element) => write!(f, "Set<{element}>"),
            Self::ArrayBuffer => f.write_str("ArrayBuffer"),
            Self::SharedArrayBuffer => f.write_str("SharedArrayBuffer"),
            Self::Int8Array => f.write_str("Int8Array"),
            Self::Uint8Array => f.write_str("Uint8Array"),
            Self::Uint8ClampedArray => f.write_str("Uint8ClampedArray"),
            Self::Int16Array => f.write_str("Int16Array"),
            Self::Uint16Array => f.write_str("Uint16Array"),
            Self::Int32Array => f.write_str("Int32Array"),
            Self::Uint32Array => f.write_str("Uint32Array"),
            Self::Float32Array => f.write_str("Float32Array"),
            Self::Float64Array => f.write_str("Float64Array"),
            Self::Symbol => f.write_str("Symbol"),
            Self::Regex => f.write_str("Regex"),
            Self::Task(value) => write!(f, "Task<{value}>"),
            Self::Generator(value) => write!(f, "Generator<{value}>"),
            Self::ModuleNamespace(module) => write!(f, "module#{module}"),
            Self::ModuleLoadError => f.write_str("ModuleLoadError"),
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
            Self::TypeParameter("$js") => f.write_str("JsValue"),
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
    /// The exact unshadowed `JS.undefined()` language primitive.
    Undefined,
    /// A non-parameter identifier default resolved to its exact semantic
    /// binding. Call-site lowering must never recover this from its spelling.
    Symbol(SymbolId),
    /// An identifier default bound to an earlier parameter of the same
    /// callable. The index preserves binding identity through detached
    /// function types so lowering can reuse that call's already-evaluated
    /// actual argument rather than resolving the spelling in the caller.
    Parameter(usize),
    /// Declaration signatures are collected before their default expressions
    /// are analyzed. This span-only placeholder is replaced with `Symbol`
    /// before the completed semantic model is returned.
    PendingIdentifier(Span),
    /// A syntactic `JS.undefined()` candidate awaiting semantic builtin
    /// resolution. Its spelling alone is never accepted as value proof.
    PendingUndefined(Span),
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
    pub base: Option<Type<'src>>,
    pub fields: IndexMap<&'src str, FieldInfo<'src>>,
    pub methods: IndexMap<&'src str, MethodInfo<'src>>,
    declared_fields: IndexMap<&'src str, FieldInfo<'src>>,
    declared_methods: IndexMap<&'src str, MethodInfo<'src>>,
    pub constructor: Option<FunctionType<'src>>,
    pub external: bool,
    pub object: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodInfo<'src> {
    pub owner: &'src str,
    pub type_params: Vec<&'src str>,
    pub signature: FunctionType<'src>,
    pub declared_pure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumInfo<'src> {
    pub name: &'src str,
    pub variants: IndexMap<&'src str, i64>,
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
    builtin_calls: AHashMap<Span, BuiltinCall>,
    optional_present_types: AHashMap<Span, Type<'src>>,
    type_check_types: AHashMap<Span, Type<'src>>,
    binding_types: AHashMap<Span, Type<'src>>,
    identifier_symbols: AHashMap<Span, SymbolId>,
    assigned_symbols: AHashSet<SymbolId>,
    symbols: Vec<Symbol<'src>>,
    structs: AHashMap<&'src str, StructInfo<'src>>,
    classes: AHashMap<&'src str, ClassInfo<'src>>,
    enums: AHashMap<&'src str, EnumInfo<'src>>,
    enum_variant_values: AHashMap<Span, i64>,
    dynamic_import_modules: AHashMap<Span, u32>,
    module_exports: AHashMap<u32, AHashMap<&'src str, &'src str>>,
    used_dynamic_exports: AHashSet<(u32, &'src str)>,
}

impl<'src> SemanticModel<'src> {
    pub fn expression_type(&self, span: Span) -> Option<&Type<'src>> {
        self.expression_types.get(&span)
    }

    pub fn binding_type(&self, span: Span) -> Option<&Type<'src>> {
        self.binding_types.get(&span)
    }

    pub(crate) fn builtin_call(&self, span: Span) -> Option<BuiltinCall> {
        self.builtin_calls.get(&span).copied()
    }

    pub fn identifier_symbol(&self, span: Span) -> Option<SymbolId> {
        self.identifier_symbols.get(&span).copied()
    }

    pub(crate) fn symbol_is_assigned(&self, symbol: SymbolId) -> bool {
        self.assigned_symbols.contains(&symbol)
    }

    pub(crate) fn type_check_type(&self, span: Span) -> Option<&Type<'src>> {
        self.type_check_types.get(&span)
    }

    pub(crate) fn optional_present_type(&self, span: Span) -> Option<&Type<'src>> {
        self.optional_present_types.get(&span)
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

    pub fn is_object(&self, name: &str) -> bool {
        self.classes.get(name).is_some_and(|class| class.object)
    }

    pub(crate) fn class_method_owner(&self, class: &str, method: &str) -> Option<&'src str> {
        self.classes
            .get(class)
            .and_then(|class| class.methods.get(method))
            .map(|method| method.owner)
    }

    pub(crate) fn base_class_name(&self, class: &str) -> Option<&'src str> {
        self.classes
            .get(class)
            .and_then(|class| class.base.as_ref())
            .and_then(class_type_name)
    }

    pub(crate) fn base_constructor(&self, class: &str) -> Option<(&'src str, FunctionType<'src>)> {
        let class = self.classes.get(class)?;
        let base_ty = class.base.as_ref()?;
        let (base_name, base_args) = class_type_parts(base_ty)?;
        let base = self.classes.get(base_name)?;
        let signature = base.constructor.clone()?;
        let substitutions = substitutions_for(&base.type_params, base_args);
        let Type::Function(signature) = substitute_type(&Type::Function(signature), &substitutions)
        else {
            unreachable!("constructor substitution preserves function type")
        };
        Some((base_name, signature))
    }

    pub(crate) fn enum_variant_value(&self, span: Span) -> Option<i64> {
        self.enum_variant_values.get(&span).copied()
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

    pub(crate) fn dynamic_import_module(&self, span: Span) -> Option<u32> {
        self.dynamic_import_modules.get(&span).copied()
    }

    pub(crate) fn dynamic_export_used(&self, module: u32, name: &str) -> bool {
        self.used_dynamic_exports.contains(&(module, name))
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
    type_parameter_scopes: Vec<AHashSet<&'src str>>,
    loop_depth: usize,
    async_depth: usize,
    callable_depth: usize,
    initializing: Option<(SymbolId, usize)>,
    constructor_classes: Vec<Option<&'src str>>,
    generator_contexts: Vec<Option<Type<'src>>>,
}

type Narrowing<'src> = AHashMap<SymbolId, Type<'src>>;

fn empty_narrowing<'src>() -> Narrowing<'src> {
    AHashMap::default()
}

fn merge_narrowing<'src>(mut left: Narrowing<'src>, right: Narrowing<'src>) -> Narrowing<'src> {
    for (symbol, ty) in right {
        left.insert(symbol, ty);
    }
    left
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
                expression_types: AHashMap::default(),
                builtin_calls: AHashMap::default(),
                optional_present_types: AHashMap::default(),
                type_check_types: AHashMap::default(),
                binding_types: AHashMap::default(),
                identifier_symbols: AHashMap::default(),
                assigned_symbols: AHashSet::default(),
                symbols: Vec::new(),
                structs: AHashMap::default(),
                classes: AHashMap::default(),
                enums: AHashMap::default(),
                enum_variant_values: AHashMap::default(),
                dynamic_import_modules: AHashMap::default(),
                module_exports: AHashMap::default(),
                used_dynamic_exports: AHashSet::default(),
            },
            scopes: vec![AHashMap::default()],
            narrowings: vec![AHashMap::default()],
            return_contexts: Vec::new(),
            type_parameter_scopes: Vec::new(),
            loop_depth: 0,
            async_depth: 0,
            callable_depth: 0,
            initializing: None,
            constructor_classes: Vec::new(),
            generator_contexts: Vec::new(),
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
        for import in program.dynamic_imports {
            self.model
                .dynamic_import_modules
                .insert(import.span, import.module);
            let exports = self.model.module_exports.entry(import.module).or_default();
            for export in import.exports {
                exports.insert(export.exported, export.binding);
            }
        }
        self.declare_nominal_types(program)?;
        self.define_enums(program)?;
        self.define_structs(program)?;
        self.define_classes(program)?;
        self.define_extern_classes(program)?;
        self.resolve_class_hierarchies()?;
        self.declare_functions(program)?;

        for item in program.items {
            match item {
                Item::Enum(_) => {}
                Item::Struct(_) => {}
                Item::Class(class) => self.analyze_class(class)?,
                Item::ExternClass(class) => self.analyze_extern_class_defaults(class)?,
                Item::Function(function) => self.analyze_function(function, None)?,
                Item::Extern(extern_decl) => self.analyze_extern_defaults(extern_decl)?,
                Item::ExternGlobal(_) => {}
                Item::Stmt(statement) => self.analyze_stmt(statement)?,
            }
        }

        self.finalize_parameter_default_bindings()?;
        Ok(self.model)
    }

    fn declare_nominal_types<'ast>(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), SemanticError> {
        for item in program.items {
            let (name, type_params, span, is_struct, external, object) = match item {
                Item::Struct(decl) => (
                    decl.name.name,
                    decl.type_params,
                    decl.span,
                    true,
                    false,
                    false,
                ),
                Item::Class(decl) => (
                    decl.name.name,
                    decl.type_params,
                    decl.span,
                    false,
                    false,
                    decl.object,
                ),
                Item::ExternClass(decl) => (
                    decl.name.name,
                    decl.type_params,
                    decl.span,
                    false,
                    true,
                    false,
                ),
                _ => continue,
            };
            let type_params = validate_type_params(type_params)?;

            if self.model.structs.contains_key(name) {
                return Err(SemanticError::new(
                    span,
                    format!("duplicate type declaration `{name}`"),
                ));
            }
            if let Some(existing) = self.model.classes.get(name) {
                if existing.object && object {
                    continue;
                }
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
                        base: None,
                        fields: IndexMap::new(),
                        methods: IndexMap::new(),
                        declared_fields: IndexMap::new(),
                        declared_methods: IndexMap::new(),
                        constructor: None,
                        external,
                        object,
                        span,
                    },
                );
            }
        }
        Ok(())
    }

    fn define_enums<'ast>(&mut self, program: &Program<'ast, 'src>) -> Result<(), SemanticError> {
        for item in program.items {
            let Item::Enum(decl) = item else {
                continue;
            };
            if self.model.enums.contains_key(decl.name.name)
                || self.model.structs.contains_key(decl.name.name)
                || self.model.classes.contains_key(decl.name.name)
            {
                return Err(SemanticError::new(
                    decl.span,
                    format!("duplicate type declaration `{}`", decl.name.name),
                ));
            }
            let mut variants = IndexMap::new();
            for (index, variant) in decl.variants.iter().enumerate() {
                if variants.insert(variant.name, index as i64).is_some() {
                    return Err(SemanticError::new(
                        variant.span,
                        format!(
                            "duplicate variant `{}` in enum `{}`",
                            variant.name, decl.name.name
                        ),
                    ));
                }
            }
            self.model.enums.insert(
                decl.name.name,
                EnumInfo {
                    name: decl.name.name,
                    variants,
                    span: decl.span,
                },
            );
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

            let base = decl
                .base
                .map(|base| self.resolve_value_type(base, "base class"))
                .transpose()?;
            if base
                .as_ref()
                .is_some_and(|base| !matches!(base, Type::Class(_) | Type::ClassInstance { .. }))
            {
                return Err(SemanticError::new(
                    decl.base.expect("checked base").span,
                    "`extends` requires a class type",
                ));
            }

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
                        let signature = self.function_type(method)?;
                        if decl.object {
                            let index = fields.len();
                            fields.insert(
                                method.name.name,
                                FieldInfo {
                                    name: method.name.name,
                                    ty: Type::Function(signature.clone()),
                                    index,
                                    span: method.span,
                                },
                            );
                        }
                        methods.insert(
                            method.name.name,
                            MethodInfo {
                                owner: decl.name.name,
                                type_params: validate_type_params(method.type_params)?,
                                signature,
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

            let merge_object = {
                let info = self
                    .model
                    .classes
                    .get_mut(decl.name.name)
                    .expect("class name was declared in the first semantic pass");
                if decl.object
                    && info.object
                    && self
                        .scopes
                        .last()
                        .is_some_and(|scope| scope.contains_key(decl.name.name))
                {
                    for name in methods.keys() {
                        if info.methods.contains_key(name) || info.fields.contains_key(name) {
                            return Err(SemanticError::new(
                                decl.span,
                                format!("duplicate member `{name}` in object `{}`", decl.name.name),
                            ));
                        }
                    }
                    for (next_index, (name, mut field)) in (info.fields.len()..).zip(fields) {
                        field.index = next_index;
                        info.fields.insert(name, field);
                    }
                    info.methods.extend(methods);
                    info.declared_fields = info.fields.clone();
                    info.declared_methods = info.methods.clone();
                    true
                } else {
                    info.fields = fields;
                    info.methods = methods;
                    info.declared_fields = info.fields.clone();
                    info.declared_methods = info.methods.clone();
                    info.base = base;
                    info.constructor = constructor.clone();
                    info.object = decl.object;
                    false
                }
            };
            self.pop_type_params();
            if merge_object {
                if let Some(&symbol) = self
                    .scopes
                    .last()
                    .and_then(|scope| scope.get(decl.name.name))
                {
                    self.model.identifier_symbols.insert(decl.name.span, symbol);
                    self.model
                        .binding_types
                        .insert(decl.name.span, Type::Class(decl.name.name));
                }
                continue;
            }

            if decl.object {
                self.declare(decl.name, Type::Class(decl.name.name))?;
                continue;
            }

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
            let base = decl
                .base
                .map(|base| self.resolve_value_type(base, "base extern class"))
                .transpose()?;
            if base
                .as_ref()
                .is_some_and(|base| !matches!(base, Type::Class(_) | Type::ClassInstance { .. }))
            {
                return Err(SemanticError::new(
                    decl.base.expect("checked base").span,
                    "`extends` requires a class type",
                ));
            }
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
                                owner: decl.name.name,
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
            info.declared_fields = info.fields.clone();
            info.declared_methods = info.methods.clone();
            info.base = base;
        }
        Ok(())
    }

    fn resolve_class_hierarchies(&mut self) -> Result<(), SemanticError> {
        let names = self.model.classes.keys().copied().collect::<Vec<_>>();
        let mut visiting = AHashSet::default();
        let mut complete = AHashSet::default();
        for name in names {
            self.resolve_class_hierarchy(name, &mut visiting, &mut complete)?;
        }
        Ok(())
    }

    fn resolve_class_hierarchy(
        &mut self,
        name: &'src str,
        visiting: &mut AHashSet<&'src str>,
        complete: &mut AHashSet<&'src str>,
    ) -> Result<(), SemanticError> {
        if complete.contains(name) {
            return Ok(());
        }
        let info = self
            .model
            .classes
            .get(name)
            .cloned()
            .expect("class hierarchy names come from the semantic model");
        if !visiting.insert(name) {
            return Err(SemanticError::new(
                info.span,
                format!("inheritance cycle involving class `{name}`"),
            ));
        }

        let Some(base_ty) = info.base.clone() else {
            visiting.remove(name);
            complete.insert(name);
            return Ok(());
        };
        let (base_name, base_args) = match &base_ty {
            Type::Class(base) => (*base, Vec::new()),
            Type::ClassInstance { name, args } => (*name, args.clone()),
            _ => unreachable!("base classes were validated while defining classes"),
        };
        let base = self.model.classes.get(base_name).cloned().ok_or_else(|| {
            SemanticError::new(info.span, format!("unknown base class `{base_name}`"))
        })?;
        if base.external != info.external {
            return Err(SemanticError::new(
                info.span,
                "internal and extern classes cannot inherit from each other",
            ));
        }
        self.resolve_class_hierarchy(base_name, visiting, complete)?;
        let base = self
            .model
            .classes
            .get(base_name)
            .cloned()
            .expect("resolved base class remains declared");
        let substitutions = substitutions_for(&base.type_params, &base_args);
        let mut fields = IndexMap::new();
        for field in base.fields.values() {
            fields.insert(
                field.name,
                FieldInfo {
                    name: field.name,
                    ty: substitute_type(&field.ty, &substitutions),
                    index: field.index,
                    span: field.span,
                },
            );
        }
        let mut methods = IndexMap::new();
        for (method_name, method) in &base.methods {
            let signature =
                match substitute_type(&Type::Function(method.signature.clone()), &substitutions) {
                    Type::Function(signature) => signature,
                    _ => unreachable!("substituting a method preserves its function type"),
                };
            methods.insert(
                *method_name,
                MethodInfo {
                    owner: method.owner,
                    type_params: method.type_params.clone(),
                    signature,
                    declared_pure: method.declared_pure,
                },
            );
        }
        for field in info.declared_fields.values() {
            if fields.contains_key(field.name) || methods.contains_key(field.name) {
                return Err(SemanticError::new(
                    field.span,
                    format!(
                        "class `{name}` cannot shadow inherited member `{}`",
                        field.name
                    ),
                ));
            }
            let mut field = field.clone();
            field.index = fields.len();
            fields.insert(field.name, field);
        }
        for (method_name, method) in &info.declared_methods {
            if fields.contains_key(method_name) || methods.contains_key(method_name) {
                return Err(SemanticError::new(
                    info.span,
                    format!("class `{name}` cannot override inherited member `{method_name}`"),
                ));
            }
            methods.insert(method_name, method.clone());
        }
        let resolved = self
            .model
            .classes
            .get_mut(name)
            .expect("derived class remains declared");
        resolved.fields = fields;
        resolved.methods = methods;
        visiting.remove(name);
        complete.insert(name);
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
                    let mut names = AHashMap::default();
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
        let declared_return = self.resolve_type(function.return_type, true, "return type")?;
        let return_type = if function.is_async {
            Type::Task(Box::new(declared_return))
        } else if function.is_generator {
            if declared_return == Type::Void {
                return Err(SemanticError::new(
                    function.return_type.span,
                    "generator element type cannot be `void`",
                ));
            }
            Type::Generator(Box::new(declared_return))
        } else {
            declared_return
        };
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
        let requires_super = self
            .model
            .classes
            .get(class.name.name)
            .and_then(|info| info.base.as_ref())
            .and_then(class_type_name)
            .and_then(|base| self.model.classes.get(base))
            .is_some_and(|base| base.constructor.is_some());
        if requires_super
            && !class
                .members
                .iter()
                .any(|member| matches!(member, ClassMember::Constructor(_)))
        {
            self.pop_type_params();
            return Err(SemanticError::new(
                class.span,
                format!(
                    "class `{}` must declare `init` and call its base constructor",
                    class.name.name
                ),
            ));
        }
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
        let class_info = self
            .model
            .classes
            .get(class_name)
            .cloned()
            .expect("constructors belong to declared classes");
        let super_calls = count_super_calls(constructor.body);
        match class_info.base.as_ref().and_then(class_type_name) {
            None if super_calls != 0 => {
                return Err(SemanticError::new(
                    constructor.span,
                    "`super` is only valid in a derived class constructor",
                ));
            }
            Some(base_name) => {
                if super_calls > 1 {
                    return Err(SemanticError::new(
                        constructor.span,
                        "a derived constructor may call `super` only once",
                    ));
                }
                let base_has_constructor = self
                    .model
                    .classes
                    .get(base_name)
                    .is_some_and(|base| base.constructor.is_some());
                if base_has_constructor && super_calls == 0 {
                    return Err(SemanticError::new(
                        constructor.span,
                        format!(
                            "derived constructor must begin with `super(...)` for `{base_name}`"
                        ),
                    ));
                }
                if super_calls != 0
                    && !matches!(constructor.body.first(), Some(Stmt::SuperCall { .. }))
                {
                    return Err(SemanticError::new(
                        constructor.span,
                        "`super(...)` must be the first statement in a derived constructor",
                    ));
                }
            }
            None => {}
        }
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
        self.constructor_classes.push(Some(class_name));
        self.generator_contexts.push(None);
        for statement in constructor.body {
            self.analyze_stmt(statement)?;
        }
        self.generator_contexts.pop();
        self.constructor_classes.pop();
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

        let generator_element = if function.is_generator {
            match signature.return_type.as_ref() {
                Type::Generator(value) => Some((**value).clone()),
                _ => unreachable!("generator signatures return Generator<T>"),
            }
        } else {
            None
        };
        let body_return_type = if function.is_async {
            match signature.return_type.as_ref() {
                Type::Task(value) => (**value).clone(),
                _ => unreachable!("async signatures return Task<T>"),
            }
        } else if function.is_generator {
            Type::Void
        } else {
            (*signature.return_type).clone()
        };
        self.return_contexts.push(ReturnContext::Declared {
            ty: body_return_type,
            saw_return: false,
        });
        self.async_depth += usize::from(function.is_async);
        self.generator_contexts.push(generator_element);
        for statement in function.body {
            self.analyze_stmt(statement)?;
        }
        self.generator_contexts.pop();
        self.async_depth -= usize::from(function.is_async);
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

    fn finalize_parameter_default_bindings(&mut self) -> Result<(), SemanticError> {
        let bindings = &self.model.identifier_symbols;
        let builtins = &self.model.builtin_calls;
        for ty in self.model.expression_types.values_mut() {
            finalize_default_bindings_in_type(ty, bindings, builtins)?;
        }
        for ty in self.model.optional_present_types.values_mut() {
            finalize_default_bindings_in_type(ty, bindings, builtins)?;
        }
        for ty in self.model.type_check_types.values_mut() {
            finalize_default_bindings_in_type(ty, bindings, builtins)?;
        }
        for ty in self.model.binding_types.values_mut() {
            finalize_default_bindings_in_type(ty, bindings, builtins)?;
        }
        for symbol in &mut self.model.symbols {
            finalize_default_bindings_in_type(&mut symbol.ty, bindings, builtins)?;
        }
        for info in self.model.structs.values_mut() {
            for field in info.fields.values_mut() {
                finalize_default_bindings_in_type(&mut field.ty, bindings, builtins)?;
            }
        }
        for info in self.model.classes.values_mut() {
            if let Some(base) = &mut info.base {
                finalize_default_bindings_in_type(base, bindings, builtins)?;
            }
            for field in info
                .fields
                .values_mut()
                .chain(info.declared_fields.values_mut())
            {
                finalize_default_bindings_in_type(&mut field.ty, bindings, builtins)?;
            }
            for method in info
                .methods
                .values_mut()
                .chain(info.declared_methods.values_mut())
            {
                finalize_default_bindings_in_signature(&mut method.signature, bindings, builtins)?;
            }
            if let Some(constructor) = &mut info.constructor {
                finalize_default_bindings_in_signature(constructor, bindings, builtins)?;
            }
        }
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
            Stmt::ArrayDestructure {
                bindings, value, ..
            } => {
                let actual = self.analyze_expr(value, None)?;
                let Type::Array(element) = actual else {
                    return Err(SemanticError::new(
                        value.span(),
                        format!("array destructuring requires an array, found `{actual}`"),
                    ));
                };
                for binding in *bindings {
                    match binding {
                        ArrayBinding::Hole(_) => {}
                        ArrayBinding::Name(name) => {
                            self.declare(*name, Type::Nullable(element.clone()))?;
                        }
                        ArrayBinding::Rest(name) => {
                            self.declare(*name, Type::Array(element.clone()))?;
                        }
                    }
                }
                Ok(())
            }
            Stmt::RecordDestructure {
                bindings,
                rest,
                value,
                ..
            } => {
                let actual = self.analyze_expr(value, None)?;
                let Type::Record(element) = actual else {
                    return Err(SemanticError::new(
                        value.span(),
                        format!("record destructuring requires a record, found `{actual}`"),
                    ));
                };
                let mut keys = AHashSet::default();
                for binding in *bindings {
                    if !keys.insert(decode_source_string(binding.key.name)) {
                        return Err(SemanticError::new(
                            binding.key.span,
                            format!("duplicate record binding key `{}`", binding.key.name),
                        ));
                    }
                    self.declare(binding.name, Type::Nullable(element.clone()))?;
                }
                if let Some(rest) = rest {
                    self.declare(*rest, Type::Record(element.clone()))?;
                }
                Ok(())
            }
            Stmt::Expr(expr) => {
                self.analyze_expr(expr, None)?;
                Ok(())
            }
            Stmt::Return { value, span } => self.analyze_return(value.as_ref(), *span),
            Stmt::Throw { value, .. } => {
                let thrown = self.analyze_expr(value, None)?;
                if thrown == Type::Void {
                    return Err(SemanticError::new(
                        value.span(),
                        "cannot throw a `void` expression",
                    ));
                }
                Ok(())
            }
            Stmt::SuperCall { args, span } => self.analyze_super_call(args, *span),
            Stmt::Yield {
                value,
                delegate,
                span,
            } => self.analyze_yield(value, *delegate, *span),
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                self.push_scope();
                for statement in *body {
                    self.analyze_stmt(statement)?;
                }
                self.pop_scope();
                if let Some(clause) = catch {
                    self.push_scope();
                    if let Some(binding) = clause.binding {
                        let ty = if binding.ty.is_auto() {
                            Type::TypeParameter("$js")
                        } else {
                            self.resolve_value_type(binding.ty, "catch binding")?
                        };
                        if !is_js_value(&ty) {
                            return Err(SemanticError::new(
                                binding.ty.span,
                                format!(
                                    "catch bindings must use `auto` or `JsValue`, found `{ty}`"
                                ),
                            ));
                        }
                        self.declare(binding.name, Type::TypeParameter("$js"))?;
                    }
                    for statement in clause.body {
                        self.analyze_stmt(statement)?;
                    }
                    self.pop_scope();
                }
                if let Some(finally) = finally {
                    self.push_scope();
                    for statement in *finally {
                        self.analyze_stmt(statement)?;
                    }
                    self.pop_scope();
                }
                Ok(())
            }
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
                let mut else_survives = else_branch.is_none() && !else_narrowing.is_empty();
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
            Stmt::ForIn {
                key_type,
                key,
                object,
                body,
                ..
            } => {
                self.push_scope();
                let key_ty = self.resolve_value_type(*key_type, "for-in key")?;
                if key_ty != Type::String {
                    return Err(SemanticError::new(
                        key_type.span,
                        format!("for-in keys must have type `string`, found `{key_ty}`"),
                    ));
                }
                let object_ty = self.analyze_expr(object, None)?;
                if !is_js_value(&object_ty) && !matches!(object_ty, Type::Record(_)) {
                    return Err(SemanticError::new(
                        object.span(),
                        format!("for-in requires a `JsValue` or `Record<T>` object, found `{object_ty}`"),
                    ));
                }
                self.declare(*key, Type::String)?;
                self.loop_depth += 1;
                self.analyze_stmt(body)?;
                self.loop_depth -= 1;
                self.pop_scope();
                Ok(())
            }
            Stmt::ForOf {
                element_type,
                element,
                iterable,
                body,
                inline,
                ..
            } => {
                self.push_scope();
                let declared = self.resolve_value_type(*element_type, "for-of element")?;
                let iterable_type = self.analyze_expr(iterable, None)?;
                if *inline {
                    if iterable.const_list_literals().is_none() {
                        return Err(SemanticError::new(
                            iterable.span(),
                            "`inline for` requires a constant array literal of int, float, string, or bool values",
                        ));
                    }
                    if statement_contains_loop_control(body, false) {
                        return Err(SemanticError::new(
                            body.span(),
                            "`inline for` cannot contain `break` or `continue`",
                        ));
                    }
                }
                let actual = match iterable_type {
                    Type::Array(element) => *element,
                    Type::Generator(element) => *element,
                    ty if TypedArrayKind::from_type(&ty).is_some() => {
                        if TypedArrayKind::from_type(&ty)
                            .is_some_and(TypedArrayKind::element_is_float)
                        {
                            Type::Float
                        } else {
                            Type::Int
                        }
                    }
                    other => {
                        return Err(SemanticError::new(
                            iterable.span(),
                            format!(
                                "for-of requires an array or typed array, or Generator<T>, found `{other}`"
                            ),
                        ));
                    }
                };
                self.require_assignable(&declared, &actual, element_type.span)?;
                self.declare(*element, declared)?;
                if *inline {
                    self.analyze_stmt(body)?;
                } else {
                    self.loop_depth += 1;
                    self.analyze_stmt(body)?;
                    self.loop_depth -= 1;
                }
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

    fn analyze_super_call<'ast>(
        &mut self,
        args: &[Expr<'ast, 'src>],
        span: Span,
    ) -> Result<(), SemanticError> {
        let class_name = self
            .constructor_classes
            .last()
            .copied()
            .flatten()
            .ok_or_else(|| {
                SemanticError::new(span, "`super` is only valid in a derived class constructor")
            })?;
        let class = self
            .model
            .classes
            .get(class_name)
            .cloned()
            .expect("constructor class metadata exists");
        let base_ty = class.base.ok_or_else(|| {
            SemanticError::new(span, "`super` is only valid in a derived class constructor")
        })?;
        let (base_name, base_args) =
            class_type_parts(&base_ty).expect("derived class bases are class types");
        let base = self
            .model
            .classes
            .get(base_name)
            .cloned()
            .expect("base class was resolved");
        let substitutions = substitutions_for(&base.type_params, base_args);
        let signature = base.constructor.map(|signature| {
            match substitute_type(&Type::Function(signature), &substitutions) {
                Type::Function(signature) => signature,
                _ => unreachable!("constructor substitution preserves function type"),
            }
        });
        match signature {
            Some(signature) => {
                self.analyze_call(&Type::Function(signature), args, span, None)?;
            }
            None if !args.is_empty() => {
                return Err(SemanticError::new(
                    span,
                    format!("implicit base constructor `{base_name}` expects no arguments"),
                ));
            }
            None => {}
        }
        Ok(())
    }

    fn analyze_yield<'ast>(
        &mut self,
        value: &Expr<'ast, 'src>,
        delegate: bool,
        span: Span,
    ) -> Result<(), SemanticError> {
        let expected = self
            .generator_contexts
            .last()
            .and_then(Clone::clone)
            .ok_or_else(|| SemanticError::new(span, "`yield` is only valid inside a generator"))?;
        if delegate {
            let iterable = self.analyze_expr(value, None)?;
            let actual = match iterable {
                Type::Array(element) | Type::Generator(element) => *element,
                ty if TypedArrayKind::from_type(&ty).is_some() => {
                    if TypedArrayKind::from_type(&ty).is_some_and(TypedArrayKind::element_is_float)
                    {
                        Type::Float
                    } else {
                        Type::Int
                    }
                }
                other => {
                    return Err(SemanticError::new(
                        value.span(),
                        format!("`yield*` requires an iterable, found `{other}`"),
                    ));
                }
            };
            self.require_assignable(&expected, &actual, value.span())
        } else {
            let actual = self.analyze_expr(value, Some(&expected))?;
            self.require_assignable(&expected, &actual, value.span())
        }
    }

    fn analyze_var_decl<'ast>(&mut self, decl: &VarDecl<'ast, 'src>) -> Result<(), SemanticError> {
        if decl.initializer.is_none() {
            return Err(SemanticError::new(
                decl.span,
                "variable declarations require an initializer",
            ));
        }
        if decl.ty.is_auto() {
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
            let mut ty = inferred;
            // A named callable carries declaration-stable default metadata, so
            // an inferred alias can retain its optional-call contract. Defaults
            // originating in a computed first-class value are erased when that
            // value enters mutable storage; otherwise a later call could cache
            // an initializer's defaults independently of the stored callable.
            if !matches!(initializer, Expr::Ident(_)) {
                strip_parameter_defaults_from_type(&mut ty);
            }
            self.declare(decl.name, ty)?;
            return Ok(());
        }

        let declared = self.resolve_value_type(decl.ty, "variable")?;
        let mut binding_ty = declared.clone();
        strip_parameter_defaults_from_type(&mut binding_ty);
        let id = self.declare(decl.name, binding_ty)?;
        let previous = self.initializing;
        self.initializing = Some((id, self.callable_depth));
        let analyzed = if let Some(initializer) = &decl.initializer {
            let actual = self.analyze_expr(initializer, Some(&declared));
            self.initializing = previous;
            let actual = actual?;
            self.require_assignable(&declared, &actual, initializer.span())
        } else {
            self.initializing = previous;
            Ok(())
        };
        analyzed?;
        let referenced = self
            .model
            .identifier_symbols
            .values()
            .filter(|symbol| **symbol == id)
            .count()
            > 1;
        if referenced {
            self.model.assigned_symbols.insert(id);
        }
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
        if matches!(
            self.return_contexts.last(),
            Some(ReturnContext::Declared { .. })
        ) {
            let expected = expected
                .as_ref()
                .expect("declared returns have an expected type");
            if !self.is_assignable(expected, &actual) {
                return Err(SemanticError::new(
                    span,
                    format!("expected return type `{expected}`, found `{actual}`"),
                ));
            }
        }

        let context = self
            .return_contexts
            .last_mut()
            .expect("return context was checked above");
        match context {
            ReturnContext::Declared { saw_return, .. } => {
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
            Expr::DynamicImport { span, .. } => {
                let module = self
                    .model
                    .dynamic_import_modules
                    .get(span)
                    .copied()
                    .ok_or_else(|| {
                        SemanticError::new(
                            *span,
                            "dynamic imports require file-based compilation so their module interface can be resolved",
                        )
                    })?;
                Type::Task(Box::new(Type::ModuleNamespace(module)))
            }
            Expr::Ident(ident) => {
                let (id, declared) = {
                    let symbol = self.resolve(ident)?;
                    (symbol.id, symbol.ty.clone())
                };
                if let Some((initializing, depth)) = self.initializing {
                    if id == initializing && self.callable_depth == depth {
                        return Err(SemanticError::new(
                            ident.span,
                            format!(
                                "cannot read `{}` in its own initializer; nest the reference in a function",
                                ident.name
                            ),
                        ));
                    }
                }
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
                    let actual = match element {
                        ArrayElement::Value(value) => self.analyze_expr(value, expected_element)?,
                        ArrayElement::Spread { value, .. } => {
                            let expected_array = expected_element
                                .map(|element| Type::Array(Box::new(element.clone())));
                            let spread = self.analyze_expr(value, expected_array.as_ref())?;
                            let Type::Array(actual) = spread else {
                                return Err(SemanticError::new(
                                    value.span(),
                                    format!("array spread requires an array, found `{spread}`"),
                                ));
                            };
                            *actual
                        }
                    };
                    if let Some(expected) = expected_element {
                        // A contextual literal creates fresh storage with the
                        // declared element representation, so one-way element
                        // conversion is safe here even though mutable array
                        // references themselves are invariant.
                        self.require_assignable(expected, &actual, element.span())?;
                        continue;
                    }
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
            Expr::ObjectLiteral { entries, .. } => {
                let mut seen = AHashSet::default();
                for entry in *entries {
                    match entry {
                        RecordElement::Entry(entry) => {
                            if !seen.insert(decode_source_string(entry.key.name)) {
                                return Err(SemanticError::new(
                                    entry.key.span,
                                    format!("duplicate object key `{}`", entry.key.name),
                                ));
                            }
                            self.analyze_expr(&entry.value, Some(&Type::TypeParameter("$js")))?;
                        }
                        RecordElement::Spread { span, .. } => {
                            return Err(SemanticError::new(
                                *span,
                                "ordinary object spread is not supported yet",
                            ));
                        }
                    }
                }
                Type::TypeParameter("$js")
            }
            Expr::RecordLiteral { entries, span } => {
                if expected.is_some_and(is_js_value_or_nullable_js_value) {
                    let mut seen = AHashSet::default();
                    for entry in *entries {
                        match entry {
                            RecordElement::Entry(entry) => {
                                if !seen.insert(decode_source_string(entry.key.name)) {
                                    return Err(SemanticError::new(
                                        entry.key.span,
                                        format!("duplicate record key `{}`", entry.key.name),
                                    ));
                                }
                                self.analyze_expr(&entry.value, Some(&Type::TypeParameter("$js")))?;
                            }
                            RecordElement::Spread { value, .. } => {
                                self.analyze_expr(value, Some(&Type::TypeParameter("$js")))?;
                            }
                        }
                    }
                    Type::TypeParameter("$js")
                } else {
                    let expected_value = match expected {
                        Some(Type::Record(value)) => Some(value.as_ref()),
                        _ => None,
                    };
                    let mut seen = AHashSet::default();
                    let mut value_type = expected_value.cloned();
                    for entry in *entries {
                        let actual = match entry {
                            RecordElement::Entry(entry) => {
                                if !seen.insert(decode_source_string(entry.key.name)) {
                                    return Err(SemanticError::new(
                                        entry.key.span,
                                        format!("duplicate record key `{}`", entry.key.name),
                                    ));
                                }
                                self.analyze_expr(&entry.value, expected_value)?
                            }
                            RecordElement::Spread { value, .. } => {
                                let expected_record = expected_value
                                    .map(|value| Type::Record(Box::new(value.clone())));
                                let spread = self.analyze_expr(value, expected_record.as_ref())?;
                                let Type::Record(actual) = spread else {
                                    return Err(SemanticError::new(
                                        value.span(),
                                        format!(
                                            "record spread requires a record, found `{spread}`"
                                        ),
                                    ));
                                };
                                *actual
                            }
                        };
                        if let Some(expected) = expected_value {
                            // Like an array literal, a record literal creates fresh
                            // storage. Each copied value may be widened into the declared
                            // slot type without making mutable Record references covariant.
                            if !self.is_assignable(expected, &actual) {
                                return Err(SemanticError::new(
                                    entry.span(),
                                    format!(
                                        "expected `Record<{expected}>`, found `Record<{actual}>`"
                                    ),
                                ));
                            }
                            continue;
                        }
                        value_type = match value_type {
                            Some(ref previous) => {
                                Some(common_type(previous, &actual).ok_or_else(|| {
                                    SemanticError::new(
                                        entry.span(),
                                        format!(
                                        "record value has type `{actual}`, expected `{previous}`"
                                    ),
                                    )
                                })?)
                            }
                            None => Some(actual),
                        };
                    }
                    let value_type = value_type.ok_or_else(|| {
                    SemanticError::new(
                        *span,
                        "cannot infer the value type of an empty record; add an explicit `Record<T>` type",
                    )
                })?;
                    Type::Record(Box::new(value_type))
                }
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
                let (field_types, result_type) = if info.type_params.is_empty() {
                    (
                        info.fields
                            .values()
                            .map(|field| field.ty.clone())
                            .collect::<Vec<_>>(),
                        Type::Struct(name.name),
                    )
                } else {
                    let Some(Type::StructInstance {
                        name: expected_name,
                        args,
                    }) = expected
                    else {
                        return Err(SemanticError::new(
                            *span,
                            format!(
                                "generic struct literal `{}` requires a contextual `{}<...>` type",
                                name.name, name.name
                            ),
                        ));
                    };
                    if *expected_name != name.name || args.len() != info.type_params.len() {
                        return Err(SemanticError::new(
                            *span,
                            format!(
                                "generic struct literal `{}` requires a contextual `{}<...>` type",
                                name.name, name.name
                            ),
                        ));
                    }
                    let substitutions = substitutions_for(&info.type_params, args);
                    (
                        info.fields
                            .values()
                            .map(|field| substitute_type(&field.ty, &substitutions))
                            .collect::<Vec<_>>(),
                        Type::StructInstance {
                            name: name.name,
                            args: args.clone(),
                        },
                    )
                };
                for (value, field_type) in values.iter().zip(&field_types) {
                    let actual = self.analyze_expr(value, Some(field_type))?;
                    self.require_assignable(field_type, &actual, value.span())?;
                }
                result_type
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
                    if info.object {
                        return Err(SemanticError::new(
                            *span,
                            format!("object `{}` cannot be constructed with `new`", class.name),
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
                    let mut substitutions = AHashMap::default();
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
                    let mut actual_args = Vec::with_capacity(args.len());
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
                        actual_args.push(actual);
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
                    for ((arg, pattern), actual) in args.iter().zip(params).zip(&actual_args) {
                        let resolved = substitute_type(pattern, &substitutions);
                        self.require_assignable(&resolved, actual, arg.span())?;
                    }
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
                object: Expr::Ident(enum_name),
                property,
                span,
            } if self.model.enums.contains_key(enum_name.name) => {
                let value = self
                    .model
                    .enums
                    .get(enum_name.name)
                    .and_then(|info| info.variants.get(property.name))
                    .copied()
                    .ok_or_else(|| {
                        SemanticError::new(
                            property.span,
                            format!(
                                "enum `{}` has no variant `{}`",
                                enum_name.name, property.name
                            ),
                        )
                    })?;
                self.model.enum_variant_values.insert(*span, value);
                Type::Enum(enum_name.name)
            }
            Expr::Member {
                object,
                property,
                span,
            } => self.analyze_member(object, *property, *span)?,
            Expr::OptionalMember {
                object,
                property,
                span,
            } => {
                let object_type = self.analyze_expr(object, None)?;
                let Type::Nullable(inner) = object_type else {
                    return Err(SemanticError::new(
                        object.span(),
                        format!(
                            "optional access requires a nullable receiver, found `{object_type}`"
                        ),
                    ));
                };
                let member = self.analyze_member_type(*inner, *property, *span)?;
                self.model
                    .optional_present_types
                    .insert(*span, member.clone());
                optional_result_type(member, *span)?
            }
            Expr::Call { callee, args, span } => {
                if let Some((builtin, result)) =
                    self.analyze_static_namespace_call(callee, args, *span, expected)?
                {
                    self.model.builtin_calls.insert(*span, builtin);
                    result
                } else if self.builtin_namespace_is_unshadowed("Math")
                    && matches!(
                        callee,
                        Expr::Member {
                            object,
                            property: Ident { name: "imul", .. },
                            ..
                        } if matches!(object, Expr::Ident(Ident { name: "Math", .. }))
                    )
                {
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
                    self.model
                        .builtin_calls
                        .insert(*span, BuiltinCall::MathImul);
                    Type::Int
                } else if self.builtin_namespace_is_unshadowed("print")
                    && matches!(callee, Expr::Ident(Ident { name: "print", .. }))
                {
                    if args.len() != 1 {
                        return Err(SemanticError::new(
                            *span,
                            format!("`print` expects one argument, found {}", args.len()),
                        ));
                    }
                    self.analyze_expr(&args[0], None)?;
                    self.model.builtin_calls.insert(*span, BuiltinCall::Print);
                    Type::Void
                } else if let Expr::Member {
                    object, property, ..
                } = callee
                {
                    if matches!(property.name, "then" | "catch" | "finally") {
                        let receiver = self.analyze_expr(object, None)?;
                        if let Type::Task(value) = receiver {
                            self.analyze_task_call(property.name, *value, args, *span)?
                        } else {
                            let callee_type = self.analyze_expr(callee, None)?;
                            self.analyze_call(&callee_type, args, *span, expected)?
                        }
                    } else {
                        match property.name {
                            "map" => self.analyze_array_map(object, args, *span)?,
                            "filter" => self.analyze_array_filter(object, args, *span)?,
                            "forEach" => self.analyze_array_for_each(object, args, *span)?,
                            "reduce" => self.analyze_array_reduce(object, args, *span)?,
                            "some" | "every" => self.analyze_array_predicate(
                                object,
                                args,
                                property.name,
                                *span,
                                Type::Bool,
                            )?,
                            "findIndex" => self.analyze_array_predicate(
                                object,
                                args,
                                property.name,
                                *span,
                                Type::Int,
                            )?,
                            _ => {
                                let callee_type = self.analyze_expr(callee, None)?;
                                self.analyze_call(&callee_type, args, *span, expected)?
                            }
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
            Expr::Await { task, span } => {
                if self.async_depth == 0 {
                    return Err(SemanticError::new(
                        *span,
                        "`await` is only valid inside an async function or method",
                    ));
                }
                let expected_task = expected.map(|value| Type::Task(Box::new(value.clone())));
                let task_type = self.analyze_expr(task, expected_task.as_ref())?;
                let Type::Task(value) = task_type else {
                    return Err(SemanticError::new(
                        task.span(),
                        format!("`await` requires a `Task<T>`, found `{task_type}`"),
                    ));
                };
                *value
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lhs_type = self.analyze_expr(lhs, None)?;
                let rhs_type = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let (then_narrowing, else_narrowing) = self.condition_narrowing(lhs)?;
                    let rhs_narrowing = if *op == BinaryOp::And {
                        then_narrowing
                    } else {
                        else_narrowing
                    };
                    self.push_scope();
                    self.apply_narrowing(rhs_narrowing);
                    let analyzed = self.analyze_expr(rhs, Some(&Type::Bool))?;
                    self.pop_scope();
                    analyzed
                } else {
                    let rhs_expected = if *op == BinaryOp::Nullish {
                        nullish_present_type(&lhs_type).or(expected)
                    } else {
                        None
                    };
                    self.analyze_expr(rhs, rhs_expected)?
                };
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
                if is_js_value(&object_type) {
                    let index_type = self.analyze_expr(index, None)?;
                    if !is_js_index_type(&index_type) {
                        return Err(SemanticError::new(
                            index.span(),
                            format!(
                                "a `JsValue` index must be numeric, `string`, or `JsValue`, found `{index_type}`"
                            ),
                        ));
                    }
                } else {
                    let expected_index = index_key_type(&object_type).ok_or_else(|| {
                        SemanticError::new(
                            *span,
                            format!("cannot index a value of type `{object_type}`"),
                        )
                    })?;
                    let index_type = self.analyze_expr(index, Some(&expected_index))?;
                    self.require_assignable(&expected_index, &index_type, index.span())?;
                }
                index_value_type(&object_type, false).ok_or_else(|| {
                    SemanticError::new(
                        *span,
                        format!("cannot index a value of type `{object_type}`"),
                    )
                })?
            }
            Expr::OptionalIndex {
                object,
                index,
                span,
            } => {
                let object_type = self.analyze_expr(object, None)?;
                let Type::Nullable(inner) = object_type else {
                    return Err(SemanticError::new(
                        object.span(),
                        format!(
                            "optional indexing requires a nullable receiver, found `{object_type}`"
                        ),
                    ));
                };
                if is_js_value(&inner) {
                    let index_type = self.analyze_expr(index, None)?;
                    if !is_js_index_type(&index_type) {
                        return Err(SemanticError::new(
                            index.span(),
                            format!(
                                "a `JsValue` index must be numeric, `string`, or `JsValue`, found `{index_type}`"
                            ),
                        ));
                    }
                } else {
                    let expected_index = index_key_type(&inner).ok_or_else(|| {
                        SemanticError::new(*span, format!("cannot index a value of type `{inner}`"))
                    })?;
                    let index_type = self.analyze_expr(index, Some(&expected_index))?;
                    self.require_assignable(&expected_index, &index_type, index.span())?;
                }
                let element = index_value_type(&inner, false).ok_or_else(|| {
                    SemanticError::new(*span, format!("cannot index a value of type `{inner}`"))
                })?;
                self.model
                    .optional_present_types
                    .insert(*span, element.clone());
                optional_result_type(element, *span)?
            }
            Expr::If {
                condition,
                then_value,
                else_value,
                span,
            } => {
                let condition_type = self.analyze_expr(condition, Some(&Type::Bool))?;
                self.require_assignable(&Type::Bool, &condition_type, condition.span())?;
                let (then_narrowing, else_narrowing) = self.condition_narrowing(condition)?;
                self.push_scope();
                self.apply_narrowing(then_narrowing);
                let then_type = self.analyze_expr(then_value, expected)?;
                self.pop_scope();
                self.push_scope();
                self.apply_narrowing(else_narrowing);
                let else_type = self.analyze_expr(else_value, expected)?;
                self.pop_scope();
                common_type(&then_type, &else_type).ok_or_else(|| {
                    SemanticError::new(
                        *span,
                        format!(
                            "expression-if arms have incompatible types `{then_type}` and `{else_type}`"
                        ),
                    )
                })?
            }
            Expr::Match { value, arms, span } => {
                self.analyze_match(value, arms, expected, *span)?
            }
            Expr::Assignment {
                op,
                target,
                value,
                span,
            } => {
                if *op != AssignmentOp::Assign && self.is_record_place(target)? {
                    return Err(SemanticError::new(
                        target.span(),
                        "record entries currently support only direct `=` assignment; read with `??` before computing an update",
                    ));
                }
                let target_type = self.analyze_lvalue(target)?;
                let value_expected = if *op == AssignmentOp::Nullish {
                    Some(nullish_present_type(&target_type).ok_or_else(|| {
                        SemanticError::new(
                            target.span(),
                            format!(
                                "operator `??=` requires a nullable target, found `{target_type}`"
                            ),
                        )
                    })?)
                } else {
                    Some(&target_type)
                };
                let value_type = self.analyze_expr(value, value_expected)?;
                let result_type = if *op == AssignmentOp::Assign {
                    self.require_assignable(&target_type, &value_type, value.span())?;
                    target_type.clone()
                } else if *op == AssignmentOp::Nullish {
                    self.require_assignable(&target_type, &value_type, value.span())?;
                    self.analyze_binary(BinaryOp::Nullish, &target_type, &value_type, *span)?
                } else {
                    let binary_op = assignment_binary_op(*op);
                    let result =
                        self.analyze_binary(binary_op, &target_type, &value_type, *span)?;
                    self.require_assignable(&target_type, &result, *span)?;
                    target_type.clone()
                };
                self.invalidate_assigned_narrowing(target);
                result_type
            }
            Expr::Update {
                target, op, span, ..
            } => {
                if self.is_record_place(target)? {
                    return Err(SemanticError::new(
                        target.span(),
                        "record entries cannot be incremented directly because the key may be absent",
                    ));
                }
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
                let (id, ty) = {
                    let (_, symbol) = self.resolve_with_scope(ident)?;
                    (symbol.id, symbol.ty.clone())
                };
                self.model.assigned_symbols.insert(id);
                self.model.identifier_symbols.insert(ident.span, id);
                ty
            }
            Expr::Member {
                object,
                property,
                span,
            } => {
                let object_type = self.analyze_expr(object, None)?;
                if matches!(&object_type, Type::Union(_)) {
                    return Err(SemanticError::new(
                        *span,
                        format!(
                            "cannot assign through member `{}` on union `{object_type}`",
                            property.name
                        ),
                    ));
                }
                match object_type {
                    Type::Record(value) => *value,
                    other => self.analyze_member_type(other, *property, *span)?,
                }
            }
            Expr::Index {
                object,
                index,
                span,
            } => {
                let object_type = self.analyze_expr(object, None)?;
                if is_js_value(&object_type) {
                    let index_type = self.analyze_expr(index, None)?;
                    if !is_js_index_type(&index_type) {
                        return Err(SemanticError::new(
                            index.span(),
                            format!(
                                "a `JsValue` index must be numeric, `string`, or `JsValue`, found `{index_type}`"
                            ),
                        ));
                    }
                    Type::TypeParameter("$js")
                } else {
                    let expected_index = index_key_type(&object_type).ok_or_else(|| {
                        SemanticError::new(
                            *span,
                            format!("cannot assign through an index on `{object_type}`"),
                        )
                    })?;
                    let index_type = self.analyze_expr(index, Some(&expected_index))?;
                    self.require_assignable(&expected_index, &index_type, index.span())?;
                    index_value_type(&object_type, true).ok_or_else(|| {
                        SemanticError::new(
                            *span,
                            format!("cannot assign through an index on `{object_type}`"),
                        )
                    })?
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

    fn is_record_place<'ast>(
        &mut self,
        expression: &Expr<'ast, 'src>,
    ) -> Result<bool, SemanticError> {
        let object = match expression {
            Expr::Member { object, .. } | Expr::Index { object, .. } => object,
            _ => return Ok(false),
        };
        Ok(matches!(self.analyze_expr(object, None)?, Type::Record(_)))
    }

    fn analyze_member<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        property: Ident<'src>,
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        let object_type = self.analyze_expr(object, None)?;
        self.analyze_member_type(object_type, property, span)
    }

    fn analyze_match<'ast>(
        &mut self,
        value: &Expr<'ast, 'src>,
        arms: &[crate::ast::MatchArm<'ast, 'src>],
        expected: Option<&Type<'src>>,
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        let value_type = self.analyze_expr(value, None)?;
        if !matches!(
            value_type,
            Type::Enum(_) | Type::Int | Type::String | Type::Bool
        ) {
            return Err(SemanticError::new(
                value.span(),
                format!("match requires an enum, int, string, or bool value, found `{value_type}`"),
            ));
        }
        let variants = match value_type {
            Type::Enum(enum_name) => Some((
                enum_name,
                self.model
                    .enums
                    .get(enum_name)
                    .expect("checked enum type has metadata")
                    .variants
                    .clone(),
            )),
            _ => None,
        };
        let mut covered = AHashSet::default();
        let mut wildcard = false;
        let mut result = None;
        for (index, arm) in arms.iter().enumerate() {
            if wildcard {
                return Err(SemanticError::new(
                    arm.pattern.span(),
                    "match arms after `_` are unreachable",
                ));
            }
            match arm.pattern {
                MatchPattern::EnumVariant {
                    enum_name: pattern_enum,
                    variant,
                    span: pattern_span,
                } => {
                    let Some((enum_name, variants)) = &variants else {
                        return Err(SemanticError::new(
                            pattern_span,
                            format!("enum pattern cannot match `{value_type}`"),
                        ));
                    };
                    if pattern_enum.name != *enum_name {
                        return Err(SemanticError::new(
                            pattern_enum.span,
                            format!(
                                "match pattern uses enum `{}`, expected `{enum_name}`",
                                pattern_enum.name
                            ),
                        ));
                    }
                    let discriminant = variants.get(variant.name).copied().ok_or_else(|| {
                        SemanticError::new(
                            variant.span,
                            format!("enum `{enum_name}` has no variant `{}`", variant.name),
                        )
                    })?;
                    let key = format!("enum:{enum_name}:{}", variant.name);
                    if !covered.insert(key) {
                        return Err(SemanticError::new(
                            pattern_span,
                            format!("duplicate match arm for `{enum_name}.{}`", variant.name),
                        ));
                    }
                    self.model
                        .enum_variant_values
                        .insert(pattern_span, discriminant);
                }
                MatchPattern::Int(value, pattern_span) => {
                    if value_type != Type::Int {
                        return Err(SemanticError::new(
                            pattern_span,
                            format!("integer pattern cannot match `{value_type}`"),
                        ));
                    }
                    if !covered.insert(format!("int:{value}")) {
                        return Err(SemanticError::new(
                            pattern_span,
                            format!("duplicate match arm for `{value}`"),
                        ));
                    }
                }
                MatchPattern::String(value, pattern_span) => {
                    if value_type != Type::String {
                        return Err(SemanticError::new(
                            pattern_span,
                            format!("string pattern cannot match `{value_type}`"),
                        ));
                    }
                    if !covered.insert(format!("string:{value}")) {
                        return Err(SemanticError::new(
                            pattern_span,
                            format!("duplicate match arm for `{value}`"),
                        ));
                    }
                }
                MatchPattern::Bool(value, pattern_span) => {
                    if value_type != Type::Bool {
                        return Err(SemanticError::new(
                            pattern_span,
                            format!("boolean pattern cannot match `{value_type}`"),
                        ));
                    }
                    if !covered.insert(format!("bool:{value}")) {
                        return Err(SemanticError::new(
                            pattern_span,
                            format!("duplicate match arm for `{value}`"),
                        ));
                    }
                }
                MatchPattern::Wildcard(pattern_span) => {
                    if wildcard || index + 1 != arms.len() {
                        return Err(SemanticError::new(
                            pattern_span,
                            "the `_` match arm must appear once and last",
                        ));
                    }
                    wildcard = true;
                }
            }
            let arm_type = self.analyze_expr(&arm.value, expected)?;
            result = Some(match result {
                Some(previous) => common_type(&previous, &arm_type).ok_or_else(|| {
                    SemanticError::new(
                        arm.span,
                        format!("match arm has type `{arm_type}`, incompatible with `{previous}`"),
                    )
                })?,
                None => arm_type,
            });
        }
        if !wildcard {
            match (&value_type, variants) {
                (Type::Enum(_), Some((enum_name, variants))) if covered.len() != variants.len() => {
                    let missing = variants
                        .keys()
                        .filter(|variant| !covered.contains(&format!("enum:{enum_name}:{variant}")))
                        .copied()
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(SemanticError::new(
                        span,
                        format!("non-exhaustive match on `{enum_name}`; missing {missing}"),
                    ));
                }
                (Type::Bool, _) => {
                    let missing = [false, true]
                        .into_iter()
                        .filter(|value| !covered.contains(&format!("bool:{value}")))
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>();
                    if !missing.is_empty() {
                        return Err(SemanticError::new(
                            span,
                            format!(
                                "non-exhaustive match on `bool`; missing {}",
                                missing.join(", ")
                            ),
                        ));
                    }
                }
                (Type::Int | Type::String, _) => {
                    return Err(SemanticError::new(
                        span,
                        format!("match on `{value_type}` requires a final `_` arm"),
                    ));
                }
                _ => {}
            }
        }
        result.ok_or_else(|| SemanticError::new(span, "match expression has no arms"))
    }

    fn analyze_member_type(
        &mut self,
        object_type: Type<'src>,
        property: Ident<'src>,
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        if property.name == "length" {
            if let Type::Union(members) = &object_type {
                if members.iter().all(indexed_collection_has_length) {
                    return Ok(Type::Int);
                }
            }
        }
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
                    return Ok(method_callable_type(method, &AHashMap::default()));
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
            Type::TypeParameter("$js") => match property.name {
                "length" => Ok(Type::Float),
                "message" | "specifier" => Ok(Type::Nullable(Box::new(Type::String))),
                "truthy" | "isArray" | "isObject" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: Box::new(Type::Bool),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("JsValue has no member `{}`", property.name),
                )),
            },
            Type::Map(_, _) | Type::Set(_) if property.name == "size" => Ok(Type::Int),
            Type::ArrayBuffer | Type::SharedArrayBuffer if property.name == "byteLength" => {
                Ok(Type::Int)
            }
            ty if crate::typed_array::is_typed_array_type(&ty)
                && matches!(property.name, "length" | "byteLength" | "byteOffset") =>
            {
                Ok(Type::Int)
            }
            ty if crate::typed_array::is_typed_array_type(&ty) && property.name == "buffer" => {
                Ok(normalize_union(vec![
                    Type::ArrayBuffer,
                    Type::SharedArrayBuffer,
                ]))
            }
            Type::ModuleNamespace(module) => {
                let binding = self
                    .model
                    .module_exports
                    .get(&module)
                    .and_then(|exports| exports.get(property.name))
                    .copied()
                    .ok_or_else(|| {
                        SemanticError::new(
                            property.span,
                            format!("dynamic module has no runtime export `{}`", property.name),
                        )
                    })?;
                let symbol = self
                    .scopes
                    .first()
                    .and_then(|scope| scope.get(binding))
                    .and_then(|symbol| self.model.symbols.get(symbol.0 as usize))
                    .ok_or_else(|| {
                        SemanticError::new(
                            property.span,
                            format!("dynamic export `{}` is type-only", property.name),
                        )
                    })?;
                let ty = symbol.ty.clone();
                self.model
                    .used_dynamic_exports
                    .insert((module, property.name));
                Ok(ty)
            }
            Type::Task(_) if matches!(property.name, "then" | "catch" | "finally") => Err(
                SemanticError::new(span, format!("Task `{}` must be called", property.name)),
            ),
            Type::ModuleLoadError if matches!(property.name, "message" | "specifier") => {
                Ok(Type::String)
            }
            Type::Float => match property.name {
                "abs" | "floor" | "ceil" | "round" | "sqrt" | "sin" | "cos" | "acos" | "exp"
                | "log" | "tan" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: Box::new(Type::Float),
                })),
                "min" | "max" | "atan2" | "hypot" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Float],
                    defaults: vec![None],
                    return_type: Box::new(Type::Float),
                })),
                "toInt" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: Box::new(Type::Int),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("float has no member `{}`", property.name),
                )),
            },
            Type::Int => match property.name {
                "toString" | "toUnsignedString" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int],
                    defaults: vec![Some(DefaultValue::Int(10))],
                    return_type: Box::new(Type::String),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("int has no member `{}`", property.name),
                )),
            },
            Type::Array(element) => match property.name {
                "map" | "filter" | "forEach" | "reduce" | "some" | "every" | "findIndex" => Err(
                    SemanticError::new(span, format!("array `{}` must be called", property.name)),
                ),
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
                "indexOf" => Ok(Type::Function(FunctionType {
                    params: vec![*element],
                    defaults: vec![None],
                    return_type: Box::new(Type::Int),
                })),
                "includes" => Ok(Type::Function(FunctionType {
                    params: vec![element.as_ref().clone(), Type::Int],
                    defaults: vec![None, Some(DefaultValue::Int(0))],
                    return_type: Box::new(Type::Bool),
                })),
                "join" if is_stringifiable_array_element(&element) => {
                    Ok(Type::Function(FunctionType {
                        params: vec![Type::String],
                        defaults: vec![Some(DefaultValue::String(","))],
                        return_type: Box::new(Type::String),
                    }))
                }
                "join" => Err(SemanticError::new(
                    span,
                    format!("array element type `{element}` cannot be joined portably"),
                )),
                "concat" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Array(element.clone())],
                    defaults: vec![None],
                    return_type: Box::new(Type::Array(element)),
                })),
                "copyWithin" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int, Type::Int, Type::Int],
                    defaults: vec![None, None, Some(DefaultValue::Int(i32::MAX as i64))],
                    return_type: Box::new(Type::Array(element)),
                })),
                "reverse" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: Box::new(Type::Array(element)),
                })),
                "slice" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int, Type::Int],
                    defaults: vec![
                        Some(DefaultValue::Int(0)),
                        Some(DefaultValue::Int(i32::MAX as i64)),
                    ],
                    return_type: Box::new(Type::Array(element)),
                })),
                "splice" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int, Type::Int],
                    defaults: vec![None, None],
                    return_type: Box::new(Type::Array(element)),
                })),
                "fill" => Ok(Type::Function(FunctionType {
                    params: vec![*element.clone()],
                    defaults: vec![None],
                    return_type: Box::new(Type::Array(element)),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("array has no member `{}`", property.name),
                )),
            },
            Type::Record(value) => Ok(nullable_type(*value)),
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
            ty if crate::typed_array::is_typed_array_type(&ty) => match property.name {
                "slice" | "subarray" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int, Type::Int],
                    defaults: vec![None, Some(DefaultValue::Int(i32::MAX as i64))],
                    return_type: Box::new(ty),
                })),
                "set" => Ok(Type::Function(FunctionType {
                    params: vec![ty.clone(), Type::Int],
                    defaults: vec![None, Some(DefaultValue::Int(0))],
                    return_type: Box::new(Type::Void),
                })),
                "fill" => {
                    let element = crate::typed_array::TypedArrayKind::from_type(&ty)
                        .expect("typed-array branch has a kind")
                        .index_value_type();
                    Ok(Type::Function(FunctionType {
                        params: vec![element, Type::Int, Type::Int],
                        defaults: vec![
                            None,
                            Some(DefaultValue::Int(0)),
                            Some(DefaultValue::Int(i32::MAX as i64)),
                        ],
                        return_type: Box::new(ty),
                    }))
                }
                "copyWithin" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int, Type::Int, Type::Int],
                    defaults: vec![None, None, Some(DefaultValue::Int(i32::MAX as i64))],
                    return_type: Box::new(ty),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("{ty} has no member `{}`", property.name),
                )),
            },
            Type::String => match property.name {
                "charCodeAt" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int],
                    defaults: vec![None],
                    return_type: Box::new(Type::Int),
                })),
                "charAt" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int],
                    defaults: vec![None],
                    return_type: Box::new(Type::String),
                })),
                "includes" | "startsWith" | "endsWith" => Ok(Type::Function(FunctionType {
                    params: vec![Type::String],
                    defaults: vec![None],
                    return_type: Box::new(Type::Bool),
                })),
                "indexOf" => Ok(Type::Function(FunctionType {
                    params: vec![Type::String, Type::Int],
                    defaults: vec![None, Some(DefaultValue::Int(0))],
                    return_type: Box::new(Type::Int),
                })),
                "lastIndexOf" => Ok(Type::Function(FunctionType {
                    params: vec![Type::String, Type::Int],
                    defaults: vec![None, Some(DefaultValue::Int(i32::MAX as i64))],
                    return_type: Box::new(Type::Int),
                })),
                "repeat" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int],
                    defaults: vec![None],
                    return_type: Box::new(Type::String),
                })),
                "toUpperCase" | "toLowerCase" | "trim" | "trimStart" | "trimEnd" => {
                    Ok(Type::Function(FunctionType {
                        params: Vec::new(),
                        defaults: Vec::new(),
                        return_type: Box::new(Type::String),
                    }))
                }
                "search" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Regex],
                    defaults: vec![None],
                    return_type: Box::new(Type::Int),
                })),
                "slice" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Int, Type::Int],
                    defaults: vec![None, Some(DefaultValue::Int(i32::MAX as i64))],
                    return_type: Box::new(Type::String),
                })),
                "replace" => Ok(Type::Function(FunctionType {
                    params: vec![Type::Regex, Type::String],
                    defaults: vec![None, None],
                    return_type: Box::new(Type::String),
                })),
                "split" => Ok(Type::Function(FunctionType {
                    params: vec![Type::String],
                    defaults: vec![None],
                    return_type: Box::new(Type::Array(Box::new(Type::String))),
                })),
                "codePointLength" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: Box::new(Type::Int),
                })),
                "truthy" => Ok(Type::Function(FunctionType {
                    params: Vec::new(),
                    defaults: Vec::new(),
                    return_type: Box::new(Type::Bool),
                })),
                _ => Err(SemanticError::new(
                    span,
                    format!("string has no member `{}`", property.name),
                )),
            },
            Type::Regex => match property.name {
                "test" => Ok(Type::Function(FunctionType {
                    params: vec![Type::String],
                    defaults: vec![None],
                    return_type: Box::new(Type::Bool),
                })),
                "exec" => Ok(Type::Function(FunctionType {
                    params: vec![Type::String],
                    defaults: vec![None],
                    return_type: Box::new(Type::TypeParameter("$js")),
                })),
                "source" | "flags" => Ok(Type::String),
                "lastIndex" => Ok(Type::Float),
                "global" | "ignoreCase" | "multiline" | "dotAll" | "sticky" | "unicode" => {
                    Ok(Type::Bool)
                }
                _ => Err(SemanticError::new(
                    span,
                    format!("Regex has no member `{}`", property.name),
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

    fn analyze_array_predicate<'ast>(
        &mut self,
        object: &Expr<'ast, 'src>,
        args: &[Expr<'ast, 'src>],
        method: &str,
        span: Span,
        return_type: Type<'src>,
    ) -> Result<Type<'src>, SemanticError> {
        let element_type = self.array_element_type(object, method, span)?;
        let signature = self.analyze_array_callback(args, &element_type, method, span)?;
        if signature.return_type.as_ref() != &Type::Bool {
            return Err(SemanticError::new(
                args[0].span(),
                format!(
                    "array `{method}` callback must return `bool`, found `{}`",
                    signature.return_type
                ),
            ));
        }
        Ok(return_type)
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
        if is_js_value(callee) {
            let js = Type::TypeParameter("$js");
            for arg in args {
                let actual = self.analyze_expr(arg, Some(&js))?;
                self.require_assignable(&js, &actual, arg.span())?;
            }
            return Ok(js);
        }
        if let Type::GenericFunction(function) = callee {
            return self.analyze_generic_call(function, args, span, expected_return);
        }
        if let Type::Union(members) = callee {
            let mut signatures = Vec::new();
            for member in members {
                let Type::Function(signature) = member else {
                    return Err(SemanticError::new(
                        span,
                        format!("cannot call a value of type `{callee}`"),
                    ));
                };
                signatures.push(signature);
            }
            if !signatures
                .iter()
                .all(|signature| args.len() == signature.params.len())
            {
                return Err(SemanticError::new(
                    span,
                    format!(
                        "cannot call a value of type `{callee}` with {} arguments; union calls require every argument explicitly",
                        args.len()
                    ),
                ));
            }
            for (index, arg) in args.iter().enumerate() {
                let expected = &signatures[0].params[index];
                let contextual = signatures
                    .iter()
                    .all(|signature| &signature.params[index] == expected)
                    .then_some(expected);
                let actual = self.analyze_expr(arg, contextual)?;
                for signature in &signatures {
                    let expected = &signature.params[index];
                    self.require_assignable(expected, &actual, arg.span())?;
                }
            }
            let returns = signatures
                .iter()
                .map(|signature| (*signature.return_type).clone())
                .collect::<Vec<_>>();
            return Ok(normalize_union(returns));
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
        self.require_omitted_defaults_in_scope(signature, args.len(), span)?;
        for (arg, expected) in args.iter().zip(&signature.params) {
            let actual = self.analyze_expr(arg, Some(expected))?;
            self.require_assignable(expected, &actual, arg.span())?;
        }
        Ok((*signature.return_type).clone())
    }

    fn require_omitted_defaults_in_scope(
        &self,
        signature: &FunctionType<'src>,
        provided: usize,
        span: Span,
    ) -> Result<(), SemanticError> {
        for default in signature.defaults.iter().skip(provided).flatten() {
            self.require_default_in_scope(default, span)?;
        }
        Ok(())
    }

    fn require_default_in_scope(
        &self,
        default: &DefaultValue<'src>,
        span: Span,
    ) -> Result<(), SemanticError> {
        let symbol = match default {
            DefaultValue::Symbol(symbol) => Some(*symbol),
            DefaultValue::PendingIdentifier(default_span) => {
                self.model.identifier_symbol(*default_span)
            }
            DefaultValue::Array(values) => {
                for value in values {
                    self.require_default_in_scope(value, span)?;
                }
                None
            }
            DefaultValue::Struct { values, .. } => {
                for value in values {
                    self.require_default_in_scope(value, span)?;
                }
                None
            }
            DefaultValue::NewClass { args, .. } => {
                for argument in args {
                    self.require_default_in_scope(argument, span)?;
                }
                None
            }
            DefaultValue::Int(_)
            | DefaultValue::Float(_)
            | DefaultValue::String(_)
            | DefaultValue::Bool(_)
            | DefaultValue::Null
            | DefaultValue::Undefined
            | DefaultValue::Parameter(_)
            | DefaultValue::PendingUndefined(_)
            | DefaultValue::Arrow(_) => None,
        };
        if symbol.is_some_and(|symbol| {
            !self
                .scopes
                .iter()
                .any(|scope| scope.values().any(|candidate| *candidate == symbol))
        }) {
            return Err(SemanticError::new(
                span,
                "parameter default depends on a local binding that is unavailable at this call site",
            ));
        }
        Ok(())
    }

    fn analyze_static_namespace_call<'ast>(
        &mut self,
        callee: &Expr<'ast, 'src>,
        args: &[Expr<'ast, 'src>],
        span: Span,
        expected: Option<&Type<'src>>,
    ) -> Result<Option<(BuiltinCall, Type<'src>)>, SemanticError> {
        let Expr::Member {
            object: Expr::Ident(namespace),
            property,
            ..
        } = callee
        else {
            return Ok(None);
        };
        if !self.builtin_namespace_is_unshadowed(namespace.name) {
            return Ok(None);
        }
        match (namespace.name, property.name) {
            ("Object", "keys" | "values") => {
                let [record] = args else {
                    return Err(SemanticError::new(
                        span,
                        format!("`Object.{}` expects one record", property.name),
                    ));
                };
                let actual = self.analyze_expr(record, None)?;
                let Type::Record(value) = actual else {
                    return Err(SemanticError::new(
                        record.span(),
                        format!(
                            "`Object.{}` requires a `Record<T>`, found `{actual}`",
                            property.name
                        ),
                    ));
                };
                Ok(Some((
                    if property.name == "keys" {
                        BuiltinCall::ObjectKeys
                    } else {
                        BuiltinCall::ObjectValues
                    },
                    if property.name == "keys" {
                        Type::Array(Box::new(Type::String))
                    } else {
                        Type::Array(value)
                    },
                )))
            }
            ("Object", "hasOwn") => {
                let [record, key] = args else {
                    return Err(SemanticError::new(
                        span,
                        "`Object.hasOwn` expects a record and string key",
                    ));
                };
                let actual = self.analyze_expr(record, None)?;
                if !matches!(actual, Type::Record(_)) {
                    return Err(SemanticError::new(
                        record.span(),
                        format!("`Object.hasOwn` requires a `Record<T>`, found `{actual}`"),
                    ));
                }
                let key_type = self.analyze_expr(key, Some(&Type::String))?;
                self.require_assignable(&Type::String, &key_type, key.span())?;
                Ok(Some((BuiltinCall::ObjectHasOwn, Type::Bool)))
            }
            ("Object", "assign") => {
                let [target, source] = args else {
                    return Err(SemanticError::new(
                        span,
                        "`Object.assign` expects two records",
                    ));
                };
                let target_type = self.analyze_expr(target, None)?;
                let Type::Record(_) = target_type else {
                    return Err(SemanticError::new(
                        target.span(),
                        format!(
                            "`Object.assign` target must be a `Record<T>`, found `{target_type}`"
                        ),
                    ));
                };
                let source_type = self.analyze_expr(source, Some(&target_type))?;
                if source_type != target_type {
                    return Err(SemanticError::new(
                        source.span(),
                        format!("`Object.assign` source has type `{source_type}`, expected `{target_type}`"),
                    ));
                }
                Ok(Some((BuiltinCall::ObjectAssign, target_type)))
            }
            ("JSON", "stringify") => {
                let [value] = args else {
                    return Err(SemanticError::new(
                        span,
                        "`JSON.stringify` expects one value",
                    ));
                };
                let actual = self.analyze_expr(value, None)?;
                if !json_stringify_type_supported(&actual) {
                    return Err(SemanticError::new(
                        value.span(),
                        format!("`JSON.stringify` does not support `{actual}` portably"),
                    ));
                }
                Ok(Some((BuiltinCall::JsonStringify, Type::String)))
            }
            ("JSON", "parse") => {
                let [value] = args else {
                    return Err(SemanticError::new(span, "`JSON.parse` expects one string"));
                };
                let actual = self.analyze_expr(value, Some(&Type::String))?;
                self.require_assignable(&Type::String, &actual, value.span())?;
                Ok(Some((BuiltinCall::JsonParse, Type::TypeParameter("$js"))))
            }
            ("Task", "resolve") => {
                let [value] = args else {
                    return Err(SemanticError::new(span, "`Task.resolve` expects one value"));
                };
                let expected_value = match expected {
                    Some(Type::Task(value)) => Some(value.as_ref()),
                    _ => None,
                };
                let value = self.analyze_expr(value, expected_value)?;
                Ok(Some((
                    BuiltinCall::TaskResolve,
                    Type::Task(Box::new(value)),
                )))
            }
            ("Task", "reject") => {
                let [reason] = args else {
                    return Err(SemanticError::new(span, "`Task.reject` expects one reason"));
                };
                let reason_type = self.analyze_expr(reason, None)?;
                if reason_type == Type::Void {
                    return Err(SemanticError::new(
                        reason.span(),
                        "`Task.reject` reason cannot be `void`",
                    ));
                }
                let Some(Type::Task(value)) = expected else {
                    return Err(SemanticError::new(
                        span,
                        "cannot infer rejected task value type; provide an expected `Task<T>` type",
                    ));
                };
                Ok(Some((BuiltinCall::TaskReject, Type::Task(value.clone()))))
            }
            ("Task", "all") => {
                let [tasks] = args else {
                    return Err(SemanticError::new(
                        span,
                        "`Task.all` expects one task array",
                    ));
                };
                let tasks = self.analyze_expr(tasks, None)?;
                let Type::Array(ref task) = tasks else {
                    return Err(SemanticError::new(
                        args[0].span(),
                        format!("`Task.all` requires a `Task<T>[]`, found `{tasks}`"),
                    ));
                };
                let Type::Task(value) = task.as_ref() else {
                    return Err(SemanticError::new(
                        args[0].span(),
                        format!("`Task.all` requires a `Task<T>[]`, found `{tasks}`"),
                    ));
                };
                Ok(Some((
                    BuiltinCall::TaskAll,
                    Type::Task(Box::new(Type::Array(value.clone()))),
                )))
            }
            ("JS", method) => self.analyze_javascript_builtin(method, args, span, expected),
            _ => Ok(None),
        }
    }

    fn builtin_namespace_is_unshadowed(&self, name: &str) -> bool {
        !self
            .scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    fn analyze_javascript_builtin<'ast>(
        &mut self,
        method: &str,
        args: &[Expr<'ast, 'src>],
        span: Span,
        expected: Option<&Type<'src>>,
    ) -> Result<Option<(BuiltinCall, Type<'src>)>, SemanticError> {
        let js = Type::TypeParameter("$js");
        let require_arity = |expected: std::ops::RangeInclusive<usize>| {
            if expected.contains(&args.len()) {
                Ok(())
            } else {
                let start = *expected.start();
                let end = *expected.end();
                let count = if start == end {
                    start.to_string()
                } else if end == usize::MAX {
                    // The builtin forwards its arguments to the callee, so the
                    // only real bound is the lower one.
                    format!("at least {start}")
                } else {
                    format!("{start} to {end}")
                };
                Err(SemanticError::new(
                    span,
                    format!(
                        "`JS.{method}` expects {count} arguments, found {}",
                        args.len()
                    ),
                ))
            }
        };

        let (builtin, result, expected_args): (BuiltinCall, Type<'src>, Vec<Type<'src>>) =
            match method {
                "object" => {
                    if !args.len().is_multiple_of(2) {
                        return Err(SemanticError::new(
                            span,
                            format!(
                                "`JS.object` expects an even number of key/value arguments, found {}",
                                args.len()
                            ),
                        ));
                    }
                    let mut expected_args = Vec::with_capacity(args.len());
                    for index in 0..args.len() {
                        if index % 2 == 0 {
                            expected_args.push(Type::String);
                        } else {
                            expected_args.push(js.clone());
                        }
                    }
                    (BuiltinCall::JsObject, js.clone(), expected_args)
                }
                "array" => {
                    // Variadic, like `JS.object`. Without it there is no way to
                    // write a `JsValue` array literal, so ports build every
                    // array by allocating an empty one and pushing into it.
                    let expected_args = vec![js.clone(); args.len()];
                    (BuiltinCall::JsArray, js.clone(), expected_args)
                }
                "undefined" => {
                    require_arity(0..=0)?;
                    (BuiltinCall::JsUndefined, js.clone(), Vec::new())
                }
                "typeOf" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsTypeOf, Type::String, vec![js.clone()])
                }
                "isNullish" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsIsNullish, Type::Bool, vec![js.clone()])
                }
                "isFalse" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsIsFalse, Type::Bool, vec![js.clone()])
                }
                "isUndefined" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsIsUndefined, Type::Bool, vec![js.clone()])
                }
                "string" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsString, Type::String, vec![js.clone()])
                }
                "number" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsNumber, Type::Float, vec![js.clone()])
                }
                "add" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsAdd, js.clone(), vec![js.clone(), js.clone()])
                }
                "mod" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsMod, js.clone(), vec![js.clone(), js.clone()])
                }
                "lessThan" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsLessThan,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "lessThanOrEqual" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsLessThanOrEqual,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "greaterThan" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsGreaterThan,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "greaterThanOrEqual" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsGreaterThanOrEqual,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "assume" => {
                    require_arity(1..=1)?;
                    let result = expected.cloned().ok_or_else(|| {
                        SemanticError::new(
                            span,
                            "cannot infer `JS.assume` result type; provide an expected type",
                        )
                    })?;
                    if result.is_void() {
                        return Err(SemanticError::new(
                            span,
                            "`JS.assume` result cannot be `void`",
                        ));
                    }
                    (BuiltinCall::JsAssume, result, vec![js.clone()])
                }
                "strictEqual" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsStrictEqual,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "strictNotEqual" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsStrictNotEqual,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "or" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsOr, js.clone(), vec![js.clone(), js.clone()])
                }
                "and" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsAnd, js.clone(), vec![js.clone(), js.clone()])
                }
                "call" => {
                    // The lowering forwards `values[1..]` verbatim, so the argument
                    // count is bounded by the callee, not by this builtin.
                    require_arity(2..=usize::MAX)?;
                    (
                        BuiltinCall::JsCall,
                        js.clone(),
                        vec![js.clone(); args.len()],
                    )
                }
                "construct" => {
                    require_arity(1..=usize::MAX)?;
                    (
                        BuiltinCall::JsConstruct,
                        js.clone(),
                        vec![js.clone(); args.len()],
                    )
                }
                "invoke" => {
                    require_arity(2..=usize::MAX)?;
                    let mut types = vec![js.clone(), Type::String];
                    types.resize(args.len(), js.clone());
                    (BuiltinCall::JsInvoke, js.clone(), types)
                }
                "apply" => {
                    require_arity(3..=3)?;
                    (BuiltinCall::JsApply, js.clone(), vec![js.clone(); 3])
                }
                "method0" | "method1" | "method2" | "method3" | "methodRest" | "staticRest" => {
                    require_arity(1..=1)?;
                    let parameter_count = match method {
                        "method0" | "staticRest" => 1,
                        "method1" | "methodRest" => 2,
                        "method2" => 3,
                        "method3" => 4,
                        _ => unreachable!(),
                    };
                    let callback = Type::Function(FunctionType {
                        params: vec![js.clone(); parameter_count],
                        defaults: vec![None; parameter_count],
                        return_type: Box::new(js.clone()),
                    });
                    (
                        match method {
                            "method0" => BuiltinCall::JsMethod0,
                            "method1" => BuiltinCall::JsMethod1,
                            "method2" => BuiltinCall::JsMethod2,
                            "method3" => BuiltinCall::JsMethod3,
                            "methodRest" => BuiltinCall::JsMethodRest,
                            "staticRest" => BuiltinCall::JsStaticRest,
                            _ => unreachable!(),
                        },
                        js.clone(),
                        vec![callback],
                    )
                }
                "get" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsGet,
                        js.clone(),
                        vec![js.clone(), Type::String],
                    )
                }
                "set" => {
                    require_arity(3..=3)?;
                    (
                        BuiltinCall::JsSet,
                        Type::Void,
                        vec![js.clone(), Type::String, js.clone()],
                    )
                }
                "delete" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsDelete,
                        Type::Void,
                        vec![js.clone(), js.clone()],
                    )
                }
                "has" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsHas,
                        Type::Bool,
                        vec![js.clone(), Type::String],
                    )
                }
                "in" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsIn, Type::Bool, vec![js.clone(), js.clone()])
                }
                "box" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsBox, js.clone(), vec![js.clone()])
                }
                "push" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsArrayPush,
                        Type::Float,
                        vec![js.clone(), js.clone()],
                    )
                }
                "pop" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsArrayPop, js.clone(), vec![js.clone()])
                }
                "slice" => {
                    require_arity(1..=3)?;
                    let mut types = vec![js.clone()];
                    types.resize(args.len(), Type::Float);
                    (BuiltinCall::JsArraySlice, js.clone(), types)
                }
                "indexOf" => {
                    require_arity(2..=3)?;
                    let mut types = vec![js.clone(), js.clone()];
                    if args.len() == 3 {
                        types.push(Type::Float);
                    }
                    (BuiltinCall::JsArrayIndexOf, Type::Float, types)
                }
                "sort" => {
                    require_arity(1..=2)?;
                    (
                        BuiltinCall::JsArraySort,
                        js.clone(),
                        vec![js.clone(); args.len()],
                    )
                }
                "splice" => {
                    require_arity(2..=4)?;
                    let mut types = vec![js.clone(), Type::Float];
                    if args.len() >= 3 {
                        types.push(Type::Float);
                    }
                    if args.len() == 4 {
                        types.push(js.clone());
                    }
                    (BuiltinCall::JsArraySplice, js.clone(), types)
                }
                "concatApply" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsArrayConcatApply,
                        js.clone(),
                        vec![js.clone(), js.clone()],
                    )
                }
                "join" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsArrayJoin,
                        Type::String,
                        vec![js.clone(), Type::String],
                    )
                }
                "shift" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsArrayShift, js.clone(), vec![js.clone()])
                }
                "unshift" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsArrayUnshift,
                        Type::Float,
                        vec![js.clone(), js.clone()],
                    )
                }
                "isArray" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsIsArray, Type::Bool, vec![js.clone()])
                }
                "stringSlice" => {
                    require_arity(2..=3)?;
                    let mut types = vec![Type::String, Type::Float];
                    if args.len() == 3 {
                        types.push(Type::Float);
                    }
                    (BuiltinCall::JsStringSlice, Type::String, types)
                }
                "stringIndexOf" => {
                    require_arity(2..=3)?;
                    let mut types = vec![Type::String, Type::String];
                    if args.len() == 3 {
                        types.push(Type::Float);
                    }
                    (BuiltinCall::JsStringIndexOf, Type::Float, types)
                }
                "stringReplace" => {
                    require_arity(3..=3)?;
                    (
                        BuiltinCall::JsStringReplace,
                        Type::String,
                        vec![Type::String, Type::Regex, js.clone()],
                    )
                }
                "stringMatch" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsStringMatch,
                        js.clone(),
                        vec![Type::String, Type::Regex],
                    )
                }
                "stringSplit" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsStringSplit,
                        Type::Array(Box::new(Type::String)),
                        vec![Type::String, Type::String],
                    )
                }
                "regexTest" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsRegexTest,
                        Type::Bool,
                        vec![Type::Regex, js.clone()],
                    )
                }
                "regexExec" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsRegexExec,
                        js.clone(),
                        vec![Type::Regex, js.clone()],
                    )
                }
                "encodeURI" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsEncodeURI, Type::String, vec![Type::String])
                }
                "encodeURIComponent" => {
                    require_arity(1..=1)?;
                    (
                        BuiltinCall::JsEncodeURIComponent,
                        Type::String,
                        vec![Type::String],
                    )
                }
                _ => return Ok(None),
            };

        for (argument, expected) in args.iter().zip(&expected_args) {
            let actual = self.analyze_expr(argument, Some(expected))?;
            self.require_assignable(expected, &actual, argument.span())?;
        }
        Ok(Some((builtin, result)))
    }

    fn analyze_task_call<'ast>(
        &mut self,
        method: &str,
        value: Type<'src>,
        args: &[Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, SemanticError> {
        if args.len() != 1 {
            return Err(SemanticError::new(
                span,
                format!("Task `{method}` expects one callback, found {}", args.len()),
            ));
        }
        let parameters = match method {
            "then" => vec![value.clone()],
            "catch" => vec![Type::TypeParameter("$js")],
            "finally" => Vec::new(),
            _ => unreachable!("task call dispatch validates the method name"),
        };
        let expected = Type::Function(FunctionType {
            params: parameters.clone(),
            defaults: vec![None; parameters.len()],
            return_type: Box::new(Type::Void),
        });
        let callback = self.analyze_expr(&args[0], Some(&expected))?;
        let Type::Function(signature) = callback else {
            return Err(SemanticError::new(
                args[0].span(),
                format!("Task `{method}` expects a function callback"),
            ));
        };
        if signature.params != parameters {
            return Err(SemanticError::new(
                args[0].span(),
                format!("Task `{method}` callback has an incompatible parameter list"),
            ));
        }
        if method == "finally" {
            return Ok(Type::Task(Box::new(value)));
        }
        let returned = match *signature.return_type {
            Type::Task(inner) => *inner,
            returned => returned,
        };
        Ok(Type::Task(Box::new(if method == "catch" {
            normalize_union(vec![value, returned])
        } else {
            returned
        })))
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
        self.require_omitted_defaults_in_scope(&function.signature, args.len(), span)?;
        let parameters = function
            .type_params
            .iter()
            .copied()
            .collect::<AHashSet<_>>();
        let mut substitutions = AHashMap::default();
        if let Some(expected_return) = expected_return {
            infer_type_arguments(
                &function.signature.return_type,
                expected_return,
                &parameters,
                &mut substitutions,
                span,
            )?;
        }
        let mut actual_args = Vec::with_capacity(args.len());
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
            actual_args.push(actual);
        }
        for parameter in &function.type_params {
            if !substitutions.contains_key(parameter) {
                return Err(SemanticError::new(
                    span,
                    format!("cannot infer type argument `{parameter}`"),
                ));
            }
        }
        for ((arg, pattern), actual) in args
            .iter()
            .zip(&function.signature.params)
            .zip(&actual_args)
        {
            let resolved = substitute_type(pattern, &substitutions);
            self.require_assignable(&resolved, actual, arg.span())?;
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

        self.push_scope();
        self.generator_contexts.push(None);
        self.constructor_classes.push(None);
        let mut parameter_types = Vec::with_capacity(params.len());
        for (index, param) in params.iter().enumerate() {
            let ty = if param.ty.is_auto() {
                expected_signature
                    .and_then(|signature| signature.params.get(index))
                    .cloned()
                    .ok_or_else(|| {
                        SemanticError::new(
                            param.ty.span,
                            "`auto` arrow parameters require a contextual callback type",
                        )
                    })?
            } else {
                self.resolve_value_type(param.ty, "arrow parameter")?
            };
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

        self.callable_depth += 1;
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
        self.callable_depth -= 1;
        self.analyze_parameter_defaults(params, &parameter_types)?;
        let global_symbols = self
            .scopes
            .first()
            .into_iter()
            .flat_map(|scope| scope.values().copied())
            .collect::<AHashSet<_>>();
        self.constructor_classes.pop();
        self.generator_contexts.pop();
        self.pop_scope();

        let defaults = resolve_analyzed_parameter_defaults(
            params,
            &parameter_types,
            &self.model,
            true,
            &global_symbols,
        )?;
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
            BinaryOp::Mod
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::Xor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight
            | BinaryOp::UnsignedShiftRight
                if lhs == &Type::Int && rhs == &Type::Int =>
            {
                Ok(Type::Int)
            }
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
            BinaryOp::Nullish => {
                if lhs == &Type::Null {
                    return Ok(rhs.clone());
                }
                let present = nullish_present_type(lhs).ok_or_else(|| {
                    SemanticError::new(
                        span,
                        format!("operator `??` requires a nullable left operand, found `{lhs}`"),
                    )
                })?;
                common_type(present, rhs).ok_or_else(|| invalid_binary(op, lhs, rhs, span))
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
            name if crate::typed_array::TypedArrayKind::from_name(name).is_some() => {
                let kind = crate::typed_array::TypedArrayKind::from_name(name)
                    .expect("typed array constructor name");
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
                let actual = self.analyze_expr(&args[0], None)?;
                if !matches!(
                    actual,
                    Type::Int | Type::ArrayBuffer | Type::SharedArrayBuffer
                ) {
                    return Err(SemanticError::new(
                        args[0].span(),
                        format!(
                            "`{}` expects an `int`, `ArrayBuffer`, or `SharedArrayBuffer`, found `{actual}`",
                            class.name
                        ),
                    ));
                }
                kind.as_type()
            }
            "Symbol" => {
                self.resolve_type_arguments(class.name, type_args, &[], span)?;
                if args.len() > 1 {
                    return Err(SemanticError::new(
                        span,
                        format!(
                            "`Symbol` constructor expects 0 or 1 arguments, found {}",
                            args.len()
                        ),
                    ));
                }
                if let Some(argument) = args.first() {
                    let actual = self.analyze_expr(argument, Some(&Type::String))?;
                    self.require_assignable(&Type::String, &actual, argument.span())?;
                }
                Type::Symbol
            }
            "Regex" => {
                self.resolve_type_arguments(class.name, type_args, &[], span)?;
                if !(1..=2).contains(&args.len()) {
                    return Err(SemanticError::new(
                        span,
                        format!(
                            "`Regex` constructor expects 1 or 2 arguments, found {}",
                            args.len()
                        ),
                    ));
                }
                for argument in args {
                    let actual = self.analyze_expr(argument, Some(&Type::String))?;
                    self.require_assignable(&Type::String, &actual, argument.span())?;
                }
                Type::Regex
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
            TypeKind::Named { name: "Task", args }
                if !self.model.structs.contains_key("Task")
                    && !self.model.classes.contains_key("Task") =>
            {
                let resolved = self.resolve_type_arguments("Task", args, &["T"], ty.span)?;
                Ok(Type::Task(Box::new(resolved[0].clone())))
            }
            TypeKind::Named {
                name: "Generator",
                args,
            } if !self.model.structs.contains_key("Generator")
                && !self.model.classes.contains_key("Generator") =>
            {
                let resolved = self.resolve_type_arguments("Generator", args, &["T"], ty.span)?;
                Ok(Type::Generator(Box::new(resolved[0].clone())))
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
            TypeKind::Named { name, args }
                if let Some(kind) = crate::typed_array::TypedArrayKind::from_name(name) =>
            {
                self.resolve_type_arguments(name, args, &[], ty.span)?;
                Ok(kind.as_type())
            }
            TypeKind::Named {
                name: "Symbol",
                args,
            } => {
                self.resolve_type_arguments("Symbol", args, &[], ty.span)?;
                Ok(Type::Symbol)
            }
            TypeKind::Named {
                name: "Regex",
                args,
            } => {
                self.resolve_type_arguments("Regex", args, &[], ty.span)?;
                Ok(Type::Regex)
            }
            TypeKind::Named {
                name: "JsValue",
                args,
            } => {
                self.resolve_type_arguments("JsValue", args, &[], ty.span)?;
                Ok(Type::TypeParameter("$js"))
            }
            TypeKind::Named {
                name: "Record",
                args,
            } => {
                let arguments =
                    self.resolve_type_arguments("Record", args, &["$value"], ty.span)?;
                let [value] = arguments.as_slice() else {
                    unreachable!("Record arity was checked")
                };
                Ok(Type::Record(Box::new(value.clone())))
            }
            TypeKind::Named { name, args } if self.model.enums.contains_key(name) => {
                self.resolve_type_arguments(name, args, &[], ty.span)?;
                Ok(Type::Enum(name))
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
        if self.is_assignable(expected, actual) {
            Ok(())
        } else {
            Err(SemanticError::new(
                span,
                format!("expected `{expected}`, found `{actual}`"),
            ))
        }
    }

    fn is_assignable(&self, expected: &Type<'src>, actual: &Type<'src>) -> bool {
        if is_type_assignable(expected, actual) {
            return true;
        }
        match (expected, actual) {
            (Type::Array(expected), Type::Array(actual)) => {
                self.is_assignable(expected, actual) && self.is_assignable(actual, expected)
            }
            (Type::Task(expected), Type::Task(actual))
            | (Type::Generator(expected), Type::Generator(actual))
            | (Type::Nullable(expected), Type::Nullable(actual)) => {
                self.is_assignable(expected, actual)
            }
            (Type::Nullable(expected), actual) => self.is_assignable(expected, actual),
            (Type::Union(expected), Type::Union(actual)) => actual.iter().all(|actual| {
                expected
                    .iter()
                    .any(|expected| self.is_assignable(expected, actual))
            }),
            (Type::Union(expected), actual) => expected
                .iter()
                .any(|expected| self.is_assignable(expected, actual)),
            (expected, Type::Union(actual)) => actual
                .iter()
                .all(|actual| self.is_assignable(expected, actual)),
            (
                Type::Class(_) | Type::ClassInstance { .. },
                Type::Class(_) | Type::ClassInstance { .. },
            ) => {
                let mut current = actual.clone();
                while let Some((name, args)) = class_type_parts(&current) {
                    let info = match self.model.classes.get(name) {
                        Some(info) => info,
                        None => return false,
                    };
                    let Some(base) = &info.base else {
                        return false;
                    };
                    let substitutions = substitutions_for(&info.type_params, args);
                    current = substitute_type(base, &substitutions);
                    if is_type_assignable(expected, &current) {
                        return true;
                    }
                }
                false
            }
            _ => false,
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
            let mut then_narrowing = empty_narrowing();
            then_narrowing.insert(symbol.id, target);
            let mut else_narrowing = empty_narrowing();
            if let Some(ty) = remaining {
                else_narrowing.insert(symbol.id, ty);
            }
            return Ok((then_narrowing, else_narrowing));
        }
        let Expr::Binary { op, lhs, rhs, .. } = condition else {
            return Ok((empty_narrowing(), empty_narrowing()));
        };
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let (lhs_then, lhs_else) = self.condition_narrowing(lhs)?;
            let (rhs_then, rhs_else) = self.condition_narrowing(rhs)?;
            return Ok(if *op == BinaryOp::And {
                (merge_narrowing(lhs_then, rhs_then), empty_narrowing())
            } else {
                (empty_narrowing(), merge_narrowing(lhs_else, rhs_else))
            });
        }
        if !matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            return Ok((empty_narrowing(), empty_narrowing()));
        }
        let ident = match (*lhs, *rhs) {
            (Expr::Ident(ident), Expr::Null(_)) | (Expr::Null(_), Expr::Ident(ident)) => ident,
            _ => return Ok((empty_narrowing(), empty_narrowing())),
        };
        let symbol = self.resolve(ident)?;
        let current = self.narrowed_type(symbol.id).unwrap_or(&symbol.ty);
        let Type::Nullable(inner) = current else {
            return Ok((empty_narrowing(), empty_narrowing()));
        };
        let mut present_narrowing = empty_narrowing();
        present_narrowing.insert(symbol.id, inner.as_ref().clone());
        Ok(if *op == BinaryOp::NotEq {
            (present_narrowing, empty_narrowing())
        } else {
            (empty_narrowing(), present_narrowing)
        })
    }

    fn apply_narrowing(&mut self, narrowing: Narrowing<'src>) {
        if narrowing.is_empty() {
            return;
        }
        let scope = self
            .narrowings
            .last_mut()
            .expect("semantic analyzer always has a narrowing scope");
        for (symbol, ty) in narrowing {
            scope.insert(symbol, ty);
        }
    }

    fn current_scope_preserves(&self, narrowing: &Narrowing<'src>) -> bool {
        if narrowing.is_empty() {
            return false;
        }
        let Some(scope) = self.narrowings.last() else {
            return false;
        };
        narrowing
            .iter()
            .all(|(symbol, ty)| scope.get(symbol) == Some(ty))
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
        self.scopes.push(AHashMap::default());
        self.narrowings.push(AHashMap::default());
    }

    fn pop_scope(&mut self) {
        debug_assert!(self.scopes.len() > 1);
        self.scopes.pop();
        self.narrowings.pop();
    }
}

fn validate_type_params<'src>(params: &[Ident<'src>]) -> Result<Vec<&'src str>, SemanticError> {
    let mut names = Vec::with_capacity(params.len());
    let mut seen = AHashSet::default();
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
            if let Expr::Ident(identifier) = expression {
                return Ok(Some(DefaultValue::PendingIdentifier(identifier.span)));
            }
            if syntactic_js_undefined_default(expression) {
                return Ok(Some(DefaultValue::PendingUndefined(expression.span())));
            }
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

fn resolve_analyzed_parameter_defaults<'ast, 'src>(
    params: &[crate::ast::Param<'ast, 'src>],
    types: &[Type<'src>],
    model: &SemanticModel<'src>,
    parameter_defaults_in_scope: bool,
    global_symbols: &AHashSet<SymbolId>,
) -> Result<Vec<Option<DefaultValue<'src>>>, SemanticError> {
    let mut defaults = resolve_parameter_defaults(params, types)?;
    let parameter_symbols = if parameter_defaults_in_scope {
        params
            .iter()
            .filter_map(|parameter| model.identifier_symbol(parameter.name.span))
            .collect::<AHashSet<_>>()
    } else {
        AHashSet::default()
    };
    for (index, param) in params.iter().enumerate() {
        let Some(expression) = &param.default else {
            continue;
        };
        if parameter_defaults_in_scope
            && default_contains_arrow_capture(expression, &parameter_symbols, model)
        {
            return Err(SemanticError::new(
                expression.span(),
                "a parameter default arrow cannot capture a parameter of its containing callable",
            ));
        }
        if parameter_defaults_in_scope
            && default_contains_non_global_arrow_capture(expression, global_symbols, model)
        {
            return Err(SemanticError::new(
                expression.span(),
                "a parameter default arrow cannot capture a local binding outside its callable",
            ));
        }
        if matches!(defaults[index], Some(DefaultValue::PendingUndefined(_))) {
            if model.builtin_call(expression.span()) == Some(BuiltinCall::JsUndefined) {
                defaults[index] = Some(DefaultValue::Undefined);
                continue;
            }
            return Err(SemanticError::new(
                expression.span(),
                "parameter default is not the unshadowed `JS.undefined()` primitive",
            ));
        }
        let Expr::Ident(identifier) = expression else {
            continue;
        };
        let Some(bound) = model.identifier_symbol(identifier.span) else {
            continue;
        };
        if parameter_defaults_in_scope {
            let bound_parameter = params
                .iter()
                .position(|candidate| model.identifier_symbol(candidate.name.span) == Some(bound));
            if let Some(bound_parameter) = bound_parameter {
                if bound_parameter >= index {
                    return Err(SemanticError::new(
                        identifier.span,
                        "parameter defaults can only reference earlier parameters",
                    ));
                }
                defaults[index] = Some(DefaultValue::Parameter(bound_parameter));
                continue;
            }
        }
        defaults[index] = Some(DefaultValue::Symbol(bound));
    }
    Ok(defaults)
}

fn syntactic_js_undefined_default(expression: &Expr<'_, '_>) -> bool {
    matches!(
        expression,
        Expr::Call {
            callee,
            args,
            ..
        } if args.is_empty()
            && matches!(
                callee,
                Expr::Member {
                    object,
                    property: Ident { name: "undefined", .. },
                    ..
                } if matches!(object, Expr::Ident(Ident { name: "JS", .. }))
            )
    )
}

fn default_contains_arrow_capture(
    expression: &Expr<'_, '_>,
    parameter_symbols: &AHashSet<SymbolId>,
    model: &SemanticModel<'_>,
) -> bool {
    let mut arrow_spans = Vec::new();
    collect_default_arrow_spans(expression, &mut arrow_spans);
    model.identifier_symbols.iter().any(|(identifier, symbol)| {
        parameter_symbols.contains(symbol)
            && arrow_spans
                .iter()
                .any(|arrow| arrow.start <= identifier.start && identifier.end <= arrow.end)
    })
}

fn default_contains_non_global_arrow_capture(
    expression: &Expr<'_, '_>,
    global_symbols: &AHashSet<SymbolId>,
    model: &SemanticModel<'_>,
) -> bool {
    let mut arrow_spans = Vec::new();
    collect_default_arrow_spans(expression, &mut arrow_spans);
    model.identifier_symbols.iter().any(|(identifier, symbol)| {
        !global_symbols.contains(symbol)
            && arrow_spans.iter().any(|arrow| {
                arrow.start <= identifier.start
                    && identifier.end <= arrow.end
                    && model.symbols.get(symbol.0 as usize).is_some_and(|symbol| {
                        symbol.span.start < arrow.start || symbol.span.end > arrow.end
                    })
            })
    })
}

fn collect_default_arrow_spans(expression: &Expr<'_, '_>, spans: &mut Vec<Span>) {
    match expression {
        Expr::ArrowFunction { span, .. } => spans.push(*span),
        Expr::ArrayLiteral { elements, .. } => {
            for element in *elements {
                if let ArrayElement::Value(value) = element {
                    collect_default_arrow_spans(value, spans);
                }
            }
        }
        Expr::StructLiteral { values, .. } | Expr::New { args: values, .. } => {
            for value in *values {
                collect_default_arrow_spans(value, spans);
            }
        }
        _ => {}
    }
}

fn finalize_default_bindings_in_signature<'src>(
    signature: &mut FunctionType<'src>,
    bindings: &AHashMap<Span, SymbolId>,
    builtins: &AHashMap<Span, BuiltinCall>,
) -> Result<(), SemanticError> {
    for parameter in &mut signature.params {
        finalize_default_bindings_in_type(parameter, bindings, builtins)?;
    }
    for default in signature.defaults.iter_mut().flatten() {
        finalize_default_binding(default, bindings, builtins)?;
    }
    finalize_default_bindings_in_type(&mut signature.return_type, bindings, builtins)
}

fn finalize_default_binding<'src>(
    default: &mut DefaultValue<'src>,
    bindings: &AHashMap<Span, SymbolId>,
    builtins: &AHashMap<Span, BuiltinCall>,
) -> Result<(), SemanticError> {
    match default {
        DefaultValue::PendingIdentifier(span) => {
            let symbol = bindings.get(span).copied().ok_or_else(|| {
                SemanticError::new(*span, "missing analyzed parameter-default binding")
            })?;
            *default = DefaultValue::Symbol(symbol);
        }
        DefaultValue::PendingUndefined(span) => {
            if builtins.get(span) != Some(&BuiltinCall::JsUndefined) {
                return Err(SemanticError::new(
                    *span,
                    "parameter default is not the unshadowed `JS.undefined()` primitive",
                ));
            }
            *default = DefaultValue::Undefined;
        }
        DefaultValue::Array(values) => {
            for value in values {
                finalize_default_binding(value, bindings, builtins)?;
            }
        }
        DefaultValue::Struct { values, .. } => {
            for value in values {
                finalize_default_binding(value, bindings, builtins)?;
            }
        }
        DefaultValue::NewClass { args, .. } => {
            for argument in args {
                finalize_default_binding(argument, bindings, builtins)?;
            }
        }
        DefaultValue::Int(_)
        | DefaultValue::Float(_)
        | DefaultValue::String(_)
        | DefaultValue::Bool(_)
        | DefaultValue::Null
        | DefaultValue::Undefined
        | DefaultValue::Symbol(_)
        | DefaultValue::Parameter(_)
        | DefaultValue::Arrow(_) => {}
    }
    Ok(())
}

fn finalize_default_bindings_in_type<'src>(
    ty: &mut Type<'src>,
    bindings: &AHashMap<Span, SymbolId>,
    builtins: &AHashMap<Span, BuiltinCall>,
) -> Result<(), SemanticError> {
    match ty {
        Type::Array(value)
        | Type::Record(value)
        | Type::Set(value)
        | Type::Task(value)
        | Type::Generator(value)
        | Type::Nullable(value) => finalize_default_bindings_in_type(value, bindings, builtins)?,
        Type::Map(key, value) => {
            finalize_default_bindings_in_type(key, bindings, builtins)?;
            finalize_default_bindings_in_type(value, bindings, builtins)?;
        }
        Type::Union(members)
        | Type::StructInstance { args: members, .. }
        | Type::ClassInstance { args: members, .. } => {
            for member in members {
                finalize_default_bindings_in_type(member, bindings, builtins)?;
            }
        }
        Type::Function(signature) => {
            finalize_default_bindings_in_signature(signature, bindings, builtins)?;
        }
        Type::GenericFunction(function) => {
            finalize_default_bindings_in_signature(&mut function.signature, bindings, builtins)?;
        }
        Type::Int
        | Type::Float
        | Type::Enum(_)
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Void
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array
        | Type::Symbol
        | Type::Regex
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::Struct(_)
        | Type::Class(_)
        | Type::TypeParameter(_) => {}
    }
    Ok(())
}

fn strip_parameter_defaults_from_type(ty: &mut Type<'_>) {
    match ty {
        Type::Array(value)
        | Type::Record(value)
        | Type::Set(value)
        | Type::Task(value)
        | Type::Generator(value)
        | Type::Nullable(value) => strip_parameter_defaults_from_type(value),
        Type::Map(key, value) => {
            strip_parameter_defaults_from_type(key);
            strip_parameter_defaults_from_type(value);
        }
        Type::Union(members)
        | Type::StructInstance { args: members, .. }
        | Type::ClassInstance { args: members, .. } => {
            for member in members {
                strip_parameter_defaults_from_type(member);
            }
        }
        Type::Function(signature) => {
            signature.defaults.fill(None);
            for parameter in &mut signature.params {
                strip_parameter_defaults_from_type(parameter);
            }
            strip_parameter_defaults_from_type(&mut signature.return_type);
        }
        Type::GenericFunction(function) => {
            function.signature.defaults.fill(None);
            for parameter in &mut function.signature.params {
                strip_parameter_defaults_from_type(parameter);
            }
            strip_parameter_defaults_from_type(&mut function.signature.return_type);
        }
        Type::Int
        | Type::Float
        | Type::Enum(_)
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Void
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array
        | Type::Symbol
        | Type::Regex
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::Struct(_)
        | Type::Class(_)
        | Type::TypeParameter(_) => {}
    }
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
            .map(|element_value| match element_value {
                ArrayElement::Value(value) => literal_default_value(value, element),
                ArrayElement::Spread { .. } => None,
            })
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
            .map(|element| match element {
                ArrayElement::Value(value) => uncontextualized_default_value(value),
                ArrayElement::Spread { .. } => None,
            })
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
        Type::Record(value) => Type::Record(Box::new(substitute_type(value, substitutions))),
        Type::Map(key, value) => Type::Map(
            Box::new(substitute_type(key, substitutions)),
            Box::new(substitute_type(value, substitutions)),
        ),
        Type::Set(element) => Type::Set(Box::new(substitute_type(element, substitutions))),
        Type::Task(value) => Type::Task(Box::new(substitute_type(value, substitutions))),
        Type::Generator(value) => Type::Generator(Box::new(substitute_type(value, substitutions))),
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
        Type::Record(value) => contains_type_parameter(value, parameters),
        Type::Map(key, value) => {
            contains_type_parameter(key, parameters) || contains_type_parameter(value, parameters)
        }
        Type::Set(element) => contains_type_parameter(element, parameters),
        Type::Task(value) => contains_type_parameter(value, parameters),
        Type::Generator(value) => contains_type_parameter(value, parameters),
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
        (Type::Record(pattern), Type::Record(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Map(pattern_key, pattern_value), Type::Map(actual_key, actual_value)) => {
            infer_type_arguments(pattern_key, actual_key, parameters, substitutions, span)?;
            infer_type_arguments(pattern_value, actual_value, parameters, substitutions, span)?;
        }
        (Type::Set(pattern), Type::Set(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Task(pattern), Type::Task(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Generator(pattern), Type::Generator(actual)) => {
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
        (Type::TypeParameter("$js"), _) => !actual.is_void(),
        (Type::Float, Type::Int) => true,
        (Type::Array(expected), Type::Array(actual)) => {
            is_type_assignable(expected, actual) && is_type_assignable(actual, expected)
        }
        (Type::Record(expected), Type::Record(actual)) => expected == actual,
        (Type::Map(expected_key, expected_value), Type::Map(actual_key, actual_value)) => {
            expected_key == actual_key && expected_value == actual_value
        }
        (Type::Set(expected), Type::Set(actual)) => expected == actual,
        (Type::Task(expected), Type::Task(actual)) => is_type_assignable(expected, actual),
        (Type::Generator(expected), Type::Generator(actual)) => {
            is_type_assignable(expected, actual)
        }
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
                && expected
                    .defaults
                    .iter()
                    .enumerate()
                    .all(|(index, default)| {
                        default.as_ref().is_none_or(|expected| {
                            actual.defaults.get(index).and_then(Option::as_ref) == Some(expected)
                        })
                    })
                && is_type_assignable(&expected.return_type, &actual.return_type)
        }
        _ => false,
    }
}

fn assignment_binary_op(op: AssignmentOp) -> BinaryOp {
    match op {
        AssignmentOp::Assign | AssignmentOp::Nullish => {
            unreachable!("non-binary assignment has no binary operator")
        }
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
    }
}

fn statements_guarantee_return(statements: &[Stmt<'_, '_>]) -> bool {
    statements.iter().any(statement_guarantees_return)
}

fn statement_guarantees_return(statement: &Stmt<'_, '_>) -> bool {
    match statement {
        Stmt::Return { .. } | Stmt::Throw { .. } => true,
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
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            finally.is_some_and(|body| statements_guarantee_return(body))
                || (statements_guarantee_return(body)
                    && catch
                        .as_ref()
                        .is_none_or(|clause| statements_guarantee_return(clause.body)))
        }
        _ => false,
    }
}

fn class_type_name<'src>(ty: &Type<'src>) -> Option<&'src str> {
    class_type_parts(ty).map(|(name, _)| name)
}

fn class_type_parts<'ty, 'src>(ty: &'ty Type<'src>) -> Option<(&'src str, &'ty [Type<'src>])> {
    match ty {
        Type::Class(name) => Some((*name, &[])),
        Type::ClassInstance { name, args } => Some((*name, args)),
        _ => None,
    }
}

fn count_super_calls(statements: &[Stmt<'_, '_>]) -> usize {
    statements.iter().map(count_stmt_super_calls).sum()
}

fn count_stmt_super_calls(statement: &Stmt<'_, '_>) -> usize {
    match statement {
        Stmt::SuperCall { .. } => 1,
        Stmt::Block { body, .. } => count_super_calls(body),
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            count_stmt_super_calls(then_branch)
                + else_branch.map_or(0, |branch| count_stmt_super_calls(branch))
        }
        Stmt::While { body, .. }
        | Stmt::For { body, .. }
        | Stmt::ForIn { body, .. }
        | Stmt::ForOf { body, .. } => count_stmt_super_calls(body),
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            count_super_calls(body)
                + catch
                    .as_ref()
                    .map_or(0, |clause| count_super_calls(clause.body))
                + finally.map_or(0, |body| count_super_calls(body))
        }
        _ => 0,
    }
}

fn statement_contains_loop_control(statement: &Stmt<'_, '_>, inside_loop: bool) -> bool {
    match statement {
        Stmt::Break(_) | Stmt::Continue(_) => !inside_loop,
        Stmt::Block { body, .. } => body
            .iter()
            .any(|stmt| statement_contains_loop_control(stmt, inside_loop)),
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            statement_contains_loop_control(then_branch, inside_loop)
                || else_branch
                    .as_ref()
                    .is_some_and(|branch| statement_contains_loop_control(branch, inside_loop))
        }
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            body.iter()
                .any(|stmt| statement_contains_loop_control(stmt, inside_loop))
                || catch.as_ref().is_some_and(|clause| {
                    clause
                        .body
                        .iter()
                        .any(|stmt| statement_contains_loop_control(stmt, inside_loop))
                })
                || finally.as_ref().is_some_and(|body| {
                    body.iter()
                        .any(|stmt| statement_contains_loop_control(stmt, inside_loop))
                })
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } | Stmt::ForIn { body, .. } => {
            statement_contains_loop_control(body, true)
        }
        Stmt::ForOf {
            inline: true, body, ..
        } => statement_contains_loop_control(body, inside_loop),
        Stmt::ForOf { body, .. } => statement_contains_loop_control(body, true),
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
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            body.iter().any(statement_contains_break)
                || catch
                    .as_ref()
                    .is_some_and(|clause| clause.body.iter().any(statement_contains_break))
                || finally.is_some_and(|body| body.iter().any(statement_contains_break))
        }
        Stmt::While { .. } | Stmt::For { .. } | Stmt::ForIn { .. } | Stmt::ForOf { .. } => false,
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
        Type::TypeParameter("$js") => true,
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

fn is_stringifiable_array_element(ty: &Type<'_>) -> bool {
    match ty {
        // Native float formatting intentionally remains separate from JavaScript's
        // shortest Number-to-string algorithm, so accepting floats here would make
        // portable `join` silently target-dependent.
        Type::Int | Type::String | Type::Bool | Type::Null => true,
        Type::Nullable(inner) => is_stringifiable_array_element(inner),
        Type::Union(members) => members.iter().all(is_stringifiable_array_element),
        _ => false,
    }
}

fn json_stringify_type_supported(ty: &Type<'_>) -> bool {
    let scalar = |ty: &Type<'_>| {
        matches!(
            ty,
            Type::Int | Type::Enum(_) | Type::String | Type::Bool | Type::Null
        )
    };
    scalar(ty)
        || matches!(ty, Type::Nullable(inner) if scalar(inner))
        || matches!(ty, Type::Array(inner) | Type::Record(inner) if scalar(inner))
}

fn decode_source_string(value: &str) -> String {
    let encoded = format!("\"{value}\"");
    serde_json::from_str(&encoded).unwrap_or_else(|_| value.to_string())
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
        (Type::Record(lhs), Type::Record(rhs)) if lhs == rhs => Some(Type::Record(lhs.clone())),
        (Type::Task(lhs), Type::Task(rhs)) => {
            common_type(lhs, rhs).map(|value| Type::Task(Box::new(value)))
        }
        (Type::Generator(lhs), Type::Generator(rhs)) => {
            common_type(lhs, rhs).map(|value| Type::Generator(Box::new(value)))
        }
        _ if !matches!(lhs, Type::Void | Type::GenericFunction(_))
            && !matches!(rhs, Type::Void | Type::GenericFunction(_)) =>
        {
            Some(normalize_union(vec![lhs.clone(), rhs.clone()]))
        }
        _ => None,
    }
}

fn nullish_present_type<'a, 'src>(ty: &'a Type<'src>) -> Option<&'a Type<'src>> {
    match ty {
        Type::Nullable(inner) => Some(inner),
        Type::Null => Some(ty),
        _ => None,
    }
}

fn optional_result_type<'src>(ty: Type<'src>, span: Span) -> Result<Type<'src>, SemanticError> {
    match ty {
        Type::Void => Err(SemanticError::new(
            span,
            "optional access cannot materialize a nullable `void` value",
        )),
        Type::Function(_) | Type::GenericFunction(_) => Err(SemanticError::new(
            span,
            "optional method calls are not yet supported; coalesce the receiver before calling",
        )),
        Type::Nullable(_) => Ok(ty),
        ty => Ok(Type::Nullable(Box::new(ty))),
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

fn is_js_value(ty: &Type<'_>) -> bool {
    matches!(ty, Type::TypeParameter("$js"))
}

fn is_js_index_type(ty: &Type<'_>) -> bool {
    matches!(ty, Type::Int | Type::Float | Type::String) || is_js_value(ty)
}

fn is_js_value_or_nullable_js_value(ty: &Type<'_>) -> bool {
    match ty {
        Type::TypeParameter("$js") => true,
        Type::Nullable(inner) => is_js_value(inner),
        _ => false,
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
    if is_js_value_or_nullable_js_value(value) {
        return if matches!(target, Type::Float | Type::String | Type::Bool) {
            Ok(())
        } else {
            Err(SemanticError::new(
                span,
                format!(
                    "a `JsValue` cannot be soundly narrowed to `{target}`; use `float` for JavaScript numbers, `.isArray()` for untyped arrays, or `.truthy()`/`.isObject()` without narrowing"
                ),
            ))
        };
    }
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
        (lhs, rhs) if is_js_value(lhs) || is_js_value(rhs) => {
            let other = if is_js_value(lhs) { rhs } else { lhs };
            matches!(
                other,
                Type::TypeParameter("$js")
                    | Type::Null
                    | Type::Bool
                    | Type::String
                    | Type::Int
                    | Type::Float
            ) || is_js_value(other)
        }
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
        | Type::GenericFunction(_)
        | Type::Void => false,
        Type::Function(_) => true,
        _ => true,
    }
}

fn index_key_type<'src>(ty: &Type<'src>) -> Option<Type<'src>> {
    match ty {
        Type::Array(_) | Type::String => Some(Type::Int),
        Type::Record(_) => Some(Type::String),
        ty if crate::typed_array::is_typed_array_type(ty) => Some(Type::Int),
        Type::Union(members) => {
            let mut keys = members.iter().map(index_key_type);
            let first = keys.next().flatten()?;
            keys.all(|key| key.as_ref() == Some(&first))
                .then_some(first)
        }
        _ => None,
    }
}

fn index_value_type<'src>(ty: &Type<'src>, writable: bool) -> Option<Type<'src>> {
    match ty {
        Type::TypeParameter("$js") => Some(Type::TypeParameter("$js")),
        Type::Array(element) => Some(element.as_ref().clone()),
        Type::Record(value) if writable => Some(value.as_ref().clone()),
        Type::Record(value) => Some(nullable_type(value.as_ref().clone())),
        Type::String if !writable => Some(Type::String),
        ty if crate::typed_array::is_typed_array_type(ty) => Some(
            crate::typed_array::TypedArrayKind::from_type(ty)
                .expect("typed array type")
                .index_value_type(),
        ),
        Type::Union(members) => {
            let values = members
                .iter()
                .map(|member| index_value_type(member, writable))
                .collect::<Option<Vec<_>>>()?;
            if writable {
                let first = values.first()?.clone();
                values.iter().all(|value| value == &first).then_some(first)
            } else {
                Some(normalize_union(values))
            }
        }
        _ => None,
    }
}

fn indexed_collection_has_length(ty: &Type<'_>) -> bool {
    match ty {
        Type::Array(_) | Type::String => true,
        ty if crate::typed_array::is_typed_array_type(ty) => true,
        Type::Union(members) => members.iter().all(indexed_collection_has_length),
        _ => false,
    }
}

fn is_stringable(ty: &Type<'_>) -> bool {
    match ty {
        Type::Union(members) => members.iter().all(is_stringable),
        _ => matches!(
            ty,
            Type::String | Type::Int | Type::Float | Type::Bool | Type::TypeParameter("$js")
        ),
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
    fn nested_parameter_defaults_may_capture_outer_locals() {
        check("func(int)->int factory(int defaultSize){return (int size=defaultSize)=>size;}")
            .unwrap();
        let shadowed = check("int value=9;auto make=(int value=value)=>value;").unwrap_err();
        assert!(
            shadowed
                .message
                .contains("parameter defaults can only reference earlier parameters"),
            "{shadowed}"
        );
    }

    #[test]
    fn checks_exhaustive_enum_matches_without_pattern_false_positives() {
        check("enum Status{Draft,Active,Sold}string label(Status value){return match(value){Status.Draft=>\"draft\",Status.Active=>\"active\",Status.Sold=>\"sold\"};}").unwrap();
        check("enum Status{Draft,Active}int code(Status value){return match(value){Status.Draft=>0,_=>1};}").unwrap();

        let missing = check("enum Status{Draft,Active}int code(Status value){return match(value){Status.Draft=>0};}").unwrap_err();
        assert!(missing.message.contains("non-exhaustive"), "{missing}");

        let duplicate = check("enum Status{Draft,Active}int code(Status value){return match(value){Status.Draft=>0,Status.Draft=>1,Status.Active=>2};}").unwrap_err();
        assert!(
            duplicate.message.contains("duplicate match arm"),
            "{duplicate}"
        );

        let wrong_enum = check("enum Status{Draft,Active}enum Other{Draft}int code(Status value){return match(value){Other.Draft=>0,Status.Active=>1};}").unwrap_err();
        assert!(
            wrong_enum.message.contains("expected `Status`"),
            "{wrong_enum}"
        );

        let unknown = check("enum Status{Draft,Active}int code(Status value){return match(value){Status.Missing=>0,Status.Active=>1};}").unwrap_err();
        assert!(
            unknown.message.contains("has no variant `Missing`"),
            "{unknown}"
        );

        let wildcard = check("enum Status{Draft,Active}int code(Status value){return match(value){_=>0,Status.Active=>1};}").unwrap_err();
        assert!(
            wildcard.message.contains("must appear once and last"),
            "{wildcard}"
        );
    }

    #[test]
    fn checks_expression_if_types_and_narrowing() {
        check("int choose(bool flag){return if(flag){1}else{2};}").unwrap();
        check("int choose(int? value){return if(value!=null){value}else{0};}").unwrap();

        let condition = check("int choose(int flag){return if(flag){1}else{2};}").unwrap_err();
        assert!(condition.message.contains("expected `bool`"), "{condition}");

        let arms =
            check("void choose(bool flag){auto value=if(flag){1}else{print(2)};}").unwrap_err();
        assert!(arms.message.contains("incompatible types"), "{arms}");
    }

    #[test]
    fn checks_scalar_literal_matches() {
        check("string label(int value){return match(value){-1=>\"negative\",0=>\"zero\",_=>\"positive\"};}").unwrap();
        check("int flag(bool value){return match(value){true=>1,false=>0};}").unwrap();
        check("int label(string value){return match(value){\"open\"=>1,_=>0};}").unwrap();

        let missing = check("int label(int value){return match(value){0=>1};}").unwrap_err();
        assert!(
            missing.message.contains("requires a final `_`"),
            "{missing}"
        );
        let duplicate =
            check("int label(string value){return match(value){\"x\"=>1,\"x\"=>2,_=>0};}")
                .unwrap_err();
        assert!(
            duplicate.message.contains("duplicate match arm"),
            "{duplicate}"
        );
        let wrong = check("int label(bool value){return match(value){0=>1,_=>0};}").unwrap_err();
        assert!(wrong.message.contains("cannot match `bool`"), "{wrong}");
    }

    #[test]
    fn checks_structural_record_presence_and_writes_soundly() {
        check("Record<int> values=record{alpha:1,beta:2};int first=values.alpha??0;values.gamma=3;int third=values[\"gamma\"]??0;").unwrap();

        let duplicate = check("auto values=record{alpha:1,alpha:2};").unwrap_err();
        assert!(
            duplicate.message.contains("duplicate record key"),
            "{duplicate}"
        );

        let escaped_duplicate =
            check(r#"auto values=record{alpha:1,"\u0061lpha":2};"#).unwrap_err();
        assert!(
            escaped_duplicate.message.contains("duplicate record key"),
            "{escaped_duplicate}"
        );

        let empty = check("auto values=record{};").unwrap_err();
        assert!(empty.message.contains("cannot infer"), "{empty}");

        let mixed = check("Record<int> values=record{alpha:1,beta:\"wrong\"};").unwrap_err();
        assert!(mixed.message.contains("expected `Record<int>`"), "{mixed}");

        let unsafe_update =
            check("Record<int> values=record{alpha:1};values.alpha+=1;").unwrap_err();
        assert!(
            unsafe_update.message.contains("only direct `=`"),
            "{unsafe_update}"
        );

        let missing_without_check =
            check("Record<int> values=record{alpha:1};int value=values.missing;").unwrap_err();
        assert!(
            missing_without_check.message.contains("expected `int`"),
            "{missing_without_check}"
        );
    }

    #[test]
    fn checks_portable_record_object_and_json_operations() {
        check(r#"Record<int> values=record{alpha:1};string[] keys=Object.keys(values);int[] entries=Object.values(values);bool has=Object.hasOwn(values,"alpha");Record<int> merged=Object.assign(values,record{beta:2});string json=JSON.stringify(merged);JsValue parsed=JSON.parse(json);bool object=parsed.isObject();"#).unwrap();

        let mismatch = check(
            "Record<int> target=record{a:1};Record<string> source=record{a:\"x\"};Object.assign(target,source);",
        )
        .unwrap_err();
        assert!(
            mismatch.message.contains("expected `Record<int>`"),
            "{mismatch}"
        );

        let float = check("float value=1.5;string json=JSON.stringify(value);").unwrap_err();
        assert!(
            float.message.contains("does not support `float` portably"),
            "{float}"
        );
    }

    #[test]
    fn checks_first_class_javascript_abi_operations() {
        check(
            r#"
                JsValue object=JS.object();
                JsValue array=JS.array();
                JS.set(object,"answer",42);
                JsValue answer=JS.get(object,"answer");
                bool present=JS.has(object,"answer");
                float length=JS.push(array,answer);
                JsValue popped=JS.pop(array);
                string text=JS.string(popped);
                float numeric=JS.number(popped);
                JsValue invoked=JS.invoke(object,"method",answer);
                bool missing=JS.isUndefined(JS.undefined());
                JS.delete(object,"answer");
            "#,
        )
        .unwrap();

        let unknown = check("JsValue value=JS.unknown();").unwrap_err();
        assert!(
            unknown.message.contains("unknown identifier `JS`"),
            "{unknown}"
        );
        let arity = check("JsValue value=JS.call();").unwrap_err();
        assert!(arity.message.contains("expects at least 2"), "{arity}");

        check("extern JsValue read();JsValue cb=read();JsValue result=cb(1,\"x\");").unwrap();
    }

    #[test]
    fn builtin_namespaces_do_not_override_shadowing_bindings() {
        check(
            r#"
                struct Api { int value; }
                int read(Api Object) { return Object.value; }
                int run(func(int)->int print) { return print(41); }
            "#,
        )
        .unwrap();
    }

    #[test]
    fn checks_for_of_element_types_and_iterables() {
        check("int[] values=[1,2];for(int value of values){print(value);}Float32Array floats=new Float32Array(2);for(float value of floats){print(value);}").unwrap();

        let wrong = check("string[] values=[\"a\"];for(int value of values){}").unwrap_err();
        assert!(wrong.message.contains("expected `int`"), "{wrong}");

        let string = check("for(string value of \"text\"){}").unwrap_err();
        assert!(string.message.contains("array or typed array"), "{string}");
    }

    #[test]
    fn checks_inline_for_requires_const_list() {
        check("int total=0;inline for(int value of [1,2,3]){total+=value;}").unwrap();

        let runtime = check("int[] values=[1,2];inline for(int value of values){}").unwrap_err();
        assert!(
            runtime.message.contains("constant array literal"),
            "{runtime}"
        );

        let control = check("inline for(int value of [1,2]){break;}").unwrap_err();
        assert!(
            control.message.contains("`break` or `continue`"),
            "{control}"
        );
    }

    #[test]
    fn checks_array_and_record_spreads_without_widening_unsafely() {
        check("int[] base=[1,2];int[] values=[0,...base,3];Record<int> source=record{a:1};Record<int> merged=record{...source,b:2};").unwrap();

        let typed_array =
            check("Uint8Array bytes=new Uint8Array(2);int[] values=[...bytes];").unwrap_err();
        assert!(
            typed_array
                .message
                .contains("array spread requires an array"),
            "{typed_array}"
        );

        let array_mismatch =
            check("string[] words=[\"x\"];int[] values=[1,...words];").unwrap_err();
        assert!(
            array_mismatch.message.contains("expected `int`")
                || array_mismatch.message.contains("expected `int[]`"),
            "{array_mismatch}"
        );

        let record_mismatch = check(
            "Record<int> numbers=record{a:1};Record<string> words=record{a:\"x\"};Record<int> merged=record{...numbers,...words};",
        )
        .unwrap_err();
        assert!(
            record_mismatch.message.contains("expected `Record<int>`"),
            "{record_mismatch}"
        );

        let wrong_kind =
            check("int[] values=[1];Record<int> merged=record{...values};").unwrap_err();
        assert!(
            wrong_kind
                .message
                .contains("record spread requires a record"),
            "{wrong_kind}"
        );
    }

    #[test]
    fn contextual_record_literals_widen_fresh_values_without_alias_covariance() {
        check("struct Entry{int value;}Record<Entry> source=record{item:Entry{1}};Record<JsValue> direct=record{item:Entry{2}};Record<JsValue> spread=record{...source};").unwrap();

        let alias = check(
            "struct Entry{int value;}Record<Entry> source=record{item:Entry{1}};Record<JsValue> alias=source;",
        )
        .unwrap_err();
        assert!(
            alias
                .message
                .contains("expected `Record<JsValue>`, found `Record<Entry>`"),
            "{alias}"
        );
    }

    #[test]
    fn rejects_nominal_array_widening_to_js_value() {
        let error =
            check("struct Entry{int value;}Entry[] entries=[Entry{1}];JsValue[] erased=entries;")
                .unwrap_err();
        assert!(
            error
                .message
                .contains("expected `JsValue[]`, found `Entry[]`"),
            "{error}"
        );

        check("struct Entry{int value;}Entry[] entries=[Entry{1}];Entry[] exact=entries;JsValue[] contextual=[Entry{2}];").unwrap();
    }

    #[test]
    fn rejects_numeric_mutable_array_widening() {
        let error = check("int[] integers=[1,2];float[] widened=integers;").unwrap_err();
        assert!(
            error.message.contains("expected `float[]`, found `int[]`"),
            "{error}"
        );

        check("int[] integers=[1,2];int[] exact=integers;float[] contextual=[1,2];").unwrap();
    }

    #[test]
    fn rejects_writes_through_heterogeneous_collection_unions() {
        let error = check("void overwrite(int[]|string[] values){values[0]=1;}").unwrap_err();
        assert!(
            error
                .message
                .contains("cannot assign through an index on `int[] | string[]`"),
            "{error}"
        );

        let member = check("void resize(int[]|string values){values.length=0;}").unwrap_err();
        assert!(
            member
                .message
                .contains("cannot assign through member `length` on union"),
            "{member}"
        );

        check("void overwrite(int[]|Int32Array values){values[0]=1;}").unwrap();
    }

    #[test]
    fn requires_every_union_callable_member_to_accept_a_call() {
        let error = check(
            "int zero(){return 0;}int one(int value){return value;}(func()->int)|(func(int)->int) choose(bool first){if(first){return zero;}return one;}int value=choose(false)();",
        )
        .unwrap_err();
        assert!(
            error.message.contains("cannot call a value of type")
                && error.message.contains("with 0 arguments"),
            "{error}"
        );

        check(
            "int fromInt(int value){return value;}int fromFloat(float value){return 1;}(func(int)->int)|(func(float)->int) choose(bool first){if(first){return fromInt;}return fromFloat;}int value=choose(false)(1);",
        )
        .unwrap();

        let omitted_default = check(
            "int one(int value=1){return value;}int two(int value=2){return value;}auto choices=[one,two];auto chosen=choices[0];int value=chosen();",
        )
        .unwrap_err();
        assert!(
            omitted_default
                .message
                .contains("union calls require every argument explicitly"),
            "{omitted_default}"
        );

        check(
            "int one(int value=1){return value;}int two(int value=2){return value;}auto choices=[one,two];auto chosen=choices[0];int value=chosen(3);",
        )
        .unwrap();
    }

    #[test]
    fn revalidates_earlier_generic_arguments_after_inference_widens() {
        let call = check(
            "struct Entry{int value;}void copy<T>(T[] target,T[] source){target[0]=source[0];}Entry[] target=[Entry{1}];JsValue[] source=[\"bad\"];copy(target,source);",
        )
        .unwrap_err();
        assert!(
            call.message
                .contains("expected `JsValue[]`, found `Entry[]`"),
            "{call}"
        );

        let constructor = check(
            "struct Entry{int value;}class Pair<T>{T[] left;T[] right;init(T[] left,T[] right){this.left=left;this.right=right;}}Entry[] entries=[Entry{1}];JsValue[] values=[\"bad\"];auto pair=new Pair(entries,values);",
        )
        .unwrap_err();
        assert!(
            constructor
                .message
                .contains("expected `JsValue[]`, found `Entry[]`"),
            "{constructor}"
        );
    }

    #[test]
    fn requires_a_common_index_key_across_union_members() {
        check("void read(Record<int>|Record<string> values){auto value=values[\"key\"];} ")
            .unwrap();

        let wrong_key =
            check("void read(Record<int>|Record<string> values){auto value=values[0];}")
                .unwrap_err();
        assert!(
            wrong_key.message.contains("expected `string`"),
            "{wrong_key}"
        );

        let mixed =
            check("void read(int[]|Record<int> values){auto value=values[0];}").unwrap_err();
        assert!(
            mixed.message.contains("cannot index a value of type"),
            "{mixed}"
        );
    }

    #[test]
    fn infers_nullable_destructured_values_and_nonnullable_array_rest() {
        check("int[] values=[1,2];auto [first,,third,...rest]=values;int a=first??0;int b=third??0;int[] tail=rest;Record<int> source=record{x:1};auto {x,missing,...remaining}=source;int c=x??0;int d=missing??0;Record<int> others=remaining;").unwrap();

        let unsafe_value =
            check("int[] values=[1];auto [first]=values;int required=first;").unwrap_err();
        assert!(
            unsafe_value.message.contains("expected `int`"),
            "{unsafe_value}"
        );

        let non_array = check("auto [first]=\"text\";").unwrap_err();
        assert!(
            non_array
                .message
                .contains("array destructuring requires an array"),
            "{non_array}"
        );

        let duplicate =
            check("Record<int> source=record{a:1};auto {a,a:other}=source;").unwrap_err();
        assert!(
            duplicate.message.contains("duplicate record binding key"),
            "{duplicate}"
        );
    }

    #[test]
    fn infers_auto_and_checks_array_map() {
        check("int[] values=[1,2,3]; auto doubled=values.map((int value)=>value*2);").unwrap();
        check("int[] values=[1,2,3];values.map((int value)=>print(value));").unwrap();
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
    fn requires_numeric_assignable_update_targets() {
        let literal = check("int value=++1;").unwrap_err();
        assert!(
            literal
                .message
                .contains("expression is not an assignable location"),
            "{literal}"
        );

        let string = check("string value=\"ready\";value++;").unwrap_err();
        assert!(
            string
                .message
                .contains("operator `++` requires a numeric target"),
            "{string}"
        );
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
    fn empty_record_assigns_to_js_value() {
        check("JsValue empty=record{};empty[\"k\"]=1;").unwrap();
    }

    #[test]
    fn allows_js_value_index_assignment() {
        check(
            r#"
                void set(JsValue options){
                    options["duration"]=0.0;
                    options["type"]="keyframes";
                    string key="delay";
                    options[key]=1.0;
                }
            "#,
        )
        .unwrap();
    }

    #[test]
    fn narrows_through_logical_and() {
        check(
            r#"
                string label(JsValue? value){
                    if(value!=null && value is string){return value;}
                    return "none";
                }
                bool ready(string? name, JsValue? element){
                    if(name!=null && element!=null){return name.length>0 && element.truthy();}
                    return false;
                }
            "#,
        )
        .unwrap();
    }

    #[test]
    fn narrows_nullable_js_value_with_portable_guards() {
        check(
            r#"
                bool isName(JsValue? value){
                    if(value is string){return value.startsWith("--");}
                    return false;
                }
                bool isZero(JsValue? value){
                    if(value is float){return value==0.0;}
                    return value==null;
                }
            "#,
        )
        .unwrap();
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
    fn narrows_javascript_values_only_when_the_runtime_check_proves_the_type() {
        check(
            "bool inspect(JsValue value){if(value is string){print(value);}else if(value is float){print(value);}else if(value is bool){print(value);}return value.isArray()||value.isObject()||value.truthy();}",
        )
        .unwrap();

        for target in ["int", "string[]", "func(int)->int"] {
            let error = check(&format!(
                "bool inspect(JsValue value){{return value is {target};}}"
            ))
            .unwrap_err();
            assert!(
                error.message.contains("cannot be soundly narrowed"),
                "{error}"
            );
        }
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
    fn js_undefined_default_requires_the_unshadowed_builtin() {
        check("JsValue read(JsValue JS,JsValue value=JS.undefined()){return value;}").unwrap();

        let error =
            check("auto read=(JsValue JS,JsValue value=JS.undefined())=>value;").unwrap_err();
        assert!(error.message.contains("undefined"), "{error}");
    }

    #[test]
    fn rejects_transported_defaults_that_depend_on_local_bindings() {
        let error = check("[1,2].map((int seed)=>(int value=seed)=>value)[1]();").unwrap_err();
        assert!(error.message.contains("local binding"), "{error}");

        check("int seed=7;int value=((int current=seed)=>current)();").unwrap();
        check("int value=((int first,int second=first)=>second)(7);").unwrap();
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
    fn rejects_unimplemented_struct_equality_and_allows_function_equality() {
        let struct_error = check(
            "struct Pair{int left;int right;}Pair a=Pair{1,2};Pair b=Pair{1,2};bool same=a==b;",
        )
        .unwrap_err();
        assert!(struct_error.message.contains("cannot be applied"));

        check("auto a=(int value)=>value;auto b=(int value)=>value;bool same=a==b;").unwrap();
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
    fn resolves_generic_struct_literals_from_context() {
        check("struct Box<T>{T value;}Box<int> box=Box{7};int value=box.value;").unwrap();

        let missing = check("struct Box<T>{T value;}auto box=Box{7};").unwrap_err();
        assert!(
            missing
                .message
                .contains("requires a contextual `Box<...>` type"),
            "{missing}"
        );
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
    fn validates_compact_array_and_typed_array_bulk_methods() {
        check(
            "int[] values=[1,2,3];bool has=values.includes(2);bool tail=values.includes(1,-2);string text=values.join(\"-\");bool any=values.some((int value)=>value>2);bool all=values.every((int value)=>value>0);int found=values.findIndex((int value)=>value==2);int[] combined=values.concat([4,5]);values.copyWithin(1,0,2).reverse();Uint8Array a=new Uint8Array(8);Uint8Array b=new Uint8Array(2);a.fill(7,1,6);a.set(b,2);a.copyWithin(4,1,3);",
        )
        .unwrap();

        let join_error =
            check("class Box{}Box[] boxes=[];string text=boxes.join(\",\");").unwrap_err();
        assert!(
            join_error.message.contains("cannot be joined portably"),
            "{join_error}"
        );

        let float_join_error =
            check("float[] values=[0.1];string text=values.join(\",\");").unwrap_err();
        assert!(
            float_join_error
                .message
                .contains("cannot be joined portably"),
            "{float_join_error}"
        );

        let set_error = check(
            "Uint8Array bytes=new Uint8Array(2);Int16Array words=new Int16Array(2);bytes.set(words);",
        )
        .unwrap_err();
        assert!(
            set_error.message.contains("expected `Uint8Array`"),
            "{set_error}"
        );

        let predicate_error =
            check("int[] values=[1];bool found=values.some((int value)=>value);").unwrap_err();
        assert!(
            predicate_error.message.contains("must return `bool`"),
            "{predicate_error}"
        );
    }

    #[test]
    fn validates_compact_string_search_and_repeat_methods() {
        check(
            "string value=\"ababa\";int first=value.indexOf(\"ba\",1);int last=value.lastIndexOf(\"ba\");string repeated=value.repeat(2);",
        )
        .unwrap();

        let position_error = check("int value=\"abc\".indexOf(\"a\",true);").unwrap_err();
        assert!(
            position_error.message.contains("expected `int`"),
            "{position_error}"
        );
    }

    #[test]
    fn validates_javascript_regex_construction_testing_and_metadata() {
        check("Regex pattern=new Regex(\"sale\",\"gi\");bool found=pattern.test(\"SALE\");JsValue matched=pattern.exec(\"SALE\");string source=pattern.source;string flags=pattern.flags;bool global=pattern.global;bool insensitive=pattern.ignoreCase;float index=pattern.lastIndex;").unwrap();

        let arity = check("Regex pattern=new Regex();").unwrap_err();
        assert!(
            arity.message.contains("expects 1 or 2 arguments"),
            "{arity}"
        );

        let argument = check("Regex pattern=new Regex(1);").unwrap_err();
        assert!(argument.message.contains("expected `string`"), "{argument}");

        let unknown = check("Regex pattern=new Regex(\"x\");pattern.matches(\"x\");").unwrap_err();
        assert!(
            unknown.message.contains("has no member `matches`"),
            "{unknown}"
        );
    }

    #[test]
    fn validates_nullish_operators_without_truthiness_coercion() {
        check(
            "string? optional=null;string first=optional??\"fallback\";string second=null??\"literal\";optional??=\"stored\";int?[] values=[null];values[0]??=7;",
        )
        .unwrap();

        let non_nullable =
            check("string value=\"\";string result=value??\"fallback\";").unwrap_err();
        assert!(non_nullable.message.contains("nullable left operand"));

        let invalid_store = check("int? value=null;value??=\"wrong\";").unwrap_err();
        assert!(invalid_store.message.contains("expected `int?`"));
    }

    #[test]
    fn validates_optional_access_only_for_nullable_data() {
        check("int[]? values=null;int? length=values?.length;int? first=values?.[0];").unwrap();
        let non_nullable = check("int[] values=[1];int? first=values?.[0];").unwrap_err();
        assert!(non_nullable.message.contains("nullable receiver"));
        let method = check("int[]? values=null;auto mapped=values?.map;").unwrap_err();
        assert!(method.message.contains("must be called"), "{method}");
    }

    #[test]
    fn treats_number_as_non_wrapping_binary64() {
        check("number value=1;number next=value*3+0.5;number step(number input){return input+1;}")
            .unwrap();
        let bitwise = check("number value=1;number shifted=value<<1;").unwrap_err();
        assert!(bitwise.message.contains("cannot be applied"), "{bitwise}");
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

        check("Map<JsValue,int> values=new Map<JsValue,int>();JsValue key=record{ok:1};values.set(key,1);").unwrap();

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
    fn allows_explicit_function_bindings_to_recurse() {
        check("func(int)->int loop=(int n)=>loop(n);int value=loop(1);").unwrap();
        check("JsValue self=(JsValue _)=>self;JsValue value=self;").unwrap();
        let inferred = check("auto fact=(int n)=>fact(n);").unwrap_err();
        assert!(
            inferred.message.contains("unknown identifier `fact`"),
            "{inferred}"
        );
        let during_init = check("int boom=boom+1;").unwrap_err();
        assert!(
            during_init
                .message
                .contains("cannot read `boom` in its own initializer"),
            "{during_init}"
        );
        let shadowing_during_init = check("int value=1;{int value=value+1;}").unwrap_err();
        assert!(
            shadowing_during_init
                .message
                .contains("cannot read `value` in its own initializer"),
            "{shadowing_during_init}"
        );
    }

    #[test]
    fn checks_typed_javascript_adapter_callback_conventions() {
        check(
            "JsValue method0=JS.method0((JsValue self)=>self);JsValue method1=JS.method1((JsValue self,JsValue value)=>value);JsValue method2=JS.method2((JsValue self,JsValue a,JsValue b)=>a);JsValue method3=JS.method3((JsValue self,JsValue a,JsValue b,JsValue c)=>a);JsValue methodRest=JS.methodRest((JsValue self,JsValue args)=>args);JsValue staticRest=JS.staticRest((JsValue args)=>args);JsValue constructed=JS.construct(method0);",
        )
        .unwrap();

        let arity = check("JS.method0((JsValue self,JsValue extra)=>self);").unwrap_err();
        assert!(
            arity.message.contains("expects 1 parameters, found 2"),
            "{arity}"
        );

        let method3_arity =
            check("JS.method3((JsValue self,JsValue a,JsValue b)=>a);").unwrap_err();
        assert!(
            method3_arity
                .message
                .contains("expects 4 parameters, found 3"),
            "{method3_arity}"
        );

        let parameter_type = check("JS.method1((int self,JsValue value)=>value);").unwrap_err();
        assert!(
            parameter_type
                .message
                .contains("callback parameter must be `JsValue`, found `int`"),
            "{parameter_type}"
        );

        let return_type = check("JS.staticRest((JsValue args)=>{});").unwrap_err();
        assert!(
            return_type
                .message
                .contains("expected `function(JsValue) -> JsValue`"),
            "{return_type}"
        );
    }

    #[test]
    fn allows_jsvalue_property_keys_on_jsvalue_bags() {
        check("extern JsValue obj;extern JsValue key;JsValue value=obj[key];obj[key]=value;")
            .unwrap();
        let error =
            check("extern JsValue obj;extern bool key;JsValue value=obj[key];").unwrap_err();
        assert!(
            error.message.contains("numeric, `string`, or `JsValue`"),
            "{error}"
        );
    }

    #[test]
    fn accepts_rebinding_captured_values() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int run(int seed){auto next=()=>{seed+=1;return seed;};return next();}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        assert!(semantics.assigned_symbols.iter().any(|symbol| {
            semantics
                .symbols()
                .get(symbol.0 as usize)
                .is_some_and(|symbol| symbol.name == "seed")
        }));
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
    fn accepts_closed_objects_and_merges_method_tables() {
        check("object Api{int add(int left,int right){return left+right;}}print(Api.add(1,2));")
            .unwrap();
        check(
            "object Api{int add(int left,int right){return left+right;}}object Api{int mul(int left,int right){return left*right;}}print(Api.add(1,2)+Api.mul(3,4));",
        )
        .unwrap();
        let constructed = check("object Api{int id(){return 1;}}Api value=new Api();").unwrap_err();
        assert!(
            constructed
                .message
                .contains("object `Api` cannot be constructed with `new`"),
            "{constructed}"
        );
        let duplicate = check(
            "object Api{int add(int left,int right){return left+right;}}object Api{int add(int left,int right){return left+right;}}",
        )
        .unwrap_err();
        assert!(
            duplicate.message.contains("duplicate member `add`"),
            "{duplicate}"
        );
        let clash = check("class Api{}object Api{int id(){return 1;}}").unwrap_err();
        assert!(
            clash.message.contains("duplicate type declaration `Api`"),
            "{clash}"
        );
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

    #[test]
    fn checks_async_and_exception_boundaries_without_narrowing_thrown_values() {
        check(
            "async int recover(){try{return await Task.reject(\"bad\");}catch(auto error){string text=error.message??\"caught\";return text.length;}finally{print(\"done\");}}",
        )
        .unwrap();
        check("try{throw null;}catch{}finally{print(1);}").unwrap();

        let error = check("try{throw 1;}catch(int error){print(error);}").unwrap_err();
        assert!(error.message.contains("`auto` or `JsValue`"), "{error}");
        let error = check("int value=await Task.resolve(1);").unwrap_err();
        assert!(
            error.message.contains("inside an async function"),
            "{error}"
        );
    }

    #[test]
    fn validates_flattened_generic_single_inheritance_without_unsound_overrides() {
        check(
            "class Base<T>{T value;init(T value){this.value=value;}T get(){return this.value;}}class Child extends Base<int>{int bonus;init(int value,int bonus){super(value);this.bonus=bonus;}int total(){return this.value+this.bonus;}}Child child=new Child(4,3);Base<int> base=child;int inherited=base.get();int total=child.total();",
        )
        .unwrap();

        let missing_super = check(
            "class Base{init(int value){print(value);}}class Child extends Base{init(){print(1);}}",
        )
        .unwrap_err();
        assert!(missing_super.message.contains("must begin with `super"));

        let override_error = check(
            "class Base{int value(){return 1;}}class Child extends Base{int value(){return 2;}}",
        )
        .unwrap_err();
        assert!(override_error
            .message
            .contains("cannot override inherited member"));

        let nested_super = check(
            "class Base{init(){}}class Child extends Base{init(){super();auto later=()=>{super();};}}",
        )
        .unwrap_err();
        assert!(nested_super.message.contains("derived class constructor"));

        check(
            "class Child extends Base<int>{}class Base<T>{T value;}Child child=new Child();child.value=7;Base<int> base=child;",
        )
        .unwrap();

        let cycle = check("class Left extends Right{}class Right extends Left{}").unwrap_err();
        assert!(cycle.message.contains("inheritance cycle"), "{cycle}");
    }

    #[test]
    fn validates_generator_boundaries_delegation_and_iteration() {
        check(
            "generator int range(int stop){for(int i=0;i<stop;i++){yield i;}}generator int values(){yield* [7,8];yield* range(2);}int sum=0;for(int value of values()){sum+=value;}",
        )
        .unwrap();

        let outside = check("yield 1;").unwrap_err();
        assert!(outside.message.contains("inside a generator"));
        let wrong = check("generator int values(){yield \"bad\";}").unwrap_err();
        assert!(wrong.message.contains("expected `int`"));
        let nested = check("generator int values(){auto bad=()=>{yield 1;};yield 2;}").unwrap_err();
        assert!(nested.message.contains("inside a generator"));
        let returned = check("generator int values(){return 1;}").unwrap_err();
        assert!(returned.message.contains("expected return type `void`"));
    }
}
