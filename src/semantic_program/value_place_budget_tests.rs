//! Publication must admit unused place verification even when a body has no ops.
use super::publication::{
    CheckpointLimit, Compilation, PlacePatch, PublicationError, SemanticId, UnitPatch,
};
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};

const SOURCE: &str = "struct Point{int x;int y;}void empty(Point left,Point right){}";
const AMPLE: u64 = 100_000_000;
const LARGE: usize = 8_192;

#[derive(Clone, Copy)]
struct Fixture {
    unit: UnitId,
    last: PlaceId,
    field: NominalMemberId,
}
impl Fixture {
    fn original(self) -> Place {
        Place::Field {
            base: PlaceId::from_index(0).unwrap(),
            field: self.field,
        }
    }
    fn redirected(self) -> Place {
        Place::Field {
            base: PlaceId::from_index(1).unwrap(),
            field: self.field,
        }
    }
    fn invalid(self) -> Place {
        Place::Field {
            base: self.last,
            field: self.field,
        }
    }
}

fn fixture(count: usize, inspect: impl FnOnce(Program<'_>, Fixture)) {
    assert!(count >= 3);
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let mut program = from_checked_source(&syntax, &semantics).unwrap();
    let CellBinding::Function(unit) = program
        .cells
        .iter()
        .find(|cell| cell.name == "empty")
        .unwrap()
        .binding
    else {
        panic!("fixture has a declared function");
    };
    let field = program
        .fields
        .iter()
        .find(|field| field.name == "x")
        .unwrap()
        .identity;
    let info = Fixture {
        unit,
        last: PlaceId::from_index(count - 1).unwrap(),
        field,
    };
    let mut working = program.units[unit.index()].clone().into_working();
    let body = working.get_mut();
    assert!(body.operations.is_empty(), "do not mask the zero-op case");
    assert!(body.values.is_empty());
    assert_eq!(body.parameters.len(), 2);
    // Fixture construction precedes admission, just as the checked frontend
    // does. The tested mutation below goes through the real publication owner.
    body.places = Vec::with_capacity(count);
    body.places.push(Place::Cell(body.parameters[0]));
    body.places.push(Place::Cell(body.parameters[1]));
    body.places.resize(count, info.original());
    program.units[unit.index()] = working.freeze();
    program.verify().unwrap();
    inspect(program, info);
}

fn compiler<'src>(optional: u64, memory: u64) -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: AMPLE,
                optional_work: optional,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 3 },
    )
    .unwrap()
}

fn edit(
    compiler: &mut Compilation<'_>,
    base: SemanticId,
    fixture: Fixture,
    replacement: &Place,
    domain: WorkDomain,
) -> Result<SemanticId, PublicationError> {
    let expected_revision = compiler
        .view(base)
        .unwrap()
        .unit_revision(fixture.unit)
        .unwrap();
    compiler.edit_source(
        base,
        &[UnitPatch {
            unit: fixture.unit,
            expected_revision,
            operations: &[],
            places: &[PlacePatch {
                place: fixture.last,
                replacement,
            }],
        }],
        domain,
    )
}

fn assert_source_unchanged(
    compiler: &Compilation<'_>,
    base: SemanticId,
    fixture: Fixture,
    revision: RevisionId,
    retained: u64,
) {
    assert_eq!(compiler.checkpoint_count(), 1);
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    assert_eq!(compiler.ledger().retained_bytes_in(WorkDomain::Optional), 0);
    let view = compiler.view(base).unwrap();
    assert_eq!(view.unit_revision(fixture.unit), Some(revision));
    let body = view.unit(fixture.unit).unwrap();
    assert!(body.operations.is_empty());
    assert_eq!(body.places[fixture.last.index()], fixture.original());
}

#[derive(Clone, Copy)]
struct Measurement {
    invalid_peak: u64,
    successful_peak: u64,
    invalid_work: u64,
    before_verification_work: u64,
}

/// A malformed last place isolates verifier admission from later index work:
/// with room it reaches InvalidReplacement, without room it must not get there.
fn measure_verification(count: usize) -> Measurement {
    let mut measured = None;
    fixture(count, |program, info| {
        let mut compiler = compiler(AMPLE, AMPLE);
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let baseline_peak = compiler.ledger().peak_retained_bytes();
        let retained = compiler.ledger().retained_bytes();
        let revision = compiler
            .view(base)
            .unwrap()
            .unit_revision(info.unit)
            .unwrap();
        assert_eq!(
            edit(
                &mut compiler,
                base,
                info,
                &info.invalid(),
                WorkDomain::Optional
            ),
            Err(PublicationError::InvalidReplacement)
        );
        assert_source_unchanged(&compiler, base, info, revision, retained);
        let peak = compiler.ledger().peak_retained_bytes();
        assert!(
            peak > baseline_peak,
            "this probe must reach the verifier workspace"
        );
        measured = Some((peak, compiler.ledger().work_used(WorkDomain::Optional)));
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    let (peak, invalid_work) = measured.unwrap();
    let mut successful_peak = None;
    fixture(count, |program, info| {
        let mut compiler = compiler(AMPLE, AMPLE);
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let next = edit(
            &mut compiler,
            base,
            info,
            &info.redirected(),
            WorkDomain::Optional,
        )
        .unwrap();
        assert_eq!(
            compiler.view(next).unwrap().receipt().verified_operations,
            0
        );
        // A valid place table continues into unit workspace and index checks;
        // an invalid last place deliberately never reaches those allocations.
        successful_peak = Some(compiler.ledger().peak_retained_bytes());
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    let mut before_verification_work = None;
    for invalid in [false, true] {
        fixture(count, |program, info| {
            let mut compiler = compiler(AMPLE, peak - 1);
            let base = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            let revision = compiler
                .view(base)
                .unwrap()
                .unit_revision(info.unit)
                .unwrap();
            let replacement = if invalid {
                info.invalid()
            } else {
                info.redirected()
            };
            assert_eq!(
                edit(
                    &mut compiler,
                    base,
                    info,
                    &replacement,
                    WorkDomain::Optional
                ),
                Err(PublicationError::Budget(BudgetError::MemoryExhausted(
                    WorkDomain::Optional
                )))
            );
            assert_source_unchanged(&compiler, base, info, revision, retained);
            assert!(compiler.ledger().peak_retained_bytes() < peak);
            let work = compiler.ledger().work_used(WorkDomain::Optional);
            assert!(
                work < invalid_work,
                "verification must be charged after workspace admission"
            );
            if let Some(previous) = before_verification_work {
                assert_eq!(
                    work, previous,
                    "a denied workspace must not inspect the invalid place"
                );
            }
            before_verification_work = Some(work);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
    Measurement {
        invalid_peak: peak,
        successful_peak: successful_peak.unwrap(),
        invalid_work,
        before_verification_work: before_verification_work.unwrap(),
    }
}

#[test]
fn zero_operation_place_table_is_admitted_before_validation_and_charged_linearly() {
    let tiny = measure_verification(LARGE / 4);
    let small = measure_verification(LARGE / 2);
    let large = measure_verification(LARGE);
    // Both copies/index setup and table allocation have already been accounted
    // for by their existing owners. This difference isolates verifier work,
    // without duplicating the verifier's allocation formulas.
    let tiny_verification = tiny.invalid_work - tiny.before_verification_work;
    let small_verification = small.invalid_work - small.before_verification_work;
    let large_verification = large.invalid_work - large.before_verification_work;
    assert!(small_verification >= (LARGE / 2) as u64);
    assert!(small_verification > tiny_verification);
    // Field lookup now pays at each actual query. Check linear growth without
    // pinning this test to the verifier's current per-field tariff.
    assert_eq!(
        large_verification - small_verification,
        2 * (small_verification - tiny_verification)
    );
    assert!(large.invalid_peak > small.invalid_peak);
}

#[test]
fn large_place_only_edit_succeeds_at_the_measured_workspace_bound_and_releases_both_domains() {
    let measured = measure_verification(LARGE);
    fixture(LARGE, |program, info| {
        let mut compiler = compiler(AMPLE, measured.successful_peak - 1);
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let original = compiler
            .view(base)
            .unwrap()
            .unit_revision(info.unit)
            .unwrap();
        assert_eq!(
            edit(
                &mut compiler,
                base,
                info,
                &info.redirected(),
                WorkDomain::Optional
            ),
            Err(PublicationError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Optional
            )))
        );
        assert_source_unchanged(&compiler, base, info, original, retained);
        assert!(compiler.ledger().peak_retained_bytes() < measured.successful_peak);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    fixture(LARGE, |program, info| {
        let mut compiler = compiler(AMPLE, measured.successful_peak);
        let empty = compiler.ledger().retained_bytes();
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let original = compiler
            .view(base)
            .unwrap()
            .unit_revision(info.unit)
            .unwrap();
        let next = edit(
            &mut compiler,
            base,
            info,
            &info.redirected(),
            WorkDomain::Optional,
        )
        .unwrap();
        let view = compiler.view(next).unwrap();
        assert_eq!(view.receipt().verified_units, 1);
        assert_eq!(view.receipt().verified_operations, 0);
        assert_eq!(view.receipt().index.rebuilt_units, 1);
        assert!(
            view.cell_changes().is_empty(),
            "unused places do not invent cell uses"
        );
        assert_ne!(view.unit_revision(info.unit), Some(original));
        assert_eq!(
            view.unit(info.unit).unwrap().places[info.last.index()],
            info.redirected()
        );
        assert!(compiler.ledger().retained_bytes_in(WorkDomain::Optional) > 0);
        assert_eq!(
            compiler.ledger().peak_retained_bytes(),
            measured.successful_peak
        );
        compiler.discard(next).unwrap();
        assert_source_unchanged(&compiler, base, info, original, retained);
        compiler.discard(base).unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), empty);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn refused_verifier_work_releases_its_workspace_and_leaves_baseline_publication_usable() {
    let measured = measure_verification(LARGE);
    fixture(LARGE, |program, info| {
        let mut compiler = compiler(measured.invalid_work - 1, measured.successful_peak);
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let revision = compiler
            .view(base)
            .unwrap()
            .unit_revision(info.unit)
            .unwrap();
        assert_eq!(
            edit(
                &mut compiler,
                base,
                info,
                &info.invalid(),
                WorkDomain::Optional
            ),
            Err(PublicationError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Optional
            )))
        );
        let paid = compiler.ledger().work_used(WorkDomain::Optional);
        assert!(paid >= measured.before_verification_work);
        assert!(paid < measured.invalid_work);
        // Admission can stop at a field query after earlier valid places were
        // inspected; all that work remains paid while publication rolls back.
        assert_eq!(
            compiler.ledger().peak_retained_bytes(),
            measured.invalid_peak
        );
        assert_source_unchanged(&compiler, base, info, revision, retained);
        // The baseline domain still owns usable headroom and the same source;
        // refusal cannot leave an orphan checkpoint or poison the transaction.
        let next = edit(
            &mut compiler,
            base,
            info,
            &info.redirected(),
            WorkDomain::Baseline,
        )
        .unwrap();
        assert_eq!(
            compiler.view(next).unwrap().receipt().verified_operations,
            0
        );
        assert_eq!(compiler.ledger().retained_bytes_in(WorkDomain::Optional), 0);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}
