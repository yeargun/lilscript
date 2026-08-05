use crate::semantic::{EscapeState, SymbolId, Type};
use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrBinaryOp {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrUnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Const {
        out: ValueId,
        value: ConstValue,
        span: Span,
    },
    Binary {
        out: ValueId,
        op: IrBinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        span: Span,
    },
    Struct {
        out: ValueId,
        fields: Vec<ValueId>,
        span: Span,
    },
    FieldGet {
        out: ValueId,
        aggregate: ValueId,
        index: usize,
        span: Span,
    },
    Call {
        out: Option<ValueId>,
        callee: FunctionId,
        args: Vec<ValueId>,
        span: Span,
    },
    Return {
        value: Option<ValueId>,
        span: Span,
    },
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: Option<BlockId>,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct IrFunction {
    pub id: Option<FunctionId>,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct IrModule {
    pub functions: Vec<IrFunction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlFlowModule<'src> {
    pub functions: Vec<ControlFlowFunction<'src>>,
    pub globals: Vec<IrGlobal<'src>>,
    pub exports: Vec<IrExport<'src>>,
    pub structs: Vec<AggregateLayout<'src>>,
    pub classes: Vec<AggregateLayout<'src>>,
    pub entry: FunctionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrExport<'src> {
    pub name: &'src str,
    pub binding: ExportBinding,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportBinding {
    Function(FunctionId),
    Global(SymbolId),
    TypeOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateLayout<'src> {
    pub name: &'src str,
    pub fields: Vec<AggregateField<'src>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateField<'src> {
    pub name: &'src str,
    pub ty: Type<'src>,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrGlobal<'src> {
    pub symbol: SymbolId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub external: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind<'src> {
    Entry,
    Function,
    Method { class: &'src str },
    Constructor { class: &'src str },
    Closure,
    Extern,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlFlowFunction<'src> {
    pub id: FunctionId,
    pub name: Option<&'src str>,
    pub kind: FunctionKind<'src>,
    pub declared_pure: bool,
    pub params: Vec<IrParameter<'src>>,
    pub capture_count: usize,
    pub return_type: Type<'src>,
    pub locals: Vec<IrLocal<'src>>,
    pub blocks: Vec<ControlFlowBlock<'src>>,
    pub shapes: Vec<ControlShape>,
    pub entry: BlockId,
    pub value_count: u32,
    pub value_escapes: Vec<EscapeState>,
    pub locals_promoted: bool,
    pub live: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlShape {
    If {
        header: BlockId,
        then_block: BlockId,
        else_block: BlockId,
        merge_block: BlockId,
    },
    Loop {
        header: BlockId,
        body: BlockId,
        update: Option<BlockId>,
        exit: BlockId,
    },
}

impl ControlShape {
    pub const fn header(&self) -> BlockId {
        match self {
            Self::If { header, .. } | Self::Loop { header, .. } => *header,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrParameter<'src> {
    pub symbol: SymbolId,
    pub local: LocalId,
    pub value: ValueId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrLocal<'src> {
    pub id: LocalId,
    pub symbol: SymbolId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlFlowBlock<'src> {
    pub id: BlockId,
    pub phis: Vec<Phi<'src>>,
    pub instructions: Vec<ControlFlowInstruction<'src>>,
    pub terminator: Option<Terminator>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phi<'src> {
    pub out: ValueId,
    pub local: LocalId,
    pub ty: Type<'src>,
    pub incoming: Vec<(BlockId, ValueId)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlFlowInstruction<'src> {
    pub out: Option<ValueId>,
    pub ty: Option<Type<'src>>,
    pub op: ControlFlowOp<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ControlFlowOp<'src> {
    Const(ConstValue),
    Unary {
        op: IrUnaryOp,
        value: ValueId,
    },
    Binary {
        op: IrBinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    },
    TypeCheck {
        value: ValueId,
        target: Type<'src>,
    },
    Array(Vec<ValueId>),
    Struct {
        name: &'src str,
        fields: Vec<ValueId>,
    },
    NewClass {
        class: &'src str,
        constructor: Option<FunctionId>,
        args: Vec<ValueId>,
    },
    Closure {
        function: FunctionId,
        captures: Vec<ValueId>,
    },
    LoadLocal(LocalId),
    StoreLocal {
        local: LocalId,
        value: ValueId,
    },
    LoadGlobal(SymbolId),
    StoreGlobal {
        global: SymbolId,
        value: ValueId,
    },
    FieldGet {
        object: ValueId,
        owner: &'src str,
        field: &'src str,
        index: usize,
    },
    FieldSet {
        object: ValueId,
        owner: &'src str,
        field: &'src str,
        index: usize,
        value: ValueId,
    },
    HostFieldGet {
        object: ValueId,
        property: &'src str,
    },
    HostFieldSet {
        object: ValueId,
        property: &'src str,
        value: ValueId,
    },
    IndexGet {
        object: ValueId,
        index: ValueId,
    },
    IndexSet {
        object: ValueId,
        index: ValueId,
        value: ValueId,
    },
    CallDirect {
        function: FunctionId,
        args: Vec<ValueId>,
    },
    CallValue {
        callee: ValueId,
        args: Vec<ValueId>,
    },
    CallMethod {
        receiver: ValueId,
        class: &'src str,
        method: &'src str,
        function: FunctionId,
        args: Vec<ValueId>,
    },
    HostCall {
        receiver: ValueId,
        method: &'src str,
        args: Vec<ValueId>,
        pure: bool,
    },
    Intrinsic {
        intrinsic: Intrinsic,
        receiver: Option<ValueId>,
        args: Vec<ValueId>,
    },
    Template(Vec<TemplateOperand>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateOperand {
    String(String),
    Value(ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intrinsic {
    Print,
    UnwrapNullable,
    UnwrapUnion,
    ArrayLength,
    ArrayMap,
    ArrayFilter,
    ArrayReduce,
    ArrayForEach,
    ArrayPush,
    ArrayPop,
    MapNew,
    MapSize,
    MapGet,
    MapSet,
    MapHas,
    MapDelete,
    MapClear,
    SetNew,
    SetSize,
    SetAdd,
    SetHas,
    SetDelete,
    SetClear,
    ArrayBufferNew,
    SharedArrayBufferNew,
    BufferByteLength,
    BufferSlice,
    Uint8ArrayNew,
    Uint8ArrayLength,
    Uint8ArrayByteLength,
    Uint8ArrayByteOffset,
    Uint8ArrayBuffer,
    Uint8ArraySlice,
    Uint8ArraySubarray,
    StringLength,
    StringIncludes,
    StringStartsWith,
    StringEndsWith,
    StringToUpperCase,
    StringToLowerCase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        condition: ValueId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return(Option<ValueId>),
    Unreachable,
}
