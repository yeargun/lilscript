//! A coherent field-name assignment for unobserved private object layouts.
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
    inherited: usize,
    first: usize,
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
            classes[definition.identity.index()] = Class {
                present: true,
                blocked: definition.external
                    || definition.reflected
                    || definition.observed
                    || definition.published
                    || !definition.type_params.is_empty(),
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
        let mut next = budget.filled(Scratch, count, 0usize)?;
        for index in 0..count {
            budget.work(WorkKind::Analysis, 1)?;
            let root = parents[index];
            classes[index].blocked = blocked[root];
            if !classes[index].present || blocked[root] {
                continue;
            }
            // Extern layouts keep only their own fields. Only an eligible
            // internal family has the flattened, base-first slot convention.
            if let Some(base) = classes[index].base {
                if classes[index].fields < classes[base].fields {
                    return Err(AllocationError::Capacity);
                }
                classes[index].inherited = classes[base].fields;
            }
            // Siblings reserve distinct slots; unrelated families reuse them.
            // This order depends only on checked identities, not allocation or
            // traversal order in a particular target representation.
            classes[index].first = next[root];
            next[root] = next[root]
                .checked_add(classes[index].fields - classes[index].inherited)
                .ok_or(AllocationError::Capacity)?;
        }
        let mut ordinals = Vec::new();
        if !preserved.is_empty() {
            let count = next.iter().copied().max().unwrap_or(0);
            ordinals = budget.vector(Scratch, count)?;
            let mut ordinal = 0usize;
            while ordinals.len() != count {
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
        discard(next, budget)?;
        Ok(Self { classes, ordinals })
    }

    pub(super) fn slot(
        &self,
        field: FieldRef,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<usize>, AllocationError> {
        let mut owner = field.nominal.index();
        let slot = field.slot as usize;
        for _ in 0..self.classes.len() {
            budget.work(WorkKind::Analysis, 1)?;
            let class = self
                .classes
                .get(owner)
                .filter(|c| c.present && slot < c.fields)
                .ok_or(AllocationError::Capacity)?;
            if class.blocked {
                return Ok(None);
            }
            if slot >= class.inherited {
                let slot = class.first + (slot - class.inherited);
                return Ok(Some(if self.ordinals.is_empty() {
                    slot
                } else {
                    self.ordinals[slot]
                }));
            }
            owner = class.base.ok_or(AllocationError::Capacity)?;
        }
        Err(AllocationError::Capacity)
    }

    /// The same base54 sequence as the lexical allocator. Property keys may
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

fn spelling_bytes(mut slot: usize, bytes: &mut [u8; usize::BITS as usize]) -> &[u8] {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ$_";
    let mut length = 0;
    loop {
        bytes[length] = ALPHABET[slot % ALPHABET.len()];
        length += 1;
        slot /= ALPHABET.len();
        if slot == 0 {
            break;
        }
        slot -= 1;
    }
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
