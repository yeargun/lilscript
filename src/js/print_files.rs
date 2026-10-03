//! One file of a delivery plan (plan M3.3, design §7.10 and §9): its links,
//! its root statements in plan order, and its exports, in the plan's
//! container. Every file prints with the one `Names` of the whole output, so
//! a root binding has one spelling in every file and cross-file imports need
//! no `as` (design §8).

use super::*;
use crate::config::JavaScriptFormat;
use crate::js::delivery::{DeliveryPlan, FileRole};

/// What one planned file prints besides its statements.
pub(in crate::js) struct PlannedPrint<'a> {
    pub plan: &'a DeliveryPlan,
    pub file: usize,
    /// Every file's final name, in plan order.
    pub names: &'a [String],
}

/// Fixed-size proof retained with the candidate, parsed once at admission.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StructurePart {
    pub start: usize,
    pub end: usize,
    pub expected: crate::admission_parse::StructureDigest,
}
#[derive(Debug, Default)]
pub(crate) struct PlannedStructure { pub parts: Vec<StructurePart> }
impl PlannedStructure {
    pub fn heap_bytes(&self) -> Result<u64, AllocationError> { crate::output_budget::vector_bytes(&self.parts) }
}
pub(in crate::js) struct PlannedText { pub code:String, pub structure:PlannedStructure, pub map:Option<String> }
impl std::ops::Deref for PlannedText {type Target=str;fn deref(&self)->&str{&self.code}}

/// One planned file, in the plan's format.
#[allow(clippy::too_many_arguments)]
pub(in crate::js) fn render_planned_file_admitted(
    module: &Module,
    names: &Names,
    literal_alternatives: &[LiteralAlternative],
    literals: LiteralOutput,
    limit: usize,
    budget: &mut AllocationBudget<'_>,
    planned: &PlannedPrint<'_>,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
    sources: Option<&crate::source_maps::Sources>,
) -> Result<PlannedText, PrintError> {
    let _timing = crate::timing::TARGET_PRINT.scope(0);
    let plan = planned.plan;
    let file = &plan.files[planned.file];
    let own = planned.names[planned.file].as_str();
    let mut phase = budget.scope();
    let mut lazy = phase
        .vector(AllocationClass::Scratch, file.links.dynamic.len())
        .map_err(PrintError::Admission)?;
    for &target in &file.links.dynamic {
        if let FileRole::Lazy(loaded) = plan.files[target as usize].role {
            let path = crate::js::names::specifier_in(
                own,
                &planned.names[target as usize],
                AllocationClass::Scratch,
                &mut phase,
            )
            .map_err(PrintError::Admission)?;
            let path=if plan.format==JavaScriptFormat::Esm {
                crate::js::names::url_specifier_in(path,AllocationClass::Scratch,&mut phase).map_err(PrintError::Admission)?
            } else {path};
            lazy.push((loaded, path));
        }
    }
    let forms = match module.print_forms.as_ref() {
        Some(forms) => std::borrow::Cow::Borrowed(forms),
        None => std::borrow::Cow::Owned(
            crate::js::spellings::PrintForms::new(
                module,
                false,
                AllocationClass::Scratch,
                &mut phase,
            )
            .map_err(PrintError::Admission)?,
        ),
    };
    phase.work(WorkKind::Render, (file.statements.len() as u64).saturating_mul(file.initializers.len() as u64 + 1)).map_err(PrintError::Admission)?;
    let activations = file.statements.iter().any(|&index| {
        let row = module.root_rows[index as usize];
        row.origin != RowOrigin::Synthetic && row.anchor == Anchor::Anchored
            && file.initializers.iter().any(|part| part.source == row.module && !part.eager)
    });
    let mut inline_prefix = String::new();
    let mut inline_owners = Vec::new();
    if !file.initializers.is_empty() {
        phase.push_str(AllocationClass::Scratch, &mut inline_prefix, "$l") .map_err(PrintError::Admission)?;
        loop {
            phase.work(WorkKind::Render, (module.bindings.len() + module.expressions.len() + module.reserved.len()) as u64).map_err(PrintError::Admission)?;
            if !(0..module.bindings.len()).any(|index| names.get(BindingId::new(index)).starts_with(&inline_prefix))
                && !module.expressions.iter().any(|expr| matches!(expr, Expr::Host(host) if host.name.starts_with(&inline_prefix)))
                && !module.reserved.iter().any(|name| name.starts_with(&inline_prefix)) { break; }
            phase.push_str(AllocationClass::Scratch, &mut inline_prefix, "$").map_err(PrintError::Admission)?;
        }
        for &index in &file.statements {
            let row = module.root_rows[index as usize];
            if !activations || row.origin == RowOrigin::Synthetic || !file.initializers.iter().any(|part| part.source == row.module && !part.eager) { continue; }
            if let Statement::Let { binding, .. } | Statement::Function { binding, .. } = module.regions[module.root.index()].statements[index as usize] {
                phase.push(AllocationClass::Scratch, &mut inline_owners, (binding, row.module)).map_err(PrintError::Admission)?;
            }
        }
        inline_owners.sort_unstable_by_key(|(binding, _)| *binding);
    }
    let inline = inline::InlineView { prefix: &inline_prefix, owners: &inline_owners, modules: &file.initializers, activations };
    let mut local_names = phase.vector(AllocationClass::Scratch, file.statements.len()).map_err(PrintError::Admission)?;
    for &index in &file.statements {
        let index = index as usize;
        let statement = &module.regions[module.root.index()].statements[index];
        let function = match *statement {
            Statement::Function { binding, function } if module.root_rows[index].hoisted => Some((binding, function)),
            Statement::Let { binding, value: Some(value) } if inline.owners.binary_search_by_key(&binding, |(binding, _)| *binding).is_ok()
                && module.settled.get(binding.index()).copied().flatten() == Some(0) =>
                match module.expressions[value.index()] { Expr::Function(function) => Some((binding, function)), _ => None },
            _ => None,
        };
        if let Some((binding, function)) = function {
            if let Some(name) = module.functions[function.index()].name.exact().and_then(|name| name.as_unicode()) { local_names.push((binding, name)); }
        }
    }
    local_names.sort_unstable_by_key(|(binding, _)| *binding);
    let private = super::print::private_map(module);
    let mut printer = Printer {
        private: private.as_ref(),
        module,
        names,
        literal_alternatives,
        literals,
        forms: &forms,
        output: Buffer {
            text: String::new(),
            points: sources.map(|_| Vec::new()),
            budget: &mut phase,
            limit,
            error: None,
        },
        discarded_root: None,
        lazy: &lazy,
        container: None,
        root_activation: true,
        planned_structure: PlannedStructure::default(),
        inline: (!file.initializers.is_empty()).then_some(&inline),
        inline_module: None,
        local_names: &local_names,
    };
    if let Some(index) = file.links.host {
        let Some(hosts) = hosts else { return Err(PrintError::Container("missing carried host body")); };
        opaque_host(&mut printer, planned, hosts, index);
    } else { match plan.format {
        JavaScriptFormat::Esm | JavaScriptFormat::Bare => esm(&mut printer, plan, planned, hosts),
        JavaScriptFormat::Iife if plan.container.global.is_none() => {
            // Application frames have no publication namespace. Keep the same
            // lexical this/strictness as the unplanned application printer;
            // retained origins and statement proofs include the frame offset.
            printer.text("(()=>{");
            esm(&mut printer, plan, planned, hosts);
            printer.text("})();");
        }
        JavaScriptFormat::Cjs | JavaScriptFormat::Iife | JavaScriptFormat::Umd => containers::render(&mut printer, planned, hosts),
        _ => return Err(PrintError::Container("unresolved output container")),
    }
    }
    let structure=printer.planned_structure;
    let Buffer { mut text, error, points, .. } = printer.output;
    if let Some(error) = error {
        drop(text);
        return Err(error);
    }
    let mut map = None;
    if let Some(sources) = sources {
        let file = own.rsplit('/').next().unwrap_or(own);
        let json = crate::source_maps::render(&text, points.as_deref().unwrap_or(&[]), sources, file, &plan.container, &mut phase).map_err(PrintError::Admission)?;
        match plan.container.source_maps {
            crate::config::SourceMaps::Inline => {
                crate::source_maps::inline_base64(&json, &mut text, &mut phase).map_err(PrintError::Admission)?;
                let bytes = json.capacity() as u64; drop(json);
                phase.release(AllocationClass::Retained, bytes).map_err(PrintError::Admission)?;
            }
            crate::config::SourceMaps::External => {
                let mut path = phase.string(AllocationClass::Scratch, file).map_err(PrintError::Admission)?;
                phase.push_str(AllocationClass::Scratch, &mut path, ".map").map_err(PrintError::Admission)?;
                let path = crate::js::names::url_specifier_in(path, AllocationClass::Scratch, &mut phase).map_err(PrintError::Admission)?;
                phase.push_str(AllocationClass::Retained, &mut text, "\n//# sourceMappingURL=").map_err(PrintError::Admission)?;
                phase.push_str(AllocationClass::Retained, &mut text, &path).map_err(PrintError::Admission)?;
                let bytes = path.capacity() as u64; drop(path);
                phase.release(AllocationClass::Scratch, bytes).map_err(PrintError::Admission)?;
                map = Some(json);
            }
            crate::config::SourceMaps::Off => unreachable!("source-map input installed only when requested"),
        }
        if text.len().checked_add(map.as_ref().map_or(0, String::len)).is_none_or(|size| size > limit) { return Err(PrintError::ByteLimit); }
    }
    drop((forms, lazy, local_names, inline_owners, inline_prefix, points));
    phase.finish_retained().map_err(PrintError::Admission)?;
    Ok(PlannedText{code:text,structure,map})
}

/// An exported or imported name: an identifier, or a string (ES2022).
fn export_name(printer: &mut Printer<'_, '_, '_>, name: &str) {
    if identifier_name(name) {
        printer.text(name);
    } else {
        printer.unicode_string(name);
    }
}

/// An opaque body keeps a native dependency boundary even though its syntax
/// cannot enter typed target optimization. No source text is rewritten here:
/// the host parser's checked body/reads and the verified delivery links own it.
fn opaque_host(printer: &mut Printer<'_, '_, '_>, planned: &PlannedPrint<'_>,
    (hosts, strict): (&crate::host_modules::HostDelivery, bool), index: usize) {
    use AllocationClass::{Retained, Scratch};
    let file = &planned.plan.files[planned.file];
    let commonjs = planned.plan.format == JavaScriptFormat::Cjs;
    if !file.statements.is_empty() || !matches!(planned.plan.format, JavaScriptFormat::Esm | JavaScriptFormat::Cjs) {
        printer.output.error = Some(PrintError::Container("opaque host placement is not a module file")); return;
    }
    let Some(prefix) = printer.output.admit(|budget| {
        let mut prefix = budget.string(Scratch, "$host")?;
        loop {
            budget.work(WorkKind::Render, (printer.module.bindings.len() + printer.module.reserved.len()) as u64)?;
            if !(0..printer.module.bindings.len()).any(|i| printer.names.get(BindingId::new(i)).starts_with(&prefix))
                && !printer.module.reserved.iter().any(|name| name.starts_with(&prefix)) { break; }
            budget.push_char(Scratch, &mut prefix, '$')?;
        }
        Ok(prefix)
    }) else { return; };
    if strict { printer.text("\"use strict\";"); }
    let mut arguments = Vec::new();
    for (source, _) in &file.links.imports {
        let Some(dependency) = planned.plan.files[*source as usize].links.host else {
            printer.output.error = Some(PrintError::Container("opaque host dependency lost its module identity")); return;
        };
        let Some(path) = specifier(printer, planned, *source) else { return; };
        let Some(local) = printer.output.admit(|budget| budget.format(Scratch, format_args!("{prefix}{dependency}"))) else { return; };
        if commonjs {
            printer.text("const "); printer.text(&local); printer.text("=require("); printer.unicode_string(&path); printer.text(").default;");
        } else {
            printer.text("import "); printer.text(&local); printer.text(" from "); printer.unicode_string(&path); printer.text(";");
        }
        printer.output.drop_string(path, Scratch);
        if printer.output.admit(|budget| budget.push(Scratch, &mut arguments, (dependency, local))).is_none() { return; }
    }
    let Some(expression) = printer.output.admit(|budget| hosts.module_expression_in(index, strict, &arguments, budget)) else { return; };
    printer.text("const "); printer.text(&prefix); printer.text("="); printer.text(&expression); printer.text(";");
    printer.output.drop_string(expression, Retained);
    for binding in &file.links.exports {
        let Some(import) = file.links.hosted.iter().map(|&i| &printer.module.imports[i]).find(|import| import.binding == *binding) else {
            printer.output.error = Some(PrintError::Container("opaque host export has no foreign binding")); return;
        };
        if import.imported.is_empty() { continue; }
        let local = printer.names.get(*binding);
        if commonjs {
            printer.text("Object.defineProperty(exports,"); printer.unicode_string(local);
            printer.text(",{enumerable:true,value:"); printer.text(&prefix); printer.text("["); printer.unicode_string(&import.imported); printer.text("]});");
        } else {
            printer.text("export const "); printer.text(local); printer.text("="); printer.text(&prefix); printer.text("["); printer.unicode_string(&import.imported); printer.text("];");
        }
    }
    if commonjs {
        printer.text("Object.defineProperty(exports,'default',{value:"); printer.text(&prefix); printer.text("});");
    } else { printer.text("export default "); printer.text(&prefix); printer.text(";"); }
    for (_, local) in arguments.drain(..) { printer.output.drop_string(local, Scratch); }
    printer.output.drop_vec(arguments, Scratch);
    printer.output.drop_string(prefix, Scratch);
}

fn esm(
    printer: &mut Printer<'_, '_, '_>,
    plan: &DeliveryPlan,
    planned: &PlannedPrint<'_>,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
) {
    let file = &plan.files[planned.file];
    let names = printer.names;
    // The old route's preload prelude, verbatim.
    let Some(preload) = printer
        .output
        .admit(|budget| plan.preloads_in(planned.file, AllocationClass::Scratch, budget))
    else {
        return;
    };
    if !preload.is_empty() {
        printer.text("typeof document!=\"undefined\"&&[");
        for (index, &target) in preload.iter().enumerate() {
            if index != 0 {
                printer.text(",");
            }
            let Some(path) = specifier(printer, planned, target as u32) else {
                return;
            };
            printer.unicode_string(&path);
            printer.output.drop_string(path, AllocationClass::Scratch);
        }
        printer.text("].forEach(a=>{let b=document.createElement(\"link\");b.rel=\"modulepreload\",b.href=a,document.head.append(b)});");
    }
    printer.output.drop_vec(preload, AllocationClass::Scratch);
    // Module requests in evaluation order: each file's bindings, its public
    // names re-exported from there, or the bare request for its effects.
    for (source, bindings) in &file.links.imports {
        if !printer.output.work(1 + bindings.len()) {
            return;
        }
        let Some(path) = specifier(printer, planned, *source) else {
            return;
        };
        if !bindings.is_empty() {
            printer.text("import{");
            for (index, binding) in bindings.iter().enumerate() {
                if index != 0 {
                    printer.text(",");
                }
                printer.text(names.get(*binding));
            }
            printer.text("}from");
            printer.unicode_string(&path);
            printer.text(";");
        }
        // A public binding this file imports anyway is exported locally.
        let mut reexports = file
            .links
            .public
            .iter()
            .filter(|(_, binding, from)| from == source && !bindings.contains(binding))
            .peekable();
        if reexports.peek().is_some() {
            printer.text("export{");
            for (index, (name, binding, _)) in reexports.enumerate() {
                if index != 0 {
                    printer.text(",");
                }
                let local = names.get(*binding);
                printer.text(local);
                if local != name {
                    printer.text(" as ");
                    export_name(printer, name);
                }
            }
            printer.text("}from");
            printer.unicode_string(&path);
            printer.text(";");
        } else if bindings.is_empty() {
            printer.text("import");
            printer.unicode_string(&path);
            printer.text(";");
        }
        printer.output.drop_string(path, AllocationClass::Scratch);
    }
    // The foreign imports this file uses, one declaration per specifier;
    // carried host modules print as host bindings instead.
    printer.foreign_imports(
        file.links.foreign.iter().copied(),
        hosts,
        Some(own_name(planned)),
        Some(&plan.container),
    );
    if let Some(hosts) = hosts {
        if !file.links.hosted.is_empty() && !file.initializers.iter().any(|part| part.host.is_some()) {
            printer.host_bindings(hosts, file.links.hosted.iter().copied());
        }
    }
    inline::body(printer, file, hosts);
    // Internal exports under their own names, then the public names this
    // file declares.
    let own = planned.file as u32;
    let mut first = true;
    let mut open = |printer: &mut Printer<'_, '_, '_>| {
        printer.text(if std::mem::take(&mut first) {
            "export{"
        } else {
            ","
        });
    };
    for binding in &file.links.exports {
        if !printer.output.work(1) {
            return;
        }
        open(printer);
        let local = printer.local(*binding);
        printer.text(local);
        if local != names.get(*binding) {
            printer.text(" as ");
            printer.text(names.get(*binding));
        }
    }
    let imported = |binding: &BindingId| {
        file.links
            .imports
            .iter()
            .any(|(_, bindings)| bindings.contains(binding))
    };
    for (name, binding, from) in &file.links.public {
        if *from != own && !imported(binding) {
            continue;
        }
        if !printer.output.work(1) {
            return;
        }
        let local = printer.local(*binding);
        if names.get(*binding) == name && file.links.exports.contains(binding) {
            continue;
        }
        open(printer);
        printer.text(local);
        if local != name {
            printer.text(" as ");
            export_name(printer, name);
        }
    }
    if !first {
        printer.text("};");
    }
}

fn specifier(
    printer: &mut Printer<'_, '_, '_>,
    planned: &PlannedPrint<'_>,
    target: u32,
) -> Option<String> {
    printer.output.admit(|budget| {
        let path=crate::js::names::specifier_in(own_name(planned),&planned.names[target as usize],AllocationClass::Scratch,budget)?;
        if planned.plan.format==JavaScriptFormat::Esm {
            crate::js::names::url_specifier_in(path,AllocationClass::Scratch,budget)
        } else {Ok(path)}
    })
}

/// The file's own delivered name.
fn own_name<'a>(planned: &PlannedPrint<'a>) -> &'a str {
    planned.names[planned.file].as_str()
}

/// A relative foreign specifier is spelled from the output directory, which
/// stands for the first entry's source directory (as a one-file output
/// does); a file delivered `depth` directories below it climbs back first.
#[cfg(test)]
pub(super) fn rebased(source: &str, file: &str) -> Option<String> {
    rebased_in(
        source,
        file,
        AllocationClass::Scratch,
        &mut AllocationBudget::new(None),
    )
    .expect("rebased specifier")
}
pub(super) fn rebased_in(
    source: &str,
    file: &str,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<String>, AllocationError> {
    budget.work(WorkKind::Render, file.len() as u64 + 1)?;
    let depth = file.matches('/').count();
    if depth == 0 || !(source.starts_with("./") || source.starts_with("../")) {
        return Ok(None);
    }
    let mut result = String::new();
    for _ in 0..depth {
        budget.push_str(class, &mut result, "../")?;
    }
    budget.push_str(
        class,
        &mut result,
        source.strip_prefix("./").unwrap_or(source),
    )?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::rebased;

    #[test]
    fn relative_foreign_specifiers_climb_to_the_output_directory() {
        assert_eq!(rebased("./util.js", "a.js"), None);
        assert_eq!(
            rebased("./util.js", "internal/0.js").as_deref(),
            Some("../util.js")
        );
        assert_eq!(
            rebased("../x.js", "a/b/c.js").as_deref(),
            Some("../../../x.js")
        );
        assert_eq!(
            rebased("./lib/h.js", "lib/x.js").as_deref(),
            Some("../lib/h.js")
        );
        assert_eq!(rebased("react", "internal/0.js"), None);
        assert_eq!(rebased("node:fs", "internal/0.js"), None);
    }
}
