//! The complete-artifact cache can move with its compilation and share derived
//! scores without losing another codec or making a refusal appear measured.
use super::*;
use crate::semantic_program::publication::Compilation;

#[test]
fn completed_artifact_owners_preserve_compilation_send() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<Compilation<'static>>();
    assert_send::<FrozenArtifact>();
    assert_sync::<FrozenArtifact>();
}

#[test]
fn compressed_zero_is_unmeasured_and_refusals_preserve_complete_coordinates() {
    let sizes = CachedSizes::new(0);
    assert_eq!(sizes.measured(CompressionCostModel::Raw), Some(0));
    for codec in [CompressionCostModel::Gzip, CompressionCostModel::Brotli] {
        assert!(sizes.publish(codec, 0).is_err());
        assert_eq!(sizes.measured(codec), None);
        // Even an empty source produces a framed, nonempty canonical stream.
        let actual = crate::compression::measure(b"", codec).unwrap();
        assert!(actual > 0);
        assert_eq!(sizes.publish(codec, actual).unwrap(), actual);
        let completed = sizes.get();
        assert!(sizes.publish(codec, 0).is_err());
        assert!(sizes.publish(codec, actual + 1).is_err());
        assert_eq!(sizes.get(), completed);
        assert_eq!(sizes.publish(codec, actual).unwrap(), actual);
    }
    // Reserving zero rather than usize::MAX leaves every positive exact score
    // representable. The cache imposes no new absolute or relative ceiling.
    let maximum = CachedSizes::new(usize::MAX);
    assert_eq!(
        maximum.measured(CompressionCostModel::Raw),
        Some(usize::MAX)
    );
    maximum
        .publish(CompressionCostModel::Gzip, usize::MAX)
        .unwrap();
    assert_eq!(
        maximum.measured(CompressionCostModel::Gzip),
        Some(usize::MAX)
    );
    assert_eq!(maximum.measured(CompressionCostModel::Brotli), None);
}

#[test]
fn concurrent_codec_publication_keeps_both_exact_scores() {
    let bytes = b"export function value(){return 17;}";
    let gzip = crate::compression::measure(bytes, CompressionCostModel::Gzip).unwrap();
    let brotli = crate::compression::measure(bytes, CompressionCostModel::Brotli).unwrap();
    let sizes = CachedSizes::new(bytes.len());
    let ready = std::sync::Barrier::new(3);
    std::thread::scope(|scope| {
        let gzip_writer = scope.spawn(|| {
            ready.wait();
            sizes.publish(CompressionCostModel::Gzip, gzip).unwrap()
        });
        let brotli_writer = scope.spawn(|| {
            ready.wait();
            sizes.publish(CompressionCostModel::Brotli, brotli).unwrap()
        });
        ready.wait();
        assert_eq!(gzip_writer.join().unwrap(), gzip);
        assert_eq!(brotli_writer.join().unwrap(), brotli);
    });
    assert_eq!(
        sizes.get(),
        Sizes {
            raw: bytes.len(),
            gzip9: Some(gzip),
            brotli11: Some(brotli),
        }
    );
    assert_eq!(
        sizes.publish(CompressionCostModel::Gzip, gzip).unwrap(),
        gzip
    );
    assert_eq!(
        sizes.publish(CompressionCostModel::Brotli, brotli).unwrap(),
        brotli
    );
}
