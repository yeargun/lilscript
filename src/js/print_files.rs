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
pub(crate) struct PlannedStructure {
    pub start: usize,
    pub end: usize,
    pub expected: crate::admission_parse::StructureDigest,
}
pub(in crate::js) struct PlannedText { pub code:String, pub structure:PlannedStructure }
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
    let mut printer = Printer {
        module,
        names,
        literal_alternatives,
        literals,
        forms: &forms,
        output: Buffer {
            text: String::new(),
            budget: &mut phase,
            limit,
            error: None,
        },
        discarded_root: None,
        lazy: &lazy,
        container: None,
        root_activation: true,
        planned_structure: None,
    };
    match plan.format {
        JavaScriptFormat::Esm => esm(&mut printer, plan, planned, hosts),
        JavaScriptFormat::Cjs | JavaScriptFormat::Iife | JavaScriptFormat::Umd => containers::render(&mut printer, planned, hosts),
        _ => return Err(PrintError::Container("unresolved output container")),
    }
    let structure=printer.planned_structure;
    let Buffer { text, error, .. } = printer.output;
    if let Some(error) = error {
        drop(text);
        return Err(error);
    }
    drop((forms, lazy));
    phase.finish_retained().map_err(PrintError::Admission)?;
    Ok(PlannedText{code:text,structure:structure.expect("planned core proof")})
}

/// An exported or imported name: an identifier, or a string (ES2022).
fn export_name(printer: &mut Printer<'_, '_, '_>, name: &str) {
    if identifier_name(name) {
        printer.text(name);
    } else {
        printer.unicode_string(name);
    }
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
    );
    if let Some(hosts) = hosts {
        if !file.links.hosted.is_empty() {
            printer.host_bindings(hosts, file.links.hosted.iter().copied());
        }
    }
    let root = &printer.module.regions[printer.module.root.index()].statements;
    let start=printer.output.text.len();
    printer.statement_list(root,file.statements.iter().map(|&statement| statement as usize));
    if !printer.output.work(printer.module.expressions.len()+printer.module.regions.len()) {return;}
    printer.planned_structure=Some(PlannedStructure{start,end:printer.output.text.len(),
        expected:crate::js::admission::planned_core_digest(printer.module,&file.statements,&[],printer.lazy,false,false)});
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
        printer.text(names.get(*binding));
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
        let local = names.get(*binding);
        if local == name && file.links.exports.contains(binding) {
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
