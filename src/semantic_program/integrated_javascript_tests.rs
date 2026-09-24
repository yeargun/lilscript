//! One original-module API slice composes the existing demand, representation,
//! call, name, edit and artifact owners. This is execution evidence for a bounded
//! portfolio, not the same-rule ownership cost experiment or full Marked gate.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy,
    ResourceLimits, TacticId, TacticPermission, WorkDomain, WorkKind,
};
use crate::structured_js::selection::{Objective, Objectives, Plan, Sizes, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;

const CONFIG: &str = include_str!("fixtures/integrated-architecture/config.toml");
const SETUP: &str = include_str!("fixtures/integrated-architecture/setup.js");
const HOST: &str = include_str!("fixtures/integrated-architecture/host.js");
const EXPECTED: &str = include_str!("fixtures/integrated-architecture/expected.json");
const NATIVE_EXPECTED: &str = include_str!("fixtures/integrated-architecture/native.expected.out");
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
const WORK: u64 = 400_000_000;
const MEMORY: u64 = 256_000_000;

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures/integrated-architecture")
}
fn digest(value: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(value.as_ref()))
}
fn verify_archives() {
    let origin: Json =
        serde_json::from_str(include_str!("fixtures/integrated-architecture/origin.json")).unwrap();
    for row in origin["files"].as_array().unwrap() {
        let bytes = std::fs::read(directory().join(row["file"].as_str().unwrap())).unwrap();
        assert_eq!(digest(&bytes), row["sha256"]);
        if let Some(original) = row["original_archive"].as_str() {
            let source = std::fs::read(directory().join(original)).unwrap();
            assert_eq!(digest(&source), row["original_sha256"]);
            let mut excerpt = Vec::new();
            for (index, region) in row["regions"].as_array().unwrap().iter().enumerate() {
                if index != 0 {
                    excerpt.push(b'\n');
                }
                let part = &source[region["start"].as_u64().unwrap() as usize
                    ..region["end"].as_u64().unwrap() as usize];
                assert_eq!(digest(part), region["sha256"]);
                excerpt.extend_from_slice(part);
            }
            assert_eq!(bytes, excerpt);
        }
    }
    for name in ["entry", "barrel", "factory", "state", "boot", "public"] {
        let existing = directory()
            .parent()
            .unwrap()
            .join("modules-javascript")
            .join(format!("{name}.lil"));
        assert_eq!(
            std::fs::read(existing).unwrap(),
            std::fs::read(directory().join("factory").join(format!("{name}.lil"))).unwrap()
        );
    }
}
fn with_modules<R>(native: bool, edited: bool, inspect: impl FnOnce(Program<'_>, Json) -> R) -> R {
    let directory = directory();
    let entry = if native {
        "native-entry.lil"
    } else {
        "entry.lil"
    };
    let mut modules = crate::module::discover_modules(&directory.join(entry)).unwrap();
    assert_eq!(modules.modules.len(), if native { 2 } else { 12 });
    if edited {
        let module = modules
            .modules
            .iter_mut()
            .find(|m| m.path.ends_with("editable.lil"))
            .unwrap();
        assert_eq!(module.source.matches("return 11").count(), 1);
        module.source = module.source.replace("return 11", "return 12");
    }
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &modules).unwrap();
    let checked = crate::semantic::analyze_modules(&syntax, &modules).unwrap();
    let program = from_checked_modules(&syntax, &checked).unwrap();
    program.verify().unwrap();
    assert_eq!(program.modules().len(), modules.modules.len());
    for (index, interface) in program.modules().iter().enumerate() {
        assert!(interface.source.same(syntax[index].source_identity()));
        assert_eq!(
            program.unit(interface.initializer).unwrap().module.index(),
            index
        );
    }
    let sources: Vec<_> = modules
        .modules
        .iter()
        .map(|m| {
            json!({
                "file":m.path.strip_prefix(&directory).unwrap().to_str().unwrap(),
                "source":m.source,"sha256":digest(&m.source)
            })
        })
        .collect();
    let order: Vec<_> = program
        .initialization()
        .iter()
        .map(|id| {
            modules.modules[program.unit(*id).unwrap().module.index()]
                .path
                .strip_prefix(&directory)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    if !native {
        let factory: Vec<_> = order
            .iter()
            .filter(|p| p.starts_with("factory/"))
            .cloned()
            .collect();
        assert_eq!(
            factory,
            [
                "factory/boot.lil",
                "factory/state.lil",
                "factory/factory.lil",
                "factory/public.lil",
                "factory/barrel.lil",
                "factory/entry.lil"
            ]
        );
        let position = |name: &str| order.iter().position(|p| p == name).unwrap();
        assert!(position("marked/rules.lil") < position("marked/str-slice.lil"));
        assert!(position("marked/str-slice.lil") < position("marked-api.lil"));
        assert!(position("marked-api.lil") < position("entry.lil"));
        assert_eq!(program.exports().len(), 10);
    }
    inspect(
        program,
        json!({"files":sources,"initialization":order,"edited":edited,
        "scope":"original-file discovery/checking/lowering; complete Marked rules module plus explicitly attributed str API excerpt; no concatenated module or full-library claim",
        "config":CONFIG,"config_sha256":digest(CONFIG),"setup_sha256":digest(SETUP),"host_sha256":digest(HOST),"expected_sha256":digest(EXPECTED)}),
    )
}
fn configuration(compact: bool) -> crate::config::ProjectConfig {
    let text = format!("{CONFIG}\n[policy.tactics]\nscalar-replacement='on'\ninlining='on'\nconstant-folding='on'\nstring-pooling='on'\ncall-specialization='on'\nidentifier-mangling='on'\nnaming-search='on'\ntarget-compaction='{}'\n", if compact {"on"} else {"off"});
    toml::from_str(&text).unwrap()
}
fn policy(compact: bool) -> ResolvedPolicy {
    configuration(compact)
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn compilation<'src>(optional: u64, search: bool) -> Compilation<'src> {
    let ledger = if search {
        BudgetLedger::new_baseline_first(
            ResourceLimits::default(),
            BaselineFirstPlan {
                logical_work: WORK,
                retained_bytes: MEMORY,
                terminal_work: 0,
            },
        )
    } else {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: optional,
                baseline_retained_bytes: 0,
                retained_bytes: MEMORY,
            },
        )
    }
    .unwrap();
    Compilation::new(ledger, CheckpointLimit { max_live: 64 }).unwrap()
}
fn cache() -> CacheLimits {
    CacheLimits {
        entries: 64,
        bytes: 4_000_000,
        result_bytes: 100_000,
    }
}
fn request() -> ScalarRequest {
    ScalarRequest {
        max_work: 2_000_000,
        scratch_bytes: 2_000_000,
        output_bytes: 2_000_000,
    }
}
fn local_request() -> LocalFactsRequest {
    LocalFactsRequest {
        work_quota: 100_000,
        result_bytes: 100_000,
    }
}
fn helper_request() -> HelperRequest {
    HelperRequest {
        max_work: 2_000_000,
        scratch_bytes: 2_000_000,
        output_bytes: 2_000_000,
        local_facts: local_request(),
    }
}
fn string_request() -> StringRequest {
    StringRequest {
        max_work: 2_000_000,
        scratch_bytes: 2_000_000,
        output_bytes: 2_000_000,
        local_facts: local_request(),
    }
}
fn named_cell(program: &SemanticView<'_, '_>, name: &str) -> CellId {
    let mut cells = (0..program.cell_count())
        .map(|index| CellId::from_index(index).unwrap())
        .filter(|&cell| program.cell(cell).unwrap().name == name);
    let cell = cells.next().expect(name);
    assert!(cells.next().is_none(), "ambiguous fixture cell {name}");
    cell
}
fn body(program: &SemanticView<'_, '_>, cell: CellId) -> UnitId {
    for index in 0..program.unit_count() {
        let data = program.unit(UnitId::from_index(index).unwrap()).unwrap();
        for operation in &data.operations {
            if !matches!(operation.kind, OperationKind::Initialize(target) if target == cell) {
                continue;
            }
            let value = data.operands(operation.operands).unwrap()[0];
            let mut definition = data.values[value.index()].definition;
            loop {
                let operation = &data.operations[definition.index()];
                if let OperationKind::Closure(unit) = operation.kind {
                    return unit;
                }
                assert!(matches!(operation.kind, OperationKind::CopyValue));
                let value = data.operands(operation.operands).unwrap()[0];
                definition = data.values[value.index()].definition;
            }
        }
    }
    panic!("fixture callable has no original creator")
}
struct Targets {
    record: CellId,
    step: CellId,
    label: CellId,
    string: ValueRef,
    caller: CellId,
    callee: CellId,
    product_step: CellId,
    product_body: UnitId,
    edit_body: UnitId,
    edit_op: OpId,
}
fn targets(program: &SemanticView<'_, '_>) -> Targets {
    let label = named_cell(program, "makeLabel");
    let label_body = body(program, label);
    let data = program.unit(label_body).unwrap();
    let value = data
        .operations
        .iter()
        .find_map(|op| {
            matches!(op.kind, OperationKind::Binary(BinaryOp::Add))
                .then_some(op.result)
                .flatten()
        })
        .unwrap();
    let product_step = named_cell(program, "productStep");
    let edit_body = body(program, named_cell(program, "editTarget"));
    let edit_op = OpId::from_index(
        program
            .unit(edit_body)
            .unwrap()
            .operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(11))))
            .unwrap(),
    )
    .unwrap();
    Targets {
        record: named_cell(program, "state"),
        step: named_cell(program, "step"),
        label,
        string: ValueRef {
            unit: label_body,
            value,
        },
        caller: named_cell(program, "valueState"),
        callee: named_cell(program, "productLocal"),
        product_step,
        product_body: body(program, product_step),
        edit_body,
        edit_op,
    }
}
fn canonical_mapping(program: &Program<'_>) -> String {
    // Test diagnostics over the existing indexed owners, with no revisions or
    // source identity addresses. Compare the full strings before retaining a
    // digest: descriptor equality alone cannot qualify unrelated lowerings.
    let units: Vec<_> = program.units().iter().map(|unit| unit.data()).collect();
    let modules: Vec<_> = program
        .modules()
        .iter()
        .map(|module| {
            (
                module.initializer,
                &module.dependencies,
                &module.imports,
                &module.exports,
            )
        })
        .collect();
    format!(
        "{:?}",
        (
            &program.cells,
            &program.types,
            &program.strings,
            &program.structs,
            &program.enums,
            &program.fields,
            &program.exports,
            &program.initialization,
            program.entry,
            modules,
            units
        )
    )
}
fn execute(javascript: &str, edited: bool) -> Json {
    let script=format!("const events=[];{SETUP}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{HOST}\nprocess.stdout.write(JSON.stringify(events));",serde_json::to_string(javascript).unwrap());
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let observed: Json = serde_json::from_slice(&output.stdout).unwrap();
    let expected = expected(edited);
    assert_eq!(observed, expected, "{javascript}");
    observed
}
fn expected(edited: bool) -> Json {
    let mut expected: Json = serde_json::from_str(EXPECTED).unwrap();
    if edited {
        *expected.as_array_mut().unwrap().last_mut().unwrap() = json!(["edit", 12]);
    }
    expected
}
fn output_metadata(output: OutputTactics) -> Json {
    json!({
        "dead_code_elimination": output.dead_code_elimination,
        "target_compaction": output.target_compaction,
        "literals": format!("{:?}", output.literals),
    })
}
fn sizes(s: Sizes) -> [usize; 3] {
    [s.raw, s.gzip9.unwrap(), s.brotli11.unwrap()]
}
fn artifact(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    metadata: &Json,
    representation: &str,
    compact: bool,
    style: Style,
    edited: bool,
) -> Json {
    // The admitted descriptor is borrowed only during this callback; diagnostic
    // copies belong to the test harness, just like the handed-off JavaScript.
    let descriptor = compiler
        .with_recipe_descriptor(candidate, WorkDomain::Baseline, |words| words.to_vec())
        .unwrap();
    let retained = compiler.ledger().retained_bytes();
    let row=compiler.with_javascript_output(candidate,policy,|output| {
        let artifact=output.render(&Plan::new(style))?;
        let (observed, actual_output)=output.with_artifact(artifact,|view|(execute(view.javascript,edited),view.output))?;
        let raw=output.measure(artifact,Objective::Raw)?;
        let gzip9=output.measure(artifact,Objective::Gzip)?;
        let brotli11=output.measure(artifact,Objective::Brotli)?;
        let javascript=output.take_artifact(artifact)?;
        assert_eq!(javascript.len(),raw);
        Ok::<_,CandidateError>(json!({"representation":representation,"descriptor":descriptor,"compact":compact,"output":output_metadata(actual_output),"style":format!("{style:?}"),"sources":metadata,"setup":SETUP,"host":HOST,"expected":expected(edited),"observed":observed,"javascript_sha256":digest(&javascript),"javascript":javascript,"raw":raw,"gzip9":gzip9,"brotli11":brotli11,"edited":edited,"source_edit":if edited {json!({"file":"editable.lil","from":"return 11","to":"return 12"})} else {Json::Null}}))
    }).unwrap().unwrap();
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    row
}
fn inline(
    c: &mut Compilation<'_>,
    base: CandidateId,
    cell: CellId,
    p: &ResolvedPolicy,
) -> CandidateId {
    match c
        .inline_helper_javascript(base, cell, helper_request(), p, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        HelperOutcome::Published(id) => id,
        other => panic!("private integrated helper must qualify: {other:?}"),
    }
}
fn flat(
    c: &mut Compilation<'_>,
    base: CandidateId,
    cell: CellId,
    p: &ResolvedPolicy,
) -> CandidateId {
    match c
        .scalar_product_javascript(base, cell, request(), p, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        ProductOutcome::Published(id) => id,
        other => panic!("closed value component must qualify: {other:?}"),
    }
}
fn fields(
    c: &mut Compilation<'_>,
    base: CandidateId,
    unit: UnitId,
    p: &ResolvedPolicy,
) -> CandidateId {
    match c
        .scalar_function_javascript(base, unit, request(), p, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        FunctionOutcome::Published(id) => id,
        other => panic!("private product transport must qualify: {other:?}"),
    }
}
fn portfolio(
    c: &mut Compilation<'_>,
    direct: CandidateId,
    t: &Targets,
    p: &ResolvedPolicy,
) -> Vec<(&'static str, CandidateId)> {
    let record = match c
        .scalar_javascript(direct, t.record, request(), p, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        ScalarOutcome::Published(id) => id,
        other => panic!("retained callback record must qualify: {other:?}"),
    };
    let step = inline(c, record, t.step, p);
    let label = inline(c, step, t.label, p);
    let mut rows = vec![
        ("direct", direct),
        ("record", record),
        ("record-step", step),
        ("record-step-label", label),
    ];
    for (name, choice) in [
        ("literal", StringChoice::LiteralAtDefinition),
        (
            "shared",
            StringChoice::SharedLiteral {
                activation: t.string.unit,
            },
        ),
    ] {
        let id = match c
            .represent_string_javascript(
                label,
                &[t.string],
                choice,
                string_request(),
                p,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        {
            StringOutcome::Published(id) => id,
            other => panic!("known string representation: {other:?}"),
        };
        rows.push((name, id));
    }
    let shared = rows[5].1;
    rows.push(("shared-caller", flat(c, shared, t.caller, p)));
    rows.push(("shared-callee", flat(c, shared, t.callee, p)));
    rows.push(("shared-fields", fields(c, shared, t.product_body, p)));
    let all = flat(c, rows[6].1, t.callee, p);
    let all = fields(c, all, t.product_body, p);
    rows.push(("shared-caller-callee-fields", all));
    rows.push((
        "shared-caller-callee-fields-inline",
        inline(c, all, t.product_step, p),
    ));
    rows.push(("shared-inline", inline(c, shared, t.product_step, p)));
    assert_eq!(rows.len(), 12);
    rows
}

#[test]
fn integrated_original_modules_deliver_the_complete_api_with_zero_optional_work() {
    verify_archives();
    for compact in [false, true] {
        with_modules(false, false, |program, metadata| {
            let mut c = compilation(0, false);
            let source = c.adopt_checked(program, WorkDomain::Baseline).unwrap();
            let p = policy(compact);
            let direct = c
                .direct_javascript(source, &p, WorkDomain::Baseline)
                .unwrap();
            for style in STYLES {
                let row = artifact(
                    &mut c, direct, &p, &metadata, "direct", compact, style, false,
                );
                eprintln!("integrated-artifact {row}");
            }
            assert_eq!(c.ledger().work_used(WorkDomain::Optional), 0);
            assert!(c.local_facts_status().is_none());
            assert_eq!(c.finish().retained_bytes(), 0);
        });
    }
}
#[test]
fn integrated_record_helper_string_product_and_call_portfolio_preserves_one_event_oracle() {
    for compact in [false, true] {
        with_modules(false, false, |program, metadata| {
            let mut c = compilation(WORK, false);
            let source = c.adopt_checked(program, WorkDomain::Baseline).unwrap();
            let t = targets(&c.view(source).unwrap());
            let p = policy(compact);
            let direct = c
                .direct_javascript(source, &p, WorkDomain::Baseline)
                .unwrap();
            c.enable_local_facts(cache(), WorkDomain::Optional).unwrap();
            let rows = portfolio(&mut c, direct, &t, &p);
            let mut measured = Vec::new();
            for (name, candidate) in rows {
                for style in STYLES {
                    let row = artifact(
                        &mut c, candidate, &p, &metadata, name, compact, style, false,
                    );
                    measured.push(row.clone());
                    if name != "direct" {
                        eprintln!("integrated-artifact {row}");
                    }
                }
            }
            let minima: Vec<_> = ["raw", "gzip9", "brotli11"]
                .map(|codec| {
                    measured
                        .iter()
                        .map(|row| row[codec].as_u64().unwrap())
                        .min()
                        .unwrap()
                })
                .into();
            eprintln!(
                "integrated-portfolio {}",
                json!({"compact":compact,"declared_structures":12,"names":3,"minima":minima,"scope":"minimum over declared 12-map supplied portfolio; not exhaustive automatic eligible space","sources":metadata})
            );
            assert_eq!(c.finish().retained_bytes(), 0);
        });
    }
}

fn assert_combined_descriptor(words: &[u32], target: &Targets, inline_product: bool) -> Json {
    fn take<'a>(words: &mut &'a [u32], count: usize) -> &'a [u32] {
        let (head, tail) = words.split_at(count);
        *words = tail;
        head
    }
    fn count(words: &mut &[u32]) -> usize {
        take(words, 1)[0] as usize
    }
    let mut remaining = words;
    assert_eq!(
        count(&mut remaining),
        3,
        "recipe format must be inspected explicitly"
    );
    assert_eq!(count(&mut remaining), 1);
    let record = take(&mut remaining, 4);
    assert_eq!(record[0] as usize, target.record.index());

    let helper_count = count(&mut remaining);
    let helpers = take(&mut remaining, helper_count * 2)
        .chunks_exact(2)
        .map(|entry| entry[0] as usize)
        .collect::<std::collections::BTreeSet<_>>();
    let mut expected_helpers = [target.step.index(), target.label.index()]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    if inline_product {
        expected_helpers.insert(target.product_step.index());
    }
    assert_eq!(helpers, expected_helpers);
    assert_eq!(helper_count, helpers.len());

    assert_eq!(count(&mut remaining), 1);
    assert_eq!(
        take(&mut remaining, 5),
        [
            1,
            target.string.unit.index() as u32,
            1,
            target.string.unit.index() as u32,
            target.string.value.index() as u32
        ],
        "one shared string must retain its exact activation and definition"
    );
    assert_eq!(count(&mut remaining), 2);
    let mut products = Vec::new();
    let mut product_schemas = std::collections::BTreeSet::new();
    for _ in 0..2 {
        let header = take(&mut remaining, 4);
        product_schemas.insert(header[1]);
        let fields = take(&mut remaining, header[2] as usize);
        let cells = take(&mut remaining, header[3] as usize * 4)
            .chunks_exact(4)
            .map(|entry| entry[0] as usize)
            .collect::<Vec<_>>();
        assert!(!fields.is_empty());
        assert!(cells.contains(&(header[0] as usize)));
        products.push(json!({"root":header[0],"schema":header[1],"fields":fields,"cells":cells}));
    }
    for selected in [target.caller, target.callee] {
        assert_eq!(
            products
                .iter()
                .filter(|product| {
                    product["cells"]
                        .as_array()
                        .unwrap()
                        .contains(&json!(selected.index()))
                })
                .count(),
            1,
            "both caller and callee product families must be selected"
        );
    }
    assert_eq!(count(&mut remaining), 1);
    assert_eq!(count(&mut remaining), target.product_body.index());
    assert_eq!(count(&mut remaining), 1);
    let parameter = take(&mut remaining, 3);
    assert_eq!(parameter[0], 0);
    assert_eq!(
        product_schemas,
        [parameter[1]]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );
    assert_eq!(
        parameter[2], 1,
        "the product parameter uses field transport"
    );
    assert!(
        remaining.is_empty(),
        "all descriptor words must be accounted for"
    );
    json!({"record":target.record.index(),"inline_helpers":helpers,
        "shared_string":{"unit":target.string.unit.index(),"value":target.string.value.index()},
        "products":products,"field_call_body":target.product_body.index(),
        "product_helper_inlined":inline_product})
}

#[test]
fn public_factory_qualifies_combined_recipes_with_exact_scores_and_original_observations() {
    use crate::compiler_service::{with_checked_path, ServiceOptions};

    verify_archives();
    let mut config = configuration(true);
    // The service's ordinary baseline lifecycle seals the ledger. The bounded
    // manual portfolio below does not run an automatic discovery neighborhood.
    config.javascript.candidate_proposal_limit = Some(0);
    let required_tactics = [
        TacticId::ScalarReplacement,
        TacticId::Inlining,
        TacticId::ConstantFolding,
        TacticId::StringPooling,
        TacticId::CallSpecialization,
    ];
    let forbidden = required_tactics.map(|tactic| {
        let mut config = config.clone();
        config
            .policy
            .as_mut()
            .unwrap()
            .tactics
            .insert(tactic, TacticPermission::Off);
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        (tactic, policy)
    });
    let activity = crate::parser::admitted_arena_activity_for_test();
    let (summary, finished) = with_checked_path(
        &directory().join("entry.lil"),
        &config,
        ServiceOptions {
            objectives: Some(Objectives::All),
            logical_work: WORK,
            retained_bytes: MEMORY,
            ..ServiceOptions::default()
        },
        |session| {
            let now = crate::parser::admitted_arena_activity_for_test();
            assert_eq!(now.0, activity.0, "syntax owner must be gone before backend work");
            assert_eq!(now.1 - activity.1, 12, "each canonical source is parsed once");
            let inputs = session.inputs().clone();
            assert_eq!(inputs["modules"].as_array().unwrap().len(), 12);
            let baseline = session.search_javascript(session.source(), |_| {}).unwrap();
            assert_eq!(baseline.report()["proposals"], 0);
            let baseline_output = baseline.javascript(Objective::Brotli).unwrap();
            assert_eq!(execute(baseline_output.javascript(), false), expected(false));
            let baseline_sizes = CODECS.map(|codec| {
                let artifact = baseline.javascript(codec).unwrap();
                let size = artifact.sizes().get(codec).unwrap();
                assert_eq!(size, crate::compression::measure(artifact.javascript().as_bytes(), codec).unwrap());
                size
            });

            let (compiler, source, policy) = session.parts_mut();
            let target = targets(&compiler.view(source).unwrap());
            let snapshot = compiler.view(source).unwrap().snapshot_identity();
            let meaning = compiler.view(source).unwrap().meaning_identity();
            if compiler.local_facts_status().is_none() {
                compiler.enable_local_facts(cache(), WorkDomain::Optional).unwrap();
            }
            let before_work = compiler.ledger().work_used(WorkDomain::Optional);
            let direct = compiler.direct_javascript(source, policy, WorkDomain::Optional).unwrap();
            let choices = portfolio(compiler, direct, &target, policy);
            let mut retained = Vec::new();
            for (name, candidate) in &choices {
                let inline_product = match *name {
                    "shared-caller-callee-fields" => false,
                    "shared-caller-callee-fields-inline" => true,
                    _ => continue,
                };
                let descriptor = compiler.with_implementation_description(
                    *candidate, WorkDomain::Optional, |description| {
                        assert_eq!(description.snapshot_identity(), Some(snapshot));
                        assert_eq!(description.meaning_identity(), Some(meaning));
                        assert!(description.rewrites().is_empty());
                        description.recipe_words().to_vec()
                    },
                ).unwrap();
                let selected = assert_combined_descriptor(&descriptor, &target, inline_product);
                let artifacts = compiler.with_javascript_output_in(
                    *candidate, policy, WorkDomain::Optional, |output| {
                        STYLES.into_iter().map(|style| {
                            let artifact = output.render(&Plan::new(style))?;
                            Ok((style, output.retain_artifact(artifact)?))
                        }).collect::<Result<Vec<_>, CandidateError>>()
                    },
                ).unwrap().unwrap();
                for (style, artifact) in artifacts {
                    retained.push((*name, inline_product, style, artifact, descriptor.clone(), selected.clone()));
                }
            }
            assert_eq!(retained.len(), 6);
            for (_, candidate) in choices {
                compiler.discard(candidate.semantic_id()).unwrap();
                assert!(compiler.view(candidate.semantic_id()).is_err());
            }
            let mut rows = Vec::new();
            for (name, inline_product, style, artifact, descriptor, selected) in retained {
                let work_before_score = compiler.ledger().work_used(WorkDomain::Optional);
                let sizes = CODECS.map(|codec| compiler.measure_artifact(artifact, codec, WorkDomain::Optional).unwrap());
                let receipts = CODECS.map(|codec| compiler.qualify_artifact(
                    artifact, policy, codec, ArtifactRuntimeEvidence::default(), None,
                    WorkDomain::Optional,
                ).unwrap());
                for (index, receipt) in receipts.iter().enumerate() {
                    assert_eq!(receipt.cost().transfer_bytes as usize, sizes[index]);
                    assert_eq!(receipt.codec(), CODECS[index]);
                    assert_eq!(receipt.policy_fingerprint(), policy.fingerprint());
                }
                let output = compiler.with_qualified_artifact(&receipts[2], |view, provenance| {
                    assert_eq!(view.implementation.recipe_words(), descriptor);
                    assert_eq!(view.implementation.snapshot_identity(), Some(snapshot));
                    assert_eq!(view.implementation.meaning_identity(), Some(meaning));
                    assert_eq!(assert_combined_descriptor(view.implementation.recipe_words(), &target, inline_product), selected);
                    assert_eq!(provenance.naming(), &Plan::new(style));
                    for tactic in required_tactics {
                        assert!(provenance.tactics().iter().any(|usage| usage.tactic == tactic), "missing {tactic:?} provenance");
                    }
                    output_metadata(view.output)
                }).unwrap();
                if style == Style::Global {
                    for (tactic, prohibited) in &forbidden {
                        let refused = compiler.qualify_artifact(
                            artifact, prohibited, Objective::Brotli,
                            ArtifactRuntimeEvidence::default(), None, WorkDomain::Optional,
                        );
                        assert!(matches!(&refused, Err(CandidateError::ForbiddenTactic(found)) if *found == *tactic), "{tactic:?}: {refused:?}");
                    }
                }
                let javascript = compiler.take_qualified_artifact(receipts[2]).unwrap();
                assert_eq!(sizes, CODECS.map(|codec| crate::compression::measure(javascript.as_bytes(), codec).unwrap()));
                let observed = execute(&javascript, false);
                assert_eq!(observed, expected(false));
                let row = json!({"schema":1,"recipe":name,"descriptor":descriptor,"selected_families":selected,
                    "style":format!("{style:?}"),"output":output,"sha256":digest(&javascript),"javascript":javascript,
                    "sizes":{"raw":sizes[0],"gzip9":sizes[1],"brotli11":sizes[2]},
                    "qualified_codecs":["Raw","Gzip","Brotli"],"policy_fingerprint":policy.fingerprint(),
                    "work_before_score":work_before_score,"work_after_handoff":compiler.ledger().work_used(WorkDomain::Optional),
                    "observations":observed,"candidate_disposed_before_scoring":true});
                eprintln!("public-combined-artifact {row}");
                rows.push(row);
            }
            json!({"schema":1,"outputs":rows.len(),"recipes":2,"styles":3,"inputs":inputs,
                "policy":policy.receipt(),"baseline_sizes":{"raw":baseline_sizes[0],"gzip9":baseline_sizes[1],"brotli11":baseline_sizes[2]},
                "optional_work_before_families":before_work,"optional_work_after_handoffs":compiler.ledger().work_used(WorkDomain::Optional),
                "peak_retained_bytes":compiler.ledger().peak_retained_bytes(),"required_tactic_vetoes":required_tactics,
                "scope":"public with_checked_path; zero-proposal service baseline followed by two existing manually selected cross-family recipes; common immutable artifact qualification for every codec",
                "limitations":"not an automatic search or optimum claim; source remains live, selected candidates are disposed before scoring; test-owned metadata, runtime and independent canonical codec replay are outside compiler accounting"})
        },
    ).unwrap();
    assert_eq!(finished.ledger.retained_bytes(), 0);
    let mut summary = summary;
    summary["retained_bytes_after_finish"] = json!(finished.ledger.retained_bytes());
    summary["frontend_resources"] = finished.report["resources"].clone();
    eprintln!("public-combined-summary {summary}");
}

#[test]
fn integrated_search_winners_belong_to_the_exact_executed_measured_union() {
    let (supplied, supplied_metadata, supplied_mapping) = with_modules(
        false,
        false,
        |program, metadata| {
            // Each compilation adopts its own exclusive original-source lowering.
            // Below, exact source metadata and indexed semantic owners establish
            // the common descriptor namespace before comparing search evidence.
            let p = policy(true);
            let mapping = canonical_mapping(&program);
            let supplied = {
                let mut manual = compilation(WORK, false);
                let source = manual.adopt_checked(program, WorkDomain::Baseline).unwrap();
                let t = targets(&manual.view(source).unwrap());
                let direct = manual
                    .direct_javascript(source, &p, WorkDomain::Baseline)
                    .unwrap();
                manual
                    .enable_local_facts(cache(), WorkDomain::Optional)
                    .unwrap();
                let mut supplied = Vec::new();
                for (name, candidate) in portfolio(&mut manual, direct, &t, &p) {
                    for style in STYLES {
                        let row = artifact(
                            &mut manual,
                            candidate,
                            &p,
                            &metadata,
                            name,
                            true,
                            style,
                            false,
                        );
                        supplied.push(json!({"representation":name,"descriptor":row["descriptor"],
                        "style":row["style"],"output":row["output"],"javascript_sha256":row["javascript_sha256"],
                        "sizes":[row["raw"],row["gzip9"],row["brotli11"]]}));
                    }
                }
                assert_eq!(supplied.len(), 36);
                assert_eq!(manual.finish().retained_bytes(), 0);
                supplied
            };
            (supplied, metadata, mapping)
        },
    );
    with_modules(false, false, |program, metadata| {
        assert_eq!(
            metadata, supplied_metadata,
            "oracle and search original modules differ"
        );
        let mapping = canonical_mapping(&program);
        assert!(
            mapping == supplied_mapping,
            "oracle and search indexed semantic owners differ"
        );
        let canonical_mapping_sha256 = digest(&mapping);
        let p = policy(true);
        let mut c = compilation(WORK, true);
        let source = c.adopt_checked(program, WorkDomain::Baseline).unwrap();
        let mut measured = Vec::new();
        let mut measured_recipes = Vec::new();
        let request = SearchRequest {
            objectives: Objectives::All,
            scalar: request(),
            helper: helper_request(),
            string: string_request(),
            facts_cache: cache(),
        };
        let search=c.search_javascript_observed(source,&p,request,|observation| {
            let observed=execute(observation.javascript,false);let score=sizes(observation.sizes);measured.push((digest(observation.javascript),score));
            measured_recipes.push((observation.recipe_descriptor.whole_words().expect("Whole cohort").to_vec(),observation.naming.style,observation.output,observation.candidate));
            eprintln!("integrated-search-artifact {}",json!({"sources":metadata,"descriptor":observation.recipe_descriptor.whole_words().expect("Whole cohort"),"style":format!("{:?}",observation.naming.style),"output":output_metadata(observation.output),"baseline":observation.baseline,"javascript":observation.javascript,"javascript_sha256":digest(observation.javascript),"raw":score[0],"gzip9":score[1],"brotli11":score[2],"expected":expected(false),"observed":observed}));
        }).unwrap();
        assert!(!measured.is_empty());
        let mut winners = Vec::new();
        for (index, objective) in CODECS.into_iter().enumerate() {
            let winner = search
                .with_winner(objective, |view, plan| {
                    let score = sizes(view.sizes);
                    let observed = execute(view.javascript, false);
                    let hash = digest(view.javascript);
                    let ((descriptor, _, _, _), _) = measured_recipes
                        .iter()
                        .zip(&measured)
                        .find(|((_, style, output, candidate), (observed_hash, observed_score))| {
                            *candidate == view.candidate
                                && *style == plan.style
                                && *output == view.output
                                && observed_hash == &hash
                                && *observed_score == score
                        })
                        .expect("winner must match one executed candidate, name and actual output mode");
                    assert_eq!(
                        score[index],
                        measured.iter().map(|row| row.1[index]).min().unwrap()
                    );
                    json!({"objective":format!("{objective:?}"),"descriptor":descriptor,"style":format!("{:?}",plan.style),
                        "output":output_metadata(view.output),"javascript_sha256":hash,"sizes":score,"observed":observed})
                })
                .unwrap();
            winners.push(winner);
        }
        let counters = search.counters();
        let supplied_minima: Vec<_> = (0..3)
            .map(|index| {
                supplied
                    .iter()
                    .map(|row| row["sizes"][index].as_u64().unwrap())
                    .min()
                    .unwrap()
            })
            .collect();
        let supplied_comparison: Vec<_> = supplied.iter().map(|row| {
            let descriptor: Vec<u32> = serde_json::from_value(row["descriptor"].clone()).unwrap();
            let style = row["style"].as_str().unwrap();
            let matches = |words: &[u32], naming: Style, output: OutputTactics| {
                words == descriptor.as_slice()
                    && format!("{naming:?}") == style
                    && output_metadata(output) == row["output"]
            };
            let reached = measured_recipes.iter().any(|(words, naming, output, _)| {
                matches(words, *naming, *output)
            });
            for ((words, naming, output, _), (hash, score)) in measured_recipes.iter().zip(&measured) {
                if matches(words, *naming, *output) {
                    assert_eq!(hash, row["javascript_sha256"].as_str().unwrap(),
                        "same recipe, naming and actual output must reproduce the supplied complete bytes");
                    assert_eq!(json!(score), row["sizes"]);
                }
            }
            json!({"representation":row["representation"],"descriptor":descriptor,"style":style,
                "output":row["output"],"javascript_sha256":row["javascript_sha256"],"sizes":row["sizes"],"reached":reached})
        }).collect();
        let search_minus_supplied: Vec<_> = (0..3)
            .map(|index| {
                measured.iter().map(|row| row.1[index]).min().unwrap() as i64
                    - supplied_minima[index] as i64
            })
            .collect();
        eprintln!(
            "integrated-search-summary {}",
            json!({"measured":measured.len(),"winners":winners,"structures":counters.structures,"proof_queries":counters.proof_queries,"renders":counters.renders,"codec_probes":counters.codec_probes,"proposals":counters.proposals,"structural_attempts":counters.structural_attempts,"unknown_proofs":counters.unknown_proofs,"skipped_unknown":counters.skipped_unknown,"skipped_truncated":counters.skipped_truncated,"duplicate_states":counters.duplicate_states,"inactive_function_layouts":counters.inactive_function_layouts,"beam_evictions":counters.beam_evictions,"inventory_truncated":counters.inventory_truncated,"stopped":search.stopped().map(|r|format!("{r:?}")),"work_baseline":search.ledger().work_used(WorkDomain::Baseline),"work_optional":search.ledger().work_used(WorkDomain::Optional),"peak_retained_bytes":search.ledger().peak_retained_bytes(),"supplied_portfolio":supplied_comparison,"supplied_minima":supplied_minima,"search_minus_supplied":search_minus_supplied,"canonical_mapping_sha256":canonical_mapping_sha256,"comparison_scope":"independent exclusive lowerings of identical original modules with exact indexed semantic-owner equality (excluding revisions/source identities); 12 independently published eligible maps times three names with each policy-default actual output, compact=true; matching includes actual DCE/compaction/literal mode; supplied scope is not exhaustive"})
        );
        drop(search);
        assert_eq!(c.finish().retained_bytes(), 0);
    });
}
#[test]
fn integrated_private_edit_changes_the_public_observation_and_reuses_unrelated_facts() {
    let clean = with_modules(false, true, |program, metadata| {
        let mut c = compilation(WORK, false);
        let source = c.adopt_checked(program, WorkDomain::Baseline).unwrap();
        let p = policy(true);
        let candidate = c
            .direct_javascript(source, &p, WorkDomain::Baseline)
            .unwrap();
        let row = artifact(
            &mut c,
            candidate,
            &p,
            &metadata,
            "clean-edit",
            true,
            Style::Scoped,
            true,
        );
        assert_eq!(c.finish().retained_bytes(), 0);
        row
    });
    with_modules(false, false, |program, metadata| {
        let mut c = compilation(WORK, false);
        let source = c.adopt_checked(program, WorkDomain::Baseline).unwrap();
        let t = targets(&c.view(source).unwrap());
        c.enable_local_facts(cache(), WorkDomain::Baseline).unwrap();
        let cold = c
            .with_local_facts(WorkDomain::Baseline, 1, |group| {
                group
                    .query(source, t.string.unit, local_request())
                    .unwrap()
                    .cache_hit()
            })
            .unwrap();
        assert!(!cold);
        let revision = c.view(source).unwrap().unit_revision(t.edit_body).unwrap();
        let kind = OperationKind::Constant(Constant::Integer(12));
        let changed = c
            .edit_source(
                source,
                &[UnitPatch {
                    unit: t.edit_body,
                    expected_revision: revision,
                    operations: &[OperationPatch {
                        operation: t.edit_op,
                        kind: &kind,
                        operands: &[],
                    }],
                    places: &[],
                }],
                WorkDomain::Optional,
            )
            .unwrap();
        assert_eq!(c.view(changed).unwrap().changes().len(), 1);
        let warm = c
            .with_local_facts(WorkDomain::Optional, 1, |group| {
                group
                    .query(changed, t.string.unit, local_request())
                    .unwrap()
                    .cache_hit()
            })
            .unwrap();
        assert!(warm);
        let p = policy(true);
        let candidate = c
            .direct_javascript(changed, &p, WorkDomain::Baseline)
            .unwrap();
        let mut row = artifact(
            &mut c,
            candidate,
            &p,
            &metadata,
            "retained-edit",
            true,
            Style::Scoped,
            true,
        );
        assert_eq!(row["javascript"], clean["javascript"]);
        row["clean_rebuild"] = clean;
        row["unrelated_fact_cache_hit"] = json!(warm);
        eprintln!("integrated-edit-artifact {row}");
        let original = c
            .direct_javascript(source, &p, WorkDomain::Baseline)
            .unwrap();
        artifact(
            &mut c,
            original,
            &p,
            &metadata,
            "retained-original",
            true,
            Style::Scoped,
            false,
        );
        assert_eq!(c.finish().retained_bytes(), 0);
    });
}
#[test]
fn integrated_value_reference_module_executes_unchanged_through_native_and_javascript() {
    with_modules(true, false, |program, metadata| {
        let mut c = compilation(WORK, false);
        let source = c.adopt_checked(program, WorkDomain::Baseline).unwrap();
        let config: crate::config::ProjectConfig = toml::from_str(CONFIG).unwrap();
        let policy = config.resolve_policy(CompilationRequest::Native).unwrap();
        let before = c.ledger().retained_bytes();
        let codec_work = c.ledger().work_by_kind(WorkKind::Codec);
        let native = c
            .with_native_c(source, &policy, WorkDomain::Baseline, |output| {
                output.take_c()
            })
            .unwrap();
        assert_eq!(c.ledger().retained_bytes(), before);
        assert_eq!(c.ledger().work_by_kind(WorkKind::Codec), codec_work);
        let executions = super::native_tests::compile_and_execute(
            &native,
            NATIVE_EXPECTED,
            "integrated-value-reference",
        );
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                // This original-module entry has no public exports, but its
                // private product/reference calls still require Module frames.
                preserve_root_exports: true,
            })
            .unwrap();
        let candidate = c
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let rows = c.with_javascript_output(candidate, &policy, |output| {
            let mut rows = Vec::new();
            for style in STYLES {
                let artifact = output.render(&Plan::new(style))?;
                let (observed, actual_output) = output.with_artifact(artifact, |view| {
                    let ran = super::native_tests::execute(Command::new("node")
                        .args(["--input-type=module", "-e", view.javascript]));
                    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
                    assert!(ran.stderr.is_empty());
                    let observed = String::from_utf8(ran.stdout).unwrap();
                    assert_eq!(observed, NATIVE_EXPECTED);
                    (observed, view.output)
                })?;
                let raw = output.measure(artifact, Objective::Raw)?;
                let gzip9 = output.measure(artifact, Objective::Gzip)?;
                let brotli11 = output.measure(artifact, Objective::Brotli)?;
                let text = output.take_artifact(artifact)?;
                rows.push(json!({"style":format!("{style:?}"),"output":output_metadata(actual_output),"javascript_sha256":digest(&text),
                    "javascript":text,"observed":observed,"raw":raw,"gzip9":gzip9,"brotli11":brotli11}));
            }
            Ok::<_, CandidateError>(rows)
        }).unwrap().unwrap();
        eprintln!(
            "integrated-native-artifact {}",
            json!({"sources":metadata,"c_sha256":digest(&native),"c":native,"expected":NATIVE_EXPECTED,"executions":executions,"javascript":rows,"scope":"same original products.lil module; host-free value/reference subset, not full factory or Marked native support"})
        );
        assert_eq!(c.finish().retained_bytes(), 0);
    });
}

#[path = "integrated_observation_tests.rs"]
mod integrated_observation_tests;
/// 006: one edited unit, compiled two ways under the same ledger rules. The
/// reused arm patches the retained semantic snapshot and keeps every other
/// unit's cached facts; the fresh arm adopts a newly converted program and
/// recomputes everything. Both must deliver identical bytes. The counters are
/// the comparison the plan asks for, not a speed claim: formation still runs
/// in full on both arms, so reuse can only save adoption and fact work.
#[test]
fn integrated_edit_reuse_and_fresh_recomputation_under_the_same_rules() {
    fn total(c: &Compilation<'_>) -> u64 {
        c.ledger().work_used(WorkDomain::Baseline) + c.ledger().work_used(WorkDomain::Optional)
    }
    // Units, cache hits and the physically executed fact steps. A hit is
    // still billed its recorded logical work, so budgets and outcomes cannot
    // depend on cache state; only the physical counter shows the saving.
    fn facts_for_every_unit(
        c: &mut Compilation<'_>,
        source: SemanticId,
        domain: WorkDomain,
    ) -> (usize, usize, u128) {
        let units = c.view(source).unwrap().unit_count();
        c.with_local_facts(domain, units, |group| {
            for index in 0..units {
                group
                    .query(source, UnitId::from_index(index).unwrap(), local_request())
                    .unwrap();
            }
            let work = group.work();
            (units, work.cache_hits, work.executed_fact_steps)
        })
        .unwrap()
    }
    fn deliver(c: &mut Compilation<'_>, source: SemanticId, p: &ResolvedPolicy) -> String {
        let candidate = c.direct_javascript(source, p, WorkDomain::Baseline).unwrap();
        c.with_javascript_output(candidate, p, |output| {
            let artifact = output.render(&Plan::new(Style::Scoped))?;
            output.take_artifact(artifact)
        })
        .unwrap()
        .unwrap()
    }
    let p = policy(true);
    let frontend = std::time::Instant::now();
    let fresh = with_modules(false, true, |program, _| {
        let frontend_ms = frontend.elapsed().as_secs_f64() * 1e3;
        let clock = std::time::Instant::now();
        let mut c = compilation(WORK, false);
        let source = c.adopt_checked(program, WorkDomain::Baseline).unwrap();
        c.enable_local_facts(cache(), WorkDomain::Baseline).unwrap();
        let before_facts = total(&c);
        let (units, hits, executed) = facts_for_every_unit(&mut c, source, WorkDomain::Baseline);
        let facts_work = total(&c) - before_facts;
        let javascript = deliver(&mut c, source, &p);
        let row = json!({
            "arm": "fresh", "units": units, "fact_cache_hits": hits,
            "fact_work": facts_work, "executed_fact_steps": executed as u64, "work": total(&c),
            "retained_bytes": c.ledger().retained_bytes(),
            "peak_retained_bytes": c.ledger().peak_retained_bytes(),
            "frontend_ms": frontend_ms, "compile_ms": clock.elapsed().as_secs_f64() * 1e3,
        });
        assert_eq!(c.finish().retained_bytes(), 0);
        (row, javascript)
    });
    let reused = with_modules(false, false, |program, _| {
        let mut c = compilation(WORK, false);
        let source = c.adopt_checked(program, WorkDomain::Baseline).unwrap();
        let t = targets(&c.view(source).unwrap());
        c.enable_local_facts(cache(), WorkDomain::Baseline).unwrap();
        facts_for_every_unit(&mut c, source, WorkDomain::Baseline);
        let previous = deliver(&mut c, source, &p);
        let before = total(&c);
        let retained_before = c.ledger().retained_bytes();
        let clock = std::time::Instant::now();
        let revision = c.view(source).unwrap().unit_revision(t.edit_body).unwrap();
        let kind = OperationKind::Constant(Constant::Integer(12));
        let changed = c
            .edit_source(
                source,
                &[UnitPatch {
                    unit: t.edit_body,
                    expected_revision: revision,
                    operations: &[OperationPatch {
                        operation: t.edit_op,
                        kind: &kind,
                        operands: &[],
                    }],
                    places: &[],
                }],
                WorkDomain::Optional,
            )
            .unwrap();
        let edit_work = total(&c) - before;
        let before_facts = total(&c);
        let (units, hits, executed) = facts_for_every_unit(&mut c, changed, WorkDomain::Optional);
        let facts_work = total(&c) - before_facts;
        let javascript = deliver(&mut c, changed, &p);
        assert_ne!(javascript, previous, "the edit must change the delivered program");
        let row = json!({
            "arm": "reused", "units": units, "fact_cache_hits": hits,
            "edit_work": edit_work, "fact_work": facts_work, "executed_fact_steps": executed as u64,
            "work": total(&c) - before,
            "retained_bytes": c.ledger().retained_bytes(),
            "history_retained_bytes": retained_before,
            "peak_retained_bytes": c.ledger().peak_retained_bytes(),
            "compile_ms": clock.elapsed().as_secs_f64() * 1e3,
        });
        assert_eq!(c.finish().retained_bytes(), 0);
        (row, javascript)
    });
    assert_eq!(reused.1, fresh.1, "reused and fresh compilation deliver the same bytes");
    let (fresh, reused) = (fresh.0, reused.0);
    eprintln!("integrated-reuse-versus-fresh {}", json!({"fresh": fresh, "reused": reused}));
    // Every unit but the edited one is answered from the retained cache.
    assert_eq!(fresh["fact_cache_hits"], 0);
    assert_eq!(reused["fact_cache_hits"].as_u64(), Some(reused["units"].as_u64().unwrap() - 1));
    // Logical billing is identical by design; physical execution is not.
    assert_eq!(reused["fact_work"], fresh["fact_work"]);
    assert!(reused["executed_fact_steps"].as_u64() < fresh["executed_fact_steps"].as_u64());
}
