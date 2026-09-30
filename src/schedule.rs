//! One rule scheduler (plan M5.3a; architecture §8.2) for the program rules
//! (`program::rules`) and the JavaScript target rules (`js::rules`).
//!
//! A rule set runs in its declared order until no rule can change it: its
//! fixed point. A previously stable suffix is retried after any prefix edit,
//! so the order of actual edits matches complete rounds. The
//! order is structural; it never depends on a printed size, a name plan, a
//! codec or the effort level, and a rule phase is never truncated by effort.
//! A round ceiling that is reached is a compiler bug: the build fails rather
//! than deliver a partial result.
//!
//! Prior art: Closure's `PhaseOptimizer` loop (`closure-compiler@0da58e1
//! src/com/google/javascript/jscomp/PhaseOptimizer.java:270-277`), which
//! stops on change stamps, and Oxc's `run_in_loop` (`oxc@591966d
//! crates/oxc_minifier/src/compressor.rs:106-140`), which stops when a pass
//! asks for no other; here each rule reports whether it changed anything
//! (the program's editor, the tree's journal).

#[cfg(test)]
thread_local! {
    static DENSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Tests can compare the optimized scheduler with the original full rounds.
/// This guard restores the thread's mode even when a test unwinds.
#[cfg(test)]
pub(crate) struct DenseAudit(bool);
#[cfg(test)]
impl DenseAudit {
    pub(crate) fn new() -> Self {
        Self(DENSE.with(|dense| dense.replace(true)))
    }
}
#[cfg(test)]
impl Drop for DenseAudit {
    fn drop(&mut self) {
        DENSE.with(|dense| dense.set(self.0));
    }
}

pub(crate) fn reuses_stability() -> bool {
    #[cfg(test)]
    if DENSE.with(std::cell::Cell::get) {
        return false;
    }
    true
}

/// Run `rules` over `state` to their fixed point: `apply` runs one rule and
/// returns whether it changed anything; `verify` checks the state after
/// each round. Returns the rounds taken, the last of which changed nothing;
/// `exceeded` is the error when `ceiling` rounds were not enough.
/// `apply` must be deterministic for an unchanged state/context; `verify`
/// checks it without changing meaning. A stable suffix needs no retry until
/// a prefix rule edits. Any such edit reopens that suffix immediately.
pub(crate) fn fixed_point<S: ?Sized, R: Copy, E>(
    state: &mut S,
    rules: &[R],
    ceiling: u32,
    mut apply: impl FnMut(&mut S, R) -> Result<bool, E>,
    mut verify: impl FnMut(&mut S, u32) -> Result<(), E>,
    exceeded: impl FnOnce(u32) -> E,
) -> Result<u32, E> {
    let mut rounds = 0;
    let reuse = reuses_stability();
    let mut pending = rules.len();
    loop {
        if rounds == ceiling {
            return Err(exceeded(rounds));
        }
        rounds += 1;
        let mut changed = false;
        let mut next_pending = 0;
        let mut index = 0;
        while index < pending {
            if apply(state, rules[index])? {
                changed = true;
                next_pending = index + 1;
                // A preceding edit invalidates every later stable answer.
                pending = rules.len();
            }
            index += 1;
        }
        verify(state, rounds)?;
        if !changed {
            return Ok(rounds);
        }
        pending = if reuse { next_pending } else { rules.len() };
    }
}

#[cfg(test)]
mod tests {
    use super::{fixed_point, DenseAudit};

    #[test]
    fn a_stable_suffix_is_skipped_and_reopens_after_any_prefix_edit() {
        fn run(dense: bool) -> (Vec<usize>, u32, u32) {
            let _audit = dense.then(DenseAudit::new);
            let mut calls = Vec::new();
            let mut value = 9u32;
            let rounds = fixed_point(
                &mut value,
                &[0, 1, 2],
                16,
                |value, rule| {
                    calls.push(rule);
                    let before = *value;
                    // The prefix sometimes changes on a subsequent round and
                    // must reopen rule 2, which was stable on the prior input.
                    match rule {
                        0 if *value > 0 => *value -= 1,
                        2 if *value > 1 && *value % 2 == 1 => *value -= 1,
                        _ => (),
                    }
                    Ok::<_, ()>(before != *value)
                },
                |_, _| Ok(()),
                |_| (),
            )
            .unwrap();
            (calls, rounds, value)
        }
        let dense = run(true);
        let reused = run(false);
        assert_eq!((reused.1, reused.2), (dense.1, dense.2));
        assert_eq!(reused.2, 0);
        assert_eq!(&reused.0, &dense.0[..dense.0.len() - 2]);
        assert_eq!(&dense.0[dense.0.len() - 3..], &[0, 1, 2]);
    }

    #[test]
    fn suffix_reuse_matches_dense_rounds_across_coupled_rules() {
        for initial in 0..64u32 {
            let runs = [false, true].map(|dense| {
                let _audit = dense.then(DenseAudit::new);
                let mut state = [initial, 0, 0];
                let mut edits = Vec::new();
                let rounds = fixed_point(
                    &mut state,
                    &[0, 1, 2],
                    256,
                    |state, rule| {
                        let before = *state;
                        match rule {
                            0 if state[0] > 0 => {
                                state[0] -= 1;
                                state[1] += 1;
                            }
                            1 if state[1] >= 3 => {
                                state[1] -= 3;
                                state[2] += 1;
                            }
                            2 if state[2] >= 2 => {
                                state[2] -= 2;
                                state[0] += 1;
                            }
                            _ => (),
                        }
                        let changed = *state != before;
                        if changed {
                            edits.push((rule, *state));
                        }
                        Ok::<_, ()>(changed)
                    },
                    |_, _| Ok(()),
                    |_| (),
                )
                .unwrap();
                (state, edits, rounds)
            });
            assert_eq!(runs[0], runs[1], "initial {initial}");
        }
    }

    #[test]
    fn rules_run_until_a_round_changes_nothing() {
        // Two rules: one halves an even number, one decrements an odd one.
        let mut value = 12u32;
        let rounds = fixed_point(
            &mut value,
            &[true, false],
            16,
            |value, halves| {
                let before = *value;
                if halves && *value % 2 == 0 && *value > 0 {
                    *value /= 2;
                } else if !halves && *value % 2 == 1 {
                    *value -= 1;
                }
                Ok::<_, String>(*value != before)
            },
            |_, _| Ok(()),
            |rounds| format!("{rounds}"),
        )
        .unwrap();
        assert_eq!(value, 0);
        // 12→6, 6→3→2, 2→1→0, then a round without change.
        assert_eq!(rounds, 4);
    }

    #[test]
    fn a_ceiling_reached_is_an_error() {
        let mut flips = false;
        let error = fixed_point(
            &mut flips,
            &[()],
            8,
            |flips, ()| {
                *flips = !*flips;
                Ok::<_, u32>(true)
            },
            |_, _| Ok(()),
            |rounds| rounds,
        )
        .unwrap_err();
        assert_eq!(error, 8);
    }
}
