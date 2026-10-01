//! S2's focused semantic matrix. Fixed traces are independent of the optimizer;
//! the interpreter and emitted JavaScript must both agree where supported.
use super::*;
use std::process::Command;
const STORAGE: RuleRequest = RuleRequest {
    unroll: true, pool: true,
    fold: false,
    dead_code: false,
    inline: false,
    scalar: true,
    native: false,
    pristine_builtins: false,
    seal: Seal::Module,
};
pub(super) fn checked(
    source: &str,
    expected: &str,
    request: RuleRequest,
    inspect: impl FnOnce(&Program<'_>, RuleReceipt),
) {
    let arena = bumpalo::Bump::new();
    let syntax =
        crate::parse_source(&arena, source).unwrap_or_else(|e| panic!("parse: {e:?}\n{source}"));
    let semantics = crate::analyze(&syntax).unwrap_or_else(|e| panic!("check: {e:?}\n{source}"));
    let reference = crate::interpreter::interpret_program(&syntax, &semantics);
    if let Ok(reference) = reference {
        assert_eq!(reference, expected, "interpreter\n{source}");
    }
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let (program, receipt) = optimize(program, request).unwrap_or_else(|e| panic!("{e}\n{source}"));
    program.verify().unwrap();
    let text = super::super::javascript::lower(&program)
        .unwrap()
        .render(crate::js::PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &text])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{text}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        expected,
        "{source}\n{text}"
    );
    inspect(&program, receipt);
}
#[test]
fn s2_aliases_share_mutable_array_fields_and_keep_effect_order() {
    checked(
        "int mark(int v){print(v);return v;}int[] a=[mark(1),mark(2)];int[] b=a;b[0]=mark(3);print(a[0]);print(b[1]);",
        "1\n2\n3\n3\n2\n",
        STORAGE,
        |_, r| assert!(r.scalarized_allocations > 0, "{r:?}"),
    );
}
#[test]
fn s2_capture_has_one_bank_per_loop_activation() {
    checked(
        "func()->int make(int n){int[] state=[n,0];return ()=>{state[0]+=1;return state[0];};}auto a=make(1);auto b=make(10);print(a());print(b());print(a());",
        "2\n11\n3\n",
        STORAGE,
        |_, r| assert!(r.scalarized_allocations > 0, "{r:?}"),
    );
    checked(
        "int total=0;for(int i=0;i<3;i+=1){int[] a=[i,1];auto read=()=>a[0];a[0]+=2;total+=read();}print(total);",
        "9\n",
        STORAGE,
        |_, r| assert!(r.scalarized_allocations > 0, "{r:?}"),
    );
}
#[test]
fn s2_whole_identity_dynamic_keys_and_escape_retain_storage() {
    for (source, expected) in [
        ("int[] a=[1,2];int[] b=a;print(a==b);", "true\n"),
        (
            "int read(int[] a){return a[0];}int[] a=[1,2];print(read(a));",
            "1\n",
        ),
        ("int k=1;int[] a=[1,2];print(a[k]);", "2\n"),
    ] {
        checked(source, expected, STORAGE, |_, r| {
            assert_eq!(r.scalarized_allocations, 0, "{r:?}")
        });
    }
}
#[test]
fn s2_scalar_permission_is_independent_of_field_folding() {
    let source = "int[] a=[1,2];a[0]=3;print(a[0]);print(a[1]);";
    checked(
        source,
        "3\n2\n",
        RuleRequest {
            scalar: false,
            native: false,
            ..STORAGE
        },
        |_, r| assert_eq!(r.scalarized_allocations, 0),
    );
    checked(source, "3\n2\n", STORAGE, |_, r| {
        assert_eq!(r.folded_fields, 0);
        assert!(r.scalarized_allocations > 0);
    });
}
#[test]
fn s2_private_class_fields_and_alias_mutation() {
    checked(
        "class State{int x=1;int y=2;}State a=new State();State b=a;b.x=4;print(a.x);print(a.y);",
        "4\n2\n",
        RuleRequest {
            unroll: true, pool: true,
            fold: true,
            dead_code: true,
            inline: true,
            ..STORAGE
        },
        |_, r| assert!(r.scalarized_allocations > 0, "{r:?}"),
    );
}
#[test]
fn s2_namespace_reads_and_calls_remove_private_receiver() {
    checked(
        "JsValue f=(JsValue n)=>JS.add(n,1);JsValue ns=JS.object(\"f\",f,\"n\",7);print(JS.invoke(ns,\"f\",3));print(ns[\"n\"]);",
        "4\n7\n",
        RuleRequest {
            unroll: true, pool: true,
            fold: true,
            dead_code: true,
            ..STORAGE
        },
        |_, r| {
            assert!(r.exposed_allocations > 0, "{r:?}");
            assert!(r.flattened_namespace_calls > 0, "{r:?}");
            assert!(r.scalarized_allocations > 0 || r.folded_fields > 0, "{r:?}");
        },
    );
}
#[test]
fn s2_field_constants_respect_alias_writers() {
    checked(
        "int[] a=[1,2];int[] b=a;b[0]=5;print(a[0]);print(a[1]);",
        "5\n2\n",
        RuleRequest {
            unroll: true, pool: true,
            fold: true,
            scalar: false,
            native: false,
            ..STORAGE
        },
        |_, r| assert!(r.folded_fields > 0, "{r:?}"),
    );
}
#[test]
fn s2_open_script_roots_remain_observable() {
    checked(
        "int[] a=[1,2];a[0]=3;print(a[0]);",
        "3\n",
        RuleRequest {
            seal: Seal::StructuralOnly,
            ..STORAGE
        },
        |_, r| assert_eq!(r.scalarized_allocations, 0, "{r:?}"),
    );
}
#[test]
fn s2_record_absence_and_special_keys_keep_normalization() {
    checked(
        "Record<int> r=record{a:1};r[\"b\"]=2;print(r[\"a\"]);print(r[\"b\"]);print(r[\"missing\"]==null);",
        "1\n2\ntrue\n",
        RuleRequest {
            unroll: true, pool: true,
            fold: true,
            dead_code: true,
            ..STORAGE
        },
        |_, _| {},
    );
}

#[test]
fn s2_record_aliases_share_captured_writes_and_absence() {
    checked(
        "Record<int> state=record{x:1};Record<int> alias=state;auto update=()=>{alias.x=4;};update();print(state.x);print(alias[\"missing\"]==null);",
        "4\ntrue\n",
        RuleRequest {
            dead_code: true,
            ..STORAGE
        },
        |_, r| assert!(r.elided_record_aliases > 0, "{r:?}"),
    );
}
#[test]
fn s2_closure_reentry_sees_completed_allocation() {
    checked(
        "int[] a=[1,2];auto change=()=>{a[1]=9;return 3;};a[0]=change();print(a[0]);print(a[1]);",
        "3\n9\n",
        RuleRequest {
            dead_code: true,
            ..STORAGE
        },
        |_, r| assert!(r.scalarized_allocations > 0, "{r:?}"),
    );
}
#[test]
fn s2_unread_fields_preserve_calls_and_overwritten_stores() {
    checked(
        "int tick(int x){print(x);return x;}int[] a=[tick(1),tick(2)];a[0]=tick(3);a[1]=tick(4);print(a[0]);",
        "1\n2\n3\n4\n3\n",
        RuleRequest {
            dead_code: true,
            ..STORAGE
        },
        |_, r| {
            assert!(r.scalarized_allocations > 0);
            assert!(r.removed_field_stores > 0, "{r:?}");
        },
    );
}

#[test]
fn s2_store_collection_keeps_record_absence_and_reads() {
    checked(
        "Record<int> state=record{};state.x=3;state.y=4;print(state.x);print(state.y);print(state.z==null);",
        "3\n4\ntrue\n",
        RuleRequest {
            dead_code: true,
            scalar: false,
            native: false,
            ..STORAGE
        },
        |_, r| assert_eq!(r.collected_field_stores, 2, "{r:?}"),
    );
}
#[test]
fn s2_fresh_field_collection_obeys_the_pristine_contract() {
    for pristine in [false, true] {
        checked(
            "JsValue state=JS.object();state[\"x\"]=3;state[\"y\"]=4;print(state[\"x\"]);print(state[\"y\"]);",
            "3\n4\n",
            RuleRequest {
                unroll: true, pool: true,
                fold: true,
                dead_code: true,
                scalar: false,
                native: false,
                pristine_builtins: pristine,
                ..STORAGE
            },
            |_, r| {
                assert_eq!(
                    r.collected_field_stores,
                    if pristine { 2 } else { 0 },
                    "{r:?}"
                )
            },
        );
    }
}

#[test]
fn s2_early_capture_and_throwing_initializers_keep_failure_order() {
    for source in [
        "int invoke(func()->int read){return read();}func()->int make(){int[] state=[1,invoke(()=>state[0])];return ()=>state[0];}try{print(make()());}catch(auto error){print(99);}",
        "int fail(){throw \"stop\";}try{int[] state=[1,fail()];print(state[0]);}catch(auto error){print(99);}",
    ] {
        checked(source,"99\n",RuleRequest{fold:true,dead_code:true,..STORAGE},|_,_|{});
    }
}

#[test]
fn s2_nullable_control_join_does_not_claim_an_allocation() {
    checked("void check(bool yes){int[]? a=if(yes){[1,2]}else{null};if(a!=null){print(a[0]);}else{print(99);}}check(true);check(false);","1\n99\n",STORAGE,|_,r|assert_eq!(r.scalarized_allocations,0));
}

#[test]
fn s2_record_undefined_payload_keeps_nullish_normalization() {
    checked(
        "Record<JsValue> state=record{x:JS.undefined()};print(JS.typeOf(state.x));",
        "object\n",
        RuleRequest {
            unroll: true, pool: true,
            fold: true,
            dead_code: true,
            ..STORAGE
        },
        |_, _| {},
    );
}

#[test]
fn s2_scalar_cells_keep_nullable_declared_storage_types() {
    checked("class State{int? value=null;}State state=new State();state.value=7;if(state.value!=null){print(state.value);}", "7\n", STORAGE, |_,r|assert!(r.scalarized_allocations>0,"{r:?}"));
}
