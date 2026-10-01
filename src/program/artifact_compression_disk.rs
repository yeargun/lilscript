//! Optional, bounded disk backing for codec measurements. One fixed slot file
//! bounds disk use across inputs and compiler versions. Independent descriptors
//! and a checksum over the complete record make interrupted/concurrent writes
//! misses; no decision, source proof or artifact eligibility is stored here.
use super::{Key, Measurement};
use crate::compression;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use crate::cache_identity::compiler_identity;

const SLOTS: usize = 4096;
const RECORD: usize = 96;
const FILE_BYTES: u64 = (SLOTS * RECORD) as u64;
const MAGIC: &[u8; 8] = b"LILCOD01";

pub(super) struct Disk {
    path: PathBuf,
    compiler: [u8; 32],
}


impl Disk {
    pub(super) fn new(directory: &Path) -> Option<Self> {
        Some(Self {
            path: directory.join("codec-v1.bin"),
            compiler: compiler_identity()?,
        })
    }

    fn key(&self, key: &Key) -> [u8; 32] {
        use crate::config::CompressionCostModel;
        use compression::{BrotliMode, Role};
        let mut hash = Sha256::new();
        hash.update(MAGIC);
        hash.update(self.compiler);
        hash.update(key.digest);
        hash.update((key.length as u64).to_le_bytes());
        hash.update([
            match key.model {
                CompressionCostModel::Raw => 0,
                CompressionCostModel::Gzip => 1,
                CompressionCostModel::Brotli => 2,
            },
            match key.role {
                Role::Exact => 0,
                Role::Proxy => 1,
            },
            match key.settings.brotli.mode {
                BrotliMode::Generic => 0,
                BrotliMode::Text => 1,
                BrotliMode::Font => 2,
            },
        ]);
        for field in [
            key.settings.brotli.quality,
            key.settings.brotli.window,
            key.settings.gzip.level,
            key.settings.gzip.window,
        ] {
            hash.update(field.to_le_bytes());
        }
        hash.finalize().into()
    }

    fn offset(key: &[u8; 32]) -> u64 {
        (usize::from(u16::from_le_bytes([key[0], key[1]])) % SLOTS * RECORD) as u64
    }

    pub(super) fn read(&self, key: &Key) -> Option<Measurement> {
        // A disk hit must not bypass the cold encoder's runtime identity gate.
        use crate::config::CompressionCostModel;
        match key.model {
            CompressionCostModel::Gzip
                if compression::canonical_zlib_version().ok()?
                    != compression::CANONICAL_ZLIB_LIBRARY_VERSION =>
            {
                return None
            }
            CompressionCostModel::Brotli
                if compression::canonical_brotli_version()
                    != compression::CANONICAL_BROTLI_LIBRARY_VERSION =>
            {
                return None
            }
            _ => {}
        }
        let key = self.key(key);
        let mut file = File::open(&self.path).ok()?;
        if file.metadata().ok()?.len() > FILE_BYTES {
            return None;
        }
        file.seek(SeekFrom::Start(Self::offset(&key))).ok()?;
        let mut record = [0u8; RECORD];
        file.read_exact(&mut record).ok()?;
        if &record[..8] != MAGIC || record[8..40] != key {
            return None;
        }
        let checksum: [u8; 32] = Sha256::digest(&record[..64]).into();
        if record[64..] != checksum {
            return None;
        }
        let fields = std::array::from_fn(|index| {
            u64::from_le_bytes(record[40 + index * 8..48 + index * 8].try_into().unwrap())
        });
        Measurement::from_cache_payload(fields)
    }

    pub(super) fn write(&self, key: &Key, measurement: Measurement) -> std::io::Result<()> {
        let key = self.key(key);
        let mut record = [0u8; RECORD];
        record[..8].copy_from_slice(MAGIC);
        record[8..40].copy_from_slice(&key);
        for (index, field) in measurement.cache_payload().iter().enumerate() {
            record[40 + index * 8..48 + index * 8].copy_from_slice(&field.to_le_bytes());
        }
        let checksum: [u8; 32] = Sha256::digest(&record[..64]).into();
        record[64..].copy_from_slice(&checksum);
        std::fs::create_dir_all(self.path.parent().unwrap())?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.path)?;
        if file.metadata()?.len() > FILE_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "codec cache exceeds its slot file",
            ));
        }
        file.seek(SeekFrom::Start(Self::offset(&key)))?;
        file.write_all(&record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compression::{CodecSettings, Role};
    use crate::config::CompressionCostModel;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn directory() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "lilscript-q2-codec-disk-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    fn key(n: u64) -> Key {
        Key {
            digest: Sha256::digest(n.to_le_bytes()).into(),
            length: n as usize,
            settings: CodecSettings::CANONICAL,
            model: CompressionCostModel::Brotli,
            role: Role::Exact,
        }
    }
    fn measurement() -> Measurement {
        Measurement::from_cache_payload([17, 31, 47]).unwrap()
    }

    #[test]
    fn q2_disk_receipts_bind_complete_keys_and_reject_corruption() {
        let directory = directory();
        let disk = Disk {
            path: directory.join("codec-v1.bin"),
            compiler: [3; 32],
        };
        let original = key(7);
        disk.write(&original, measurement()).unwrap();
        assert_eq!(disk.read(&original).unwrap().cache_payload(), [17, 31, 47]);
        let mut changes = Vec::new();
        let mut changed = original;
        changed.length += 1;
        changes.push(changed);
        let mut changed = original;
        changed.digest[1] ^= 1;
        changes.push(changed);
        let mut changed = original;
        changed.model = CompressionCostModel::Gzip;
        changes.push(changed);
        let mut changed = original;
        changed.role = Role::Proxy;
        changes.push(changed);
        let mut changed = original;
        changed.settings.brotli.quality = 4;
        changes.push(changed);
        let mut changed = original;
        changed.settings.brotli.window = 19;
        changes.push(changed);
        let mut changed = original;
        changed.settings.brotli.mode = compression::BrotliMode::Font;
        changes.push(changed);
        let mut changed = original;
        changed.settings.gzip.level = 3;
        changes.push(changed);
        let mut changed = original;
        changed.settings.gzip.window = 13;
        changes.push(changed);
        for changed in changes {
            assert!(disk.read(&changed).is_none());
        }
        let other = Disk {
            path: disk.path.clone(),
            compiler: [4; 32],
        };
        assert!(other.read(&original).is_none());
        let mut file = OpenOptions::new().write(true).open(&disk.path).unwrap();
        file.seek(SeekFrom::Start(Disk::offset(&disk.key(&original)) + 40))
            .unwrap();
        file.write_all(&999u64.to_le_bytes()).unwrap();
        assert!(
            disk.read(&original).is_none(),
            "corrupt payload is never a score"
        );
        disk.write(&original, measurement()).unwrap();
        file.set_len(Disk::offset(&disk.key(&original)) + 70)
            .unwrap();
        assert!(disk.read(&original).is_none(), "partial writes are misses");
        drop(file);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn q2_disk_collisions_and_concurrent_writers_stay_bounded() {
        let directory = directory();
        let disk = Disk {
            path: directory.join("codec-v1.bin"),
            compiler: [9; 32],
        };
        let first = key(0);
        let offset = Disk::offset(&disk.key(&first));
        let second = (1..100_000)
            .map(key)
            .find(|key| Disk::offset(&disk.key(key)) == offset)
            .unwrap();
        disk.write(&first, measurement()).unwrap();
        disk.write(&second, measurement()).unwrap();
        assert!(disk.read(&first).is_none());
        assert!(disk.read(&second).is_some());
        std::thread::scope(|scope| {
            for i in 0..4 {
                let disk = &disk;
                scope.spawn(move || {
                    for n in 0..96 {
                        let key = key(i * 96 + n);
                        disk.write(&key, measurement()).unwrap();
                        if let Some(found) = disk.read(&key) {
                            assert_eq!(found.cache_payload(), [17, 31, 47]);
                        }
                    }
                });
            }
        });
        assert!(std::fs::metadata(&disk.path).unwrap().len() <= FILE_BYTES);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
