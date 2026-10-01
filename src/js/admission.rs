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
    format: crate::config::JavaScriptFormat,
) -> StructureDigest {
    let body = program(module, hosts);
    if format == crate::config::JavaScriptFormat::Iife {
        let function = CanonFunction {
            arrow: true,
            asynchronous: false,
            generator: false,
            strict: false,
            parameters: vec![],
            rest: false,
            body: admission_parse::statement_list(body),
        };
        admission_parse::digest(&[Canon::statement(Canon::Call(
            Box::new(Canon::Function(Box::new(function))),
            vec![],
        ))])
    } else {
        admission_parse::digest(&body)
    }
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
    // One import declaration per foreign specifier, at its first occurrence,
    // with every import of that specifier (the printer's `foreign_imports`).
    let mut sources: Vec<(&crate::literal::StringValue, usize)> = Vec::new();
    for import in module
        .imports
        .iter()
        .filter(|import| !hosted(hosts, import))
    {
        match sources
            .iter_mut()
            .find(|(source, _)| **source == import.source)
        {
            Some((_, count)) => *count += 1,
            None => sources.push((&import.source, 1)),
        }
    }
    for (_, specifiers) in sources {
        out.push(Canon::Import(specifiers));
    }
    if module.imports.iter().any(|import| hosted(hosts, import)) {
        // `let{…}=<host modules>;`: foreign text, compared by kind.
        out.push(Canon::Pattern);
    }
    let walk = Walk { module, member_imports:&[], lazy:&[], commonjs:false, module_root:false, root_activation:true, inline_loads:false };
    for statement in &module.regions[module.root.index()].statements {
        out.push(walk.statement(statement));
    }
    if !module.exports.is_empty() {
        out.push(Canon::Export(module.exports.len()));
    }
    out
}

#[derive(Clone, Copy)]
struct Walk<'a> {
    module: &'a Module,
    member_imports: &'a [BindingId],
    lazy: &'a [(u32,String)],
    commonjs: bool,
    module_root: bool,
    root_activation: bool,
    inline_loads: bool,
}

/// A planned file's source-owned statements, independently of wrapper text.
/// Import reads and lazy links are part of its explicit delivery recipe.
pub(super) fn planned_core_digest(module:&Module, statements:&[u32], member_imports:&[BindingId],
    lazy:&[(u32,String)], commonjs:bool, module_root:bool, inline_loads:bool, hoist:bool) -> StructureDigest {
    let walk=Walk{module,member_imports,lazy,commonjs,module_root,root_activation:true,inline_loads};
    let root=&module.regions[module.root.index()].statements;
    admission_parse::digest(&admission_parse::statement_list(statements.iter().map(|&index| {
        let statement = &root[index as usize];
        if hoist {
            if let Statement::Let { binding, value: Some(value) } = *statement {
                if module.settled.get(binding.index()).copied().flatten() == Some(0) {
                    if let Expr::Function(function) = module.expressions[value.index()] {
                        return walk.statement(&Statement::Function { binding, function });
                    }
                }
            }
        }
        walk.statement(statement)
    }).collect()))
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
            Statement::If {
                condition,
                yes,
                no: None,
            } if self.module.print_forms.as_ref().is_some_and(|forms| {
                forms.logical_assignments[yes.index()]
                    .is_some_and(|form| form.condition == *condition)
            }) =>
            {
                let form = self
                    .module
                    .print_forms
                    .as_ref()
                    .unwrap()
                    .logical_assignments[yes.index()]
                .unwrap();
                Canon::statement(Canon::LogicalAssign(
                    binary(form.op),
                    Box::new(self.expression(form.left)),
                    Box::new(self.expression(form.right)),
                ))
            }
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
        let walk=Self{root_activation:self.root_activation && self.module.functions[id.index()].arrow,..*self};
        walk.function_inner(id)
    }
    fn function_inner(&self, id: FunctionId) -> CanonFunction {
        let (defaults, absorbed) = self.native_defaults(id);
        let function = &self.module.functions[id.index()];
        // From `length` on, a parameter prints its native default; the one at
        // `length` without a default prints `=void 0`, which keeps the
        // reflected length, and later ones print none.
        let parameters = (0..function.parameters.len() - usize::from(function.rest))
            .map(|index| {
                let length = function.length.filter(|&length| index >= length)?;
                match defaults.get(index).copied().flatten() {
                    Some(default) => Some(self.expression(default)),
                    None if index == length => Some(Canon::Lit),
                    None => None,
                }
            })
            .collect();
        CanonFunction {
            arrow: function.arrow,
            asynchronous: function.suspension == Suspension::Async,
            generator: function.suspension == Suspension::Generator,
            strict: function.strict,
            parameters,
            rest: function.rest,
            body: self.list_from(function.body, absorbed),
        }
    }

    fn expressions(&self, values: &[ExprId]) -> Vec<Canon> {
        values.iter().map(|value| self.expression(*value)).collect()
    }

    fn expression(&self, id: ExprId) -> Canon {
        if let Some(member) = self
            .module
            .print_forms
            .as_ref()
            .and_then(|forms| forms.optional[id.index()])
        {
            let Expr::Member { object, property } = &self.module.expressions[member.index()] else {
                unreachable!("proved optional member")
            };
            let key = match property {
                Property::Named(_) => None,
                Property::Computed(key) => Canon::key(self.expression(*key)).map(Box::new),
            };
            return Canon::OptionalMember(Box::new(self.expression(*object)), key);
        }
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
            Expr::Binding(binding) => if self.member_imports.binary_search(binding).is_ok() {
                Canon::member(Canon::Ident,None)
            } else {Canon::Ident},
            Expr::Host(host) => host
                .name
                .split('.')
                .skip(1)
                .fold(Canon::Ident, |object, _| Canon::member(object, None)),
            Expr::This => if self.root_activation && self.module_root {Canon::Lit} else {Canon::This},
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
                constructor
                    .iter()
                    .copied()
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
                module, members, promise, string, ..
            } => {
                if self.inline_loads || self.lazy.iter().any(|(loaded,_)|loaded==module) {
                    let arrow=|parameters, value| Canon::Function(Box::new(CanonFunction {
                        arrow:true,asynchronous:false,generator:false,strict:false,
                        parameters:vec![None;parameters],rest:false,body:vec![Canon::Return(Some(Box::new(value)))],
                    }));
                    let loaded=if self.commonjs || self.inline_loads {
                        let resolve=Canon::Call(Box::new(Canon::member(self.expression(*promise),None)),vec![]);
                        Canon::Call(Box::new(Canon::member(resolve,None)),vec![arrow(0,Canon::Call(Box::new(Canon::Ident),vec![Canon::Lit]))])
                    } else {Canon::ImportCall(Box::new(Canon::Lit))};
                    let error=Canon::Object(vec![(None,Canon::Lit),(None,Canon::Call(Box::new(self.expression(*string)),vec![Canon::Ident]))]);
                    return Canon::Call(Box::new(Canon::member(loaded,None)),vec![arrow(1,Canon::Call(Box::new(Canon::member(self.expression(*promise),None)),vec![error]))]);
                }
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
