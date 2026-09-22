use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};
use std::io::Write;

fn ledger(memory: u64, work: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: work,
            optional_work: work,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}
fn old_gzip(bytes: &[u8]) -> usize {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap().len()
}
fn original_brotli(bytes: &[u8]) -> usize {
    let capacity = unsafe { compu_brotli_sys::BrotliEncoderMaxCompressedSize(bytes.len()) };
    let mut result = vec![0u8; capacity.max(1)];
    let mut size = result.len();
    assert_ne!(
        unsafe {
            compu_brotli_sys::BrotliEncoderCompress(
                11,
                22,
                compu_brotli_sys::BrotliEncoderMode_BROTLI_MODE_GENERIC,
                bytes.len(),
                bytes.as_ptr(),
                &mut size,
                result.as_mut_ptr(),
            )
        },
        0
    );
    size
}
fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x1234_5678u32;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

#[test]
fn admitted_codecs_match_pinned_convenience_scores_without_retaining_encoded_output() {
    let mut inputs = vec![
        Vec::new(),
        b"x".to_vec(),
        b"export{a as value};".to_vec(),
        "const emoji='🦀';".as_bytes().to_vec(),
        b"abcabcabcabc".repeat(10_000),
    ];
    for length in [2, 17, 1024, 16_383, 16_384, 16_385, 65_537] {
        inputs.push(noise(length));
    }
    for input in inputs {
        let expected = [input.len(), old_gzip(&input), original_brotli(&input)];
        for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
            let mut ledger = ledger(100_000_000, 100_000_000);
            ledger.retain(WorkDomain::Baseline, 1024).unwrap();
            ledger.retain(WorkDomain::Optional, 2048).unwrap();
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
                for (model, expected) in [
                    CompressionCostModel::Raw,
                    CompressionCostModel::Gzip,
                    CompressionCostModel::Brotli,
                ]
                .into_iter()
                .zip(expected)
                {
                    assert_eq!(
                        measure_admitted(&input, model, &mut budget).unwrap(),
                        expected
                    );
                }
            }
            assert_eq!(ledger.retained_bytes(), 3072);
            assert!(ledger.peak_retained_bytes() > 3072);
            let other = if domain == WorkDomain::Baseline {
                WorkDomain::Optional
            } else {
                WorkDomain::Baseline
            };
            assert_eq!(ledger.work_used(other), 0);
            assert!(ledger.work_used(domain) >= input.len() as u64 * 2);
            ledger.release(WorkDomain::Baseline, 1024).unwrap();
            ledger.release(WorkDomain::Optional, 2048).unwrap();
        }
    }
}

#[test]
fn encoder_admission_failures_release_scratch_and_preserve_the_other_domain() {
    let input = noise(65_537);
    for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
        for model in [CompressionCostModel::Gzip, CompressionCostModel::Brotli] {
            for memory in [0, (OUTPUT_CHUNK + HEADER) as u64, 100_000] {
                let mut ledger = ledger(memory, 100_000_000);
                let result = {
                    let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
                    measure_admitted(&input, model, &mut budget)
                };
                assert_eq!(
                    result,
                    Err(CodecError::Admission(AllocationError::Budget(
                        BudgetError::MemoryExhausted(domain)
                    )))
                );
                assert_eq!(ledger.retained_bytes(), 0);
                assert!(ledger.peak_retained_bytes() <= memory);
            }
            let mut ledger = ledger(100_000_000, input.len() as u64 - 1);
            let result = {
                let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
                measure_admitted(&input, model, &mut budget)
            };
            assert_eq!(
                result,
                Err(CodecError::Admission(AllocationError::Budget(
                    BudgetError::WorkExhausted(domain)
                )))
            );
            assert_eq!(ledger.peak_retained_bytes(), 0);
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }
}

#[test]
fn late_brotli_denial_does_not_exit_the_process() {
    const CHILD: &str = "LILSCRIPT_BROTLI_DENIAL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "compression::tests::late_brotli_denial_does_not_exit_the_process",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("late allocation denial survived"));
        return;
    }
    let input = noise(65_537);
    let mut ledger = ledger(100_000, 100_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let mut memory = CodecMemory {
            budget: &mut budget,
            error: None,
        };
        let state = unsafe {
            compu_brotli_sys::BrotliEncoderCreateInstance(
                Some(c_alloc),
                Some(c_free),
                (&mut memory as *mut CodecMemory<'_, '_>).cast(),
            )
        };
        assert!(
            !state.is_null(),
            "fixture must admit state creation before rejecting later work"
        );
        let guard = BrotliGuard(state);
        assert_ne!(
            unsafe {
                compu_brotli_sys::BrotliEncoderSetParameter(
                    state,
                    compu_brotli_sys::BrotliEncoderParameter_BROTLI_PARAM_QUALITY,
                    11,
                )
            },
            0
        );
        let mut output = [0u8; 1024];
        let mut available_in = input.len();
        let mut next_in = input.as_ptr();
        let mut available_out = output.len();
        let mut next_out = output.as_mut_ptr();
        let result = unsafe {
            compu_brotli_sys::BrotliEncoderCompressStream(
                state,
                compu_brotli_sys::BrotliEncoderOperation_BROTLI_OPERATION_FINISH,
                &mut available_in,
                &mut next_in,
                &mut available_out,
                &mut next_out,
                ptr::null_mut(),
            )
        };
        assert_eq!(result, 0);
        assert_eq!(
            memory.error,
            Some(CodecError::Admission(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Optional)
            )))
        );
        drop(guard);
    }
    assert_eq!(ledger.retained_bytes(), 0);
    println!("late allocation denial survived");
}

#[test]
fn raw_has_no_codec_work_or_scratch_even_under_zero_admission() {
    let mut ledger = ledger(0, 0);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        assert_eq!(
            measure_admitted(b"complete artifact", CompressionCostModel::Raw, &mut budget),
            Ok(17)
        );
    }
    assert_eq!(ledger.peak_retained_bytes(), 0);
    assert_eq!(ledger.work_used(WorkDomain::Optional), 0);
}
