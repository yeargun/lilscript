//! Source constraints travel independently of optional search assignments.
use super::*;
use crate::output_budget::AllocationClass::Retained;
use crate::representation::RegionalChoices;

impl Module {
    pub(crate) fn expression_choices(&self, id: ExprId) -> RegionalChoices {
        self.authored_expressions
            .get(id.index())
            .copied()
            .unwrap_or_default()
    }
    pub(crate) fn region_choices(&self, id: RegionId) -> RegionalChoices {
        self.authored_regions
            .get(id.index())
            .copied()
            .unwrap_or_default()
    }
    pub(crate) fn set_region_choices(
        &mut self,
        id: RegionId,
        choices: RegionalChoices,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        if self.authored_regions.is_empty() && choices.is_empty() {
            return Ok(());
        }
        if self.authored_regions.len() < self.regions.len() {
            let count = self.regions.len() - self.authored_regions.len();
            budget.reserve_vec(Retained, &mut self.authored_regions, count)?;
            self.authored_regions
                .resize(self.regions.len(), RegionalChoices::NONE);
        }
        self.authored_regions[id.index()] = choices;
        Ok(())
    }
    pub(crate) fn copy_author_choices(&mut self, from: ExprId, to: ExprId) {
        if !self.authored_expressions.is_empty() {
            self.authored_expressions[to.index()] = self.expression_choices(from);
            // A copy of a pinned expression keeps its stable source site. It
            // must remain fixed even when tail inlining duplicates that site.
            if !self.expression_choices(from).is_empty() && !self.spelling_nodes.is_empty() {
                self.spelling_nodes[to.index()] = self.spelling_nodes[from.index()];
            }
        }
    }
    pub(crate) fn authored_site(&self, site: SiteId) -> RegionalChoices {
        let SiteId::Target {
            head,
            kind,
            ordinal,
        } = site
        else {
            return RegionalChoices::NONE;
        };
        if head != self.spelling_head {
            return RegionalChoices::NONE;
        }
        match kind {
            0 if (ordinal as usize) < self.spelling_node_count => self
                .authored_sites
                .get(ordinal as usize)
                .copied()
                .unwrap_or_default(),
            0 => self
                .authored_regions
                .get(ordinal as usize - self.spelling_node_count)
                .copied()
                .unwrap_or_default(),
            1 => self
                .authored_regions
                .get(ordinal as usize)
                .copied()
                .unwrap_or_default(),
            2 => self
                .functions
                .get(ordinal as usize)
                .map(|f| self.region_choices(f.body))
                .unwrap_or_default(),
            _ => RegionalChoices::NONE,
        }
    }
    pub(crate) fn conflicts_with_authors(&self, choices: &ChoiceMap) -> bool {
        choices.iter().any(|(key, alt)| {
            self.authored_site(key.site)
                .get(key.family)
                .is_some_and(|pin| pin != alt)
        })
    }
}
