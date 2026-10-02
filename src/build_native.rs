//! Source-qualified native checks share target admission with real builds.
use super::*;
use crate::output_budget::AllocationClass::Retained;

/// Borrow the factory's existing source arena, without copying source texts.
/// The small index is admitted and released with the checked session.
#[derive(Default)]
pub(super) struct NativeSources<'src> { texts: Vec<&'src str>, bytes: u64 }
impl<'src> NativeSources<'src> {
    pub(super) fn prepare(inputs: impl ExactSizeIterator<Item=&'src str>, ledger: &mut BudgetLedger) -> Result<Self, ServiceError> {
        let mut budget = AllocationBudget::new(Some((ledger, WorkDomain::Baseline)));
        let texts = budget.retained_phase(|budget| {
            let mut texts = budget.vector(Retained, inputs.len())?;
            for text in inputs { budget.push(Retained, &mut texts, text)?; }
            Ok::<_, ServiceResourceError>(texts)
        }).map_err(|e| ServiceError::resources("native diagnostic inputs", e))?;
        let bytes = (texts.capacity() * std::mem::size_of::<&str>()) as u64;
        let charge = budget.detach_retained((), bytes).map_err(|e| ServiceError::resources("native diagnostic inputs", e))?;
        let (domain, transferred) = charge.into_parts(&()).unwrap_or_else(|_| unreachable!("same-factory input index"));
        debug_assert_eq!(domain, WorkDomain::Baseline);
        debug_assert_eq!(transferred, bytes);
        Ok(Self { texts, bytes })
    }
    pub(super) fn discard(self, ledger: &mut BudgetLedger) {
        drop(self.texts);
        ledger.release(WorkDomain::Baseline, self.bytes).expect("owned native source index");
    }
}

impl CheckedSourceSession<'_> {
    pub(super) fn native_error(&self, source: SemanticId, error: NativeError) -> ServiceError {
        if let NativeError::Unsupported { feature, .. } = &error {
            if let Some((module, span)) = self.compilation.view(source).ok()
                .and_then(|view| view.native_error_location(&error)) {
                if let Some(text) = self.native_sources.texts.get(module) {
                    let path = self.inputs["modules"][module]["path"].as_str().unwrap_or("<source>");
                    return ServiceError::module("native capability", ModuleError::new(path, *text, span,
                        format!("unsupported native capability: {feature}")));
                }
            }
        }
        ServiceError::new("native", error)
    }
    /// Check the selected native target without rendering, search or C processes.
    /// Callers that compile directly do not pay for a duplicate preflight.
    pub fn check_native(&mut self, source: SemanticId) -> Result<(), ServiceError> {
        let policy = self.native.as_ref().ok_or_else(|| ServiceError::new("native", "not a native session"))?;
        let bindings = self.native_bindings.iter().map(|(cell, link_name)| NativeHostBinding { cell: *cell, link_name }).collect::<Vec<_>>();
        let hosts = NativeHostBindings { callback_abi_version: NativeHostBindings::ABI_VERSION, bindings: &bindings };
        self.compilation.check_native(source, policy, WorkDomain::Baseline, &hosts)
            .map_err(|error| self.native_error(source, error))
    }
}

/// Target-aware source checking. The original check_source/check_path API is
/// JavaScript checking; only a requested native target asserts native support.
pub fn check_source_for_target(source: &str, config: &ProjectConfig, options: ServiceOptions) -> Result<(), ServiceError> {
    with_checked_source(source, config, options, |session| {
        if options.native_request().is_some() { session.check_native(session.source()) } else { Ok(()) }
    })?.0
}
pub fn check_entries_for_target(entries: &[EntrySource], config: &ProjectConfig, options: ServiceOptions) -> Result<(), ServiceError> {
    with_checked_entries(entries, config, options, |session| {
        if options.native_request().is_some() { session.check_native(session.source()) } else { Ok(()) }
    })?.0
}
