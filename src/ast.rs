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
        params: &'ast [ParameterType<'ast, 'src>],
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
    source: SourceIdentity,
    data: ProgramData<'ast, 'src>,
}

impl<'ast, 'src> Program<'ast, 'src> {
    pub(crate) fn new(source: SourceIdentity, data: ProgramData<'ast, 'src>) -> Self {
        Self { source, data }
    }

    pub fn source_identity(&self) -> &SourceIdentity {
        &self.source
    }

    pub(crate) fn with_items(self, nodes: &SourceNodes, items: &'ast [Item<'ast, 'src>]) -> Self {
        Self::new(nodes.finish(), ProgramData { items, ..self.data })
    }
}

// The completed syntax has no mutable dereference. Replacing its items must
// pass through a construction boundary that also replaces source ownership.
impl<'ast, 'src> std::ops::Deref for Program<'ast, 'src> {
    type Target = ProgramData<'ast, 'src>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgramData<'ast, 'src> {
    pub imports: &'ast [ImportDecl<'ast, 'src>],
    pub foreign_imports: &'ast [ForeignImportDecl<'ast, 'src>],
    pub dynamic_imports: &'ast [DynamicImportDecl<'ast, 'src>],
    pub module_bindings: &'ast [ModuleBinding<'ast, 'src>],
    /// Class bindings explicitly published as runtime constructor values.
    pub constructor_values: &'ast [Ident<'src>],
    pub exports: &'ast [ExportDecl<'src>],
    pub items: &'ast [Item<'ast, 'src>],
    pub span: Span,
}

/// A module-scoped value cell created while linking, before any module body is
/// evaluated. Parsed single-file programs do not need this metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleBinding<'ast, 'src> {
    pub name: Ident<'src>,
    pub ty: TypeRef<'ast, 'src>,
    pub module_span: Span,
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
    pub kind: ExportKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportKind {
    #[default]
    Binding,
    ConstructorValue,
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
    /// `init(params);` — the host constructor's parameters. It exists only so an
    /// internal subclass can type-check `super(...)`; a host class is still never
    /// constructed with `new` from LilScript.
    Constructor(ExternConstructorDecl<'ast, 'src>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternConstructorDecl<'ast, 'src> {
    pub params: &'ast [Param<'ast, 'src>],
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

/// Behaviour the author pinned to one region of the program with an `@`
/// attribute, kept when the project-wide objective would decide otherwise.
///
/// The objective (`javascript.priority`, `javascript.cost_model`) answers for
/// the artifact as a whole. It cannot answer for a function whose cost is not
/// the artifact's cost — a parser's inner loop inside a size-first library, a
/// literal table the author wrote *to be* pooled under an objective whose
/// admission model refuses it (finer/hypotheses/011). A region policy is how
/// that intent survives the objective, and it only ever pins behaviour on:
/// the default for every field is "let the objective decide".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RegionPolicy {
    /// `@pool` — string constants authored in this region are admitted to the
    /// string pool whatever the objective's savings threshold would say.
    pub pool_strings: bool,
}

impl RegionPolicy {
    pub const fn is_default(self) -> bool {
        !self.pool_strings
    }

    /// Policy for a function synthesized from two others. Pinned behaviour is
    /// kept if either source asked for it: a transform that fuses or outlines
    /// code must not be the reason an author's `@pool` silently stops applying
    /// to the literals it moved.
    pub const fn union(self, other: Self) -> Self {
        Self {
            pool_strings: self.pool_strings || other.pool_strings,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl<'ast, 'src> {
    pub region: RegionPolicy,
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
    pub parameter: ParameterType<'ast, 'src>,
    pub name: Ident<'src>,
    pub default: Option<Expr<'ast, 'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterType<'ast, 'src> {
    pub ty: TypeRef<'ast, 'src>,
    pub passing: crate::primitive::ParameterPassing,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Argument<'ast, 'src> {
    pub expression: Expr<'ast, 'src>,
    pub passing: crate::primitive::ParameterPassing,
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
        args: &'ast [Argument<'ast, 'src>],
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
        inline: bool,
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

impl<'ast, 'src> Expr<'ast, 'src> {
    pub fn const_list_literals(&self) -> Option<Vec<&Expr<'ast, 'src>>> {
        let Expr {
            kind: ExprKind::ArrayLiteral { elements, .. },
            ..
        } = self
        else {
            return None;
        };
        let mut values = Vec::with_capacity(elements.len());
        for element in *elements {
            match element {
                ArrayElement::Value(value) if value.is_const_scalar() => values.push(value),
                _ => return None,
            }
        }
        Some(values)
    }

    pub const fn is_const_scalar(&self) -> bool {
        matches!(
            self.kind,
            ExprKind::Int(..)
                | ExprKind::Float(..)
                | ExprKind::String(..)
                | ExprKind::Bool(..)
                | ExprKind::Null(..)
        )
    }
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

/// An index is meaningful only within the source program that owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceNodeId(std::num::NonZeroU32);

impl SourceNodeId {
    pub const fn index(self) -> usize {
        self.0.get() as usize - 1
    }
}

/// Immutable source ownership. Clones retain the same opaque stamp; syntax
/// expansion and linking finish a distinct state before facts are produced.
#[derive(Clone)]
pub struct SourceIdentity {
    stamp: std::num::NonZeroU64,
    nodes: u32,
}

impl SourceIdentity {
    pub fn len(&self) -> usize {
        self.nodes as usize
    }

    pub fn same(&self, other: &Self) -> bool {
        self.stamp == other.stamp
    }
}

impl std::fmt::Debug for SourceIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("SourceIdentity")
            .field(&self.len())
            .finish()
    }
}

impl PartialEq for SourceIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
    }
}

impl Eq for SourceIdentity {}

/// Construction owns the sequence. Template fragments share this counter;
/// only finishing a source issues a process-wide stamp, never individual nodes.
#[derive(Debug, Default)]
pub(crate) struct SourceNodes(std::cell::Cell<u32>);

impl SourceNodes {
    pub(crate) fn continuing(source: &SourceIdentity) -> Self {
        Self(std::cell::Cell::new(source.len() as u32))
    }

    pub(crate) fn expression<'ast, 'src>(&self, kind: ExprKind<'ast, 'src>) -> Expr<'ast, 'src> {
        let next = self
            .0
            .get()
            .checked_add(1)
            .expect("source expression identity limit");
        self.0.set(next);
        Expr {
            id: SourceNodeId(std::num::NonZeroU32::new(next).unwrap()),
            kind,
        }
    }

    pub(crate) fn finish(&self) -> SourceIdentity {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_SOURCE: AtomicU64 = AtomicU64::new(1);
        let stamp = NEXT_SOURCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .expect("source identity capacity exceeded");
        SourceIdentity {
            stamp: std::num::NonZeroU64::new(stamp).expect("source identities start at one"),
            nodes: self.0.get(),
        }
    }
}

#[cfg(test)]
mod source_identity_tests {
    use super::*;

    #[test]
    fn clones_preserve_identity_without_heap_ownership() {
        let nodes = SourceNodes::default();
        let expression = nodes.expression(ExprKind::Int(7, Span::default()));
        let source = nodes.finish();
        let clone = source.clone();
        assert!(source.same(&clone));
        assert_eq!(source, clone);
        assert_eq!(source.len(), 1);
        assert_eq!(expression.id.index(), 0);
        assert_eq!(format!("{source:?}"), "SourceIdentity(1)");
        assert!(!std::mem::needs_drop::<SourceIdentity>());
        assert!(std::mem::size_of::<SourceIdentity>() <= 2 * std::mem::size_of::<u64>());
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<SourceIdentity>(), 16);
        drop(source);
        assert_eq!(clone.len(), 1);
    }

    #[test]
    fn each_finished_state_is_fresh_even_without_new_nodes() {
        let nodes = SourceNodes::default();
        let empty = nodes.finish();
        assert_eq!(empty.len(), 0);
        assert_ne!(empty, nodes.finish());
        let continued = SourceNodes::continuing(&empty);
        assert_ne!(empty, continued.finish());
        let first = continued.expression(ExprKind::Bool(true, Span::default()));
        let source = continued.finish();
        let next = SourceNodes::continuing(&source);
        let second = next.expression(ExprKind::Bool(false, Span::default()));
        let extended = next.finish();
        assert_eq!((first.id.index(), second.id.index()), (0, 1));
        assert_eq!((source.len(), extended.len()), (1, 2));
        assert_ne!(source, extended);
    }

    #[test]
    fn independently_finished_sources_do_not_alias_across_threads() {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                std::thread::spawn(|| {
                    (0..32)
                        .map(|_| SourceNodes::default().finish())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut identities = std::collections::HashSet::new();
        for worker in workers {
            for source in worker.join().unwrap() {
                assert!(identities.insert(source.stamp));
                assert_eq!(source.len(), 0);
            }
        }
        assert_eq!(identities.len(), 128);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr<'ast, 'src> {
    pub id: SourceNodeId,
    pub kind: ExprKind<'ast, 'src>,
}

impl Expr<'_, '_> {
    pub const fn span(&self) -> Span {
        self.kind.span()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind<'ast, 'src> {
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
    ObjectLiteral {
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
        args: &'ast [Argument<'ast, 'src>],
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
        args: &'ast [Argument<'ast, 'src>],
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
    If {
        condition: &'ast Expr<'ast, 'src>,
        then_value: &'ast Expr<'ast, 'src>,
        else_value: &'ast Expr<'ast, 'src>,
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
impl<'ast, 'src> ExprKind<'ast, 'src> {
    pub const fn span(&self) -> Span {
        match self {
            Self::Int(_, span)
            | Self::Float(_, span)
            | Self::String(_, span)
            | Self::Bool(_, span)
            | Self::Null(span)
            | Self::ArrayLiteral { span, .. }
            | Self::RecordLiteral { span, .. }
            | Self::ObjectLiteral { span, .. }
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
            | Self::If { span, .. }
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
    Int(i64, Span),
    String(&'src str, Span),
    Bool(bool, Span),
    Wildcard(Span),
}

impl MatchPattern<'_> {
    pub const fn span(self) -> Span {
        match self {
            Self::EnumVariant { span, .. }
            | Self::Int(_, span)
            | Self::String(_, span)
            | Self::Bool(_, span)
            | Self::Wildcard(span) => span,
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
