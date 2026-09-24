//! The call graph view (M6.1): resolution, escapes, complete call sets and
//! components, under module and script sealing.
use super::call_graph::{CallGraph, Callee, EdgeKind, Seal};
use super::*;

fn checked<T>(source: &str, inspect: impl FnOnce(&Program<'_>) -> T) -> T {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    inspect(&program)
}

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

/// The bodies `caller` calls directly, in call order.
fn callees(graph: &CallGraph, program: &Program<'_>, caller: UnitId) -> Vec<Callee> {
    let data = program.unit(caller).unwrap();
    (0..data.calls.len())
        .map(|call| graph.callee(caller, CallId::from_index(call).unwrap()))
        .collect()
}

#[test]
fn calls_resolve_through_declarations_and_single_initialization_aliases() {
    checked(
        "void warningImpl(bool check,string message){}\
         func(bool,string)->void warning=warningImpl;\
         int twice(int x){return x+x;}\
         void run(){warning(true,\"a\");twice(2);warningImpl(false,\"b\");}\
         run();",
        |program| {
            let graph = CallGraph::build(program, Seal::Module);
            let run = body(program, "run");
            let implementation = body(program, "warningImpl");
            let twice = body(program, "twice");
            assert_eq!(
                callees(&graph, program, run),
                vec![
                    Callee::Unit(implementation),
                    Callee::Unit(twice),
                    Callee::Unit(implementation)
                ]
            );
            // Every call of the body is direct: its call set is complete.
            let callers = graph.complete_callers(implementation).unwrap();
            assert_eq!(callers.len(), 2);
            assert!(callers.iter().all(|edge| edge.caller == run));
            assert!(!graph.address_taken(twice));
        },
    );
}

#[test]
fn script_frames_leave_root_storage_unsealed() {
    checked(
        "int twice(int x){return x+x;}void run(){twice(2);}run();",
        |program| {
            let graph = CallGraph::build(program, Seal::StructuralOnly);
            let run = body(program, "run");
            // Another script can replace a global binding.
            assert_eq!(callees(&graph, program, run), vec![Callee::Unknown]);
            // A local closure is sealed in any frame.
        },
    );
    checked(
        "int outer(int y){func(int)->int add=(int x)=>x+y;return add(1);}print(outer(2));",
        |program| {
            let graph = CallGraph::build(program, Seal::StructuralOnly);
            let outer = body(program, "outer");
            assert!(matches!(
                callees(&graph, program, outer).as_slice(),
                [Callee::Unit(_)]
            ));
        },
    );
}

#[test]
fn reassigned_parameter_and_host_callees_stay_unknown() {
    checked(
        "extern int host(int x);pure extern int pureHost(int x);\
         int one(){return 1;}int two(){return 2;}\
         func()->int pick=one;\
         int apply(func()->int f){return f();}\
         void run(){pick=two;pick();host(1);pureHost(2);apply(one);}run();",
        |program| {
            let graph = CallGraph::build(program, Seal::Module);
            let run = body(program, "run");
            let apply = body(program, "apply");
            let calls = callees(&graph, program, run);
            assert_eq!(calls[0], Callee::Unknown, "reassigned storage");
            assert!(matches!(calls[1], Callee::Extern { pure: false, .. }));
            assert!(matches!(calls[2], Callee::Extern { pure: true, .. }));
            assert_eq!(calls[3], Callee::Unit(apply));
            // A parameter can hold any callable.
            assert_eq!(callees(&graph, program, apply), vec![Callee::Unknown]);
            // `one` escapes as an argument and through `pick`: no complete set.
            let one = body(program, "one");
            assert!(graph.address_taken(one));
            assert!(graph.complete_callers(one).is_none());
        },
    );
}

#[test]
fn exported_bodies_escape_to_the_host() {
    checked(
        "export int shown(int x){return x;}int hidden(int x){return x;}\
         export int run(){return shown(1)+hidden(2);}",
        |program| {
            let graph = CallGraph::build(program, Seal::Module);
            assert!(graph.address_taken(body(program, "shown")));
            assert!(!graph.address_taken(body(program, "hidden")));
            assert_eq!(
                graph
                    .complete_callers(body(program, "hidden"))
                    .unwrap()
                    .len(),
                1
            );
        },
    );
}

#[test]
fn components_put_callees_first_and_mark_recursion() {
    checked(
        "bool even(int n){if(n==0){return true;}return odd(n-1);}\
         bool odd(int n){if(n==0){return false;}return even(n-1);}\
         int fact(int n){if(n<=1){return 1;}return n*fact(n-1);}\
         int leaf(int n){return n+1;}\
         int top(int n){if(even(n)){return fact(n);}return leaf(n);}\
         print(top(4));",
        |program| {
            let graph = CallGraph::build(program, Seal::Module);
            let (even, odd, fact, leaf, top) = (
                body(program, "even"),
                body(program, "odd"),
                body(program, "fact"),
                body(program, "leaf"),
                body(program, "top"),
            );
            assert!(graph.recursive(even) && graph.recursive(odd));
            assert!(graph.recursive(fact));
            assert!(!graph.recursive(leaf) && !graph.recursive(top));
            let position = |unit: UnitId| {
                graph
                    .components()
                    .iter()
                    .position(|component| component.contains(&unit))
                    .unwrap()
            };
            assert_eq!(position(even), position(odd));
            for callee in [even, fact, leaf] {
                assert!(position(callee) < position(top));
            }
        },
    );
}

#[test]
fn array_callbacks_are_call_edges() {
    checked(
        "int total=0;void add(int x){total=total+x;}\
         void run(int[] xs){xs.forEach((int x)=>add(x));}run([1,2]);print(total);",
        |program| {
            let graph = CallGraph::build(program, Seal::Module);
            let run = body(program, "run");
            let callback = graph
                .calls_from(run)
                .iter()
                .find(|edge| edge.kind == EdgeKind::Callback)
                .expect("forEach runs its callback")
                .callee;
            assert!(graph
                .calls_from(callback)
                .iter()
                .any(|edge| edge.callee == body(program, "add")));
            // A callback is invoked by the intrinsic, not by a call operation.
            assert!(graph.complete_callers(callback).is_none());
        },
    );
}
