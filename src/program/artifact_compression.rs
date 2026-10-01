//! Per-compilation, bounded exact-byte measurement reuse. No artifact,
//! provenance, candidate state or admission decision belongs in this table.
use super::{CandidateError, RevisionId};
use crate::compilation_policy::{BudgetError, WorkKind};
use crate::compression::{self, CodecSettings, Measurement, Role};
use crate::config::CompressionCostModel;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError, RetainedCharge};
use sha2::{Digest, Sha256};

// A fixed direct-mapped table bounds both memory and lookup work. Collisions
// evict measurements, never conflate them: the complete key is compared.
const CAPACITY: usize = 256;

#[path = "artifact_compression_disk.rs"]
mod disk;

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub(crate) struct MeasurementStats {
    pub memory_hits: u64,
    pub disk_hits: u64,
    pub encodes: u64,
    pub disk_write_errors: u64,
}


#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    digest: [u8; 32],
    length: usize,
    settings: CodecSettings,
    model: CompressionCostModel,
    role: Role,
}

#[derive(Clone, Copy)]
struct Entry {
    key: Key,
    measurement: Measurement,
}

#[derive(Default)]
pub(super) struct Measurements {
    entries: Vec<Option<Entry>>,
    charge: Option<RetainedCharge<RevisionId>>,
    config: crate::config::CacheConfig,
    disk: Option<disk::Disk>,
    pub(super) stats: MeasurementStats,
    #[cfg(test)]
    pub(super) hits: usize,
}

impl Measurements {
    pub(super) fn configure(&mut self, config: &crate::config::CacheConfig) {
        if self.config == *config { return; }
        // Resolve the executable identity only when a compressed measurement
        // actually misses memory; raw-only requests need no disk-cache work.
        self.disk = None;
        self.config = config.clone();
    }

    fn prepare(
        &mut self,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), CandidateError> {
        if self.entries.is_empty() {
            let mut phase = budget.scope();
            let entries = phase.filled(AllocationClass::Retained, CAPACITY, None)?;
            let bytes = phase.retained_bytes(AllocationClass::Retained);
            let charge = phase.detach_retained(owner, bytes)?;
            self.entries = entries;
            self.charge = Some(charge);
        }
        Ok(())
    }

    pub(super) fn measure(
        &mut self,
        owner: RevisionId,
        bytes: &[u8],
        model: CompressionCostModel,
        settings: &CodecSettings,
        role: Role,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, CandidateError> {
        if model == CompressionCostModel::Raw {
            return Ok(bytes.len());
        }
        // Lack of room for an optional table must not refuse a service that
        // could still fit without it. Work/deadline refusals remain refusals.
        if let Err(error) = self.prepare(owner, budget) {
            return match error {
                CandidateError::Budget(BudgetError::MemoryExhausted(_))
                | CandidateError::AllocationFailed => {
                    self.stats.encodes += 1;
                    Ok(compression::measure_admitted_at(bytes, model, settings, role, budget)?.size)
                },
                error => Err(error),
            };
        }
        let (key, slot, previous) = {
            let _timing = crate::timing::CODEC_LOOKUP.scope(bytes.len());
            budget.work(
                WorkKind::Analysis,
                u64::try_from(bytes.len())
                    .map_err(|_| AllocationError::Capacity)?
                    .saturating_add(1),
            )?;
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            let key = Key {
                digest,
                length: bytes.len(),
                settings: *settings,
                model,
                role,
            };
            // Partition the low hash bits by role/model so an exact encode
            // does not immediately evict the same stream's proxy.
            let salt = match (model, role) {
                (CompressionCostModel::Gzip, _) => 0x55,
                (CompressionCostModel::Brotli, Role::Exact) => 0xaa,
                _ => 0,
            };
            let slot = usize::from(digest[0] ^ salt);
            (
                key,
                slot,
                self.entries[slot].filter(|entry| entry.key == key),
            )
        };
        if let Some(entry) = previous.filter(|_| self.config.codec_reuse && reuse_enabled()) {
            let _timing = crate::timing::CODEC_REUSE.scope(bytes.len());
            let size = entry.measurement.replay(budget)?;
            self.stats.memory_hits += 1;
            #[cfg(test)]
            {
                self.hits += 1;
            }
            return Ok(size);
        }
        if self.config.codec_reuse && reuse_enabled() {
            if self.disk.is_none() {
                self.disk = self.config.directory.as_deref().and_then(disk::Disk::new);
            }
            if let Some(measurement) = self.disk.as_ref().and_then(|disk| disk.read(&key)) {
                let _timing = crate::timing::CODEC_REUSE.scope(bytes.len());
                let size = measurement.replay(budget)?;
                self.entries[slot] = Some(Entry { key, measurement });
                self.stats.disk_hits += 1;
                #[cfg(test)] { self.hits += 1; }
                return Ok(size);
            }
        }
        self.stats.encodes += 1;
        let measurement = compression::measure_admitted_at(bytes, model, settings, role, budget)?;
        if self.config.codec_reuse && reuse_enabled() {
            if self.disk.as_ref().is_some_and(|disk| disk.write(&key, measurement).is_err()) {
                self.stats.disk_write_errors += 1;
            }
        }

        // The memory table belongs to this compilation; disk backing adds the
        // compiler/backend identities and integrity checks of its own owner.
        self.entries[slot] = Some(Entry { key, measurement });
        Ok(measurement.size)
    }

    pub(super) fn clear(&mut self, owner: RevisionId, budget: &mut AllocationBudget<'_>) {
        drop(std::mem::take(&mut self.entries));
        if let Some(charge) = self.charge.take() {
            super::release(charge, owner, budget);
        }
    }
}

#[cfg(not(test))]
fn reuse_enabled() -> bool {
    true
}

#[cfg(test)]
thread_local! {
    static REUSE: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

#[cfg(test)]
fn reuse_enabled() -> bool {
    REUSE.get()
}

/// Test the identical logical schedule with physical encoding on every miss
/// and hit. Table storage, keys and work are unchanged by this audit switch.
#[cfg(test)]
pub(in crate::program) fn without_reuse<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            REUSE.set(self.0);
        }
    }
    let _restore = Restore(REUSE.replace(false));
    run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

    #[test]
    fn memo_keeps_bytes_codecs_settings_and_roles_independent_and_storage_bounded() {
        let owner = RevisionId::fresh();
        let mut ledger = BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 200_000_000,
                optional_work: 0,
                baseline_retained_bytes: 0,
                retained_bytes: 100_000_000,
            },
        )
        .unwrap();
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut memo = Measurements::default();
        let first = b"const a='repeated';export{a};";
        let second = b"const a='different';export{a};";
        for bytes in [first.as_slice(), second.as_slice()] {
            for model in [CompressionCostModel::Gzip, CompressionCostModel::Brotli] {
                for role in [Role::Exact, Role::Proxy] {
                    for mut settings in [
                        CodecSettings::CANONICAL,
                        CodecSettings {
                            brotli: compression::BrotliSettings {
                                quality: 4,
                                window: 18,
                                mode: compression::BrotliMode::Text,
                            },
                            gzip: compression::GzipSettings {
                                level: 2,
                                window: 12,
                            },
                        },
                    ] {
                        let expected = compression::measure_admitted_at(
                            bytes,
                            model,
                            &settings,
                            role,
                            &mut budget,
                        )
                        .unwrap()
                        .size;
                        assert_eq!(
                            memo.measure(owner, bytes, model, &settings, role, &mut budget)
                                .unwrap(),
                            expected
                        );
                        let before = memo.hits;
                        assert_eq!(
                            memo.measure(owner, bytes, model, &settings, role, &mut budget)
                                .unwrap(),
                            expected
                        );
                        assert_eq!(memo.hits, before + 1);
                        settings.gzip.level = 5;
                        let expected = compression::measure_admitted_at(
                            bytes,
                            model,
                            &settings,
                            role,
                            &mut budget,
                        )
                        .unwrap()
                        .size;
                        assert_eq!(
                            memo.measure(owner, bytes, model, &settings, role, &mut budget)
                                .unwrap(),
                            expected
                        );
                    }
                }
            }
        }
        let retained = budget.with_ledger(|ledger| ledger.unwrap().0.retained_bytes());
        // More distinct streams than slots must evict some prior scores,
        // while fresh encoding and replay remain exact after replacement.
        for value in 0..CAPACITY * 2 {
            let bytes = value.to_string();
            let expected =
                compression::measure(bytes.as_bytes(), CompressionCostModel::Gzip).unwrap();
            assert_eq!(
                memo.measure(
                    owner,
                    bytes.as_bytes(),
                    CompressionCostModel::Gzip,
                    &CodecSettings::CANONICAL,
                    Role::Exact,
                    &mut budget
                )
                .unwrap(),
                expected
            );
        }
        assert_eq!(
            budget.with_ledger(|ledger| ledger.unwrap().0.retained_bytes()),
            retained
        );
        assert_eq!(memo.entries.len(), CAPACITY);
        memo.clear(owner, &mut budget);
        assert_eq!(
            budget.with_ledger(|ledger| ledger.unwrap().0.retained_bytes()),
            0
        );
    }
}

#[cfg(test)]
mod persistent_tests {
    use super::*;
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
    use crate::config::CacheConfig;

    fn measure(config: &CacheConfig, model: CompressionCostModel, work: u64, memory: u64)
        -> (Result<usize, CandidateError>, MeasurementStats, u64)
    {
        let owner = RevisionId::fresh();
        let mut ledger = BudgetLedger::new(ResourceLimits::default(), BudgetPlan {
            baseline_work: work, optional_work: 0, baseline_retained_bytes: 0, retained_bytes: memory,
        }).unwrap();
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut memo = Measurements::default();
        memo.configure(config);
        let result = memo.measure(owner, b"export function one(a){return a+1}export function two(a){return a+2}", model,
            &CodecSettings::CANONICAL, Role::Exact, &mut budget);
        let stats = memo.stats;
        memo.clear(owner, &mut budget);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), 0);
        (result, stats, ledger.work_used(WorkDomain::Baseline))
    }

    #[test]
    fn q2_disk_hits_replay_cold_work_and_refuse_cold_resource_limits() {
        let directory = std::env::temp_dir().join(format!("lilscript-q2-codec-replay-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let on = CacheConfig { normalization_reuse: true, codec_reuse: true, directory: Some(directory.clone()) };
        let off = CacheConfig { normalization_reuse: true, codec_reuse: false, ..on.clone() };
        for model in [CompressionCostModel::Gzip, CompressionCostModel::Brotli] {
            let cold = measure(&on, model, 200_000_000, 100_000_000);
            let warm = measure(&on, model, 200_000_000, 100_000_000);
            let disabled = measure(&off, model, 200_000_000, 100_000_000);
            assert_eq!(cold.0.as_ref().unwrap(), warm.0.as_ref().unwrap());
            assert_eq!(cold.0.as_ref().unwrap(), disabled.0.as_ref().unwrap());
            assert_eq!((cold.2, cold.2), (warm.2, disabled.2));
            assert_eq!(cold.1.encodes, 1);
            assert_eq!(warm.1.encodes, 0);
            assert_eq!(warm.1.disk_hits, 1);
            assert_eq!(disabled.1.encodes, 1);
            assert_eq!(disabled.1.disk_hits, 0);
            for config in [&on, &off] {
                assert!(measure(config, model, cold.2 - 1, 100_000_000).0.is_err());
                assert!(measure(config, model, 200_000_000, 128).0.is_err());
            }
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn q2_unavailable_disk_storage_falls_back_to_encoding() {
        let path = std::env::temp_dir().join(format!("lilscript-q2-codec-file-{}", std::process::id()));
        std::fs::write(&path, b"not a directory").unwrap();
        let config = CacheConfig { normalization_reuse: true, codec_reuse: true, directory: Some(path.clone()) };
        let result = measure(&config, CompressionCostModel::Gzip, 200_000_000, 100_000_000);
        assert!(result.0.is_ok());
        assert_eq!(result.1.disk_write_errors, 1);
        assert_eq!(result.1.encodes, 1);
        std::fs::remove_file(path).unwrap();
    }
}
