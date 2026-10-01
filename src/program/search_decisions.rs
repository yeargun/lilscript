//! Persistent assignments contain choices, never trusted proof payloads.
//! Exact identities are checked by the build owner; reconstruction still uses
//! this compilation's current proof producers and ordinary optional ledger.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SavedDecision {
    pub recipe: Vec<u32>,
    pub output: OutputTactics,
    pub naming: Plan,
}

impl SavedDecision {
    pub(crate) fn capture(
        description: ImplementationDescription<'_>,
        output: &OutputTactics,
        naming: &Plan,
    ) -> Option<Self> {
        // A word recipe is incomplete when a publication rewrite changed the
        // meaning. Never silently drop that lineage when exporting a lock.
        Some(Self {
            recipe: description.whole_words()?.to_vec(),
            output: output.clone(),
            naming: naming.clone(),
        })
    }
}

struct Words<'a>(&'a [u32]);
impl<'a> Words<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u32], CandidateError> {
        let (head, rest) = self
            .0
            .split_at_checked(count)
            .ok_or(CandidateError::InvalidRequest)?;
        self.0 = rest;
        Ok(head)
    }
    fn word(&mut self) -> Result<u32, CandidateError> {
        Ok(self.take(1)?[0])
    }
    fn rows(&mut self, width: usize) -> Result<&'a [u32], CandidateError> {
        let count = self.word()? as usize;
        self.take(
            count
                .checked_mul(width)
                .ok_or(CandidateError::InvalidRequest)?,
        )
    }
}

enum Hint<'a> {
    Record(u32),
    Helper(u32),
    String {
        shared: bool,
        activation: u32,
        definitions: &'a [u32],
    },
    Product(u32),
    Function(u32),
}

fn visit(
    recipe: &[u32],
    mut apply: impl FnMut(Hint<'_>) -> Result<(), CandidateError>,
) -> Result<(), CandidateError> {
    let mut words = Words(recipe);
    if words.word()? != crate::program::implementation_identity::FORMAT {
        return Err(CandidateError::InvalidRequest);
    }
    for row in words.rows(4)?.chunks_exact(4) {
        apply(Hint::Record(row[0]))?;
    }
    for row in words.rows(2)?.chunks_exact(2) {
        apply(Hint::Helper(row[0]))?;
    }
    for _ in 0..words.word()? {
        let choice = words.word()?;
        let activation = words.word()?;
        if choice > 1 || (choice == 0 && activation != 0) {
            return Err(CandidateError::InvalidRequest);
        }
        let definitions = words.rows(2)?;
        if definitions.is_empty() {
            return Err(CandidateError::InvalidRequest);
        }
        apply(Hint::String {
            shared: choice == 1,
            activation,
            definitions,
        })?;
    }
    for _ in 0..words.word()? {
        let header = words.take(4)?;
        words.take(header[2] as usize)?;
        words.take(
            (header[3] as usize)
                .checked_mul(4)
                .ok_or(CandidateError::InvalidRequest)?,
        )?;
        apply(Hint::Product(header[0]))?;
    }
    for _ in 0..words.word()? {
        let body = words.word()?;
        words.rows(3)?;
        apply(Hint::Function(body))?;
    }
    if !words.0.is_empty() {
        return Err(CandidateError::InvalidRequest);
    }
    Ok(())
}

impl JavaScriptSearch<'_, '_> {
    /// Find already proved evidence or build a fresh complete recipe. Every
    /// temporary checkpoint is discarded, on refusal as well as success.
    pub(super) fn decision_state(
        &mut self,
        decision: &SavedDecision,
        policy: &ResolvedPolicy,
        request: SearchRequest,
    ) -> Result<usize, SearchError> {
        decision.output.check_policy(policy)?;
        decision
            .naming
            .check_policy(policy)
            .map_err(CandidateError::from)?;
        let work = (self.states.len() as u64)
            .checked_mul(decision.recipe.len() as u64)
            .ok_or(AllocationError::Capacity)?;
        self.compilation
            .ledger
            .charge(WorkDomain::Optional, WorkKind::Analysis, work)?;
        if let Some((position, _)) = self.states.iter().enumerate().find(|(_, state)| {
            state.as_ref().is_some_and(|state| {
                state.identity.description().whole_words() == Some(&decision.recipe)
            })
        }) {
            return Ok(position);
        }
        // Validate all persisted hint indices before a proof producer sees
        // them. Other descriptor words are checked by full canonical equality
        // after reconstruction, not used as executable handles.
        let root = self.states[0].as_ref().unwrap().candidate;
        let slot = self.compilation.candidate_slot(root)?;
        let program = &self.compilation.slots[slot]
            .checkpoint
            .as_ref()
            .unwrap()
            .semantic
            .program;
        visit(&decision.recipe, |hint| {
            let valid = match hint {
                Hint::Record(cell) | Hint::Helper(cell) | Hint::Product(cell) => {
                    (cell as usize) < program.cells().len()
                }
                Hint::Function(body) => (body as usize) < program.units().len(),
                Hint::String {
                    shared,
                    activation,
                    definitions,
                } => {
                    (!shared || (activation as usize) < program.units().len())
                        && definitions.chunks_exact(2).all(|row| {
                            UnitId::from_index(row[0] as usize)
                                .and_then(|unit| program.unit(unit))
                                .is_some_and(|unit| (row[1] as usize) < unit.values.len())
                        })
                }
            };
            if valid {
                Ok(())
            } else {
                Err(CandidateError::InvalidRequest)
            }
        })?;
        let mut current = root;
        let outcome = visit(&decision.recipe, |hint| {
            let slot = self.compilation.candidate_slot(current)?;
            let map = self.compilation.slots[slot]
                .checkpoint
                .as_ref()
                .unwrap()
                .implementations
                .as_ref()
                .unwrap();
            let already = match hint {
                Hint::Record(cell) => map
                    .records()
                    .any(|family| family.root().state.index() == cell as usize),
                Hint::Helper(cell) => map
                    .helpers()
                    .any(|family| family.root().cell.index() == cell as usize),
                Hint::Product(cell) => map
                    .products()
                    .any(|family| family.root().index() == cell as usize),
                Hint::Function(body) => map
                    .functions()
                    .any(|family| family.body().index() == body as usize),
                Hint::String { definitions, .. } => map.strings().any(|family| {
                    family.definitions().len() == definitions.len() / 2
                        && family
                            .definitions()
                            .iter()
                            .zip(definitions.chunks_exact(2))
                            .all(|(a, b)| {
                                a.unit.index() == b[0] as usize && a.value.index() == b[1] as usize
                            })
                }),
            };
            if already {
                return Ok(());
            }
            if matches!(hint, Hint::Helper(_) | Hint::String { .. })
                && self.compilation.local_facts.is_none()
            {
                self.compilation
                    .enable_local_facts(request.facts_cache, WorkDomain::Optional)?;
            }
            self.counters.proof_queries += 1;
            let next = match hint {
                Hint::Record(cell) => match self
                    .compilation
                    .scalar_javascript(
                        current,
                        CellId::from_index(cell as usize).unwrap(),
                        request.scalar,
                        policy,
                        WorkDomain::Optional,
                    )?
                    .outcome
                {
                    ScalarOutcome::Published(next) => next,
                    _ => return Err(CandidateError::InvalidRequest),
                },
                Hint::Helper(cell) => match self
                    .compilation
                    .inline_helper_javascript(
                        current,
                        CellId::from_index(cell as usize).unwrap(),
                        request.helper,
                        policy,
                        WorkDomain::Optional,
                    )?
                    .outcome
                {
                    HelperOutcome::Published(next) => next,
                    _ => return Err(CandidateError::InvalidRequest),
                },
                Hint::Product(cell) => match self
                    .compilation
                    .scalar_product_javascript(
                        current,
                        CellId::from_index(cell as usize).unwrap(),
                        request.scalar,
                        policy,
                        WorkDomain::Optional,
                    )?
                    .outcome
                {
                    ProductOutcome::Published(next) => next,
                    _ => return Err(CandidateError::InvalidRequest),
                },
                Hint::Function(body) => match self
                    .compilation
                    .scalar_function_javascript(
                        current,
                        UnitId::from_index(body as usize).unwrap(),
                        request.scalar,
                        policy,
                        WorkDomain::Optional,
                    )?
                    .outcome
                {
                    FunctionOutcome::Published(next) => next,
                    _ => return Err(CandidateError::InvalidRequest),
                },
                Hint::String {
                    shared,
                    activation,
                    definitions,
                } => {
                    let (definitions, charge) = {
                        let mut budget = AllocationBudget::new(Some((
                            &mut self.compilation.ledger,
                            WorkDomain::Optional,
                        )));
                        budget.work(WorkKind::Analysis, (definitions.len() / 2) as u64)?;
                        let mut values = budget.vector(Retained, definitions.len() / 2)?;
                        values.extend(definitions.chunks_exact(2).map(|row| ValueRef {
                            unit: UnitId::from_index(row[0] as usize).unwrap(),
                            value: ValueId::from_index(row[1] as usize).unwrap(),
                        }));
                        let charge = budget
                            .detach_retained(self.owner, bytes::<ValueRef>(values.capacity())?)?;
                        (values, charge)
                    };
                    let choice = if shared {
                        StringChoice::SharedLiteral {
                            activation: UnitId::from_index(activation as usize).unwrap(),
                        }
                    } else {
                        StringChoice::LiteralAtDefinition
                    };
                    let result = self.compilation.represent_string_javascript(
                        current,
                        &definitions,
                        choice,
                        request.string,
                        policy,
                        WorkDomain::Optional,
                    );
                    drop(definitions);
                    charge
                        .discard(&self.owner, &mut self.compilation.ledger)
                        .map_err(|(_, error)| error)?;
                    match result?.outcome {
                        StringOutcome::Published(next) => next,
                        _ => return Err(CandidateError::InvalidRequest),
                    }
                }
            };
            if current != root {
                self.compilation.discard(current.semantic_id())?;
            }
            current = next;
            Ok(())
        })
        .and_then(|()| {
            self.compilation
                .with_recipe_descriptor(current, WorkDomain::Optional, |words| {
                    words == decision.recipe
                })
                .and_then(|same| {
                    if same {
                        Ok(())
                    } else {
                        Err(CandidateError::InvalidRequest)
                    }
                })
        });
        if let Err(error) = outcome {
            if current != root {
                self.compilation.discard(current.semantic_id())?;
            }
            return Err(error.into());
        }
        if current == root {
            return Ok(0);
        }
        if let Err(error) = self.prepare_state_slot(self.states.len().saturating_add(1)) {
            self.compilation.discard(current.semantic_id())?;
            return Err(error);
        }
        self.insert_state(current, 0, WorkDomain::Optional)?
            .ok_or(CandidateError::InvalidRequest.into())
    }
}
