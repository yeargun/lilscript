//! Finite author-visible regional choices. A pin selects an already legal
//! spelling; it is neither a semantic proof nor permission to run a tactic.
use super::{AltId, ChoiceFamily};
use crate::compilation_policy::{AdmissionError, ResolvedPolicy, TacticId, TacticUse};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RegionalChoices(u32);

impl RegionalChoices {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(0xAA_AAAA);
    pub const FAMILIES: [(ChoiceFamily, &'static str); 12] = [
        (ChoiceFamily::ConditionalValues, "conditional_values"),
        (ChoiceFamily::ExitPoints, "exit_points"),
        (ChoiceFamily::LoopFusion, "loop_fusion"),
        (ChoiceFamily::ConditionalReturns, "conditional_returns"),
        (ChoiceFamily::LogicalBranches, "logical_branches"),
        (ChoiceFamily::LoopHeads, "loop_heads"),
        (ChoiceFamily::LogicalStatements, "logical_statements"),
        (ChoiceFamily::CompoundAssignments, "compound_assignments"),
        (ChoiceFamily::QuoteDelimiter, "quote_delimiter"),
        (ChoiceFamily::ReceiverAlias, "receiver_alias"),
        (ChoiceFamily::OptionalChain, "optional_chain"),
        (ChoiceFamily::LogicalAssignment, "logical_assignment"),
    ];
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    fn index(family: ChoiceFamily) -> Option<usize> {
        Self::FAMILIES.iter().position(|(f, _)| *f == family)
    }
    pub fn named(name: &str) -> Option<ChoiceFamily> {
        Self::FAMILIES
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(f, _)| *f)
    }
    pub fn get(self, family: ChoiceFamily) -> Option<AltId> {
        if self.is_empty() {
            return None;
        }
        let shift = Self::index(family)? * 2;
        match (self.0 >> shift) & 3 {
            0 => None,
            1 => Some(AltId(0)),
            _ => Some(AltId(1)),
        }
    }
    pub fn insert(&mut self, family: ChoiceFamily, alternative: AltId) -> bool {
        let Some(index) = Self::index(family) else {
            return false;
        };
        if alternative.0 > 1 || self.get(family).is_some() {
            return false;
        }
        self.0 |= (u32::from(alternative.0) + 1) << (2 * index);
        true
    }
    /// Aggregate requested permissions, not a merged region's behavior.
    pub const fn requests(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub fn iter(self) -> impl Iterator<Item = (ChoiceFamily, AltId)> {
        Self::FAMILIES
            .into_iter()
            .take(if self.is_empty() {
                0
            } else {
                Self::FAMILIES.len()
            })
            .filter_map(move |(family, _)| self.get(family).map(|alt| (family, alt)))
    }
    pub fn uses(self, tactic: TacticId) -> bool {
        self.iter().any(|(family, alt)| {
            alt == AltId(1)
                && (tactic == TacticId::TargetCompaction || family.spec().tactic == tactic)
        })
    }
    pub fn check(self, policy: &ResolvedPolicy) -> Result<(), AdmissionError> {
        if policy.javascript_contract().is_none() {
            return Ok(());
        }
        for (family, alt) in self.iter() {
            if alt != AltId(0) {
                family.spec().check(policy)?;
            }
        }
        Ok(())
    }
    pub fn permitted(policies: &[&ResolvedPolicy]) -> Self {
        let mut result = Self::NONE;
        for (family, _) in Self::FAMILIES {
            if policies
                .iter()
                .filter(|p| p.javascript_contract().is_some())
                .all(|p| family.spec().enabled(p))
            {
                result.insert(family, AltId(1));
            }
        }
        result
    }
    pub fn allowed_by(self, permissions: Self) -> bool {
        self.iter()
            .all(|(family, alt)| alt == AltId(0) || permissions.get(family) == Some(AltId(1)))
    }
    pub fn tactic_uses<'a>(self) -> impl Iterator<Item = &'a TacticUse> {
        use crate::compilation_policy::RuntimeRisk;
        const USES: [TacticUse; 3] = [
            TacticUse {
                tactic: TacticId::TargetCompaction,
                risk: RuntimeRisk::Neutral,
            },
            TacticUse {
                tactic: TacticId::StatementSpellings,
                risk: RuntimeRisk::Neutral,
            },
            TacticUse {
                tactic: TacticId::ReceiverAliases,
                risk: RuntimeRisk::Neutral,
            },
        ];
        USES.iter().filter(move |usage| self.uses(usage.tactic))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn s4_regional_registry_has_stable_binary_choices_and_hard_permissions() {
        let mut pins = RegionalChoices::NONE;
        for (family, name) in RegionalChoices::FAMILIES {
            assert_eq!(RegionalChoices::named(name), Some(family));
            assert_eq!(family.spec().alternatives, 2);
            assert!(pins.insert(family, AltId(1)));
            assert!(!pins.insert(family, AltId(0)));
        }
        assert_eq!(pins, RegionalChoices::ALL);
        assert!(pins.allowed_by(RegionalChoices::ALL));
        assert!(!pins.allowed_by(RegionalChoices::NONE));
        assert_eq!(pins.tactic_uses().count(), 3);
    }
}
