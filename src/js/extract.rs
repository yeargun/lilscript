//! Target choices borrow an unchanged program. Its operators survive
//! optimization and extraction; no complete JavaScript program is cloned, and
//! no proof can outlive a mutation of the program it describes.

use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError, RetainedCharge};
use std::cell::RefCell;

/// Accounted failures retain only fixed metadata. Formatting a user diagnostic
/// belongs to the explicit inspection/reporting boundary, not target admission.
#[derive(Debug)]
pub enum OutputError {
    Invalid(&'static str),
    InvalidAt {
        reason: &'static str,
        index: usize,
    },
    Syntax {
        edition: crate::js_syntax_target::EcmaScriptEdition,
        feature: crate::js_syntax_target::JsSyntaxFeature,
    },
    Admission(AllocationError),
    ByteLimit,
}

impl From<AllocationError> for OutputError {
    fn from(error: AllocationError) -> Self {
        Self::Admission(error)
    }
}
impl From<&'static str> for OutputError {
    fn from(reason: &'static str) -> Self {
        Self::Invalid(reason)
    }
}
impl std::fmt::Display for OutputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => formatter.write_str(reason),
            Self::InvalidAt { reason, index } => write!(formatter, "{reason} {index}"),
            Self::Syntax { feature, .. } => write!(
                formatter,
                "{} requires javascript.ecmascript {} or newer",
                feature.construct(),
                feature.min_edition().name(),
            ),
            Self::Admission(error) => write!(formatter, "output admission failed: {error:?}"),
            Self::ByteLimit => formatter.write_str("output exceeds its byte limit"),
        }
    }
}
impl std::error::Error for OutputError {}

/// One prepared output boundary shares its naming constraints, verified
/// structure and target facts across every candidate render.
pub struct Output<'a> {
    module: &'a Module,
    format: crate::config::JavaScriptFormat,
    pub(super) basis: naming::Basis<'a>,
    naming: naming::Eligibility,
    literal_alternatives: &'a [LiteralAlternative],
    has_literal_alternative: bool,
    permits_observed_literals: bool,
    /// Host modules the output carries, and whether they run strict.
    hosts: Option<(&'a crate::host_modules::HostDelivery, bool)>,
    // Basis (including installed lazy caches) drops before its reservation owner.
    budget: RefCell<AllocationBudget<'a>>,
}

impl<'a> Output<'a> {
    /// Explicit policy-free inspection route, sharing the production algorithms.
    pub(super) fn from_module(module: &'a Module) -> Result<Self, String> {
        Self::from_module_in(
            module,
            naming::Eligibility::Search {
                alphabets: true,
                compact: true,
            },
            None,
            AllocationBudget::new(None),
        )
        .map_err(|error| error.to_string())
    }

    fn from_module_in(
        module: &'a Module,
        naming: naming::Eligibility,
        edition: Option<crate::js_syntax_target::EcmaScriptEdition>,
        budget: AllocationBudget<'a>,
    ) -> Result<Self, OutputError> {
        Self::from_module_with_literals_in(module, naming, edition, &[], false, budget)
    }

    fn from_module_with_literals_in(
        module: &'a Module,
        naming: naming::Eligibility,
        edition: Option<crate::js_syntax_target::EcmaScriptEdition>,
        literal_alternatives: &'a [LiteralAlternative],
        permits_observed_literals: bool,
        mut budget: AllocationBudget<'a>,
    ) -> Result<Self, OutputError> {
        let (basis, has_literal_alternative) = {
            let mut prepare = budget.scope();
            let structure = verify::verify_in(module, &mut prepare)?;
            if let Some(edition) = edition {
                verify::verify_edition_in(module, &structure, edition, &mut prepare)?;
            }
            let has_literal_alternative =
                literal_output::validate(module, literal_alternatives, &structure, &mut prepare)?;
            let basis = naming::Basis::new_in(module, &structure, &mut prepare)?;
            drop(structure);
            prepare.finish_retained()?;
            (basis, has_literal_alternative)
        };
        Ok(Self {
            module,
            format: crate::config::JavaScriptFormat::Bare,
            basis,
            naming,
            literal_alternatives,
            has_literal_alternative,
            permits_observed_literals,
            hosts: None,
            budget: RefCell::new(budget),
        })
    }

    /// Carry these host modules in every render; `strict` for a script.
    pub(crate) fn set_hosts(
        &mut self,
        hosts: Option<(&'a crate::host_modules::HostDelivery, bool)>,
    ) {
        self.hosts = hosts;
    }

    /// The layout of the tree's delivery plan, when the output is several
    /// files (plan M3.3).
    pub(crate) fn has_delivery_plan(&self) -> bool {
        self.module.delivery.is_some()
    }

    pub(crate) fn is_accounted(&self) -> bool {
        self.budget.borrow().is_accounted()
    }

    /// The structural digest of the single file `render_with_literals_admitted`
    /// prints, for the admission parse (plan task M2.5). A bundle's files are
    /// printed by `render_file_admitted` and are parsed without one.
    pub(crate) fn structure_digest(
        &self,
    ) -> Result<crate::admission_parse::StructureDigest, OutputError> {
        let _timing = crate::timing::ADMISSION_STRUCTURE.scope(0);
        let mut budget = self.budget.borrow_mut();
        let nodes = self.module.expressions.len() + self.module.regions.len();
        budget.work(WorkKind::Analysis, nodes as u64)?;
        Ok(super::admission::digest(
            self.module,
            self.hosts,
            self.format,
        ))
    }

    /// Coordinate the compilation's private artifact owner without exposing the
    /// output budget or its underlying ledger to an external caller.
    pub(crate) fn with_allocation_budget<R>(
        &self,
        inspect: impl FnOnce(&mut AllocationBudget<'_>) -> R,
    ) -> R {
        inspect(&mut self.budget.borrow_mut())
    }

    /// Inspection returns owned bytes at its explicit, unaccounted boundary.
    /// Production must instead transfer admission with the bytes to its store.
    pub fn render(&self, plan: &naming::Plan) -> Result<String, String> {
        self.render_bounded(plan, usize::MAX)
    }

    pub(super) fn render_bounded(
        &self,
        plan: &naming::Plan,
        limit: usize,
    ) -> Result<String, String> {
        if self.is_accounted() {
            return Err("accounted output requires the compilation artifact owner".into());
        }
        self.render_scoped(plan, LiteralOutput::Original, limit)
            .map_err(|error| error.to_string())
    }

    fn render_scoped(
        &self,
        plan: &naming::Plan,
        literals: LiteralOutput,
        limit: usize,
    ) -> Result<String, OutputError> {
        self.naming.check_in(plan)?;
        let mut budget = self.budget.borrow_mut();
        let mut render = budget.scope();
        let result = (|| {
            let names = self.basis.names_in(plan, &mut render)?;
            print::render_with_literals_admitted(
                self.module,
                &names,
                self.literal_alternatives,
                literals,
                limit,
                &mut render,
                self.hosts,
                self.format,
            )
            .map_err(|error| match error {
                print::PrintError::Admission(error) => OutputError::Admission(error),
                print::PrintError::ByteLimit => OutputError::ByteLimit,
                print::PrintError::Container(reason) => {
                    OutputError::Invalid(reason)
                }
            })
        })();
        // Lazy Basis caches already installed by this render stay live even if
        // naming or printing failed. Printer rolls back its own partial bytes;
        // Names and other scratch have dropped before this commit.
        render.finish_retained()?;
        result
    }

    pub(crate) fn render_admitted<Owner: Eq>(
        &self,
        plan: &naming::Plan,
        limit: usize,
        owner: Owner,
    ) -> Result<(String, RetainedCharge<Owner>), OutputError> {
        self.render_with_literals_admitted(plan, LiteralOutput::Original, limit, owner)
            .map(|(text, charge, _)| (text, charge))
    }

    pub(crate) fn has_literal_alternative_admitted(&self) -> Result<bool, OutputError> {
        self.budget.borrow_mut().work(WorkKind::Analysis, 1)?;
        Ok(self.has_literal_alternative && self.permits_observed_literals)
    }

    pub(crate) fn render_with_literals_admitted<Owner: Eq>(
        &self,
        plan: &naming::Plan,
        literals: LiteralOutput,
        limit: usize,
        owner: Owner,
    ) -> Result<(String, RetainedCharge<Owner>, LiteralOutput), OutputError> {
        if !self.is_accounted() {
            return Err(AllocationError::Unaccounted.into());
        }
        let literals = {
            let mut budget = self.budget.borrow_mut();
            budget.work(WorkKind::Analysis, 1)?;
            if literals == LiteralOutput::Observed && !self.permits_observed_literals {
                return Err(OutputError::Invalid(
                    "observed literal output requires target-compaction permission",
                ));
            }
            if self.has_literal_alternative {
                literals
            } else {
                LiteralOutput::Original
            }
        };
        let text = self.render_scoped(plan, literals, limit)?;
        let bytes = u64::try_from(text.capacity()).map_err(|_| AllocationError::Capacity)?;
        let mut budget = self.budget.borrow_mut();
        // The last print/copy segment may finish after the deadline. Reject
        // before publishing bytes, while keeping already installed Basis caches
        // owned. Cleanup and successful ownership commits do not check time.
        if let Err(error) = budget.work(WorkKind::Render, 0) {
            drop(text);
            budget.release(AllocationClass::Retained, bytes)?;
            return Err(error.into());
        }
        let charge = budget.detach_retained(owner, bytes)?;
        Ok((text, charge, literals))
    }

    /// Every file of the tree's delivery plan (plan M3.3): one admitted
    /// bundle, one naming of the whole output, then each file printed with
    /// it. When a file name is its content hash, the files print once with
    /// provisional names, hash bottom-up over static imports, and print
    /// again with the final names (design §8).
    /// `file` moves name/code into an inline artifact record without allocating
    /// additional payload. The single returned charge owns the complete bundle.
    #[allow(clippy::type_complexity)]
    pub(crate) fn render_plan_with_literals_admitted<Owner: Eq + Copy, T>(
        &self,
        plan: &naming::Plan,
        literals: LiteralOutput,
        limit: usize,
        owner: Owner,
        mut file: impl FnMut(String, String, print::PlannedStructure) -> T,
    ) -> Result<
        (
            Vec<T>,
            delivery::DeliveredLayout,
            RetainedCharge<Owner>,
            LiteralOutput,
        ),
        OutputError,
    > {
        use AllocationClass::{Retained, Scratch};
        if !self.is_accounted() {
            return Err(AllocationError::Unaccounted.into());
        }
        let delivery = self
            .module
            .delivery
            .as_ref()
            .ok_or(OutputError::Invalid("the tree has no delivery plan"))?;
        self.naming.check_in(plan)?;
        let mut budget = self.budget.borrow_mut();
        budget.work(WorkKind::Analysis, 1)?;
        if literals == LiteralOutput::Observed && !self.permits_observed_literals {
            return Err(OutputError::Invalid(
                "observed literal output requires target-compaction permission",
            ));
        }
        let literals = if self.has_literal_alternative {
            literals
        } else {
            LiteralOutput::Original
        };
        let mut render = budget.scope();
        let result = (|| {
            let names = self.basis.names_in(plan, &mut render)?;
            // Keep installed naming caches, but roll all incomplete bundle
            // storage back before this preparation returns an error.
            render.retained_phase(|render| {
                let print_all = |file_names: &[String],
                                 budget: &mut AllocationBudget<'_>|
                 -> Result<Vec<print::PlannedText>, OutputError> {
                    let mut texts = budget.vector(Retained, delivery.files.len())?;
                    let mut used = 0usize;
                    for file in 0..delivery.files.len() {
                        let text = print::render_planned_file_admitted(
                            self.module,
                            &names,
                            self.literal_alternatives,
                            literals,
                            limit.saturating_sub(used),
                            budget,
                            &print::PlannedPrint {
                                plan: delivery,
                                file,
                                names: file_names,
                            },
                            self.hosts,
                        )
                        .map_err(|error| match error {
                            print::PrintError::Admission(error) => OutputError::Admission(error),
                            print::PrintError::ByteLimit => OutputError::ByteLimit,
                            print::PrintError::Container(reason) => {
                                OutputError::Invalid(reason)
                            }
                        })?;
                        used = used
                            .checked_add(text.len())
                            .ok_or(AllocationError::Capacity)?;
                        texts.push(text);
                    }
                    Ok(texts)
                };
                let file_names = if !delivery.needs_hash() {
                    delivery.file_names_in(None, Retained, render)?
                } else {
                    render.with_temporary(
                        |budget| {
                            let provisional = delivery.file_names_in(None, Scratch, budget)?;
                            let texts = print_all(&provisional, budget)?;
                            let hashes = content_hashes(delivery, &texts, budget)?;
                            drop_planned_texts(texts, Retained, budget)?;
                            drop_strings(provisional, Scratch, budget)?;
                            Ok::<_, OutputError>(hashes)
                        },
                        |hashes, budget| {
                            Ok::<_, OutputError>(delivery.file_names_in(
                                Some(hashes),
                                Retained,
                                budget,
                            )?)
                        },
                    )?
                };
                let mut sorted = render.vector(Scratch, file_names.len())?;
                sorted.extend(file_names.iter().map(String::as_str));
                sorted.sort_unstable();
                if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
                    return Err(OutputError::Invalid(
                        "two delivered files get one name: lengthen `[hash:N]`",
                    ));
                }
                drop_vec(sorted, Scratch, render)?;
                let texts = print_all(&file_names, render)?;
                let layout = delivery.layout_in(render)?;
                let containers = crate::output_budget::vector_bytes(&file_names)?
                    .checked_add(crate::output_budget::vector_bytes(&texts)?)
                    .ok_or(AllocationError::Capacity)?;
                let mut files = render.vector(Retained, texts.len())?;
                let mut bytes = layout
                    .heap_bytes()?
                    .checked_add(crate::output_budget::vector_bytes(&files)?)
                    .ok_or(AllocationError::Capacity)?;
                for (name, code) in file_names.into_iter().zip(texts) {
                    bytes = bytes
                        .checked_add(name.capacity() as u64)
                        .and_then(|sum| sum.checked_add(code.code.capacity() as u64))
                        .ok_or(AllocationError::Capacity)?;
                    files.push(file(name, code.code, code.structure));
                }
                render.release(Retained, containers)?;
                render.work(WorkKind::Render, 0)?;
                Ok((files, layout, bytes))
            })
        })();
        render.finish_retained()?;
        let (files, layout, bytes) = result?;
        let charge = budget.detach_retained(owner, bytes)?;
        Ok((files, layout, charge, literals))
    }

    pub(crate) fn source_candidates_admitted(&self) -> Result<&[BindingId], OutputError> {
        self.basis
            .source_candidates_in(&mut self.budget.borrow_mut())
    }
}

impl Module {
    fn check_import_execution(
        &self,
        policy: &crate::compilation_policy::ResolvedPolicy,
    ) -> Result<(), OutputError> {
        let imported = self.imports.iter().any(|import| {
            !import
                .source
                .as_unicode()
                .is_some_and(|source| self.carried.iter().any(|carried| carried == source))
        });
        if imported
            && policy.delivery().is_none_or(|delivery| delivery.format != crate::config::JavaScriptFormat::Cjs)
            && policy.javascript_contract().is_some_and(|contract| {
                contract.execution != crate::compilation_contract::JavaScriptExecution::Module
            })
        {
            return Err("static imports require ECMAScript module execution".into());
        }
        Ok(())
    }

    /// Explicit inspection wrapper; permissions are copied into prepared output.
    pub fn prepare_output_with_policy(
        &self,
        policy: &crate::compilation_policy::ResolvedPolicy,
    ) -> Result<Output<'_>, String> {
        self.check_import_execution(policy)
            .map_err(|error| error.to_string())?;
        let naming =
            naming::Eligibility::from_policy_in(policy).map_err(|error| error.to_string())?;
        let edition = policy
            .javascript_contract()
            .expect("naming checked the target")
            .ecmascript;
        let mut output =
            Output::from_module_in(self, naming, Some(edition), AllocationBudget::new(None))
                .map_err(|error| error.to_string())?;
        output.format = policy.delivery().expect("JavaScript delivery").format;
        self.check_container(output.format)
            .map_err(|error| error.to_string())?;
        Ok(output)
    }

    /// Production retains verifier/naming storage under the compilation's one
    /// allocation owner. No bare owned artifact can escape this prepared view.
    pub(crate) fn prepare_output_admitted<'a>(
        &'a self,
        policy: &crate::compilation_policy::ResolvedPolicy,
        parent: &'a mut AllocationBudget<'_>,
    ) -> Result<Output<'a>, OutputError> {
        self.prepare_output_in(policy, &[], parent)
    }

    /// `prepare_output_admitted`, with the module's observed literals.
    pub(crate) fn prepare_output_with_literals_admitted<'a>(
        &'a self,
        policy: &crate::compilation_policy::ResolvedPolicy,
        parent: &'a mut AllocationBudget<'_>,
    ) -> Result<Output<'a>, OutputError> {
        self.prepare_output_in(policy, &self.observed_literals, parent)
    }

    /// `prepare_output_admitted` with rows a test supplies, valid or not.
    #[cfg(test)]
    pub(crate) fn prepare_output_with_rows_admitted<'a>(
        &'a self,
        policy: &crate::compilation_policy::ResolvedPolicy,
        rows: &'a [LiteralAlternative],
        parent: &'a mut AllocationBudget<'_>,
    ) -> Result<Output<'a>, OutputError> {
        self.prepare_output_in(policy, rows, parent)
    }

    fn prepare_output_in<'a>(
        &'a self,
        policy: &crate::compilation_policy::ResolvedPolicy,
        rows: &'a [LiteralAlternative],
        parent: &'a mut AllocationBudget<'_>,
    ) -> Result<Output<'a>, OutputError> {
        self.check_import_execution(policy)?;
        let naming = naming::Eligibility::from_policy_in(policy)?;
        let edition = policy
            .javascript_contract()
            .expect("naming checked the target")
            .ecmascript;
        let format = policy.delivery().expect("JavaScript delivery").format;
        self.check_container(format)?;
        let mut output = Output::from_module_with_literals_in(
            self,
            naming,
            Some(edition),
            rows,
            policy
                .tactic(crate::compilation_policy::TacticId::TargetCompaction)
                .enabled,
            parent.scope(),
        )?;
        output.format = format;
        Ok(output)
    }

    fn check_container(&self, format: crate::config::JavaScriptFormat) -> Result<(), OutputError> {
        use crate::config::JavaScriptFormat as F;
        match format {
            F::Auto => Err("unresolved output container".into()),
            F::Bare if !self.exports.is_empty() || self.delivery.is_some() => Err(
                "a private script container cannot publish exports or a module file plan".into(),
            ),
            _ => Ok(()),
        }
    }
}

fn drop_vec<T>(
    values: Vec<T>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let bytes = crate::output_budget::vector_bytes(&values)?;
    drop(values);
    budget.release(class, bytes)
}
fn drop_strings(
    values: Vec<String>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let bytes =
        values
            .iter()
            .try_fold(crate::output_budget::vector_bytes(&values)?, |sum, text| {
                sum.checked_add(text.capacity() as u64)
                    .ok_or(AllocationError::Capacity)
            })?;
    drop(values);
    budget.release(class, bytes)
}

fn drop_planned_texts(values:Vec<print::PlannedText>,class:AllocationClass,budget:&mut AllocationBudget<'_>) -> Result<(),AllocationError> {
    let bytes=crate::output_budget::vector_bytes(&values)?;
    for value in values {let capacity=value.code.capacity() as u64;drop(value);budget.release(class,capacity)?;}
    budget.release(class,bytes)
}

/// Each file's hex SHA-256 over its text printed with provisional names and
/// over the texts of every file it can load, statically, with `import()`
/// or by preloading, in plan order (esbuild's content hash). Any change to a
/// byte a file's name stands for renames it, however far away the change
/// is, and cycles through `import()` need no special case.
fn content_hashes(
    plan: &delivery::DeliveryPlan,
    texts: &[print::PlannedText],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<String>, OutputError> {
    use sha2::{Digest, Sha256};
    use AllocationClass::{Retained, Scratch};
    budget.retained_phase(|budget| {
        let mut own = budget.vector(Scratch, texts.len())?;
        for text in texts {
            budget.work(WorkKind::Render, text.len() as u64)?;
            own.push(Sha256::digest(text.as_bytes()));
        }
        let count = texts.len();
        let mut hashes = budget.vector(Retained, count)?;
        let mut reached = budget.filled(Scratch, count, false)?;
        let mut pending = Vec::new();
        for file in 0..count {
            budget.work(WorkKind::Render, count as u64)?;
            reached.fill(false);
            pending.clear();
            budget.push(Scratch, &mut pending, file)?;
            while let Some(current) = pending.pop() {
                budget.work(WorkKind::Render, 1)?;
                if std::mem::replace(&mut reached[current], true) {
                    continue;
                }
                let links = &plan.files[current].links;
                for target in links
                    .imports
                    .iter()
                    .map(|&(source, _)| source)
                    .chain(links.dynamic.iter().copied())
                {
                    budget.push(Scratch, &mut pending, target as usize)?;
                }
                let preloads = plan.preloads_in(current, Scratch, budget)?;
                budget.extend_copy(Scratch, &mut pending, &preloads)?;
                drop_vec(preloads, Scratch, budget)?;
            }
            let mut digest = Sha256::new();
            digest.update(own[file]);
            for (other, _) in reached
                .iter()
                .enumerate()
                .filter(|&(other, &seen)| seen && other != file)
            {
                budget.work(WorkKind::Render, own[other].len() as u64)?;
                digest.update(own[other]);
            }
            hashes.push(budget.format(Retained, format_args!("{:x}", digest.finalize()))?);
        }
        Ok(hashes)
    })
}
