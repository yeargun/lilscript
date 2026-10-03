//! Script containers for the same verified delivery plan. Namespaces retain
//! live getters; imports are property reads, never snapshots. All wrapper,
//! linkage and interoperability bytes pass through the normal budget/scorer.
use super::*;
use crate::config::{CjsExports, EsModuleMarker, GlobalBinding, JavaScriptFormat};
use crate::js::delivery::FileRole;

pub(super) struct ImportRead {
    binding: BindingId,
    request: usize,
    name: String,
    default_interop: bool,
}
pub(super) struct ContainerView<'a> {
    pub prefix: &'a str,
    pub imports: &'a [ImportRead],
    pub commonjs: bool,
    pub strict: bool,
}
impl ContainerView<'_> {
    pub fn member(&self, binding: BindingId) -> bool {
        self.imports.binary_search_by_key(&binding, |read| read.binding)
            .is_ok_and(|index| !self.imports[index].default_interop)
    }
    pub fn imported(&self, binding: BindingId) -> bool {
        self.imports
            .binary_search_by_key(&binding, |read| read.binding)
            .is_ok()
    }
}
impl Printer<'_, '_, '_> {
    pub(super) fn binding_read(&mut self, binding: BindingId) {
        if let Some((prefix, owner)) = self.inline.and_then(|view| view.owner(binding, self.inline_module).map(|owner| (view.prefix, owner))) {
            self.text(prefix);
            let _ = write!(self.output, "c{owner}[");
            self.unicode_string(self.names.get(binding)); self.text("]");
            return;
        }
        let view = self.container;
        let read = view.and_then(|view| {
            view.imports
                .binary_search_by_key(&binding, |read| read.binding)
                .ok()
                .map(|index| (&view.imports[index], view.prefix))
        });
        let Some((read, prefix)) = read else {
            self.text(self.local(binding));
            return;
        };
        if !self
            .output
            .work((usize::BITS - read.binding.index().leading_zeros()) as usize + 1)
        {
            return;
        }
        if read.default_interop {
            self.text(prefix);
            let _ = write!(self.output, "{}", read.request);
        } else {
            self.text(prefix);
            let _ = write!(self.output, "{}", read.request);
            self.text("[");
            self.unicode_string(&read.name);
            self.text("]");
        }
    }
}

struct Request {
    path: String,
    global: Option<String>,
}
struct Published {
    name: String,
    binding: BindingId,
}

pub(super) fn render(
    printer: &mut Printer<'_, '_, '_>,
    planned: &files::PlannedPrint<'_>,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
) {
    // The local view borrows admitted storage; restore the caller's empty view
    // before returning and release all scratch explicitly.
    let result = render_inner(printer, planned, hosts);
    if let Err(error) = result {
        printer.output.error.get_or_insert(error);
    }
}
fn render_inner(
    printer: &mut Printer<'_, '_, '_>,
    planned: &files::PlannedPrint<'_>,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
) -> Result<(), PrintError> {
    use AllocationClass::Scratch;
    let plan = planned.plan;
    let file = &plan.files[planned.file];
    let config = &plan.container;
    let mut requests: Vec<Request> = Vec::new();
    let mut reads: Vec<ImportRead> = Vec::new();
    let mut exports: Vec<Published> = Vec::new();
    let mut global = None;
    let mut prefix = String::new();
    let budget = &mut printer.output;
    macro_rules! admit {
        ($expr:expr) => {
            budget
                .admit($expr)
                .ok_or_else(|| budget.error.unwrap_or(PrintError::ByteLimit))?
        };
    }
    if let Some(template) = config.global.as_deref() {
        if matches!(plan.format, JavaScriptFormat::Iife | JavaScriptFormat::Umd) {
            let FileRole::Entry(entry) = file.role else {
                return Err(PrintError::Container(
                    "global containers require one file per entry",
                ));
            };
            let name = admit!(|b| crate::js::names::expand_in(
                template,
                &crate::js::names::Fields {
                    name: &plan.entry_names[entry as usize],
                    index: entry as usize,
                    path: "",
                    ext: "",
                    hash: ""
                },
                Scratch,
                b
            ));
            if !identifier(&name)
                || matches!(name.as_str(), "eval" | "arguments")
                || (config.global_binding == GlobalBinding::Var
                    && matches!(name.as_str(), "module" | "exports" | "require" | "define" | "Object" | "globalThis" | "self"))
            {
                return Err(PrintError::Container("expanded delivery.global is not a safe publication identifier; use a namespace prefix or [index]"));
            }
            global = Some(name);
        }
    }
    admit!(|b| b.push_str(Scratch, &mut prefix, "$"));
    loop {
        if !budget.work(
            printer.module.bindings.len()
                + printer.module.expressions.len()
                + printer.module.reserved.len(),
        ) {
            return Err(budget.error.unwrap());
        }
        let collision = printer
            .module
            .bindings
            .iter()
            .enumerate()
            .any(|(index, _)| {
                printer
                    .names
                    .get(BindingId::new(index))
                    .starts_with(&prefix)
            })
            || printer
                .module
                .expressions
                .iter()
                .any(|expr| matches!(expr,Expr::Host(host) if host.name.starts_with(&prefix)))
            || printer
                .module
                .reserved
                .iter()
                .any(|name| name.starts_with(&prefix));
        if !collision {
            break;
        }
        admit!(|b| b.push_str(Scratch, &mut prefix, "$"));
    }
    for (source, bindings) in &file.links.imports {
        let path = admit!(|b| crate::js::names::specifier_in(
            &planned.names[planned.file],
            &planned.names[*source as usize],
            Scratch,
            b
        ));
        let request = requests.len();
        admit!(|b| b.push(Scratch, &mut requests, Request { path, global: None }));
        for &binding in bindings {
            let name = admit!(|b| b.string(Scratch, printer.names.get(binding)));
            admit!(|b| b.push(
                Scratch,
                &mut reads,
                ImportRead {
                    binding,
                    request,
                    name,
                    default_interop: false
                }
            ));
        }
        for (_, binding, from) in &file.links.public {
            if from == source && !bindings.contains(binding) {
                let name = admit!(|b| b.string(Scratch, printer.names.get(*binding)));
                admit!(|b| b.push(
                    Scratch,
                    &mut reads,
                    ImportRead {
                        binding: *binding,
                        request,
                        name,
                        default_interop: false
                    }
                ));
            }
        }
    }
    // Keep the same first-occurrence ordering as the ESM printer.
    let mut sources: Vec<usize> = Vec::new();
    for &index in &file.links.foreign {
        let import = &printer.module.imports[index];
        if hosted(hosts, import) {
            continue;
        }
        let existing = sources
            .iter()
            .position(|&i| printer.module.imports[i].source == import.source);
        let offset = file.links.imports.len();
        let request = if let Some(index) = existing {
            offset + index
        } else {
            let source = import.source.as_unicode().ok_or(PrintError::Container(
                "container imports require a Unicode module specifier",
            ))?;
            let delivered_source = config.external_specifier(source);
            let rebased =
                admit!(|b| files::rebased_in(delivered_source, &planned.names[planned.file], Scratch, b));
            let path = match rebased {
                Some(path) => path,
                None => admit!(|b| b.string(Scratch, delivered_source)),
            };
            let global = config
                .globals
                .iter()
                .find(|(known, _)| known == source)
                .map(|(_, value)| value);
            if matches!(plan.format, JavaScriptFormat::Iife | JavaScriptFormat::Umd)
                && global.is_none()
            {
                return Err(PrintError::Container(
                    "IIFE/UMD foreign imports require a delivery.globals mapping",
                ));
            }
            let global = global
                .map(|value| {
                    budget
                        .admit(|b| b.string(Scratch, value))
                        .ok_or(PrintError::ByteLimit)
                })
                .transpose()?;
            let request = requests.len();
            admit!(|b| b.push(Scratch, &mut requests, Request { path, global }));
            admit!(|b| b.push(Scratch, &mut sources, index));
            request
        };
        if import.imported.is_empty() { continue; }
        let name = admit!(|b| b.string(Scratch, &import.imported));
        admit!(|b| b.push(
            Scratch,
            &mut reads,
            ImportRead {
                binding: import.binding,
                request,
                name,
                default_interop: import.imported == "default"
                    && config.default_interop == crate::config::DefaultInterop::Node
            }
        ));
    }
    for &binding in &file.links.exports {
        let name = admit!(|b| b.string(Scratch, printer.names.get(binding)));
        admit!(|b| b.push(Scratch, &mut exports, Published { name, binding }));
    }
    for (name, binding, _) in &file.links.public {
        if exports
            .iter()
            .any(|export| export.name == *name && export.binding == *binding)
        {
            continue;
        }
        let name = admit!(|b| b.string(Scratch, name));
        admit!(|b| b.push(
            Scratch,
            &mut exports,
            Published {
                name,
                binding: *binding
            }
        ));
    }
    if !budget.work(reads.len().saturating_mul(usize::BITS as usize)) {
        return Err(budget.error.unwrap());
    }
    reads.sort_by_key(|read| read.binding);
    let mut index = 1;
    while index < reads.len() {
        if reads[index - 1].binding == reads[index].binding {
            let removed = reads.remove(index);
            budget.drop_string(removed.name, Scratch);
        } else {
            index += 1;
        }
    }
    let default_only =
        config.exports == CjsExports::Default && matches!(file.role, FileRole::Entry(_));
    if default_only && (exports.len() != 1 || exports[0].name != "default") {
        return Err(PrintError::Container(
            "delivery.exports = default requires exactly one default export per entry",
        ));
    }
    if default_only {
        let binding = exports[0].binding;
        // Replacing module.exports cannot express a later reassignment. Keep
        // the explicit direct-default surface limited to settled bindings.
        if printer.module.expressions.iter().any(|expr| match expr {
            Expr::Assign{target,..} => matches!(printer.module.expressions[target.index()],Expr::Binding(found) if found==binding),
            _=>false,
        }) {return Err(PrintError::Container("a reassigned default needs delivery.exports = named for live updates"));}
    }
    let marker = !default_only
        && match config.es_module_marker {
            EsModuleMarker::Always => true,
            EsModuleMarker::Never => false,
            EsModuleMarker::IfDefault => exports.iter().any(|export| export.name == "default"),
        };
    if marker && exports.iter().any(|export| export.name == "__esModule") {
        return Err(PrintError::Container(
            "a source __esModule export requires delivery.es_module_marker = never",
        ));
    }
    // A reborrow gives the temporary substitutions exactly this render's
    // lifetime, without changing the persistent output or naming plan.
    let view = ContainerView {
        prefix: &prefix,
        imports: &reads,
        commonjs: plan.format == JavaScriptFormat::Cjs,
        strict: config.strict,
    };
    {
        let mut out = Printer {
            private: printer.private,
            optional_link: None,
            local_names: printer.local_names,
            module: printer.module,
            names: printer.names,
            literal_alternatives: printer.literal_alternatives,
            literals: printer.literals,
            forms: printer.forms,
            output: Buffer {
                text: std::mem::take(&mut printer.output.text),
                points: printer.output.points.take(),
                budget: printer.output.budget,
                limit: printer.output.limit,
                error: printer.output.error,
            },
            discarded_root: None,
            lazy: printer.lazy,
            container: Some(&view),
            root_activation: true,
            planned_structure: files::PlannedStructure::default(),
            inline: printer.inline,
            inline_module: None,
        };
        header(
            &mut out,
            planned,
            &requests,
            default_only,
            global.as_deref(),
        );
        out.text("(");
        out.text(&prefix);
        out.text("e,");
        out.text(&prefix);
        out.text("r,");
        out.text(&prefix);
        out.text("o)=>{");
        if config.strict {
            out.text("\"use strict\";");
        }
        if marker {
            out.text(&prefix);
            out.text("o.defineProperty(");
            out.text(&prefix);
            out.text("e,\"__esModule\",{value:true});");
        }
        for export in &exports {
            out.text(&prefix);
            out.text("o.defineProperty(");
            out.text(&prefix);
            out.text("e,");
            out.unicode_string(&export.name);
            out.text(",{enumerable:true,get:()=>");
            out.binding_read(export.binding);
            out.text("});");
        }
        for (index, request) in requests.iter().enumerate() {
            out.text("let ");
            out.text(&prefix);
            let _ = write!(out.output, "{index}=");
            out.text(&prefix);
            out.text("r(");
            out.unicode_string(&request.path);
            out.text(");");
        }
        if let Some(hosts) = hosts {
            if !file.links.hosted.is_empty() && !file.initializers.iter().any(|part| part.host.is_some()) {
                out.host_bindings(hosts, file.links.hosted.iter().copied());
            }
        }
        inline::body(&mut out, file, hosts);
        out.output.mark(None);
        printer.planned_structure = std::mem::take(&mut out.planned_structure);
        out.text("return ");
        if default_only {
            out.binding_read(exports[0].binding);
        } else {
            out.text(&prefix);
            out.text("e");
        }
        out.text("}");
        footer(&mut out, planned, &requests);
        printer.output.text = out.output.text;
        printer.output.points = out.output.points;
        printer.output.error = out.output.error;
    }
    let request_bytes =
        crate::output_budget::vector_bytes(&requests).map_err(PrintError::Admission)?;
    for request in requests {
        printer.output.drop_string(request.path, Scratch);
        if let Some(global) = request.global {
            printer.output.drop_string(global, Scratch);
        }
    }
    printer
        .output
        .budget
        .release(Scratch, request_bytes)
        .map_err(PrintError::Admission)?;
    // Consuming a Vec releases its payload strings above, and the backing
    // charge separately below (capture capacity before moving it).
    for read in &mut reads {
        printer
            .output
            .drop_string(std::mem::take(&mut read.name), Scratch);
    }
    for export in &mut exports {
        printer
            .output
            .drop_string(std::mem::take(&mut export.name), Scratch);
    }
    printer.output.drop_vec(reads, Scratch);
    printer.output.drop_vec(exports, Scratch);
    printer.output.drop_vec(sources, Scratch);
    printer.output.drop_string(prefix, Scratch);
    if let Some(global) = global {
        printer.output.drop_string(global, Scratch);
    }
    Ok(())
}

fn root(out: &mut Printer<'_, '_, '_>) {
    out.text("(typeof globalThis!=\"undefined\"?globalThis:typeof self!=\"undefined\"?self:this)");
}
fn global_assignment(
    out: &mut Printer<'_, '_, '_>,
    planned: &files::PlannedPrint<'_>,
    declaration: bool,
    global: Option<&str>,
) {
    let config = &planned.plan.container;
    let Some(global) = global else {
        out.output.error = Some(PrintError::Container(
            "unsupported or unnamed library container",
        ));
        return;
    };
    if config.global_binding == GlobalBinding::Var {
        if declaration {
            out.text("var ");
        }
        out.text(global);
        out.text("=");
    } else {
        root(out);
        out.text("[");
        out.unicode_string(global);
        out.text("]=");
    }
}
fn global_provider(out: &mut Printer<'_, '_, '_>, requests: &[Request]) {
    out.text("function(s){switch(s){");
    for request in requests {
        out.text("case ");
        out.unicode_string(&request.path);
        out.text(":return ");
        root(out);
        for segment in request.global.as_deref().unwrap_or("").split('.') {
            out.text("[");
            out.unicode_string(segment);
            out.text("]");
        }
        out.text(";");
    }
    out.text("}}");
}
// Node deletes a failing require from its cache. A delivered lazy graph needs
// module evaluation failures to remain cached, so internal files publish a
// cached activation. Entry exports retain the ordinary CommonJS surface.
fn cached_graph(planned: &files::PlannedPrint<'_>) -> bool {
    planned.plan.format == JavaScriptFormat::Cjs
        && planned.plan.files.iter().any(|file| matches!(file.role, FileRole::Lazy(_)))
}
fn cached_chunk(planned: &files::PlannedPrint<'_>) -> bool {
    cached_graph(planned) && !matches!(planned.plan.files[planned.file].role, FileRole::Entry(_))
}
fn runtime_text(out: &mut Printer<'_, '_, '_>, text: &str) {
    let prefix = out.container.expect("container recipe").prefix;
    for (index, part) in text.split('$').enumerate() {
        if index != 0 { out.text(prefix); }
        out.text(part);
    }
}
fn require_provider(out: &mut Printer<'_, '_, '_>, planned: &files::PlannedPrint<'_>, requests: &[Request]) {
    let internal = planned.plan.files[planned.file].links.imports.len();
    if !cached_graph(planned) || internal == 0 && out.lazy.is_empty() { out.text("require"); return; }
    if internal == requests.len() { out.text("s=>require(s)()"); return; }
    out.text("s=>{switch(s){");
    for request in &requests[..internal] {
        out.text("case "); out.unicode_string(&request.path); out.text(":");
    }
    for (_, path) in out.lazy {
        out.text("case "); out.unicode_string(path); out.text(":");
    }
    out.text("return require(s)();default:return require(s)}}");
}

fn header(
    out: &mut Printer<'_, '_, '_>,
    planned: &files::PlannedPrint<'_>,
    requests: &[Request],
    default_only: bool,
    global: Option<&str>,
) {
    match planned.plan.format {
        JavaScriptFormat::Cjs => {
            if cached_chunk(planned) {
                runtime_text(out, "let $s=0,$x={},$z;module.exports=()=>{if($s===3)throw $z;if($s)return $x;$s=1;try{");
            }
            if default_only {
                out.text("module.exports=");
            }
            out.text("(");
        }
        JavaScriptFormat::Iife => {
            global_assignment(out, planned, true, global);
            out.text("(");
        }
        JavaScriptFormat::Umd => {
            let config = &planned.plan.container;
            if config.global_binding == GlobalBinding::Var {
                out.text("var ");
                out.text(global.unwrap_or(""));
                out.text(";");
            }
            out.text("(function(f){if(typeof module==\"object\"&&module.exports)");
            if default_only {
                out.text("module.exports=");
            }
            out.text("f(exports,require,Object);else if(typeof define==\"function\"&&define.amd)define([\"exports\"");
            for request in requests {
                out.text(",");
                out.unicode_string(&request.path);
            }
            out.text("],function(e");
            for index in 0..requests.len() {
                let _ = write!(out.output, ",d{index}");
            }
            out.text("){return f(e,function(s){switch(s){");
            for (index, request) in requests.iter().enumerate() {
                out.text("case ");
                out.unicode_string(&request.path);
                let _ = write!(out.output, ":return d{index};");
            }
            out.text("}},Object)});else ");
            global_assignment(out, planned, false, global);
            out.text("f({},");
            global_provider(out, requests);
            out.text(",Object)})(");
        }
        _ => {
            out.output.error = Some(PrintError::Container(
                "unsupported or unnamed library container",
            ))
        }
    }
}
fn footer(out: &mut Printer<'_, '_, '_>, planned: &files::PlannedPrint<'_>, requests: &[Request]) {
    match planned.plan.format {
        JavaScriptFormat::Cjs => {
            out.text(")(");
            if cached_chunk(planned) { runtime_text(out, "$x,"); } else { out.text("exports,"); }
            require_provider(out, planned, requests);
            out.text(",Object);");
            if cached_chunk(planned) { runtime_text(out, "$s=2;return $x}catch(e){$s=3;$z=e;throw e}};"); }
        }
        JavaScriptFormat::Iife => {
            out.text(")({},");
            global_provider(out, requests);
            out.text(",Object);");
        }
        JavaScriptFormat::Umd => out.text(");"),
        _ => {
            out.output.error = Some(PrintError::Container(
                "unsupported or unnamed library container",
            ))
        }
    }
}
