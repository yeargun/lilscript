//! `migration/debug-class` (language.md R15, M10.11's first batch): an
//! `extern` named `debugLog` is dropped under `strip_debug` by its name. The
//! fix declares it `debug`, which the rule's second batch makes the only way
//! a call is strippable logging.
use crate::ast::{self, Item};
use crate::module::ModuleId;
use crate::span::Span;

use super::{LintEdit, LintFix, PendingDiagnostic};

pub(super) const RULE: &str = "migration/debug-class";

pub(super) fn lint(module: ModuleId, syntax: &ast::Program<'_, '_>, pending: &mut Vec<PendingDiagnostic>) {
    for item in syntax.items {
        let Item::Extern(declaration) = item else {
            continue;
        };
        if declaration.name.name != "debugLog" || declaration.declared_debug {
            continue;
        }
        // A `pure` declaration has no effect to strip: it keeps its report.
        let fix = (!declaration.declared_pure && declaration.return_type.is_void()).then(|| LintFix {
            applicability: "machine-applicable",
            edits: vec![LintEdit {
                span: Span {
                    start: declaration.span.start,
                    end: declaration.span.start,
                },
                replacement: "debug ".to_string(),
            }],
        });
        pending.push(PendingDiagnostic {
            module,
            span: declaration.name.span,
            rule: RULE,
            message: "`strip_debug` drops `debugLog` by its name until it is declared `debug` (R15)"
                .to_string(),
            evidence: None,
            help: Some(if fix.is_some() {
                "`lilscript-lint --fix` writes `debug extern`".to_string()
            } else {
                "a `debug` declaration returns `void` and is not `pure`".to_string()
            }),
            fix,
        });
    }
}
