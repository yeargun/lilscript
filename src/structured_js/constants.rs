//! Exact values attached to one immutable analysis revision. Literal payloads
//! stay in the program; only evaluated values have additional owned storage.
//! A handle proves a value, never permission to move its producing operation.

use super::{analysis::Facts, Expr, ExprId, Literal, Module, StringValue};
use ahash::AHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Known(pub(super) ExprId);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Work {
    pub evaluated_values: usize,
    pub owned_string_bytes: usize,
    pub materialized_string_bytes: usize,
    pub charged_work: usize,
    pub budget_stops: usize,
}

// These are evaluation bounds, not profitability thresholds. Exhaustion keeps
// the runtime operation. Input literals are borrowed and need no cache entry.
const MAX_RESULT_BYTES: usize = 16 * 1024;
const MAX_OWNED_BYTES: usize = 1024 * 1024;
const MAX_WORK: usize = 8 * 1024 * 1024;

#[derive(Debug, Default)]
pub(super) struct Values {
    evaluated: Option<AHashMap<ExprId, Literal>>,
    pub work: Work,
}

#[derive(Clone, Copy)]
pub(super) enum Text<'a> {
    Chunk(&'a StringValue),
    Known(Known),
    Integer(i64),
    Static(&'static str),
}

impl Values {
    pub fn get<'a>(&'a self, module: &'a Module, known: Known) -> &'a Literal {
        match &module.expressions[known.0.index()] {
            Expr::Literal(value) => value,
            _ => &self.evaluated.as_ref().expect("evaluated value owner")[&known.0],
        }
    }

    pub fn string<'a>(&'a self, module: &'a Module, facts: Facts) -> Option<&'a StringValue> {
        match self.get(module, facts.constant?) {
            Literal::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn join(&mut self, module: &Module, a: Option<Known>, b: Option<Known>) -> Option<Known> {
        let (a, b) = a.zip(b)?;
        if a == b {
            return Some(a);
        }
        let bytes = match (self.get(module, a), self.get(module, b)) {
            (Literal::String(a), Literal::String(b)) => {
                a.storage_bytes().saturating_add(b.storage_bytes())
            }
            (Literal::Number(left), Literal::Number(right)) => {
                return (left.to_bits() == right.to_bits()).then_some(a);
            }
            _ => 0,
        };
        if !self.charge(bytes) {
            return None;
        }
        (self.get(module, a) == self.get(module, b)).then_some(a)
    }

    pub fn charge(&mut self, work: usize) -> bool {
        if work > MAX_WORK.saturating_sub(self.work.charged_work) {
            self.work.budget_stops += 1;
            return false;
        }
        self.work.charged_work += work;
        true
    }

    pub fn evaluate_string(
        &mut self,
        id: ExprId,
        size_bound: usize,
        work_bound: usize,
        make: impl FnOnce(&Self) -> StringValue,
    ) -> Option<Known> {
        if self
            .evaluated
            .as_ref()
            .is_some_and(|values| values.contains_key(&id))
        {
            return Some(Known(id));
        }
        if size_bound > MAX_RESULT_BYTES || size_bound > self.remaining_bytes() {
            self.work.budget_stops += 1;
            return None;
        }
        if !self.charge(work_bound) {
            return None;
        }
        let value = make(self);
        debug_assert!(value.storage_bytes() <= size_bound);
        self.work.evaluated_values += 1;
        self.work.owned_string_bytes += value.storage_bytes();
        self.evaluated
            .get_or_insert_with(AHashMap::default)
            .insert(id, Literal::String(value));
        Some(Known(id))
    }

    /// Only exact primitive conversions are admitted. Integer ranges exclude
    /// negative zero and stay below the exponential-format threshold. General
    /// IEEE Number::toString remains a runtime operation.
    pub fn text(&self, module: &Module, facts: Facts) -> Option<Text<'static>> {
        // These singleton kinds already are exact values. They need neither
        // a payload nor a distinct identity at each occurrence.
        match facts.value {
            super::analysis::ValueKind::Null => return Some(Text::Static("null")),
            super::analysis::ValueKind::Undefined => return Some(Text::Static("undefined")),
            _ => {}
        }
        if let Some(known) = facts.constant {
            match self.get(module, known) {
                Literal::String(_) => return Some(Text::Known(known)),
                Literal::Bool(value) => {
                    return Some(Text::Static(if *value { "true" } else { "false" }));
                }
                Literal::Null => return Some(Text::Static("null")),
                Literal::Undefined => return Some(Text::Static("undefined")),
                Literal::Number(_) => {}
            }
        }
        facts
            .integer
            .filter(|range| range.minimum == range.maximum)
            .map(|range| Text::Integer(range.minimum))
    }

    pub fn text_value(&self, module: &Module, text: Text<'_>) -> StringValue {
        match text {
            Text::Chunk(value) => value.clone(),
            Text::Known(known) => match self.get(module, known) {
                Literal::String(value) => value.clone(),
                _ => unreachable!("text conversion retains a proven string"),
            },
            Text::Integer(value) => value.to_string().into(),
            Text::Static(value) => value.into(),
        }
    }

    fn remaining_bytes(&self) -> usize {
        MAX_OWNED_BYTES
            .saturating_sub(self.work.owned_string_bytes)
            .saturating_sub(self.work.materialized_string_bytes)
    }

    fn materialize_bytes(&mut self, bytes: usize) -> bool {
        if bytes > MAX_RESULT_BYTES || bytes > self.remaining_bytes() {
            self.work.budget_stops += 1;
            return false;
        }
        if !self.charge(bytes) {
            return false;
        }
        self.work.materialized_string_bytes += bytes;
        true
    }

    pub fn materialize(&mut self, module: &Module, facts: Facts) -> Option<Literal> {
        let known = facts.constant?;
        if let Literal::String(value) = self.get(module, known) {
            if !self.materialize_bytes(value.storage_bytes()) {
                return None;
            }
        }
        Some(self.get(module, known).clone())
    }

    pub fn materialize_text(&mut self, module: &Module, facts: Facts) -> Option<StringValue> {
        let text = self.text(module, facts)?;
        let bytes = self.text_size(module, text);
        self.materialize_bytes(bytes)
            .then(|| self.text_value(module, text))
    }

    fn text_size(&self, module: &Module, text: Text<'_>) -> usize {
        match text {
            Text::Chunk(value) => value.storage_bytes(),
            Text::Known(known) => match self.get(module, known) {
                Literal::String(value) => value.storage_bytes(),
                _ => unreachable!("text conversion retains a proven string"),
            },
            Text::Integer(_) => 20,
            Text::Static(value) => value.len(),
        }
    }

    pub fn template(&mut self, module: &Module, id: ExprId, parts: &[Text<'_>]) -> Option<Known> {
        let mut size = 0usize;
        let mut work = 0usize;
        for &part in parts {
            size = size.saturating_add(self.text_size(module, part));
            // A join that changes UTF-16 storage may revisit its prefix.
            work = work.saturating_add(size);
        }
        self.evaluate_string(
            id,
            size.saturating_mul(2),
            work.saturating_mul(2),
            |values| {
                let mut result = StringValue::default();
                for &part in parts {
                    match part {
                        Text::Chunk(value) => result.push(value),
                        Text::Known(known) => match values.get(module, known) {
                            Literal::String(value) => result.push(value),
                            _ => unreachable!("text conversion retains a proven string"),
                        },
                        part => result.push(&values.text_value(module, part)),
                    }
                }
                result
            },
        )
    }
}
