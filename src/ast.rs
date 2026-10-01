use crate::span::Span;

/// A name as the source writes it. Its `id` is the occurrence's own source
/// node (plan M4.4): the checker's facts about an identifier (its symbol,
/// its declared type) are keyed by it, never by its span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ident<'src> {
    pub name: &'src str,
    pub span: Span,
    pub id: SourceNodeId,
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
    Intersection(&'ast [TypeRef<'ast, 'src>]),
    Function {
        params: &'ast [ParameterType<'ast, 'src>],
        return_type: &'ast TypeRef<'ast, 'src>,
    },
}

impl<'ast, 'src> TypeRef<'ast, 'src> {
    pub const fn is_auto(self) -> bool {
        matches!(self.kind, TypeKind::Auto)
    }

    pub const fn is_void(self) -> bool {
        matches!(self.kind, TypeKind::Void)
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
    pub shape: bool,
    pub name: Ident<'src>,
    pub type_params: &'ast [Ident<'src>],
    pub base: Option<TypeRef<'ast, 'src>>,
    pub members: &'ast [ClassMember<'ast, 'src>],
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
    /// The implicit receiver's binding, `this`, with its own source node.
    pub this: Ident<'src>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl<'ast, 'src> {
    /// An immutable literal identifying a declared shape view.
    pub discriminant: bool,
    /// A shape accessor may run host code; ordinary fields are data.
    pub accessor: bool,
    pub ty: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    /// `T name = e;` (R3): the value every construction gives the field
    /// before `init` runs, in place of an implicit default.
    pub initializer: Option<Expr<'ast, 'src>>,
    pub span: Span,
}

/// Behaviour the author pinned to one region of the program with an `@`
/// attribute, kept when the project-wide objective would decide otherwise.
///
/// The objective (`javascript.priority`, `[objective] codecs`) answers for
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
    /// `debug void f(...)` (R15): its calls are strippable logging or
    /// assertions, which `strip_debug` drops.
    pub declared_debug: bool,
    pub is_async: bool,
    pub is_generator: bool,
    pub return_type: TypeRef<'ast, 'src>,
    pub name: Ident<'src>,
    pub type_params: &'ast [Ident<'src>],
    pub params: &'ast [Param<'ast, 'src>],
    pub body: &'ast [Stmt<'ast, 'src>],
    pub span: Span,
    /// A method's implicit receiver binding, `this`, with its own source
    /// node; a function's is unused.
    pub this: Ident<'src>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternDecl<'ast, 'src> {
    pub declared_pure: bool,
    /// `debug extern void f(...)` (R15), as for a function.
    pub declared_debug: bool,
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
    pub role: ParamRole,
    pub span: Span,
}

/// What a lambda's parameter receives (R7): an argument, the receiver (the
/// call's `this`), or the rest of the arguments as a list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParamRole {
    #[default]
    Value,
    /// `(this JsValue self, …)`: the first parameter only.
    Receiver,
    /// `(…, JsValue... rest)`: the last parameter only.
    Rest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterType<'ast, 'src> {
    pub receiver: bool,
    pub ty: TypeRef<'ast, 'src>,
    /// A variadic element type in a function-type annotation.
    pub rest: bool,
    pub passing: crate::primitive::ParameterPassing,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Argument<'ast, 'src> {
    pub expression: Expr<'ast, 'src>,
    pub passing: crate::primitive::ParameterPassing,
    /// `...xs`: the argument spreads an iterable into a JavaScript call (R7).
    pub spread: bool,
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
    VarDecl(VarDecl<'ast, 'src>, SourceNodeId),
    ArrayDestructure {
        id: SourceNodeId,
        bindings: &'ast [ArrayBinding<'src>],
        value: Expr<'ast, 'src>,
        span: Span,
    },
    RecordDestructure {
        id: SourceNodeId,
        bindings: &'ast [RecordBinding<'src>],
        rest: Option<Ident<'src>>,
        value: Expr<'ast, 'src>,
        span: Span,
    },
    Expr(Expr<'ast, 'src>, SourceNodeId),
    Return {
        id: SourceNodeId,
        value: Option<Expr<'ast, 'src>>,
        span: Span,
    },
    Throw {
        id: SourceNodeId,
        value: Expr<'ast, 'src>,
        span: Span,
    },
    SuperCall {
        id: SourceNodeId,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
    },
    Yield {
        id: SourceNodeId,
        value: Expr<'ast, 'src>,
        delegate: bool,
        span: Span,
    },
    Try {
        id: SourceNodeId,
        body: &'ast [Stmt<'ast, 'src>],
        catch: Option<CatchClause<'ast, 'src>>,
        finally: Option<&'ast [Stmt<'ast, 'src>]>,
        span: Span,
    },
    Block {
        id: SourceNodeId,
        body: &'ast [Stmt<'ast, 'src>],
        span: Span,
    },
    If {
        id: SourceNodeId,
        condition: Expr<'ast, 'src>,
        then_branch: &'ast Stmt<'ast, 'src>,
        else_branch: Option<&'ast Stmt<'ast, 'src>>,
        span: Span,
    },
    While {
        id: SourceNodeId,
        condition: Expr<'ast, 'src>,
        body: &'ast Stmt<'ast, 'src>,
        span: Span,
    },
    For {
        id: SourceNodeId,
        initializer: Option<ForInitializer<'ast, 'src>>,
        condition: Option<Expr<'ast, 'src>>,
        update: Option<Expr<'ast, 'src>>,
        body: &'ast Stmt<'ast, 'src>,
        span: Span,
    },
    ForIn {
        id: SourceNodeId,
        key_type: TypeRef<'ast, 'src>,
        key: Ident<'src>,
        object: Expr<'ast, 'src>,
        body: &'ast Stmt<'ast, 'src>,
        span: Span,
    },
    ForOf {
        id: SourceNodeId,
        element_type: TypeRef<'ast, 'src>,
        element: Ident<'src>,
        /// `for (K k, V v of map)`: the entry's value binding (R14).
        value: Option<(TypeRef<'ast, 'src>, Ident<'src>)>,
        iterable: Expr<'ast, 'src>,
        body: &'ast Stmt<'ast, 'src>,
        inline: bool,
        span: Span,
    },
    Break(Span, SourceNodeId),
    Continue(Span, SourceNodeId),
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
    /// Identity of this statement occurrence in its owning source.
    pub const fn id(&self) -> SourceNodeId {
        match self {
            Self::VarDecl(_, id)
            | Self::Expr(_, id)
            | Self::Break(_, id)
            | Self::Continue(_, id)
            | Self::ArrayDestructure { id, .. }
            | Self::RecordDestructure { id, .. }
            | Self::Return { id, .. }
            | Self::Throw { id, .. }
            | Self::SuperCall { id, .. }
            | Self::Yield { id, .. }
            | Self::Try { id, .. }
            | Self::Block { id, .. }
            | Self::If { id, .. }
            | Self::While { id, .. }
            | Self::For { id, .. }
            | Self::ForIn { id, .. }
            | Self::ForOf { id, .. } => *id,
        }
    }
    pub const fn span(&self) -> Span {
        match self {
            Self::VarDecl(decl, ..) => decl.span,
            Self::ArrayDestructure { span, .. } | Self::RecordDestructure { span, .. } => *span,
            Self::Expr(expr, ..) => expr.span(),
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
            | Self::Break(span, ..)
            | Self::Continue(span, ..) => *span,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceNodeId(std::num::NonZeroU32);

impl SourceNodeId {
    pub const fn index(self) -> usize {
        self.0.get() as usize - 1
    }

    /// A node no source owns, for a test that declares a name by hand.
    #[cfg(test)]
    pub(crate) const fn detached(index: u32) -> Self {
        match std::num::NonZeroU32::new(index + 1) {
            Some(id) => Self(id),
            None => panic!("detached node index overflow"),
        }
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
        Expr {
            id: self.node(),
            kind,
        }
    }

    /// An identifier occurrence, with its own source node (M4.4).
    pub(crate) fn ident<'src>(&self, name: &'src str, span: Span) -> Ident<'src> {
        Ident {
            name,
            span,
            id: self.node(),
        }
    }

    pub(crate) fn node(&self) -> SourceNodeId {
        let next = self
            .0
            .get()
            .checked_add(1)
            .expect("source node identity limit");
        self.0.set(next);
        SourceNodeId(std::num::NonZeroU32::new(next).unwrap())
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
        /// A named shape literal shares ordered keyed entries with records.
        name: Option<Ident<'src>>,
        entries: &'ast [RecordElement<'ast, 'src>],
        span: Span,
    },
    ObjectLiteral {
        entries: &'ast [RecordElement<'ast, 'src>],
        span: Span,
    },
    /// Snapshot a struct value, then update the named fields left to right.
    With {
        value: &'ast Expr<'ast, 'src>,
        fields: &'ast [RecordEntry<'ast, 'src>],
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
    /// `value as T`: a trusted view of a `JsValue` as `T`, no code. With
    /// `checked`, `value as? T`: a test, then the value as `T` or null (R12).
    Cast {
        value: &'ast Expr<'ast, 'src>,
        target: TypeRef<'ast, 'src>,
        checked: bool,
        span: Span,
    },
    /// `T(value)`: an explicit conversion that emits the coercion (R12).
    Convert {
        target: TypeRef<'ast, 'src>,
        value: &'ast Expr<'ast, 'src>,
        span: Span,
    },
    /// `new callee(args)` whose constructor is a value, not a class name: a
    /// member chain or a parenthesized expression (R12).
    Construct {
        callee: &'ast Expr<'ast, 'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
    },
    /// An operator that only a `JsValue` has (R12).
    DynamicBinary {
        op: DynamicBinaryOp,
        lhs: &'ast Expr<'ast, 'src>,
        rhs: &'ast Expr<'ast, 'src>,
        span: Span,
    },
    /// A prefix operator that only a `JsValue` has (R12).
    DynamicUnary {
        op: DynamicUnaryOp,
        expr: &'ast Expr<'ast, 'src>,
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
            | Self::With { span, .. }
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
            | Self::Template { span, .. }
            | Self::Cast { span, .. }
            | Self::Convert { span, .. }
            | Self::Construct { span, .. }
            | Self::DynamicBinary { span, .. }
            | Self::DynamicUnary { span, .. } => *span,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// How tightly each form binds, as the parser climbs: an operand that binds
/// less tightly than its position needs parentheses. Binary operators take
/// theirs from [`BinaryOp::precedence`] and [`DynamicBinaryOp::precedence`].
pub mod precedence {
    /// Assignments, arrows and the `if` and `match` expressions.
    pub const LOWEST: u8 = 0;
    /// `value is T`.
    pub const TYPE_TEST: u8 = 4;
    /// `<`, `in`, `instanceof` and `value as T`.
    pub const RELATIONAL: u8 = 7;
    /// Prefix operators and `await`.
    pub const UNARY: u8 = 11;
    /// Member access, indexing, calls and postfix updates.
    pub const POSTFIX: u8 = 12;
    /// Literals, names, `new`, conversions and parenthesized expressions.
    pub const PRIMARY: u8 = 13;
}

impl BinaryOp {
    pub const fn precedence(self) -> u8 {
        match self {
            Self::Nullish | Self::Or => 1,
            Self::And => 2,
            Self::BitOr => 3,
            Self::Xor => 4,
            Self::BitAnd => 5,
            Self::Eq | Self::NotEq => 6,
            Self::Less | Self::LessEq | Self::Greater | Self::GreaterEq => precedence::RELATIONAL,
            Self::ShiftLeft | Self::ShiftRight | Self::UnsignedShiftRight => 8,
            Self::Add | Self::Sub => 9,
            Self::Mul | Self::Div | Self::Mod => 10,
        }
    }
}

impl DynamicBinaryOp {
    pub const fn precedence(self) -> u8 {
        match self {
            Self::StrictEq | Self::StrictNotEq => 6,
            Self::In | Self::InstanceOf => precedence::RELATIONAL,
        }
    }
}

impl<'ast, 'src> Expr<'ast, 'src> {
    /// How tightly this expression binds (see [`precedence`]).
    pub const fn precedence(&self) -> u8 {
        match &self.kind {
            ExprKind::Assignment { .. }
            | ExprKind::ArrowFunction { .. }
            | ExprKind::If { .. }
            | ExprKind::Match { .. } => precedence::LOWEST,
            ExprKind::Binary { op, .. } => op.precedence(),
            ExprKind::DynamicBinary { op, .. } => op.precedence(),
            ExprKind::TypeCheck { .. } => precedence::TYPE_TEST,
            ExprKind::Cast { .. } => precedence::RELATIONAL,
            ExprKind::Unary { .. }
            | ExprKind::DynamicUnary { .. }
            | ExprKind::Await { .. }
            | ExprKind::Update { prefix: true, .. } => precedence::UNARY,
            ExprKind::Update { .. }
            | ExprKind::Member { .. }
            | ExprKind::OptionalMember { .. }
            | ExprKind::Index { .. }
            | ExprKind::OptionalIndex { .. }
            | ExprKind::Call { .. } => precedence::POSTFIX,
            _ => precedence::PRIMARY,
        }
    }
}

/// Operators with JavaScript's meaning that apply only through a `JsValue`
/// (R12). They lower to the dynamic operations, so the IR's typed operators
/// never see them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicBinaryOp {
    /// `===`
    StrictEq,
    /// `!==`
    StrictNotEq,
    /// `key in object`
    In,
    /// `value instanceof constructor`
    InstanceOf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicUnaryOp {
    /// `typeof value`
    TypeOf,
    /// `delete object.key`, `delete object[key]`
    Delete,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
