use super::*;

#[test]
fn q2_cache_controls_are_strict_relative_and_outside_semantic_identity() {
    use crate::compilation_policy::CompilationRequest;
    let request = CompilationRequest::JavaScript { preserve_root_exports: true };
    let mut config = parse_project_config("[cache]\ncodec_reuse=true\ndirectory='build/cache'\n").unwrap().config;
    config.config_dir = Some(std::path::PathBuf::from("/project"));
    let enabled = config.resolve_policy(request).unwrap();
    assert_eq!(enabled.cache().directory.as_deref(), Some(std::path::Path::new("/project/build/cache")));
    config.cache.build_reuse = false;
    config.cache.codec_reuse = false;
    config.cache.normalization_reuse = false;
    config.cache.directory = None;
    let disabled = config.resolve_policy(request).unwrap();
    assert_eq!(enabled.fingerprint(), disabled.fingerprint());
    assert_eq!(enabled.receipt(), disabled.receipt());
    assert_ne!(enabled.resolution(), disabled.resolution());
    assert!(parse_project_config("[cache]\nbuild_reuse=\"off\"\n").is_err());
    assert!(parse_project_config("[cache]\ncodec_reuse='off'\n").is_err());
    assert!(parse_project_config("[cache]\nnormalization_reuse='off'\n").is_err());
    assert!(parse_project_config("[cache]\nunknown=true\n").is_err());
    config.cache.directory = Some(std::path::PathBuf::new());
    assert!(config.resolve_policy(request).unwrap_err().contains("cache.directory"));
}
