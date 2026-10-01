//! The vocabulary of output selection: naming plans, codec objectives and
//! measured sizes. The search over them is `program::search`.

pub use super::naming::{Alphabet, Plan, Style};
pub use crate::config::CompressionCostModel as Objective;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objectives {
    One(Objective),
    /// The two requested coordinates; iteration is canonical and deduplicated.
    Two(Objective, Objective),
    All,
}

impl Objectives {
    /// A nonempty set, normalized to raw/gzip/Brotli order. Configuration
    /// validation diagnoses duplicates; API callers may supply repeated names.
    pub fn from_codecs(codecs: &[Objective]) -> Option<Self> {
        let mask = codecs.iter().fold(0u8, |mask, codec| {
            mask | match codec {
                Objective::Raw => 1,
                Objective::Gzip => 2,
                Objective::Brotli => 4,
            }
        });
        match mask {
            1 => Some(Self::One(Objective::Raw)),
            2 => Some(Self::One(Objective::Gzip)),
            3 => Some(Self::Two(Objective::Raw, Objective::Gzip)),
            4 => Some(Self::One(Objective::Brotli)),
            5 => Some(Self::Two(Objective::Raw, Objective::Brotli)),
            6 => Some(Self::Two(Objective::Gzip, Objective::Brotli)),
            7 => Some(Self::All),
            _ => None,
        }
    }

    pub fn normalized(self) -> Self {
        match self {
            Self::Two(first, second) => Self::from_codecs(&[first, second]).unwrap(),
            other => other,
        }
    }

    pub fn iter(self) -> impl Iterator<Item = Objective> {
        [Objective::Raw, Objective::Gzip, Objective::Brotli]
            .into_iter()
            .filter(move |objective| match self {
                Self::One(codec) => *objective == codec,
                Self::Two(first, second) => *objective == first || *objective == second,
                Self::All => true,
            })
    }
}

/// Missing scores are unmeasured, never a fabricated zero or an estimate.
/// Raw length is available even when it is only a tie-breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
