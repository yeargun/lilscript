//! Fixed observations for the same reference code under independently selected
//! product storage and helper transport. Every complete artifact executes before
//! raw/gzip/Brotli measurement. This is a test client of existing publication,
//! facts, process execution and native qualification owners, not a new runner.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Plan, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::process::Command;

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    source: &'static str,
    setup: &'static str,
    observations: &'static str,
    expected: &'static str,
    root: &'static str,
    helpers: &'static [&'static str],
    // An additional existing by-value physical transport axis, independent of
    // selected root storage. References retain their own location protocol.
    function_fields: Option<&'static str>,
}
fn policy(compact: bool) -> ResolvedPolicy {
    let config:crate::config::ProjectConfig=toml::from_str(&format!("[javascript]\nstrip_console=false\n[policy.tactics]\ncall-specialization='on'\nscalar-replacement='on'\ninlining='on'\ntarget-compaction='{}'\n",if compact{"on"}else{"off"})).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 200_000_000,
                optional_work: 200_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 256_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 64 },
    )
    .unwrap()
}
fn named(program: &Program<'_>, name: &str) -> CellId {
    let mut matches = program
        .cells()
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.name == name);
    let cell =
        CellId::from_index(matches.next().unwrap_or_else(|| panic!("missing {name}")).0).unwrap();
    assert!(matches.next().is_none(), "unique {name}");
    cell
}
fn product_request() -> ProductRequest {
    ProductRequest {
        max_work: 2_000_000,
        scratch_bytes: 2_000_000,
        output_bytes: 2_000_000,
    }
}
fn helper_request() -> HelperRequest {
    HelperRequest {
        max_work: 2_000_000,
        scratch_bytes: 2_000_000,
        output_bytes: 2_000_000,
        local_facts: LocalFactsRequest {
            work_quota: 200_000,
            result_bytes: 100_000,
        },
    }
}
fn flatten(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    root: CellId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .scalar_product_javascript(base, root, product_request(), policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        ProductOutcome::Published(candidate) => candidate,
        other => panic!("expected reference-compatible product: {other:?}"),
    }
}
fn inline(
    compiler: &mut Compilation<'_>,
    mut base: CandidateId,
    helpers: &[CellId],
    policy: &ResolvedPolicy,
) -> CandidateId {
    for &helper in helpers {
        base = match compiler
            .inline_helper_javascript(base, helper, helper_request(), policy, WorkDomain::Optional)
            .unwrap()
            .outcome
        {
            HelperOutcome::Published(candidate) => candidate,
            other => panic!("expected leaf reference helper: {other:?}"),
        };
    }
    base
}
fn fields(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    body: UnitId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .scalar_function_javascript(base, body, product_request(), policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        FunctionOutcome::Published(candidate) => candidate,
        other => panic!("expected independent by-value field transport: {other:?}"),
    }
}
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn execute(case: &Case, javascript: &str) -> Json {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, javascript, oxc_span::SourceType::mjs()).parse();
    assert!(
        !parsed.panicked && parsed.diagnostics.is_empty(),
        "{}: {:?}\n{javascript}",
        case.name,
        parsed.diagnostics
    );
    let script = format!(
        "const events=[];console.log=value=>events.push(value);\n{}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));\n{}\nprocess.stdout.write(JSON.stringify(events));",
        case.setup,
        serde_json::to_string(javascript).unwrap(),
        case.observations
    );
    let result = super::native_tests::execute(Command::new("node").args([
        "--input-type=module",
        "-e",
        &script,
    ]));
    assert!(
        result.status.success(),
        "{}: {}\n{script}",
        case.name,
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let observed: Json = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        observed,
        serde_json::from_str::<Json>(case.expected).unwrap(),
        "{}\n{script}",
        case.name
    );
    observed
}
#[derive(Clone, Copy)]
struct Axes {
    dead_code_elimination: bool,
    fields: bool,
    inlined: bool,
    parameter_fields: bool,
    reverse: bool,
}
fn artifact(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    case: &Case,
    axes: Axes,
    compact: bool,
    style: Style,
) -> Json {
    let descriptor = compiler
        .with_recipe_descriptor(candidate, WorkDomain::Baseline, |words| words.to_vec())
        .unwrap();
    let retained = compiler.ledger().retained_bytes();
    let mut choices = OutputTactics::from_policy(policy);
    choices.dead_code_elimination = axes.dead_code_elimination;
    let row=compiler.with_javascript_output_choices_in(candidate,policy,choices,WorkDomain::Baseline,|output|{
        let artifact=output.render(&Plan::new(style))?;let observed=output.with_artifact(artifact,|view|execute(case,view.javascript))?;
        let raw=output.measure(artifact,Objective::Raw)?;let gzip9=output.measure(artifact,Objective::Gzip)?;let brotli11=output.measure(artifact,Objective::Brotli)?;let javascript=output.take_artifact(artifact)?;assert_eq!(raw,javascript.len());
        Ok::<_,CandidateError>(json!({"case":case.name,"dead_code_elimination":axes.dead_code_elimination,"fields":axes.fields,"inline":axes.inlined,"parameter_fields":axes.parameter_fields,"reverse_publication":axes.reverse,"compact":compact,"style":format!("{style:?}"),"source":case.source,"source_sha256":digest(case.source),"setup":case.setup,"observations":case.observations,"expected":serde_json::from_str::<Json>(case.expected).unwrap(),"observed":observed,"recipe_descriptor":descriptor,"javascript_sha256":digest(&javascript),"javascript":javascript,"raw":raw,"gzip9":gzip9,"brotli11":brotli11}))
    }).unwrap().unwrap();
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    eprintln!("product-reference-artifact {row}");
    row
}
fn matrix(case: Case) {
    matrix_with_dce(case, true);
}
fn matrix_both_demand_modes(case: Case) {
    matrix_with_dce(case, true);
    matrix_with_dce(case, false);
}
fn matrix_with_dce(case: Case, dead_code_elimination: bool) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let root = named(&program, case.root);
    let helpers: Vec<_> = case
        .helpers
        .iter()
        .map(|name| named(&program, name))
        .collect();
    let function = case.function_fields.map(|name| {
        let CellBinding::Function(body) = program.cells[named(&program, name).index()].binding
        else {
            panic!("named function")
        };
        body
    });
    let revisions: Vec<_> = program.units().iter().map(|unit| unit.revision()).collect();
    let mut compiler = compilation();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 32,
                bytes: 2_000_000,
                result_bytes: 100_000,
            },
            WorkDomain::Optional,
        )
        .unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let selected = policy(true);
    let direct = compiler
        .direct_javascript(source, &selected, WorkDomain::Baseline)
        .unwrap();
    let flat = flatten(&mut compiler, direct, root, &selected);
    let inlined = inline(&mut compiler, direct, &helpers, &selected);
    let both = inline(&mut compiler, flat, &helpers, &selected);
    let reverse = flatten(&mut compiler, inlined, root, &selected);
    let base = [
        (false, false, direct),
        (true, false, flat),
        (false, true, inlined),
        (true, true, both),
    ];
    let mut rows = 0;
    for parameter_fields in [false, true] {
        if parameter_fields && function.is_none() {
            continue;
        }
        let reverse = if parameter_fields {
            fields(&mut compiler, reverse, function.unwrap(), &selected)
        } else {
            reverse
        };
        for &(is_flat, is_inline, original) in &base {
            let candidate = if parameter_fields {
                fields(&mut compiler, original, function.unwrap(), &selected)
            } else {
                original
            };
            let view = compiler.view(candidate.semantic_id()).unwrap();
            for (index, &revision) in revisions.iter().enumerate() {
                assert_eq!(
                    view.unit_revision(UnitId::from_index(index).unwrap()),
                    Some(revision)
                );
            }
            for compact in [false, true] {
                let selected = policy(compact);
                for style in [Style::Global, Style::Scoped, Style::Source] {
                    let axes = Axes {
                        dead_code_elimination,
                        fields: is_flat,
                        inlined: is_inline,
                        parameter_fields,
                        reverse: false,
                    };
                    let row = artifact(
                        &mut compiler,
                        candidate,
                        &selected,
                        &case,
                        axes,
                        compact,
                        style,
                    );
                    rows += 1;
                    if is_flat && is_inline {
                        let other = artifact(
                            &mut compiler,
                            reverse,
                            &selected,
                            &case,
                            Axes {
                                reverse: true,
                                ..axes
                            },
                            compact,
                            style,
                        );
                        rows += 1;
                        assert_eq!(row["recipe_descriptor"], other["recipe_descriptor"]);
                        assert_eq!(
                            row["javascript"], other["javascript"],
                            "publication order changes physical output"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(rows, if function.is_some() { 60 } else { 30 });
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

const OVERLAP: &str = r#"
struct Pair{int left;int right;}
int update(Pair snapshot,ref Pair whole,ref int leaf,int next){
    whole=Pair{7,8};leaf+=next;return snapshot.left+whole.left+whole.right;
}
Pair state=Pair{1,2};Pair saved=state;
print(update(state,ref state,ref state.left,3));
print(state.left);print(state.right);print(saved.left);print(saved.right);
"#;
#[test]
fn product_reference_whole_replacement_and_value_arguments_have_independent_storage() {
    matrix(Case {
        name: "whole-overlap-value-snapshot",
        source: OVERLAP,
        setup: "",
        observations: "",
        expected: "[19,10,8,1,2]",
        root: "state",
        helpers: &["update"],
        function_fields: Some("update"),
    });
    matrix(Case {
        name: "byvalue-parameter-location",
        source: r#"
struct Pair{int left;int right;}
int mutate(ref Pair whole,ref int leaf){whole=Pair{40,50};leaf+=2;return whole.left+whole.right;}
int run(Pair local){int before=local.left;int changed=mutate(ref local,ref local.left);return before+changed+local.left;}
Pair state=Pair{1,2};Pair saved=state;
print(run(state));print(run(Pair{3,4}));
print(state.left);print(state.right);print(saved.left);print(saved.right);
"#,
        setup: "",
        observations: "",
        expected: "[135,137,1,2,1,2]",
        root: "local",
        helpers: &["mutate"],
        function_fields: Some("run"),
    });
    // Identical original source, existing six GCC/Clang profiles and three
    // direct JS naming plans. No reference lowering or C test shim is copied.
    super::native_struct_tests::qualify(
        "product-reference-whole-overlap",
        OVERLAP,
        "19\n10\n8\n1\n2\n",
    );
}

const REENTRY: &str = r#"
struct Pair{int left;int right;}struct Box{Pair pair;int sibling;}
extern void keep(func()->void callback);extern int amount();
Box state=Box{Pair{1,2},3};Box saved=state;
keep(()=>{print(state.pair.right);state=Box{Pair{40,50},60};});
int update(ref int target,ref int other,int delta){other=8;target+=(delta|0);return target;}
try{print(update(ref state.pair.left,ref state.pair.right,amount()));}catch{print(77);}
print(state.pair.left);print(state.pair.right);print(state.sibling);print(saved.pair.left);
"#;
#[test]
fn product_reference_current_parents_survive_coercion_reentry_and_throw() {
    matrix(Case {
        name: "nested-reentry",
        source: REENTRY,
        setup: r#"let callback;globalThis.keep=value=>callback=value;globalThis.amount=()=>({[Symbol.toPrimitive](hint){events.push('coerce:'+hint);callback();events.push('replaced');return 3;}});"#,
        observations: "",
        expected: r#"["coerce:number",8,"replaced",4,4,50,60,1]"#,
        root: "state",
        helpers: &["update"],
        function_fields: None,
    });
    matrix(Case {
        name: "nested-reentry-throw",
        source: REENTRY,
        setup: r#"let callback;globalThis.keep=value=>callback=value;globalThis.amount=()=>({[Symbol.toPrimitive](hint){events.push('coerce:'+hint);callback();events.push('replaced');throw 13;}});"#,
        observations: "",
        expected: r#"["coerce:number",8,"replaced",77,40,50,60,1]"#,
        root: "state",
        helpers: &["update"],
        function_fields: None,
    });
}

const NORMALIZATION: &str = r#"
struct Holder{int value;}extern int opaque();extern int later();
int read(ref int value){return value;}
int ignore(ref int value,int next){return next;}
void overwrite(ref int value){value=7;}
int plain=opaque();Holder state=Holder{plain};
print(read(ref plain));try{print(read(ref state.value));}catch{print(99);}
print(ignore(ref state.value,later()));overwrite(ref state.value);print(state.value);
"#;
#[test]
fn product_reference_normalization_is_specific_to_the_actual_location() {
    for (name, setup, expected) in [
        (
            "opaque-int-field",
            r#"const payload={[Symbol.toPrimitive](hint){events.push('coerce:'+hint);return 4294967297;}};globalThis.opaque=()=>payload;globalThis.later=()=>{events.push('later');return 8};console.log=value=>events.push(value===payload?'raw-object':value);"#,
            r#"["raw-object","coerce:number",1,"later",8,7]"#,
        ),
        (
            "throwing-int-field",
            r#"const payload={[Symbol.toPrimitive](hint){events.push('coerce:'+hint);throw 13;}};globalThis.opaque=()=>payload;globalThis.later=()=>{events.push('later');return 8};console.log=value=>events.push(value===payload?'raw-object':value);"#,
            r#"["raw-object","coerce:number",99,"later",8,7]"#,
        ),
        (
            "bigint-int-field",
            r#"globalThis.opaque=()=>1n;globalThis.later=()=>{events.push('later');return 8};console.log=value=>events.push(typeof value==='bigint'?'raw-bigint':value);"#,
            r#"["raw-bigint",99,"later",8,7]"#,
        ),
        (
            "symbol-int-field",
            r#"const payload=Symbol('value');globalThis.opaque=()=>payload;globalThis.later=()=>{events.push('later');return 8};console.log=value=>events.push(value===payload?'raw-symbol':value);"#,
            r#"["raw-symbol",99,"later",8,7]"#,
        ),
    ] {
        matrix(Case {
            name,
            source: NORMALIZATION,
            setup,
            observations: "",
            expected,
            root: "state",
            helpers: &["read", "ignore", "overwrite"],
            function_fields: None,
        });
    }
}

#[test]
fn product_reference_empty_and_effect_only_preparations_do_not_read_payloads() {
    matrix_both_demand_modes(Case {
        name: "zero-field-reference",
        source: r#"struct Empty{}extern int later();void ignore(ref Empty value,int next){}Empty state=Empty{};ignore(ref state,later());print(5);"#,
        setup: "globalThis.later=()=>{events.push('later');return 8};",
        observations: "",
        expected: r#"["later",5]"#,
        root: "state",
        helpers: &["ignore"],
        function_fields: None,
    });
    matrix_both_demand_modes(Case {
        name: "unused-reference-payload",
        source: r#"struct P{int value;}extern int opaque();extern int later();int ignore(ref int value,int next){return next;}void overwrite(ref int value){value=7;}P state=P{opaque()};print(ignore(ref state.value,later()));overwrite(ref state.value);print(state.value);"#,
        setup: r#"globalThis.opaque=()=>({[Symbol.toPrimitive](){events.push('unexpected-coercion');throw 13;}});globalThis.later=()=>{events.push('later');return 8};"#,
        observations: "",
        expected: r#"["later",8,7]"#,
        root: "state",
        helpers: &["ignore", "overwrite"],
        function_fields: None,
    });
}

#[test]
fn product_reference_inline_actual_depth_composes_with_nested_formal_fields() {
    // Public rendering checks actual target nesting, and execute() independently
    // parses the emitted module. These are composed paths and arithmetic, not
    // assertions restating the placement owner's projected-height formula.
    matrix(Case {
        name: "composed-inline-reference-depth",
        source: r#"
struct Inner{int x;int y;}struct Middle{Inner inner;int keep;}
struct Outer{Middle mid;int keep;}
int calculate(ref int value,int factor){
 value=((((value+factor)*3)-2)^7)+1;return ((value*5)+3)/2;
}
void bump(ref Middle node){node.inner.x+=2;}
Outer state=Outer{Middle{Inner{5,9},11},13};Outer saved=state;
print(calculate(ref state.mid.inner.x,4));print(state.mid.inner.x);
print(saved.mid.inner.x);print(state.mid.inner.y);print(state.mid.keep);print(state.keep);
bump(ref state.mid);print(state.mid.inner.x);
print(calculate(ref state.mid.inner.x,-2));print(state.mid.inner.x);print(saved.mid.inner.x);
"#,
        setup: "",
        observations: "",
        expected: "[79,31,5,9,11,13,33,234,93,5]",
        root: "state",
        helpers: &["calculate", "bump"],
        function_fields: None,
    });
}

#[test]
fn maintained_integrated_reference_consumer_keeps_fresh_local_activations() {
    matrix(Case {
        name: "integrated-reference-consumer",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/program/fixtures/integrated-architecture/products.lil"
        )),
        setup: "",
        observations:
            "events.push(library.productScore(5,9));events.push(library.productScore(2,3));",
        expected: "[17,5,9,11,10,5,9,28,8,2,3,8,4,2,3,16]",
        root: "referenceState",
        helpers: &["updateReference"],
        function_fields: None,
    });
}

#[test]
fn early_reference_initialization_refuses_optional_evidence_and_preserves_tdz() {
    let case = Case {
        name: "early-reference-initialization",
        source: r#"struct P{int value;}extern int during(func()->void callback);extern int later();void ignore(ref P value,int next){}P state=P{during(()=>{ignore(ref state,later());})};print(state.value);"#,
        setup: r#"globalThis.during=callback=>{events.push('during');try{callback()}catch(error){events.push(error.name)}return 7};globalThis.later=()=>{events.push('later');return 9};"#,
        observations: "",
        expected: r#"["during","ReferenceError",7]"#,
        root: "state",
        helpers: &["ignore"],
        function_fields: None,
    };
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let root = named(&program, case.root);
    let helper = named(&program, "ignore");
    let mut compiler = compilation();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 4,
                bytes: 500_000,
                result_bytes: 100_000,
            },
            WorkDomain::Optional,
        )
        .unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let selected = policy(true);
    let direct = compiler
        .direct_javascript(source, &selected, WorkDomain::Baseline)
        .unwrap();
    let retained = compiler.ledger().retained_bytes();
    let checkpoints = compiler.checkpoint_count();
    let product = compiler
        .scalar_product_javascript(
            direct,
            root,
            product_request(),
            &selected,
            WorkDomain::Optional,
        )
        .unwrap();
    assert!(matches!(product.outcome, ProductOutcome::Unknown(_)));
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    assert_eq!(compiler.checkpoint_count(), checkpoints);
    let helper = compiler
        .inline_helper_javascript(
            direct,
            helper,
            helper_request(),
            &selected,
            WorkDomain::Optional,
        )
        .unwrap();
    assert!(matches!(helper.outcome, HelperOutcome::Unknown(_)));
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    assert_eq!(compiler.checkpoint_count(), checkpoints);
    for compact in [false, true] {
        let selected = policy(compact);
        for style in [Style::Global, Style::Scoped, Style::Source] {
            artifact(
                &mut compiler,
                direct,
                &selected,
                &case,
                Axes {
                    dead_code_elimination: true,
                    fields: false,
                    inlined: false,
                    parameter_fields: false,
                    reverse: false,
                },
                compact,
                style,
            );
        }
    }
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
