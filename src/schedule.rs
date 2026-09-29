//! One rule scheduler (plan M5.3a; architecture §8.2) for the program rules
//! (`program::rules`) and the JavaScript target rules (`js::rules`).
//!
//! A rule set runs its rules in their declared order, round after round,
//! until a round in which no rule changed anything: its fixed point. The
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

/// Run `rules` over `state` to their fixed point: `apply` runs one rule and
/// returns whether it changed anything; `verify` checks the state after
/// each round. Returns the rounds taken, the last of which changed nothing;
/// `exceeded` is the error when `ceiling` rounds were not enough.
pub(crate) fn fixed_point<S: ?Sized, R: Copy, E>(
    state: &mut S,
    rules: &[R],
    ceiling: u32,
    mut apply: impl FnMut(&mut S, R) -> Result<bool, E>,
    mut verify: impl FnMut(&mut S, u32) -> Result<(), E>,
    exceeded: impl FnOnce(u32) -> E,
) -> Result<u32, E> {
    let mut rounds = 0;
    loop {
        if rounds == ceiling {
            return Err(exceeded(rounds));
        }
        rounds += 1;
        let mut changed = false;
        for &rule in rules {
            changed |= apply(state, rule)?;
        }
        verify(state, rounds)?;
        if !changed {
            return Ok(rounds);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixed_point;

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
