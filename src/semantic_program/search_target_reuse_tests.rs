//! Formation counts and exact observations across the baseline/optional boundary.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetError, BudgetLedger, CompilationRequest, ResolvedPolicy,
    ResourceLimits, WorkDomain, WorkKind,
};
use crate::structured_js::selection::{Objective, Objectives, Sizes, Style};
use std::cell::Cell;
use std::ops::Deref;
use std::process::Command;

thread_local! {
    static TARGET_ACTIVITY: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
    static PREPARED_OUTPUT_ACTIVITY: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

pub(super) struct AdmittedOutputOwner<T>(Option<T>);

impl<T> AdmittedOutputOwner<T> {
    pub(super) fn new(value: T) -> Self {
        PREPARED_OUTPUT_ACTIVITY.with(|activity| {
            let (live, prepared) = activity.get();
            activity.set((live + 1, prepared + 1));
        });
        Self(Some(value))
    }
}

impl<T> Deref for AdmittedOutputOwner<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.as_ref().unwrap()
    }
}

impl<T> Drop for AdmittedOutputOwner<T> {
    fn drop(&mut self) {
        drop(self.0.take());
        PREPARED_OUTPUT_ACTIVITY.with(|activity| {
            let (live, prepared) = activity.get();
            activity.set((live - 1, prepared));
        });
    }
}

fn prepared_output_activity_for_test() -> (usize, usize) {
    PREPARED_OUTPUT_ACTIVITY.with(Cell::get)
}

pub(super) struct TargetLifetime;

impl TargetLifetime {
    pub(super) fn new() -> Self {
        TARGET_ACTIVITY.with(|activity| {
            let (live, formed) = activity.get();
            activity.set((live + 1, formed + 1));
        });
        Self
    }
}

impl Drop for TargetLifetime {
    fn drop(&mut self) {
        TARGET_ACTIVITY.with(|activity| {
            let (live, formed) = activity.get();
            activity.set((live - 1, formed));
        });
    }
}

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 128_000_000;
const ANSWER: &str = "export int answer(){return 17;}";
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];

// Captured before target reuse from the preserved accepted test executable
// (SHA256 6c35b04eb852de3a061ab9e92f0a491bab9d96b5ae12845e5f63060f54ef7560),
// then re-derived in 008: the exported function now initializes its own
// binding, so both naming styles print the same bytes.
const MANGLED: &str = "let answer=function(){return 17};export{answer};";
const SOURCE_NAMES: &str = "let answer=function(){return 17};export{answer};";

fn config(
    proposals: usize,
    naming: bool,
    scalar: bool,
    schedule: &str,
) -> crate::config::ProjectConfig {
    toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncandidate_proposal_limit={proposals}\nterminal_codec_probe_limit=128\n[policy.search]\ncodec_schedule='{schedule}'\n[policy.tactics]\ntarget-compaction='on'\nidentifier-mangling='on'\nnaming-search='{}'\nscalar-replacement='{}'\ninlining='off'\nconstant-folding='off'\nstring-pooling='off'",
        if naming { "on" } else { "off" }, if scalar { "on" } else { "off" },
    )).unwrap()
}

fn policy(proposals: usize, naming: bool, scalar: bool, schedule: &str) -> ResolvedPolicy {
    config(proposals, naming, scalar, schedule)
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn request() -> SearchRequest {
    let local_facts = LocalFactsRequest {
        work_quota: 100_000,
        result_bytes: 100_000,
    };
    SearchRequest {
        objectives: Objectives::All,
        scalar: ScalarRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
        },
        helper: HelperRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
            local_facts,
        },
        string: StringRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
            local_facts,
        },
        facts_cache: CacheLimits {
            entries: 16,
            bytes: 1_000_000,
            result_bytes: 100_000,
        },
    }
}

fn with_source<R>(
    source: &str,
    work: u64,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: work,
            retained_bytes: MEMORY,
            terminal_work: 0,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 32 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let result = inspect(&mut compiler, source);
    assert_eq!(compiler.finish().retained_bytes(), 0);
    assert_eq!(TARGET_ACTIVITY.with(Cell::get).0, 0);
    assert_eq!(prepared_output_activity_for_test().0, 0);
    result
}

fn counts() -> (usize, usize) {
    assert_eq!(TARGET_ACTIVITY.with(Cell::get).0, 0);
    assert_eq!(prepared_output_activity_for_test().0, 0);
    (
        TARGET_ACTIVITY.with(Cell::get).1,
        prepared_output_activity_for_test().1,
    )
}

fn check_counts(before: (usize, usize), targets: usize, bases: usize) {
    let after = counts();
    assert_eq!((after.0 - before.0, after.1 - before.1), (targets, bases));
}

fn observe_lifetimes() {
    assert_eq!(TARGET_ACTIVITY.with(Cell::get).0, 1);
    assert_eq!(
        prepared_output_activity_for_test().0,
        1,
        "mandatory Basis must not coexist with Optional Basis"
    );
}

fn exact_sizes(javascript: &str, sizes: Sizes) -> [usize; 3] {
    let expected =
        CODECS.map(|codec| crate::compression::measure(javascript.as_bytes(), codec).unwrap());
    assert_eq!(CODECS.map(|codec| sizes.get(codec).unwrap()), expected);
    expected
}

fn execute(javascript: &str, setup: &str, observation: &str, expected: &str) {
    let script = format!(
        "{setup}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{observation}",
        serde_json::to_string(javascript).unwrap(),
    );
    let result = super::native_tests::execute(Command::new("node").args([
        "--input-type=module",
        "-e",
        &script,
    ]));
    assert!(
        result.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8(result.stdout).unwrap(), expected);
}

fn execute_answer(javascript: &str) {
    execute(javascript, "", "console.log(JSON.stringify([library.answer.name,library.answer.length,library.answer(),library.answer()]));", "[\"answer\",0,17,17]\n");
}

#[test]
fn one_target_two_bases_preserve_three_styles_and_exact_old_artifacts() {
    for schedule in ["immediate", "staged"] {
        with_source(ANSWER, WORK, |compiler, source| {
            let before = counts();
            let mut rows = Vec::new();
            let policy = policy(8, true, false, schedule);
            let mut search = compiler
                .search_javascript_observed(source, &policy, request(), |entry| {
                    if entry.baseline || schedule == "immediate" {
                        observe_lifetimes();
                    } else {
                        let targets = TARGET_ACTIVITY.with(Cell::get).0;
                        let bases = prepared_output_activity_for_test().0;
                        assert!(targets <= 1 && bases <= 1);
                        assert_eq!(targets, bases, "deferred scoring may outlive its target");
                    }
                    let (expected, sizes) = if entry.naming.style == Style::Source {
                        (SOURCE_NAMES, [48, 64, 48])
                    } else {
                        (MANGLED, [48, 64, 48])
                    };
                    assert_eq!(entry.javascript, expected);
                    assert_eq!(exact_sizes(entry.javascript, entry.sizes), sizes);
                    execute_answer(entry.javascript);
                    rows.push((entry.naming.style, entry.baseline));
                })
                .unwrap();
            assert!(search.stopped().is_none(), "{:?}", search.stopped());
            // All three styles print the same bytes, so equal trials keep each
            // schedule's own fixed order.
            let (second, third) = if schedule == "immediate" {
                (Style::Scoped, Style::Source)
            } else {
                (Style::Source, Style::Scoped)
            };
            assert_eq!(
                rows,
                [(Style::Global, true), (second, false), (third, false)]
            );
            assert_eq!(search.counters().structures, 1);
            assert_eq!(search.counters().renders, 3);
            check_counts(before, 1, 2);
            eprintln!(
                "target-reuse {}",
                serde_json::json!({
                    "schedule":schedule,"target_formations":1,"basis_preparations":2,"styles":3,
                    "baseline_work":search.ledger().work_used(WorkDomain::Baseline),
                    "optional_work":search.ledger().work_used(WorkDomain::Optional),
                    "analysis_work":search.ledger().work_by_kind(WorkKind::Analysis),
                    "render_work":search.ledger().work_by_kind(WorkKind::Render),
                    "codec_work":search.ledger().work_by_kind(WorkKind::Codec),
                    "baseline_seal_retained_bytes":search.baseline_seal().baseline_retained_bytes,
                    "retained_bytes":search.ledger().retained_bytes(),
                    "peak_retained_bytes":search.ledger().peak_retained_bytes(),
                    "scope":"mechanism and logical accounting, not wall-speed evidence; Render includes shared allocation/movement tariffs"
                })
            );
            let qualified = search.take_qualified_winner(Objective::Brotli).unwrap();
            drop(search);
            assert_eq!(
                compiler.take_qualified_artifact(qualified).unwrap(),
                MANGLED
            );
            check_counts(before, 1, 2);
        });
    }
}

#[test]
fn no_continuation_drops_target_before_seal_without_optional_naming_work() {
    for (proposals, naming) in [(0, true), (8, false)] {
        with_source(ANSWER, WORK, |compiler, source| {
            let before = counts();
            let policy = policy(proposals, naming, false, "immediate");
            let mut observations = 0;
            let search = compiler
                .search_javascript_observed(source, &policy, request(), |entry| {
                    observe_lifetimes();
                    assert!(entry.baseline);
                    observations += 1;
                })
                .unwrap();
            assert!(search.stopped().is_none(), "{:?}", search.stopped());
            assert_eq!(observations, 1);
            assert_eq!(search.counters().renders, 1);
            check_counts(before, 1, 1);
            assert_eq!(
                search.baseline_seal().baseline_retained_bytes,
                search.ledger().retained_bytes_in(WorkDomain::Baseline)
            );
            if proposals == 0 {
                assert_eq!(search.ledger().work_used(WorkDomain::Optional), 0);
                assert_eq!(search.ledger().retained_bytes_in(WorkDomain::Optional), 0);
            }
        });
    }
}

#[test]
fn literal_only_continuation_reuses_target_with_naming_search_disabled() {
    const SOURCE: &str = "extern JsValue event(string label);export void run(){string truth=\"truth-only-payload\";JS.and(truth,event(\"hit\"));}";
    with_source(SOURCE, WORK, |compiler, source| {
        let before = counts();
        let policy = policy(8, false, false, "immediate");
        let mut modes = Vec::new();
        let search = compiler.search_javascript_observed(source, &policy, request(), |entry| {
            observe_lifetimes();
            assert_eq!(entry.naming.style, Style::Global);
            exact_sizes(entry.javascript, entry.sizes);
            execute(entry.javascript, "const events=[];globalThis.event=label=>{events.push(label);return 0;};", "library.run();library.run();console.log(JSON.stringify([library.run.name,library.run.length,events]));", "[\"run\",0,[\"hit\",\"hit\"]]\n");
            modes.push(entry.output.literals);
        }).unwrap();
        assert!(search.stopped().is_none(), "{:?}", search.stopped());
        assert_eq!(modes, [LiteralOutput::Observed, LiteralOutput::Original]);
        assert_eq!(search.counters().structures, 1);
        check_counts(before, 1, 2);
    });
}

#[test]
fn optional_preparation_refusal_drops_target_and_preserves_qualified_baseline() {
    let policy = policy(8, true, false, "immediate");
    let baseline_work = with_source(ANSWER, WORK, |compiler, source| {
        compiler
            .search_javascript(source, &policy, request())
            .unwrap()
            .baseline_seal()
            .baseline_work
    });
    with_source(ANSWER, baseline_work + 1, |compiler, source| {
        let before = counts();
        let mut observations = 0;
        let mut search = compiler
            .search_javascript_observed(source, &policy, request(), |entry| {
                observe_lifetimes();
                assert!(entry.baseline);
                observations += 1;
            })
            .unwrap();
        assert!(
            matches!(
                search.stopped(),
                Some(SearchError::Candidate(CandidateError::Budget(
                    BudgetError::WorkExhausted(WorkDomain::Optional)
                )))
            ),
            "{:?}",
            search.stopped()
        );
        assert_eq!(observations, 1);
        check_counts(before, 1, 1);
        let artifact = search.take_qualified_winner(Objective::Brotli).unwrap();
        drop(search);
        let code = compiler.take_qualified_artifact(artifact).unwrap();
        assert_eq!(code, MANGLED);
        execute_answer(&code);
    });
}

#[test]
fn observer_panics_release_target_and_each_naming_basis() {
    for panic_on_baseline in [true, false] {
        with_source(ANSWER, WORK, |compiler, source| {
            let before = counts();
            let policy = policy(8, true, false, "immediate");
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = compiler.search_javascript_observed(source, &policy, request(), |entry| {
                    observe_lifetimes();
                    if entry.baseline == panic_on_baseline {
                        std::panic::panic_any("target reuse observer");
                    }
                });
            }))
            .expect_err("observer must run");
            assert_eq!(panic.downcast_ref::<&str>(), Some(&"target reuse observer"));
            check_counts(before, 1, if panic_on_baseline { 1 } else { 2 });
            assert!(compiler.view(source).is_ok());
        });
    }
}

#[test]
fn structural_children_still_form_distinct_targets() {
    const SOURCE: &str = "struct Pair{int x;int y;}export int pick(int input){Pair pair=Pair{input,7};return pair.x+pair.y;}";
    with_source(SOURCE, WORK, |compiler, source| {
        let before = counts();
        let policy = policy(64, true, true, "immediate");
        let mut recipes = std::collections::BTreeSet::new();
        let search = compiler.search_javascript_observed(source, &policy, request(), |entry| {
            observe_lifetimes();
            recipes.insert(entry.recipe_fingerprint);
            exact_sizes(entry.javascript, entry.sizes);
            execute(entry.javascript, "", "console.log(JSON.stringify([library.pick.name,library.pick.length,library.pick(5),library.pick(-2)]));", "[\"pick\",1,12,5]\n");
        }).unwrap();
        assert!(
            search.stopped().is_none(),
            "{:?}; {:?}",
            search.stopped(),
            search.counters()
        );
        assert!(
            recipes.len() > 1,
            "a genuinely different structural recipe must be rendered"
        );
        assert!(search.counters().structures > 1);
        check_counts(
            before,
            search.counters().structures,
            search.counters().structures + 1,
        );
    });
}

#[test]
fn public_service_transfers_winners_without_reforming_targets() {
    use crate::compiler_service::{compile_source_semantic, ServiceOptions};
    let before = counts();
    let output = compile_source_semantic(
        ANSWER,
        &config(8, true, false, "immediate"),
        ServiceOptions {
            objectives: Some(Objectives::All),
            logical_work: WORK,
            retained_bytes: MEMORY,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    check_counts(before, 1, 2);
    for codec in CODECS {
        let artifact = output.javascript(codec).unwrap();
        assert_eq!(artifact.javascript(), MANGLED);
        exact_sizes(artifact.javascript(), artifact.sizes());
        execute_answer(artifact.javascript());
    }
}
