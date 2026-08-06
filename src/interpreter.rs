use std::fmt;

use ahash::AHashMap;

use crate::ast::{
    AssignmentOp, BinaryOp, Expr, ForInitializer, FunctionDecl, Item, Program, Stmt, TemplatePart,
    TypeKind, UnaryOp, UpdateOp, VarDecl,
};
use crate::semantic::{SemanticModel, SymbolId, Type};
use crate::span::Span;

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
    Null,
    Void,
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

/// Evaluates the checked scalar and control-flow core without going through IR.
///
/// This intentionally independent path is used as a semantic oracle for
/// differential compiler testing. Unsupported host, aggregate, class, and
/// closure operations fail explicitly instead of approximating their behavior.
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
            Expr::Ident(identifier) => self.read(self.symbol(identifier.span)?, identifier.span),
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
            Expr::ArrayLiteral { span, .. }
            | Expr::StructLiteral { span, .. }
            | Expr::New { span, .. }
            | Expr::Member { span, .. }
            | Expr::ArrowFunction { span, .. }
            | Expr::Index { span, .. } => Err(InterpretError::new(
                *span,
                "reference interpreter does not support aggregate, class, or closure expressions",
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
        let Expr::Ident(identifier) = callee else {
            return Err(InterpretError::new(
                span,
                "reference interpreter only supports direct function calls",
            ));
        };
        if identifier.name == "print" {
            let [argument] = args else {
                return Err(InterpretError::new(span, "print requires one argument"));
            };
            let rendered = self.evaluate(argument)?.display(argument.span())?;
            self.output.push_str(&rendered);
            self.output.push('\n');
            return Ok(Value::Void);
        }

        let symbol = self.symbol(identifier.span)?;
        let function = *self.functions.get(&symbol).ok_or_else(|| {
            InterpretError::new(
                span,
                format!("unknown interpreted function `{}`", identifier.name),
            )
        })?;
        let mut values = Vec::with_capacity(function.params.len());
        for argument in args {
            values.push(self.evaluate(argument)?);
        }
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
        if self.recursion_depth >= self.recursion_limit {
            return Err(InterpretError::new(
                span,
                "reference interpreter recursion limit exceeded",
            ));
        }

        let mut frame = AHashMap::with_capacity(function.params.len());
        for (parameter, value) in function.params.iter().zip(values) {
            frame.insert(self.symbol(parameter.name.span)?, value);
        }
        self.frames.push(frame);
        self.recursion_depth += 1;
        let result = self.execute_statements(function.body);
        self.recursion_depth -= 1;
        self.frames.pop();
        match result? {
            Flow::Return(value) => Ok(value),
            Flow::Next => Ok(Value::Void),
            Flow::Break | Flow::Continue => Err(InterpretError::new(
                function.span,
                "loop control escaped a function",
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
        let Expr::Ident(identifier) = target else {
            return Err(InterpretError::new(
                span,
                "reference interpreter only supports scalar assignments",
            ));
        };
        let symbol = self.symbol(identifier.span)?;
        let value = if op == AssignmentOp::Assign {
            self.evaluate(rhs)?
        } else {
            let lhs = self.read(symbol, identifier.span)?;
            let rhs = self.evaluate(rhs)?;
            self.evaluate_binary(assignment_binary_op(op), lhs, rhs, target, span)?
        };
        self.assign(symbol, value.clone(), span)?;
        Ok(value)
    }

    fn evaluate_update(
        &mut self,
        op: UpdateOp,
        target: &Expr<'ast, 'src>,
        prefix: bool,
        span: Span,
    ) -> Result<Value, InterpretError> {
        let Expr::Ident(identifier) = target else {
            return Err(InterpretError::new(
                span,
                "reference interpreter only supports scalar updates",
            ));
        };
        let symbol = self.symbol(identifier.span)?;
        let Value::Int(old) = self.read(symbol, span)? else {
            return Err(InterpretError::new(span, "update target is not int"));
        };
        let new = match op {
            UpdateOp::Increment => old.wrapping_add(1),
            UpdateOp::Decrement => old.wrapping_sub(1),
        };
        self.assign(symbol, Value::Int(new), span)?;
        Ok(Value::Int(if prefix { new } else { old }))
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
        TypeKind::Nullable(_) if matches!(value, Value::Null) => Some(true),
        TypeKind::Nullable(inner) => value_matches_type(value, inner.kind),
        TypeKind::Union(members) => {
            let mut matches = false;
            for member in members {
                matches |= value_matches_type(value, member.kind)?;
            }
            Some(matches)
        }
        TypeKind::Void
        | TypeKind::Auto
        | TypeKind::Named { .. }
        | TypeKind::Array(_)
        | TypeKind::Function { .. } => None,
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
