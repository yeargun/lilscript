//! Bounded complete-artifact selection over one prepared output. Candidate
//! choices share semantic storage, target facts and naming constraints.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

pub use super::naming::{Plan, Style};
pub use crate::config::CompressionCostModel as Objective;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objectives {
    One(Objective),
    All,
}

impl Objectives {
    pub fn iter(self) -> impl Iterator<Item = Objective> {
        [Objective::Raw, Objective::Gzip, Objective::Brotli]
            .into_iter()
            .filter(move |objective| self == Self::All || self == Self::One(*objective))
    }
}

fn metric_index(objective: Objective) -> usize {
    match objective {
        Objective::Raw => 0,
        Objective::Gzip => 1,
        Objective::Brotli => 2,
    }
}

/// Missing scores are unmeasured, never a fabricated zero or an estimate.
/// Raw length is available even when it is only a tie-breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Sizes {
    pub raw: usize,
    pub gzip9: Option<usize>,
    pub brotli11: Option<usize>,
}

impl Sizes {
    pub fn get(self, objective: Objective) -> Option<usize> {
        match objective {
            Objective::Raw => Some(self.raw),
            Objective::Gzip => self.gzip9,
            Objective::Brotli => self.brotli11,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub plans: usize,
    /// Retained candidate text plus the next render's logical byte limit.
    /// This is not a claim about allocator capacity or whole-process peak RSS.
    pub candidate_bytes: usize,
}

#[derive(Debug)]
pub struct Candidate {
    pub plan: Plan,
    pub javascript: String,
    pub sha256: [u8; 32],
    pub sizes: Sizes,
}

#[derive(Debug)]
pub struct Attempt {
    pub plan: Plan,
    pub artifact: Option<usize>,
    pub rejection: Option<String>,
}

#[derive(Debug)]
pub struct Selection {
    pub candidates: Vec<Candidate>,
    pub attempts: Vec<Attempt>,
    pub objectives: Objectives,
    /// Requested raw, gzip, Brotli winners from the same complete union.
    /// An unrequested objective has no winner, even if raw length is known.
    pub winners: [Option<usize>; 3],
    pub rendered_bytes: usize,
    pub retained_bytes: usize,
    pub duplicate_artifacts: usize,
    /// Actual measurement-service calls: once per artifact per requested
    /// compressed representation. Raw length requires no service call.
    pub measurement_calls: usize,
    pub proposal_steps: usize,
}

impl Selection {
    pub fn winner(&self, objective: Objective) -> Option<&Candidate> {
        self.winners[metric_index(objective)].map(|id| &self.candidates[id])
    }
}

enum Pending {
    Evaluate(Plan),
    Expand { plan: Plan, next: usize },
}

impl extract::Output<'_> {
    /// The measurement service is explicit. Each requested compressed score
    /// receives complete bytes once per distinct artifact. Raw never calls it.
    pub fn select(
        &self,
        budget: Budget,
        objectives: Objectives,
        mut measure: impl FnMut(&[u8], Objective) -> Result<usize, String>,
    ) -> Result<Selection, String> {
        if self.is_accounted() {
            return Err("inspection selection cannot own admitted artifacts".into());
        }
        if !(1..=512).contains(&budget.plans) || budget.candidate_bytes == 0 {
            return Err("invalid output search budget".into());
        }
        let mut selection = Selection {
            candidates: vec![],
            attempts: vec![],
            objectives,
            winners: [None; 3],
            rendered_bytes: 0,
            retained_bytes: 0,
            duplicate_artifacts: 0,
            measurement_calls: 0,
            proposal_steps: 0,
        };
        let mut pending: VecDeque<_> = self
            .naming_seeds()
            .iter()
            .map(|&style| Pending::Evaluate(Plan::new(style)))
            .collect();
        let mut seen = crate::stable_hash::StableHashSet::default();
        let mut hashes = BTreeMap::<[u8; 32], Vec<usize>>::new();
        while selection.proposal_steps < budget.plans {
            let Some(next) = pending.pop_front() else {
                break;
            };
            // Duplicated plans and exhausted expansion cursors consume work
            // too. They cannot extend execution beyond the declared schedule.
            selection.proposal_steps += 1;
            let plan = match next {
                Pending::Evaluate(plan) => plan,
                Pending::Expand { plan, mut next } => {
                    while self
                        .source_candidates_admitted()
                        .map_err(|error| error.to_string())?
                        .get(next)
                        .is_some_and(|binding| plan.source_names.contains(binding))
                    {
                        next += 1;
                    }
                    let Some(&binding) = self
                        .source_candidates_admitted()
                        .map_err(|error| error.to_string())?
                        .get(next)
                    else {
                        continue;
                    };
                    let mut child = plan.clone();
                    child.source_names.push(binding);
                    child.source_names.sort_unstable();
                    pending.push_back(Pending::Expand {
                        plan,
                        next: next + 1,
                    });
                    child
                }
            };
            if !seen.insert(plan.clone()) {
                continue;
            }
            let code = match self
                .render_bounded(&plan, budget.candidate_bytes - selection.retained_bytes)
            {
                Ok(code) => code,
                Err(reason) => {
                    selection.attempts.push(Attempt {
                        plan,
                        artifact: None,
                        rejection: Some(reason),
                    });
                    continue;
                }
            };
            selection.rendered_bytes += code.len();
            let hash: [u8; 32] = Sha256::digest(code.as_bytes()).into();
            // A digest indexes candidates; equality of complete bytes proves
            // deduplication, so a hash collision cannot change selection.
            if let Some(&same) = hashes.get(&hash).and_then(|bucket| {
                bucket
                    .iter()
                    .find(|&&index| selection.candidates[index].javascript == code)
            }) {
                selection.duplicate_artifacts += 1;
                selection.attempts.push(Attempt {
                    plan: plan.clone(),
                    artifact: Some(same),
                    rejection: None,
                });
                // Different legal name plans may have different useful neighbours.
                if self.permits_naming_search()
                    && plan.style != Style::Source
                    && selection.winners.contains(&Some(same))
                {
                    pending.push_back(Pending::Expand { plan, next: 0 });
                }
                continue;
            }
            let mut sizes = Sizes {
                raw: code.len(),
                gzip9: None,
                brotli11: None,
            };
            for objective in objectives.iter() {
                if objective == Objective::Raw {
                    continue;
                }
                let encoded = measure(code.as_bytes(), objective)?;
                if encoded == 0 {
                    return Err("measurement service returned an invalid encoded size".into());
                }
                selection.measurement_calls += 1;
                match objective {
                    Objective::Gzip => sizes.gzip9 = Some(encoded),
                    Objective::Brotli => sizes.brotli11 = Some(encoded),
                    Objective::Raw => unreachable!(),
                }
            }
            let index = selection.candidates.len();
            let mut improved = false;
            for objective in objectives.iter() {
                let winner = &mut selection.winners[metric_index(objective)];
                let better = winner.is_none_or(|id| {
                    let incumbent = selection.candidates[id].sizes;
                    (sizes.get(objective).unwrap(), sizes.raw)
                        < (incumbent.get(objective).unwrap(), incumbent.raw)
                });
                if better {
                    *winner = Some(index);
                    improved = true;
                }
            }
            selection.retained_bytes += code.len();
            selection.candidates.push(Candidate {
                plan: plan.clone(),
                javascript: code,
                sha256: hash,
                sizes,
            });
            hashes.entry(hash).or_default().push(index);
            selection.attempts.push(Attempt {
                plan: plan.clone(),
                artifact: Some(index),
                rejection: None,
            });
            if self.permits_naming_search() && improved && plan.style != Style::Source {
                pending.push_back(Pending::Expand { plan, next: 0 });
            }
        }
        if selection.candidates.is_empty() {
            return Err("no valid artifact within output search budget".into());
        }
        Ok(selection)
    }
}
