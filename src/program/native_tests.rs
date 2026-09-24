//! Executable qualification of the program's C client. The fixed
//! expected traces are the oracle; Node is an additional execution comparison.
//! The generated C is compiled unchanged, without an old backend or test shim.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits,
    WorkDomain, WorkKind,
};
use crate::output_budget::AllocationError;
use crate::js::selection::{Plan, Style};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 256_000_000;
const SIMPLE: &str = "int twice(int value){return value*2;}print(twice(21));";

fn native_policy() -> ResolvedPolicy {
    crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::Native)
        .unwrap()
}

fn javascript_policy() -> ResolvedPolicy {
    let config: crate::config::ProjectConfig =
        toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: false,
        })
        .unwrap()
}

fn compilation<'src>(optional_work: u64, memory: u64) -> Compilation<'src> {
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
         print(xs.pop());print(xs[7]);int[] doubled=xs.map((int v)=>v*2);print(doubled[2]);\
         print(xs.reduce((int a,int b)=>a+b,0));print(xs.indexOf(10));print(xs.includes(4));\
         int[][] grid=[[1],[2,3]];print(grid[1].length);",
        "20\n5\n0\n4\n15\n1\nfalse\n2\n",
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
         Uint8Array bytes=new Uint8Array(2);bytes[0]=257;bytes[5]=1;print(bytes[0]);print(bytes[5]);",
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
