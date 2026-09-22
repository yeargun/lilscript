//! Resource descriptions borrow the existing sealed owners. One traversal feeds
//! both hashing and exact comparison; producer text never becomes recipe words.
//! Source-snapshot compatibility is checked separately by publication.
use crate::compilation_policy::{RuntimeRisk, TacticUse, WorkKind};
use crate::output_budget::{AllocationBudget, AllocationError};
use crate::semantic_program::artifact_provenance::OutputTactics;
use crate::semantic_program::fixed_resource::ResourceChoice;
use crate::semantic_program::rewrite_lineage::RewriteDescription;
use crate::semantic_program::{CellId, TypeId, UnitId};
use crate::structured_js::selection::Plan;
use std::cmp::Ordering;

pub use crate::semantic_program::function_layout::ProductTransport;
pub use crate::semantic_program::physical_export::PhysicalParameter;

/// Canonical physical ABI over the caller-qualified semantic source tables.
/// This view contains no revision, proof handle, allocator identity or Arc.
/// Invocation is unbound and strict, and all argument/result transport is raw;
/// these are invariants of the current sealed physical-export constructor.
#[derive(Debug, Clone, Copy)]
pub struct PhysicalExportDescription<'a> {
    cell: CellId,
    body: UnitId,
    initializer: UnitId,
    signature: TypeId,
    result: TypeId,
    source_specifier: &'a str,
    export_name: &'a str,
    parameters: &'a [PhysicalParameter],
}
impl<'a> PhysicalExportDescription<'a> {
    pub(in crate::semantic_program) fn new(
        cell: CellId,
        body: UnitId,
        initializer: UnitId,
        signature: TypeId,
        result: TypeId,
        source_specifier: &'a str,
        export_name: &'a str,
        parameters: &'a [PhysicalParameter],
    ) -> Self {
        Self {
            cell,
            body,
            initializer,
            signature,
            result,
            source_specifier,
            export_name,
            parameters,
        }
    }
    pub fn cell(self) -> CellId {
        self.cell
    }
    pub fn body(self) -> UnitId {
        self.body
    }
    pub fn initializer(self) -> UnitId {
        self.initializer
    }
    pub fn signature(self) -> TypeId {
        self.signature
    }
    pub fn result(self) -> TypeId {
        self.result
    }
    pub fn source_specifier(self) -> &'a str {
        self.source_specifier
    }
    pub fn export_name(self) -> &'a str {
        self.export_name
    }
    pub fn parameters(self) -> &'a [PhysicalParameter] {
        self.parameters
    }
}

/// Actual output and permission evidence retained with immutable producer bytes.
/// Naming provenance is explicit: the same Source plan can be a mandatory
/// baseline or an optional explored plan with different policy eligibility.
#[derive(Debug, Clone, Copy)]
pub struct ArtifactProvenanceDescription<'a> {
    naming: &'a Plan,
    output: OutputTactics,
    naming_tactics: &'a [TacticUse],
    tactics: &'a [TacticUse],
}
impl<'a> ArtifactProvenanceDescription<'a> {
    pub(in crate::semantic_program) fn new(
        naming: &'a Plan,
        output: OutputTactics,
        naming_tactics: &'a [TacticUse],
        tactics: &'a [TacticUse],
    ) -> Self {
        Self {
            naming,
            output,
            naming_tactics,
            tactics,
        }
    }
    pub fn naming(self) -> &'a Plan {
        self.naming
    }
    pub fn output(self) -> OutputTactics {
        self.output
    }
    pub fn naming_tactics(self) -> &'a [TacticUse] {
        self.naming_tactics
    }
    pub fn tactics(self) -> &'a [TacticUse] {
        self.tactics
    }
}

/// Borrowed complete fixed producer. Scores are derived from these exact bytes
/// and are deliberately absent from structural identity, as are capacities.
#[derive(Debug, Clone, Copy)]
pub struct FrozenProducerDescription<'a> {
    contract: PhysicalExportDescription<'a>,
    javascript: &'a str,
    recipes: &'a [u32],
    rewrites: RewriteDescription<'a>,
    snapshot: Option<crate::semantic_program::RevisionId>,
    meaning: Option<crate::semantic_program::RevisionId>,
    provenance: ArtifactProvenanceDescription<'a>,
}
impl<'a> FrozenProducerDescription<'a> {
    pub(in crate::semantic_program) fn new(
        contract: PhysicalExportDescription<'a>,
        javascript: &'a str,
        recipes: &'a [u32],
        rewrites: RewriteDescription<'a>,
        snapshot: Option<crate::semantic_program::RevisionId>,
        meaning: Option<crate::semantic_program::RevisionId>,
        provenance: ArtifactProvenanceDescription<'a>,
    ) -> Self {
        Self {
            contract,
            javascript,
            recipes,
            rewrites,
            snapshot,
            meaning,
            provenance,
        }
    }
    pub fn contract(self) -> PhysicalExportDescription<'a> {
        self.contract
    }
    pub fn javascript(self) -> &'a str {
        self.javascript
    }
    pub fn recipe_words(self) -> &'a [u32] {
        self.recipes
    }
    pub fn rewrites(self) -> RewriteDescription<'a> {
        self.rewrites
    }
    pub fn snapshot_identity(self) -> Option<crate::semantic_program::RevisionId> {
        self.snapshot
    }
    pub fn meaning_identity(self) -> Option<crate::semantic_program::RevisionId> {
        self.meaning
    }
    pub fn provenance(self) -> ArtifactProvenanceDescription<'a> {
        self.provenance
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ResourceDescription<'a> {
    Whole,
    Producer {
        contract: PhysicalExportDescription<'a>,
    },
    Consumer {
        consumed: PhysicalExportDescription<'a>,
        producer: FrozenProducerDescription<'a>,
    },
}
impl<'a> ResourceDescription<'a> {
    pub(super) fn of(choice: Option<&'a ResourceChoice>) -> Self {
        match choice {
            None => Self::Whole,
            Some(ResourceChoice::Producer(contract)) => Self::Producer {
                contract: contract.borrow().description(),
            },
            Some(ResourceChoice::Consumer { producer, consumed }) => Self::Consumer {
                consumed: consumed.borrow().description(),
                producer: producer.description(),
            },
        }
    }
    fn kind(self) -> u64 {
        match self {
            Self::Whole => 0,
            Self::Producer { .. } => 1,
            Self::Consumer { .. } => 2,
        }
    }
}

// Resource format is separate from the unchanged Whole FORMAT=3 word encoding.
// Extend it when any sealed invocation, transport or output assumption changes.
const RESOURCE_FORMAT: u64 = 3;

trait Parts {
    fn scalar(&mut self, left: u64, right: u64) -> Result<Ordering, AllocationError>;
    fn bytes(&mut self, left: &[u8], right: &[u8]) -> Result<Ordering, AllocationError>;
}
macro_rules! part {
    ($expression:expr) => {
        match $expression? {
            Ordering::Equal => {}
            order => return Ok(order),
        }
    };
}

fn visit(
    left: ResourceDescription<'_>,
    right: ResourceDescription<'_>,
    parts: &mut impl Parts,
) -> Result<Ordering, AllocationError> {
    part!(parts.scalar(RESOURCE_FORMAT, RESOURCE_FORMAT));
    part!(parts.scalar(left.kind(), right.kind()));
    match (left, right) {
        (ResourceDescription::Whole, ResourceDescription::Whole) => Ok(Ordering::Equal),
        (
            ResourceDescription::Producer { contract: left },
            ResourceDescription::Producer { contract: right },
        ) => contract(left, right, parts),
        (
            ResourceDescription::Consumer {
                consumed: left,
                producer: lp,
            },
            ResourceDescription::Consumer {
                consumed: right,
                producer: rp,
            },
        ) => {
            part!(contract(left, right, parts));
            part!(contract(lp.contract(), rp.contract(), parts));
            part!(parts.scalar(
                number(lp.recipe_words().len())?,
                number(rp.recipe_words().len())?
            ));
            for (&left, &right) in lp.recipe_words().iter().zip(rp.recipe_words()) {
                part!(parts.scalar(u64::from(left), u64::from(right)));
            }
            part!(parts.scalar(number(lp.rewrites().len())?, number(rp.rewrites().len())?));
            for (left, right) in lp.rewrites().steps().zip(rp.rewrites().steps()) {
                for (left, right) in left.rule.words().into_iter().zip(right.rule.words()) {
                    part!(parts.scalar(u64::from(left), u64::from(right)));
                }
            }
            part!(parts.bytes(lp.javascript().as_bytes(), rp.javascript().as_bytes()));
            provenance(lp.provenance(), rp.provenance(), parts)
        }
        _ => unreachable!("resource kind comparison returns before unequal variants"),
    }
}

fn contract(
    left: PhysicalExportDescription<'_>,
    right: PhysicalExportDescription<'_>,
    parts: &mut impl Parts,
) -> Result<Ordering, AllocationError> {
    for (left, right) in [
        (left.cell().index(), right.cell().index()),
        (left.body().index(), right.body().index()),
        (left.initializer().index(), right.initializer().index()),
        (left.signature().index(), right.signature().index()),
        (left.result().index(), right.result().index()),
    ] {
        part!(parts.scalar(number(left)?, number(right)?));
    }
    part!(parts.bytes(
        left.source_specifier().as_bytes(),
        right.source_specifier().as_bytes()
    ));
    part!(parts.bytes(
        left.export_name().as_bytes(),
        right.export_name().as_bytes()
    ));
    // Count precedes rows so every scalar/string slice is unambiguous in the
    // fingerprint stream; equality still always compares actual values.
    part!(parts.scalar(
        number(left.parameters().len())?,
        number(right.parameters().len())?
    ));
    for (left, right) in left.parameters().iter().zip(right.parameters()) {
        part!(parts.scalar(u64::from(left.position()), u64::from(right.position())));
        part!(parts.scalar(
            number(left.schema().index())?,
            number(right.schema().index())?
        ));
        part!(parts.scalar(transport(left.transport()), transport(right.transport())));
        part!(parts.scalar(number(left.fields().len())?, number(right.fields().len())?));
        for (left, right) in left.fields().iter().zip(right.fields()) {
            part!(parts.scalar(number(left.index())?, number(right.index())?));
        }
        part!(parts.scalar(
            number(left.field_types().len())?,
            number(right.field_types().len())?
        ));
        for (left, right) in left.field_types().iter().zip(right.field_types()) {
            part!(parts.scalar(number(left.index())?, number(right.index())?));
        }
    }
    Ok(Ordering::Equal)
}

fn provenance(
    left: ArtifactProvenanceDescription<'_>,
    right: ArtifactProvenanceDescription<'_>,
    parts: &mut impl Parts,
) -> Result<Ordering, AllocationError> {
    use crate::structured_js::selection::Style;
    let style = |value| match value {
        Style::Global => 0,
        Style::Scoped => 1,
        Style::Source => 2,
    };
    part!(parts.scalar(style(left.naming().style), style(right.naming().style)));
    part!(parts.scalar(
        number(left.naming().source_names.len())?,
        number(right.naming().source_names.len())?
    ));
    for (left, right) in left
        .naming()
        .source_names
        .iter()
        .zip(&right.naming().source_names)
    {
        part!(parts.scalar(number(left.index())?, number(right.index())?));
    }
    part!(parts.scalar(
        u64::from(left.output().dead_code_elimination),
        u64::from(right.output().dead_code_elimination)
    ));
    part!(parts.scalar(
        u64::from(left.output().target_compaction),
        u64::from(right.output().target_compaction)
    ));
    let literal = |value| match value {
        crate::structured_js::LiteralOutput::Original => 0,
        crate::structured_js::LiteralOutput::Observed => 1,
    };
    part!(parts.scalar(
        literal(left.output().literals),
        literal(right.output().literals)
    ));
    part!(tactics(
        left.naming_tactics(),
        right.naming_tactics(),
        parts
    ));
    tactics(left.tactics(), right.tactics(), parts)
}
fn tactics(
    left: &[TacticUse],
    right: &[TacticUse],
    parts: &mut impl Parts,
) -> Result<Ordering, AllocationError> {
    part!(parts.scalar(number(left.len())?, number(right.len())?));
    for (left, right) in left.iter().zip(right) {
        part!(parts.scalar(
            number(left.tactic as usize)?,
            number(right.tactic as usize)?
        ));
        part!(parts.scalar(risk(left.risk), risk(right.risk)));
    }
    Ok(Ordering::Equal)
}
fn transport(value: ProductTransport) -> u64 {
    match value {
        ProductTransport::Packed => 0,
        ProductTransport::Fields => 1,
    }
}
fn risk(value: RuntimeRisk) -> u64 {
    match value {
        RuntimeRisk::Neutral => 0,
        RuntimeRisk::Startup => 1,
        RuntimeRisk::Recurring => 2,
    }
}
fn number(value: usize) -> Result<u64, AllocationError> {
    u64::try_from(value).map_err(|_| AllocationError::Capacity)
}

struct Compare<'a, 'ledger>(&'a mut AllocationBudget<'ledger>);
impl Parts for Compare<'_, '_> {
    fn scalar(&mut self, left: u64, right: u64) -> Result<Ordering, AllocationError> {
        self.0.work(WorkKind::Analysis, 1)?;
        Ok(left.cmp(&right))
    }
    fn bytes(&mut self, left: &[u8], right: &[u8]) -> Result<Ordering, AllocationError> {
        for (left, right) in left.iter().zip(right) {
            self.0.work(WorkKind::Analysis, 1)?;
            let order = left.cmp(right);
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        self.scalar(number(left.len())?, number(right.len())?)
    }
}
pub(super) fn compare(
    left: ResourceDescription<'_>,
    right: ResourceDescription<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Ordering, AllocationError> {
    visit(left, right, &mut Compare(budget))
}

struct Fingerprint<'a, 'ledger> {
    hash: &'a mut u64,
    budget: &'a mut AllocationBudget<'ledger>,
}
impl Fingerprint<'_, '_> {
    fn admitted_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            *self.hash = (*self.hash ^ u64::from(byte)).wrapping_mul(super::HASH_PRIME);
        }
    }
}
impl Parts for Fingerprint<'_, '_> {
    fn scalar(&mut self, left: u64, _: u64) -> Result<Ordering, AllocationError> {
        self.budget.work(WorkKind::Analysis, 1)?;
        self.admitted_bytes(&left.to_le_bytes());
        Ok(Ordering::Equal)
    }
    fn bytes(&mut self, left: &[u8], _: &[u8]) -> Result<Ordering, AllocationError> {
        let count = number(left.len())?;
        self.budget.work(
            WorkKind::Analysis,
            count.checked_add(1).ok_or(AllocationError::Capacity)?,
        )?;
        self.admitted_bytes(&count.to_le_bytes());
        self.admitted_bytes(left);
        Ok(Ordering::Equal)
    }
}
pub(super) fn fingerprint(
    description: ResourceDescription<'_>,
    hash: &mut u64,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    // The same borrowed traversal hashes the left side, so it cannot omit an
    // eligibility/ABI field which the full equality path would distinguish.
    visit(description, description, &mut Fingerprint { hash, budget })?;
    Ok(())
}

#[cfg(test)]
mod producer_recipe_tests {
    use super::*;
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
    use crate::structured_js::{selection::Style, LiteralOutput};

    #[test]
    fn equal_producer_bytes_and_tactics_do_not_hide_different_structural_recipes() {
        let mut ledger = BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 10_000,
                optional_work: 10_000,
                baseline_retained_bytes: 0,
                retained_bytes: 10_000,
            },
        )
        .unwrap();
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let contract = PhysicalExportDescription::new(
            CellId::from_index(0).unwrap(),
            UnitId::from_index(0).unwrap(),
            UnitId::from_index(0).unwrap(),
            TypeId::from_index(0).unwrap(),
            TypeId::from_index(0).unwrap(),
            "./producer.mjs",
            "answer",
            &[],
        );
        let naming = Plan::new(Style::Global);
        let provenance = ArtifactProvenanceDescription::new(
            &naming,
            OutputTactics {
                dead_code_elimination: false,
                target_compaction: false,
                literals: LiteralOutput::Original,
            },
            &[],
            &[],
        );
        let left = ResourceDescription::Consumer {
            consumed: contract,
            producer: FrozenProducerDescription::new(
                contract,
                "export function answer(){return 7}",
                &[3, 0, 0, 0, 0, 0],
                RewriteDescription::default(),
                None,
                None,
                provenance,
            ),
        };
        let right = ResourceDescription::Consumer {
            consumed: contract,
            producer: FrozenProducerDescription::new(
                contract,
                "export function answer(){return 7}",
                &[3, 0, 1, 0, 0, 0],
                RewriteDescription::default(),
                None,
                None,
                provenance,
            ),
        };
        assert_eq!(compare(left, left, &mut budget).unwrap(), Ordering::Equal);
        assert_ne!(compare(left, right, &mut budget).unwrap(), Ordering::Equal);
        let mut hashes = [super::super::HASH_OFFSET; 2];
        fingerprint(left, &mut hashes[0], &mut budget).unwrap();
        fingerprint(right, &mut hashes[1], &mut budget).unwrap();
        assert_ne!(hashes[0], hashes[1]);
    }
}
