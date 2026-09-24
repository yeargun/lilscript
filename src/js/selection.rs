//! The vocabulary of output selection: naming plans, codec objectives and
//! measured sizes. The search over them is `program::search`.

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
