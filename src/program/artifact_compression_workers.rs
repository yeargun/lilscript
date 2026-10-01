//! Bounded physical scoring of independent delivered files. Fixed four-file
//! batches have the same admission and ordered receipt consumption at jobs 1-4.
//! A worker owns a finite private ledger; it cannot spend the shared ledger or
//! publish an artifact. Incomplete receipts fall back to the ordinary scorer.
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

const CODEC_MEMORY: u64 = 8 * 1024 * 1024;
const STACK: usize = 512 * 1024;
const SLOT_MEMORY: u64 = CODEC_MEMORY + STACK as u64 + 4096;

impl Measurements {
    pub(in crate::program) fn prepare_batch(
        &mut self,
        inputs: &[Option<&[u8]>; 4],
        model: CompressionCostModel,
        settings: &CodecSettings,
        role: Role,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<[Option<Measurement>; 4], CandidateError> {
        let mut results = [None; 4];
        let count = inputs.iter().flatten().count();
        if model == CompressionCostModel::Raw || count < 2 {
            return Ok(results);
        }
        // Speculation never evades a wall deadline. Deadline requests use the
        // ordinary cooperative codec path, independently of requested jobs.
        let memory = SLOT_MEMORY * count as u64;
        let allowance = budget.with_ledger(|ledger| match ledger {
            Some((ledger, domain))
                if ledger.wall_time_ms().is_none()
                    && ledger.clone().retain(domain, memory).is_ok() =>
            {
                Some(
                    ledger
                        .work_limit(domain)
                        .saturating_sub(ledger.work_used(domain))
                        / count as u64,
                )
            }
            None => Some(200_000_000 / count as u64),
            _ => None,
        });
        let Some(allowance) = allowance.filter(|allowance| *allowance != 0) else {
            return Ok(results);
        };
        let mut phase = budget.scope();
        // Also reserve absent worker stacks at jobs=1: a physical scheduling
        // knob must not buy extra logical memory or a different exploration.
        phase.retain(AllocationClass::Scratch, memory)?;
        self.stats.worker_batches += 1;
        let mut pending = [false; 4];
        for (index, input) in inputs.iter().enumerate() {
            let Some(bytes) = input else {
                continue;
            };
            let key = Key {
                digest: Sha256::digest(bytes).into(),
                length: bytes.len(),
                settings: *settings,
                model,
                role,
            };
            let hit = self.config.codec_reuse
                && reuse_enabled()
                && (self.entries.iter().flatten().any(|entry| entry.key == key)
                    || self
                        .disk
                        .as_ref()
                        .is_some_and(|disk| disk.read(&key).is_some()));
            pending[index] = !hit;
        }
        let run = |index: usize| {
            let mut ledger = BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    baseline_work: allowance,
                    optional_work: 0,
                    baseline_retained_bytes: 0,
                    retained_bytes: CODEC_MEMORY,
                },
            )
            .expect("finite private codec admission");
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            compression::measure_admitted_at(
                inputs[index].unwrap(),
                model,
                settings,
                role,
                &mut budget,
            )
        };
        let indices: Vec<_> = (0..4).filter(|&index| pending[index]).collect();
        let jobs = self.jobs.clamp(1, 4);
        for wave in indices.chunks(jobs) {
            std::thread::scope(|scope| {
                let mut handles = [None, None, None, None];
                // The calling thread is one worker, never an extra fifth one.
                for &index in &wave[1..] {
                    handles[index] = std::thread::Builder::new()
                        .stack_size(STACK)
                        .spawn_scoped(scope, move || run(index))
                        .ok();
                }
                self.stats.peak_workers = self
                    .stats
                    .peak_workers
                    .max(1 + handles.iter().flatten().count());
                for &index in wave {
                    let result = match handles[index].take() {
                        Some(handle) => handle
                            .join()
                            .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
                        None => run(index),
                    };
                    self.stats.worker_encodes += 1;
                    if let Ok(measurement) = result {
                        results[index] = Some(measurement);
                    } else {
                        self.stats.worker_fallbacks += 1;
                    }
                }
            });
        }
        // All threads, encoders and their buffers are gone before reservation
        // release. Only four fixed-size receipts escape. Their normal ordered
        // scorer checks cold scratch/work again before accepting or caching.
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn q3_workers_preserve_scores_cold_accounting_and_refusal_with_cache_controls() {
        let inputs = [
            Some(b"export const a='repeated repeated repeated';".as_slice()),
            Some(b"export function a(x){return x*x+1}".as_slice()),
            Some(b"export const a=[1,2,3,4,5,6,7,8];".as_slice()),
            None,
        ];
        for model in [CompressionCostModel::Gzip, CompressionCostModel::Brotli] {
            for (work, memory) in [
                (10_000_000, 100_000_000),
                (10_000_000, 600_000),
                (17, 100_000_000),
            ] {
                let mut expected = None;
                for reuse in [true, false] {
                    for jobs in [1, 4] {
                        let owner = RevisionId::fresh();
                        let mut ledger = BudgetLedger::new(
                            ResourceLimits::default(),
                            BudgetPlan {
                                baseline_work: work,
                                optional_work: 0,
                                baseline_retained_bytes: 0,
                                retained_bytes: memory,
                            },
                        )
                        .unwrap();
                        let mut cache = Measurements::default();
                        cache.jobs = jobs;
                        cache.config.codec_reuse = reuse;
                        let mut results = Vec::new();
                        {
                            let mut budget =
                                AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
                            for _ in 0..2 {
                                let prepared = cache
                                    .prepare_batch(
                                        &inputs,
                                        model,
                                        &CodecSettings::CANONICAL,
                                        Role::Exact,
                                        &mut budget,
                                    )
                                    .unwrap();
                                for (index, input) in inputs.iter().enumerate() {
                                    if let Some(input) = input {
                                        results.push(
                                            cache
                                                .measure_prepared(
                                                    owner,
                                                    input,
                                                    model,
                                                    &CodecSettings::CANONICAL,
                                                    Role::Exact,
                                                    prepared[index],
                                                    &mut budget,
                                                )
                                                .map_err(|error| format!("{error:?}")),
                                        );
                                    }
                                }
                            }
                            cache.clear(owner, &mut budget);
                        }
                        if memory == 100_000_000 && work == 10_000_000 {
                            assert_eq!(cache.stats.peak_workers, jobs.min(3));
                            for (result, input) in
                                results.iter().zip(inputs.iter().flatten().cycle())
                            {
                                assert_eq!(
                                    result.as_ref().unwrap(),
                                    &compression::measure(input, model).unwrap()
                                );
                            }
                        }
                        assert_eq!(ledger.retained_bytes(), 0);
                        let receipt = (
                            results,
                            ledger.work_used(WorkDomain::Baseline),
                            ledger.work_by_kind(WorkKind::Analysis),
                            ledger.work_by_kind(WorkKind::Codec),
                        );
                        if let Some(expected) = &expected {
                            assert_eq!(
                                &receipt, expected,
                                "{model:?} {work}/{memory} jobs={jobs} reuse={reuse}"
                            );
                        } else {
                            expected = Some(receipt);
                        }
                    }
                }
            }
        }
    }
}
