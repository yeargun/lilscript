//! Identity of the running compiler and its exact codec implementation.
use crate::compression;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::sync::OnceLock;

pub(crate) fn compiler_identity() -> Option<[u8; 32]> {
    static IDENTITY: OnceLock<Option<[u8; 32]>> = OnceLock::new();
    *IDENTITY.get_or_init(|| {
        // Linux exposes the running inode even if a new executable has replaced
        // its pathname. Other hosts use the executable path returned by std.
        #[cfg(target_os = "linux")]
        let mut file = File::open("/proc/self/exe").ok()?;
        #[cfg(not(target_os = "linux"))]
        let mut file = File::open(std::env::current_exe().ok()?).ok()?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 16 * 1024];
        loop {
            let read = file.read(&mut buffer).ok()?;
            if read == 0 {
                break;
            }
            hash.update(&buffer[..read]);
        }
        hash.update(crate::compilation_policy::POLICY_ALGORITHM_VERSION.to_le_bytes());
        hash.update(compression::CANONICAL_ZLIB_PACKAGE_VERSION);
        hash.update(compression::CANONICAL_ZLIB_LIBRARY_VERSION);
        hash.update(compression::CANONICAL_BROTLI_PACKAGE_VERSION);
        hash.update(compression::CANONICAL_BROTLI_LIBRARY_VERSION.to_le_bytes());
        hash.update(compression::CANONICAL_BROTLI_ALLOCATION_MODE);
        hash.update([
            std::mem::size_of::<usize>() as u8,
            u8::from(cfg!(target_endian = "little")),
        ]);
        Some(hash.finalize().into())
    })
}
