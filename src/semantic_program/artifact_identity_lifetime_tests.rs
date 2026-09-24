use super::*;

#[test]
fn qualified_winner_keeps_complete_recipe_after_search_and_source_disposal() {
    with_source(BYTE, true, WORK, MEMORY, |compiler, source| {
        let policy = byte_policy("candidate_proposal_limit=3\nterminal_codec_probe_limit=6");
        let mut search = compiler
            .search_javascript(source, &policy, request())
            .unwrap();
        let (candidate, words, fingerprint, pointer, bytes) = search
            .with_winner(Objective::Brotli, |view, _| {
                (
                    view.candidate,
                    view.implementation.recipe_words().to_vec(),
                    view.recipe_fingerprint,
                    view.implementation.recipe_words().as_ptr(),
                    view.javascript.to_owned(),
                )
            })
            .unwrap();
        let receipt = search.take_qualified_winner(Objective::Brotli).unwrap();
        drop(search);
        assert!(compiler
            .with_recipe_descriptor(candidate, WorkDomain::Baseline, |_| ())
            .is_err());
        compiler.discard(source).unwrap();
        compiler
            .with_qualified_artifact(&receipt, |view, _| {
                assert_eq!(view.implementation.recipe_words(), words);
                assert_eq!(view.implementation.recipe_words().as_ptr(), pointer);
                assert_eq!(view.recipe_fingerprint, fingerprint);
                assert_eq!(view.javascript, bytes);
            })
            .unwrap();
        assert_eq!(compiler.take_qualified_artifact(receipt).unwrap(), bytes);
    });
}
