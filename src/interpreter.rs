use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ahash::AHashMap;

use crate::ast::{
    ArrowBody, AssignmentOp, BinaryOp, Expr, ForInitializer, FunctionDecl, Item, Param, Program,
    Stmt, TemplatePart, TypeKind, UnaryOp, UpdateOp, VarDecl,
};
use crate::semantic::{SemanticModel, SymbolId, Type};
use crate::span::Span;
use crate::typed_array::TypedArrayKind;

const DEFAULT_STEP_LIMIT: u64 = 10_000_000;
const DEFAULT_RECURSION_LIMIT: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpretError {
    pub span: Span,
    pub message: String,
}

impl InterpretError {
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

impl fmt::Display for InterpretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for InterpretError {}

#[derive(Debug, Clone)]
enum Value {
    Int(i32),
    Float(f64),
    String(String),
    Bool(bool),
    Array(Rc<RefCell<Vec<Value>>>),
    Buffer(Rc<BufferValue>),
    TypedArray(Rc<TypedArrayValue>),
    Symbol(Rc<SymbolValue>),
    Callable(Callable),
    Null,
    Void,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Callable {
    Function(SymbolId),
    Closure(usize),
}

#[derive(Debug)]
struct BufferValue {
    bytes: RefCell<Vec<u8>>,
    shared: bool,
}

#[derive(Debug)]
struct TypedArrayValue {
    kind: TypedArrayKind,
    buffer: Rc<BufferValue>,
    offset: usize,
    length: usize,
}

#[derive(Debug)]
struct SymbolValue {
    #[allow(dead_code)]
    description: Option<String>,
}

fn new_symbol(description: Option<String>) -> Rc<SymbolValue> {
    Rc::new(SymbolValue { description })
}

#[derive(Debug, Clone)]
struct RuntimeClosure<'ast, 'src> {
    params: &'ast [Param<'ast, 'src>],
    body: ArrowBody<'ast, 'src>,
    captures: AHashMap<SymbolId, Value>,
}

#[derive(Debug, Clone)]
enum RuntimePlace {
    Binding(SymbolId),
    ArrayElement {
        array: Rc<RefCell<Vec<Value>>>,
        index: usize,
    },
    TypedArrayElement {
        view: Rc<TypedArrayValue>,
        index: usize,
    },
}

impl Value {
    fn display(&self, span: Span) -> Result<String, InterpretError> {
        match self {
            Self::Int(value) => Ok(value.to_string()),
            Self::Float(value) if value.is_nan() => Ok("NaN".to_string()),
            Self::Float(value) if *value == f64::INFINITY => Ok("Infinity".to_string()),
            Self::Float(value) if *value == f64::NEG_INFINITY => Ok("-Infinity".to_string()),
            Self::Float(value) => Ok(value.to_string()),
            Self::String(value) => Ok(value.clone()),
            Self::Bool(value) => Ok(value.to_string()),
            Self::Array(_) => Err(InterpretError::new(
                span,
                "array value cannot be printed directly",
            )),
            Self::Buffer(_) => Err(InterpretError::new(
                span,
                "buffer value cannot be printed directly",
            )),
            Self::TypedArray(view) => Err(InterpretError::new(
                span,
                format!("{} value cannot be printed directly", view.kind.name()),
            )),
            Self::Symbol(_) => Err(InterpretError::new(
                span,
                "Symbol value cannot be printed directly",
            )),
            Self::Callable(_) => Err(InterpretError::new(
                span,
                "callable value cannot be printed directly",
            )),
            Self::Null => Ok("null".to_string()),
            Self::Void => Err(InterpretError::new(span, "void value cannot be printed")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterpreterLimits {
    pub steps: u64,
    pub recursion_depth: usize,
}

impl Default for InterpreterLimits {
    fn default() -> Self {
        Self {
            steps: DEFAULT_STEP_LIMIT,
            recursion_depth: DEFAULT_RECURSION_LIMIT,
        }
    }
}

/// Evaluates the checked language core without going through IR.
///
/// This intentionally independent path is used as a semantic oracle for
/// differential compiler testing. Unsupported host, nominal aggregate,
/// map/set, and class operations fail explicitly instead of approximating them.
pub fn interpret_program<'ast, 'src>(
    program: &Program<'ast, 'src>,
    semantics: &SemanticModel<'src>,
) -> Result<String, InterpretError> {
    ReferenceInterpreter::new(program, semantics, InterpreterLimits::default()).run()
}

pub fn interpret_program_with_limits<'ast, 'src>(
    program: &Program<'ast, 'src>,
    semantics: &SemanticModel<'src>,
    limits: InterpreterLimits,
) -> Result<String, InterpretError> {
    ReferenceInterpreter::new(program, semantics, limits).run()
}

struct ReferenceInterpreter<'program, 'ast, 'src> {
    program: &'program Program<'ast, 'src>,
    semantics: &'program SemanticModel<'src>,
    functions: AHashMap<SymbolId, &'program FunctionDecl<'ast, 'src>>,
    globals: AHashMap<SymbolId, Value>,
    frames: Vec<AHashMap<SymbolId, Value>>,
    closures: Vec<RuntimeClosure<'ast, 'src>>,
    output: String,
    remaining_steps: u64,
    recursion_depth: usize,
    recursion_limit: usize,
}

impl<'program, 'ast, 'src> ReferenceInterpreter<'program, 'ast, 'src> {
    fn new(
        program: &'program Program<'ast, 'src>,
        semantics: &'program SemanticModel<'src>,
        limits: InterpreterLimits,
    ) -> Self {
        let functions = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(function) => semantics
                    .identifier_symbol(function.name.span)
                    .map(|symbol| (symbol, function)),
                _ => None,
            })
            .collect();
        Self {
            program,
            semantics,
            functions,
            globals: AHashMap::new(),
            frames: Vec::new(),
            closures: Vec::new(),
            output: String::new(),
            remaining_steps: limits.steps,
            recursion_depth: 0,
            recursion_limit: limits.recursion_depth,
        }
    }

    fn run(mut self) -> Result<String, InterpretError> {
        for item in self.program.items {
            match item {
                Item::Stmt(statement) => match self.execute_stmt(statement)? {
                    Flow::Next => {}
                    Flow::Return(_) => {
                        return Err(InterpretError::new(
                            statement.span(),
                            "return escaped top-level execution",
                        ));
                    }
                    Flow::Break | Flow::Continue => {
                        return Err(InterpretError::new(
                            statement.span(),
                            "loop control escaped top-level execution",
                        ));
                    }
                },
                Item::Struct(_) | Item::Function(_) => {}
                Item::Class(_) | Item::ExternClass(_) | Item::Extern(_) | Item::ExternGlobal(_) => {
                    return Err(InterpretError::new(
                        item.span(),
                        "reference interpreter does not support host or class declarations",
                    ));
                }
            }
        }
        Ok(self.output)
    }

    fn step(&mut self, span: Span) -> Result<(), InterpretError> {
        self.remaining_steps = self.remaining_steps.checked_sub(1).ok_or_else(|| {
            InterpretError::new(span, "reference interpreter step limit exceeded")
        })?;
        Ok(())
    }

    fn execute_statements(
        &mut self,
        statements: &'ast [Stmt<'ast, 'src>],
    ) -> Result<Flow, InterpretError> {
        for statement in statements {
            let flow = self.execute_stmt(statement)?;
            if flow != Flow::Next {
                return Ok(flow);
            }
        }
        Ok(Flow::Next)
    }

    fn execute_stmt(&mut self, statement: &Stmt<'ast, 'src>) -> Result<Flow, InterpretError> {
        self.step(statement.span())?;
        match statement {
            Stmt::VarDecl(declaration) => {
                self.execute_var_decl(declaration)?;
                Ok(Flow::Next)
            }
            Stmt::Expr(expression) => {
                self.evaluate(expression)?;
                Ok(Flow::Next)
            }
            Stmt::Return { value, .. } => Ok(Flow::Return(match value {
                Some(value) => self.evaluate(value)?,
                None => Value::Void,
            })),
            Stmt::Block { body, .. } => self.execute_statements(body),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if self.evaluate_bool(condition)? {
                    self.execute_stmt(then_branch)
                } else if let Some(else_branch) = else_branch {
                    self.execute_stmt(else_branch)
                } else {
                    Ok(Flow::Next)
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                while self.evaluate_bool(condition)? {
                    match self.execute_stmt(body)? {
                        Flow::Next | Flow::Continue => {}
                        Flow::Break => break,
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
                Ok(Flow::Next)
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
                        ForInitializer::VarDecl(declaration) => {
                            self.execute_var_decl(declaration)?;
                        }
                        ForInitializer::Expr(expression) => {
                            self.evaluate(expression)?;
                        }
                    }
                }
                loop {
                    if let Some(condition) = condition {
                        if !self.evaluate_bool(condition)? {
                            break;
                        }
                    }
                    match self.execute_stmt(body)? {
                        Flow::Next | Flow::Continue => {}
                        Flow::Break => break,
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                    if let Some(update) = update {
                        self.evaluate(update)?;
                    }
                }
                Ok(Flow::Next)
            }
            Stmt::ForIn { span, .. } => Err(InterpretError::new(
                *span,
                "for-in over JsValue is only available for JavaScript targets",
            )),
            Stmt::Break(_) => Ok(Flow::Break),
            Stmt::Continue(_) => Ok(Flow::Continue),
        }
    }

    fn execute_var_decl(
        &mut self,
        declaration: &VarDecl<'ast, 'src>,
    ) -> Result<(), InterpretError> {
        let initializer = declaration.initializer.as_ref().ok_or_else(|| {
            InterpretError::new(
                declaration.span,
                "reference interpreter requires initialized variables",
            )
        })?;
        let value = self.evaluate(initializer)?;
        let symbol = self.symbol(declaration.name.span)?;
        self.declare(symbol, value);
        Ok(())
    }

    fn evaluate(&mut self, expression: &Expr<'ast, 'src>) -> Result<Value, InterpretError> {
        self.step(expression.span())?;
        match expression {
            Expr::Int(value, span) => i32::try_from(*value)
                .map(Value::Int)
                .map_err(|_| InterpretError::new(*span, "integer is outside the i32 range")),
            Expr::Float(value, _) => Ok(Value::Float(*value)),
            Expr::String(value, _) => Ok(Value::String((*value).to_string())),
            Expr::Bool(value, _) => Ok(Value::Bool(*value)),
            Expr::Null(_) => Ok(Value::Null),
            Expr::DynamicImport { span, .. } => Err(InterpretError::new(
                *span,
                "dynamic module tasks execute only in the JavaScript backend",
            )),
            Expr::Ident(identifier) => {
                let symbol = self.symbol(identifier.span)?;
                if self.functions.contains_key(&symbol) {
                    Ok(Value::Callable(Callable::Function(symbol)))
                } else {
                    self.read(symbol, identifier.span)
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                let mut values = Vec::with_capacity(elements.len());
                for element in *elements {
                    values.push(self.evaluate(element)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }
            Expr::New {
                class, args, span, ..
            } => self.evaluate_new(class.name, args, *span),
            Expr::Unary { op, expr, span } => {
                let value = self.evaluate(expr)?;
                match (op, value) {
                    (UnaryOp::Neg, Value::Int(value)) => Ok(Value::Int(value.wrapping_neg())),
                    (UnaryOp::Neg, Value::Float(value)) => Ok(Value::Float(-value)),
                    (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
                    _ => Err(InterpretError::new(*span, "invalid unary operand")),
                }
            }
            Expr::Binary { op, lhs, rhs, span } => {
                if *op == BinaryOp::And {
                    return if self.evaluate_bool(lhs)? {
                        Ok(Value::Bool(self.evaluate_bool(rhs)?))
                    } else {
                        Ok(Value::Bool(false))
                    };
                }
                if *op == BinaryOp::Or {
                    return if self.evaluate_bool(lhs)? {
                        Ok(Value::Bool(true))
                    } else {
                        Ok(Value::Bool(self.evaluate_bool(rhs)?))
                    };
                }
                let lhs = self.evaluate(lhs)?;
                let rhs = self.evaluate(rhs)?;
                self.evaluate_binary(*op, lhs, rhs, expression, *span)
            }
            Expr::Call { callee, args, span } => self.evaluate_call(callee, args, *span),
            Expr::ArrowFunction {
                params, body, span, ..
            } => {
                let captures = self.frames.last().cloned().unwrap_or_default();
                let closure = self.closures.len();
                self.closures.push(RuntimeClosure {
                    params,
                    body: body.clone(),
                    captures,
                });
                self.semantics.expression_type(*span).ok_or_else(|| {
                    InterpretError::new(*span, "closure has no checked function type")
                })?;
                Ok(Value::Callable(Callable::Closure(closure)))
            }
            Expr::Assignment {
                op,
                target,
                value,
                span,
            } => self.evaluate_assignment(*op, target, value, *span),
            Expr::Update {
                op,
                target,
                prefix,
                span,
            } => self.evaluate_update(*op, target, *prefix, *span),
            Expr::Template { parts, span } => {
                let mut output = String::new();
                for part in *parts {
                    match part {
                        TemplatePart::String(value, _) => output.push_str(value),
                        TemplatePart::Expr(value) => {
                            output.push_str(&self.evaluate(value)?.display(value.span())?);
                        }
                    }
                }
                if self.semantics.expression_type(*span) != Some(&Type::String) {
                    return Err(InterpretError::new(*span, "template has no string type"));
                }
                Ok(Value::String(output))
            }
            Expr::TypeCheck {
                value,
                target,
                span,
            } => {
                let value = self.evaluate(value)?;
                let matches = value_matches_type(&value, target.kind).ok_or_else(|| {
                    InterpretError::new(
                        *span,
                        "reference interpreter does not support this type-check target",
                    )
                })?;
                Ok(Value::Bool(matches))
            }
            Expr::Member {
                object,
                property,
                span,
            } => {
                let object = self.evaluate(object)?;
                self.evaluate_member(object, property.name, *span)
            }
            Expr::Index { span, .. } => {
                let place = self.evaluate_place(expression)?;
                self.load_place(&place, *span)
            }
            Expr::StructLiteral { span, .. } => Err(InterpretError::new(
                *span,
                "reference interpreter does not support nominal aggregate or class expressions",
            )),
        }
    }

    fn evaluate_bool(&mut self, expression: &Expr<'ast, 'src>) -> Result<bool, InterpretError> {
        match self.evaluate(expression)? {
            Value::Bool(value) => Ok(value),
            _ => Err(InterpretError::new(
                expression.span(),
                "condition did not evaluate to bool",
            )),
        }
    }

    fn evaluate_new(
        &mut self,
        name: &str,
        args: &'ast [Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Value, InterpretError> {
        if name == "Symbol" {
            if args.len() > 1 {
                return Err(InterpretError::new(
                    span,
                    "Symbol constructor expects 0 or 1 arguments",
                ));
            }
            let description = match args.first() {
                None => None,
                Some(argument) => match self.evaluate(argument)? {
                    Value::String(value) => Some(value),
                    _ => {
                        return Err(InterpretError::new(
                            span,
                            "Symbol description must be a string",
                        ));
                    }
                },
            };
            return Ok(Value::Symbol(new_symbol(description)));
        }
        let [argument] = args else {
            return Err(InterpretError::new(
                span,
                "reference interpreter only supports one-argument binary constructors",
            ));
        };
        let argument = self.evaluate(argument)?;
        match (name, argument) {
            ("ArrayBuffer", Value::Int(length)) => {
                Ok(Value::Buffer(new_buffer(length, false, span)?))
            }
            ("SharedArrayBuffer", Value::Int(length)) => {
                Ok(Value::Buffer(new_buffer(length, true, span)?))
            }
            (name, argument) if let Some(kind) = TypedArrayKind::from_name(name) => {
                Ok(Value::TypedArray(new_typed_array(kind, argument, span)?))
            }
            _ => Err(InterpretError::new(
                span,
                format!("unsupported interpreted constructor `{name}`"),
            )),
        }
    }

    fn evaluate_member(
        &self,
        object: Value,
        property: &str,
        span: Span,
    ) -> Result<Value, InterpretError> {
        match (object, property) {
            (Value::Array(values), "length") => Ok(Value::Int(
                i32::try_from(values.borrow().len())
                    .map_err(|_| InterpretError::new(span, "array length exceeds the i32 range"))?,
            )),
            (Value::Buffer(buffer), "byteLength") => Ok(Value::Int(
                i32::try_from(buffer.bytes.borrow().len()).map_err(|_| {
                    InterpretError::new(span, "buffer length exceeds the i32 range")
                })?,
            )),
            (Value::TypedArray(view), "length") => {
                Ok(Value::Int(i32::try_from(view.length).map_err(|_| {
                    InterpretError::new(
                        span,
                        format!("{} length exceeds the i32 range", view.kind.name()),
                    )
                })?))
            }
            (Value::TypedArray(view), "byteLength") => {
                let byte_length = view
                    .length
                    .checked_mul(view.kind.bytes_per_element() as usize)
                    .ok_or_else(|| {
                        InterpretError::new(
                            span,
                            format!("{} byteLength exceeds the i32 range", view.kind.name()),
                        )
                    })?;
                Ok(Value::Int(i32::try_from(byte_length).map_err(|_| {
                    InterpretError::new(
                        span,
                        format!("{} byteLength exceeds the i32 range", view.kind.name()),
                    )
                })?))
            }
            (Value::TypedArray(view), "byteOffset") => {
                Ok(Value::Int(i32::try_from(view.offset).map_err(|_| {
                    InterpretError::new(
                        span,
                        format!("{} offset exceeds the i32 range", view.kind.name()),
                    )
                })?))
            }
            (Value::TypedArray(view), "buffer") => Ok(Value::Buffer(view.buffer.clone())),
            _ => Err(InterpretError::new(
                span,
                "reference interpreter does not support this member expression",
            )),
        }
    }

    fn evaluate_binary(
        &self,
        op: BinaryOp,
        lhs: Value,
        rhs: Value,
        expression: &Expr<'ast, 'src>,
        span: Span,
    ) -> Result<Value, InterpretError> {
        use BinaryOp::{
            Add, BitAnd, BitOr, Div, Eq, Greater, GreaterEq, Less, LessEq, Mod, Mul, NotEq,
            ShiftLeft, ShiftRight, Sub, UnsignedShiftRight, Xor,
        };
        match (op, lhs, rhs) {
            (Add, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs.wrapping_add(rhs))),
            (Sub, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs.wrapping_sub(rhs))),
            (Mul, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(js_i32_multiply(lhs, rhs))),
            (Div, Value::Int(_), Value::Int(0)) => Ok(Value::Int(0)),
            (Div, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs.wrapping_div(rhs))),
            (Mod, Value::Int(_), Value::Int(0)) => Ok(Value::Int(0)),
            (Mod, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs.wrapping_rem(rhs))),
            (BitAnd, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs & rhs)),
            (BitOr, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs | rhs)),
            (Xor, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs ^ rhs)),
            (ShiftLeft, Value::Int(lhs), Value::Int(rhs)) => {
                Ok(Value::Int(lhs.wrapping_shl((rhs as u32) & 31)))
            }
            (ShiftRight, Value::Int(lhs), Value::Int(rhs)) => {
                Ok(Value::Int(lhs >> ((rhs as u32) & 31)))
            }
            (UnsignedShiftRight, Value::Int(lhs), Value::Int(rhs)) => {
                Ok(Value::Int(((lhs as u32) >> ((rhs as u32) & 31)) as i32))
            }
            (Add, Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs + rhs)),
            (Sub, Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs - rhs)),
            (Mul, Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs * rhs)),
            (Div, Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs / rhs)),
            (Add, Value::Int(lhs), Value::Float(rhs)) => Ok(Value::Float(f64::from(lhs) + rhs)),
            (Sub, Value::Int(lhs), Value::Float(rhs)) => Ok(Value::Float(f64::from(lhs) - rhs)),
            (Mul, Value::Int(lhs), Value::Float(rhs)) => Ok(Value::Float(f64::from(lhs) * rhs)),
            (Div, Value::Int(lhs), Value::Float(rhs)) => Ok(Value::Float(f64::from(lhs) / rhs)),
            (Add, Value::Float(lhs), Value::Int(rhs)) => Ok(Value::Float(lhs + f64::from(rhs))),
            (Sub, Value::Float(lhs), Value::Int(rhs)) => Ok(Value::Float(lhs - f64::from(rhs))),
            (Mul, Value::Float(lhs), Value::Int(rhs)) => Ok(Value::Float(lhs * f64::from(rhs))),
            (Div, Value::Float(lhs), Value::Int(rhs)) => Ok(Value::Float(lhs / f64::from(rhs))),
            (Add, Value::String(lhs), Value::String(rhs)) => {
                Ok(Value::String(format!("{lhs}{rhs}")))
            }
            (Eq, lhs, rhs) => Ok(Value::Bool(values_equal(&lhs, &rhs))),
            (NotEq, lhs, rhs) => Ok(Value::Bool(!values_equal(&lhs, &rhs))),
            (Less, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Bool(lhs < rhs)),
            (LessEq, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Bool(lhs <= rhs)),
            (Greater, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Bool(lhs > rhs)),
            (GreaterEq, Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Bool(lhs >= rhs)),
            (Less, Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Bool(lhs < rhs)),
            (LessEq, Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Bool(lhs <= rhs)),
            (Greater, Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Bool(lhs > rhs)),
            (GreaterEq, Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Bool(lhs >= rhs)),
            (Less, Value::String(lhs), Value::String(rhs)) => {
                Ok(Value::Bool(compare_js_strings(&lhs, &rhs).is_lt()))
            }
            (LessEq, Value::String(lhs), Value::String(rhs)) => {
                Ok(Value::Bool(!compare_js_strings(&lhs, &rhs).is_gt()))
            }
            (Greater, Value::String(lhs), Value::String(rhs)) => {
                Ok(Value::Bool(compare_js_strings(&lhs, &rhs).is_gt()))
            }
            (GreaterEq, Value::String(lhs), Value::String(rhs)) => {
                Ok(Value::Bool(!compare_js_strings(&lhs, &rhs).is_lt()))
            }
            (BinaryOp::And | BinaryOp::Or, _, _) => unreachable!(),
            _ => Err(InterpretError::new(
                span,
                format!(
                    "invalid operands for binary expression of type {:?}",
                    self.semantics.expression_type(expression.span())
                ),
            )),
        }
    }

    fn evaluate_call(
        &mut self,
        callee: &Expr<'ast, 'src>,
        args: &'ast [Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Value, InterpretError> {
        if matches!(callee, Expr::Ident(identifier) if identifier.name == "print") {
            let [argument] = args else {
                return Err(InterpretError::new(span, "print requires one argument"));
            };
            let rendered = self.evaluate(argument)?.display(argument.span())?;
            self.output.push_str(&rendered);
            self.output.push('\n');
            return Ok(Value::Void);
        }

        if let Expr::Member {
            object, property, ..
        } = callee
        {
            return self.evaluate_method_call(object, property.name, args, span);
        }

        let callable = self.evaluate(callee)?;
        let mut values = Vec::with_capacity(args.len());
        for argument in args {
            values.push(self.evaluate(argument)?);
        }
        self.invoke_callable(callable, values, span)
    }

    fn invoke_callable(
        &mut self,
        callable: Value,
        values: Vec<Value>,
        span: Span,
    ) -> Result<Value, InterpretError> {
        match callable {
            Value::Callable(Callable::Function(symbol)) => {
                self.invoke_function(symbol, values, span)
            }
            Value::Callable(Callable::Closure(closure)) => {
                self.invoke_closure(closure, values, span)
            }
            _ => Err(InterpretError::new(span, "value is not callable")),
        }
    }

    fn invoke_function(
        &mut self,
        symbol: SymbolId,
        mut values: Vec<Value>,
        span: Span,
    ) -> Result<Value, InterpretError> {
        let function = *self
            .functions
            .get(&symbol)
            .ok_or_else(|| InterpretError::new(span, "unknown interpreted function symbol"))?;
        for parameter in function.params.iter().skip(values.len()) {
            let default = parameter.default.as_ref().ok_or_else(|| {
                InterpretError::new(
                    parameter.span,
                    "missing function argument without a default",
                )
            })?;
            values.push(self.evaluate(default)?);
        }
        if values.len() != function.params.len() {
            return Err(InterpretError::new(
                span,
                "function argument count mismatch",
            ));
        }
        let mut frame = AHashMap::with_capacity(function.params.len());
        for (parameter, value) in function.params.iter().zip(values) {
            frame.insert(self.symbol(parameter.name.span)?, value);
        }
        self.execute_callable_frame(frame, function.body, None, function.span)
    }

    fn invoke_closure(
        &mut self,
        closure: usize,
        mut values: Vec<Value>,
        span: Span,
    ) -> Result<Value, InterpretError> {
        let closure = self
            .closures
            .get(closure)
            .cloned()
            .ok_or_else(|| InterpretError::new(span, "unknown interpreted closure"))?;
        for parameter in closure.params.iter().skip(values.len()) {
            let default = parameter.default.as_ref().ok_or_else(|| {
                InterpretError::new(parameter.span, "missing closure argument without a default")
            })?;
            values.push(self.evaluate(default)?);
        }
        if values.len() != closure.params.len() {
            return Err(InterpretError::new(span, "closure argument count mismatch"));
        }
        let mut frame = closure.captures;
        for (parameter, value) in closure.params.iter().zip(values) {
            frame.insert(self.symbol(parameter.name.span)?, value);
        }
        match closure.body {
            ArrowBody::Expr(expression) => {
                self.execute_callable_frame(frame, &[], Some(expression), span)
            }
            ArrowBody::Block(body) => self.execute_callable_frame(frame, body, None, span),
        }
    }

    fn execute_callable_frame(
        &mut self,
        frame: AHashMap<SymbolId, Value>,
        body: &'ast [Stmt<'ast, 'src>],
        expression: Option<&'ast Expr<'ast, 'src>>,
        span: Span,
    ) -> Result<Value, InterpretError> {
        if self.recursion_depth >= self.recursion_limit {
            return Err(InterpretError::new(
                span,
                "reference interpreter recursion limit exceeded",
            ));
        }
        self.frames.push(frame);
        self.recursion_depth += 1;
        let result = if let Some(expression) = expression {
            self.evaluate(expression).map(Flow::Return)
        } else {
            self.execute_statements(body)
        };
        self.recursion_depth -= 1;
        self.frames.pop();
        match result? {
            Flow::Return(value) => Ok(value),
            Flow::Next => Ok(Value::Void),
            Flow::Break | Flow::Continue => {
                Err(InterpretError::new(span, "loop control escaped a callable"))
            }
        }
    }

    fn evaluate_method_call(
        &mut self,
        object: &Expr<'ast, 'src>,
        method: &str,
        args: &'ast [Expr<'ast, 'src>],
        span: Span,
    ) -> Result<Value, InterpretError> {
        let receiver = self.evaluate(object)?;
        let mut arguments = Vec::with_capacity(args.len());
        for argument in args {
            arguments.push(self.evaluate(argument)?);
        }
        if matches!(receiver, Value::Buffer(_) | Value::TypedArray(_)) {
            return self.evaluate_binary_method(receiver, method, &arguments, span);
        }
        if let Value::Float(value) = &receiver {
            let value = *value;
            if method == "toInt" {
                return Ok(Value::Int(js_to_i32(value)));
            }
            let argument = || match arguments.first() {
                Some(Value::Float(argument)) => Ok(*argument),
                Some(Value::Int(argument)) => Ok(f64::from(*argument)),
                _ => Err(InterpretError::new(
                    span,
                    "float method argument must be numeric",
                )),
            };
            let result = match method {
                "abs" => value.abs(),
                "floor" => value.floor(),
                "ceil" => value.ceil(),
                "round" => js_round(value),
                "sqrt" => value.sqrt(),
                "sin" => value.sin(),
                "cos" => value.cos(),
                "acos" => value.acos(),
                "exp" => value.exp(),
                "log" => value.ln(),
                "tan" => value.tan(),
                "atan2" => value.atan2(argument()?),
                "hypot" => value.hypot(argument()?),
                "min" => js_min(value, argument()?),
                "max" => js_max(value, argument()?),
                _ => {
                    return Err(InterpretError::new(
                        span,
                        format!("unsupported float method `{method}`"),
                    ));
                }
            };
            return Ok(Value::Float(result));
        }
        if let Value::String(receiver) = &receiver {
            return self.evaluate_string_method(receiver, method, &arguments, span);
        }
        let Value::Array(array) = receiver else {
            return Err(InterpretError::new(
                span,
                "reference interpreter only supports array method calls",
            ));
        };

        match method {
            "push" => {
                let [value] = arguments.as_slice() else {
                    return Err(InterpretError::new(span, "array push requires one value"));
                };
                let mut array = array.borrow_mut();
                array.push(value.clone());
                Ok(Value::Int(i32::try_from(array.len()).map_err(|_| {
                    InterpretError::new(span, "array length exceeds the i32 range")
                })?))
            }
            "pop" => {
                if !arguments.is_empty() {
                    return Err(InterpretError::new(span, "array pop takes no arguments"));
                }
                let value = array
                    .borrow_mut()
                    .pop()
                    .ok_or_else(|| InterpretError::new(span, "cannot pop an empty typed array"))?;
                Ok(value)
            }
            "indexOf" => {
                let [needle] = arguments.as_slice() else {
                    return Err(InterpretError::new(
                        span,
                        "array indexOf requires one value",
                    ));
                };
                let array = array.borrow();
                let index = array
                    .iter()
                    .position(|value| values_equal(value, needle))
                    .map_or(-1, |index| index as i32);
                Ok(Value::Int(index))
            }
            "slice" => {
                let (start, end) = match arguments.as_slice() {
                    [] => (0, i32::MAX),
                    [Value::Int(start)] => (*start, i32::MAX),
                    [Value::Int(start), Value::Int(end)] => (*start, *end),
                    [_, ..] => {
                        return Err(InterpretError::new(
                            span,
                            "array slice expects zero, one, or two int arguments",
                        ));
                    }
                };
                let array = array.borrow();
                let start = normalize_slice_index(start, array.len());
                let end = normalize_slice_index(end, array.len()).max(start);
                Ok(Value::Array(Rc::new(RefCell::new(
                    array[start..end].to_vec(),
                ))))
            }
            "splice" => {
                let [start, delete_count] = arguments.as_slice() else {
                    return Err(InterpretError::new(
                        span,
                        "array splice requires start and deleteCount",
                    ));
                };
                let Value::Int(start) = start else {
                    return Err(InterpretError::new(
                        span,
                        "array splice start must be an int",
                    ));
                };
                let Value::Int(delete_count) = delete_count else {
                    return Err(InterpretError::new(
                        span,
                        "array splice deleteCount must be an int",
                    ));
                };
                let mut array = array.borrow_mut();
                let start = (*start).max(0) as usize;
                let delete_count = (*delete_count).max(0) as usize;
                let removed = if start >= array.len() {
                    Vec::new()
                } else {
                    let end = (start + delete_count).min(array.len());
                    array.drain(start..end).collect::<Vec<_>>()
                };
                Ok(Value::Array(Rc::new(RefCell::new(removed))))
            }
            "map" => {
                let [callback] = arguments.as_slice() else {
                    return Err(InterpretError::new(span, "array map requires one callback"));
                };
                let length = array.borrow().len();
                let mut mapped = Vec::with_capacity(length);
                for index in 0..length {
                    let value = array.borrow().get(index).cloned().ok_or_else(|| {
                        InterpretError::new(
                            span,
                            "array shrank during map before a pending element was visited",
                        )
                    })?;
                    mapped.push(self.invoke_callable(callback.clone(), vec![value], span)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(mapped))))
            }
            "filter" => {
                let [callback] = arguments.as_slice() else {
                    return Err(InterpretError::new(
                        span,
                        "array filter requires one callback",
                    ));
                };
                let length = array.borrow().len();
                let mut filtered = Vec::with_capacity(length);
                for index in 0..length {
                    let value = array.borrow().get(index).cloned().ok_or_else(|| {
                        InterpretError::new(
                            span,
                            "array shrank during filter before a pending element was visited",
                        )
                    })?;
                    let keep = self.invoke_callable(callback.clone(), vec![value.clone()], span)?;
                    match keep {
                        Value::Bool(true) => filtered.push(value),
                        Value::Bool(false) => {}
                        _ => {
                            return Err(InterpretError::new(
                                span,
                                "array filter callback did not return bool",
                            ));
                        }
                    }
                }
                Ok(Value::Array(Rc::new(RefCell::new(filtered))))
            }
            "reduce" => {
                let [callback, initial] = arguments.as_slice() else {
                    return Err(InterpretError::new(
                        span,
                        "array reduce requires a callback and initial value",
                    ));
                };
                let length = array.borrow().len();
                let mut accumulator = initial.clone();
                for index in 0..length {
                    let value = array.borrow().get(index).cloned().ok_or_else(|| {
                        InterpretError::new(
                            span,
                            "array shrank during reduce before a pending element was visited",
                        )
                    })?;
                    accumulator =
                        self.invoke_callable(callback.clone(), vec![accumulator, value], span)?;
                }
                Ok(accumulator)
            }
            "forEach" => {
                let [callback] = arguments.as_slice() else {
                    return Err(InterpretError::new(
                        span,
                        "array forEach requires one callback",
                    ));
                };
                let length = array.borrow().len();
                for index in 0..length {
                    let value = array.borrow().get(index).cloned().ok_or_else(|| {
                        InterpretError::new(
                            span,
                            "array shrank during forEach before a pending element was visited",
                        )
                    })?;
                    self.invoke_callable(callback.clone(), vec![value], span)?;
                }
                Ok(Value::Void)
            }
            _ => Err(InterpretError::new(
                span,
                format!("unsupported interpreted array method `{method}`"),
            )),
        }
    }

    fn evaluate_binary_method(
        &self,
        receiver: Value,
        method: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, InterpretError> {
        match (receiver, method) {
            (Value::Buffer(buffer), "slice") => {
                let length = buffer.bytes.borrow().len();
                let (start, end) = slice_range(arguments, length, span)?;
                let bytes = buffer.bytes.borrow()[start..end].to_vec();
                Ok(Value::Buffer(Rc::new(BufferValue {
                    bytes: RefCell::new(bytes),
                    shared: buffer.shared,
                })))
            }
            (Value::TypedArray(view), "slice") => {
                let (start, end) = slice_range(arguments, view.length, span)?;
                let bpe = view.kind.bytes_per_element() as usize;
                let absolute_start = view.offset + start * bpe;
                let absolute_end = view.offset + end * bpe;
                let bytes = view.buffer.bytes.borrow()[absolute_start..absolute_end].to_vec();
                let length = bytes.len() / bpe;
                let buffer = Rc::new(BufferValue {
                    bytes: RefCell::new(bytes),
                    shared: false,
                });
                Ok(Value::TypedArray(Rc::new(TypedArrayValue {
                    kind: view.kind,
                    buffer,
                    offset: 0,
                    length,
                })))
            }
            (Value::TypedArray(view), "subarray") => {
                let (start, end) = slice_range(arguments, view.length, span)?;
                let bpe = view.kind.bytes_per_element() as usize;
                Ok(Value::TypedArray(Rc::new(TypedArrayValue {
                    kind: view.kind,
                    buffer: view.buffer.clone(),
                    offset: view.offset + start * bpe,
                    length: end - start,
                })))
            }
            _ => Err(InterpretError::new(
                span,
                format!("unsupported interpreted binary-memory method `{method}`"),
            )),
        }
    }

    fn evaluate_assignment(
        &mut self,
        op: AssignmentOp,
        target: &Expr<'ast, 'src>,
        rhs: &Expr<'ast, 'src>,
        span: Span,
    ) -> Result<Value, InterpretError> {
        let place = self.evaluate_place(target)?;
        let value = if op == AssignmentOp::Assign {
            self.evaluate(rhs)?
        } else {
            let lhs = self.load_place(&place, target.span())?;
            let rhs = self.evaluate(rhs)?;
            self.evaluate_binary(assignment_binary_op(op), lhs, rhs, target, span)?
        };
        self.store_place(&place, value.clone(), span)?;
        Ok(value)
    }

    fn evaluate_update(
        &mut self,
        op: UpdateOp,
        target: &Expr<'ast, 'src>,
        prefix: bool,
        span: Span,
    ) -> Result<Value, InterpretError> {
        let place = self.evaluate_place(target)?;
        let old = self.load_place(&place, span)?;
        let new = match (&old, op) {
            (Value::Int(value), UpdateOp::Increment) => Value::Int(value.wrapping_add(1)),
            (Value::Int(value), UpdateOp::Decrement) => Value::Int(value.wrapping_sub(1)),
            (Value::Float(value), UpdateOp::Increment) => Value::Float(value + 1.0),
            (Value::Float(value), UpdateOp::Decrement) => Value::Float(value - 1.0),
            _ => return Err(InterpretError::new(span, "update target is not numeric")),
        };
        self.store_place(&place, new.clone(), span)?;
        Ok(if prefix { new } else { old })
    }

    fn evaluate_place(
        &mut self,
        expression: &Expr<'ast, 'src>,
    ) -> Result<RuntimePlace, InterpretError> {
        match expression {
            Expr::Ident(identifier) => Ok(RuntimePlace::Binding(self.symbol(identifier.span)?)),
            Expr::Index {
                object,
                index,
                span,
            } => {
                let object = self.evaluate(object)?;
                let Value::Int(index) = self.evaluate(index)? else {
                    return Err(InterpretError::new(*span, "index is not int"));
                };
                let index = usize::try_from(index)
                    .map_err(|_| InterpretError::new(*span, "index is negative"))?;
                match object {
                    Value::Array(array) => {
                        if index >= array.borrow().len() {
                            return Err(InterpretError::new(*span, "array index is out of bounds"));
                        }
                        Ok(RuntimePlace::ArrayElement { array, index })
                    }
                    Value::TypedArray(view) => {
                        if index >= view.length {
                            return Err(InterpretError::new(
                                *span,
                                format!("{} index is out of bounds", view.kind.name()),
                            ));
                        }
                        Ok(RuntimePlace::TypedArrayElement { view, index })
                    }
                    _ => Err(InterpretError::new(
                        *span,
                        "reference interpreter only supports array and typed array indexing",
                    )),
                }
            }
            _ => Err(InterpretError::new(
                expression.span(),
                "reference interpreter does not support this assignable location",
            )),
        }
    }

    fn load_place(&self, place: &RuntimePlace, span: Span) -> Result<Value, InterpretError> {
        match place {
            RuntimePlace::Binding(symbol) => self.read(*symbol, span),
            RuntimePlace::ArrayElement { array, index } => array
                .borrow()
                .get(*index)
                .cloned()
                .ok_or_else(|| InterpretError::new(span, "array index is out of bounds")),
            RuntimePlace::TypedArrayElement { view, index } => typed_array_get(view, *index, span),
        }
    }

    fn store_place(
        &mut self,
        place: &RuntimePlace,
        value: Value,
        span: Span,
    ) -> Result<(), InterpretError> {
        match place {
            RuntimePlace::Binding(symbol) => self.assign(*symbol, value, span),
            RuntimePlace::ArrayElement { array, index } => {
                let mut array = array.borrow_mut();
                let target = array
                    .get_mut(*index)
                    .ok_or_else(|| InterpretError::new(span, "array index is out of bounds"))?;
                *target = value;
                Ok(())
            }
            RuntimePlace::TypedArrayElement { view, index } => {
                typed_array_set(view, *index, value, span)
            }
        }
    }

    fn symbol(&self, span: Span) -> Result<SymbolId, InterpretError> {
        self.semantics
            .identifier_symbol(span)
            .ok_or_else(|| InterpretError::new(span, "missing semantic symbol"))
    }

    fn declare(&mut self, symbol: SymbolId, value: Value) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(symbol, value);
        } else {
            self.globals.insert(symbol, value);
        }
    }

    fn read(&self, symbol: SymbolId, span: Span) -> Result<Value, InterpretError> {
        self.frames
            .last()
            .and_then(|frame| frame.get(&symbol))
            .or_else(|| self.globals.get(&symbol))
            .cloned()
            .ok_or_else(|| InterpretError::new(span, "read from an uninitialized binding"))
    }

    fn assign(&mut self, symbol: SymbolId, value: Value, span: Span) -> Result<(), InterpretError> {
        if let Some(frame) = self.frames.last_mut() {
            if let Some(target) = frame.get_mut(&symbol) {
                *target = value;
                return Ok(());
            }
        }
        if let Some(target) = self.globals.get_mut(&symbol) {
            *target = value;
            return Ok(());
        }
        Err(InterpretError::new(
            span,
            "assignment to an uninitialized binding",
        ))
    }
}

#[derive(Debug, Clone)]
enum Flow {
    Next,
    Return(Value),
    Break,
    Continue,
}

impl PartialEq for Flow {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Next, Self::Next)
                | (Self::Break, Self::Break)
                | (Self::Continue, Self::Continue)
        )
    }
}

fn values_equal(lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (Value::Int(lhs), Value::Int(rhs)) => lhs == rhs,
        (Value::Float(lhs), Value::Float(rhs)) => lhs == rhs,
        (Value::Int(lhs), Value::Float(rhs)) => f64::from(*lhs) == *rhs,
        (Value::Float(lhs), Value::Int(rhs)) => *lhs == f64::from(*rhs),
        (Value::String(lhs), Value::String(rhs)) => lhs == rhs,
        (Value::Bool(lhs), Value::Bool(rhs)) => lhs == rhs,
        (Value::Array(lhs), Value::Array(rhs)) => Rc::ptr_eq(lhs, rhs),
        (Value::Buffer(lhs), Value::Buffer(rhs)) => Rc::ptr_eq(lhs, rhs),
        (Value::TypedArray(lhs), Value::TypedArray(rhs)) => Rc::ptr_eq(lhs, rhs),
        (Value::Symbol(lhs), Value::Symbol(rhs)) => Rc::ptr_eq(lhs, rhs),
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}

fn compare_js_strings(lhs: &str, rhs: &str) -> std::cmp::Ordering {
    lhs.encode_utf16().cmp(rhs.encode_utf16())
}

fn value_matches_type(value: &Value, target: TypeKind<'_, '_>) -> Option<bool> {
    match target {
        TypeKind::Int => Some(matches!(value, Value::Int(_))),
        TypeKind::Float => Some(matches!(value, Value::Float(_))),
        TypeKind::String => Some(matches!(value, Value::String(_))),
        TypeKind::Bool => Some(matches!(value, Value::Bool(_))),
        TypeKind::Array(_) => Some(matches!(value, Value::Array(_))),
        TypeKind::Function { .. } => Some(matches!(value, Value::Callable(_))),
        TypeKind::Named {
            name: "ArrayBuffer",
            ..
        } => Some(matches!(value, Value::Buffer(buffer) if !buffer.shared)),
        TypeKind::Named {
            name: "SharedArrayBuffer",
            ..
        } => Some(matches!(value, Value::Buffer(buffer) if buffer.shared)),
        TypeKind::Named { name, .. } if let Some(kind) = TypedArrayKind::from_name(name) => {
            Some(matches!(value, Value::TypedArray(view) if view.kind == kind))
        }
        TypeKind::Named { name: "Symbol", .. } => Some(matches!(value, Value::Symbol(_))),
        TypeKind::Nullable(_) if matches!(value, Value::Null) => Some(true),
        TypeKind::Nullable(inner) => value_matches_type(value, inner.kind),
        TypeKind::Union(members) => {
            let mut matches = false;
            for member in members {
                matches |= value_matches_type(value, member.kind)?;
            }
            Some(matches)
        }
        TypeKind::Void | TypeKind::Auto | TypeKind::Named { .. } => None,
    }
}

fn new_buffer(length: i32, shared: bool, span: Span) -> Result<Rc<BufferValue>, InterpretError> {
    let length = usize::try_from(length)
        .map_err(|_| InterpretError::new(span, "buffer length is negative"))?;
    Ok(Rc::new(BufferValue {
        bytes: RefCell::new(vec![0; length]),
        shared,
    }))
}

fn new_typed_array(
    kind: TypedArrayKind,
    argument: Value,
    span: Span,
) -> Result<Rc<TypedArrayValue>, InterpretError> {
    let bpe = kind.bytes_per_element() as usize;
    match argument {
        Value::Int(length) => {
            let byte_length = (length as i64)
                .checked_mul(bpe as i64)
                .and_then(|value| i32::try_from(value).ok())
                .ok_or_else(|| {
                    InterpretError::new(
                        span,
                        format!("{} length exceeds the i32 range", kind.name()),
                    )
                })?;
            let buffer = new_buffer(byte_length, false, span)?;
            Ok(Rc::new(TypedArrayValue {
                kind,
                length: usize::try_from(length).map_err(|_| {
                    InterpretError::new(span, format!("{} length cannot be negative", kind.name()))
                })?,
                buffer,
                offset: 0,
            }))
        }
        Value::Buffer(buffer) => {
            let byte_length = buffer.bytes.borrow().len();
            if byte_length % bpe != 0 {
                return Err(InterpretError::new(
                    span,
                    format!("{} buffer length must be divisible by {}", kind.name(), bpe),
                ));
            }
            Ok(Rc::new(TypedArrayValue {
                kind,
                length: byte_length / bpe,
                buffer,
                offset: 0,
            }))
        }
        _ => Err(InterpretError::new(
            span,
            format!(
                "{} expects an int, ArrayBuffer, or SharedArrayBuffer",
                kind.name()
            ),
        )),
    }
}

fn typed_array_get(
    view: &TypedArrayValue,
    index: usize,
    span: Span,
) -> Result<Value, InterpretError> {
    let bpe = view.kind.bytes_per_element() as usize;
    let start = view.offset + index * bpe;
    let bytes = view.buffer.bytes.borrow();
    let slice = bytes.get(start..start + bpe).ok_or_else(|| {
        InterpretError::new(span, format!("{} index is out of bounds", view.kind.name()))
    })?;
    Ok(match view.kind {
        TypedArrayKind::Int8 => Value::Int(i32::from(slice[0] as i8)),
        TypedArrayKind::Uint8 | TypedArrayKind::Uint8Clamped => Value::Int(i32::from(slice[0])),
        TypedArrayKind::Int16 => {
            let mut raw = [0u8; 2];
            raw.copy_from_slice(slice);
            Value::Int(i32::from(i16::from_le_bytes(raw)))
        }
        TypedArrayKind::Uint16 => {
            let mut raw = [0u8; 2];
            raw.copy_from_slice(slice);
            Value::Int(i32::from(u16::from_le_bytes(raw)))
        }
        TypedArrayKind::Int32 | TypedArrayKind::Uint32 => {
            let mut raw = [0u8; 4];
            raw.copy_from_slice(slice);
            Value::Int(i32::from_le_bytes(raw))
        }
        TypedArrayKind::Float32 => {
            let mut raw = [0u8; 4];
            raw.copy_from_slice(slice);
            Value::Float(f64::from(f32::from_le_bytes(raw)))
        }
        TypedArrayKind::Float64 => {
            let mut raw = [0u8; 8];
            raw.copy_from_slice(slice);
            Value::Float(f64::from_le_bytes(raw))
        }
    })
}

fn typed_array_set(
    view: &TypedArrayValue,
    index: usize,
    value: Value,
    span: Span,
) -> Result<(), InterpretError> {
    let bpe = view.kind.bytes_per_element() as usize;
    let start = view.offset + index * bpe;
    let mut bytes = view.buffer.bytes.borrow_mut();
    let target = bytes.get_mut(start..start + bpe).ok_or_else(|| {
        InterpretError::new(span, format!("{} index is out of bounds", view.kind.name()))
    })?;
    if view.kind.element_is_float() {
        let float = match value {
            Value::Float(value) => value,
            Value::Int(value) => f64::from(value),
            _ => {
                return Err(InterpretError::new(
                    span,
                    format!("{} assignment value is not float", view.kind.name()),
                ));
            }
        };
        match view.kind {
            TypedArrayKind::Float32 => target.copy_from_slice(&(float as f32).to_le_bytes()),
            TypedArrayKind::Float64 => target.copy_from_slice(&float.to_le_bytes()),
            _ => unreachable!(),
        }
        return Ok(());
    }
    let Value::Int(value) = value else {
        return Err(InterpretError::new(
            span,
            format!("{} assignment value is not int", view.kind.name()),
        ));
    };
    match view.kind {
        TypedArrayKind::Int8 => target[0] = value as i8 as u8,
        TypedArrayKind::Uint8 => target[0] = value as u8,
        TypedArrayKind::Uint8Clamped => {
            target[0] = if value < 0 {
                0
            } else if value > 255 {
                255
            } else {
                value as u8
            };
        }
        TypedArrayKind::Int16 => target.copy_from_slice(&(value as i16).to_le_bytes()),
        TypedArrayKind::Uint16 => target.copy_from_slice(&(value as u16).to_le_bytes()),
        TypedArrayKind::Int32 | TypedArrayKind::Uint32 => {
            target.copy_from_slice(&value.to_le_bytes());
        }
        TypedArrayKind::Float32 | TypedArrayKind::Float64 => unreachable!(),
    }
    Ok(())
}

fn slice_range(
    arguments: &[Value],
    length: usize,
    span: Span,
) -> Result<(usize, usize), InterpretError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(InterpretError::new(
            span,
            "binary-memory slice requires a start and optional end",
        ));
    }
    let Value::Int(start) = arguments[0] else {
        return Err(InterpretError::new(span, "slice start is not int"));
    };
    let end = match arguments.get(1) {
        Some(Value::Int(end)) => *end,
        Some(_) => return Err(InterpretError::new(span, "slice end is not int")),
        None => i32::MAX,
    };
    let start = normalize_slice_index(start, length);
    let end = normalize_slice_index(end, length).max(start);
    Ok((start, end))
}

fn normalize_slice_index(index: i32, length: usize) -> usize {
    if index < 0 {
        (length as i64 + i64::from(index)).clamp(0, length as i64) as usize
    } else {
        usize::try_from(index).unwrap_or(usize::MAX).min(length)
    }
}

fn js_round(value: f64) -> f64 {
    if value.is_sign_negative() && value >= -0.5 {
        -0.0
    } else {
        (value + 0.5).floor()
    }
}

fn js_to_i32(value: f64) -> i32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let mut normalized = value.trunc() % 4_294_967_296.0;
    if normalized < 0.0 {
        normalized += 4_294_967_296.0;
    }
    if normalized >= 2_147_483_648.0 {
        (normalized - 4_294_967_296.0) as i32
    } else {
        normalized as i32
    }
}

fn js_min(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_negative() || right.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else if left < right {
        left
    } else {
        right
    }
}

fn js_max(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_negative() && right.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else if left > right {
        left
    } else {
        right
    }
}

impl<'program, 'ast, 'src> ReferenceInterpreter<'program, 'ast, 'src> {
    fn evaluate_string_method(
        &self,
        receiver: &str,
        method: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, InterpretError> {
        match method {
            "charCodeAt" => {
                let Value::Int(index) = arguments.first().cloned().unwrap_or(Value::Int(0)) else {
                    return Err(InterpretError::new(
                        span,
                        "charCodeAt requires an int index",
                    ));
                };
                let code = if index < 0 {
                    0
                } else {
                    receiver
                        .encode_utf16()
                        .nth(usize::try_from(index).unwrap_or(usize::MAX))
                        .unwrap_or(0)
                };
                Ok(Value::Int(i32::from(code)))
            }
            "charAt" => {
                let Value::Int(index) = arguments.first().cloned().unwrap_or(Value::Int(0)) else {
                    return Err(InterpretError::new(span, "charAt requires an int index"));
                };
                let unit = if index < 0 {
                    None
                } else {
                    receiver
                        .encode_utf16()
                        .nth(usize::try_from(index).unwrap_or(usize::MAX))
                };
                Ok(Value::String(match unit {
                    None => String::new(),
                    Some(unit) => char::decode_utf16([unit])
                        .next()
                        .and_then(Result::ok)
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                }))
            }
            "length" => Ok(Value::Int(
                i32::try_from(receiver.encode_utf16().count()).map_err(|_| {
                    InterpretError::new(span, "string length exceeds the i32 range")
                })?,
            )),
            _ => Err(InterpretError::new(
                span,
                format!("unsupported interpreted string method `{method}`"),
            )),
        }
    }
}

fn js_i32_multiply(lhs: i32, rhs: i32) -> i32 {
    let product = f64::from(lhs) * f64::from(rhs);
    (product as i64 as u32) as i32
}

const fn assignment_binary_op(op: AssignmentOp) -> BinaryOp {
    match op {
        AssignmentOp::Assign => unreachable!(),
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

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::{analyze, parse_source};

    fn run(source: &str) -> String {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        interpret_program(&program, &semantics).unwrap()
    }

    #[test]
    fn evaluates_i32_edge_semantics() {
        assert_eq!(
            run("print(2147483647+1);print((-2147483647-1)/-1);print(9/0);print(-1>>>1);"),
            "-2147483648\n-2147483648\n0\n2147483647\n"
        );
    }

    #[test]
    fn preserves_short_circuit_side_effects() {
        assert_eq!(
            run("int calls=0;bool probe(bool value){calls++;return value;}print(probe(false)&&probe(true)||probe(true));print(calls);"),
            "true\n2\n"
        );
    }

    #[test]
    fn executes_loops_functions_and_shadowed_bindings() {
        assert_eq!(
            run("int sum(int limit){int total=0;for(int i=0;i<limit;i++){if(i==2){continue;}if(i==6){break;}total+=i;}int i=0;while(i<2){total+=i;i++;}{int total=9;total++;}return total;}print(sum(8));"),
            "14\n"
        );
    }

    #[test]
    fn evaluates_array_identity_mutation_and_methods() {
        assert_eq!(
            run(r#"
                    int[] values=[1,2,3];
                    int[] alias=values;
                    print(values==alias);
                    print(values==[1,2,3]);
                    print(++alias[0]);
                    print(values[0]++);
                    print(values[0]);
                    print(values.push(4));
                    print(values.pop());
                    int factor=3;
                    int[] mapped=values.map((int value)=>value*factor);
                    int[] filtered=values.filter((int value)=>value>2);
                    int sum=values.reduce((int total,int value)=>total+value,0);
                    values.forEach((int value)=>{print(value);});
                    print(values.length);
                    print(mapped[2]);
                    print(filtered.length);
                    print(sum);
                "#,),
            "true\nfalse\n2\n2\n3\n4\n4\n3\n2\n3\n3\n9\n2\n8\n"
        );
    }

    #[test]
    fn snapshots_array_iteration_length() {
        assert_eq!(
            run(r#"
                    int[] values=[1,2];
                    int append(int value){if(value==1){values.push(3);}return value*2;}
                    int[] mapped=values.map(append);
                    print(values.length);
                    print(mapped.length);
                    print(mapped[1]);
                "#,),
            "3\n2\n4\n"
        );
    }

    #[test]
    fn copies_array_slices_with_javascript_index_normalization() {
        assert_eq!(
            run(r#"
                    int[] values=[1,2,3,4];
                    int[] copied=values.slice();
                    copied[0]=9;
                    int[] middle=values.slice(1,-1);
                    int[] tail=values.slice(-2);
                    int[] empty=values.slice(3,1);
                    print(values[0]);
                    print(copied[0]);
                    print(middle.length);
                    print(middle[0]);
                    print(middle[1]);
                    print(tail[0]);
                    print(tail[1]);
                    print(empty.length);
                "#),
            "1\n9\n2\n2\n3\n3\n4\n0\n"
        );
    }

    #[test]
    fn evaluates_binary_memory_aliasing_and_byte_coercion() {
        assert_eq!(
            run(r#"
                    ArrayBuffer buffer=new ArrayBuffer(4);
                    Uint8Array bytes=new Uint8Array(buffer);
                    Uint8Array same=bytes;
                    print(same==bytes);
                    print(bytes==new Uint8Array(buffer));
                    print(bytes.buffer==buffer);
                    bytes[0]=257;
                    bytes[1]=-1;
                    print(buffer.byteLength);
                    print(bytes[0]);
                    print(bytes[1]);
                    print(++bytes[1]);
                    print(bytes[1]);
                    Uint8Array window=bytes.subarray(-2,4);
                    print(window.byteOffset);
                    window[0]=9;
                    print(bytes[2]);
                    Uint8Array copied=bytes.slice(1,4);
                    copied[1]=7;
                    print(copied.length);
                    print(copied[1]);
                    print(bytes[2]);
                    SharedArrayBuffer shared=new SharedArrayBuffer(3);
                    Uint8Array sharedBytes=new Uint8Array(shared);
                    sharedBytes[1]=42;
                    SharedArrayBuffer sharedCopy=shared.slice(1,3);
                    Uint8Array sharedCopyBytes=new Uint8Array(sharedCopy);
                    print(shared.byteLength);
                    print(sharedCopy.byteLength);
                    print(sharedCopyBytes[0]);
                "#,),
            "true\nfalse\ntrue\n4\n1\n255\n256\n0\n2\n9\n3\n7\n9\n3\n2\n42\n"
        );
    }

    #[test]
    fn enforces_step_limits() {
        let arena = Bump::new();
        let program = parse_source(&arena, "while(true){}").unwrap();
        let semantics = analyze(&program).unwrap();
        let error = interpret_program_with_limits(
            &program,
            &semantics,
            InterpreterLimits {
                steps: 20,
                recursion_depth: 4,
            },
        )
        .unwrap_err();
        assert!(error.message.contains("step limit"));
    }
}
