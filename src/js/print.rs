use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use std::fmt::Write;

impl Binary {
    fn token(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Remainder => "%",
            Self::ShiftLeft => "<<",
            Self::ShiftRight => ">>",
            Self::UnsignedShiftRight => ">>>",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::StrictEqual => "===",
            Self::StrictNotEqual => "!==",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::In => " in ",
            Self::InstanceOf => " instanceof ",
            Self::BitAnd => "&",
            Self::BitXor => "^",
            Self::BitOr => "|",
            Self::And => "&&",
            Self::Or => "||",
            Self::Nullish => "??",
        }
    }
    fn precedence(self) -> u8 {
        match self {
            Self::Or | Self::Nullish => 4,
            Self::And => 5,
            Self::BitOr => 6,
            Self::BitXor => 7,
            Self::BitAnd => 8,
            Self::StrictEqual | Self::StrictNotEqual | Self::Equal | Self::NotEqual => 9,
            Self::Less
            | Self::LessEqual
            | Self::Greater
            | Self::GreaterEqual
            | Self::In
            | Self::InstanceOf => 10,
            Self::ShiftLeft | Self::ShiftRight | Self::UnsignedShiftRight => 11,
            Self::Add | Self::Subtract => 12,
            Self::Multiply | Self::Divide | Self::Remainder => 13,
        }
    }
}

pub(super) fn precedence(expression: &Expr) -> u8 {
    match expression {
        Expr::Sequence(_) => 1,
        Expr::Assign { .. } => 2,
        Expr::Conditional { .. } => 3,
        Expr::Binary { op, .. } => op.precedence(),
        Expr::ToInt32(_) | Expr::IntBinary { .. } | Expr::IntNegate(_) => {
            Binary::BitOr.precedence()
        }
        Expr::Intrinsic { operation, .. } if integer_intrinsic(*operation) => {
            Binary::BitOr.precedence()
        }
        Expr::Intrinsic { .. } => 17,
        Expr::Unary { .. }
        | Expr::Await(_)
        | Expr::Literal(Literal::Undefined)
        | Expr::Literal(Literal::Bool(_)) => 14,
        // A YieldExpression is an AssignmentExpression.
        Expr::Yield { .. } => 2,
        Expr::Literal(Literal::Number(value)) if value.is_sign_negative() => 14,
        Expr::Call { .. }
        | Expr::Construct { .. }
        | Expr::ConstructIntrinsic { .. }
        | Expr::SuperCall { .. }
        | Expr::LoadModule { .. } => 17,
        Expr::Member { .. } => 18,
        _ => 19,
    }
}

pub(super) fn render(module: &Module, names: &Names) -> String {
    render_bounded(module, names, usize::MAX).expect("unbounded output")
}

/// Error before a complete artifact is published. No partial text escapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrintError {
    Admission(AllocationError),
    ByteLimit,
    Container(&'static str),
}

pub(super) fn render_bounded(
    module: &Module,
    names: &Names,
    limit: usize,
) -> Result<String, String> {
    let mut budget = AllocationBudget::new(None);
    render_admitted(module, names, limit, &mut budget).map_err(|error| match error {
        PrintError::ByteLimit => "render exceeds candidate byte budget".into(),
        PrintError::Admission(error) => format!("render admission failed: {error:?}"),
        PrintError::Container(reason) => reason.into(),
    })
}

/// The only printer, also used by inspection. Successful text retains its
/// capacity in the caller's budget; that owner must drop text before releasing
/// the retained allocation. Failed output is destroyed inside this scope.
pub(super) fn render_admitted(
    module: &Module,
    names: &Names,
    limit: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<String, PrintError> {
    render_with_literals_admitted(
        module,
        names,
        &[],
        LiteralOutput::Original,
        limit,
        budget,
        None,
        crate::config::JavaScriptFormat::Bare,
    )
}

pub(super) fn render_with_literals_admitted(
    module: &Module,
    names: &Names,
    literal_alternatives: &[LiteralAlternative],
    literals: LiteralOutput,
    limit: usize,
    budget: &mut AllocationBudget<'_>,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
    format: crate::config::JavaScriptFormat,
) -> Result<String, PrintError> {
    let _timing = crate::timing::TARGET_PRINT.scope(0);
    let mut phase = budget.scope();
    let forms = match module.print_forms.as_ref() {
        Some(forms) => std::borrow::Cow::Borrowed(forms),
        None => std::borrow::Cow::Owned(
            super::spellings::PrintForms::new(module, false, AllocationClass::Scratch, &mut phase)
                .map_err(PrintError::Admission)?,
        ),
    };
    let private = private_map(module);
    let mut printer = Printer {
        private: private.as_ref(),
        optional_link: None,
        module,
        names,
        literal_alternatives,
        literals,
        forms: &forms,
        output: Buffer {
            text: String::new(),
            points: None,
            budget: &mut phase,
            limit,
            error: None,
        },
        discarded_root: None,
        lazy: &[],
        container: None,
        root_activation: true,
        planned_structure: PlannedStructure::default(),
        inline: None,
        inline_module: None,
        local_names: &[],
    };
    let wrapped = format == crate::config::JavaScriptFormat::Iife;
    if wrapped {
        // Arrow scope preserves the entry's lexical `this` and strictness.
        // The complete private frame belongs to exact scoring.
        printer.text("(()=>{");
    }
    printer.foreign_imports(0..module.imports.len(), hosts, None, None);
    if let Some(hosts) = hosts {
        printer.host_bindings(hosts, 0..module.imports.len());
    }
    printer.region(module.root, false);
    if !module.exports.is_empty() {
        printer.text("export{");
        for (index, export) in module.exports.iter().enumerate() {
            if !printer.output.work(1) {
                break;
            }
            if index != 0 {
                printer.text(",");
            }
            let local = names.get(export.binding);
            if !printer.output.work(local.len()) {
                break;
            }
            printer.text(local);
            if local != export.name {
                printer.text(" as ");
                printer.text(&export.name);
            }
        }
        printer.text("};");
    }
    if wrapped {
        printer.text("})();");
    }
    let Buffer { text, error, .. } = printer.output;
    if let Some(error) = error {
        drop(text);
        return Err(error);
    }
    drop(forms);
    phase.finish_retained().map_err(PrintError::Admission)?;
    Ok(text)
}

/// Whether an import's source is a host module the output carries.
fn hosted(hosts: Option<(&crate::host_modules::HostDelivery, bool)>, import: &Import) -> bool {
    hosts.is_some_and(|(hosts, _)| {
        import
            .source
            .as_unicode()
            .is_some_and(|source| hosts.position(source).is_some())
    })
}

#[path = "print_containers.rs"]
mod containers;

#[path = "print_files.rs"]
mod files;
pub(super) use files::{render_planned_file_admitted, PlannedPrint, PlannedText};
pub(crate) use files::PlannedStructure;

struct Buffer<'a, 'ledger> {
    text: String,
    points: Option<Vec<crate::source_maps::Point>>,
    budget: &'a mut AllocationBudget<'ledger>,
    limit: usize,
    error: Option<PrintError>,
}
impl Buffer<'_, '_> {
    fn mark(&mut self, origin: Option<crate::program::SourceOriginId>) {
        let Some(points) = &mut self.points else { return; };
        if self.error.is_some() { return; }
        let point = crate::source_maps::Point { offset: self.text.len(), origin };
        if let Some(last) = points.last_mut() {
            if last.offset == point.offset { *last = point; return; }
            if last.origin == point.origin { return; }
        }
        if let Err(error) = self.budget.push(AllocationClass::Scratch, points, point) {
            self.error = Some(PrintError::Admission(error));
        }
    }
    fn shifted(&mut self, at: usize) {
        if let Some(points) = &mut self.points {
            for point in points.iter_mut().rev().take_while(|point| point.offset >= at) { point.offset += 1; }
        }
    }

    fn admit<T>(
        &mut self,
        build: impl FnOnce(&mut AllocationBudget<'_>) -> Result<T, AllocationError>,
    ) -> Option<T> {
        if self.error.is_some() {
            return None;
        }
        match build(self.budget) {
            Ok(value) => Some(value),
            Err(error) => {
                self.error = Some(PrintError::Admission(error));
                None
            }
        }
    }
    fn drop_vec<T>(&mut self, values: Vec<T>, class: AllocationClass) {
        let bytes = crate::output_budget::vector_bytes(&values);
        drop(values);
        if let Err(error) = bytes.and_then(|bytes| self.budget.release(class, bytes)) {
            self.error.get_or_insert(PrintError::Admission(error));
        }
    }
    fn drop_string(&mut self, value: String, class: AllocationClass) {
        let bytes = value.capacity() as u64;
        drop(value);
        if let Err(error) = self.budget.release(class, bytes) {
            self.error.get_or_insert(PrintError::Admission(error));
        }
    }
    fn work(&mut self, units: usize) -> bool {
        if self.error.is_some() {
            return false;
        }
        let result = u64::try_from(units)
            .map_err(|_| AllocationError::Capacity)
            .and_then(|units| self.budget.work(WorkKind::Render, units));
        if let Err(error) = result {
            self.error = Some(PrintError::Admission(error));
            return false;
        }
        true
    }
    /// Separate a binary `+`/`-` ending at `at` from an operand printed after
    /// it that starts with the same sign, which would otherwise form `++` or
    /// `--`. The space is appended through the admitted path and rotated into
    /// place, so capacity accounting is unchanged.
    fn separate_sign(&mut self, at: usize) {
        let (Some(&token), Some(&next)) = (
            self.text.as_bytes().get(at.wrapping_sub(1)),
            self.text.as_bytes().get(at),
        ) else {
            return;
        };
        if !matches!((token, next), (b'+', b'+') | (b'-', b'-')) {
            return;
        }
        self.push_str(" ");
        if self.error.is_some() {
            return;
        }
        let mut bytes = std::mem::take(&mut self.text).into_bytes();
        bytes[at..].rotate_right(1);
        self.shifted(at);
        self.text = String::from_utf8(bytes).expect("moving one ASCII byte keeps UTF-8");
    }
    /// A `/` ending at `at` followed by the `/` that opens a regular
    /// expression literal would read as a comment: separate them.
    fn separate_slash(&mut self, at: usize) {
        if !matches!(
            (
                self.text.as_bytes().get(at.wrapping_sub(1)),
                self.text.as_bytes().get(at)
            ),
            (Some(b'/'), Some(b'/'))
        ) {
            return;
        }
        self.push_str(" ");
        if self.error.is_some() {
            return;
        }
        let mut bytes = std::mem::take(&mut self.text).into_bytes();
        bytes[at..].rotate_right(1);
        self.shifted(at);
        self.text = String::from_utf8(bytes).expect("moving one ASCII byte keeps UTF-8");
    }
    /// A keyword printed just before `at` needs a space only when the
    /// following token would otherwise continue its word.
    fn separate_word(&mut self, at: usize) {
        let Some(&next) = self.text.as_bytes().get(at) else {
            return;
        };
        if !(next.is_ascii_alphanumeric() || matches!(next, b'_' | b'$' | b'\\') || next >= 0x80) {
            return;
        }
        self.push_str(" ");
        if self.error.is_some() {
            return;
        }
        let mut bytes = std::mem::take(&mut self.text).into_bytes();
        bytes[at..].rotate_right(1);
        self.shifted(at);
        self.text = String::from_utf8(bytes).expect("moving one ASCII byte keeps UTF-8");
    }
    fn push_str(&mut self, value: &str) {
        if self.error.is_some() {
            return;
        }
        if value.len() > self.limit.saturating_sub(self.text.len()) {
            self.error = Some(PrintError::ByteLimit);
            return;
        }
        if let Err(error) = self
            .budget
            .push_str(AllocationClass::Retained, &mut self.text, value)
        {
            self.error = Some(PrintError::Admission(error));
        }
    }
}
impl std::fmt::Write for Buffer<'_, '_> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.push_str(value);
        if self.error.is_some() {
            Err(std::fmt::Error)
        } else {
            Ok(())
        }
    }
}

struct Printer<'a, 'budget, 'ledger> {
    module: &'a Module,
    names: &'a Names,
    local_names: &'a [(BindingId, &'a str)],
    literal_alternatives: &'a [LiteralAlternative],
    literals: LiteralOutput,
    output: Buffer<'budget, 'ledger>,
    /// The statement value being printed without its normalization.
    discarded_root: Option<ExprId>,
    /// The specifier of each lazily delivered module's file (module,
    /// specifier), when this prints one file of several.
    lazy: &'a [(u32, String)],
    forms: &'a super::spellings::PrintForms,
    container: Option<&'a containers::ContainerView<'a>>,
    root_activation: bool,
    planned_structure: PlannedStructure,
    inline: Option<&'a inline::InlineView<'a>>,
    inline_module: Option<u32>,
    /// Spellings of program-private property names for this render
    /// (`private_map`), when the naming contract holds.
    private: Option<&'a std::collections::HashMap<String, String>>,
    /// The first link of the optional chain being printed (`forms.optional`):
    /// that member prints `?.` instead of `.`.
    optional_link: Option<ExprId>,
}

/// Host property names a generated private spelling never takes: protocol
/// members host code calls on any object (`then`, `next`, …) and short names
/// of common host objects (`x`, `id`, `at`, …), so a renamed private field can
/// neither turn an object into a thenable nor shadow a member host code reads.
const PRIVATE_SPELLING_AVOIDS: &[&str] = &[
    "x", "y", "z", "id", "at", "of", "is", "on", "to", "by", "do", "if", "in",
    "then", "next", "done", "value", "length", "name", "call", "apply", "bind",
    "get", "set", "has", "add", "constructor", "prototype", "toString", "valueOf",
    "toJSON", "handleEvent",
];

/// The spellings of program-private property names for one render, under
/// the naming contract (`assume_private_underscore_properties`). Every
/// property name of the tree that matches `^_(?!_)` and is not preserved
/// gets one spelling, the most frequent the shortest, that no other property
/// name or string literal of the tree spells and no host protocol uses.
/// Every occurrence prints through the same map, so a name written in one
/// module and read through a `JsValue` in another is renamed consistently.
/// Dynamic keys and strings handed to host functions are left alone, as
/// Terser's `mangle.properties.regex` leaves them.
pub(super) fn private_map(module: &Module) -> Option<std::collections::HashMap<String, String>> {
    use std::collections::{HashMap, HashSet};
    let preserved = module.private_names.as_ref()?;
    let private = |name: &str| {
        name.len() > 1 && name.starts_with('_') && !name.starts_with("__")
            && !preserved.iter().any(|kept| kept == name)
    };
    // Terser's `mangle.properties` (its default `undeclared: false`) does not
    // collect a name read only from an undeclared global (`globalThis._x`,
    // `window._x`): host code shares such names. A member chain's root is
    // found as Terser finds it, through receivers, callees and constructors.
    let host_rooted = |mut id: ExprId| loop {
        match &module.expressions[id.index()] {
            Expr::Member { object, .. } => id = *object,
            Expr::Call { callee, .. } | Expr::Construct { callee, .. } => id = *callee,
            Expr::Intrinsic { receiver, .. } => id = *receiver,
            Expr::Host(_) => break true,
            _ => break false,
        }
    };
    // Computed member keys print through the same map (`o["_k"]` is `o._k`).
    let mut computed_keys: HashSet<usize> = HashSet::new();
    for expression in &module.expressions {
        if let Expr::Member { property: Property::Computed(key), .. } = expression {
            computed_keys.insert(key.index());
        }
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut taken: HashSet<&str> = PRIVATE_SPELLING_AVOIDS.iter().copied().collect();
    let mut keys: Vec<&str> = Vec::new();
    let mut shared: Vec<&str> = Vec::new();
    let mut loose: HashSet<&str> = HashSet::new();
    for (index, expression) in module.expressions.iter().enumerate() {
        match expression {
            Expr::Member { object, property: Property::Named(name) } if host_rooted(*object) => {
                shared.push(name)
            }
            Expr::Member { property: Property::Named(name), .. } => keys.push(name),
            Expr::Object(entries) => {
                for (key, _) in entries {
                    if let Property::Named(name) = key {
                        keys.push(name);
                    }
                }
            }
            Expr::Class { methods, members, .. } => {
                for (name, _) in methods {
                    keys.push(name);
                }
                for member in members {
                    keys.push(&member.name);
                }
            }
            // A string may be a key elsewhere (`o[k]`, `k in o`, a host call):
            // no private spelling takes it, and a private name spelled by a
            // string anywhere but a member key keeps its spelling everywhere.
            Expr::Literal(Literal::String(value)) => {
                if let Some(text) = value.as_unicode() {
                    if !private(text) {
                        taken.insert(text);
                    } else if !computed_keys.contains(&index) {
                        loose.insert(text);
                    }
                }
            }
            _ => {}
        }
    }
    for name in keys {
        if private(name) && !loose.contains(name) {
            *counts.entry(name).or_default() += 1;
        } else {
            taken.insert(name);
        }
    }
    // A host-rooted read of a collected name is renamed with it, as Terser
    // renames every occurrence of a collected name.
    for name in shared {
        if let Some(count) = counts.get_mut(name) {
            *count += 1;
        } else {
            taken.insert(name);
        }
    }
    if counts.is_empty() {
        return None;
    }
    let mut names: Vec<(&str, usize)> = counts.into_iter().collect();
    names.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    const FIRST: &[u8] = b"etaoinsrhldcumfpgwybvkxjqzETAOINSRHLDCUMFPGWYBVKXJQZ";
    const REST: &[u8] = b"etaoinsrhldcumfpgwybvkxjqzETAOINSRHLDCUMFPGWYBVKXJQZ0123456789";
    let spelling = |mut index: usize| {
        let mut text = String::new();
        text.push(FIRST[index % FIRST.len()] as char);
        index /= FIRST.len();
        while index > 0 {
            index -= 1;
            text.push(REST[index % REST.len()] as char);
            index /= REST.len();
        }
        text
    };
    let mut map = HashMap::with_capacity(names.len());
    let mut next = 0usize;
    for (name, _) in names {
        let short = loop {
            let candidate = spelling(next);
            next += 1;
            if !taken.contains(candidate.as_str()) {
                break candidate;
            }
        };
        map.insert(name.to_string(), short);
    }
    Some(map)
}

/// The surrounding JavaScript syntax's named-evaluation behavior. A computed
/// object key may infer a runtime name even when its spelling is unknown here.
#[derive(Clone, Copy)]
enum InferredName<'a> {
    None,
    Known(&'a str),
    Computed,
}

impl<'a> Printer<'a, '_, '_> {
    fn local(&self, binding: BindingId) -> &'a str {
        self.local_names.binary_search_by_key(&binding, |(binding, _)| *binding)
            .map_or_else(|_| self.names.get(binding), |index| self.local_names[index].1)
    }

    /// Foreign imports (indices into the module's imports, in order): one
    /// declaration per specifier, at its first occurrence. Module requests
    /// are unique and ordered by first appearance (ECMA-262
    /// ModuleRequests), so merging a later import of a specifier into the
    /// first changes no linking or evaluation order. The imported name
    /// `default` is the default binding, `import x from"s"` (Rolldown
    /// `esm.rs` `create_import_declaration`; esbuild's printer likewise
    /// prints the default clause before the named ones). A planned file
    /// (`file`) delivered below the output directory spells a relative
    /// specifier from there (`files::rebased`).
    fn foreign_imports(
        &mut self,
        imports: impl Iterator<Item = usize>,
        hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
        file: Option<&str>,
        config: Option<&crate::compilation_policy::ContainerContract>,
    ) {
        let module = self.module;
        let mut order: Vec<usize> = Vec::new();
        for index in imports {
            if !self.output.work(1) {
                return;
            }
            if !hosted(hosts, &module.imports[index]) {
                if self
                    .output
                    .admit(|budget| budget.push(AllocationClass::Scratch, &mut order, index))
                    .is_none()
                {
                    return;
                }
            }
        }
        let Some(mut done) = self
            .output
            .admit(|budget| budget.filled(AllocationClass::Scratch, order.len(), false))
        else {
            return;
        };
        let mut group: Vec<usize> = Vec::new();
        for first in 0..order.len() {
            if done[first] {
                continue;
            }
            let source = &module.imports[order[first]].source;
            group.clear();
            for later in first..order.len() {
                if !self.output.work(1) {
                    return;
                }
                if !done[later] && module.imports[order[later]].source == *source {
                    done[later] = true;
                    if self
                        .output
                        .admit(|budget| {
                            budget.push(AllocationClass::Scratch, &mut group, order[later])
                        })
                        .is_none()
                    {
                        return;
                    }
                }
            }
            group.retain(|&index| !module.imports[index].imported.is_empty());
            let default = group
                .iter()
                .position(|&index| module.imports[index].imported == "default");
            self.text("import");
            if let Some(position) = default {
                let local = self.local(module.imports[group[position]].binding);
                if !self.output.work(local.len()) {
                    return;
                }
                self.text(" ");
                self.text(local);
                if group.len() > 1 {
                    self.text(",");
                }
            }
            if group.len() > usize::from(default.is_some()) {
                self.text("{");
                let mut first_named = true;
                for (position, &index) in group.iter().enumerate() {
                    if Some(position) == default {
                        continue;
                    }
                    let import = &module.imports[index];
                    let local = self.local(import.binding);
                    if !self.output.work(local.len().min(import.imported.len()) + 1) {
                        return;
                    }
                    if !first_named {
                        self.text(",");
                    }
                    first_named = false;
                    self.text(&import.imported);
                    if local != import.imported {
                        self.text(" as ");
                        self.text(local);
                    }
                }
                self.text("}");
            } else {
                self.text(" ");
            }
            if !group.is_empty() { self.text("from"); }
            let mapped = source.as_unicode().map(|source| config.map_or(source, |config| config.external_specifier(source)));
            let Some(rebased) = self
                .output
                .admit(|budget| match file.zip(mapped) {
                    Some((file, source)) => {
                        files::rebased_in(source, file, AllocationClass::Scratch, budget)
                    }
                    None => Ok(None),
                })
            else {
                return;
            };
            match rebased {
                Some(rebased) => {
                    self.unicode_string(&rebased);
                    self.output.drop_string(rebased, AllocationClass::Scratch);
                }
                None => match mapped { Some(source) => self.unicode_string(source), None => self.string(source) },
            }
            self.text(";");
        }
        self.output.drop_vec(group, AllocationClass::Scratch);
        self.output.drop_vec(done, AllocationClass::Scratch);
        self.output.drop_vec(order, AllocationClass::Scratch);
    }

    /// Formation has already proved the scope and iteration-identity rules.
    fn loop_head(&mut self, region: RegionId, closing: bool) -> bool {
        let Some(form) = self.forms.loops.get(region.index()).copied().flatten() else {
            return false;
        };
        self.text("for(let ");
        self.text(self.local(form.binding));
        self.text("=");
        self.expression(form.value, 2);
        self.text(";");
        if let Some(condition) = form.condition {
            self.expression(condition, 0);
        }
        self.text(";");
        if let Some(update) = form.update {
            self.expression(update, 0);
        }
        self.text(")");
        self.body(form.body, false, closing);
        true
    }

    /// `let{a:x,b}=<host modules>;` binding each carried import among
    /// `imports` (indices into the module's imports), before any statement,
    /// as ES evaluates imported modules first.
    fn host_bindings(
        &mut self,
        (hosts, strict): (&crate::host_modules::HostDelivery, bool),
        imports: impl Iterator<Item = usize>,
    ) {
        let Some(mut groups) = self.output.admit(|budget| {
            budget.vector::<Vec<usize>>(AllocationClass::Scratch, hosts.modules.len())
        }) else {
            return;
        };
        groups.resize_with(hosts.modules.len(), Vec::new);
        let mut requested = Vec::new();
        for index in imports {
            let import = &self.module.imports[index];
            if let Some(position) = import
                .source
                .as_unicode()
                .and_then(|source| hosts.position(source))
            {
                if groups[position].is_empty() && self.output.admit(|budget| budget.push(AllocationClass::Scratch, &mut requested, position)).is_none() { return; }
                if !groups[position].contains(&index) {
                    if self
                        .output
                        .admit(|budget| {
                            budget.push(AllocationClass::Scratch, &mut groups[position], index)
                        })
                        .is_none()
                    {
                        return;
                    }
                }
            }
        }
        if groups.iter().all(Vec::is_empty) {
            self.output.drop_vec(groups, AllocationClass::Scratch);
            return;
        }
        let pattern = |printer: &mut Self, group: &[usize]| {
            printer.text("{");
            let mut comma = false;
            for &index in group {
                let import = &printer.module.imports[index];
                if import.imported.is_empty() { continue; }
                if std::mem::replace(&mut comma, true) { printer.text(","); }
                let local = printer.local(import.binding);
                if identifier_name(&import.imported) {
                    printer.text(&import.imported);
                } else {
                    printer.unicode_string(&import.imported);
                }
                if local != import.imported {
                    printer.text(":");
                    printer.text(local);
                }
            }
            printer.text("}");
        };
        self.text("let");
        let Some(single) = self.output.admit(|budget| hosts.single_in(strict, budget)) else {
            return;
        };
        if let Some(single) = single {
            pattern(self, &groups[0]);
            self.text("=");
            self.text(&single);
            self.output.drop_string(single, AllocationClass::Retained);
        } else {
            self.text("[");
            let last = groups.iter().rposition(|group| !group.is_empty()).unwrap();
            for (position, group) in groups.iter().enumerate().take(last + 1) {
                if position != 0 {
                    self.text(",");
                }
                if !group.is_empty() {
                    pattern(self, group);
                }
            }
            self.text("]=");
            let Some(expression) = self
                .output
                .admit(|budget| hosts.expression_selected_in(strict, requested.iter().copied(), budget))
            else {
                return;
            };
            self.text(&expression);
            self.output
                .drop_string(expression, AllocationClass::Retained);
        }
        self.text(";");
        for group in &mut groups {
            self.output
                .drop_vec(std::mem::take(group), AllocationClass::Scratch);
        }
        self.output.drop_vec(groups, AllocationClass::Scratch);
        self.output.drop_vec(requested, AllocationClass::Scratch);
    }

    /// Returns only a nonnegative numeric literal, preserving primary precedence.
    /// The source observation was established by Formation, not by this lookup.
    fn observed_literal(&mut self, id: ExprId) -> Option<bool> {
        if self.literals != LiteralOutput::Observed {
            return None;
        }
        let Expr::Literal(Literal::String(value)) = &self.module.expressions[id.index()] else {
            return None;
        };
        let observation = literal_output::lookup(self.literal_alternatives, id, |n| {
            if self.output.work(n) {
                Ok(())
            } else {
                Err(())
            }
        })
        .ok()
        .flatten()?;
        if !self.output.work(1) {
            return None;
        }
        Some(match observation {
            WeakLiteralObservation::Truthy => !value.is_empty(),
            WeakLiteralObservation::Nullish => false,
        })
    }

    fn binary_operand(&mut self, id: ExprId, parent: Binary, minimum: u8) {
        if !matches!(parent, Binary::Nullish | Binary::And | Binary::Or) {
            self.expression(id, minimum);
            return;
        }
        let mut effective = id;
        while let Expr::ToInt32(value) = self.module.expressions[effective.index()] {
            if !self.output.work(1) {
                return;
            }
            if !self.plain_integer(effective) {
                break;
            }
            effective = value;
        }
        // JavaScript forbids unparenthesized mixing of ?? with &&/||,
        // independently of their numeric precedence. Inspect the selected
        // operand shape, including an omitted integer conversion.
        let mixed = matches!(self.module.expressions[effective.index()],
            Expr::Binary { op, .. } if
                (parent == Binary::Nullish && matches!(op, Binary::And | Binary::Or))
                || (matches!(parent, Binary::And | Binary::Or) && op == Binary::Nullish));
        if mixed {
            self.text("(");
            self.expression(id, 0);
            self.text(")");
        } else {
            self.expression(id, minimum);
        }
    }

    /// A typed intrinsic whose original returns an int32 (R10), or a value
    /// nothing reads: no `|0`. Under the `int32_hints` family the rule is the
    /// one before R10: an original's int32 is trusted only under pristine
    /// builtins.
    fn plain_integer(&self, id: ExprId) -> bool {
        self.discarded_root == Some(id)
            || (!self.module.int32_hints || self.module.pristine_builtins)
                && matches!(
                    self.module.expressions[id.index()],
                    Expr::Intrinsic { operation, .. } if original_int32_intrinsic(operation)
                )
    }

    /// The part of a statement's value worth printing. The value is
    /// discarded, so `void` is dropped, and so is `|0` on integer arithmetic,
    /// whose operands are numbers: neither can have an effect there.
    fn discarded(&mut self, mut id: ExprId) -> ExprId {
        loop {
            match self.module.expressions[id.index()] {
                Expr::Unary {
                    op: Unary::Void,
                    value,
                } => id = value,
                Expr::ToInt32(value)
                    if matches!(
                        self.module.expressions[value.index()],
                        Expr::IntBinary { .. }
                            | Expr::IntNegate(_)
                            | Expr::Literal(Literal::Number(_))
                    ) =>
                {
                    id = value
                }
                Expr::IntBinary { .. } | Expr::IntNegate(_) => {
                    self.discarded_root = Some(id);
                    return id;
                }
                // A typed integer method's original returns a number (R10):
                // `|0` on it has no effect when the value is discarded. The
                // `int32_hints` family keeps the rule before R10.
                Expr::Intrinsic { operation, .. }
                    if (!self.module.int32_hints || self.module.pristine_builtins)
                        && integer_intrinsic(operation) =>
                {
                    self.discarded_root = Some(id);
                    return id;
                }
                _ => return id,
            }
        }
    }

    fn precedence(&mut self, id: ExprId) -> u8 {
        if !self.output.work(1) {
            return 19;
        }
        let expression = &self.module.expressions[id.index()];
        if self.plain_integer(id) {
            match expression {
                Expr::IntBinary { op, .. } => return op.javascript().precedence(),
                Expr::IntNegate(_) => return 14,
                Expr::ToInt32(value) => return self.precedence(*value),
                Expr::Intrinsic { operation, .. } => {
                    return if matches!(intrinsic_form(*operation), IntrinsicForm::Property(_)) {
                        18
                    } else {
                        17
                    };
                }
                _ => {}
            }
        }
        match expression {
            Expr::Function(id) if self.module.functions[id.index()].arrow => 2,
            // An optional chain is a left-hand-side expression that a member
            // access, call or `new` must not extend: `(x?.a).b` is not `x?.a.b`.
            Expr::Conditional { .. } | Expr::Binary { op: Binary::And, .. }
                if self.forms.optional[id.index()].is_some() => 16,
            _ => precedence(expression),
        }
    }
    /// A string in `"`, or under the `quotes` family in the quote it escapes
    /// least (`'{"a":1}'`).
    fn string(&mut self, value: &StringValue) {
        self.string_chosen(value, self.module.quotes);
    }

    fn unicode_string(&mut self, value: &str) {
        if !self.output.work(value.len().saturating_mul(3)) {
            return;
        }
        let quote =
            if self.module.quotes && value.matches('"').count() > value.matches('\'').count() {
                '\''
            } else {
                '"'
            };
        let mut delimiter = [0; 4];
        let delimiter = quote.encode_utf8(&mut delimiter);
        self.text(delimiter);
        let _ = crate::js_string::unicode_contents(&mut self.output, value, quote, false);
        self.text(delimiter);
    }
    fn string_chosen(&mut self, value: &StringValue, compact: bool) {
        let quote = match value.as_unicode() {
            Some(text) if compact && text.matches('"').count() > text.matches('\'').count() => '\'',
            _ => '"',
        };
        self.string_delimited(value, quote);
    }
    fn string_delimited(&mut self, value: &StringValue, quote: char) {
        let mut delimiter = [0; 4];
        let delimiter = quote.encode_utf8(&mut delimiter);
        self.text(delimiter);
        self.string_content(value, quote);
        self.text(delimiter);
    }

    fn string_content(&mut self, value: &StringValue, quote: char) {
        // Charge decoding/escaping before scanning; the writer separately
        // admits each emitted byte and any output-buffer relocation.
        if !self.output.work(value.storage_bytes()) {
            return;
        }
        let after_dollar = self.output.text.ends_with('$');
        let _ = crate::js_string::contents(&mut self.output, value, quote, after_dollar);
    }

    fn text(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn constructor_target(&mut self, id: ExprId) -> bool {
        if !self.output.work(1) {
            return false;
        }
        match &self.module.expressions[id.index()] {
            Expr::Member { object, .. } => self.constructor_target(*object),
            Expr::Binding(_) | Expr::Host(_) | Expr::This => true,
            _ => false,
        }
    }

    fn statement_needs_group(&mut self, id: ExprId, minimum: u8) -> bool {
        if !self.output.work(1) {
            return false;
        }
        let expression = &self.module.expressions[id.index()];
        if let Expr::ToInt32(value) = expression {
            if self.plain_integer(id) {
                return self.statement_needs_group(*value, minimum);
            }
        }
        if self.precedence(id) < minimum {
            return false;
        }
        match expression {
            Expr::Literal(Literal::String(_)) => self.observed_literal(id).is_none(),
            Expr::Object(_) | Expr::Function(_) | Expr::Class { .. } => true,
            Expr::Call { .. } if self.forms.spread[id.index()] => true,
            Expr::Binary { op, left, .. } => self.statement_needs_group(*left, op.precedence()),
            Expr::ToInt32(value) => self.statement_needs_group(*value, Binary::BitOr.precedence()),
            Expr::IntBinary { op, left, .. } => {
                self.statement_needs_group(*left, op.javascript().precedence())
            }
            Expr::Conditional { condition, .. } => self.statement_needs_group(*condition, 4),
            Expr::Assign { target, .. } => self.statement_needs_group(*target, 18),
            Expr::Sequence(values) => self.statement_needs_group(values[0], 2),
            // The member and invocation printers already group function/object
            // bases. Other valid bases cannot start a declaration or block.
            _ => false,
        }
    }

    /// A property name as printed: its private spelling under the naming
    /// contract (`private_map`), otherwise itself.
    fn property_spelling<'n>(&self, name: &'n str) -> &'n str
    where
        'a: 'n,
    {
        match self.private.and_then(|map| map.get(name)) {
            Some(short) => short.as_str(),
            None => name,
        }
    }

    fn property(&mut self, property: &Property) {
        match property {
            Property::Named(name) => {
                self.text(".");
                let name = self.property_spelling(name);
                self.text(name);
            }
            Property::Computed(key) => {
                // `o["name"]` and `o.name` read the same property.
                if let Some(name) = self.identifier_key(*key) {
                    self.text(".");
                    let name = self.property_spelling(name);
                    self.text(name);
                    return;
                }
                // `o["32"]` and `o[32]` read the same property too.
                if let Some(number) = self.number_key(*key, true) {
                    self.text("[");
                    self.text(number);
                    self.text("]");
                    return;
                }
                self.text("[");
                self.expression(*key, 0);
                self.text("]");
            }
        }
    }

    /// A computed key that is a plain string literal spelled as an
    /// identifier name, printed as itself rather than weakened or shared.
    fn identifier_key(&mut self, key: ExprId) -> Option<&'a str> {
        let module = self.module;
        let Expr::Literal(Literal::String(value)) = &module.expressions[key.index()] else {
            return None;
        };
        let name = value.as_unicode().filter(|name| identifier_name(name))?;
        if self.observed_literal(key).is_some() || !self.output.work(name.len()) {
            return None;
        }
        Some(name)
    }

    /// A literal string key without an observed alternative that a numeric
    /// literal spells (`simple_number_key`): `{32:v}`, `o[32]`, and `o[-1]`
    /// where `signed`.
    fn number_key(&mut self, key: ExprId, signed: bool) -> Option<&'a str> {
        let module = self.module;
        let Expr::Literal(Literal::String(value)) = &module.expressions[key.index()] else {
            return None;
        };
        let number = value
            .as_unicode()
            .filter(|text| simple_number_key(text, signed))?;
        if self.observed_literal(key).is_some() || !self.output.work(number.len()) {
            return None;
        }
        Some(number)
    }

    /// A literal string key without an observed alternative, other than
    /// `__proto__`, with its text as a function name.
    fn string_key(&mut self, key: ExprId) -> Option<(&'a StringValue, &'a str)> {
        let module = self.module;
        let Expr::Literal(Literal::String(value)) = &module.expressions[key.index()] else {
            return None;
        };
        let name = value.as_unicode().filter(|name| *name != "__proto__")?;
        if self.observed_literal(key).is_some() || !self.output.work(name.len()) {
            return None;
        }
        Some((value, name))
    }

    fn receiver(&mut self, value: ExprId) {
        let force = self.observed_literal(value).is_some()
            || self.forms.spread[value.index()]
            || matches!(
                self.module.expressions[value.index()],
                Expr::Literal(Literal::Number(_)) | Expr::Object(_) | Expr::Function(_)
            );
        if force {
            self.text("(");
        }
        self.expression(value, 17);
        if force {
            self.text(")");
        }
    }

    fn arguments(&mut self, values: &[ExprId]) {
        self.text("(");
        self.list(values);
        self.text(")");
    }

    fn list(&mut self, values: &[ExprId]) {
        for (index, value) in values.iter().enumerate() {
            if !self.output.work(1) {
                return;
            }
            if index != 0 {
                self.text(",");
            }
            self.expression(*value, 2);
        }
    }

    fn expression(&mut self, id: ExprId, minimum: u8) {
        self.expression_with_name(id, minimum, InferredName::None);
    }

    fn expression_with_name(&mut self, id: ExprId, minimum: u8, inferred: InferredName<'_>) {
        if !self.output.work(1) {
            return;
        }
        if self.output.points.is_some() {
            if let Some(origin) = self.module.origins.get(id.index()).copied().flatten() { self.output.mark(Some(origin)); }
        }
        let expression = &self.module.expressions[id.index()];
        if let Expr::Function(function) = expression {
            if let Some(name) = self.module.functions[function.index()].name.exact() {
                // Name comparison and two possible identifier scans.
                let Some(work) = name.storage_bytes().checked_mul(3) else {
                    self.output.error = Some(PrintError::Admission(AllocationError::Capacity));
                    return;
                };
                if !self.output.work(work) {
                    return;
                }
                let matches = match inferred {
                    InferredName::None => name.is_empty(),
                    InferredName::Known(inferred) => name.as_unicode() == Some(inferred),
                    InferredName::Computed => false,
                };
                if !matches && self.names.self_named(*function) {
                    // `function name(){…}` owns its name wherever it stands.
                    self.named_function_expression(*function, name);
                    return;
                }
                if !matches {
                    // A sequence suppresses accidental named evaluation and
                    // produces a value, never a property reference receiver.
                    // Where a binding, target or key would name the value, the
                    // position is an initializer or entry, never a callee, a
                    // statement start or an arrow body: `{name:f}.name`, a
                    // member access, is already a value there.
                    let bare = !name.is_empty() && !matches!(inferred, InferredName::None);
                    if !bare {
                        self.text("(0,");
                    }
                    if name.is_empty() {
                        self.function_expression(*function);
                    } else {
                        self.text("{");
                        if let Some(name) = name
                            .as_unicode()
                            .filter(|name| identifier(name) && *name != "__proto__")
                        {
                            self.text(name);
                        } else {
                            self.text("[");
                            self.string(name);
                            self.text("]");
                        }
                        self.text(":");
                        self.function_expression(*function);
                        self.text("}");
                        if let Some(name) = name.as_unicode().filter(|name| identifier(name)) {
                            self.text(".");
                            self.text(name);
                        } else {
                            self.text("[");
                            self.string(name);
                            self.text("]");
                        }
                    }
                    if !bare {
                        self.text(")");
                    }
                    return;
                }
            }
        }
        if let Expr::ToInt32(value) = expression {
            if self.plain_integer(id) {
                self.expression(*value, minimum);
                return;
            }
        }
        let level = self.precedence(id);
        let parens = level < minimum;
        if parens {
            self.text("(");
        }
        match expression {
            Expr::Literal(value) => match value {
                Literal::Number(value) => {
                    if *value == 0.0 && value.is_sign_negative() {
                        self.text("-0");
                    } else if value.is_finite() {
                        if !self.output.work(64) {
                            return;
                        }
                        let _ = write_number(&mut self.output, *value);
                    } else {
                        let _ = write!(self.output, "{value}");
                    }
                }
                Literal::String(value) => {
                    if let Some(truthy) = self.observed_literal(id) {
                        self.text(if truthy { "1" } else { "0" });
                    } else {
                        self.string_delimited(
                            value,
                            if self.forms.quotes.get(id.index()).copied().unwrap_or(false) {
                                '\''
                            } else {
                                '"'
                            },
                        );
                    }
                }
                // `!0` and `!1` are the booleans, three and four bytes shorter.
                Literal::Bool(value) => self.text(if *value { "!0" } else { "!1" }),
                Literal::Null => self.text("null"),
                Literal::Undefined => self.text("void 0"),
            },
            Expr::Binding(symbol) => self.binding_read(*symbol),
            Expr::Host(host) if host.kind == crate::catalog::HostKind::ModuleUrl => {
                match self.container {
                    None => self.text("import.meta.url"),
                    Some(view) if view.commonjs => self.text("require('node:url').pathToFileURL(__filename).href"),
                    Some(_) => { self.output.error.get_or_insert(PrintError::Container("JS.moduleUrl requires ESM or Node CommonJS delivery")); }
                }
            }
            Expr::Host(host) => self.text(&host.name),
            Expr::Regex(literal) => {
                // `a/ /x/`: a division before the literal would otherwise
                // open a comment.
                let at = self.output.text.len();
                self.text(literal);
                self.output.separate_slash(at);
            }
            Expr::This => {
                if self.root_activation && self.container.is_some_and(|view|view.strict) {self.text("(void 0)");}
                else {self.text("this");}
            },
            Expr::Unary { op, value } => {
                self.text(match op {
                    Unary::Negate => "-",
                    Unary::Not => "!",
                    Unary::BitNot => "~",
                    Unary::Plus => "+",
                    Unary::TypeOf => "typeof ",
                    Unary::Void => "void ",
                    Unary::Delete => "delete ",
                });
                // Parenthesizing a sign operand also prevents accidental ++/--.
                self.expression(
                    *value,
                    if matches!(op, Unary::Negate | Unary::Plus) {
                        15
                    } else {
                        14
                    },
                );
            }
            Expr::ToInt32(value) => {
                self.expression(*value, Binary::BitOr.precedence());
                self.text("|0");
            }
            Expr::IntNegate(value) => {
                self.text("-");
                self.expression(*value, 15);
                if !self.plain_integer(id) {
                    self.text("|0");
                }
            }
            Expr::IntBinary { op, left, right } => {
                let op = op.javascript();
                let level = op.precedence();
                self.expression(*left, level);
                self.text(op.token());
                let at = self.output.text.len();
                self.expression(*right, level + 1);
                self.output.separate_sign(at);
                if !self.plain_integer(id) {
                    self.text("|0");
                }
            }
            Expr::ConstructIntrinsic {
                operation,
                arguments,
            } => {
                self.text("new ");
                self.text(
                    native_constructor(*operation)
                        .expect("verified construction")
                        .name,
                );
                // `new X` constructs as `new X()` does wherever no call or
                // member access follows it.
                if !arguments.is_empty() || minimum >= 17 {
                    self.arguments(arguments);
                }
            }
            Expr::Intrinsic {
                operation,
                receiver,
                arguments,
            } => {
                self.receiver(*receiver);
                self.text(".");
                match intrinsic_form(*operation) {
                    IntrinsicForm::Property(name) => self.text(name),
                    IntrinsicForm::Method(name) => {
                        self.text(name);
                        self.arguments(arguments);
                    }
                }
                if integer_intrinsic(*operation) && !self.plain_integer(id) {
                    self.text("|0");
                }
            }
            // `x!=null&&x.a` in a test: the chain alone, `x?.a`.
            Expr::Binary { op: Binary::And, right, .. } if self.forms.optional[id.index()].is_some() => {
                let link = self.forms.optional[id.index()].unwrap();
                let outer = self.optional_link.replace(link);
                self.expression(*right, 0);
                self.optional_link = outer;
            }
            Expr::Binary { op, left, right } => {
                let level = op.precedence();
                self.binary_operand(*left, *op, level);
                self.text(op.token());
                // A sign can otherwise join the binary token into ++ or --.
                let at = self.output.text.len();
                self.binary_operand(*right, *op, level + 1);
                self.output.separate_sign(at);
            }
            Expr::Member { object, property } if self.optional_link == Some(id) => {
                self.optional_link = None;
                self.receiver(*object);
                self.text("?");
                match property {
                    // `?.[k]`; any other key prints as `property` prints it.
                    Property::Computed(key)
                        if self.identifier_key(*key).is_none() && self.number_key(*key, true).is_none() =>
                    {
                        self.text(".[");
                        self.expression(*key, 0);
                        self.text("]");
                    }
                    Property::Computed(key) if self.identifier_key(*key).is_none() => {
                        self.text(".");
                        self.property(property);
                    }
                    _ => self.property(property),
                }
            }
            Expr::Member { object, property } => {
                self.receiver(*object);
                self.property(property);
            }
            // `Object.assign({},a,{k:v})` as the spread `{...a,k:v}`.
            Expr::Call { callee, arguments, .. } if self.forms.spread[id.index()] => {
                let module = self.module;
                let (sources, entries) = module.spread_assign(*callee, arguments).expect("proved spread form");
                self.object_literal(sources, entries);
            }
            Expr::Call {
                callee,
                arguments,
                invocation,
            } => {
                let callee_node = &self.module.expressions[callee.index()];
                let unbind = matches!(callee_node, Expr::Binding(binding) if self.member_binding(*binding)) || *invocation == Invocation::Value
                    && (self.forms.optional[callee.index()].is_some()
                        || match callee_node {
                            Expr::Member { .. } => true,
                            Expr::Host(host) => host.kind == crate::catalog::HostKind::Eval,
                            Expr::Binding(symbol) => self.local(*symbol) == "eval" || self.member_binding(*symbol),
                            _ => false,
                        });
                let function_literal = matches!(callee_node, Expr::Function(_));
                if unbind {
                    self.text("(0,");
                } else if function_literal {
                    self.text("(");
                }
                // Inside its own parentheses a function literal needs none.
                let minimum = if unbind {
                    2
                } else if function_literal {
                    0
                } else {
                    17
                };
                self.expression(*callee, minimum);
                if unbind || function_literal {
                    self.text(")");
                }
                self.arguments(arguments);
            }
            Expr::Construct { callee, arguments } => {
                self.text("new ");
                // A call used as the constructor must be grouped: new f()()
                // calls an instance; new (f())() constructs the returned value.
                let force = !self.constructor_target(*callee);
                if force {
                    self.text("(");
                }
                self.expression(*callee, 18);
                if force {
                    self.text(")");
                }
                if !arguments.is_empty() || minimum >= 17 {
                    self.arguments(arguments);
                }
            }
            Expr::Conditional { yes, no, .. } if self.forms.optional[id.index()].is_some() => {
                // The chain is the branch that is not `undefined`; its first
                // link, read from the tested binding, prints `?.`.
                let link = self.forms.optional[id.index()].unwrap();
                let chain = if matches!(self.module.expressions[yes.index()], Expr::Literal(Literal::Undefined)) {
                    *no
                } else {
                    *yes
                };
                let outer = self.optional_link.replace(link);
                self.expression(chain, 0);
                self.optional_link = outer;
            }
            Expr::Conditional { condition, yes, no } => {
                self.expression(*condition, 4);
                self.text("?");
                self.expression(*yes, 2);
                self.text(":");
                self.expression(*no, 2);
            }
            // `x=x+1` of a number is `++x` (its value is the new one too).
            Expr::Assign { target, .. } if self.forms.increment[id.index()].is_some() => {
                let op = self.forms.increment[id.index()].unwrap();
                self.text(op);
                self.expression(*target, 18);
            }
            Expr::Assign { target, .. } if self.forms.compound[id.index()].is_some() => {
                let (op, right) = self.forms.compound[id.index()].unwrap();
                self.expression(*target, 18);
                self.text(op.token());
                self.text("=");
                self.expression(right, 2);
            }
            Expr::Assign { target, value } => {
                self.expression(*target, 18);
                self.text("=");
                let inferred = match &self.module.expressions[target.index()] {
                    Expr::Binding(binding) => InferredName::Known(self.local(*binding)),
                    Expr::Host(host) => InferredName::Known(&host.name),
                    _ => InferredName::None,
                };
                self.expression_with_name(*value, 2, inferred);
            }
            Expr::Sequence(values) => self.list(values),
            Expr::Template(parts) => {
                self.text("`");
                for part in parts {
                    if !self.output.work(1) {
                        return;
                    }
                    match part {
                        TemplatePart::String(value) => self.string_content(value, '`'),
                        TemplatePart::Expression(value) => {
                            self.text("${");
                            self.expression(*value, 1);
                            self.text("}");
                        }
                    }
                }
                self.text("`");
            }
            Expr::Array(values) => {
                self.text("[");
                self.list(values);
                self.text("]");
            }
            Expr::Spread(value) => {
                self.text("...");
                self.expression(*value, 2);
            }
            Expr::Await(value) => {
                self.text("await ");
                self.expression(*value, 14);
            }
            Expr::LoadModule {
                module,
                specifier,
                members,
                promise,
                string,
            } => {
                let chunk = self
                    .lazy
                    .iter()
                    .find(|(loaded, _)| loaded == module)
                    .map(|(_, specifier)| specifier);
                if chunk.is_some() || self.inline.is_some() {
                    // The file's own namespace; a failed load reports the
                    // source specifier, as the old route did.
                    if let Some(inline) = self.inline {
                        self.expression(*promise,18); self.text(".resolve().then(()=>"); self.text(inline.prefix);
                        let _ = write!(self.output,"l({module}))");
                    } else if let Some(view)=self.container.filter(|view| view.commonjs) {
                        self.expression(*promise,18);self.text(".resolve().then(()=>");self.text(view.prefix);self.text("r(");self.unicode_string(chunk.unwrap());self.text("))");
                    } else { self.text("import(");self.unicode_string(chunk.unwrap());self.text(")"); }
                    self.text(".catch(e=>");
                    self.expression(*promise, 18);
                    self.text(".reject({specifier:");
                    self.unicode_string(specifier);
                    self.text(",message:");
                    self.expression(*string, 18);
                    self.text("(e)}))");
                } else if members.is_empty() {
                    self.expression(*promise, 18);
                    self.text(".resolve({})");
                } else {
                    // Built a turn later, once every module has initialized.
                    self.expression(*promise, 18);
                    self.text(".resolve().then(()=>({");
                    for (index, (name, value)) in members.iter().enumerate() {
                        if !self.output.work(1) {
                            return;
                        }
                        if index != 0 {
                            self.text(",");
                        }
                        if identifier_name(name) || simple_number_key(name, false) {
                            self.text(name);
                        } else {
                            self.unicode_string(name);
                        }
                        self.text(":");
                        self.expression(*value, 2);
                    }
                    self.text("}))");
                }
            }
            Expr::Yield { value, delegate } => {
                self.text(if *delegate { "yield*" } else { "yield " });
                self.expression(*value, 2);
            }
            Expr::Object(entries) => self.object_literal(&[], entries),
            Expr::Function(function) => {
                self.function_expression(*function);
            }
            Expr::Class {
                name,
                base,
                constructor,
                methods,
                members,
            } => {
                self.text("class");
                if !name.is_empty() {
                    self.text(" ");
                    self.text(name);
                }
                if let Some(base) = base {
                    self.text(" extends ");
                    // The heritage is a LeftHandSideExpression.
                    self.expression(*base, 18);
                }
                self.text("{");
                if let Some(constructor) = constructor {
                    self.text("constructor");
                    self.function(*constructor);
                }
                for (method, function) in methods {
                    if self.module.functions[function.index()].suspension == Suspension::Async {
                        self.text("async ");
                    }
                    let method = self.property_spelling(method);
                    self.text(method);
                    self.function(*function);
                }
                for member in members {
                    self.text(match member.kind {
                        MemberKind::Getter => "get ",
                        MemberKind::Static => "static ",
                    });
                    let name = self.property_spelling(&member.name);
                    self.text(name);
                    self.function(member.function);
                }
                self.text("}");
            }
            Expr::SuperCall { arguments } => {
                self.text("super");
                self.arguments(arguments);
            }
        }
        if parens {
            self.text(")");
        }
    }

    /// An object literal: `...source` for each source (the spread form of
    /// `Object.assign({},…)`), then its entries.
    fn object_literal(&mut self, sources: &[ExprId], entries: &'a [(Property, ExprId)]) {
        self.text("{");
        for (index, source) in sources.iter().enumerate() {
            if index != 0 {
                self.text(",");
            }
            self.text("...");
            self.expression(*source, 2);
        }
        for (index, (key, value)) in entries.iter().enumerate() {
            if !self.output.work(1) {
                return;
            }
            if index != 0 || !sources.is_empty() {
                self.text(",");
            }
            // `{["name"]:v}` and `{name:v}` define the same own data
            // property, and both name an anonymous function `name`;
            // only `__proto__:` would set the prototype instead.
            let literal = match key {
                Property::Computed(key) => self
                    .identifier_key(*key)
                    .filter(|name| *name != "__proto__"),
                Property::Named(_) => None,
            };
            // A canonical integer key is `32:`, the same own data
            // property as `"32":` (L2 spelling; Closure's printer,
            // Terser's `print_property_name`, esbuild and Oxc all
            // print it so).
            let numeric = match (key, literal) {
                (Property::Computed(key), None) => self.number_key(*key, false),
                _ => None,
            };
            // Any other literal string key is `"s":`, the same own
            // data property as `["s"]:`.
            let quoted = match (key, literal, numeric) {
                (Property::Computed(key), None, None) => self.string_key(*key),
                _ => None,
            };
            // `{k}` is `{k:k}`: the value is a reference printed with
            // the key's own spelling (`__proto__` never, it would set
            // the prototype in one form and not the other).
            let spelled = match (key, literal) {
                (_, Some(name)) => Some(self.property_spelling(name)),
                (Property::Named(name), None) if name != "__proto__" => {
                    Some(self.property_spelling(name.as_str()))
                }
                _ => None,
            };
            let shorthand =
                spelled.is_some_and(|name| match &self.module.expressions[value.index()] {
                    Expr::Binding(binding) => self.local(*binding) == name && !self.member_binding(*binding),
                    Expr::Host(host) => host.name == *name,
                    _ => false,
                });
            if shorthand {
                self.text(spelled.unwrap());
                continue;
            }
            match (key, literal) {
                (_, Some(name)) => {
                    let name = self.property_spelling(name);
                    self.text(name)
                }
                (Property::Computed(_), None) if numeric.is_some() => {
                    self.text(numeric.unwrap())
                }
                (Property::Computed(_), None) if quoted.is_some() => {
                    self.string(quoted.unwrap().0)
                }
                (Property::Named(name), None) => {
                    let name = self.property_spelling(name);
                    self.text(name)
                }
                // A computed key is an AssignmentExpression, so a
                // sequence needs parentheses: `{[(a,b)]:v}`.
                (Property::Computed(key), None) => {
                    self.text("[");
                    self.expression(*key, 2);
                    self.text("]");
                }
            }
            self.text(":");
            let inferred = match (key, literal) {
                (_, Some(name)) => InferredName::Known(name),
                (Property::Computed(_), None) if numeric.is_some() => {
                    InferredName::Known(numeric.unwrap())
                }
                (Property::Computed(_), None) if quoted.is_some() => {
                    InferredName::Known(quoted.unwrap().1)
                }
                (Property::Named(name), None) if name == "__proto__" => InferredName::None,
                (Property::Named(name), None) => InferredName::Known(name),
                (Property::Computed(_), None) => InferredName::Computed,
            };
            self.expression_with_name(*value, 2, inferred);
        }
        self.text("}");
    }

    fn named_function_expression(&mut self, id: FunctionId, name: &StringValue) {
        self.function_annotation(id);
        let function = &self.module.functions[id.index()];
        self.text(match function.suspension {
            Suspension::None => "function ",
            Suspension::Async => "async function ",
            Suspension::Generator => "function*",
        });
        self.text(
            name.as_unicode()
                .expect("a self-named function has an identifier name"),
        );
        self.function(id);
    }

    fn function_expression(&mut self, id: FunctionId) {
        self.function_annotation(id);
        let function = &self.module.functions[id.index()];
        self.text(match (function.arrow, function.suspension) {
            (false, Suspension::None) => "function",
            (false, Suspension::Async) => "async function",
            (false, Suspension::Generator) => "function*",
            (true, Suspension::Async) => "async",
            (true, _) => "",
        });
        self.function(id);
    }

    fn function(&mut self, id: FunctionId) {
        let activation=self.root_activation;
        if !self.module.functions[id.index()].arrow {self.root_activation=false;}
        self.function_inner(id);
        self.root_activation=activation;
    }

    fn function_inner(&mut self, id: FunctionId) {
        if !self.output.work(1) {
            return;
        }
        let defaults = &self.forms.defaults[id.index()].values;
        let absorbed = self.forms.defaults[id.index()].absorbed;
        let function = &self.module.functions[id.index()];
        // `a=>`: one plain parameter needs no parentheses. `async a=>` would
        // need a separating space, so only a plain arrow drops them.
        let bare = function.arrow
            && !function.rest
            && function.parameters.len() == 1
            && function.length.is_none()
            && function.suspension == Suspension::None;
        if !bare {
            self.text("(");
        }
        for (index, parameter) in function.parameters.iter().enumerate() {
            if !self.output.work(1) {
                return;
            }
            if index != 0 {
                self.text(",");
            }
            let rest = function.rest && index + 1 == function.parameters.len();
            if rest {
                self.text("...");
            }
            self.text(self.local(*parameter));
            if !rest && function.length.is_some_and(|length| index >= length) {
                match defaults.get(index).copied().flatten() {
                    Some(default) => {
                        self.text("=");
                        self.expression(default, 2);
                    }
                    // `length` counts the parameters before the first one
                    // with an initializer, so only that one needs one; a
                    // later parameter receives `undefined` either way.
                    None if function.length == Some(index) => self.text("=void 0"),
                    None => {}
                }
            }
        }
        if !bare {
            self.text(")");
        }
        if function.arrow {
            self.text("=>");
            // `=>(a,b,v)` is `=>{a;b;return v}` for plain expression
            // statements (admission folds both into one sequence).
            if let (false, [leading @ .., Statement::Return(Some(value))]) = (
                function.strict,
                &self.module.regions[function.body.index()].statements[absorbed..],
            ) {
                let module = self.module;
                let plain = |statement: &Statement| match statement {
                    Statement::Evaluate(value) => match &module.expressions[value.index()] {
                        Expr::Binary { op: Binary::And | Binary::Or, .. } => false,
                        Expr::Unary { op: Unary::Void, .. } => false,
                        Expr::Assign { target, value } => !(matches!(module.expressions[target.index()], Expr::Binding(_))
                            && matches!(module.expressions[value.index()], Expr::Conditional { .. })),
                        Expr::Sequence(_) => false,
                        _ => true,
                    },
                    _ => false,
                };
                if !leading.is_empty() && leading.iter().all(plain) {
                    self.text("(");
                    for statement in leading {
                        let Statement::Evaluate(item) = statement else { unreachable!() };
                        self.expression(*item, 2);
                        self.text(",");
                    }
                    self.expression(*value, 2);
                    self.text(")");
                    return;
                }
            }
            // `=>value` is `=>{return value}`. Its body cannot begin with `{`.
            if let (false, [Statement::Return(Some(value))]) = (
                function.strict,
                &self.module.regions[function.body.index()].statements[absorbed..],
            ) {
                let group = self.leading_object(*value, 2);
                if group {
                    self.text("(");
                }
                self.expression(*value, 2);
                if group {
                    self.text(")");
                }
                return;
            }
        }
        if function.strict {
            // The directive prologue must open the body.
            self.text("{\"use strict\";");
            if self.output.work(1) {
                self.statements(function.body, true);
            }
            self.text("}");
            return;
        }
        if !self.output.work(1) {
            return;
        }
        self.text("{");
        self.statements_from(function.body, true, absorbed);
        self.text("}");
    }

    /// Whether printing `id` at `minimum` precedence would begin with an
    /// object literal's `{`, which a concise arrow body cannot.
    fn leading_object(&mut self, id: ExprId, minimum: u8) -> bool {
        if !self.output.work(1) {
            return false;
        }
        let expression = &self.module.expressions[id.index()];
        if let Expr::ToInt32(value) = expression {
            if self.plain_integer(id) {
                return self.leading_object(*value, minimum);
            }
        }
        if self.precedence(id) < minimum {
            return false;
        }
        match expression {
            Expr::Object(_) => true,
            Expr::Call { .. } if self.forms.spread[id.index()] => true,
            Expr::Binary { op, left, .. } => self.leading_object(*left, op.precedence()),
            Expr::ToInt32(value) => self.leading_object(*value, Binary::BitOr.precedence()),
            Expr::IntBinary { op, left, .. } => {
                self.leading_object(*left, op.javascript().precedence())
            }
            Expr::Conditional { condition, .. } => self.leading_object(*condition, 4),
            Expr::Assign { target, .. } => self.leading_object(*target, 18),
            Expr::Sequence(values) => self.leading_object(values[0], 2),
            _ => false,
        }
    }

    fn region(&mut self, id: RegionId, braces: bool) {
        if !self.output.work(1) {
            return;
        }
        if braces {
            self.text("{");
        }
        self.statements(id, braces);
        if braces {
            self.text("}");
        }
    }

    /// A region's statements. `closing` says a `}` follows the last one, so
    /// its terminating `;` is implied. Adjacent declarations share one `let`.
    fn statements(&mut self, id: RegionId, closing: bool) {
        self.statements_from(id, closing, 0);
    }

    /// The region's statements from `start` on.
    fn statements_from(&mut self, id: RegionId, closing: bool, start: usize) {
        let statements = &self.module.regions[id.index()].statements;
        let mut declaring = false;
        let mut skip = start;
        for (index, statement) in statements.iter().enumerate() {
            if !self.output.work(1) {
                return;
            }
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let last = index + 1 == statements.len();
            if let Statement::Let { binding, value } = statement {
                self.text(if declaring { "," } else { "let " });
                self.text(self.local(*binding));
                if let Some(value) = value {
                    self.text("=");
                    if id == self.module.root { self.root_call_annotation(index, *value); }
                    self.expression_with_name(
                        *value,
                        2,
                        InferredName::Known(self.local(*binding)),
                    );
                }
                declaring = matches!(statements.get(index + 1), Some(Statement::Let { .. }));
                if !declaring && !(last && closing) {
                    self.text(";");
                }
                continue;
            }
            declaring = false;
            self.statement(statement, last && closing);
        }
    }

    /// Selected root statements, in order: adjacent declarations still share
    /// one `let`, and every statement keeps its `;`.
    fn statement_list(&mut self, statements: &[Statement], order: impl Iterator<Item = usize>) {
        let mut declaring = false;
        let mut order = order.peekable();
        while let Some(index) = order.next() {
            self.output.mark(None);
            if !self.output.work(1) {
                return;
            }
            if let Some((binding, function)) = self.inline_module.and_then(|_| inline::hoisted(self.module, &statements[index])) {
                self.statement(&Statement::Function { binding, function }, false);
                declaring = false;
                continue;
            }
            if let Statement::Let { binding, value } = &statements[index] {
                self.text(if declaring { "," } else { "let " });
                self.text(self.local(*binding));
                if let Some(value) = value {
                    self.text("=");
                    self.root_call_annotation(index, *value);
                    self.expression_with_name(
                        *value,
                        2,
                        InferredName::Known(self.local(*binding)),
                    );
                }
                declaring = order
                    .peek()
                    .is_some_and(|&next| matches!(statements[next], Statement::Let { .. }) && !(self.inline_module.is_some() && inline::hoisted(self.module, &statements[next]).is_some()));
                if !declaring {
                    self.text(";");
                }
                continue;
            }
            declaring = false;
            self.statement(&statements[index], false);
        }
        self.output.mark(None);
    }

    /// One statement; with `closing`, a following `}` implies its `;`.
    fn statement(&mut self, statement: &Statement, closing: bool) {
        self.output.mark(None);
        let end = |printer: &mut Self| {
            if !closing {
                printer.text(";");
            }
        };
        match statement {
            Statement::Let { .. } => unreachable!("declarations print as lists"),
            Statement::Evaluate(value) => {
                let value = self.discarded(*value);
                let group = self.statement_needs_group(value, 0);
                if group {
                    self.text("(");
                }
                self.expression(value, 0);
                if group {
                    self.text(")");
                }
                self.discarded_root = None;
                end(self);
            }
            Statement::Return(value) => {
                self.text("return");
                if let Some(value) = value {
                    let at = self.output.text.len();
                    self.expression(*value, 0);
                    self.output.separate_word(at);
                }
                end(self);
            }
            Statement::Throw(value) => {
                self.text("throw");
                let at = self.output.text.len();
                self.expression(*value, 0);
                self.output.separate_word(at);
                end(self);
            }
            Statement::If {
                condition,
                yes,
                no: None,
            } if self.forms.logical_assignments[yes.index()]
                .is_some_and(|form| form.condition == *condition) =>
            {
                let form = self.forms.logical_assignments[yes.index()].unwrap();
                self.expression(form.left, 18);
                self.text(form.op.token());
                self.text("=");
                self.expression(form.right, 2);
                end(self);
            }
            Statement::If { condition, yes, no }
                if no.is_none() && self.logical_statement(*condition, *yes) =>
            {
                end(self);
            }
            Statement::If { condition, yes, no } => {
                self.text("if(");
                self.expression(*condition, 0);
                self.text(")");
                self.body(*yes, no.is_some(), closing && no.is_none());
                if let Some(no) = no {
                    let statements = &self.module.regions[no.index()].statements;
                    // `else if` chains, and a single simple statement, need
                    // no braces; a keyword or name must not touch `else`.
                    if let [only] = statements.as_slice() {
                        if matches!(only, Statement::If { .. }) || Self::simple(only) {
                            self.text("else");
                            let at = self.output.text.len();
                            self.statement(only, closing);
                            self.output.separate_word(at);
                            return;
                        }
                    }
                    self.text("else");
                    self.region(*no, true);
                }
            }
            Statement::Loop {
                condition,
                update,
                body,
            } => {
                let while_form = condition.is_some() && update.is_none();
                self.text(if while_form { "while(" } else { "for(;" });
                if let Some(condition) = condition {
                    self.expression(*condition, 0);
                }
                if !while_form {
                    self.text(";");
                    if let Some(update) = update {
                        self.expression(*update, 0);
                    }
                }
                self.text(")");
                self.body(*body, false, closing);
            }
            Statement::Block(region) => {
                if !self.loop_head(*region, closing) {
                    self.region(*region, true);
                }
            }
            Statement::ForIn {
                binding,
                object,
                body,
            } => {
                self.text("for(let ");
                self.text(self.local(*binding));
                self.text(" in ");
                // A comma expression would end the `in` operand early.
                self.expression(*object, 3);
                self.text(")");
                self.body(*body, false, closing);
            }
            Statement::ForOf {
                binding,
                iterable,
                body,
            } => {
                self.text("for(let ");
                self.text(self.local(*binding));
                self.text(" of ");
                // The head takes an AssignmentExpression.
                self.expression(*iterable, 2);
                self.text(")");
                self.body(*body, false, closing);
            }
            Statement::Try {
                body,
                catch,
                finally,
            } => {
                self.text("try");
                self.region(*body, true);
                if let Some(catch) = catch {
                    self.text("catch");
                    if let Some(binding) = catch.binding {
                        self.text("(");
                        self.text(self.local(binding));
                        self.text(")");
                    }
                    self.region(catch.body, true);
                }
                if let Some(finally) = finally {
                    self.text("finally");
                    self.region(*finally, true);
                }
            }
            Statement::Break => {
                self.text("break");
                end(self);
            }
            Statement::Continue => {
                self.text("continue");
                end(self);
            }
            Statement::Function { binding, function } => {
                self.function_annotation(*function);
                self.text(match self.module.functions[function.index()].suspension {
                    Suspension::None => "function ",
                    Suspension::Async => "async function ",
                    Suspension::Generator => "function*",
                });
                self.text(self.local(*binding));
                self.function(*function);
            }
        }
    }

    /// Render a selected logical form; legality and site selection are complete.
    fn function_annotation(&mut self, function: FunctionId) {
        if self.module.consumer_annotations.functions()
            && self.module.discardable_functions.binary_search(&function).is_ok() {
            self.text("/*#__NO_SIDE_EFFECTS__*/");
        }
    }
    fn root_call_annotation(&mut self, index: usize, value: ExprId) {
        if self.module.consumer_annotations.calls()
            && self.module.root_rows.get(index).is_some_and(|row| row.anchor == Anchor::Definition && row.origin == RowOrigin::Source)
            && matches!(self.module.expressions[value.index()], Expr::Call { .. } | Expr::Construct { .. }) {
            self.text("/*#__PURE__*/");
        }
    }

    fn logical_statement(&mut self, condition: ExprId, yes: RegionId) -> bool {
        let Some(form) = self
            .forms
            .logical
            .get(yes.index())
            .copied()
            .flatten()
            .filter(|form| form.condition == condition)
        else {
            return false;
        };
        let level = form.op.precedence();
        let group = self.statement_needs_group(form.left, level);
        if group {
            self.text("(");
        }
        self.binary_operand(form.left, form.op, level);
        self.text(form.op.token());
        let right = self.discarded(form.right);
        self.binary_operand(right, form.op, level + 1);
        self.discarded_root = None;
        if group {
            self.text(")");
        }
        true
    }

    fn simple(statement: &Statement) -> bool {
        matches!(
            statement,
            Statement::Evaluate(_)
                | Statement::Return(_)
                | Statement::Throw(_)
                | Statement::Break
                | Statement::Continue
        )
    }

    /// The body of an `if` or loop. One statement needs no braces unless it
    /// declares a binding, or an `else` follows that a nested `if` would take.
    fn body(&mut self, id: RegionId, before_else: bool, closing: bool) {
        if !self.output.work(1) {
            return;
        }
        if let [only] = self.module.regions[id.index()].statements.as_slice() {
            let braceless = Self::simple(only)
                || !before_else
                    && matches!(
                        only,
                        Statement::If { .. }
                            | Statement::Loop { .. }
                            | Statement::ForIn { .. }
                            | Statement::ForOf { .. }
                    );
            if braceless {
                self.statement(only, closing && !before_else);
                return;
            }
        }
        self.region(id, true);
    }
}

#[cfg(test)]
#[path = "print_budget_tests.rs"]
mod budget_tests;

/// The shortest JavaScript spelling of a finite number: the shortest
/// round-trip digits, as a plain decimal without a leading zero (`.5`) or
/// with a decimal exponent (`1e3`, `15e-5`), whichever is shorter.
pub(super) fn number_spelling(value: f64) -> String {
    let mut result = String::new();
    write_number(&mut result, value).expect("number text");
    result
}

/// Length only; the shared formatter keeps its digit buffers on the stack.
pub(crate) fn number_spelling_length(value: f64) -> Option<usize> {
    if !value.is_finite() { return None; }
    crate::text_measure::measure(|out| write_number(out, value))
}

/// LowerExp's finite f64 spelling fits in 32 ASCII bytes. Keep its digits on
/// the stack and emit only the selected form; no discarded plain/scientific
/// strings or repeated-zero buffers need heap storage.
fn write_number(out: &mut impl std::fmt::Write, value: f64) -> std::fmt::Result {
    struct Scientific {
        bytes: [u8; 32],
        len: usize,
    }
    impl std::fmt::Write for Scientific {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let end = self
                .len
                .checked_add(text.len())
                .filter(|&end| end <= self.bytes.len())
                .ok_or(std::fmt::Error)?;
            self.bytes[self.len..end].copy_from_slice(text.as_bytes());
            self.len = end;
            Ok(())
        }
    }
    let mut scientific = Scientific {
        bytes: [0; 32],
        len: 0,
    };
    write!(&mut scientific, "{:e}", value.abs())?;
    let scientific =
        std::str::from_utf8(&scientific.bytes[..scientific.len]).expect("ASCII number");
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("finite LowerExp exponent");
    let exponent: i32 = exponent.parse().expect("LowerExp exponent");
    let mut digits = [0u8; 32];
    let mut len = 0;
    for byte in mantissa.bytes().filter(|&byte| byte != b'.') {
        digits[len] = byte;
        len += 1;
    }
    while len > 1 && digits[len - 1] == b'0' {
        len -= 1;
    }
    let digits = std::str::from_utf8(&digits[..len]).expect("ASCII digits");
    let point = exponent + 1;
    let count = len as i32;
    let shift = point - count;
    let plain_len = if point <= 0 {
        1 + (-point) as usize + len
    } else if point >= count {
        point as usize
    } else {
        len + 1
    };
    let magnitude = shift.unsigned_abs();
    let exponent_len = if magnitude < 10 {
        1
    } else if magnitude < 100 {
        2
    } else {
        3
    };
    let exponential_len = len + 1 + usize::from(shift < 0) + exponent_len;
    if value < 0.0 {
        out.write_str("-")?;
    }
    if shift != 0 && exponential_len < plain_len {
        write!(out, "{digits}e{shift}")
    } else if point <= 0 {
        out.write_str(".")?;
        for _ in 0..-point {
            out.write_str("0")?;
        }
        out.write_str(digits)
    } else if point >= count {
        out.write_str(digits)?;
        for _ in count..point {
            out.write_str("0")?;
        }
        Ok(())
    } else {
        write!(
            out,
            "{}.{}",
            &digits[..point as usize],
            &digits[point as usize..]
        )
    }
}

#[cfg(test)]
mod number_spelling_tests {
    use super::number_spelling;

    #[test]
    fn finite_number_spelling_round_trips_across_exponents_and_signs() {
        let mut bits = 0x7f4a7c159e3779b9u64;
        for _ in 0..20_000 {
            bits ^= bits << 13;
            bits ^= bits >> 7;
            bits ^= bits << 17;
            let value = f64::from_bits(bits);
            if value.is_finite() && value != 0.0 {
                let text = number_spelling(value);
                let parsed: f64 = text.parse().unwrap();
                assert_eq!(parsed.to_bits(), bits, "{text}");
            }
        }
    }

    #[test]
    fn numbers_take_their_shortest_exact_spelling() {
        for (value, spelling) in [
            (0.0, "0"),
            (1.0, "1"),
            (0.5, ".5"),
            (0.16, ".16"),
            (-0.7, "-.7"),
            (1000.0, "1e3"),
            (100.0, "100"),
            (400000.0, "4e5"),
            (1200.0, "1200"),
            (12000.0, "12e3"),
            (0.0001, "1e-4"),
            (0.00015, "15e-5"),
            (0.001, ".001"),
            (1.00375, "1.00375"),
            (2147483647.0, "2147483647"),
            (1e21, "1e21"),
            (123456789012.0, "123456789012"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "17976931348623157e292"),
        ] {
            assert_eq!(number_spelling(value), spelling, "{value}");
            // The spelling reads back as the same double.
            let read: f64 = spelling
                .strip_prefix('-')
                .map(|rest| -format!("0{rest}").parse::<f64>().unwrap())
                .unwrap_or_else(|| format!("0{spelling}").parse().unwrap());
            assert_eq!(read.to_bits(), value.to_bits(), "{spelling}");
        }
    }
}

#[path = "print_inline.rs"]
mod inline;
