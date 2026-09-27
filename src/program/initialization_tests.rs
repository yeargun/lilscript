//! The initialization owner (M6.5): settled root cells, the first point
//! each body may run at, and the per-access answer effects and liveness
//! read, with the D3.7 refusals.
use super::call_graph::Seal;
use super::initialization::{Moment, ProgramInitialization, RootPoint};
use super::publication::*;
use super::*;
use crate::check::ReadInitialization;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Plan, Style};
use std::sync::Arc;

fn source_program<'src>(arena: &'src bumpalo::Bump, source: &'src str) -> Program<'src> {
    let syntax = crate::parse_source(arena, source)
        .unwrap_or_else(|error| panic!("parse: {error:?}\n{source}"));
    let checked =
        crate::analyze(&syntax).unwrap_or_else(|error| panic!("check: {error:?}\n{source}"));
    let program = from_checked_source(&syntax, &checked)
        .unwrap_or_else(|error| panic!("convert: {error:?}\n{source}"));
    program.verify().unwrap();
    program
}

fn modules(sources: &[&str], dependencies: &[&[usize]], inspect: impl FnOnce(Program<'_>)) {
    let graph = crate::module::ModuleSet {
        modules: sources
            .iter()
            .zip(dependencies)
            .enumerate()
            .map(|(id, (source, deps))| crate::module::ModuleSource {
                path: format!("/initialization-{id}.lil").into(),
                source: (*source).into(),
                dependencies: deps.to_vec(),
                foreign_dependencies: Vec::new(),
                dynamic_dependencies: Vec::new(),
                offset: 0,
            })
            .collect(),
        dependency_order: static_order(dependencies),
        root: 0,
        eager: vec![true; sources.len()],
    };
    let arena = bumpalo::Bump::new();
    let syntax: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let checked = crate::check::analyze_modules(&syntax, &graph)
        .unwrap_or_else(|error| panic!("check: {error:?}"));
    let program = from_checked_modules(&syntax, &checked).unwrap();
    program.verify().unwrap();
    inspect(program);
}

/// Post-order from the entry, dependencies in import order.
fn static_order(dependencies: &[&[usize]]) -> Vec<usize> {
    fn visit(
        module: usize,
        dependencies: &[&[usize]],
        seen: &mut Vec<bool>,
        order: &mut Vec<usize>,
    ) {
        if seen[module] {
            return;
        }
        seen[module] = true;
        for &dependency in dependencies[module] {
            visit(dependency, dependencies, seen, order);
        }
        order.push(module);
    }
    let mut seen = vec![false; dependencies.len()];
    let mut order = Vec::new();
    visit(0, dependencies, &mut seen, &mut order);
    order
}

/// The body a named function or a function-valued binding denotes.
fn body(program: &Program<'_>, name: &str) -> UnitId {
    program
        .cells()
        .iter()
        .find_map(|cell| match cell.binding {
            CellBinding::Function(unit) if cell.name == name => Some(unit),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no function `{name}`"))
}

fn cell(program: &Program<'_>, name: &str) -> CellId {
    let index = program
        .cells()
        .iter()
        .position(|cell| cell.name == name && !cell.synthetic)
        .unwrap_or_else(|| panic!("no cell `{name}`"));
    CellId::from_index(index).unwrap()
}

/// Whether each load of `name` in `unit` is past the cell's initialization.
fn loads(
    program: &Program<'_>,
    facts: &ProgramInitialization,
    unit: UnitId,
    name: &str,
) -> Vec<bool> {
    let target = cell(program, name);
    let data = program.unit(unit).unwrap();
    data.operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| match operation.kind {
            OperationKind::Load(place) if data.places[place.index()] == Place::Cell(target) => {
                Some(facts.initialized(program, unit, OpId::from_index(index).unwrap(), target))
            }
            _ => None,
        })
        .collect()
}

fn facts(program: &Program<'_>) -> Arc<ProgramInitialization> {
    program.initialization_facts(Seal::Module)
}

fn compile(source: &str, module: bool) -> String {
    let arena = bumpalo::Bump::new();
    let program = source_program(&arena, source);
    let config: crate::config::ProjectConfig =
        toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
    let policy: ResolvedPolicy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: module,
        })
        .unwrap();
    let mut compiler = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 20_000_000,
                optional_work: 20_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 32_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 4 },
    )
    .unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let candidate = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    compiler
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result)
        .unwrap()
}

#[test]
fn a_root_cell_settles_at_its_initializer_and_later_calls_read_it_initialized() {
    let arena = bumpalo::Bump::new();
    let program = source_program(&arena, "int K=7;int read(){return K;}print(read());");
    let facts = facts(&program);
    let settled = facts
        .settled(cell(&program, "K"))
        .expect("a settled root cell");
    let Some(Moment::Statement { unit, operation }) = facts.moment(settled) else {
        panic!("settled by a root statement");
    };
    assert_eq!(unit, program.initialization()[0]);
    assert!(matches!(
        program.unit(unit).unwrap().operations[operation.index()].kind,
        OperationKind::Initialize(cell) if program.cells()[cell.index()].name == "K"
    ));
    let read = body(&program, "read");
    assert!(
        facts.first_run(read) > settled,
        "{:?}",
        facts.first_run(read)
    );
    assert_eq!(loads(&program, &facts, read, "K"), [true]);
    // Named functions exist from instantiation.
    assert_eq!(
        facts.settled(cell(&program, "read")),
        Some(RootPoint::INSTANTIATION)
    );
    // The effects owner reads the answer: the read cannot throw.
    let effects = program.effects(Seal::Module);
    assert!(!effects.summary(read).unwrap().effects.may_throw);
}

#[test]
fn a_function_called_before_the_cell_it_reads_is_set_keeps_the_dead_zone() {
    // D3.7: `read` runs during the first statement, before `K` settles.
    let arena = bumpalo::Bump::new();
    let program = source_program(
        &arena,
        "int early=read();int K=4;int read(){return K;}print(early);",
    );
    let facts = facts(&program);
    let read = body(&program, "read");
    let settled = facts.settled(cell(&program, "K")).unwrap();
    assert!(facts.first_run(read) < settled);
    assert_eq!(loads(&program, &facts, read, "K"), [false]);
    let effects = program.effects(Seal::Module);
    assert!(effects.summary(read).unwrap().effects.may_throw);
    // A caller of `read` inherits the possible throw, directly or not.
    let javascript = compile(
        "int early=read();int K=4;int read(){return K;}print(early);",
        true,
    );
    assert!(javascript.contains("4"), "{javascript}");
}

#[test]
fn every_body_a_root_call_reaches_runs_at_that_statement() {
    let arena = bumpalo::Bump::new();
    let source = "int K=1;int inner(){return K;}int outer(){return inner();}\
                  int first=outer();int J=2;int late(){return J;}print(first+late());";
    let program = source_program(&arena, source);
    let facts = facts(&program);
    let (inner, outer, late) = (
        body(&program, "inner"),
        body(&program, "outer"),
        body(&program, "late"),
    );
    assert_eq!(facts.first_run(inner), facts.first_run(outer));
    assert!(facts.first_run(outer) > facts.settled(cell(&program, "K")).unwrap());
    assert!(facts.first_run(outer) < facts.settled(cell(&program, "J")).unwrap());
    assert!(facts.first_run(late) > facts.settled(cell(&program, "J")).unwrap());
    assert_eq!(loads(&program, &facts, inner, "K"), [true]);
    assert_eq!(loads(&program, &facts, late, "J"), [true]);
}

#[test]
fn a_body_handed_to_the_host_may_run_from_that_statement_on() {
    let arena = bumpalo::Bump::new();
    // Escaped at a host call before `K` settles: the host may call it there.
    let program = source_program(
        &arena,
        "extern void hostCall(func()->int f);hostCall(read_k);int K=3;\
         int read_k(){return K;}print(read_k());",
    );
    let facts = facts(&program);
    let read_k = body(&program, "read_k");
    assert!(facts.first_run(read_k) < facts.settled(cell(&program, "K")).unwrap());
    assert_eq!(loads(&program, &facts, read_k, "K"), [false]);
    // Escaped after `K` settles: no earlier statement can reach it.
    let arena = bumpalo::Bump::new();
    let program = source_program(
        &arena,
        "extern void hostCall(func()->int f);int K=3;hostCall(read_k);\
         int read_k(){return K;}print(read_k());",
    );
    let facts = self::facts(&program);
    assert_eq!(
        loads(&program, &facts, body(&program, "read_k"), "K"),
        [true]
    );
}

#[test]
fn a_body_escaped_before_a_throwing_statement_may_run_after_initialization_stops() {
    // Stored into a host object without a hazard, then a statement that may
    // throw: if it throws, initialization stops and the host may call the
    // body later, with `K` never initialized.
    let arena = bumpalo::Bump::new();
    let program = source_program(
        &arena,
        "extern int fail();JsValue keep=JS.object(\"f\",read_k);int n=fail();int K=3;\
         int read_k(){return K;}print(n);",
    );
    let facts = facts(&program);
    let read_k = body(&program, "read_k");
    let point = facts.first_run(read_k);
    assert!(facts.hazard(point));
    assert!(point < facts.settled(cell(&program, "K")).unwrap());
    assert_eq!(loads(&program, &facts, read_k, "K"), [false]);
    // Without a later hazard before `K`, the body runs only once `K` is set.
    let arena = bumpalo::Bump::new();
    let program = source_program(
        &arena,
        "JsValue keep=JS.object(\"f\",read_k);int K=3;int read_k(){return K;}print(K);",
    );
    let facts = self::facts(&program);
    assert_eq!(
        loads(&program, &facts, body(&program, "read_k"), "K"),
        [true]
    );
}

#[test]
fn exported_bodies_run_after_initialization_unless_host_code_can_call_them_early() {
    let source = "int K=3;export int read_k(){return K;}";
    let arena = bumpalo::Bump::new();
    let program = source_program(&arena, source);
    // A module's importers run once it has initialized.
    let facts = facts(&program);
    let read_k = body(&program, "read_k");
    assert_eq!(facts.first_run(read_k), RootPoint::END);
    assert_eq!(loads(&program, &facts, read_k, "K"), [true]);
    // A classic script's functions are globals other code may call at any
    // hazard, and there is none here: still the end.
    let script = program.initialization_facts(Seal::StructuralOnly);
    assert_eq!(script.first_run(read_k), RootPoint::END);
    // A host module may import the program back and call its exports
    // before any root statement runs.
    let mut hosted = program.clone();
    let module = hosted.entry.index();
    let foreign = cell(&hosted, "K");
    Arc::make_mut(&mut hosted.modules)[module]
        .foreign_imports
        .push(ForeignImport {
            cell: foreign,
            source: "./host.mjs".into(),
            imported: "host".into(),
        });
    let facts = self::facts(&hosted);
    assert_eq!(facts.moment(RootPoint::FIRST), Some(Moment::HostModules));
    assert_eq!(facts.first_run(read_k), RootPoint::FIRST);
    assert_eq!(loads(&hosted, &facts, read_k, "K"), [false]);
}

#[test]
fn a_script_global_function_may_run_at_the_first_hazard() {
    let arena = bumpalo::Bump::new();
    let program = source_program(
        &arena,
        "extern int input();int n=input();int K=2;int read_k(){return K;}print(n+read_k());",
    );
    // Under module sealing only the root call runs it, after `K`.
    let module = program.initialization_facts(Seal::Module);
    let read_k = body(&program, "read_k");
    assert_eq!(loads(&program, &module, read_k, "K"), [true]);
    // A script's function declarations are globals: host code running in
    // `input()` may call it before `K` is set.
    let script = program.initialization_facts(Seal::StructuralOnly);
    assert!(script.first_run(read_k) < script.settled(cell(&program, "K")).unwrap());
    assert_eq!(loads(&program, &script, read_k, "K"), [false]);
}

#[test]
fn an_import_cycle_that_calls_back_during_root_initialization_keeps_the_dead_zone() {
    // The entry imports `b`, which imports the entry back: `b` initializes
    // first and its root statement calls `read_a` before `KA` is set.
    modules(
        &[
            "import {fromB} from \"./initialization-1\";export int KA=5;\
             export int read_a(){return KA;}print(fromB);print(read_a());",
            "import {read_a} from \"./initialization-0\";export int fromB=read_a();",
        ],
        &[&[1], &[0]],
        |program| {
            let facts = facts(&program);
            let read_a = body(&program, "read_a");
            assert!(facts.first_run(read_a) < facts.settled(cell(&program, "KA")).unwrap());
            assert_eq!(loads(&program, &facts, read_a, "KA"), [false]);
        },
    );
    // Without the early call, the entry's function reads its cell initialized,
    // and a root read of another module's settled cell is initialized too.
    modules(
        &[
            "import {KB, readB} from \"./initialization-1\";export int KA=KB+1;\
             export int read_a(){return KA+readB();}print(read_a());",
            "export int KB=5;export int readB(){return KB;}",
        ],
        &[&[1], &[]],
        |program| {
            let facts = facts(&program);
            assert_eq!(
                loads(&program, &facts, body(&program, "read_a"), "KA"),
                [true]
            );
            assert_eq!(
                loads(&program, &facts, body(&program, "readB"), "KB"),
                [true]
            );
            let entry = program.initialization()[1];
            assert_eq!(loads(&program, &facts, entry, "KB"), [true]);
        },
    );
}

#[test]
fn zodlil_enum_constants_are_initialized_in_every_function_that_reads_them() {
    // zodlil's `export int KString = 0;` kinds, read by functions called
    // from later root statements and from its exported API. The tree does
    // not substitute them yet (formation must publish the settled statement
    // and each function's first point, M5.2); the facts hold.
    let arena = bumpalo::Bump::new();
    let program = source_program(
        &arena,
        "export int KString=0;export int KNumber=1;export int KCustom=2;\
         int kindOf(string t){if(t==\"int\"){return KNumber;}if(t==\"string\"){return KString;}return KCustom;}\
         int[] kinds=[kindOf(\"int\"),kindOf(\"x\")];\
         export int api(string t){return kindOf(t);}print(kinds[0]);",
    );
    let facts = facts(&program);
    let kind_of = body(&program, "kindOf");
    for name in ["KString", "KNumber", "KCustom"] {
        assert!(facts.first_run(kind_of) > facts.settled(cell(&program, name)).unwrap());
        assert_eq!(loads(&program, &facts, kind_of, name), [true], "{name}");
    }
    let effects = program.effects(Seal::Module);
    assert!(!effects.summary(kind_of).unwrap().effects.may_throw);
}

#[test]
fn motionlil_warning_calls_through_the_exported_binding_leave_the_output() {
    // motionlil's development-only `warning`/`invariant`: empty bodies held
    // by exported bindings, called from functions that run only after the
    // bindings settle. The calls go; an argument's own effect stays.
    let javascript = compile(
        "void warningImpl(bool check,string message){}\
         export func(bool,string)->void warning=warningImpl;\
         bool noted(bool value){print(\"noted\");return value;}\
         int clamp(int value){warning(value>10,\"clamped\");warning(noted(value>=0),\"negative\");return value;}\
         print(clamp(12));export int run(int value){return clamp(value);}",
        true,
    );
    assert!(!javascript.contains("clamped"), "{javascript}");
    assert!(!javascript.contains("negative"), "{javascript}");
    assert!(javascript.contains("\"noted\""), "{javascript}");
    assert!(
        javascript.contains("warning"),
        "the export stays: {javascript}"
    );
    // Called before the binding settles, the call stays whole: its read
    // throws there.
    let javascript = compile(
        "void warningImpl(bool check,string message){}\
         int early=clamp(1);\
         export func(bool,string)->void warning=warningImpl;\
         int clamp(int value){warning(value>10,\"clamped\");return value;}print(early);",
        true,
    );
    assert!(javascript.contains("clamped"), "{javascript}");
}

#[test]
fn the_checker_proves_occurrences_after_initialization_and_defers_the_rest() {
    let arena = bumpalo::Bump::new();
    let source = "int K=1;int later=K+1;\
                  int read(){return K;}\
                  int local(){int x=2;func()->int f=()=>x;return f();}\
                  func(int)->int recurse=(int n)=>{if(n==0){return 0;}return recurse(n-1);};\
                  print(later+read()+local()+recurse(2));";
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let symbol = |name: &str| {
        checked
            .symbols()
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap()
            .id
    };
    // A module binding read by a function is left to the call graph; one read
    // only at the top level after its declaration is proved.
    assert!(checked.symbol_is_observable_before_initialization(symbol("K")));
    assert!(!checked.symbol_is_observable_before_initialization(symbol("later")));
    // A local read by a closure created after it: proved.
    assert!(!checked.symbol_is_observable_before_initialization(symbol("x")));
    // A binding read inside its own initializer, from a nested function.
    assert!(checked.symbol_is_observable_before_initialization(symbol("recurse")));
    assert!(!checked.symbol_is_reassigned(symbol("recurse")));
    // Per occurrence: the top-level read of `K` is definite, the one in
    // `read` is not.
    let mut kinds = Vec::new();
    for item in syntax.items {
        let crate::ast::Item::Function(function) = item else {
            continue;
        };
        if function.name.name != "read" {
            continue;
        }
        let [crate::ast::Stmt::Return {
            value: Some(value), ..
        }] = function.body
        else {
            panic!("return K");
        };
        kinds.push(checked.read_initialization(value.id));
    }
    assert_eq!(kinds, [ReadInitialization::NeedsCallGraph]);
    // The cells carry the split flags.
    let program = from_checked_source(&syntax, &checked).unwrap();
    let recurse = &program.cells()[cell(&program, "recurse").index()];
    assert!(recurse.observable_before_initialization && !recurse.reassigned);
    let later = &program.cells()[cell(&program, "later").index()];
    assert!(!later.observable_before_initialization && !later.reassigned);
}

#[test]
fn a_parameter_default_is_read_at_every_call_site() {
    let arena = bumpalo::Bump::new();
    let source = "int K=4;int shifted(int x,int by=K){return x+by;}print(shifted(1));";
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let k = checked
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "K")
        .unwrap()
        .id;
    assert!(checked.symbol_is_observable_before_initialization(k));
}
