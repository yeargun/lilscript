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
            naming::Eligibility::Search { alphabets: true },
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
    pub(crate) fn delivery_layout(&self) -> Option<delivery::DeliveredLayout> {
        self.module
            .delivery
            .as_ref()
            .map(delivery::DeliveryPlan::layout)
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
                print::PrintError::Container => {
                    OutputError::Invalid("unsupported output container")
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

    /// Every file of the tree's delivery plan (plan M3.3), each separately
    /// charged: one naming of the whole output, then each file printed with
    /// it. When a file name is its content hash, the files print once with
    /// provisional names, hash bottom-up over static imports, and print
    /// again with the final names (design §8).
    #[allow(clippy::type_complexity)]
    pub(crate) fn render_plan_with_literals_admitted<Owner: Eq + Copy>(
        &self,
        plan: &naming::Plan,
        literals: LiteralOutput,
        limit: usize,
        owner: Owner,
    ) -> Result<(Vec<RenderedFile<Owner>>, LiteralOutput), OutputError> {
        if !self.is_accounted() {
            return Err(AllocationError::Unaccounted.into());
        }
        let delivery = self
            .module
            .delivery
            .as_ref()
            .ok_or(OutputError::Invalid("the tree has no delivery plan"))?;
        self.naming.check_in(plan)?;
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
        let mut budget = self.budget.borrow_mut();
        let mut render = budget.scope();
        let result = (|| {
            let names = self.basis.names_in(plan, &mut render)?;
            let print_all = |file_names: &[String], budget: &mut AllocationBudget<'_>| {
                let mut texts = Vec::with_capacity(delivery.files.len());
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
                        print::PrintError::Container => {
                            OutputError::Invalid("unsupported output container")
                        }
                    })?;
                    used += text.len();
                    texts.push(text);
                }
                Ok::<_, OutputError>(texts)
            };
            if !delivery.needs_hash() {
                let file_names = delivery.file_names(None);
                let texts = print_all(&file_names, &mut render)?;
                return Ok((file_names, texts));
            }
            let provisional = delivery.file_names(None);
            let texts = print_all(&provisional, &mut render)?;
            let hashes = content_hashes(delivery, &texts, &mut render)?;
            drop(texts);
            let file_names = delivery.file_names(Some(&hashes));
            let mut sorted = file_names.clone();
            sorted.sort_unstable();
            if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(OutputError::Invalid(
                    "two delivered files get one name: lengthen `[hash:N]`",
                ));
            }
            let texts = print_all(&file_names, &mut render)?;
            Ok((file_names, texts))
        })();
        render.finish_retained()?;
        let (file_names, texts) = result?;
        let mut files = Vec::with_capacity(texts.len());
        for (name, code) in file_names.into_iter().zip(texts) {
            // The printer retained the text; the name is admitted here.
            let bytes = u64::try_from(code.capacity() + name.capacity())
                .map_err(|_| AllocationError::Capacity)?;
            let admitted = u64::try_from(name.capacity())
                .map_err(|_| AllocationError::Capacity)
                .and_then(|name| budget.retain(AllocationClass::Retained, name))
                .and_then(|()| budget.detach_retained(owner, bytes));
            match admitted {
                Ok(charge) => files.push(RenderedFile { name, code, charge }),
                Err(error) => {
                    budget.with_ledger(|ledger| {
                        if let Some((ledger, _)) = ledger {
                            for file in files {
                                let _ = file.charge.discard(&owner, ledger);
                            }
                        }
                    });
                    return Err(error.into());
                }
            }
        }
        Ok((files, literals))
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
            F::Auto | F::Cjs => Err("unresolved or unsupported output container".into()),
            F::Iife | F::Bare if !self.exports.is_empty() || self.delivery.is_some() => Err(
                "a private script container cannot publish exports or a module file plan".into(),
            ),
            _ => Ok(()),
        }
    }
}

/// One delivered file of a plan, with its retained charge.
pub(crate) struct RenderedFile<Owner> {
    pub name: String,
    pub code: String,
    pub charge: RetainedCharge<Owner>,
}

/// Each file's hex SHA-256 over its text printed with provisional names and
/// over the texts of every file it can load, statically, with `import()`
/// or by preloading, in plan order (esbuild's content hash). Any change to a
/// byte a file's name stands for renames it, however far away the change
/// is, and cycles through `import()` need no special case.
fn content_hashes(
    plan: &delivery::DeliveryPlan,
    texts: &[String],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<String>, OutputError> {
    use sha2::{Digest, Sha256};
    let own = texts
        .iter()
        .map(|text| Sha256::digest(text.as_bytes()))
        .collect::<Vec<_>>();
    budget.work(
        WorkKind::Render,
        texts.iter().map(|text| text.len() as u64).sum::<u64>(),
    )?;
    let count = texts.len();
    budget.work(
        WorkKind::Render,
        (count as u64).saturating_mul(count as u64),
    )?;
    let mut hashes = Vec::with_capacity(count);
    for file in 0..count {
        let mut reached = vec![false; count];
        let mut pending = vec![file];
        while let Some(current) = pending.pop() {
            if std::mem::replace(&mut reached[current], true) {
                continue;
            }
            let links = &plan.files[current].links;
            pending.extend(links.imports.iter().map(|&(source, _)| source as usize));
            pending.extend(links.dynamic.iter().map(|&target| target as usize));
            // The preload prelude spells the files it preloads too.
            pending.extend(plan.preloads(current));
        }
        let mut digest = Sha256::new();
        digest.update(own[file]);
        for (other, _) in reached
            .iter()
            .enumerate()
            .filter(|&(other, &seen)| seen && other != file)
        {
            digest.update(own[other]);
        }
        hashes.push(format!("{:x}", digest.finalize()));
    }
    Ok(hashes)
}
