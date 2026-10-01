//! R3's single constructor-flow owner. The compatibility lint and the explicit
//! language contract read the same facts. Unknown paths are conservative; work
//! and branch-state storage are admitted before use.
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::AllocationClass::Scratch;

type State = Vec<u8>;
const NORMAL: usize = 0;
const RETURN: usize = 1;
const THROW: usize = 2;
const BREAK: usize = 3;
const CONTINUE: usize = 4;
type Flow = [Option<State>; 5];
fn flow(kind: usize, state: State) -> Flow {
    let mut result = [None, None, None, None, None];
    result[kind] = Some(state);
    result
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldInitializationFacts {
    /// Canonical own fields whose legacy initial value can be observed before
    /// assignment or on a normal constructor completion.
    pub implicit: Vec<NominalMemberId>,
    /// A use of the receiver before the required base construction.
    pub before_super: Option<Span>,
}

pub(super) fn analyze<'src>(
    class: &ClassDecl<'_, 'src>,
    view: CheckedView<'_, '_, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<FieldInitializationFacts, AllocationError> {
    let identity = view.type_binding(class.name.name).expect("checked class");
    let info = view.nominal_class(identity).expect("class schema");
    let init = class.members.iter().find_map(|member| match member {
        ClassMember::Constructor(init) => Some(init),
        _ => None,
    });
    // The last bit states whether `this` exists (derived constructors before
    // super cannot read it, even when the class has no fields).
    let mut defaults = budget.filled(Scratch, info.fields.len() + 1, 1u8)?;
    for member in class.members {
        if let ClassMember::Field(field) = member {
            defaults[info.fields[field.name.name].index] = u8::from(field.initializer.is_some());
        }
    }
    let missing = budget.filled(Scratch, info.fields.len(), false)?;
    let mut walk = Walk {
        view,
        budget,
        info,
        this: init.and_then(|init| view.identifier_symbol(init.this.id)),
        missing,
        before_super: None,
        defaults,
    };
    walk.budget
        .work(WorkKind::Analysis, info.fields.len() as u64)?;
    if let Some(init) = init {
        let explicit_super = matches!(init.body.first(), Some(Stmt::SuperCall { .. }));
        let mut state = walk.initial()?;
        // Defaults execute before the instance's own field initializers.
        for member in class.members {
            if let ClassMember::Field(field) = member {
                state[info.fields[field.name.name].index] = 0;
            }
        }
        if explicit_super {
            state.fill(0);
        }
        for param in init.params {
            if let Some(value) = &param.default {
                walk.expression(value, &mut state)?;
            }
        }
        if !explicit_super {
            state = walk.initial()?;
        }
        let result = walk.body(init.body, state)?;
        for kind in [NORMAL, RETURN] {
            if let Some(state) = &result[kind] {
                walk.observe(state, init.span)?;
            }
        }
    } else {
        let state = walk.initial()?;
        walk.observe(&state, class.span)?;
    }
    let mut implicit = walk
        .budget
        .vector(Scratch, walk.missing.iter().filter(|&&v| v).count())?;
    for member in class.members {
        if let ClassMember::Field(field) = member {
            let field = &info.fields[field.name.name];
            if walk.missing[field.index] {
                walk.budget.push(Scratch, &mut implicit, field.member)?;
            }
        }
    }
    Ok(FieldInitializationFacts {
        implicit,
        before_super: walk.before_super,
    })
}

struct Walk<'a, 'view, 'ast, 'src, 'budget> {
    view: CheckedView<'view, 'ast, 'src>,
    budget: &'a mut AllocationBudget<'budget>,
    info: &'view ClassInfo<'src>,
    this: Option<SymbolId>,
    missing: Vec<bool>,
    before_super: Option<Span>,
    defaults: State,
}
impl Walk<'_, '_, '_, '_, '_> {
    fn initial(&mut self) -> Result<State, AllocationError> {
        self.budget.copy_slice(Scratch, &self.defaults)
    }
    fn copy(&mut self, state: &State) -> Result<State, AllocationError> {
        self.budget.copy_slice(Scratch, state)
    }
    fn join(&mut self, to: &mut Option<State>, from: State) -> Result<(), AllocationError> {
        if let Some(to) = to {
            self.budget.work(WorkKind::Analysis, to.len() as u64)?;
            for (a, b) in to.iter_mut().zip(from) {
                *a &= b;
            }
        } else {
            *to = Some(from);
        }
        Ok(())
    }
    fn merge(&mut self, to: &mut Flow, from: Flow) -> Result<(), AllocationError> {
        for (slot, state) in to.iter_mut().zip(from) {
            if let Some(state) = state {
                self.join(slot, state)?;
            }
        }
        Ok(())
    }
    fn observe(&mut self, state: &State, span: Span) -> Result<(), AllocationError> {
        self.budget.work(WorkKind::Analysis, state.len() as u64)?;
        for (missing, &ready) in self.missing.iter_mut().zip(state) {
            *missing |= ready == 0;
        }
        if state.last() == Some(&0) {
            self.before_super.get_or_insert(span);
        }
        Ok(())
    }
    fn is_this(&self, expr: &Expr<'_, '_>) -> bool {
        matches!(&expr.kind,ExprKind::Ident(id) if self.this.is_some() && self.view.identifier_symbol(id.id)==self.this)
    }
    fn field(&self, target: &Expr<'_, '_>) -> Option<usize> {
        let ExprKind::Member {
            object, property, ..
        } = &target.kind
        else {
            return None;
        };
        self.is_this(object)
            .then(|| self.info.fields.get(property.name).map(|f| f.index))
            .flatten()
    }
    fn body(&mut self, body: &[Stmt<'_, '_>], state: State) -> Result<Flow, AllocationError> {
        let mut result = flow(NORMAL, state);
        for statement in body {
            let Some(state) = result[NORMAL].take() else {
                break;
            };
            let next = self.statement(statement, state)?;
            self.merge(&mut result, next)?;
        }
        Ok(result)
    }
    fn statement(
        &mut self,
        stmt: &Stmt<'_, '_>,
        mut state: State,
    ) -> Result<Flow, AllocationError> {
        self.budget.work(WorkKind::Analysis, 1)?;
        match stmt {
            Stmt::VarDecl(decl, ..) => {
                if let Some(value) = &decl.initializer {
                    self.expression(value, &mut state)?;
                }
            }
            Stmt::Expr(value, ..)
            | Stmt::ArrayDestructure { value, .. }
            | Stmt::RecordDestructure { value, .. }
            | Stmt::Yield { value, .. } => self.expression(value, &mut state)?,
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.expression(value, &mut state)?;
                }
                return Ok(flow(RETURN, state));
            }
            Stmt::Throw { value, .. } => {
                self.expression(value, &mut state)?;
                return Ok(flow(THROW, state));
            }
            Stmt::Break(..) => return Ok(flow(BREAK, state)),
            Stmt::Continue(..) => return Ok(flow(CONTINUE, state)),
            Stmt::SuperCall { args, .. } => {
                for arg in *args {
                    self.expression(&arg.expression, &mut state)?;
                }
                state = self.initial()?;
            }
            Stmt::Block { body, .. } => return self.body(body, state),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expression(condition, &mut state)?;
                let other = self.copy(&state)?;
                let mut result = self.statement(then_branch, state)?;
                let right = if let Some(branch) = else_branch {
                    self.statement(branch, other)?
                } else {
                    flow(NORMAL, other)
                };
                self.merge(&mut result, right)?;
                return Ok(result);
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expression(condition, &mut state)?;
                return self.loop_body(body, None, state);
            }
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                match initializer {
                    Some(ForInitializer::VarDecl(decl)) => {
                        if let Some(value) = &decl.initializer {
                            self.expression(value, &mut state)?;
                        }
                    }
                    Some(ForInitializer::Expr(value)) => self.expression(value, &mut state)?,
                    None => {}
                }
                if let Some(value) = condition {
                    self.expression(value, &mut state)?;
                }
                return self.loop_body(body, update.as_ref(), state);
            }
            Stmt::ForIn {
                object: iterable,
                body,
                ..
            }
            | Stmt::ForOf { iterable, body, .. } => {
                self.expression(iterable, &mut state)?;
                return self.loop_body(body, None, state);
            }
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                // A catch/finally can run after any throwing operation in the
                // try. Its guaranteed state is at least the entry state; do
                // not pretend a later assignment already happened.
                let exceptional = self.copy(&state)?;
                let mut result = self.body(body, state)?;
                self.join(&mut result[THROW], exceptional)?;
                if let Some(catch) = catch {
                    let state = result[THROW].take().expect("exception path");
                    let caught = self.body(catch.body, state)?;
                    self.merge(&mut result, caught)?;
                }
                if let Some(finally) = finally {
                    let mut composed = [None, None, None, None, None];
                    for (kind, state) in result.into_iter().enumerate() {
                        if let Some(state) = state {
                            let mut final_flow = self.body(finally, state)?;
                            if let Some(state) = final_flow[NORMAL].take() {
                                self.join(&mut composed[kind], state)?;
                            }
                            self.merge(&mut composed, final_flow)?;
                        }
                    }
                    return Ok(composed);
                }
                return Ok(result);
            }
        }
        Ok(flow(NORMAL, state))
    }
    fn loop_body(
        &mut self,
        body: &Stmt<'_, '_>,
        update: Option<&Expr<'_, '_>>,
        state: State,
    ) -> Result<Flow, AllocationError> {
        let inside = self.copy(&state)?;
        let mut result = self.statement(body, inside)?;
        let mut repeating = result[NORMAL].take();
        if let Some(continued) = result[CONTINUE].take() {
            self.join(&mut repeating, continued)?;
        }
        if let (Some(update), Some(mut state)) = (update, repeating) {
            self.expression(update, &mut state)?;
        }
        // Zero iterations and any break retain only the entry guarantees.
        result[BREAK] = None;
        result[NORMAL] = Some(state);
        Ok(result)
    }
    fn expression(
        &mut self,
        expr: &Expr<'_, '_>,
        state: &mut State,
    ) -> Result<(), AllocationError> {
        self.budget.work(WorkKind::Analysis, 1)?;
        match &expr.kind {
            ExprKind::Int(..)
            | ExprKind::Float(..)
            | ExprKind::String(..)
            | ExprKind::Bool(..)
            | ExprKind::Null(..)
            | ExprKind::DynamicImport { .. } => {}
            ExprKind::Ident(..) => {
                if self.is_this(expr) {
                    self.observe(state, expr.span())?;
                }
            }
            ExprKind::Assignment {
                op: AssignmentOp::Assign,
                target,
                value,
                ..
            } if self.field(target).is_some() => {
                if state.last() == Some(&0) {
                    self.before_super.get_or_insert(target.span());
                }
                self.expression(value, state)?;
                state[self.field(target).unwrap()] = 1;
            }
            ExprKind::Assignment {
                op: AssignmentOp::Nullish,
                target,
                value,
                ..
            } => {
                self.expression(target, state)?;
                let mut conditional = self.copy(state)?;
                self.expression(value, &mut conditional)?;
            }
            ExprKind::Assignment { target, value, .. } => {
                self.expression(target, state)?;
                self.expression(value, state)?;
            }
            ExprKind::Update { target, .. } => self.expression(target, state)?,
            ExprKind::Member { object, .. } | ExprKind::OptionalMember { object, .. } => {
                self.expression(object, state)?
            }
            ExprKind::Index { object, index, .. } => {
                self.expression(object, state)?;
                self.expression(index, state)?;
            }
            ExprKind::OptionalIndex { object, index, .. } => {
                self.expression(object, state)?;
                let mut conditional = self.copy(state)?;
                self.expression(index, &mut conditional)?;
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::DynamicUnary { expr, .. }
            | ExprKind::Await { task: expr, .. }
            | ExprKind::TypeCheck { value: expr, .. }
            | ExprKind::Cast { value: expr, .. }
            | ExprKind::Convert { value: expr, .. } => self.expression(expr, state)?,
            ExprKind::Binary { op, lhs, rhs, .. } => {
                self.expression(lhs, state)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Nullish) {
                    let mut conditional = self.copy(state)?;
                    self.expression(rhs, &mut conditional)?;
                } else {
                    self.expression(rhs, state)?;
                }
            }
            ExprKind::DynamicBinary { lhs, rhs, .. } => {
                self.expression(lhs, state)?;
                self.expression(rhs, state)?;
            }
            ExprKind::If {
                condition,
                then_value,
                else_value,
                ..
            } => {
                self.expression(condition, state)?;
                let mut other = self.copy(state)?;
                self.expression(then_value, state)?;
                self.expression(else_value, &mut other)?;
                for (a, b) in state.iter_mut().zip(other) {
                    *a &= b;
                }
            }
            ExprKind::Match { value, arms, .. } => {
                self.expression(value, state)?;
                let mut joined = None;
                for arm in *arms {
                    let mut branch = self.copy(state)?;
                    self.expression(&arm.value, &mut branch)?;
                    self.join(&mut joined, branch)?;
                }
                if let Some(joined) = joined {
                    *state = joined;
                }
            }
            ExprKind::Call { callee, args, .. } | ExprKind::Construct { callee, args, .. } => {
                self.expression(callee, state)?;
                let conditional = matches!(
                    &callee.kind,
                    ExprKind::OptionalMember { .. } | ExprKind::OptionalIndex { .. }
                );
                let short = matches!(
                    self.view.builtin_call(expr.id),
                    Some(BuiltinCall::JsAnd | BuiltinCall::JsOr)
                );
                for (index, arg) in args.iter().enumerate() {
                    if conditional || (short && index > 0) {
                        let mut branch = self.copy(state)?;
                        self.expression(&arg.expression, &mut branch)?;
                    } else {
                        self.expression(&arg.expression, state)?;
                    }
                }
            }
            ExprKind::New { args, .. } => {
                for arg in *args {
                    self.expression(&arg.expression, state)?;
                }
            }
            ExprKind::ArrayLiteral { elements, .. } => {
                for item in *elements {
                    let (ArrayElement::Value(value) | ArrayElement::Spread { value, .. }) = item;
                    self.expression(value, state)?;
                }
            }
            ExprKind::RecordLiteral { entries, .. } | ExprKind::ObjectLiteral { entries, .. } => {
                for entry in *entries {
                    self.expression(entry.value(), state)?;
                }
            }
            ExprKind::StructLiteral { values, .. } => {
                for value in *values {
                    self.expression(value, state)?;
                }
            }
            ExprKind::With { value, fields, .. } => {
                self.expression(value, state)?;
                for field in *fields {
                    self.expression(&field.value, state)?;
                }
            }
            ExprKind::Template { parts, .. } => {
                for part in *parts {
                    if let TemplatePart::Expr(value) = part {
                        self.expression(value, state)?;
                    }
                }
            }
            ExprKind::ArrowFunction { .. } => {
                // Capturing an incompletely initialized receiver is a read of
                // `this`. The closure's writes never initialize the caller.
                let mut captured = false;
                let view = self.view;
                let this = self.this;
                let mut work = Ok(());
                crate::ast_walk::expression(expr, &mut |node| {
                    if work.is_ok() {
                        work = self.budget.work(WorkKind::Analysis, 1);
                    }
                    if let ExprKind::Ident(id) = &node.kind {
                        captured |= this.is_some() && view.identifier_symbol(id.id) == this;
                    }
                });
                work?;
                if captured {
                    self.observe(state, expr.span())?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn missing(body: &str) -> Vec<String> {
        let arena = bumpalo::Bump::new();
        let source = format!("class C{{int x;int y;init(bool b){{{body}}}}}");
        let syntax = crate::parse_source(&arena, &source).unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let view = checked.view();
        let info = view.nominal_class(view.type_binding("C").unwrap()).unwrap();
        info.initialization
            .implicit
            .iter()
            .map(|&id| match view.nominal_member(id).unwrap() {
                NominalMember::Field { field, .. } => field.name.to_string(),
                _ => unreachable!(),
            })
            .collect()
    }
    #[test]
    fn s4_field_flow_handles_early_completion_and_observation() {
        for (body, expected) in [
            ("this.x=1;this.y=2;", vec![]),
            ("if(b){return;}this.x=1;this.y=2;", vec!["x", "y"]),
            ("if(b){this.x=1;}else{this.x=2;}this.y=3;", vec![]),
            ("if(b){this.x=1;return;}this.x=2;this.y=3;", vec!["y"]),
            ("this.x=1;print(this.x);this.y=2;", vec!["y"]),
            ("this.x=this.y;this.y=2;", vec!["x", "y"]),
            ("auto f=()=>this.x;this.x=1;this.y=2;", vec!["x", "y"]),
            ("while(b){this.x=1;}this.y=2;", vec!["x"]),
            ("b && ((this.x=1)==1);this.y=2;", vec!["x"]),
            ("throw 1;", vec![]),
            ("try{return;}finally{this.x=1;this.y=2;}", vec![]),
            ("try{this.x=1;this.y=2;}catch{this.x=3;}", vec!["y"]),
            ("try{if(b){return;}this.x=1;}finally{this.y=2;}", vec!["x"]),
        ] {
            assert_eq!(missing(body), expected, "{body}");
        }
    }
}
