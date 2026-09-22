//! Real publication transactions exercise the fixed verifier's authority and
//! accounting. Fixture construction precedes adoption; no test creates a
//! FixedUnitEdits capability or supplies an unchecked verifier footprint.
use super::*;
use crate::compilation_policy::{BudgetError, BudgetPlan, ResourceLimits};
use std::fmt::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const AMPLE: u64 = 500_000_000;
const MEMORY: u64 = 256_000_000;

fn owner<'src>(optional_work: u64, optional_bytes: u64) -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: AMPLE,
                optional_work,
                baseline_retained_bytes: MEMORY - optional_bytes,
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
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program)
}
fn function(program: &Program<'_>, name: &str) -> UnitId {
    let CellBinding::Function(unit) = program
        .cells
        .iter()
        .find(|cell| cell.name == name)
        .unwrap()
        .binding
    else {
        panic!("named function");
    };
    unit
}
fn integer(program: &Program<'_>, unit: UnitId, value: i32) -> OpId {
    OpId::from_index(program.unit(unit).unwrap().operations.iter().position(|operation|
        matches!(operation.kind, OperationKind::Constant(Constant::Integer(found)) if found == value)
    ).unwrap()).unwrap()
}
fn literal_edit(
    compiler: &mut Compilation<'_>,
    source: SemanticId,
    unit: UnitId,
    operation: OpId,
    constant: Constant,
    domain: WorkDomain,
    advance: bool,
) -> Result<SemanticId, PublicationError> {
    let revision = compiler.view(source)?.unit_revision(unit).unwrap();
    let kind = OperationKind::Constant(constant);
    let patches = [UnitPatch {
        unit,
        expected_revision: revision,
        operations: &[OperationPatch {
            operation,
            kind: &kind,
            operands: &[],
        }],
        places: &[],
    }];
    if advance {
        compiler.advance_source(source, &patches, domain)
    } else {
        compiler.edit_source(source, &patches, domain)
    }
}
fn unchanged(
    compiler: &mut Compilation<'_>,
    source: SemanticId,
    unit: UnitId,
    revision: RevisionId,
    retained: u64,
) {
    assert_eq!(compiler.checkpoint_count(), 1);
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    assert_eq!(
        compiler.view(source).unwrap().unit_revision(unit),
        Some(revision)
    );
    compiler
        .with_semantic(source, |program, uses, _| {
            program.verify().unwrap();
            assert!(uses.valid_for(program));
        })
        .unwrap();
}
fn execute(compiler: &mut Compilation<'_>, source: SemanticId) -> String {
    let javascript = compiler
        .with_semantic(source, |program, uses, _| {
            assert!(uses.valid_for(program));
            program
                .to_javascript()
                .unwrap()
                .render(crate::structured_js::PrintPolicy {
                    mangle_bindings: true,
                })
                .unwrap()
        })
        .unwrap();
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &javascript])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn fixed_edits_reuse_an_unchanged_large_owned_callable_header() {
    let mut observed = None;
    for width in [1, 4096] {
        let parameters = std::iter::repeat_n("int", width)
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("int editTarget(func({parameters})->int unused){{return 11;}}");
        let measured = checked(&text, |program| {
            let unit = function(&program, "editTarget");
            let operation = integer(&program, unit, 11);
            assert_eq!(program.unit(unit).unwrap().parameters.len(), 1);
            let mut compiler = owner(AMPLE, MEMORY);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let before = compiler.ledger().retained_bytes();
            let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
            let start = compiler.ledger().work_used(WorkDomain::Baseline);
            assert_eq!(
                literal_edit(
                    &mut compiler,
                    source,
                    unit,
                    operation,
                    Constant::Boolean(true),
                    WorkDomain::Baseline,
                    true
                ),
                Err(PublicationError::InvalidReplacement)
            );
            let invalid_work = compiler.ledger().work_used(WorkDomain::Baseline) - start;
            unchanged(&mut compiler, source, unit, revision, before);
            let next = literal_edit(
                &mut compiler,
                source,
                unit,
                operation,
                Constant::Integer(12),
                WorkDomain::Baseline,
                true,
            )
            .unwrap();
            let receipt = compiler.view(next).unwrap().receipt();
            assert_eq!(receipt.verified_units, 1);
            assert_eq!(receipt.reused_units, 1);
            assert_eq!(receipt.copied_units, 0);
            assert_eq!(compiler.finish().retained_bytes(), 0);
            (
                invalid_work,
                receipt.logical_work,
                receipt.index.logical_work,
            )
        });
        if let Some(previous) = observed {
            assert_eq!(
                measured, previous,
                "the unused nested callable header is immutable transaction metadata"
            );
        } else {
            observed = Some(measured);
        }
    }
}

fn nested_source(depth: usize) -> String {
    let ty = format!("int{}", "[]".repeat(depth));
    format!("{ty} editTarget({ty}? left,{ty} right){{int changed=11;return left??right;}}")
}
#[derive(Clone, Copy)]
struct QueryMeasurement {
    work: u64,
    optional_peak: u64,
}
fn measure_nested(depth: usize) -> QueryMeasurement {
    checked(&nested_source(depth), |program| {
        let unit = function(&program, "editTarget");
        let operation = integer(&program, unit, 11);
        assert!(program
            .unit(unit)
            .unwrap()
            .operations
            .iter()
            .any(|op| matches!(
                op.kind,
                OperationKind::ShortCircuit {
                    kind: ShortCircuit::Nullish,
                    ..
                }
            )));
        let mut compiler = owner(AMPLE, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let baseline_peak = compiler.ledger().peak_retained_bytes();
        let next = literal_edit(
            &mut compiler,
            source,
            unit,
            operation,
            Constant::Integer(12),
            WorkDomain::Optional,
            true,
        )
        .unwrap();
        let peak = compiler.ledger().peak_retained_bytes();
        assert!(
            peak > baseline_peak,
            "the transaction query workspace must establish this measured peak"
        );
        let result = QueryMeasurement {
            work: compiler.view(next).unwrap().receipt().logical_work,
            optional_peak: peak - retained,
        };
        assert_eq!(compiler.finish().retained_bytes(), 0);
        result
    })
}

#[test]
fn fixed_binary_type_queries_admit_nested_results_and_preserve_refusal_identity() {
    let shallow = measure_nested(1);
    let deep = measure_nested(128);
    assert!(deep.work > shallow.work);
    assert!(
        deep.optional_peak > shallow.optional_peak,
        "the Nullish expected type must retain its cloned array boxes through comparison"
    );
    for (work, memory, expected) in [
        (
            shallow.work,
            MEMORY,
            BudgetError::WorkExhausted(WorkDomain::Optional),
        ),
        (
            AMPLE,
            shallow.optional_peak,
            BudgetError::MemoryExhausted(WorkDomain::Optional),
        ),
    ] {
        checked(&nested_source(128), |program| {
            let unit = function(&program, "editTarget");
            let operation = integer(&program, unit, 11);
            let mut compiler = owner(work, memory);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
            let result = literal_edit(
                &mut compiler,
                source,
                unit,
                operation,
                Constant::Integer(12),
                WorkDomain::Optional,
                true,
            );
            assert_eq!(
                result,
                Err(PublicationError::Budget(expected)),
                "query denial is not InvalidReplacement"
            );
            assert!(compiler.ledger().work_used(WorkDomain::Optional) > 0);
            unchanged(&mut compiler, source, unit, revision, retained);
            assert_eq!(compiler.ledger().retained_bytes_in(WorkDomain::Optional), 0);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn fixed_place_edits_validate_unchanged_dependent_operations_and_roll_back() {
    const TEXT: &str = "int editTarget(){int early=11;int copied=early;int later=12;return copied+later;}print(editTarget());";
    for advance in [false, true] {
        checked(TEXT, |program| {
            let unit = function(&program, "editTarget");
            let cell = CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "early")
                    .unwrap(),
            )
            .unwrap();
            let body = program.unit(unit).unwrap();
            let place = PlaceId::from_index(
                body.places
                    .iter()
                    .position(|place| *place == Place::Cell(cell))
                    .unwrap(),
            )
            .unwrap();
            let early = body.operations[integer(&program, unit, 11).index()]
                .result
                .unwrap();
            let later = body.operations[integer(&program, unit, 12).index()]
                .result
                .unwrap();
            let load = body
                .operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::Load(found) if found == place))
                .unwrap();
            assert!(load < body.values[later.index()].definition.index());
            let mut compiler = owner(AMPLE, MEMORY);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
            let replacement = Place::Value(later);
            let patches = [UnitPatch {
                unit,
                expected_revision: revision,
                operations: &[],
                places: &[PlacePatch {
                    place,
                    replacement: &replacement,
                }],
            }];
            let result = if advance {
                compiler.advance_source(source, &patches, WorkDomain::Baseline)
            } else {
                compiler.edit_source(source, &patches, WorkDomain::Baseline)
            };
            assert_eq!(result, Err(PublicationError::InvalidReplacement));
            unchanged(&mut compiler, source, unit, revision, retained);
            assert_eq!(execute(&mut compiler, source), "23\n");
            // An earlier value is a valid read-only root; the same real edit
            // owner must accept it after the failed future-value replacement.
            let valid = Place::Value(early);
            let next = compiler
                .advance_source(
                    source,
                    &[UnitPatch {
                        unit,
                        expected_revision: revision,
                        operations: &[],
                        places: &[PlacePatch {
                            place,
                            replacement: &valid,
                        }],
                    }],
                    WorkDomain::Baseline,
                )
                .unwrap();
            assert_eq!(execute(&mut compiler, next), "23\n");
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-fixed-publication-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("fixed publication fixture retained: {}", self.0.display());
        } else {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn module_fixture<R>(unrelated: usize, inspect: impl FnOnce(Program<'_>) -> R) -> R {
    let directory = Directory::new();
    std::fs::write(
        directory.0.join("target.lil"),
        "export int editTarget(){return -(11+0);}",
    )
    .unwrap();
    let mut entry = String::from("import {editTarget} from \"./target\";\n");
    for index in 0..unrelated {
        std::fs::write(
            directory.0.join(format!("part{index}.lil")),
            "export int value=1;",
        )
        .unwrap();
        writeln!(
            entry,
            "import {{value as value{index}}} from \"./part{index}\";"
        )
        .unwrap();
    }
    entry.push_str("int total=0;\n");
    for index in 0..unrelated {
        writeln!(entry, "total+=value{index};").unwrap();
    }
    entry.push_str("print(total);print(editTarget());\n");
    std::fs::write(directory.0.join("entry.lil"), entry).unwrap();
    let discovered = crate::module::discover_modules(&directory.0.join("entry.lil")).unwrap();
    assert_eq!(discovered.modules.len(), unrelated + 2);
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &discovered).unwrap();
    let semantics = crate::semantic::analyze_modules(&syntax, &discovered).unwrap();
    let program =
        crate::semantic_program::from_source::from_checked_modules(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program)
}

#[test]
fn consuming_five_operation_edit_work_is_independent_of_unrelated_module_count() {
    let mut measured = None;
    for count in [128, 1024] {
        let current = module_fixture(count, |program| {
            let unit = function(&program, "editTarget");
            let operation = integer(&program, unit, 11);
            assert_eq!(
                program.unit(unit).unwrap().operations.len(),
                5,
                "the target body remains fixed"
            );
            let mut compiler = owner(AMPLE, MEMORY);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let next = literal_edit(
                &mut compiler,
                source,
                unit,
                operation,
                Constant::Integer(12),
                WorkDomain::Baseline,
                true,
            )
            .unwrap();
            let receipt = compiler.view(next).unwrap().receipt();
            assert_eq!(
                (
                    receipt.reused_units,
                    receipt.copied_units,
                    receipt.verified_units,
                    receipt.verified_operations
                ),
                (1, 0, 1, 5)
            );
            assert_eq!(execute(&mut compiler, next), format!("{count}\n-12\n"));
            assert_eq!(compiler.finish().retained_bytes(), 0);
            (
                receipt.logical_work,
                receipt.index.logical_work,
                receipt.allocated_bytes,
            )
        });
        if let Some(previous) = measured {
            assert_eq!(
                current, previous,
                "unrelated module/table headers are not reverified or rebuilt for an exclusive fixed edit"
            );
        } else {
            measured = Some(current);
        }
    }
}

fn captures_source(creations: usize, captures: usize) -> String {
    let parameters = (0..captures)
        .map(|index| format!("int p{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let mut source = format!("int editTarget({parameters}){{");
    for index in 0..creations {
        write!(source, "auto c{index}=()=>0;").unwrap();
    }
    source.push_str("auto wide=()=>");
    source.push_str(
        &(0..captures)
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>()
            .join("+"),
    );
    source.push_str(";return ");
    source.push_str(
        &(0..creations)
            .map(|index| format!("c{index}()"))
            .collect::<Vec<_>>()
            .join("+"),
    );
    source.push_str(";}print(editTarget(");
    source.push_str(
        &std::iter::repeat_n("1", captures)
            .collect::<Vec<_>>()
            .join(","),
    );
    source.push_str("));");
    source
}
fn retargets(
    program: &Program<'_>,
    creations: usize,
    captures: usize,
) -> (UnitId, UnitId, Vec<OpId>) {
    let unit = function(program, "editTarget");
    let mut children = program
        .unit(unit)
        .unwrap()
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            if let OperationKind::Closure(body) = operation.kind {
                Some((OpId::from_index(index).unwrap(), body))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(children.len(), creations + 1);
    let (_, wide) = children.pop().unwrap();
    assert_eq!(program.unit(wide).unwrap().captures.len(), captures);
    assert!(children
        .iter()
        .all(|(_, child)| program.unit(*child).unwrap().captures.is_empty()));
    (
        unit,
        wide,
        children
            .into_iter()
            .map(|(operation, _)| operation)
            .collect(),
    )
}
fn retarget(
    compiler: &mut Compilation<'_>,
    source: SemanticId,
    unit: UnitId,
    wide: UnitId,
    operations: &[OpId],
) -> Result<SemanticId, PublicationError> {
    let kind = OperationKind::Closure(wide);
    let operations = operations
        .iter()
        .map(|&operation| OperationPatch {
            operation,
            kind: &kind,
            operands: &[],
        })
        .collect::<Vec<_>>();
    let revision = compiler.view(source)?.unit_revision(unit).unwrap();
    compiler.advance_source(
        source,
        &[UnitPatch {
            unit,
            expected_revision: revision,
            operations: &operations,
            places: &[],
        }],
        WorkDomain::Baseline,
    )
}

#[test]
fn fixed_closure_retargets_pay_for_each_queried_capture_set_and_roll_back_refusal() {
    let mut work = [[0u64; 2]; 2];
    for (row, creations) in [1, 12].into_iter().enumerate() {
        for (column, captures) in [1, 64].into_iter().enumerate() {
            work[row][column] = checked(&captures_source(creations, captures), |program| {
                let (unit, wide, operations) = retargets(&program, creations, captures);
                let mut compiler = owner(AMPLE, MEMORY);
                let source = compiler
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                let next = retarget(&mut compiler, source, unit, wide, &operations).unwrap();
                let work = compiler.view(next).unwrap().receipt().logical_work;
                // Multiple creations of the same semantic body are valid core
                // edits; the current target cannot materialize them. Qualify
                // this stress case against full verification and a clean index.
                compiler
                    .with_semantic(next, |program, uses, ledger| {
                        program.verify().unwrap();
                        let rebuilt =
                            UseIndex::build(program, ledger, WorkDomain::Baseline).unwrap();
                        for body in program.units.iter() {
                            let id = body.id();
                            assert_eq!(
                                uses.unit(id).unwrap().cell_uses(),
                                rebuilt.unit(id).unwrap().cell_uses()
                            );
                            assert_eq!(
                                uses.unit(id).unwrap().closures(),
                                rebuilt.unit(id).unwrap().closures()
                            );
                            assert_eq!(
                                uses.creators(id).unwrap().sites(),
                                rebuilt.creators(id).unwrap().sites()
                            );
                        }
                        rebuilt.discard(ledger).unwrap();
                    })
                    .unwrap();
                assert_eq!(compiler.finish().retained_bytes(), 0);
                work
            });
        }
    }
    // A one-to-one retarget swaps the two original bodies, retaining a
    // supported executable target shape and observing the newly captured data.
    checked(&captures_source(1, 64), |program| {
        let (unit, wide, operations) = retargets(&program, 1, 64);
        let body = program.unit(unit).unwrap();
        let OperationKind::Closure(small) = body.operations[operations[0].index()].kind else {
            unreachable!()
        };
        let wide_creation = OpId::from_index(
            body.operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::Closure(id) if id == wide))
                .unwrap(),
        )
        .unwrap();
        let mut compiler = owner(AMPLE, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
        let next = compiler
            .advance_source(
                source,
                &[UnitPatch {
                    unit,
                    expected_revision: revision,
                    operations: &[
                        OperationPatch {
                            operation: operations[0],
                            kind: &OperationKind::Closure(wide),
                            operands: &[],
                        },
                        OperationPatch {
                            operation: wide_creation,
                            kind: &OperationKind::Closure(small),
                            operands: &[],
                        },
                    ],
                    places: &[],
                }],
                WorkDomain::Baseline,
            )
            .unwrap();
        assert_eq!(execute(&mut compiler, next), "64\n");
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    // The edited body's own metadata grows additively with these two axes.
    // Retargeting each creation queries its child's complete capture list, so
    // the admitted work must also contain this actual multiplicative term.
    let crossed = i128::from(work[1][1]) - i128::from(work[0][1]) - i128::from(work[1][0])
        + i128::from(work[0][0]);
    assert!(
        crossed >= (12 - 1) * (64 - 1),
        "missing per-creation capture visits: {work:?}"
    );
    checked(&captures_source(12, 64), |program| {
        let (unit, wide, operations) = retargets(&program, 12, 64);
        let mut compiler = owner(AMPLE, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
        let spend = AMPLE - compiler.ledger().work_used(WorkDomain::Baseline) - (work[1][1] - 1);
        compiler
            .ledger
            .charge(WorkDomain::Baseline, WorkKind::Edit, spend)
            .unwrap();
        let result = retarget(&mut compiler, source, unit, wide, &operations);
        assert!(matches!(
            result,
            Err(PublicationError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Baseline
            ))) | Err(PublicationError::Uses(UseError::Budget(
                BudgetError::WorkExhausted(WorkDomain::Baseline)
            )))
        ));
        unchanged(&mut compiler, source, unit, revision, retained);
        assert_eq!(execute(&mut compiler, source), "0\n");
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}
