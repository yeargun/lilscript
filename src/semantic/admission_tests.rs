use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};
use crate::module::{ModuleSet, ModuleSource};
use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};

const SENTINEL: u64 = 29;
const WORK: u64 = 10_000_000;
const MEMORY: u64 = 10_000_000;

fn ledger(work: u64, memory: u64) -> BudgetLedger {
    let mut ledger = BudgetLedger::new(
        ResourceLimits {
            wall_time_ms: Some(100_000),
            ..ResourceLimits::default()
        },
        BudgetPlan {
            baseline_work: work,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap();
    ledger.retain(WorkDomain::Baseline, SENTINEL).unwrap();
    ledger
}

fn fact_bytes(source: &crate::ast::SourceIdentity) -> u64 {
    (source.len()
        * (std::mem::size_of::<Option<Type<'_>>>() + std::mem::size_of::<SourceInfo<'_, '_>>()))
        as u64
}

fn small_declaration_bytes() -> u64 {
    (4 * (std::mem::size_of::<Symbol<'_>>()
        + std::mem::size_of::<Option<crate::module::ModuleId>>())) as u64
}

fn base_scope_bytes() -> u64 {
    (std::mem::size_of::<AHashMap<&str, SymbolId>>() + std::mem::size_of::<Narrowing<'_>>()) as u64
}

fn simple_function_frame_peak() -> u64 {
    4 * (base_scope_bytes()
        + std::mem::size_of::<AHashSet<&str>>() as u64
        + std::mem::size_of::<ReturnContext<'_>>() as u64
        + std::mem::size_of::<Option<Type<'_>>>() as u64)
}

fn assert_small_declarations(declarations: &DeclarationTables<'_>, symbols: usize) {
    assert!((1..=4).contains(&symbols));
    assert_eq!(declarations.symbols.len(), symbols);
    assert_eq!(declarations.symbol_modules.len(), symbols);
    assert_eq!(declarations.symbols.capacity(), 4);
    assert_eq!(declarations.symbol_modules.capacity(), 4);
    assert_eq!(declarations.structs.capacity(), 0);
    assert_eq!(declarations.nominal_members.capacity(), 0);
}

fn graph(sources: &[&str], dependencies: &[&[usize]], order: &[usize]) -> ModuleSet {
    ModuleSet {
        modules: sources
            .iter()
            .zip(dependencies)
            .enumerate()
            .map(|(module, (source, dependencies))| ModuleSource {
                path: format!("/checker-admission-{module}.lil").into(),
                source: (*source).into(),
                dependencies: dependencies.to_vec(),
                foreign_dependencies: Vec::new(),
                dynamic_dependencies: Vec::new(),
                offset: module * 100_000,
            })
            .collect(),
        dependency_order: order.to_vec(),
        root: 0,
        eager: vec![true; sources.len()],
        for_of_specialize_family: 0,
    }
}

const INTERFACE_SOURCES: [&str; 3] = [
    "export int first(){return 3;}",
    "import \"./first\";import {first as local} from \"./first\";import {second} from \"./second\";print(local()+second());",
    "export int second(){return 4;}",
];

fn interface_graph() -> ModuleSet {
    let mut modules = graph(&INTERFACE_SOURCES, &[&[], &[0, 0, 2], &[]], &[0, 2, 1]);
    modules.root = 1;
    modules
}

struct ModuleStorage {
    order: u64,
    states: u64,
    stack: u64,
    facts: u64,
    interfaces: u64,
    rows: Vec<[u64; 3]>,
}

impl ModuleStorage {
    fn new(programs: &[Program<'_, '_>], modules: &ModuleSet) -> Self {
        let count = programs.len();
        Self {
            order: (count * std::mem::size_of::<usize>()) as u64,
            states: (count * std::mem::size_of::<u8>()) as u64,
            stack: (count
                * std::mem::size_of::<(usize, std::iter::Copied<std::slice::Iter<'_, usize>>)>())
                as u64,
            facts: (count * std::mem::size_of::<ModuleFacts<'_, '_>>()) as u64
                + programs
                    .iter()
                    .map(|program| fact_bytes(program.source_identity()))
                    .sum::<u64>(),
            interfaces: (count * std::mem::size_of::<ModuleInterface<'_>>()) as u64,
            rows: programs
                .iter()
                .zip(&modules.modules)
                .map(|(program, module)| {
                    [
                        (module.dependencies.len() * std::mem::size_of::<usize>()) as u64,
                        (program
                            .imports
                            .iter()
                            .map(|import| import.specifiers.len())
                            .sum::<usize>()
                            * std::mem::size_of::<ModuleImport<'_>>())
                            as u64,
                        (program.exports.len() * std::mem::size_of::<ModuleExport<'_>>()) as u64,
                    ]
                })
                .collect(),
        }
    }

    fn live(&self) -> u64 {
        self.order + self.facts + self.interfaces + self.rows.iter().flatten().sum::<u64>()
    }

    fn schedule_peak(&self) -> u64 {
        self.order + self.states + self.stack
    }

    fn peak(&self) -> u64 {
        self.schedule_peak().max(self.live())
    }

    fn checked_live(&self) -> u64 {
        self.live() + small_declaration_bytes()
    }

    fn checked_peak(&self) -> u64 {
        self.schedule_peak()
            .max(self.checked_live() + simple_function_frame_peak())
    }
}

#[test]
fn admitted_source_tables_match_the_original_model_and_release_after_callback() {
    let source = "int value=4;int next(){value+=1;return value;}print(next());";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let expected = analyze(&syntax).unwrap();
    let expected_debug = format!("{expected:?}");
    drop(expected);
    let live_facts = live_facts_for_test();
    let bytes = fact_bytes(syntax.source_identity()) + small_declaration_bytes();
    let mut ledger = ledger(WORK, SENTINEL + bytes + simple_function_frame_peak());
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    with_analyzed_source(&syntax, &mut budget, |model, budget| {
        assert_eq!(format!("{model:?}"), expected_debug);
        assert!(model.belongs_to(syntax.source_identity()));
        assert_small_declarations(&model.declarations, 2);
        assert_eq!(live_facts_for_test(), live_facts + 1);
        assert_eq!(
            model.facts.expression_types.capacity(),
            syntax.source_identity().len()
        );
        assert_eq!(
            model.facts.source_info.capacity(),
            syntax.source_identity().len()
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), bytes);
        budget.with_ledger(|owner| {
            assert_eq!(owner.unwrap().0.retained_bytes(), SENTINEL + bytes);
        });
    })
    .unwrap();
    assert_eq!(live_facts_for_test(), live_facts);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
    assert_eq!(
        ledger.peak_retained_bytes(),
        SENTINEL + bytes + simple_function_frame_peak()
    );
}

#[test]
fn admitted_facts_observer_runs_after_real_drop_and_public_models_still_move_threads() {
    struct Probe<'a> {
        expected_live: usize,
        dropped: &'a Cell<bool>,
    }
    impl Drop for Probe<'_> {
        fn drop(&mut self) {
            assert_eq!(live_facts_for_test(), self.expected_live);
            self.dropped.set(true);
        }
    }
    let before = live_facts_for_test();
    for unwind in [false, true] {
        let dropped = Cell::new(false);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let owner = AdmittedFactsOwner::new(
                Probe {
                    expected_live: before + 2,
                    dropped: &dropped,
                },
                2,
            );
            assert_eq!(live_facts_for_test(), before + 2);
            if unwind {
                panic!("injected admitted-owner panic");
            }
            drop(owner);
        }));
        assert_eq!(result.is_err(), unwind);
        assert!(dropped.get());
        assert_eq!(live_facts_for_test(), before);
    }
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "int value=7;").unwrap();
    let model = analyze(&syntax).unwrap();
    let clone = model.clone();
    assert!(clone.belongs_to(syntax.source_identity()));
    std::thread::scope(|threads| {
        threads.spawn(move || drop(clone)).join().unwrap();
    });
    assert_eq!(
        model.facts.expression_types.len(),
        syntax.source_identity().len()
    );
    drop(model);
    assert_eq!(live_facts_for_test(), before);
}

#[test]
fn source_refusals_cover_both_fixed_arrays_without_releasing_parent_storage() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "int value=7;print(value);").unwrap();
    let nodes = syntax.source_identity().len() as u64;
    let first_bytes = nodes * std::mem::size_of::<Option<Type<'_>>>() as u64;
    for (work, memory, expected, peak) in [
        (
            WORK,
            SENTINEL,
            BudgetError::MemoryExhausted(WorkDomain::Baseline),
            SENTINEL,
        ),
        (
            WORK,
            SENTINEL + first_bytes,
            BudgetError::MemoryExhausted(WorkDomain::Baseline),
            SENTINEL + first_bytes,
        ),
        (
            0,
            MEMORY,
            BudgetError::WorkExhausted(WorkDomain::Baseline),
            SENTINEL,
        ),
        (
            nodes + 1,
            MEMORY,
            BudgetError::WorkExhausted(WorkDomain::Baseline),
            SENTINEL + first_bytes,
        ),
    ] {
        let mut ledger = ledger(work, memory);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let called = Cell::new(false);
        let error =
            with_analyzed_source(&syntax, &mut budget, |_, _| called.set(true)).unwrap_err();
        assert_eq!(
            error,
            AdmittedSemanticError::Resources(AllocationError::Budget(expected))
        );
        assert!(!called.get());
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(ledger.peak_retained_bytes(), peak);
    }
}

#[test]
fn source_semantic_errors_keep_original_spans_messages_and_release_facts() {
    for source in [
        "int value=missing;",
        "int answer(){return \"wrong\";}",
        "Record<int> value=record{x:1,x:2};",
        "struct A{int first;int second;}int value=missing;",
        "extern void consume(int same,int same);",
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let expected = analyze(&syntax).unwrap_err();
        let live_facts = live_facts_for_test();
        let mut ledger = ledger(WORK, MEMORY);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let error = with_analyzed_source(&syntax, &mut budget, |_, _| {
            panic!("invalid source cannot reach client")
        })
        .unwrap_err();
        assert_eq!(error, AdmittedSemanticError::Semantic(expected));
        assert_eq!(live_facts_for_test(), live_facts);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
    }
}

#[test]
fn callback_error_panic_and_deadline_drop_checker_before_scope_release() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "int value=7;").unwrap();
    let live_facts = live_facts_for_test();
    let mut ledger = ledger(WORK, MEMORY);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let result =
        with_analyzed_source(&syntax, &mut budget, |_, _| Err::<(), _>("client refusal")).unwrap();
    assert_eq!(result, Err("client refusal"));
    assert_eq!(live_facts_for_test(), live_facts);
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let _ = with_analyzed_source(&syntax, &mut budget, |_, budget| {
            assert_eq!(live_facts_for_test(), live_facts + 1);
            assert_eq!(
                budget.retained_bytes(AllocationClass::Scratch),
                fact_bytes(syntax.source_identity()) + small_declaration_bytes()
            );
            panic!("injected checker client panic");
        });
    }));
    assert!(failure.is_err());
    assert_eq!(live_facts_for_test(), live_facts);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    budget.with_ledger(|owner| {
        owner
            .unwrap()
            .0
            .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
    });
    let error = with_analyzed_source(&syntax, &mut budget, |_, _| ()).unwrap_err();
    assert_eq!(
        error,
        AdmittedSemanticError::Resources(AllocationError::Budget(BudgetError::DeadlineExceeded))
    );
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn callback_retained_output_transfers_without_leaving_fixed_table_charges() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "int value=7;").unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let parent = budget.string(AllocationClass::Retained, "parent").unwrap();
    let output = with_analyzed_source(&syntax, &mut budget, |_, scope| {
        scope.string(AllocationClass::Retained, "result").unwrap()
    })
    .unwrap();
    assert_eq!(output, "result");
    assert_eq!(parent, "parent");
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    assert_eq!(budget.retained_bytes(AllocationClass::Retained), 12);
    drop(output);
    budget.release(AllocationClass::Retained, 6).unwrap();
    drop(parent);
    budget.release(AllocationClass::Retained, 6).unwrap();
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn parser_checker_and_lower_use_one_ledger_through_detached_prepared_output() {
    let source = "int value=4;int next(){value+=1;return value;}print(next());print(next());";
    let mut ledger = ledger(WORK, MEMORY);
    let arena = crate::parser::AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let syntax = arena.parse(source).unwrap();
    let backing = arena.allocated_bytes() as u64;
    let prepared = arena.with_ledger(|ledger, domain| {
        let mut budget = AllocationBudget::new(Some((ledger, domain)));
        with_analyzed_source(&syntax, &mut budget, |model, budget| {
            budget.with_ledger(|owner| {
                assert_eq!(
                    owner.unwrap().0.retained_bytes(),
                    SENTINEL
                        + backing
                        + fact_bytes(syntax.source_identity())
                        + small_declaration_bytes()
                );
            });
            crate::semantic_program::from_checked_source_admitted(&syntax, model, budget)
        })
        .unwrap()
        .unwrap()
    });
    drop(syntax);
    drop(arena);
    assert!(ledger.retained_bytes() > SENTINEL);
    let javascript = prepared
        .program()
        .to_javascript()
        .unwrap()
        .render(crate::structured_js::PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &javascript])
        .output()
        .expect("Node is required for checked frontend observations");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "5\n6\n");
    prepared.discard(&mut ledger);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn admitted_modules_share_declarations_and_charge_exact_facts_interfaces_and_schedule() {
    let modules = interface_graph();
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = INTERFACE_SOURCES
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let expected = analyze_modules(&programs, &modules).unwrap();
    let storage = ModuleStorage::new(&programs, &modules);
    let mut ledger = ledger(WORK, SENTINEL + storage.checked_peak());
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    with_analyzed_modules(&programs, &modules, &mut budget, |checked, budget| {
        assert_eq!(format!("{checked:?}"), format!("{expected:?}"));
        assert_eq!(checked.symbols(), expected.symbols());
        assert_eq!(
            checked.initialization_order(),
            expected.initialization_order()
        );
        assert_eq!(checked.initialization_order(), &[0, 2, 1]);
        assert_eq!(checked.root(), 1);
        let first = checked.view(0).unwrap();
        let second = checked.view(1).unwrap();
        assert!(std::ptr::eq(first.declarations, second.declarations));
        assert_small_declarations(first.declarations, 2);
        assert_eq!(first.declarations.symbol_modules, [Some(0), Some(2)]);
        for (module, program) in programs.iter().enumerate() {
            let view = checked.view(module).unwrap();
            assert!(view.belongs_to(program.source_identity()));
            assert_eq!(
                view.facts.expression_types.capacity(),
                program.source_identity().len()
            );
            assert_eq!(
                view.facts.source_info.capacity(),
                program.source_identity().len()
            );
            assert_eq!(
                format!("{:?}", view.facts),
                format!("{:?}", expected.view(module).unwrap().facts)
            );
            let interface = &checked.interfaces()[module];
            let imports = program
                .imports
                .iter()
                .map(|import| import.specifiers.len())
                .sum::<usize>();
            assert_eq!(interface.module, module);
            assert_eq!(interface.dependencies, modules.modules[module].dependencies);
            assert_eq!(
                interface.dependencies.capacity(),
                interface.dependencies.len()
            );
            assert_eq!(interface.imports.len(), imports);
            assert_eq!(interface.imports.capacity(), imports);
            assert_eq!(interface.exports.len(), program.exports.len());
            assert_eq!(interface.exports.capacity(), program.exports.len());
        }
        let root = &checked.interfaces()[1];
        assert_eq!(root.dependencies.len(), 3);
        assert_eq!(root.imports.len(), 2);
        assert_eq!(root.imports[0].local, "local");
        assert_eq!(
            root.imports[0].target,
            checked.interfaces()[0].exports[0].target
        );
        assert_eq!(
            root.imports[1].target,
            checked.interfaces()[2].exports[0].target
        );
        assert_eq!(
            budget.retained_bytes(AllocationClass::Scratch),
            storage.checked_live()
        );
        budget.with_ledger(|owner| {
            let ledger = owner.unwrap().0;
            assert_eq!(ledger.retained_bytes(), SENTINEL + storage.checked_live());
            assert_eq!(
                ledger.peak_retained_bytes(),
                SENTINEL + storage.checked_peak()
            );
        });
    })
    .unwrap();
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
    assert_eq!(
        ledger.peak_retained_bytes(),
        SENTINEL + storage.checked_peak()
    );
}

#[test]
fn module_resource_refusal_reports_the_actual_source_and_releases_partial_facts() {
    let sources = ["import \"./other\";int first=1;", "int second=2;"];
    let modules = graph(&sources, &[&[1], &[]], &[1, 0]);
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let outer = (programs.len() * std::mem::size_of::<ModuleFacts<'_, '_>>()) as u64;
    let first = fact_bytes(programs[0].source_identity());
    let storage = ModuleStorage::new(&programs, &modules);
    let partial = storage.order + outer + first;
    for (memory, module, peak) in [
        (SENTINEL, modules.root, SENTINEL),
        (
            SENTINEL + partial,
            1,
            SENTINEL + partial.max(storage.schedule_peak()),
        ),
    ] {
        let live_facts = live_facts_for_test();
        let mut ledger = ledger(WORK, memory);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let error = with_analyzed_modules(&programs, &modules, &mut budget, |_, _| {
            panic!("resource refusal cannot reach client")
        })
        .unwrap_err();
        assert_eq!(
            error,
            AdmittedModuleSemanticError {
                module,
                error: AdmittedSemanticError::Resources(AllocationError::Budget(
                    BudgetError::MemoryExhausted(WorkDomain::Baseline)
                )),
            }
        );
        assert_eq!(live_facts_for_test(), live_facts);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(ledger.peak_retained_bytes(), peak);
    }
}

#[test]
fn module_declaration_refusals_keep_canonical_owner_attribution_and_parent_storage() {
    let sources = [
        "import \"./other\";int a=1;int b=2;",
        "int c=3;int d=4;int e=5;",
    ];
    let modules = graph(&sources, &[&[1], &[]], &[1, 0]);
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let storage = ModuleStorage::new(&programs, &modules);
    let symbols = 4 * std::mem::size_of::<Symbol<'_>>() as u64;
    let owners = 4 * std::mem::size_of::<Option<crate::module::ModuleId>>() as u64;
    let scope = std::mem::size_of::<AHashMap<&str, SymbolId>>() as u64;
    let frames = base_scope_bytes();
    for (required, peak, module) in [
        (scope, 0, 0),
        (frames, scope, 0),
        (frames + symbols, frames, 0),
        (frames + symbols + owners, frames + symbols, 0),
        (frames + 3 * symbols + owners, frames + symbols + owners, 1),
    ] {
        let mut ledger = ledger(WORK, SENTINEL + 6 + storage.live() + required - 1);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let parent = budget.string(AllocationClass::Retained, "parent").unwrap();
        let error = with_analyzed_modules(&programs, &modules, &mut budget, |_, _| {
            panic!("refused declaration must not reach callback")
        })
        .unwrap_err();
        assert_eq!(
            error,
            AdmittedModuleSemanticError {
                module,
                error: AdmittedSemanticError::Resources(AllocationError::Budget(
                    BudgetError::MemoryExhausted(WorkDomain::Baseline)
                )),
            }
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), 6);
        assert_eq!(parent, "parent");
        drop(parent);
        budget.release(AllocationClass::Retained, 6).unwrap();
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(
            ledger.peak_retained_bytes(),
            SENTINEL + 6 + storage.schedule_peak().max(storage.live() + peak)
        );
    }
    let expected_peak = storage
        .schedule_peak()
        .max(storage.live() + frames + (3 * symbols + owners).max(2 * symbols + 3 * owners));
    let mut ledger = ledger(WORK, SENTINEL + expected_peak);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    with_analyzed_modules(&programs, &modules, &mut budget, |checked, budget| {
        let declarations = checked.view(0).unwrap().declarations;
        assert!(std::ptr::eq(
            declarations,
            checked.view(1).unwrap().declarations
        ));
        assert_eq!(
            declarations
                .symbols
                .iter()
                .map(|symbol| symbol.name)
                .collect::<Vec<_>>(),
            ["a", "b", "c", "d", "e"]
        );
        assert_eq!(
            declarations.symbol_modules,
            [Some(0), Some(0), Some(1), Some(1), Some(1)]
        );
        assert_eq!(
            (
                declarations.symbols.capacity(),
                declarations.symbol_modules.capacity()
            ),
            (8, 8)
        );
        assert_eq!(
            budget.retained_bytes(AllocationClass::Scratch),
            storage.live() + 2 * (symbols + owners)
        );
    })
    .unwrap();
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
    assert_eq!(ledger.peak_retained_bytes(), SENTINEL + expected_peak);
}

#[test]
fn module_schedule_and_each_interface_buffer_refuse_before_growth_and_release_scratch() {
    let modules = interface_graph();
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = INTERFACE_SOURCES
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let storage = ModuleStorage::new(&programs, &modules);
    let mut cases = vec![
        ("schedule order", modules.root, storage.order, 0),
        (
            "schedule states",
            modules.root,
            storage.order + storage.states,
            storage.order,
        ),
        (
            "schedule stack",
            modules.root,
            storage.schedule_peak(),
            storage.order + storage.states,
        ),
        (
            "interface outer",
            modules.root,
            storage.order + storage.facts + storage.interfaces,
            storage.schedule_peak().max(storage.order + storage.facts),
        ),
    ];
    let mut live = storage.order + storage.facts + storage.interfaces;
    for (module, rows) in storage.rows.iter().enumerate() {
        for (name, &bytes) in ["dependencies", "imports", "exports"].iter().zip(rows) {
            if bytes != 0 {
                cases.push((
                    *name,
                    module,
                    live + bytes,
                    live.max(storage.schedule_peak()),
                ));
            }
            live += bytes;
        }
    }
    assert_eq!(live, storage.live());
    for (name, module, required, peak) in cases {
        let live_facts = live_facts_for_test();
        let mut ledger = ledger(WORK, SENTINEL + required - 1);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let error = with_analyzed_modules(&programs, &modules, &mut budget, |_, _| {
            panic!("{name} refusal cannot reach client");
        })
        .unwrap_err();
        assert_eq!(
            error,
            AdmittedModuleSemanticError {
                module,
                error: AdmittedSemanticError::Resources(AllocationError::Budget(
                    BudgetError::MemoryExhausted(WorkDomain::Baseline)
                )),
            },
            "{name} in module {module}"
        );
        assert_eq!(live_facts_for_test(), live_facts, "{name}");
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0, "{name}");
        assert_eq!(
            budget.retained_bytes(AllocationClass::Retained),
            0,
            "{name}"
        );
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL, "{name}");
        assert_eq!(ledger.peak_retained_bytes(), SENTINEL + peak, "{name}");
    }
}

#[test]
fn module_graph_schedule_and_late_interface_work_refusals_keep_actual_attribution() {
    let modules = interface_graph();
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = INTERFACE_SOURCES
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let storage = ModuleStorage::new(&programs, &modules);
    let mut successful = ledger(WORK, MEMORY);
    with_analyzed_modules(
        &programs,
        &modules,
        &mut AllocationBudget::new(Some((&mut successful, WorkDomain::Baseline))),
        |_, _| (),
    )
    .unwrap();
    let total = successful.work_used(WorkDomain::Baseline);
    let graph_work = (modules.modules.len()
        + modules
            .modules
            .iter()
            .map(|module| module.dependencies.len())
            .sum::<usize>()) as u64;
    let published_rows = programs
        .iter()
        .map(|program| {
            program.exports.len()
                + program
                    .imports
                    .iter()
                    .map(|import| import.specifiers.len())
                    .sum::<usize>()
        })
        .sum::<usize>() as u64;
    assert_eq!(published_rows, 4);
    // Two canonical function symbols: two initial vector allocations, then
    // two row publications at two work units each. Import aliases add no rows.
    let declaration_work = 2 + 2 * 2;
    // Four analyzer passes over three modules; two signature scopes, then two
    // function bodies with one lexical scope and return/generator contexts.
    // Root's one binary expression adds three visits, three continuation
    // probes, two pushes and one first allocation, all during body checking.
    let binary_work = 3 + 3 + 2 + 1;
    let analyzer_work = 4 * 3 * 4 + 2 * 2 + 2 * (2 + 6 + 2 + 2) + binary_work;
    let body_work = 3 * 4 + 2 * (2 + 6 + 2 + 2) + binary_work;
    let registration_peak = storage.schedule_peak().max(
        storage.checked_live()
            + base_scope_bytes()
            + 4 * std::mem::size_of::<AHashSet<&str>>() as u64,
    );
    for (name, work, module, peak) in [
        ("first graph row", 0, 0, 0),
        ("later graph row", 1, 1, 0),
        ("graph edge", 2, 1, 0),
        ("scheduler entry", graph_work, modules.root, 0),
        (
            "scheduler states work",
            graph_work + 2,
            modules.root,
            storage.order,
        ),
        (
            "last interface table row",
            total - published_rows - declaration_work - analyzer_work - 1,
            2,
            storage.peak(),
        ),
        (
            "last published export",
            total - body_work - 1,
            2,
            registration_peak,
        ),
    ] {
        let live_facts = live_facts_for_test();
        let mut ledger = ledger(work, MEMORY);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let error = with_analyzed_modules(&programs, &modules, &mut budget, |_, _| {
            panic!("{name} refusal cannot reach client");
        })
        .unwrap_err();
        assert_eq!(
            error,
            AdmittedModuleSemanticError {
                module,
                error: AdmittedSemanticError::Resources(AllocationError::Budget(
                    BudgetError::WorkExhausted(WorkDomain::Baseline)
                )),
            },
            "{name} with work {work}"
        );
        assert_eq!(live_facts_for_test(), live_facts, "{name}");
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0, "{name}");
        assert_eq!(
            budget.retained_bytes(AllocationClass::Retained),
            0,
            "{name}"
        );
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL, "{name}");
        assert_eq!(ledger.peak_retained_bytes(), SENTINEL + peak, "{name}");
    }
}

#[test]
fn module_callback_error_panic_and_deadline_preserve_parent_ownership() {
    let modules = interface_graph();
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = INTERFACE_SOURCES
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let storage = ModuleStorage::new(&programs, &modules);
    let live_facts = live_facts_for_test();
    let mut ledger = ledger(WORK, MEMORY);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let parent = budget.string(AllocationClass::Retained, "parent").unwrap();
    let output = with_analyzed_modules(&programs, &modules, &mut budget, |_, scope| {
        assert_eq!(
            scope.retained_bytes(AllocationClass::Scratch),
            storage.checked_live()
        );
        assert_eq!(live_facts_for_test(), live_facts + programs.len());
        Err::<(), _>("client refusal")
    })
    .unwrap();
    assert_eq!(output, Err("client refusal"));
    assert_eq!(live_facts_for_test(), live_facts);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    assert_eq!(budget.retained_bytes(AllocationClass::Retained), 6);
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let _ = with_analyzed_modules(&programs, &modules, &mut budget, |_, scope| {
            assert_eq!(
                scope.retained_bytes(AllocationClass::Scratch),
                storage.checked_live()
            );
            assert_eq!(live_facts_for_test(), live_facts + programs.len());
            panic!("injected module client panic");
        });
    }));
    assert!(failure.is_err());
    assert_eq!(live_facts_for_test(), live_facts);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    assert_eq!(budget.retained_bytes(AllocationClass::Retained), 6);
    let output = with_analyzed_modules(&programs, &modules, &mut budget, |_, scope| {
        scope.with_ledger(|owner| {
            owner
                .unwrap()
                .0
                .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
        });
        scope.work(crate::compilation_policy::WorkKind::Analysis, 0)
    })
    .unwrap();
    assert_eq!(
        output,
        Err(AllocationError::Budget(BudgetError::DeadlineExceeded))
    );
    assert_eq!(live_facts_for_test(), live_facts);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    assert_eq!(budget.retained_bytes(AllocationClass::Retained), 6);
    let error = with_analyzed_modules(&programs, &modules, &mut budget, |_, _| {
        panic!("expired checker cannot reach client");
    })
    .unwrap_err();
    assert_eq!(
        error,
        AdmittedModuleSemanticError {
            module: 0,
            error: AdmittedSemanticError::Resources(AllocationError::Budget(
                BudgetError::DeadlineExceeded
            )),
        }
    );
    assert_eq!(parent, "parent");
    drop(parent);
    budget.release(AllocationClass::Retained, 6).unwrap();
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn module_graph_and_semantic_diagnostics_are_unchanged() {
    let sources = ["import \"./other\";int first=1;", "int second=missing;"];
    let valid_graph = graph(&sources, &[&[1], &[]], &[1, 0]);
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let empty = graph(&[], &[], &[]);
    let mut invalid_root = valid_graph.clone();
    invalid_root.root = programs.len();
    let mut invalid_target = valid_graph.clone();
    invalid_target.modules[0].dependencies[0] = programs.len();
    let mut invalid_count = valid_graph.clone();
    invalid_count.modules[0].dependencies.clear();
    let mut invalid_order = valid_graph.clone();
    invalid_order.dependency_order.reverse();
    for (programs, modules, memory) in [
        (programs.as_slice(), &valid_graph, MEMORY),
        (&[][..], &empty, SENTINEL),
        (programs.as_slice(), &empty, SENTINEL),
        (programs.as_slice(), &invalid_root, SENTINEL),
        (programs.as_slice(), &invalid_target, SENTINEL),
        (programs.as_slice(), &invalid_count, SENTINEL),
        (programs.as_slice(), &invalid_order, MEMORY),
    ] {
        let expected = analyze_modules(programs, modules).unwrap_err();
        let mut ledger = ledger(WORK, memory);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let error = with_analyzed_modules(programs, modules, &mut budget, |_, _| {
            panic!("invalid module graph cannot reach client")
        })
        .unwrap_err();
        assert_eq!(
            error,
            AdmittedModuleSemanticError {
                module: expected.module,
                error: AdmittedSemanticError::Semantic(expected.error)
            }
        );
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
    }
}

#[test]
fn module_callback_transfers_prepared_output_without_retaining_checker_or_ast() {
    let sources = [
        "import {answer} from \"./answer\";print(answer());",
        "export int answer(){return 17;}",
    ];
    let modules = graph(&sources, &[&[1], &[]], &[1, 0]);
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let identities: Vec<_> = programs
        .iter()
        .map(|program| program.source_identity().clone())
        .collect();
    let live_facts = live_facts_for_test();
    let mut ledger = ledger(WORK, MEMORY);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let prepared = with_analyzed_modules(&programs, &modules, &mut budget, |checked, budget| {
        assert_eq!(live_facts_for_test(), live_facts + programs.len());
        crate::semantic_program::from_checked_modules_admitted(&programs, checked, budget)
    })
    .unwrap()
    .unwrap();
    assert_eq!(live_facts_for_test(), live_facts);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
    drop(budget);
    drop(programs);
    drop(arena);
    for (module, identity) in prepared.program().modules().iter().zip(&identities) {
        assert!(module.source.same(identity));
    }
    let javascript = prepared
        .program()
        .to_javascript()
        .unwrap()
        .render(crate::structured_js::PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &javascript])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "17\n");
    prepared.discard(&mut ledger);
    assert_eq!(live_facts_for_test(), live_facts);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn borrowed_source_payloads_reuse_the_original_module_checker() {
    let sources = [
        "import {answer} from \"./answer\";print(answer());",
        "export int answer(){return 17;}",
    ];
    let owned = graph(&sources, &[&[1], &[]], &[1, 0]);
    let borrowed = ModuleSet {
        modules: owned
            .modules
            .iter()
            .map(|module| ModuleSource {
                path: module.path.clone(),
                source: module.source.as_str(),
                dependencies: module.dependencies.clone(),
                foreign_dependencies: Vec::new(),
                dynamic_dependencies: Vec::new(),
                offset: module.offset,
            })
            .collect(),
        dependency_order: owned.dependency_order.clone(),
        root: owned.root,
        eager: owned.eager.clone(),
        for_of_specialize_family: owned.for_of_specialize_family,
    };
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let expected = analyze_modules(&programs, &owned).unwrap();
    let expected_debug = format!("{expected:?}");
    drop(expected);
    let mut ledger = ledger(WORK, MEMORY);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    with_analyzed_modules(&programs, &borrowed, &mut budget, |checked, _| {
        assert_eq!(format!("{checked:?}"), expected_debug);
        for (index, program) in programs.iter().enumerate() {
            assert!(checked
                .source(index)
                .unwrap()
                .same(program.source_identity()));
        }
    })
    .unwrap();
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}
