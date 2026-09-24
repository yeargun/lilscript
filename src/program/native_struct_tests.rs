//! Fixed value-semantics observations precede native/JavaScript comparison.
//! Generated C is executed by both qualified compilers at O0/O2 and under
//! UBSan through the existing harness.
use super::native_tests::{compile_and_execute, execute};
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain, WorkKind,
};
use crate::output_budget::AllocationError;
use crate::js::selection::{Objective, Plan, Style};
use sha2::{Digest, Sha256};
use std::process::Command;

const MEMORY: u64 = 256_000_000;
const NESTED: &str = r#"
struct Leaf { int number; bool enabled; string text; }
struct Box { Leaf leaf; float weight; }
Box change(Box value) {
    value.leaf.number = value.leaf.number + 100;
    value.leaf.enabled = false;
    value.leaf.text = "Q\0\udfff";
    value.weight = value.weight + 1.0;
    return value;
}
Leaf input = Leaf{7, true, "A\0\ud800"};
Box first = Box{input, 1.5};
Box second = first;
second.leaf.number = 9;
second.leaf.enabled = false;
second.leaf.text = "Z";
Box third = change(first);
print(input.number); print(first.leaf.number); print(second.leaf.number); print(third.leaf.number);
print(first.leaf.enabled); print(second.leaf.enabled); print(third.leaf.enabled);
print(first.leaf.text.length); print(third.leaf.text.length);
print(first.leaf.text.charCodeAt(2)); print(third.leaf.text.charCodeAt(2));
print(first.weight == 1.5); print(third.weight == 2.5);
"#;
const ORDERED: &str = r#"
struct Pair { int left; int right; }
int mark(int value) { print(value); return value; }
Pair make(int start) { return Pair{mark(start), mark(start + 1)}; }
Pair increment(Pair value) { value.left = value.left + 100; return value; }
Pair a = make(10);
Pair b = increment(a);
print(a.left); print(b.left);
Pair saved = (a = b);
a.left = 50;
b.right = 70;
print(saved.left); print(saved.right); print(a.right); print(b.right);
Pair selected = if(true){a}else{b};
selected.right = 90;
print(a.right); print(selected.right);
a.left = mark(30);
print(a.left);
print(make(20).right);
"#;
const EMPTY: &str = r#"
struct Empty {}
struct Packet { Empty empty; int value; }
Empty echo(Empty value) { return value; }
Packet make(int value) { return Packet{echo(Empty{}), value}; }
Packet original = make(3);
Packet copy = original;
copy.empty = echo(Empty{});
copy.value = 4;
print(original.value); print(copy.value);
print(make(5).value);
"#;

fn compilation<'src>(optional_memory: u64) -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: MEMORY - optional_memory,
                retained_bytes: MEMORY,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 3 },
    )
    .unwrap()
}
fn checked<R>(source: &str, inspect: impl FnOnce(Program<'_>) -> R) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    inspect(program)
}
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
pub(super) fn qualify(name: &str, source_text: &str, expected: &str) {
    checked(source_text, |program| {
        let mut compilation = compilation(MEMORY);
        let source = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = crate::config::ProjectConfig::default()
            .resolve_policy(CompilationRequest::Native)
            .unwrap();
        let before = compilation.ledger().retained_bytes();
        let codecs = compilation.ledger().work_by_kind(WorkKind::Codec);
        let c = compilation
            .with_native_c(source, &policy, WorkDomain::Baseline, |output| {
                output.take_c()
            })
            .unwrap();
        assert_eq!(compilation.ledger().retained_bytes(), before);
        assert_eq!(compilation.ledger().work_by_kind(WorkKind::Codec), codecs);
        assert_eq!(compilation.checkpoint_count(), 1);
        let executions = compile_and_execute(&c, expected, name);
        let config: crate::config::ProjectConfig =
            toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                // The execution harness runs this artifact as a strict module.
                preserve_root_exports: true,
            })
            .unwrap();
        let candidate = compilation
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let javascript = compilation.with_javascript_output(candidate, &policy, |output| {
            let mut rows = Vec::new();
            for style in [Style::Global, Style::Scoped, Style::Source] {
                let artifact = output.render(&Plan::new(style))?;
                output.with_artifact(artifact, |view| {
                    let ran = execute(Command::new("node").args(["--input-type=module", "-e", view.javascript]));
                    assert!(ran.status.success(), "{name}/{style:?}: {}", String::from_utf8_lossy(&ran.stderr));
                    assert!(ran.stderr.is_empty());
                    assert_eq!(String::from_utf8_lossy(&ran.stdout), expected, "{name}/{style:?}");
                })?;
                let raw = output.measure(artifact, Objective::Raw)?;
                let gzip9 = output.measure(artifact, Objective::Gzip)?;
                let brotli11 = output.measure(artifact, Objective::Brotli)?;
                let text = output.take_artifact(artifact)?;
                rows.push(serde_json::json!({"style":format!("{style:?}"), "javascript_sha256":digest(&text),
                    "javascript":text,"raw":raw,"gzip9":gzip9,"brotli11":brotli11}));
            }
            Ok::<_, CandidateError>(rows)
        }).unwrap().unwrap();
        eprintln!(
            "native-struct-artifact {}",
            serde_json::json!({
                "case":name,"source":source_text,"source_sha256":digest(source_text),"expected":expected,
                "c":c,"c_sha256":digest(&c),"executions":executions,"javascript":javascript,
                "qualification":"semantic values and places; fixed trace; two qualified C compilers and Node"
            })
        );
        assert_eq!(compilation.finish().retained_bytes(), 0);
    });
}

#[test]
fn native_nested_value_copies_preserve_originals_and_immutable_utf16_backing() {
    qualify(
        "nested-values",
        NESTED,
        "7\n7\n9\n107\ntrue\nfalse\nfalse\n3\n3\n55296\n57343\ntrue\ntrue\n",
    );
}
#[test]
fn native_struct_arguments_returns_assignment_values_and_field_effect_order() {
    qualify(
        "ordered-values",
        ORDERED,
        "10\n11\n10\n110\n110\n11\n11\n70\n11\n90\n30\n30\n20\n21\n21\n",
    );
}
#[test]
fn native_empty_structs_have_no_observable_padding_field() {
    qualify("empty-values", EMPTY, "3\n4\n5\n");
}
#[test]
fn native_struct_reference_fields_remain_explicitly_unsupported() {
    for source in [
        "struct Holder { int[] items; } Holder value = Holder{[1,2]}; print(value.items.length);",
        "struct Holder { Record<int> items; } Holder value = Holder{record{item:1}}; print(value.items.item);",
    ] {
        checked(source, |program| {
            let mut compilation = compilation(MEMORY);
            let source = compilation.adopt_checked(program, WorkDomain::Baseline).unwrap();
            let policy = crate::config::ProjectConfig::default().resolve_policy(CompilationRequest::Native).unwrap();
            let before = compilation.ledger().retained_bytes();
            assert!(matches!(compilation.with_native_c(source, &policy, WorkDomain::Baseline, |_| {
                panic!("unsupported native reference field reached output");
            }), Err(NativeError::Unsupported { .. })));
            assert_eq!(compilation.ledger().retained_bytes(), before);
            assert_eq!(compilation.finish().retained_bytes(), 0);
        });
    }
}
#[test]
fn refused_native_struct_output_releases_layouts_and_partial_text() {
    // A large immutable string field forces partial C buffer growth after the
    // admitted nested schema/place plan has been built. The baseline allowance
    // must still finish this exact source after the optional attempt fails.
    let source = format!("struct Text {{ string value; }} struct Box {{ Text text; }} Box value=Box{{Text{{\"{}\"}}}}; print(value.text.value.length);", "q".repeat(12_000));
    checked(&source, |program| {
        let mut compilation = compilation(16_384);
        let source = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = crate::config::ProjectConfig::default()
            .resolve_policy(CompilationRequest::Native)
            .unwrap();
        let before = compilation.ledger().clone();
        assert!(matches!(
            compilation.with_native_c(source, &policy, WorkDomain::Optional, |_| {
                panic!("partial native aggregate output escaped");
            }),
            Err(NativeError::Allocation(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Optional)
            )))
        ));
        assert_eq!(
            compilation.ledger().retained_bytes(),
            before.retained_bytes()
        );
        assert_eq!(
            compilation.ledger().work_used(WorkDomain::Baseline),
            before.work_used(WorkDomain::Baseline)
        );
        assert!(
            compilation.ledger().work_by_kind(WorkKind::Render)
                - before.work_by_kind(WorkKind::Render)
                > super::native_runtime::PROLOGUE.len() as u64 * 3
        );
        compilation
            .with_native_c(source, &policy, WorkDomain::Baseline, |output| {
                assert!(!output.as_str().is_empty());
            })
            .unwrap();
        assert_eq!(
            compilation.ledger().retained_bytes(),
            before.retained_bytes()
        );
        assert_eq!(compilation.finish().retained_bytes(), 0);
    });
}

#[test]
fn native_place_check_rejects_uninitialized_root_before_rhs_without_leaking() {
    checked(
        "struct Point { int value; } Point point=Point{1}; point.value=2; print(point.value);",
        |mut program| {
            let root = program.initialization()[0];
            let mut working = program.units[root.index()].clone().into_working();
            let data = working.get_mut();
            let check = data
                .operations
                .iter()
                .enumerate()
                .find_map(|(index, operation)| {
                    matches!(operation.kind, OperationKind::CheckPlace(_))
                        .then(|| OpId::from_index(index).unwrap())
                })
                .expect("field assignment retains an ordered storage check");
            assert!(data.operations[check.index()].result.is_none());
            assert!(data
                .operands(data.operations[check.index()].operands)
                .unwrap()
                .is_empty());
            let entry = &mut data.regions[data.entry.index()].operations;
            let position = entry.iter().position(|id| *id == check).unwrap();
            entry.remove(position);
            entry.insert(0, check);
            program.units[root.index()] = working.freeze();
            // The program may express a runtime TDZ failure. This
            // direct native slice must refuse it before C emission, since its
            // ordinary C local variables do not implement dynamic TDZ state.
            program.verify().unwrap();
            let mut compilation = compilation(MEMORY);
            let source = compilation
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let policy = crate::config::ProjectConfig::default()
                .resolve_policy(CompilationRequest::Native)
                .unwrap();
            let before = compilation.ledger().retained_bytes();
            let codecs = compilation.ledger().work_by_kind(WorkKind::Codec);
            let result = compilation.with_native_c(source, &policy, WorkDomain::Baseline, |_| {
                panic!("an uninitialized checked place reached native output");
            });
            assert!(
                matches!(
                    result,
                    Err(NativeError::Unsupported {
                        feature: "native cell access before initialization",
                        ..
                    })
                ),
                "{result:?}"
            );
            assert_eq!(compilation.ledger().retained_bytes(), before);
            assert_eq!(compilation.ledger().work_by_kind(WorkKind::Codec), codecs);
            assert_eq!(compilation.finish().retained_bytes(), 0);
        },
    );
}
