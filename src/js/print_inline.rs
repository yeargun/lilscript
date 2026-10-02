//! Single-file module activations. A generator is instantiated to its first
//! yield before dependencies evaluate, so ordinary lexical cells retain their
//! TDZ and function declarations are available throughout a static cycle.
//! Only lazy source statements move into these activations. Eager public cells
//! remain native root bindings, including their live exports.
use super::*;
use crate::js::delivery::{InlineModule, PlannedFile};

pub(super) struct InlineView<'a> {
    pub prefix: &'a str,
    pub owners: &'a [(BindingId, u32)],
    pub modules: &'a [InlineModule],
    pub activations: bool,
}
impl InlineView<'_> {
    pub fn owner(&self, binding: BindingId, current: Option<u32>) -> Option<u32> {
        let index = self
            .owners
            .binary_search_by_key(&binding, |(binding, _)| *binding)
            .ok()?;
        let owner = self.owners[index].1;
        (Some(owner) != current).then_some(owner)
    }
    fn deferred(&self, row: RootRow) -> bool {
        self.activations
            && row.origin != RowOrigin::Synthetic
            && self
                .modules
                .iter()
                .any(|part| part.source == row.module && !part.eager)
    }
}

pub(super) fn hoisted(module: &Module, statement: &Statement) -> Option<(BindingId, FunctionId)> {
    let Statement::Let {
        binding,
        value: Some(value),
    } = *statement
    else {
        return None;
    };
    if module.settled.get(binding.index()).copied().flatten() != Some(0) {
        return None;
    }
    match module.expressions[value.index()] {
        Expr::Function(function) => Some((binding, function)),
        _ => None,
    }
}

impl Printer<'_, '_, '_> {
    pub(super) fn member_binding(&self, binding: BindingId) -> bool {
        self.container.is_some_and(|view| view.imported(binding))
            || self
                .inline
                .is_some_and(|view| view.owner(binding, self.inline_module).is_some())
    }
    /// Runtime identifiers all share a prefix proven disjoint from allocated
    /// bindings, exact names and host references. No author text is substituted.
    fn inline_text(&mut self, text: &str) {
        let prefix = self.inline.expect("inline module recipe").prefix;
        for (index, part) in text.split('$').enumerate() {
            if index != 0 {
                self.text(prefix);
            }
            self.text(part);
        }
    }
    fn inline_cell(&mut self, source: u32) {
        self.text(self.inline.unwrap().prefix);
        let _ = write!(self.output, "c{source}");
    }
}

pub(super) fn body(printer: &mut Printer<'_, '_, '_>, file: &PlannedFile,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>) {
    if file.initializers.iter().any(|part| part.host.is_some()) {
        let Some(hosts) = hosts else { printer.output.error = Some(PrintError::Container("missing ordered host bodies")); return; };
        ordered_body(printer, file, hosts);
        return;
    }
    if let Some(view) = printer.inline.filter(|view| view.activations || view.modules.iter().any(|part| part.publish)) {
        if view.activations {
            printer.inline_text("let $i=[],$f=[],$d=[],$s=[],$e=[],$n=[],$p=[];");
        } else {
            printer.inline_text("let $n=[],$p=[];");
        }
        for part in view.modules {
            if view.activations {
                printer.text("let ");
                printer.inline_cell(part.source);
                printer.text(";");
                printer.inline_text("$d[");
                let _ = write!(printer.output, "{}]=[", part.source);
                for (index, dependency) in part.dependencies.iter().enumerate() {
                    if index != 0 {
                        printer.text(",");
                    }
                    let _ = write!(printer.output, "{dependency}");
                }
                printer.text("];");
                if part.eager {
                    printer.inline_text("$s[");
                    let _ = write!(printer.output, "{}]=2;", part.source);
                }
                printer.inline_text("$f[");
                let _ = write!(printer.output, "{}]=function*(){{", part.source);
                printer.inline_cell(part.source);
                printer.text("={");
                let mut first = true;
                for &index in &file.statements {
                    let row = printer.module.root_rows[index as usize];
                    if row.module != part.source || row.origin == RowOrigin::Synthetic {
                        continue;
                    }
                    let binding = match printer.module.regions[printer.module.root.index()]
                        .statements[index as usize]
                    {
                        Statement::Let { binding, .. } | Statement::Function { binding, .. } => {
                            binding
                        }
                        _ => continue,
                    };
                    if !std::mem::take(&mut first) {
                        printer.text(",");
                    }
                    // Accessor spelling prevents __proto__ from changing the
                    // namespace prototype. Setter parameters use the reserved prefix.
                    printer.text("get[");
                    printer.unicode_string(printer.names.get(binding));
                    printer.text("](){return ");
                    printer.text(printer.local(binding));
                    printer.text("},set[");
                    printer.unicode_string(printer.names.get(binding));
                    printer.inline_text("]($v){");
                    printer.text(printer.local(binding));
                    printer.inline_text("=$v}");
                }
                printer.text("};yield;");
                if !part.eager {
                    printer.inline_module = Some(part.source);
                    core(printer, file, Some(part.source));
                    printer.inline_module = None;
                }
                printer.text("};");
            }
            if !part.publish { continue; }
            printer.inline_text("$p[");
            let _ = write!(
                printer.output,
                "{}]=()=>Object.freeze({{__proto__:null",
                part.source
            );
            for (name, binding) in &part.members {
                printer.text(",get[");
                printer.unicode_string(name);
                printer.text("](){return ");
                printer.binding_read(*binding);
                printer.text("}");
            }
            printer.text("});");
        }
        if view.activations {
            printer.inline_text("function $a(i){if(!$i[i]){let g=$i[i]=$f[i]();g.next();for(let j of $d[i])$a(j)}}function $v(i){if($s[i]===3)throw $e[i];if($s[i])return;$s[i]=1;try{for(let j of $d[i])$v(j);$i[i].next();$s[i]=2}catch(e){$s[i]=3;$e[i]=e;throw e}}function $l(i){$a(i);$v(i);return $n[i]||($n[i]=$p[i]())}");
        } else {
            printer.inline_text("function $l(i){return $n[i]||($n[i]=$p[i]())}");
        }
    }
    core(printer, file, None);
}

fn core(printer: &mut Printer<'_, '_, '_>, file: &PlannedFile, source: Option<u32>) {
    use AllocationClass::Scratch;
    let Some(mut statements) = printer
        .output
        .admit(|budget| budget.vector(Scratch, file.statements.len()))
    else {
        return;
    };
    for &index in &file.statements {
        let row = printer.module.root_rows[index as usize];
        let selected = match (printer.inline, source) {
            (Some(view), Some(source)) => view.deferred(row) && row.module == source,
            (Some(view), None) => !view.deferred(row),
            (None, _) => true,
        };
        if selected {
            statements.push(index);
        }
    }
    core_statements(printer, file, source, &statements);
    printer.output.drop_vec(statements, Scratch);
}

fn core_statements(printer: &mut Printer<'_, '_, '_>, file: &PlannedFile,
    source: Option<u32>, statements: &[u32]) {
    use AllocationClass::{Retained, Scratch};
    if statements.is_empty() { return; }
    let start = printer.output.text.len();
    let root = &printer.module.regions[printer.module.root.index()].statements;
    printer.statement_list(root, statements.iter().map(|&index| index as usize));
    let Some(mut members) = printer
        .output
        .admit(|budget| budget.vector(Scratch, printer.module.bindings.len()))
    else {
        return;
    };
    for index in 0..printer.module.bindings.len() {
        let binding = BindingId::new(index);
        // A direct-default interop import is a local value, not a member read.
        let container = printer.container.is_some_and(|view| view.member(binding));
        let inline = printer
            .inline
            .is_some_and(|view| view.owner(binding, source).is_some());
        if container || inline {
            members.push(binding);
        }
    }
    if !printer.output.work(
        printer.module.expressions.len()
            + printer.module.regions.len()
            + printer.module.bindings.len()
            + file.statements.len(),
    ) {
        return;
    }
    let expected = crate::js::admission::planned_core_digest(
        printer.module,
        statements,
        &members,
        printer.lazy,
        printer.container.is_some_and(|view| view.commonjs),
        printer.container.is_some_and(|view| view.strict),
        printer.inline.is_some(),
        source.is_some(),
    );
    let part = files::StructurePart {
        start,
        end: printer.output.text.len(),
        expected,
    };
    printer
        .output
        .admit(|budget| budget.push(Retained, &mut printer.planned_structure.parts, part));
    printer.output.drop_vec(members, Scratch);
}

/// Static opaque bodies keep their exact graph positions. Hoisted source
/// functions and synthetic helpers remain available before any initialization;
/// lexical source cells and immutable host import cells initialize in order.
/// Each shared host dependency has one namespace in this entry's instance.
fn ordered_body(printer: &mut Printer<'_, '_, '_>, file: &PlannedFile,
    (hosts, strict): (&crate::host_modules::HostDelivery, bool)) {
    use AllocationClass::{Retained, Scratch};
    let Some(view) = printer.inline else { printer.output.error = Some(PrintError::Container("missing ordered source graph")); return; };
    if view.activations || view.modules.iter().any(|part| !part.eager || part.publish) {
        printer.output.error = Some(PrintError::Container("opaque lazy modules require split delivery")); return;
    }
    let root = &printer.module.regions[printer.module.root.index()].statements;
    let Some(mut pending) = printer.output.admit(|budget| budget.vector(Scratch, file.statements.len())) else { return; };
    let Some(mut done) = printer.output.admit(|budget| budget.filled(Scratch, root.len(), false)) else { return; };
    for &index in &file.statements {
        let row = printer.module.root_rows[index as usize];
        if row.origin == RowOrigin::Synthetic || matches!(root[index as usize], Statement::Function { .. })
            || hoisted(printer.module, &root[index as usize]).is_some() {
            done[index as usize] = true;
            pending.push(index);
        }
    }
    core_statements(printer, file, None, &pending);
    pending.clear();
    for part in view.modules {
        if let Some(host) = part.host {
            let Some(local) = printer.output.admit(|budget| budget.format(Scratch, format_args!("{}h{host}", view.prefix))) else { return; };
            let Some(mut arguments) = printer.output.admit(|budget| budget.vector(Scratch, hosts.modules[host].imports().len())) else { return; };
            for &(dependency, _) in hosts.modules[host].imports() {
                // Several import/re-export declarations can name one module.
                // It still has one namespace argument and one formal parameter.
                if arguments.iter().any(|(index, _)| *index == dependency) { continue; }
                let Some(name) = printer.output.admit(|budget| budget.format(Scratch, format_args!("{}h{dependency}", view.prefix))) else { return; };
                arguments.push((dependency, name));
            }
            let Some(expression) = printer.output.admit(|budget| hosts.module_expression_in(host, strict, &arguments, budget)) else { return; };
            printer.text("let "); printer.text(&local); printer.text("="); printer.text(&expression); printer.text(";");
            printer.output.drop_string(expression, Retained);
            for &index in &file.links.hosted {
                let import = &printer.module.imports[index];
                if import.source.as_unicode() != Some(hosts.modules[host].specifier.as_str()) || import.imported.is_empty() { continue; }
                printer.text("let "); printer.text(printer.local(import.binding)); printer.text("="); printer.text(&local);
                printer.text("["); printer.unicode_string(&import.imported); printer.text("];");
            }
            for (_, name) in arguments.drain(..) { printer.output.drop_string(name, Scratch); }
            printer.output.drop_vec(arguments, Scratch);
            printer.output.drop_string(local, Scratch);
        }
        for &index in &file.statements {
            if !printer.output.work(1) { return; }
            if !done[index as usize] && printer.module.root_rows[index as usize].module == part.source {
                done[index as usize] = true; pending.push(index);
            }
        }
        core_statements(printer, file, None, &pending);
        pending.clear();
    }
    if file.statements.iter().any(|&index| !done[index as usize]) {
        printer.output.error = Some(PrintError::Container("ordered host plan lost a source statement"));
    }
    printer.output.drop_vec(done, Scratch);
    printer.output.drop_vec(pending, Scratch);
}
