//! Executable qualification of the program's C client. The fixed
//! expected traces are the oracle; Node is an additional execution comparison.
//! The generated C is compiled unchanged, without an old backend or test shim.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits,
    WorkDomain, WorkKind,
};
use crate::js::selection::{Plan, Style};
use crate::output_budget::AllocationError;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 256_000_000;
const SIMPLE: &str = "int twice(int value){return value*2;}print(twice(21));";

pub(super) fn native_policy() -> ResolvedPolicy {
    crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::Native)
        .unwrap()
}

fn javascript_policy() -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str("[javascript]\n").unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: false,
        })
        .unwrap()
}

pub(super) fn compilation<'src>(optional_work: u64, memory: u64) -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 3 },
    )
    .unwrap()
}

fn checked<R>(
    source: &str,
    optional_work: u64,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let mut compilation = compilation(optional_work, MEMORY);
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let result = inspect(&mut compilation, source);
    assert_eq!(compilation.finish().retained_bytes(), 0);
    result
}

fn revisions(compilation: &Compilation<'_>, source: SemanticId) -> Vec<RevisionId> {
    let view = compilation.view(source).unwrap();
    (0..view.unit_count())
        .map(|index| {
            view.unit_revision(UnitId::from_index(index).unwrap())
                .unwrap()
        })
        .collect()
}

fn emit_native(compilation: &mut Compilation<'_>, source: SemanticId) -> String {
    let before = compilation.ledger().retained_bytes();
    let units = revisions(compilation, source);
    let checkpoints = compilation.checkpoint_count();
    let codecs = compilation.ledger().work_by_kind(WorkKind::Codec);
    let code = compilation
        .with_native_c(source, &native_policy(), WorkDomain::Baseline, |output| {
            let original = output.as_str().as_ptr();
            let code = output.take_c();
            assert_eq!(original, code.as_ptr(), "terminal handoff must not copy C");
            code
        })
        .unwrap();
    assert!(!code.is_empty());
    assert_eq!(compilation.ledger().retained_bytes(), before);
    assert_eq!(compilation.ledger().work_by_kind(WorkKind::Codec), codecs);
    assert_eq!(compilation.checkpoint_count(), checkpoints);
    assert_eq!(revisions(compilation, source), units);
    code
}

fn emit_javascript(compilation: &mut Compilation<'_>, source: SemanticId) -> String {
    let policy = javascript_policy();
    let candidate = compilation
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let code = compilation
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .unwrap()
        .unwrap();
    compilation.discard(candidate.semantic_id()).unwrap();
    code
}

struct ScratchDirectory(PathBuf);
impl ScratchDirectory {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-native-test-{}-{}-{name}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("native failure inputs retained at {}", self.0.display());
        } else {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

pub(super) fn execute(command: &mut Command) -> Output {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let invocation = format!("{command:?}");
    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("cannot execute {invocation}: {error}"));
    let started = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if started.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "timed out: {invocation}\nstdout:{}\nstderr:{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

struct Compiler {
    family: &'static str,
    executable: OsString,
    version: String,
}

fn compilers() -> [Compiler; 2] {
    let gcc = std::env::var_os("LILSCRIPT_NATIVE_CC").unwrap_or_else(|| "cc".into());
    let clang = std::env::var_os("LILSCRIPT_NATIVE_CLANG")
        .or_else(|| {
            ["clang", "clang-18", "clang-19", "clang-20"]
                .into_iter()
                .find(|name| {
                    Command::new(name)
                        .arg("--version")
                        .output()
                        .is_ok_and(|output| output.status.success())
                })
                .map(OsString::from)
        })
        .expect(
            "Clang is required; set LILSCRIPT_NATIVE_CLANG to its executable (no skipped gate)",
        );
    [("gcc", gcc), ("clang", clang)].map(|(family, executable)| {
        let output = execute(Command::new(&executable).arg("--version"));
        assert!(output.status.success(), "{family} version probe failed");
        let version = String::from_utf8(output.stdout).unwrap();
        if family == "clang" {
            assert!(
                version.contains("clang"),
                "Clang gate requires Clang: {version}"
            );
        } else {
            assert!(
                !version.contains("clang") && version.contains("Free Software Foundation"),
                "GCC gate requires GCC: {version}"
            );
        }
        Compiler {
            family,
            executable,
            version,
        }
    })
}

pub(super) fn compile_and_execute(
    code: &str,
    expected: &str,
    name: &str,
) -> Vec<serde_json::Value> {
    let directory = ScratchDirectory::new(name);
    let input = directory.0.join("program.c");
    std::fs::write(&input, code).unwrap();
    let mut observations = Vec::new();
    for compiler in compilers() {
        // Clang also runs AddressSanitizer with leak detection: native
        // ownership must neither free a live object nor leak one.
        let asan: &[(&str, &[&str])] = if compiler.family == "clang" {
            &[(
                "asan",
                &[
                    "-fsanitize=address,undefined",
                    "-fno-sanitize-recover=all",
                    "-fno-omit-frame-pointer",
                ][..],
            )]
        } else {
            &[]
        };
        for &(profile, flags) in [
            ("O0", &[][..]),
            ("O2", &[][..]),
            (
                "ubsan",
                &["-fsanitize=undefined", "-fno-sanitize-recover=all"][..],
            ),
        ]
        .iter()
        .chain(asan)
        {
            let executable = directory.0.join(format!("{}-{profile}", compiler.family));
            let mut command = Command::new(&compiler.executable);
            command.args([
                "-std=c11",
                "-fno-fast-math",
                "-ffp-contract=off",
                if profile == "O0" { "-O0" } else { "-O2" },
            ]);
            command
                .args(flags)
                .arg(&input)
                .arg("-lm")
                .arg("-o")
                .arg(&executable);
            let invocation = format!("{command:?}");
            let compile_started = Instant::now();
            let built = execute(&mut command);
            let compile_elapsed_seconds = compile_started.elapsed().as_secs_f64();
            assert!(
                built.status.success(),
                "{name}/{}/{profile} compile failed\n{invocation}\n{}\n{code}",
                compiler.family,
                String::from_utf8_lossy(&built.stderr)
            );
            let executable_bytes = std::fs::read(&executable).unwrap();
            let executable_sha256 = digest(&executable_bytes);
            let executable_size = executable_bytes.len();
            drop(executable_bytes);
            let run_started = Instant::now();
            let ran = execute(
                Command::new(&executable)
                    .env("UBSAN_OPTIONS", "halt_on_error=1")
                    .env("ASAN_OPTIONS", "detect_leaks=1:halt_on_error=1"),
            );
            let run_elapsed_seconds = run_started.elapsed().as_secs_f64();
            assert!(
                ran.status.success(),
                "{name}/{}/{profile} failed: {}\n{code}",
                compiler.family,
                String::from_utf8_lossy(&ran.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&ran.stderr), "", "{name}/{profile}");
            assert_eq!(
                String::from_utf8_lossy(&ran.stdout),
                expected,
                "{name}/{}/{profile}",
                compiler.family
            );
            observations.push(serde_json::json!({
                "compiler": compiler.family, "executable": compiler.executable,
                "version": compiler.version, "profile": profile, "command": invocation,
                "compile_status": built.status.code(),
                "compile_elapsed_seconds": compile_elapsed_seconds,
                "compile_stderr": String::from_utf8_lossy(&built.stderr),
                "run_status": ran.status.code(), "stdout": String::from_utf8_lossy(&ran.stdout),
                "stderr": String::from_utf8_lossy(&ran.stderr),
                "run_elapsed_seconds": run_elapsed_seconds,
                "native_executable_sha256": executable_sha256,
                "native_executable_bytes": executable_size,
                "timing_scope": "subprocess wall time including spawn/wait polling; RSS unmeasured",
            }));
        }
    }
    observations
}

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

fn fixture(name: &str, source: &str, expected: &str) {
    checked(source, WORK, |compilation, source_id| {
        let render_before = compilation.ledger().work_by_kind(WorkKind::Render);
        let c = emit_native(compilation, source_id);
        let native_work = compilation.ledger().work_by_kind(WorkKind::Render) - render_before;
        let javascript = emit_javascript(compilation, source_id);
        let js = execute(Command::new("node").args(["--input-type=module", "-e", &javascript]));
        assert!(
            js.status.success(),
            "{name}: {}\n{javascript}",
            String::from_utf8_lossy(&js.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&js.stderr), "", "{name}");
        assert_eq!(
            String::from_utf8_lossy(&js.stdout),
            expected,
            "{name}\n{javascript}"
        );
        let observations = compile_and_execute(&c, expected, name);
        eprintln!(
            "native-artifact {}",
            serde_json::json!({
                "case": name, "source": source, "source_sha256": digest(source),
                "c": c, "c_sha256": digest(&c), "javascript": javascript,
                "javascript_sha256": digest(&javascript), "expected": expected,
                "native_render_work": native_work, "executions": observations,
                "qualification": "new semantic-core public API; complete emitted C unchanged; fixed expected traces; Node additional oracle",
            })
        );
    });
}

macro_rules! native_fixture {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            fixture(
                $name,
                include_str!(concat!("fixtures/native/", $name, ".lil")),
                include_str!(concat!("fixtures/native/", $name, ".expected.out")),
            );
        }
    };
}
native_fixture!(unchanged_recursion_and_integer_source, "recursion-integer");
native_fixture!(
    integer_arithmetic_shift_division_and_rounded_multiply,
    "integer-edges"
);
native_fixture!(
    ordered_calls_lazy_regions_and_continue_updates,
    "ordered-control"
);
native_fixture!(binary64_rounding_nan_and_negative_zero, "binary64");
native_fixture!(utf16_embedded_nul_surrogates_and_borrowed_slices, "utf16");

#[test]
fn unchanged_native_primitives_example() {
    fixture(
        "native-primitives-example",
        include_str!("../../examples/native_primitives.lil"),
        "15\n",
    );
}

#[test]
fn unchanged_finite_loop_control_source() {
    fixture(
        "finite-loop-control",
        include_str!("fixtures/demand/finite-loop-control.lil"),
        include_str!("fixtures/demand/finite-loop-control.expected.out"),
    );
}

#[test]
fn native_output_releases_on_return_error_unwind_and_zero_copy_handoff() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    checked(SIMPLE, WORK, |compilation, source| {
        let retained = compilation.ledger().retained_bytes();
        let native = native_policy();
        let expected_hash = compilation
            .with_native_c(source, &native, WorkDomain::Baseline, |output| {
                digest(output.as_str())
            })
            .unwrap();
        assert_eq!(compilation.ledger().retained_bytes(), retained);
        let rejected: Result<(), &'static str> = compilation
            .with_native_c(source, &native, WorkDomain::Optional, |output| {
                assert_eq!(digest(output.as_str()), expected_hash);
                Err("consumer declined native output")
            })
            .unwrap();
        assert!(rejected.is_err());
        assert_eq!(compilation.ledger().retained_bytes(), retained);
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            compilation
                .with_native_c(source, &native, WorkDomain::Optional, |output| {
                    assert_eq!(digest(output.as_str()), expected_hash);
                    panic!("consumer stopped after native output formation");
                })
                .unwrap();
        }));
        assert!(unwound.is_err());
        assert_eq!(compilation.ledger().retained_bytes(), retained);
        let handed_off = emit_native(compilation, source);
        assert_eq!(digest(&handed_off), expected_hash);
    });
}

#[test]
fn optional_native_work_exhaustion_does_not_consume_baseline_output_allowance() {
    checked(SIMPLE, 0, |compilation, source| {
        let retained = compilation.ledger().retained_bytes();
        let baseline = compilation.ledger().work_used(WorkDomain::Baseline);
        let mut called = false;
        let result =
            compilation.with_native_c(source, &native_policy(), WorkDomain::Optional, |_| {
                called = true;
            });
        assert!(matches!(
            result,
            Err(NativeError::Allocation(AllocationError::Budget(
                BudgetError::WorkExhausted(WorkDomain::Optional)
            )))
        ));
        assert!(!called);
        assert_eq!(compilation.ledger().retained_bytes(), retained);
        assert_eq!(
            compilation.ledger().work_used(WorkDomain::Baseline),
            baseline
        );
        assert!(!emit_native(compilation, source).is_empty());
    });
}

#[test]
fn optional_native_output_cannot_consume_reserved_baseline_memory() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SIMPLE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let mut compilation = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: MEMORY,
                retained_bytes: MEMORY,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 3 },
    )
    .unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let before = compilation.ledger().clone();
    let result = compilation.with_native_c(source, &native_policy(), WorkDomain::Optional, |_| {
        panic!("unadmitted native output entered callback");
    });
    assert!(matches!(
        result,
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
    assert!(!emit_native(&mut compilation, source).is_empty());
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

#[test]
fn partial_native_buffer_growth_refusal_releases_output_and_plan() {
    // One large literal keeps the semantic plan small while requiring repeated
    // C buffer growth. The reserved baseline remains able to finish the same
    // source after optional output fails; there is no allocation-failure hook.
    let source = format!("string text=\"{}\";print(text.length);", "q".repeat(12_000));
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, &source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let mut compilation = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: MEMORY - 16_384,
                retained_bytes: MEMORY,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 3 },
    )
    .unwrap();
    let id = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let before = compilation.ledger().clone();
    let result = compilation.with_native_c(id, &native_policy(), WorkDomain::Optional, |_| {
        panic!("partial C cannot enter the callback");
    });
    assert!(matches!(
        result,
        Err(NativeError::Allocation(AllocationError::Budget(
            BudgetError::MemoryExhausted(WorkDomain::Optional)
        )))
    ));
    let rendered =
        compilation.ledger().work_by_kind(WorkKind::Render) - before.work_by_kind(WorkKind::Render);
    // The first prologue copy plus the small plan cannot account for this
    // amount: emission has continued into subsequent writes/growth. In
    // particular this is not the first-header refusal tested above.
    assert!(rendered > (super::native_runtime::PROLOGUE.len() as u64) * 3);
    assert_eq!(
        compilation.ledger().retained_bytes(),
        before.retained_bytes()
    );
    assert_eq!(
        compilation.ledger().retained_bytes_in(WorkDomain::Optional),
        0
    );
    assert_eq!(
        compilation.ledger().work_used(WorkDomain::Baseline),
        before.work_used(WorkDomain::Baseline)
    );
    let completed = emit_native(&mut compilation, id);
    assert!(completed.len() > 16_384);
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

#[test]
fn wrong_target_stale_and_foreign_source_never_enter_native_callback() {
    checked(SIMPLE, WORK, |compilation, source| {
        let retained = compilation.ledger().retained_bytes();
        let mut called = false;
        let wrong =
            compilation.with_native_c(source, &javascript_policy(), WorkDomain::Baseline, |_| {
                called = true;
            });
        assert!(matches!(wrong, Err(NativeError::WrongTarget)));
        assert!(!called);
        assert_eq!(compilation.ledger().retained_bytes(), retained);
        checked(SIMPLE, WORK, |other, other_source| {
            assert_ne!(source, other_source);
            let retained = other.ledger().retained_bytes();
            assert!(matches!(
                other.with_native_c(source, &native_policy(), WorkDomain::Baseline, |_| panic!(
                    "foreign source accepted"
                )),
                Err(NativeError::Publication(
                    PublicationError::UnknownCheckpoint
                ))
            ));
            assert_eq!(other.ledger().retained_bytes(), retained);
            assert!(!emit_native(other, other_source).is_empty());
        });
        compilation.discard(source).unwrap();
        let retained = compilation.ledger().retained_bytes();
        assert!(matches!(
            compilation.with_native_c(source, &native_policy(), WorkDomain::Baseline, |_| panic!(
                "stale source accepted"
            )),
            Err(NativeError::Publication(
                PublicationError::UnknownCheckpoint
            ))
        ));
        assert_eq!(compilation.ledger().retained_bytes(), retained);
    });
}

#[test]
fn unsupported_boundaries_reject_complete_source_without_backend_fallback() {
    for source in [
        "Regex pattern=new Regex(\"a\");print(pattern.test(\"a\"));",
        "func()->int make(){auto value=()=>1;return ()=>value();}auto read=make();print(read());",
        "extern int effect();print(effect());",
        "try{print(1);}finally{print(2);}",
    ] {
        checked(source, WORK, |compilation, id| {
            let retained = compilation.ledger().retained_bytes();
            let units = revisions(compilation, id);
            let result =
                compilation.with_native_c(id, &native_policy(), WorkDomain::Baseline, |_| {
                    panic!("unsupported native source entered output callback: {source}");
                });
            assert!(
                matches!(result, Err(NativeError::Unsupported { .. })),
                "{source}: {result:?}"
            );
            assert_eq!(compilation.ledger().retained_bytes(), retained);
            assert_eq!(revisions(compilation, id), units);
        });
    }
}

#[test]
fn arrays_grow_nest_and_run_callbacks_natively() {
    fixture(
        "native-arrays",
        "int[] xs=[3,1,2];xs.push(5);xs[1]=10;int total=0;for(int x of xs){total+=x;}print(total);\
         print(xs.pop());print(xs[2]);int[] doubled=xs.map((int v)=>v*2);print(doubled[2]);\
         print(xs.reduce((int a,int b)=>a+b,0));print(xs.indexOf(10));print(xs.includes(4));\
         int[][] grid=[[1],[2,3]];print(grid[1].length);",
        "20\n5\n2\n4\n15\n1\nfalse\n2\n",
    );
}

#[test]
fn strings_convert_and_print_as_javascript_does() {
    fixture(
        "native-strings",
        "string s=\"Lil\"+\"Script\";print(s);print(`${s}:${1}:${true}:${0.1+0.2}`);\
         print(s.slice(-6,-3));print(s.toUpperCase());print(s.indexOf(\"S\"));\
         string[] parts=\"a/b/c\".split(\"/\");print(parts.length);print(-0.0);print(1.0/3.0);\
         print(255.toString(16));print(1000000000000000000000.0);print(0.000001);",
        "LilScript\nLilScript:1:true:0.30000000000000004\nScr\nLILSCRIPT\n3\n3\n-0\n0.3333333333333333\nff\n1e+21\n0.000001\n",
    );
}

#[test]
fn classes_tagged_values_and_adapted_generic_callbacks_run_natively() {
    fixture(
        "native-classes-tagged",
        "class Counter{int value;init(int value){this.value=value;}\
         int add(int amount){this.value+=amount;return this.value;}}\
         T apply<T>(T value,func(T)->T transform){return transform(value);}\
         Counter counter=new Counter(1);print(counter.add(2));int? maybe=null;print(maybe==null);\
         maybe=5;print(maybe==5);string|int either=\"x\";print(either is string);\
         print(apply(3,(int v)=>v*4));",
        "3\ntrue\ntrue\ntrue\n12\n",
    );
}

#[test]
fn maps_sets_and_typed_arrays_keep_javascript_semantics() {
    fixture(
        "native-collections",
        "Map<string,int> m=new Map<string,int>();m.set(\"a\",1).set(\"b\",2);print(m.size);\
         int? a=m.get(\"a\");print(a==1);print(m.get(\"z\")==null);print(m.delete(\"a\"));\
         print(m.size);Set<float> s=new Set<float>();s.add(0.0).add(-0.0);print(s.size);\
         Uint8Array bytes=new Uint8Array(2);bytes[0]=257;bytes[5]=1;print(bytes[0]);print(bytes[1]);",
        "2\ntrue\ntrue\ntrue\n1\n1\n1\n0\n",
    );
}

#[test]
fn match_optional_access_and_array_destructuring_run_natively() {
    fixture(
        "native-syntax",
        "enum Status{Draft,Active,Sold}\
         string label(Status status){return match(status){Status.Draft=>\"draft\",Status.Active=>\"active\",Status.Sold=>\"sold\"};}\
         print(label(Status.Active));print(match(-1){0=>10,-1=>20,_=>30});\
         class Box{int value;init(int value){this.value=value;}}\
         Box? maybe(bool present){if(present){return new Box(4);}return null;}\
         print(maybe(true)?.value==4);print(maybe(false)?.value??9);\
         int[] values=[1,2,3];auto [first,,third,...tail]=values;print(first==1);print(third==3);\
         print(tail.length);auto [only,absent]=[9];print(absent==null);",
        "active\n20\ntrue\n9\ntrue\ntrue\n0\ntrue\n",
    );
}

#[test]
fn native_nullish_assignment_keeps_lazy_values_and_captured_index_references() {
    fixture(
        "native-nullish-assignment",
        "int effects=0;int rhs(){effects+=1;return 7;}\
         int? missing=null;int? present=0;print(missing??=rhs());print(present??=rhs());\
         (int?)[] cells=[null,0];int index=0;\
         print(cells[index++]??=rhs());print(index);print(cells[index++]??=rhs());\
         print(index);print(effects);print(cells[0]??-1);print(cells[1]??-1);",
        "7\n0\n7\n1\n0\n2\n2\n7\n0\n",
    );
}

#[test]
fn generic_class_chains_run_natively() {
    fixture(
        "native-generic-inheritance",
        "class Pair<A,B>{A first;B second;init(A first,B second){this.first=first;this.second=second;}\
         B right(){return this.second;}A left(){return this.first;}}\
         class Named<T> extends Pair<string,T>{int uses;init(string name,T value){super(name,value);this.uses=0;}\
         T use(){this.uses+=1;return this.right();}}\
         class Counted extends Named<int>{init(string name,int count){super(name,count);}int twice(){return this.use()*2;}}\
         string describe(Pair<string,int> pair){return pair.left()+\"=\"+pair.right();}\
         Counted counted=new Counted(\"apples\",21);print(counted.twice());print(counted.uses);print(describe(counted));",
        "42\n1\napples=21\n",
    );
}

#[test]
fn escaped_scalar_closure_executes_after_its_factory_returns() {
    fixture(
        "escaped-scalar-closure",
        "func()->int make(){int value=1;return ()=>value;}auto read=make();print(read());",
        "1\n",
    );
}

#[path = "native_closure_tests.rs"]
mod closure_tests;

#[test]
fn s4_optimized_nullable_views_keep_native_unboxing() {
    let arena = bumpalo::Bump::new();
    let source="int choose(int n){int? present=199;return n+(present??9);}for(int i=0;i<7;i+=1){print(choose(i));}";
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let (program, _) = super::rules::optimize(
        program,
        super::rules::RuleRequest {
            fold: true,
            dead_code: true,
            inline: false,
            scalar: false,
            native: true,
            pristine_builtins: false,
            seal: super::call_graph::Seal::Module,
        },
    )
    .unwrap();
    program.verify().unwrap();
    let mut compiler = compilation(WORK, MEMORY);
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let c = emit_native(&mut compiler, source);
    compile_and_execute(&c, "199\n200\n201\n202\n203\n204\n205\n", "s4-typed-view");
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn s4_native_defaults_distinguish_omission_from_zero_and_follow_aliases() {
    let source="int first(int x=3){return x;}int second(int x=9){return x;}auto alias=first;print(alias());alias=second;print(alias());print(alias(0));auto local=(int x=4)=>x;print(local());print(local(0));";
    checked(source, WORK, |compilation, source| {
        let c = emit_native(compilation, source);
        compile_and_execute(&c, "3\n9\n0\n4\n0\n", "s4-native-defaults");
    });
}

#[test]
fn s4_native_captured_and_earlier_parameter_defaults() {
    checked("auto make=(int seed)=>(int value=seed)=>value;auto f=make(7);print(f());print(f(0));print(((int a,int b=a)=>b)(8));",WORK,|compilation,source| {
        let c=emit_native(compilation,source);
        compile_and_execute(&c,"7\n0\n8\n","s4-captured-defaults");
    });
}

#[test]
fn s4_native_checked_reads_value_updates_and_generic_methods() {
    for (source, expected) in [
        ("int[] xs=[4,8];print(xs.get(-1)??99);print(xs.get(1)??99);print(xs.get(2)??99);int?[] ys=[null,7];print(ys.get(0)??3);print(ys.get(1)??3);", "99\n8\n99\n3\n7\n"),
        ("struct Point{int x;int y;}Point p=Point{1,2};int change(){p.y=99;return 7;}Point q=p with {x:change()};q.x=8;print(p.x);print(p.y);print(q.x);print(q.y);", "1\n99\n8\n2\n"),
        (r#"class Box<T>{T value;init(T x){this.value=x;}U second<U>(U x){return x;}T first<U>(U x){return this.value;}}Box<int> b=new Box<int>(7);print(b.second(3));print(b.second("word"));print(b.first(false));"#, "3\nword\n7\n"),
    ] {
        checked(source,WORK,|compilation,source| {
            let c=emit_native(compilation,source);
            compile_and_execute(&c,expected,"s4-value-contracts");
        });
    }
}

#[test]
fn s4_native_variadic_values_keep_typed_arrays_and_call_contracts() {
    checked("auto sum=(int first,int... rest)=>{int result=first;for(int i=0;i<rest.length;i++){result+=rest[i];}return result;};func(int,int...)->int f=sum;print(f(2));print(f(2,3,4));int[] xs=[5,6];print(f(2,...xs));",WORK,|compilation,source| {
        let c=emit_native(compilation,source);compile_and_execute(&c,"2\n9\n13\n","s4-native-rest");
    });
}

#[test]
fn s4_native_receiver_functions_and_spread_snapshots() {
    checked("auto add=(this int self,int first,int... rest)=>{int total=self+first;for(int i=0;i<rest.length;i++){total+=rest[i];}return total;};func(this:int,int,int...)->int f=add;print(f.call(7,2));int[] xs=[3,4];int mutate(){xs[0]=9;return 5;}print(f.call(7,2,...xs,mutate()));",WORK,|compilation,source| {
        let c=emit_native(compilation,source);compile_and_execute(&c,"9\n21\n","s4-native-receiver");
    });
}

#[test]
fn s4_native_defaults_and_constructor_rest() {
    checked("int sum(int first=7,int... rest){int n=first;for(int i=0;i<rest.length;i++){n+=rest[i];}return n;}print(sum());print(sum(2,3,4));class Box{int value;init(int first=7,int... rest){this.value=first;for(int i=0;i<rest.length;i++){this.value+=rest[i];}}}Box a=new Box();int[] xs=[3,4];Box b=new Box(2,...xs);print(a.value);print(b.value);auto f=(int first=7,int... rest)=>first+rest.length;print(f());print(f(2,3,4));",WORK,|compilation,source| {
        let c=emit_native(compilation,source);compile_and_execute(&c,"7\n9\n7\n9\n7\n4\n","s4-native-rest-defaults");
    });
}

#[test]
fn s4_native_callee_default_expressions_and_parameter_captures() {
    checked("int calls=0;int next(){calls+=1;return calls;}int read(int first=next(),func()->int get=()=>first){return get();}print(read());print(read(9));print(calls);auto make=(int seed)=>{auto f=(int n=seed+next(),func()->int get=()=>n)=>get();return f;};auto f=make(10);print(f());print(f());print(calls);",WORK,|compilation,source| {
        let c=emit_native(compilation,source);compile_and_execute(&c,"1\n9\n1\n12\n13\n3\n","s4-native-default-expressions");
    });
}

#[test]
fn s4_checked_binary_reads_share_nullable_and_evaluation_contracts() {
    fixture("checked-binary-reads", r#"
        Int8Array a=new Int8Array(1);a[0]=255;
        Uint8Array b=new Uint8Array(1);b[0]=255;
        Uint8ClampedArray c=new Uint8ClampedArray(1);c[0]=999;
        Int16Array d=new Int16Array(1);d[0]=65535;
        Uint16Array e=new Uint16Array(1);e[0]=65535;
        Int32Array f=new Int32Array(1);f[0]=-7;
        Uint32Array g=new Uint32Array(1);g[0]=7;
        Float32Array h=new Float32Array(1);h[0]=1.5;
        Float64Array j=new Float64Array(1);j[0]=2.25;
        print(a.get(0)??0);print(b.get(0)??0);print(c.get(0)??0);
        print(d.get(0)??0);print(e.get(0)??0);print(f.get(0)??0);
        print(g.get(0)??0);print(h.get(0)??0.0);print(j.get(0)??0.0);
        print(a.get(-1)==null);print(b.get(1)==null);print(h.get(-1)==null);
        print(j.get(1)==null);
        int order=0;
        Uint8Array receiver(){order=order*10+1;return b;}
        int index(){order=order*10+2;return 0;}
        print(receiver().get(index())??0);print(order);
        print("𐐀".codeUnitAt(0));print("𐐀".codeUnitAt(1));
        print("".charCodeAt(-1));print("x".charCodeAt(1));
    "#, "-1\n255\n255\n-1\n65535\n-7\n7\n1.5\n2.25\ntrue\ntrue\ntrue\ntrue\n255\n12\n55297\n56320\n0\n0\n");
}

#[test]
fn s4_native_code_unit_and_every_binary_kind_trap_invalid_reads() {
    let directory = ScratchDirectory::new("index-traps");
    let compiler = std::env::var_os("LILSCRIPT_NATIVE_CC").unwrap_or_else(|| "cc".into());
    let cases = crate::typed_array::TypedArrayKind::ALL.into_iter().map(|kind| {
        (kind.name().to_string(), format!("{0} a=new {0}(1);print(a[selected()]);",kind.name()))
    }).chain(std::iter::once(("code-unit".into(), "print(\"x\".codeUnitAt(selected()));".into())));
    for (name, body) in cases {
        let source = format!("extern int selected();{body}");
        let c = checked(&source, WORK, |compilation, source| {
            let cell=compilation.with_semantic(source, |program,_,_| CellId::from_index(
                program.cells().iter().position(|cell|cell.name=="selected" && cell.binding==CellBinding::Foreign).unwrap()
            ).unwrap()).unwrap();
            let bindings=[NativeHostBinding{cell,link_name:"host_selected"}];
            let hosts=NativeHostBindings{callback_abi_version:1,bindings:&bindings};
            compilation.with_native_c_and_hosts(source,&native_policy(),WorkDomain::Baseline,&hosts,|output|output.take_c()).unwrap()
        });
        let input = directory.0.join(format!("{name}.c"));
        let executable = directory.0.join(&name);
        // A runtime index keeps these executions independent of constant
        // evaluation. The exact generated program is followed by its host ABI.
        std::fs::write(&input, format!("{c}\nint32_t host_selected(void){{return atoi(getenv(\"INDEX\"));}}\n")).unwrap();
        let built = execute(Command::new(&compiler).args(["-std=c11","-O2","-fno-fast-math","-ffp-contract=off"])
            .arg(&input).arg("-lm").arg("-o").arg(&executable));
        assert!(built.status.success(), "{name}: {}",String::from_utf8_lossy(&built.stderr));
        for index in ["-1","1","2147483647"] {
            let ran = execute(Command::new(&executable).env("INDEX",index));
            assert!(!ran.status.success(), "{name}/{index} did not trap");
            assert!(String::from_utf8_lossy(&ran.stderr).contains("index out of range"), "{name}/{index}: {ran:?}");
        }
        let valid = execute(Command::new(&executable).env("INDEX","0"));
        assert!(valid.status.success(), "{name}: {valid:?}");
    }
}


#[test]
fn s4_native_public_build_preserves_dead_result_bounds_traps() {
    let directory=ScratchDirectory::new("optimized-index-traps");
    let compiler=std::env::var_os("LILSCRIPT_NATIVE_CC").unwrap_or_else(||"cc".into());
    for (name,source) in [
        ("array","void read(int i){int[] a=[1];a[i];}read(9);"),
        ("binary","void read(int i){Uint8Array a=new Uint8Array(1);a[i];}read(9);"),
        ("unit","void read(int i){\"x\".codeUnitAt(i);}read(9);"),
        ("string-index","void read(int i){\"x\"[i];}read(9);"),
    ] {
        let result=crate::build::compile_source(source,&crate::config::ProjectConfig::default(),crate::build::ServiceOptions {
            target:crate::build::ServiceTarget::Native,preserve_root_exports:false,..Default::default()
        }).unwrap();
        let input=directory.0.join(format!("{name}.c"));let executable=directory.0.join(name);
        std::fs::write(&input,result.native_c().unwrap()).unwrap();
        let built=execute(Command::new(&compiler).args(["-std=c11","-O2","-fno-fast-math","-ffp-contract=off"])
            .arg(&input).arg("-lm").arg("-o").arg(&executable));
        assert!(built.status.success(),"{name}: {}",String::from_utf8_lossy(&built.stderr));
        let ran=execute(&mut Command::new(executable));
        assert!(!ran.status.success(),"{name} failed to trap");
    }
}


#[test]
fn s4_native_string_indexing_keeps_utf16_views_and_char_at_compatibility() {
    fixture("string-indexing",r#"
        string text="A𐐀B";
        print(text[0]);print(text[1].codeUnitAt(0));print(text[2].codeUnitAt(0));print(text[3]);
        print(text.charAt(-1));print(text.charAt(4));
    "#,"A\n55297\n56320\nB\n\n\n");
}

#[test]
fn s4_observed_generic_class_instances_keep_shared_native_identity() {
    fixture(
        "s4-observed-generics",
        r#"
        class Base { int count=1; }
        class Box<T> extends Base { T value; init(T value){super();this.value=value;} T get(){return this.value;} }
        class Child extends Box<int> { init(int value){super(value);} }
        class Other extends Box<int> { init(int value){super(value);} }
        bool child(Base? value){return value is Child;}
        int read(Base value){Child? narrowed=value as? Child;if(narrowed!=null){return narrowed.get();}return -1;}
        Box<int> a=new Box<int>(7);Box<string> b=new Box<string>("text");
        Base base=a;print(a.get());print(b.get());print(base.count);print(base is Base);
        Base alias=a;print(alias==base);
        Base left=new Child(9);Base right=new Other(11);
        print(child(left));print(child(right));print(child(b));print(child(null));
        print(read(left));print(read(right));
    "#,
        "7\ntext\n1\ntrue\ntrue\ntrue\nfalse\nfalse\nfalse\n9\n-1\n",
    );
}
