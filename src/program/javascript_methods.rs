//! A method is its own function (architecture §10.2, law P1; plan M8.2 A1).
//!
//! `JS.methodN(f)`, `JS.methodRest(f)` and `JS.staticRest(f)` hand a callback
//! a receiver and an argument list. When `f` is private to the adapter (a
//! lambda whose one use is the adapter's argument, or a function or lambda in
//! a cell whose one read is), the adapter's result *is* `f`, formed as an
//! ordinary function:
//!
//! * the receiver parameter reads `this`, or, when a nested function with a
//!   `this` of its own reads it or the program assigns it, an alias
//!   `let r=this` at entry; an unread receiver is dropped;
//! * a rest list read only at constant indices becomes named formals, whose
//!   `length` stays declared as the adapter's 0 (the first formal prints
//!   `p=void 0`); any other rest list reads `arguments` (or
//!   `let l=arguments` where a nested function would see its own);
//! * the remaining `N` parameters keep the adapter's `length`, it stays
//!   constructible, and it is an arrow only where nothing can construct it or
//!   read its `prototype`, as before;
//! * its name is the adapter result's: the empty name where the contract
//!   observes names, and unobserved otherwise (a method may then gain the
//!   name JavaScript infers, which R6 makes unobservable; a case pins it).
//!
//! No shared adapter factory is emitted for such a callback, so no function
//! body serves two callbacks (law P1: a shared factory's one inner call site
//! can see unrelated receiver types). Legality: the callback does not
//! suspend, reads no `this` or `arguments` of an enclosing function unless it
//! is an arrow (a function of its own would see its own; an arrow sees the
//! enclosing ones, as the lambda did), and carries no struct across a
//! boundary (its D2 wrapper passes the receiver as an argument, and a classic
//! script prints its frame strict where the adapter's is sloppy).
//!
//! A published class's prototype method whose static body nothing else calls
//! is that body, formed inside the class with its receiver as `this`
//! (diagnosis C1), under the same legality (no suspension, no ambient `this`
//! or `arguments`). A class body is strict, so under `execution = "script"`
//! the body must also be insensitive to strictness: no write that a sloppy
//! frame would ignore and a strict one throws for (host member, index and
//! global writes, `JS.set`, `JS.delete`).
//!
//! Prior art: Closure keeps `FunctionRewriter` off by default because its
//! shared helpers cost engines their fast paths (`FunctionRewriter.java:42-44`);
//! this is the inverse of its `DevirtualizeMethods` (`:39-56`, receiver as
//! first argument), under the mirror of its preconditions (no `arguments` of
//! the wrong function); Terser's `arguments` option turns `arguments[k]` into
//! formals (`lib/compress/index.js:3613-3651`); the old route printed these
//! callbacks as methods (b80's fused spelling).
use super::*;

/// How a method spells its receiver parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Receiver {
    /// Every read is `this`.
    This,
    /// `let r=this` at entry: a nested function with its own `this` reads the
    /// receiver, or the program assigns it.
    Alias,
    /// Nothing reads it.
    Unread,
}

/// How a rest method spells its argument list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum List {
    /// Every read is `arguments`.
    Arguments,
    /// `let l=arguments` at entry.
    Alias,
    /// Read only as `l[k]` for constant `k`: this many named formals.
    Formals(usize),
    /// Nothing reads it.
    Unread,
}

/// A unit formed as a JavaScript method.
#[derive(Debug, Clone, Copy)]
pub(super) struct MethodForm {
    /// The adapter call whose result this function is.
    pub(super) adapter: Option<(UnitId, CallId)>,
    pub(super) receiver: Option<(CellId, Receiver)>,
    pub(super) list: Option<(CellId, List)>,
    pub(super) arrow: bool,
    /// The empty name, where the contract observes the adapter result's.
    pub(super) exact_empty_name: bool,
}

/// The `JS.methodN`/`methodRest`/`staticRest` convention: forwarded
/// parameter count, whether `this` is passed, whether `arguments` is.
fn adapter_convention(builtin: BuiltinCall) -> Option<(usize, bool, bool)> {
    use BuiltinCall as B;
    Some(match builtin {
        B::JsMethod0 => (0, true, false),
        B::JsMethod1 => (1, true, false),
        B::JsMethod2 => (2, true, false),
        B::JsMethod3 => (3, true, false),
        B::JsMethod4 => (4, true, false),
        B::JsMethod5 => (5, true, false),
        B::JsMethod6 => (6, true, false),
        B::JsMethod7 => (7, true, false),
        B::JsMethod8 => (8, true, false),
        B::JsMethod9 => (9, true, false),
        B::JsMethod10 => (10, true, false),
        B::JsMethodRest => (0, true, true),
        B::JsStaticRest => (0, false, true),
        _ => return None,
    })
}

/// A use that only invokes the value: a call of it, or `JS.call`/`JS.apply`
/// of it (neither constructs it or reads its properties).
fn invoking_use(data: &UnitData, usage: &ValueUse) -> bool {
    match *usage {
        ValueUse::CallCallee { .. } => true,
        ValueUse::CallArgument {
            call, position: 0, ..
        } => matches!(
            data.calls[call.index()].target,
            CallTarget::Builtin(BuiltinCall::JsCall | BuiltinCall::JsApply)
        ),
        _ => false,
    }
}

impl Formation<'_, '_, '_, '_, '_> {
    /// The unit that creates `unit` with a closure operation, when exactly
    /// one does.
    fn creator(&self, unit: UnitId) -> Option<UnitId> {
        let sites = self.uses?.creators(unit)?.sites();
        match sites {
            [site] => Some(site.unit),
            _ => None,
        }
    }

    /// Whether `inner` is `outer` or created inside it through closures
    /// that are arrows (lambdas that neither suspend as generators nor
    /// construct a class): `this` and `arguments` there are `outer`'s.
    fn arrow_nested(&mut self, inner: UnitId, outer: UnitId) -> Result<bool, FormationError> {
        let mut unit = inner;
        loop {
            self.work(1)?;
            if unit == outer {
                return Ok(true);
            }
            let Some(data) = self.program.unit(unit) else {
                return Ok(false);
            };
            if !ambient::inherits(self.program, data)
                || data.suspension == Suspension::Generator
                || data.constructor_of.is_some()
            {
                return Ok(false);
            }
            match self.creator(unit) {
                Some(parent) => unit = parent,
                None => return Ok(false),
            }
        }
    }

    /// Whether `inner` is `outer` or created inside it at any depth.
    fn nested_in(&mut self, inner: UnitId, outer: UnitId) -> Result<bool, FormationError> {
        let mut unit = inner;
        loop {
            self.work(1)?;
            if unit == outer {
                return Ok(true);
            }
            match self.creator(unit) {
                Some(parent) => unit = parent,
                None => return Ok(false),
            }
        }
    }

    /// Whether `unit`, or a closure created inside it that inherits its
    /// activation, reads the ambient `this` or `arguments`.
    fn reads_ambient_activation(&mut self, unit: UnitId) -> Result<bool, FormationError> {
        let mut pending = vec![unit];
        while let Some(unit) = pending.pop() {
            let Some(data) = self.program.unit(unit) else {
                return Ok(true);
            };
            for operation in &data.operations {
                self.work(1)?;
                match operation.kind {
                    OperationKind::Load(place) => {
                        if let Place::Cell(cell) = data.places[place.index()] {
                            if ambient::classify(&self.program.cells[cell.index()]).is_some() {
                                return Ok(true);
                            }
                        }
                    }
                    OperationKind::Closure(child) => {
                        if self.program.unit(child).is_some_and(|child| {
                            ambient::inherits(self.program, child)
                                && child.suspension != Suspension::Generator
                        }) {
                            pending.push(child);
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    /// Whether `cell`, a parameter of `unit`, can be spelled `this` (or
    /// `arguments`) in `unit`'s own function: nothing assigns it, and every
    /// unit reading it is `unit` or an arrow inside it. `None` when some use
    /// rules out any spelling but the parameter itself (a reference or an
    /// export); `Some(false)` when an alias is needed.
    pub(super) fn activation_spelling(
        &mut self,
        unit: UnitId,
        cell: CellId,
    ) -> Result<Option<bool>, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(None);
        };
        if references::is_reference(self.program, cell) {
            return Ok(None);
        }
        let Some(users) = uses.cell(cell) else {
            return Ok(None);
        };
        let mut direct = true;
        for site in users.sites() {
            self.work(1)?;
            match *site {
                CellUseSite::Unit {
                    unit: owner,
                    usage: CellUse::Parameter(_),
                } if owner == unit => {}
                CellUseSite::Unit {
                    unit: reader,
                    usage: CellUse::Read { .. } | CellUse::Capture,
                } => {
                    if !self.nested_in(reader, unit)? {
                        return Ok(None);
                    }
                    if !self.arrow_nested(reader, unit)? {
                        direct = false;
                    }
                }
                CellUseSite::Unit {
                    unit: writer,
                    usage: CellUse::Write { .. },
                } => {
                    if !self.nested_in(writer, unit)? {
                        return Ok(None);
                    }
                    direct = false;
                }
                _ => return Ok(None),
            }
        }
        Ok(Some(direct))
    }

    /// Whether any unit reads `cell` (a read site, not a capture).
    fn cell_read(&mut self, cell: CellId) -> Result<bool, FormationError> {
        let Some(users) = self.uses.and_then(|uses| uses.cell(cell)) else {
            return Ok(true);
        };
        for site in users.sites() {
            self.work(1)?;
            if matches!(
                site,
                CellUseSite::Unit {
                    usage: CellUse::Read { .. } | CellUse::Write { .. } | CellUse::Capture,
                    ..
                }
            ) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The number of formals a rest list read only as `list[k]`, for
    /// constant non-negative integers `k`, needs: the largest `k` plus one.
    /// `None` when anything else reads or writes it.
    fn list_formals(
        &mut self,
        unit: UnitId,
        list: CellId,
    ) -> Result<Option<usize>, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(None);
        };
        let Some(users) = uses.cell(list) else {
            return Ok(None);
        };
        let mut count = 0usize;
        for site in users.sites() {
            self.work(1)?;
            match *site {
                CellUseSite::Unit {
                    unit: owner,
                    usage: CellUse::Parameter(_),
                } if owner == unit => {}
                CellUseSite::Unit {
                    unit: reader,
                    usage: CellUse::Capture,
                } if self.nested_in(reader, unit)? => {}
                CellUseSite::Unit {
                    unit: reader,
                    usage: CellUse::Read { operation, place },
                } => {
                    if !self.nested_in(reader, unit)? {
                        return Ok(None);
                    }
                    let data = self.program.units[reader.index()].data();
                    if !matches!(data.places[place.index()], Place::Cell(found) if found == list)
                        || !matches!(
                            data.operations[operation.index()].kind,
                            OperationKind::Load(_)
                        )
                    {
                        return Ok(None);
                    }
                    let Some(loaded) = data.operations[operation.index()].result else {
                        return Ok(None);
                    };
                    let Some(readers) = uses.unit(reader).and_then(|uses| uses.value_uses(loaded))
                    else {
                        return Ok(None);
                    };
                    for usage in readers {
                        self.work(1)?;
                        let ValueUse::PlaceReceiver { operation, place } = *usage else {
                            return Ok(None);
                        };
                        if !matches!(data.operations[operation.index()].kind,
                            OperationKind::Load(read) if read == place)
                        {
                            return Ok(None);
                        }
                        let Some(index) = self.constant_index(data, place, loaded) else {
                            return Ok(None);
                        };
                        count = count.max(index + 1);
                    }
                }
                _ => return Ok(None),
            }
        }
        Ok(Some(count))
    }

    /// `k` when `place` is `receiver[k]` for a constant non-negative
    /// integer `k` (an array index a formal can stand for).
    fn constant_index(&self, data: &UnitData, place: PlaceId, receiver: ValueId) -> Option<usize> {
        let Place::Index {
            receiver: found,
            key,
        } = data.places[place.index()]
        else {
            return None;
        };
        if found != receiver {
            return None;
        }
        let index = match data.operations[data.values[key.index()].definition.index()].kind {
            OperationKind::Constant(Constant::Integer(value)) => i64::from(value),
            OperationKind::Constant(Constant::Number(bits)) => {
                let value = f64::from_bits(bits);
                if value.fract() != 0.0 || !(0.0..=4_294_967_294.0).contains(&value) {
                    return None;
                }
                // `-0` reads index 0 too, as `list[0]`.
                value as i64
            }
            _ => return None,
        };
        // A formal list stays one `function` head: JavaScript's own limit
        // on parameters is far above anything a program reads by index.
        (0..=u16::MAX as i64)
            .contains(&index)
            .then_some(index as usize)
    }

    /// Whether the unit's signature carries a struct across a boundary. Its
    /// value then reaches host code through a D2 wrapper that passes the
    /// receiver as an ordinary argument (and, in a classic script, its frame
    /// is printed strict), so it keeps its parameter list.
    fn struct_boundary_unit(&self, unit: UnitId) -> bool {
        !self.struct_plan.boundary_types.is_empty()
            && self
                .program
                .unit(unit)
                .and_then(|data| data.callable_type)
                .is_some_and(|ty| self.struct_plan.boundary_types[ty.index()])
    }

    /// Whether every use of `value` only invokes it (through the local cells
    /// it initializes): a call, or `JS.call`/`JS.apply` of it, so nothing
    /// can construct it or read its `prototype`; or the contract assumes
    /// callers never do and the program does not through a cell it stores
    /// it in.
    fn arrow_legal(&mut self, unit: UnitId, value: ValueId) -> Result<bool, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(false);
        };
        let Some(readers) = uses.unit(unit).and_then(|uses| uses.value_uses(value)) else {
            return Ok(false);
        };
        let data = self.program.units[unit.index()].data();
        let mut called = !readers.is_empty();
        let mut constructed = false;
        for reader in readers {
            self.work(1)?;
            match *reader {
                usage if invoking_use(data, &usage) => {}
                ValueUse::Operand {
                    operation,
                    position: 0,
                } if matches!(data.operations[operation.index()].kind,
                    OperationKind::Initialize(cell)
                        if self.program.cells[cell.index()].binding == CellBinding::Local) =>
                {
                    let OperationKind::Initialize(cell) = data.operations[operation.index()].kind
                    else {
                        unreachable!("matched above")
                    };
                    if !self.invoked_cell(cell)? {
                        called = false;
                        constructed |= self.cell_constructed(cell)?;
                    }
                }
                ValueUse::CallArgument {
                    call, position: 0, ..
                } if matches!(
                    data.calls[call.index()].target,
                    CallTarget::Builtin(BuiltinCall::JsConstruct)
                ) =>
                {
                    called = false;
                    constructed = true;
                }
                _ => called = false,
            }
        }
        Ok(called || self.contract.assumptions.unconstructed_callbacks && !constructed)
    }

    /// Whether local `cell` is initialized once, never written, exported or
    /// read through a module namespace, and every read only invokes it.
    fn invoked_cell(&mut self, cell: CellId) -> Result<bool, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(false);
        };
        if self.namespace_member(cell)? {
            return Ok(false);
        }
        let Some(users) = uses.cell(cell) else {
            return Ok(false);
        };
        for site in users.sites() {
            self.work(1)?;
            match *site {
                CellUseSite::Unit {
                    usage: CellUse::Initialize(_) | CellUse::Capture,
                    ..
                } => {}
                CellUseSite::Unit {
                    unit: reader,
                    usage: CellUse::Read { operation, place },
                } => {
                    let data = self.program.units[reader.index()].data();
                    if !matches!(data.places[place.index()], Place::Cell(found) if found == cell)
                        || !matches!(
                            data.operations[operation.index()].kind,
                            OperationKind::Load(_)
                        )
                    {
                        return Ok(false);
                    }
                    let Some(loaded) = data.operations[operation.index()].result else {
                        return Ok(false);
                    };
                    let Some(readers) = uses.unit(reader).and_then(|uses| uses.value_uses(loaded))
                    else {
                        return Ok(false);
                    };
                    self.work(readers.len())?;
                    if !readers.iter().all(|usage| invoking_use(data, usage)) {
                        return Ok(false);
                    }
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    /// Whether the program constructs a value read from local `cell`, or
    /// reads a property of it (its `prototype` among them).
    fn cell_constructed(&mut self, cell: CellId) -> Result<bool, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(true);
        };
        let Some(users) = uses.cell(cell) else {
            return Ok(true);
        };
        for site in users.sites() {
            self.work(1)?;
            let CellUseSite::Unit {
                unit: reader,
                usage: CellUse::Read { operation, .. },
            } = *site
            else {
                continue;
            };
            let data = self.program.units[reader.index()].data();
            let Some(loaded) = data.operations[operation.index()].result else {
                continue;
            };
            let Some(readers) = uses.unit(reader).and_then(|uses| uses.value_uses(loaded)) else {
                return Ok(true);
            };
            for usage in readers {
                self.work(1)?;
                match *usage {
                    ValueUse::PlaceReceiver { .. } => return Ok(true),
                    ValueUse::CallArgument {
                        call, position: 0, ..
                    } if matches!(
                        data.calls[call.index()].target,
                        CallTarget::Builtin(
                            BuiltinCall::JsConstruct
                                | BuiltinCall::JsGet
                                | BuiltinCall::JsSet
                                | BuiltinCall::JsInvoke
                                | BuiltinCall::JsHas
                                | BuiltinCall::JsDelete
                        )
                    ) =>
                    {
                        return Ok(true)
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    /// The adapter call whose argument is `value`, created in `unit`: its
    /// only use, or the only read of a cell `value` initializes and nothing
    /// else touches, read in the same region, so each creation meets one
    /// adapter evaluation (every evaluation of an adapter makes a fresh
    /// function, and so does each creation).
    fn adapter_of(
        &mut self,
        unit: UnitId,
        value: ValueId,
    ) -> Result<Option<(UnitId, CallId, BuiltinCall)>, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(None);
        };
        let data = self.program.units[unit.index()].data();
        let direct = |data: &UnitData, usage: &ValueUse| match *usage {
            ValueUse::CallArgument {
                call, position: 0, ..
            } => match data.calls[call.index()].target {
                CallTarget::Builtin(builtin) if adapter_convention(builtin).is_some() => {
                    Some((call, builtin))
                }
                _ => None,
            },
            _ => None,
        };
        let Some(&[usage]) = uses.unit(unit).and_then(|uses| uses.value_uses(value)) else {
            return Ok(None);
        };
        if let Some((call, builtin)) = direct(data, &usage) {
            return Ok(Some((unit, call, builtin)));
        }
        let ValueUse::Operand {
            operation: initialize,
            position: 0,
        } = usage
        else {
            return Ok(None);
        };
        let OperationKind::Initialize(cell) = data.operations[initialize.index()].kind else {
            return Ok(None);
        };
        if !matches!(
            self.program.cells[cell.index()].binding,
            CellBinding::Local | CellBinding::Function(_)
        ) || self.namespace_member(cell)?
            || self
                .program
                .value_exports()
                .any(|(_, exported)| exported == cell)
        {
            return Ok(None);
        }
        let Some(users) = uses.cell(cell) else {
            return Ok(None);
        };
        let mut read = None;
        for site in users.sites() {
            self.work(1)?;
            match *site {
                CellUseSite::Unit {
                    unit: owner,
                    usage: CellUse::Initialize(found),
                } if owner == unit && found == initialize => {}
                CellUseSite::Unit {
                    unit: reader,
                    usage: CellUse::Read { operation, place },
                } if reader == unit && read.is_none() => {
                    if !matches!(data.places[place.index()], Place::Cell(found) if found == cell)
                        || !matches!(
                            data.operations[operation.index()].kind,
                            OperationKind::Load(_)
                        )
                        || data.operations[operation.index()].region
                            != data.operations[initialize.index()].region
                    {
                        return Ok(None);
                    }
                    read = Some(operation);
                }
                _ => return Ok(None),
            }
        }
        let Some(read) = read else {
            return Ok(None);
        };
        let Some(loaded) = data.operations[read.index()].result else {
            return Ok(None);
        };
        let Some(&[usage]) = uses.unit(unit).and_then(|uses| uses.value_uses(loaded)) else {
            return Ok(None);
        };
        Ok(direct(data, &usage).map(|(call, builtin)| (unit, call, builtin)))
    }

    /// The method form of the closure `operation` creates in `context`, when
    /// its value is an adapter's private callback (module comment).
    pub(super) fn adapter_method_form(
        &mut self,
        context: ContextId,
        operation: OpId,
    ) -> Result<Option<MethodForm>, FormationError> {
        let unit = self.semantic(context);
        let data = self.data(context);
        let OperationKind::Closure(child) = data.operations[operation.index()].kind else {
            return Ok(None);
        };
        let Some(value) = data.operations[operation.index()].result else {
            return Ok(None);
        };
        let Some((owner, call, builtin)) = self.adapter_of(unit, value)? else {
            return Ok(None);
        };
        let (count, receiver, rest) = adapter_convention(builtin).expect("an adapter");
        let Some(body) = self.program.unit(child) else {
            return Ok(None);
        };
        if body.parameters.len() != usize::from(receiver) + count + usize::from(rest)
            || body.suspension != Suspension::None
            || body.constructor_of.is_some()
            || self.struct_plan.wrapped(child)
            || self.struct_boundary_unit(child)
            || self.encoded(context, value)?
        {
            return Ok(None);
        }
        let parameters = body.parameters.clone();
        // An enclosing method's receiver or list read inside this callback
        // is that method's `this` or `arguments`, as is an ambient one: an
        // arrow sees them, and a function of its own would shadow them.
        let mut lexical = false;
        'cells: for index in 0..self.activation_cells.len() {
            self.work(1)?;
            let cell = self.activation_cells[index];
            let Some(users) = self.uses.and_then(|uses| uses.cell(cell)) else {
                continue;
            };
            for site in users.sites() {
                if let CellUseSite::Unit {
                    unit: reader,
                    usage: CellUse::Read { .. } | CellUse::Capture,
                } = *site
                {
                    if self.nested_in(reader, child)? {
                        lexical = true;
                        break 'cells;
                    }
                }
            }
        }
        let lexical = lexical || self.reads_ambient_activation(child)?;
        let receiver = match receiver {
            false => None,
            true => {
                let cell = parameters[0];
                let spelling = if !self.cell_read(cell)? {
                    Receiver::Unread
                } else {
                    match self.activation_spelling(child, cell)? {
                        Some(true) => Receiver::This,
                        Some(false) => Receiver::Alias,
                        None => return Ok(None),
                    }
                };
                Some((cell, spelling))
            }
        };
        let list = match rest {
            false => None,
            true => {
                let cell = *parameters.last().expect("counted above");
                let spelling = if !self.cell_read(cell)? {
                    List::Unread
                } else if let Some(count) = self.list_formals(child, cell)? {
                    List::Formals(count)
                } else {
                    match self.activation_spelling(child, cell)? {
                        Some(true) => List::Arguments,
                        Some(false) => List::Alias,
                        None => return Ok(None),
                    }
                };
                Some((cell, spelling))
            }
        };
        // The adapter's result: where it goes decides its name and whether
        // an arrow can stand for it. The adapter call is in this same unit.
        debug_assert_eq!(owner, unit);
        let result = self
            .uses
            .and_then(|uses| uses.unit(owner))
            .and_then(|uses| uses.call_operation(call))
            .and_then(|operation| {
                self.program.units[owner.index()].data().operations[operation.index()].result
            });
        let own_activation = matches!(receiver, Some((_, Receiver::This | Receiver::Alias)))
            || matches!(list, Some((_, List::Arguments | List::Alias)));
        let arrow = match result {
            Some(result) if self.compact && !own_activation => self.arrow_legal(owner, result)?,
            _ => false,
        };
        if lexical && !arrow {
            return Ok(None);
        }
        let observed = match result {
            None => false,
            Some(result) => {
                let unobserved = self.compact && self.unobserved_closure_name(context, result)?;
                let internal = self.compact
                    && !self.contract.abi.keep_function_names
                    && !(self.contract.abi.keep_published_function_names
                        && self.flows_into_exported_cell(context, result)?);
                !(unobserved || internal)
            }
        };
        Ok(Some(MethodForm {
            adapter: Some((owner, call)),
            receiver,
            list,
            arrow,
            exact_empty_name: observed,
        }))
    }

    /// Whether `unit` is a static method body that only its class's
    /// prototype reaches: a kept class publishes it, nothing reads its cell,
    /// and under a classic script its body cannot tell the class body's
    /// strictness. Its receiver's spelling then.
    pub(super) fn prototype_method_form(
        &mut self,
        unit: UnitId,
    ) -> Result<Option<MethodForm>, FormationError> {
        if let Some(&(_, form)) = self
            .prototype_forms
            .iter()
            .find(|(known, _)| *known == unit)
        {
            return Ok(form);
        }
        let form = self.compute_prototype_method_form(unit)?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.prototype_forms,
            (unit, form),
        )?;
        Ok(form)
    }

    fn compute_prototype_method_form(
        &mut self,
        unit: UnitId,
    ) -> Result<Option<MethodForm>, FormationError> {
        let Some(body) = self.program.unit(unit) else {
            return Ok(None);
        };
        if body.kind != UnitKind::Function
            || body.constructor_of.is_some()
            || body.suspension != Suspension::None
            || body.parameters.is_empty()
            || self.struct_plan.wrapped(unit)
            || self.struct_boundary_unit(unit)
        {
            return Ok(None);
        }
        let program = self.program;
        let Some(cell) = program.classes.iter().find_map(|class| {
            class
                .prototype
                .iter()
                .find(|(_, cell)| {
                    program.cells[cell.index()].binding == CellBinding::Function(unit)
                })
                .map(|&(_, cell)| cell)
        }) else {
            return Ok(None);
        };
        self.work(program.classes.len())?;
        if self.namespace_member(cell)?
            || program
                .value_exports()
                .any(|(_, exported)| exported == cell)
        {
            return Ok(None);
        }
        let Some(users) = self.uses.and_then(|uses| uses.cell(cell)) else {
            return Ok(None);
        };
        for site in users.sites() {
            self.work(1)?;
            if !matches!(
                site,
                CellUseSite::Unit {
                    usage: CellUse::Initialize(_),
                    ..
                }
            ) {
                return Ok(None);
            }
        }
        if self.reads_ambient_activation(unit)? {
            return Ok(None);
        }
        if !self.contract.execution.guarantees_strict_execution() && self.writes_fallibly(unit)? {
            return Ok(None);
        }
        let receiver = body.parameters[0];
        let spelling = if !self.cell_read(receiver)? {
            Receiver::Unread
        } else {
            match self.activation_spelling(unit, receiver)? {
                Some(true) => Receiver::This,
                Some(false) => Receiver::Alias,
                None => return Ok(None),
            }
        };
        Ok(Some(MethodForm {
            adapter: None,
            receiver: Some((receiver, spelling)),
            list: None,
            arrow: false,
            exact_empty_name: false,
        }))
    }

    /// Whether `unit` or a unit created inside it makes a write a sloppy
    /// frame ignores when it fails and a strict one throws for: a host
    /// member, index or global write, `JS.set` or `JS.delete`.
    fn writes_fallibly(&mut self, unit: UnitId) -> Result<bool, FormationError> {
        let mut pending = vec![unit];
        while let Some(unit) = pending.pop() {
            let Some(data) = self.program.unit(unit) else {
                return Ok(true);
            };
            for operation in &data.operations {
                self.work(1)?;
                match operation.kind {
                    OperationKind::Store(place) => {
                        let mut root = place;
                        while let Place::Field { base, .. } = data.places[root.index()] {
                            self.work(1)?;
                            root = base;
                        }
                        let fallible = match data.places[root.index()] {
                            Place::Cell(cell) => {
                                self.program.cells[cell.index()].binding == CellBinding::Foreign
                            }
                            Place::ClassField { field, .. } => self
                                .program
                                .class(field.nominal)
                                .is_none_or(|class| class.external),
                            Place::Member { .. } | Place::Index { .. } | Place::Value(_) => true,
                            Place::Field { .. } => unreachable!("walked to its root"),
                        };
                        if fallible {
                            return Ok(true);
                        }
                    }
                    OperationKind::Call(call) => {
                        if matches!(
                            data.calls[call.index()].target,
                            CallTarget::Builtin(BuiltinCall::JsSet | BuiltinCall::JsDelete)
                        ) {
                            return Ok(true);
                        }
                    }
                    OperationKind::Closure(child) => pending.push(child),
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    /// A source receiver lambda is a typed function with its own activation,
    /// regardless of whether it escapes. Its receiver is not a printed formal.
    pub(super) fn declared_method_form(
        &mut self,
        unit: UnitId,
    ) -> Result<Option<MethodForm>, FormationError> {
        let Some(data) = self.program.unit(unit) else {
            return Ok(None);
        };
        let receiver=data.callable_type.is_some_and(|ty| matches!(&self.program.types[ty.index()],Type::Function(signature) if signature.has_receiver()));
        if !receiver {
            return Ok(None);
        }
        let cell = data.parameters[0];
        let spelling = match self.activation_spelling(unit, cell)? {
            Some(true) => Receiver::This,
            _ => Receiver::Alias,
        };
        Ok(Some(MethodForm {
            adapter: None,
            receiver: Some((cell, spelling)),
            list: None,
            arrow: false,
            exact_empty_name: false,
        }))
    }

    /// Prepare the body of `form`'s function before its context is planned:
    /// register its receiver and list spellings, and create the formals in
    /// `body`'s scope. Returns the formals. `method_aliases` follows the
    /// planning.
    pub(super) fn begin_method(
        &mut self,
        body: js::RegionId,
        form: &MethodForm,
    ) -> Result<Vec<js::BindingId>, FormationError> {
        let mut formals = Vec::new();
        if let Some((cell, spelling)) = form.receiver {
            if spelling == Receiver::This {
                self.budget
                    .push(AllocationClass::Scratch, &mut self.activation_cells, cell)?;
                self.budget
                    .push(AllocationClass::Scratch, &mut self.this_cells, cell)?;
            }
            if spelling != Receiver::Alias {
                self.budget
                    .push(AllocationClass::Scratch, &mut self.unbound_cells, cell)?;
            }
        }
        if let Some((cell, spelling)) = form.list {
            if spelling != List::Alias {
                self.budget
                    .push(AllocationClass::Scratch, &mut self.unbound_cells, cell)?;
            }
        }
        if let Some((cell, spelling)) = form.list {
            match spelling {
                List::Arguments => {
                    self.budget
                        .push(AllocationClass::Scratch, &mut self.activation_cells, cell)?;
                    self.budget
                        .push(AllocationClass::Scratch, &mut self.arguments_cells, cell)?;
                }
                List::Formals(count) => {
                    let scope = self.module.regions[body.index()].scope;
                    formals = self.budget.vector(AllocationClass::Retained, count)?;
                    for _ in 0..count {
                        self.work(1)?;
                        let formal = self.fresh_binding(scope, "argument")?;
                        self.append(&mut formals, formal)?;
                    }
                    let mut copy = self.budget.vector(AllocationClass::Scratch, count)?;
                    for &formal in &formals {
                        self.append(&mut copy, formal)?;
                    }
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut self.formal_lists,
                        (cell, copy),
                    )?;
                }
                List::Alias | List::Unread => {}
            }
        }
        if let Some(adapter) = form.adapter {
            self.budget
                .push(AllocationClass::Scratch, &mut self.method_calls, adapter)?;
        }
        Ok(formals)
    }

    /// Declare the aliases of `form`'s receiver (`let r=this`) and list
    /// (`let l=arguments`) at the start of its planned body.
    pub(super) fn method_aliases(
        &mut self,
        context: ContextId,
        body: js::RegionId,
        form: &MethodForm,
    ) -> Result<(), FormationError> {
        if let Some((cell, Receiver::Alias)) = form.receiver {
            let binding = self.cell_binding(context, cell)?;
            let this = self.expression(js::Expr::This)?;
            self.statement(
                body,
                js::Statement::Let {
                    binding,
                    value: Some(this),
                },
            )?;
        }
        if let Some((cell, List::Alias)) = form.list {
            let binding = self.cell_binding(context, cell)?;
            let arguments = self.text("arguments")?;
            let arguments = self.expression(js::Expr::Host(js::Host::new(arguments)))?;
            self.statement(
                body,
                js::Statement::Let {
                    binding,
                    value: Some(arguments),
                },
            )?;
        }
        Ok(())
    }

    /// The target function's parameters for `form`: the unit's physical
    /// ones less the receiver (first) and the list (last), with the formals
    /// in the list's place. A receiver and a list are one binding each: a
    /// class instance and a `JsValue` are never expanded products, and a
    /// reference receiver or list has no method form. An unbound one is
    /// never a parameter; an aliased one is dropped here.
    pub(super) fn method_parameters(
        &mut self,
        context: ContextId,
        form: &MethodForm,
        formals: Vec<js::BindingId>,
    ) -> Result<Vec<js::BindingId>, FormationError> {
        let mut parameters = self.physical_parameters(context)?;
        if matches!(form.list, Some((_, List::Alias))) {
            parameters.pop();
        }
        if matches!(form.receiver, Some((_, Receiver::Alias))) {
            parameters.remove(0);
        }
        for formal in formals {
            self.append(&mut parameters, formal)?;
        }
        Ok(parameters)
    }

    /// The spelling of a read of `cell` that a method form owns: `this`,
    /// `arguments`, or none.
    pub(super) fn activation_read(
        &mut self,
        cell: CellId,
    ) -> Result<Option<js::ExprId>, FormationError> {
        if self.this_cells.contains(&cell) {
            return Ok(Some(self.expression(js::Expr::This)?));
        }
        if self.arguments_cells.contains(&cell)
            || self.formal_lists.iter().any(|(list, _)| *list == cell)
        {
            let arguments = self.text("arguments")?;
            return Ok(Some(
                self.expression(js::Expr::Host(js::Host::new(arguments)))?,
            ));
        }
        Ok(None)
    }

    /// A read `list[k]` of a rest list spelled as formals: formal `k`.
    pub(super) fn formal_read(
        &mut self,
        unit: ContextId,
        receiver: ValueId,
        place: PlaceId,
    ) -> Result<Option<js::ExprId>, FormationError> {
        if self.formal_lists.is_empty() {
            return Ok(None);
        }
        let Some(list) = self.formal_list_value(unit, receiver) else {
            return Ok(None);
        };
        let data = self.data(unit);
        let Some(index) = self.constant_index(data, place, receiver) else {
            return Err(self.error(Span::default(), "rest formal read at a varying index"));
        };
        let formal = self
            .formal_lists
            .iter()
            .find(|(found, _)| *found == list)
            .and_then(|(_, formals)| formals.get(index).copied())
            .ok_or_else(|| self.error(Span::default(), "rest formal outside its list"))?;
        Ok(Some(self.reference(formal)?))
    }

    /// The rest list whose plain read defines `value`, when it is spelled
    /// as formals.
    pub(super) fn formal_list_value(&self, unit: ContextId, value: ValueId) -> Option<CellId> {
        if self.formal_lists.is_empty() {
            return None;
        }
        let data = self.data(unit);
        let OperationKind::Load(place) =
            data.operations[data.values[value.index()].definition.index()].kind
        else {
            return None;
        };
        let Place::Cell(cell) = data.places[place.index()] else {
            return None;
        };
        self.formal_lists
            .iter()
            .any(|(list, _)| *list == cell)
            .then_some(cell)
    }

    /// Whether this adapter call's argument was formed as the method itself.
    pub(super) fn method_call(&self, unit: UnitId, call: CallId) -> bool {
        self.method_calls.contains(&(unit, call))
    }

    /// The prototype method a static body only its class reaches is: the
    /// body itself, formed once, in the context that creates it (the
    /// class's module), whichever of its creation and its class comes
    /// first. Its `length` stops at the first default, as the source
    /// method's does.
    pub(super) fn prototype_method_function(
        &mut self,
        unit: ContextId,
        method: UnitId,
        span: Span,
    ) -> Result<js::FunctionId, FormationError> {
        if let Some(&(_, function)) = self
            .class_methods
            .iter()
            .find(|(found, _)| *found == method)
        {
            return Ok(function);
        }
        let Some(form) = self.prototype_method_form(method)? else {
            return Err(self.error(span, "prototype method form"));
        };
        // The creating closure operation, in this context's unit.
        let creation = self
            .uses
            .and_then(|uses| uses.creators(method))
            .and_then(|creators| match creators.sites() {
                [site] if site.unit == self.semantic(unit) => Some(site.operation),
                _ => None,
            })
            .ok_or_else(|| self.error(span, "prototype method outside its class's module"))?;
        let child = self
            .demand
            .child(unit, creation)
            .ok_or_else(|| self.error(span, "missing callable demand context"))?;
        let region = self.data(unit).operations[creation.index()].region;
        let parent = self.plan(unit).regions[region.index()];
        let body = self
            .module
            .region_in(self.module.regions[parent.index()].scope, self.budget)?;
        let formals = self.begin_method(body, &form)?;
        self.plan_context(child, body)?;
        self.method_aliases(child, body, &form)?;
        self.statement_region(child, self.data(child).entry)?;
        self.finish_unit(child)?;
        let parameters = self.method_parameters(child, &form, formals)?;
        let program = self.program;
        let length = program
            .unit(method)
            .and_then(|data| data.declared_length)
            .map(|p| p as usize - 1);
        let function = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                rest: program
                    .unit(method)
                    .and_then(|data| data.callable_type)
                    .and_then(|ty| program.types[ty.index()].callable_signature())
                    .is_some_and(|signature| signature.has_rest()),
                parameters,
                body,
                arrow: false,
                // A class method's name is its key.
                name: js::FunctionName::Unobserved,
                // A class body is strict (legality: `prototype_method_form`).
                strict: false,
                length,
                suspension: js::Suspension::None,
            },
        )?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.unit_functions,
            (method, function),
        )?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.class_methods,
            (method, function),
        )?;
        Ok(function)
    }
}
