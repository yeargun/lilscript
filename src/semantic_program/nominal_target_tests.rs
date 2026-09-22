//! The Motion geometry declarations are byte-exact library input. Mutation,
//! aliasing and private-return clients are authored integration fixtures. The
//! fixed trace is checked before measuring complete emitted artifacts.
use super::native_tests::{compile_and_execute, execute};
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain, WorkKind,
};
use crate::structured_js::selection::{Objective, Plan, Style};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const FILES: &[(&str, &str)] = &[
    (
        "entry.lil",
        include_str!("fixtures/nominal-modules/entry.lil"),
    ),
    (
        "motion-geometry.lil",
        include_str!("fixtures/nominal-modules/motion-geometry.lil"),
    ),
    (
        "left-types.lil",
        include_str!("fixtures/nominal-modules/left-types.lil"),
    ),
    (
        "right-types.lil",
        include_str!("fixtures/nominal-modules/right-types.lil"),
    ),
    (
        "helpers.lil",
        include_str!("fixtures/nominal-modules/helpers.lil"),
    ),
    (
        "private-left.lil",
        include_str!("fixtures/nominal-modules/private-left.lil"),
    ),
    (
        "private-right.lil",
        include_str!("fixtures/nominal-modules/private-right.lil"),
    ),
    (
        "only-types.lil",
        include_str!("fixtures/nominal-modules/only-types.lil"),
    ),
    (
        "incompatible-value.lil",
        include_str!("fixtures/nominal-modules/incompatible-value.lil"),
    ),
    (
        "incompatible-reference.lil",
        include_str!("fixtures/nominal-modules/incompatible-reference.lil"),
    ),
];
const EXPECTED: &str = include_str!("fixtures/nominal-modules/expected.out");
const ORIGIN: &str = include_str!("fixtures/nominal-modules/motion-geometry.origin.json");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-nominal-modules-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        for &(name, source) in FILES {
            std::fs::write(path.join(name), source).unwrap();
        }
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("nominal module failure inputs: {}", self.0.display());
        } else {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn with_modules<R>(
    entry: &str,
    inspect: impl FnOnce(Program<'_>, &crate::module::ModuleSet) -> R,
) -> R {
    let directory = Directory::new();
    let modules = crate::module::discover_modules(&directory.0.join(entry)).unwrap();
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &modules).unwrap();
    let checked = crate::semantic::analyze_modules(&syntax, &modules).unwrap();
    let program = from_checked_modules(&syntax, &checked).unwrap();
    program.verify().unwrap();
    inspect(program, &modules)
}
fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 256_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 4 },
    )
    .unwrap()
}
fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
fn filename(modules: &crate::module::ModuleSet, id: ModuleId) -> &str {
    modules.modules[id.index()]
        .path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
}
fn struct_export(program: &Program<'_>, module: ModuleId, name: &str) -> NominalId {
    program.exports[program.modules()[module.index()].exports.clone()]
        .iter()
        .find_map(|export| match export.target {
            InterfaceTarget::Struct(id) if export.name == name => Some(id),
            _ => None,
        })
        .unwrap()
}
fn schema_evidence(
    program: &Program<'_>,
    modules: &crate::module::ModuleSet,
) -> Vec<serde_json::Value> {
    program
        .structs()
        .iter()
        .map(|schema| {
            let source = &modules.modules[schema.module.index()].source;
            assert!(schema.span.start < schema.span.end);
            assert!(schema.span.end <= source.len(), "original source span");
            assert!(source[schema.span.start..schema.span.end].contains(&schema.name));
            for field in &program.fields()[schema.fields.clone()] {
                assert_eq!(field.owner, schema.identity);
            }
            serde_json::json!({
                "name": schema.name, "identity": format!("{:?}",schema.identity),
                "module": filename(modules,schema.module), "span": schema.span,
                "fields": program.fields()[schema.fields.clone()].iter()
                    .map(|field| serde_json::json!({"name":field.name,"member":format!("{:?}",field.identity)})).collect::<Vec<_>>(),
            })
        })
        .collect()
}
fn qualify(
    case: &str,
    entry: &str,
    program: Program<'_>,
    modules: &crate::module::ModuleSet,
    expected: &str,
) {
    assert!(program.value_exports().next().is_none());
    assert!(
        !program.exports().is_empty(),
        "types are a real public interface"
    );
    assert!(program
        .exports()
        .iter()
        .all(|export| matches!(export.target, InterfaceTarget::Struct(_))));
    let schemas = schema_evidence(&program, modules);
    let initialization: Vec<_> = program
        .initialization()
        .iter()
        .map(|&unit| filename(modules, program.unit(unit).unwrap().module))
        .collect();
    let source_rows: Vec<_> = modules
        .modules
        .iter()
        .map(|module| {
            serde_json::json!({"file":module.path.file_name().unwrap().to_str().unwrap(),
                "source":module.source,"source_sha256":digest(&module.source)})
        })
        .collect();
    let origin: serde_json::Value = serde_json::from_str(ORIGIN).unwrap();
    let geometry = modules
        .modules
        .iter()
        .find(|module| module.path.file_name().unwrap() == "motion-geometry.lil")
        .unwrap();
    assert_eq!(origin["archived_sha256"], digest(&geometry.source));
    assert_eq!(origin["original_sha256"], origin["archived_sha256"]);

    let mut compilation = compilation();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let native_policy = crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::Native)
        .unwrap();
    let before = compilation.ledger().retained_bytes();
    let codecs_before = compilation.ledger().work_by_kind(WorkKind::Codec);
    let c = compilation
        .with_native_c(source, &native_policy, WorkDomain::Baseline, |output| {
            output.take_c()
        })
        .unwrap();
    assert_eq!(compilation.ledger().retained_bytes(), before);
    assert_eq!(
        compilation.ledger().work_by_kind(WorkKind::Codec),
        codecs_before
    );
    assert_eq!(compilation.checkpoint_count(), 1);
    let executions = compile_and_execute(&c, expected, case);
    assert_eq!(executions.len(), 7, "two compilers, O0/O2/UBSan, and Clang ASan");

    let mut javascript = Vec::new();
    for compact in [false, true] {
        let config: crate::config::ProjectConfig = toml::from_str(&format!(
            "[javascript]\nstrip_console=false\n[policy.tactics]\ntarget-compaction='{}'\n",
            if compact { "on" } else { "off" },
        ))
        .unwrap();
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let candidate = compilation
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        compilation
            .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
            .unwrap();
        for style in [Style::Global, Style::Scoped, Style::Source] {
            let retained = compilation.ledger().retained_bytes();
            let row = compilation.with_javascript_output(candidate, &policy, |output| {
                let artifact = output.render(&Plan::new(style))?;
                let observed = output.with_artifact(artifact, |view| {
                    // Actual Module loading also checks that public types did
                    // not become observable JavaScript namespace entries.
                    let script = format!(
                        "const library=await import('data:text/javascript,'+encodeURIComponent({}));if(Object.keys(library).length!==0)throw new Error('type export became runtime binding');",
                        serde_json::to_string(view.javascript).unwrap(),
                    );
                    let ran = execute(Command::new("node").args(["--input-type=module", "-e", &script]));
                    assert!(ran.status.success(), "{case}/{compact}/{style:?}: {}", String::from_utf8_lossy(&ran.stderr));
                    assert!(ran.stderr.is_empty());
                    let observed = String::from_utf8(ran.stdout).unwrap();
                    assert_eq!(observed, expected, "{case}/{compact}/{style:?}");
                    observed
                })?;
                let raw = output.measure(artifact, Objective::Raw)?;
                let gzip9 = output.measure(artifact, Objective::Gzip)?;
                let brotli11 = output.measure(artifact, Objective::Brotli)?;
                let text = output.take_artifact(artifact)?;
                assert_eq!(raw,text.len());
                Ok::<_, CandidateError>(serde_json::json!({
                    "compact":compact,"style":format!("{style:?}"),"javascript":text,
                    "javascript_sha256":digest(&text),"raw":raw,"gzip9":gzip9,"brotli11":brotli11,
                    "expected":expected,"observed":observed,"runtime_exports":[],
                }))
            }).unwrap().unwrap();
            assert_eq!(compilation.ledger().retained_bytes(), retained);
            javascript.push(row);
        }
    }
    eprintln!(
        "nominal-module-artifact {}",
        serde_json::json!({
            "case":case,"entry":entry,"sources":source_rows,"library_origin":origin,
            "schemas":schemas,"initialization":initialization,"expected":expected,
            "c":c,"c_sha256":digest(&c),"executions":executions,"javascript":javascript,
            "qualification":"byte-exact Motion geometry declarations plus authored original-module clients; direct semantic-core JS and native; fixed observations; no full Motion claim",
        })
    );
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

#[test]
fn imported_motion_schemas_preserve_diamond_identity_copies_and_overlapping_references() {
    with_modules("entry.lil", |program, modules| {
        assert_eq!(program.modules().len(), 7);
        assert_eq!(program.structs().len(), 8);
        let module = |name| {
            ModuleId::from_index(
                modules
                    .modules
                    .iter()
                    .position(|m| m.path.file_name().unwrap() == name)
                    .unwrap(),
            )
            .unwrap()
        };
        let geometry = module("motion-geometry.lil");
        let left = module("left-types.lil");
        let right = module("right-types.lil");
        assert_eq!(
            struct_export(&program, geometry, "Axis"),
            struct_export(&program, left, "Interval")
        );
        assert_eq!(
            struct_export(&program, left, "Interval"),
            struct_export(&program, right, "SpanAxis")
        );
        assert_eq!(
            struct_export(&program, geometry, "Box"),
            struct_export(&program, left, "Frame")
        );
        assert_eq!(
            struct_export(&program, left, "Frame"),
            struct_export(&program, right, "OtherFrame")
        );
        assert_eq!(
            struct_export(&program, program.entry_module(), "GeometryBox"),
            struct_export(&program, geometry, "Box")
        );
        let private: Vec<_> = program
            .structs()
            .iter()
            .filter(|schema| schema.name == "Private")
            .collect();
        assert_eq!(private.len(), 2);
        assert_ne!(private[0].identity, private[1].identity);
        assert_ne!(private[0].module, private[1].module);
        assert_ne!(private[0].fields.len(), private[1].fields.len());
        for name in [
            "GeometryBox",
            "GeometryAxis",
            "Interval",
            "Frame",
            "SpanAxis",
            "OtherFrame",
        ] {
            assert!(
                !program.cells().iter().any(|cell| cell.name == name),
                "type alias must not create a cell: {name}"
            );
        }
        qualify(
            "motion-geometry-diamond-references",
            "entry.lil",
            program,
            modules,
            EXPECTED,
        );
    });
}
#[test]
fn unchanged_motion_geometry_type_only_interface_has_no_runtime_exports_or_cells() {
    with_modules("only-types.lil", |program, modules| {
        assert_eq!(program.modules().len(), 2);
        assert_eq!(program.structs().len(), 6);
        assert_eq!(program.exports().len(), 6);
        assert!(program.cells().is_empty());
        for unit in program.units() {
            assert!(unit.data().operations.is_empty());
        }
        qualify(
            "motion-geometry-types-only",
            "only-types.lil",
            program,
            modules,
            "",
        );
    });
}
fn reject_same_name(entry: &str) {
    let directory = Directory::new();
    let modules = crate::module::discover_modules(&directory.0.join(entry)).unwrap();
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &modules).unwrap();
    let errors = crate::semantic::analyze_modules(&syntax, &modules)
        .expect_err("same diagnostic name does not make private declarations compatible");
    assert_eq!(errors.module, modules.root);
    assert!(errors.error.message.contains("Private"), "{errors}");
}
#[test]
fn same_spelled_private_schema_cannot_cross_a_value_parameter() {
    reject_same_name("incompatible-value.lil");
}
#[test]
fn same_spelled_private_schema_cannot_cross_a_reference_parameter() {
    reject_same_name("incompatible-reference.lil");
}
