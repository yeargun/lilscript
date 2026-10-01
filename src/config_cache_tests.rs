use super::*;

#[test]
fn q2_cache_controls_are_strict_relative_and_outside_semantic_identity() {
    use crate::compilation_policy::CompilationRequest;
    let request = CompilationRequest::JavaScript {
        preserve_root_exports: true,
    };
    let mut config = parse_project_config("[cache]\ncodec_reuse=true\ndirectory='build/cache'\n")
        .unwrap()
        .config;
    config.config_dir = Some(std::path::PathBuf::from("/project"));
    let enabled = config.resolve_policy(request).unwrap();
    assert_eq!(
        enabled.cache().directory.as_deref(),
        Some(std::path::Path::new("/project/build/cache"))
    );
    config.cache.build_reuse = false;
    config.cache.codec_reuse = false;
    config.cache.normalization_reuse = false;
    config.cache.formation_reuse = false;
    config.cache.directory = None;
    let disabled = config.resolve_policy(request).unwrap();
    assert_eq!(enabled.fingerprint(), disabled.fingerprint());
    assert_eq!(enabled.receipt(), disabled.receipt());
    assert_ne!(enabled.resolution(), disabled.resolution());
    assert!(parse_project_config("[cache]\nbuild_reuse=\"off\"\n").is_err());
    assert!(parse_project_config("[cache]\ncodec_reuse='off'\n").is_err());
    assert!(parse_project_config("[cache]\nnormalization_reuse='off'\n").is_err());
    assert!(parse_project_config("[cache]\nformation_reuse='off'\n").is_err());
    assert!(parse_project_config("[cache]\nunknown=true\n").is_err());
    config.cache.directory = Some(std::path::PathBuf::new());
    assert!(config
        .resolve_policy(request)
        .unwrap_err()
        .contains("cache.directory"));
}

#[test]
fn q3_worker_controls_are_strict_and_only_search_choices_change_semantic_identity() {
    let request = crate::compilation_policy::CompilationRequest::JavaScript { preserve_root_exports: true };
    let mut config = parse_project_config("[execution]\njobs=4\n[policy.search]\nprotect_effort=false\nobjective_prior='on'\n").unwrap().config;
    let threaded = config.resolve_policy(request).unwrap();
    assert_eq!(threaded.execution().jobs, 4);
    assert!(!threaded.objective().unwrap().search.protect_effort);
    config.execution.jobs = 1;
    let serial = config.resolve_policy(request).unwrap();
    assert_eq!(threaded.fingerprint(), serial.fingerprint());
    assert_ne!(threaded.resolution(), serial.resolution());
    config.policy.as_mut().unwrap().search.protect_effort = true;
    assert_ne!(serial.fingerprint(), config.resolve_policy(request).unwrap().fingerprint());
    for source in ["[execution]\njobs=0", "[execution]\njobs=5", "[execution]\njobs='4'", "[execution]\nunknown=1",
        "[policy.search]\nprotect_effort='off'", "[policy.search]\nobjective_prior='guess'"] {
        assert!(parse_project_config(source).is_err(), "{source}");
    }
}
