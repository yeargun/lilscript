//! The printed tree in the admission parse's canonical form (plan task M2.5).
//!
//! `crate::admission_parse` states the form and reads the delivered text back
//! into it through Oxc; this is the other side: what `print` renders from a
//! module, reduced to the same form. It follows the printer's single-file
//! layout (imports not carried by host modules, the host-module prelude, the
//! root region, the export list) and its native parameter defaults; every
//! other spelling choice is reduced away by the form itself, so this walk
//! does not depend on the printer's decisions.
use super::*;
use crate::admission_parse::{self, Canon, CanonFunction, StructureDigest};

/// The structural digest of the single file `print` renders for `module`.
pub(crate) fn digest(
    module: &Module,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
) -> StructureDigest {
    admission_parse::digest(&program(module, hosts))
}

fn hosted(hosts: Option<&crate::host_modules::HostDelivery>, import: &Import) -> bool {
    hosts.is_some_and(|hosts| {
        import
            .source
            .as_unicode()
            .is_some_and(|source| hosts.position(source).is_some())
    })
}

pub(crate) fn program(
    module: &Module,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
) -> Vec<Canon> {
    let hosts = hosts.map(|(hosts, _)| hosts);
    let mut out = Vec::new();
    for import in &module.imports {
        if !hosted(hosts, import) {
            out.push(Canon::Import(1));
        }
    }
    if module.imports.iter().any(|import| hosted(hosts, import)) {
        // `let{…}=<host modules>;`: foreign text, compared by kind.
        out.push(Canon::Pattern);
    }
    let walk = Walk { module };
    for statement in &module.regions[module.root.index()].statements {
        out.push(walk.statement(statement));
    }
    if !module.exports.is_empty() {
        out.push(Canon::Export(module.exports.len()));
    }
    out
}

struct Walk<'a> {
    module: &'a Module,
}

fn unary(op: Unary) -> &'static str {
    match op {
        Unary::Negate => "-",
        Unary::Not => "!",
        Unary::BitNot => "~",
        Unary::Plus => "+",
        Unary::TypeOf => "typeof",
        Unary::Void => "void",
        Unary::Delete => "delete",
    }
}

fn binary(op: Binary) -> &'static str {
    match op {
        Binary::Add => "+",
        Binary::Subtract => "-",
        Binary::Multiply => "*",
        Binary::Divide => "/",
        Binary::Remainder => "%",
        Binary::ShiftLeft => "<<",
        Binary::ShiftRight => ">>",
        Binary::UnsignedShiftRight => ">>>",
        Binary::Less => "<",
        Binary::LessEqual => "<=",
        Binary::Greater => ">",
        Binary::GreaterEqual => ">=",
        Binary::StrictEqual => "===",
        Binary::StrictNotEqual => "!==",
        Binary::Equal => "==",
        Binary::NotEqual => "!=",
        Binary::In => "in",
        Binary::InstanceOf => "instanceof",
        Binary::BitAnd => "&",
        Binary::BitXor => "^",
        Binary::BitOr => "|",
        Binary::And => "&&",
        Binary::Or => "||",
        Binary::Nullish => "??",
    }
}

impl Walk<'_> {
    fn list(&self, region: RegionId) -> Vec<Canon> {
        self.list_from(region, 0)
    }

    fn list_from(&self, region: RegionId, start: usize) -> Vec<Canon> {
        admission_parse::statement_list(
            self.module.regions[region.index()].statements[start..]
                .iter()
                .map(|statement| self.statement(statement))
                .collect(),
        )
    }

    fn statement(&self, statement: &Statement) -> Canon {
        match statement {
            Statement::Let { value, .. } => {
                Canon::Let(value.map(|value| Box::new(self.expression(value))))
            }
            Statement::Evaluate(value) => Canon::statement(self.expression(*value)),
            Statement::Return(value) => {
                Canon::Return(value.map(|value| Box::new(self.expression(value))))
            }
            Statement::Throw(value) => Canon::Throw(Box::new(self.expression(*value))),
            Statement::If { condition, yes, no } => Canon::if_statement(
                self.expression(*condition),
                self.list(*yes),
                no.map(|no| self.list(no)),
            ),
            Statement::Loop {
                condition,
                update,
                body,
            } => Canon::Loop(
                condition.map(|condition| Box::new(self.expression(condition))),
                update.map(|update| Box::new(self.expression(update))),
                self.list(*body),
            ),
            Statement::Block(region) => Canon::Block(self.list(*region)),
            Statement::ForIn { object, body, .. } => {
                Canon::ForIn(Box::new(self.expression(*object)), self.list(*body))
            }
            Statement::ForOf { iterable, body, .. } => {
                Canon::ForOf(Box::new(self.expression(*iterable)), self.list(*body))
            }
            Statement::Try {
                body,
                catch,
                finally,
            } => Canon::Try(
                self.list(*body),
                catch
                    .as_ref()
                    .map(|catch| (catch.binding.is_some(), self.list(catch.body))),
                finally.map(|finally| self.list(finally)),
            ),
            Statement::Break => Canon::Break,
            Statement::Continue => Canon::Continue,
            Statement::Function { function, .. } => {
                Canon::FunctionDeclaration(Box::new(self.function(*function)))
            }
        }
    }

    /// The parameter defaults the printer spells natively (`(a,b=null)`),
    /// and how many leading body statements they absorb: the printer's rule,
    /// on the module's own `default_check` and `arguments_free`.
    fn native_defaults(&self, id: FunctionId) -> (Vec<Option<ExprId>>, usize) {
        let function = &self.module.functions[id.index()];
        let Some(length) = function.length else {
            return (Vec::new(), 0);
        };
        if function.strict || !self.module.arguments_free(id) {
            return (Vec::new(), 0);
        }
        let mut defaults = vec![None; function.parameters.len()];
        let mut absorbed = 0;
        let mut last = None;
        for statement in &self.module.regions[function.body.index()].statements {
            let Some((parameter, default)) = self.module.default_check(statement) else {
                break;
            };
            let Some(index) = function.parameters.iter().position(|&p| p == parameter) else {
                break;
            };
            if index < length || last.is_some_and(|last| index <= last) {
                break;
            }
            defaults[index] = Some(default);
            last = Some(index);
            absorbed += 1;
        }
        (defaults, absorbed)
    }

    fn function(&self, id: FunctionId) -> CanonFunction {
        let (defaults, absorbed) = self.native_defaults(id);
        let function = &self.module.functions[id.index()];
        let parameters = (0..function.parameters.len())
            .map(|index| {
                function.length.filter(|&length| index >= length).map(|_| {
                    defaults
                        .get(index)
                        .copied()
                        .flatten()
                        // `b=void 0` keeps the length without a default.
                        .map_or(Canon::Lit, |default| self.expression(default))
                })
            })
            .collect();
        CanonFunction {
            arrow: function.arrow,
            asynchronous: function.suspension == Suspension::Async,
            generator: function.suspension == Suspension::Generator,
            strict: function.strict,
            parameters,
            rest: false,
            body: self.list_from(function.body, absorbed),
        }
    }

    fn expressions(&self, values: &[ExprId]) -> Vec<Canon> {
        values.iter().map(|value| self.expression(*value)).collect()
    }

    fn expression(&self, id: ExprId) -> Canon {
        match &self.module.expressions[id.index()] {
            Expr::Literal(Literal::Number(value)) if !value.is_finite() => {
                // Printed as Rust spells it: `NaN`, `inf` or `-inf`.
                if value.is_sign_negative() && !value.is_nan() {
                    Canon::Unary("-", Box::new(Canon::Ident))
                } else {
                    Canon::Ident
                }
            }
            Expr::Literal(_) => Canon::Lit,
            Expr::Binding(_) => Canon::Ident,
            Expr::Host(name) => name
                .split('.')
                .skip(1)
                .fold(Canon::Ident, |object, _| Canon::member(object, None)),
            Expr::This => Canon::This,
            Expr::Regex(_) => Canon::Regex,
            Expr::Unary { op, value } => Canon::unary(unary(*op), self.expression(*value)),
            // `|0` is reduced away by the form, printed or not.
            Expr::ToInt32(value) => self.expression(*value),
            Expr::IntNegate(value) => Canon::unary("-", self.expression(*value)),
            Expr::IntBinary { op, left, right } => Canon::binary(
                binary(op.javascript()),
                self.expression(*left),
                self.expression(*right),
            ),
            Expr::Binary { op, left, right } => {
                Canon::binary(binary(*op), self.expression(*left), self.expression(*right))
            }
            Expr::Intrinsic {
                operation,
                receiver,
                arguments,
            } => {
                let member = Canon::member(self.expression(*receiver), None);
                match intrinsic_form(*operation) {
                    IntrinsicForm::Property(_) => member,
                    IntrinsicForm::Method(_) => {
                        Canon::Call(Box::new(member), self.expressions(arguments))
                    }
                }
            }
            Expr::ConstructIntrinsic { arguments, .. } => {
                Canon::New(Box::new(Canon::Ident), self.expressions(arguments))
            }
            Expr::Member { object, property } => {
                let key = match property {
                    Property::Named(_) => None,
                    Property::Computed(key) => Canon::key(self.expression(*key)),
                };
                Canon::member(self.expression(*object), key)
            }
            Expr::Call {
                callee, arguments, ..
            } => Canon::Call(
                Box::new(self.expression(*callee)),
                self.expressions(arguments),
            ),
            Expr::Construct { callee, arguments } => Canon::New(
                Box::new(self.expression(*callee)),
                self.expressions(arguments),
            ),
            Expr::Conditional { condition, yes, no } => Canon::Conditional(
                Box::new(self.expression(*condition)),
                Box::new(self.expression(*yes)),
                Box::new(self.expression(*no)),
            ),
            Expr::Assign { target, value } => Canon::Assign(
                Box::new(self.expression(*target)),
                Box::new(self.expression(*value)),
            ),
            Expr::Sequence(values) => Canon::sequence(self.expressions(values)),
            Expr::Template(parts) => Canon::Template(
                parts
                    .iter()
                    .filter_map(|part| match part {
                        TemplatePart::Expression(value) => Some(self.expression(*value)),
                        TemplatePart::String(_) => None,
                    })
                    .collect(),
            ),
            Expr::Array(values) => Canon::Array(self.expressions(values)),
            Expr::Spread(value) => Canon::Spread(Box::new(self.expression(*value))),
            Expr::Object(entries) => Canon::Object(
                entries
                    .iter()
                    .map(|(key, value)| {
                        let key = match key {
                            Property::Named(_) => None,
                            Property::Computed(key) => Canon::key(self.expression(*key)),
                        };
                        (key, self.expression(*value))
                    })
                    .collect(),
            ),
            Expr::Function(function) => Canon::Function(Box::new(self.function(*function))),
            Expr::Class {
                base,
                constructor,
                methods,
                ..
            } => Canon::Class(
                base.map(|base| Box::new(self.expression(base))),
                std::iter::once(*constructor)
                    .chain(methods.iter().map(|(_, function)| *function))
                    .map(|function| self.function(function))
                    .collect(),
            ),
            Expr::SuperCall { arguments } => {
                Canon::Call(Box::new(Canon::Super), self.expressions(arguments))
            }
            Expr::Await(value) => Canon::Await(Box::new(self.expression(*value))),
            Expr::Yield { value, delegate } => {
                Canon::Yield(*delegate, Some(Box::new(self.expression(*value))))
            }
            Expr::LoadModule {
                members, promise, ..
            } => {
                // A single file has no lazy chunk: the namespace is
                // `P.resolve({})`, or `P.resolve().then(()=>({…}))` a turn later.
                let resolve = Canon::member(self.expression(*promise), None);
                if members.is_empty() {
                    Canon::Call(Box::new(resolve), vec![Canon::Object(Vec::new())])
                } else {
                    let namespace = Canon::Object(
                        members
                            .iter()
                            .map(|(_, value)| (None, self.expression(*value)))
                            .collect(),
                    );
                    Canon::Call(
                        Box::new(Canon::member(
                            Canon::Call(Box::new(resolve), Vec::new()),
                            None,
                        )),
                        vec![Canon::Function(Box::new(CanonFunction {
                            arrow: true,
                            asynchronous: false,
                            generator: false,
                            strict: false,
                            parameters: Vec::new(),
                            rest: false,
                            body: vec![Canon::Return(Some(Box::new(namespace)))],
                        }))],
                    )
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
