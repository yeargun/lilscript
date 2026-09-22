//! Cached scores do not spend another probe or replace provenance admission.
use super::*;
use crate::compilation_policy::{BaselineFirstPlan, CompilationRequest, ResourceLimits};

fn policy(proposals: usize, naming: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncandidate_proposal_limit={proposals}\nterminal_codec_probe_limit=2\ncandidate_limit=4\n[policy.search]\ncodec_schedule='staged'\n[policy.tactics]\nidentifier-mangling='on'\nnaming-search='{naming}'\n"
    ))
    .unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn request() -> SearchRequest {
    let local_facts = LocalFactsRequest {
        work_quota: 1_000,
        result_bytes: 1_000,
    };
    SearchRequest {
        objectives: Objectives::All,
        scalar: ScalarRequest {
            max_work: 1_000,
            scratch_bytes: 1_000,
            output_bytes: 1_000,
        },
        helper: HelperRequest {
            max_work: 1_000,
            scratch_bytes: 1_000,
            output_bytes: 1_000,
            local_facts,
        },
        string: StringRequest {
            max_work: 1_000,
            scratch_bytes: 1_000,
            output_bytes: 1_000,
            local_facts,
        },
        facts_cache: CacheLimits {
            entries: 1,
            bytes: 10_000,
            result_bytes: 1_000,
        },
    }
}

#[test]
fn staged_equal_bytes_reuse_exhausted_probes_but_recheck_naming_permission() {
    for permitted in [true, false] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, "export int answer(){return 17;}").unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let program = crate::semantic_program::from_checked_source(&syntax, &checked).unwrap();
        let ledger = BudgetLedger::new_baseline_first(
            ResourceLimits::default(),
            BaselineFirstPlan {
                logical_work: 100_000_000,
                retained_bytes: 128_000_000,
                terminal_work: 0,
            },
        )
        .unwrap();
        let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 4 }).unwrap();
        let source = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let baseline = policy(0, "on");
        let optional = policy(2, "on");
        let mut search = compilation
            .search_javascript(source, &baseline, request())
            .unwrap();
        let original = search
            .with_winner(Objective::Brotli, |view, _| view.sizes)
            .unwrap();
        let mut observed = Vec::new();
        search
            .evaluate_structure(
                0,
                &optional,
                Objectives::All,
                &[Style::Scoped],
                &mut |entry| observed.push(entry.naming.style),
            )
            .unwrap();
        assert!(observed.is_empty());
        assert_eq!(search.counters.queued_artifacts, 1);
        // The queued trial already exists when other work consumes the allowance.
        search.counters.codec_probes = optional.objective().unwrap().optional_codec_probes;
        let scoring = policy(2, if permitted { "on" } else { "off" });
        let before = search.ledger().work_by_kind(WorkKind::Codec);
        let result = search.score_next(&scoring, Objectives::All, &mut |entry| {
            observed.push(entry.naming.style);
            assert_eq!(entry.sizes, original);
        });
        if permitted {
            assert!(matches!(result, Ok(true)));
            assert_eq!(observed, [Style::Scoped]);
            assert_eq!(search.counters.admitted_artifacts, 2);
        } else {
            assert!(matches!(
                result,
                Err(SearchError::Admission(AdmissionError::ForbiddenTactic(_)))
                    | Err(SearchError::Candidate(CandidateError::Output(_)))
            ));
            assert!(observed.is_empty());
            assert_eq!(search.counters.admitted_artifacts, 1);
        }
        assert_eq!(search.counters.codec_probes, 2);
        // Record cache lookups remain charged, but no encoder runs.
        assert!(search.ledger().work_by_kind(WorkKind::Codec) - before <= 3);
        assert_eq!(
            search.with_winner(Objective::Brotli, |view, _| view.sizes),
            Some(original)
        );
        drop(search);
        assert_eq!(compilation.finish().retained_bytes(), 0);
    }
}
