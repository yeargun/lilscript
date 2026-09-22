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
            Self::Less | Self::LessEqual | Self::Greater | Self::GreaterEqual | Self::In => 10,
            Self::ShiftLeft | Self::ShiftRight | Self::UnsignedShiftRight => 11,
            Self::Add | Self::Subtract => 12,
            Self::Multiply | Self::Divide | Self::Remainder => 13,
        }
    }
}

fn precedence(expression: &Expr) -> u8 {
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
        Expr::Unary { .. } | Expr::Await(_) | Expr::Literal(Literal::Undefined) => 14,
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

pub(super) fn render(
    module: &Module,
    names: &Names,
    choices: Option<extract::JavaScriptChoices<'_>>,
) -> String {
    render_bounded(module, names, choices, usize::MAX).expect("unbounded output")
}

/// Error before a complete artifact is published. No partial text escapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrintError {
    Admission(AllocationError),
    ByteLimit,
}

pub(super) fn render_bounded(
    module: &Module,
    names: &Names,
    choices: Option<extract::JavaScriptChoices<'_>>,
    limit: usize,
) -> Result<String, String> {
    let mut budget = AllocationBudget::new(None);
    render_admitted(module, names, choices, limit, &mut budget).map_err(|error| match error {
        PrintError::ByteLimit => "render exceeds candidate byte budget".into(),
        PrintError::Admission(error) => format!("render admission failed: {error:?}"),
    })
}

/// The only printer, also used by inspection. Successful text retains its
/// capacity in the caller's budget; that owner must drop text before releasing
/// the retained allocation. Failed output is destroyed inside this scope.
pub(super) fn render_admitted(
    module: &Module,
    names: &Names,
    choices: Option<extract::JavaScriptChoices<'_>>,
    limit: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<String, PrintError> {
    render_with_literals_admitted(
        module,
        names,
        choices,
        &[],
        LiteralOutput::Original,
        limit,
        budget,
        None,
    )
}

pub(super) fn render_with_literals_admitted(
    module: &Module,
    names: &Names,
    choices: Option<extract::JavaScriptChoices<'_>>,
    literal_alternatives: &[LiteralAlternative],
    literals: LiteralOutput,
    limit: usize,
    budget: &mut AllocationBudget<'_>,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
) -> Result<String, PrintError> {
    let _timing = crate::timing::TARGET_PRINT.scope(0);
    let mut phase = budget.scope();
    let mut printer = Printer {
        module,
        names,
        choices,
        literal_alternatives,
        literals,
        output: Buffer {
            text: String::new(),
            budget: &mut phase,
            limit,
            error: None,
        },
        discarded_root: None,
        files: None,
    };
    for import in &module.imports {
        if !printer.output.work(1) {
            break;
        }
        if hosted(hosts, import) {
            continue;
        }
        printer.text("import{");
        printer.text(&import.imported);
        let local = names.get(import.binding);
        if !printer.output.work(local.len().min(import.imported.len())) {
            break;
        }
        if local != import.imported {
            printer.text(" as ");
            printer.text(local);
        }
        printer.text("}from");
        printer.string(&import.source);
        printer.text(";");
    }
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
    let Buffer { text, error, .. } = printer.output;
    if let Some(error) = error {
        drop(text);
        return Err(error);
    }
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

/// A delivered file prints in two parts: its body (statements and exports),
/// whose digest names a chunk, then the imports that name other files.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FilePart {
    Header,
    Body,
}

/// One delivered file of a module: imports from the other files, the
/// foreign imports it uses, its root statements, then its exports (and the
/// public exports, for the entry).
#[allow(clippy::too_many_arguments)]
pub(super) fn render_file_admitted(
    module: &Module,
    names: &Names,
    choices: Option<extract::JavaScriptChoices<'_>>,
    literal_alternatives: &[LiteralAlternative],
    literals: LiteralOutput,
    limit: usize,
    budget: &mut AllocationBudget<'_>,
    files: &[delivery::DeliveryFile],
    file: usize,
    links: &delivery::FileLinks,
    entry: bool,
    preload: &[String],
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
    part: FilePart,
) -> Result<String, PrintError> {
    let _timing = crate::timing::TARGET_PRINT.scope(0);
    let mut phase = budget.scope();
    let mut printer = Printer {
        module,
        names,
        choices,
        literal_alternatives,
        literals,
        output: Buffer {
            text: String::new(),
            budget: &mut phase,
            limit,
            error: None,
        },
        discarded_root: None,
        files: Some(files),
    };
    let header = part == FilePart::Header;
    if header && entry && !preload.is_empty() {
        // The default route's preload prelude, verbatim.
        printer.text("typeof document!=\"undefined\"&&[");
        for (index, file) in preload.iter().enumerate() {
            if index != 0 {
                printer.text(",");
            }
            let path = format!("./{file}");
            printer.string(&crate::literal::StringValue::from(path.as_str()));
        }
        printer.text("].forEach(a=>{let b=document.createElement(\"link\");b.rel=\"modulepreload\",b.href=a,document.head.append(b)});");
    }
    for (source, bindings) in links.imports.iter().filter(|_| header) {
        printer.text("import{");
        for (index, binding) in bindings.iter().enumerate() {
            if !printer.output.work(1) {
                break;
            }
            if index != 0 {
                printer.text(",");
            }
            printer.text(names.get(*binding));
        }
        printer.text("}from");
        let path = format!("./{}", files[*source].name);
        printer.string(&crate::literal::StringValue::from(path.as_str()));
        printer.text(";");
    }
    for &index in links.foreign.iter().filter(|_| header) {
        let import = &module.imports[index];
        if hosted(hosts, import) {
            continue;
        }
        printer.text("import{");
        printer.text(&import.imported);
        let local = names.get(import.binding);
        if local != import.imported {
            printer.text(" as ");
            printer.text(local);
        }
        printer.text("}from");
        printer.string(&import.source);
        printer.text(";");
    }
    if let Some(hosts) = hosts.filter(|_| header && entry) {
        printer.host_bindings(hosts, links.foreign.iter().copied());
    }
    let root = &module.regions[module.root.index()].statements;
    if !header {
        printer.statement_list(root, &files[file].statements);
    }
    if !header && (!links.exports.is_empty() || !links.namespace.is_empty()) {
        printer.text("export{");
        let mut first = true;
        for binding in &links.exports {
            if !printer.output.work(1) {
                break;
            }
            if !std::mem::take(&mut first) {
                printer.text(",");
            }
            printer.text(names.get(*binding));
        }
        // A lazy chunk's namespace members, under their export names.
        for (name, binding) in &links.namespace {
            if !printer.output.work(1) {
                break;
            }
            if links.exports.contains(binding) && names.get(*binding) == name {
                continue;
            }
            if !std::mem::take(&mut first) {
                printer.text(",");
            }
            let local = names.get(*binding);
            printer.text(local);
            if local != name {
                printer.text(" as ");
                if identifier_name(name) {
                    printer.text(name);
                } else {
                    printer.string(&crate::literal::StringValue::from(name.as_str()));
                }
            }
        }
        printer.text("};");
    }
    if !header && entry && !module.exports.is_empty() {
        printer.text("export{");
        for (index, export) in module.exports.iter().enumerate() {
            if !printer.output.work(1) {
                break;
            }
            if index != 0 {
                printer.text(",");
            }
            let local = names.get(export.binding);
            printer.text(local);
            if local != export.name {
                printer.text(" as ");
                printer.text(&export.name);
            }
        }
        printer.text("};");
    }
    let Buffer { text, error, .. } = printer.output;
    if let Some(error) = error {
        drop(text);
        return Err(error);
    }
    phase.finish_retained().map_err(PrintError::Admission)?;
    Ok(text)
}

struct Buffer<'a, 'ledger> {
    text: String,
    budget: &'a mut AllocationBudget<'ledger>,
    limit: usize,
    error: Option<PrintError>,
}
impl Buffer<'_, '_> {
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
    choices: Option<extract::JavaScriptChoices<'a>>,
    literal_alternatives: &'a [LiteralAlternative],
    literals: LiteralOutput,
    output: Buffer<'budget, 'ledger>,
    /// The statement value being printed without its normalization.
    discarded_root: Option<ExprId>,
    /// The delivered files, when this prints one of several.
    files: Option<&'a [delivery::DeliveryFile]>,
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
    /// `let{a:x,b}=<host modules>;` binding each carried import among
    /// `imports` (indices into the module's imports), before any statement,
    /// as ES evaluates imported modules first.
    fn host_bindings(
        &mut self,
        (hosts, strict): (&crate::host_modules::HostDelivery, bool),
        imports: impl Iterator<Item = usize>,
    ) {
        let mut groups: Vec<Vec<usize>> = vec![Vec::new(); hosts.modules.len()];
        for index in imports {
            let import = &self.module.imports[index];
            if let Some(position) = import.source.as_unicode().and_then(|source| hosts.position(source)) {
                if !groups[position].contains(&index) {
                    groups[position].push(index);
                }
            }
        }
        if groups.iter().all(Vec::is_empty) {
            return;
        }
        let pattern = |printer: &mut Self, group: &[usize]| {
            printer.text("{");
            for (position, &index) in group.iter().enumerate() {
                if position != 0 {
                    printer.text(",");
                }
                let import = &printer.module.imports[index];
                let local = printer.names.get(import.binding);
                if identifier_name(&import.imported) {
                    printer.text(&import.imported);
                } else {
                    printer.string(&StringValue::from(import.imported.as_str()));
                }
                if local != import.imported {
                    printer.text(":");
                    printer.text(local);
                }
            }
            printer.text("}");
        };
        self.text("let");
        if let Some(single) = hosts.single(strict) {
            pattern(self, &groups[0]);
            self.text("=");
            self.text(&single);
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
            self.text(&hosts.expression(strict));
        }
        self.text(";");
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

    fn plain_integer(&self, id: ExprId) -> bool {
        self.discarded_root == Some(id)
            || self
                .choices
                .is_some_and(|choices| choices.plain_integer[id.index()])
            || self.module.pristine_builtins
                && matches!(
                    self.module.expressions[id.index()],
                    Expr::Intrinsic { operation, .. } if pristine_int32_intrinsic(operation)
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
                        Expr::IntBinary { .. } | Expr::IntNegate(_) | Expr::Literal(Literal::Number(_))
                    ) =>
                {
                    id = value
                }
                Expr::IntBinary { .. } | Expr::IntNegate(_) => {
                    self.discarded_root = Some(id);
                    return id;
                }
                // An unpatched integer method returns a number: `|0` on it
                // has no effect when the value is discarded.
                Expr::Intrinsic { operation, .. }
                    if self.module.pristine_builtins && integer_intrinsic(operation) =>
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
            _ => precedence(expression),
        }
    }
    fn string(&mut self, value: &StringValue) {
        self.text("\"");
        self.string_content(value, false);
        self.text("\"");
    }

    fn string_content(&mut self, value: &StringValue, template: bool) {
        // Charge decoding/escaping before scanning; the writer separately
        // admits each emitted byte and any output-buffer relocation.
        if !self.output.work(value.storage_bytes()) {
            return;
        }
        let after_dollar = self.output.text.ends_with('$');
        let _ = crate::js_string::contents(
            &mut self.output,
            value,
            if template { '`' } else { '"' },
            after_dollar,
        );
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

    fn property(&mut self, property: &Property) {
        match property {
            Property::Named(name) => {
                self.text(".");
                self.text(name);
            }
            Property::Computed(key) => {
                // `o["name"]` and `o.name` read the same property.
                if let Some(name) = self.identifier_key(*key) {
                    self.text(".");
                    self.text(name);
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

    fn receiver(&mut self, value: ExprId) {
        let force = self.observed_literal(value).is_some()
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
        let expression = &self.module.expressions[id.index()];
        if let Expr::Function(function) = expression {
            if let Some(name) = extract::function_name(self.module, *function, self.choices) {
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
                if !matches {
                    // A sequence suppresses accidental named evaluation and
                    // produces a value, never a property reference receiver.
                    self.text("(0,");
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
                    self.text(")");
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
                    } else {
                        let _ = write!(self.output, "{value}");
                    }
                }
                Literal::String(value) => {
                    if let Some(truthy) = self.observed_literal(id) {
                        self.text(if truthy { "1" } else { "0" });
                    } else {
                        self.string(value);
                    }
                }
                Literal::Bool(value) => self.text(if *value { "true" } else { "false" }),
                Literal::Null => self.text("null"),
                Literal::Undefined => self.text("void 0"),
            },
            Expr::Binding(symbol) => self.text(self.names.get(*symbol)),
            Expr::Host(name) => self.text(name),
            Expr::This => self.text("this"),
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
                self.arguments(arguments);
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
            Expr::Binary { op, left, right } => {
                let level = op.precedence();
                self.binary_operand(*left, *op, level);
                self.text(op.token());
                // A sign can otherwise join the binary token into ++ or --.
                let at = self.output.text.len();
                self.binary_operand(*right, *op, level + 1);
                self.output.separate_sign(at);
            }
            Expr::Member { object, property } => {
                self.receiver(*object);
                self.property(property);
            }
            Expr::Call {
                callee,
                arguments,
                invocation,
            } => {
                let callee_node = &self.module.expressions[callee.index()];
                let unbind = *invocation == Invocation::Value
                    && match callee_node {
                        Expr::Member { .. } => true,
                        Expr::Host(name) => name == "eval",
                        Expr::Binding(symbol) => self.names.get(*symbol) == "eval",
                        _ => false,
                    };
                let function_literal = matches!(callee_node, Expr::Function(_));
                if unbind {
                    self.text("(0,");
                } else if function_literal {
                    self.text("(");
                }
                self.expression(*callee, if unbind { 2 } else { 17 });
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
                self.arguments(arguments);
            }
            Expr::Conditional { condition, yes, no } => {
                self.expression(*condition, 4);
                self.text("?");
                self.expression(*yes, 2);
                self.text(":");
                self.expression(*no, 2);
            }
            Expr::Assign { target, value } => {
                self.expression(*target, 18);
                self.text("=");
                let inferred = match &self.module.expressions[target.index()] {
                    Expr::Binding(binding) => InferredName::Known(self.names.get(*binding)),
                    Expr::Host(name) => InferredName::Known(name),
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
                        TemplatePart::String(value) => self.string_content(value, true),
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
                let chunk = self.files.and_then(|files| {
                    files
                        .iter()
                        .find(|file| file.lazy && file.modules.first() == Some(module))
                });
                if let Some(chunk) = chunk {
                    // The chunk's own namespace; a failed load reports the
                    // source specifier, as the default route does.
                    self.text("import(");
                    let path = format!("./{}", chunk.name);
                    self.string(&StringValue::from(path.as_str()));
                    self.text(").catch(e=>");
                    self.expression(*promise, 18);
                    self.text(".reject({specifier:");
                    self.string(&StringValue::from(specifier.as_str()));
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
                        if identifier_name(name) {
                            self.text(name);
                        } else {
                            self.string(&StringValue::from(name.as_str()));
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
            Expr::Object(entries) => {
                self.text("{");
                for (index, (key, value)) in entries.iter().enumerate() {
                    if !self.output.work(1) {
                        return;
                    }
                    if index != 0 {
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
                    match (key, literal) {
                        (_, Some(name)) => self.text(name),
                        (Property::Named(name), None) => self.text(name),
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
                        (Property::Named(name), None) if name == "__proto__" => InferredName::None,
                        (Property::Named(name), None) => InferredName::Known(name),
                        (Property::Computed(_), None) => InferredName::Computed,
                    };
                    self.expression_with_name(*value, 2, inferred);
                }
                self.text("}");
            }
            Expr::Function(function) => {
                self.function_expression(*function);
            }
            Expr::Class {
                name,
                base,
                constructor,
            } => {
                self.text("class ");
                self.text(name);
                self.text(" extends ");
                // The heritage is a LeftHandSideExpression.
                self.expression(*base, 18);
                self.text("{constructor");
                self.function(*constructor);
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

    fn function_expression(&mut self, id: FunctionId) {
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
        if !self.output.work(1) {
            return;
        }
        let function = &self.module.functions[id.index()];
        // `a=>`: one plain parameter needs no parentheses. `async a=>` would
        // need a separating space, so only a plain arrow drops them.
        let bare = function.arrow
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
            self.text(self.names.get(*parameter));
            if function.length.is_some_and(|length| index >= length) {
                self.text("=void 0");
            }
        }
        if !bare {
            self.text(")");
        }
        if function.arrow {
            self.text("=>");
            // `=>value` is `=>{return value}`. Its body cannot begin with `{`.
            if let (false, [Statement::Return(Some(value))]) = (
                function.strict,
                self.module.regions[function.body.index()].statements.as_slice(),
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
        self.region(function.body, true);
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
        let statements = &self.module.regions[id.index()].statements;
        let mut declaring = false;
        for (index, statement) in statements.iter().enumerate() {
            if !self.output.work(1) {
                return;
            }
            let last = index + 1 == statements.len();
            if let Statement::Let { binding, value } = statement {
                self.text(if declaring { "," } else { "let " });
                self.text(self.names.get(*binding));
                if let Some(value) = value {
                    self.text("=");
                    self.expression_with_name(
                        *value,
                        2,
                        InferredName::Known(self.names.get(*binding)),
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
    fn statement_list(&mut self, statements: &[Statement], order: &[usize]) {
        let mut declaring = false;
        for (position, &index) in order.iter().enumerate() {
            if !self.output.work(1) {
                return;
            }
            if let Statement::Let { binding, value } = &statements[index] {
                self.text(if declaring { "," } else { "let " });
                self.text(self.names.get(*binding));
                if let Some(value) = value {
                    self.text("=");
                    self.expression_with_name(
                        *value,
                        2,
                        InferredName::Known(self.names.get(*binding)),
                    );
                }
                declaring = order
                    .get(position + 1)
                    .is_some_and(|&next| matches!(statements[next], Statement::Let { .. }));
                if !declaring {
                    self.text(";");
                }
                continue;
            }
            declaring = false;
            self.statement(&statements[index], false);
        }
    }

    /// One statement; with `closing`, a following `}` implies its `;`.
    fn statement(&mut self, statement: &Statement, closing: bool) {
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
                    self.text(" ");
                    self.expression(*value, 0);
                }
                end(self);
            }
            Statement::Throw(value) => {
                self.text("throw ");
                self.expression(*value, 0);
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
                            self.text("else ");
                            self.statement(only, closing);
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
            Statement::Block(region) => self.region(*region, true),
            Statement::ForIn {
                binding,
                object,
                body,
            } => {
                self.text("for(let ");
                self.text(self.names.get(*binding));
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
                self.text(self.names.get(*binding));
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
                        self.text(self.names.get(binding));
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
                self.text(match self.module.functions[function.index()].suspension {
                    Suspension::None => "function ",
                    Suspension::Async => "async function ",
                    Suspension::Generator => "function*",
                });
                self.text(self.names.get(*binding));
                self.function(*function);
            }
        }
    }

    /// Statements that complete in one piece: none declares a binding or can
    /// end in an `if` that would capture a following `else`.
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
