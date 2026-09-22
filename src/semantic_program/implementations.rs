//! Immutable semantic implementation choices owned by retained JS candidates.
//!
//! Family analysis and policy admission happen in the compilation owner. This
//! map stores neither target bindings nor emitted names. Private Arc ownership
//! shares already charged evidence; callers can borrow evidence but cannot
//! retain an uncharged clone. The compilation owner enforces same-ledger use
//! and consuming discard. Ordinary Drop conservatively leaves memory charged.

use super::fixed_resource::{union_resource, ResourceChoice};
use super::function_layout::FunctionLayout;
use super::helper_family::HelperFamily;
use super::product_family::ProductFamily;
use super::record_family::RecordFamily;
use super::string_family::{StringChoice, StringFamily};
use super::uses::UseIndex;
use super::Program;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, RuntimeRisk, TacticId, TacticUse, WorkDomain, WorkKind,
};
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};
use std::cmp::Ordering;
use std::mem::size_of;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImplementationError {
    DuplicateRoot,
    ConflictingChoice,
    Capacity,
    AllocationFailed,
    Budget(BudgetError),
}

fn work(budget: &mut AllocationBudget<'_>, amount: usize) -> Result<(), ImplementationError> {
    budget.work(
        WorkKind::Edit,
        u64::try_from(amount).map_err(|_| ImplementationError::Capacity)?,
    )?;
    Ok(())
}

/// Both inputs are privately maintained in increasing semantic-root order.
/// The same walk counts, validates, and finally copies immutable references;
/// no proof is cloned and no root is compared by Arc allocation identity.
fn merge<T, K: Ord>(
    left: &[Arc<T>],
    right: &[Arc<T>],
    key: impl Fn(&T) -> K,
    same: impl Fn(&T, &T, &mut AllocationBudget<'_>) -> Result<bool, ImplementationError>,
    budget: &mut AllocationBudget<'_>,
    mut visit: impl FnMut(&Arc<T>, &mut AllocationBudget<'_>) -> Result<(), ImplementationError>,
) -> Result<usize, ImplementationError> {
    let (mut l, mut r, mut count) = (0usize, 0usize, 0usize);
    while l < left.len() || r < right.len() {
        work(budget, 1)?;
        let ordering = match (left.get(l), right.get(r)) {
            (Some(a), Some(b)) => key(a).cmp(&key(b)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => unreachable!(),
        };
        let selected = match ordering {
            Ordering::Less => {
                let selected = &left[l];
                l += 1;
                selected
            }
            Ordering::Greater => {
                let selected = &right[r];
                r += 1;
                selected
            }
            Ordering::Equal => {
                if !same(&left[l], &right[r], budget)? {
                    return Err(ImplementationError::ConflictingChoice);
                }
                let selected = &left[l];
                l += 1;
                r += 1;
                selected
            }
        };
        count = count.checked_add(1).ok_or(ImplementationError::Capacity)?;
        visit(selected, budget)?;
    }
    Ok(count)
}

fn same_record(
    left: &SharedRecord,
    right: &SharedRecord,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, ImplementationError> {
    work(budget, 1)?;
    // The current closed scalar recipe is fixed by this checked source root.
    Ok(left.family.root() == right.family.root()
        && left.family.allocation() == right.family.allocation())
}
fn same_helper(
    left: &SharedHelper,
    right: &SharedHelper,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, ImplementationError> {
    work(budget, 1)?;
    Ok(left.family.root() == right.family.root())
}
fn same_function(
    left: &SharedFunction,
    right: &SharedFunction,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, ImplementationError> {
    work(budget, 1)?;
    let (a, b) = (&left.family, &right.family);
    if a.body() != b.body() || a.parameters().len() != b.parameters().len() {
        return Ok(false);
    }
    for (a, b) in a.parameters().iter().zip(b.parameters()) {
        work(budget, 1)?;
        if a.position != b.position || a.schema != b.schema {
            return Ok(false);
        }
    }
    Ok(true)
}
fn same_product(
    left: &SharedProduct,
    right: &SharedProduct,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, ImplementationError> {
    work(budget, 1)?;
    let (a, b) = (&left.family, &right.family);
    if a.root() != b.root()
        || a.schema() != b.schema()
        || a.cells().len() != b.cells().len()
        || a.fields().len() != b.fields().len()
    {
        return Ok(false);
    }
    for (a, b) in a.cells().iter().zip(b.cells()) {
        work(budget, 1)?;
        if a.cell != b.cell {
            return Ok(false);
        }
    }
    for (a, b) in a.fields().iter().zip(b.fields()) {
        work(budget, 1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn same_string(
    left: &SharedString,
    right: &SharedString,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, ImplementationError> {
    work(budget, 1)?;
    if left.family.choice() != right.family.choice()
        || left.family.definitions().len() != right.family.definitions().len()
    {
        return Ok(false);
    }
    for (a, b) in left
        .family
        .definitions()
        .iter()
        .zip(right.family.definitions())
    {
        work(budget, 1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
impl From<AllocationError> for ImplementationError {
    fn from(error: AllocationError) -> Self {
        match error {
            AllocationError::Budget(error) => Self::Budget(error),
            AllocationError::Capacity => Self::Capacity,
            AllocationError::AllocationFailed => Self::AllocationFailed,
            AllocationError::Unaccounted | AllocationError::WrongOwner => {
                unreachable!("implementation maps always borrow their compilation ledger")
            }
        }
    }
}
impl From<BudgetError> for ImplementationError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}

#[derive(Debug, Clone, Copy)]
struct Charge {
    domain: WorkDomain,
    bytes: u64,
}

#[derive(Debug)]
struct SharedRecord {
    family: RecordFamily,
    /// Only wrapper/control storage: family construction already paid for its
    /// inline evidence and vector payload, possibly in a different work domain.
    charge: Charge,
}

#[derive(Debug)]
struct SharedHelper {
    family: HelperFamily,
    charge: Charge,
}

#[derive(Debug)]
pub(super) struct SharedFunction {
    family: FunctionLayout,
    charge: Charge,
}

/// A retained reference to the existing selected function proof owner. Physical
/// exports and implementation maps share this same evidence; no copied query
/// result or auxiliary cache is introduced. The private handle has no Clone.
#[derive(Debug)]
#[must_use = "retain in the compilation or discard through its original ledger"]
pub(super) struct FunctionEvidence(Arc<SharedFunction>);
impl FunctionEvidence {
    pub(super) fn borrow(&self) -> &FunctionLayout {
        &self.0.family
    }
    pub(super) fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub(super) fn retained_bytes(&self) -> u64 {
        self.0.charge.bytes + self.0.family.retained_bytes()
    }
    pub(super) fn owns_shared(&self, owner: &Arc<SharedFunction>) -> bool {
        Arc::ptr_eq(&self.0, owner)
    }
    /// Packed exports still require the complete proof. Its original family
    /// charge moves into the same private wrapper used by selected layouts.
    pub(super) fn from_family(
        family: FunctionLayout,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let prepared = (|| {
            let mut phase = budget.scope();
            phase.work(WorkKind::Edit, 1)?;
            let bytes = (size_of::<SharedFunction>() - size_of::<FunctionLayout>()) as u64
                + 2 * size_of::<usize>() as u64;
            phase.retain(Retained, bytes)?;
            let charge = phase.detach_retained((), bytes)?;
            let domain = charge.domain();
            // Transfer to the existing function wrapper's consuming protocol.
            drop(charge);
            Ok::<_, AllocationError>(Charge { domain, bytes })
        })();
        let charge = match prepared {
            Ok(charge) => charge,
            Err(error) => {
                budget.with_ledger(|owner| family.discard(owner.unwrap().0))?;
                return Err(error);
            }
        };
        Ok(Self(Arc::new(SharedFunction { family, charge })))
    }
    pub(super) fn discard(self, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
        Self::discard_shared(self.0, ledger)
    }
    fn discard_shared(
        shared: Arc<SharedFunction>,
        ledger: &mut BudgetLedger,
    ) -> Result<(), BudgetError> {
        if let Ok(SharedFunction { family, charge }) = Arc::try_unwrap(shared) {
            family.discard(ledger)?;
            ledger.release(charge.domain, charge.bytes)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct SharedProduct {
    family: ProductFamily,
    charge: Charge,
}

#[derive(Debug)]
struct SharedString {
    family: StringFamily,
    charge: Charge,
}
type MapArrays = (
    Vec<Arc<SharedRecord>>,
    Vec<Arc<SharedHelper>>,
    Vec<Arc<SharedString>>,
    Vec<Arc<SharedProduct>>,
    Vec<Arc<SharedFunction>>,
);

#[must_use = "retain choices in their compilation owner or discard through the original ledger"]
#[derive(Debug)]
pub(super) struct ImplementationMap {
    records: Vec<Arc<SharedRecord>>,
    helpers: Vec<Arc<SharedHelper>>,
    strings: Vec<Arc<SharedString>>,
    products: Vec<Arc<SharedProduct>>,
    functions: Vec<Arc<SharedFunction>>,
    /// A copied provenance summary, maintained only by the private constructor
    /// and insertion path. Policy queries never rescan retained definitions.
    pooled_strings: bool,
    resource: Option<ResourceChoice>,
    charge: Charge,
}

impl ImplementationMap {
    /// The direct route allocates no optional choice/evidence storage.
    pub(super) fn direct() -> Self {
        Self {
            records: Vec::new(),
            helpers: Vec::new(),
            strings: Vec::new(),
            products: Vec::new(),
            functions: Vec::new(),
            pooled_strings: false,
            resource: None,
            charge: Charge {
                domain: WorkDomain::Baseline,
                bytes: 0,
            },
        }
    }

    pub(super) fn records(&self) -> impl ExactSizeIterator<Item = &RecordFamily> {
        self.records.iter().map(|record| &record.family)
    }

    pub(super) fn helpers(&self) -> impl ExactSizeIterator<Item = &HelperFamily> {
        self.helpers.iter().map(|helper| &helper.family)
    }

    /// Borrow canonical selected evidence. The caller admits the binary
    /// lookup before querying; this creates no auxiliary membership index.
    pub(super) fn helper_for_cell(&self, cell: super::CellId) -> Option<&HelperFamily> {
        self.helpers
            .binary_search_by_key(&cell, |helper| helper.family.root().cell)
            .ok()
            .map(|index| &self.helpers[index].family)
    }

    pub(super) fn strings(&self) -> impl ExactSizeIterator<Item = &StringFamily> {
        self.strings.iter().map(|string| &string.family)
    }

    pub(super) fn functions(&self) -> impl ExactSizeIterator<Item = &FunctionLayout> {
        self.functions.iter().map(|function| &function.family)
    }

    fn function_position(
        &self,
        body: super::UnitId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<usize>, AllocationError> {
        let work = u64::from(usize::BITS - self.functions.len().leading_zeros()) + 1;
        budget.work(WorkKind::Analysis, work)?;
        Ok(self
            .functions
            .binary_search_by_key(&body, |entry| entry.family.body())
            .ok())
    }

    /// Lookup and admission share the existing sorted recipe owner. The exact
    /// requested body matters when a required input already selects other forms.
    pub(super) fn function_for_body(
        &self,
        body: super::UnitId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<&FunctionLayout>, AllocationError> {
        Ok(self
            .function_position(body, budget)?
            .map(|index| &self.functions[index].family))
    }

    /// The immutable candidate already owns complete source-qualified evidence.
    /// The same paid lookup can retain it for another client without a query.
    pub(super) fn share_function(
        &self,
        body: super::UnitId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<FunctionEvidence>, AllocationError> {
        Ok(self
            .function_position(body, budget)?
            .map(|index| FunctionEvidence(Arc::clone(&self.functions[index]))))
    }

    /// Product components are ordered by their canonical first cell, which may
    /// differ from the requested occurrence. Use their already sorted member
    /// lists to find the exact covering proof without another membership index.
    pub(super) fn product_for_cell(
        &self,
        cell: super::CellId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<&ProductFamily>, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        for product in &self.products {
            let cells = product.family.cells();
            let work = u64::from(usize::BITS - cells.len().leading_zeros()) + 1;
            budget.work(WorkKind::Analysis, work)?;
            if cells
                .binary_search_by_key(&cell, |member| member.cell)
                .is_ok()
            {
                return Ok(Some(&product.family));
            }
        }
        Ok(None)
    }

    pub(super) fn products(&self) -> impl ExactSizeIterator<Item = &ProductFamily> {
        self.products.iter().map(|product| &product.family)
    }

    pub(super) fn is_direct(&self) -> bool {
        self.records.is_empty()
            && self.helpers.is_empty()
            && self.strings.is_empty()
            && self.products.is_empty()
            && self.functions.is_empty()
            && self.resource.is_none()
    }

    pub(super) fn owns_shell(&self) -> bool {
        self.charge.bytes != 0
    }

    pub(super) fn resource(&self) -> Option<&ResourceChoice> {
        self.resource.as_ref()
    }

    /// Allocate/charge a complete map shell even when its only choice will be
    /// a resource. The future resource is installed only after all admissions.
    pub(super) fn prepare_resource_map(
        &self,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, ImplementationError> {
        self.charge_sharing_work(
            self.records.len(),
            self.helpers.len(),
            self.strings.len(),
            self.products.len(),
            self.functions.len(),
            ledger,
            domain,
        )?;
        let (mut map, _) = self.allocate_shared(
            self.records.len(),
            self.helpers.len(),
            self.strings.len(),
            self.products.len(),
            self.functions.len(),
            0,
            ledger,
            domain,
        )?;
        if let Some(resource) = map.resource.take() {
            let mut budget = AllocationBudget::new(Some((ledger, domain)));
            resource
                .discard(&mut budget)
                .unwrap_or_else(|error| panic!("shared resource owner invariant: {error:?}"));
        }
        Ok(map)
    }
    pub(super) fn install_resource_prepared(&mut self, resource: ResourceChoice) {
        assert!(self.resource.is_none() && self.charge.bytes >= size_of::<Self>() as u64);
        self.resource = Some(resource);
    }
    pub(super) fn validate_resource(
        &self,
        identity: super::RevisionId,
        program: &Program<'_>,
        uses: &UseIndex,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        match &self.resource {
            None => Ok(true),
            Some(resource) => resource.validate(identity, program, uses, self, budget),
        }
    }

    /// Neutral is this recipe's risk class, not a measured runtime guarantee.
    /// The owner still checks tactic permissions and candidate cost constraints.
    pub(super) fn tactics(&self) -> &[TacticUse] {
        const CALL: TacticUse = TacticUse {
            tactic: TacticId::CallSpecialization,
            risk: RuntimeRisk::Neutral,
        };
        const SCALAR: TacticUse = TacticUse {
            tactic: TacticId::ScalarReplacement,
            risk: RuntimeRisk::Neutral,
        };
        const INLINE: TacticUse = TacticUse {
            tactic: TacticId::Inlining,
            risk: RuntimeRisk::Neutral,
        };
        const FOLD: TacticUse = TacticUse {
            tactic: TacticId::ConstantFolding,
            risk: RuntimeRisk::Neutral,
        };
        const POOL: TacticUse = TacticUse {
            tactic: TacticId::StringPooling,
            risk: RuntimeRisk::Neutral,
        };
        let mask = u8::from(!self.records.is_empty() || !self.products.is_empty())
            | (u8::from(!self.helpers.is_empty()) << 1)
            | (u8::from(!self.strings.is_empty()) << 2)
            | (u8::from(self.pooled_strings) << 3)
            | (u8::from(!self.functions.is_empty()) << 4);
        // The closed recipes have fixed provenance. No dynamic tactic list or
        // per-query family scan is needed; sharing keeps the same summary.
        match mask {
            0 => &[],
            1 => &[SCALAR],
            2 => &[INLINE],
            3 => &[SCALAR, INLINE],
            4 => &[FOLD],
            5 => &[SCALAR, FOLD],
            6 => &[INLINE, FOLD],
            7 => &[SCALAR, INLINE, FOLD],
            12 => &[FOLD, POOL],
            13 => &[SCALAR, FOLD, POOL],
            14 => &[INLINE, FOLD, POOL],
            15 => &[SCALAR, INLINE, FOLD, POOL],
            16 => &[CALL],
            17 => &[SCALAR, CALL],
            18 => &[INLINE, CALL],
            19 => &[SCALAR, INLINE, CALL],
            20 => &[FOLD, CALL],
            21 => &[SCALAR, FOLD, CALL],
            22 => &[INLINE, FOLD, CALL],
            23 => &[SCALAR, INLINE, FOLD, CALL],
            28 => &[FOLD, POOL, CALL],
            29 => &[SCALAR, FOLD, POOL, CALL],
            30 => &[INLINE, FOLD, POOL, CALL],
            31 => &[SCALAR, INLINE, FOLD, POOL, CALL],
            _ => unreachable!("pooling requires a retained string family"),
        }
    }

    /// Retain choices for another candidate/checkpoint without copying evidence.
    /// The owner validates dependencies and current permissions independently.
    pub(super) fn share(
        &self,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, ImplementationError> {
        if self.is_direct() {
            return Ok(Self::direct());
        }
        self.charge_sharing_work(
            self.records.len(),
            self.helpers.len(),
            self.strings.len(),
            self.products.len(),
            self.functions.len(),
            ledger,
            domain,
        )?;
        self.allocate_shared(
            self.records.len(),
            self.helpers.len(),
            self.strings.len(),
            self.products.len(),
            self.functions.len(),
            0,
            ledger,
            domain,
        )
        .map(|(map, _)| map)
    }

    /// Union complete choices already validated against one semantic snapshot.
    /// Compilation checks snapshot identity and policy before this operation.
    /// Identical recipes share one input's evidence; incompatible choices fail
    /// without consuming either input or retaining any new evidence.
    pub(super) fn union(
        &self,
        other: &Self,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, ImplementationError> {
        if self.is_direct() {
            return other.share(ledger, domain);
        }
        if other.is_direct() {
            return self.share(ledger, domain);
        }
        let mut budget = AllocationBudget::new(Some((ledger, domain)));
        let resource =
            union_resource(self.resource.as_ref(), other.resource.as_ref(), &mut budget)?;
        // Each input is a valid independent choice over this same snapshot.
        // Inlining the fixed exported endpoint removes the physical function
        // required by the resource cut. This is a choice conflict, not stale
        // source evidence, and must refuse before any output map allocation.
        if let Some(resource) = resource {
            let endpoint = resource.export().cell();
            for selected in [self, other] {
                if selected.helpers.is_empty() {
                    continue;
                }
                work(
                    &mut budget,
                    (usize::BITS - selected.helpers.len().leading_zeros()) as usize + 1,
                )?;
                if selected.helper_for_cell(endpoint).is_some() {
                    return Err(ImplementationError::ConflictingChoice);
                }
            }
        }
        let records = merge(
            &self.records,
            &other.records,
            |entry| entry.family.root().state,
            same_record,
            &mut budget,
            |_, _| Ok(()),
        )?;
        let helpers = merge(
            &self.helpers,
            &other.helpers,
            |entry| entry.family.root().cell,
            same_helper,
            &mut budget,
            |_, _| Ok(()),
        )?;
        let functions = merge(
            &self.functions,
            &other.functions,
            |entry| entry.family.body(),
            same_function,
            &mut budget,
            |_, _| Ok(()),
        )?;
        let mut covered = 0usize;
        let products = merge(
            &self.products,
            &other.products,
            |entry| entry.family.root(),
            same_product,
            &mut budget,
            |entry, _| {
                covered = covered
                    .checked_add(entry.family.cells().len())
                    .ok_or(ImplementationError::Capacity)?;
                Ok(())
            },
        )?;
        let mut cells = budget.vector(Scratch, covered)?;
        merge(
            &self.products,
            &other.products,
            |entry| entry.family.root(),
            same_product,
            &mut budget,
            |entry, budget| {
                work(budget, entry.family.cells().len())?;
                cells.extend(entry.family.cells().iter().map(|cell| cell.cell));
                Ok(())
            },
        )?;
        work(
            &mut budget,
            covered
                .checked_mul((usize::BITS - covered.max(1).leading_zeros()) as usize)
                .ok_or(ImplementationError::Capacity)?,
        )?;
        cells.sort_unstable();
        for pair in cells.windows(2) {
            work(&mut budget, 1)?;
            if pair[0] == pair[1] {
                return Err(ImplementationError::ConflictingChoice);
            }
        }
        let bytes = cells
            .capacity()
            .checked_mul(size_of::<super::CellId>())
            .ok_or(ImplementationError::Capacity)?;
        drop(cells);
        budget.release(Scratch, bytes as u64)?;
        let mut definitions = 0usize;
        let strings = merge(
            &self.strings,
            &other.strings,
            |entry| entry.family.definitions()[0],
            same_string,
            &mut budget,
            |entry, _| {
                definitions = definitions
                    .checked_add(entry.family.definitions().len())
                    .ok_or(ImplementationError::Capacity)?;
                Ok(())
            },
        )?;

        // Group minima do not prove disjointness: {a,c} and {b,c} interleave.
        // Identical full groups were deduplicated above. A duplicate in this
        // temporary flat set now means conflicting physical storage choices.
        let mut values = budget.vector(Scratch, definitions)?;
        merge(
            &self.strings,
            &other.strings,
            |entry| entry.family.definitions()[0],
            same_string,
            &mut budget,
            |entry, budget| {
                work(budget, entry.family.definitions().len())?;
                values.extend_from_slice(entry.family.definitions());
                Ok(())
            },
        )?;
        let sorting = definitions
            .checked_mul((usize::BITS - definitions.max(1).leading_zeros()) as usize)
            .ok_or(ImplementationError::Capacity)?;
        work(&mut budget, sorting)?;
        values.sort_unstable();
        for adjacent in values.windows(2) {
            work(&mut budget, 1)?;
            if adjacent[0] == adjacent[1] {
                return Err(ImplementationError::ConflictingChoice);
            }
        }
        let scratch_bytes = values
            .capacity()
            .checked_mul(size_of::<super::string_family::ValueRef>())
            .ok_or(ImplementationError::Capacity)?;
        drop(values);
        budget.release(
            Scratch,
            u64::try_from(scratch_bytes).map_err(|_| ImplementationError::Capacity)?,
        )?;

        let (
            mut selected_records,
            mut selected_helpers,
            mut selected_strings,
            mut selected_products,
            mut selected_functions,
        ) = Self::allocate_arrays(
            records,
            helpers,
            strings,
            products,
            functions,
            0,
            &mut budget,
        )?;
        merge(
            &self.records,
            &other.records,
            |entry| entry.family.root().state,
            same_record,
            &mut budget,
            |entry, _| {
                selected_records.push(Arc::clone(entry));
                Ok(())
            },
        )?;
        merge(
            &self.helpers,
            &other.helpers,
            |entry| entry.family.root().cell,
            same_helper,
            &mut budget,
            |entry, _| {
                selected_helpers.push(Arc::clone(entry));
                Ok(())
            },
        )?;
        merge(
            &self.strings,
            &other.strings,
            |entry| entry.family.definitions()[0],
            same_string,
            &mut budget,
            |entry, _| {
                selected_strings.push(Arc::clone(entry));
                Ok(())
            },
        )?;
        merge(
            &self.products,
            &other.products,
            |entry| entry.family.root(),
            same_product,
            &mut budget,
            |entry, _| {
                selected_products.push(Arc::clone(entry));
                Ok(())
            },
        )?;
        merge(
            &self.functions,
            &other.functions,
            |entry| entry.family.body(),
            same_function,
            &mut budget,
            |entry, _| {
                selected_functions.push(Arc::clone(entry));
                Ok(())
            },
        )?;
        let resource = resource
            .map(|resource| resource.share(&mut budget))
            .transpose()?;
        Self::finish_arrays(
            (
                selected_records,
                selected_helpers,
                selected_strings,
                selected_products,
                selected_functions,
            ),
            self.pooled_strings || other.pooled_strings,
            0,
            &mut budget,
        )
        .map(|(mut map, _)| {
            map.resource = resource;
            map
        })
    }

    /// Add one independently analyzed family. Failure consumes/discards the
    /// incoming evidence and releases every new reservation; the base survives.
    /// Stale-dependency validation and policy admission remain owner operations.
    pub(super) fn with_scalar(
        &self,
        family: RecordFamily,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, ImplementationError> {
        let prepared = (|| {
            let count = self
                .records
                .len()
                .checked_add(1)
                .ok_or(ImplementationError::Capacity)?;
            self.charge_sharing_work(
                count,
                self.helpers.len(),
                self.strings.len(),
                self.products.len(),
                self.functions.len(),
                ledger,
                domain,
            )?;
            let position = self
                .records
                .binary_search_by_key(&family.root().state, |record| record.family.root().state)
                .err()
                .ok_or(ImplementationError::DuplicateRoot)?;
            let wrapper_bytes = (size_of::<SharedRecord>() - size_of::<RecordFamily>()) as u64
                + 2 * size_of::<usize>() as u64;
            let (map, wrapper) = self.allocate_shared(
                count,
                self.helpers.len(),
                self.strings.len(),
                self.products.len(),
                self.functions.len(),
                wrapper_bytes,
                ledger,
                domain,
            )?;
            Ok((map, position, wrapper))
        })();
        let (mut map, position, wrapper) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                family.discard(ledger)?;
                return Err(error);
            }
        };
        map.records.insert(
            position,
            Arc::new(SharedRecord {
                family,
                charge: wrapper,
            }),
        );
        Ok(map)
    }

    pub(super) fn with_product(
        &self,
        family: ProductFamily,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, ImplementationError> {
        let prepared = (|| {
            let count = self
                .products
                .len()
                .checked_add(1)
                .ok_or(ImplementationError::Capacity)?;
            self.charge_sharing_work(
                self.records.len(),
                self.helpers.len(),
                self.strings.len(),
                count,
                self.functions.len(),
                ledger,
                domain,
            )?;
            for existing in &self.products {
                let (mut a, mut b) = (0, 0);
                while a < existing.family.cells().len() && b < family.cells().len() {
                    ledger.charge(domain, WorkKind::Edit, 1)?;
                    match existing.family.cells()[a].cell.cmp(&family.cells()[b].cell) {
                        Ordering::Less => a += 1,
                        Ordering::Greater => b += 1,
                        Ordering::Equal => return Err(ImplementationError::DuplicateRoot),
                    }
                }
            }
            let position = self
                .products
                .binary_search_by_key(&family.root(), |record| record.family.root())
                .err()
                .ok_or(ImplementationError::DuplicateRoot)?;
            let wrapper_bytes = (size_of::<SharedProduct>() - size_of::<ProductFamily>()) as u64
                + 2 * size_of::<usize>() as u64;
            let (map, wrapper) = self.allocate_shared(
                self.records.len(),
                self.helpers.len(),
                self.strings.len(),
                count,
                self.functions.len(),
                wrapper_bytes,
                ledger,
                domain,
            )?;
            Ok((map, position, wrapper))
        })();
        let (mut map, position, wrapper) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                family.discard(ledger)?;
                return Err(error);
            }
        };
        map.products.insert(
            position,
            Arc::new(SharedProduct {
                family,
                charge: wrapper,
            }),
        );
        Ok(map)
    }

    pub(super) fn with_function(
        &self,
        family: FunctionLayout,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, ImplementationError> {
        let prepared = (|| {
            let count = self
                .functions
                .len()
                .checked_add(1)
                .ok_or(ImplementationError::Capacity)?;
            self.charge_sharing_work(
                self.records.len(),
                self.helpers.len(),
                self.strings.len(),
                self.products.len(),
                count,
                ledger,
                domain,
            )?;
            let position = self
                .functions
                .binary_search_by_key(&family.body(), |entry| entry.family.body())
                .err()
                .ok_or(ImplementationError::DuplicateRoot)?;
            let wrapper_bytes = (size_of::<SharedFunction>() - size_of::<FunctionLayout>()) as u64
                + 2 * size_of::<usize>() as u64;
            let (map, wrapper) = self.allocate_shared(
                self.records.len(),
                self.helpers.len(),
                self.strings.len(),
                self.products.len(),
                count,
                wrapper_bytes,
                ledger,
                domain,
            )?;
            Ok((map, position, wrapper))
        })();
        let (mut map, position, wrapper) = match prepared {
            Ok(value) => value,
            Err(error) => {
                family.discard(ledger)?;
                return Err(error);
            }
        };
        map.functions.insert(
            position,
            Arc::new(SharedFunction {
                family,
                charge: wrapper,
            }),
        );
        Ok(map)
    }

    /// Every recipe vector participates in each sibling. Adding helper
    /// expansion retains the selected scalar layouts and string representations.
    pub(super) fn with_inline_helper(
        &self,
        family: HelperFamily,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, ImplementationError> {
        let prepared = (|| {
            // A newly proved helper and a union of pre-existing choices obey
            // the same fixed endpoint requirement before map allocation.
            if let Some(resource) = &self.resource {
                ledger.charge(domain, WorkKind::Edit, 1)?;
                if family.root().cell == resource.export().cell() {
                    return Err(ImplementationError::ConflictingChoice);
                }
            }
            let count = self
                .helpers
                .len()
                .checked_add(1)
                .ok_or(ImplementationError::Capacity)?;
            self.charge_sharing_work(
                self.records.len(),
                count,
                self.strings.len(),
                self.products.len(),
                self.functions.len(),
                ledger,
                domain,
            )?;
            let position = self
                .helpers
                .binary_search_by_key(&family.root().cell, |helper| helper.family.root().cell)
                .err()
                .ok_or(ImplementationError::DuplicateRoot)?;
            let wrapper_bytes = (size_of::<SharedHelper>() - size_of::<HelperFamily>()) as u64
                + 2 * size_of::<usize>() as u64;
            let (map, wrapper) = self.allocate_shared(
                self.records.len(),
                count,
                self.strings.len(),
                self.products.len(),
                self.functions.len(),
                wrapper_bytes,
                ledger,
                domain,
            )?;
            Ok((map, position, wrapper))
        })();
        let (mut map, position, wrapper) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                family.discard(ledger)?;
                return Err(error);
            }
        };
        map.helpers.insert(
            position,
            Arc::new(SharedHelper {
                family,
                charge: wrapper,
            }),
        );
        Ok(map)
    }

    /// Definitions are private sorted proof identities, not caller-provided
    /// equivalence claims. Every overlap rejects, including a partial overlap
    /// with a differently grouped literal/pooling choice.
    pub(super) fn with_string(
        &self,
        family: StringFamily,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, ImplementationError> {
        let prepared = (|| {
            let count = self
                .strings
                .len()
                .checked_add(1)
                .ok_or(ImplementationError::Capacity)?;
            self.charge_sharing_work(
                self.records.len(),
                self.helpers.len(),
                count,
                self.products.len(),
                self.functions.len(),
                ledger,
                domain,
            )?;
            let definitions = family.definitions();
            let root = *definitions.first().ok_or(ImplementationError::Capacity)?;
            for existing in &self.strings {
                let previous = existing.family.definitions();
                let (small, large) = if previous.len() < definitions.len() {
                    (previous, definitions)
                } else {
                    (definitions, previous)
                };
                let comparisons = (usize::BITS - large.len().leading_zeros()) as u64 + 1;
                let work = (small.len() as u64)
                    .checked_mul(comparisons)
                    .ok_or(ImplementationError::Capacity)?;
                ledger.charge(domain, WorkKind::Edit, work)?;
                if small
                    .iter()
                    .any(|definition| large.binary_search(definition).is_ok())
                {
                    return Err(ImplementationError::DuplicateRoot);
                }
            }
            let position = self
                .strings
                .binary_search_by_key(&root, |string| string.family.definitions()[0])
                .err()
                .ok_or(ImplementationError::DuplicateRoot)?;
            let wrapper_bytes = (size_of::<SharedString>() - size_of::<StringFamily>()) as u64
                + 2 * size_of::<usize>() as u64;
            let (map, wrapper) = self.allocate_shared(
                self.records.len(),
                self.helpers.len(),
                count,
                self.products.len(),
                self.functions.len(),
                wrapper_bytes,
                ledger,
                domain,
            )?;
            Ok((map, position, wrapper))
        })();
        let (mut map, position, wrapper) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                family.discard(ledger)?;
                return Err(error);
            }
        };
        map.pooled_strings |= matches!(family.choice(), StringChoice::SharedLiteral { .. });
        map.strings.insert(
            position,
            Arc::new(SharedString {
                family,
                charge: wrapper,
            }),
        );
        Ok(map)
    }

    fn charge_sharing_work(
        &self,
        record_count: usize,
        helper_count: usize,
        string_count: usize,
        product_count: usize,
        function_count: usize,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<(), ImplementationError> {
        let count = record_count
            .checked_add(helper_count)
            .and_then(|count| count.checked_add(string_count))
            .and_then(|count| count.checked_add(product_count))
            .and_then(|count| count.checked_add(function_count))
            .ok_or(ImplementationError::Capacity)?;
        let work = (count as u64)
            .checked_mul(3)
            .ok_or(ImplementationError::Capacity)?;
        // Covers search, the sharing walks and the insertion shift. String
        // definition-overlap comparisons have their own admission above.
        ledger.charge(domain, WorkKind::Edit, work)?;
        Ok(())
    }

    fn allocate_shared(
        &self,
        record_count: usize,
        helper_count: usize,
        string_count: usize,
        product_count: usize,
        function_count: usize,
        wrapper_bytes: u64,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<(Self, Charge), ImplementationError> {
        let mut budget = AllocationBudget::new(Some((ledger, domain)));
        let (mut records, mut helpers, mut strings, mut products, mut functions) =
            Self::allocate_arrays(
                record_count,
                helper_count,
                string_count,
                product_count,
                function_count,
                wrapper_bytes,
                &mut budget,
            )?;
        records.extend(self.records.iter().map(Arc::clone));
        helpers.extend(self.helpers.iter().map(Arc::clone));
        strings.extend(self.strings.iter().map(Arc::clone));
        products.extend(self.products.iter().map(Arc::clone));
        functions.extend(self.functions.iter().map(Arc::clone));
        let resource = self
            .resource
            .as_ref()
            .map(|resource| resource.share(&mut budget))
            .transpose()?;
        Self::finish_arrays(
            (records, helpers, strings, products, functions),
            self.pooled_strings,
            wrapper_bytes,
            &mut budget,
        )
        .map(|(mut map, wrapper)| {
            map.resource = resource;
            (map, wrapper)
        })
    }

    fn allocate_arrays(
        records: usize,
        helpers: usize,
        strings: usize,
        products: usize,
        functions: usize,
        wrapper_bytes: u64,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<MapArrays, ImplementationError> {
        // This one audited allocation owner establishes exact Vec capacities;
        // all source and destination backing coexist in the same ledger peak.
        budget.retain(
            Retained,
            (size_of::<Self>() as u64)
                .checked_add(wrapper_bytes)
                .ok_or(ImplementationError::Capacity)?,
        )?;
        Ok((
            budget.vector(Retained, records)?,
            budget.vector(Retained, helpers)?,
            budget.vector(Retained, strings)?,
            budget.vector(Retained, products)?,
            budget.vector(Retained, functions)?,
        ))
    }

    fn finish_arrays(
        (records, helpers, strings, products, functions): MapArrays,
        pooled_strings: bool,
        wrapper_bytes: u64,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(Self, Charge), ImplementationError> {
        let shell_bytes = Self::shell_bytes(
            records.capacity(),
            helpers.capacity(),
            strings.capacity(),
            products.capacity(),
            functions.capacity(),
        )?;
        let reserved = shell_bytes
            .checked_add(wrapper_bytes)
            .ok_or(ImplementationError::Capacity)?;
        // Transfer the common allocation scope's backing charge into the map's
        // existing consuming Charge protocol. The token itself has no Drop
        // release; map and optional Arc wrapper become the sole release owners.
        let charge = budget.detach_retained((), reserved)?;
        let domain = charge.domain();
        debug_assert_eq!(charge.bytes(), reserved);
        drop(charge);
        Ok((
            Self {
                records,
                helpers,
                strings,
                products,
                functions,
                pooled_strings,
                resource: None,
                charge: Charge {
                    domain,
                    bytes: shell_bytes,
                },
            },
            Charge {
                domain,
                bytes: wrapper_bytes,
            },
        ))
    }

    fn shell_bytes(
        record_count: usize,
        helper_count: usize,
        string_count: usize,
        product_count: usize,
        function_count: usize,
    ) -> Result<u64, ImplementationError> {
        (record_count as u64)
            .checked_mul(size_of::<Arc<SharedRecord>>() as u64)
            .and_then(|bytes| {
                (helper_count as u64)
                    .checked_mul(size_of::<Arc<SharedHelper>>() as u64)
                    .and_then(|helpers| bytes.checked_add(helpers))
            })
            .and_then(|bytes| {
                (string_count as u64)
                    .checked_mul(size_of::<Arc<SharedString>>() as u64)
                    .and_then(|strings| bytes.checked_add(strings))
            })
            .and_then(|bytes| {
                (product_count as u64)
                    .checked_mul(size_of::<Arc<SharedProduct>>() as u64)
                    .and_then(|products| bytes.checked_add(products))
            })
            .and_then(|bytes| {
                (function_count as u64)
                    .checked_mul(size_of::<Arc<SharedFunction>>() as u64)
                    .and_then(|functions| bytes.checked_add(functions))
            })
            .and_then(|bytes| bytes.checked_add(size_of::<Self>() as u64))
            .ok_or(ImplementationError::Capacity)
    }

    /// Bound revision checks, including one complete index freshness check.
    /// The owner charges this before calling valid_for; no bodies are rescanned.
    pub(super) fn validation_work(
        &self,
        program: &Program<'_>,
    ) -> Result<u64, ImplementationError> {
        self.published_validation_work()?
            .checked_add(program.units.len() as u64)
            .and_then(|work| work.checked_add(4))
            .ok_or(ImplementationError::Capacity)
    }

    pub(super) fn published_validation_work(&self) -> Result<u64, ImplementationError> {
        let records = self.records.iter().try_fold(1u64, |work, record| {
            work.checked_add(4)
                .and_then(|work| {
                    work.checked_add(record.family.dependencies().units().len() as u64)
                })
                .ok_or(ImplementationError::Capacity)
        })?;
        let helpers = self.helpers.iter().try_fold(records, |work, helper| {
            work.checked_add(
                helper
                    .family
                    .dependencies()
                    .validation_work()
                    .ok_or(ImplementationError::Capacity)?,
            )
            .ok_or(ImplementationError::Capacity)
        })?;
        let strings = self.strings.iter().try_fold(helpers, |work, string| {
            work.checked_add(
                string
                    .family
                    .dependencies()
                    .validation_work()
                    .ok_or(ImplementationError::Capacity)?,
            )
            .ok_or(ImplementationError::Capacity)
        })?;
        let products = self.products.iter().try_fold(strings, |work, product| {
            work.checked_add(
                product
                    .family
                    .dependencies()
                    .validation_work()
                    .ok_or(ImplementationError::Capacity)?,
            )
            .ok_or(ImplementationError::Capacity)
        })?;
        self.functions.iter().try_fold(products, |work, function| {
            work.checked_add(
                function
                    .family
                    .dependencies()
                    .validation_work()
                    .ok_or(ImplementationError::Capacity)?,
            )
            .ok_or(ImplementationError::Capacity)
        })
    }

    pub(super) fn valid_for(&self, program: &Program<'_>, uses: &UseIndex) -> bool {
        uses.valid_for(program) && self.valid_for_published(program, uses)
    }

    /// Publication already guarantees the checked Program/UseIndex pairing.
    /// Only its trusted route may skip the global revision-metadata scan.
    pub(super) fn valid_for_published(&self, program: &Program<'_>, uses: &UseIndex) -> bool {
        self.records.iter().all(|record| {
            record
                .family
                .dependencies()
                .valid_for_published(program, uses)
        }) && self.helpers.iter().all(|helper| {
            helper
                .family
                .dependencies()
                .valid_for_published(program, uses)
        }) && self.strings.iter().all(|string| {
            string
                .family
                .dependencies()
                .valid_for_published(program, uses)
        }) && self.functions.iter().all(|function| {
            function
                .family
                .dependencies()
                .valid_for_published(program, uses)
        }) && self.products.iter().all(|product| {
            product
                .family
                .dependencies()
                .valid_for_published(program, uses)
        })
    }

    /// Reachable payload, not a reservation to charge again for shared evidence.
    pub(super) fn retained_bytes(&self) -> u64 {
        self.charge.bytes
            + self
                .resource
                .as_ref()
                .map_or(0, ResourceChoice::retained_bytes)
            + self
                .functions
                .iter()
                .filter(|function| {
                    self.resource
                        .as_ref()
                        .is_none_or(|resource| !resource.contains_function_owner(function))
                })
                .map(|function| function.charge.bytes + function.family.retained_bytes())
                .sum::<u64>()
            + self
                .products
                .iter()
                .map(|product| product.charge.bytes + product.family.retained_bytes())
                .sum::<u64>()
            + self
                .records
                .iter()
                .map(|record| record.charge.bytes + record.family.retained_bytes())
                .sum::<u64>()
            + self
                .strings
                .iter()
                .map(|string| string.charge.bytes + string.family.retained_bytes())
                .sum::<u64>()
            + self
                .helpers
                .iter()
                .map(|helper| helper.charge.bytes + helper.family.retained_bytes())
                .sum::<u64>()
    }

    pub(super) fn discard(self, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
        let Self {
            records,
            helpers,
            strings,
            products,
            functions,
            pooled_strings: _,
            resource,
            charge,
        } = self;
        if let Some(resource) = resource {
            let mut budget = AllocationBudget::new(Some((ledger, WorkDomain::Baseline)));
            resource
                .discard(&mut budget)
                .unwrap_or_else(|error| panic!("fixed resource owner invariant: {error:?}"));
        }
        for record in records {
            if let Ok(SharedRecord { family, charge }) = Arc::try_unwrap(record) {
                family.discard(ledger)?;
                ledger.release(charge.domain, charge.bytes)?;
            }
        }
        for helper in helpers {
            if let Ok(SharedHelper { family, charge }) = Arc::try_unwrap(helper) {
                family.discard(ledger)?;
                ledger.release(charge.domain, charge.bytes)?;
            }
        }
        for string in strings {
            if let Ok(SharedString { family, charge }) = Arc::try_unwrap(string) {
                family.discard(ledger)?;
                ledger.release(charge.domain, charge.bytes)?;
            }
        }
        for product in products {
            if let Ok(SharedProduct { family, charge }) = Arc::try_unwrap(product) {
                family.discard(ledger)?;
                ledger.release(charge.domain, charge.bytes)?;
            }
        }
        for function in functions {
            FunctionEvidence::discard_shared(function, ledger)?;
        }
        ledger.release(charge.domain, charge.bytes)
    }
}
