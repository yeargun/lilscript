//! Exact imported callable evidence extends the same lexical family machinery.
//! Real module roots supply instantiation; value storage retains TDZ obligations.
use super::helper_family::{FamilyOutcome, UnknownReason};
use super::helper_family_tests::{analyze, cache, edited, ledger, root};
use super::implementations::ImplementationMap;
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{CompilationRequest, WorkDomain};
use crate::js::PrintPolicy;
use std::process::Command;

fn modules(
    sources: &[&str],
    dependencies: &[&[usize]],
    order: &[usize],
    inspect: impl FnOnce(&Program<'_>),
) {
    let graph = crate::module::ModuleSet {
        modules: sources
            .iter()
            .zip(dependencies)
            .enumerate()
            .map(|(id, (source, deps))| crate::module::ModuleSource {
                path: format!("/helper-module-{id}.lil").into(),
                source: (*source).into(),
                dependencies: deps.to_vec(),
                foreign_dependencies: Vec::new(),
                dynamic_dependencies: Vec::new(),
                offset: 0,
            })
            .collect(),
        dependency_order: order.to_vec(),
        root: 0,
        eager: vec![true; sources.len()],
    };
    let arena = bumpalo::Bump::new();
    let syntax: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let checked = crate::check::analyze_modules(&syntax, &graph).unwrap();
    let program = from_source::from_checked_modules(&syntax, &checked).unwrap();
    program.verify().unwrap();
    inspect(&program);
}
fn execute(module: &crate::js::Module, setup: &str, after: &str) -> String {
    let mut observed = None;
    for mangle_bindings in [false, true] {
        let javascript = module.render(PrintPolicy { mangle_bindings }).unwrap();
        let script = format!("{setup}\n{javascript}\n{after}");
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .expect("Node is required for module-helper tests");
        assert!(
            result.status.success(),
            "{}\n{script}",
            String::from_utf8_lossy(&result.stderr)
        );
        let trace = String::from_utf8(result.stdout).unwrap();
        if let Some(prior) = &observed {
            assert_eq!(prior, &trace);
        } else {
            observed = Some(trace);
        }
    }
    observed.unwrap()
}

const SOURCES: [&str; 3] = [
    r#"import {helper} from "./helper";import {run} from "./caller";print(helper(4));print(run(2));export func(int)->int later(){return (int n)=>helper(n);}"#,
    r#"import {helper} from "./helper";export int run(int n){return helper(n);}print(helper(3));"#,
    r#"import {run} from "./caller";export int helper(int value){return value+1;}print(90);"#,
];
const DEPS: [&[usize]; 3] = [&[2, 1], &[2], &[1]];
const ORDER: [usize; 3] = [1, 2, 0];

#[test]
fn imported_named_helper_uses_real_module_roots_before_its_module_evaluation() {
    modules(&SOURCES, &DEPS, &ORDER, |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let mut cache = cache(&mut ledger);
        let helper = root(program, "helper");
        let (outcome, _) = analyze(program, &uses, helper, &mut ledger, &mut cache);
        let FamilyOutcome::Complete(family) = outcome else {
            panic!("{outcome:?}")
        };
        assert!(family.dependencies().valid_for(program, &uses));
        let roots: Vec<_> = family
            .environments()
            .iter()
            .filter(|environment| environment.creator.is_none())
            .map(|environment| environment.caller)
            .collect();
        assert_eq!(roots.len(), 3);
        assert!(roots.iter().all(|unit| program
            .modules
            .iter()
            .any(|module| module.initializer == *unit)));
        assert!(family
            .calls()
            .iter()
            .any(|call| call.caller == program.modules[1].initializer));
        assert_ne!(program.modules[1].initializer, family.initialize().unit);
        let direct = program.to_javascript().unwrap();
        // The dependency cycle executes caller's suffix first. Its helper call
        // still sees the other module's instantiated named function.
        let expected = "4\n90\n5\n3\n";
        assert_eq!(execute(&direct, "", ""), expected);
        let map = ImplementationMap::direct()
            .with_inline_helper(family, &mut ledger, WorkDomain::Optional)
            .unwrap();
        let mut config = crate::config::ProjectConfig::default();
        config.javascript.strip_console = false;
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let inline = javascript::lower_with_implementations(
            program,
            &uses,
            &map,
            policy.javascript_contract().unwrap(),
            demand::DemandMode::Prune,
            &mut ledger,
            WorkDomain::Optional,
        )
        .unwrap();
        assert_eq!(execute(&inline, "", ""), expected);
        drop(direct);
        drop(inline);
        map.discard(&mut ledger).unwrap();
        cache.discard(&mut ledger).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn module_instantiation_does_not_prove_captured_scalar_or_arrow_cell_initialization() {
    for dependent in [
        r#"import {call} from "./entry";extern void seen(JsValue error);try {print(call());}catch(auto error){seen(error);}export int value=7;export int helper(){return value;}"#,
        r#"import {call} from "./entry";extern void seen(JsValue error);try {print(call());}catch(auto error){seen(error);}export func()->int helper=()=>7;"#,
    ] {
        let sources = [
            r#"import {helper} from "./dep";export int call(){return helper();}"#,
            dependent,
        ];
        modules(&sources, &[&[1], &[0]], &[1, 0], |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let (outcome, _) = analyze(
                program,
                &uses,
                root(program, "helper"),
                &mut ledger,
                &mut cache,
            );
            assert!(
                matches!(
                    outcome,
                    FamilyOutcome::Unknown(
                        UnknownReason::EarlyCaptureOrRead | UnknownReason::UnrootedCapture
                    )
                ),
                "{outcome:?}"
            );
            let direct = program.to_javascript().unwrap();
            assert_eq!(
                execute(&direct, "function seen(error){console.log(error.name)}", ""),
                "ReferenceError\n"
            );
            drop(direct);
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn imported_helper_proofs_track_producer_body_and_consumer_revisions() {
    modules(&SOURCES, &DEPS, &ORDER, |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let mut cache = cache(&mut ledger);
        let (outcome, _) = analyze(
            program,
            &uses,
            root(program, "helper"),
            &mut ledger,
            &mut cache,
        );
        let FamilyOutcome::Complete(family) = outcome else {
            panic!("{outcome:?}")
        };
        for (unit, old, next) in [
            (family.initialize().unit, 90, 91),
            (family.root().body, 1, 2),
            (program.modules[1].initializer, 3, 4),
        ] {
            let changed = edited(program, unit, |data| {
                data.operations.iter_mut().find(|operation| matches!(operation.kind, OperationKind::Constant(Constant::Integer(value)) if value == old)).unwrap().kind = OperationKind::Constant(Constant::Integer(next));
            });
            let updated = uses
                .updated(&changed, &[unit], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert!(!family.dependencies().valid_for(&changed, &updated));
            assert!(family.dependencies().valid_for(program, &uses));
            updated.discard(&mut ledger).unwrap();
        }
        let mut changed_tables = program.clone();
        changed_tables.tables_revision = RevisionId::fresh();
        assert!(!family
            .dependencies()
            .valid_for_published(&changed_tables, &uses));
        family.discard(&mut ledger).unwrap();
        cache.discard(&mut ledger).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}
