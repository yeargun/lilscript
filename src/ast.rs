use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ident<'src> {
    pub name: &'src str,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeRef<'ast, 'src> {
    pub kind: TypeKind<'ast, 'src>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind<'ast, 'src> {
    Int,
    Float,
    String,
    Bool,
    Void,
    Auto,
    Named {
        name: &'src str,
        args: &'ast [TypeRef<'ast, 'src>],
    },
    Array(&'ast TypeRef<'ast, 'src>),
    Nullable(&'ast TypeRef<'ast, 'src>),
    Union(&'ast [TypeRef<'ast, 'src>]),
    Function {
        params: &'ast [TypeRef<'ast, 'src>],
        return_type: &'ast TypeRef<'ast, 'src>,
    },
}

impl<'ast, 'src> TypeRef<'ast, 'src> {
    pub const fn is_auto(self) -> bool {
        matches!(self.kind, TypeKind::Auto)
    }

    pub const fn named(name: &'src str, span: Span) -> Self {
        Self {
            kind: TypeKind::Named { name, args: &[] },
            span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program<'ast, 'src> {
    pub imports: &'ast [ImportDecl<'ast, 'src>],
    pub foreign_imports: &'ast [ForeignImportDecl<'ast, 'src>],
    pub dynamic_imports: &'ast [DynamicImportDecl<'ast, 'src>],
    pub exports: &'ast [ExportDecl<'src>],
    pub items: &'ast [Item<'ast, 'src>],
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForeignImportDecl<'ast, 'src> {
    pub specifiers: &'ast [ImportSpecifier<'src>],
    pub source: &'src str,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicImportDecl<'ast, 'src> {
    pub module: u32,
    pub source: &'src str,
    pub span: Span,
    pub exports: &'ast [DynamicExport<'src>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicExport<'src> {
    pub exported: &'src str,
    pub binding: &'src str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl<'ast, 'src> {
    pub specifiers: &'ast [ImportSpecifier<'src>],
    pub source: &'src str,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportSpecifier<'src> {
    pub imported: Ident<'src>,
    pub local: Ident<'src>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDecl<'src> {
    pub local: Ident<'src>,
    pub exported: Ident<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
// Arena-owned AST nodes stay inline so parsing does not add per-statement heap boxes.
#[allow(clippy::large_enum_variant)]
pub enum Item<'ast, 'src> {
    Enum(EnumDecl<'ast, 'src>),
    Struct(StructDecl<'ast, 'src>),
    Class(ClassDecl<'ast, 'src>),
    ExternClass(ExternClassDecl<'ast, 'src>),
    Function(FunctionDecl<'ast, 'src>),
    Extern(ExternDecl<'ast, 'src>),
    ExternGlobal(ExternGlobalDecl<'ast, 'src>),
    Stmt(Stmt<'ast, 'src>),
}

impl<'ast, 'src> Item<'ast, 'src> {
    pub const fn span(&self) -> Span {
        match self {
            Self::Enum(decl) => decl.span,
            Self::Struct(decl) => decl.span,
            Self::Class(decl) => decl.span,
            Self::ExternClass(decl) => decl.span,
            Self::Function(decl) => decl.span,
            Self::Extern(decl) => decl.span,
            Self::ExternGlobal(decl) => decl.span,
            Self::Stmt(stmt) => stmt.span(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl<'ast, 'src> {
    pub name: Ident<'src>,
    pub variants: &'ast [Ident<'src>],
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl<'ast, 'src> {
    pub name: Ident<'src>,
    pub type_params: &'ast [Ident<'src>],
    pub fields: &'ast [FieldDecl<'ast, 'src>],
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl<'ast, 'src> {
    pub name: Ident<'src>,
    pub type_params: &'ast [Ident<'src>],
    pub base: Option<TypeRef<'ast, 'src>>,
    pub members: &'ast [ClassMember<'ast, 'src>],
    pub object: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternClassDecl<'ast, 'src> {
    pub name: Ident<'src>,
    pub type_params: &'ast [Ident<'src>],
    pub base: Option<TypeRef<'ast, 'src>>,
    pub members: &'ast [ExternClassMember<'ast, 'src>],
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExternClassMember<'ast, 'src> {
    Field(FieldDecl<'ast, 'src>),
    Method(ExternDecl<'ast, 'src>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember<'ast, 'src> {
    Field(FieldDecl<'ast, 'src>),
    Constructor(ConstructorDecl<'ast, 'src>),
    Method(FunctionDecl<'ast, 'src>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorDecl<'ast, 'src> {
    pub params: &'ast [Param<'ast, 'src>],
    pub body: &'ast [Stmt<'ast, 'src>],
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl<'ast, 'src> {
    pub ty: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl<'ast, 'src> {
    pub declared_pure: bool,
    pub is_async: bool,
    pub is_generator: bool,
    pub return_type: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    pub type_params: &'ast [Ident<'src>],
    pub params: &'ast [Param<'ast, 'src>],
    pub body: &'ast [Stmt<'ast, 'src>],
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternDecl<'ast, 'src> {
    pub declared_pure: bool,
    pub return_type: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    pub type_params: &'ast [Ident<'src>],
    pub params: &'ast [Param<'ast, 'src>],
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternGlobalDecl<'ast, 'src> {
    pub ty: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param<'ast, 'src> {
    pub ty: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    pub default: Option<Expr<'ast, 'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatchBinding<'ast, 'src> {
    pub ty: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause<'ast, 'src> {
    pub binding: Option<CatchBinding<'ast, 'src>>,
    pub body: &'ast [Stmt<'ast, 'src>],
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt<'ast, 'src> {
    VarDecl(VarDecl<'ast, 'src>),
    ArrayDestructure {
        bindings: &'ast [ArrayBinding<'src>],
        value: Expr<'ast, 'src>,
        span: Span,
    },
    RecordDestructure {
        bindings: &'ast [RecordBinding<'src>],
        rest: Option<Ident<'src>>,
        value: Expr<'ast, 'src>,
        span: Span,
    },
    Expr(Expr<'ast, 'src>),
    Return {
        value: Option<Expr<'ast, 'src>>,
        span: Span,
    },
    Throw {
        value: Expr<'ast, 'src>,
        span: Span,
    },
    SuperCall {
        args: &'ast [Expr<'ast, 'src>],
        span: Span,
    },
    Yield {
        value: Expr<'ast, 'src>,
        delegate: bool,
        span: Span,
    },
    Try {
        body: &'ast [Stmt<'ast, 'src>],
        catch: Option<CatchClause<'ast, 'src>>,
        finally: Option<&'ast [Stmt<'ast, 'src>]>,
        span: Span,
    },
    Block {
        body: &'ast [Stmt<'ast, 'src>],
        span: Span,
    },
    If {
        condition: Expr<'ast, 'src>,
        then_branch: &'ast Stmt<'ast, 'src>,
        else_branch: Option<&'ast Stmt<'ast, 'src>>,
        span: Span,
    },
    While {
        condition: Expr<'ast, 'src>,
        body: &'ast Stmt<'ast, 'src>,
        span: Span,
    },
    For {
        initializer: Option<ForInitializer<'ast, 'src>>,
        condition: Option<Expr<'ast, 'src>>,
        update: Option<Expr<'ast, 'src>>,
        body: &'ast Stmt<'ast, 'src>,
        span: Span,
    },
    ForIn {
        key_type: TypeRef<'ast, 'src>,
        key: Ident<'src>,
        object: Expr<'ast, 'src>,
        body: &'ast Stmt<'ast, 'src>,
        span: Span,
    },
    ForOf {
        element_type: TypeRef<'ast, 'src>,
        element: Ident<'src>,
        iterable: Expr<'ast, 'src>,
        body: &'ast Stmt<'ast, 'src>,
        span: Span,
    },
    Break(Span),
    Continue(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForInitializer<'ast, 'src> {
    VarDecl(VarDecl<'ast, 'src>),
    Expr(Expr<'ast, 'src>),
}

impl<'ast, 'src> Stmt<'ast, 'src> {
    pub const fn span(&self) -> Span {
        match self {
            Self::VarDecl(decl) => decl.span,
            Self::ArrayDestructure { span, .. } | Self::RecordDestructure { span, .. } => *span,
            Self::Expr(expr) => expr.span(),
            Self::Return { span, .. }
            | Self::Throw { span, .. }
            | Self::SuperCall { span, .. }
            | Self::Yield { span, .. }
            | Self::Try { span, .. } => *span,
            Self::Block { span, .. }
            | Self::If { span, .. }
            | Self::While { span, .. }
            | Self::For { span, .. }
            | Self::ForIn { span, .. }
            | Self::ForOf { span, .. }
            | Self::Break(span)
            | Self::Continue(span) => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayBinding<'src> {
    Hole(Span),
    Name(Ident<'src>),
    Rest(Ident<'src>),
}

impl ArrayBinding<'_> {
    pub const fn span(self) -> Span {
        match self {
            Self::Hole(span) => span,
            Self::Name(name) | Self::Rest(name) => name.span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordBinding<'src> {
    pub key: Ident<'src>,
    pub name: Ident<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl<'ast, 'src> {
    pub ty: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    pub initializer: Option<Expr<'ast, 'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr<'ast, 'src> {
    Int(i64, Span),
    Float(f64, Span),
    String(&'src str, Span),
    Bool(bool, Span),
    Null(Span),
    Ident(Ident<'src>),
    ArrayLiteral {
        elements: &'ast [ArrayElement<'ast, 'src>],
        span: Span,
    },
    RecordLiteral {
        entries: &'ast [RecordElement<'ast, 'src>],
        span: Span,
    },
    StructLiteral {
        name: Ident<'src>,
        values: &'ast [Expr<'ast, 'src>],
        span: Span,
    },
    New {
        class: Ident<'src>,
        type_args: &'ast [TypeRef<'ast, 'src>],
        args: &'ast [Expr<'ast, 'src>],
        span: Span,
    },
    DynamicImport {
        source: &'src str,
        span: Span,
    },
    Member {
        object: &'ast Expr<'ast, 'src>,
        property: Ident<'src>,
        span: Span,
    },
    OptionalMember {
        object: &'ast Expr<'ast, 'src>,
        property: Ident<'src>,
        span: Span,
    },
    Call {
        callee: &'ast Expr<'ast, 'src>,
        args: &'ast [Expr<'ast, 'src>],
        span: Span,
    },
    ArrowFunction {
        params: &'ast [Param<'ast, 'src>],
        body: ArrowBody<'ast, 'src>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: &'ast Expr<'ast, 'src>,
        span: Span,
    },
    Await {
        task: &'ast Expr<'ast, 'src>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        lhs: &'ast Expr<'ast, 'src>,
        rhs: &'ast Expr<'ast, 'src>,
        span: Span,
    },
    TypeCheck {
        value: &'ast Expr<'ast, 'src>,
        target: TypeRef<'ast, 'src>,
        span: Span,
    },
    Index {
        object: &'ast Expr<'ast, 'src>,
        index: &'ast Expr<'ast, 'src>,
        span: Span,
    },
    OptionalIndex {
        object: &'ast Expr<'ast, 'src>,
        index: &'ast Expr<'ast, 'src>,
        span: Span,
    },
    Match {
        value: &'ast Expr<'ast, 'src>,
        arms: &'ast [MatchArm<'ast, 'src>],
        span: Span,
    },
    Assignment {
        op: AssignmentOp,
        target: &'ast Expr<'ast, 'src>,
        value: &'ast Expr<'ast, 'src>,
        span: Span,
    },
    Update {
        op: UpdateOp,
        target: &'ast Expr<'ast, 'src>,
        prefix: bool,
        span: Span,
    },
    Template {
        parts: &'ast [TemplatePart<'ast, 'src>],
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordEntry<'ast, 'src> {
    pub key: Ident<'src>,
    pub value: Expr<'ast, 'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayElement<'ast, 'src> {
    Value(Expr<'ast, 'src>),
    Spread { value: Expr<'ast, 'src>, span: Span },
}

impl<'ast, 'src> ArrayElement<'ast, 'src> {
    pub const fn value(&self) -> &Expr<'ast, 'src> {
        match self {
            Self::Value(value) | Self::Spread { value, .. } => value,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Value(value) => value.span(),
            Self::Spread { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordElement<'ast, 'src> {
    Entry(RecordEntry<'ast, 'src>),
    Spread { value: Expr<'ast, 'src>, span: Span },
}

impl<'ast, 'src> RecordElement<'ast, 'src> {
    pub const fn value(&self) -> &Expr<'ast, 'src> {
        match self {
            Self::Entry(entry) => &entry.value,
            Self::Spread { value, .. } => value,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Entry(entry) => entry.span,
            Self::Spread { span, .. } => *span,
        }
    }
}
impl<'ast, 'src> Expr<'ast, 'src> {
    pub const fn span(&self) -> Span {
        match self {
            Self::Int(_, span)
            | Self::Float(_, span)
            | Self::String(_, span)
            | Self::Bool(_, span)
            | Self::Null(span)
            | Self::ArrayLiteral { span, .. }
            | Self::RecordLiteral { span, .. }
            | Self::StructLiteral { span, .. }
            | Self::New { span, .. }
            | Self::DynamicImport { span, .. }
            | Self::Member { span, .. }
            | Self::OptionalMember { span, .. }
            | Self::Call { span, .. }
            | Self::ArrowFunction { span, .. }
            | Self::Unary { span, .. }
            | Self::Await { span, .. }
            | Self::Binary { span, .. }
            | Self::TypeCheck { span, .. }
            | Self::Index { span, .. }
            | Self::OptionalIndex { span, .. }
            | Self::Match { span, .. }
            | Self::Assignment { span, .. }
            | Self::Update { span, .. }
            | Self::Template { span, .. } => *span,
            Self::Ident(ident) => ident.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm<'ast, 'src> {
    pub pattern: MatchPattern<'src>,
    pub value: Expr<'ast, 'src>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchPattern<'src> {
    EnumVariant {
        enum_name: Ident<'src>,
        variant: Ident<'src>,
        span: Span,
    },
    Wildcard(Span),
}

impl MatchPattern<'_> {
    pub const fn span(self) -> Span {
        match self {
            Self::EnumVariant { span, .. } | Self::Wildcard(span) => span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrowBody<'ast, 'src> {
    Expr(&'ast Expr<'ast, 'src>),
    Block(&'ast [Stmt<'ast, 'src>]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOp {
    Assign,
    Nullish,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    Xor,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart<'ast, 'src> {
    String(&'src str, Span),
    Expr(Expr<'ast, 'src>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    Xor,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    And,
    Or,
    Nullish,
}
