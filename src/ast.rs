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
    pub exports: &'ast [ExportDecl<'src>],
    pub items: &'ast [Item<'ast, 'src>],
    pub span: Span,
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
    Struct(StructDecl<'ast, 'src>),
    Class(ClassDecl<'ast, 'src>),
    Function(FunctionDecl<'ast, 'src>),
    Extern(ExternDecl<'ast, 'src>),
    Stmt(Stmt<'ast, 'src>),
}

impl<'ast, 'src> Item<'ast, 'src> {
    pub const fn span(&self) -> Span {
        match self {
            Self::Struct(decl) => decl.span,
            Self::Class(decl) => decl.span,
            Self::Function(decl) => decl.span,
            Self::Extern(decl) => decl.span,
            Self::Stmt(stmt) => stmt.span(),
        }
    }
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
    pub members: &'ast [ClassMember<'ast, 'src>],
    pub span: Span,
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
pub struct Param<'ast, 'src> {
    pub ty: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    pub default: Option<Expr<'ast, 'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt<'ast, 'src> {
    VarDecl(VarDecl<'ast, 'src>),
    Expr(Expr<'ast, 'src>),
    Return {
        value: Option<Expr<'ast, 'src>>,
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
            Self::Expr(expr) => expr.span(),
            Self::Return { span, .. } => *span,
            Self::Block { span, .. }
            | Self::If { span, .. }
            | Self::While { span, .. }
            | Self::For { span, .. }
            | Self::Break(span)
            | Self::Continue(span) => *span,
        }
    }
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
        elements: &'ast [Expr<'ast, 'src>],
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
    Member {
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
    Binary {
        op: BinaryOp,
        lhs: &'ast Expr<'ast, 'src>,
        rhs: &'ast Expr<'ast, 'src>,
        span: Span,
    },
    Index {
        object: &'ast Expr<'ast, 'src>,
        index: &'ast Expr<'ast, 'src>,
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

impl<'ast, 'src> Expr<'ast, 'src> {
    pub const fn span(&self) -> Span {
        match self {
            Self::Int(_, span)
            | Self::Float(_, span)
            | Self::String(_, span)
            | Self::Bool(_, span)
            | Self::Null(span)
            | Self::ArrayLiteral { span, .. }
            | Self::StructLiteral { span, .. }
            | Self::New { span, .. }
            | Self::Member { span, .. }
            | Self::Call { span, .. }
            | Self::ArrowFunction { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Index { span, .. }
            | Self::Assignment { span, .. }
            | Self::Update { span, .. }
            | Self::Template { span, .. } => *span,
            Self::Ident(ident) => ident.span,
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
    Add,
    Sub,
    Mul,
    Div,
    Mod,
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
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    And,
    Or,
}
