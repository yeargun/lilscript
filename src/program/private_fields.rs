//! A coherent field-name assignment for proved private object layouts.
//! Identity, never an emitted spelling, joins a construction and its accesses.
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};

#[derive(Clone, Copy, Default)]
struct Class {
    present: bool,
    blocked: bool,
    base: Option<usize>,
    fields: usize,
}

pub(super) struct Plan {
    classes: Vec<Class>,
    /// Logical short-name slots mapped around reserved public spellings.
    /// Empty when there are no reserved names, so the common map is identity.
    ordinals: Vec<usize>,
}

fn root(
    parents: &mut [usize],
    start: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<usize, AllocationError> {
    let mut at = start;
    for _ in 0..parents.len() {
        budget.work(WorkKind::Analysis, 1)?;
        if parents[at] == at {
            let result = at;
            at = start;
            while parents[at] != at {
                budget.work(WorkKind::Analysis, 1)?;
                let next = parents[at];
                parents[at] = result;
                at = next;
            }
            return Ok(result);
        }
        at = parents[at];
    }
    Err(AllocationError::Capacity)
}

impl Plan {
    pub(super) fn new(
        program: &Program<'_>,
        preserved: &[String],
        pristine: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        budget.work(WorkKind::Analysis, program.classes.len() as u64)?;
        let count = program
            .classes
            .iter()
            .map(|c| c.identity.index() + 1)
            .max()
            .unwrap_or(0);
        let mut classes = budget.filled(Scratch, count, Class::default())?;
        let mut parents = budget.vector(Scratch, count)?;
        for index in 0..count {
            budget.work(WorkKind::Analysis, 1)?;
            parents.push(index);
        }
        for definition in program.classes.iter() {
            budget.work(WorkKind::Analysis, 1)?;
            // Kept classes initialize through assignments. Without the explicit
            // pristine contract an inherited setter could observe their keys.
            // Even pristine Object.prototype has the __proto__ setter.
            let mut prototype_observable = definition.observed && !pristine;
            if definition.observed && pristine {
                for &(key, _) in &definition.fields {
                    budget.work(WorkKind::Analysis, 1)?;
                    prototype_observable |=
                        program.strings[key.index()].as_unicode() == Some("__proto__");
                }
            }
            classes[definition.identity.index()] = Class {
                present: true,
                blocked: definition.external
                    || definition.reflected
                    || prototype_observable
                    || definition.published,
                base: definition.base.map(|base| base.index()),
                fields: definition.fields.len(),
                ..Class::default()
            };
        }
        for index in 0..count {
            budget.work(WorkKind::Analysis, 1)?;
            if let Some(base) = classes[index].base {
                let base = classes
                    .get(base)
                    .filter(|c| c.present)
                    .map(|_| base)
                    .ok_or(AllocationError::Capacity)?;
                let left = root(&mut parents, index, budget)?;
                let right = root(&mut parents, base, budget)?;
                parents[left] = right;
            }
        }
        let mut blocked = budget.filled(Scratch, count, false)?;
        for index in 0..count {
            let root = root(&mut parents, index, budget)?;
            blocked[root] |= classes[index].blocked;
        }
        let mut width = 0usize;
        for index in 0..count {
            budget.work(WorkKind::Analysis, 1)?;
            let root = parents[index];
            classes[index].blocked = blocked[root];
            if !classes[index].present || blocked[root] {
                continue;
            }
            // Flattened, base-first slots are the interference relation:
            // ancestors coexist on an instance; sibling-only fields do not.
            // The checked field identity still selects each access's slot.
            if let Some(base) = classes[index].base {
                if classes[index].fields < classes[base].fields {
                    return Err(AllocationError::Capacity);
                }
            }
            width = width.max(classes[index].fields);
        }
        let mut ordinals = Vec::new();
        if !preserved.is_empty() {
            ordinals = budget.vector(Scratch, width)?;
            let mut ordinal = 0usize;
            while ordinals.len() != width {
                let mut bytes = [0; usize::BITS as usize];
                let spelling = spelling_bytes(ordinal, &mut bytes);
                let mut reserved = false;
                for name in preserved {
                    budget.work(WorkKind::Analysis, name.len() as u64 + 1)?;
                    reserved |= name.as_bytes() == spelling;
                }
                if !reserved {
                    ordinals.push(ordinal);
                }
                ordinal = ordinal.checked_add(1).ok_or(AllocationError::Capacity)?;
            }
        }
        discard(parents, budget)?;
        discard(blocked, budget)?;
        Ok(Self { classes, ordinals })
    }

    pub(super) fn slot(
        &self,
        field: FieldRef,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<usize>, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        let slot = field.slot as usize;
        let class = self
            .classes
            .get(field.nominal.index())
            .filter(|class| class.present && slot < class.fields)
            .ok_or(AllocationError::Capacity)?;
        Ok((!class.blocked).then(|| {
            if self.ordinals.is_empty() {
                slot
            } else {
                self.ordinals[slot]
            }
        }))
    }

    /// Full legal identifier continuations, shared with lexical compaction. Keys may
    /// be keywords, and constructors create these as own data properties.
    pub(super) fn spelling(
        slot: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<String, AllocationError> {
        let mut bytes = [0; usize::BITS as usize];
        let spelling = spelling_bytes(slot, &mut bytes);
        budget.work(WorkKind::Render, spelling.len() as u64)?;
        budget.string(
            crate::output_budget::AllocationClass::Retained,
            std::str::from_utf8(spelling).expect("ASCII property alphabet"),
        )
    }
}

fn spelling_bytes(slot: usize, bytes: &mut [u8; usize::BITS as usize]) -> &[u8] {
    let length =
        crate::identifier_names::encode(slot, crate::identifier_names::ALPHABET, true, bytes);
    &bytes[..length]
}

fn discard<T>(value: Vec<T>, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
    let bytes = value
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(AllocationError::Capacity)?;
    drop(value);
    budget.release(Scratch, bytes as u64)
}
