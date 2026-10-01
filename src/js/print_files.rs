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
) -> Result<String, PrintError> {
    let _timing = crate::timing::TARGET_PRINT.scope(0);
    let plan = planned.plan;
    let file = &plan.files[planned.file];
    let own = planned.names[planned.file].as_str();
    let specifier = |target: u32| -> String {
        crate::js::names::specifier(own, &planned.names[target as usize])
    };
    let lazy = file
        .links
        .dynamic
        .iter()
        .filter_map(|&target| match plan.files[target as usize].role {
            FileRole::Lazy(loaded) => Some((loaded, specifier(target))),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut phase = budget.scope();
    let forms = match module.print_forms.as_ref() {
        Some(forms) => std::borrow::Cow::Borrowed(forms),
        None => std::borrow::Cow::Owned(crate::js::spellings::PrintForms::new(module, false, AllocationClass::Scratch, &mut phase).map_err(PrintError::Admission)?),
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
    };
    match plan.format {
        JavaScriptFormat::Esm => esm(&mut printer, plan, planned, hosts, &specifier),
        _ => return Err(PrintError::Container),
    }
    let Buffer { text, error, .. } = printer.output;
    if let Some(error) = error {
        drop(text);
        return Err(error);
    }
    phase.finish_retained().map_err(PrintError::Admission)?;
    Ok(text)
}

/// An exported or imported name: an identifier, or a string (ES2022).
fn export_name(printer: &mut Printer<'_, '_, '_>, name: &str) {
    if identifier_name(name) {
        printer.text(name);
    } else {
        printer.string(&StringValue::from(name));
    }
}

fn esm(
    printer: &mut Printer<'_, '_, '_>,
    plan: &DeliveryPlan,
    planned: &PlannedPrint<'_>,
    hosts: Option<(&crate::host_modules::HostDelivery, bool)>,
    specifier: &dyn Fn(u32) -> String,
) {
    let file = &plan.files[planned.file];
    let names = printer.names;
    // The old route's preload prelude, verbatim.
    let preload = plan.preloads(planned.file);
    if !preload.is_empty() {
        printer.text("typeof document!=\"undefined\"&&[");
        for (index, &target) in preload.iter().enumerate() {
            if index != 0 {
                printer.text(",");
            }
            printer.string(&StringValue::from(specifier(target as u32).as_str()));
        }
        printer.text("].forEach(a=>{let b=document.createElement(\"link\");b.rel=\"modulepreload\",b.href=a,document.head.append(b)});");
    }
    // Module requests in evaluation order: each file's bindings, its public
    // names re-exported from there, or the bare request for its effects.
    for (source, bindings) in &file.links.imports {
        if !printer.output.work(1 + bindings.len()) {
            return;
        }
        let path = specifier(*source);
        if !bindings.is_empty() {
            printer.text("import{");
            for (index, binding) in bindings.iter().enumerate() {
                if index != 0 {
                    printer.text(",");
                }
                printer.text(names.get(*binding));
            }
            printer.text("}from");
            printer.string(&StringValue::from(path.as_str()));
            printer.text(";");
        }
        // A public binding this file imports anyway is exported locally.
        let reexports = file
            .links
            .public
            .iter()
            .filter(|(_, binding, from)| from == source && !bindings.contains(binding))
            .collect::<Vec<_>>();
        if !reexports.is_empty() {
            printer.text("export{");
            for (index, (name, binding, _)) in reexports.iter().enumerate() {
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
            printer.string(&StringValue::from(path.as_str()));
            printer.text(";");
        } else if bindings.is_empty() {
            printer.text("import");
            printer.string(&StringValue::from(path.as_str()));
            printer.text(";");
        }
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
    let order = file
        .statements
        .iter()
        .map(|&statement| statement as usize)
        .collect::<Vec<_>>();
    printer.statement_list(root, &order);
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

/// The file's own delivered name.
fn own_name<'a>(planned: &PlannedPrint<'a>) -> &'a str {
    planned.names[planned.file].as_str()
}

/// A relative foreign specifier is spelled from the output directory, which
/// stands for the first entry's source directory (as a one-file output
/// does); a file delivered `depth` directories below it climbs back first.
pub(super) fn rebased(source: &str, file: &str) -> Option<String> {
    let depth = file.matches('/').count();
    if depth == 0 || !(source.starts_with("./") || source.starts_with("../")) {
        return None;
    }
    let mut rebased = "../".repeat(depth);
    rebased.push_str(source.strip_prefix("./").unwrap_or(source));
    Some(rebased)
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
